// Adapted from VectorCraft crates/pathops/src/{planar,pathfinder}.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
//! Polygon face arrangement on i_overlay; no linesweeper or UI dependencies.
//! Coverage is even-odd, matching Varos compound fills. Cubics are sampled at 32 steps.
use crate::boolean::{Ring, Seg, Shape};
use crate::geom::{cubic, point_in_poly, Pt};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::{single::SingleFloatOverlay, slice::FloatSlice},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathfinderOp {
    Divide,
    Trim,
    Merge,
    Crop,
    Outline,
    MinusBack,
}
#[derive(Clone, Debug)]
pub struct Face {
    pub shape: Shape,
    /// Ascending input indices; last is the front-most owner. Each owner has odd winding parity.
    pub owners: Vec<usize>,
    /// Signed winding of each input at an interior point; fill membership is odd parity.
    pub winding: Vec<i32>,
}
impl Face {
    pub fn contains(&self, p: Pt) -> bool {
        contains(&self.shape, p)
    }
}
#[derive(Clone, Debug)]
pub struct Piece {
    pub shape: Shape,
    pub owner: usize,
    pub closed: bool,
}

pub fn flatten(contours: &[Vec<Seg>]) -> Shape {
    contours
        .iter()
        .map(|c| {
            c.iter()
                .flat_map(|s| {
                    (0..32).map(move |i| {
                        let p = cubic(s.0, s.1, s.2, s.3, i as f32 / 32.0);
                        [p[0] as f64, p[1] as f64]
                    })
                })
                .collect()
        })
        .collect()
}
pub fn contains(shape: &Shape, p: Pt) -> bool {
    shape.iter().filter(|r| point_in_poly(&r.iter().map(|p| [p[0] as f32, p[1] as f32]).collect::<Vec<_>>(), p)).count()
        % 2
        == 1
}
pub fn area(shape: &Shape) -> f64 {
    shape
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let a = (0..r.len())
                .map(|k| {
                    let p = r[k];
                    let q = r[(k + 1) % r.len()];
                    p[0] * q[1] - q[0] * p[1]
                })
                .sum::<f64>()
                .abs()
                * 0.5;
            if i == 0 {
                a
            } else {
                -a
            }
        })
        .sum()
}
fn overlay(a: &Shape, b: &Shape, rule: OverlayRule) -> Vec<Shape> {
    a.overlay(b, rule, FillRule::EvenOdd)
}
pub fn union(shapes: &[Shape]) -> Vec<Shape> {
    let mut acc: Shape = Vec::new();
    let mut out = Vec::new();
    for s in shapes {
        out = overlay(&acc, s, OverlayRule::Union);
        acc = out.iter().flatten().cloned().collect();
    }
    out
}
/// Incrementally split both sides of every incoming boundary. Disconnected components stay separate,
/// holes remain attached to their outer ring, and identical coverage never duplicates area.
pub fn faces(shapes: &[Shape]) -> Vec<Face> {
    let mut out: Vec<Face> = Vec::new();
    for (owner, shape) in shapes.iter().enumerate() {
        let covered: Shape = out.iter().flat_map(|f| f.shape.clone()).collect();
        let mut next = Vec::new();
        for f in out {
            for s in overlay(&f.shape, shape, OverlayRule::Difference) {
                next.push(Face { shape: s, owners: f.owners.clone(), winding: Vec::new() });
            }
            let mut owners = f.owners;
            owners.push(owner);
            for s in overlay(&f.shape, shape, OverlayRule::Intersect) {
                next.push(Face { shape: s, owners: owners.clone(), winding: Vec::new() });
            }
        }
        for s in overlay(shape, &covered, OverlayRule::Difference) {
            next.push(Face { shape: s, owners: vec![owner], winding: Vec::new() });
        }
        out = next;
    }
    for face in &mut out {
        face.winding = winding_at(&face.shape, shapes);
    }
    out
}
/// Open selected paths cut the filled faces without themselves adding filled area.
pub fn faces_with_cuts(shapes: &[Shape], cuts: &[Ring]) -> Vec<Face> {
    let mut regions = faces(shapes);
    for cut in cuts {
        regions = regions
            .into_iter()
            .flat_map(|face| {
                knife(&face.shape, cut).into_iter().map(move |shape| Face {
                    shape,
                    owners: face.owners.clone(),
                    winding: face.winding.clone(),
                })
            })
            .collect();
    }
    regions
}
fn winding_at(face: &Shape, shapes: &[Shape]) -> Vec<i32> {
    let Some(r) = face.first() else { return vec![0; shapes.len()] };
    let signed = (0..r.len())
        .map(|i| {
            let a = r[i];
            let b = r[(i + 1) % r.len()];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .signum();
    let mut point = r.first().copied().unwrap_or([0., 0.]);
    'outer: for i in 0..r.len() {
        let a = r[i];
        let b = r[(i + 1) % r.len()];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        for scale in [1e-4, 1e-5, 1e-3] {
            let p = [(a[0] + b[0]) * 0.5 - dy * signed * scale, (a[1] + b[1]) * 0.5 + dx * signed * scale];
            if contains(face, [p[0] as f32, p[1] as f32]) {
                point = p;
                break 'outer;
            }
        }
    }
    shapes
        .iter()
        .map(|shape| {
            shape
                .iter()
                .map(|ring| {
                    (0..ring.len())
                        .map(|i| {
                            let a = ring[i];
                            let b = ring[(i + 1) % ring.len()];
                            let cross = (b[0] - a[0]) * (point[1] - a[1]) - (point[0] - a[0]) * (b[1] - a[1]);
                            if a[1] <= point[1] && b[1] > point[1] && cross > 0. {
                                1
                            } else if a[1] > point[1] && b[1] <= point[1] && cross < 0. {
                                -1
                            } else {
                                0
                            }
                        })
                        .sum::<i32>()
                })
                .sum()
        })
        .collect()
}
/// Paint keys compare fill and opacity, provided by the editor; Merge discards strokes.
pub fn pathfinder(op: PathfinderOp, shapes: &[Shape], keys: &[usize]) -> Vec<Piece> {
    if shapes.len() != keys.len() || shapes.is_empty() {
        return Vec::new();
    }
    let regions = faces(shapes);
    let last = shapes.len() - 1;
    let piece = |shape, owner| Piece { shape, owner, closed: true };
    if op == PathfinderOp::Divide {
        return regions.into_iter().filter_map(|f| Some(piece(f.shape, *f.owners.last()?))).collect();
    }
    if op == PathfinderOp::Outline {
        return outlines(shapes, &regions);
    }
    let mut groups: Vec<(usize, Vec<Shape>)> = Vec::new();
    for f in regions {
        let owner = match op {
            PathfinderOp::Crop if f.owners.contains(&last) => f.owners.iter().copied().rfind(|i| *i != last),
            PathfinderOp::Crop => None,
            PathfinderOp::MinusBack => (f.owners == [last]).then_some(last),
            _ => f.owners.last().copied(),
        };
        let Some(owner) = owner else { continue };
        let group =
            groups
                .iter()
                .position(|(i, _)| if op == PathfinderOp::Merge { keys[*i] == keys[owner] } else { *i == owner });
        if let Some(g) = group {
            groups[g].1.push(f.shape);
        } else {
            groups.push((owner, vec![f.shape]));
        }
    }
    groups.into_iter().flat_map(|(owner, s)| union(&s).into_iter().map(move |s| piece(s, owner))).collect()
}
fn on_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> bool {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = d[0] * d[0] + d[1] * d[1];
    if l == 0.0 {
        return false;
    }
    let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l;
    let q = [a[0] + t * d[0], a[1] + t * d[1]];
    (-1e-6..=1.000001).contains(&t) && (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-4
}
/// Face edges are already split at intersections. Emit only original input boundaries, once per
/// owner; never invent the internal seams of another shape as that owner's outline.
fn outlines(shapes: &[Shape], regions: &[Face]) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for f in regions {
        for r in &f.shape {
            for k in 0..r.len() {
                let a = r[k];
                let b = r[(k + 1) % r.len()];
                let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
                for (owner, s) in shapes.iter().enumerate() {
                    if !s.iter().any(|r| (0..r.len()).any(|j| on_segment(mid, r[j], r[(j + 1) % r.len()]))) {
                        continue;
                    }
                    if out.iter().any(|p| p.owner == owner && (p.shape[0] == [a, b] || p.shape[0] == [b, a])) {
                        continue;
                    }
                    out.push(Piece { shape: vec![vec![a, b]], owner, closed: false });
                }
            }
        }
    }
    out
}
pub fn knife(shape: &Shape, line: &Ring) -> Vec<Shape> {
    shape.slice_by(line, FillRule::EvenOdd)
}
pub fn subtract(shape: &Shape, brush: &[Shape]) -> Vec<Shape> {
    let clip: Shape = union(brush).into_iter().flatten().collect();
    overlay(shape, &clip, OverlayRule::Difference)
}
/// Round brush sweep, including caps and joins. Capsules are unioned before subtraction.
pub fn brush(points: &[Pt], radius: f32) -> Vec<Shape> {
    if points.is_empty() || !radius.is_finite() || radius <= 0.0 || points.iter().flatten().any(|v| !v.is_finite()) {
        return Vec::new();
    }
    let mut shapes: Vec<Shape> = points
        .iter()
        .map(|p| {
            vec![(0..32)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / 32.0;
                    [p[0] as f64 + radius as f64 * a.cos(), p[1] as f64 + radius as f64 * a.sin()]
                })
                .collect()]
        })
        .collect();
    for w in points.windows(2) {
        let dx = w[1][0] - w[0][0];
        let dy = w[1][1] - w[0][1];
        let len = dx.hypot(dy);
        if len <= f32::EPSILON {
            continue;
        }
        let n = [-dy / len * radius, dx / len * radius];
        shapes.push(vec![vec![
            [w[0][0] as f64 + n[0] as f64, w[0][1] as f64 + n[1] as f64],
            [w[1][0] as f64 + n[0] as f64, w[1][1] as f64 + n[1] as f64],
            [w[1][0] as f64 - n[0] as f64, w[1][1] as f64 - n[1] as f64],
            [w[0][0] as f64 - n[0] as f64, w[0][1] as f64 - n[1] as f64],
        ]]);
    }
    shapes
}
