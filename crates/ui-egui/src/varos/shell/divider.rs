//! Dividers: the seams between boxes. Hover shows the resize cursor; a drag moves the seam live
//! (the new ratio is laid out in the same frame, within both sides' minimum sizes); a double-click
//! puts the split's default ratio back. No line, no motion.

use egui::{CursorIcon, Sense};

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
        let tree = &mut app.ui.varos_shell.tree;
        if response.double_clicked() {
            tree.reset_ratio(&d.path);
            ctx.data_mut(|m| m.remove_temp::<f32>(grab_id(&d.path)));
            changed = true;
            continue;
        }
        if response.drag_started() {
            let origin = ctx.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos()).unwrap_or(d.rect.center());
            let grab = along(origin) - along(d.rect.center());
            ctx.data_mut(|m| m.insert_temp(grab_id(&d.path), grab));
        }
        if response.dragged()
            && let Some(p) = response.interact_pointer_pos()
        {
            let grab = ctx.data(|m| m.get_temp::<f32>(grab_id(&d.path))).unwrap_or(0.0);
            let ratio = tree.ratio_for(&d.path, along(p) - grab, d.span);
            if tree.ratio(&d.path) != Some(ratio) {
                tree.set_ratio(&d.path, ratio);
                changed = true;
            }
        }
        if response.drag_stopped() {
            ctx.data_mut(|m| m.remove_temp::<f32>(grab_id(&d.path)));
        }
    }
    changed
}

/// One separator per split, painted over neighboring surfaces.
pub(crate) fn paint(ui: &egui::Ui, layout: &Layout) {
    for d in &layout.dividers {
        let response = ui.ctx().read_response(id(&d.path));
        let t = crate::theme::Tokens::get(ui.ctx());
        let stroke = egui::Stroke::new(
            if response.as_ref().is_some_and(|r| r.hovered()) || response.as_ref().is_some_and(|r| r.dragged()) { 2.0 } else { 1.0 },
            if response.as_ref().is_some_and(|r| r.dragged()) {
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
