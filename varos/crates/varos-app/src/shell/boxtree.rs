//! The box tree — the SINGLE place `egui_tiles` is used in the whole app (ruling 9). The rest of the
//! code holds only [`ShellState`] and calls its small API. egui_tiles owns exactly one job here: the
//! recursive split layout + drag-resize + serde. The chip-tabs, the `⌄` host-menu, the box chrome and
//! all panel bodies are OURS, drawn in plain egui — so if the crate ever dies we swap this file alone.
use egui::{Align, Align2, FontId, Layout, Margin, RichText, Sense, StrokeKind, vec2};
use egui_tiles::{Behavior, Container, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse};
use super::registry::{self, PanelId};
use super::tokens as T;

/// One box in the tree = a set of panels shown as chip-tabs, with one active. This is the egui_tiles
/// "pane" type, so the whole tree (and future saved workspaces) serialise straight through serde.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BoxState {
    pub panels: Vec<PanelId>,
    pub active: usize,
}

impl BoxState {
    fn one(p: PanelId) -> Self { Self { panels: vec![p], active: 0 } }
    fn many(panels: Vec<PanelId>) -> Self { Self { panels, active: 0 } }
    fn active_panel(&self) -> PanelId {
        self.panels[self.active.min(self.panels.len().saturating_sub(1))]
    }
}

/// The public handle the whole app holds. The app never names `egui_tiles`.
pub struct ShellState {
    tree: Tree<BoxState>,
}

impl ShellState {
    /// The STANDARD layout as a default tree value (BOX_SYSTEM_PLAN §4.3), built from dummy panels:
    /// `Split(H)[ Board , Split(V)[ [Align|Pathfinder] , [Properties|Layers] ] ]`.
    pub fn standard() -> Self {
        let mut tiles = Tiles::default();
        let board = tiles.insert_pane(BoxState::one(PanelId::Board));
        let upper = tiles.insert_pane(BoxState::many(vec![PanelId::Align, PanelId::Pathfinder]));
        let lower = tiles.insert_pane(BoxState::many(vec![PanelId::Properties, PanelId::Layers]));
        let right = tiles.insert_vertical_tile(vec![upper, lower]);
        let root = tiles.insert_horizontal_tile(vec![board, right]);
        let mut tree = Tree::new("varos_shell", root, tiles);
        // biased shares → the board is the abundant dimension (mockup: right column ~274px)
        set_share(&mut tree, root, board, 0.80);
        set_share(&mut tree, root, right, 0.20);
        set_share(&mut tree, right, upper, 0.34);
        set_share(&mut tree, right, lower, 0.66);
        Self { tree }
    }

    /// Render the whole box tree into `ui` (fills the available rect); the 6px seams show the void behind.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut behavior = ShellBehavior;
        self.tree.ui(&mut behavior, ui);
    }

    /// Serialise the layout — proves "serde from day one" (future: saved workspaces).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.tree)
    }
}

/// Bias a Linear container's child share. Reaching into egui_tiles internals stays inside this file.
fn set_share(tree: &mut Tree<BoxState>, container: TileId, child: TileId, share: f32) {
    if let Some(Tile::Container(Container::Linear(lin))) = tree.tiles.get_mut(container) {
        lin.shares.set_share(child, share);
    }
}

// ───────────────────────── the behaviour (our look, over egui_tiles' layout) ─────────────────────────

struct ShellBehavior;

impl Behavior<BoxState> for ShellBehavior {
    fn tab_title_for_pane(&mut self, pane: &BoxState) -> egui::WidgetText {
        pane.active_panel().title().into()
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, _id: TileId, pane: &mut BoxState) -> UiResponse {
        let rect = ui.max_rect();
        if pane.active_panel().is_board() {
            draw_board(ui, rect);
            return UiResponse::None;
        }

        // the box: panel fill + hairline + near-sharp corners (rules 2 & 3)
        ui.painter().rect(rect, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Middle);

        // ── header: chip-tabs on the left, the ⌄ host-menu on the right ──
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(5.0);
            let mut clicked = None;
            for i in 0..pane.panels.len() {
                if chip_tab(ui, pane.panels[i].title(), i == pane.active).clicked() {
                    clicked = Some(i);
                }
                ui.add_space(4.0);
            }
            if let Some(i) = clicked { pane.active = i; }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| host_menu(ui, pane));
        });
        ui.add_space(3.0);
        let sep_y = ui.cursor().top();
        ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, sep_y, T::hairline());

        // ── content ──
        egui::Frame::NONE
            .inner_margin(Margin { left: 10, right: 10, top: 6, bottom: 10 })
            .show(ui, |ui| registry::render_panel(pane.active_panel(), ui));

        UiResponse::None
    }

    // ── the look: equal 6px void seams, no drag-tear (that's Stage 5), stable tree, min sizes ──
    fn gap_width(&self, _style: &egui::Style) -> f32 { T::SEAM_GAP }
    fn min_size(&self) -> f32 { 140.0 }
    fn is_tile_draggable(&self, _tiles: &Tiles<BoxState>, _id: TileId) -> bool { false }
    fn simplification_options(&self) -> SimplificationOptions { SimplificationOptions::OFF }
}

// ───────────────────────── our hand-painted chrome ─────────────────────────

/// A Brave/Claude chip-tab: active = filled SURFACE pill, inactive = bare muted text, hover = faint wash.
fn chip_tab(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    let text_w = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(12.0), T::TEXT).size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(text_w + 22.0, 28.0), Sense::click());
    let p = ui.painter();
    if active { p.rect_filled(rect, T::r_ctrl(), T::SURFACE); }
    else if resp.hovered() { p.rect_filled(rect, T::r_ctrl(), T::VOID_HOVER); }
    let col = if active || resp.hovered() { T::TEXT } else { T::MUTED };
    p.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), col);
    resp
}

/// The `⌄` menu — "click a box, choose what it hosts" (Stage 2's headline interaction).
fn host_menu(ui: &mut egui::Ui, pane: &mut BoxState) {
    ui.menu_button(RichText::new("⌄").color(T::MUTED).size(15.0), |ui| {
        ui.set_min_width(168.0);
        ui.label(RichText::new("HOST IN THIS BOX").color(T::FAINT).size(9.5).strong());
        for p in PanelId::DOCKABLE {
            let here = pane.panels.get(pane.active) == Some(&p);
            if ui.selectable_label(here, p.title()).clicked() {
                pane.panels[pane.active] = p; // swap what this box hosts
                ui.close();
            }
        }
        ui.separator();
        for p in PanelId::DOCKABLE {
            if ui.button(format!("+  Add “{}” tab", p.title())).clicked() {
                pane.panels.push(p);
                pane.active = pane.panels.len() - 1;
                ui.close();
            }
        }
        if pane.panels.len() > 1 {
            ui.separator();
            if ui.button(RichText::new("✕  Close this tab").color(T::NONE_RED)).clicked() {
                pane.panels.remove(pane.active);
                if pane.active >= pane.panels.len() { pane.active = pane.panels.len().saturating_sub(1); }
                ui.close();
            }
        }
    });
}

/// The dummy Board: warm-black canvas + dotted grid (mockup 22px), with the two floating hands over it.
fn draw_board(ui: &mut egui::Ui, rect: egui::Rect) {
    {
        let p = ui.painter();
        p.rect(rect, T::r_box(), T::BG, T::hairline(), StrokeKind::Middle);
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
    }
    draw_hands(ui, rect);
}

/// The two DUMMY floating hands over the board (Stage 3): HAND 1 = centred control-bar strip,
/// HAND 2 = left tool-rail column. Painted placeholders, clamped to fit — they shrink/vanish on a
/// tiny board (the edge case) instead of overflowing it.
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
        // the active snap-magnet, azure (rule 4: accent only on active)
        p.text(egui::pos2(bar.right() - 15.0, cy), Align2::CENTER_CENTER, "◆", FontId::proportional(12.0), T::ACCENT);
    }

    // ── HAND 2 — tool rail (left, over the board) ──
    if board.width() > 200.0 && board.height() > 220.0 {
        let rw = 44.0;
        let top = board.top() + 104.0;
        let rh = (board.bottom() - top - 40.0).min(360.0);
        if rh > 120.0 {
            let rail = egui::Rect::from_min_size(egui::pos2(board.left() + 34.0, top), vec2(rw, rh));
            p.rect(rail, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Middle);
            let tools = ["V", "A", "P", "M", "L", "T", "H", "Z", "I"]; // Illustrator tool letters (placeholder icons)
            let mut ty = rail.top() + 18.0;
            for (i, g) in tools.iter().enumerate() {
                if ty > rail.bottom() - 40.0 { break; }
                let c = egui::pos2(rail.center().x, ty);
                if i == 0 {
                    p.rect_filled(egui::Rect::from_center_size(c, vec2(32.0, 32.0)), T::r_ctrl(), T::ACCENT);
                    p.text(c, Align2::CENTER_CENTER, *g, FontId::proportional(12.0), egui::Color32::WHITE);
                } else {
                    p.text(c, Align2::CENTER_CENTER, *g, FontId::proportional(12.0), T::MUTED);
                }
                ty += 34.0;
            }
            // Fill/Stroke swatch cluster at the bottom (Illustrator DNA)
            let sc = egui::pos2(rail.center().x, rail.bottom() - 22.0);
            p.rect(egui::Rect::from_min_size(sc + vec2(-3.0, -3.0), vec2(15.0, 15.0)), egui::CornerRadius::same(2), T::PANEL, egui::Stroke::new(1.5, T::NAVY), StrokeKind::Middle);
            p.rect(egui::Rect::from_min_size(sc + vec2(-11.0, -11.0), vec2(15.0, 15.0)), egui::CornerRadius::same(2), T::AMBER, egui::Stroke::new(1.5, T::LINE2), StrokeKind::Middle);
        }
    }
}

// ───────────────────────── headless safety net (no window needed) ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_panel_and_host_swap() {
        let mut b = BoxState::many(vec![PanelId::Align, PanelId::Pathfinder]);
        b.active = 1;
        assert_eq!(b.active_panel(), PanelId::Pathfinder);
        b.panels[b.active] = PanelId::Layers; // the ⌄ swap
        assert_eq!(b.active_panel(), PanelId::Layers);
    }

    #[test]
    fn standard_layout_serdes_roundtrip() {
        let shell = ShellState::standard();
        let json = shell.to_json().expect("serialise");
        assert!(json.contains("Board"));
        let _back: Tree<BoxState> = serde_json::from_str(&json).expect("deserialise"); // serde from day one
    }

    /// Render the whole tree headlessly for a few frames — catches any runtime panic in pane_ui /
    /// egui_tiles without a window (Ahmed still verifies the *look* by hand).
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
