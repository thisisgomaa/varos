//! A26 — a boolean (Pathfinder) result must keep sharp corners as CORNER points, not smooth them into
//! anchors with handles. The union of two axis-aligned rectangles is all right angles, so every point of
//! the result must be a clean corner (no hin/hout, not smooth). Pure core, no UI.
//!
//! Run with:  cargo test -p varos-core --test boolean_corners

use varos_core::boolean::BoolOp;
use varos_core::editor::Editor;
use varos_core::model::{Anchor, Path, ShapeKind};

fn corner(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn rect(pid: u32, base: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
    Path::new(
        pid,
        vec![corner(base, x0, y0), corner(base + 1, x1, y0), corner(base + 2, x1, y1), corner(base + 3, x0, y1)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

#[test]
fn uniting_two_rectangles_yields_only_corner_points() {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(rect(100, 1, 0.0, 0.0, 60.0, 40.0));
    ed.doc.paths.push(rect(200, 10, 30.0, 20.0, 90.0, 60.0)); // overlaps the first
    ed.doc.ids = 1000;
    ed.objsel.insert(100);
    ed.objsel.insert(200);
    ed.pathfinder(BoolOp::Unite);

    let mut n = 0;
    for p in &ed.doc.paths {
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            n += 1;
            assert!(
                a.hin.is_none() && a.hout.is_none(),
                "a right-angle corner from a boolean must have NO handles, got {a:?}"
            );
            assert!(!a.smooth, "a right-angle corner must not be a smooth point, got {a:?}");
        }
    }
    assert!(n >= 6, "the L-shaped union has at least 6 corners — the op must have produced a path, got {n}");
}

// ---------- 2026-09-23 re-verification of A26 against the full acceptance list ----------

/// A closed path built from `pts` with corner anchors only (a pen-drawn polygon: zigzag, slanted rect…).
fn poly(ed: &mut Editor, pts: &[[f32; 2]]) -> u32 {
    let anchors: Vec<Anchor> = pts
        .iter()
        .map(|&p| {
            let id = ed.doc.nid();
            Anchor { id, p, hin: None, hout: None, smooth: false }
        })
        .collect();
    let pid = ed.doc.nid();
    ed.doc.paths.push(Path::new(pid, anchors, true, Some([0.9, 0.8, 0.1, 1.0]), None, 1.0));
    pid
}

/// A rectangle of size w×h centred on (cx, cy), rotated by `deg` — every corner is a sharp 90°.
fn slanted_rect(cx: f32, cy: f32, w: f32, h: f32, deg: f32) -> Vec<[f32; 2]> {
    let (s, c) = deg.to_radians().sin_cos();
    [[-w / 2.0, -h / 2.0], [w / 2.0, -h / 2.0], [w / 2.0, h / 2.0], [-w / 2.0, h / 2.0]]
        .iter()
        .map(|&[x, y]| [cx + x * c - y * s, cy + x * s + y * c])
        .collect()
}

fn fresh() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.ids = 1000;
    ed
}

fn all_anchors(ed: &Editor) -> Vec<Anchor> {
    ed.doc.paths.iter().flat_map(|p| p.anchors.iter().chain(p.holes.iter().flatten()).cloned()).collect()
}

/// The Pathfinder ops only ever see straight edges here, so NO output anchor may carry a handle.
fn assert_all_corners(ed: &Editor, what: &str) {
    let anchors = all_anchors(ed);
    assert!(anchors.len() >= 3, "{what}: the op must produce a path, got {} anchors", anchors.len());
    for a in &anchors {
        assert!(a.hin.is_none() && a.hout.is_none(), "{what}: sharp corner grew handles: {a:?}");
        assert!(!a.smooth, "{what}: sharp corner became a smooth point: {a:?}");
    }
}

#[test]
fn every_pathfinder_op_on_slanted_rectangles_keeps_clean_corners() {
    // The reported case: slanted/zigzag shapes, not axis-aligned — the straight-edge detection must hold
    // for any direction and for coordinates far from the origin (f32 precision after the engine's splits).
    for (name, op) in [
        ("Unite", BoolOp::Unite),
        ("MinusFront", BoolOp::MinusFront),
        ("Intersect", BoolOp::Intersect),
        ("Exclude", BoolOp::Exclude),
    ] {
        let mut ed = fresh();
        let a = poly(&mut ed, &slanted_rect(1240.0, 860.0, 300.0, 120.0, 27.0));
        let b = poly(&mut ed, &slanted_rect(1330.0, 905.0, 260.0, 90.0, -41.0));
        ed.objsel.insert(a);
        ed.objsel.insert(b);
        ed.pathfinder(op);
        assert_all_corners(&ed, name);
    }
}

#[test]
fn uniting_a_zigzag_with_a_bar_keeps_every_zig_a_corner() {
    // The screenshot case: a yellow zigzag (acute sharp corners) united with a bar crossing it.
    let mut ed = fresh();
    let zig = poly(
        &mut ed,
        &[
            [100.0, 300.0],
            [160.0, 180.0],
            [220.0, 300.0],
            [280.0, 180.0],
            [340.0, 300.0],
            [400.0, 180.0],
            [400.0, 230.0],
            [340.0, 350.0],
            [280.0, 230.0],
            [220.0, 350.0],
            [160.0, 230.0],
            [100.0, 350.0],
        ],
    );
    let bar = poly(&mut ed, &slanted_rect(250.0, 260.0, 380.0, 30.0, 8.0));
    ed.objsel.insert(zig);
    ed.objsel.insert(bar);
    ed.pathfinder(BoolOp::Unite);
    assert_all_corners(&ed, "zigzag ∪ bar");
}

#[test]
fn circle_united_with_a_rectangle_keeps_arcs_smooth_and_straight_edges_cornered() {
    // Circle r=50 at the origin ∪ a bar sticking out to the right. Expected (Illustrator):
    //  · anchors on the arc (away from the bar) = smooth points with both handles;
    //  · the bar's two outer corners = corner points with NO handles;
    //  · where the arc meets a straight bar edge = a corner whose straight side has NO handle
    //    (only the arc side keeps its handle).
    let mut ed = fresh();
    let circle = ed.doc.build_shape(ShapeKind::Ellipse, [-50.0, -50.0], [50.0, 50.0]);
    let cid = ed.doc.nid();
    ed.doc.paths.push(Path::new(cid, circle, true, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0));
    let bar = poly(&mut ed, &[[0.0, -20.0], [120.0, -20.0], [120.0, 20.0], [0.0, 20.0]]);
    ed.objsel.insert(cid);
    ed.objsel.insert(bar);
    ed.pathfinder(BoolOp::Unite);

    assert_eq!(ed.doc.paths.len(), 1, "circle ∪ bar is one shape");
    let p = &ed.doc.paths[0];
    let near = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 0.5;
    let on_bar_edge = |q: [f32; 2]| (q[1].abs() - 20.0).abs() < 0.5 && q[0] > 40.0;
    let on_arc = |q: [f32; 2]| (q[0].hypot(q[1]) - 50.0).abs() < 0.5;
    let bar_corner = |q: [f32; 2]| near(q, [120.0, -20.0]) || near(q, [120.0, 20.0]);

    let (mut arc_smooth, mut bar_corners, mut junctions) = (0, 0, 0);
    for a in &p.anchors {
        if bar_corner(a.p) {
            bar_corners += 1;
            assert!(a.hin.is_none() && a.hout.is_none() && !a.smooth, "bar corner must be clean: {a:?}");
        } else if on_arc(a.p) && on_bar_edge(a.p) {
            junctions += 1;
            assert!(!a.smooth, "arc/edge junction is a corner, not smooth: {a:?}");
            let handles = a.hin.is_some() as u8 + a.hout.is_some() as u8;
            assert_eq!(handles, 1, "junction keeps ONLY its arc-side handle, got {a:?}");
        } else if on_arc(a.p) {
            arc_smooth += 1;
            assert!(a.smooth && a.hin.is_some() && a.hout.is_some(), "arc anchor must stay smooth: {a:?}");
        } else {
            panic!("unexpected anchor off both the arc and the bar: {a:?}");
        }
    }
    assert_eq!(bar_corners, 2, "both outer bar corners present");
    assert_eq!(junctions, 2, "the arc meets the bar at two points");
    assert!(arc_smooth >= 3, "the untouched arc keeps its smooth anchors (top, left, bottom), got {arc_smooth}");

    // No handle may sit on a straight segment: consecutive anchors both on the bar outline ⇒ no handles.
    let n = p.anchors.len();
    let straight = |q: [f32; 2]| on_bar_edge(q) || bar_corner(q);
    for i in 0..n {
        let (x, y) = (&p.anchors[i], &p.anchors[(i + 1) % n]);
        if straight(x.p) && straight(y.p) {
            assert!(x.hout.is_none() && y.hin.is_none(), "straight bar segment carries handles: {x:?} → {y:?}");
        }
    }
}
