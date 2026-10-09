//! Shared bounded evaluator; derived geometry is never persisted.
// Adapted VectorCraft doc/src/blend.rs:35-380 and doc/src/live.rs @ a469568.
// Copyright 2026 ArtCraft Team. MIT OR Apache-2.0; see varos/NOTICE.
use super::{map_path, sources, Envelope, Kind, Orientation, Repeat, Warp};
use crate::{
    geom::Pt,
    model::{Anchor, Document, Node, Paint, Path},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
pub(crate) const MAX_ANCHORS: usize = 200_000;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Stats {
    pub evaluations: usize,
    pub hits: usize,
    pub document_evaluations: usize,
    pub document_hits: usize,
}
#[derive(Clone, Default)]
pub struct Cache(Arc<Mutex<State>>);
#[derive(Default)]
struct State {
    entries: HashMap<u32, Entry>,
    stats: Stats,
    snapshot: Option<Snapshot>,
}
struct Snapshot {
    source: Document,
    isolation: Option<u32>,
    output: Arc<Document>,
}
struct Entry {
    kind: Kind,
    sources: Vec<Path>,
    spine: Option<Path>,
    paths: Arc<Vec<Path>>,
}
impl Cache {
    pub(super) fn document(&self, doc: &Document, isolation: Option<u32>) -> Result<Arc<Document>, String> {
        {
            let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(s) = &state.snapshot {
                if &s.source == doc && s.isolation == isolation {
                    let out = s.output.clone();
                    state.stats.document_hits += 1;
                    return Ok(out);
                }
            }
        }
        let output = Arc::new(super::evaluated_with(doc, self, isolation)?);
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.stats.document_evaluations += 1;
        state.snapshot = Some(Snapshot { source: doc.clone(), isolation, output: output.clone() });
        Ok(output)
    }
    pub fn stats(&self) -> Stats {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).stats
    }
    pub fn evaluate(&self, doc: &Document, node: &Node, kind: Kind) -> Result<Arc<Vec<Path>>, String> {
        let sources = sources(doc, node, kind)?;
        let spine = match kind {
            Kind::Blend { spine: Some(id), .. } => doc.paths.iter().find(|p| p.id == id).cloned(),
            _ => None,
        };
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.entries.retain(|id, _| doc.node(*id).is_some_and(|n| matches!(n.kind, crate::model::NodeKind::Live(_))));
        if let Some(e) = state.entries.get(&node.id) {
            if e.kind == kind && e.sources == sources && e.spine == spine {
                let paths = e.paths.clone();
                state.stats.hits += 1;
                return Ok(paths);
            }
        }
        check(doc, kind, &sources)?;
        let paths = Arc::new(evaluate(kind, &sources, spine.as_ref())?);
        state.stats.evaluations += 1;
        state.entries.insert(node.id, Entry { kind, sources, spine, paths: paths.clone() });
        Ok(paths)
    }
}
fn lerp(a: Pt, b: Pt, t: f32) -> Pt {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}
fn anchors(paths: &[Path]) -> usize {
    paths.iter().map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum()
}
pub(crate) fn check(doc: &Document, kind: Kind, paths: &[Path]) -> Result<usize, String> {
    if paths.is_empty() || paths.iter().any(|p| p.anchors.is_empty()) {
        return Err("Live sources must be nonempty paths".into());
    }
    if paths
        .iter()
        .flat_map(|p| p.anchors.iter().chain(p.holes.iter().flatten()))
        .flat_map(|a| [Some(a.p), a.hin, a.hout])
        .flatten()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err("Live source coordinates must be finite within +/-1,000,000 pt".into());
    }
    let copies = match kind {
        Kind::Blend { steps, spine, .. } => {
            if paths.len() != 2 || steps > 1000 {
                return Err("Blend needs two paths and 0–1000 steps".into());
            }
            let (a, b) = (&paths[0], &paths[1]);
            if a.closed != b.closed || a.holes.len() != b.holes.len() {
                return Err("Blend needs matching open/closed state and subpath count".into());
            }

            if let Some(id) = spine {
                let p = doc.paths.iter().find(|p| p.id == id).ok_or("Missing spine")?;
                if p.closed || p.anchors.len() < 2 || p.anchors.len() > 64 || !p.holes.is_empty() {
                    return Err("Spine must be an open contour".into());
                }
            }
            for i in 0..=usize::from(steps) + 1 {
                let t = i as f32 / (f32::from(steps) + 1.0);
                for paint in [colour(&a.fill, &b.fill, t), colour(&a.stroke, &b.stroke, t)] {
                    if let Paint::Gradient(g) = paint {
                        g.validate().map_err(|e| format!("Blend paint: {e}"))?;
                    }
                }
            }
            usize::from(steps) + 2
        }
        Kind::Repeat { repeat: Repeat::Radial { count, radius } } => {
            if count == 0 || count > 1000 || !radius.is_finite() || !(0.0..=1e6).contains(&radius) {
                return Err("Invalid radial count/radius".into());
            }
            usize::from(count)
        }
        Kind::Repeat { repeat: Repeat::Grid { rows, cols, gap } } => {
            if rows == 0
                || cols == 0
                || rows > 1000
                || cols > 1000
                || gap.iter().any(|v| !v.is_finite() || !(0.0..=1e6).contains(v))
            {
                return Err("Invalid grid rows/cols/gap".into());
            }
            usize::from(rows) * usize::from(cols)
        }
        Kind::Repeat { repeat: Repeat::Mirror { .. } } => 2,
        Kind::Envelope { envelope } => {
            match envelope {
                Envelope::Warp { bend, .. } if !bend.is_finite() || !(-1.0..=1.0).contains(&bend) => {
                    return Err("Warp bend must be finite in -1..1".into())
                }
                Envelope::Mesh { points } if points.iter().flatten().any(|v| !v.is_finite() || v.abs() > 1e6) => {
                    return Err("Invalid 2×2 mesh points".into())
                }
                _ => {}
            }
            if paths.iter().any(|p| matches!(p.fill, Paint::Gradient(_)) || matches!(p.stroke, Paint::Gradient(_))) {
                return Err("Envelope gradient warping is not supported yet".into());
            }
            64
        }
    };
    let count = if matches!(kind, Kind::Blend { .. }) { copies } else { paths.len().saturating_mul(copies) };
    let cost = if matches!(kind, Kind::Blend { .. }) {
        let (a, b) = common_paths(&paths[0], &paths[1])?;
        anchors(&[a, b]).saturating_mul(copies)
    } else {
        anchors(paths).saturating_mul(copies)
    };
    if count > 4096 || cost > MAX_ANCHORS {
        return Err("Live geometry budget exceeded".into());
    }
    Ok(cost)
}
fn bbox(paths: &[Path]) -> [f32; 4] {
    let mut b = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
    for pt in paths
        .iter()
        .flat_map(|p| p.anchors.iter().chain(p.holes.iter().flatten()))
        .flat_map(|a| [Some(a.p), a.hin, a.hout])
        .flatten()
    {
        b[0] = b[0].min(pt[0]);
        b[1] = b[1].min(pt[1]);
        b[2] = b[2].max(pt[0]);
        b[3] = b[3].max(pt[1]);
    }
    b
}
fn centre(p: &Path) -> Pt {
    let b = bbox(std::slice::from_ref(p));
    [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5]
}
fn colour(a: &Paint, b: &Paint, t: f32) -> Paint {
    match (a, b) {
        (Paint::Gradient(a), Paint::Gradient(b)) if a.kind == b.kind => {
            let mut g = if t < 0.5 { a.clone() } else { b.clone() };
            let mut offsets: Vec<f32> = a.stops.iter().chain(&b.stops).map(|s| s.offset).collect();
            offsets.sort_by(f32::total_cmp);
            offsets.dedup();
            g.stops = offsets
                .into_iter()
                .map(|offset| {
                    let x = a.sample_pad(offset);
                    let y = b.sample_pad(offset);
                    crate::gradient::Stop::new(offset, std::array::from_fn(|i| x[i] + (y[i] - x[i]) * t))
                })
                .collect();
            g.placement = std::array::from_fn(|i| a.placement[i] + (b.placement[i] - a.placement[i]) * t);
            g.focal = lerp(a.focal, b.focal, t);
            Paint::Gradient(g)
        }
        (Paint::Solid(a), Paint::Solid(b)) => Paint::Solid(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)),
        (Paint::None, Paint::Solid(b)) => {
            let mut b = *b;
            b[3] *= t;
            Paint::Solid(b)
        }
        (Paint::Solid(a), Paint::None) => {
            let mut a = *a;
            a[3] *= 1.0 - t;
            Paint::Solid(a)
        }
        _ => {
            if t < 0.5 {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}
fn anchor(a: &Anchor, b: &Anchor, t: f32) -> Anchor {
    Anchor {
        p: lerp(a.p, b.p, t),
        hin: (a.hin.is_some() || b.hin.is_some()).then(|| lerp(a.hin.unwrap_or(a.p), b.hin.unwrap_or(b.p), t)),
        hout: (a.hout.is_some() || b.hout.is_some()).then(|| lerp(a.hout.unwrap_or(a.p), b.hout.unwrap_or(b.p), t)),
        ..a.clone()
    }
}
fn spine_sample(path: &Path, t: f32) -> (Pt, f32) {
    let mut pts = vec![path.anchors[0].p];
    for pair in path.anchors.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        for i in 1..=64 {
            pts.push(crate::geom::cubic(a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p, i as f32 / 64.0));
        }
    }
    let mut left = t * pts.windows(2).map(|p| crate::geom::dist(p[0], p[1])).sum::<f32>();
    for pair in pts.windows(2) {
        let d = crate::geom::dist(pair[0], pair[1]);
        if left <= d && d > 0.0 {
            return (lerp(pair[0], pair[1], left / d), (pair[1][1] - pair[0][1]).atan2(pair[1][0] - pair[0][0]));
        }
        left -= d;
    }
    (*pts.last().unwrap_or(&path.anchors[0].p), 0.0)
}
fn rotate(pt: Pt, c: Pt, a: f32) -> Pt {
    let (s, k) = a.sin_cos();
    let x = pt[0] - c[0];
    let y = pt[1] - c[1];
    [c[0] + k * x - s * y, c[1] + s * x + k * y]
}
fn evaluate(kind: Kind, paths: &[Path], spine: Option<&Path>) -> Result<Vec<Path>, String> {
    let mut out = Vec::new();
    let bounds = bbox(paths);
    let c = [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5];
    let w = (bounds[2] - bounds[0]).max(1e-6);
    let h = (bounds[3] - bounds[1]).max(1e-6);
    match kind {
        Kind::Blend { steps, orientation, .. } => {
            let (a, b) = common_paths(&paths[0], &paths[1])?;
            let (a, b) = (&a, &b);
            for i in 0..=usize::from(steps) + 1 {
                let t = i as f32 / (f32::from(steps) + 1.0);
                let mut p = if t < 0.5 { a.clone() } else { b.clone() };
                p.anchors = a.anchors.iter().zip(&b.anchors).map(|(a, b)| anchor(a, b, t)).collect();
                p.holes = a
                    .holes
                    .iter()
                    .zip(&b.holes)
                    .map(|(a, b)| a.iter().zip(b).map(|(a, b)| anchor(a, b, t)).collect())
                    .collect();
                p.fill = colour(&a.fill, &b.fill, t);
                p.stroke = colour(&a.stroke, &b.stroke, t);
                p.stroke_width = a.stroke_width + (b.stroke_width - a.stroke_width) * t;
                p.opacity = a.opacity + (b.opacity - a.opacity) * t;
                let origin = lerp(centre(a), centre(b), t);
                let (target, angle) = spine
                    .map(|s| spine_sample(s, t))
                    .unwrap_or((origin, (centre(b)[1] - centre(a)[1]).atan2(centre(b)[0] - centre(a)[0])));
                map_path(&mut p, |pt| {
                    let q = if orientation == Orientation::Path { rotate(pt, origin, angle) } else { pt };
                    [q[0] + target[0] - origin[0], q[1] + target[1] - origin[1]]
                });
                out.push(p);
            }
        }
        Kind::Repeat { repeat } => {
            let count = match repeat {
                Repeat::Radial { count, .. } => usize::from(count),
                Repeat::Grid { rows, cols, .. } => usize::from(rows) * usize::from(cols),
                Repeat::Mirror { .. } => 2,
            };
            for i in 0..count {
                for source in paths {
                    let mut p = source.clone();
                    map_path(&mut p, |pt| match repeat {
                        Repeat::Radial { count, radius } => {
                            let a = i as f32 * std::f32::consts::TAU / f32::from(count);
                            let q = rotate(pt, c, a);
                            [q[0] + radius * a.cos(), q[1] + radius * a.sin()]
                        }
                        Repeat::Grid { cols, gap, .. } => [
                            pt[0] + (i % usize::from(cols)) as f32 * (w + gap[0]),
                            pt[1] + (i / usize::from(cols)) as f32 * (h + gap[1]),
                        ],
                        Repeat::Mirror { axis } if i == 1 => match axis {
                            super::Axis::Horizontal => [pt[0], 2.0 * bounds[3] - pt[1]],
                            super::Axis::Vertical => [2.0 * bounds[2] - pt[0], pt[1]],
                        },
                        _ => pt,
                    });
                    out.push(p);
                }
            }
        }
        Kind::Envelope { envelope } => {
            for source in paths {
                let mut p = source.clone();
                for ring in std::iter::once(&mut p.anchors).chain(p.holes.iter_mut()) {
                    let old = ring.clone();
                    let mut sampled = Vec::new();
                    for (i, a) in old.iter().enumerate() {
                        let mut q = a.clone();
                        q.hin = None;
                        q.hout = None;
                        sampled.push(q);
                        if i + 1 < old.len() || p.closed {
                            let next = &old[(i + 1) % old.len()];
                            for j in 1..64 {
                                let mut q = a.clone();
                                q.p = crate::geom::cubic(
                                    a.p,
                                    a.hout.unwrap_or(a.p),
                                    next.hin.unwrap_or(next.p),
                                    next.p,
                                    j as f32 / 64.0,
                                );
                                q.hin = None;
                                q.hout = None;
                                sampled.push(q);
                            }
                        }
                    }
                    *ring = sampled;
                }
                map_path(&mut p, |pt| {
                    let u = (pt[0] - bounds[0]) / w;
                    let v = (pt[1] - bounds[1]) / h;
                    match envelope {
                        Envelope::Mesh { points } => {
                            lerp(lerp(points[0], points[1], u), lerp(points[2], points[3], u), v)
                        }
                        Envelope::Warp { preset, bend } => match preset {
                            Warp::Arc => [pt[0], pt[1] - bend * h * 4.0 * u * (1.0 - u)],
                            Warp::Flag => [pt[0], pt[1] + bend * h * (u * std::f32::consts::TAU).sin()],
                            Warp::Bulge => [c[0] + (pt[0] - c[0]) * (1.0 + bend * 4.0 * v * (1.0 - v)), pt[1]],
                        },
                    }
                });
                out.push(p);
            }
        }
    }
    if out.iter().any(|p| [&p.fill, &p.stroke].iter().any(|p| matches!(p,Paint::Gradient(g) if g.validate().is_err())))
    {
        return Err("Blend gradient placement is singular".into());
    }
    if anchors(&out) > MAX_ANCHORS
        || out
            .iter()
            .flat_map(|p| p.anchors.iter().chain(p.holes.iter().flatten()))
            .any(|a| a.p.iter().any(|v| !v.is_finite()))
    {
        return Err("Evaluated geometry exceeds limits".into());
    }
    Ok(out)
}

fn common_paths(a: &Path, b: &Path) -> Result<(Path, Path), String> {
    fn common(a: &[Anchor], b: &[Anchor], closed: bool) -> Result<(Vec<Anchor>, Vec<Anchor>), String> {
        if a.len() == b.len() {
            return Ok((a.to_vec(), b.to_vec()));
        }
        let na = a.len().saturating_sub(usize::from(!closed));
        let nb = b.len().saturating_sub(usize::from(!closed));
        if na == 0 || nb == 0 {
            return Err("Blend contours need segments".into());
        }
        let (mut x, mut y) = (na, nb);
        while y != 0 {
            (x, y) = (y, x % y);
        }
        let n = na.checked_mul(nb / x).ok_or("Blend topology budget")?;
        if n > 4096 {
            return Err("Blend topology budget exceeded".into());
        }
        Ok((split_ring(a, closed, n / na), split_ring(b, closed, n / nb)))
    }
    let (mut a, mut b) = (a.clone(), b.clone());
    (a.anchors, b.anchors) = common(&a.anchors, &b.anchors, a.closed)?;
    for (x, y) in a.holes.iter_mut().zip(&mut b.holes) {
        (*x, *y) = common(x, y, true)?;
    }
    Ok((a, b))
}
fn split_ring(ring: &[Anchor], closed: bool, parts: usize) -> Vec<Anchor> {
    let n = ring.len() - usize::from(!closed);
    let mut out = Vec::new();
    let mut end_handles = Vec::new();
    for i in 0..n {
        let a = &ring[i];
        let b = &ring[(i + 1) % ring.len()];
        let p = [a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p];
        let derivative = |t: f32| {
            std::array::from_fn::<_, 2, _>(|axis| {
                3.0 * ((1.0 - t).powi(2) * (p[1][axis] - p[0][axis])
                    + 2.0 * (1.0 - t) * t * (p[2][axis] - p[1][axis])
                    + t * t * (p[3][axis] - p[2][axis]))
            })
        };
        for j in 0..parts {
            let t = j as f32 / parts as f32;
            let u = (j + 1) as f32 / parts as f32;
            let start = crate::geom::cubic(p[0], p[1], p[2], p[3], t);
            let end = crate::geom::cubic(p[0], p[1], p[2], p[3], u);
            let curve = a.hout.is_some() || b.hin.is_some();
            let da = derivative(t);
            let db = derivative(u);
            let k = (u - t) / 3.0;
            out.push(Anchor {
                id: a.id,
                p: start,
                hin: None,
                hout: curve.then(|| [start[0] + da[0] * k, start[1] + da[1] * k]),
                smooth: curve,
            });
            end_handles.push(curve.then(|| [end[0] - db[0] * k, end[1] - db[1] * k]));
        }
    }
    if !closed {
        let mut a = ring[ring.len() - 1].clone();
        a.hout = None;
        out.push(a);
    }
    let len = out.len();
    for (i, handle) in end_handles.into_iter().enumerate() {
        out[(i + 1) % len].hin = handle;
    }
    out
}
