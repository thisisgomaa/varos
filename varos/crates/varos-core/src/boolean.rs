//! Curve-preserving boolean adapter; `planar` separately uses i_overlay for face construction.
//! Primary: `flo_curves` — boolean ops DIRECTLY on cubic-bezier paths (curves & handles survive).
//! Fallback: `i_overlay` — robust polygon boolean (also guards the primary result area).
//!
//! A `Shape` (flat) = rings of points; a result is `ResultShape` = outer cubic contour + hole cubic contours.

use crate::geom::{cubic, point_in_poly, Pt};
use flo_curves::bezier::path::*;
use flo_curves::*;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

pub type Ring = Vec<[f64; 2]>;
pub type Shape = Vec<Ring>;
/// One cubic segment: (start, control1, control2, end).
pub type Seg = (Pt, Pt, Pt, Pt);

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum BoolOp {
    #[serde(rename = "Unite")]
    Unite,
    #[serde(rename = "MinusFront")]
    MinusFront,
    #[serde(rename = "Intersect")]
    Intersect,
    #[serde(rename = "Exclude")]
    Exclude,
}

/// A boolean result: an outer cubic-bezier contour plus zero+ hole contours.
pub struct ResultShape {
    pub outer: Vec<Seg>,
    pub holes: Vec<Vec<Seg>>,
}

// ============================ flo_curves (curve-preserving, primary) ============================

fn c2(p: Pt) -> Coord2 {
    Coord2(p[0] as f64, p[1] as f64)
}
fn pt(c: Coord2) -> Pt {
    [c.x() as f32, c.y() as f32]
}

/// One Varos contour (cubic segs) → a flo_curves path (geometrically closed: last end == start).
fn to_flo(contour: &[Seg]) -> Option<SimpleBezierPath> {
    if contour.len() < 2 {
        return None;
    }
    let mut b = BezierPathBuilder::<SimpleBezierPath>::start(c2(contour[0].0));
    for s in contour {
        b = b.curve_to((c2(s.1), c2(s.2)), c2(s.3));
    }
    Some(b.build())
}
fn shape_to_flo(shape: &[Vec<Seg>]) -> Vec<SimpleBezierPath> {
    shape.iter().filter_map(|c| to_flo(c)).collect()
}
/// A flo_curves path → cubic segs (start threaded through endpoints).
fn from_flo(path: &SimpleBezierPath) -> Vec<Seg> {
    let mut p0 = pt(path.start_point());
    let mut segs = vec![];
    for (cp1, cp2, end) in path.points() {
        let (c1, c2v, e) = (pt(cp1), pt(cp2), pt(end));
        segs.push((p0, c1, c2v, e));
        p0 = e;
    }
    segs
}

fn flo_op(op: BoolOp, shapes: &[Vec<SimpleBezierPath>], acc: f64) -> Vec<ResultShape> {
    if shapes.len() < 2 {
        return vec![];
    }
    let fold = |rule: fn(&Vec<SimpleBezierPath>, &Vec<SimpleBezierPath>, f64) -> Vec<SimpleBezierPath>| {
        let mut a = shapes[0].clone();
        for s in &shapes[1..] {
            a = rule(&a, s, acc);
        }
        a
    };
    let contours = match op {
        BoolOp::Unite => fold(path_add::<SimpleBezierPath>),
        BoolOp::Intersect => fold(path_intersect::<SimpleBezierPath>),
        BoolOp::Exclude => {
            // Keep the two differences separately grouped: path_add can restore the
            // original overlapping rings instead of joining the symmetric difference.
            let mut a = shapes[0].clone();
            let mut result = Vec::new();
            for b in &shapes[1..] {
                let left = path_sub::<SimpleBezierPath>(&a, b, acc);
                let right = path_sub::<SimpleBezierPath>(b, &a, acc);
                result = group(left.iter().map(from_flo).collect());
                result.extend(group(right.iter().map(from_flo).collect()));
                a = left.into_iter().chain(right).collect();
            }
            return result;
        }
        BoolOp::MinusFront => {
            let mut clip = shapes[1].clone();
            for s in &shapes[2..] {
                clip = path_add::<SimpleBezierPath>(&clip, s, acc);
            }
            path_sub::<SimpleBezierPath>(&shapes[0], &clip, acc)
        }
    };
    group(contours.iter().map(from_flo).collect())
}

fn sample_contour(segs: &[Seg]) -> Vec<Pt> {
    sample_contour_at(segs, 8)
}

fn sample_contour_at(segs: &[Seg], steps: usize) -> Vec<Pt> {
    let mut poly = vec![];
    for s in segs {
        for k in 0..steps {
            poly.push(cubic(s.0, s.1, s.2, s.3, k as f32 / steps as f32));
        }
    }
    poly
}

fn signed_area(poly: &[Pt]) -> f64 {
    (0..poly.len())
        .map(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            a[0] as f64 * b[1] as f64 - b[0] as f64 * a[1] as f64
        })
        .sum::<f64>()
        * 0.5
}

/// A vertex can lie on another ring's edge. Use a point just inside this ring,
/// near its boundary (a centroid can instead fall inside a nested child ring).
fn interior_point(poly: &[Pt]) -> Pt {
    let winding = signed_area(poly).signum() as f32;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        for scale in [1e-4, 1e-5, 1e-3] {
            let p = [(a[0] + b[0]) * 0.5 - dy * winding * scale, (a[1] + b[1]) * 0.5 + dx * winding * scale];
            if point_in_poly(poly, p) {
                return p;
            }
        }
    }
    poly.first().copied().unwrap_or([0.0, 0.0])
}

fn result_area(shapes: &[ResultShape]) -> f64 {
    shapes
        .iter()
        .map(|s| {
            signed_area(&sample_contour_at(&s.outer, 32)).abs()
                - s.holes.iter().map(|h| signed_area(&sample_contour_at(h, 32)).abs()).sum::<f64>()
        })
        .sum()
}

/// Group result subpaths into outer+holes shapes by even-odd nesting depth (point-in-polygon).
fn group(contours: Vec<Vec<Seg>>) -> Vec<ResultShape> {
    let polys: Vec<Vec<Pt>> = contours.iter().map(|c| sample_contour(c)).collect();
    let representatives: Vec<Pt> = polys.iter().map(|p| interior_point(p)).collect();
    let n = contours.len();
    let depth: Vec<usize> = (0..n)
        .map(|i| {
            let rep = representatives[i];
            (0..n).filter(|&j| j != i && polys[j].len() >= 3 && point_in_poly(&polys[j], rep)).count()
        })
        .collect();
    let mut shapes: Vec<ResultShape> = vec![];
    let mut idx_of = vec![usize::MAX; n];
    for i in 0..n {
        if depth[i].is_multiple_of(2) && contours[i].len() >= 2 {
            idx_of[i] = shapes.len();
            shapes.push(ResultShape { outer: contours[i].clone(), holes: vec![] });
        }
    }
    for i in 0..n {
        if depth[i] % 2 == 1 && contours[i].len() >= 2 {
            let rep = representatives[i];
            let mut best: Option<usize> = None;
            let mut bestd = 0usize;
            for j in 0..n {
                if j != i
                    && depth[j].is_multiple_of(2)
                    && polys[j].len() >= 3
                    && point_in_poly(&polys[j], rep)
                    && (best.is_none() || depth[j] >= bestd)
                {
                    best = Some(j);
                    bestd = depth[j];
                }
            }
            if let Some(j) = best {
                if idx_of[j] != usize::MAX {
                    shapes[idx_of[j]].holes.push(contours[i].clone());
                }
            }
        }
    }
    shapes
}

/// Curve-preserving Pathfinder. `shapes[i]` = one path as contours (outer + holes), in z order (bottom→top).
pub fn run_boolean_curves(op: BoolOp, shapes: &[Vec<Vec<Seg>>]) -> Vec<ResultShape> {
    if shapes.len() < 2 {
        return vec![];
    }
    // accuracy in path units (screen-ish px) — small enough not to fuse intersections.
    let acc = 0.1_f64;
    let flo_in: Vec<Vec<SimpleBezierPath>> = shapes.iter().map(|s| shape_to_flo(s)).collect();
    let out = flo_op(op, &flo_in, acc);
    // Independently check area even for nonempty primary results: flo_curves
    // can silently drop or misclassify disconnected rings on touching edges.
    let flat: Vec<Shape> = shapes
        .iter()
        .map(|s| {
            s.iter().map(|c| sample_contour_at(c, 32).iter().map(|p| [p[0] as f64, p[1] as f64]).collect()).collect()
        })
        .collect();
    let fallback: Vec<ResultShape> = run_boolean_raw(op, &flat)
        .into_iter()
        .map(|sh| {
            let mut rings: Vec<Vec<Seg>> =
                sh.into_iter().map(|r| ring_to_straight_segs(&r)).filter(|s| s.len() >= 2).collect();
            // first ring = outer, rest = holes (i_overlay convention)
            if rings.is_empty() {
                ResultShape { outer: vec![], holes: vec![] }
            } else {
                let outer = rings.remove(0);
                ResultShape { outer, holes: rings }
            }
        })
        .filter(|rs| rs.outer.len() >= 2)
        .collect();
    guard_result(out, fallback)
}

fn guard_result(out: Vec<ResultShape>, fallback: Vec<ResultShape>) -> Vec<ResultShape> {
    let expected = result_area(&fallback);
    let tolerance = 2e-3 * expected.max(1.0);
    if (result_area(&out) - expected).abs() > tolerance {
        fallback
    } else {
        out
    }
}

fn ring_to_straight_segs(ring: &Ring) -> Vec<Seg> {
    let m = ring.len();
    if m < 3 {
        return vec![];
    }
    (0..m)
        .map(|i| {
            let a = [ring[i][0] as f32, ring[i][1] as f32];
            let b = [ring[(i + 1) % m][0] as f32, ring[(i + 1) % m][1] as f32];
            (a, a, b, b)
        })
        .collect()
}

// ============================ i_overlay (polygon, fallback) ============================

fn flatten(shapes: &[Shape]) -> Shape {
    shapes.iter().flatten().cloned().collect()
}

fn fold(shapes: &[Shape], rule: OverlayRule) -> Vec<Shape> {
    if shapes.is_empty() {
        return vec![];
    }
    if shapes.len() == 1 {
        return vec![shapes[0].clone()];
    }
    let mut acc: Shape = shapes[0].clone();
    let mut last: Vec<Shape> = vec![];
    for s in &shapes[1..] {
        last = acc.overlay(s, rule, FillRule::EvenOdd);
        acc = flatten(&last);
    }
    last
}

/// Polygon boolean over flat rings (returns simplified flat result shapes).
pub fn run_boolean(op: BoolOp, shapes: &[Shape]) -> Vec<Shape> {
    if shapes.len() < 2 {
        return vec![];
    }
    run_boolean_raw(op, shapes)
        .into_iter()
        .map(|sh| sh.into_iter().map(|r| rdp(&r, 0.75)).filter(|r| r.len() >= 3).collect::<Shape>())
        .filter(|sh: &Shape| !sh.is_empty())
        .collect()
}

// The engine guard uses unsimplified rings: RDP may itself change the area.
fn run_boolean_raw(op: BoolOp, shapes: &[Shape]) -> Vec<Shape> {
    match op {
        BoolOp::Unite => fold(shapes, OverlayRule::Union),
        BoolOp::Intersect => fold(shapes, OverlayRule::Intersect),
        BoolOp::Exclude => fold(shapes, OverlayRule::Xor),
        BoolOp::MinusFront => {
            let clip = flatten(&fold(&shapes[1..], OverlayRule::Union));
            shapes[0].overlay(&clip, OverlayRule::Difference, FillRule::EvenOdd)
        }
    }
}

fn perp(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
    ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt()
}
fn rdp(pts: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (mut idx, mut dmax) = (0usize, 0.0f64);
    for (i, &pt) in pts.iter().enumerate().skip(1).take(pts.len() - 2) {
        let d = perp(pt, a, b);
        if d > dmax {
            dmax = d;
            idx = i;
        }
    }
    if dmax > tol {
        let mut left = rdp(&pts[..=idx], tol);
        let right = rdp(&pts[idx..], tol);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![a, b]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn area_guard_rejects_nonempty_incomplete_primary_result() {
        let square = |size: f64| ResultShape {
            outer: ring_to_straight_segs(&vec![[0., 0.], [size, 0.], [size, size], [0., size]]),
            holes: vec![],
        };
        let guarded = guard_result(vec![square(10.)], vec![square(20.)]);
        assert!((result_area(&guarded) - 400.).abs() < 1e-3);
        // A valid primary result retains its cubic handles instead of being flattened.
        let curved = ResultShape {
            outer: vec![([0., 0.], [0., 10.], [10., 10.], [10., 0.]), ([10., 0.], [10., -10.], [0., -10.], [0., 0.])],
            holes: vec![],
        };
        let fallback = ResultShape {
            outer: ring_to_straight_segs(
                &sample_contour_at(&curved.outer, 32).iter().map(|p| [p[0] as f64, p[1] as f64]).collect(),
            ),
            holes: vec![],
        };
        let kept = guard_result(vec![curved], vec![fallback]);
        assert_eq!(kept[0].outer.len(), 2);
        assert_eq!(kept[0].outer[0].1, [0., 10.]);
    }
}
