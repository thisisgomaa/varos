use super::*;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};

struct Tabs {
    app: VectorcraftApp,
    ctx: Context,
    scope: Id,
    time: f64,
    focused: bool,
    painted: Vec<(String, egui::Rect)>,
}

impl Tabs {
    fn new() -> Self {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.ui.varos_shell.enabled = true;
        for (index, name) in ["First drawing", "Second drawing", "Third drawing"].into_iter().enumerate() {
            app.run("file.new", json!({"name": name, "width": 300, "height": 200})).unwrap();
            if index == 1 {
                app.run("shape.rectangle", json!({"x":10,"y":20,"width":40,"height":30})).unwrap();
                app.run("shape.rectangle", json!({"x":60,"y":20,"width":40,"height":30})).unwrap();
                app.run("edit.undo", json!({})).unwrap();
            }
            app.views[index].zoom = index as f64 + 1.25;
            app.views[index].rotation = index as f64 * 15.0;
        }
        let ctx = Context::default();
        crate::theme::install_fonts(&ctx);
        let mut tabs = Self { app, ctx, scope: Id::NULL, time: 0.0, focused: true, painted: Vec::new() };
        tabs.frame(vec![]);
        tabs.frame(vec![]);
        tabs
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame_after(events, 0.1);
    }

    fn frame_after(&mut self, events: Vec<Event>, elapsed: f64) {
        fn collect(shape: &egui::Shape, painted: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(text) => painted.push((text.galley.text().to_owned(), Rect::from_min_size(text.pos, text.galley.size()))),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, painted)),
                _ => {}
            }
        }
        self.time += elapsed;
        let input = egui::RawInput {
            time: Some(self.time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 400.0))),
            events,
            focused: self.focused,
            ..Default::default()
        };
        self.app.begin_host_iteration();
        let mut output = self.ctx.run_ui(input, |ui| {
            self.scope = ui.scope_id();
            crate::chrome::doc_tabs(&mut self.app, ui);
        });
        output.textures_delta.clear();
        self.painted.clear();
        for shape in &output.shapes {
            collect(&shape.shape, &mut self.painted);
        }
    }

    fn painted_title(&self, prefix: &str) -> Rect {
        let titles: Vec<_> = self.painted.iter().filter(|(text, _)| text.starts_with(prefix)).map(|(_, rect)| *rect).collect();
        assert_eq!(titles.len(), 1, "each actual tab title is painted once: {prefix}");
        titles[0]
    }

    fn rect(&self, uid: u64) -> Rect {
        self.maybe_rect(uid).expect("the actual document tab is drawn")
    }

    fn maybe_rect(&self, uid: u64) -> Option<Rect> {
        // Outside run_ui, this_pass is the recycled older buffer. Read the completed
        // pass that egui will actually use for hit-testing the next pointer event.
        self.ctx.viewport(|viewport| viewport.prev_pass.widgets.get(self.scope.with(("tab", uid))).map(|widget| widget.rect))
    }

    fn order(&self) -> Vec<u64> {
        self.app.session.documents().iter().map(|document| document.uid).collect()
    }

    fn hold(&mut self, uid: u64, to: Pos2) {
        let from = self.rect(uid).center();
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for step in 1..=4 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (step as f32 / 4.0))]);
        }
        assert_eq!(
            self.ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())).map(|drag| drag.uid),
            Some(uid),
            "the pressed tab identity owns the reorder gesture"
        );
    }

    fn release(&mut self, to: Pos2) {
        self.frame(vec![Event::PointerMoved(to), button(to, false)]);
        self.frame(vec![]);
        assert!(self.ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())).is_none());
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

#[test]
fn dragging_tabs_reorders_on_release_and_preserves_document_state_and_views() {
    let mut tabs = Tabs::new();
    let original = tabs.order();
    let active = tabs.app.session.active().unwrap().uid;
    let resets = tabs.app.session.tool_resets();
    let saved: Vec<_> = tabs
        .app
        .session
        .documents()
        .iter()
        .zip(&tabs.app.views)
        .map(|(document, view)| {
            (
                document.uid,
                document.doc.clone(),
                document.selection.clone(),
                document.is_dirty(),
                document.history.undo.len(),
                document.history.redo.len(),
                serde_json::to_value(view).unwrap(),
            )
        })
        .collect();
    let to = tabs.rect(original[2]).right_center() - vec2(4.0, 0.0);
    tabs.hold(original[0], to);
    assert_eq!(tabs.order(), original, "dragging previews insertion without mutating document order");
    tabs.release(to);
    assert_eq!(tabs.order(), [original[1], original[2], original[0]]);
    assert_eq!(tabs.app.session.active().unwrap().uid, active);
    assert_eq!(tabs.app.session.tool_resets(), resets, "reordering does not switch or reset the active tool");
    for (index, document) in tabs.app.session.documents().iter().enumerate() {
        let (_, arc, selection, dirty, undo, redo, view) = saved.iter().find(|item| item.0 == document.uid).unwrap();
        assert!(std::sync::Arc::ptr_eq(&document.doc, arc));
        assert_eq!(&document.selection, selection);
        assert_eq!(document.is_dirty(), *dirty);
        assert_eq!((document.history.undo.len(), document.history.redo.len()), (*undo, *redo));
        assert_eq!(serde_json::to_value(tabs.app.views[index]).unwrap(), *view, "camera state follows its document");
    }
    let to = tabs.rect(original[1]).left_center() + vec2(3.0, 0.0);
    tabs.hold(original[0], to);
    assert!(tabs.app.ui.dialog.is_none(), "tab dragging does not open a dialog: {:?}", tabs.app.ui.dialog.as_ref().map(|dialog| &dialog.kind));
    tabs.release(to);
    assert_eq!(
        tabs.order(),
        original,
        "the same tab can move back to the first position; at={to:?}, first={:?}, status={}, dialog={:?}",
        tabs.rect(original[1]),
        tabs.app.ui.status,
        tabs.app.ui.dialog.as_ref().map(|dialog| &dialog.kind)
    );
    assert_eq!(tabs.app.session.active().unwrap().uid, active);
}

#[test]
fn held_tab_paint_follows_pointer_and_neighbors_animate_before_commit_or_cancel() {
    let mut tabs = Tabs::new();
    let original = tabs.order();
    let source = tabs.rect(original[0]);
    let title = tabs.painted_title("First drawing");
    let neighbor = tabs.painted_title("Second drawing");
    let from = source.center();
    let to = tabs.rect(original[2]).right_center() - vec2(4.0, -3.0);
    tabs.frame(vec![Event::PointerMoved(from)]);
    tabs.frame(vec![button(from, true)]);
    tabs.frame_after(vec![Event::PointerMoved(to)], 0.01);
    assert_eq!(tabs.order(), original, "live positioning is still a reversible preview");
    let held = tabs.painted_title("First drawing");
    assert!((held.min - title.min - vec2(to.x - from.x, 0.0)).length() < 0.6, "the held title follows horizontal movement only");
    tabs.frame_after(vec![], 0.04);
    let intermediate = tabs.painted_title("Second drawing").left();
    assert!(intermediate < neighbor.left() - 0.5, "the neighboring tab starts moving while the pointer is held");
    assert!(intermediate > neighbor.left() - source.width() + 0.5, "the neighbor slides rather than jumping instantly");
    tabs.frame_after(vec![], 0.12);
    assert!((tabs.painted_title("Second drawing").left() - (neighbor.left() - source.width())).abs() < 0.6);
    assert_eq!(tabs.order(), original, "even the completed visual animation leaves committed order untouched");
    tabs.frame_after(vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 0.01);
    assert_eq!(tabs.order(), original);
    assert!((tabs.painted_title("First drawing").min - title.min).length() < 0.6);
    assert!((tabs.painted_title("Second drawing").min - neighbor.min).length() < 0.6);
    tabs.release(to);
    assert_eq!(tabs.order(), original, "a later release cannot commit the cancelled visual preview");
}

#[test]
fn cancelled_tab_drags_never_reorder_or_close_documents() {
    for cancellation in ["outside", "escape", "focus"] {
        let mut tabs = Tabs::new();
        let original = tabs.order();
        let active = tabs.app.session.active().unwrap().uid;
        let inside = tabs.rect(original[2]).right_center() - vec2(4.0, 0.0);
        tabs.hold(original[0], inside);
        let mut to = inside;
        match cancellation {
            "outside" => to = pos2(inside.x, 150.0),
            "escape" => {
                tabs.frame(vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }])
            }
            "focus" => {
                tabs.focused = false;
                tabs.frame(vec![]);
                tabs.focused = true;
            }
            _ => panic!("unknown cancellation case"),
        }
        tabs.release(to);
        assert_eq!(tabs.order(), original, "cancelled by {cancellation}");
        assert_eq!(tabs.app.session.active().unwrap().uid, active);
    }
}

#[test]
fn document_reorder_command_checks_inputs_and_moves_the_active_tab_without_switching_it() {
    let mut tabs = Tabs::new();
    let original = tabs.order();
    let views = serde_json::to_value(&tabs.app.views).unwrap();
    for params in [json!({}), json!({"uid":u64::MAX,"index":0}), json!({"uid":original[0],"index":3}), json!({"uid":original[0],"index":-1})] {
        assert!(tabs.app.run("document.reorder", params).is_err());
        assert_eq!(tabs.order(), original);
        assert_eq!(serde_json::to_value(&tabs.app.views).unwrap(), views);
    }
    let active = tabs.app.session.active().unwrap().uid;
    let resets = tabs.app.session.tool_resets();
    tabs.app.run("document.reorder", json!({"uid":active,"index":0})).unwrap();
    assert_eq!(tabs.order(), [original[2], original[0], original[1]]);
    assert_eq!(tabs.app.session.active_index(), Some(0));
    assert_eq!(tabs.app.session.active().unwrap().uid, active);
    assert_eq!(tabs.app.session.tool_resets(), resets);
}

#[test]
fn held_tab_edge_dwell_reaches_hidden_tabs_in_both_directions_without_changing_documents() {
    let mut tabs = Tabs::new();
    for index in 3..18 {
        tabs.app.run("file.new", json!({"name": format!("Overflow drawing {index}"), "width":300,"height":200})).unwrap();
        tabs.app.views[index].zoom = index as f64 + 1.25;
    }
    tabs.app.session.set_active(0);
    tabs.frame(vec![]);
    let original = tabs.order();
    let source = original[0];
    let last = *original.last().unwrap();
    let resets = tabs.app.session.tool_resets();
    let saved: Vec<_> = tabs
        .app
        .session
        .documents()
        .iter()
        .zip(&tabs.app.views)
        .map(|(document, view)| {
            (
                document.uid,
                document.doc.clone(),
                document.is_dirty(),
                document.history.undo.len(),
                document.history.redo.len(),
                serde_json::to_value(view).unwrap(),
            )
        })
        .collect();
    assert!(tabs.maybe_rect(last).is_none(), "the destination begins outside the visible tab window");
    // The existing overflow arrows occupy the strip's right end; the shell reserves
    // another 28 points for New Document immediately after the reorderable region.
    let previous_arrow =
        tabs.ctx.viewport(|viewport| viewport.prev_pass.widgets.get(tabs.scope.with("chevron-left")).map(|widget| widget.rect)).unwrap();
    let right = pos2(previous_arrow.left() - 28.0 - 5.0, previous_arrow.center().y);
    tabs.hold(source, right);
    for _ in 0..80 {
        if tabs.maybe_rect(last).is_some() {
            break;
        }
        tabs.frame(vec![]);
    }
    assert!(tabs.maybe_rect(source).is_none(), "the source UID survives scrolling its tab out of view");
    assert!(tabs.painted_title("First drawing").is_positive(), "the actual held tab stays painted after its original slot scrolls offscreen");
    let to = tabs.rect(last).right_center() - vec2(4.0, 0.0);
    assert_eq!(tabs.order(), original, "autoscroll alone does not reorder documents");
    tabs.release(to);
    let mut expected = original[1..].to_vec();
    expected.push(source);
    assert_eq!(tabs.order(), expected);
    assert_eq!(tabs.app.session.active().unwrap().uid, source);

    // The active moved tab is revealed at the end. Hold it at the opposite edge
    // until the original second document becomes reachable, then put it first.
    let visible_left = tabs
        .app
        .session
        .documents()
        .iter()
        .filter_map(|document| tabs.maybe_rect(document.uid))
        .map(|rect| rect.left())
        .fold(f32::INFINITY, f32::min);
    let left = pos2(visible_left + 5.0, tabs.rect(source).center().y);
    tabs.hold(source, left);
    for _ in 0..80 {
        if tabs.maybe_rect(original[1]).is_some() {
            break;
        }
        tabs.frame(vec![]);
    }
    let to = tabs.rect(original[1]).left_center() + vec2(3.0, 0.0);
    tabs.release(to);
    assert_eq!(tabs.order(), original);
    assert_eq!(tabs.app.session.active().unwrap().uid, source);
    assert_eq!(tabs.app.session.tool_resets(), resets);
    for (index, document) in tabs.app.session.documents().iter().enumerate() {
        let (_, arc, dirty, undo, redo, view) = saved.iter().find(|saved| saved.0 == document.uid).unwrap();
        assert!(std::sync::Arc::ptr_eq(&document.doc, arc));
        assert_eq!((document.is_dirty(), document.history.undo.len(), document.history.redo.len()), (*dirty, *undo, *redo));
        assert_eq!(serde_json::to_value(tabs.app.views[index]).unwrap(), *view);
    }
}

/// G3: released in a scrolling edge before the dwell scrolls, with tabs hidden beyond that
/// edge, a tab goes past all of them (the end, or the start), never in front of hidden tabs.
/// Checked with the panel workspace presenting the editor.
#[test]
fn releasing_in_a_scroll_edge_before_the_dwell_drops_past_the_hidden_tabs() {
    let mut tabs = Tabs::new();
    tabs.app.ui.varos_panel_review = Some(Default::default());
    tabs.app.ui.varos_panel_review.as_mut().unwrap().layout = crate::varos::panel_system::model::Layout::studio_workspace().unwrap();
    assert!(crate::varos::panel_system::enabled(&tabs.app));
    for index in 3..18 {
        tabs.app.run("file.new", json!({"name": format!("Overflow drawing {index}"), "width":300,"height":200})).unwrap();
    }
    tabs.app.session.set_active(0);
    tabs.frame(vec![]);
    tabs.frame(vec![]);
    let original = tabs.order();
    let (source, last) = (original[0], *original.last().unwrap());
    assert!(tabs.maybe_rect(last).is_none(), "tabs are hidden beyond the right edge");
    let previous_arrow =
        tabs.ctx.viewport(|viewport| viewport.prev_pass.widgets.get(tabs.scope.with("chevron-left")).map(|widget| widget.rect)).unwrap();
    let right = pos2(previous_arrow.left() - 28.0 - 5.0, previous_arrow.center().y);
    tabs.hold(source, right);
    assert_eq!(tabs.order(), original);
    tabs.release(right);
    let mut expected = original[1..].to_vec();
    expected.push(source);
    assert_eq!(tabs.order(), expected, "the tab lands at the very end, after the hidden tabs");

    // The moved (active) tab is revealed at the end; tabs are now hidden before the left edge.
    tabs.frame(vec![]);
    tabs.frame(vec![]);
    assert!(tabs.maybe_rect(original[1]).is_none(), "tabs are hidden beyond the left edge");
    let visible_left = tabs
        .app
        .session
        .documents()
        .iter()
        .filter_map(|document| tabs.maybe_rect(document.uid))
        .map(|rect| rect.left())
        .fold(f32::INFINITY, f32::min);
    let left = pos2(visible_left + 5.0, tabs.rect(source).center().y);
    tabs.hold(source, left);
    tabs.release(left);
    assert_eq!(tabs.order(), original, "the tab lands at the very start, before the hidden tabs");
}

#[test]
fn leaving_a_tab_scroll_edge_resets_its_dwell_and_a_stall_advances_only_once() {
    let mut tabs = Tabs::new();
    let uid = tabs.order()[0];
    let strip = Rect::from_min_size(Pos2::ZERO, vec2(500.0, 28.0));
    let at = pos2(495.0, 14.0);
    tabs.frame(vec![Event::PointerMoved(at), button(at, true)]);
    tabs.ctx.data_mut(|data| data.insert_temp(tab_drag_id(), TabDrag { uid, edge: None, origin: strip, grab: egui::Vec2::ZERO }));
    assert_eq!(tab_scroll_step(&tabs.app, &tabs.ctx, Some(strip)), 0);
    tabs.time += 10.0;
    // These frames draw no tabs: the timer is exercised without chrome polling it first.
    let poll = |time, point, held| {
        let input =
            egui::RawInput { time: Some(time), events: vec![Event::PointerMoved(point), button(point, held)], focused: true, ..Default::default() };
        let mut step = 0;
        let mut output = tabs.ctx.run_ui(input, |_| {
            step = tab_scroll_step(&tabs.app, &tabs.ctx, Some(strip));
        });
        output.textures_delta.clear();
        step
    };
    assert_eq!(poll(tabs.time, at, true), 1, "a long stall cannot skip several windows");
    assert_eq!(poll(tabs.time + 0.1, pos2(250.0, 14.0), true), 0);
    assert_eq!(poll(tabs.time + 0.2, at, true), 0, "re-entering starts a fresh dwell");
    assert_eq!(poll(tabs.time + 0.4, at, true), 0);
    assert_eq!(poll(tabs.time + 0.6, at, true), 1);
    assert_eq!(poll(tabs.time + 1.0, at, false), 0, "release never scrolls");
    assert!(tabs.ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())).unwrap().edge.is_none());
}

#[test]
fn whole_window_tab_drag_owns_escape_and_tool_shortcuts_without_changing_artwork() {
    fn frame(app: &mut VectorcraftApp, ctx: &Context, time: &mut f64, events: Vec<Event>) -> egui::FullOutput {
        *time += 0.1;
        app.begin_host_iteration();
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(*time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 1000.0))),
                events,
                focused: true,
                max_texture_side: Some(8192),
                ..Default::default()
            },
            |ui| app.ui(ui),
        );
        output.textures_delta.clear();
        assert!(!app.ui.status.starts_with("Internal error:"), "{}", app.ui.status);
        output
    }
    let mut tabs = Tabs::new();
    tabs.app.canvas.worker_started = true;
    tabs.app.custom_titlebar = true;
    tabs.app.ui.varos_panel_review = Some(Default::default());
    tabs.app.ui.varos_panel_review.as_mut().unwrap().layout = crate::varos::panel_system::model::Layout::studio_workspace().unwrap();
    let object = tabs.app.run("shape.rectangle", json!({"x":20,"y":20,"width":60,"height":40})).unwrap()["id"].clone();
    tabs.app.run("select.set", json!({"ids":[object]})).unwrap();
    tabs.app.select_tool("selection");
    for _ in 0..5 {
        frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![]);
    }
    tabs.ctx.enable_accesskit();
    let output = frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![]);
    let tree = output.platform_output.accesskit_update.unwrap();
    let tab_rect = |title: &str| {
        let id = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == egui::accesskit::Role::Tab && node.label().is_some_and(|label| label.starts_with(title)))
            .unwrap()
            .0;
        tabs.ctx.viewport(|viewport| {
            viewport.prev_pass.widgets.layers().flat_map(|(_, widgets)| widgets.iter()).find(|widget| widget.id.accesskit_id() == id).unwrap().rect
        })
    };
    let from = tab_rect("First drawing").center();
    let to = tab_rect("Second drawing").right_center() - vec2(4.0, 0.0);
    let original = tabs.order();
    let active = tabs.app.session.active().unwrap().uid;
    let document = tabs.app.session.active().unwrap().doc.clone();
    let selection = tabs.app.session.active().unwrap().selection.clone();
    let history = (tabs.app.session.active().unwrap().history.undo.len(), tabs.app.session.active().unwrap().history.redo.len());
    let key = |key, pressed| Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Modifiers::NONE };
    frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![Event::PointerMoved(from)]);
    frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![button(from, true)]);
    for step in 1..=4 {
        frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![Event::PointerMoved(from + (to - from) * (step as f32 / 4.0))]);
    }
    assert_eq!(tabs.ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())).unwrap().uid, original[0]);
    frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![key(Key::P, true), Event::Text("p".into())]);
    frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![key(Key::P, false)]);
    assert_eq!(tabs.app.session.tool_id(), "selection", "held tab drags own editor letter shortcuts");
    frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![key(Key::Escape, true)]);
    assert!(tabs.ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())).is_none(), "Escape cancels the tab gesture");
    frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![key(Key::Escape, false), button(to, false)]);
    frame(&mut tabs.app, &tabs.ctx, &mut tabs.time, vec![]);
    assert_eq!(tabs.order(), original);
    let current = tabs.app.session.active().unwrap();
    assert_eq!(current.uid, active);
    assert!(std::sync::Arc::ptr_eq(&current.doc, &document));
    assert_eq!(current.selection, selection, "Escape does not clear the document selection behind the tabs");
    assert_eq!((current.history.undo.len(), current.history.redo.len()), history);
    assert_eq!(tabs.app.session.tool_id(), "selection");
}
