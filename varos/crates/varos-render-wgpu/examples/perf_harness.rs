//! P11 CPU performance harness.
//! Run from `varos/`: cargo run -p varos-render-wgpu --release --example perf_harness -- -j 4

use std::hint::black_box;
use std::time::{Duration, Instant};
use varos_core::editor::{Editor, ToolKind};
use varos_core::geom::View;
use varos_core::model::{Anchor, Path};
use varos_core::scene::build_scene;
use varos_render_wgpu::perf::profile_content;

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
    ]
}

fn median(values: &mut [Duration]) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn main() {
    println!("P11 headless CPU harness: {RUNS} measured runs, median, 1920x1080");
    for case in cases() {
        let _ = profile_content(&build_scene(&case.editor, case.view.zoom), case.view, WIDTH, HEIGHT);
        let mut scene_times = Vec::with_capacity(RUNS);
        let mut content_times = Vec::with_capacity(RUNS);
        let mut frame_times = Vec::with_capacity(RUNS);
        let mut counts = (0, 0, 0, 0);
        for _ in 0..RUNS {
            let frame_start = Instant::now();
            let scene_start = Instant::now();
            let scene = black_box(build_scene(black_box(&case.editor), case.view.zoom));
            scene_times.push(scene_start.elapsed());
            let profile = profile_content(&scene, case.view, WIDTH, HEIGHT);
            content_times.push(profile.elapsed);
            counts =
                (profile.fill_vertices, profile.foreground_vertices, profile.opacity_vertices, profile.draw_groups);
            black_box(&scene);
            frame_times.push(frame_start.elapsed());
        }
        println!(
            "{:<34} scene={:>8.3}ms content={:>8.3}ms cpu_frame={:>8.3}ms vertices={}/{}/{} groups={}",
            case.name,
            median(&mut scene_times).as_secs_f64() * 1_000.0,
            median(&mut content_times).as_secs_f64() * 1_000.0,
            median(&mut frame_times).as_secs_f64() * 1_000.0,
            counts.0,
            counts.1,
            counts.2,
            counts.3,
        );
    }
}
