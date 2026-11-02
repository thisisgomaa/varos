//! Reorder open document tabs, or transfer editable artwork between them without changing its source.

use egui::{Context, Id, Response};
use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_geom::Point;

use crate::{VectorcraftApp, canvas, widgets};

const HOVER_SECONDS: f64 = 0.4;
/// The outer strip width that scrolls hidden tabs into view while a tab is held there.
const TAB_SCROLL_EDGE: f32 = 18.0;

#[derive(Clone)]
struct TabDrag {
    uid: u64,
    edge: Option<TabEdge>,
    origin: egui::Rect,
    grab: egui::Vec2,
}

#[derive(Clone)]
struct TabEdge {
    direction: i32,
    since: f64,
}

fn tab_drag_id() -> Id {
    Id::unique("document-tab-reorder")
}

/// A tab reorder owns keyboard input before editor shortcuts or canvas tools see it.
/// Escape cancels only this gesture, including its pending edge-scroll timer.
pub(crate) fn tab_reorder_keys(ctx: &Context) -> bool {
    if ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())).is_none() {
        return false;
    }
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        ctx.input_mut(|input| input.events.retain(|event| !matches!(event, egui::Event::Key { key: egui::Key::Escape, .. })));
        ctx.data_mut(|data| data.remove::<TabDrag>(tab_drag_id()));
        ctx.stop_dragging();
        ctx.request_repaint();
    }
    true
}

/// Workspace command: keep each document's view alongside its stable document identity.
pub(crate) fn reorder(app: &mut VectorcraftApp, params: &serde_json::Value) -> Result<serde_json::Value, String> {
    let uid = params.get("uid").and_then(serde_json::Value::as_u64).ok_or("Missing document uid")?;
    let index =
        params.get("index").and_then(serde_json::Value::as_u64).and_then(|index| usize::try_from(index).ok()).ok_or("Invalid document tab index")?;
    let from = app.session.documents().iter().position(|document| document.uid == uid).ok_or("Unknown document uid")?;
    if index >= app.session.documents().len() {
        return Err("Document tab index is out of range".into());
    }
    app.sync_views();
    if !app.session.reorder_document(uid, index) {
        return Err("Document tab could not be moved".into());
    }
    // sync_views gives every open document a view; the checked indices remain valid after
    // removing one entry because insertion permits the end position.
    let view = app.views.remove(from);
    app.views.insert(index, view);
    Ok(json!({"uid": uid, "index": index}))
}

pub(crate) fn begin_tab_reorder(app: &VectorcraftApp, response: &Response, uid: u64, close: egui::Rect) {
    if response.drag_started()
        && !crate::dialogs::blocks_workspace(app)
        && response.ctx.input(|input| input.pointer.press_origin().is_some_and(|point| !close.contains(point)))
    {
        let press = response.ctx.input(|input| input.pointer.press_origin()).unwrap_or(response.rect.center());
        response.ctx.data_mut(|data| {
            data.insert_temp(tab_drag_id(), TabDrag { uid, edge: None, origin: response.rect, grab: press - response.rect.min });
        });
    }
}

pub(crate) struct TabPreview {
    pub positions: Vec<(u64, egui::Rect)>,
    pub dragged: Option<(u64, egui::Rect)>,
}

/// Logical insertion uses the original slot centers, never animated or pointer-following
/// rectangles. This keeps the target stable while its neighbors slide out of the way.
/// `tabs` are the tabs drawn this frame; positions count every open document. Held in a
/// scrolling edge with tabs hidden beyond it, the drop goes past all of them: to the very end
/// (right edge) or the very start (left edge), never in front of tabs the user cannot see.
fn tab_insertion(app: &VectorcraftApp, source: u64, pointer: egui::Pos2, strip: egui::Rect, tabs: &[(u64, egui::Rect)]) -> Option<usize> {
    if !strip.contains(pointer) {
        return None;
    }
    let documents = app.session.documents();
    let index_of = |uid: u64| documents.iter().position(|document| document.uid == uid);
    let from = index_of(source)?;
    let hidden_after = tabs.last().and_then(|(uid, _)| index_of(*uid)).is_some_and(|last| last + 1 < documents.len());
    let hidden_before = tabs.first().and_then(|(uid, _)| index_of(*uid)).is_some_and(|first| first > 0);
    let position = if pointer.x >= strip.right() - TAB_SCROLL_EDGE && hidden_after {
        documents.len()
    } else if pointer.x <= strip.left() + TAB_SCROLL_EDGE && hidden_before {
        0
    } else {
        let before = tabs.iter().find(|(_, rect)| pointer.x < rect.center().x);
        let (uid, after) = before.map(|(uid, _)| (*uid, false)).or_else(|| tabs.last().map(|(uid, _)| (*uid, true)))?;
        index_of(uid)? + usize::from(after)
    };
    Some(position.saturating_sub(usize::from(from < position)))
}

/// A temporary painted layout only: the held tab tracks its original grab point, and
/// neighboring tabs reach their insertion slots in 90 ms. Cancel returns to the model immediately.
pub(crate) fn tab_preview(app: &VectorcraftApp, ctx: &Context, strip: egui::Rect, tabs: &[(u64, egui::Rect)]) -> TabPreview {
    let (pointer, valid) = ctx.input(|input| {
        (
            input.pointer.interact_pos(),
            input.focused && !input.key_pressed(egui::Key::Escape) && (input.pointer.primary_down() || input.pointer.primary_released()),
        )
    });
    let drag = ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())).filter(|_| valid && !crate::dialogs::blocks_workspace(app));
    let source = drag.as_ref().and_then(|drag| app.session.documents().iter().position(|document| document.uid == drag.uid));
    let destination = drag.as_ref().zip(pointer).and_then(|(drag, pointer)| tab_insertion(app, drag.uid, pointer, strip, tabs));
    let source_visible = drag.as_ref().is_some_and(|drag| tabs.iter().any(|(uid, _)| *uid == drag.uid));
    let mut positions = Vec::with_capacity(tabs.len());
    for (uid, rect) in tabs {
        let mut target = rect.left();
        if let (Some(drag), Some(source), Some(destination)) = (&drag, source, destination)
            && *uid != drag.uid
            && let Some(index) = app.session.documents().iter().position(|document| document.uid == *uid)
        {
            if source_visible && index > source {
                target -= drag.origin.width();
            }
            let remaining_index = index.saturating_sub(usize::from(index > source));
            if remaining_index >= destination {
                target += drag.origin.width();
            }
        }
        let x = ctx.animate_value_with_time(Id::unique(("document-tab-slide", *uid)), target, if drag.is_some() { 0.09 } else { 0.0 });
        positions.push((*uid, rect.translate(egui::vec2(x - rect.left(), 0.0))));
    }
    let dragged = drag
        .zip(pointer)
        .filter(|_| source.is_some())
        .map(|(drag, pointer)| (drag.uid, egui::Rect::from_min_size(egui::pos2(pointer.x - drag.grab.x, drag.origin.top()), drag.origin.size())));
    TabPreview { positions, dragged }
}

/// Advance the existing overflow strip at most once per dwell interval. A document's UID
/// stays the drag source when its tab scrolls out of view; leaving an edge resets its timer.
pub(crate) fn tab_scroll_step(app: &VectorcraftApp, ctx: &Context, strip: Option<egui::Rect>) -> i32 {
    const DWELL: f64 = 0.35;
    const EDGE: f32 = TAB_SCROLL_EDGE;
    let Some(mut drag) = ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())) else { return 0 };
    let (now, pointer, held) = ctx.input(|input| {
        (input.time, input.pointer.interact_pos(), input.focused && input.pointer.primary_down() && !input.key_pressed(egui::Key::Escape))
    });
    let direction = if held && !crate::dialogs::blocks_workspace(app) {
        strip.zip(pointer).filter(|(strip, pointer)| strip.contains(*pointer)).and_then(|(strip, pointer)| {
            if pointer.x <= strip.left() + EDGE {
                Some(-1)
            } else if pointer.x >= strip.right() - EDGE {
                Some(1)
            } else {
                None
            }
        })
    } else {
        None
    };
    let mut step = 0;
    drag.edge = direction.map(|direction| {
        let mut edge = drag.edge.take().filter(|edge| edge.direction == direction).unwrap_or(TabEdge { direction, since: now });
        if now - edge.since >= DWELL {
            step = direction;
            edge.since = now;
        }
        let remaining = (DWELL - (now - edge.since)).max(0.0);
        ctx.request_repaint_after(std::time::Duration::try_from_secs_f64(remaining).unwrap_or(std::time::Duration::from_millis(350)));
        edge
    });
    ctx.data_mut(|data| data.insert_temp(tab_drag_id(), drag));
    step
}

/// Preview an insertion in the existing tab strip, committing only on a valid release.
/// Artwork transfers use their separate payload; a tab never becomes a canvas drop.
pub(crate) fn finish_tab_reorder(app: &mut VectorcraftApp, ui: &egui::Ui, strip: egui::Rect, tabs: &[(u64, egui::Rect)]) {
    let ctx = ui.ctx();
    let Some(drag) = ctx.data(|data| data.get_temp::<TabDrag>(tab_drag_id())) else { return };
    let cancelled = ctx.input(|input| {
        !input.focused || input.key_pressed(egui::Key::Escape) || (!input.pointer.primary_down() && !input.pointer.primary_released())
    });
    let source = app.session.documents().iter().position(|document| document.uid == drag.uid);
    if cancelled || source.is_none() || crate::dialogs::blocks_workspace(app) {
        ctx.data_mut(|data| data.remove::<TabDrag>(tab_drag_id()));
        return;
    }
    let pointer = ctx.input(|input| input.pointer.interact_pos()).filter(|point| strip.contains(*point));
    let insertion = pointer.and_then(|point| tab_insertion(app, drag.uid, point, strip, tabs));
    if ctx.input(|input| input.pointer.primary_released()) {
        ctx.data_mut(|data| data.remove::<TabDrag>(tab_drag_id()));
        if let Some(index) = insertion
            && source != Some(index)
            && let Err(error) = app.run("document.reorder", json!({"uid": drag.uid, "index": index}))
        {
            app.status(error);
        }
    }
}

#[derive(Clone)]
struct Origin {
    uid: u64,
    grab: Point,
}

#[derive(Clone)]
struct Artwork {
    origin: Origin,
    ids: Vec<NodeId>,
    revision: u64,
    target: u64,
}

#[derive(Clone)]
struct Hover {
    uid: u64,
    since: f64,
}

/// Key for the source document and original canvas grab point.
fn origin_id() -> Id {
    Id::unique("document-drag-origin")
}
/// Key for the tab identity and dwell start time of the current hover.
fn hover_id() -> Id {
    Id::unique("document-drag-hover")
}
/// Key for the pointer position latched after tab activation to prevent cascading switches.
fn activated_at_id() -> Id {
    Id::unique("document-drag-activated-at")
}

/// Record the Selection drag source and grab point, clearing any previous tab-hover timer.
pub(crate) fn begin(app: &VectorcraftApp, ctx: &Context, grab: Point) {
    ctx.data_mut(|d| {
        d.remove::<Origin>(origin_id());
        d.remove::<Hover>(hover_id());
        d.remove::<egui::Pos2>(activated_at_id());
        if app.session.tool_id() == "selection"
            && let Some(st) = app.session.active()
        {
            d.insert_temp(origin_id(), Origin { uid: st.uid, grab });
        }
    });
}

/// Called before panels. Cancelled or abandoned transfers must never reach a later mouse-up.
pub(crate) fn cleanup(app: &VectorcraftApp, ctx: &Context) {
    // Activating a tab can scroll the strip or change title widths. A tab moving under a
    // stationary pointer must not start another hover; actual pointer movement rearms it.
    let at = ctx.input(|i| i.pointer.interact_pos());
    ctx.data_mut(|d| {
        if d.get_temp::<egui::Pos2>(activated_at_id()) != at {
            d.remove::<egui::Pos2>(activated_at_id());
        }
    });
    if let Some(art) = egui::DragAndDrop::payload::<Artwork>(ctx) {
        let valid = app.session.documents().iter().any(|d| d.uid == art.origin.uid && d.revision == art.revision)
            && app.session.active().is_some_and(|d| d.uid == art.target);
        let cancel = ctx.input(|i| {
            !i.focused
                || i.key_pressed(egui::Key::Escape)
                || (!i.pointer.primary_down() && !i.pointer.primary_released())
                || i.multi_touch().is_some()
                || i.events.iter().any(|e| matches!(e, egui::Event::Touch { phase: egui::TouchPhase::Cancel, .. }))
        });
        let outside_release =
            ctx.input(|i| i.pointer.primary_released() && !app.canvas_rect.is_some_and(|r| i.pointer.interact_pos().is_some_and(|p| r.contains(p))));
        if !valid || cancel || outside_release || app.ui.dialog.is_some() {
            egui::DragAndDrop::clear_payload(ctx);
        }
    }
    if !ctx.input(|i| i.pointer.primary_down()) {
        ctx.data_mut(|d| {
            d.remove::<Origin>(origin_id());
            d.remove::<Hover>(hover_id());
            d.remove::<egui::Pos2>(activated_at_id());
        });
    }
}

/// Hover uses `contains_pointer`: the source canvas still owns egui's dragged widget.
pub(crate) fn hover_tab(app: &mut VectorcraftApp, resp: &Response, index: usize, close: egui::Rect) -> bool {
    let ctx = &resp.ctx;
    let eligible = resp.contains_pointer()
        && ctx
            .input(|i| i.pointer.primary_down() && !i.key_pressed(egui::Key::Escape) && i.pointer.interact_pos().is_some_and(|p| !close.contains(p)))
        && app.ui.dialog.is_none();
    if !eligible || app.session.active_index() == Some(index) || ctx.data(|d| d.get_temp::<egui::Pos2>(activated_at_id()).is_some()) {
        return false;
    }
    let transfer = egui::DragAndDrop::has_payload_of_type::<Artwork>(ctx);
    let art = egui::DragAndDrop::payload::<widgets::PanelDrag>(ctx).is_some_and(|p| matches!(*p, widgets::PanelDrag::Art(_)));
    if !transfer && !art {
        return false;
    }
    let Some(uid) = app.session.documents().get(index).map(|d| d.uid) else {
        return false;
    };
    let now = ctx.input(|i| i.time);
    let hover = ctx.data(|d| d.get_temp::<Hover>(hover_id())).filter(|h| h.uid == uid).unwrap_or(Hover { uid, since: now });
    ctx.data_mut(|d| d.insert_temp(hover_id(), hover.clone()));
    let remaining = HOVER_SECONDS - (now - hover.since);
    if remaining > 0.0 {
        ctx.request_repaint_after(std::time::Duration::try_from_secs_f64(remaining).unwrap_or(std::time::Duration::from_millis(400)));
        return true;
    }
    if !transfer {
        let Some(origin) = ctx.data(|d| d.get_temp::<Origin>(origin_id())) else {
            return true;
        };
        if !app
            .session
            .active()
            .is_some_and(|d| d.uid == origin.uid && d.interaction.as_ref().is_some_and(|i| i.label == "Move" || i.label == "Copy"))
        {
            return true;
        }
        if let Err(e) = app.session.cancel_interaction() {
            app.status(e.to_string());
            return true;
        }
        // Alt-drag previews use temporary duplicate ids; only the restored originals travel.
        let ids = app.session.active().map(|d| d.selection.objects.clone()).unwrap_or_default();
        if ids.is_empty() {
            return true;
        }
        canvas::end_document_drag(ctx);
        let revision = app.session.active().map(|d| d.revision).unwrap_or_default();
        egui::DragAndDrop::set_payload(ctx, Artwork { origin, ids, revision, target: uid });
    }
    if transfer && let Some(art) = egui::DragAndDrop::payload::<Artwork>(ctx) {
        let mut art = (*art).clone();
        art.target = uid;
        egui::DragAndDrop::set_payload(ctx, art);
    }
    app.ui.home = None;
    app.session.set_active(index);
    ctx.request_repaint();
    let at = ctx.input(|i| i.pointer.interact_pos());
    ctx.data_mut(|d| {
        d.remove::<Hover>(hover_id());
        if let Some(at) = at {
            d.insert_temp(activated_at_id(), at);
        }
    });
    true
}

/// Reset the dwell timer when no eligible inactive tab is under the artwork.
pub(crate) fn finish_tabs(ctx: &Context, hovering: bool) {
    if !hovering {
        ctx.data_mut(|d| d.remove::<Hover>(hover_id()));
    }
}

/// Release artwork into the active canvas; returning to the source resumes Move or Alt-copy.
pub(crate) fn drop_on_canvas(app: &mut VectorcraftApp, resp: &Response, xf: &canvas::Xf) {
    let Some(payload) = egui::DragAndDrop::payload::<Artwork>(&resp.ctx) else {
        return;
    };
    let same = app.session.active().is_some_and(|d| d.uid == payload.origin.uid);
    let copy = resp.ctx.input(|i| i.modifiers.alt);
    if resp.contains_pointer() {
        resp.ctx.set_cursor_icon(if same && !copy { egui::CursorIcon::Move } else { egui::CursorIcon::Copy });
    }
    if let Some(art) = resp.dnd_release_payload::<Artwork>()
        && let Some(at) = resp.ctx.input(|i| i.pointer.interact_pos()).map(|p| xf.to_doc(p))
    {
        let result = if same {
            // Back in the source tab, resume the ordinary move (Alt copies).
            app.run("object.move", json!({"ids": art.ids, "dx": at.x-art.origin.grab.x, "dy": at.y-art.origin.grab.y, "copy": copy}))
        } else {
            app.run(
                "edit.dropSelection",
                json!({"sourceDocument": art.origin.uid, "ids": art.ids, "grab": [art.origin.grab.x, art.origin.grab.y], "at": [at.x, at.y]}),
            )
        };
        if let Err(e) = result {
            app.status(e);
        }
    }
}

#[cfg(test)]
mod tab_reorder_tests {
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
            let input = egui::RawInput {
                time: Some(time),
                events: vec![Event::PointerMoved(point), button(point, held)],
                focused: true,
                ..Default::default()
            };
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
                viewport
                    .prev_pass
                    .widgets
                    .layers()
                    .flat_map(|(_, widgets)| widgets.iter())
                    .find(|widget| widget.id.accesskit_id() == id)
                    .unwrap()
                    .rect
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
}
