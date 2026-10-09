// Adapted from VectorCraft crates/pathops/src/edit.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
//! Pure path edits: preserve object metadata and process hole rings independently.
//! Newly created anchors have ID zero; document insertion must allocate fresh IDs.
use super::{
    fit::{cubics_to_path, end_tangent, fit_cubics, start_tangent, unit},
    kurbo::{anchor, point, pt},
};
use crate::model::{Anchor, AnchorRing, Path};
use ::kurbo::{CubicBez, ParamCurve, ParamCurveNearest, Point};
use std::ops::Range;

pub(crate) fn curves(anchors: &[Anchor], closed: bool) -> Vec<CubicBez> {
    if anchors.len() < 2 {
        return Vec::new();
    }
    (0..if closed { anchors.len() } else { anchors.len() - 1 })
        .map(|i| {
            let a = &anchors[i];
            let b = &anchors[(i + 1) % anchors.len()];
            if a.hout.is_none() && b.hin.is_none() {
                let (p, q) = (point(a.p), point(b.p));
                CubicBez::new(p, p.lerp(q, 1. / 3.), p.lerp(q, 2. / 3.), q)
            } else {
                CubicBez::new(point(a.p), point(a.hout.unwrap_or(a.p)), point(b.hin.unwrap_or(b.p)), point(b.p))
            }
        })
        .collect()
}
/// Simplify by refitting runs separated by corners sharper than `corner_angle` degrees.
/// `curve_precision` is maximum geometric error in canvas units, not a UI percentage.
/// A conservative bidirectional curve-distance certificate rejects fits over the bound;
/// failed certification or a fit that increases anchor count leaves that ring unchanged.
/// Retained endpoints/corners keep IDs. Newly fitted interior anchors have ID zero.
pub fn simplify(path: &Path, curve_precision: f64, corner_angle: f64) -> Path {
    if !curve_precision.is_finite() || curve_precision <= 0. {
        return path.clone();
    }
    let mut out = path.clone();
    out.anchors = simplify_ring(&path.anchors, path.closed, curve_precision, corner_angle);
    out.holes = path.holes.iter().map(|r| simplify_ring(r, true, curve_precision, corner_angle)).collect();
    out
}
fn simplify_ring(anchors: &[Anchor], closed: bool, tol: f64, corner_angle: f64) -> Vec<Anchor> {
    let cs = curves(anchors, closed);
    let n = cs.len();
    if n < 2 {
        return anchors.to_vec();
    }
    let mut corners = vec![0];
    for i in 1..n {
        let turn = end_tangent(&cs[i - 1]).dot(start_tangent(&cs[i])).clamp(-1., 1.).acos().to_degrees();
        if turn > corner_angle.clamp(0., 180.) {
            corners.push(i);
        }
    }
    corners.push(n);
    let mut result: Vec<Anchor> = Vec::new();
    for w in corners.windows(2) {
        let run = &cs[w[0]..w[1]];
        let mut samples = Vec::new();
        for c in run {
            // Adaptive hull subdivision keeps even small loops visible to the fitter.
            sample_curve(*c, tol * 0.125, 0, &mut samples);
        }
        samples.push(run[run.len() - 1].p3);
        let fitted = fit_cubics(&samples, start_tangent(&run[0]), end_tangent(&run[run.len() - 1]), tol * 0.5);
        let candidate = cubics_to_path(&fitted);
        let converted = curves(&candidate.anchors, false); // certificate covers f64 -> f32 quantisation too
        if !within(run, &converted, tol) || !within(&converted, run, tol) {
            return anchors.to_vec();
        }
        let mut a = candidate.anchors;
        if a.is_empty() {
            return anchors.to_vec();
        }
        a[0].id = anchors[w[0]].id;
        a[0].smooth = anchors[w[0]].smooth;
        let last = a.len() - 1;
        let source = &anchors[w[1] % anchors.len()];
        a[last].id = source.id;
        a[last].smooth = source.smooth;
        if let Some(prev) = result.last_mut() {
            prev.hout = a[0].hout;
            a.remove(0);
        }
        result.extend(a);
    }
    if closed {
        if let Some(last) = result.pop() {
            result[0].hin = last.hin;
        }
    } else {
        result[0].hin = anchors[0].hin;
        if let Some(last) = result.last_mut() {
            last.hout = anchors[anchors.len() - 1].hout;
        }
    }
    if result.len() > anchors.len() {
        anchors.to_vec()
    } else {
        result
    }
}
fn split(c: CubicBez) -> (CubicBez, CubicBez) {
    let a = c.p0.lerp(c.p1, 0.5);
    let b = c.p1.lerp(c.p2, 0.5);
    let d = c.p2.lerp(c.p3, 0.5);
    let e = a.lerp(b, 0.5);
    let f = b.lerp(d, 0.5);
    let m = e.lerp(f, 0.5);
    (CubicBez::new(c.p0, a, e, m), CubicBez::new(m, f, d, c.p3))
}
fn sample_curve(c: CubicBez, spacing: f64, depth: usize, out: &mut Vec<Point>) {
    let diameter = c.p0.distance(c.p1).max(c.p0.distance(c.p2)).max(c.p0.distance(c.p3));
    if diameter <= spacing || depth >= 8 {
        out.push(c.p0);
        return;
    }
    let (a, b) = split(c);
    sample_curve(a, spacing, depth + 1, out);
    sample_curve(b, spacing, depth + 1, out);
}
// The cubic lies in its control hull. Distance to a set is 1-Lipschitz: a midpoint's distance
// plus its hull radius is an upper bound for every point. Nearest search supplies an actual
// target point, hence cannot underestimate distance even if its numerical search is imperfect.
fn within(source: &[CubicBez], target: &[CubicBez], tol: f64) -> bool {
    !target.is_empty() && source.iter().all(|c| certify(*c, target, tol, 0))
}
fn certify(c: CubicBez, target: &[CubicBez], tol: f64, depth: usize) -> bool {
    let mid = c.eval(0.5);
    let d = target.iter().map(|t| mid.distance(t.eval(t.nearest(mid, tol * 0.01).t))).fold(f64::INFINITY, f64::min);
    let radius = [c.p0, c.p1, c.p2, c.p3].into_iter().map(|p| p.distance(mid)).fold(0., f64::max);
    if d + radius <= tol {
        return true;
    }
    if d > tol || depth >= 20 {
        return false;
    }
    let (a, b) = split(c);
    certify(a, target, tol, depth + 1) && certify(b, target, tol, depth + 1)
}
/// Blend handles towards Catmull-Rom tangents; strength 0–1. `range` selects anchor indices
/// in each ring independently. Positions and open endpoint handles remain unchanged.
pub fn smooth(path: &Path, strength: f32, range: Range<usize>) -> Path {
    let mut out = path.clone();
    let t = if strength.is_finite() { strength.clamp(0., 1.) as f64 } else { 0. };
    if t == 0. {
        return out;
    }
    smooth_ring(&mut out.anchors, path.closed, t, &range);
    for ring in &mut out.holes {
        smooth_ring(ring, true, t, &range);
    }
    out
}
fn smooth_ring(ring: &mut [Anchor], closed: bool, t: f64, range: &Range<usize>) {
    let n = ring.len();
    if n < 3 {
        return;
    }
    let original = ring.to_vec();
    for i in range.start.min(n)..range.end.min(n) {
        if !closed && (i == 0 || i == n - 1) {
            continue;
        }
        let (prev, next, p) =
            (point(original[(i + n - 1) % n].p), point(original[(i + 1) % n].p), point(original[i].p));
        let Some(dir) = unit(next - prev) else { continue };
        ring[i].hin =
            Some(pt(point(original[i].hin.unwrap_or(original[i].p)).lerp(p - dir * (p.distance(prev) / 3.), t)));
        ring[i].hout =
            Some(pt(point(original[i].hout.unwrap_or(original[i].p)).lerp(p + dir * (p.distance(next) / 3.), t)));
        let u = p - point(ring[i].hin.unwrap_or(ring[i].p));
        let v = point(ring[i].hout.unwrap_or(ring[i].p)) - p;
        ring[i].smooth = u.dot(v) > 0. && u.cross(v).abs() <= 1e-6 * u.hypot() * v.hypot();
    }
}
/// Insert exact de Casteljau midpoints in every segment; existing IDs/flags survive.
pub fn add_anchor_points(path: &Path) -> Path {
    let mut out = path.clone();
    out.anchors = bisect(&path.anchors, path.closed);
    out.holes = path.holes.iter().map(|r| bisect(r, true)).collect();
    out
}
fn bisect(ring: &[Anchor], closed: bool) -> Vec<Anchor> {
    let cs = curves(ring, closed);
    if cs.is_empty() {
        return ring.to_vec();
    }
    let mut out = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        let (left, right) = split(*c);
        let mut a = ring[i].clone();
        let previous = if i > 0 {
            Some(i - 1)
        } else if closed {
            Some(cs.len() - 1)
        } else {
            None
        };
        if let Some(j) = previous {
            if ring[j].hout.is_some() || a.hin.is_some() {
                a.hin = Some(pt(split(cs[j]).1.p2));
            }
        }
        let curved = ring[i].hout.is_some() || ring[(i + 1) % ring.len()].hin.is_some();
        let mut mid = anchor(left.p3);
        if curved {
            a.hout = Some(pt(left.p1));
            mid.hin = Some(pt(left.p2));
            mid.hout = Some(pt(right.p1));
            mid.smooth = true;
        }
        out.extend([a, mid]);
    }
    if !closed {
        let mut last = ring[ring.len() - 1].clone();
        if last.hin.is_some() || ring[ring.len() - 2].hout.is_some() {
            last.hin = Some(pt(split(cs[cs.len() - 1]).1.p2));
        }
        out.push(last);
    }
    out
}
/// Axis used by [`average_anchors`]. Horizontal changes y; vertical changes x.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AverageAxis {
    Horizontal,
    Vertical,
    Both,
}
fn get(path: &Path, address: (AnchorRing, usize)) -> Option<&Anchor> {
    match address.0 {
        AnchorRing::Outer => path.anchors.get(address.1),
        AnchorRing::Hole(i) => path.holes.get(i)?.get(address.1),
    }
}
/// Move selected `(ring, index)` anchors and their absolute handles to the average on the chosen axis.
/// Invalid addresses are ignored, duplicate addresses count once; IDs and flags survive.
pub fn average_anchors(path: &Path, selection: &[(AnchorRing, usize)], axis: AverageAxis) -> Path {
    let mut selected = Vec::new();
    for &a in selection {
        if !selected.contains(&a) && get(path, a).is_some() {
            selected.push(a);
        }
    }
    let mut out = path.clone();
    if selected.is_empty() {
        return out;
    }
    let c =
        selected.iter().filter_map(|&a| get(path, a)).fold(::kurbo::Vec2::ZERO, |sum, a| sum + point(a.p).to_vec2())
            / selected.len() as f64;
    for address in selected {
        let a = match address.0 {
            AnchorRing::Outer => out.anchors.get_mut(address.1),
            AnchorRing::Hole(i) => out.holes.get_mut(i).and_then(|r| r.get_mut(address.1)),
        };
        if let Some(a) = a {
            let p = point(a.p);
            let d = match axis {
                AverageAxis::Horizontal => ::kurbo::Vec2::new(0., c.y - p.y),
                AverageAxis::Vertical => ::kurbo::Vec2::new(c.x - p.x, 0.),
                AverageAxis::Both => c - p.to_vec2(),
            };
            a.p = pt(p + d);
            a.hin = a.hin.map(|h| pt(point(h) + d));
            a.hout = a.hout.map(|h| pt(point(h) + d));
        }
    }
    out
}
fn reverse(anchors: &mut [Anchor]) {
    anchors.reverse();
    for a in anchors {
        std::mem::swap(&mut a.hin, &mut a.hout);
    }
}
/// Join open paths nearest-endpoint-first, merging endpoints within tolerance, otherwise
/// inserting a straight connector. Closed/empty paths pass through. One open path is closed.
/// Input holes and object metadata survive; the first path of each merge owns the result style.
/// Callers must put all paths in one coordinate space, allocate zero anchor IDs, and wrap
/// the complete operation in one undo entry. Command/Bridge wiring belongs to the command lane.
/// Cached pair candidates require O(n²) distance evaluations and O(n²) storage.
pub fn join_open_paths(paths: &[Path], tolerance: f32) -> Vec<Path> {
    join_with_distance(paths, tolerance, super::dist)
}

// Stable slots and generations invalidate only candidates involving a merged path.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct JoinCandidate {
    distance_bits: u32,
    i: usize,
    j: usize,
    ei: bool,
    ej: bool,
    gi: usize,
    gj: usize,
}

fn join_candidate(
    open: &[Option<Path>],
    generations: &[usize],
    i: usize,
    j: usize,
    distance: &mut impl FnMut([f32; 2], [f32; 2]) -> f32,
) -> Option<JoinCandidate> {
    let (a, b) = (open[i].as_ref()?, open[j].as_ref()?);
    let mut best = JoinCandidate {
        distance_bits: f32::INFINITY.to_bits(),
        i,
        j,
        ei: false,
        ej: false,
        gi: generations[i],
        gj: generations[j],
    };
    for ei in [false, true] {
        for ej in [false, true] {
            let d = distance(
                a.anchors[if ei { a.anchors.len() - 1 } else { 0 }].p,
                b.anchors[if ej { b.anchors.len() - 1 } else { 0 }].p,
            );
            if d < f32::from_bits(best.distance_bits) {
                best.distance_bits = d.to_bits();
                best.ei = ei;
                best.ej = ej;
            }
        }
    }
    Some(best)
}

fn join_with_distance(
    paths: &[Path],
    tolerance: f32,
    mut distance: impl FnMut([f32; 2], [f32; 2]) -> f32,
) -> Vec<Path> {
    let tolerance = if tolerance.is_finite() { tolerance.max(0.) } else { 0. };
    let (mut open, mut out): (Vec<_>, Vec<_>) = paths.iter().cloned().partition(|p| !p.closed && p.anchors.len() >= 2);
    if open.len() == 1 {
        let mut p = open.remove(0);
        let n = p.anchors.len();
        if n > 2 && super::dist(p.anchors[0].p, p.anchors[n - 1].p) <= tolerance {
            if let Some(last) = p.anchors.pop() {
                p.anchors[0].hin = last.hin.map(|h| super::add(h, super::sub(p.anchors[0].p, last.p)));
            }
        } else {
            p.anchors[0].hin = None;
            if let Some(last) = p.anchors.last_mut() {
                last.hout = None;
            }
        }
        p.closed = true;
        out.push(p);
        return out;
    }
    let mut open: Vec<Option<Path>> = open.into_iter().map(Some).collect();
    let mut generations = vec![0; open.len()];
    let mut candidates = std::collections::BinaryHeap::new();
    for i in 0..open.len() {
        for j in i + 1..open.len() {
            if let Some(candidate) = join_candidate(&open, &generations, i, j, &mut distance) {
                candidates.push(std::cmp::Reverse(candidate));
            }
        }
    }
    while let Some(std::cmp::Reverse(candidate)) = candidates.pop() {
        let JoinCandidate { distance_bits, i, j, ei, ej, gi, gj } = candidate;
        if generations[i] != gi || generations[j] != gj || open[i].is_none() || open[j].is_none() {
            continue;
        }
        let d = f32::from_bits(distance_bits);
        let Some(mut b) = open[j].take() else { continue };
        let Some(a) = open[i].as_mut() else { continue };
        if !ei {
            reverse(&mut a.anchors);
        }
        if ej {
            reverse(&mut b.anchors);
        }
        if d <= tolerance {
            let first = b.anchors.remove(0);
            if let Some(last) = a.anchors.last_mut() {
                last.hout = first.hout.map(|h| super::add(h, super::sub(last.p, first.p)));
                last.smooth = false;
            }
        } else {
            if let Some(last) = a.anchors.last_mut() {
                last.hout = None;
                last.smooth = false;
            }
            if let Some(first) = b.anchors.first_mut() {
                first.hin = None;
                first.smooth = false;
            }
        }
        a.anchors.extend(b.anchors);
        a.holes.extend(b.holes);
        generations[i] += 1;
        generations[j] += 1;
        for k in 0..open.len() {
            if k != i {
                if let Some(candidate) = join_candidate(&open, &generations, i.min(k), i.max(k), &mut distance) {
                    candidates.push(std::cmp::Reverse(candidate));
                }
            }
        }
    }
    out.extend(open.into_iter().flatten());
    out
}

#[cfg(test)]
mod join_tests {
    use super::*;

    #[test]
    fn thousand_paths_use_quadratic_distance_evaluations() {
        let n = 1000;
        let paths: Vec<_> =
            (0..n).map(|i| super::super::shapes::line([i as f32 * 3., 0.], [i as f32 * 3. + 1., 0.])).collect();
        let mut evaluations = 0;
        let joined = join_with_distance(&paths, 0., |a, b| {
            evaluations += 1;
            super::super::dist(a, b)
        });
        assert_eq!(evaluations, 4 * (n - 1) * (n - 1));
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].anchors.len(), n * 2);
        for (i, pair) in joined[0].anchors.as_chunks::<2>().0.iter().enumerate() {
            assert_eq!(pair[0].p, [i as f32 * 3., 0.]);
            assert_eq!(pair[1].p, [i as f32 * 3. + 1., 0.]);
        }
    }
}
