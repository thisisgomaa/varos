// Adapted from VectorCraft pathops/src/offset.rs:79 @ a469568 (MIT OR Apache-2.0).
// Copyright (c) 2026 ArtCraft Team and the VectorCraft contributors. See NOTICE.
//! Region offset uses kurbo stroke coverage and i_overlay; no new dependencies.
use crate::{
    live_corners,
    model::{Anchor, Paint, Path},
    stroke::{StrokeCap, StrokeJoin, StrokeStyle},
    Editor,
};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Outline,
    Offset { delta: f32, join: StrokeJoin, miter: f32 },
    Expand,
}
fn anchors(ring: Vec<[f32; 2]>) -> Vec<Anchor> {
    ring.into_iter().map(|p| Anchor { id: 0, p, hin: None, hout: None, smooth: false }).collect()
}
fn region(mut rings: Vec<Vec<[f32; 2]>>, source: &Path, paint: Paint) -> Path {
    let mut out = source.clone();
    out.corners.clear();
    out.closed = true;
    out.anchors = if rings.is_empty() { vec![] } else { anchors(rings.remove(0)) };
    out.holes = rings.into_iter().map(anchors).collect();
    out.fill = paint;
    out.stroke = Paint::None;
    out.stroke_width = 0.;
    out.stroke_style = StrokeStyle::default();
    out
}
pub fn outline(path: &Path) -> Result<Path, String> {
    let coverage = crate::stroke::evaluate(path, 0.01, &|| false).map_err(|e| e.to_string())?;
    Ok(region(coverage.rings, path, path.stroke.clone()))
}
pub fn offset(path: &Path, delta: f32, join: StrokeJoin, miter: f32) -> Result<Path, String> {
    if !delta.is_finite() || delta.abs() > 7200. || !miter.is_finite() || !(1.0..=1000.0).contains(&miter) {
        return Err("Offset requires finite delta ±7200 and miter 1–1000".into());
    }
    let source = live_corners::evaluated(path);
    if delta == 0. {
        return Ok(source);
    }
    let mut boundary = source.clone();
    boundary.stroke = Paint::Solid([0., 0., 0., 1.]);
    boundary.stroke_width = 2. * delta.abs();
    boundary.stroke_style = StrokeStyle { join, cap: StrokeCap::Butt, miter_limit: miter, ..StrokeStyle::default() };
    let coverage = crate::stroke::evaluate(&boundary, 0.01, &|| false).map_err(|e| e.to_string())?;
    if !source.closed {
        return Ok(region(coverage.rings, &source, source.fill.clone()));
    }
    let fill: Vec<Vec<[f32; 2]>> = std::iter::once(crate::model::Document::ring_px(&source.anchors, true, 16.))
        .chain(source.holes.iter().map(|h| crate::model::Document::ring_px(h, true, 16.)))
        .collect();
    let result = fill
        .overlay_as::<i64>(
            &coverage.rings,
            if delta > 0. { OverlayRule::Union } else { OverlayRule::Difference },
            FillRule::EvenOdd,
        )
        .into_iter()
        .filter(|shape| {
            delta >= 0.
                || shape.first().and_then(|r| deep_point(r)).is_some_and(|p| distance(&fill, p) >= delta.abs() - 0.02)
        })
        .flat_map(|shape| {
            shape.into_iter().enumerate().filter_map(|(i, ring)| {
                if delta > 0. && i > 0 && deep_point(&ring).is_some_and(|p| distance(&fill, p) < delta - 0.02) {
                    None
                } else {
                    Some(ring)
                }
            })
        })
        .collect();
    Ok(region(result, &source, source.fill.clone()))
}
pub fn check(ed: &Editor, action: Action) -> Result<(), String> {
    let ids = ed.selected_pids();
    if ids.is_empty() {
        return Err("Select paths first".into());
    }
    for id in ids {
        let p = ed.doc.paths.iter().find(|p| p.id == id).ok_or("unknown path")?;
        if ed.doc.eff_locked(id) || ed.doc.eff_hidden(id) || !ed.in_isolation(id) {
            return Err("Path is unavailable".into());
        }
        if ed.doc.is_mask_source(id) && matches!(action, Action::Outline | Action::Expand) {
            return Err("Release the clipping mask before converting its source stroke".into());
        }
        // Splitting paint into siblings cannot preserve object isolation or stroke knockout.
        if matches!(action, Action::Expand)
            && p.fill.solid().is_some()
            && p.stroke.solid().is_some_and(|c| p.stroke_width > 0. && (p.opacity < 1. || c[3] < 1.))
        {
            return Err(
                "Expand cannot preserve translucent fill/stroke compositing; outline the stroke separately".into()
            );
        }
        match action {
            Action::Outline | Action::Expand => {
                outline(p)?;
            }
            Action::Offset { delta, join, miter } => {
                offset(p, delta, join, miter)?;
            }
        }
    }
    Ok(())
}
impl Editor {
    pub fn path_advanced(&mut self, action: Action) {
        if check(self, action).is_err() {
            return;
        }
        let mut ids: Vec<_> = self.selected_pids().into_iter().collect();
        ids.sort_unstable();
        // Prepare every region before publishing any geometry.
        let prepared: Result<Vec<_>, _> = ids
            .iter()
            .map(|id| {
                let p = self.doc.paths.iter().find(|p| p.id == *id).ok_or("unknown path".to_owned())?;
                match action {
                    Action::Offset { delta, join, miter } => offset(p, delta, join, miter),
                    _ => outline(p),
                }
            })
            .collect();
        let Ok(prepared) = prepared else { return };
        if matches!(action, Action::Outline) && prepared.iter().all(|p| p.anchors.is_empty()) {
            return;
        }
        self.begin();
        let mut bake = std::collections::BTreeSet::new();
        for (id, mut generated) in ids.into_iter().zip(prepared) {
            if matches!(action, Action::Outline) && generated.anchors.is_empty() {
                continue;
            }
            let Some(pi) = self.doc.pidx(id) else { continue };
            if matches!(action, Action::Expand) {
                let unit = self.doc.unit_of(id);
                let xf = self.doc.unit_xform(id);
                if let Some(unit) = unit {
                    bake.insert(unit);
                }
                let mut fill = live_corners::evaluated(&self.doc.paths[pi]);
                fill.stroke = Paint::None;
                fill.stroke_width = 0.;
                fill.stroke_style = StrokeStyle::default();
                for a in &mut fill.anchors {
                    if a.id == 0 {
                        a.id = self.doc.nid();
                    }
                }
                self.doc.paths[pi] = fill;
                if !generated.anchors.is_empty() {
                    let parent = self.doc.node_of_path(id).and_then(|n| self.doc.node(n)).and_then(|n| n.parent);
                    let source_leaf = self.doc.node_of_path(id);
                    let clip_exempt = source_leaf.is_some_and(|n| self.doc.node_clip_exempt(n));
                    let active = self.doc.active_layer;
                    generated.id = self.doc.nid();
                    for a in generated.anchors.iter_mut().chain(generated.holes.iter_mut().flatten()) {
                        a.id = self.doc.nid();
                    }
                    let generated_id = generated.id;
                    self.doc.paths.push(generated);
                    self.doc.sync_tree();
                    self.doc.active_layer = active;
                    if let Some(leaf) = self.doc.node_of_path(generated_id) {
                        self.doc.set_node_clip_exempt(leaf, clip_exempt);
                        let old_parent = self.doc.node(leaf).and_then(|n| n.parent);
                        if let Some(old) = old_parent.and_then(|n| self.doc.node_mut(n)) {
                            old.children.retain(|n| *n != leaf);
                        }
                        if let Some(node) = self.doc.node_mut(leaf) {
                            node.parent = parent;
                        }
                        if let Some(parent) = parent.and_then(|n| self.doc.node_mut(n)) {
                            parent.children.retain(|n| *n != leaf);
                            if let Some(i) = source_leaf.and_then(|n| parent.children.iter().position(|c| *c == n)) {
                                parent.children.insert(i, leaf);
                            } else {
                                parent.children.push(leaf);
                            }
                        }
                        self.doc.sync_tree();
                    }
                    // Group children inherit the group transform. Standalone leaves need a copy.
                    if self.doc.unit_of(generated_id) == self.doc.node_of_path(generated_id) {
                        if let Some(node) = self.doc.node_of_path(generated_id).and_then(|n| self.doc.node_mut(n)) {
                            node.xform = xf;
                        }
                        if let Some(unit) = self.doc.unit_of(generated_id) {
                            bake.insert(unit);
                        }
                    }
                }
            } else {
                for a in generated.anchors.iter_mut().chain(generated.holes.iter_mut().flatten()) {
                    a.id = self.doc.nid();
                }
                self.doc.paths[pi] = generated;
            }
        }
        // Bake only after every prepared local region has been inserted.
        // Baking a shared group inside the loop would transform later fills but not their strokes.
        for unit in bake {
            if let Some(pid) = self.doc.node_paths(unit).first().copied() {
                self.bake_unit_of(pid);
            }
        }
        self.dirty = true;
        self.commit();
    }
}

fn distance(rings: &[Vec<[f32; 2]>], p: [f32; 2]) -> f32 {
    rings
        .iter()
        .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()))
        .map(|(a, b)| {
            let d = crate::geom::sub(*b, *a);
            let v = crate::geom::sub(p, *a);
            let n = d[0] * d[0] + d[1] * d[1];
            let t = if n > 0. { ((v[0] * d[0] + v[1] * d[1]) / n).clamp(0., 1.) } else { 0. };
            crate::geom::dist(p, [a[0] + d[0] * t, a[1] + d[1] * t])
        })
        .fold(f32::INFINITY, f32::min)
}
fn deep_point(ring: &[[f32; 2]]) -> Option<[f32; 2]> {
    let min = ring.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max = ring.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
    let mut best = None;
    let mut width = 0.;
    for fraction in [0.5, 0.3, 0.7] {
        let y = min + (max - min) * fraction;
        let mut xs = vec![];
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
            if (a[1] <= y) != (b[1] <= y) {
                xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
            }
        }
        xs.sort_by(f32::total_cmp);
        for span in xs.as_chunks::<2>().0 {
            if span[1] - span[0] > width {
                width = span[1] - span[0];
                best = Some([(span[0] + span[1]) * 0.5, y]);
            }
        }
    }
    best
}

/// Transform panel's typed bounds use the same preference as the Scale dialog.
pub(crate) fn scale_selected_strokes(ed: &mut Editor, sx: f32, sy: f32) {
    if !ed.select_transform.scale_strokes {
        return;
    }
    let scale = (sx * sy).abs().sqrt();
    let ids = ed.selected_pids();
    for p in &mut ed.doc.paths {
        if ids.contains(&p.id) {
            p.stroke_width *= scale;
            for dash in &mut p.stroke_style.dash {
                *dash *= scale;
            }
            p.stroke_style.dash_phase *= scale;
        }
    }
}
