//! Pure Width tool gestures use the same fraction/normal coordinates as profile outlines.
use crate::{effects::Action, width_profile::WidthProfile, EditCommand, Editor, Pt, ToolKind};
#[derive(Clone, Default)]
pub struct State {
    pub drag: Option<Drag>,
}
#[derive(Clone)]
pub struct Drag {
    pub id: u32,
    pub t: f64,
    pub center: Pt,
    pub normal: Pt,
    pub before: Option<WidthProfile>,
}
pub fn points(ed: &Editor, id: u32) -> Vec<(f64, Pt, Pt)> {
    let Some(i) = ed.doc.pidx(id) else { return vec![] };
    let p = &ed.doc.paths[i];
    let q = crate::effects::evaluated(p);
    let mut pts = crate::model::Document::ring_px(&q.anchors, q.closed, 4.);
    if q.closed {
        if let Some(first) = pts.first().copied() {
            pts.push(first);
        }
    }
    let total: f32 = pts.windows(2).map(|w| crate::geom::dist(w[0], w[1])).sum();
    let mut at = 0.;
    let xf = ed.doc.unit_xform(id);
    let mut samples: Vec<_> = pts
        .windows(2)
        .filter_map(|w| {
            let len = crate::geom::dist(w[0], w[1]);
            let t = f64::from(at / total.max(1e-9));
            at += len;
            if len <= 1e-9 {
                return None;
            }
            let n = [(w[1][1] - w[0][1]) / len, -(w[1][0] - w[0][0]) / len];
            let c = xf.apply(w[0]);
            let nn = xf.apply([w[0][0] + n[0], w[0][1] + n[1]]);
            let d = crate::geom::sub(nn, c);
            let l = crate::geom::dist(nn, c).max(1e-9);
            Some((t, c, [d[0] / l, d[1] / l]))
        })
        .collect();
    if let (Some(last), Some(prev)) = (pts.last(), samples.last().copied()) {
        samples.push((1., xf.apply(*last), prev.2));
    }
    samples
}
pub fn location(ed: &Editor, id: u32, t: f64) -> Option<(Pt, Pt)> {
    let samples = points(ed, id);
    let first = samples.first().copied()?;
    let mut prev = first;
    for sample in samples.iter().copied().skip(1) {
        if t <= sample.0 {
            let u = ((t - prev.0) / (sample.0 - prev.0).max(1e-9)) as f32;
            return Some((
                [prev.1[0] + (sample.1[0] - prev.1[0]) * u, prev.1[1] + (sample.1[1] - prev.1[1]) * u],
                prev.2,
            ));
        }
        prev = sample;
    }
    // Include the final endpoint for t=1.
    let i = ed.doc.pidx(id)?;
    let p = &ed.doc.paths[i];
    let end = if p.closed { p.anchors.first() } else { p.anchors.last() }?;
    Some((ed.doc.unit_xform(id).apply(end.p), prev.2))
}
pub fn down(ed: &mut Editor, pos: Pt) -> bool {
    if ed.tool != ToolKind::Width {
        return false;
    }
    let ids = ed.selected_pids();
    let mut best = None;
    let mut distance = f32::MAX;
    for id in ids {
        let samples = points(ed, id);
        for w in samples.windows(2) {
            let d = crate::geom::sub(w[1].1, w[0].1);
            let v = crate::geom::sub(pos, w[0].1);
            let len2 = d[0] * d[0] + d[1] * d[1];
            let u = if len2 > 1e-9 { ((v[0] * d[0] + v[1] * d[1]) / len2).clamp(0., 1.) } else { 0. };
            let center = [w[0].1[0] + d[0] * u, w[0].1[1] + d[1] * u];
            let dist = crate::geom::dist(center, pos);
            if dist < distance {
                distance = dist;
                best = Some((id, w[0].0 + (w[1].0 - w[0].0) * f64::from(u), center, w[0].2));
            }
        }
    }
    if let Some((id, t, center, normal)) = best {
        if let Some(i) = ed.doc.pidx(id) {
            if ed.doc.eff_locked(id) {
                return true;
            }
            let before = ed.doc.paths[i].stroke_style.width_profile.clone();
            ed.begin();
            ed.width_tool.drag = Some(Drag { id, t, center, normal, before });
        }
    }
    true
}
pub fn movement(ed: &mut Editor, pos: Pt) -> bool {
    let Some(d) = ed.width_tool.drag.clone() else { return false };
    let Some(i) = ed.doc.pidx(d.id) else {
        cancel(ed);
        return true;
    };
    let width = ed.doc.paths[i].stroke_width;
    if width <= 0. {
        return true;
    }
    let offset = crate::geom::sub(pos, d.center);
    let factor = (2. * (offset[0] * d.normal[0] + offset[1] * d.normal[1]).abs() / width).min(100.) as f64;
    let mut p = d.before.clone().unwrap_or_else(|| WidthProfile::preset("uniform").unwrap_or_default());
    p.points.retain(|p| (p.0 - d.t).abs() > 1e-5);
    p.points.push((d.t, factor, factor));
    p.points.sort_by(|a, b| a.0.total_cmp(&b.0));
    ed.execute_ui(EditCommand::LiveEffects(Action::WidthLive { id: d.id, profile: p }));
    true
}
pub fn up(ed: &mut Editor) -> bool {
    if ed.width_tool.drag.take().is_some() {
        ed.finish_document_setup();
        true
    } else {
        false
    }
}
pub fn cancel(ed: &mut Editor) {
    if let Some(d) = ed.width_tool.drag.take() {
        if let Some(i) = ed.doc.pidx(d.id) {
            ed.doc.paths[i].stroke_style.width_profile = d.before;
        }
        ed.finish_document_setup();
    }
}
