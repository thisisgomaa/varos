//! The box tree — the SINGLE place `egui_tiles` is used in the whole app (ruling 9).
//!
//! egui_tiles gives us the split-tree data model + rendering + resize. But its docking only reflows on
//! DROP; Ahmed wants LIVE reflow (neighbours make room and the panel takes its real slot while you drag).
//! So we drive the drag OURSELVES (from the move-pill), and each frame we rebuild the layout from a
//! snapshot taken at grab-time + one `dock()` into the slot under the cursor. The result: the real
//! layout reflows live under the hand. All of it stays inside this file.
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, UiBuilder, Visuals, pos2, vec2,
};
use egui_tiles::{
    Behavior, Container, LinearDir, SimplificationOptions, TabState, Tabs, Tile, TileId, Tiles, Tree,
    UiResponse,
};
use std::collections::HashSet;
use super::registry::{self, PanelId};
use super::tokens as T;

pub struct ShellState {
    tree: Tree<PanelId>,
    /// Active custom drag: the grabbed tile + the layout snapshot taken when the grab began.
    drag: Option<DragState>,
}

struct DragState {
    tile: TileId,
    base: Tree<PanelId>,
    /// The layout's tile rects captured at grab-time — a STABLE reference for the drop test, so the
    /// live reflow never oscillates.
    base_rects: Vec<(TileId, Rect)>,
    /// The last slot we docked into — we only rebuild the tree when this changes (no per-frame churn).
    last: Option<(TileId, Side)>,
}

impl ShellState {
    pub fn standard() -> Self {
        let mut tiles = Tiles::default();
        let board = tiles.insert_pane(PanelId::Board);
        let align = tiles.insert_pane(PanelId::Align);
        let props = tiles.insert_pane(PanelId::Properties);
        let layers = tiles.insert_pane(PanelId::Layers);
        let right = tiles.insert_vertical_tile(vec![align, props, layers]);
        let root = tiles.insert_horizontal_tile(vec![board, right]);
        let mut tree = Tree::new("varos_shell", root, tiles);
        set_share(&mut tree, root, board, 0.80);
        set_share(&mut tree, root, right, 0.20);
        set_share(&mut tree, right, align, 0.26);
        set_share(&mut tree, right, props, 0.42);
        set_share(&mut tree, right, layers, 0.32);
        Self { tree, drag: None }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // panes inside a Tabs container get egui_tiles' (styled) tab bar, not our single-box header
        let mut tabbed = HashSet::new();
        for (_id, tile) in self.tree.tiles.iter() {
            if let Tile::Container(Container::Tabs(t)) = tile {
                for &c in &t.children {
                    tabbed.insert(c);
                }
            }
        }

        let mut behavior = ShellBehavior {
            tabbed,
            dragging: self.drag.as_ref().map(|d| d.tile),
            ..Default::default()
        };
        self.tree.ui(&mut behavior, ui);

        // menu intents apply only when not dragging
        if self.drag.is_none() {
            if let Some((id, panel)) = behavior.switch {
                if let Some(Tile::Pane(p)) = self.tree.tiles.get_mut(id) {
                    *p = panel;
                }
            }
            if let Some(id) = behavior.close {
                detach(&mut self.tree, id);
                self.tree.tiles.remove(id);
            }
            // a grab just started — snapshot the layout AND its rects (the stable reference)
            if let Some(t) = behavior.drag_start {
                let base_rects: Vec<(TileId, Rect)> = self
                    .tree
                    .tiles
                    .iter()
                    .filter_map(|(id, _)| self.tree.tiles.rect(*id).map(|r| (*id, r)))
                    .collect();
                self.drag = Some(DragState { tile: t, base: self.tree.clone(), base_rects, last: None });
            }
        }

        // LIVE docking: measure the drop slot against the STABLE snapshot rects (never oscillates),
        // and only rebuild when the slot actually CHANGES (no per-frame churn → no flicker).
        if self.drag.is_some() {
            if !ui.input(|i| i.pointer.any_down()) {
                self.drag = None; // released → commit
            } else if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
                let cur = {
                    let d = self.drag.as_ref().unwrap();
                    compute_drop(&d.base, &d.base_rects, d.tile, ptr)
                };
                if self.drag.as_ref().unwrap().last != cur {
                    let (tile, mut rebuilt) = {
                        let d = self.drag.as_ref().unwrap();
                        (d.tile, d.base.clone())
                    };
                    if let Some((target, side)) = cur {
                        dock(&mut rebuilt, tile, target, side);
                    }
                    self.tree = rebuilt;
                    self.drag.as_mut().unwrap().last = cur;
                    ui.ctx().request_repaint();
                }
            }
        }

        draw_resize_handles(&self.tree, ui);
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.tree)
    }
}

// ───────────────────────── tree surgery (all public egui_tiles API) ─────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

fn set_share(tree: &mut Tree<PanelId>, container: TileId, child: TileId, share: f32) {
    if let Some(Tile::Container(Container::Linear(lin))) = tree.tiles.get_mut(container) {
        lin.shares.set_share(child, share);
    }
}

/// Which leaf pane the cursor is over (in the STABLE snapshot), and which edge/centre → the drop slot.
fn compute_drop(base: &Tree<PanelId>, base_rects: &[(TileId, Rect)], dragged: TileId, ptr: Pos2) -> Option<(TileId, Side)> {
    let mut hit = None;
    for &(id, r) in base_rects {
        if id == dragged || !matches!(base.tiles.get(id), Some(Tile::Pane(_))) {
            continue;
        }
        if r.contains(ptr) {
            hit = Some((id, r));
        }
    }
    let (tid, r) = hit?;
    let fx = ((ptr.x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0);
    let fy = ((ptr.y - r.top()) / r.height().max(1.0)).clamp(0.0, 1.0);
    let (l, rt, tp, bt) = (fx, 1.0 - fx, fy, 1.0 - fy);
    let m = l.min(rt).min(tp).min(bt);
    let board = matches!(base.tiles.get(tid), Some(Tile::Pane(PanelId::Board)));
    let side = if m > 0.28 && !board {
        Side::Center
    } else if m == l {
        Side::Left
    } else if m == rt {
        Side::Right
    } else if m == tp {
        Side::Top
    } else {
        Side::Bottom
    };
    Some((tid, side))
}

/// Move `dragged` next to `target` on the given side (split), or into a tab group (centre).
fn dock(tree: &mut Tree<PanelId>, dragged: TileId, target: TileId, side: Side) {
    if dragged == target {
        return;
    }
    detach(tree, dragged);
    match side {
        Side::Center => {
            let parent = tree.tiles.parent_of(target);
            if let Some(p) = parent {
                if let Some(Tile::Container(Container::Tabs(t))) = tree.tiles.get_mut(p) {
                    let idx = t.children.iter().position(|&x| x == target).map_or(t.children.len(), |i| i + 1);
                    t.children.insert(idx, dragged);
                    t.active = Some(dragged);
                    return;
                }
            }
            let new = tree.tiles.insert_tab_tile(vec![target, dragged]);
            if let Some(Tile::Container(Container::Tabs(t))) = tree.tiles.get_mut(new) {
                t.active = Some(dragged);
            }
            replace_child(tree, parent, target, new);
        }
        _ => {
            let horizontal = matches!(side, Side::Left | Side::Right);
            let before = matches!(side, Side::Left | Side::Top);
            let parent = tree.tiles.parent_of(target);
            // fast path: target's parent is already a Linear of the right direction → just insert
            if let Some(p) = parent {
                let same_dir = matches!(
                    tree.tiles.get(p),
                    Some(Tile::Container(Container::Linear(l))) if (l.dir == LinearDir::Horizontal) == horizontal
                );
                if same_dir {
                    if let Some(Tile::Container(Container::Linear(l))) = tree.tiles.get_mut(p) {
                        let pos = l.children.iter().position(|&x| x == target).unwrap_or(l.children.len());
                        let at = (if before { pos } else { pos + 1 }).min(l.children.len());
                        l.children.insert(at, dragged);
                    }
                    return;
                }
            }
            let children = if before { vec![dragged, target] } else { vec![target, dragged] };
            let new = if horizontal {
                tree.tiles.insert_horizontal_tile(children)
            } else {
                tree.tiles.insert_vertical_tile(children)
            };
            // balanced-ish split so the layout doesn't jump to weird proportions (target stays dominant)
            if let Some(Tile::Container(Container::Linear(l))) = tree.tiles.get_mut(new) {
                l.shares.set_share(dragged, 0.34);
                l.shares.set_share(target, 0.66);
            }
            replace_child(tree, parent, target, new);
        }
    }
}

/// Remove `id` from its parent container's child list (does NOT delete the tile itself).
fn detach(tree: &mut Tree<PanelId>, id: TileId) {
    if let Some(parent) = tree.tiles.parent_of(id) {
        if let Some(Tile::Container(c)) = tree.tiles.get_mut(parent) {
            match c {
                Container::Linear(l) => {
                    l.children.retain(|&x| x != id);
                    l.shares.retain(|x| x != id);
                }
                Container::Tabs(t) => {
                    t.children.retain(|&x| x != id);
                    if t.active == Some(id) {
                        t.active = t.children.first().copied();
                    }
                }
                Container::Grid(_) => {}
            }
        }
    }
}

/// Replace `target` with `new` in `parent`'s child list (or make `new` the root if there is no parent).
fn replace_child(tree: &mut Tree<PanelId>, parent: Option<TileId>, target: TileId, new: TileId) {
    match parent {
        Some(p) => {
            if let Some(Tile::Container(c)) = tree.tiles.get_mut(p) {
                match c {
                    Container::Linear(l) => {
                        if let Some(pos) = l.children.iter().position(|&x| x == target) {
                            l.children[pos] = new;
                            l.shares.replace_with(target, new);
                        }
                    }
                    Container::Tabs(t) => {
                        if let Some(pos) = t.children.iter().position(|&x| x == target) {
                            t.children[pos] = new;
                            if t.active == Some(target) {
                                t.active = Some(new);
                            }
                        }
                    }
                    Container::Grid(_) => {}
                }
            }
        }
        None => tree.root = Some(new),
    }
}

/// Small FIXED resize-grabber on the hovered seam.
fn draw_resize_handles(tree: &Tree<PanelId>, ui: &egui::Ui) {
    let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) else { return };
    for (_id, tile) in tree.tiles.iter() {
        let Tile::Container(Container::Linear(lin)) = tile else { continue };
        for pair in lin.children.windows(2) {
            let (Some(ra), Some(rb)) = (tree.tiles.rect(pair[0]), tree.tiles.rect(pair[1])) else { continue };
            match lin.dir {
                LinearDir::Horizontal => {
                    let x = (ra.right() + rb.left()) * 0.5;
                    if (ptr.x - x).abs() <= 6.0 && ptr.y >= ra.top() && ptr.y <= ra.bottom() {
                        ui.painter().rect_filled(Rect::from_center_size(pos2(x, ra.center().y), vec2(3.0, 22.0)), CornerRadius::same(2), T::FAINT);
                    }
                }
                LinearDir::Vertical => {
                    let y = (ra.bottom() + rb.top()) * 0.5;
                    if (ptr.y - y).abs() <= 6.0 && ptr.x >= ra.left() && ptr.x <= ra.right() {
                        ui.painter().rect_filled(Rect::from_center_size(pos2(ra.center().x, y), vec2(22.0, 3.0)), CornerRadius::same(2), T::FAINT);
                    }
                }
            }
        }
    }
}

// ───────────────────────── the behaviour ─────────────────────────

#[derive(Default)]
struct ShellBehavior {
    switch: Option<(TileId, PanelId)>,
    close: Option<TileId>,
    tabbed: HashSet<TileId>,
    dragging: Option<TileId>,
    drag_start: Option<TileId>,
}

impl Behavior<PanelId> for ShellBehavior {
    fn tab_title_for_pane(&mut self, pane: &PanelId) -> egui::WidgetText {
        pane.title().into()
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, pane: &mut PanelId) -> UiResponse {
        let rect = ui.max_rect();
        if pane.is_board() {
            draw_board(ui, rect);
            return UiResponse::None;
        }

        let being_dragged = self.dragging == Some(tile_id);
        let border = if being_dragged { Stroke::new(1.5, T::ACCENT) } else { T::hairline() };

        // MULTI-panel box: egui_tiles drew our styled tab bar above → just the body here.
        if self.tabbed.contains(&tile_id) {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, T::PANEL);
            egui::Frame::NONE
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| registry::render_panel(*pane, ui));
            return UiResponse::None;
        }

        // SINGLE-panel box: our dead-simple header + body.
        ui.painter().rect(rect, T::r_box(), T::PANEL, border, StrokeKind::Middle);
        let hh = 30.0;
        let pad = 10.0;
        let mid = rect.top() + hh / 2.0;

        // move-pill — hidden until the header is hovered; soft light-grey (Ahmed's reference)
        let header_rect = Rect::from_min_size(rect.min, vec2(rect.width(), hh));
        let header_hovered = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| header_rect.contains(p));
        let pill = Rect::from_center_size(pos2(rect.center().x, rect.top() + 8.0), vec2(30.0, 4.0));
        let mv = ui
            .interact(pill.expand2(vec2(10.0, 8.0)), ui.id().with(("move", tile_id)), Sense::click_and_drag())
            .on_hover_text("Move");
        if header_hovered || mv.dragged() {
            ui.painter().rect_filled(pill, CornerRadius::same(2), if mv.hovered() || mv.dragged() { T::TEXT } else { T::GRIP });
        }

        // name
        ui.painter().text(pos2(rect.left() + pad, mid), Align2::LEFT_CENTER, pane.title(), FontId::proportional(12.5), T::TEXT);

        // ✕ close
        let x_rect = Rect::from_min_size(pos2(rect.right() - pad - 18.0, mid - 9.0), vec2(18.0, 18.0));
        let x = ui.interact(x_rect, ui.id().with(("close", tile_id)), Sense::click());
        if x.hovered() {
            ui.painter().rect_filled(x_rect, T::r_ctrl(), T::HOVER);
        }
        paint_cross(ui, x_rect, if x.hovered() { T::CLOSE_RED } else { T::MUTED });

        // ☰ type menu
        let menu_rect = Rect::from_min_size(pos2(x_rect.left() - 6.0 - 22.0, mid - 12.0), vec2(22.0, 24.0));
        let mut menu_switch: Option<PanelId> = None;
        ui.scope_builder(UiBuilder::new().max_rect(menu_rect), |ui| {
            frameless_buttons(ui);
            ui.menu_button(RichText::new("☰").color(T::MUTED).size(14.0), |ui| {
                ui.set_min_width(172.0);
                ui.label(RichText::new("CHANGE THIS PANEL TO").color(T::FAINT).size(9.5).strong());
                for p in PanelId::DOCKABLE {
                    if ui.button(p.title()).clicked() {
                        menu_switch = Some(p);
                        ui.close();
                    }
                }
            });
        });
        if let Some(p) = menu_switch {
            self.switch = Some((tile_id, p));
        }

        ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, rect.top() + hh, T::hairline());

        let body = Rect::from_min_max(pos2(rect.left(), rect.top() + hh), rect.max);
        ui.scope_builder(
            UiBuilder::new().max_rect(body.shrink2(vec2(10.0, 8.0))).layout(Layout::top_down(Align::Min)),
            |ui| registry::render_panel(*pane, ui),
        );

        if x.clicked() {
            self.close = Some(tile_id);
        }
        // start our OWN drag from the move-pill (egui_tiles' drag stays off)
        if mv.drag_started() {
            self.drag_start = Some(tile_id);
        }
        UiResponse::None
    }

    fn top_bar_right_ui(&mut self, _tiles: &Tiles<PanelId>, ui: &mut egui::Ui, _tile_id: TileId, tabs: &Tabs, _scroll: &mut f32) {
        let active = tabs.active;
        let mut do_close = false;
        let mut do_switch: Option<PanelId> = None;
        ui.add_space(4.0);
        ui.scope(|ui| {
            frameless_buttons(ui);
            if ui.button(RichText::new("✕").color(T::MUTED).size(13.0)).clicked() {
                do_close = true;
            }
            ui.menu_button(RichText::new("☰").color(T::MUTED).size(14.0), |ui| {
                ui.set_min_width(172.0);
                ui.label(RichText::new("CHANGE THIS PANEL TO").color(T::FAINT).size(9.5).strong());
                for p in PanelId::DOCKABLE {
                    if ui.button(p.title()).clicked() {
                        do_switch = Some(p);
                        ui.close();
                    }
                }
            });
        });
        if do_close {
            if let Some(a) = active {
                self.close = Some(a);
            }
        }
        if let Some(p) = do_switch {
            if let Some(a) = active {
                self.switch = Some((a, p));
            }
        }
    }

    fn tab_bar_color(&self, _v: &Visuals) -> Color32 { T::PANEL }
    fn tab_bar_height(&self, _s: &egui::Style) -> f32 { 30.0 }
    fn tab_title_spacing(&self, _v: &Visuals) -> f32 { 12.0 }
    fn tab_bar_hline_stroke(&self, _v: &Visuals) -> Stroke { Stroke::new(1.0, T::LINE) }
    fn tab_bg_color(&self, _v: &Visuals, _t: &Tiles<PanelId>, _id: TileId, state: &TabState) -> Color32 {
        if state.active { T::SURFACE } else { Color32::TRANSPARENT }
    }
    fn tab_text_color(&self, _v: &Visuals, _t: &Tiles<PanelId>, _id: TileId, state: &TabState) -> Color32 {
        if state.active { T::TEXT } else { T::MUTED }
    }
    fn is_tab_closable(&self, _t: &Tiles<PanelId>, _id: TileId) -> bool { false }

    fn gap_width(&self, _style: &egui::Style) -> f32 { T::SEAM_GAP }
    fn is_tile_draggable(&self, _tiles: &Tiles<PanelId>, _tile_id: TileId) -> bool { false } // we drive drag ourselves
    fn min_size(&self) -> f32 { 120.0 }
    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_empty_tabs: true,
            prune_empty_containers: true,
            prune_single_child_tabs: true,
            prune_single_child_containers: true,
            all_panes_must_have_tabs: false,
            join_nested_linear_containers: true,
        }
    }
}

fn frameless_buttons(ui: &mut egui::Ui) {
    let v = ui.visuals_mut();
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = T::HOVER;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.weak_bg_fill = T::HOVER;
}

fn paint_cross(ui: &egui::Ui, rect: Rect, col: Color32) {
    let p = ui.painter();
    let r = rect.shrink(5.0);
    p.line_segment([r.left_top(), r.right_bottom()], Stroke::new(1.3, col));
    p.line_segment([r.right_top(), r.left_bottom()], Stroke::new(1.3, col));
}

// ───────────────────────── the dummy board + its two floating hands ─────────────────────────

fn draw_board(ui: &mut egui::Ui, rect: egui::Rect) {
    {
        let p = ui.painter();
        p.rect(rect, T::r_box(), T::BG, T::hairline(), StrokeKind::Middle);
        let step = 22.0;
        let mut y = rect.top() + 11.0;
        while y < rect.bottom() {
            let mut x = rect.left() + 11.0;
            while x < rect.right() {
                p.circle_filled(pos2(x, y), 1.0, T::DOT_GRID);
                x += step;
            }
            y += step;
        }
        let aw = (rect.width() * 0.44).min(360.0);
        let ah = (rect.height() * 0.62).min(380.0);
        if aw > 90.0 && ah > 90.0 {
            let ab = Rect::from_center_size(rect.center(), vec2(aw, ah));
            p.rect(ab, CornerRadius::ZERO, Color32::from_gray(245), Stroke::new(1.0, Color32::from_black_alpha(70)), StrokeKind::Middle);
            p.text(ab.center(), Align2::CENTER_CENTER, "VAROS", FontId::proportional((ah * 0.13).min(46.0)), T::NAVY);
        }
    }
    draw_hands(ui, rect);
}

fn draw_hands(ui: &egui::Ui, board: egui::Rect) {
    let p = ui.painter();
    let cw = (board.width() - 40.0).min(470.0);
    if board.width() > 240.0 && cw > 180.0 {
        let ch = 36.0;
        let bar = Rect::from_center_size(pos2(board.center().x, board.top() + 30.0 + ch / 2.0), vec2(cw, ch));
        p.rect(bar, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Middle);
        let cy = bar.center().y;
        let mut x = bar.left() + 10.0;
        p.text(pos2(x, cy), Align2::LEFT_CENTER, "Path", FontId::proportional(11.5), T::MUTED);
        x += 36.0;
        for (l, v) in [("X", "266"), ("Y", "118"), ("W", "126"), ("H", "64"), ("∠", "0°")] {
            let fw = 46.0;
            if x + fw > bar.right() - 30.0 { break; }
            let f = Rect::from_min_size(pos2(x, cy - 12.0), vec2(fw, 24.0));
            p.rect(f, T::r_ctrl(), T::SURFACE, T::hairline(), StrokeKind::Middle);
            p.text(pos2(f.left() + 6.0, cy), Align2::LEFT_CENTER, l, FontId::proportional(9.0), T::FAINT);
            p.text(pos2(f.left() + 17.0, cy), Align2::LEFT_CENTER, v, FontId::monospace(10.5), T::TEXT);
            x += fw + 5.0;
        }
        p.text(pos2(bar.right() - 15.0, cy), Align2::CENTER_CENTER, "◆", FontId::proportional(12.0), T::ACCENT);
    }
    if board.width() > 200.0 && board.height() > 220.0 {
        let top = board.top() + 104.0;
        let rh = (board.bottom() - top - 40.0).min(360.0);
        if rh > 120.0 {
            let rail = Rect::from_min_size(pos2(board.left() + 34.0, top), vec2(44.0, rh));
            p.rect(rail, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Middle);
            let tools = ["V", "A", "P", "M", "L", "T", "H", "Z", "I"];
            let mut ty = rail.top() + 18.0;
            for (i, g) in tools.iter().enumerate() {
                if ty > rail.bottom() - 40.0 { break; }
                let c = pos2(rail.center().x, ty);
                if i == 0 {
                    p.rect_filled(Rect::from_center_size(c, vec2(32.0, 32.0)), T::r_ctrl(), T::ACCENT);
                    p.text(c, Align2::CENTER_CENTER, *g, FontId::proportional(12.0), Color32::WHITE);
                } else {
                    p.text(c, Align2::CENTER_CENTER, *g, FontId::proportional(12.0), T::MUTED);
                }
                ty += 34.0;
            }
            let sc = pos2(rail.center().x, rail.bottom() - 22.0);
            p.rect(Rect::from_min_size(sc + vec2(-3.0, -3.0), vec2(15.0, 15.0)), CornerRadius::same(2), T::PANEL, Stroke::new(1.5, T::NAVY), StrokeKind::Middle);
            p.rect(Rect::from_min_size(sc + vec2(-11.0, -11.0), vec2(15.0, 15.0)), CornerRadius::same(2), T::AMBER, Stroke::new(1.5, T::LINE2), StrokeKind::Middle);
        }
    }
}

// ───────────────────────── headless tests (validate the docking logic) ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn col3() -> (Tree<PanelId>, TileId, TileId, TileId, TileId) {
        let mut tiles = Tiles::default();
        let a = tiles.insert_pane(PanelId::Align);
        let b = tiles.insert_pane(PanelId::Properties);
        let c = tiles.insert_pane(PanelId::Layers);
        let col = tiles.insert_vertical_tile(vec![a, b, c]);
        (Tree::new("t", col, tiles), col, a, b, c)
    }

    #[test]
    fn dock_reorder_within_column() {
        let (mut tree, col, a, _b, c) = col3();
        dock(&mut tree, c, a, Side::Top); // move c above a
        let Some(Tile::Container(Container::Linear(l))) = tree.tiles.get(col) else { panic!() };
        let pc = l.children.iter().position(|&x| x == c).unwrap();
        let pa = l.children.iter().position(|&x| x == a).unwrap();
        assert!(pc < pa, "c should now sit above a");
        assert_eq!(l.children.len(), 3, "no tile lost");
    }

    #[test]
    fn dock_right_makes_horizontal_split() {
        let (mut tree, _col, a, b, _c) = col3();
        dock(&mut tree, b, a, Side::Right); // b to the right of a → a new horizontal container around them
        let parent = tree.tiles.parent_of(a).unwrap();
        let Some(Tile::Container(Container::Linear(l))) = tree.tiles.get(parent) else { panic!() };
        assert_eq!(l.dir, LinearDir::Horizontal);
        assert!(l.children.contains(&a) && l.children.contains(&b));
        let pa = l.children.iter().position(|&x| x == a).unwrap();
        let pb = l.children.iter().position(|&x| x == b).unwrap();
        assert!(pa < pb, "a on the left, b on the right");
    }

    #[test]
    fn dock_center_tabifies() {
        let (mut tree, _col, a, _b, c) = col3();
        dock(&mut tree, c, a, Side::Center); // c onto a → tabs
        let parent = tree.tiles.parent_of(a).unwrap();
        let Some(Tile::Container(Container::Tabs(t))) = tree.tiles.get(parent) else { panic!() };
        assert!(t.children.contains(&a) && t.children.contains(&c));
        assert_eq!(t.active, Some(c));
    }

    #[test]
    fn standard_layout_serdes_roundtrip() {
        let shell = ShellState::standard();
        let json = shell.to_json().expect("serialise");
        assert!(json.contains("Board"));
        let _back: Tree<PanelId> = serde_json::from_str(&json).expect("deserialise");
    }

    #[test]
    fn renders_headless_without_panic() {
        let ctx = egui::Context::default();
        super::T::apply(&ctx);
        let mut shell = ShellState::standard();
        for _ in 0..3 {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| shell.ui(ui));
        }
    }
}
