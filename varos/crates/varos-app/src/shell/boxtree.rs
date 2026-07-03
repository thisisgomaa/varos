//! The box tree — the SINGLE place `egui_tiles` is used in the whole app (ruling 9). The rest of the
//! code holds only [`ShellState`] and calls its small API.
//!
//! egui_tiles owns the DYNAMIC layout: the recursive split tree, drag-resize, **drag a panel to re-dock
//! (with a drop preview)**, tabs, and serde. We supply only the *look* (via the `Behavior` styling hooks)
//! so it obeys the constitution: clean void seams (no divider strip), an azure grab-handle on hover,
//! chip-style tabs, and a Blender-style ☰ menu on each box to CHANGE its panel or ADD a tab.
//! Each pane is one `PanelId`; a box = a Tabs container.
use egui::{Align2, Color32, FontId, Margin, RichText, Stroke, StrokeKind, Visuals, vec2};
use egui_tiles::{
    Behavior, Container, ResizeState, SimplificationOptions, TabState, Tabs, Tile, TileId, Tiles, Tree,
    UiResponse,
};
use super::registry::{self, PanelId};
use super::tokens as T;

/// The public handle the whole app holds. The app never names `egui_tiles`.
pub struct ShellState {
    tree: Tree<PanelId>,
}

impl ShellState {
    /// The STANDARD layout as a default tree value (BOX_SYSTEM_PLAN §4.3):
    /// `Split(H)[ Board , Split(V)[ Tabs[Align|Pathfinder] , Tabs[Properties|Layers] ] ]`.
    pub fn standard() -> Self {
        let mut tiles = Tiles::default();
        let board = tiles.insert_pane(PanelId::Board); // a bare pane → no tab bar
        let align = tiles.insert_pane(PanelId::Align);
        let pathfinder = tiles.insert_pane(PanelId::Pathfinder);
        let upper = tiles.insert_tab_tile(vec![align, pathfinder]);
        let props = tiles.insert_pane(PanelId::Properties);
        let layers = tiles.insert_pane(PanelId::Layers);
        let lower = tiles.insert_tab_tile(vec![props, layers]);
        let right = tiles.insert_vertical_tile(vec![upper, lower]);
        let root = tiles.insert_horizontal_tile(vec![board, right]);
        let mut tree = Tree::new("varos_shell", root, tiles);
        // the board is the abundant dimension (mockup: right column ~274px)
        set_share(&mut tree, root, board, 0.80);
        set_share(&mut tree, root, right, 0.20);
        set_share(&mut tree, right, upper, 0.34);
        set_share(&mut tree, right, lower, 0.66);
        Self { tree }
    }

    /// Render the whole box tree into `ui` (fills the available rect); the seams show the void behind.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut behavior = ShellBehavior::default();
        self.tree.ui(&mut behavior, ui);
        // apply the deferred ☰-menu intent (top_bar_right_ui only had &Tiles)
        match behavior.action {
            Some(MenuAction::Add(container, panel)) => {
                let new_pane = self.tree.tiles.insert_pane(panel);
                if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(container) {
                    tabs.children.push(new_pane);
                    tabs.active = Some(new_pane);
                }
            }
            Some(MenuAction::Switch(container, panel)) => {
                // Blender-style "change this panel": swap the box's ACTIVE tab to a new type.
                let active = match self.tree.tiles.get(container) {
                    Some(Tile::Container(Container::Tabs(t))) => t.active,
                    _ => None,
                };
                if let Some(pane_id) = active {
                    if let Some(Tile::Pane(p)) = self.tree.tiles.get_mut(pane_id) {
                        *p = panel;
                    }
                }
            }
            None => {}
        }
    }

    /// Serialise the layout — proves "serde from day one" (future: saved workspaces).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.tree)
    }
}

/// Bias a Linear container's child share. Reaching into egui_tiles internals stays inside this file.
fn set_share(tree: &mut Tree<PanelId>, container: TileId, child: TileId, share: f32) {
    if let Some(Tile::Container(Container::Linear(lin))) = tree.tiles.get_mut(container) {
        lin.shares.set_share(child, share);
    }
}

fn is_board(tiles: &Tiles<PanelId>, id: TileId) -> bool {
    matches!(tiles.get(id), Some(Tile::Pane(PanelId::Board)))
}

// ───────────────────────── the behaviour: our look over egui_tiles' dynamic layout ─────────────────────────

/// A deferred edit requested from a box's ☰ menu, applied after the tree is drawn.
enum MenuAction {
    /// Change the box's active tab to a new panel type (Blender editor-type style).
    Switch(TileId, PanelId),
    /// Add a new panel as a tab in the box.
    Add(TileId, PanelId),
}

#[derive(Default)]
struct ShellBehavior {
    action: Option<MenuAction>,
}

impl Behavior<PanelId> for ShellBehavior {
    fn tab_title_for_pane(&mut self, pane: &PanelId) -> egui::WidgetText {
        pane.title().into()
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut PanelId) -> UiResponse {
        let rect = ui.max_rect();
        if pane.is_board() {
            draw_board(ui, rect);
        } else {
            ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, T::PANEL);
            egui::Frame::NONE
                .inner_margin(Margin { left: 10, right: 10, top: 8, bottom: 10 })
                .show(ui, |ui| registry::render_panel(*pane, ui));
        }
        UiResponse::None
    }

    /// Blender-style editor-type menu (☰) at the top-right of every box: change what this box shows, or
    /// add another panel as a tab.
    fn top_bar_right_ui(&mut self, _tiles: &Tiles<PanelId>, ui: &mut egui::Ui, tile_id: TileId, _tabs: &Tabs, _scroll: &mut f32) {
        ui.add_space(4.0);
        ui.menu_button(RichText::new("☰").color(T::MUTED).size(14.0), |ui| {
            ui.set_min_width(184.0);
            ui.label(RichText::new("CHANGE THIS PANEL TO").color(T::FAINT).size(9.5).strong());
            for p in PanelId::DOCKABLE {
                if ui.button(p.title()).clicked() {
                    self.action = Some(MenuAction::Switch(tile_id, p));
                    ui.close();
                }
            }
            ui.separator();
            ui.label(RichText::new("ADD AS A NEW TAB").color(T::FAINT).size(9.5).strong());
            for p in PanelId::DOCKABLE {
                if ui.button(format!("＋  {}", p.title())).clicked() {
                    self.action = Some(MenuAction::Add(tile_id, p));
                    ui.close();
                }
            }
        });
    }

    // ── the seams: clean void, azure grab-handle on hover (fixes the "ugly strip") ──
    fn gap_width(&self, _style: &egui::Style) -> f32 { T::SEAM_GAP }
    fn resize_stroke(&self, _style: &egui::Style, state: ResizeState) -> Stroke {
        match state {
            ResizeState::Idle => Stroke::NONE,                    // pure void — NO divider strip
            ResizeState::Hovering => Stroke::new(2.0, T::ACCENT), // the grab affordance
            ResizeState::Dragging => Stroke::new(2.0, T::ACCENT),
        }
    }

    // ── chip-style tabs (active = filled SURFACE pill, inactive = bare muted text) ──
    fn tab_bar_color(&self, _visuals: &Visuals) -> Color32 { T::PANEL }
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 { 30.0 }
    fn tab_title_spacing(&self, _visuals: &Visuals) -> f32 { 10.0 }
    fn tab_bg_color(&self, _v: &Visuals, _t: &Tiles<PanelId>, _id: TileId, state: &TabState) -> Color32 {
        if state.active { T::SURFACE } else { Color32::TRANSPARENT }
    }
    fn tab_text_color(&self, _v: &Visuals, _t: &Tiles<PanelId>, _id: TileId, state: &TabState) -> Color32 {
        if state.active { T::TEXT } else { T::MUTED }
    }
    fn tab_outline_stroke(&self, _v: &Visuals, _t: &Tiles<PanelId>, _id: TileId, _s: &TabState) -> Stroke {
        Stroke::NONE
    }
    fn tab_bar_hline_stroke(&self, _visuals: &Visuals) -> Stroke { Stroke::new(1.0, T::LINE) }

    // ── the drop preview when dragging a panel to re-dock (azure, per rule 4) ──
    fn drag_preview_stroke(&self, _visuals: &Visuals) -> Stroke { Stroke::new(1.5, T::ACCENT) }
    fn drag_preview_color(&self, _visuals: &Visuals) -> Color32 {
        Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, 40)
    }
    fn dragged_overlay_color(&self, _visuals: &Visuals) -> Color32 {
        Color32::from_rgba_unmultiplied(0x1b, 0x19, 0x19, 160)
    }

    // ── dynamics: tabs draggable to re-dock, closable; the board stays put ──
    fn is_tile_draggable(&self, tiles: &Tiles<PanelId>, tile_id: TileId) -> bool { !is_board(tiles, tile_id) }
    fn is_tab_closable(&self, _tiles: &Tiles<PanelId>, _tile_id: TileId) -> bool { true }
    fn min_size(&self) -> f32 { 150.0 }
    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_empty_tabs: true,
            prune_empty_containers: true,
            prune_single_child_tabs: false,   // keep the tab bar (+ ☰) even with one tab
            prune_single_child_containers: true,
            all_panes_must_have_tabs: false,  // the board (a bare pane) has NO tab bar
            join_nested_linear_containers: true,
        }
    }
}

// ───────────────────────── the dummy board + its two floating hands ─────────────────────────

/// The dummy Board: warm-black canvas + dotted grid (mockup 22px) + a page, with the two floating hands over it.
fn draw_board(ui: &mut egui::Ui, rect: egui::Rect) {
    {
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::ZERO, T::BG);
        let step = 22.0;
        let mut y = rect.top() + 11.0;
        while y < rect.bottom() {
            let mut x = rect.left() + 11.0;
            while x < rect.right() {
                p.circle_filled(egui::pos2(x, y), 1.0, T::DOT_GRID);
                x += step;
            }
            y += step;
        }
        // a dummy "artboard" (the page) so the board reads as a real canvas, not an empty field
        let aw = (rect.width() * 0.44).min(360.0);
        let ah = (rect.height() * 0.62).min(380.0);
        if aw > 90.0 && ah > 90.0 {
            let ab = egui::Rect::from_center_size(rect.center(), vec2(aw, ah));
            p.rect(ab, egui::CornerRadius::ZERO, Color32::from_gray(245), Stroke::new(1.0, Color32::from_black_alpha(70)), StrokeKind::Middle);
            p.text(ab.center(), Align2::CENTER_CENTER, "VAROS", FontId::proportional((ah * 0.13).min(46.0)), T::NAVY);
        }
    }
    draw_hands(ui, rect);
}

/// The two DUMMY floating hands over the board: HAND 1 = centred control-bar strip, HAND 2 = left tool-rail.
/// Painted placeholders, clamped to fit — they shrink/vanish on a tiny board instead of overflowing it.
fn draw_hands(ui: &egui::Ui, board: egui::Rect) {
    let p = ui.painter();

    // ── HAND 1 — control bar (top of board, centred) ──
    let cw = (board.width() - 40.0).min(470.0);
    if board.width() > 240.0 && cw > 180.0 {
        let ch = 36.0;
        let bar = egui::Rect::from_center_size(egui::pos2(board.center().x, board.top() + 30.0 + ch / 2.0), vec2(cw, ch));
        p.rect(bar, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Middle);
        let cy = bar.center().y;
        let mut x = bar.left() + 10.0;
        p.text(egui::pos2(x, cy), Align2::LEFT_CENTER, "Path", FontId::proportional(11.5), T::MUTED);
        x += 36.0;
        for (l, v) in [("X", "266"), ("Y", "118"), ("W", "126"), ("H", "64"), ("∠", "0°")] {
            let fw = 46.0;
            if x + fw > bar.right() - 30.0 { break; }
            let f = egui::Rect::from_min_size(egui::pos2(x, cy - 12.0), vec2(fw, 24.0));
            p.rect(f, T::r_ctrl(), T::SURFACE, T::hairline(), StrokeKind::Middle);
            p.text(egui::pos2(f.left() + 6.0, cy), Align2::LEFT_CENTER, l, FontId::proportional(9.0), T::FAINT);
            p.text(egui::pos2(f.left() + 17.0, cy), Align2::LEFT_CENTER, v, FontId::monospace(10.5), T::TEXT);
            x += fw + 5.0;
        }
        p.text(egui::pos2(bar.right() - 15.0, cy), Align2::CENTER_CENTER, "◆", FontId::proportional(12.0), T::ACCENT);
    }

    // ── HAND 2 — tool rail (left, over the board) ──
    if board.width() > 200.0 && board.height() > 220.0 {
        let top = board.top() + 104.0;
        let rh = (board.bottom() - top - 40.0).min(360.0);
        if rh > 120.0 {
            let rail = egui::Rect::from_min_size(egui::pos2(board.left() + 34.0, top), vec2(44.0, rh));
            p.rect(rail, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Middle);
            let tools = ["V", "A", "P", "M", "L", "T", "H", "Z", "I"];
            let mut ty = rail.top() + 18.0;
            for (i, g) in tools.iter().enumerate() {
                if ty > rail.bottom() - 40.0 { break; }
                let c = egui::pos2(rail.center().x, ty);
                if i == 0 {
                    p.rect_filled(egui::Rect::from_center_size(c, vec2(32.0, 32.0)), T::r_ctrl(), T::ACCENT);
                    p.text(c, Align2::CENTER_CENTER, *g, FontId::proportional(12.0), Color32::WHITE);
                } else {
                    p.text(c, Align2::CENTER_CENTER, *g, FontId::proportional(12.0), T::MUTED);
                }
                ty += 34.0;
            }
            let sc = egui::pos2(rail.center().x, rail.bottom() - 22.0);
            p.rect(egui::Rect::from_min_size(sc + vec2(-3.0, -3.0), vec2(15.0, 15.0)), egui::CornerRadius::same(2), T::PANEL, Stroke::new(1.5, T::NAVY), StrokeKind::Middle);
            p.rect(egui::Rect::from_min_size(sc + vec2(-11.0, -11.0), vec2(15.0, 15.0)), egui::CornerRadius::same(2), T::AMBER, Stroke::new(1.5, T::LINE2), StrokeKind::Middle);
        }
    }
}

// ───────────────────────── headless safety net (no window needed) ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_layout_serdes_roundtrip() {
        let shell = ShellState::standard();
        let json = shell.to_json().expect("serialise");
        assert!(json.contains("Board"));
        let _back: Tree<PanelId> = serde_json::from_str(&json).expect("deserialise"); // serde from day one
    }

    /// Render the whole tree headlessly for a few frames — catches any runtime panic in pane_ui /
    /// egui_tiles without a window (Ahmed still verifies the *look/feel* by hand).
    #[test]
    fn renders_headless_without_panic() {
        let ctx = egui::Context::default();
        super::T::apply(&ctx);
        let mut shell = ShellState::standard();
        for _ in 0..3 {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| shell.ui(ui));
        }
    }

    /// Render EVERY panel body headlessly (not just the default-active tabs).
    #[test]
    fn every_panel_body_renders_headless() {
        let ctx = egui::Context::default();
        super::T::apply(&ctx);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            for id in PanelId::DOCKABLE {
                registry::render_panel(id, ui);
            }
        });
    }
}
