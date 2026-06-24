//! The ONLY place that touches the external boolean-geometry crates.
//! Primary: `flo_curves` — boolean ops DIRECTLY on cubic-bezier paths (curves & handles survive).
//! Fallback: `i_overlay` — robust polygon boolean (used if flo_curves yields nothing for overlapping input).
//!
//! A `Shape` (flat) = rings of points; a result is `ResultShape` = outer cubic contour + hole cubic contours.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use flo_curves::*;
use flo_curves::bezier::path::*;
use crate::geom::{cubic, point_in_poly, Pt};

pub type Ring = Vec<[f64; 2]>;
pub type Shape = Vec<Ring>;
/// One cubic segment: (start, control1, control2, end).
pub type Seg = (Pt, Pt, Pt, Pt);

#[derive(Clone, Copy)]
pub enum BoolOp { Unite, MinusFront, Intersect, Exclude }

/// A boolean result: an outer cubic-bezier contour plus zero+ hole contours.
pub struct ResultShape { pub outer: Vec<Seg>, pub holes: Vec<Vec<Seg>> }

// ============================ flo_curves (curve-preserving, primary) ============================

fn c2(p: Pt) -> Coord2 { Coord2(p[0] as f64, p[1] as f64) }
fn pt(c: Coord2) -> Pt { [c.x() as f32, c.y() as f32] }

/// One Varos contour (cubic segs) → a flo_curves path (geometrically closed: last end == start).
fn to_flo(contour: &[Seg]) -> Option<SimpleBezierPath> {
    if contour.len() < 2 { return None; }
    let mut b = BezierPathBuilder::<SimpleBezierPath>::start(c2(contour[0].0));
    for s in contour { b = b.curve_to((c2(s.1), c2(s.2)), c2(s.3)); }
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

fn flo_op(op: BoolOp, shapes: &[Vec<SimpleBezierPath>], acc: f64) -> Vec<SimpleBezierPath> {
    if shapes.len() < 2 { return vec![]; }
    let fold = |rule: fn(&Vec<SimpleBezierPath>, &Vec<SimpleBezierPath>, f64) -> Vec<SimpleBezierPath>| {
        let mut a = shapes[0].clone();
        for s in &shapes[1..] { a = rule(&a, s, acc); }
        a
    };
    match op {
        BoolOp::Unite => fold(|a, b, e| path_add::<SimpleBezierPath>(a, b, e)),
        BoolOp::Intersect => fold(|a, b, e| path_intersect::<SimpleBezierPath>(a, b, e)),
        BoolOp::Exclude => {
            // XOR via symmetric difference (A−B)∪(B−A), folded pairwise. This is robust to
            // the pinch/touch cases where (A∪B)−(A∩B) degenerates into a self-cancelling result.
            let mut a = shapes[0].clone();
            for b in &shapes[1..] {
                let a_minus_b = path_sub::<SimpleBezierPath>(&a, b, acc);
                let b_minus_a = path_sub::<SimpleBezierPath>(b, &a, acc);
                a = path_add::<SimpleBezierPath>(&a_minus_b, &b_minus_a, acc);
            }
            a
        }
        BoolOp::MinusFront => {
            let mut clip = shapes[1].clone();
            for s in &shapes[2..] { clip = path_add::<SimpleBezierPath>(&clip, s, acc); }
            path_sub::<SimpleBezierPath>(&shapes[0], &clip, acc)
        }
    }
}

fn sample_contour(segs: &[Seg]) -> Vec<Pt> {
    let mut poly = vec![];
    for s in segs { for k in 0..8 { poly.push(cubic(s.0, s.1, s.2, s.3, k as f32 / 8.0)); } }
    poly
}

/// Group result subpaths into outer+holes shapes by even-odd nesting depth (point-in-polygon).
fn group(contours: Vec<Vec<Seg>>) -> Vec<ResultShape> {
    let polys: Vec<Vec<Pt>> = contours.iter().map(|c| sample_contour(c)).collect();
    let n = contours.len();
    let depth: Vec<usize> = (0..n).map(|i| {
        let rep = polys[i].first().copied().unwrap_or([0.0, 0.0]);
        (0..n).filter(|&j| j != i && polys[j].len() >= 3 && point_in_poly(&polys[j], rep)).count()
    }).collect();
    let mut shapes: Vec<ResultShape> = vec![];
    let mut idx_of = vec![usize::MAX; n];
    for i in 0..n { if depth[i] % 2 == 0 && contours[i].len() >= 2 { idx_of[i] = shapes.len(); shapes.push(ResultShape { outer: contours[i].clone(), holes: vec![] }); } }
    for i in 0..n {
        if depth[i] % 2 == 1 && contours[i].len() >= 2 {
            let rep = polys[i][0];
            let mut best: Option<usize> = None; let mut bestd = 0usize;
            for j in 0..n {
                if j != i && depth[j] % 2 == 0 && polys[j].len() >= 3 && point_in_poly(&polys[j], rep) && (best.is_none() || depth[j] >= bestd) {
                    best = Some(j); bestd = depth[j];
                }
            }
            if let Some(j) = best { if idx_of[j] != usize::MAX { shapes[idx_of[j]].holes.push(contours[i].clone()); } }
        }
    }
    shapes
}

/// Curve-preserving Pathfinder. `shapes[i]` = one path as contours (outer + holes), in z order (bottom→top).
pub fn run_boolean_curves(op: BoolOp, shapes: &[Vec<Vec<Seg>>]) -> Vec<ResultShape> {
    if shapes.len() < 2 { return vec![]; }
    // accuracy in path units (screen-ish px) — small enough not to fuse intersections.
    let acc = 0.1_f64;
    let flo_in: Vec<Vec<SimpleBezierPath>> = shapes.iter().map(|s| shape_to_flo(s)).collect();
    let out = flo_op(op, &flo_in, acc);
    if !out.is_empty() {
        return group(out.iter().map(from_flo).filter(|s| s.len() >= 2).collect());
    }
    // fallback: i_overlay (polygonal) so the op still produces something on hard input
    let flat: Vec<Shape> = shapes.iter().map(|s| s.iter().map(|c| sample_contour(c).iter().map(|p| [p[0] as f64, p[1] as f64]).collect()).collect()).collect();
    run_boolean(op, &flat).into_iter().map(|sh| {
        let mut rings: Vec<Vec<Seg>> = sh.into_iter().map(|r| ring_to_straight_segs(&r)).filter(|s| s.len() >= 2).collect();
        // first ring = outer, rest = holes (i_overlay convention)
        if rings.is_empty() { ResultShape { outer: vec![], holes: vec![] } }
        else { let outer = rings.remove(0); ResultShape { outer, holes: rings } }
    }).filter(|rs| rs.outer.len() >= 2).collect()
}

fn ring_to_straight_segs(ring: &Ring) -> Vec<Seg> {
    let m = ring.len(); if m < 3 { return vec![]; }
    (0..m).map(|i| {
        let a = [ring[i][0] as f32, ring[i][1] as f32];
        let b = [ring[(i+1)%m][0] as f32, ring[(i+1)%m][1] as f32];
        (a, a, b, b)
    }).collect()
}

// ============================ i_overlay (polygon, fallback) ============================

fn flatten(shapes: &[Shape]) -> Shape { shapes.iter().flatten().cloned().collect() }

fn fold(shapes: &[Shape], rule: OverlayRule) -> Vec<Shape> {
    if shapes.is_empty() { return vec![]; }
    if shapes.len() == 1 { return vec![shapes[0].clone()]; }
    let mut acc: Shape = shapes[0].clone();
    let mut last: Vec<Shape> = vec![];
    for s in &shapes[1..] { last = acc.overlay(s, rule, FillRule::EvenOdd); acc = flatten(&last); }
    last
}

/// Polygon boolean over flat rings (returns simplified flat result shapes).
pub fn run_boolean(op: BoolOp, shapes: &[Shape]) -> Vec<Shape> {
    if shapes.len() < 2 { return vec![]; }
    let raw = match op {
        BoolOp::Unite => fold(shapes, OverlayRule::Union),
        BoolOp::Intersect => fold(shapes, OverlayRule::Intersect),
        BoolOp::Exclude => fold(shapes, OverlayRule::Xor),
        BoolOp::MinusFront => {
            let clip = flatten(&fold(&shapes[1..], OverlayRule::Union));
            shapes[0].overlay(&clip, OverlayRule::Difference, FillRule::EvenOdd)
        }
    };
    raw.into_iter()
        .map(|sh| sh.into_iter().map(|r| rdp(&r, 0.75)).filter(|r| r.len() >= 3).collect::<Shape>())
        .filter(|sh: &Shape| !sh.is_empty())
        .collect()
}

fn perp(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0]-a[0], b[1]-a[1]);
    let len2 = dx*dx + dy*dy;
    let t = if len2 < 1e-12 { 0.0 } else { (((p[0]-a[0])*dx + (p[1]-a[1])*dy) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (a[0] + t*dx, a[1] + t*dy);
    ((p[0]-cx).powi(2) + (p[1]-cy).powi(2)).sqrt()
}
fn rdp(pts: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    if pts.len() < 3 { return pts.to_vec(); }
    let (a, b) = (pts[0], pts[pts.len()-1]);
    let (mut idx, mut dmax) = (0usize, 0.0f64);
    for i in 1..pts.len()-1 { let d = perp(pts[i], a, b); if d > dmax { dmax = d; idx = i; } }
    if dmax > tol {
        let mut left = rdp(&pts[..=idx], tol);
        let right = rdp(&pts[idx..], tol);
        left.pop();
        left.extend(right);
        left
    } else { vec![a, b] }
}
