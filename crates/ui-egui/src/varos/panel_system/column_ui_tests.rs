//! Independent dock-column interaction through the real frame; native acceptance is separate.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{State, geometry, model, registry};
use crate::VectorcraftApp;
use egui::{Event, Pos2, Rect, pos2, vec2};
use model::{Action, Edge, Layout, Mode, Source, Target};
use serde_json::json;
use vectorcraft_engine::Session;

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
    workspace: Rect,
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

impl Harness {
    fn new(edge: Edge) -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("ui.panelReview", json!({"enabled":true})).unwrap();
        app.run("file.new", json!({"width":300,"height":300})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self { app, ctx, time: 0.0, workspace: Rect::NOTHING };
        h.settle();
        let control = h.host_rect("control");
        let tools = h.host_rect("tools");
        h.workspace = Rect::from_min_max(control.min, pos2(control.right(), tools.bottom()));
        h.action(Action::Hide { host: h.host_id("swatches") });
        let layers = h.group_id("layers");
        h.action(Action::Move { source: Source::Panel("artboards".into()), target: Target::Split { group: layers, edge: Edge::Top } });
        let host = h.host_id("layers");
        h.action(Action::Move { source: Source::Panel("blend".into()), target: Target::Attach { host, edge: Edge::Left } });
        h.action(Action::Move { source: Source::Host(host), target: Target::Dock { edge } });
        h.assert_owners();
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
        let mut out = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
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

    fn action(&mut self, action: Action) {
        self.app.run("ui.panelReview.action", json!({"action":action})).unwrap();
        self.settle();
    }

    fn hit(&self, name: &str) -> Rect {
        self.state().hits.iter().find_map(|(key, rect)| (key == name).then_some(*rect)).unwrap_or_else(|| panic!("missing drawn hit {name}"))
    }

    fn host_id(&self, panel: &str) -> u64 {
        self.layout().owner(panel).unwrap().id
    }

    fn host_rect(&self, panel: &str) -> Rect {
        self.hit(&format!("host:{}", self.host_id(panel)))
    }

    fn group_id(&self, panel: &str) -> u64 {
        self.layout().group_of(panel).unwrap().id
    }

    fn group_rect(&self, panel: &str) -> Rect {
        let group = self.group_id(panel);
        geometry::measure(self.layout(), self.workspace).into_iter().flat_map(|host| host.groups).find(|g| g.id == group).unwrap().rect
    }

    fn mode(&self, panel: &str) -> Mode {
        self.layout().column_mode(self.group_id(panel)).unwrap()
    }

    fn key(&self, panel: &str) -> u64 {
        self.layout().column_key(self.group_id(panel)).unwrap()
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.settle();
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for step in 1..=8 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (step as f32 / 8.0))]);
        }
        assert!(self.state().gesture.is_some(), "drawn column handle must begin a gesture: {from:?} → {to:?}");
        self.frame(vec![Event::PointerMoved(to), button(to, false)]);
        self.settle();
        assert!(self.state().gesture.is_none());
    }

    fn toggle(&mut self, panel: &str) {
        self.click(self.hit(&format!("column-mode:{}", self.key(panel))).center());
    }

    fn assert_owners(&self) {
        self.layout().validate().unwrap();
        let mut actual: Vec<_> =
            self.layout().hosts.iter().flat_map(|h| model::groups(&h.root)).flat_map(|g| g.panels.iter().map(String::as_str)).collect();
        let mut expected: Vec<_> = registry::BUILTINS.iter().map(|p| p.id).collect();
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected, "every panel retains exactly one owner through collapse, resize and flyout actions");
    }
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1.1, "expected {expected}, got {actual}");
}

#[test]
fn column_arrow_collapses_only_its_column_and_expansion_restores_the_saved_width() {
    for edge in [Edge::Left, Edge::Right] {
        let mut h = Harness::new(edge);
        let owner = h.host_id("blend");
        let blend = h.group_rect("blend");
        let artboards = h.group_rect("artboards");
        let layers = h.group_rect("layers");
        let host = h.host_rect("blend");
        assert_ne!(h.key("blend"), h.key("artboards"));
        assert_eq!(h.key("artboards"), h.key("layers"), "vertical groups share a physical column");
        for _ in 0..3 {
            h.toggle("blend");
            assert_eq!(h.mode("blend"), Mode::Icons);
            assert_eq!(h.mode("artboards"), Mode::Full);
            assert_eq!(h.mode("layers"), Mode::Full);
            assert_eq!(h.host_id("blend"), owner, "collapse keeps the same dock owner");
            assert_eq!(h.layout().owner("blend").unwrap().dock, Some(edge));
            assert!(!h.layout().owner("blend").unwrap().hidden);
            assert!(h.group_rect("blend").width() < 60.0, "compact column must release its full-width reservation");
            near(h.group_rect("artboards").width(), artboards.width());
            near(h.group_rect("layers").width(), layers.width());
            near(h.group_rect("artboards").height(), artboards.height());
            near(h.group_rect("layers").height(), layers.height());
            if edge == Edge::Right {
                near(h.group_rect("artboards").left(), artboards.left());
                near(h.group_rect("layers").right(), layers.right());
            } else {
                near(h.host_rect("blend").left(), host.left());
            }
            h.toggle("blend");
            assert_eq!(h.mode("blend"), Mode::Full);
            near(h.group_rect("blend").width(), blend.width());
            near(h.host_rect("blend").width(), host.width());
            near(h.group_rect("artboards").left(), artboards.left());
            h.assert_owners();
        }
    }
}

#[test]
fn compact_icon_flyout_closes_back_to_its_anchor_without_collapsing_the_neighbor() {
    let mut h = Harness::new(Edge::Right);
    h.toggle("blend");
    let group = h.group_id("blend");
    let before = h.layout().clone();
    let icon = h.hit("tab:blend");
    let other = h.group_rect("layers");
    for _ in 0..2 {
        h.click(icon.center());
        assert_eq!(h.state().flyout, Some(group));
        assert!(h.state().hits.iter().filter(|(name, _)| name == "tab:blend").count() >= 2, "compact anchor and full flyout are both drawn");
        let close = h.hit(&format!("flyout-close:{group}"));
        assert!(close.right() <= icon.left(), "right-docked flyout opens toward the workspace");
        h.click(close.center());
        assert!(h.state().flyout.is_none());
        assert_eq!(h.layout(), &before, "dismissing contents does not move, hide or reparent the compact column");
        assert_eq!(h.mode("layers"), Mode::Full);
        assert_eq!(h.hit("tab:blend"), icon);
        assert_eq!(h.group_rect("layers"), other);
    }
    h.assert_owners();
}

#[test]
fn compact_column_edge_switches_icon_and_name_modes_without_resizing_the_full_neighbor() {
    let mut h = Harness::new(Edge::Right);
    h.toggle("blend");
    let other = h.group_rect("layers");
    let key = h.key("blend");
    let from = h.hit(&format!("column-resize-left:{key}")).center();
    h.drag(from, from - vec2(96.0, 0.0));
    assert_eq!(h.mode("blend"), Mode::Names);
    assert!(h.group_rect("blend").width() > 100.0);
    near(h.group_rect("layers").width(), other.width());
    near(h.group_rect("layers").left(), other.left());
    let from = h.hit(&format!("column-resize-left:{key}")).center();
    h.drag(from, from + vec2(140.0, 0.0));
    assert_eq!(h.mode("blend"), Mode::Icons);
    assert!(h.group_rect("blend").width() < 60.0);
    near(h.group_rect("layers").width(), other.width());
    h.assert_owners();
}

#[test]
fn stacked_group_collapse_and_workspace_roundtrip_preserve_independent_column_states() {
    let mut h = Harness::new(Edge::Right);
    let group = h.group_id("layers");
    h.app.run("window.panel.collapse", json!({"panel":"layers","collapsed":true})).unwrap();
    h.settle();
    assert_eq!(h.mode("layers"), Mode::Icons);
    assert_eq!(h.mode("artboards"), Mode::Icons);
    assert_eq!(h.mode("blend"), Mode::Full);
    assert_eq!(h.layout().column_key(group), Some(h.key("artboards")));
    let before = h.layout().clone();
    let restored: Layout = serde_json::from_slice(&serde_json::to_vec(&before).unwrap()).unwrap();
    restored.validate().unwrap();
    h.app.ui.varos_panel_review.as_mut().unwrap().layout = restored;
    h.settle();
    assert_eq!(h.layout(), &before);
    assert_eq!(h.mode("blend"), Mode::Full);
    assert_eq!(h.mode("layers"), Mode::Icons);
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Full);
    assert_eq!(h.mode("artboards"), Mode::Full);
    assert_eq!(h.mode("blend"), Mode::Full);
    h.assert_owners();
}

#[test]
fn vertical_divider_resizes_only_its_two_stacked_neighbors_and_horizontal_divider_keeps_total_width() {
    let mut h = Harness::new(Edge::Right);
    let host = h.host_id("blend");
    let measured = geometry::measure(h.layout(), h.workspace).into_iter().find(|candidate| candidate.id == host).unwrap();
    let vertical = measured.dividers.iter().find(|divider| divider.axis == model::Axis::Vertical).unwrap();
    let blend = h.group_rect("blend");
    let artboards = h.group_rect("artboards");
    let layers = h.group_rect("layers");
    let from = h.hit(&format!("divider:{}", vertical.id)).center();
    h.drag(from, from + vec2(0.0, 70.0));
    assert_eq!(h.group_rect("blend"), blend, "a row divider cannot resize the separate column");
    assert!(h.group_rect("artboards").height() > artboards.height() + 40.0);
    assert!(h.group_rect("layers").height() < layers.height() - 40.0);
    near(h.group_rect("artboards").width(), artboards.width());
    let measured = geometry::measure(h.layout(), h.workspace).into_iter().find(|candidate| candidate.id == host).unwrap();
    let horizontal = measured.dividers.iter().find(|divider| divider.axis == model::Axis::Horizontal).unwrap();
    let from = h.hit(&format!("divider:{}", horizontal.id)).center();
    let total = h.host_rect("blend");
    let blend = h.group_rect("blend");
    let layers = h.group_rect("layers");
    h.drag(from, from + vec2(60.0, 0.0));
    near(h.host_rect("blend").width(), total.width());
    assert!(h.group_rect("blend").width() > blend.width() + 20.0);
    assert!(h.group_rect("layers").width() < layers.width() - 20.0);
    near(h.group_rect("artboards").width(), h.group_rect("layers").width());
    h.assert_owners();
}

#[test]
fn divider_beside_a_compact_column_resizes_its_names_without_squeezing_the_full_neighbor() {
    let mut h = Harness::new(Edge::Right);
    let saved_blend_width = h.group_rect("blend").width();
    let artboards = h.group_rect("artboards");
    let layers = h.group_rect("layers");
    h.toggle("blend");
    assert_eq!(h.mode("blend"), Mode::Icons);
    let compact_width = h.group_rect("blend").width();
    let host = h.host_id("blend");
    let measured = geometry::measure(h.layout(), h.workspace).into_iter().find(|candidate| candidate.id == host).unwrap();
    let divider = measured.dividers.iter().find(|divider| divider.axis == model::Axis::Horizontal).unwrap();
    let from = h.hit(&format!("divider:{}", divider.id)).center();
    h.drag(from, from + vec2(80.0, 0.0));
    assert_eq!(h.mode("blend"), Mode::Names, "the boundary widens the compact column into its name rows");
    near(h.group_rect("blend").width(), compact_width + 80.0);
    assert_eq!(h.mode("artboards"), Mode::Full);
    assert_eq!(h.mode("layers"), Mode::Full);
    near(h.group_rect("artboards").width(), artboards.width());
    near(h.group_rect("layers").width(), layers.width());
    near(h.group_rect("artboards").height(), artboards.height());
    near(h.group_rect("layers").height(), layers.height());
    h.toggle("blend");
    assert_eq!(h.mode("blend"), Mode::Full);
    near(h.group_rect("blend").width(), saved_blend_width);
    near(h.group_rect("artboards").width(), artboards.width());
    near(h.group_rect("layers").width(), layers.width());
    h.assert_owners();
}

#[test]
fn dragging_an_independent_column_flyout_keeps_the_visible_preview_and_detached_panel_full() {
    let mut h = Harness::new(Edge::Right);
    h.toggle("blend");
    let source_host = h.host_id("blend");
    let group = h.group_id("blend");
    let neighbor = h.group_rect("layers");
    let icon = h.hit("tab:blend");
    h.click(icon.center());
    assert_eq!(h.state().flyout, Some(group));
    let full_tab = h.state().hits.iter().rev().find_map(|(name, rect)| (name == "tab:blend").then_some(*rect)).unwrap();
    assert!(full_tab.width() > icon.width());
    let before = h.layout().clone();
    let from = full_tab.center();
    let to = pos2(650.0, 650.0);
    h.frame(vec![Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    for step in 1..=8 {
        h.frame(vec![Event::PointerMoved(from + (to - from) * (step as f32 / 8.0))]);
    }
    assert!(h.state().gesture.is_some());
    assert_eq!(h.layout(), &before, "the held drag is only a preview");
    let preview_tab = h.hit("tab:blend");
    near(preview_tab.height(), super::paint::TABS);
    assert!(preview_tab.width() > icon.width(), "dragging the expanded flyout cannot flash back into an icon preview");
    assert!(h.state().hits.iter().any(|(name, _)| name == &format!("panel-menu:{group}")), "the full panel's content chrome remains drawn");
    h.frame(vec![Event::PointerMoved(to), button(to, false)]);
    h.settle();
    assert!(h.state().gesture.is_none());
    assert!(h.state().flyout.is_none());
    assert_eq!(h.mode("blend"), Mode::Full);
    assert_eq!(h.layout().owner("blend").unwrap().dock, None);
    assert_ne!(h.host_id("blend"), source_host);
    assert_eq!(h.host_id("layers"), source_host);
    assert_eq!(h.layout().owner("layers").unwrap().dock, Some(Edge::Right));
    assert_eq!(h.mode("layers"), Mode::Full);
    assert_eq!(h.group_rect("layers"), neighbor);
    near(h.hit("tab:blend").height(), super::paint::TABS);
    h.assert_owners();
}

/// Actual panel rendering for the review; pointer/native acceptance stays separate.
#[test]
#[ignore = "optional software visual capture; set VAROS_COLUMN_CAPTURE"]
fn capture_independent_dock_columns() {
    let Some(path) = std::env::var_os("VAROS_COLUMN_CAPTURE").map(std::path::PathBuf::from) else { return };
    let mut h = Harness::new(Edge::Right);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let save = |app: &mut VectorcraftApp, suffix: Option<&str>| {
        let pixels = super::ui_tests::software_capture(app, 1600, 1000);
        let output = suffix.map_or_else(|| path.clone(), |suffix| path.with_file_name(format!("{stem}-{suffix}.png")));
        image::save_buffer(output, &pixels, 1600, 1000, image::ColorType::Rgba8).unwrap();
    };
    save(&mut h.app, None);
    let group = h.group_id("blend");
    h.action(Action::SetColumnMode { group, mode: Mode::Icons });
    save(&mut h.app, Some("icons"));
    h.app.ui.varos_panel_review.as_mut().unwrap().flyout = Some(group);
    save(&mut h.app, Some("flyout"));
    h.app.ui.varos_panel_review.as_mut().unwrap().flyout = None;
    h.action(Action::ResizeColumn { group, width: 160.0 });
    save(&mut h.app, Some("names"));
}
