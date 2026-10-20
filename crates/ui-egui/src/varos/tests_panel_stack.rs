//! Icon panels open as independent cards ([`super::panel_stack`]), through real clicks on the
//! dock's icon column and on the cards.

use egui::{Pos2, Rect, pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;
use crate::varos::panel_stack;

const SCREEN: egui::Vec2 = vec2(1600.0, 1400.0);

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
    screen: egui::Vec2,
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

impl Harness {
    fn new() -> Self {
        Self::sized(SCREEN)
    }

    fn sized(screen: egui::Vec2) -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self { app, ctx, time: 0.0, screen };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.frame_out(events);
    }

    /// One frame; true when it asked to be drawn again at once.
    fn frame_out(&mut self, events: Vec<egui::Event>) -> bool {
        self.time += 0.05;
        let screen = Rect::from_min_size(egui::Pos2::ZERO, self.screen);
        let input = egui::RawInput { time: Some(self.time), events, screen_rect: Some(screen), ..Default::default() };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
        out.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.repaint_delay.is_zero())
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.settle();
    }

    /// Press at `from`, move to `to` in steps and let go there.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for k in 1..=8 {
            self.frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0))]);
        }
        self.frame(vec![button(to, false)]);
        self.settle();
    }

    /// The icon column's left edge, as drawn last.
    fn column(&self) -> f32 {
        self.ctx.data(|d| d.get_temp::<f32>(egui::Id::new("dock-icon-column-left"))).unwrap()
    }

    /// The icon of panel `id` in the column (the column holds the icon panels in
    /// `ICON_PANEL_GROUPS` order; an open card keeps its icon there).
    fn icon(&self, id: &str) -> Rect {
        let left = self.column();
        let mut icons: Vec<Rect> = self.ctx.viewport(|vp| {
            vp.prev_pass.widgets.layers().flat_map(|(_, w)| w.iter()).filter(|w| w.rect.size() == vec2(30.0, 30.0)).map(|w| w.rect).collect()
        });
        icons.retain(|r| r.left() >= left - 1.0);
        icons.sort_by(|a, b| a.top().total_cmp(&b.top()));
        icons.dedup();
        let order: Vec<&str> = crate::state::ICON_PANEL_GROUPS
            .iter()
            .flat_map(|g| g.iter().copied())
            .filter(|p| crate::state::ICON_PANELS.iter().any(|e| e.0 == *p))
            .collect();
        assert_eq!(icons.len(), order.len(), "one icon per icon panel, cards' too");
        icons[order.iter().position(|p| *p == id).expect("an icon panel")]
    }

    /// Click panel `id`'s icon where it is drawn now.
    fn tap(&mut self, id: &str) {
        let at = self.icon(id).center();
        self.click(at);
    }

    /// The panels open as cards, in the order they were opened.
    fn cards(&self) -> Vec<&str> {
        self.app.ui.varos_panel_stack.cards.iter().map(String::as_str).collect()
    }

    /// Panel `first`'s card as drawn last.
    fn card(&self, first: &str) -> Rect {
        let id = egui::Id::new("vectorcraft-panel-docking").with(("floating", &first.to_string()));
        self.ctx.memory(|m| m.area_rect(id)).unwrap_or_else(|| panic!("{first}'s card is drawn"))
    }

    /// Panel `first`'s card was drawn in the last frame.
    fn card_drawn(&self, first: &str) -> bool {
        let id = egui::Id::new("vectorcraft-panel-docking").with(("floating", &first.to_string()));
        self.ctx.memory(|m| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Middle, id)))
    }

    /// The floating groups' panels.
    fn groups(&self) -> Vec<Vec<&str>> {
        self.app.ui.floating_panels.iter().map(|g| g.panels.iter().map(String::as_str).collect()).collect()
    }
}

/// Character then Paragraph from their icons: each opens as a card of its own (an upstream
/// floating group), both stay open, neither overlaps the other, both sit left of the dock, each
/// icon stays in the column and is lit. Character's icon again closes only Character's card.
#[test]
fn each_icon_opens_its_own_card_and_the_others_stay_open() {
    let mut h = Harness::new();
    assert!(h.app.ui.varos_panel_stack.keep_open, "keep-open is the Varos default");
    h.tap("character");
    h.tap("paragraph");
    assert_eq!(h.cards(), ["character", "paragraph"]);
    assert_eq!(h.groups(), [["character"], ["paragraph"]], "two independent floating groups");
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("paragraph"), "upstream's open panel is the newest card");
    assert!(panel_stack::is_open(&h.app, "character") && panel_stack::is_open(&h.app, "paragraph"), "both icons lit");
    let (a, b) = (h.card("character"), h.card("paragraph"));
    assert!(!a.intersects(b), "side by side, not overlapping: {a:?} {b:?}");
    let column = h.column();
    assert!(a.right() <= column && b.right() <= column, "left of the icon column: {a:?} {b:?} {column}");
    assert!(b.top() > a.bottom(), "the second opens under the first: {a:?} {b:?}");
    for id in ["character", "paragraph"] {
        assert_eq!(crate::menus::checked(&h.app, "window.panel", &json!({ "panel": id })), Some(true), "Window menu ticks {id}");
    }
    h.tap("character");
    assert_eq!(h.cards(), ["paragraph"]);
    assert_eq!(h.groups(), [["paragraph"]], "Character's card closed, Paragraph's stays");
    assert!(!panel_stack::is_open(&h.app, "character") && h.card_drawn("paragraph"));
    assert!(h.app.ui.open_panel.as_deref() != Some("character"), "no pop-out instead of the card");
}

/// A card's own × closes it (and only it): it goes back to its icon, not as a pop-out.
#[test]
fn a_cards_close_button_closes_only_that_card() {
    let mut h = Harness::new();
    for id in ["character", "paragraph", "glyphs"] {
        h.tap(id);
    }
    assert_eq!(h.cards(), ["character", "paragraph", "glyphs"]);
    // The × sits in the header at the right of the card's tab strip, left of its panel menu.
    let card = h.card("paragraph");
    h.click(pos2(card.right() - 36.0, card.top() + 16.0));
    assert_eq!(h.cards(), ["character", "glyphs"], "Paragraph closed, the others stayed");
    assert_eq!(h.groups(), [["character"], ["glyphs"]]);
    assert!(h.app.ui.open_panel.as_deref() != Some("paragraph"), "it doesn't pop out of its icon");
    assert!(!panel_stack::is_open(&h.app, "paragraph"));
    // Its icon opens it again, where it was.
    h.tap("paragraph");
    assert_eq!(h.card("paragraph").min, card.min);
}

/// Keep-open off, or Auto-Collapse Iconic Panels on: upstream's one pop-out at a time.
#[test]
fn without_keep_open_an_icon_swaps_the_flyout() {
    for auto_collapse in [false, true] {
        let mut h = Harness::new();
        if auto_collapse {
            h.app.session.execute("prefs.set", &json!({"key": "autoCollapseIconPanels", "value": true})).unwrap();
        } else {
            h.app.ui.varos_panel_stack.keep_open = false;
        }
        h.tap("character");
        h.tap("paragraph");
        assert_eq!(h.app.ui.open_panel.as_deref(), Some("paragraph"), "{auto_collapse}");
        assert!(!panel_stack::is_open(&h.app, "character"), "{auto_collapse}: Paragraph took Character's place");
        assert!(h.cards().is_empty() && h.groups().is_empty(), "{auto_collapse}: no cards");
    }
}

/// Where new cards go: down the column beside the dock without overlapping, then (window full)
/// cascading left and down from the column's top, always on screen.
#[test]
fn new_cards_stack_down_the_column_then_cascade() {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0));
    let size = vec2(256.0, 300.0);
    let (right, top) = (1100.0, 100.0);
    let first = panel_stack::place(size, &[], screen, right, top);
    assert_eq!(first, pos2(right - 256.0, top));
    let a = Rect::from_min_size(first, size);
    let second = panel_stack::place(size, &[a], screen, right, top);
    assert_eq!(second, pos2(first.x, a.bottom() + 6.0), "right under the first");
    let b = Rect::from_min_size(second, size);
    // No room for a third under them (800 high): it cascades from the column's top.
    let third = panel_stack::place(size, &[a, b], screen, right, top);
    assert_eq!(third, pos2(first.x - 24.0, top + 24.0));
    let c = Rect::from_min_size(third, size);
    let fourth = panel_stack::place(size, &[a, b, c], screen, right, top);
    assert_eq!(fourth, pos2(first.x - 48.0, top + 48.0), "the next cascade step");
    // A card moved away frees its place in the column.
    let moved = a.translate(vec2(-600.0, 0.0));
    assert_eq!(panel_stack::place(size, &[moved, b], screen, right, top), first);
    // A card taller than the window still opens at the column's top, on screen.
    let tall = panel_stack::place(vec2(256.0, 2000.0), &[], screen, right, top);
    assert!(tall.y >= screen.top() && tall.x == right - 256.0, "{tall:?}");
}

/// Three cards in a short window: all open at once, on screen, none on top of another's corner.
#[test]
fn three_cards_in_a_short_window_stay_on_screen() {
    let mut h = Harness::sized(vec2(1600.0, 900.0));
    for id in ["character", "paragraph", "glyphs"] {
        h.tap(id);
    }
    assert_eq!(h.cards(), ["character", "paragraph", "glyphs"]);
    let cards: Vec<Rect> = ["character", "paragraph", "glyphs"].iter().map(|id| h.card(id)).collect();
    for c in &cards {
        assert!(c.top() >= 0.0 && c.bottom() <= 900.0 + 0.5 && c.right() <= h.column(), "on screen, left of the column: {c:?}");
    }
    for (i, a) in cards.iter().enumerate() {
        for b in &cards[i + 1..] {
            assert!((a.min - b.min).length() > 10.0, "distinct places: {a:?} {b:?}");
        }
    }
}

/// A card fits its panel's contents: no empty space under them and nothing to scroll (the window
/// being tall enough), until resized by hand.
#[test]
fn a_card_is_as_tall_as_its_panel() {
    let mut h = Harness::new();
    let note = |h: &Harness| {
        h.ctx.data(|d| d.get_temp::<(egui::Vec2, egui::Vec2, u64)>(egui::Id::new(("varos-card-body", "align")))).map(|(c, v, _)| (c, v)).unwrap()
    };
    // Opened outside a click (as a Properties button does): the frame that opens it asks for the
    // next one, which draws it at its first size and asks for the one that fits it.
    h.app.ui.open_panel = Some("align".into());
    assert!(h.frame_out(vec![]), "opening a card asks for a repaint");
    assert!(h.frame_out(vec![]), "fitting it asks for a repaint");
    // Fitted within a few frames, without any input; then nothing more to do.
    let settled = (0..6).position(|_| !h.frame_out(vec![]));
    assert!(settled.is_some(), "fitting comes to rest");
    let idle = (0..3).map(|_| h.frame_out(vec![])).collect::<Vec<_>>();
    assert_eq!(idle, [false, false, false], "a fitted card leaves the window idle");
    let (content, viewport) = note(&h);
    assert!((content.y - viewport.y).abs() <= 1.5, "fitted: contents {content:?}, room {viewport:?}");
    assert!(content.x <= viewport.x + 0.5, "every row fits its width: {content:?} in {viewport:?}");
    assert!(h.card("align").right() <= h.column(), "widened leftwards, clear of the icon column");
    // Resized by hand (its corner), it keeps that size.
    let card = h.card("align");
    h.drag(card.right_bottom() - vec2(4.0, 4.0), card.right_bottom() + vec2(0.0, 120.0));
    let resized = h.card("align");
    assert!(resized.height() > card.height() + 60.0, "{card:?} -> {resized:?}");
    h.settle();
    assert!((h.card("align").height() - resized.height()).abs() < 1.0, "not fitted back");
    assert!(!h.app.ui.varos_panel_stack.places["align"].fit);
}

/// The open cards and each panel's place are saved with the UI state (`ui.json`); a restored
/// profile shows the cards again with their icons lit.
#[test]
fn cards_and_their_places_are_saved_with_the_ui_state() {
    let mut h = Harness::new();
    for id in ["character", "paragraph"] {
        h.tap(id);
    }
    let place = h.card("paragraph").min;
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    assert_eq!(saved["varos_panel_stack"]["cards"], json!(["character", "paragraph"]));
    assert!(saved["varos_panel_stack"]["places"]["paragraph"]["pos"].is_array());
    let restored: crate::state::UiState = serde_json::from_value(saved).unwrap();
    let mut h2 = Harness::new();
    h2.app.ui = restored.sanitized();
    h2.settle();
    assert_eq!(h2.cards(), ["character", "paragraph"]);
    assert!(panel_stack::is_open(&h2.app, "character") && panel_stack::is_open(&h2.app, "paragraph"));
    assert_eq!(h2.card("paragraph").min, place);
    // Older profiles: without the field, keep-open on and no cards; lane-ux1's `open` list is
    // ignored.
    let mut old = serde_json::to_value(crate::state::UiState::default()).unwrap();
    old.as_object_mut().unwrap().remove("varos_panel_stack");
    let restored: crate::state::UiState = serde_json::from_value(old.clone()).unwrap();
    assert!(restored.varos_panel_stack.keep_open && restored.varos_panel_stack.cards.is_empty());
    old["varos_panel_stack"] = json!({"keep_open": true, "open": ["character"]});
    let restored: crate::state::UiState = serde_json::from_value(old).unwrap();
    assert!(restored.varos_panel_stack.cards.is_empty());
}

/// A card moved and closed opens again where it was left, even after other cards took the
/// column's first place.
#[test]
fn a_card_opens_again_where_it_was_left() {
    let mut h = Harness::new();
    h.tap("character");
    let card = h.card("character");
    // Moved by its grip (the strip's left 12 points).
    let to = pos2(500.0, 420.0);
    h.drag(card.left_top() + vec2(6.0, 16.0), to);
    let moved = h.card("character");
    assert!(moved.contains(to) && (moved.min - card.min).length() > 100.0, "{card:?} -> {moved:?}");
    h.tap("character");
    assert!(h.cards().is_empty());
    h.tap("paragraph");
    h.tap("character");
    assert_eq!(h.card("character").min, moved.min);
}

/// Tab (`window.dock`) and Shift+Tab (`window.panels`) hide every card; showing the panels again
/// brings every card back where it was.
#[test]
fn hiding_the_panels_hides_every_card_and_brings_them_back() {
    for command in ["window.dock", "window.panels"] {
        let mut h = Harness::new();
        h.tap("character");
        h.tap("paragraph");
        let (a, b) = (h.card("character"), h.card("paragraph"));
        h.app.run(command, json!({})).unwrap();
        h.settle();
        assert!(!h.app.ui.dock, "{command} hid the panels");
        assert!(!h.card_drawn("character") && !h.card_drawn("paragraph"), "{command}: no card visible");
        assert_eq!(h.cards(), ["character", "paragraph"], "{command}: the cards wait");
        h.app.run(command, json!({})).unwrap();
        h.settle();
        assert!(h.card_drawn("character") && h.card_drawn("paragraph"), "{command}: both back");
        assert_eq!((h.card("character"), h.card("paragraph")), (a, b), "{command}: where they were");
        assert!(panel_stack::is_open(&h.app, "character") && panel_stack::is_open(&h.app, "paragraph"));
    }
}

/// Window › a panel and its shortcut toggle its card like its icon; a panel opened another way
/// (a Properties button, a workspace) opens as a card too.
#[test]
fn the_window_menu_and_other_ways_in_open_cards() {
    let mut h = Harness::new();
    h.app.run("window.panel", json!({"panel": "character", "toggle": true})).unwrap();
    h.app.run("window.panel", json!({"panel": "paragraph", "toggle": true})).unwrap();
    h.settle();
    assert_eq!(h.cards(), ["character", "paragraph"]);
    h.app.run("window.panel", json!({"panel": "character", "toggle": true})).unwrap();
    h.settle();
    assert_eq!(h.cards(), ["paragraph"], "the menu closed Character's card only");
    // Upstream code that pops a panel out of its icon (here as a Properties button does).
    h.app.ui.open_panel = Some("align".into());
    h.settle();
    assert_eq!(h.cards(), ["paragraph", "align"]);
    assert!(h.card_drawn("align"));
}

/// A card dragged by its tab onto another card's tabs joins it as a tab (upstream's grouping);
/// the group's panels stay open with their icons lit, and each icon closes its own tab.
#[test]
fn a_card_dropped_on_another_groups_with_it() {
    let mut h = Harness::new();
    h.tap("character");
    h.tap("paragraph");
    let tab = |h: &Harness, p: &str| h.ctx.read_response(egui::Id::new("vectorcraft-panel-docking").with(("tab", &p.to_string()))).unwrap().rect;
    let onto = tab(&h, "character").right_center() - vec2(3.0, 0.0);
    h.drag(tab(&h, "paragraph").center(), onto);
    assert_eq!(h.groups(), [["character", "paragraph"]], "one card, two tabs");
    assert!(panel_stack::is_open(&h.app, "character") && panel_stack::is_open(&h.app, "paragraph"));
    h.tap("paragraph");
    assert_eq!(h.groups(), [["character"]], "Paragraph's icon closed its tab only");
}

/// A card closed and opened again after another card took its place doesn't land on top of it:
/// it takes the next free place instead.
#[test]
fn a_card_reopened_where_another_now_is_takes_a_free_place() {
    let mut h = Harness::new();
    h.tap("character");
    let first = h.card("character");
    h.tap("character");
    h.tap("paragraph");
    assert!((h.card("paragraph").min - first.min).length() <= 2.0, "Paragraph took the column's first place");
    h.tap("character");
    let (a, b) = (h.card("character"), h.card("paragraph"));
    assert!(!a.intersects(b), "Character isn't hidden under Paragraph: {a:?} {b:?}");
}

/// A card left far right in a wide window, opened again in a narrower one, or restored from a
/// profile saved in a wider window, stays left of the icon column and the dock.
#[test]
fn a_card_never_opens_over_the_dock() {
    let mut h = Harness::new();
    h.tap("character");
    let card = h.card("character");
    h.drag(card.left_top() + vec2(6.0, 16.0), card.left_top() + vec2(6.0 + 200.0, 216.0));
    h.tap("character");
    let saved = h.app.ui.varos_panel_stack.places["character"].pos;
    h.screen = vec2(1000.0, 1400.0);
    h.settle();
    h.tap("character");
    let dock = h.ctx.data(|d| d.get_temp::<Rect>(crate::floating::dock_rect_id())).unwrap();
    let reopened = h.card("character");
    assert!(reopened.right() <= h.column() && reopened.right() <= dock.left(), "{saved:?} -> {reopened:?}, column {}", h.column());
    // Saved open in the wide window, loaded in the narrow one.
    let mut wide = Harness::new();
    wide.tap("paragraph");
    let pos = wide.card("paragraph").min;
    wide.drag(pos + vec2(6.0, 16.0), pos + vec2(206.0, 16.0));
    let saved: crate::state::UiState = serde_json::from_value(serde_json::to_value(&wide.app.ui).unwrap()).unwrap();
    let mut narrow = Harness::sized(vec2(1000.0, 1400.0));
    narrow.app.ui = saved.sanitized();
    narrow.settle();
    let dock = narrow.ctx.data(|d| d.get_temp::<Rect>(crate::floating::dock_rect_id())).unwrap();
    let restored = narrow.card("paragraph");
    assert!(restored.right() <= narrow.column() && restored.right() <= dock.left(), "{restored:?}, dock {dock:?}");
}

/// With the dock expanded (its tabbed group showing), cards open left of all of it.
#[test]
fn cards_open_left_of_the_expanded_dock() {
    let mut h = Harness::new();
    assert!(!h.app.ui.dock_collapsed);
    for id in ["character", "paragraph", "align"] {
        h.tap(id);
    }
    let dock = h.ctx.data(|d| d.get_temp::<Rect>(crate::floating::dock_rect_id())).unwrap();
    for id in ["character", "paragraph", "align"] {
        let c = h.card(id);
        assert!(c.right() <= dock.left() && c.right() <= h.column(), "{id}: {c:?}, dock {dock:?}");
    }
}

/// A shortcut or an agent asking for a card already open (`window.panel` without `toggle`) only
/// shows it; it never closes it.
#[test]
fn showing_an_open_card_again_keeps_it_open() {
    let mut h = Harness::new();
    h.tap("character");
    for _ in 0..2 {
        h.app.run("window.panel", json!({"panel": "character"})).unwrap();
        h.settle();
        assert_eq!(h.cards(), ["character"]);
        assert!(h.card_drawn("character"));
    }
}

/// A panel's key (Cmd+T for Character) toggles its card, as in Illustrator, without touching the
/// other cards.
#[test]
fn a_panels_key_toggles_its_card() {
    let mut h = Harness::new();
    h.tap("paragraph");
    let cmd_t = || {
        let m = egui::Modifiers::COMMAND;
        vec![egui::Event::ModifiersChanged(m), egui::Event::Key { key: egui::Key::T, physical_key: None, pressed: true, repeat: false, modifiers: m }]
    };
    h.frame(cmd_t());
    h.settle();
    assert_eq!(h.cards(), ["paragraph", "character"], "Cmd+T opened Character's card");
    h.frame(cmd_t());
    h.settle();
    assert_eq!(h.cards(), ["paragraph"], "Cmd+T again closed it, Paragraph stayed");
}

/// A card remembered larger than the room left of the dock (here 900 wide, opened again in a
/// narrow window) is made to fit that room; it never covers the dock.
#[test]
fn a_card_is_never_larger_than_the_room_left_of_the_dock() {
    let mut h = Harness::sized(vec2(1000.0, 1000.0));
    h.tap("character");
    h.tap("character");
    let place = h.app.ui.varos_panel_stack.places.get_mut("character").unwrap();
    place.size = [900.0, 300.0];
    place.fit = false;
    h.tap("character");
    let card = h.card("character");
    let dock = h.ctx.data(|d| d.get_temp::<Rect>(crate::floating::dock_rect_id())).unwrap();
    assert!(card.left() >= 0.0 && card.right() <= h.column() && card.right() <= dock.left(), "{card:?}, dock {dock:?}");
}

/// A card dragged over the dock, or resized across it, goes back left of it once let go.
#[test]
fn a_card_dragged_or_resized_over_the_dock_moves_back() {
    let mut h = Harness::new();
    h.tap("character");
    let card = h.card("character");
    h.drag(card.left_top() + vec2(6.0, 16.0), card.left_top() + vec2(206.0, 316.0));
    let moved = h.card("character");
    assert!(moved.right() <= h.column() + 0.5, "moved back left of the column: {moved:?}, column {}", h.column());
    assert!(moved.top() > card.top() + 200.0, "it kept the drag's height: {moved:?}");
    let corner = moved.right_bottom() - vec2(4.0, 4.0);
    h.drag(corner, corner + vec2(300.0, 0.0));
    let resized = h.card("character");
    assert!(resized.right() <= h.column() + 0.5, "resized back left of the column: {resized:?}");
}

/// The cascade never lands two cards on one corner, also when its steps are pushed back into a
/// narrow room.
#[test]
fn cascade_steps_moved_into_the_room_stay_distinct() {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(256.0, 800.0));
    let size = vec2(256.0, 300.0);
    let mut taken = Vec::new();
    for _ in 0..5 {
        let p = panel_stack::place(size, &taken, screen, 256.0, 100.0);
        assert!(screen.contains_rect(Rect::from_min_size(p, size).shrink(0.5)), "{p:?} in the room");
        assert!(taken.iter().all(|t: &Rect| (t.min - p).length() >= 12.0), "{p:?} isn't on another card's corner: {taken:?}");
        taken.push(Rect::from_min_size(p, size));
    }
}

/// A workspace saved with cards open brings them back as cards (icons kept and lit).
#[test]
fn a_workspace_restores_its_cards() {
    let mut h = Harness::new();
    h.tap("character");
    h.tap("paragraph");
    let saved = crate::workspaces::capture(&h.app.ui, "Cards");
    crate::workspaces::apply(&mut h.app.ui, &crate::workspaces::Workspace::default());
    h.settle();
    assert!(h.cards().is_empty());
    crate::workspaces::apply(&mut h.app.ui, &saved);
    h.settle();
    assert_eq!(h.cards(), ["character", "paragraph"]);
    for id in ["character", "paragraph"] {
        assert!(panel_stack::is_open(&h.app, id) && h.card_drawn(id), "{id}: a card again, its icon lit");
        h.icon(id);
    }
}

/// Turning Auto-Collapse Iconic Panels on with cards open closes them; the newest pops out of its
/// icon as upstream's single transient pop-out.
#[test]
fn auto_collapse_turns_the_cards_into_one_pop_out() {
    let mut h = Harness::new();
    h.tap("character");
    h.tap("paragraph");
    h.app.session.execute("prefs.set", &json!({"key": "autoCollapseIconPanels", "value": true})).unwrap();
    h.settle();
    assert!(h.groups().is_empty() && h.cards().is_empty(), "no unmanaged floating cards left: {:?}", h.groups());
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("paragraph"));
    assert!(h.ctx.memory(|m| m.area_rect(egui::Id::new("icon-panel"))).is_some(), "the pop-out shows");
}

/// Optional software capture of the real window with three cards open; set
/// `VAROS_PANELS_CAPTURE_DIR` to write `panels-3-cards.png` there.
#[test]
#[ignore = "optional software visual capture; set VAROS_PANELS_CAPTURE_DIR"]
fn capture_three_cards() {
    let Some(root) = std::env::var_os("VAROS_PANELS_CAPTURE_DIR").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&root).unwrap();
    let (width, height) = (1440_u32, 900_u32);
    let mut h = Harness::sized(vec2(width as f32, height as f32));
    for id in ["character", "paragraph", "align"] {
        h.tap(id);
    }
    // Drawn again in a fresh context, so that every texture (the font atlas first) is captured.
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    h.app.fonts_ready = true;
    h.app.canvas.worker_started = true;
    let mut textures = std::collections::HashMap::<egui::TextureId, egui::ColorImage>::new();
    let mut shot = None;
    for frame in 0..5 {
        let raw = egui::RawInput {
            time: Some(100.0 + 0.05 * frame as f64),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height as f32))),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let app = &mut h.app;
        let mut out = ctx.run_ui(raw, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                if let Some([x, y]) = delta.pos {
                    let target = textures.get_mut(id).unwrap();
                    for row in 0..image.size[1] {
                        for col in 0..image.size[0] {
                            target.pixels[(y + row) * target.size[0] + x + col] = image.pixels[row * image.size[0] + col];
                        }
                    }
                } else {
                    textures.insert(*id, (**image).clone());
                }
            }
        }
        out.textures_delta.clear(); // Applied to the software atlas above.
        shot = Some(ctx.tessellate(out.shapes, out.pixels_per_point));
    }
    let pixels = crate::varos::home::capture::raster(&shot.unwrap(), &textures, width, height);
    image::save_buffer(root.join("panels-3-cards.png"), &pixels, width, height, image::ColorType::Rgba8).unwrap();
}
