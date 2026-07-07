//! The box tree — the SINGLE place `egui_tiles` is used in the whole app (ruling 9).
//!
//! Drag model (Ahmed 07-04, "the Claude way"): grab a box's move-pill and it LIFTS off — the real panel
//! floats on TOP of everything, easing toward the cursor (light, smooth). egui_tiles owns the docking:
//! it shows a clean azure preview of where it'll land and commits on release. We just paint the lifted
//! ghost + style the look. No reflow-among-boxes.
use super::registry::{self, PanelId};
use super::tokens as T;
use egui::{
    pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, UiBuilder, Visuals,
};
use egui_tiles::{
    Behavior, Container, DropPreview, DropSide, LinearDir, ResizeState, SimplificationOptions, Tile, TileId, Tiles,
    Tree, UiResponse,
};
use std::collections::HashMap;

/// The host's panel renderer: paints the REAL body of `PanelId` into the pane's `Ui`. Returns `true`
/// if it handled the panel; `false` (without drawing) falls back to the registry's dummy body — so
/// the sandbox keeps working unhosted and the real app only wires the panels it has migrated.
pub type HostFn<'h> = dyn FnMut(PanelId, &mut egui::Ui) -> bool + 'h;

pub struct ShellState {
    tree: Tree<PanelId>,
}

impl ShellState {
    pub fn standard() -> Self {
        // The LAW tree (BOX_SYSTEM_PLAN §4.3): board ≈ .80 · right column = two TABBED boxes —
        // [Align|Pathfinder] over [Properties|Layers] (the growing box).
        let mut tiles = Tiles::default();
        let board = tiles.insert_pane(PanelId::Board);
        let align = tiles.insert_pane(PanelId::Align);
        let path = tiles.insert_pane(PanelId::Pathfinder);
        let upper = tiles.insert_tab_tile(vec![align, path]);
        let props = tiles.insert_pane(PanelId::Properties);
        let layers = tiles.insert_pane(PanelId::Layers);
        let lower = tiles.insert_tab_tile(vec![props, layers]);
        let right = tiles.insert_vertical_tile(vec![upper, lower]);
        let root = tiles.insert_horizontal_tile(vec![board, right]);
        let mut tree = Tree::new("varos_shell", root, tiles);
        set_share(&mut tree, root, board, 0.80);
        set_share(&mut tree, root, right, 0.20);
        set_share(&mut tree, right, upper, 0.24);
        set_share(&mut tree, right, lower, 0.76);
        Self { tree }
    }

    /// Unhosted (sandbox) entry: every panel body comes from the registry dummies.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.ui_hosted(ui, &mut |_, _| false);
    }

    pub fn ui_hosted(&mut self, ui: &mut egui::Ui, host: &mut HostFn<'_>) {
        // Merge any nested Tabs-in-Tabs a prior drop may have created, so every box reads as ONE flat tab
        // strip (dropping a multi-tab box onto a tab must give siblings, not a hidden nested box).
        flatten_nested_tabs(&mut self.tree);

        // For each Tabs container, map its ACTIVE pane's tile → the group; we render the whole tabbed
        // box ourselves in pane_ui (fully rounded, floating pills), with egui_tiles' tab bar at 0 height.
        let mut groups: HashMap<TileId, TabGroup> = HashMap::new();
        for (id, tile) in self.tree.tiles.iter() {
            if let Tile::Container(Container::Tabs(t)) = tile {
                if let Some(active) = t.active.or_else(|| t.children.first().copied()) {
                    let tabs = t
                        .children
                        .iter()
                        .filter_map(|&c| match self.tree.tiles.get(c) {
                            Some(Tile::Pane(p)) => Some((c, *p)),
                            _ => None,
                        })
                        .collect();
                    groups.insert(active, TabGroup { container: *id, tabs, active });
                }
            }
        }

        let mut behavior =
            ShellBehavior { switch: None, close: None, set_active: None, groups, tree_id: Some(self.tree.id()), host };
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
                // dragging a whole tabbed box (grip) → ghost the active tab's panel as its stand-in
                Some(Tile::Container(Container::Tabs(t))) => {
                    t.active.or_else(|| t.children.first().copied()).and_then(|c| match self.tree.tiles.get(c) {
                        Some(Tile::Pane(p)) => Some(*p),
                        _ => None,
                    })
                }
                _ => None,
            };
            if let (Some(panel), Some(cur)) = (panel, cursor) {
                if !panel.is_board() {
                    let mut gpos = ui.ctx().data(|d| d.get_temp::<Pos2>(ghost_id)).unwrap_or(cur);
                    gpos += (cur - gpos) * 0.55; // ease toward the cursor — snappier so it doesn't feel heavy
                    ui.ctx().data_mut(|d| d.insert_temp(ghost_id, gpos));
                    render_drag_ghost(ui, panel, gpos, &mut *behavior.host);
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

    /// Is a pane of this panel anywhere in the tree?
    pub fn is_open(&self, panel: PanelId) -> bool {
        find_pane(&self.tree, panel).is_some()
    }

    /// The Window-menu action (Ahmed 07-07): CLOSED → open in an AUTOMATIC spot (a tab beside its
    /// family, else beside Properties/Layers, else a fresh box on the root split). OPEN but hidden
    /// behind a sibling tab → bring it forward. OPEN and visible → close it (a toggle).
    pub fn toggle_panel(&mut self, panel: PanelId) {
        if panel.is_board() {
            return;
        }
        if let Some(id) = find_pane(&self.tree, panel) {
            let parent_tabs = self.tree.tiles.parent_of(id).and_then(|p| match self.tree.tiles.get(p) {
                Some(Tile::Container(Container::Tabs(t))) => Some((p, t.active)),
                _ => None,
            });
            match parent_tabs {
                Some((pid, active)) if active != Some(id) => {
                    if let Some(Tile::Container(Container::Tabs(t))) = self.tree.tiles.get_mut(pid) {
                        t.active = Some(id); // it was buried behind a sibling — surface it
                    }
                }
                _ => {
                    detach(&mut self.tree, id);
                    self.tree.tiles.remove(id);
                }
            }
            return;
        }
        // open: prefer a family sibling's box, then the Properties/Layers box, else a fresh box
        let anchor = family(panel)
            .iter()
            .chain([PanelId::Properties, PanelId::Layers].iter())
            .filter(|p| **p != panel)
            .find_map(|p| find_pane(&self.tree, *p));
        let new = self.tree.tiles.insert_pane(panel);
        match anchor {
            Some(a) => add_tab_beside(&mut self.tree, a, new),
            None => {
                let root = self.tree.root();
                if let Some(Tile::Container(Container::Linear(lin))) = root.and_then(|r| self.tree.tiles.get_mut(r)) {
                    lin.children.push(new);
                    lin.shares.set_share(new, 0.2);
                }
            }
        }
    }
}

/// Panels that belong together — the Window menu drops a newcomer beside its family first.
fn family(panel: PanelId) -> &'static [PanelId] {
    match panel {
        PanelId::Align | PanelId::Pathfinder => &[PanelId::Align, PanelId::Pathfinder],
        PanelId::Properties | PanelId::Layers => &[PanelId::Properties, PanelId::Layers],
        PanelId::Swatches => &[PanelId::Properties],
        PanelId::History | PanelId::Assets => &[PanelId::Layers],
        _ => &[],
    }
}

fn find_pane(tree: &Tree<PanelId>, panel: PanelId) -> Option<TileId> {
    tree.tiles.iter().find_map(|(id, t)| match t {
        Tile::Pane(p) if *p == panel => Some(*id),
        _ => None,
    })
}

/// Put `new` in as a TAB beside `anchor`: into the anchor's existing Tabs parent, else wrap the
/// anchor's slot in a fresh Tabs container holding [anchor, new]. The newcomer becomes active.
fn add_tab_beside(tree: &mut Tree<PanelId>, anchor: TileId, new: TileId) {
    let parent = tree.tiles.parent_of(anchor);
    if let Some(pid) = parent {
        if let Some(Tile::Container(Container::Tabs(t))) = tree.tiles.get_mut(pid) {
            let at = t.children.iter().position(|&c| c == anchor).map(|i| i + 1).unwrap_or(t.children.len());
            t.children.insert(at, new);
            t.active = Some(new);
            return;
        }
    }
    // wrap: a new Tabs tile takes the anchor's slot in its Linear parent (share carried over)
    let tabs_id = tree.tiles.insert_tab_tile(vec![anchor, new]);
    if let Some(Tile::Container(Container::Tabs(t))) = tree.tiles.get_mut(tabs_id) {
        t.active = Some(new);
    }
    if let Some(pid) = parent {
        if let Some(Tile::Container(Container::Linear(lin))) = tree.tiles.get_mut(pid) {
            if let Some(slot) = lin.children.iter().position(|&c| c == anchor) {
                lin.children[slot] = tabs_id;
                lin.shares.replace_with(anchor, tabs_id);
            }
        }
    }
}

fn set_share(tree: &mut Tree<PanelId>, container: TileId, child: TileId, share: f32) {
    if let Some(Tile::Container(Container::Linear(lin))) = tree.tiles.get_mut(container) {
        lin.shares.set_share(child, share);
    }
}

/// Flatten Tabs-in-Tabs into ONE tab strip: dropping a multi-tab box onto a tab must give all panes as
/// siblings in the same box, not a hidden nested container (Ahmed 07-06). egui_tiles has no "join nested
/// tabs" simplification, so we splice the inner tabs' children into the outer each frame before rendering.
fn flatten_nested_tabs(tree: &mut Tree<PanelId>) {
    loop {
        // find an outer Tabs whose child at `index` is itself a Tabs container
        let mut hit: Option<(TileId, usize, TileId)> = None;
        for (id, tile) in tree.tiles.iter() {
            if let Tile::Container(Container::Tabs(t)) = tile {
                for (i, &child) in t.children.iter().enumerate() {
                    if matches!(tree.tiles.get(child), Some(Tile::Container(Container::Tabs(_)))) {
                        hit = Some((*id, i, child));
                        break;
                    }
                }
            }
            if hit.is_some() {
                break;
            }
        }
        let Some((outer_id, index, inner_id)) = hit else { break };
        let (kids, inner_active) = match tree.tiles.get(inner_id) {
            Some(Tile::Container(Container::Tabs(inner))) => (inner.children.clone(), inner.active),
            _ => break,
        };
        if let Some(Tile::Container(Container::Tabs(outer))) = tree.tiles.get_mut(outer_id) {
            let adopt = outer.active == Some(inner_id); // if the dropped box was the active tab, keep its active
            outer.children.splice(index..=index, kids.iter().copied());
            if adopt {
                outer.active = inner_active.or_else(|| kids.first().copied());
            }
        }
        tree.tiles.remove(inner_id);
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
fn render_drag_ghost(ui: &egui::Ui, panel: PanelId, pos: Pos2, host: &mut HostFn<'_>) {
    let w = 232.0;
    egui::Area::new(egui::Id::new("varos_drag_ghost"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos - vec2(w * 0.5, 6.0))
        .show(ui.ctx(), |ui| {
            ui.set_width(w);
            egui::Frame::default().fill(T::PANEL).stroke(Stroke::new(1.0, T::ACCENT)).corner_radius(T::r_box()).show(
                ui,
                |ui| {
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
                            egui::Frame::NONE.inner_margin(Margin::same(10)).show(ui, |ui| {
                                if !host(panel, ui) {
                                    registry::render_panel(panel, ui);
                                }
                            });
                        });
                    });
                },
            );
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
                        ui.painter().rect_filled(
                            Rect::from_center_size(pos2(x, ra.center().y), vec2(3.0, 22.0)),
                            CornerRadius::same(2),
                            T::FAINT,
                        );
                    }
                }
                LinearDir::Vertical => {
                    let y = (ra.bottom() + rb.top()) * 0.5;
                    if (ptr.y - y).abs() <= 6.0 && ptr.x >= ra.left() && ptr.x <= ra.right() {
                        ui.painter().rect_filled(
                            Rect::from_center_size(pos2(ra.center().x, y), vec2(22.0, 3.0)),
                            CornerRadius::same(2),
                            T::FAINT,
                        );
                    }
                }
            }
        }
    }
}

/// The move-grip: a small bar at the top-centre of a box. HIDDEN at rest, revealed only when the cursor
/// is over the box's top band (or while it's being dragged) — Ahmed 07-05: "مخفي لحد ما أعمل hover عنده".
/// The grab zone stays live always, so grabbing still hands the drag to egui_tiles (lift → preview → dock).
fn draw_grip(ui: &egui::Ui, rect: Rect, tile_id: TileId) -> egui::Response {
    let grip = Rect::from_center_size(pos2(rect.center().x, rect.top() + 8.0), vec2(26.0, 3.0));
    let r = ui.interact(grip.expand2(vec2(16.0, 8.0)), ui.id().with(("grip", tile_id)), Sense::click_and_drag());
    // reveal only near the top of the box (where the grip lives) — everywhere else it's invisible chrome.
    let near_top = ui.rect_contains_pointer(Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + 24.0)));
    if near_top || r.dragged() {
        let col = if r.hovered() || r.dragged() { T::TEXT } else { T::LINE2 };
        ui.painter().rect_filled(grip, CornerRadius::same(2), col);
    }
    r
}

/// A round, tab-shaped chevron button for scrolling the pill strip when tabs overflow (Ahmed 07-06).
/// `dir`: -1.0 draws ‹ (scroll left), +1.0 draws › (scroll right). Same capsule look as a tab. Returns
/// true on click. Only the caller decides WHEN to show it (left only if scrolled off left, etc.).
fn scroll_arrow(ui: &egui::Ui, center: Pos2, dir: f32, tile_id: TileId) -> bool {
    let rad = 11.0;
    let hit = Rect::from_center_size(center, vec2(2.0 * rad, 2.0 * rad));
    let r = ui.interact(hit, ui.id().with(("tabarrow", tile_id, dir as i32)), Sense::click());
    let p = ui.painter();
    p.circle_filled(center, rad, if r.hovered() { T::HOVER } else { T::SURFACE });
    let col = if r.hovered() { T::TEXT } else { T::MUTED };
    // a chevron: two short strokes meeting at the tip (tip points the way it scrolls)
    let tip = pos2(center.x + dir * 2.5, center.y);
    p.line_segment([pos2(center.x - dir * 2.5, center.y - 4.0), tip], Stroke::new(1.7, col));
    p.line_segment([tip, pos2(center.x - dir * 2.5, center.y + 4.0)], Stroke::new(1.7, col));
    r.clicked()
}

/// The scrollable body below a box's header, with one consistent inner margin (12 × 10) so every panel
/// breathes the same (Ahmed 07-04: "مفيش مسافات مظبوطة"). Scrollbars are the thin floating overlay set
/// globally in `tokens::apply`. The ScrollArea (and the whole body) is id-salted by `tile_id` so a
/// tabbed box's body never collides with its pills strip — the "ScrollArea ID clash" (Ahmed 07-05).
fn render_body(ui: &mut egui::Ui, rect: Rect, hh: f32, tile_id: TileId, pane: PanelId, host: &mut HostFn<'_>) {
    let body = Rect::from_min_max(pos2(rect.left(), rect.top() + hh), rect.max);
    // a HOSTED panel owns its margins/footers, but the BOX owns overflow: unless the panel manages
    // its own list scroll (Layers), the body SCROLLS instead of slicing the bottom row mid-glyph
    // (Ahmed 07-07: "الحرف اللي تحت مقصوص"). The dummy fallback keeps its shared margin + scroll.
    let mut handled = false;
    ui.scope_builder(UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)), |ui| {
        if pane.self_scrolling() {
            handled = host(pane, ui);
        } else {
            egui::ScrollArea::vertical().id_salt(("hostbody", tile_id)).auto_shrink([false, false]).show(ui, |ui| {
                handled = host(pane, ui);
                if handled {
                    ui.add_space(10.0); // breathing room — the last row never kisses the border
                }
            });
        }
    });
    if handled {
        return;
    }
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

struct ShellBehavior<'h> {
    switch: Option<(TileId, PanelId)>,
    close: Option<TileId>,
    set_active: Option<(TileId, TileId)>,
    groups: HashMap<TileId, TabGroup>,
    /// The tree's egui id, set each frame — lets a pill/grip drag start the drag on the RIGHT tile.
    tree_id: Option<egui::Id>,
    /// The real app's panel renderer (the sandbox passes a no-op that always falls back).
    host: &'h mut HostFn<'h>,
}

impl ShellBehavior<'_> {
    /// Draw the ☰ (change-type) + ✕ (close) controls at the header's right edge and wire their intents.
    /// Returns the left x of the controls block so the caller keeps the title / pills clear of them.
    fn header_controls(&mut self, ui: &mut egui::Ui, tile_id: TileId, mid: f32, right: f32) -> f32 {
        let pad = 10.0;
        let x_rect = Rect::from_min_size(pos2(right - pad - 18.0, mid - 9.0), vec2(18.0, 18.0));
        let x = ui.interact(x_rect, ui.id().with(("close", tile_id)), Sense::click());
        if x.hovered() {
            ui.painter().rect_filled(x_rect, T::r_ctrl(), T::HOVER);
        }
        paint_cross(ui, x_rect, if x.hovered() { T::CLOSE_RED } else { T::MUTED });
        if x.clicked() {
            self.close = Some(tile_id);
        }

        let menu_rect = Rect::from_min_size(pos2(x_rect.left() - 6.0 - 22.0, mid - 12.0), vec2(22.0, 24.0));
        let mut menu_switch: Option<PanelId> = None;
        ui.scope_builder(UiBuilder::new().max_rect(menu_rect), |ui| {
            frameless_buttons(ui);
            ui.menu_button(RichText::new("☰").color(T::MUTED).size(14.0), |ui| {
                ui.set_min_width(180.0);
                ui.label(RichText::new("CHANGE THIS PANEL TO").color(T::FAINT).size(9.5).strong());
                ui.add_space(2.0);
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
        menu_rect.left() - 8.0
    }
}

impl Behavior<PanelId> for ShellBehavior<'_> {
    fn tab_title_for_pane(&mut self, pane: &PanelId) -> egui::WidgetText {
        pane.title().into()
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, pane: &mut PanelId) -> UiResponse {
        let rect = ui.max_rect();
        if pane.is_board() {
            // hosted board = the REAL canvas hole (rulers + hands, wgpu scene showing through);
            // unhosted (sandbox) = the dummy painted board.
            if !(self.host)(PanelId::Board, ui) {
                draw_board(ui, rect);
            }
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
            let mut pill_drag: Option<TileId> = None;
            // one-shot forced offset set by an arrow click last frame; the wheel scrolls the rest of the time
            let scroll_key = egui::Id::new(("pills_scroll", tile_id));
            let forced = ui.ctx().data(|d| d.get_temp::<f32>(scroll_key));
            if forced.is_some() {
                ui.ctx().data_mut(|d| d.remove::<f32>(scroll_key));
            }
            let (mut offset_x, mut viewport_w, mut content_w) = (0.0_f32, 0.0_f32, 0.0_f32);
            ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
                let mut sa = egui::ScrollArea::horizontal()
                    .id_salt(("pills", tile_id))
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden) // chevrons replace the ugly slider (Ahmed)
                    .scroll_source(egui::scroll_area::ScrollSource::MOUSE_WHEEL); // wheel scrolls; drag LIFTS the pill
                if let Some(x) = forced {
                    sa = sa.horizontal_scroll_offset(x);
                }
                let out = sa.show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        for (tid, pid) in &group.tabs {
                            let active = *tid == group.active;
                            let tw =
                                ui.painter().layout_no_wrap(pid.title().to_owned(), font.clone(), T::TEXT).size().x;
                            let (pill, r) = ui.allocate_exact_size(vec2(tw + 20.0, 22.0), Sense::click_and_drag());
                            let bg = if active {
                                T::SURFACE
                            } else if r.hovered() {
                                T::HOVER
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(pill, CornerRadius::same(11), bg); // capsule = a Claude bubble
                            ui.painter().text(
                                pill.center(),
                                Align2::CENTER_CENTER,
                                pid.title(),
                                font.clone(),
                                if active || r.hovered() { T::TEXT } else { T::MUTED },
                            );
                            if r.clicked() {
                                clicked = Some(*tid);
                            }
                            // ANY pill (active or inactive) can be lifted out as its own tab.
                            if r.drag_started() {
                                pill_drag = Some(*tid);
                            }
                        }
                    });
                });
                offset_x = out.state.offset.x;
                viewport_w = out.inner_rect.width();
                content_w = out.content_size.x;
            });
            // overflow chevrons: a round tab-shaped button at each end, shown ONLY when there's more that
            // way — ‹ if scrolled off the left, › if tabs run past the right (Ahmed 07-06).
            let step = (viewport_w * 0.7).max(40.0);
            if offset_x > 1.0 && scroll_arrow(ui, pos2(strip.left() + 10.0, strip.center().y), -1.0, tile_id) {
                ui.ctx().data_mut(|d| d.insert_temp(scroll_key, (offset_x - step).max(0.0)));
            }
            if offset_x + viewport_w < content_w - 1.0
                && scroll_arrow(ui, pos2(strip.right() - 10.0, strip.center().y), 1.0, tile_id)
            {
                ui.ctx().data_mut(|d| d.insert_temp(scroll_key, offset_x + step));
            }
            if let Some(t) = clicked {
                self.set_active = Some((group.container, t));
            }

            // Two grab targets (Ahmed 07-05): a PILL (active OR inactive) lifts THAT one tab; the GRIP
            // lifts the WHOLE box (its Tabs container — all tabs). We start the egui_tiles drag on the right
            // tile ourselves; returning DragStarted would only ever drag the rendered (active) pane.
            if let Some(tree_id) = self.tree_id {
                if let Some(tid) = pill_drag {
                    ui.set_dragged_id(tid.egui_id(tree_id));
                } else if g.drag_started() {
                    ui.set_dragged_id(group.container.egui_id(tree_id));
                }
            }

            ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, rect.top() + hh, T::hairline());
            render_body(ui, rect, hh, tile_id, *pane, &mut *self.host);

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
        ui.painter().text(
            pos2(rect.left() + 12.0, mid),
            Align2::LEFT_CENTER,
            pane.title(),
            FontId::proportional(12.5),
            T::TEXT,
        );

        ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, rect.top() + hh, T::hairline());
        render_body(ui, rect, hh, tile_id, *pane, &mut *self.host);

        if g.drag_started() || hdr.drag_started() {
            return UiResponse::DragStarted;
        }
        UiResponse::None
    }

    // egui_tiles' own tab bar is 0-height — we draw the whole tabbed box in `pane_ui` instead. This is
    // the ONE tab-bar method we keep; the rest (tab_ui / tab colours / top_bar_right_ui) were dead code
    // that also painted a stray glyph into the 0-height strip — the "weird corner" (Ahmed 07-04).
    fn tab_bar_height(&self, _s: &egui::Style) -> f32 {
        0.0
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        T::SEAM_GAP
    }
    fn resize_stroke(&self, _style: &egui::Style, _state: ResizeState) -> Stroke {
        Stroke::NONE
    } // pure void seam — no line
    fn is_tile_draggable(&self, tiles: &Tiles<PanelId>, tile_id: TileId) -> bool {
        !is_board_tile(tiles, tile_id)
    }
    fn pane_is_drop_target(&self, _pane: &PanelId) -> bool {
        true
    } // the board docks/tabs like any normal box (Ahmed 07-05)
    fn min_size(&self) -> f32 {
        224.0
    } // panels never crush below a usable width (Ahmed 07-07); content scrolls, never breaks

    // ── drag look: no distorted double-render (we paint our own lifted ghost); egui_tiles shows a
    //    clean azure preview of the drop slot; the vacated spot reads as empty void ──
    fn preview_dragged_panes(&self) -> bool {
        false
    }
    /// The drop highlight, exactly Ahmed's 07-05 model (three cases, one fixed 20% band):
    ///   • tab (middle)        → the box glows evenly, no direction, no bar.
    ///   • edge, no neighbour  → the box glows toward that edge + one bar on the edge (cases 3/4/5).
    ///   • edge WITH neighbour → BOTH boxes glow toward the shared seam + ONE bar in the seam (cases 1/2).
    fn paint_drag_preview(&self, _visuals: &Visuals, painter: &egui::Painter, p: DropPreview) {
        match p.side {
            None => glow(painter, p.target, None), // tab → even wash
            Some(side) => {
                glow(painter, p.target, Some(side)); // the box the cursor is over always glows
                match p.neighbor {
                    Some(nb) => {
                        // between two boxes: the neighbour glows toward the shared seam, one line in it
                        glow(painter, nb, Some(side.opposite()));
                        seam_bar(painter, p.target, nb, side);
                    }
                    None => edge_bar(painter, p.target, side), // outer edge: one bar on it
                }
            }
        }
    }
    fn dragged_overlay_color(&self, _visuals: &Visuals) -> Color32 {
        T::SEAM // OPAQUE void: the lifted panel's old spot reads as truly empty, not a see-through ghost (Ahmed 07-05)
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
fn azure(a: u8) -> Color32 {
    let c = T::ACCENT; // single source: the azure scalpel token, never a re-hardcoded hex
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

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

/// The outward unit vector for a dock side (points at the edge the gradient is strongest on).
fn side_dir(side: DropSide) -> egui::Vec2 {
    match side {
        DropSide::Left => vec2(-1.0, 0.0),
        DropSide::Right => vec2(1.0, 0.0),
        DropSide::Top => vec2(0.0, -1.0),
        DropSide::Bottom => vec2(0.0, 1.0),
    }
}

/// The strip of `rect` the gradient lives in — clamped along `dir` so a huge neighbour (the canvas) glows
/// as a band near the seam instead of flooding the whole box.
fn grad_band(rect: Rect, dir: egui::Vec2) -> Rect {
    const REACH: f32 = 150.0;
    if dir.x != 0.0 {
        let w = rect.width().min(REACH);
        if dir.x > 0.0 {
            Rect::from_min_max(pos2(rect.right() - w, rect.top()), rect.right_bottom())
        } else {
            Rect::from_min_max(rect.left_top(), pos2(rect.left() + w, rect.bottom()))
        }
    } else {
        let h = rect.height().min(REACH);
        if dir.y > 0.0 {
            Rect::from_min_max(pos2(rect.left(), rect.bottom() - h), rect.right_bottom())
        } else {
            Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + h))
        }
    }
}

/// One glowing box: a crisp uniform rounded azure outline, plus — for a directional dock — a soft azure
/// gradient strongest at the `side` edge and fading inward. `None` → an even wash (a tab highlight).
fn glow(painter: &egui::Painter, rect: Rect, side: Option<DropSide>) {
    let rect = rect.shrink(1.0);
    match side {
        None => {
            painter.rect_filled(rect, T::r_box(), azure(22));
        }
        Some(side) => {
            let dir = side_dir(side);
            painter.rect_filled(rect, T::r_box(), azure(8));
            let band = grad_band(rect, dir).shrink(1.5);
            fill_gradient(painter, band, dir, 90, 0);
        }
    }
    painter.rect_stroke(rect, T::r_box(), Stroke::new(1.6, T::ACCENT), StrokeKind::Inside);
}

/// One thick bright azure bar on `rect`'s `side` edge — the outer-dock "خط" (cases 3/4/5, Ahmed 07-05).
fn edge_bar(painter: &egui::Painter, rect: Rect, side: DropSide) {
    let rect = rect.shrink(1.0);
    let (w, inset) = (3.0, 10.0);
    let (a, b) = match side {
        DropSide::Bottom => {
            (pos2(rect.left() + inset, rect.bottom() - w * 0.5), pos2(rect.right() - inset, rect.bottom() - w * 0.5))
        }
        DropSide::Top => {
            (pos2(rect.left() + inset, rect.top() + w * 0.5), pos2(rect.right() - inset, rect.top() + w * 0.5))
        }
        DropSide::Right => {
            (pos2(rect.right() - w * 0.5, rect.top() + inset), pos2(rect.right() - w * 0.5, rect.bottom() - inset))
        }
        DropSide::Left => {
            (pos2(rect.left() + w * 0.5, rect.top() + inset), pos2(rect.left() + w * 0.5, rect.bottom() - inset))
        }
    };
    painter.line_segment([a, b], Stroke::new(w, azure(255)));
}

/// One thick bright azure bar in the MIDDLE of the gap between two boxes — the "خط في النص" for the
/// between-two case (cases 1/2, Ahmed 07-05). `side` is the dock edge of `a` (so the seam faces `b`).
fn seam_bar(painter: &egui::Painter, a: Rect, b: Rect, side: DropSide) {
    let (w, inset) = (3.0, 10.0);
    let (p1, p2) = match side {
        DropSide::Bottom | DropSide::Top => {
            let y = if side == DropSide::Bottom { (a.bottom() + b.top()) * 0.5 } else { (a.top() + b.bottom()) * 0.5 };
            let x0 = a.left().max(b.left()) + inset;
            let x1 = a.right().min(b.right()) - inset;
            (pos2(x0, y), pos2(x1, y))
        }
        DropSide::Right | DropSide::Left => {
            let x = if side == DropSide::Right { (a.right() + b.left()) * 0.5 } else { (a.left() + b.right()) * 0.5 };
            let y0 = a.top().max(b.top()) + inset;
            let y1 = a.bottom().min(b.bottom()) - inset;
            (pos2(x, y0), pos2(x, y1))
        }
    };
    painter.line_segment([p1, p2], Stroke::new(w, azure(255)));
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
            p.rect(
                ab,
                CornerRadius::ZERO,
                Color32::from_gray(245),
                Stroke::new(1.0, Color32::from_black_alpha(70)),
                StrokeKind::Middle,
            );
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
            if x + fw > bar.right() - 30.0 {
                break;
            }
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
                if ty > rail.bottom() - 40.0 {
                    break;
                }
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
            p.rect(
                Rect::from_min_size(sc + vec2(-3.0, -3.0), vec2(15.0, 15.0)),
                CornerRadius::same(2),
                T::PANEL,
                Stroke::new(1.5, T::NAVY),
                StrokeKind::Middle,
            );
            p.rect(
                Rect::from_min_size(sc + vec2(-11.0, -11.0), vec2(15.0, 15.0)),
                CornerRadius::same(2),
                T::AMBER,
                Stroke::new(1.5, T::LINE2),
                StrokeKind::Middle,
            );
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
