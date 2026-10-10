// Adapted from VectorCraft effects/src/{distort,warp,stylize,util}.rs (MIT OR Apache-2.0).
// Copyright (c) 2026 ArtCraft Team and contributors. See NOTICE.
//! Bounded authored effects; every geometry consumer uses this evaluator.
use crate::{
    model::{Anchor, Path},
    stroke::StrokeJoin,
    Editor, Pt,
};
use kurbo::{ParamCurve, Shape};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarpStyle {
    Arc,
    ArcLower,
    ArcUpper,
    Arch,
    Bulge,
    ShellLower,
    ShellUpper,
    Flag,
    Wave,
    Fish,
    Rise,
    Fisheye,
    Inflate,
    Squeeze,
    Twist,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Offset {
        delta: f32,
        join: StrokeJoin,
        miter: f32,
    },
    ZigZag {
        size: f32,
        ridges: u32,
        smooth: bool,
    },
    Transform {
        copies: u32,
        #[serde(rename = "move")]
        movement: Pt,
        scale: Pt,
        rotate: f32,
        reflect: [bool; 2],
    },
    Warp {
        style: WarpStyle,
        bend: f32,
        h: f32,
        v: f32,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Tool,
    SetPerPath { paths: Vec<(u32, Vec<Effect>)> },
    PreviewAppend { ids: Vec<u32>, effect: Effect },
    Preview { ids: Vec<u32>, effects: Vec<Effect> },
    EndPreview { accept: bool },
    WidthLive { id: u32, profile: crate::width_profile::WidthProfile },
    Set { ids: Vec<u32>, effects: Vec<Effect> },
    Expand { ids: Vec<u32> },
    Width { ids: Vec<u32>, profile: Option<crate::width_profile::WidthProfile> },
}
pub const MAX_ANCHORS: usize = 100_000;
fn count(p: &Path) -> usize {
    p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()
}
pub fn validate(p: &Path) -> Result<(), String> {
    if p.effects.len() > 16 {
        return Err("at most 16 live effects".into());
    }
    let mut n = count(p);
    let finite = |v: f32, max: f32| v.is_finite() && v.abs() <= max;
    for e in &p.effects {
        let valid = match e {
            Effect::Offset { delta, miter, .. } => finite(*delta, 7200.) && (1.0..=1000.).contains(miter),
            Effect::ZigZag { size, ridges, .. } => {
                n = n.saturating_mul(*ridges as usize + 2);
                finite(*size, 10000.) && *ridges <= 100
            }
            Effect::Transform { copies, movement, scale, rotate, .. } => {
                n = n.saturating_mul(*copies as usize + 1);
                *copies <= 1000
                    && movement.iter().all(|v| finite(*v, 1e6))
                    && scale.iter().all(|v| finite(*v, 10.))
                    && finite(*rotate, 36000.)
            }
            Effect::Warp { bend, h, v, .. } => {
                n = n.saturating_mul(24);
                [*bend, *h, *v].iter().all(|v| finite(*v, 100.))
            }
        };
        if !valid || n > MAX_ANCHORS {
            return Err("invalid effect parameters or geometry budget exceeded".into());
        }
    }
    Ok(())
}
#[derive(Default)]
pub struct Cache {
    entries: Vec<(Path, Path)>,
    pub evaluations: u64,
}
impl Cache {
    pub fn resolve(&mut self, p: &Path) -> Result<Path, String> {
        if let Some((_, out)) = self.entries.iter().find(|(key, _)| key == p) {
            return Ok(out.clone());
        }
        let out = evaluate(p)?;
        self.evaluations += 1;
        self.entries.retain(|(key, _)| key.id != p.id);
        while !self.entries.is_empty()
            && (self.entries.len() >= 128
                || self.entries.iter().map(|(a, b)| count(a) + count(b)).sum::<usize>() + count(p) + count(&out)
                    > 200_000)
        {
            self.entries.remove(0);
        }
        self.entries.push((p.clone(), out.clone()));
        Ok(out)
    }
}
pub fn evaluated(p: &Path) -> Path {
    let Ok(parts) = crate::effects_document::resolved_many(p) else {
        let mut q = crate::live_corners::corners_only(p);
        q.effects.clear();
        return q;
    };
    combine(p, parts)
}
fn combine(source: &Path, parts: Vec<Path>) -> Path {
    let mut parts = parts.into_iter();
    let mut out = parts.next().unwrap_or_else(|| source.clone());
    for p in parts {
        out.holes.push(p.anchors);
        out.holes.extend(p.holes);
    }
    out
}
fn map_anchors(p: &mut Path, f: impl Fn(Pt) -> Pt) {
    for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
        a.p = f(a.p);
        a.hin = a.hin.map(&f);
        a.hout = a.hout.map(&f);
    }
}
fn anchors(points: &[Pt], closed: bool, smooth: bool) -> Vec<Anchor> {
    points
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let mut a = Anchor { id: 0, p, hin: None, hout: None, smooth };
            if smooth && points.len() > 1 {
                let n = points.len();
                let prev = points[if i > 0 {
                    i - 1
                } else if closed {
                    n - 1
                } else {
                    0
                }];
                let next = points[if i + 1 < n {
                    i + 1
                } else if closed {
                    0
                } else {
                    n - 1
                }];
                let d = [(next[0] - prev[0]) / 6., (next[1] - prev[1]) / 6.];
                a.hin = Some([p[0] - d[0], p[1] - d[1]]);
                a.hout = Some([p[0] + d[0], p[1] + d[1]]);
            }
            a
        })
        .collect()
}
fn zig(ring: &[Anchor], closed: bool, size: f32, ridges: u32, smooth: bool) -> Vec<Anchor> {
    let bp = crate::geom::kurbo::contour(ring, closed);
    let mut pts = Vec::new();
    for seg in bp.segments() {
        for j in 0..=ridges {
            let t = f64::from(j) / f64::from(ridges + 1);
            let q = seg.eval(t);
            let d = crate::width_geometry_helpers::tangent(&seg, t);
            let len = d.hypot();
            let amp = f64::from(size) * if pts.len() % 2 == 0 { 1. } else { -1. };
            pts.push([
                (q.x + if len > 1e-9 { d.y / len * amp } else { 0. }) as f32,
                (q.y - if len > 1e-9 { d.x / len * amp } else { 0. }) as f32,
            ]);
        }
    }
    if !closed {
        if let Some(a) = ring.last() {
            let tangent =
                bp.segments().last().map(|seg| crate::width_geometry_helpers::tangent(&seg, 1.)).unwrap_or_default();
            let sign = if pts.len().is_multiple_of(2) { 1. } else { -1. };
            pts.push([a.p[0] + tangent.y as f32 * size * sign, a.p[1] - tangent.x as f32 * size * sign]);
        }
    }
    anchors(&pts, closed, smooth)
}
fn evaluate_single(source: &Path, bounds: Option<kurbo::Rect>) -> Result<Path, String> {
    validate(source)?;
    let mut out = crate::live_corners::corners_only(source);
    out.effects.clear();
    for effect in &source.effects {
        match effect {
            Effect::Offset { delta, join, miter } => {
                let q = crate::path_advanced::offset(&out, *delta, *join, *miter)?;
                out.anchors = q.anchors;
                out.holes = q.holes;
                out.closed = q.closed;
            }
            Effect::ZigZag { size, ridges, smooth } => {
                if *size == 0. {
                    continue;
                }
                out.anchors = zig(&out.anchors, out.closed, *size, *ridges, *smooth);
                for h in &mut out.holes {
                    *h = zig(h, true, *size, *ridges, *smooth);
                }
            }
            Effect::Transform { copies, movement, scale, rotate, reflect } => {
                let b = bounds.unwrap_or_else(|| crate::geom::kurbo::to_bez_path(&out).bounding_box());
                let c = b.center();
                let (s, cos) = (-f64::from(*rotate).to_radians()).sin_cos();
                let f = |p: Pt| {
                    let x = (f64::from(p[0]) - c.x) * f64::from(scale[0]) * if reflect[0] { -1. } else { 1. };
                    let y = (f64::from(p[1]) - c.y) * f64::from(scale[1]) * if reflect[1] { -1. } else { 1. };
                    [
                        (c.x + x * cos - y * s + f64::from(movement[0])) as f32,
                        (c.y + x * s + y * cos + f64::from(movement[1])) as f32,
                    ]
                };
                if *copies == 0 {
                    map_anchors(&mut out, f)
                } else {
                    let mut cur = out.clone();
                    for _ in 0..*copies {
                        map_anchors(&mut cur, f);
                        out.holes.push(cur.anchors.clone());
                        out.holes.extend(cur.holes.iter().cloned());
                    }
                }
            }
            Effect::Warp { style, bend, h, v } => {
                if *bend == 0. && *h == 0. && *v == 0. {
                    continue;
                }
                let b = bounds.unwrap_or_else(|| crate::geom::kurbo::to_bez_path(&out).bounding_box());
                let c = b.center();
                let hw = (b.width() / 2.).max(1e-9);
                let hh = (b.height() / 2.).max(1e-9);
                let f = |q: kurbo::Point| {
                    let (x, y) = crate::effects_warp::warp_point(
                        *style,
                        f64::from(*bend) / 100.,
                        f64::from(*h) / 100.,
                        f64::from(*v) / 100.,
                        (q.x - c.x) / hw,
                        (q.y - c.y) / hh,
                    );
                    kurbo::Point::new(c.x + x * hw, c.y + y * hh)
                };
                let mut bp = kurbo::BezPath::new();
                for (ring, closed) in
                    std::iter::once((&out.anchors, out.closed)).chain(out.holes.iter().map(|r| (r, true)))
                {
                    if let Some(a) = ring.first() {
                        bp.move_to(f(kurbo::Point::new(a.p[0] as f64, a.p[1] as f64)));
                    }
                    for seg in crate::geom::kurbo::contour(ring, closed).segments() {
                        for k in 0..24 {
                            let piece = seg.subsegment(k as f64 / 24. ..(k + 1) as f64 / 24.).to_cubic();
                            bp.curve_to(f(piece.p1), f(piece.p2), f(piece.p3));
                        }
                    }
                    if closed {
                        bp.close_path();
                    }
                }
                let mut paths = crate::geom::kurbo::from_bez_path(&bp).into_iter();
                if let Some(p) = paths.next() {
                    out.anchors = p.anchors;
                    out.closed = p.closed;
                    out.holes = paths.map(|p| p.anchors).collect();
                }
            }
        }
        if count(&out) > MAX_ANCHORS
            || out.anchors.iter().chain(out.holes.iter().flatten()).any(|a| {
                std::iter::once(a.p).chain(a.hin).chain(a.hout).flatten().any(|v| !v.is_finite() || v.abs() > 1e9)
            })
        {
            return Err("effect numeric/geometry limit exceeded".into());
        }
    }
    Ok(out)
}
pub fn check(ed: &Editor, action: &Action) -> Result<(), String> {
    if let Action::SetPerPath { paths } = action {
        if paths.is_empty() || paths.len() > 1000 {
            return Err("effects require 1–1000 paths".into());
        }
        let mut seen = std::collections::HashSet::new();
        for (id, effects) in paths {
            if !seen.insert(id) {
                return Err("duplicate effect target".into());
            }
            check(ed, &Action::Set { ids: vec![*id], effects: effects.clone() })?;
        }
        return Ok(());
    }
    if let Action::PreviewAppend { ids, effect } = action {
        return check(ed, &Action::SetPerPath { paths: crate::effects_preview::appended(ed, ids, effect) });
    }

    if let Action::Tool | Action::EndPreview { .. } = action {
        return Ok(());
    }
    if let Action::Preview { ids, effects } = action {
        return check(ed, &Action::Set { ids: ids.clone(), effects: effects.clone() });
    }
    if let Action::WidthLive { id, profile } = action {
        return check(ed, &Action::Width { ids: vec![*id], profile: Some(profile.clone()) });
    }
    let ids = match action {
        Action::Set { ids, .. } | Action::Expand { ids } | Action::Width { ids, .. } => ids,
        _ => return Ok(()),
    };
    if ids.is_empty() || ids.len() > 1000 {
        return Err("effects require 1–1000 paths".into());
    }
    if matches!(action, Action::Expand { .. }) {
        crate::effects_document::bake_selected(&ed.doc, ids)?;
    }
    for id in ids {
        let p = ed.doc.paths.iter().find(|p| p.id == *id).ok_or("unknown path")?;
        if p.locked || ed.doc.eff_locked(*id) {
            return Err("locked effect target".into());
        }
        let mut q = p.clone();
        match action {
            Action::Set { effects, .. } => q.effects = effects.clone(),
            Action::Width { profile, .. } => {
                if let Some(p) = profile {
                    p.validate()?;
                }
                q.stroke_style.width_profile = profile.clone();
            }
            _ => {}
        }
        let parts = crate::effects_document::resolved_many(&q)?;
        if parts.len() > 1 && ed.doc.is_mask_source(*id) {
            return Err("Transform copies cannot replace a clipping-mask source".into());
        }
        if matches!(action, Action::Width { .. }) {
            crate::stroke::evaluate(&q, 0.01, &|| false).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub fn apply(ed: &mut Editor, action: Action) {
    if check(ed, &action).is_err() {
        return;
    }
    match &action {
        Action::SetPerPath { paths } => {
            if paths.iter().all(|(id, effects)| ed.doc.pidx(*id).is_some_and(|i| ed.doc.paths[i].effects == *effects)) {
                return;
            }
            ed.begin();
            for (id, effects) in paths {
                if let Some(i) = ed.doc.pidx(*id) {
                    ed.doc.paths[i].effects = effects.clone();
                }
            }
            ed.dirty = true;
            ed.commit();
            return;
        }
        Action::PreviewAppend { ids, effect } => {
            crate::effects_preview::preview_append(ed, ids, effect);
            return;
        }
        Action::Tool => {
            ed.set_tool(crate::ToolKind::Width);
            return;
        }
        Action::Preview { ids, effects } => {
            crate::effects_preview::preview(ed, ids, effects);
            return;
        }
        Action::EndPreview { accept } => {
            crate::effects_preview::finish(ed, *accept);
            return;
        }
        Action::WidthLive { id, profile } => {
            if let Some(i) = ed.doc.pidx(*id) {
                ed.doc.paths[i].stroke_style.width_profile = Some(profile.clone());
                ed.dirty = true;
            }
            return;
        }
        Action::Expand { ids } => {
            let _ = crate::effects_document::expand(ed, ids);
            return;
        }
        _ => {}
    }
    let ids = match &action {
        Action::Set { ids, .. } | Action::Expand { ids } | Action::Width { ids, .. } => ids,
        _ => return,
    };
    let mut replacements = Vec::new();
    for id in ids {
        if let Some(i) = ed.doc.pidx(*id) {
            let mut p = ed.doc.paths[i].clone();
            match &action {
                Action::Set { effects, .. } => p.effects = effects.clone(),
                Action::Expand { .. } => {
                    let Ok(q) = evaluate(&p) else { return };
                    p = q;
                    for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
                        a.id = 0;
                    }
                }
                Action::Width { profile, .. } => p.stroke_style.width_profile = profile.clone(),
                _ => return,
            };
            if p != ed.doc.paths[i] {
                replacements.push((i, p));
            }
        }
    }
    if replacements.is_empty() {
        return;
    }
    ed.begin();
    for (i, mut p) in replacements {
        for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
            if a.id == 0 {
                a.id = ed.doc.nid();
            }
        }
        ed.doc.paths[i] = p;
    }
    ed.dirty = true;
    ed.commit();
}

/// One authored stack yields separately painted paths; open contours never become holes.
pub fn evaluate_many(source: &Path) -> Result<Vec<Path>, String> {
    validate(source)?;
    let mut first = crate::live_corners::corners_only(source);
    first.effects.clear();
    let mut parts = vec![first];
    for effect in &source.effects {
        let bounds = parts.iter().map(|p| crate::geom::kurbo::to_bez_path(p).bounding_box()).reduce(|a, b| a.union(b));
        if let Effect::Transform { copies, movement, scale, rotate, reflect } = effect {
            let recipe =
                Effect::Transform { copies: 0, movement: *movement, scale: *scale, rotate: *rotate, reflect: *reflect };
            let mut current = parts.clone();
            let mut output = if *copies > 0 { parts.clone() } else { vec![] };
            for _ in 0..(*copies).max(1) {
                current = current
                    .iter()
                    .map(|p| {
                        let mut q = p.clone();
                        q.effects = vec![recipe.clone()];
                        evaluate_single(&q, bounds)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                output.extend(current.iter().cloned());
            }
            parts = output;
        } else {
            parts = parts
                .iter()
                .map(|p| {
                    let mut q = p.clone();
                    q.effects = vec![effect.clone()];
                    evaluate_single(&q, bounds)
                })
                .collect::<Result<Vec<_>, _>>()?;
        }
        if parts.len() > 1001 || parts.iter().map(count).sum::<usize>() > MAX_ANCHORS {
            return Err("effect geometry budget exceeded".into());
        }
    }
    Ok(parts)
}
/// Single-contour compatibility projection for editing bounds/hits. Painting uses evaluate_many.
pub fn evaluate(source: &Path) -> Result<Path, String> {
    Ok(combine(source, evaluate_many(source)?))
}
