//! Board-owned iconic strip. Open state is transient; group dimensions and mode are saved.
use super::{Hit, boxes, tree::Node};
use crate::VectorcraftApp;
use egui::{Rect, Sense, pos2, vec2};
use serde::{Deserialize, Serialize};

/// The Board edge that owns this strip. Ordinary boxes never own strips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Edge {
    #[default]
    Right,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Icons,
    Labels,
}
/// Ordered panel owners and their remembered flyout dimensions in points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub panels: Vec<String>,
    #[serde(default)]
    pub temporary: bool,
    #[serde(default = "initial_width")]
    pub width: f32,
    pub height: f32,
}
fn initial_width() -> f32 {
    260.0
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Strip {
    pub edge: Edge,
    pub mode: Mode,
    pub groups: Vec<Group>,
}
impl Strip {
    pub fn default_for(tree: &Node) -> Self {
        let groups = crate::state::ICON_PANEL_GROUPS
            .iter()
            .map(|g| Group {
                temporary: false,
                panels: g.iter().filter(|p| tree.find_tab(p).is_none()).map(|p| (*p).into()).collect(),
                width: 260.0,
                height: 360.0,
            })
            .filter(|g| !g.panels.is_empty())
            .collect();
        Self { groups, ..Self::default() }
    }
    pub fn group(&self, panel: &str) -> Option<usize> {
        self.groups.iter().position(|g| g.panels.iter().any(|p| p == panel))
    }
    pub fn remove(&mut self, panel: &str) {
        for g in &mut self.groups {
            g.panels.retain(|p| p != panel);
        }
        self.groups.retain(|g| !g.panels.is_empty());
    }
    pub fn is_default(panel: &str) -> bool {
        crate::state::ICON_PANEL_GROUPS.iter().any(|g| g.contains(&panel))
    }
    pub fn append(&mut self, panel: &str) {
        self.remove(panel);
        let temporary = !Self::is_default(panel);
        let index = if temporary { self.groups.iter().position(|g| g.temporary) } else { self.groups.iter().rposition(|g| !g.temporary) };
        let index = index.unwrap_or_else(|| {
            let index = if temporary { self.groups.len() } else { self.groups.iter().position(|g| g.temporary).unwrap_or(self.groups.len()) };
            self.groups.insert(index, Group { temporary, panels: Vec::new(), width: 260.0, height: 360.0 });
            index
        });
        if let Some(g) = self.groups.get_mut(index) {
            g.panels.push(panel.into());
            if matches!(panel, "properties" | "layers" | "libraries") {
                g.width = g.width.max(300.0);
            }
        }
    }

    pub fn sanitize(&mut self, tree: &Node) {
        let mut seen = std::collections::HashSet::new();
        for g in &mut self.groups {
            g.panels.retain(|p| super::persist::known(p) && tree.find_tab(p).is_none() && seen.insert(p.clone()));
            g.width = if g.width.is_finite() { g.width.clamp(190.0, 1200.0) } else { 260.0 };
            g.height = if g.height.is_finite() { g.height.clamp(120.0, 1600.0) } else { 360.0 };
        }
        // Older stage-2 state appended auxiliary panels into a default group. Split them out.
        let mut auxiliary = Vec::new();
        for g in &mut self.groups {
            if !g.temporary {
                g.panels.retain(|p| {
                    if Self::is_default(p) {
                        true
                    } else {
                        auxiliary.push(p.clone());
                        false
                    }
                });
            }
        }
        self.groups.retain(|g| !g.panels.is_empty());
        for p in auxiliary {
            self.append(&p);
        }
        for group in Self::default_for(tree).groups {
            let panels: Vec<_> = group.panels.into_iter().filter(|p| seen.insert(p.clone())).collect();
            if !panels.is_empty() {
                self.groups.push(Group { panels, temporary: false, width: 260.0, height: 360.0 });
            }
        }
        self.groups.sort_by_key(|g| g.temporary);
    }
}

pub(crate) fn close(app: &mut VectorcraftApp) {
    app.ui.varos_shell.open = None;
    app.ui.open_panel = None;
}
pub(crate) fn open(app: &mut VectorcraftApp, panel: &str) {
    app.ui.varos_shell.open = Some(panel.into());
    app.ui.open_panel = Some(panel.into());
}

/// Called inside guarded host shortcuts: Escape belongs to this flyout only over it or with its focus.
pub(crate) fn escape(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !super::enabled(app)
        || app.ui.varos_shell.open.is_none()
        || app.ui.dialog.is_some()
        || egui::Popup::is_any_open(ctx)
        || ctx.egui_wants_keyboard_input()
        || app.session.tool_composing()
    {
        return;
    }
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("shell-flyout"));
    let over = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| ctx.layer_id_at(p) == Some(layer));
    let focused = ctx.memory(|m| m.focused()).and_then(|id| ctx.read_response(id)).is_some_and(|r| r.layer_id == layer);
    if (over || focused) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        close(app);
    }
}

/// Toggle gestures execute once even if egui discards a sizing pass and draws it again.
fn clicked_once(ctx: &egui::Context, id: egui::Id, clicked: bool) -> bool {
    if !clicked {
        return false;
    }
    let frame = ctx.cumulative_frame_nr();
    let key = id.with("shell-click-frame");
    ctx.data_mut(|d| {
        if d.get_temp::<u64>(key) == Some(frame) {
            false
        } else {
            d.insert_temp(key, frame);
            true
        }
    })
}

pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut egui::Ui, board: Rect, hits: &mut Vec<(Hit, Rect)>) -> Rect {
    let t = crate::theme::Tokens::get(ui.ctx());
    let width_for = |app: &VectorcraftApp| {
        if app.ui.varos_shell.strip.mode == Mode::Icons {
            34.0
        } else {
            app.ui
                .varos_shell
                .strip
                .groups
                .iter()
                .flat_map(|g| &g.panels)
                .map(|p| ui.painter().layout_no_wrap(boxes::label(app, p), egui::FontId::proportional(11.0), t.text).size().x + 42.0)
                .fold(70.0, f32::max)
                .min(180.0)
        }
    };
    // Read the prior widget's response before registering this pass's final geometry. The mode
    // changes on release, and the very next press must hit the expanded/collapsed rectangle.
    let toggle_id = ui.scope_id().with("strip-mode");
    if clicked_once(ui.ctx(), toggle_id, ui.ctx().read_response(toggle_id).is_some_and(|r| r.clicked())) {
        app.ui.varos_shell.strip.mode = if app.ui.varos_shell.strip.mode == Mode::Icons { Mode::Labels } else { Mode::Icons };
    }
    let width = width_for(app).min(board.width());
    let rail = Rect::from_min_max(pos2(board.right() - width, board.top()), board.max);
    hits.push((Hit::Strip, rail));
    let toggle = Rect::from_min_size(rail.min, vec2(width, 24.0));
    let toggle_response = ui.interact(toggle, toggle_id, Sense::click());
    toggle_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Toggle panel labels"));
    hits.push((Hit::StripToggle, toggle));
    ui.painter().rect_filled(rail, boxes::RADIUS, t.panel);
    ui.painter().rect_stroke(rail, boxes::RADIUS, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    if toggle_response.hovered() {
        ui.painter().rect_filled(toggle.shrink(3.0), 3.0, t.hover);
    }
    ui.painter().text(
        toggle.center(),
        egui::Align2::CENTER_CENTER,
        if app.ui.varos_shell.strip.mode == Mode::Icons { "<<" } else { ">>" },
        egui::FontId::proportional(11.0),
        t.text,
    );
    let groups = app.ui.varos_shell.strip.groups.clone();
    let mut tops = Vec::new();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(rail.left(), toggle.bottom()), rail.max)));
    child.set_clip_rect(rail.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing.y = 0.0;
    let mut icons = Vec::new();
    egui::ScrollArea::vertical().id_salt("shell-strip-scroll").auto_shrink([false, false]).show(&mut child, |ui| {
        for (index, g) in groups.iter().enumerate() {
            let (grip, _) = ui.allocate_exact_size(vec2(width, 12.0), Sense::hover());
            tops.push(grip.top());
            hits.push((Hit::StripGroup(index), grip));
            if g.panels.is_empty() {
                continue;
            }
            for y in [4.0, 8.0] {
                for x in [-4.0, 0.0, 4.0] {
                    ui.painter().circle_filled(pos2(grip.center().x + x, grip.top() + y), 0.7, t.text_dim);
                }
            }
            for panel in &g.panels {
                let (row, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
                let rect = Rect::from_min_size(pos2(row.left() + 4.0, row.top()), vec2((width - 8.0).max(1.0), 26.0));
                let r = ui.interact(rect, ui.scope_id().with(panel), Sense::click());
                if clicked_once(ui.ctx(), r.id, r.clicked()) {
                    if app.ui.varos_shell.open.as_deref() == Some(panel) {
                        close(app);
                    } else {
                        open(app, panel);
                    }
                }
                icons.push((panel.clone(), rect, r));
            }
            ui.painter().hline(rail.x_range(), ui.cursor().top(), egui::Stroke::new(1.0, t.border));
        }
    });
    flyout(app, ui, board, rail, &tops, hits);
    // Paint after all strip and flyout input: exactly one active icon on the click frame.
    let mut paint_ui = child.new_child(egui::UiBuilder::new().max_rect(rail));
    paint_ui.set_clip_rect(Rect::from_min_max(pos2(rail.left(), toggle.bottom()), rail.max).intersect(ui.clip_rect()));
    for (panel, rect, r) in icons {
        let active = app.ui.varos_shell.open.as_deref() == Some(panel.as_str());
        r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, boxes::label(app, &panel)));
        if r.hovered() || active {
            paint_ui.painter().rect_filled(rect, 3.0, if active { t.button } else { t.hover });
        }
        let icon = crate::state::rail_panel(&panel).map_or("panel-left", |p| p.2);
        egui::Image::new(crate::icons::source(icon))
            .tint(if active { t.text_strong } else { t.text_dim })
            .paint_at(&paint_ui, Rect::from_min_size(rect.min + vec2(5.0, 5.0), vec2(16.0, 16.0)));
        if app.ui.varos_shell.strip.mode == Mode::Labels {
            paint_ui.painter().text(
                rect.min + vec2(29.0, 13.0),
                egui::Align2::LEFT_CENTER,
                boxes::label(app, &panel),
                egui::FontId::proportional(11.0),
                t.text,
            );
        } else {
            r.on_hover_text(boxes::label(app, &panel));
        }
        if paint_ui.clip_rect().intersects(rect) {
            hits.push((Hit::StripIcon(panel.clone()), rect.intersect(paint_ui.clip_rect())));
        }
    }
    Rect::from_min_max(board.min, pos2((rail.left() - super::tree::SEAM).max(board.left()), board.bottom()))
}

fn flyout(app: &mut VectorcraftApp, ui: &mut egui::Ui, board: Rect, rail: Rect, tops: &[f32], hits: &mut Vec<(Hit, Rect)>) {
    let Some(panel) = app.ui.varos_shell.open.clone() else { return };
    let Some(index) = app.ui.varos_shell.strip.group(&panel) else {
        close(app);
        return;
    };
    let Some(group) = app.ui.varos_shell.strip.groups.get(index).cloned() else { return };
    let room = (rail.left() - board.left()).max(1.0);
    let mut width = group.width.min(room);
    let height = group.height.min(board.height());
    let top = tops.get(index).copied().unwrap_or(board.top()).min(board.bottom() - height).max(board.top());
    let mut rect = Rect::from_min_size(pos2(rail.left() - width, top), vec2(width, height));
    let ctx = ui.ctx().clone();
    if app.session.prefs.auto_collapse_icon_panels
        && !egui::Popup::is_any_open(&ctx)
        && ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p) && !rail.contains(p)))
    {
        close(app);
        return;
    }
    let area_id = egui::Id::unique("shell-flyout");
    let layer = egui::LayerId::new(egui::Order::Middle, area_id);
    let resize_ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(board));
    let edge = Rect::from_min_max(rect.min, pos2(rect.left() + 5.0, rect.bottom()));
    let r = resize_ui.interact(edge, area_id.with("resize"), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if r.dragged()
        && let Some(p) = r.interact_pointer_pos()
    {
        width = (rail.left() - p.x).clamp(190.0_f32.min(room), room);
        rect.min.x = rail.left() - width;
        if let Some(g) = app.ui.varos_shell.strip.groups.get_mut(index) {
            g.width = width;
        }
    }
    // Known geometry avoids egui's invisible first sizing pass and blocks the canvas immediately.
    ctx.memory_mut(|m| m.areas_mut().set_state(layer, egui::AreaState { pivot_pos: Some(rect.min), size: Some(rect.size()), ..Default::default() }));
    egui::Area::new(area_id).order(egui::Order::Middle).fixed_pos(rect.min).constrain(false).fade_in(false).movable(false).show(&ctx, |ui| {
        ui.set_min_size(rect.size());
        ui.set_clip_rect(board);
        let t = crate::theme::Tokens::get(&ctx);
        let head = Rect::from_min_size(rect.min, vec2(width, boxes::HEADER));
        let collapse = Rect::from_min_size(pos2(head.right() - 48.0, head.top() + 4.0), vec2(22.0, 24.0));
        let collapse_response = ui.interact(collapse, area_id.with("close"), Sense::click());
        collapse_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Collapse panel"));
        if collapse_response.clicked() {
            close(app);
            return;
        }
        ui.painter().rect_filled(rect, boxes::RADIUS, t.panel);
        let menu = collapse.translate(vec2(23.0, 0.0));
        let row = Rect::from_min_max(head.min + vec2(6.0, 0.0), pos2(collapse.left(), head.bottom()));
        let tabs: Vec<_> = group.panels.iter().map(|p| (p.clone(), boxes::label(app, p))).collect();
        let active = group.panels.iter().position(|p| p == &panel).unwrap_or(0);
        let out = boxes::tab_row(ui, &tabs, active, row, &t);
        let front = group.panels.get(out.clicked.unwrap_or(active)).cloned().unwrap_or(panel.clone());
        if out.clicked.is_some() {
            open(app, &front);
        }
        for ((p, _), r) in tabs.iter().zip(out.tabs) {
            hits.push((Hit::FlyoutTab(p.clone()), r));
        }
        if collapse_response.hovered() {
            ui.painter().rect_filled(collapse, 3.0, t.hover);
        }
        ui.painter().text(collapse.center(), egui::Align2::CENTER_CENTER, ">>", egui::FontId::proportional(11.0), t.text);
        if boxes::menu_button(app, ui, menu, u32::MAX, &front, &t) {
            close_panel(app, &front);
            return;
        }
        hits.push((Hit::FlyoutClose, collapse));
        hits.push((Hit::FlyoutResize, Rect::from_min_max(rect.min, pos2(rect.left() + 5.0, rect.bottom()))));
        hits.push((Hit::Flyout, rect));
        ui.painter().hline((rect.left() + 1.0)..=(rect.right() - 1.0), head.bottom() - 0.5, egui::Stroke::new(1.0, t.border));
        let inner = Rect::from_min_max(pos2(rect.left() + 6.0, head.bottom()), rect.max - vec2(1.0, 1.0));
        let mut body = ui.new_child(egui::UiBuilder::new().id_salt("shell-flyout-body").max_rect(inner));
        body.set_clip_rect(inner.intersect(board));
        let output = egui::ScrollArea::vertical().id_salt(("shell-flyout-scroll", index)).auto_shrink([false, false]).show(&mut body, |ui| {
            crate::dock::panel_body(app, ui, &front, width - 7.0, (inner.height() - 20.0).max(0.0));
        });
        let wanted = (output.content_size.y + boxes::HEADER + 1.0).clamp(120.0, 1600.0).min(board.height());
        let wanted_width = (output.content_size.x + 7.0).max(width).min(room);
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("shell-flyout-content"), output.content_size));
        if let Some(g) = app.ui.varos_shell.strip.groups.get_mut(index)
            && ((height - wanted).abs() > 1.0 || wanted_width > width + 1.0)
        {
            g.height = wanted;
            g.width = wanted_width;
            ctx.request_discard("fit shell flyout content before presentation");
        }
        ui.painter().rect_stroke(rect, boxes::RADIUS, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    });
    // Panel bodies may request more space; only the visible flyout blocks canvas input.
    if let Some(mut state) = egui::AreaState::load(&ctx, area_id) {
        state.size = Some(rect.size());
        ctx.memory_mut(|m| m.areas_mut().set_state(layer, state));
    }
}

pub(crate) fn close_panel(app: &mut VectorcraftApp, panel: &str) {
    if !Strip::is_default(panel) {
        app.ui.varos_shell.strip.remove(panel);
    }
    close(app);
}
