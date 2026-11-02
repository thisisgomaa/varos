//! Floating collapse uses the arrow, preserves its visible origin, and remains distinct from X.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{State, model};
use crate::VectorcraftApp;
use egui::{Event, Pos2, Rect, vec2};
use model::{Action, Edge, Layout, Mode, Source, Target};
use serde_json::json;
use vectorcraft_engine::Session;

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

impl Harness {
    fn new() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("ui.panelReview", json!({"enabled":true})).unwrap();
        app.ui.varos_panel_review = Some(State { layout: Layout::new_slice(), ..Default::default() });
        app.run("file.new", json!({"width":300,"height":300})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self { app, ctx, time: 0.0 };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = egui::RawInput {
            time: Some(self.time),
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))),
            focused: true,
            ..Default::default()
        };
        let app = &mut self.app;
        app.begin_host_iteration();
        let mut output = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        output.textures_delta.clear();
    }

    fn settle(&mut self) {
        for _ in 0..5 {
            self.frame(vec![]);
        }
    }

    fn state(&self) -> &State {
        self.app.ui.varos_panel_review.as_ref().unwrap()
    }

    fn layout(&self) -> &Layout {
        &self.state().layout
    }

    fn host(&self, panel: &str) -> u64 {
        self.layout().owner(panel).unwrap().id
    }

    fn group(&self, panel: &str) -> u64 {
        self.layout().group_of(panel).unwrap().id
    }

    fn mode(&self, panel: &str) -> Mode {
        self.layout().column_mode(self.group(panel)).unwrap()
    }

    fn hit(&self, name: &str) -> Rect {
        self.state().hits.iter().find_map(|(key, rect)| (key == name).then_some(*rect)).unwrap_or_else(|| panic!("missing drawn hit {name}"))
    }

    fn rect(&self, panel: &str) -> Rect {
        self.hit(&format!("host:{}", self.host(panel)))
    }

    fn action(&mut self, action: Action) {
        self.app.run("ui.panelReview.action", json!({"action":action})).unwrap();
        self.settle();
    }

    fn click(&mut self, pos: Pos2) {
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![button(pos, true)]);
        self.frame(vec![button(pos, false)]);
        self.settle();
    }

    fn toggle(&mut self, panel: &str) {
        let key = self.layout().column_key(self.group(panel)).unwrap();
        self.click(self.hit(&format!("column-mode:{key}")).center());
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for step in 1..=8 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (step as f32 / 8.0))]);
        }
        assert!(self.state().gesture.is_some(), "the visible edge begins the resize gesture");
        self.frame(vec![Event::PointerMoved(to), button(to, false)]);
        self.settle();
        assert!(self.state().gesture.is_none());
    }

    fn assert_owners(&self) {
        self.layout().validate().unwrap();
        let mut owners: Vec<_> =
            self.layout().hosts.iter().flat_map(|host| model::groups(&host.root)).flat_map(|g| g.panels.iter().map(String::as_str)).collect();
        owners.sort_unstable();
        assert_eq!(owners, ["layers", "swatches"]);
    }
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1.1, "expected {expected}, got {actual}");
}

#[test]
fn floating_arrow_uses_icons_even_after_names_and_preserves_a_clamped_visible_origin() {
    let mut h = Harness::new();
    let host = h.host("layers");
    let group = h.group("layers");
    h.action(Action::SetColumnMode { group, mode: Mode::Icons });
    h.action(Action::ResizeColumn { group, width: 160.0 });
    assert_eq!(h.mode("layers"), Mode::Names);
    h.action(Action::SetColumnMode { group, mode: Mode::Full });
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [1550.0, 900.0] } });
    let full = h.rect("layers");
    assert!(full.left() < 1550.0 && full.top() < 900.0, "the saved position is outside the visible full-size extent");
    let expanded = h.layout().host(host).unwrap().expanded;
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Icons, "the floating arrow always goes to icons, never the previously remembered name list");
    near(h.rect("layers").width(), 44.0);
    near(h.rect("layers").left(), full.left());
    near(h.rect("layers").top(), full.top());
    assert_eq!(h.layout().host(host).unwrap().dock, None);
    assert!(!h.layout().host(host).unwrap().hidden);
    assert_eq!(h.layout().host(host).unwrap().expanded, expanded);
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Full);
    near(h.rect("layers").left(), full.left());
    near(h.rect("layers").top(), full.top());
    near(h.rect("layers").width(), full.width());
    near(h.rect("layers").height(), full.height());
    h.assert_owners();
}

#[test]
fn floating_full_resize_cannot_collapse_but_compact_resize_and_icon_peek_still_work() {
    let mut h = Harness::new();
    let host = h.host("layers");
    let group = h.group("layers");
    h.action(Action::Hide { host: h.host("swatches") });
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [400.0, 200.0] } });
    let from = h.hit(&format!("resize-right:{host}")).center();
    h.drag(from, from - vec2(250.0, 0.0));
    assert_eq!(h.mode("layers"), Mode::Full, "a full panel edge resize must stop at its minimum instead of changing presentation");
    assert!(h.rect("layers").width() >= 239.0);
    assert!(h.state().flyout.is_none());
    let full = h.rect("layers");
    let expanded = h.layout().host(host).unwrap().expanded;
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Icons);
    let compact = h.rect("layers");
    near(compact.left(), full.left());
    near(compact.top(), full.top());
    let icon = h.hit("tab:layers");
    h.click(icon.center());
    assert_eq!(h.state().flyout, Some(group));
    assert_eq!(h.mode("layers"), Mode::Icons);
    h.click(h.hit(&format!("flyout-close:{group}")).center());
    assert!(h.state().flyout.is_none());
    assert_eq!(h.rect("layers"), compact, "dismissing the peek keeps the icon at its floating anchor");
    let from = h.hit(&format!("resize-right:{host}")).center();
    h.drag(from, from + vec2(116.0, 0.0));
    assert_eq!(h.mode("layers"), Mode::Names, "edge resizing remains available inside the already-compact state");
    near(h.rect("layers").width(), 160.0);
    assert_eq!(h.layout().host(host).unwrap().expanded, expanded);
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Full);
    near(h.rect("layers").width(), full.width());
    near(h.rect("layers").height(), full.height());
    near(h.rect("layers").left(), full.left());
    near(h.rect("layers").top(), full.top());
    h.assert_owners();
}

#[test]
fn floating_x_hides_the_whole_mixed_block_and_window_restores_its_exact_modes_and_position() {
    let mut h = Harness::new();
    let host = h.host("layers");
    h.action(Action::Move { source: Source::Panel("swatches".into()), target: Target::Attach { host, edge: Edge::Right } });
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [400.0, 180.0] } });
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Icons);
    assert_eq!(h.mode("swatches"), Mode::Full);
    let before = h.layout().host(host).unwrap().clone();
    let rect = h.rect("layers");
    h.click(h.hit(&format!("close:{host}")).center());
    assert!(h.layout().owner("layers").unwrap().hidden);
    assert!(h.layout().owner("swatches").unwrap().hidden);
    assert_eq!(h.host("swatches"), host);
    assert!(h.state().hits.iter().all(|(name, _)| name != "tab:layers" && name != "tab:swatches"));
    h.app.run("window.panel", json!({"panel":"layers"})).unwrap();
    h.settle();
    assert_eq!(h.layout().host(host), Some(&before));
    assert_eq!(h.rect("layers"), rect);
    assert_eq!(h.mode("layers"), Mode::Icons);
    assert_eq!(h.mode("swatches"), Mode::Full);
    h.assert_owners();
}
