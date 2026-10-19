//! Several icon panels open at once ([`super::panel_stack`]), through real clicks on the dock's
//! icon column and the stacked cards.

use egui::{Rect, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;
use crate::varos::panel_stack;

const SCREEN: egui::Vec2 = vec2(1600.0, 1400.0);

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self { app, ctx, time: 0.0 };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.05;
        let screen = Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
        let input = egui::RawInput { time: Some(self.time), events, screen_rect: Some(screen), ..Default::default() };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| {
                crate::dock::show(app, ui);
                crate::dock::floating_panel(app, ui.ctx());
            })
            .textures_delta
            .clear();
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    fn click(&mut self, at: egui::Pos2) {
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(vec![egui::Event::PointerMoved(at), button(true)]);
        self.frame(vec![button(false)]);
        self.settle();
    }

    /// The icon of panel `id` in the column (the expanded dock's column holds the icon panels in
    /// `ICON_PANEL_GROUPS` order).
    fn icon(&self, id: &str) -> Rect {
        let left = self.ctx.data(|d| d.get_temp::<f32>(egui::Id::new("dock-icon-column-left"))).unwrap();
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
        assert_eq!(icons.len(), order.len(), "one icon per icon panel");
        icons[order.iter().position(|p| *p == id).expect("an icon panel")]
    }

    /// Click panel `id`'s icon where it is drawn now (the column can shift as panels open).
    fn tap(&mut self, id: &str) {
        let at = self.icon(id).center();
        self.click(at);
    }

    fn open(&self) -> Vec<&'static str> {
        panel_stack::shown(&self.app)
    }

    fn stack_rect(&self) -> Option<Rect> {
        self.ctx.memory(|m| m.area_rect(egui::Id::new("varos-panel-stack")))
    }
}

/// Character then Paragraph from their icons: both stay open, stacked (newest below), each icon
/// lit; Character's icon again puts Character away and leaves Paragraph.
#[test]
fn opening_a_second_panel_keeps_the_first_open() {
    let mut h = Harness::new();
    assert!(h.app.ui.varos_panel_stack.keep_open, "keep-open is the Varos default");
    h.tap("character");
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("character"));
    h.tap("paragraph");
    assert_eq!(h.open(), ["character", "paragraph"], "both open, the newest below");
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("paragraph"), "the newest is upstream's open panel");
    assert!(panel_stack::is_open(&h.app, "character") && panel_stack::is_open(&h.app, "paragraph"), "both icons lit");
    let stack = h.stack_rect().expect("the stack is drawn");
    let column = h.ctx.data(|d| d.get_temp::<f32>(egui::Id::new("dock-icon-column-left"))).unwrap();
    assert!(stack.right() <= column && stack.height() > 2.0 * 26.0, "beside the column, two cards tall: {stack:?}");
    assert_eq!(crate::menus::checked(&h.app, "window.panel", &json!({"panel": "character"})), Some(true), "Window menu ticks both");
    h.tap("character");
    assert_eq!(h.open(), ["paragraph"], "one left: upstream's single flyout shows it");
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("paragraph"), "Paragraph stays open");
    assert!(!panel_stack::is_open(&h.app, "character"));
}

/// Each card's » puts that panel away only.
#[test]
fn a_cards_close_button_closes_only_that_panel() {
    let mut h = Harness::new();
    for id in ["character", "paragraph", "glyphs"] {
        h.tap(id);
    }
    assert_eq!(h.open(), ["character", "paragraph", "glyphs"]);
    // The top card is Character's: its » is 13 points in from the stack's top right.
    let stack = h.stack_rect().unwrap();
    h.click(egui::pos2(stack.right() - 13.0, stack.top() + 13.0));
    assert_eq!(h.open(), ["paragraph", "glyphs"], "Character went, the others stayed");
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("glyphs"));
    // Closing the newest leaves the one before it as upstream's open panel.
    h.tap("glyphs");
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("paragraph"));
}

/// Keep-open off, or Auto-Collapse Iconic Panels on: upstream's one flyout at a time.
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
        assert!(h.open().is_empty());
    }
}

/// The open set and the mode are saved with the UI state (`ui.json`).
#[test]
fn the_open_panels_are_saved_with_the_ui_state() {
    let mut h = Harness::new();
    for id in ["character", "paragraph"] {
        h.tap(id);
    }
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    assert_eq!(saved["varos_panel_stack"]["open"], json!(["character", "paragraph"]));
    let restored: crate::state::UiState = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.varos_panel_stack.open, ["character", "paragraph"]);
    assert!(restored.varos_panel_stack.keep_open);
    // Older profiles without the field get the default: keep-open on, nothing stacked.
    let mut old = serde_json::to_value(crate::state::UiState::default()).unwrap();
    old.as_object_mut().unwrap().remove("varos_panel_stack");
    let restored: crate::state::UiState = serde_json::from_value(old).unwrap();
    assert!(restored.varos_panel_stack.keep_open && restored.varos_panel_stack.open.is_empty());
}

/// A panel floated out of the stack leaves it; a workspace applied starts the stack again.
#[test]
fn floating_or_a_workspace_takes_panels_out_of_the_stack() {
    let mut h = Harness::new();
    for id in ["character", "paragraph", "glyphs"] {
        h.tap(id);
    }
    // Upstream puts its one flyout away on any panel-layout change; floating the newest panel
    // takes just that one out of the stack.
    h.app.run("window.panel.float", json!({"panel": "glyphs"})).unwrap();
    h.settle();
    assert_eq!(h.open(), ["character", "paragraph"]);
    h.app.run("window.workspace", json!({"name": crate::workspaces::ESSENTIALS})).unwrap();
    h.settle();
    assert!(h.open().len() <= 1, "the workspace's own panel at most: {:?}", h.open());
}
