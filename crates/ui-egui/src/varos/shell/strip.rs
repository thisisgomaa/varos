//! Board-owned iconic strip. Open state is transient; group dimensions and mode are saved.
//! Floating strips ([`super::floats`]) draw their groups with the same [`column`] and open the same
//! [`flyout`]. An icon is dragged by itself, a group by its grip dots ([`super::drag`]).
use super::drag::{self, Item};
use super::{Hit, boxes, tree::Node};
use crate::VectorcraftApp;
use egui::{Rect, Sense, pos2, vec2};
use serde::{Deserialize, Serialize};

/// A strip: the Board's, or a floating one (a lone icon is a floating strip of one icon).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StripRef {
    Board,
    Float(u32),
}

impl StripRef {
    /// The hit of this strip's group `index` (its grip dots).
    pub(crate) fn group_hit(self, index: usize) -> Hit {
        match self {
            StripRef::Board => Hit::StripGroup(index),
            StripRef::Float(strip) => Hit::FloatStripGroup { strip, index },
        }
    }

    /// The hit of this strip's icon of `panel`.
    pub(crate) fn icon_hit(self, panel: &str) -> Hit {
        match self {
            StripRef::Board => Hit::StripIcon(panel.to_string()),
            StripRef::Float(strip) => Hit::FloatStripIcon { strip, panel: panel.to_string() },
        }
    }
}

/// A strip group's grip dots: their height, and an icon row's height.
pub(crate) const GRIP: f32 = 12.0;
pub(crate) const ICON_ROW: f32 = 30.0;

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
        let mut strip = Self { groups, ..Self::default() };
        strip.append("control");
        strip
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
        self.sanitize_owned(tree, &|_| false, true);
    }

    /// [`Self::sanitize`] when panels may also be owned outside the tree and this strip
    /// (`elsewhere`: floating panels and floating strips). `legacy`: stage-2 state, whose
    /// auxiliary panels were appended into default groups and whose temporary groups go last;
    /// later state keeps the owner's arrangement.
    pub fn sanitize_owned(&mut self, tree: &Node, elsewhere: &dyn Fn(&str) -> bool, legacy: bool) {
        let mut seen = std::collections::HashSet::new();
        for g in &mut self.groups {
            g.panels.retain(|p| super::persist::known(p) && tree.find_tab(p).is_none() && !elsewhere(p) && seen.insert(p.clone()));
            g.width = if g.width.is_finite() { g.width.clamp(190.0, 1200.0) } else { 260.0 };
            g.height = if g.height.is_finite() { g.height.clamp(120.0, 1600.0) } else { 360.0 };
        }
        if !legacy {
            self.groups.retain(|g| !g.panels.is_empty());
            for group in Self::default_for(tree).groups {
                let panels: Vec<_> = group.panels.into_iter().filter(|p| !elsewhere(p) && seen.insert(p.clone())).collect();
                if !panels.is_empty() {
                    self.groups.push(Group { panels, temporary: false, width: 260.0, height: 360.0 });
                }
            }
            return;
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
            let panels: Vec<_> = group.panels.into_iter().filter(|p| !elsewhere(p) && seen.insert(p.clone())).collect();
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
            40.0
        } else {
            app.ui
                .varos_shell
                .strip
                .groups
                .iter()
                .flat_map(|g| &g.panels)
                .map(|p| {
                    ui.painter().layout_no_wrap(boxes::label(app, p), egui::FontId::proportional(super::tokens::CHROME_GLYPH), t.text).size().x + 42.0
                })
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
    // The Board's ≡ on top: Maximize Board, or Restore Layout.
    let menu_row = Rect::from_min_size(pos2(rail.left(), (rail.bottom() - 48.0).max(rail.top())), vec2(width, 24.0));
    let toggle = Rect::from_min_size(pos2(rail.left(), menu_row.bottom()), vec2(width, 24.0));
    let toggle_response = ui.interact(toggle, toggle_id, Sense::click());
    toggle_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Toggle panel labels"));
    hits.push((Hit::StripToggle, toggle));
    ui.painter().rect_filled(rail, 0.0, t.panel);
    ui.painter().vline(rail.left() + 0.5, rail.y_range(), egui::Stroke::new(1.0, t.border));
    if toggle_response.hovered() {
        ui.painter().rect_filled(toggle.shrink(3.0), 3.0, t.hover);
    }
    hits.push((Hit::BoardMenu, menu_row));
    let shell = &app.ui.varos_shell;
    let board_id = shell.tree.boxes().into_iter().find(|b| b.is_board()).map(|b| b.id);
    let maximized = board_id.is_some_and(|id| shell.maximized_box() == Some(id));
    if boxes::board_menu(ui, menu_row, maximized, &t)
        && let Some(id) = board_id
    {
        close(app);
        app.ui.varos_shell.toggle_maximized(id);
        ui.ctx().request_repaint();
    }
    crate::icons::paint(
        ui,
        if app.ui.varos_shell.strip.mode == Mode::Icons { "chevrons-left" } else { "chevrons-right" },
        Rect::from_center_size(toggle.center(), vec2(16.0, 16.0)),
        t.text_dim,
    );
    let groups = app.ui.varos_shell.strip.groups.clone();
    let mut tops = Vec::new();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(rail.min, pos2(rail.right(), menu_row.top()))));
    child.set_clip_rect(rail.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing.y = 0.0;
    let mut icons = Vec::new();
    // Icons and grips are dragged; the column scrolls by wheel and bar only (egui drags it on touch).
    egui::ScrollArea::vertical().id_salt("shell-strip-scroll").auto_shrink([false, false]).show(&mut child, |ui| {
        column(app, ui, StripRef::Board, &groups, width, rail, hits, &mut tops, &mut icons);
    });
    let tools_width = if app.ui.toolbar && app.ui.varos_shell.tools_pos.is_none() {
        super::tools::WIDTH + 28.0 * f32::from(app.ui.varos_shell.tools_columns.clamp(1, 3) - 1)
    } else {
        0.0
    };
    let bounds = Rect::from_min_max(board.min + vec2(tools_width, 0.0), board.max);
    flyout(app, ui, bounds, rail, &tops, StripRef::Board, hits);
    // Paint after all strip and flyout input: exactly one active icon on the click frame.
    let mut paint_ui = child.new_child(egui::UiBuilder::new().max_rect(rail));
    paint_ui.set_clip_rect(Rect::from_min_max(rail.min, pos2(rail.right(), menu_row.top())).intersect(ui.clip_rect()));
    let mode = app.ui.varos_shell.strip.mode;
    paint_icons(app, &paint_ui, icons, mode, StripRef::Board, hits);
    Rect::from_min_max(board.min, pos2(rail.left().max(board.left()), board.bottom()))
}

/// A strip's groups, laid out down `ui` `width` wide: each group's grip dots (dragged: the whole
/// group) and its icons (clicked: the flyout opens or closes; dragged: the icon leaves). Each
/// group's top goes to `tops`; each icon's rect and response to `icons`, painted by
/// [`paint_icons`] once all input is taken.
#[allow(clippy::too_many_arguments)]
pub(crate) fn column(
    app: &mut VectorcraftApp,
    ui: &mut egui::Ui,
    sref: StripRef,
    groups: &[Group],
    width: f32,
    _rail: Rect,
    hits: &mut Vec<(Hit, Rect)>,
    tops: &mut Vec<f32>,
    icons: &mut Vec<(String, Rect, egui::Response)>,
) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    for (index, g) in groups.iter().enumerate() {
        let (grip, _) = ui.allocate_exact_size(vec2(width, GRIP), Sense::hover());
        tops.push(grip.top());
        // Geometry for drop slots and source outlines; only the first is draggable.
        hits.push((sref.group_hit(index), grip));
        if g.panels.is_empty() {
            continue;
        }
        if index == 0 {
            let response = ui.interact(grip, egui::Id::unique("varos-shell-grip").with(sref), Sense::drag());
            if response.drag_started() {
                drag::begin(app, &ctx, Item::Group { strip: sref, group: g.clone() }, grip);
            }
            if ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| grip.expand(15.0).contains(p))) {
                ui.painter().rect_filled(Rect::from_center_size(grip.center(), vec2(30.0, 3.0)), 1.5, t.text_dim);
            }
        } else {
            ui.painter().hline((grip.center().x - 10.0)..=(grip.center().x + 10.0), grip.center().y, egui::Stroke::new(1.0, t.border));
        }
        for panel in &g.panels {
            let (row, _) = ui.allocate_exact_size(vec2(width, ICON_ROW), Sense::hover());
            let rect = Rect::from_center_size(row.center(), vec2((width - 12.0).max(1.0), 28.0));
            let r = ui.interact(rect, ui.scope_id().with(panel), Sense::click_and_drag());
            if r.drag_started() {
                drag::begin(app, &ctx, Item::Icon(panel.clone()), rect);
            } else if clicked_once(&ctx, r.id, r.clicked()) {
                if app.ui.varos_shell.open.as_deref() == Some(panel) {
                    close(app);
                } else {
                    open(app, panel);
                }
            }
            icons.push((panel.clone(), rect, r));
        }
    }
}

/// Paint the icons [`column`] laid out in `paint_ui` (the open panel's lit) and publish them.
pub(crate) fn paint_icons(
    app: &VectorcraftApp,
    paint_ui: &egui::Ui,
    icons: Vec<(String, Rect, egui::Response)>,
    mode: Mode,
    sref: StripRef,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let t = crate::theme::Tokens::get(paint_ui.ctx());
    for (panel, rect, r) in icons {
        let active = app.ui.varos_shell.open.as_deref() == Some(panel.as_str());
        r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, boxes::label(app, &panel)));
        if r.hovered() || active {
            paint_ui.painter().rect_filled(rect, 3.0, if active { t.button } else { t.hover });
        }
        let icon = crate::state::rail_panel(super::persist::instance_kind(&panel))
            .map_or(if panel == "control" { "sliders-horizontal" } else { "panel-left" }, |p| p.2);
        egui::Image::new(crate::icons::source(icon))
            .tint(if active { t.text_strong } else { t.text_dim })
            .paint_at(paint_ui, Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), vec2(16.0, 16.0)));
        if mode == Mode::Labels {
            paint_ui.painter().text(
                rect.min + vec2(29.0, 13.0),
                egui::Align2::LEFT_CENTER,
                boxes::label(app, &panel),
                egui::FontId::proportional(super::tokens::CHROME_GLYPH),
                t.text,
            );
        } else {
            r.on_hover_text(boxes::label(app, &panel));
        }
        if paint_ui.clip_rect().intersects(rect) {
            hits.push((sref.icon_hit(&panel), rect.intersect(paint_ui.clip_rect())));
        }
    }
}

/// The flyout of the open panel when strip `sref` owns it, beside `rail` (the strip) inside
/// `bounds`: left of the Board's strip; right of a floating strip when it fits there, else left.
/// Its top is its group's (`tops`), moved up to fit.
pub(crate) fn flyout(
    app: &mut VectorcraftApp,
    ui: &mut egui::Ui,
    bounds: Rect,
    rail: Rect,
    tops: &[f32],
    sref: StripRef,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let motion_id = egui::Id::unique("shell-flyout-motion").with(sref);
    let requested = app
        .ui
        .varos_shell
        .open
        .as_deref()
        .filter(|p| app.ui.varos_shell.strip_groups(sref).is_some_and(|gs| gs.iter().any(|g| g.panels.iter().any(|q| q == p))));
    let Some((panel, alpha, slide)) = super::motion::flyout(ui.ctx(), motion_id, requested) else { return };
    let mut closing = requested.is_none();
    let Some(index) = app.ui.varos_shell.strip_groups(sref).and_then(|gs| gs.iter().position(|g| g.panels.contains(&panel))) else {
        return;
    };
    let Some(group) = app.ui.varos_shell.strip_groups(sref).and_then(|gs| gs.get(index)).cloned() else { return };
    let right_room = (bounds.right() - rail.right()).max(1.0);
    let left_room = (rail.left() - bounds.left()).max(1.0);
    let on_right = sref != StripRef::Board && (right_room >= group.width.min(bounds.width()) || right_room > left_room);
    let room = (if on_right { right_room } else { left_room } - 6.0).max(1.0);
    let mut width = 240.0_f32.min(room);
    let height = group.height.min(bounds.height());
    let top = tops.get(index).copied().unwrap_or(bounds.top()).min(bounds.bottom() - height).max(bounds.top());
    let place = |width: f32| {
        let x = if on_right { rail.right() + 6.0 } else { rail.left() - 6.0 - width };
        Rect::from_min_size(pos2(x, top), vec2(width, height))
    };
    let mut rect = place(width);
    let ctx = ui.ctx().clone();
    if app.session.prefs.auto_collapse_icon_panels
        && !egui::Popup::is_any_open(&ctx)
        && ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p) && !rail.contains(p)))
    {
        close(app);
        closing = true;
        // Start the fade on the event frame; this pass still paints its visible body.
        let _ = super::motion::flyout(ui.ctx(), motion_id, None);
        if ui.ctx().global_style().animation_time == 0.0 {
            return;
        }
    }
    let area_id = egui::Id::unique("shell-flyout");
    let layer = egui::LayerId::new(egui::Order::Middle, area_id);
    let resize_ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(bounds));
    // The edge away from the strip resizes.
    let edge_of = |rect: Rect| {
        if on_right {
            Rect::from_min_max(pos2(rect.right() - 5.0, rect.top()), rect.max)
        } else {
            Rect::from_min_max(rect.min, pos2(rect.left() + 5.0, rect.bottom()))
        }
    };
    let r = resize_ui.interact(edge_of(rect), area_id.with("resize"), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if r.dragged()
        && let Some(p) = r.interact_pointer_pos()
    {
        let wanted = if on_right { p.x - rail.right() } else { rail.left() - p.x };
        width = wanted.clamp(190.0_f32.min(room), room);
        rect = place(width);
        if let Some(g) = app.ui.varos_shell.strip_groups_mut(sref).and_then(|gs| gs.get_mut(index)) {
            g.width = width;
        }
    }
    rect = rect.translate(vec2(if on_right { -slide } else { slide }, 0.0));
    // Over the boxes and floating panels.
    ctx.move_to_top(layer);
    // Known geometry avoids egui's invisible first sizing pass and blocks the canvas immediately.
    ctx.memory_mut(|m| m.areas_mut().set_state(layer, egui::AreaState { pivot_pos: Some(rect.min), size: Some(rect.size()), ..Default::default() }));
    egui::Area::new(area_id).order(egui::Order::Middle).fixed_pos(rect.min).constrain(false).fade_in(false).movable(false).show(&ctx, |ui| {
        if closing {
            ui.disable();
        }
        ui.spacing_mut().scroll = egui::style::ScrollStyle { bar_width: 4.0, floating_width: 2.0, ..egui::style::ScrollStyle::floating() };
        ui.set_opacity(alpha);
        ui.set_min_size(rect.size());
        ui.set_clip_rect(bounds);
        let t = crate::theme::Tokens::get(&ctx);
        let head = Rect::from_min_size(rect.min, vec2(width, 32.0));
        let collapse = Rect::from_min_size(pos2(head.right() - 48.0, head.top() + 4.0), vec2(22.0, 24.0));
        let collapse_response = ui.interact(collapse, area_id.with("close"), Sense::click());
        collapse_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Collapse panel"));
        if collapse_response.clicked() {
            close(app);
            // Keep painting on the close frame, instead of a blank frame before the fade.
            let _ = super::motion::flyout(&ctx, motion_id, None);
            if ctx.global_style().animation_time == 0.0 {
                return;
            }
            ui.disable();
            ui.set_opacity(alpha);
        }
        ui.painter().rect_filled(rect, boxes::RADIUS, t.panel);
        let menu = collapse.translate(vec2(23.0, 0.0));
        let row = Rect::from_min_max(head.min + vec2(6.0, 0.0), pos2(collapse.left(), head.bottom()));
        let tabs: Vec<_> = group.panels.iter().map(|p| (p.clone(), boxes::label(app, p))).collect();
        let active = group.panels.iter().position(|p| p == &panel).unwrap_or(0);
        let out = boxes::tab_row(ui, &tabs, active, row, &t);
        // A flyout's tab dragged out: the panel leaves the strip.
        if let Some(i) = out.drag_started
            && let (Some((p, _)), Some(pill)) = (tabs.get(i), out.tabs.get(i))
        {
            drag::begin(app, &ctx, Item::Tab(p.clone()), *pill);
            return;
        }
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
        crate::icons::paint(ui, "chevrons-right", Rect::from_center_size(collapse.center(), vec2(16.0, 16.0)), t.text_dim);
        if boxes::menu_button(app, ui, menu, u32::MAX, &front, &t) {
            close_panel(app, &front);
            // A closing default panel keeps its body for the fade, but takes no input.
            let _ = super::motion::flyout(&ctx, motion_id, None);
            if ctx.global_style().animation_time == 0.0 {
                return;
            }
            ui.disable();
            ui.set_opacity(alpha);
        }
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("review-strip-opacity"), ui.opacity()));
        hits.push((Hit::FlyoutClose, collapse));
        hits.push((Hit::FlyoutResize, edge_of(rect)));
        hits.push((Hit::Flyout, rect));
        ui.painter().hline((rect.left() + 1.0)..=(rect.right() - 1.0), head.bottom() - 0.5, egui::Stroke::new(1.0, t.border));
        let inner = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max - vec2(1.0, 1.0));
        let mut body = ui.new_child(egui::UiBuilder::new().id_salt("shell-flyout-body").max_rect(inner));
        body.set_clip_rect(inner.intersect(bounds));
        let output = egui::ScrollArea::vertical().id_salt(("shell-flyout-scroll", sref, index)).auto_shrink([false, false]).show(&mut body, |ui| {
            boxes::panel_body(app, ui, &front, width, (inner.height() - 20.0).max(0.0));
        });
        let wanted = (output.content_size.y + boxes::HEADER + 1.0).clamp(120.0, 1600.0).min(bounds.height());
        let wanted_width = width;
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("shell-flyout-content"), output.content_size));
        if let Some(g) = app.ui.varos_shell.strip_groups_mut(sref).and_then(|gs| gs.get_mut(index))
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
        app.ui.varos_shell.take(panel);
    }
    close(app);
}
