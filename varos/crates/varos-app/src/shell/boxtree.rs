//! The box tree — the SINGLE place `egui_tiles` is used in the whole app (ruling 9). The rest of the
//! code holds only [`ShellState`] and calls its small API.
//!
//! Model = Blender editor-areas: each box is ONE panel with a dead-simple header we draw ourselves —
//! `[name] … [·move pill·] … [☰ type] [✕ close]`. egui_tiles owns only the DYNAMIC layout: the split
//! tree, thin drag-resize, and drag-to-re-dock (we start the drag from the move pill via
//! `UiResponse::DragStarted`, with egui_tiles painting the drop preview). Everything visual is ours.
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind,
    UiBuilder, Visuals, pos2, vec2,
};
use egui_tiles::{
    Behavior, Container, LinearDir, ResizeState, SimplificationOptions, Tile, TileId, Tiles, Tree,
    UiResponse,
};
use super::registry::{self, PanelId};
use super::tokens as T;

/// The public handle the whole app holds. The app never names `egui_tiles`.
pub struct ShellState {
    tree: Tree<PanelId>,
}

impl ShellState {
    /// The STANDARD layout: `Split(H)[ Board , Split(V)[ Align , Properties , Layers ] ]` — each box is one
    /// bare panel (Blender-style). Change any box's type with its ☰; drag its move-pill to re-dock.
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
        Self { tree }
    }

    /// Render the whole box tree into `ui` (fills the available rect); the seams show the void behind.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut behavior = ShellBehavior::default();
        self.tree.ui(&mut behavior, ui);

        // change a box's panel type (from its ☰ menu)
        if let Some((id, panel)) = behavior.switch {
            if let Some(Tile::Pane(p)) = self.tree.tiles.get_mut(id) {
                *p = panel;
            }
        }
        // close a box: detach from its parent container, then drop it (simplification prunes the rest)
        if let Some(id) = behavior.close {
            if let Some(parent) = self.tree.tiles.parent_of(id) {
                if let Some(Tile::Container(c)) = self.tree.tiles.get_mut(parent) {
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
            self.tree.tiles.remove(id);
        }

        // a small resize grabber that shows ONLY when hovering a seam (Ahmed: like the move handle)
        draw_resize_handles(&self.tree, ui);
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

/// Paint a small resize-grabber pill on the seam under the pointer (hover only) — egui_tiles owns the
/// drag interaction; this is just the little handle Ahmed asked for, following the cursor along the seam.
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
                        let y = ptr.y.clamp(ra.top() + 20.0, ra.bottom() - 20.0);
                        ui.painter().rect_filled(Rect::from_center_size(pos2(x, y), vec2(4.0, 34.0)), CornerRadius::same(2), T::MUTED);
                    }
                }
                LinearDir::Vertical => {
                    let y = (ra.bottom() + rb.top()) * 0.5;
                    if (ptr.y - y).abs() <= 6.0 && ptr.x >= ra.left() && ptr.x <= ra.right() {
                        let x = ptr.x.clamp(ra.left() + 20.0, ra.right() - 20.0);
                        ui.painter().rect_filled(Rect::from_center_size(pos2(x, y), vec2(34.0, 4.0)), CornerRadius::same(2), T::MUTED);
                    }
                }
            }
        }
    }
}

// ───────────────────────── the behaviour: our simple header over egui_tiles' dynamic layout ─────────────────────────

#[derive(Default)]
struct ShellBehavior {
    switch: Option<(TileId, PanelId)>, // change this box's panel type
    close: Option<TileId>,             // close this box
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

        // the box: rounded panel fill + one hairline (rules 2 & 3, rounded a touch per Ahmed)
        ui.painter().rect(rect, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Middle);

        let hh = 30.0;
        let pad = 10.0;
        let mid = rect.top() + hh / 2.0;

        // ── the move-pill: HIDDEN until you hover the header, then a small grab handle (Windows/Claude style) ──
        let header_rect = Rect::from_min_size(rect.min, vec2(rect.width(), hh));
        let header_hovered = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| header_rect.contains(p));
        let move_id = ui.id().with(("move", tile_id));
        let pill = Rect::from_center_size(pos2(rect.center().x, rect.top() + 8.0), vec2(30.0, 4.0));
        let mv = ui
            .interact(pill.expand2(vec2(10.0, 8.0)), move_id, Sense::click_and_drag())
            .on_hover_text("Move");
        if header_hovered || mv.dragged() {
            ui.painter().rect_filled(pill, CornerRadius::same(2), if mv.hovered() || mv.dragged() { T::TEXT } else { T::MUTED });
        }

        // ── name (left) ──
        ui.painter().text(pos2(rect.left() + pad, mid), Align2::LEFT_CENTER, pane.title(), FontId::proportional(12.5), T::TEXT);

        // ── ✕ close (far right) ──
        let close_id = ui.id().with(("close", tile_id));
        let x_rect = Rect::from_min_size(pos2(rect.right() - pad - 18.0, mid - 9.0), vec2(18.0, 18.0));
        let x = ui.interact(x_rect, close_id, Sense::click());
        if x.hovered() {
            ui.painter().rect_filled(x_rect, T::r_ctrl(), T::HOVER);
        }
        paint_cross(ui, x_rect, if x.hovered() { T::CLOSE_RED } else { T::MUTED });

        // ── ☰ type menu (left of the ✕) ──
        let menu_rect = Rect::from_min_size(pos2(x_rect.left() - 6.0 - 22.0, mid - 12.0), vec2(22.0, 24.0));
        ui.scope_builder(UiBuilder::new().max_rect(menu_rect), |ui| {
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
            ui.menu_button(RichText::new("☰").color(T::MUTED).size(14.0), |ui| {
                ui.set_min_width(172.0);
                ui.label(RichText::new("CHANGE THIS PANEL TO").color(T::FAINT).size(9.5).strong());
                for p in PanelId::DOCKABLE {
                    if ui.button(p.title()).clicked() {
                        self.switch = Some((tile_id, p));
                        ui.close();
                    }
                }
            });
        });

        // hairline under the header
        ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, rect.top() + hh, T::hairline());

        // ── body ──
        let body = Rect::from_min_max(pos2(rect.left(), rect.top() + hh), rect.max);
        ui.scope_builder(
            UiBuilder::new().max_rect(body.shrink2(vec2(10.0, 8.0))).layout(Layout::top_down(Align::Min)),
            |ui| registry::render_panel(*pane, ui),
        );

        if x.clicked() {
            self.close = Some(tile_id);
        }
        if mv.drag_started() {
            return UiResponse::DragStarted; // grab the move-pill → egui_tiles re-docks with a drop preview
        }
        UiResponse::None
    }

    // ── seams: pure void, a THIN azure line only while hovering/dragging (Ahmed: small, on the mouse) ──
    fn gap_width(&self, _style: &egui::Style) -> f32 { T::SEAM_GAP }
    fn resize_stroke(&self, _style: &egui::Style, _state: ResizeState) -> Stroke {
        Stroke::NONE // no full-height divider line at all — just the resize cursor (Ahmed: keep it tiny)
    }

    // ── drop preview when re-docking (azure, per rule 4) ──
    fn drag_preview_stroke(&self, _visuals: &Visuals) -> Stroke { Stroke::new(1.5, T::ACCENT) }
    fn drag_preview_color(&self, _visuals: &Visuals) -> Color32 {
        Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, 40)
    }
    fn dragged_overlay_color(&self, _visuals: &Visuals) -> Color32 {
        Color32::from_rgba_unmultiplied(0x1b, 0x19, 0x19, 170)
    }

    // ── dynamics ──
    fn is_tile_draggable(&self, tiles: &Tiles<PanelId>, tile_id: TileId) -> bool { !is_board(tiles, tile_id) }
    fn min_size(&self) -> f32 { 120.0 }
    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_empty_tabs: true,
            prune_empty_containers: true,
            prune_single_child_tabs: true,
            prune_single_child_containers: true,
            all_panes_must_have_tabs: false, // bare panes → NO egui_tiles tab bar (we draw our own header)
            join_nested_linear_containers: true,
        }
    }
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

/// The two DUMMY floating hands over the board: HAND 1 = centred control-bar strip, HAND 2 = left tool-rail.
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

// ───────────────────────── headless safety net (no window needed) ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
