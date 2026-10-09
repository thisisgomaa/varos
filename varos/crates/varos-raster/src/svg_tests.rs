//! Compare exported SVG with the actual CPU scene raster, without a window or GPU.
use super::*;
use std::sync::atomic::AtomicBool;
use varos_core::model::{Anchor, Artboard, Paint, Path, Xform};
use varos_core::svg::{export_svg_files, plan_svg_export, ExportScope};

fn rect(id: u32, xy: [f32; 2], wh: [f32; 2]) -> Path {
    Path::new(
        id,
        [[xy[0], xy[1]], [xy[0] + wh[0], xy[1]], [xy[0] + wh[0], xy[1] + wh[1]], [xy[0], xy[1] + wh[1]]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([0.2, 0.7, 0.3, 1.0]),
        None,
        1.0,
    )
}
fn compare(d: Document, label: &str) {
    let plan =
        plan_svg_export(&d, if d.artboards.is_empty() { ExportScope::WholeBoard } else { ExportScope::ActiveArtboard })
            .unwrap();
    let file = export_svg_files(&d, &plan, &AtomicBool::new(false)).unwrap().remove(0);
    let [x, y, w, h] = file.page.rect;
    let size = 256;
    let xf =
        Transform::from_row(size as f32 / w, 0.0, 0.0, size as f32 / h, -x * size as f32 / w, -y * size as f32 / h);
    let mut ed = Editor::new();
    ed.replace_doc(d);
    let scene = build_scene(&ed, size as f32 / w);
    let mut cpu = Pixmap::new(size, size).unwrap();
    draw_groups(&scene.content, &mut cpu, xf);
    let tree = resvg::usvg::Tree::from_data(&file.bytes, &resvg::usvg::Options::default()).unwrap();
    let mut svg = Pixmap::new(size, size).unwrap();
    resvg::render(
        &tree,
        Transform::from_scale(size as f32 / tree.size().width(), size as f32 / tree.size().height()),
        &mut svg.as_mut(),
    );
    let mut sum = 0u64;
    let mut changed = 0;
    for (a, b) in cpu.data().as_chunks::<4>().0.iter().zip(svg.data().as_chunks::<4>().0) {
        let delta = a.iter().zip(b).map(|(a, b)| a.abs_diff(*b) as u64).sum::<u64>();
        sum += delta;
        if delta > 32 {
            changed += 1;
        }
    }
    let mean = sum as f64 / (size * size * 4) as f64;
    let fraction = changed as f64 / (size * size) as f64;
    eprintln!("{label}: mean={mean:.4}, fraction={fraction:.4}");
    // Exact curves vs adaptive flattened scene rings differ at antialiased edges. A paint/compositing
    // error affects interiors, far beyond these limits (mean < 0.25/255 per channel, < 0.5% edge pixels).
    assert!(mean < 0.25 && fraction < 0.005, "{label}: mean={mean:.4}, differing fraction={fraction:.4}");
}
fn board(paths: Vec<Path>) -> Document {
    let mut d = Document {
        paths,
        ids: 1000,
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, ..Artboard::default() }],
        ..Document::default()
    };
    d.sync_tree();
    d
}
#[test]
fn svg_matches_cpu_for_isolation_knockout_curves_holes_and_clips() {
    for (opacity, alpha) in [(1.0, 1.0), (0.4, 1.0), (1.0, 0.5), (0.4, 0.5), (1.0, 0.0)] {
        let mut p = rect(2, [20.0, 20.0], [60.0, 60.0]);
        p.stroke = Paint::Solid([0.8, 0.1, 0.2, alpha]);
        p.stroke_width = 8.0;
        p.opacity = opacity;
        compare(board(vec![p]), &format!("opacity {opacity}, stroke {alpha}"));
    }
    let mut p = rect(2, [15.0, 15.0], [70.0, 70.0]);
    p.anchors[0].hout = Some([20.0, -5.0]);
    p.anchors[1].hin = Some([80.0, 35.0]);
    p.anchors[3].hout = Some([0.0, 40.0]);
    p.anchors[0].hin = Some([5.0, 25.0]);
    p.holes = vec![rect(4, [35.0, 35.0], [20.0, 20.0]).anchors];
    let mut d = board(vec![p]);
    let unit = d.unit_of(2).unwrap();
    d.set_node_xform(unit, Xform { rot: 0.2, piv: [50.0, 50.0] });
    compare(d, "rotated compound curves");
    let mask = rect(2, [10.0, 10.0], [50.0, 70.0]);
    let mut member = rect(3, [0.0, 0.0], [100.0, 100.0]);
    member.opacity = 0.6;
    let mut d = board(vec![mask, member]);
    d.clip_group(&[2, 3], 2).unwrap();
    compare(d, "clip");
}
#[test]
fn svg_frozen_artboard_fixtures_match_cpu_with_edge_tolerance() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures");
    for name in [
        "v1/v1_plain.vrs",
        "v1/v1_rotated.vrs",
        "v1/v1_masked.vrs",
        "v1/v1_translucent.vrs",
        "v2/v2_masked_rotated.vrs",
    ] {
        let d = varos_pdf::load_vrs(&root.join(name)).unwrap();
        compare(d, name);
    }
}

#[test]
fn frozen_stroke_styles_match_svg_on_cpu() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v5");
    let index = std::fs::read_to_string(root.join("INDEX")).unwrap();
    for name in index.lines().filter(|name| *name != "plain") {
        let loaded = varos_core::format::decode_model(
            &std::fs::read(root.join(format!("{name}.json"))).unwrap(),
            None,
            &varos_core::format::Limits::DEFAULT,
        )
        .unwrap();
        compare(loaded.doc, name);
    }
}

#[test]
fn styled_native_overlapping_subpaths_paint_once_under_alpha() {
    let mut p = rect(10, [10.0, 10.0], [80.0, 80.0]);
    p.fill = Paint::None;
    p.stroke = Paint::Solid([0.8, 0.2, 0.1, 0.5]);
    p.stroke_width = 30.0;
    p.opacity = 0.8;
    p.stroke_style.cap = varos_core::stroke::StrokeCap::Butt;
    p.holes.push(rect(20, [50.0, 20.0], [20.0, 40.0]).anchors);
    compare(board(vec![p]), "native overlapping stroke subpaths");
}

#[test]
fn degenerate_styled_caps_use_shared_coverage_instead_of_native_zero_line_rules() {
    use varos_core::stroke::StrokeCap;
    for cap in [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square] {
        let mut p = rect(10, [50.0, 50.0], [0.0, 0.0]);
        p.anchors.truncate(2);
        p.closed = false;
        p.fill = Paint::None;
        p.stroke = Paint::Solid([0.8, 0.2, 0.1, 0.5]);
        p.stroke_width = 10.0;
        p.stroke_style.cap = cap;
        p.stroke_style.miter_limit = 11.0;
        compare(board(vec![p]), &format!("degenerate {cap:?}"));
    }
    let mut p = rect(10, [50.0, 50.0], [0.0, 0.0]);
    p.anchors.truncate(1);
    p.closed = false;
    p.fill = Paint::None;
    p.stroke = Paint::Solid([0.8, 0.2, 0.1, 0.5]);
    p.stroke_width = 10.0;
    p.stroke_style.miter_limit = 11.0;
    compare(board(vec![p]), "single-anchor styled round cap");

    let mut p = rect(10, [50.0, 50.0], [0.0, 0.0]);
    p.anchors.truncate(1);
    p.closed = false;
    p.fill = Paint::None;
    p.stroke = Paint::Solid([0.8, 0.2, 0.1, 0.5]);
    p.stroke_width = 10.0;
    p.stroke_style.cap = StrokeCap::Square;
    compare(board(vec![p]), "single-anchor square cap");
}

#[test]
fn gradients_match_cpu_for_spread_radial_focal_transform_and_knockout() {
    use varos_core::gradient::{Gradient, GradientKind, Spread};
    for kind in [GradientKind::Linear, GradientKind::Radial] {
        for spread in [Spread::Pad, Spread::Repeat, Spread::Reflect] {
            for stroke_gradient in [false, true] {
                let mut g = Gradient {
                    kind,
                    spread,
                    placement: [30., 5., -4., 35., 40., 45.],
                    focal: [0.25, -0.2],
                    ..Default::default()
                };
                g.stops[0].colour = [0.8, 0.2, 0.1, 0.5];
                g.stops[0].midpoint = 0.2;
                g.stops[1].colour = [0.1, 0.3, 0.7, 0.8];
                let mut p = rect(2, [15., 15.], [70., 70.]);
                p.stroke_width = 8.;
                p.opacity = 0.6;
                if stroke_gradient {
                    p.stroke = Paint::Gradient(g);
                } else {
                    p.fill = Paint::Gradient(g);
                    p.stroke = Paint::Solid([0.1, 0.7, 0.3, 0.4]);
                }
                let mut d = board(vec![p]);
                let unit = d.unit_of(2).unwrap();
                d.set_node_xform(unit, Xform { rot: 0.1, piv: [50., 50.] });
                compare(d, &format!("{kind:?} {spread:?} stroke={stroke_gradient}"));
            }
        }
    }
}
