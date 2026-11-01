//! Dividers: the seams between boxes. Hover shows the resize cursor; a drag moves the seam live
//! (the new ratio is laid out in the same frame, within both sides' minimum sizes); a double-click
//! puts the split's default ratio back. No line, no motion.

use egui::{CursorIcon, Rect, Sense};

use super::Hit;
use super::tree::{Axis, Layout};
use crate::VectorcraftApp;

/// The id of the divider of the split at `path`.
pub(crate) fn id(path: &[u8]) -> egui::Id {
    egui::Id::unique("varos-shell-divider").with(path)
}

/// Where along its axis the pointer held the divider, from its centre (kept while it is dragged).
fn grab_id(path: &[u8]) -> egui::Id {
    id(path).with("grab")
}

/// The pointer's axis coordinate this drag (so release can see it after the pointer is up).
fn center_id(path: &[u8]) -> egui::Id {
    id(path).with("center")
}

/// Set while the pointer wants a column narrower than its minimum (the azure collapse hint).
fn past_id(path: &[u8]) -> egui::Id {
    id(path).with("past")
}

/// Take this frame's input on `layout`'s dividers; true when a ratio changed (the caller lays the
/// tree out again before drawing, and publishes the dividers where they are drawn).
pub(crate) fn interact(app: &mut VectorcraftApp, ui: &mut egui::Ui, layout: &Layout) -> bool {
    let ctx = ui.ctx().clone();
    let mut changed = false;
    for d in &layout.dividers {
        let hit = match d.axis {
            Axis::Horizontal => d.rect.expand2(egui::vec2(3.5, 0.0)),
            Axis::Vertical => d.rect.expand2(egui::vec2(0.0, 3.5)),
        };
        let response = ui.interact(hit, id(&d.path), Sense::click_and_drag());
        if response.hovered() || response.dragged() {
            ctx.set_cursor_icon(match d.axis {
                Axis::Horizontal => CursorIcon::ResizeHorizontal,
                Axis::Vertical => CursorIcon::ResizeVertical,
            });
        }
        let along = |p: egui::Pos2| d.axis.of(p.to_vec2());
        let mut collapse: Option<bool> = None;
        {
            let tree = &mut app.ui.varos_shell.tree;
            if response.double_clicked() {
                tree.reset_ratio(&d.path);
                ctx.data_mut(|m| {
                    m.remove_temp::<f32>(grab_id(&d.path));
                    m.remove_temp::<f32>(center_id(&d.path));
                    m.remove_temp::<bool>(past_id(&d.path));
                });
                changed = true;
                continue;
            }
            if response.drag_started() {
                let origin = ctx.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos()).unwrap_or(d.rect.center());
                let grab = along(origin) - along(d.rect.center());
                ctx.data_mut(|m| m.insert_temp(grab_id(&d.path), grab));
                super::resize::begin(&ctx, id(&d.path), tree.ratio(&d.path).map(|r| (d.path.clone(), r)));
            }
            if !super::resize::active(&ctx, id(&d.path)) {
                ctx.data_mut(|m| {
                    m.remove_temp::<f32>(grab_id(&d.path));
                    m.remove_temp::<f32>(center_id(&d.path));
                    m.remove_temp::<bool>(past_id(&d.path));
                });
                continue;
            }
            if response.dragged()
                && let Some(p) = response.interact_pointer_pos()
            {
                let grab = ctx.data(|m| m.get_temp::<f32>(grab_id(&d.path))).unwrap_or(0.0);
                let center = along(p) - grab;
                let ratio = tree.ratio_for(&d.path, center, d.span);
                if tree.ratio(&d.path) != Some(ratio) {
                    tree.set_ratio(&d.path, ratio);
                    changed = true;
                }
                let past = tree.side_overshoot(&d.path, center, d.span).is_some_and(|(_, past)| past >= super::tree::COLLAPSE_HINT);
                ctx.data_mut(|m| {
                    m.insert_temp(center_id(&d.path), center);
                    if past {
                        m.insert_temp(past_id(&d.path), true);
                    } else {
                        m.remove_temp::<bool>(past_id(&d.path));
                    }
                });
            }
            if response.drag_stopped() {
                let commit = super::resize::finish(&ctx, id(&d.path));
                let center = ctx.data(|m| m.get_temp::<f32>(center_id(&d.path)));
                ctx.data_mut(|m| {
                    m.remove_temp::<f32>(grab_id(&d.path));
                    m.remove_temp::<f32>(center_id(&d.path));
                    m.remove_temp::<bool>(past_id(&d.path));
                });
                if let Some(center) = center.filter(|_| commit) {
                    collapse = tree
                        .side_overshoot(&d.path, center, d.span)
                        .filter(|(_, past)| *past >= super::tree::COLLAPSE_RELEASE)
                        .map(|(take_a, _)| take_a);
                }
            }
        }
        if let Some(take_a) = collapse
            && app.ui.varos_shell.collapse_side(&d.path, take_a)
        {
            changed = true;
        }
    }
    changed
}

/// One separator per split, painted over neighboring surfaces. Past a column's minimum the seam
/// is the azure collapse hint ([`Hit::AzureHint`]).
pub(crate) fn paint(ui: &egui::Ui, layout: &Layout, hits: &mut Vec<(Hit, Rect)>) {
    for d in &layout.dividers {
        let response = ui.ctx().read_response(id(&d.path));
        let past = ui.ctx().data(|m| m.get_temp::<bool>(past_id(&d.path))).unwrap_or(false);
        if past {
            hits.push((Hit::AzureHint, d.rect));
        }
        let t = crate::theme::Tokens::get(ui.ctx());
        let stroke = egui::Stroke::new(
            if past {
                3.0
            } else if response.as_ref().is_some_and(|r| r.hovered()) || response.as_ref().is_some_and(|r| r.dragged()) {
                2.0
            } else {
                1.0
            },
            if past || response.as_ref().is_some_and(|r| r.dragged()) {
                t.accent
            } else if response.as_ref().is_some_and(|r| r.hovered()) {
                t.hover
            } else {
                t.border
            },
        );
        match d.axis {
            Axis::Horizontal => {
                ui.painter().vline(d.rect.center().x, d.rect.y_range(), stroke);
            }
            Axis::Vertical => {
                ui.painter().hline(d.rect.x_range(), d.rect.center().y, stroke);
            }
        }
    }
}
