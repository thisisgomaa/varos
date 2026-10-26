// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The Fill/Stroke cluster without a frame (owner: «بوكس وهمي»): the stroke ring behind, the fill
//! circle in front (the focused one painted on top), the curved swap arrow and the None glyph. The
//! focused target has the 1.5 pt accent outline; a click on a circle focuses it (X), the arrow
//! swaps them (Shift+X) and None sets the focused one to None (/).

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, vec2};
use vectorcraft_color::Paint;

use super::{Action, View};
use crate::theme::Tokens;

pub(crate) const STROKE_CENTER: [f32; 2] = [192.0, 213.0];
pub(crate) const FILL_CENTER: [f32; 2] = [210.0, 227.0];
pub(crate) const STROKE_R: f32 = 16.0;
pub(crate) const STROKE_BAND: f32 = 8.0;
pub(crate) const FILL_R: f32 = 20.0;
const FOCUS: f32 = 1.5;
const SWAP_POS: [f32; 2] = [222.0, 204.0];
const NONE_POS: [f32; 2] = [182.0, 238.0];
const NONE_SIZE: f32 = 14.0;
const GLYPH: f32 = 16.0;
const MIXED_GAP: f32 = 6.0;
const MIXED_STROKE: f32 = 2.0;

/// Which circle a point is on: the `front` one where the fill circle and the stroke ring overlap
/// (what is painted on top); the stroke ring stays hittable outside the fill circle.
pub(crate) fn target_at(origin: Pos2, p: Pos2, front_fill: bool) -> Option<bool> {
    let fill = origin + vec2(FILL_CENTER[0], FILL_CENTER[1]);
    let stroke = origin + vec2(STROKE_CENTER[0], STROKE_CENTER[1]);
    let on_fill = p.distance(fill) <= FILL_R;
    let on_stroke = (STROKE_R - STROKE_BAND..=STROKE_R).contains(&p.distance(stroke));
    match (on_fill, on_stroke) {
        (true, true) => Some(front_fill),
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
    }
}

/// The colour a paint shows in the cluster: a solid's, a gradient's first stop; `None` for None.
fn shown(p: &Paint) -> Option<Option<Color32>> {
    match p {
        Paint::None => None,
        Paint::Solid { color, .. } => Some(Some(crate::panels::c32(color))),
        Paint::Gradient(g) => Some(g.gradient.stops.first().map(|s| crate::panels::c32(&s.color))),
        Paint::Pattern { .. } => Some(None),
    }
}

/// The cluster drawn relative to `origin` (the wheel area's top left).
pub(super) fn show(ui: &mut Ui, origin: Pos2, v: &View, acts: &mut Vec<Action>) {
    let t = Tokens::get(ui.ctx());
    let stroke = (false, STROKE_CENTER, STROKE_R, &v.paints.1, v.mixed_pair.1);
    let fill = (true, FILL_CENTER, FILL_R, &v.paints.0, v.mixed_pair.0);
    let order = if v.fill_active { [stroke, fill] } else { [fill, stroke] };
    let bounds = Rect::from_min_max(
        origin + vec2(STROKE_CENTER[0] - STROKE_R, STROKE_CENTER[1] - STROKE_R),
        origin + vec2(FILL_CENTER[0] + FILL_R, FILL_CENTER[1] + FILL_R),
    );
    let resp = ui.interact(bounds, ui.scope_id().with("varos-picker-cluster"), Sense::click());
    let hit = resp.hover_pos().or(resp.interact_pointer_pos()).and_then(|p| target_at(origin, p, v.fill_active));
    for (is_fill, point, radius, paint, mixed) in order {
        let center = origin + vec2(point[0], point[1]);
        let p = ui.painter();
        p.circle_filled(center, radius, t.input);
        match shown(paint) {
            _ if mixed => {
                // Mixed: diagonal chords, distinct from the alpha checkerboard.
                let unit = std::f32::consts::FRAC_1_SQRT_2;
                let (tangent, normal) = (vec2(unit, -unit), vec2(unit, unit));
                let mut d = -radius;
                while d <= radius {
                    let length = (radius * radius - d * d).max(0.0).sqrt();
                    let mid = center + normal * d;
                    p.line_segment([mid - tangent * length, mid + tangent * length], Stroke::new(MIXED_STROKE, t.text_dim));
                    d += MIXED_GAP;
                }
            }
            Some(Some(c)) => {
                p.circle_filled(center, radius, c);
            }
            Some(None) => {
                p.circle_filled(center, radius, t.text_dim);
            }
            None => {
                let r = radius / 2.0;
                p.line_segment([center + vec2(-r, r), center + vec2(r, -r)], Stroke::new(FOCUS, t.error));
            }
        }
        if !is_fill {
            p.circle_filled(center, radius - STROKE_BAND, t.panel);
        }
        let focused = is_fill == v.fill_active;
        p.circle_stroke(center, radius, Stroke::new(FOCUS, if focused { t.accent } else { t.border }));
    }
    let tip = match hit {
        Some(false) => tl!("Stroke"),
        _ => tl!("Fill"),
    };
    let clicked = resp.clicked();
    let double = resp.double_clicked();
    if resp.secondary_clicked()
        && let Some(fill) = hit
    {
        acts.push(Action::Focus(fill));
    }
    resp.context_menu(|ui| {
        if ui.button("Gradient…").clicked() {
            acts.push(Action::Gradient);
            ui.close();
        }
    });
    if double {
        acts.push(Action::Gradient);
    }
    if hit.is_some() {
        let _ = resp.on_hover_text(tip);
    }
    if clicked && let Some(fill) = hit {
        acts.push(Action::Focus(fill));
    }
    for (icon, point, size, swap) in [("dc-swap", SWAP_POS, GLYPH, true), ("dc-fill-none", NONE_POS, NONE_SIZE, false)] {
        let center = origin + vec2(point[0], point[1]);
        let r = Rect::from_center_size(center, egui::Vec2::splat(size));
        let resp = ui.interact(r, ui.scope_id().with(("varos-picker-cluster-icon", swap)), Sense::click());
        let ink = if resp.hovered() { t.text } else { t.text_dim };
        super::icons::paint(ui, icon, r, ink);
        let tip = if swap { tl!("Swap Fill and Stroke (Shift+X)") } else { tl!("None") };
        if resp.on_hover_text(tip).clicked() {
            acts.push(if swap { Action::Swap } else { Action::NoneTarget });
        }
    }
}
