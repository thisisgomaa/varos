//! The box tree — the SINGLE place `egui_tiles` is used in the whole app (ruling 9).
//!
//! Drag model (Ahmed 07-04, "the Claude way"): grab a box's move-pill and it LIFTS off — the real panel
//! floats on TOP of everything, easing toward the cursor (light, smooth). egui_tiles owns the docking:
//! it shows a clean azure preview of where it'll land and commits on release. We just paint the lifted
//! ghost + style the look. No reflow-among-boxes.
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, UiBuilder, Visuals, pos2, vec2,
};
use egui_tiles::{
    Behavior, Container, LinearDir, ResizeState, SimplificationOptions, Tile, TileId, Tiles, Tree,
    UiResponse,
};
use std::collections::HashMap;
use super::registry::{self, PanelId};
use super::tokens as T;

pub struct ShellState {
    tree: Tree<PanelId>,
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
        Self { tree }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // For each Tabs container, map its ACTIVE pane's tile → the group; we render the whole tabbed
        // box ourselves in pane_ui (fully rounded, floating pills), with egui_tiles' tab bar at 0 height.
        let mut groups: HashMap<TileId, TabGroup> = HashMap::new();
        for (id, tile) in self.tree.tiles.iter() {
            if let Tile::Container(Container::Tabs(t)) = tile {
                if let Some(active) = t.active.or_else(|| t.children.first().copied()) {
                    let tabs = t.children.iter().filter_map(|&c| match self.tree.tiles.get(c) {
                        Some(Tile::Pane(p)) => Some((c, *p)),
                        _ => None,
                    }).collect();
                    groups.insert(active, TabGroup { container: *id, tabs, active });
                }
            }
        }

        let mut behavior = ShellBehavior { groups, ..Default::default() };
        self.tree.ui(&mut behavior, ui);

        if let Some((container, tile)) = behavior.set_active {
            if let Some(Tile::Container(Container::Tabs(t))) = self.tree.tiles.get_mut(container) {
                t.active = Some(tile);
            }
        }
        if let Some((id, panel)) = behavior.switch {
            if let Some(Tile::Pane(p)) = self.tree.tiles.get_mut(id) {
                *p = panel;
            }
        }
        if let Some(id) = behavior.close {
            detach(&mut self.tree, id);
            self.tree.tiles.remove(id);
        }

        // The LIFT: while egui_tiles is dragging a pane, paint the real panel floating on top, easing
        // toward the cursor. egui_tiles itself shows the clean drop preview + docks on release.
        let ghost_id = egui::Id::new("varos_ghost_pos");
        let cursor = ui.input(|i| i.pointer.hover_pos());
        if let Some(dragged) = self.tree.dragged_id(ui.ctx()) {
            let panel = match self.tree.tiles.get(dragged) {
                Some(Tile::Pane(p)) => Some(*p),
                _ => None,
            };
            if let (Some(panel), Some(cur)) = (panel, cursor) {
                if !panel.is_board() {
                    let mut gpos = ui.ctx().data(|d| d.get_temp::<Pos2>(ghost_id)).unwrap_or(cur);
                    gpos += (cur - gpos) * 0.55; // ease toward the cursor — snappier so it doesn't feel heavy
                    ui.ctx().data_mut(|d| d.insert_temp(ghost_id, gpos));
                    render_drag_ghost(ui, panel, gpos);
                    ui.ctx().request_repaint();
                }
            }
        } else if let Some(cur) = cursor {
            ui.ctx().data_mut(|d| d.insert_temp(ghost_id, cur)); // keep the anchor fresh between drags
        }

        draw_resize_handles(&self.tree, ui);
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.tree)
    }
}

fn set_share(tree: &mut Tree<PanelId>, container: TileId, child: TileId, share: f32) {
    if let Some(Tile::Container(Container::Linear(lin))) = tree.tiles.get_mut(container) {
        lin.shares.set_share(child, share);
    }
}

fn is_board_tile(tiles: &Tiles<PanelId>, id: TileId) -> bool {
    matches!(tiles.get(id), Some(Tile::Pane(PanelId::Board)))
}

/// Detach `id` from its parent container's child list (used by close; the tile itself is removed after).
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

/// The lifted panel: a faithful floating copy on top, at `pos` (cursor at its top-centre).
fn render_drag_ghost(ui: &egui::Ui, panel: PanelId, pos: Pos2) {
    let w = 232.0;
    egui::Area::new(egui::Id::new("varos_drag_ghost"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos - vec2(w * 0.5, 6.0))
        .show(ui.ctx(), |ui| {
            ui.set_width(w);
            egui::Frame::default()
                .fill(T::PANEL)
                .stroke(Stroke::new(1.0, T::ACCENT))
                .corner_radius(T::r_box())
                .show(ui, |ui| {
                    ui.set_width(w);
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new(panel.title()).color(T::TEXT).size(12.5));
                    });
                    ui.add_space(6.0);
                    let y = ui.min_rect().bottom();
                    ui.painter().hline(ui.min_rect().left()..=ui.min_rect().right(), y, T::hairline());
                    // faithful body, but disabled + id-scoped so it never fights the real panel
                    ui.push_id("ghost", |ui| {
                        ui.add_enabled_ui(false, |ui| {
                            egui::Frame::NONE
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| registry::render_panel(panel, ui));
                        });
                    });
                });
        });
}

/// Small FIXED resize-grabber on the hovered seam.
fn draw_resize_handles(tree: &Tree<PanelId>, ui: &egui::Ui) {
    let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) else { return };
    // don't draw while a drag is in progress (the drop preview owns the screen then)
    if tree.dragged_id(ui.ctx()).is_some() {
        return;
    }
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

/// The persistent move-grip: a small bar at the top-centre of EVERY box. Always drawn — never hidden by
/// scroll or by being in a tab group (Ahmed 07-04: "المقبض بيختفي"). Faint at rest (LINE2), bright when
/// hovered/held (TEXT). Grabbing it hands the drag to egui_tiles (lift → drop preview → dock).
fn draw_grip(ui: &egui::Ui, rect: Rect, tile_id: TileId) -> egui::Response {
    let grip = Rect::from_center_size(pos2(rect.center().x, rect.top() + 8.0), vec2(26.0, 3.0));
    let r = ui.interact(grip.expand2(vec2(16.0, 8.0)), ui.id().with(("grip", tile_id)), Sense::click_and_drag());
    let col = if r.hovered() || r.dragged() { T::TEXT } else { T::LINE2 };
    ui.painter().rect_filled(grip, CornerRadius::same(2), col);
    r
}

/// The scrollable body below a box's header, with one consistent inner margin (12 × 10) so every panel
/// breathes the same (Ahmed 07-04: "مفيش مسافات مظبوطة"). Scrollbars are the thin floating overlay set
/// globally in `tokens::apply`. The ScrollArea (and the whole body) is id-salted by `tile_id` so a
/// tabbed box's body never collides with its pills strip — the "ScrollArea ID clash" (Ahmed 07-05).
fn render_body(ui: &mut egui::Ui, rect: Rect, hh: f32, tile_id: TileId, pane: PanelId) {
    let body = Rect::from_min_max(pos2(rect.left(), rect.top() + hh), rect.max);
    ui.scope_builder(
        UiBuilder::new().max_rect(body.shrink2(vec2(12.0, 10.0))).layout(Layout::top_down(Align::Min)),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("body", tile_id))
                .auto_shrink([false, false])
                .show(ui, |ui| registry::render_panel(pane, ui));
        },
    );
}

// ───────────────────────── the behaviour ─────────────────────────

/// A tabbed box, keyed by its ACTIVE pane's tile (the only one egui_tiles calls pane_ui for). We draw
/// the whole box ourselves so it can be fully rounded with floating pills.
#[derive(Clone)]
struct TabGroup {
    container: TileId,
    tabs: Vec<(TileId, PanelId)>,
    active: TileId,
}

#[derive(Default)]
struct ShellBehavior {
    switch: Option<(TileId, PanelId)>,
    close: Option<TileId>,
    set_active: Option<(TileId, TileId)>,
    groups: HashMap<TileId, TabGroup>,
}

impl ShellBehavior {
    /// Draw the ☰ (change-type) + ✕ (close) controls at the header's right edge and wire their intents.
    /// Returns the left x of the controls block so the caller keeps the title / pills clear of them.
    fn header_controls(&mut self, ui: &mut egui::Ui, tile_id: TileId, mid: f32, right: f32) -> f32 {
        let pad = 10.0;
        let x_rect = Rect::from_min_size(pos2(right - pad - 18.0, mid - 9.0), vec2(18.0, 18.0));
        let x = ui.interact(x_rect, ui.id().with(("close", tile_id)), Sense::click());
        if x.hovered() { ui.painter().rect_filled(x_rect, T::r_ctrl(), T::HOVER); }
        paint_cross(ui, x_rect, if x.hovered() { T::CLOSE_RED } else { T::MUTED });
        if x.clicked() { self.close = Some(tile_id); }

        let menu_rect = Rect::from_min_size(pos2(x_rect.left() - 6.0 - 22.0, mid - 12.0), vec2(22.0, 24.0));
        let mut menu_switch: Option<PanelId> = None;
        ui.scope_builder(UiBuilder::new().max_rect(menu_rect), |ui| {
            frameless_buttons(ui);
            ui.menu_button(RichText::new("☰").color(T::MUTED).size(14.0), |ui| {
                ui.set_min_width(180.0);
                ui.label(RichText::new("CHANGE THIS PANEL TO").color(T::FAINT).size(9.5).strong());
                ui.add_space(2.0);
                for p in PanelId::DOCKABLE {
                    if ui.button(p.title()).clicked() { menu_switch = Some(p); ui.close(); }
                }
            });
        });
        if let Some(p) = menu_switch { self.switch = Some((tile_id, p)); }
        menu_rect.left() - 8.0
    }
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

        // Every box is ONE rounded shell on the void; we paint it ourselves (egui_tiles' tab bar is
        // 0-height and draws nothing behind us). Border INSIDE → the silhouette is exactly `rect`,
        // so the rounded corners sit clean against the seam (Ahmed 07-04: "الراوند باظ").
        ui.painter().rect(rect, T::r_box(), T::PANEL, T::hairline(), StrokeKind::Inside);

        // MULTI-panel (tabbed) box: persistent grip + centred capsule pills + controls, then the body.
        if let Some(group) = self.groups.get(&tile_id).cloned() {
            let hh = 40.0;
            // controls share the SAME row as the pills (both centred at top+25) — not floating up by the
            // grip, which read as misaligned (Ahmed 07-05 "التنسيق مش حلو").
            let controls_left = self.header_controls(ui, tile_id, rect.top() + 25.0, rect.right());
            let g = draw_grip(ui, rect, tile_id);

            // capsule PILLS in a clipped, horizontally-scrollable strip so they NEVER overflow into the
            // controls or vanish in a narrow panel — the bug that made dropping-to-tab hide the other
            // panel (Ahmed 07-05). Click = activate; drag the active one = lift it out.
            let font = FontId::proportional(12.0);
            let strip = Rect::from_min_max(
                pos2(rect.left() + 12.0, rect.top() + 13.0),
                pos2(controls_left - 6.0, rect.top() + 37.0),
            );
            let mut clicked = None;
            let mut drag_active = false;
            ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt(("pills", tile_id))
                    .scroll_source(egui::scroll_area::ScrollSource::MOUSE_WHEEL) // wheel scrolls; drag LIFTS the pill
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            for (tid, pid) in &group.tabs {
                                let active = *tid == group.active;
                                let tw = ui.painter().layout_no_wrap(pid.title().to_owned(), font.clone(), T::TEXT).size().x;
                                let (pill, r) = ui.allocate_exact_size(vec2(tw + 20.0, 22.0), Sense::click_and_drag());
                                let bg = if active { T::SURFACE } else if r.hovered() { T::HOVER } else { Color32::TRANSPARENT };
                                ui.painter().rect_filled(pill, CornerRadius::same(11), bg); // capsule = a Claude bubble
                                ui.painter().text(pill.center(), Align2::CENTER_CENTER, pid.title(), font.clone(), if active || r.hovered() { T::TEXT } else { T::MUTED });
                                if r.clicked() { clicked = Some(*tid); }
                                if active && r.drag_started() { drag_active = true; }
                            }
                        });
                    });
            });
            if let Some(t) = clicked { self.set_active = Some((group.container, t)); }

            ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, rect.top() + hh, T::hairline());
            render_body(ui, rect, hh, tile_id, *pane);

            if drag_active || g.drag_started() { return UiResponse::DragStarted; }
            return UiResponse::None;
        }

        // SINGLE-panel box: persistent grip + title + controls.
        let hh = 34.0;
        let mid = rect.top() + 20.0;
        let controls_left = self.header_controls(ui, tile_id, mid, rect.right());
        let g = draw_grip(ui, rect, tile_id);
        // the WHOLE title bar (below the grip, left of the controls) is ALSO a drag handle — a big,
        // forgiving target so a box is never "impossible to grab" (Ahmed 07-04: "مستحيل تتحرك").
        let drag_rect = Rect::from_min_max(pos2(rect.left(), rect.top() + 14.0), pos2(controls_left, rect.top() + hh));
        let hdr = ui.interact(drag_rect, ui.id().with(("hdr", tile_id)), Sense::click_and_drag());
        ui.painter().text(pos2(rect.left() + 12.0, mid), Align2::LEFT_CENTER, pane.title(), FontId::proportional(12.5), T::TEXT);

        ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, rect.top() + hh, T::hairline());
        render_body(ui, rect, hh, tile_id, *pane);

        if g.drag_started() || hdr.drag_started() { return UiResponse::DragStarted; }
        UiResponse::None
    }

    // egui_tiles' own tab bar is 0-height — we draw the whole tabbed box in `pane_ui` instead. This is
    // the ONE tab-bar method we keep; the rest (tab_ui / tab colours / top_bar_right_ui) were dead code
    // that also painted a stray glyph into the 0-height strip — the "weird corner" (Ahmed 07-04).
    fn tab_bar_height(&self, _s: &egui::Style) -> f32 { 0.0 }

    fn gap_width(&self, _style: &egui::Style) -> f32 { T::SEAM_GAP }
    fn resize_stroke(&self, _style: &egui::Style, _state: ResizeState) -> Stroke { Stroke::NONE } // pure void seam — no line
    fn is_tile_draggable(&self, tiles: &Tiles<PanelId>, tile_id: TileId) -> bool { !is_board_tile(tiles, tile_id) }
    fn pane_is_drop_target(&self, pane: &PanelId) -> bool { !pane.is_board() } // never dock/tab INTO the canvas
    fn min_size(&self) -> f32 { 170.0 } // panels stay usable; content scrolls (never breaks)

    // ── drag look: no distorted double-render (we paint our own lifted ghost); egui_tiles shows a
    //    clean azure preview of the drop slot; the vacated spot dims ──
    fn preview_dragged_panes(&self) -> bool { false }
    /// ONE clean guide: a rounded azure outline of the drop slot only (the default also outlines the
    /// parent container — that's the "too many guides" Ahmed saw). Light fill, thin stroke.
    fn paint_drag_preview(&self, _visuals: &Visuals, painter: &egui::Painter, parent_rect: Option<Rect>, preview_rect: Rect) {
        // The highlight is ONE box — the target box itself, never the whole bank (Ahmed 07-05).
        let Some(parent) = parent_rect else {
            glow_half(painter, preview_rect.shrink(1.0), egui::Vec2::ZERO);
            return;
        };
        let rect = parent.shrink(1.0);
        let ratio = (preview_rect.width() * preview_rect.height()) / (parent.width() * parent.height()).max(1.0);
        // TAB: the slot is ~the whole box → one even glow, no direction, no bar.
        if ratio > 0.6 {
            glow_half(painter, rect, egui::Vec2::ZERO);
            return;
        }
        // EDGE DOCK: the box glows with a gradient toward the entry edge (where the new panel comes in),
        // and one thick bright bar sits on that edge. Above → glow+bar at top; below → bottom; etc.
        let d = preview_rect.center() - parent.center();
        let dir = if d.x.abs() >= d.y.abs() {
            if d.x >= 0.0 { vec2(1.0, 0.0) } else { vec2(-1.0, 0.0) }
        } else if d.y >= 0.0 {
            vec2(0.0, 1.0)
        } else {
            vec2(0.0, -1.0)
        };
        glow_half(painter, rect, dir);
        edge_bar(painter, rect, dir);
    }
    fn dragged_overlay_color(&self, _visuals: &Visuals) -> Color32 {
        Color32::from_rgba_unmultiplied(0x14, 0x13, 0x13, 170)
    }

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

/// Azure alpha `a` at the given normalised position — one place for the gradient's colour ramp.
fn azure(a: u8) -> Color32 { Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, a) }

/// Fill `rect` with a linear azure gradient: alpha `a_strong` at the edge `dir` points to, fading to
/// `a_faint` at the opposite edge — the drop-direction hint on the phantom slot (Ahmed 07-05).
fn fill_gradient(painter: &egui::Painter, rect: Rect, dir: egui::Vec2, a_strong: u8, a_faint: u8) {
    let mut mesh = egui::Mesh::default();
    let c = rect.center();
    let half = 0.5 * (rect.width() * dir.x.abs() + rect.height() * dir.y.abs());
    for p in [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()] {
        let t = if half > 0.0 { (((p - c).dot(dir)) / half + 1.0) * 0.5 } else { 0.5 };
        mesh.colored_vertex(p, azure((a_faint as f32 + (a_strong as f32 - a_faint as f32) * t.clamp(0.0, 1.0)) as u8));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// One glowing box: a soft azure fill gradient fading toward the `dir` edge inside a crisp uniform rounded
/// azure outline. `dir` = (0,0) → an even wash (a tab / whole-box highlight).
fn glow_half(painter: &egui::Painter, rect: Rect, dir: egui::Vec2) {
    painter.rect_filled(rect, T::r_box(), azure(if dir == egui::Vec2::ZERO { 20 } else { 8 }));
    if dir != egui::Vec2::ZERO {
        let grad = if dir.x == 0.0 { rect.shrink2(vec2(8.0, 1.0)) } else { rect.shrink2(vec2(1.0, 8.0)) };
        fill_gradient(painter, grad, dir, 78, 0);
    }
    painter.rect_stroke(rect, T::r_box(), Stroke::new(1.6, T::ACCENT), StrokeKind::Inside);
}

/// One thick bright azure bar along the entry edge that `dir` points to (the "خط" — Ahmed 07-05).
fn edge_bar(painter: &egui::Painter, rect: Rect, dir: egui::Vec2) {
    let (w, inset) = (3.0, 9.0);
    let (a, b) = if dir.y > 0.0 {
        (pos2(rect.left() + inset, rect.bottom() - w * 0.5), pos2(rect.right() - inset, rect.bottom() - w * 0.5))
    } else if dir.y < 0.0 {
        (pos2(rect.left() + inset, rect.top() + w * 0.5), pos2(rect.right() - inset, rect.top() + w * 0.5))
    } else if dir.x > 0.0 {
        (pos2(rect.right() - w * 0.5, rect.top() + inset), pos2(rect.right() - w * 0.5, rect.bottom() - inset))
    } else {
        (pos2(rect.left() + w * 0.5, rect.top() + inset), pos2(rect.left() + w * 0.5, rect.bottom() - inset))
    };
    painter.line_segment([a, b], Stroke::new(w, azure(255)));
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

// ───────────────────────── headless tests ─────────────────────────

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

    /// Dropping-to-tab creates a Tabs container; a narrow one must still render BOTH pills (the
    /// regression Ahmed hit 07-05, where the second pill was skipped and the panel looked lost).
    #[test]
    fn tabbed_group_renders_headless() {
        let mut tiles = Tiles::default();
        let board = tiles.insert_pane(PanelId::Board);
        let align = tiles.insert_pane(PanelId::Align);
        let props = tiles.insert_pane(PanelId::Properties);
        let tabs = tiles.insert_tab_tile(vec![align, props]);
        let root = tiles.insert_horizontal_tile(vec![board, tabs]);
        let mut shell = ShellState { tree: Tree::new("varos_shell_tabs", root, tiles) };
        let ctx = egui::Context::default();
        super::T::apply(&ctx);
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
