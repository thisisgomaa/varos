// Adapted from VectorCraft crates/trace/src/fit.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
//! Closed-loop Douglas–Peucker followed by bounded cubic interpolation.
//! TODO: unify with the shared geom/fit adapter when that parallel lane lands.
use super::TraceOptions;
use crate::model::Anchor;
type Pt = [f64; 2];
fn distance(p: Pt, a: Pt, b: Pt) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = d[0] * d[0] + d[1] * d[1];
    let t = if l == 0. { 0. } else { ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l }.clamp(0., 1.);
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}
fn rdp(pts: &[Pt], start: usize, end: usize, tol: f64, keep: &mut [bool]) {
    let mut stack = vec![(start, end)];
    while let Some((a, b)) = stack.pop() {
        let mut best = (tol, a);
        for i in a + 1..b {
            let d = distance(pts[i], pts[a], pts[b]);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.1 != a {
            keep[best.1] = true;
            stack.push((a, best.1));
            stack.push((best.1, b));
        }
    }
}
pub(super) fn fit(raw: &[(i32, i32)], options: &TraceOptions, id: &mut u32) -> Vec<Anchor> {
    let mut pts: Vec<Pt> = raw.iter().map(|p| [p.0 as f64, p.1 as f64]).collect();
    let n = pts.len();
    if n < 3 {
        return vec![];
    }
    let tol = 0.35 + (100. - options.paths_fidelity) * 0.025;
    let far = (1..n)
        .max_by(|&a, &b| distance(pts[a], pts[0], pts[0]).total_cmp(&distance(pts[b], pts[0], pts[0])))
        .unwrap_or(n / 2);
    pts.push(pts[0]);
    let mut keep = vec![false; n + 1];
    keep[0] = true;
    keep[far] = true;
    rdp(&pts, 0, far, tol, &mut keep);
    rdp(&pts, far, n, tol, &mut keep);
    let mut indices: Vec<usize> = (0..n).filter(|i| keep[*i]).collect();
    if indices.len() < 3 {
        indices = (0..n).collect();
    }
    let poly: Vec<Pt> = indices.iter().map(|i| pts[*i]).collect();
    let n = poly.len();
    let mut anchors: Vec<Anchor> = poly
        .iter()
        .map(|p| {
            let aid = *id;
            *id += 1;
            Anchor { id: aid, p: [p[0] as f32, p[1] as f32], hin: None, hout: None, smooth: false }
        })
        .collect();
    for i in 0..n {
        let prev = poly[(i + n - 1) % n];
        let p = poly[i];
        let next = poly[(i + 1) % n];
        let a = [p[0] - prev[0], p[1] - prev[1]];
        let b = [next[0] - p[0], next[1] - p[1]];
        let angle = ((a[0] * b[0] + a[1] * b[1]) / (a[0].hypot(a[1]) * b[0].hypot(b[1]))).clamp(-1., 1.).acos();
        if angle < (100. - options.corners) * 0.012 + 0.20 {
            let tangent = [next[0] - prev[0], next[1] - prev[1]];
            let len = tangent[0].hypot(tangent[1]);
            if len > 0. {
                let inside = a[0].hypot(a[1]) / 3.;
                let outside = b[0].hypot(b[1]) / 3.;
                anchors[i].hin =
                    Some([(p[0] - tangent[0] / len * inside) as f32, (p[1] - tangent[1] / len * inside) as f32]);
                anchors[i].hout =
                    Some([(p[0] + tangent[0] / len * outside) as f32, (p[1] + tangent[1] / len * outside) as f32]);
                anchors[i].smooth = true;
            }
        }
    }
    // Cubics that depart from the source boundary beyond tolerance fall back to polygon edges.
    for i in 0..n {
        let j = (i + 1) % n;
        let a = anchors[i].p;
        let b = anchors[j].p;
        let c = anchors[i].hout.unwrap_or(a);
        let d = anchors[j].hin.unwrap_or(b);
        let outside = (1..16).any(|s| {
            let t = s as f64 / 16.;
            let u = 1. - t;
            let p = [
                u * u * u * a[0] as f64
                    + 3. * u * u * t * c[0] as f64
                    + 3. * u * t * t * d[0] as f64
                    + t * t * t * b[0] as f64,
                u * u * u * a[1] as f64
                    + 3. * u * u * t * c[1] as f64
                    + 3. * u * t * t * d[1] as f64
                    + t * t * t * b[1] as f64,
            ];
            pts[indices[i]..=if j == 0 { pts.len() - 1 } else { indices[j] }]
                .windows(2)
                .map(|e| distance(p, e[0], e[1]))
                .fold(f64::INFINITY, f64::min)
                > tol + 0.5
        });
        if outside {
            anchors[i].hout = None;
            anchors[j].hin = None;
            anchors[i].smooth = false;
            anchors[j].smooth = false;
        }
    }
    anchors
}
