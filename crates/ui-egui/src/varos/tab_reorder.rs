//! Reorder open document tabs by dragging them in the tab strip. The strip paints a preview;
//! the order changes only through `document.reorder` on a valid release.

use egui::{Context, Id, Response};
use serde_json::json;

use crate::theme::Tokens;
use crate::{VectorcraftApp, icons};
use egui::{Stroke, Ui, vec2};

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

/// Paint the strip's drawn tabs at their preview positions, and the held tab (by document index,
/// so it paints even while scrolled out of view) above them following the pointer. `drawn` is
/// `(uid, document index, hovered, close hovered)` per tab the strip laid out; `tabs` holds every
/// open document's title galley.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_tabs(
    app: &VectorcraftApp,
    ui: &mut egui::Ui,
    strip: egui::Rect,
    rects: &[(u64, egui::Rect)],
    drawn: &[(u64, usize, bool, bool)],
    tabs: &[(String, std::sync::Arc<egui::Galley>)],
    active: Option<usize>,
    shell: bool,
) {
    let preview = tab_preview(app, ui.ctx(), strip, rects);
    for &(uid, index, hovered, close_hovered) in drawn {
        if preview.dragged.is_some_and(|(dragged, _)| dragged == uid) {
            continue;
        }
        if let Some((_, rect)) = preview.positions.iter().find(|(candidate, _)| *candidate == uid)
            && let Some((_, galley)) = tabs.get(index)
        {
            let mut paint_ui = ui.new_child(egui::UiBuilder::new().max_rect(*rect));
            paint_ui.set_clip_rect(strip.intersect(ui.clip_rect()));
            paint_tab(&paint_ui, *rect, galley, shell, Some(index) == active, hovered && preview.dragged.is_none(), close_hovered);
        }
    }
    if let Some((uid, rect)) = preview.dragged
        && let Some(index) = app.session.documents().iter().position(|document| document.uid == uid)
        && let Some((_, galley)) = tabs.get(index)
    {
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("document-tab-drag-paint"));
        let mut paint_ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(rect));
        paint_ui.set_clip_rect(strip.intersect(ui.clip_rect()));
        paint_ui.painter().rect_filled(rect, if shell { 6.0 } else { 0.0 }, Tokens::get(ui.ctx()).app_bar);
        paint_tab(&paint_ui, rect, galley, shell, Some(index) == active, false, false);
    }
}

fn paint_tab(ui: &Ui, rect: egui::Rect, galley: &std::sync::Arc<egui::Galley>, shell: bool, active: bool, hovered: bool, close_hovered: bool) {
    let t = Tokens::get(ui.ctx());
    if active {
        ui.painter().rect_filled(rect, if shell { 6.0 } else { 0.0 }, if shell { t.hover } else { t.panel });
    } else if hovered {
        ui.painter().rect_filled(rect, 0.0, t.hover.gamma_multiply(0.4));
    }
    if !shell {
        ui.painter().line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.5, t.border));
    }
    let (x, title_left) =
        if crate::chrome::CLOSE_BEFORE_TITLE { (rect.left() + 16.0, rect.left() + 32.0) } else { (rect.right() - 16.0, rect.left() + 14.0) };
    let close = egui::Rect::from_center_size(egui::pos2(x, rect.center().y), vec2(12.0, 12.0));
    icons::paint(ui, "x", close, if close_hovered { t.text_strong } else { t.text });
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).galley(
        egui::pos2(title_left, rect.center().y - galley.size().y / 2.0),
        galley.clone(),
        t.text,
    );
}

#[cfg(test)]
#[path = "tests_tab_reorder.rs"]
mod tests;
