//! Full-registry pointer regressions through the real app frame, separate from native acceptance.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::test_support::*;

impl Harness {
    fn set_viewport_width(&mut self, width: f32) {
        self.screen.x = width;
        self.settle();
        self.measure_workspace();
    }

    fn assert_control_fitted(&self) -> f32 {
        let host = self.host_id("control");
        let content = super::content::control_height(&self.ctx, "control").expect("the real Control body reports its intrinsic height");
        let (hosts, _) = geometry::arrange(self.layout(), self.workspace, self.tabs());
        let measured = hosts.iter().find(|candidate| candidate.id == host).unwrap();
        let body = measured.groups.first().unwrap().body;
        assert!(
            (body.height() - content).abs() < 1.1,
            "the fitted body has neither clipped content nor unused bottom space: {} / {content}",
            body.height()
        );
        assert!((measured.rect.height() - content - 2.0).abs() < 1.1, "Control adds only its one-point top and bottom borders");
        assert!((body.top() - measured.rect.top() - 1.0).abs() < 0.1, "no title or tab band above the contents");
        assert!((measured.rect.bottom() - body.bottom() - 1.0).abs() < 0.1, "no footer or blank reserved height");
        assert!(!self.has_hit(&format!("close:{host}")));
        assert!(!self.has_hit(&format!("mode:{host}")));
        let grip = self.hit(&format!("control-grip:{host}"));
        assert!(grip.width() <= 12.1 && grip.is_positive());
        assert!(measured.rect.contains_rect(grip));
        assert!(grip.right() <= body.left() + 0.1, "the grip is beside the controls, not above them");
        assert!(!self.has_hit("tab:control"));
        measured.rect.height()
    }

    fn text_shapes(&mut self) -> Vec<(String, Rect)> {
        fn walk(shape: &egui::Shape, output: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => output.push((t.galley.text().to_string(), Rect::from_min_size(t.pos, t.galley.size()))),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, output)),
                _ => {}
            }
        }
        let out = self.frame(vec![]);
        let mut texts = Vec::new();
        for shape in &out.shapes {
            walk(&shape.shape, &mut texts);
        }
        texts
    }

    fn assert_canvas_reserved(&self) {
        let (hosts, board) = geometry::arrange(self.layout(), self.workspace, self.tabs());
        let canvas = self.canvas();
        assert!(board.expand(0.6).contains_rect(canvas), "canvas {canvas:?} must stay inside the dock-reserved board {board:?}");
        for host in hosts {
            if self.layout().host(host.id).unwrap().dock.is_some() {
                assert!(!host.rect.shrink(0.6).intersects(canvas), "canvas input cannot overlap docked host {}", host.id);
            }
        }
    }

    fn assert_frame_chrome(&mut self, panel: &str, docked: bool) {
        fn radii(shape: &egui::Shape, rect: Rect, output: &mut Vec<egui::CornerRadius>) {
            match shape {
                egui::Shape::Rect(shape) if shape.rect == rect => output.push(shape.corner_radius),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| radii(shape, rect, output)),
                _ => {}
            }
        }
        let host = self.host_id(panel);
        assert_eq!(
            self.has_hit(&format!("close:{host}")),
            !docked && panel != "control",
            "Control has no close chrome; ordinary floating hosts expose X"
        );
        let rect = self.host_rect(panel);
        let out = self.frame(vec![]);
        let mut corners = Vec::new();
        for shape in &out.shapes {
            radii(&shape.shape, rect, &mut corners);
        }
        assert!(!corners.is_empty(), "the actual host frame is painted");
        if docked {
            assert!(corners.iter().all(|radius| *radius == egui::CornerRadius::ZERO), "docked outer surfaces are square: {corners:?}");
            let measured = geometry::arrange(self.layout(), self.workspace, self.tabs()).0;
            let host = measured.iter().find(|candidate| candidate.id == host).unwrap();
            let bottom = host
                .groups
                .iter()
                .map(|group| if group.footer.is_positive() { group.footer.bottom() } else { group.body.bottom() })
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((host.rect.bottom() - bottom).abs() <= 1.1, "docked content/footer meets its bottom edge: {bottom} / {}", host.rect.bottom());
        } else {
            assert!(corners.iter().any(|radius| *radius != egui::CornerRadius::ZERO), "a detached host restores its floating frame");
        }
    }

    fn assert_header_cursor(&mut self, host: u64) {
        let at = self.hit(&format!("host-grip:{host}")).center();
        self.frame(vec![Event::PointerMoved(at)]);
        let out = self.frame(vec![]);
        assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::Default, "the dark drag band uses the ordinary pointer");
    }
}

#[test]
fn tools_header_floats_redocks_right_and_escape_preserves_the_committed_layout() {
    let mut h = Harness::workspace();
    // Isolate the Tools edge under test from the new fresh-workspace AI right dock.
    h.action(Action::Hide { host: h.host_id("ai") });
    let id = h.host_id("tools");
    assert_eq!(h.layout().host(id).unwrap().dock, Some(Edge::Left));
    assert!(!h.has_hit("tab:tools"), "standalone docked Tools does not repeat its name in a tab band");
    h.assert_frame_chrome("tools", true);
    h.assert_header_cursor(id);
    let initial_canvas = h.canvas();
    h.assert_canvas_reserved();
    let before = h.layout().clone();
    let free = pos2(1050.0, h.workspace.top() + 160.0);
    h.hold_drag(h.hit(&format!("host-grip:{id}")).center(), free);
    assert_eq!(h.layout(), &before, "dragging previews do not move committed dock ownership");
    assert_eq!(h.frame(vec![]).platform_output.cursor_icon, egui::CursorIcon::Default, "moving a panel keeps the ordinary pointer");
    h.release(free);
    assert_eq!(h.host_id("tools"), id);
    assert_eq!(h.layout().host(id).unwrap().dock, None);
    assert!(!h.has_hit("tab:tools"), "floating Tools does not add a redundant name tab");
    let group = h.layout().group_of("tools").unwrap().id;
    assert!(!h.has_hit(&format!("panel-menu:{group}")));
    h.assert_frame_chrome("tools", false);
    h.assert_header_cursor(id);
    assert!(h.canvas().left() < initial_canvas.left() - 20.0, "floating Tools gives the reserved left edge back to the canvas");
    h.assert_owners();

    let right_edge = pos2(h.workspace.right() - 3.0, h.workspace.center().y);
    h.drag(h.hit(&format!("host-grip:{id}")).center(), right_edge);
    assert_eq!(h.layout().host(id).unwrap().dock, Some(Edge::Right));
    assert!(!h.has_hit("tab:tools"));
    h.assert_frame_chrome("tools", true);
    h.assert_header_cursor(id);
    assert!((h.host_rect("tools").right() - h.workspace.right()).abs() < 0.6);
    assert!(h.canvas().right() <= h.host_rect("tools").left() + 0.6);
    h.assert_canvas_reserved();

    let before = h.layout().clone();
    let to = pos2(1020.0, h.workspace.top() + 180.0);
    h.hold_drag(h.hit(&format!("host-grip:{id}")).center(), to);
    h.frame(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
    assert!(h.state().gesture.is_none());
    h.release(to);
    assert_eq!(h.layout(), &before);
    h.assert_owners();
}

#[test]
fn docked_tools_width_resize_updates_the_real_canvas_reservation() {
    let mut h = Harness::workspace();
    let tools = h.host_id("tools");
    let before = h.host_rect("tools");
    let canvas = h.canvas();
    let from = h.hit(&format!("resize-right:{tools}")).center();
    h.drag(from, from - vec2(36.0, 0.0));
    assert!((h.host_rect("tools").width() - 44.0).abs() < 0.6);
    assert!((h.canvas().left() - canvas.left() + 36.0).abs() < 0.6);
    h.assert_canvas_reserved();
    let from = h.hit(&format!("resize-right:{tools}")).center();
    h.drag(from, from + vec2(120.0, 0.0));
    assert!((h.host_rect("tools").width() - 80.0).abs() < 0.6);
    assert!((h.host_rect("tools").width() - before.width()).abs() < 0.6);
    assert!((h.canvas().left() - canvas.left()).abs() < 0.6);
    assert_eq!(h.layout().host(tools).unwrap().mode, Mode::Full);
    assert_eq!(h.layout().host(tools).unwrap().dock, Some(Edge::Left));
    h.assert_canvas_reserved();
    h.assert_owners();
}

#[test]
fn bars_reject_tab_merges_but_tools_accepts_an_independent_group_beside_it() {
    for bar in ["tools", "control"] {
        let mut h = Harness::workspace();
        let host = h.host_id(bar);
        let group = h.layout().group_of(bar).unwrap().id;
        let before = h.layout().clone();
        let action = Action::Move { source: Source::Panel("layers".into()), target: Target::Merge { group, index: 1 } };
        assert!(h.app.run("ui.panelReview.action", json!({"action":action})).is_err(), "{bar} cannot share its tab group");
        assert_eq!(h.layout(), &before, "rejecting a merge preserves the complete layout");
        let measured = geometry::arrange(h.layout(), h.workspace, h.tabs()).0;
        let target = measured.iter().find(|candidate| candidate.id == host).unwrap().groups.first().unwrap().body.center();
        h.drag(h.hit("tab:layers").center(), target);
        assert_ne!(h.host_id("layers"), host, "dropping inside {bar} must not merge its tabs");
        assert_eq!(h.layout().group_of(bar).unwrap().panels, [bar.to_string()]);
        assert!(!h.has_hit(&format!("tab:{bar}")));
        h.assert_owners();
    }

    let mut h = Harness::workspace();
    let tools = h.host_id("tools");
    let tools_group = h.layout().group_of("tools").unwrap().id;
    let layers_group = h.layout().group_of("layers").unwrap().id;
    let before = h.layout().host(tools).unwrap().clone();
    h.action(Action::Resize { host: tools, position: before.position, width: 52.0, height: before.expanded[1] });
    assert!((h.host_rect("tools").width() - 52.0).abs() < 0.6);
    let target = pos2(h.host_rect("tools").right() - 3.0, h.host_rect("tools").center().y);
    h.drag(h.hit("tab:layers").center(), target);
    assert_eq!(h.host_id("layers"), tools, "a side drop joins the shared host");
    let host = h.layout().host(tools).unwrap();
    assert_eq!(host.dock, Some(Edge::Left));
    assert!(matches!(&host.root, model::Node::Split { axis: model::Axis::Horizontal, .. }), "Tools remains beside the regular group");
    assert_eq!(model::groups(&host.root).len(), 2);
    assert_eq!(h.layout().group_of("tools").unwrap().id, tools_group);
    assert_eq!(h.layout().group_of("layers").unwrap().id, layers_group);
    assert_eq!(h.layout().group(tools_group).unwrap().panels, ["tools"]);
    assert_eq!(h.layout().group(layers_group).unwrap().panels, ["layers"]);
    let measured = geometry::arrange(h.layout(), h.workspace, h.tabs()).0;
    let host = measured.iter().find(|candidate| candidate.id == tools).unwrap();
    let tool_rect = host.groups.iter().find(|g| g.id == tools_group).unwrap().rect;
    let layer_rect = host.groups.iter().find(|g| g.id == layers_group).unwrap().rect;
    assert!(tool_rect.right() < layer_rect.left());
    assert!((tool_rect.top() - layer_rect.top()).abs() < 0.6);
    assert!((tool_rect.bottom() - layer_rect.bottom()).abs() < 0.6);
    h.assert_canvas_reserved();
    h.assert_owners();

    assert!(!h.has_hit("tab:tools"), "docked Tools keeps its integrated chrome beside regular groups");
    assert!(h.has_hit("tab:layers"), "the regular group retains its normal tab");
    let grip = h.hit(&format!("group-grip:{tools_group}")).center();
    h.frame(vec![Event::PointerMoved(grip)]);
    assert_eq!(h.frame(vec![]).platform_output.cursor_icon, egui::CursorIcon::Default);
    let free = pos2(1050.0, h.workspace.top() + 400.0);
    h.drag(grip, free);
    assert_ne!(h.host_id("tools"), tools, "the group handle detaches only Tools from the shared host");
    assert_eq!(h.layout().owner("tools").unwrap().dock, None);
    assert_eq!(h.layout().group_of("tools").unwrap().id, tools_group);
    assert_eq!(h.layout().owner("tools").unwrap().mode, Mode::Full);
    assert!(!h.has_hit("tab:tools"), "the narrow floating Tools body omits the redundant name tab");
    assert!(
        crate::toolbar::tests::widget_rects(&h.ctx, egui::Vec2::splat(super::metrics::TOOL_BUTTON))
            .iter()
            .any(|button| h.host_rect("tools").contains_rect(*button)),
        "detaching narrow Tools retains its actual tool controls"
    );
    assert_eq!(h.host_id("layers"), tools);
    assert_eq!(h.layout().owner("layers").unwrap().dock, Some(Edge::Left));
    assert_eq!(h.layout().group_of("layers").unwrap().id, layers_group);
    assert_eq!(model::groups(&h.layout().owner("layers").unwrap().root).len(), 1);
    assert!(h.has_hit("tab:layers"));
    h.assert_canvas_reserved();
    h.assert_owners();
}

#[test]
fn bar_headers_remain_floating_when_dropped_on_an_incompatible_workspace_edge() {
    for (bar, edge) in [("tools", Edge::Top), ("control", Edge::Left)] {
        let mut h = Harness::workspace();
        let host = h.host_id(bar);
        let free = pos2(1050.0, h.workspace.top() + 480.0);
        h.drag(h.hit(&format!("host-grip:{host}")).center(), free);
        assert_eq!(h.layout().host(host).unwrap().dock, None);
        let before = h.host_rect(bar);
        let target = if edge == Edge::Top {
            pos2(h.workspace.center().x, h.workspace.top() + 3.0)
        } else {
            pos2(h.workspace.left() + 3.0, h.workspace.center().y)
        };
        h.drag(h.hit(&format!("host-grip:{host}")).center(), target);
        assert_eq!(h.host_id(bar), host);
        assert_eq!(h.layout().host(host).unwrap().dock, None, "{bar} cannot dock to {edge:?}");
        assert_ne!(h.host_rect(bar).min, before.min, "an invalid dock target still permits the requested floating move");
        assert_eq!(h.layout().group_of(bar).unwrap().panels, [bar.to_string()]);
        assert_eq!(h.has_hit(&format!("close:{host}")), bar != "control");
        h.assert_canvas_reserved();
        h.assert_owners();
    }
}

#[test]
fn bottom_docked_control_fits_its_content_and_preserves_its_floating_size() {
    let mut h = Harness::workspace();
    let host = h.host_id("control");
    let stored = h.layout().host(host).unwrap().expanded;
    let initial_height = h.assert_control_fitted();
    let bottom = pos2(h.workspace.center().x, h.workspace.bottom() - 3.0);
    h.drag(h.hit(&format!("host-grip:{host}")).center(), bottom);
    assert_eq!(h.layout().host(host).unwrap().dock, Some(Edge::Bottom));
    assert!((h.assert_control_fitted() - initial_height).abs() < 1.1, "redocking at the same width retains the intrinsic height");
    for edge in ["top", "bottom", "left", "right"] {
        assert!(!h.has_hit(&format!("resize-{edge}:{host}")), "docked Control fits its rows instead of exposing a manual height handle");
    }
    assert_eq!(h.layout().host(host).unwrap().expanded, stored, "fitting a dock does not overwrite its remembered floating size");
    let docked = h.host_rect("control");
    let (_, board) = geometry::arrange(h.layout(), h.workspace, h.tabs());
    assert!((board.bottom() - docked.top()).abs() < 0.6, "the board reserves exactly the fitted bar");
    assert!((docked.bottom() - h.workspace.bottom()).abs() < 0.6);
    h.assert_canvas_reserved();

    let canvas = h.canvas();
    let free = pos2(900.0, h.workspace.top() + 480.0);
    h.drag(h.hit(&format!("host-grip:{host}")).center(), free);
    assert_eq!(h.layout().host(host).unwrap().dock, None);
    assert_eq!(h.layout().host(host).unwrap().expanded, stored);
    h.assert_control_fitted();
    assert!(h.host_rect("control").height() < stored[1], "floating ignores the obsolete extra height and fits its contents");
    assert!((h.canvas().bottom() - canvas.bottom() - docked.height()).abs() < 0.6);
    assert!(!h.has_hit("tab:control"));
    assert!(!h.has_hit(&format!("close:{host}")));
    h.assert_canvas_reserved();
    h.assert_owners();
}

#[test]
fn docked_control_keeps_one_row_when_the_viewport_changes_width() {
    let mut h = Harness::workspace();
    let host = h.host_id("control");
    let stored = h.layout().host(host).unwrap().expanded;
    let wide_height = h.assert_control_fitted();
    let wide_canvas_top = h.canvas().top();
    h.set_viewport_width(700.0);
    let narrow_height = h.assert_control_fitted();
    assert!((narrow_height - wide_height).abs() < 1.1, "a narrow viewport cannot add a second row");
    assert!((h.canvas().top() - wide_canvas_top - (narrow_height - wide_height)).abs() < 1.1);
    assert_eq!(h.layout().host(host).unwrap().expanded, stored);
    assert!(!h.has_hit(&format!("resize-bottom:{host}")));
    h.assert_canvas_reserved();

    h.set_viewport_width(1600.0);
    assert!((h.assert_control_fitted() - wide_height).abs() < 1.1, "the bottom gap disappears again when controls return to one row");
    assert!((h.canvas().top() - wide_canvas_top).abs() < 1.1);
    assert_eq!(h.layout().host(host).unwrap().expanded, stored);
    h.assert_canvas_reserved();
    h.assert_owners();
}

#[test]
fn window_opens_existing_color_properties_and_character_bodies_without_duplicate_hosts() {
    let mut h = Harness::workspace();
    for (panel, body_text) in [("color", "R"), ("properties", "Document"), ("character", "No text selected")] {
        let host = h.host_id(panel);
        let count = h.layout().hosts.len();
        assert!(h.layout().host(host).unwrap().hidden);
        h.app.run("window.panel", json!({"panel":panel})).unwrap();
        h.settle();
        assert!(!h.layout().host(host).unwrap().hidden);
        assert_eq!(h.host_id(panel), host);
        assert_eq!(h.layout().hosts.len(), count);
        let rect = h.host_rect(panel);
        let tab = h.hit(&format!("tab:{panel}"));
        let texts = h.text_shapes();
        assert!(
            texts.iter().any(|(text, r)| text == body_text && rect.contains(r.center()) && r.center().y > tab.bottom()),
            "{panel}'s existing body must be drawn below its tab: {texts:?}"
        );
        h.app.run("window.panel", json!({"panel":panel})).unwrap();
        h.settle();
        assert_eq!(h.host_id(panel), host, "showing the same panel again reuses its owner");
        assert_eq!(h.layout().hosts.len(), count);
        h.assert_owners();
    }
}

#[test]
fn narrow_tools_hide_and_window_restore_the_same_docked_owner() {
    let mut h = Harness::workspace();
    let host = h.host_id("tools");
    h.click(h.hit(&format!("mode:{host}")).center());
    assert_eq!(h.layout().host(host).unwrap().mode, Mode::Full);
    assert!((h.host_rect("tools").width() - 44.0).abs() < 0.6);
    let narrow = h.layout().host(host).unwrap().clone();
    assert!(!h.has_hit(&format!("close:{host}")), "the narrow dock also omits the floating X");
    h.app.run("window.toolbar", json!({})).unwrap();
    h.settle();
    assert!(h.layout().host(host).unwrap().hidden);
    assert!(!h.state().hits.iter().any(|(name, _)| name == "tab:tools"));
    assert_eq!(crate::menus::checked(&h.app, "window.toolbar", &json!({})), Some(false));

    h.app.run("window.toolbar", json!({})).unwrap();
    h.settle();
    assert_eq!(h.host_id("tools"), host);
    assert_eq!(h.layout().host(host).unwrap(), &narrow);
    assert_eq!(crate::menus::checked(&h.app, "window.toolbar", &json!({})), Some(true));
    assert!(!h.has_hit("tab:tools"), "restored docked Tools keeps its integrated chrome");
    assert!((h.host_rect("tools").width() - 44.0).abs() < 0.6);
    assert!(
        crate::toolbar::tests::widget_rects(&h.ctx, egui::Vec2::splat(super::metrics::TOOL_BUTTON))
            .iter()
            .any(|button| h.host_rect("tools").contains_rect(*button)),
        "restoring the narrow toolbar restores its real tool controls"
    );
    h.assert_canvas_reserved();
    h.assert_owners();
}

#[test]
fn control_keeps_inline_grip_and_no_panel_chrome_when_floated_and_redocked() {
    let mut h = Harness::workspace();
    let host = h.host_id("control");
    assert!(!h.has_hit("tab:control"), "docked Control has no redundant name band");
    h.assert_frame_chrome("control", true);
    h.assert_header_cursor(host);
    let free = pos2(900.0, h.workspace.top() + 560.0);
    h.drag(h.hit(&format!("host-grip:{host}")).center(), free);
    assert_eq!(h.layout().host(host).unwrap().dock, None);
    assert!(!h.has_hit("tab:control"));
    h.assert_control_fitted();
    for edge in ["top", "bottom"] {
        assert!(!h.has_hit(&format!("resize-{edge}:{host}")), "floating Control also fits its content height");
    }
    h.assert_frame_chrome("control", false);

    let top = pos2(h.workspace.center().x, h.workspace.top() + 3.0);
    h.drag(h.hit(&format!("host-grip:{host}")).center(), top);
    assert_eq!(h.layout().host(host).unwrap().dock, Some(Edge::Top));
    assert!(!h.has_hit("tab:control"));
    h.assert_frame_chrome("control", true);
    h.assert_header_cursor(host);
    h.assert_canvas_reserved();
    h.assert_owners();
}

#[test]
fn docked_standard_groups_keep_their_tabs_under_one_shared_header() {
    let mut h = Harness::workspace();
    let layers = h.host_id("layers");
    let swatches = h.host_id("swatches");
    let right = pos2(h.workspace.right() - 3.0, h.workspace.center().y);
    h.drag(h.hit(&format!("host-grip:{layers}")).center(), right);
    assert_eq!(h.layout().host(layers).unwrap().dock, Some(Edge::Right));
    assert!(h.hit("tab:layers").is_positive(), "standard docked panels retain their tabs");
    h.assert_frame_chrome("layers", true);

    h.action(Action::Move { source: Source::Host(swatches), target: Target::Attach { host: layers, edge: Edge::Bottom } });
    assert_eq!(h.host_id("swatches"), layers);
    assert_eq!(model::groups(&h.layout().host(layers).unwrap().root).len(), 2);
    assert!(h.hit("tab:layers").is_positive() && h.hit("tab:swatches").is_positive());
    assert_eq!(h.state().hits.iter().filter(|(name, _)| name == &format!("host-grip:{layers}")).count(), 1);
    assert_eq!(h.state().hits.iter().filter(|(name, _)| name == &format!("mode:{layers}")).count(), 1);
    assert!(!h.has_hit(&format!("host-grip:{swatches}")), "stacking does not leave a second host header");
    h.assert_frame_chrome("layers", true);
    h.assert_header_cursor(layers);

    let free = pos2(1000.0, h.workspace.top() + 180.0);
    h.drag(h.hit(&format!("host-grip:{layers}")).center(), free);
    assert_eq!(h.layout().host(layers).unwrap().dock, None);
    assert_eq!(h.host_id("swatches"), layers);
    assert_eq!(h.state().hits.iter().filter(|(name, _)| name == &format!("close:{layers}")).count(), 1);
    assert!(h.has_hit("tab:layers") && h.has_hit("tab:swatches"));
    h.assert_frame_chrome("layers", false);
    h.assert_owners();
}

#[test]
fn tab_overflow_lists_every_group_member_and_selecting_one_makes_its_tab_visible() {
    let mut h = Harness::workspace();
    let group = h.layout().group_of("layers").unwrap().id;
    for panel in ["swatches", "color", "properties", "character", "paragraph", "brushes", "stroke"] {
        let index = h.layout().group(group).unwrap().panels.len();
        h.action(Action::Move { source: Source::Panel(panel.into()), target: Target::Merge { group, index } });
    }
    let expected = h.layout().group(group).unwrap().panels.clone();
    let (hosts, _) = geometry::arrange(h.layout(), h.workspace, h.tabs());
    let measured = hosts.iter().flat_map(|host| &host.groups).find(|candidate| candidate.id == group).unwrap();
    assert_eq!(measured.overflow, expected, "the menu offers all group members, including clipped tabs");
    assert!(measured.tabs.len() < expected.len());
    let active = h.layout().group(group).unwrap().active.clone();
    assert!(measured.tabs.iter().any(|(panel, _)| panel == &active), "the active tab stays visible");
    let hidden = expected.iter().find(|panel| !measured.tabs.iter().any(|(shown, _)| shown == *panel)).unwrap().clone();
    h.click(h.hit(&format!("tabs-overflow:{group}")).center());
    let labels = h.text_shapes();
    for panel in &expected {
        assert!(labels.iter().any(|(label, _)| label == registry::label(panel)), "overflow menu includes {panel}");
    }
    // The hidden tab has no painted label in the tab row; its matching label belongs to the menu.
    let at = labels.iter().rev().find(|(label, _)| label == registry::label(&hidden)).map(|(_, rect)| rect.center()).unwrap();
    h.click(at);
    assert_eq!(h.layout().group(group).unwrap().active, hidden);
    assert!(h.hit(&format!("tab:{hidden}")).is_positive());
    assert_eq!(h.layout().group(group).unwrap().panels, expected);
    h.assert_owners();
}

#[test]
fn a_tall_compact_block_scrolls_to_the_last_panel_without_reparenting() {
    let mut h = Harness::workspace();
    let group = h.layout().group_of("layers").unwrap().id;
    for panel in super::registry::BUILTINS.iter().map(|p| p.id).filter(|p| !matches!(*p, "layers" | "tools" | "control")) {
        let index = h.layout().group(group).unwrap().panels.len();
        h.action(Action::Move { source: Source::Panel(panel.into()), target: Target::Merge { group, index } });
    }
    assert_eq!(h.layout().group(group).unwrap().panels.len(), crate::state::all_panels().count(), "only regular panels share the compact group");
    let host = h.host_id("layers");
    h.action(Action::SetMode { host, mode: Mode::Icons });
    let rect = h.host_rect("layers");
    let before = h.layout().clone();
    h.frame(crate::toolbar::tests::wheel(rect.center(), 5000.0));
    for _ in 0..12 {
        h.frame(vec![]);
    }
    let last = h.hit("tab:variables");
    assert!(rect.contains(last.center()), "last compact row must scroll inside the visible host: {last:?} / {rect:?}");
    assert_eq!(h.layout(), &before, "scrolling is a view change, not an ownership edit");
    h.assert_owners();
}

#[test]
fn floating_control_width_resize_keeps_one_row_without_manual_height_handles() {
    let mut h = Harness::workspace();
    let host = h.host_id("control");
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [160.0, 200.0] } });
    let wide_height = h.assert_control_fitted();
    let before = h.host_rect("control");
    let from = h.hit(&format!("resize-right:{host}")).center();
    h.drag(from, from - vec2(500.0, 0.0));
    let narrow = h.host_rect("control");
    assert!(narrow.width() < before.width() - 200.0, "the real edge narrows the bar");
    assert!((h.assert_control_fitted() - wide_height).abs() < 1.1, "narrowing cannot add a second row");
    assert!((narrow.left() - before.left()).abs() < 0.6);
    assert!((narrow.top() - before.top()).abs() < 0.6);
    for edge in ["top", "bottom"] {
        assert!(!h.has_hit(&format!("resize-{edge}:{host}")));
    }
    let from = h.hit(&format!("resize-right:{host}")).center();
    h.drag(from, from + vec2(before.width() - narrow.width(), 0.0));
    assert!((h.assert_control_fitted() - wide_height).abs() < 1.1, "widening removes the no-longer-needed wrapped height");
    h.assert_owners();
}

#[test]
fn only_the_inline_control_grip_moves_the_host_while_content_remains_interactive() {
    let mut h = Harness::workspace();
    let host = h.host_id("control");
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [120.0, 180.0] } });
    h.assert_control_fitted();
    let before = h.layout().host(host).unwrap().clone();
    let rect = h.host_rect("control");
    let content = h.text_shapes().into_iter().find(|(text, r)| text == "100%" && rect.contains_rect(*r)).expect("real Opacity numeric field").1;
    let from = content.center();
    h.frame(vec![Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    for step in 1..=6 {
        h.frame(vec![Event::PointerMoved(from + vec2(step as f32 * 8.0, 0.0))]);
        assert!(
            h.state().gesture.is_none(),
            "editing numeric content never starts a host gesture: gesture={:?}, field={content:?}, control={rect:?}",
            h.state().gesture.as_ref().map(|gesture| &gesture.kind)
        );
    }
    h.release(from + vec2(48.0, 0.0));
    assert_eq!(h.layout().host(host), Some(&before));

    let grip = h.hit(&format!("control-grip:{host}")).center();
    let to = grip + vec2(80.0, 100.0);
    h.hold_drag(grip, to);
    assert_eq!(h.layout().host(host), Some(&before), "moving is a preview until release");
    h.release(to);
    let moved = h.host_rect("control");
    assert!((moved.left() - rect.left() - 80.0).abs() < 0.6);
    assert!((moved.top() - rect.top() - 100.0).abs() < 0.6);
    assert_eq!(h.layout().host(host).unwrap().root, before.root);
    h.assert_control_fitted();
    h.assert_owners();
}

#[test]
fn clicking_group_and_host_grips_does_not_detach_but_deliberate_group_drag_does() {
    let mut h = Harness::workspace();
    h.app.run("window.workspace", json!({"name":"Studio"})).unwrap();
    h.settle();
    let group = h.layout().group_of("swatches").unwrap().id;
    let owner = h.host_id("swatches");
    let before = h.layout().clone();
    for name in [
        format!("group-grip:{group}"),
        format!("host-grip:{owner}"),
        format!("host-grip:{}", h.host_id("tools")),
        format!("control-grip:{}", h.host_id("control")),
    ] {
        let at = h.hit(&name).center();
        h.frame(vec![Event::PointerMoved(at), button(at, true)]);
        assert!(h.state().gesture.is_none(), "pressing {name} alone must not start a move");
        h.frame(vec![Event::PointerMoved(at + vec2(1.0, 0.0))]);
        assert!(h.state().gesture.is_none(), "one-point hand jitter must not detach {name}");
        h.release(at + vec2(1.0, 0.0));
        // Clicking may raise a host in z-order, but must not change its layout.
        assert_eq!(h.layout().hosts.len(), before.hosts.len());
        for host in &before.hosts {
            assert_eq!(h.layout().host(host.id), Some(host), "clicking {name} preserves docking and grouping");
        }
    }
    let grip = h.hit(&format!("group-grip:{group}"));
    assert!(grip.height() <= 4.0);
    let icon = h.hit("tab:swatches");
    assert!((icon.top() - grip.bottom()).abs() < 0.1, "no extra blank padding above the icon");
    h.drag(grip.center(), pos2(650.0, 350.0));
    assert_eq!(h.layout().owner("swatches").unwrap().dock, None);
    assert_eq!(h.layout().owner("color").unwrap().dock, Some(Edge::Right));
    h.assert_owners();
}

/// S1: Control bar links (Opacity, Style) and other `open_panel` requests show their panel.
#[test]
fn control_bar_opacity_link_opens_transparency_in_the_panel_workspace() {
    let mut h = Harness::workspace();
    h.select_rectangle();
    assert!(!h.visible("transparency"));
    let output = h.settle();
    let (_, opacity, _) = texts(&output).into_iter().find(|(text, _, _)| text == "Opacity:").expect("the hosted Control bar draws Opacity:");
    h.click(opacity.center());
    assert!(h.visible("transparency"), "clicking Opacity: shows Transparency");
    assert!(h.app.ui.open_panel.is_none(), "the request is consumed");
    assert!(crate::menus::checked(&h.app, "window.panel", &json!({"panel": "transparency"})).unwrap());
    // Any other caller of the compatibility request is followed too.
    h.app.ui.open_panel = Some("graphicStyles".into());
    h.settle();
    assert!(h.visible("graphicStyles"));
    assert!(h.app.ui.open_panel.is_none());
}

/// S2: a library panel opened in the workspace is open for its own lifecycle commands.
#[test]
fn library_panels_follow_their_lifecycle_in_the_panel_workspace() {
    let mut h = Harness::workspace();
    let panel = h.app.run("window.swatchLibrary", json!({"library": "pastels"})).unwrap()["panel"].as_str().unwrap().to_string();
    h.settle();
    assert!(h.visible(&panel), "the library panel shows in the presented layout");
    assert!(crate::panels::library_panel::is_open(&h.app.ui, &panel));
    let settings = h.app.run("ui.libraryPanel", json!({"panel": panel, "view": "largeList", "persistent": true})).unwrap();
    assert_eq!((settings["open"].as_bool(), settings["persistent"].as_bool()), (Some(true), Some(true)));
    // Showing another library keeps the panel's place under its new identity.
    let host = h.layout().owner(&panel).unwrap().id;
    let renamed = h.app.run("ui.libraryPanel", json!({"panel": panel, "library": "brights"})).unwrap()["panel"].as_str().unwrap().to_string();
    assert_eq!(renamed, "library:swatches:brights");
    assert_eq!(h.layout().owner(&renamed).map(|h| h.id), Some(host));
    assert!(h.layout().owner(&panel).is_none());
    h.settle();
    // Closing every swatch library forgets their panels.
    h.app.run("window.swatchLibrary", json!({"library": null})).unwrap();
    assert!(h.layout().owner(&renamed).is_none(), "a closed library panel leaves the layout");
    assert!(!crate::panels::library_panel::is_open(&h.app.ui, &renamed));
    h.layout().validate().unwrap();
    // A persistent library comes back at startup.
    crate::panels::library_panel::restore(&mut h.app);
    assert!(h.visible(&renamed), "restore floats the persistent library back into the layout");
    h.settle();
}

/// S3: Presentation Mode presents the artwork alone: no Tools, Control or panels to edit with.
#[test]
fn presentation_mode_hides_all_workspace_chrome() {
    let mut h = Harness::workspace();
    h.select_rectangle();
    h.app.run("window.panel", json!({"panel": "color"})).unwrap();
    let before = h.settle();
    assert!(texts(&before).iter().any(|(text, _, _)| text == "Opacity:"));
    assert!(!h.state().hits.is_empty());
    h.app.run("view.presentation", json!({})).unwrap();
    assert_eq!(h.app.ui.screen_mode, 3);
    let output = h.settle();
    assert!(h.state().hits.is_empty(), "no panel host is drawn: {:?}", h.state().hits.iter().map(|(n, _)| n).collect::<Vec<_>>());
    assert!(!texts(&output).iter().any(|(text, _, _)| text == "Opacity:" || text == "Color"), "no Control or panel content is painted");
    let canvas = h.app.canvas_rect.expect("the canvas is drawn");
    assert!(canvas.width() >= h.screen.x - 1.0 && canvas.height() >= h.screen.y - 1.0, "no block reserves canvas space: {canvas:?}");
    // Leaving Presentation Mode restores the same workspace.
    let layout = h.layout().clone();
    h.app.run("view.presentation", json!({})).unwrap();
    h.settle();
    assert_eq!(h.layout(), &layout);
    assert!(!h.state().hits.is_empty());
}

/// S4: Window-menu items and panel keys toggle a shown panel off again.
#[test]
fn window_menu_toggle_and_panel_keys_hide_a_shown_panel() {
    let mut h = Harness::workspace();
    h.app.run("window.panel", json!({"panel": "color", "toggle": true})).unwrap();
    assert!(h.visible("color"));
    h.app.run("window.panel", json!({"panel": "color", "toggle": true})).unwrap();
    assert!(!h.visible("color"), "a checked Window item hides its panel");
    // Toggling one tab off leaves its group's other tabs shown.
    h.app.run("window.panel", json!({"panel": "align"})).unwrap();
    h.app.run("window.panel.move", json!({"panel": "align", "anchor": "swatches", "zone": "tab"})).unwrap();
    assert_eq!(h.layout().owner("align").map(|h| h.id), h.layout().owner("swatches").map(|h| h.id));
    h.app.run("window.panel", json!({"panel": "align", "toggle": true})).unwrap();
    assert!(!h.visible("align") && h.visible("swatches"), "only the toggled panel hides");
    h.layout().validate().unwrap();
    // The real panel key (Shift+F7, Align) shows it, then hides it again.
    let press = |h: &mut Harness| {
        let events = vec![Event::ModifiersChanged(Modifiers::SHIFT), key(Key::F7, Modifiers::SHIFT)];
        h.ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| crate::shortcuts::handle(&mut h.app, ui.ctx())).textures_delta.clear();
    };
    press(&mut h);
    assert!(h.visible("align"), "Shift+F7 shows Align");
    press(&mut h);
    assert!(!h.visible("align"), "Shift+F7 again hides Align");
}

/// S5: Tab hides Tools, Control and every panel; Shift+Tab hides the panels only, one panel at a
/// time, so a panel attached beside Tools hides while Tools stays.
#[test]
fn tab_hides_everything_and_shift_tab_hides_panels_beside_tools_too() {
    let mut h = Harness::workspace();
    let tools = h.layout().owner("tools").unwrap().id;
    let layers = h.layout().owner("layers").unwrap().id;
    h.app
        .run(
            "ui.panelReview.action",
            json!({"action": Action::Move { source: Source::Host(layers), target: Target::Attach { host: tools, edge: Edge::Right } }}),
        )
        .unwrap();
    assert_eq!(h.layout().owner("layers").map(|h| h.id), h.layout().owner("tools").map(|h| h.id), "Layers is attached beside Tools");
    h.settle();
    let arranged = h.layout().clone();
    h.app.run("window.panels", json!({})).unwrap();
    assert!(h.visible("tools") && h.visible("control"), "Shift+Tab leaves Tools and Control");
    assert!(!h.visible("layers") && !h.visible("swatches"), "Shift+Tab hides Layers beside Tools and the floating Swatches");
    h.settle();
    h.app.run("window.panels", json!({})).unwrap();
    assert_eq!(h.layout(), &arranged, "a second Shift+Tab restores the exact arrangement");
    h.app.run("window.dock", json!({})).unwrap();
    for panel in ["tools", "control", "layers", "swatches"] {
        assert!(!h.visible(panel), "Tab hides {panel}");
    }
    h.settle();
    h.app.run("window.dock", json!({})).unwrap();
    assert_eq!(h.layout(), &arranged, "a second Tab restores everything it hid");
    // Through the real keys.
    let press = |h: &mut Harness, modifiers: Modifiers| {
        h.frame(vec![Event::ModifiersChanged(modifiers), key(Key::Tab, modifiers)]);
        h.frame(vec![Event::ModifiersChanged(Modifiers::NONE)]);
    };
    press(&mut h, Modifiers::SHIFT);
    assert!(h.visible("tools") && !h.visible("layers"), "Shift+Tab key");
    press(&mut h, Modifiers::SHIFT);
    press(&mut h, Modifiers::NONE);
    assert!(!h.visible("tools") && !h.visible("control") && !h.visible("swatches"), "Tab key");
    press(&mut h, Modifiers::NONE);
    assert_eq!(h.layout(), &arranged);
}

/// G4: Apply sends the fields the user edited, even back at their opening value; a document
/// switch skips only the units; Escape in a field keeps Preferences and its draft.
#[test]
fn preferences_apply_sends_touched_fields_and_escape_in_a_field_keeps_the_draft() {
    let mut h = Harness::workspace();
    crate::prefs_dialog::open(&mut h.app, Some("General"));
    h.settle();
    let opening = h.app.session.prefs.keyboard_increment;
    // Another editor changes the setting while Preferences is open.
    h.app.run("prefs.set", json!({"key": "keyboardIncrement", "value": opening + 6.0})).unwrap();
    let output = h.frame(vec![]);
    let (_, label, _) =
        texts(&output).into_iter().find(|(text, _, _)| text == "Keyboard Increment:").expect("the General page shows Keyboard Increment");
    // The row's label column is 210 points wide; the field follows it.
    let field = Rect::from_min_size(pos2(label.left() + 214.0, label.center().y - 9.0), vec2(80.0, 18.0));
    // The user types 5, then puts the field back at its opening value.
    for typed in [format!("{}", opening + 4.0), format!("{opening}")] {
        h.click(field.center());
        h.frame(vec![key(Key::A, Modifiers::COMMAND)]);
        h.frame(vec![Event::Text(typed)]);
        h.frame(vec![key(Key::Enter, Modifiers::NONE)]);
        h.settle();
    }
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().fields["keyboardIncrement"].as_f64(), Some(opening));
    h.app.run("preferences.apply", json!({})).unwrap();
    assert_eq!(h.app.session.prefs.keyboard_increment, opening, "the edited field wins over the outside change");
    // Escape in a focused field leaves the field, not Preferences, and keeps the draft.
    h.click(field.center());
    h.frame(vec![key(Key::A, Modifiers::COMMAND)]);
    h.frame(vec![Event::Text(format!("{}", opening + 2.0))]);
    h.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    h.click(field.center());
    assert!(h.ctx.egui_wants_keyboard_input(), "the field has the keyboard");
    h.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    h.settle();
    let d = h.app.ui.dialog.as_ref().expect("Escape in a field keeps Preferences open");
    assert_eq!(d.fields["keyboardIncrement"].as_f64(), Some(opening + 2.0), "the unapplied draft is kept");
    assert!(!h.ctx.egui_wants_keyboard_input(), "the field gave up the keyboard");
    // Units of a document that is no longer active are skipped; the other edits apply.
    let units = h.app.ui.dialog.as_ref().unwrap().fields["unitsGeneral"].clone();
    let other = if units == "millimeters" { "points" } else { "millimeters" };
    // An agent's edit through the control channel counts as the user's.
    let (request, _) = crate::control::ControlRequest::new(
        "ui.dialog.set",
        json!({"fields": [{"field": "unitsGeneral", "value": other}, {"field": "keyboardIncrement", "value": 3}]}),
    );
    let crate::control::Outcome::Done(reply) = crate::control::handle(&mut h.app, &h.ctx, &request) else { panic!("ui.dialog.set replies at once") };
    assert_eq!(reply["ok"], true, "{reply}");
    let before: Vec<_> = h.app.session.documents().iter().map(|d| d.doc.units).collect();
    h.app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    let result = h.app.run("preferences.apply", json!({})).unwrap();
    assert_eq!(result["skipped"], json!(["unitsGeneral"]));
    assert_eq!(h.app.session.prefs.keyboard_increment, 3.0, "unrelated edits still apply");
    assert_eq!(h.app.session.documents()[..before.len()].iter().map(|d| d.doc.units).collect::<Vec<_>>(), before);
    assert!(h.app.ui.dialog.is_some());
}

/// Each drawn tab is as wide as its painted, localized label plus the tab padding (within the tab
/// limits): the layout measures the real text, in English and after the interface language
/// changes, never a per-character estimate.
#[test]
fn tabs_are_sized_from_their_measured_localized_labels() {
    let mut h = Harness::workspace();
    let check = |h: &mut Harness| -> Vec<(String, f32)> {
        let texts = h.text_shapes();
        let mut checked = Vec::new();
        for (name, tab) in h.state().hits.iter().filter(|(name, _)| name.starts_with("tab:")) {
            let panel = &name["tab:".len()..];
            if h.layout().column_mode(h.layout().group_of(panel).unwrap().id) != Some(Mode::Full) {
                continue;
            }
            let label = super::paint::label(panel);
            let Some((_, text)) = texts.iter().find(|(text, rect)| text == label && tab.contains(rect.left_center())) else { continue };
            let expected = (text.width().ceil() + 22.0).clamp(62.0, 180.0);
            assert!((tab.width() - expected).abs() < 0.6 || tab.width() < expected, "{panel}: tab {} for a {}-wide label", tab.width(), text.width());
            checked.push((label.to_string(), tab.width()));
        }
        assert!(!checked.is_empty(), "the workspace draws named tabs");
        checked
    };
    let english = check(&mut h);
    let lang = crate::i18n::Lang::for_picker()
        .into_iter()
        .find(|lang| english.iter().any(|(label, _)| crate::i18n::tr(*lang, label) != label && crate::i18n::tr(*lang, label).is_ascii()))
        .expect("a language that translates a tab label");
    h.app.run("prefs.set", json!({"key": "interfaceLanguage", "value": lang.code()})).unwrap();
    h.settle();
    let translated = check(&mut h);
    assert_ne!(translated, english, "the tabs follow the translated labels");
    crate::i18n::set_current(crate::i18n::Lang::EN);
}

/// The tool buttons drawn last frame inside `within`, top to bottom, then left to right.
fn tool_buttons(h: &Harness, within: Rect) -> Vec<Rect> {
    let mut buttons: Vec<Rect> = crate::toolbar::tests::widget_rects(&h.ctx, egui::Vec2::splat(super::metrics::TOOL_BUTTON))
        .into_iter()
        .filter(|button| within.contains_rect(*button))
        .collect();
    buttons.sort_by(|a, b| a.top().total_cmp(&b.top()).then(a.left().total_cmp(&b.left())));
    buttons.dedup();
    buttons
}

/// A right press on the tool button at `at`: the first tool of the flyout it opened, if any.
fn open_tool_flyout(h: &mut Harness, at: Rect) -> Option<String> {
    let secondary = |pressed| Event::PointerButton { pos: at.center(), button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
    h.frame(vec![Event::PointerMoved(at.center()), secondary(true)]);
    h.frame(vec![secondary(false)]);
    h.settle();
    h.app.ui.flyout?;
    h.ctx.data(|d| d.get_temp::<Vec<String>>(egui::Id::unique("flyout-tools"))).and_then(|tools| tools.first().cloned())
}

fn close_tool_flyout(h: &mut Harness) {
    h.app.ui.flyout = None;
    h.settle();
}

/// Each tool group's flyout among `buttons`: its first tool and its button.
fn flyout_buttons(h: &mut Harness, buttons: &[Rect]) -> Vec<(String, Rect)> {
    let mut found: Vec<(String, Rect)> = Vec::new();
    for &button in buttons {
        if let Some(tool) = open_tool_flyout(h, button)
            && !found.iter().any(|(known, _)| *known == tool)
        {
            found.push((tool, button));
        }
        close_tool_flyout(h);
    }
    found
}

/// The open tool flyout's identity is the presenters' one record: flyout A opened in a hosted
/// Tools, the workspace removed, flyout B opened in the box shell's strip, and the workspace back
/// without reopening anything: B shows, beside its own hosted button, not A's.
#[test]
fn a_tool_flyout_opened_in_another_presenter_anchors_to_its_own_hosted_button() {
    let mut h = Harness::workspace();
    let layout = h.layout().clone();
    let hosted = tool_buttons(&h, h.host_rect("tools"));
    let groups = flyout_buttons(&mut h, &hosted);
    let (a, a_button) = groups.first().cloned().expect("a hosted tool group with a flyout");
    let (b, b_button) = groups
        .iter()
        .find(|(_, button)| (button.top() - a_button.top()).abs() > 2.0 * super::metrics::TOOL_BUTTON)
        .cloned()
        .expect("a second tool group on another row");
    assert_eq!(open_tool_flyout(&mut h, a_button).as_deref(), Some(a.as_str()));
    let area = || h.ctx.memory(|m| m.area_rect(egui::Id::unique("tool-flyout")));
    near(area().unwrap().top(), a_button.top());

    h.app.run("ui.panelReview", json!({"enabled": false})).unwrap();
    h.settle();
    assert!(h.app.ui.flyout.is_some(), "the box shell keeps the open flyout");
    let strip = crate::varos::shell::hit_rects(&h.ctx)
        .into_iter()
        .find_map(|(hit, rect)| matches!(hit, crate::varos::shell::Hit::Tools).then_some(rect))
        .expect("the box shell's Tools strip");
    let strip_buttons = tool_buttons(&h, strip);
    let shell_b = strip_buttons
        .iter()
        .copied()
        .find(|&button| {
            let opened = open_tool_flyout(&mut h, button);
            if opened.as_deref() != Some(b.as_str()) {
                close_tool_flyout(&mut h);
                return false;
            }
            true
        })
        .expect("B's button in the box shell's strip");
    assert_ne!(shell_b, b_button, "the strip draws B's button elsewhere");

    h.app.run("ui.panelReview", json!({"enabled": true})).unwrap();
    h.settle();
    assert_eq!(h.layout(), &layout, "the same Design workspace presents again");
    assert!(h.app.ui.flyout.is_some(), "B is still open");
    let tools: Vec<String> = h.ctx.data(|d| d.get_temp(egui::Id::unique("flyout-tools"))).unwrap();
    assert_eq!(tools.first(), Some(&b));
    let area = h.ctx.memory(|m| m.area_rect(egui::Id::unique("tool-flyout"))).unwrap();
    near(area.top(), b_button.top());
    assert!(area.left() > h.host_rect("tools").right() || area.right() < h.host_rect("tools").left(), "beside the hosted Tools");
}
