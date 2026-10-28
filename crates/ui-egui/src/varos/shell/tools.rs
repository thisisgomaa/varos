//! Board-edge tools, with the stage-3 strip drag transaction and legacy tool behaviours.
use super::{Hit, drag, floats};
use crate::{VectorcraftApp, theme::Tokens};
use egui::{Rect, Sense, pos2, vec2};

pub(crate) const WIDTH: f32 = super::tokens::TOOL_STRIP_WIDTH;

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
    let grip = Rect::from_min_size(rect.min, vec2(rect.width(), 18.0_f32.min(rect.height())));
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
        response.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(tl!("Drag tools; drop on the Board's left edge to dock"));
    }
    hits.push((Hit::Tools, rect));
    hits.push((Hit::ToolsGrip, grip));
    let wells_h = controls_height(app).min((rect.height() - grip.height()).max(0.0));
    let body = Rect::from_min_max(pos2(rect.left(), grip.bottom()), pos2(rect.right(), rect.bottom() - wells_h));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("shell-tool-buttons").max_rect(body));
    child.set_clip_rect(body.intersect(ui.clip_rect()));
    crate::toolbar::shell_body(app, &mut child);
    // The drawer's outside-click exclusion includes its opener and all bottom controls.
    ui.ctx().data_mut(|d| d.insert_temp(crate::edit_toolbar::toolbar_id(), rect));
    let wells = Rect::from_min_max(pos2(rect.left(), body.bottom()), rect.max);
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("shell-tool-wells").max_rect(wells).layout(egui::Layout::top_down(egui::Align::Min)));
    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
    child.set_clip_rect(wells.intersect(ui.clip_rect()));
    crate::toolbar::bottom_controls(app, &mut child, &t, 1);
}

fn controls_height(app: &VectorcraftApp) -> f32 {
    let show = app.ui.toolbar_show;
    30.0 + if show.fill_stroke { 48.0 } else { 0.0 } + if show.coloring { 20.0 } else { 0.0 } + if show.draw_mode { 30.0 } else { 0.0 }
}

fn content_height(app: &VectorcraftApp) -> f32 {
    let slots = crate::toolbar::slots(app);
    18.0 + 8.0 + slots.len() as f32 * 29.0 + slots.iter().filter(|s| s.0.is_some()).count() as f32 * super::strip::GRIP + controls_height(app)
}
