//! Pointer integration through the real app frame and the Layers/Swatches content.
//! These headless checks establish event/layout contracts, not native visual acceptance.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{State, model};
use crate::VectorcraftApp;
use egui::{Event, Pos2, Rect, pos2, vec2};
use model::{Layout, Mode};
use serde_json::json;
use vectorcraft_engine::Session;

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
    focused: bool,
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

impl Harness {
    fn new() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("ui.panelReview", json!({"enabled": true})).unwrap();
        app.ui.varos_panel_review = Some(State::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self { app, ctx, time: 0.0, focused: true };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 0.05;
        let input = egui::RawInput {
            time: Some(self.time),
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))),
            focused: self.focused,
            ..Default::default()
        };
        let app = &mut self.app;
        app.begin_host_iteration();
        let mut out = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
        out
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    fn state(&self) -> &State {
        self.app.ui.varos_panel_review.as_ref().unwrap()
    }

    fn layout(&self) -> &Layout {
        &self.state().layout
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

    fn tab(&self, panel: &str) -> Rect {
        self.hit(&format!("tab:{panel}"))
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at), button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.settle();
    }

    fn hold_drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for step in 1..=8 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (step as f32 / 8.0))]);
        }
        assert!(self.state().gesture.is_some(), "the drawn handle must begin a pointer drag");
    }

    fn release(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at), button(at, false)]);
        self.settle();
        assert!(self.state().gesture.is_none(), "pointer-up finishes the gesture");
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.hold_drag(from, to);
        self.release(to);
    }

    fn merge_tabs(&mut self) {
        let destination = self.tab("layers");
        self.drag(self.tab("swatches").center(), pos2(destination.left() + 20.0, destination.center().y));
        assert_eq!(self.layout().hosts.len(), 1);
        assert_eq!(self.layout().group_of("layers").unwrap().panels, ["swatches", "layers"]);
        self.assert_owners();
    }

    fn assert_owners(&self) {
        self.layout().validate().unwrap();
        let mut panels: Vec<&str> =
            self.layout().hosts.iter().flat_map(|host| model::groups(&host.root)).flat_map(|g| g.panels.iter().map(String::as_str)).collect();
        panels.sort_unstable();
        assert_eq!(panels, ["layers", "swatches"], "each actual panel has exactly one owner, including hidden blocks");
    }

    fn tools_workspace(dock: Option<model::Edge>) -> Self {
        let mut h = Self::new();
        h.app.ui.varos_panel_review = Some(State { layout: Layout::new_workspace(), ..Default::default() });
        let host = h.host_id("tools");
        let target = dock.map_or(model::Target::Float { position: [900.0, 120.0] }, |edge| model::Target::Dock { edge });
        h.app.run("ui.panelReview.action", json!({"action": model::Action::Move { source: model::Source::Host(host), target }})).unwrap();
        h.settle();
        h
    }

    fn assert_tools_visible(&self, width: f32) {
        let host = self.layout().owner("tools").unwrap();
        assert_eq!(host.mode, Mode::Full, "Tools never becomes a compact panel icon or name row");
        assert!(!host.hidden);
        assert!(self.state().flyout.is_none(), "Tools content stays in its own host");
        let rect = self.host_rect("tools");
        assert!((rect.width() - width).abs() < 0.6, "expected Tools width {width}, got {}", rect.width());
        let body = self.ctx.data(|d| d.get_temp::<Rect>(crate::edit_toolbar::toolbar_id())).unwrap();
        assert!(rect.contains_rect(body), "the real Tools body is hosted inside the current frame");
        // Tool slots precede the bottom controls. Edit Toolbar is also 28×28, but
        // its centered position is not a third tool column in the two-column layout.
        let slots = crate::toolbar::slots(&self.app).len();
        let buttons: Vec<_> = crate::toolbar::tests::widget_rects(&self.ctx, egui::Vec2::splat(super::metrics::TOOL_BUTTON))
            .into_iter()
            .filter(|button| button.left() >= body.left() && button.right() <= body.right() && button.top() >= body.top())
            .take(slots)
            .collect();
        assert_eq!(buttons.len(), slots, "all real tool slots are laid out at width {width}");
        assert!(buttons.iter().filter(|button| rect.contains_rect(**button)).count() >= 2, "real tool controls remain visible at width {width}");
        if width == 44.0 || width == 80.0 {
            let mut columns: Vec<_> = buttons.iter().map(Rect::left).collect();
            columns.sort_by(f32::total_cmp);
            columns.dedup_by(|a, b| (*a - *b).abs() < 0.1);
            assert_eq!(columns.len(), if width == 44.0 { 1 } else { 2 });
        }
        self.layout().validate().unwrap();
        let mut actual: Vec<_> =
            self.layout().hosts.iter().flat_map(|h| model::groups(&h.root)).flat_map(|g| g.panels.iter().map(String::as_str)).collect();
        let mut expected: Vec<_> = super::registry::BUILTINS.iter().map(|p| p.id).collect();
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected, "width changes preserve every panel owner");
    }
}

fn assert_near(actual: Pos2, expected: Pos2) {
    assert!((actual - expected).length() < 0.6, "expected {expected:?}, got {actual:?}");
}

#[test]
fn tools_title_arrow_repeatedly_switches_columns_without_collapsing_or_moving_owners() {
    for dock in [Some(model::Edge::Left), Some(model::Edge::Right), None] {
        let mut h = Harness::tools_workspace(dock);
        let original = h.layout().owner("tools").unwrap().clone();
        let original_rect = h.host_rect("tools");
        h.assert_tools_visible(80.0);
        for width in [44.0, 80.0, 44.0, 80.0] {
            h.click(h.hit(&format!("mode:{}", original.id)).center());
            h.assert_tools_visible(width);
            let host = h.layout().owner("tools").unwrap();
            assert_eq!(host.id, original.id);
            assert_eq!(host.root, original.root);
            assert_eq!(host.dock, dock);
            assert_eq!(host.position, original.position, "column toggles preserve the stored floating origin");
            assert_eq!(host.expanded[1], original.expanded[1], "column toggles preserve the remembered height");
            let rect = h.host_rect("tools");
            assert!((rect.height() - original_rect.height()).abs() < 0.6);
            if dock == Some(model::Edge::Right) {
                assert_near(rect.right_top(), original_rect.right_top());
            } else {
                assert_near(rect.min, original_rect.min);
            }
        }
    }
}

#[test]
fn tools_pointer_edges_clamp_width_and_keep_the_opposite_edge_anchored() {
    for (dock, left) in [(Some(model::Edge::Left), false), (Some(model::Edge::Right), true), (None, false), (None, true)] {
        let mut h = Harness::tools_workspace(dock);
        let original = h.layout().owner("tools").unwrap().clone();
        for (delta_width, expected) in [(-120.0, 44.0), (18.0, 62.0), (120.0, 80.0)] {
            let before = h.host_rect("tools");
            let from = h.hit(&format!("resize-{}:{}", if left { "left" } else { "right" }, original.id)).center();
            h.drag(from, from + vec2(if left { -delta_width } else { delta_width }, 0.0));
            h.assert_tools_visible(expected);
            let after = h.host_rect("tools");
            assert_near(if left { after.right_top() } else { after.left_top() }, if left { before.right_top() } else { before.left_top() });
            assert!((after.height() - before.height()).abs() < 0.6);
            let host = h.layout().owner("tools").unwrap();
            assert_eq!(host.id, original.id);
            assert_eq!(host.root, original.root);
            assert_eq!(host.dock, dock);
            assert_eq!(host.expanded[1], original.expanded[1]);
        }
    }
}

#[test]
fn mixed_tools_host_outer_edges_resize_only_the_column_they_touch() {
    let mut h = Harness::tools_workspace(Some(model::Edge::Left));
    let tools = h.host_id("tools");
    let layers = h.host_id("layers");
    h.app
        .run(
            "ui.panelReview.action",
            json!({"action": model::Action::Move {
                source: model::Source::Host(tools),
                target: model::Target::Attach { host: layers, edge: model::Edge::Left },
            }}),
        )
        .unwrap();
    h.app
        .run(
            "ui.panelReview.action",
            json!({"action": model::Action::Move {
                source: model::Source::Host(layers),
                target: model::Target::Float { position: [900.0, 120.0] },
            }}),
        )
        .unwrap();
    h.settle();
    assert_eq!(h.host_id("tools"), layers);
    assert!((h.layout().tools_width().unwrap() - 80.0).abs() < 0.01);
    let owners: Vec<_> =
        super::registry::BUILTINS.iter().map(|panel| (panel.id, h.host_id(panel.id), h.layout().group_of(panel.id).unwrap().clone())).collect();
    let before = h.host_rect("tools");
    let ordinary_group = h.layout().group_of("layers").unwrap().id;
    let ordinary_width = h.layout().expanded_group_size(ordinary_group).unwrap()[0];
    let from = h.hit(&format!("resize-left:{layers}")).center();
    let to = from - vec2(120.0, 0.0);
    h.hold_drag(from, to);
    assert_near(h.host_rect("tools").right_top(), before.right_top());
    h.release(to);
    let after = h.host_rect("tools");
    assert!((after.width() - before.width()).abs() < 0.6, "the Tools edge stops at its own maximum; it cannot enlarge its ordinary neighbor");
    assert_near(after.right_top(), before.right_top());
    assert_near(after.left_top(), before.left_top());
    assert!((after.height() - before.height()).abs() < 0.6);
    assert!((h.layout().tools_width().unwrap() - 80.0).abs() < 0.01);
    assert!((h.layout().expanded_group_size(ordinary_group).unwrap()[0] - ordinary_width).abs() < 0.6);

    let from = h.hit(&format!("resize-right:{layers}")).center();
    let to = from + vec2(100.0, 0.0);
    h.hold_drag(from, to);
    assert_near(h.host_rect("tools").left_top(), before.left_top());
    h.release(to);
    let grown = h.host_rect("tools");
    assert_near(grown.left_top(), before.left_top());
    assert!((grown.width() - before.width() - 100.0).abs() < 1.1, "the ordinary outer edge grows its own column");
    assert!((h.layout().expanded_group_size(ordinary_group).unwrap()[0] - ordinary_width - 100.0).abs() < 1.1);
    assert!((h.layout().tools_width().unwrap() - 80.0).abs() < 0.01, "growing the adjacent column does not resize Tools");
    assert!((grown.height() - before.height()).abs() < 0.6);
    assert_eq!(h.layout().owner("tools").unwrap().mode, Mode::Full);
    let body = h.ctx.data(|d| d.get_temp::<Rect>(crate::edit_toolbar::toolbar_id())).unwrap();
    assert!(body.width() + model::HOST_CHROME[0] <= 80.6, "painted Tools also stays within its width limit");
    assert!(
        crate::toolbar::tests::widget_rects(&h.ctx, egui::Vec2::splat(super::metrics::TOOL_BUTTON)).iter().any(|button| body.contains_rect(*button))
    );
    for (panel, owner, group) in owners {
        assert_eq!(h.host_id(panel), owner, "resizing preserves {panel}'s host");
        assert_eq!(h.layout().group_of(panel), Some(&group), "resizing preserves {panel}'s group");
    }
    h.layout().validate().unwrap();
}

#[test]
fn floating_tools_has_no_name_or_menu_and_arrow_keeps_both_tool_widths() {
    let mut h = Harness::tools_workspace(None);
    let group = h.layout().group_of("tools").unwrap().id;
    let host = h.host_id("tools");
    for width in [80.0, 44.0, 80.0] {
        assert!(!h.state().hits.iter().any(|(name, _)| name == "tab:tools" || name == &format!("panel-menu:{group}")));
        h.assert_tools_visible(width);
        h.click(h.hit(&format!("mode:{host}")).center());
    }
}

#[test]
fn pointer_tabs_merge_reorder_and_detach_only_on_release() {
    let mut h = Harness::new();
    let before = h.layout().clone();
    let destination = h.tab("layers");
    let at = pos2(destination.left() + 20.0, destination.center().y);
    h.hold_drag(h.tab("swatches").center(), at);
    assert_eq!(h.layout(), &before, "drag preview cannot mutate committed ownership");
    h.release(at);
    assert_eq!(h.layout().hosts.len(), 1);
    assert_eq!(h.layout().group_of("layers").unwrap().panels, ["swatches", "layers"]);

    let group = h.layout().group_of("layers").unwrap().id;
    let destination = h.tab("layers");
    h.drag(h.tab("swatches").center(), pos2(destination.right() - 10.0, destination.center().y));
    assert_eq!(h.layout().group(group).unwrap().panels, ["layers", "swatches"]);

    let before = h.layout().clone();
    let host_rect = h.host_rect("swatches");
    let from = h.tab("swatches").center();
    let to = pos2(1050.0, 570.0);
    h.hold_drag(from, to);
    assert_eq!(h.layout(), &before);
    h.release(to);
    assert_eq!(h.layout().hosts.len(), 2);
    assert_ne!(h.host_id("layers"), h.host_id("swatches"));
    assert_eq!(h.layout().group(group).unwrap().panels, ["layers"]);
    assert_near(h.host_rect("swatches").min, host_rect.min + (to - from));
    h.assert_owners();
}

#[test]
fn dark_header_moves_the_whole_host_without_a_release_jump() {
    let mut h = Harness::new();
    let id = h.host_id("layers");
    let before = h.layout().clone();
    let rect = h.host_rect("layers");
    let from = h.hit(&format!("host-grip:{id}")).center();
    let to = pos2(1060.0, 540.0);
    h.hold_drag(from, to);
    assert_eq!(h.layout(), &before);
    let preview = h.hit(&format!("host:{id}"));
    assert_near(preview.min, rect.min + (to - from));
    h.release(to);
    assert_eq!(h.host_id("layers"), id);
    assert_near(h.host_rect("layers").min, preview.min);
    assert_eq!(h.layout().host(id).unwrap().root, before.host(id).unwrap().root);
    h.assert_owners();
}

#[test]
fn escape_cancels_a_pending_tab_detach_without_losing_panels() {
    let mut h = Harness::new();
    h.merge_tabs();
    let before = h.layout().clone();
    let to = pos2(1050.0, 570.0);
    h.hold_drag(h.tab("swatches").center(), to);
    h.frame(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
    assert!(h.state().gesture.is_none());
    assert_eq!(h.layout(), &before);
    h.release(to);
    assert_eq!(h.layout(), &before, "a later pointer-up cannot commit the cancelled operation");
    h.assert_owners();
}

#[test]
fn losing_focus_cancels_a_host_drag_without_committing_the_preview() {
    let mut h = Harness::new();
    let before = h.layout().clone();
    let id = h.host_id("layers");
    let to = pos2(1050.0, 570.0);
    h.hold_drag(h.hit(&format!("host-grip:{id}")).center(), to);
    h.focused = false;
    h.frame(vec![]);
    assert!(h.state().gesture.is_none());
    assert_eq!(h.layout(), &before);
    h.focused = true;
    h.release(to);
    assert_eq!(h.layout(), &before);
    h.assert_owners();
}

#[test]
fn compact_edges_switch_icon_and_name_modes_and_click_opens_a_flyout() {
    let mut h = Harness::new();
    let id = h.host_id("layers");
    let original = h.layout().host(id).unwrap().clone();
    h.click(h.hit(&format!("mode:{id}")).center());
    assert_eq!(h.layout().host(id).unwrap().mode, Mode::Icons);
    let compact = h.host_rect("layers");
    let from = h.hit(&format!("resize-right:{id}")).center();
    h.drag(from, from + vec2(130.0, 0.0));
    assert_eq!(h.layout().host(id).unwrap().mode, Mode::Names);
    assert_near(h.host_rect("layers").min, compact.min);
    assert!((h.host_rect("layers").width() - compact.width() - 130.0).abs() < 0.6);
    assert_eq!(h.layout().host(id).unwrap().expanded, original.expanded);
    assert_eq!(h.layout().host(id).unwrap().root, original.root);

    let named = h.host_rect("layers");
    let from = h.hit(&format!("resize-left:{id}")).center();
    h.drag(from, from + vec2(130.0, 0.0));
    assert_eq!(h.layout().host(id).unwrap().mode, Mode::Icons);
    assert!((h.host_rect("layers").right() - named.right()).abs() < 0.6, "left-edge resize anchors the opposite edge");
    assert!((h.host_rect("layers").width() - compact.width()).abs() < 0.6);

    let group = h.layout().group_of("layers").unwrap().id;
    h.click(h.tab("layers").center());
    assert_eq!(h.state().flyout, Some(group), "activating the compact icon must keep the flyout open");
    assert_eq!(
        h.state().hits.iter().filter(|(name, _)| name == "tab:layers").count(),
        2,
        "both the compact anchor and its expanded flyout are drawn"
    );
    assert_eq!(h.layout().host(id).unwrap().mode, Mode::Icons);
    h.assert_owners();
}

#[test]
fn close_hides_the_whole_block_and_window_restores_its_layout() {
    let mut h = Harness::new();
    h.merge_tabs();
    let id = h.host_id("layers");
    let original = h.layout().host(id).unwrap().clone();
    h.click(h.hit(&format!("close:{id}")).center());
    assert!(h.layout().host(id).unwrap().hidden);
    assert!(h.state().hits.iter().all(|(name, _)| !name.starts_with("tab:")));
    assert_eq!(h.layout().host(id).unwrap().root, original.root);
    h.assert_owners();

    h.app.run("window.panel", json!({"panel": "swatches"})).unwrap();
    h.settle();
    let restored = h.layout().host(id).unwrap();
    assert!(!restored.hidden);
    assert_eq!(restored.position, original.position);
    assert_eq!(restored.expanded, original.expanded);
    assert_eq!(restored.root, original.root);
    assert_eq!(h.layout().group_of("swatches").unwrap().active, "swatches");
    assert_eq!(h.host_id("swatches"), id);
    assert!(h.tab("layers").is_positive());
    assert!(h.tab("swatches").is_positive());
    h.assert_owners();
}

#[test]
fn dropping_a_host_on_the_bottom_edge_stacks_distinct_groups() {
    let mut h = Harness::new();
    let destination_id = h.host_id("layers");
    let source_id = h.host_id("swatches");
    let layers_group = h.layout().group_of("layers").unwrap().id;
    let swatches_group = h.layout().group_of("swatches").unwrap().id;
    let destination = h.host_rect("layers");
    let to = pos2(destination.center().x, destination.bottom() + 4.0);
    let before = h.layout().clone();
    h.hold_drag(h.hit(&format!("host-grip:{source_id}")).center(), to);
    assert_eq!(h.layout(), &before);
    h.release(to);
    assert_eq!(h.layout().hosts.len(), 1);
    assert_eq!(h.host_id("swatches"), destination_id);
    let host = h.layout().host(destination_id).unwrap();
    assert!(matches!(&host.root, model::Node::Split { axis: model::Axis::Vertical, .. }));
    let groups = model::groups(&host.root);
    assert_eq!(groups.iter().map(|g| g.id).collect::<Vec<_>>(), [layers_group, swatches_group]);
    assert_eq!(groups.iter().map(|g| g.panels.clone()).collect::<Vec<_>>(), [vec!["layers".to_owned()], vec!["swatches".to_owned()]]);
    assert!(h.tab("swatches").top() > h.tab("layers").bottom());
    h.assert_owners();
}

#[test]
fn free_host_drop_retains_front_order_and_background_click_raises_the_lower_host() {
    let mut h = Harness::new();
    let layers = h.host_id("layers");
    let swatches = h.host_id("swatches");
    let before = h.layout().clone();
    let to = pos2(1060.0, 540.0);
    h.hold_drag(h.hit(&format!("host-grip:{layers}")).center(), to);
    assert_eq!(h.layout(), &before, "pressing and previewing must not change committed stacking order");
    h.release(to);
    assert_eq!(h.layout().hosts.last().unwrap().id, layers, "the dropped host stays in front after pointer-up");

    let before_click = h.layout().clone();
    let background = h.host_rect("swatches");
    let at = pos2(background.left() + 16.0, background.bottom() - 18.0);
    h.frame(vec![Event::PointerMoved(at), button(at, true)]);
    assert_eq!(h.layout(), &before_click, "a press alone is not a committed raise action");
    h.frame(vec![button(at, false)]);
    h.settle();
    assert_eq!(h.layout().hosts.last().unwrap().id, swatches, "an exposed panel background raises its whole host");
    for host in &before_click.hosts {
        assert_eq!(h.layout().host(host.id).unwrap(), host, "raising changes order, not host content or geometry");
    }
    h.assert_owners();
}

#[test]
fn detaching_a_full_flyout_tab_from_compact_mode_keeps_a_full_sized_floating_body() {
    let mut h = Harness::new();
    h.merge_tabs();
    let source = h.host_id("layers");
    let group = h.layout().group_of("swatches").unwrap().id;
    let expanded = h.layout().expanded_group_size(group).unwrap();
    h.click(h.hit(&format!("mode:{source}")).center());
    assert_eq!(h.layout().host(source).unwrap().mode, Mode::Icons);
    h.click(h.tab("swatches").center());
    assert_eq!(h.state().flyout, Some(group));
    let full_tab = h.state().hits.iter().rev().find_map(|(name, rect)| (name == "tab:swatches").then_some(*rect)).unwrap();
    assert!(full_tab.width() > h.tab("swatches").width(), "drag the expanded flyout tab, not its compact icon anchor");
    let before = h.layout().clone();
    let to = pos2(1050.0, 570.0);
    h.hold_drag(full_tab.center(), to);
    assert_eq!(h.layout(), &before);
    h.release(to);

    let detached = h.layout().owner("swatches").unwrap();
    assert_ne!(detached.id, source);
    assert_eq!(detached.mode, Mode::Full);
    assert_eq!(detached.expanded, expanded, "detachment retains the content dimensions represented by the flyout");
    assert!((h.host_rect("swatches").width() - expanded[0]).abs() < 0.6);
    assert!((h.host_rect("swatches").height() - expanded[1]).abs() < 0.6);
    assert_eq!(h.host_id("layers"), source);
    assert_eq!(h.layout().host(source).unwrap().mode, Mode::Icons);
    assert_eq!(h.layout().group_of("layers").unwrap().panels, ["layers"]);
    assert!(h.state().flyout.is_none());
    h.assert_owners();
}

#[test]
fn ratio_aware_minimum_width_preserves_both_unequal_group_bodies() {
    for (layers_width, swatches_width) in [(300.0, 600.0), (650.0, 260.0)] {
        let mut layout = Layout::new_slice();
        let layers = layout.owner("layers").unwrap().id;
        let swatches = layout.owner("swatches").unwrap().id;
        for (host, width) in [(layers, layers_width), (swatches, swatches_width)] {
            layout.apply(&model::Action::Resize { host, position: [0.0, 0.0], width, height: 380.0 }).unwrap();
        }
        layout
            .apply(&model::Action::Move {
                source: model::Source::Host(swatches),
                target: model::Target::Attach { host: layers, edge: model::Edge::Right },
            })
            .unwrap();
        let mut host = layout.host(layers).unwrap().clone();
        let model::Node::Split { ratio, .. } = &host.root else { panic!("side attachment must preserve two groups") };
        assert!((*ratio - 0.5).abs() > 0.1, "exercise unequal retained group dimensions");
        host.expanded[0] = super::geometry::minimum_width(&host.root);
        let geometry = super::geometry::measure_host(&host, Rect::from_min_size(Pos2::ZERO, vec2(2000.0, 1200.0)));
        assert_eq!(geometry.groups.len(), 2);
        for group in &geometry.groups {
            assert!(group.body.width() >= 237.99, "group {} is clipped to {} at its host's minimum width", group.id, group.body.width());
        }
        assert!(geometry.groups.iter().any(|group| (group.body.width() - 238.0).abs() < 0.02), "the minimum should be tight for the limiting group");
    }
}

/// Rasterize the actual app's settled egui output without a GPU or native window.
pub(super) fn software_capture(app: &mut VectorcraftApp, width: u32, height: u32) -> Vec<u8> {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    app.fonts_ready = true;
    app.canvas.worker_started = true;
    let mut textures = std::collections::HashMap::<egui::TextureId, egui::ColorImage>::new();
    let mut shot = None;
    for frame in 0..6 {
        let input = egui::RawInput {
            time: Some(100.0 + 0.05 * frame as f64),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height as f32))),
            max_texture_side: Some(8192),
            focused: true,
            ..Default::default()
        };
        app.begin_host_iteration();
        let mut out = ctx.run_ui(input, |ui| {
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
        out.textures_delta.clear();
        shot = Some(ctx.tessellate(out.shapes, out.pixels_per_point));
    }
    crate::varos::home::capture::raster(&shot.unwrap(), &textures, width, height)
}

/// Optional real-code software previews: the initial full panels, then one compact stack.
/// This produces review material only; it does not establish native interaction acceptance.
#[test]
#[ignore = "optional software visual capture; set VAROS_PANEL_SLICE_CAPTURE"]
fn capture_panel_slice() {
    let Some(path) = std::env::var_os("VAROS_PANEL_SLICE_CAPTURE").map(std::path::PathBuf::from) else { return };
    let mut h = Harness::new();
    h.app.run("shape.rectangle", json!({"x": 40, "y": 40, "width": 120, "height": 80})).unwrap();
    h.settle();
    let (width, height) = (1600_u32, 1000_u32);
    let pixels = software_capture(&mut h.app, width, height);
    image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();

    let destination = h.host_id("layers");
    let source = h.host_id("swatches");
    for action in [
        model::Action::Move { source: model::Source::Host(source), target: model::Target::Attach { host: destination, edge: model::Edge::Bottom } },
        model::Action::SetMode { host: destination, mode: Mode::Icons },
        model::Action::Resize { host: destination, position: [72.0, 72.0], width: 200.0, height: 380.0 },
    ] {
        h.app.run("ui.panelReview.action", json!({"action": action})).unwrap();
    }
    h.assert_owners();
    assert_eq!(model::groups(&h.layout().host(destination).unwrap().root).len(), 2);
    assert_eq!(h.layout().host(destination).unwrap().mode, Mode::Names);
    let pixels = software_capture(&mut h.app, width, height);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let compact_path = path.with_file_name(format!("{stem}-compact-stack.png"));
    image::save_buffer(&compact_path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

#[test]
fn collapsing_after_dragging_to_workspace_edge_keeps_the_visible_origin() {
    let mut h = Harness::new();
    let id = h.host_id("layers");
    let from = h.hit(&format!("host-grip:{id}")).center();
    h.drag(from, pos2(1570.0, 580.0));
    let visible = h.host_rect("layers");
    h.click(h.hit(&format!("mode:{id}")).center());
    assert_eq!(h.layout().host(id).unwrap().mode, Mode::Icons);
    assert_near(h.host_rect("layers").min, visible.min);
    h.click(h.hit(&format!("mode:{id}")).center());
    assert_near(h.host_rect("layers").min, visible.min);
}

#[test]
#[ignore = "optional software visual capture; set VAROS_PANEL_WORKSPACE_CAPTURE"]
fn capture_panel_workspace() {
    let Ok(path) = std::env::var("VAROS_PANEL_WORKSPACE_CAPTURE") else { return };
    let mut h = Harness::new();
    h.app.ui.varos_panel_review = Some(State { layout: Layout::new_workspace(), ..Default::default() });
    h.settle();
    let width = 1600;
    let height = 1000;
    let pixels = software_capture(&mut h.app, width, height);
    image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
    let group = h.layout().group_of("layers").unwrap().id;
    h.app
        .run(
            "ui.panelReview.action",
            json!({"action":model::Action::Move{source:model::Source::Panel("swatches".into()),target:model::Target::Merge{group,index:0}}}),
        )
        .unwrap();
    let pixels = software_capture(&mut h.app, width, height);
    let path = std::path::Path::new(&path);
    let stem = path.file_stem().unwrap().to_string_lossy();
    image::save_buffer(path.with_file_name(format!("{stem}-tabs.png")), &pixels, width, height, image::ColorType::Rgba8).unwrap();
    h.app.run("ui.panelReview.action", json!({"action":model::Action::SetToolsWidth{width:super::metrics::TOOLS_MIN_WIDTH}})).unwrap();
    let pixels = software_capture(&mut h.app, width, height);
    image::save_buffer(path.with_file_name(format!("{stem}-narrow.png")), &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

/// Software output of the actual Control contents in its three permitted placements.
/// The owner still reviews native dragging and appearance in the single working app.
#[test]
#[ignore = "optional software visual capture; set VAROS_CONTROL_BAR_CAPTURE"]
fn capture_control_bar_inline_chrome() {
    let Some(path) = std::env::var_os("VAROS_CONTROL_BAR_CAPTURE").map(std::path::PathBuf::from) else { return };
    let mut h = Harness::new();
    h.app.ui.varos_panel_review = Some(State { layout: Layout::new_workspace(), ..Default::default() });
    h.settle();
    let host = h.host_id("control");
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    for (name, target) in [
        ("top", model::Target::Dock { edge: model::Edge::Top }),
        ("floating", model::Target::Float { position: [180.0, 260.0] }),
        ("bottom", model::Target::Dock { edge: model::Edge::Bottom }),
    ] {
        h.app.run("ui.panelReview.action", json!({"action":model::Action::Move{source:model::Source::Host(host),target}})).unwrap();
        let pixels = software_capture(&mut h.app, 1600, 1000);
        let output = if name == "top" { path.clone() } else { path.with_file_name(format!("{stem}-{name}.png")) };
        image::save_buffer(output, &pixels, 1600, 1000, image::ColorType::Rgba8).unwrap();
    }
}
