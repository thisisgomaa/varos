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
        app.begin_host_iteration();
        let mut out = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
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

    /// The dock area's controls as drawn last.
    fn hits(&self) -> Vec<(craft_ui::docking::Hit<String>, Rect)> {
        crate::panel_docking::hit_rects(&self.ctx)
    }

    /// The icon columns' bounds, as drawn last.
    fn icons(&self) -> Rect {
        self.hits()
            .into_iter()
            .filter(|(hit, _)| matches!(hit, craft_ui::docking::Hit::Icon(_) | craft_ui::docking::Hit::IconGrip(_)))
            .map(|(_, r)| r)
            .reduce(|a, b| a.union(b))
            .expect("an icon column is drawn")
    }

    /// The icon column's left edge, as drawn last.
    fn column(&self) -> f32 {
        self.icons().left()
    }

    /// The docked groups' left edge (their tabs and bars), as drawn last; the column's when none
    /// shows.
    fn dock_left(&self) -> f32 {
        use craft_ui::docking::Hit;
        self.hits()
            .into_iter()
            .filter(|(hit, _)| match hit {
                Hit::Tab(p) | Hit::DragBar(p) | Hit::ColumnBar(p) => {
                    p != crate::panel_docking::CANVAS && !crate::panel_docking::floats(&self.app.ui, p)
                }
                _ => false,
            })
            .map(|(_, r)| r.left())
            .fold(self.column(), f32::min)
    }

    /// The icon of panel `id` in its column (an open card keeps its icon there).
    fn icon(&self, id: &str) -> Rect {
        self.hits()
            .into_iter()
            .find_map(|(hit, r)| matches!(&hit, craft_ui::docking::Hit::Icon(p) if p == id).then_some(r))
            .unwrap_or_else(|| panic!("{id}'s icon is drawn"))
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
        let id = egui::Id::unique("vectorcraft-panel-docking").with(("floating", &first.to_string()));
        self.ctx.memory(|m| m.area_rect(id)).unwrap_or_else(|| panic!("{first}'s card is drawn"))
    }

    /// Panel `first`'s card was drawn in the last frame.
    fn card_drawn(&self, first: &str) -> bool {
        let id = egui::Id::unique("vectorcraft-panel-docking").with(("floating", &first.to_string()));
        self.ctx.memory(|m| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Middle, id)))
    }

    /// Panel `p`'s tab in its card's header, as drawn last.
    fn tab(&self, p: &str) -> Rect {
        self.ctx.read_response(egui::Id::unique("varos-card-tab").with(p)).unwrap_or_else(|| panic!("{p}'s tab is drawn")).rect
    }

    /// A point of the empty part of card `first`'s header (between its tabs and its ×).
    fn header(&self, first: &str) -> Pos2 {
        let card = self.card(first);
        pos2(card.right() - 60.0, card.top() + 16.0)
    }

    /// Press at `from`, then move the pointer through `path` (one frame each), calling `each` after
    /// every move; let go at the last point.
    fn drag_through(&mut self, from: Pos2, path: &[Pos2], mut each: impl FnMut(&Self, Pos2)) {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for &p in path {
            self.frame(vec![egui::Event::PointerMoved(p)]);
            each(self, p);
        }
        let last = path.last().copied().unwrap_or(from);
        self.frame(vec![button(last, false)]);
    }

    /// The cards' panels.
    fn groups(&self) -> Vec<Vec<&str>> {
        self.app.ui.varos_panel_stack.live.iter().map(|c| c.panels.iter().map(String::as_str).collect()).collect()
    }

    /// The tab in front of the first card.
    fn front(&self) -> String {
        let c = &self.app.ui.varos_panel_stack.live[0];
        c.panels[c.active].clone()
    }
}

/// Character then Paragraph from their icons: each opens as a card of its own, both stay open, neither overlaps the other, both sit left of the dock, each
/// icon stays in the column and is lit. Character's icon again closes only Character's card.
#[test]
fn each_icon_opens_its_own_card_and_the_others_stay_open() {
    let mut h = Harness::new();
    assert!(h.app.ui.varos_panel_stack.keep_open, "keep-open is the Varos default");
    h.tap("character");
    h.tap("paragraph");
    assert_eq!(h.cards(), ["character", "paragraph"]);
    assert_eq!(h.groups(), [["character"], ["paragraph"]], "two independent cards");
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

/// A card whose icon is low in the column, with no room under the cards in its way, goes right
/// above them (as near its icon as it can), before cascading.
#[test]
fn a_card_with_no_room_below_goes_above() {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 900.0));
    let (right, size) = (1100.0, vec2(256.0, 250.0));
    let below = Rect::from_min_size(pos2(right - 256.0, 540.0), vec2(256.0, 340.0));
    let p = panel_stack::place(size, &[below], screen, right, 500.0);
    assert_eq!(p, pos2(right - 256.0, below.top() - 6.0 - 250.0), "right above the card in its way");
    // No room above either: the cascade.
    let above = Rect::from_min_size(pos2(right - 256.0, 0.0), vec2(256.0, 300.0));
    let p = panel_stack::place(size, &[below, above], screen, right, 500.0);
    assert_eq!(p, pos2(right - 256.0 - 24.0, 524.0));
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
        h.ctx.data(|d| d.get_temp::<(egui::Vec2, egui::Vec2, u64)>(egui::Id::unique(("varos-card-body", "align")))).map(|(c, v, _)| (c, v)).unwrap()
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

/// A card dragged by its tab onto another card's tabs joins it as a tab; the group's panels stay
/// open with their icons lit, and each icon closes its own tab.
#[test]
fn a_card_dropped_on_another_groups_with_it() {
    let mut h = Harness::new();
    h.tap("character");
    h.tap("paragraph");
    let onto = h.tab("character").right_center() - vec2(3.0, 0.0);
    h.drag(h.tab("paragraph").center(), onto);
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
    // Paragraph moved by its header onto the place Character had.
    let card = h.card("paragraph");
    h.drag(h.header("paragraph"), h.header("paragraph") + (first.min - card.min));
    assert!((h.card("paragraph").min - first.min).length() <= 2.0, "Paragraph lies where Character was");
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
    let dock = h.dock_left();
    let reopened = h.card("character");
    assert!(reopened.right() <= h.column() && reopened.right() <= dock, "{saved:?} -> {reopened:?}, column {}", h.column());
    // Saved open in the wide window, loaded in the narrow one.
    let mut wide = Harness::new();
    wide.tap("paragraph");
    let pos = wide.card("paragraph").min;
    wide.drag(pos + vec2(6.0, 16.0), pos + vec2(206.0, 16.0));
    let saved: crate::state::UiState = serde_json::from_value(serde_json::to_value(&wide.app.ui).unwrap()).unwrap();
    let mut narrow = Harness::sized(vec2(1000.0, 1400.0));
    narrow.app.ui = saved.sanitized();
    narrow.settle();
    let dock = narrow.dock_left();
    let restored = narrow.card("paragraph");
    assert!(restored.right() <= narrow.column() && restored.right() <= dock, "{restored:?}, dock {dock:?}");
}

/// With the dock expanded (its tabbed group showing), cards open left of all of it.
#[test]
fn cards_open_left_of_the_expanded_dock() {
    let mut h = Harness::new();
    assert!(!h.app.ui.dock_collapsed);
    for id in ["character", "paragraph", "align"] {
        h.tap(id);
    }
    let dock = h.dock_left();
    for id in ["character", "paragraph", "align"] {
        let c = h.card(id);
        assert!(c.right() <= dock && c.right() <= h.column(), "{id}: {c:?}, dock {dock:?}");
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
    let dock = h.dock_left();
    assert!(card.left() >= 0.0 && card.right() <= h.column() && card.right() <= dock, "{card:?}, dock {dock:?}");
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

/// A workspace switch keeps the cards whose panels stay in an icon column (icons kept and lit);
/// since the 2026-10-26 merge cards are Varos' own and workspaces (craft-ui layouts) don't carry
/// them.
#[test]
fn a_workspace_switch_keeps_the_cards() {
    let mut h = Harness::new();
    h.tap("character");
    h.tap("paragraph");
    let saved = crate::workspaces::capture(&h.app.ui, "Cards");
    crate::workspaces::apply(&mut h.app.ui, &crate::workspaces::Workspace::default());
    h.settle();
    crate::workspaces::apply(&mut h.app.ui, &saved);
    h.settle();
    assert_eq!(h.cards(), ["character", "paragraph"]);
    for id in ["character", "paragraph"] {
        assert!(panel_stack::is_open(&h.app, id) && h.card_drawn(id), "{id}: a card, its icon lit");
        h.icon(id);
    }
}

/// Turning Auto-Collapse Iconic Panels on with cards open closes them; the newest shows as its icon
/// column's flyout, upstream's single transient pop-out.
#[test]
fn auto_collapse_turns_the_cards_into_one_pop_out() {
    let mut h = Harness::new();
    h.tap("character");
    h.tap("paragraph");
    h.app.session.execute("prefs.set", &json!({"key": "autoCollapseIconPanels", "value": true})).unwrap();
    h.settle();
    assert!(h.groups().is_empty() && h.cards().is_empty(), "no cards left: {:?}", h.groups());
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("paragraph"));
    assert!(crate::panel_docking::showing(&h.app.ui, "paragraph"), "its flyout shows");
}

/// A card moves with the pointer from anywhere on its header (its empty part or its tab), in the
/// same frame as the pointer (no outline that follows and a jump on release), and ends exactly
/// where it was let go. Escape puts it back. The card work of a drag frame stays under 1 ms.
#[test]
fn a_card_moves_live_from_its_header() {
    let mut h = Harness::new();
    h.tap("character");
    for grab_at in ["header", "tab"] {
        let card = h.card("character");
        let from = if grab_at == "header" { h.header("character") } else { h.tab("character").center() };
        let grab = from - card.min;
        let path: Vec<Pos2> = (1..=10).map(|k| from + vec2(-30.0 * k as f32, 24.0 * k as f32)).collect();
        let mut costs = Vec::new();
        h.drag_through(from, &path, |h, p| {
            let now = h.card("character");
            assert!((now.min - (p - grab)).length() < 1.0, "{grab_at}: the card is under the pointer this frame: {now:?}, pointer {p:?}");
            assert_eq!(now.size(), card.size(), "{grab_at}: moving doesn't resize");
            costs.push(h.app.ui.varos_panel_stack.cost_ms);
        });
        h.settle();
        let end = *path.last().unwrap();
        let moved = h.card("character");
        assert!((moved.min - (end - grab)).length() < 1.0, "{grab_at}: let go where the pointer was: {moved:?}, pointer {end:?}");
        assert_eq!(h.cards(), ["character"], "{grab_at}: still one card");
        costs.sort_by(f64::total_cmp);
        let median = costs[costs.len() / 2];
        eprintln!("{grab_at} drag: card work per frame median {median:.3} ms, max {:.3} ms", costs[costs.len() - 1]);
        assert!(median < 1.0, "{grab_at}: a drag frame's card work (move + draw) takes {median:.3} ms");
    }
    // Escape while moving puts the card back.
    let card = h.card("character");
    let from = h.header("character");
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    h.frame(vec![egui::Event::PointerMoved(from + vec2(-120.0, 80.0))]);
    assert!((h.card("character").min - card.min).length() > 100.0);
    h.frame(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    h.frame(vec![button(from + vec2(-120.0, 80.0), false)]);
    h.settle();
    assert_eq!(h.card("character").min, card.min, "Escape put it back");
}

/// A card resizes live by its bottom-right corner, and by its right or bottom edge alone.
#[test]
fn a_card_resizes_live_by_its_corner_and_edges() {
    let mut h = Harness::new();
    h.tap("character");
    let card = h.card("character");
    // Moved left first, so that it can grow rightwards without reaching the dock.
    h.drag(h.header("character"), h.header("character") - vec2(300.0, 0.0));
    let card = Rect::from_min_size(card.min - vec2(300.0, 0.0), card.size());
    assert_eq!(h.card("character"), card);
    let from = card.right_bottom() - vec2(4.0, 4.0);
    let path: Vec<Pos2> = (1..=6).map(|k| from + vec2(20.0 * k as f32, 15.0 * k as f32)).collect();
    h.drag_through(from, &path, |h, p| {
        let now = h.card("character");
        assert_eq!(now.min, card.min, "the corner moves, not the card");
        assert!((now.max - (p + vec2(4.0, 4.0))).length() < 1.0, "the corner is under the pointer this frame: {now:?}, {p:?}");
    });
    h.settle();
    let resized = h.card("character");
    assert!((resized.max - (*path.last().unwrap() + vec2(4.0, 4.0))).length() < 1.0, "{resized:?}");
    assert!(!h.app.ui.varos_panel_stack.places["character"].fit, "resized by hand: it isn't fitted any more");
    // Its right edge: wider only.
    let from = pos2(resized.right() - 2.0, resized.center().y);
    h.drag(from, from + vec2(-40.0, 30.0));
    let narrower = h.card("character");
    assert!((narrower.width() - (resized.width() - 40.0)).abs() < 1.0 && narrower.height() == resized.height(), "{narrower:?}");
    // Its bottom edge: taller only.
    let from = pos2(narrower.center().x, narrower.bottom() - 2.0);
    h.drag(from, from + vec2(25.0, 50.0));
    let taller = h.card("character");
    assert!((taller.height() - (narrower.height() + 50.0)).abs() < 1.0 && taller.width() == narrower.width(), "{taller:?}");
}

/// A new card opens beside its own icon: its top level with the icon's, its right edge 6 points
/// left of the dock; one whose icon is low in a short window still fits in the window.
#[test]
fn a_card_opens_beside_its_icon() {
    let mut h = Harness::new();
    for id in ["paragraph", "align", "glyphs"] {
        h.tap(id);
        let (card, icon) = (h.card(id), h.icon(id));
        assert!((card.top() - icon.top()).abs() <= 1.0, "{id}: top {} beside its icon's {}", card.top(), icon.top());
        let dock = h.dock_left();
        assert!((card.right() - (dock.min(h.column()) - 6.0)).abs() <= 1.0, "{id}: {card:?}, dock {dock:?}");
        h.tap(id);
    }
    // A short window: History's icon is near its bottom; the card goes up to fit.
    let mut h = Harness::sized(vec2(1600.0, 760.0));
    h.tap("history");
    let card = h.card("history");
    assert!(card.bottom() <= 760.0 - 8.0 + 0.5 && card.top() >= 0.0, "{card:?}");
    assert!(card.top() <= h.icon("history").top() + 0.5, "{card:?}");
}

/// Panels grouped as tabs of one card and closed by its × come back together: any of them opens
/// the whole card again, same tabs, same tab in front, same place and size. The grouping is saved
/// with the UI state; a tab closed alone by its icon opens again in its card.
#[test]
fn a_closed_group_opens_again_as_it_was() {
    let mut h = Harness::new();
    h.tap("character");
    h.tap("paragraph");
    h.drag(h.tab("paragraph").center(), h.tab("character").right_center() - vec2(3.0, 0.0));
    assert_eq!(h.groups(), [["character", "paragraph"]]);
    // Character in front, the card moved and made taller.
    h.click(h.tab("character").center());
    h.drag(h.header("character"), h.header("character") + vec2(-260.0, 40.0));
    let corner = h.card("character").right_bottom() - vec2(4.0, 4.0);
    h.drag(corner, corner + vec2(0.0, 60.0));
    let card = h.card("character");
    assert_eq!(h.front(), "character");
    h.click(pos2(card.right() - 36.0, card.top() + 16.0));
    assert!(h.groups().is_empty() && h.cards().is_empty(), "its × closed the whole card");
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    assert_eq!(saved["varos_panel_stack"]["groups"], json!([{"panels": ["character", "paragraph"], "active": "character"}]));
    // Paragraph's icon brings the whole card back, as it was.
    h.tap("paragraph");
    assert_eq!(h.groups(), [["character", "paragraph"]], "the same tabs");
    assert_eq!(h.front(), "character", "the same tab in front");
    assert_eq!(h.card("character"), card, "the same place and size");
    assert!(panel_stack::is_open(&h.app, "character") && panel_stack::is_open(&h.app, "paragraph"), "both icons lit");
    // A tab closed by its icon opens again in its card.
    h.tap("paragraph");
    assert_eq!(h.groups(), [["character"]]);
    h.tap("paragraph");
    assert_eq!(h.groups(), [["character", "paragraph"]], "back in its card");
    // Restored from ui.json (also after a restart).
    h.click(pos2(card.right() - 36.0, card.top() + 16.0));
    let restored: crate::state::UiState = serde_json::from_value(serde_json::to_value(&h.app.ui).unwrap()).unwrap();
    let mut h2 = Harness::new();
    h2.app.ui = restored.sanitized();
    h2.settle();
    h2.tap("character");
    assert_eq!(h2.groups(), [["character", "paragraph"]]);
    // Older profiles have no groups.
    let mut old = serde_json::to_value(crate::state::UiState::default()).unwrap();
    old["varos_panel_stack"] = json!({"keep_open": true, "cards": [], "places": {}});
    let restored: crate::state::UiState = serde_json::from_value(old).unwrap();
    assert!(restored.varos_panel_stack.groups.is_empty());
}

/// Tabs move between cards: a tab dragged out of a card with several tabs becomes a card of its
/// own that follows the pointer at once; a card let go on another card's header joins it; a card
/// let go on the icon column goes back to its icon (it closes).
#[test]
fn tabs_move_between_cards_and_to_the_dock() {
    let mut h = Harness::new();
    for id in ["character", "paragraph", "glyphs"] {
        h.tap(id);
    }
    h.drag(h.tab("paragraph").center(), h.tab("character").right_center() - vec2(3.0, 0.0));
    assert_eq!(h.groups(), [vec!["character", "paragraph"], vec!["glyphs"]]);
    // Paragraph's tab dragged out of the card, down-left onto the canvas.
    let from = h.tab("paragraph").center();
    let path: Vec<Pos2> = (1..=10).map(|k| from + vec2(-40.0 * k as f32, 30.0 * k as f32)).collect();
    let mut followed = 0;
    h.drag_through(from, &path, |h, p| {
        if h.groups().iter().any(|g| g == &["paragraph"]) && h.card_drawn("paragraph") {
            // Held by its tab: the pointer 16 points into the new card, on its tab.
            let card = h.card("paragraph");
            // (pushed left of the dock where the pointer would put it over it).
            let room_right = h.dock_left().min(h.column()) - 6.0;
            let x_follows = (card.left() - (p.x - 16.0)).abs() < 1.0 || (card.right() - room_right).abs() < 1.5;
            assert!(x_follows && (card.top() - (p.y - 16.0)).abs() < 1.0, "the torn card is under the pointer: {card:?}, {p:?}");
            followed += 1;
        }
    });
    h.settle();
    assert!(followed >= 5, "the torn-out card followed the pointer while dragged ({followed} frames)");
    assert_eq!(h.groups(), [vec!["character"], vec!["glyphs"], vec!["paragraph"]], "Paragraph is a card of its own");
    assert!(h.card("paragraph").contains(*path.last().unwrap()), "where it was let go");
    // Glyphs let go on Paragraph's header joins Paragraph.
    h.drag(h.header("glyphs"), h.header("paragraph"));
    assert_eq!(h.groups(), [vec!["character"], vec!["paragraph", "glyphs"]]);
    assert_eq!(h.cards().len(), 3, "every panel is still a card's");
    // Character's card let go on the icon column docks it: it goes back to its icon.
    let icon = h.icon("swatches");
    h.drag(h.header("character"), icon.center());
    assert_eq!(h.groups(), [vec!["paragraph", "glyphs"]]);
    assert!(!panel_stack::is_open(&h.app, "character"), "Character closed");
    assert!(h.app.ui.open_panel.as_deref() != Some("character"), "no pop-out instead");
}

/// The card is drawn as the kit's card: 8-point corners, a 1-point line, no shadow, a 32-point
/// header whose tab in front is a dark well, no azure on an idle card.
#[test]
fn a_card_is_drawn_as_the_kit_card() {
    let mut h = Harness::new();
    h.tap("character");
    let card = h.card("character");
    let t = crate::theme::Tokens::get(&h.ctx);
    let screen = Rect::from_min_size(Pos2::ZERO, h.screen);
    h.time += 0.05;
    let input = egui::RawInput { time: Some(h.time), screen_rect: Some(screen), ..Default::default() };
    let app = &mut h.app;
    app.begin_host_iteration();
    let mut out = h.ctx.run_ui(input, |ui| {
        app.tick(ui.ctx());
        app.ui(ui);
    });
    out.textures_delta.clear();
    let rects: Vec<&egui::epaint::RectShape> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r) => Some(r),
            _ => None,
        })
        .collect();
    let same = |r: Rect| (r.min - card.min).length() < 0.6 && (r.max - card.max).length() < 0.6;
    let fill = rects.iter().find(|r| same(r.rect) && r.fill == t.panel).expect("the card's fill");
    assert_eq!(fill.corner_radius, egui::CornerRadius::same(8));
    let line = rects.iter().find(|r| same(r.rect) && r.stroke.width > 0.0).expect("the card's line");
    assert_eq!((line.stroke.width, line.stroke.color, line.corner_radius), (1.0, t.border, egui::CornerRadius::same(8)));
    assert!(!rects.iter().any(|r| r.blur_width > 0.0 && r.rect.intersects(card.expand(24.0))), "no shadow around the card");
    let tab = h.tab("character");
    assert_eq!(tab.height(), 24.0);
    assert!((tab.center().y - (card.top() + 16.0)).abs() < 0.6, "tabs centred in a 32-point header");
    let well = rects.iter().find(|r| (r.rect.min - tab.min).length() < 0.6 && r.fill == t.input).expect("the tab in front is a dark well");
    assert_eq!(well.corner_radius, egui::CornerRadius::same(3));
    assert!(!rects.iter().any(|r| r.fill == t.accent && r.rect.intersects(card)), "no azure on an idle card");
}

/// Review fix 1: a press on a card's × or ≡ that then moves never moves the card (the header's drag
/// area stops short of its buttons).
#[test]
fn the_close_and_menu_buttons_never_move_the_card() {
    let mut h = Harness::new();
    h.tap("character");
    let card = h.card("character");
    for at in [pos2(card.right() - 36.0, card.top() + 16.0), pos2(card.right() - 14.0, card.top() + 16.0)] {
        h.drag(at, at + vec2(-80.0, 60.0));
        assert_eq!(h.card("character"), card, "pressed at {at:?} and moved: the card stays");
        assert_eq!(h.cards(), ["character"]);
    }
}

/// Review fix 2: while held, a card stays in the room left of the dock and above the status bars
/// (the pointer may go over the dock); no jump back on release.
#[test]
fn a_held_card_stays_in_the_room() {
    let mut h = Harness::new();
    h.tap("character");
    let from = h.header("character");
    let icons = h.icons();
    let right = h.dock_left().min(h.column()) - 6.0;
    let path: Vec<Pos2> = (1..=10).map(|k| from + vec2(40.0 * k as f32, 120.0 * k as f32)).collect();
    h.drag_through(from, &path, |h, p| {
        let c = h.card("character");
        assert!(
            c.right() <= right + 0.5 && c.bottom() <= h.ctx.content_rect().bottom() + 0.5,
            "held at {p:?}: {c:?} stays in the room (right {right}, icons {icons:?})"
        );
    });
}

/// A card let go on a docked group's tabs joins that group (its panel leaves the icon column and
/// its card closes).
#[test]
fn a_card_dropped_on_the_dock_joins_its_group() {
    use craft_ui::docking::Hit;
    let mut h = Harness::new();
    h.tap("character");
    let tab = h.hits().into_iter().find_map(|(hit, r)| matches!(&hit, Hit::Tab(p) if p == "layers").then_some(r)).expect("Layers' tab in the dock");
    h.drag(h.header("character"), tab.center());
    assert!(h.groups().is_empty(), "the card closed");
    let layout = crate::panel_docking::layout_of(&h.app.ui);
    assert!(!crate::panel_docking::in_iconic_column(&layout, "character"), "Character left its icon column");
    assert!(layout.contains(&"character".to_string()), "Character is in the dock");
}

/// Review fix 4: a tab closed alone and opened again goes back to its place among its card's tabs,
/// in front (it was asked for).
#[test]
fn a_tab_opens_again_at_its_place() {
    let mut h = Harness::new();
    for id in ["character", "paragraph", "glyphs"] {
        h.tap(id);
    }
    h.drag(h.tab("paragraph").center(), h.tab("character").right_center() - vec2(3.0, 0.0));
    h.drag(h.tab("glyphs").center(), h.tab("paragraph").right_center() - vec2(3.0, 0.0));
    assert_eq!(h.groups(), [["character", "paragraph", "glyphs"]]);
    h.tap("paragraph");
    assert_eq!(h.groups(), [["character", "glyphs"]]);
    h.tap("paragraph");
    assert_eq!(h.groups(), [["character", "paragraph", "glyphs"]], "back between Character and Glyphs");
    assert_eq!(h.front(), "paragraph", "in front");
}

/// Review fix 5: a tab dragged along its own header takes the place of the tabs it passes, live;
/// the card doesn't move.
#[test]
fn a_tab_dragged_along_its_header_reorders_live() {
    let mut h = Harness::new();
    for id in ["character", "paragraph", "glyphs"] {
        h.tap(id);
    }
    h.drag(h.tab("paragraph").center(), h.tab("character").right_center() - vec2(3.0, 0.0));
    h.drag(h.tab("glyphs").center(), h.tab("paragraph").right_center() - vec2(3.0, 0.0));
    let card = h.card("character");
    let from = h.tab("glyphs").center();
    let to = h.tab("character").left_center() + vec2(4.0, 0.0);
    let path: Vec<Pos2> = (1..=8).map(|k| from + (to - from) * (k as f32 / 8.0)).collect();
    let mut orders = Vec::new();
    h.drag_through(from, &path, |h, _| orders.push(h.groups().concat().join(",")));
    h.settle();
    assert!(orders.contains(&"character,glyphs,paragraph".to_string()), "it passed Paragraph while held: {orders:?}");
    assert_eq!(h.groups(), [["glyphs", "character", "paragraph"]], "then Character");
    assert_eq!(h.card("glyphs"), card, "the card stayed where it was");
}

/// Review fix 6: a last move and the pointer leaving the window in the same frame still move the
/// card there; it waits there and is let go there.
#[test]
fn a_move_then_the_pointer_leaving_still_counts() {
    let mut h = Harness::new();
    h.tap("character");
    let card = h.card("character");
    let from = h.header("character");
    let grab = from - card.min;
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    h.frame(vec![egui::Event::PointerMoved(from + vec2(-40.0, 20.0))]);
    let last = from + vec2(-200.0, 90.0);
    h.frame(vec![egui::Event::PointerMoved(last), egui::Event::PointerGone]);
    assert!((h.card("character").min - (last - grab)).length() < 1.0, "the last move counted: {:?}", h.card("character"));
    h.frame(vec![]);
    assert!((h.card("character").min - (last - grab)).length() < 1.0, "it waits there");
    h.frame(vec![button(last, false)]);
    h.settle();
    assert!((h.card("character").min - (last - grab)).length() < 1.0, "let go there");
}

/// Optional software capture of the real window with three cards open, one of them with two tabs
/// (Paragraph and Glyphs); set `VAROS_PANELS_CAPTURE_DIR` to write `panels3-cards.png` there.
#[test]
#[ignore = "optional software visual capture; set VAROS_PANELS_CAPTURE_DIR"]
fn capture_three_cards() {
    let Some(root) = std::env::var_os("VAROS_PANELS_CAPTURE_DIR").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&root).unwrap();
    let (width, height) = (1440_u32, 900_u32);
    let mut h = Harness::sized(vec2(width as f32, height as f32));
    for id in ["character", "paragraph", "align", "glyphs"] {
        h.tap(id);
    }
    h.drag(h.tab("glyphs").center(), h.tab("paragraph").right_center() - vec2(3.0, 0.0));
    assert_eq!(h.groups().len(), 3, "three cards: {:?}", h.groups());
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
        app.begin_host_iteration();
        let mut out = ctx.run_ui(raw, |ui| {
            app.tick(ui.ctx());
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
    image::save_buffer(root.join("panels3-cards.png"), &pixels, width, height, image::ColorType::Rgba8).unwrap();
}
