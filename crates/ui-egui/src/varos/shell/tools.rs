//! Board-edge tools, with the stage-3 strip drag transaction and legacy tool behaviours.
use super::{Hit, drag, floats};
use crate::{VectorcraftApp, theme::Tokens};
use egui::{Rect, Sense, pos2, vec2};

pub(crate) const WIDTH: f32 = super::tokens::TOOL_STRIP_WIDTH;

fn edge_rect_id() -> egui::Id {
    egui::Id::unique("shell-tools-edge-rect")
}

fn tools_rect_id() -> egui::Id {
    egui::Id::unique("shell-tools-rect")
}

pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut egui::Ui, board: Rect, region: Rect, hits: &mut Vec<(Hit, Rect)>) -> Rect {
    if !app.ui.toolbar || app.ui.screen_mode >= 3 || !board.is_positive() {
        return board;
    }
    let width = WIDTH + 28.0 * f32::from(app.ui.varos_shell.tools_columns.clamp(1, 3) - 1);
    let docked = app.ui.varos_shell.tools_pos.is_none();
    let rect = match app.ui.varos_shell.tools_pos {
        None => Rect::from_min_size(board.min, vec2(width.min(board.width()), board.height())),
        Some([x, y]) => floats::clamp_into(Rect::from_min_size(pos2(x, y), vec2(width, content_height(app).min(region.height()))), region),
    };
    let canvas_left = (rect.right()).min(board.right());
    let rect = super::motion::settle(ui.ctx(), "__tools", rect, region);
    if docked {
        contents(app, ui, rect, hits);
    } else {
        let ctx = ui.ctx().clone();
        let id = egui::Id::unique("shell-tools");
        let layer = egui::LayerId::new(egui::Order::Middle, id);
        ctx.memory_mut(|m| {
            m.areas_mut().set_state(layer, egui::AreaState { pivot_pos: Some(rect.min), size: Some(rect.size()), ..Default::default() })
        });
        egui::Area::new(id).order(egui::Order::Middle).fixed_pos(rect.min).movable(false).fade_in(false).show(&ctx, |ui| {
            ui.set_min_size(rect.size());
            contents(app, ui, rect, hits);
        });
    }
    if docked { Rect::from_min_max(pos2(canvas_left, board.top()), board.max) } else { board }
}

fn contents(app: &mut VectorcraftApp, ui: &mut egui::Ui, rect: Rect, hits: &mut Vec<(Hit, Rect)>) {
    let t = Tokens::get(ui.ctx());
    let floating = app.ui.varos_shell.tools_pos.is_some();
    ui.painter().rect_filled(rect, if floating { super::boxes::RADIUS } else { 0 }, t.panel);
    if floating {
        ui.painter().rect_stroke(rect, super::boxes::RADIUS, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    }
    let movable = app.ui.screen_mode == 0;
    let grip_right = if floating { rect.right() } else { rect.right() - 4.0 };
    let grip = Rect::from_min_max(rect.min, pos2(grip_right, rect.top() + 18.0_f32.min(rect.height())));
    let response = ui.interact(grip, egui::Id::unique("shell-tools-grip"), if movable { Sense::drag() } else { Sense::hover() });
    if ui.input(|i| i.pointer.hover_pos().is_some_and(|p| grip.expand(15.0).contains(p))) {
        ui.painter().rect_filled(Rect::from_center_size(grip.center(), vec2(30.0, 3.0)), 1.5, t.text_dim);
    }
    if !floating {
        ui.painter().vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0, t.border));
    }
    if movable && response.drag_started() {
        drag::begin(app, ui.ctx(), drag::Item::Tools, rect);
    }
    if movable {
        response.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(tl!("Drag tools; drop on the window's left edge to dock"));
    }
    hits.push((Hit::Tools, rect));
    hits.push((Hit::ToolsGrip, grip));
    if !floating {
        // The canvas is drawn after the strip and would take this 8 pt band. `finish_edge` grabs
        // it last, so a drag on the canvas-side edge reaches the strip.
        let edge = Rect::from_min_max(pos2(rect.right() - 4.0, rect.top()), pos2(rect.right() + 4.0, rect.bottom()));
        hits.push((Hit::ToolsEdge, edge));
        super::stamp_rect(ui.ctx(), edge_rect_id(), edge);
        super::stamp_rect(ui.ctx(), tools_rect_id(), rect);
    }
    let wells_h = controls_height(app).min((rect.height() - grip.height()).max(0.0));
    let body = Rect::from_min_max(pos2(rect.left(), grip.bottom()), pos2(rect.right(), rect.bottom() - wells_h));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("shell-tool-buttons").max_rect(body));
    child.set_clip_rect(body.intersect(ui.clip_rect()));
    crate::toolbar::shell_body(app, &mut child);
    // The drawer's outside-click exclusion includes its opener and all bottom controls.
    ui.ctx().data_mut(|d| d.insert_temp(crate::edit_toolbar::toolbar_id(), rect));
    let wells = Rect::from_min_max(pos2(rect.left(), body.bottom()), rect.max);
    let mut child =
        ui.new_child(egui::UiBuilder::new().id_salt("shell-tool-wells").max_rect(wells).layout(egui::Layout::top_down(egui::Align::Center)));
    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
    child.set_clip_rect(wells.intersect(ui.clip_rect()));
    if app.ui.toolbar_show.fill_stroke {
        child.add_space(8.0);
    }
    let columns = usize::from(app.ui.varos_shell.tools_columns.clamp(1, 3));
    crate::toolbar::bottom_controls(app, &mut child, &t, columns);
}

/// Grab the canvas-side edge after the board, so the board's drag does not take the 8 pt band.
pub(crate) fn finish_edge(app: &mut VectorcraftApp, ui: &mut egui::Ui, hits: &mut Vec<(Hit, Rect)>) {
    let Some(edge) = super::stamped_rect(ui.ctx(), edge_rect_id()) else { return };
    let rect = super::stamped_rect(ui.ctx(), tools_rect_id()).unwrap_or(edge);
    let edge_id = egui::Id::unique("shell-tools-edge");
    let edge_response = ui.interact(edge, edge_id, Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if edge_response.drag_started() {
        super::resize::begin(ui.ctx(), edge_id, None);
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(edge.center());
        ui.ctx().data_mut(|d| {
            d.insert_temp(edge_id.with("x"), origin.x);
            d.insert_temp(edge_id.with("cols"), app.ui.varos_shell.tools_columns);
        });
    }
    if !super::resize::active(ui.ctx(), edge_id) {
        ui.ctx().data_mut(|d| {
            d.remove_temp::<f32>(edge_id.with("x"));
            d.remove_temp::<u8>(edge_id.with("cols"));
            d.remove_temp::<u8>(edge_id.with("snap"));
        });
        return;
    }
    if edge_response.dragged()
        && let Some(p) = edge_response.interact_pointer_pos()
    {
        let origin = ui.ctx().data(|d| d.get_temp::<f32>(edge_id.with("x"))).unwrap_or(p.x);
        let cols = ui.ctx().data(|d| d.get_temp::<u8>(edge_id.with("cols"))).unwrap_or(app.ui.varos_shell.tools_columns);
        let base = WIDTH + 28.0 * f32::from(cols.clamp(1, 3) - 1);
        let snap = nearest_columns(base + (p.x - origin));
        let snap_x = rect.left() + WIDTH + 28.0 * f32::from(snap - 1);
        let hint_h = rect.height() / 3.0;
        let hint = Rect::from_center_size(pos2(snap_x, rect.center().y), vec2(2.0, hint_h));
        ui.painter().vline(snap_x, hint.y_range(), egui::Stroke::new(3.0, crate::varos::theme::ACCENT));
        hits.push((Hit::AzureHint, hint));
        ui.ctx().data_mut(|d| d.insert_temp(edge_id.with("snap"), snap));
    }
    if edge_response.drag_stopped() {
        let commit = super::resize::finish(ui.ctx(), edge_id);
        let snap = ui.ctx().data(|d| d.get_temp::<u8>(edge_id.with("snap"))).unwrap_or(app.ui.varos_shell.tools_columns);
        ui.ctx().data_mut(|d| {
            d.remove_temp::<f32>(edge_id.with("x"));
            d.remove_temp::<u8>(edge_id.with("cols"));
            d.remove_temp::<u8>(edge_id.with("snap"));
        });
        if commit {
            app.ui.varos_shell.tools_columns = snap.clamp(1, 3);
        }
        ui.ctx().request_repaint();
    }
}

/// 40, 68 or 96, whichever the dragged width is nearest.
fn nearest_columns(width: f32) -> u8 {
    let mut best = (f32::MAX, 1u8);
    for (w, cols) in [(40.0, 1u8), (68.0, 2), (96.0, 3)] {
        let distance = (w - width).abs();
        if distance < best.0 {
            best = (distance, cols);
        }
    }
    best.1
}

fn controls_height(app: &VectorcraftApp) -> f32 {
    let show = app.ui.toolbar_show;
    let columns = app.ui.varos_shell.tools_columns.clamp(1, 3);
    let width = WIDTH + 28.0 * f32::from(columns - 1);
    let wells = if show.fill_stroke { 8.0 + (if columns <= 1 { 28.0 } else { 40.0 }) + 6.0 } else { 0.0 };
    let chips = if show.coloring { if width >= 3.0 * 28.0 + 8.0 { 28.0 + 6.0 } else { 3.0 * 28.0 + 2.0 * 4.0 + 6.0 } } else { 0.0 };
    30.0 + wells + chips + if show.draw_mode { 30.0 } else { 0.0 }
}

fn content_height(app: &VectorcraftApp) -> f32 {
    let slots = crate::toolbar::slots(app);
    let columns = usize::from(app.ui.varos_shell.tools_columns.clamp(1, 3));
    let mut rows = 0;
    let mut in_group = 0usize;
    for (group, _) in &slots {
        if group.is_some() {
            rows += in_group.div_ceil(columns);
            in_group = 0;
        }
        in_group += 1;
    }
    rows += in_group.div_ceil(columns);
    18.0 + 8.0 + rows as f32 * 30.0 + slots.iter().filter(|s| s.0.is_some()).count() as f32 * (super::strip::GRIP + 2.0) + controls_height(app)
}
