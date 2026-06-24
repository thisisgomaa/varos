//! Headless math tests for the pure core — NO GPU, NO window, NO synthetic UI/feel testing.
//! These lock in the gnarly geometry (boolean ops, transforms, point-in-poly-with-holes,
//! beziers) that is impossible to eyeball. They exercise ONLY the public API across the
//! hard seam, so they also prove the core is usable on its own.
//!
//! Run with:  cargo test -p varos-core

use varos_core::boolean::{run_boolean_curves, BoolOp, ResultShape, Seg};
use varos_core::geom::{
    add, cubic, dist, mirror, point_in_poly, rotate_about, scale, sub, Pt,
};
use varos_core::model::{Anchor, Document, Path};

// ---------------------------------------------------------------- helpers

/// A rectangle as 4 straight cubic segs (control points == endpoints → straight edges).
fn rect_segs(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Seg> {
    let p = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    (0..4)
        .map(|i| {
            let a = p[i];
            let b = p[(i + 1) % 4];
            (a, a, b, b)
        })
        .collect()
}

/// Shoelace area of a closed polyline (sign-independent).
fn poly_area(poly: &[Pt]) -> f32 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        s += a[0] * b[1] - b[0] * a[1];
    }
    (s * 0.5).abs()
}

/// Sample a cubic-seg contour into a polyline for area measurement.
fn seg_poly(segs: &[Seg]) -> Vec<Pt> {
    let mut poly = vec![];
    for s in segs {
        for k in 0..16 {
            poly.push(cubic(s.0, s.1, s.2, s.3, k as f32 / 16.0));
        }
    }
    poly
}

/// Net filled area of a boolean result = sum(outer) - sum(holes), over all shapes.
fn result_area(rs: &[ResultShape]) -> f32 {
    let mut a = 0.0;
    for r in rs {
        a += poly_area(&seg_poly(&r.outer));
        for h in &r.holes {
            a -= poly_area(&seg_poly(h));
        }
    }
    a
}

/// Two unit-ish squares: A=[0,0,10,10], B=[5,5,15,15]. Overlap = 25.
fn two_overlapping() -> Vec<Vec<Vec<Seg>>> {
    vec![
        vec![rect_segs(0.0, 0.0, 10.0, 10.0)], // bottom (z order)
        vec![rect_segs(5.0, 5.0, 15.0, 15.0)], // top
    ]
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

// ---------------------------------------------------------------- boolean ops

#[test]
fn unite_area() {
    let r = run_boolean_curves(BoolOp::Unite, &two_overlapping());
    assert!(!r.is_empty(), "Unite produced nothing");
    let a = result_area(&r);
    assert!(close(a, 175.0, 2.0), "Unite area = {a}, expected ~175 (100+100-25)");
}

#[test]
fn intersect_area() {
    let r = run_boolean_curves(BoolOp::Intersect, &two_overlapping());
    assert!(!r.is_empty(), "Intersect produced nothing");
    let a = result_area(&r);
    assert!(close(a, 25.0, 2.0), "Intersect area = {a}, expected ~25");
}

#[test]
fn minus_front_area() {
    // bottom (shapes[0]) minus the rest → A - B = 75
    let r = run_boolean_curves(BoolOp::MinusFront, &two_overlapping());
    assert!(!r.is_empty(), "MinusFront produced nothing");
    let a = result_area(&r);
    assert!(close(a, 75.0, 2.0), "MinusFront area = {a}, expected ~75 (100-25)");
}

#[test]
fn exclude_disjoint() {
    // XOR of two non-touching squares = both of them = 200.
    let shapes = vec![
        vec![rect_segs(0.0, 0.0, 10.0, 10.0)],
        vec![rect_segs(20.0, 20.0, 30.0, 30.0)],
    ];
    let r = run_boolean_curves(BoolOp::Exclude, &shapes);
    let a = result_area(&r);
    assert!(close(a, 200.0, 2.0), "Exclude(disjoint) area = {a}, expected ~200");
}

#[test]
fn exclude_nested() {
    // XOR of a small square fully inside a big one = a donut (big minus small).
    let shapes = vec![
        vec![rect_segs(0.0, 0.0, 10.0, 10.0)],
        vec![rect_segs(2.0, 2.0, 8.0, 8.0)], // area 36, fully inside
    ];
    let r = run_boolean_curves(BoolOp::Exclude, &shapes);
    let a = result_area(&r);
    assert!(close(a, 64.0, 2.0), "Exclude(nested) area = {a}, expected ~64 (100-36)");
    let holes: usize = r.iter().map(|s| s.holes.len()).sum();
    assert!(holes >= 1, "nested Exclude should leave a hole");
}

#[test]
fn exclude_corner_overlap() {
    // Regression: two squares overlapping at a corner pinch the XOR region at two single
    // points. The old (A∪B)−(A∩B) formula degenerated into a self-cancelling net-zero result;
    // the symmetric-difference (A−B)∪(B−A) formula handles it. Must stay ~150 (175-25).
    let r = run_boolean_curves(BoolOp::Exclude, &two_overlapping());
    let a = result_area(&r);
    assert!(close(a, 150.0, 2.0), "Exclude(corner overlap) area = {a}, expected ~150");
}

#[test]
fn minus_front_makes_a_hole() {
    // a small square fully inside a big one → subtracting it leaves a donut: outer + 1 hole.
    let shapes = vec![
        vec![rect_segs(0.0, 0.0, 20.0, 20.0)],  // big, bottom (area 400)
        vec![rect_segs(5.0, 5.0, 15.0, 15.0)],  // small, fully inside (area 100)
    ];
    let r = run_boolean_curves(BoolOp::MinusFront, &shapes);
    assert!(!r.is_empty(), "donut produced nothing");
    let holes: usize = r.iter().map(|s| s.holes.len()).sum();
    assert!(holes >= 1, "expected an editable hole contour, got {holes}");
    let a = result_area(&r);
    assert!(close(a, 300.0, 3.0), "donut net area = {a}, expected ~300 (400-100)");
}

// ---------------------------------------------------------------- transforms

#[test]
fn rotate_round_trip() {
    let (p, c) = ([10.0, 2.0], [3.0, 4.0]);
    let there = rotate_about(p, c, 0.7);
    let back = rotate_about(there, c, -0.7);
    assert!(dist(p, back) < 1e-3, "rotate round-trip drifted: {back:?} vs {p:?}");
}

#[test]
fn rotate_quarter_turn() {
    // [1,0] about origin by +90° → [0,1]
    let q = rotate_about([1.0, 0.0], [0.0, 0.0], std::f32::consts::FRAC_PI_2);
    assert!(dist(q, [0.0, 1.0]) < 1e-4, "quarter turn = {q:?}, expected [0,1]");
}

#[test]
fn scale_about_pivot_round_trip() {
    // scale about a pivot up by k then down by 1/k returns the original point.
    let scale_about = |p: Pt, c: Pt, k: f32| add(c, scale(sub(p, c), k));
    let (p, c, k) = ([10.0, 2.0], [3.0, 4.0], 2.5);
    let up = scale_about(p, c, k);
    let back = scale_about(up, c, 1.0 / k);
    assert!(dist(p, back) < 1e-3, "scale round-trip drifted: {back:?} vs {p:?}");
    // sanity: scaling by 2 doubles the distance from the pivot.
    assert!(close(dist(c, scale_about(p, c, 2.0)), 2.0 * dist(c, p), 1e-3));
}

#[test]
fn mirror_is_point_reflection() {
    assert_eq!(mirror([0.0, 0.0], [1.0, 1.0]), [-1.0, -1.0]);
    // mirror twice = identity
    let (p, q) = ([5.0, -3.0], [2.0, 7.0]);
    assert!(dist(mirror(p, mirror(p, q)), q) < 1e-4);
}

// ---------------------------------------------------------------- beziers

#[test]
fn cubic_endpoints() {
    let (p0, c1, c2, p3) = ([0.0, 0.0], [1.0, 5.0], [9.0, 5.0], [10.0, 0.0]);
    assert_eq!(cubic(p0, c1, c2, p3, 0.0), p0);
    assert_eq!(cubic(p0, c1, c2, p3, 1.0), p3);
}

#[test]
fn cubic_straight_is_linear() {
    // control points on the endpoints → straight line → t=0.5 is the midpoint.
    let (a, b) = ([0.0, 0.0], [10.0, 4.0]);
    let mid = cubic(a, a, b, b, 0.5);
    assert!(dist(mid, [5.0, 2.0]) < 1e-4, "straight cubic midpoint = {mid:?}");
}

// ---------------------------------------------------------------- point-in-poly (+ holes)

#[test]
fn point_in_poly_basic() {
    let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    assert!(point_in_poly(&sq, [5.0, 5.0]));
    assert!(!point_in_poly(&sq, [15.0, 5.0]));
    assert!(!point_in_poly(&sq, [-1.0, 5.0]));
}

#[test]
fn point_in_path_respects_holes() {
    // a 20×20 square with a 5..15 square hole (donut). even-odd: hole interior is NOT filled.
    let mk = |id: u32, x: f32, y: f32| Anchor {
        id,
        p: [x, y],
        hin: None,
        hout: None,
        smooth: false,
    };
    let outer = vec![mk(1, 0.0, 0.0), mk(2, 20.0, 0.0), mk(3, 20.0, 20.0), mk(4, 0.0, 20.0)];
    let hole = vec![mk(5, 5.0, 5.0), mk(6, 15.0, 5.0), mk(7, 15.0, 15.0), mk(8, 5.0, 15.0)];
    let mut doc = Document::default();
    doc.paths.push(Path {
        id: 1,
        anchors: outer,
        closed: true,
        fill: Some([0.0, 0.0, 0.0, 1.0]),
        stroke: None,
        stroke_width: 1.0,
        holes: vec![hole],
    });

    assert!(doc.point_in_path(0, [2.0, 2.0]), "ring band should be filled");
    assert!(!doc.point_in_path(0, [10.0, 10.0]), "inside the hole must NOT be filled (even-odd)");
    assert!(!doc.point_in_path(0, [25.0, 25.0]), "outside everything");
}
