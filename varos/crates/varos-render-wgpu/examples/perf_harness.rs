//! P11 CPU performance harness.
//! Run from `varos/`: cargo run -p varos-render-wgpu --release --example perf_harness -- -j 4

use std::hint::black_box;
use std::time::{Duration, Instant};
use varos_core::editor::{Editor, ToolKind};
use varos_core::geom::View;
use varos_core::model::{Anchor, Path};
use varos_core::scene::{build_scene_in_view, scene_signature};
use varos_render_wgpu::perf::{profile_content, profile_overlay};

const WIDTH: f32 = 1920.0;
const HEIGHT: f32 = 1080.0;
const RUNS: usize = 15;

struct Case {
    name: &'static str,
    editor: Editor,
    view: View,
}

fn curved_path(id: u32, anchor_base: u32, center: [f32; 2], radius: f32, count: usize) -> Path {
    let step = std::f32::consts::TAU / count as f32;
    let handle = radius * (4.0 / 3.0) * (step * 0.25).tan();
    let anchors = (0..count)
        .map(|i| {
            let angle = i as f32 * step;
            let (sin, cos) = angle.sin_cos();
            let p = [center[0] + cos * radius, center[1] + sin * radius];
            let tangent = [-sin, cos];
            Anchor {
                id: anchor_base + i as u32,
                p,
                hin: Some([p[0] - tangent[0] * handle, p[1] - tangent[1] * handle]),
                hout: Some([p[0] + tangent[0] * handle, p[1] + tangent[1] * handle]),
                smooth: true,
            }
        })
        .collect();
    Path::new(id, anchors, true, Some([0.18, 0.55, 0.86, 1.0]), Some([0.04, 0.04, 0.05, 1.0]), 2.0)
}

fn rectangle(id: u32, anchor_base: u32, x: f32, y: f32, w: f32, h: f32) -> Path {
    let anchors = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
        .into_iter()
        .enumerate()
        .map(|(i, p)| Anchor { id: anchor_base + i as u32, p, hin: None, hout: None, smooth: false })
        .collect();
    Path::new(id, anchors, true, Some([0.72, 0.34, 0.20, 1.0]), Some([0.08, 0.08, 0.09, 1.0]), 1.5)
}

fn finish(mut editor: Editor) -> Editor {
    editor.doc.sync_tree();
    editor.doc.ids = editor
        .doc
        .paths
        .iter()
        .flat_map(|path| std::iter::once(path.id).chain(path.anchors.iter().map(|anchor| anchor.id)))
        .max()
        .unwrap_or(editor.doc.ids);
    editor
}

fn cases() -> Vec<Case> {
    let mut single = Editor::new();
    let path = curved_path(10, 100, [320.0, 220.0], 170.0, 150);
    single.objsel.insert(path.id);
    single.selected.extend(path.anchors.iter().map(|anchor| anchor.id));
    single.tool = ToolKind::Direct;
    single.doc.paths.push(path);

    let mut extreme = Editor::new();
    let path = curved_path(10, 100, [320.0, 220.0], 170.0, 150);
    extreme.objsel.insert(path.id);
    extreme.selected.extend(path.anchors.iter().map(|anchor| anchor.id));
    extreme.tool = ToolKind::Direct;
    extreme.doc.paths.push(path);

    let mut rectangles = Editor::new();
    for i in 0..500u32 {
        let col = i % 25;
        let row = i / 25;
        rectangles.doc.paths.push(rectangle(
            10_000 + i,
            20_000 + i * 4,
            col as f32 * 34.0,
            row as f32 * 34.0,
            26.0,
            26.0,
        ));
    }

    let mut curves = Editor::new();
    for i in 0..100u32 {
        let col = i % 10;
        let row = i / 10;
        curves.doc.paths.push(curved_path(
            40_000 + i,
            50_000 + i * 12,
            [60.0 + col as f32 * 90.0, 60.0 + row as f32 * 90.0],
            32.0,
            12,
        ));
    }

    let mut rect_partial = Editor::new();
    rect_partial.doc.paths = rectangles.doc.paths.clone();

    vec![
        Case {
            name: "A curved-150 selected ppu=3.0",
            editor: finish(single),
            view: View { pan: [0.0, 0.0], zoom: 3.0 },
        },
        Case {
            name: "B rectangles-500 ppu=0.3",
            editor: finish(rectangles),
            view: View { pan: [0.0, 0.0], zoom: 0.3 },
        },
        Case { name: "C curves-100 ppu=1.0", editor: finish(curves), view: View::identity() },
        // P11.2 symptom (d): 4000% zoom on the selected 150-anchor path, the view centred on its right
        // edge so only a sliver of the outline (and a couple of handles) is on screen.
        Case {
            name: "D curved-150 selected ppu=40",
            editor: finish(extreme),
            view: View { pan: [WIDTH * 0.5 - 490.0 * 40.0, HEIGHT * 0.5 - 220.0 * 40.0], zoom: 40.0 },
        },
        // P11.2 many-objects culling: scene B's 500 rectangles at 400%, so only one corner is on screen.
        Case {
            name: "E rectangles-500 ppu=4.0 partial",
            editor: finish(rect_partial),
            view: View { pan: [0.0, 0.0], zoom: 4.0 },
        },
    ]
}

fn median(values: &mut [Duration]) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

struct Frame {
    scene: Duration,
    content: Duration,
    overlay: Duration,
    total: Duration,
    counts: (usize, usize, usize, usize),
    overlay_vertices: usize,
}

/// One full CPU canvas frame, exactly as the app builds it (P11.2: view-culled scene).
fn frame(case: &Case) -> Frame {
    let frame_start = Instant::now();
    let scene_start = Instant::now();
    let scene = black_box(build_scene_in_view(black_box(&case.editor), case.view, [WIDTH as u32, HEIGHT as u32]));
    let scene_elapsed = scene_start.elapsed();
    let profile = profile_content(&scene, case.view, WIDTH, HEIGHT);
    let (overlay, overlay_vertices) = profile_overlay(&scene.overlay, case.view, WIDTH, HEIGHT);
    black_box(&scene);
    Frame {
        scene: scene_elapsed,
        content: profile.elapsed,
        overlay,
        total: frame_start.elapsed(),
        counts: (profile.fill_vertices, profile.foreground_vertices, profile.opacity_vertices, profile.draw_groups),
        overlay_vertices,
    }
}

fn ms(values: &mut [Duration]) -> f64 {
    median(values).as_secs_f64() * 1_000.0
}

fn main() {
    println!("P11 headless CPU harness: {RUNS} measured runs, median, 1920x1080");
    for case in cases() {
        let _ = frame(&case); // warm-up
        let (mut scene_cold, mut scene_warm) = (Vec::with_capacity(RUNS), Vec::with_capacity(RUNS));
        let (mut content_times, mut overlay_times) = (Vec::with_capacity(RUNS), Vec::with_capacity(RUNS));
        let (mut cold_times, mut warm_times) = (Vec::with_capacity(RUNS), Vec::with_capacity(RUNS));
        let mut cache_hit_times = Vec::with_capacity(RUNS);
        let mut counts = (0, 0, 0, 0);
        let mut overlay_vertices = 0;
        let expected_signature = scene_signature(&case.editor, case.view, [WIDTH as u32, HEIGHT as u32]);
        for _ in 0..RUNS {
            // cold: the flatten cache is empty (first frame after open / a zoom-bucket change)
            case.editor.flatten_cache.lock().clear();
            let cold = frame(&case);
            scene_cold.push(cold.scene);
            content_times.push(cold.content);
            overlay_times.push(cold.overlay);
            cold_times.push(cold.total);
            counts = cold.counts;
            overlay_vertices = cold.overlay_vertices;

            // warm: the scene signature missed (pan, hover, selection, an edit elsewhere) but every
            // unchanged path's flatten is reused from the cache
            let warm = frame(&case);
            assert_eq!(warm.counts, cold.counts, "warm frame must emit exactly the cold geometry");
            scene_warm.push(warm.scene);
            warm_times.push(warm.total);

            let hit_start = Instant::now();
            let signature =
                black_box(scene_signature(black_box(&case.editor), case.view, [WIDTH as u32, HEIGHT as u32]));
            assert_eq!(signature, expected_signature);
            cache_hit_times.push(hit_start.elapsed());
        }
        println!(
            "{:<34} scene_cold={:>7.3}ms scene_warm={:>7.3}ms content={:>7.3}ms overlay={:>7.3}ms \
             cold={:>7.3}ms warm={:>7.3}ms hit={:>7.3}ms vertices={}/{}/{} overlay_vertices={} groups={}",
            case.name,
            ms(&mut scene_cold),
            ms(&mut scene_warm),
            ms(&mut content_times),
            ms(&mut overlay_times),
            ms(&mut cold_times),
            ms(&mut warm_times),
            ms(&mut cache_hit_times),
            counts.0,
            counts.1,
            counts.2,
            overlay_vertices,
            counts.3,
        );
    }
}
