//! CPU-only round-3 regressions. All coverage predicates use f64 on the emitted f32 NDC mesh.
use super::*;
use varos_core::editor::Editor;
use varos_core::model::{Anchor, Path};

fn circle_w(rad: f32, width: f32) -> Path {
    // Dense stress fixture: 64 smooth spans of a radius-2000 circle, each adaptively flattened.
    let n = 64;
    let k = 4.0 / 3.0 * (std::f32::consts::PI / (2.0 * n as f32)).tan() * rad;
    let anchors = (0..n)
        .map(|i| {
            let angle = i as f32 / n as f32 * std::f32::consts::TAU;
            let p = [angle.cos() * rad, angle.sin() * rad];
            let t = [-angle.sin() * k, angle.cos() * k];
            Anchor {
                id: 100 + i as u32,
                p,
                hin: Some([p[0] - t[0], p[1] - t[1]]),
                hout: Some([p[0] + t[0], p[1] + t[1]]),
                smooth: true,
            }
        })
        .collect();
    Path::new(10, anchors, true, None, Some([0.0, 0.0, 1.0, 1.0]), width)
}
fn blob() -> Path {
    let radii = [240.0f32, 190.0, 225.0, 170.0, 235.0, 185.0, 210.0];
    let n = radii.len();
    let pts: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            [a.cos() * radii[i], a.sin() * radii[i] * 0.83]
        })
        .collect();
    let anchors = (0..n)
        .map(|i| {
            let (p, prev, next) = (pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]);
            let t = [(next[0] - prev[0]) / 6.0, (next[1] - prev[1]) / 6.0];
            Anchor {
                id: 100 + i as u32,
                p,
                hin: Some([p[0] - t[0], p[1] - t[1]]),
                hout: Some([p[0] + t[0], p[1] + t[1]]),
                smooth: true,
            }
        })
        .collect();
    Path::new(10, anchors, true, None, Some([0.0, 0.0, 1.0, 1.0]), 80.0)
}

type Dpt = [f64; 2];
fn screen(p: Pt, view: View) -> Dpt {
    [p[0] as f64 * view.zoom as f64 + view.pan[0] as f64, p[1] as f64 * view.zoom as f64 + view.pan[1] as f64]
}
fn contains(p: Dpt, t: &[Dpt; 3]) -> bool {
    let cross = |a: Dpt, b: Dpt, c: Dpt| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if cross(t[0], t[1], t[2]) == 0.0 {
        return false;
    }
    let ds = [cross(t[0], t[1], p), cross(t[1], t[2], p), cross(t[2], t[0], p)];
    !(ds.iter().any(|d| *d < 0.0) && ds.iter().any(|d| *d > 0.0))
}
fn triangles(verts: &[Vertex], w: f32, h: f32) -> Vec<[Dpt; 3]> {
    verts
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            std::array::from_fn(|i| {
                [(t[i].pos[0] as f64 + 1.0) * w as f64 / 2.0, (1.0 - t[i].pos[1] as f64) * h as f64 / 2.0]
            })
        })
        .collect()
}
// Sample every retained interior vertex, including the padded area outside the small viewport.
// The bisector alone misses T-junction cracks: also sample both radial fan/quad seams.
// Each direction is checked at 25/50/75/95% of the half-width, with no coverage epsilon.
fn measure(path: Path, view: View) -> (usize, usize) {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.doc.paths.push(path);
    ed.doc.ids = 10_000;
    ed.doc.sync_tree();
    let scene = varos_core::scene::build_scene_in_view(&ed, view, [1600, 1000]);
    let (_, verts, _, _) = build_content(&scene.content, view, view.zoom, 1600.0, 1000.0);
    let tris = triangles(&verts, 1600.0, 1000.0);
    let (mut checked, mut bad) = (0, 0);
    for prim in scene.content.iter().flat_map(|g| g.prims()) {
        let Prim::Stroke { pts, width, .. } = prim else { continue };
        let r = *width as f64 * view.zoom as f64 * 0.5;
        for win in pts.windows(3) {
            let [a, b, c] = [screen(win[0], view), screen(win[1], view), screen(win[2], view)];
            let normal = |a: Dpt, b: Dpt| {
                let d = [b[0] - a[0], b[1] - a[1]];
                let l = d[0].hypot(d[1]);
                [-d[1] / l, d[0] / l]
            };
            if a == b || b == c {
                continue;
            }
            let (n0, n1) = (normal(a, b), normal(b, c));
            let cross = n0[0] * n1[1] - n0[1] * n1[0];
            let side = if cross > 0.0 { -1.0 } else { 1.0 };
            let bis = [n0[0] + n1[0], n0[1] + n1[1]];
            let len = bis[0].hypot(bis[1]);
            if len == 0.0 {
                continue;
            }
            for dir in [[bis[0] / len, bis[1] / len], n0, n1] {
                for f in [0.25, 0.5, 0.75, 0.95] {
                    let q = [b[0] + side * dir[0] * r * f, b[1] + side * dir[1] * r * f];
                    checked += 1;
                    if !tris.iter().any(|t| contains(q, t)) {
                        bad += 1;
                    }
                }
            }
        }
    }
    (checked, bad)
}
fn translated(mut p: Path, center: Pt) -> Path {
    for a in &mut p.anchors {
        let shift = |p: Pt| [p[0] + center[0], p[1] + center[1]];
        a.p = [a.p[0] + center[0], a.p[1] + center[1]];
        a.hin = a.hin.map(shift);
        a.hout = a.hout.map(shift);
    }
    p
}
#[test]
fn round3_circle_outer_band() {
    let mut failures = 0;
    for center in [[0.0, 0.0], [-20000.0, 15000.0]] {
        for zoom in [40.0, 100.0, 400.0] {
            let (mut samples, mut bad) = (0, 0);
            for deg in [20.0f32, 30.0, 45.0, 70.0, 110.0, 200.0, 300.0] {
                for f in [0.25, 0.5, 0.75, 0.95] {
                    let angle = deg.to_radians();
                    let rr = 2000.0 + 20.0 * f;
                    let focus = [center[0] + angle.cos() * rr, center[1] + angle.sin() * rr];
                    let view = View { pan: [800.0 - focus[0] * zoom, 500.0 - focus[1] * zoom], zoom };
                    let (n, b) = measure(translated(circle_w(2000.0, 40.0), center), view);
                    samples += n;
                    bad += b;
                }
            }
            println!("ROUND3 circle center={center:?} zoom={zoom}: {bad}/{samples} uncovered");
            assert!(samples > 0);
            failures += bad;
        }
    }
    assert_eq!(failures, 0);
}
#[test]
fn round3_seven_point_outer_band() {
    let mut failures = 0;
    for zoom in [3.27, 40.0] {
        let view = View { pan: [800.0 - 260.0 * zoom, 500.0], zoom };
        let (samples, bad) = measure(blob(), view);
        println!("ROUND3 seven-point width=80 zoom={zoom}: {bad}/{samples} uncovered");
        assert!(samples > 0);
        failures += bad;
    }
    assert_eq!(failures, 0);
}

fn mesh(pts: &[Pt], width: f32) -> Vec<Vertex> {
    build_fg(
        &[Prim::Stroke { pts: pts.to_vec(), width, color: [0.0, 0.0, 0.0, 1.0], clip: None }],
        View::identity(),
        1.0,
        1600.0,
        1000.0,
    )
}
#[test]
fn round3_fan_shares_quad_end_edge_exactly_at_large_coordinates() {
    for turn in [-1.0, 1.0] {
        let a = [990_000.0, -1_000_000.0];
        let b = [1_000_000.0, -1_000_000.0];
        let c = [1_010_000.0, -1_000_000.0 + 1000.0 * turn];
        let incoming = mesh(&[a, b], 8000.0);
        let outgoing = mesh(&[b, c], 8000.0);
        let full = mesh(&[a, b, c], 8000.0);
        let (outer_in, inner_in, outer_out) = if turn > 0.0 {
            (incoming[2].pos, incoming[1].pos, outgoing[5].pos)
        } else {
            (incoming[1].pos, incoming[2].pos, outgoing[0].pos)
        };
        // Compare actual emitted vertices, not a second implementation of the offset formula.
        let fan_start = full.as_chunks::<3>().0.iter().find(|t| t[1].pos == outer_in && t[0].pos == inner_in);
        assert!(fan_start.is_some(), "fan must share the incoming quad's entire end edge");
        assert!(
            full.as_chunks::<3>().0.iter().any(|t| t[0].pos == inner_in && t[2].pos == outer_out),
            "fan must land bitwise on the outgoing quad corner"
        );
    }
}
#[test]
fn round3_nonzero_tiny_turn_is_never_elided() {
    // Its area falls below HEAD's 1e-4 threshold, but the closing triangle is still required.
    let a = [-10.0, 0.0];
    let b = [0.0, 0.0];
    let c = [10.0, 0.00001];
    let v = mesh(&[a, b, c], 2.0);
    // 2 quads + two half-disc caps (r = 1: 4 chords = 3 triangles each) + the one closing triangle.
    assert_eq!(v.len(), 12 + 2 * 9 + 3, "a nonzero cross always emits the outer triangle");
}
#[test]
fn round3_short_segments_and_duplicates_keep_the_band_closed() {
    let points = [[-200.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0005, 0.0005], [0.0, 200.0]];
    let verts = mesh(&points, 80.0);
    let tris = triangles(&verts, 1600.0, 1000.0);
    let mut bad = 0;
    for y in 1..=38 {
        // Immediately outside the outgoing endpoint of the short segment: HEAD loses its wedge.
        let p = [0.0001, -(y as f64)];
        if !tris.iter().any(|t| contains(p, t)) {
            bad += 1;
        }
    }
    assert_eq!(bad, 0, "short segment gap");
    let without_duplicate = mesh(&[points[0], points[1], points[3], points[4]], 80.0);
    assert_eq!(verts.len(), without_duplicate.len(), "only exact duplicates are skipped");
    assert!(verts.iter().zip(without_duplicate).all(|(a, b)| a.pos == b.pos));
}

#[test]
fn round3_small_angle_sagitta_controls_subdivision() {
    // f32 cos(theta) rounds to 1, but r*theta^2/8 is 1.25 px: one chord is insufficient.
    let v = mesh(&[[-1_000_000.0, 0.0], [0.0, 0.0], [1_000_000.0, 100.0]], 2_000_000_000.0);
    // 2 quads + two half-disc caps at the JOIN_MAX_STEPS cap (128 chords = 127 triangles each).
    assert_eq!(v.len(), 12 + 2 * 127 * 3 + 9, "stable sagitta needs three fan triangles");
}

#[test]
fn round3_world_translation_preserves_emitted_geometry() {
    let pts = [[0.0, 0.0], [12.0, 0.00390625], [24.0, 0.01171875]];
    let emit = |center: Pt| {
        let translated = pts.iter().map(|p| [p[0] + center[0], p[1] + center[1]]).collect();
        let view = View { pan: [800.0 - center[0] * 400.0, 500.0 - center[1] * 400.0], zoom: 400.0 };
        build_fg(
            &[Prim::Stroke { pts: translated, width: 40.0, color: [0.0, 0.0, 0.0, 1.0], clip: None }],
            view,
            400.0,
            1600.0,
            1000.0,
        )
    };
    let near = emit([0.0, 0.0]);
    let far = emit([-20000.0, 15000.0]);
    assert_eq!(near.len(), far.len());
    assert!(
        near.iter().zip(far).all(|(a, b)| a.pos == b.pos),
        "world→screen cancellation must retain the segment offsets"
    );
}

fn assert_large_stroke_cover(knockout: bool) {
    for axis in [0, 1] {
        for sign in [-1.0, 1.0] {
            let mut pts = vec![[100.0, 100.0], [300.0, 300.0]];
            for p in &mut pts {
                p[axis] = sign * 1e8;
            }
            let mut pan = [0.0, 0.0];
            pan[axis] = sign * 108.0;
            let view = View { pan, zoom: 1.0 };
            let stroke = Prim::Stroke { pts: pts.clone(), width: 2e8, color: [0.0, 0.0, 1.0, 0.5], clip: None };
            let group = if knockout {
                Group::Knockout(vec![
                    Prim::Fill { rings: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]], color: [1.0; 4] },
                    stroke,
                ])
            } else {
                Group::Opaque(vec![stroke])
            };
            let (_, vertices, _, meta) = build_content(&[group], view, 1.0, 1600.0, 1000.0);
            let GroupDraw::Opaque { draws } = &meta[0] else { panic!("expected opaque routing") };
            let (band, cover) = match draws[0] {
                Draw::StrokeCov { tris, cover } if !knockout => (tris, cover),
                Draw::Knockout { band, bcover, .. } if knockout => (band, bcover),
                _ => panic!("incorrect stroke routing"),
            };
            assert!(band.1 > 0);
            assert_eq!(cover.1, 6);
            let cover_vertices = &vertices[cover.0 as usize..(cover.0 + cover.1) as usize];
            for dim in [0, 1] {
                let lo = cover_vertices.iter().map(|v| v.pos[dim]).fold(f32::INFINITY, f32::min);
                let hi = cover_vertices.iter().map(|v| v.pos[dim]).fold(f32::NEG_INFINITY, f32::max);
                for v in &vertices[band.0 as usize..(band.0 + band.1) as usize] {
                    assert!(
                        lo <= v.pos[dim] && v.pos[dim] <= hi,
                        "knockout={knockout} axis={axis} sign={sign}: cover [{lo},{hi}] excludes band {}",
                        v.pos[dim]
                    );
                }
                // The 1.5 px pad must survive until the final NDC cast, including the near
                // edge obtained by cancelling a huge centre against the radius (108 → 106.5).
                let pixels_lo = pts.iter().map(|p| screen(*p, view)[dim] - 1e8 - 1.5).fold(f64::INFINITY, f64::min);
                let pixels_hi = pts.iter().map(|p| screen(*p, view)[dim] + 1e8 + 1.5).fold(f64::NEG_INFINITY, f64::max);
                let to_ndc =
                    |p: f64| if dim == 0 { (p / 1600.0 * 2.0 - 1.0) as f32 } else { (1.0 - p / 1000.0 * 2.0) as f32 };
                assert_eq!(lo, to_ndc(pixels_lo).min(to_ndc(pixels_hi)), "padded lower cover bound");
                assert_eq!(hi, to_ndc(pixels_lo).max(to_ndc(pixels_hi)), "padded upper cover bound");
            }
        }
    }
}
#[test]
fn round3_translucent_cover_contains_large_band_with_padding() {
    assert_large_stroke_cover(false);
}
#[test]
fn round3_knockout_cover_contains_large_band_with_padding() {
    assert_large_stroke_cover(true);
}
