//! The editor's recovery notice (owner decision 2026-10-06, direction B "canvas card"; design of record
//! `recovery-B-canvas-card-*.png` + `review-panel.png`). After an unexpected quit a document tab shows
//! Start's Recovered band as ONE floating card, centred in the Board box and `RC_GAP` above its bottom
//! edge: "Recovered N unsaved copies  from your last session" · Later · Review. Nothing in the layout
//! moves. Review turns the card, in place, into the Review panel: one row per copy (Discard / Restore),
//! × = Later, and the recovery setting in the footer. When the last row goes, the panel goes.
//!
//! Presentation only. The rows come from the host (`RecoveryHost`); clicks return Start's own
//! [`StartAction`]s (`Recover`, `DiscardRecovery`, `Later`), which the host turns into the very commands
//! Start's band raises — there is no second recovery state machine. The one piece of state here is
//! "the Review panel is open" (egui memory, this session). Drawn as an `egui::Area` on `Order::Middle`
//! (like the floating control bar), so the editor's layer-aware pointer test hands clicks on it to the
//! UI, never to the canvas. Not a box: it never docks. Tokens only; azure for keyboard focus only, no shadow, no animation.
// ---- Lane F: text adapters ----
use crate::shell::kit::text::ShapedResponse as _;
// ---- end Lane F ----
use crate::shell::kit::text::ShapedPainter as _;
use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{Context, Galley, Id, Rect, Sense, Ui};

use crate::shell::kit::{board as kb, Availability, Icon};
use crate::shell::tokens as t;
use crate::start::StartAction;
use crate::start_page::{format, path_galley, roles, text, text_elided, Role, RESTORE};

/// One recovery copy as the Review panel lists it (the host formats the text).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewRow {
    pub rid: String,
    pub name: String,
    /// "unsaved changes from 11:48 today" / "never saved · started 11:52 today".
    pub when: String,
    /// The original file's folder (shown in mono after `when`), when there is one.
    pub folder: Option<String>,
    /// Why this copy can't be restored (shown instead of `when`; Restore is disabled).
    pub problem: Option<String>,
    /// A Restore / Discard of this copy is running.
    pub busy: bool,
}

pub const LATER: &str = "Later";
pub const REVIEW: &str = "Review";
pub const DISCARD: &str = "Discard";
/// The card's second phrase, dropped in a narrow Board box.
pub const SUBLINE: &str = "from your last session";
pub const PANEL_TITLE: &str = "Recovered copies";
const WORKING: &str = "Working…";

/// The card's first phrase: "Recovered 1 unsaved copy" / "Recovered N unsaved copies".
pub fn headline(n: usize) -> String {
    if n == 1 {
        "Recovered 1 unsaved copy".to_string()
    } else {
        format!("Recovered {n} unsaved copies")
    }
}

/// Stable ids (tests and accessibility read their responses).
pub mod ids {
    use egui::Id;
    pub fn card() -> Id {
        Id::new("recovery-card")
    }
    pub fn panel() -> Id {
        Id::new("recovery-review-panel")
    }
    pub fn later() -> Id {
        Id::new("recovery-card-later")
    }
    pub fn save_as() -> Id {
        Id::new("save-recovered")
    }
    pub fn review() -> Id {
        Id::new("recovery-card-review")
    }
    /// The panel header's × (= Later).
    pub fn close() -> Id {
        Id::new("recovery-review-close")
    }
    pub fn restore(rid: &str) -> Id {
        Id::new(("recovery-review-restore", rid))
    }
    pub fn discard(rid: &str) -> Id {
        Id::new(("recovery-review-discard", rid))
    }
}

// ───────────────────────────── pure geometry ─────────────────────────────

/// The card keeps "from your last session" only while the Board box is at least `RC_NARROW_W` wide.
pub fn shows_subline(board: Rect) -> bool {
    board.width() >= t::RC_NARROW_W
}

/// The card's width for its parts: 20 · glyph 20 · 14 · sentence · 20 · Later · 8 · Review · 12.
pub fn card_width(sentence_w: f32, later_w: f32, review_w: f32) -> f32 {
    t::SB_RECOV_PAD_L
        + t::SB_ICON_HERO
        + t::SB_RECOV_ICON_GAP
        + sentence_w
        + t::RC_TEXT_BTN_GAP
        + later_w
        + t::RC_BTN_GAP
        + review_w
        + t::SB_RECOV_PAD_R
}

/// The card at `width`: 52 tall, centred on the Board box, its bottom `RC_GAP` above the box's bottom
/// edge; never wider than the box minus `RC_GAP` each side.
pub fn card_rect(board: Rect, width: f32) -> Rect {
    anchored_rect(board, width, t::SB_RECOV_H)
}

/// The Review panel for `rows` copies, in the card's place: `RC_PANEL_W` wide (less in a small box),
/// header + one `RC_ROW_H` row per copy + footer, growing UP from the card's bottom anchor. In a box
/// too short for every row the panel stops `RC_GAP` under the box's top and its rows scroll.
pub fn panel_rect(board: Rect, rows: usize) -> Rect {
    let inner_width = anchored_rect(board, t::RC_PANEL_W, 0.0).width() - 2.0 * t::KIT_STROKE;
    let row_h = if compact_rows(inner_width) { t::RC_STACK_ROW_H } else { t::RC_ROW_H };
    anchored_rect(board, t::RC_PANEL_W, t::RC_HEAD_H + rows as f32 * row_h + t::RC_FOOT_H)
}

fn anchored_rect(board: Rect, width: f32, height: f32) -> Rect {
    let gap = t::RC_GAP.min(board.width() / 2.0).min(board.height() / 2.0);
    let room = board.shrink(gap);
    let size = egui::vec2(width.min(room.width()).max(0.0), height.min(room.height()).max(0.0));
    Rect::from_min_size(egui::pos2(room.center().x - size.x / 2.0, room.bottom() - size.y), size)
}

fn compact_rows(width: f32) -> bool {
    width < t::RC_COMPACT_ROW_W
}

// Like Start, this floating object owns Tab order and uses the kit's pointer buttons + focus ring.
fn focus_key() -> Id {
    Id::new("recovery-keyboard-focus")
}
pub fn focused(ctx: &Context) -> Option<Id> {
    // A widget can claim focus after the Board pane has drawn this frame.
    if ctx.memory(|m| m.focused()).is_some() {
        ctx.data_mut(|d| {
            d.remove::<Id>(focus_key());
            d.remove::<Id>(Id::new("recovery-keyboard-activate"));
        });
        return None;
    }
    ctx.data(|d| d.get_temp::<Id>(focus_key()))
}
fn keyboard(ctx: &Context, order: &[Id]) -> Option<Id> {
    let mut focus = focused(ctx).filter(|id| order.contains(id));
    let widget_focused = ctx.memory(|m| m.focused()).is_some();
    let mut activated = None;
    let mut took_tab = false;
    ctx.input_mut(|i| {
        i.events.retain(|event| {
            if let egui::Event::Key { key, pressed: true, repeat, modifiers, .. } = event {
                match key {
                    egui::Key::Tab
                        if (!widget_focused || focus.is_some())
                            && !modifiers.alt
                            && !modifiers.ctrl
                            && !modifiers.command =>
                    {
                        took_tab = true;
                        if !order.is_empty() {
                            let next = focus.and_then(|id| order.iter().position(|v| *v == id)).map_or(
                                if modifiers.shift { order.len() - 1 } else { 0 },
                                |at| {
                                    if modifiers.shift {
                                        (at + order.len() - 1) % order.len()
                                    } else {
                                        (at + 1) % order.len()
                                    }
                                },
                            );
                            focus = Some(order[next]);
                        }
                        return false;
                    }
                    egui::Key::Enter | egui::Key::Space if focus.is_some() => {
                        if !repeat {
                            activated = focus;
                        }
                        return false;
                    }
                    egui::Key::Escape if focus.is_some() => {
                        focus = None;
                        activated = None;
                        return false;
                    }
                    _ => {}
                }
            }
            if matches!(event, egui::Event::PointerButton { pressed: true, .. }) {
                focus = None;
            }
            true
        });
    });
    if took_tab {
        ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
    }
    ctx.data_mut(|d| {
        d.remove::<Id>(Id::new("recovery-keyboard-activate"));
        d.remove::<Id>(focus_key());
        if let Some(id) = focus {
            d.insert_temp(focus_key(), id);
        }
        if let Some(id) = activated {
            d.insert_temp(Id::new("recovery-keyboard-activate"), id);
        }
    });
    activated
}

#[allow(clippy::too_many_arguments)]
fn button(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    label: Arc<Galley>,
    kind: kb::ButtonKind,
    availability: Availability<'_>,
    name: &str,
) -> bool {
    let focus = focused(ui.ctx()) == Some(id);
    if focus {
        ui.scroll_to_rect(rect, None);
    }
    if !rect.intersects(ui.clip_rect()) {
        return false;
    }
    let response = kb::text_button(ui, id, rect.intersect(ui.clip_rect()), label, kind, availability, focus, name);
    response.activated
        || (matches!(availability, Availability::Enabled)
            && ui.ctx().data(|d| d.get_temp::<Id>(Id::new("recovery-keyboard-activate"))) == Some(id))
}

// ───────────────────────────── state ─────────────────────────────

fn open_key() -> Id {
    Id::new("recovery-card-review-open")
}
/// Review turned the card into the panel (this session; Later or the last row closes it).
pub fn review_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_key()).unwrap_or(false))
}
fn set_review_open(ctx: &Context, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_key(), open));
}

// ───────────────────────────── paint ─────────────────────────────

/// Draw the card (or, after Review, the panel) for `rows` inside the Board box `board`. Nothing is
/// drawn — and the panel state resets — when there are no rows (the host passes none on Home, after
/// Later, and once every copy is restored or discarded). Returns this frame's clicks.
pub fn show(ctx: &Context, board: Rect, rows: &[ReviewRow], footer: &str) -> Vec<StartAction> {
    let mut actions = Vec::new();
    if rows.is_empty() {
        set_review_open(ctx, false);
        return actions;
    }
    let mut order = vec![ids::later(), ids::review()];
    if review_open(ctx) {
        order = vec![ids::close()];
        for row in rows {
            if !row.busy {
                order.push(ids::discard(&row.rid));
                if row.problem.is_none() {
                    order.push(ids::restore(&row.rid));
                }
            }
        }
    }
    keyboard(ctx, &order);
    if review_open(ctx) {
        let rect = panel_rect(board, rows.len());
        floating(ctx, ids::panel(), rect, |ui, rect| panel(ui, rect, rows, footer, &mut actions));
    } else {
        let parts = CardParts::measure(ctx, board, rows.len());
        let rect = parts.rect(board);
        let mut opened = false;
        floating(ctx, ids::card(), rect, |ui, rect| opened = card(ui, rect, parts, ids::review(), &mut actions));
        if opened {
            set_review_open(ctx, true);
            ctx.request_repaint(); // the panel replaces the card on the very next frame
        }
    }
    if actions.contains(&StartAction::Later) {
        set_review_open(ctx, false);
    }
    actions
}

/// The same floating card for a restored, pathless tab. Later is remembered per session id.
/// Returns true only for Save As; the host owns the save command and retirement of the recovery copy.
pub fn show_restored(ctx: &Context, board: Rect, sid: u64, notice: &str) -> bool {
    set_review_open(ctx, false);
    let hidden = Id::new(("restored-notice-hidden", sid));
    if ctx.data(|d| d.get_temp::<bool>(hidden).unwrap_or(false)) {
        return false;
    }
    keyboard(ctx, &[ids::later(), ids::save_as()]);
    let (headline, explanation) = notice
        .split_once(" The newest copy")
        .map_or((notice, None), |(head, tail)| (head, Some(format!("The newest copy{tail}"))));
    let mut parts = CardParts {
        headline: headline.to_owned(),
        detail: None,
        head: galley(ctx, headline, roles::BODY_MEDIUM),
        sub: None,
        later: galley(ctx, LATER, roles::SMALL_MEDIUM),
        review: galley(ctx, "Save As…", roles::SMALL_MEDIUM),
    };
    if let Some(explanation) = explanation {
        let mut job = LayoutJob::default();
        job.append(&explanation, 0.0, format(roles::SMALL));
        job.wrap.max_width = (card_rect(board, parts.width()).width() - 2.0 * t::RC_PAD_X).max(0.0);
        parts.detail = Some(ctx.fonts_mut(|f| f.layout_job(job)));
    }
    let rect = parts.rect(board);
    let mut actions = vec![];
    let mut save = false;
    floating(ctx, ids::card(), rect, |ui, rect| {
        save = card(ui, rect, parts, ids::save_as(), &mut actions);
        ui.interact(rect, Id::new("restored-notice-help"), Sense::hover()).shaped_hover_text(notice);
    });
    if actions.contains(&StartAction::Later) {
        ctx.data_mut(|d| d.insert_temp(hidden, true));
    }
    save
}

/// One floating, non-docking layer above the Board box at `rect` (Order::Middle, like the control
/// bar); the layer is exactly `rect`, so the pointer test hands every click inside it to the UI.
/// It stays on top of the other Middle layers (the artboards' on-canvas chips) and under the
/// Foreground ones (menus, the colour picker, the Export sheet).
fn floating(ctx: &Context, id: Id, rect: Rect, add: impl FnOnce(&mut Ui, Rect)) {
    // one layer for the card AND the panel: Review swaps the content in place, with no first-frame
    // sizing pass in between (egui shows a never-seen Area invisibly for one frame)
    let area = egui::Area::new(Id::new("recovery-card-layer")).order(egui::Order::Middle).fixed_pos(rect.min);
    let area = area.constrain(false);
    let shown = area.show(ctx, |ui| {
        ui.set_clip_rect(rect);
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.allocate_exact_size(rect.size(), Sense::hover());
        ui.interact(rect, id, Sense::hover());
        add(ui, rect)
    });
    ctx.move_to_top(shown.response.layer_id);
}

/// A galley in `role`, laid out on the context (before any layer exists to measure with).
fn galley(ctx: &Context, s: &str, role: Role) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(s, 0.0, format(role));
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// The card's measured text (its width follows the sentence).
struct CardParts {
    headline: String,
    detail: Option<Arc<Galley>>,
    head: Arc<Galley>,
    sub: Option<Arc<Galley>>,
    later: Arc<Galley>,
    review: Arc<Galley>,
}
impl CardParts {
    fn measure(ctx: &Context, board: Rect, count: usize) -> Self {
        CardParts {
            headline: headline(count),
            detail: None,
            head: galley(ctx, &headline(count), roles::BODY_MEDIUM),
            sub: shows_subline(board).then(|| galley(ctx, SUBLINE, roles::BODY)),
            later: galley(ctx, LATER, roles::SMALL_MEDIUM),
            review: galley(ctx, REVIEW, roles::SMALL_MEDIUM),
        }
    }
    fn stacked(&self, board: Rect) -> bool {
        let (lw, rw) = self.buttons();
        card_rect(board, self.width()).width() < card_width(t::RC_MIN_TEXT_W, lw, rw)
    }
    fn rect(&self, board: Rect) -> Rect {
        let main_h = if self.stacked(board) { t::RC_STACK_CARD_H } else { t::SB_RECOV_H };
        anchored_rect(board, self.width(), main_h + self.detail.as_ref().map_or(0.0, |g| g.size().y + t::RC_PAD_X))
    }
    fn sentence(&self) -> f32 {
        self.head.size().x + self.sub.as_ref().map_or(0.0, |s| t::SB_RECOV_TEXT_GAP + s.size().x)
    }
    fn buttons(&self) -> (f32, f32) {
        (kb::pill_width(&self.later, t::SB_BTN_PAD), kb::pill_width(&self.review, t::SB_BTN_PAD))
    }
    fn width(&self) -> f32 {
        let (lw, rw) = self.buttons();
        card_width(self.sentence(), lw, rw)
    }
}

/// The card at `rect`. Returns true when Review was clicked.
fn card(ui: &mut Ui, rect: Rect, parts: CardParts, solid_id: Id, actions: &mut Vec<StartAction>) -> bool {
    let p = ui.painter().clone();
    kb::recovery_box(&p, rect);
    let (lw, rw) = parts.buttons();
    let stacked = rect.width() < card_width(t::RC_MIN_TEXT_W, lw, rw);
    let main_h = if stacked { t::RC_STACK_CARD_H } else { t::SB_RECOV_H };
    if let Some(detail) = &parts.detail {
        p.shaped_galley(egui::pos2(rect.left() + t::RC_PAD_X, rect.top() + main_h), detail.clone(), t::MUTED);
    }
    let rect = Rect::from_min_size(rect.min, egui::vec2(rect.width(), main_h.min(rect.height())));
    let text_fits = !stacked || rect.height() >= t::RC_STACK_CARD_H;
    let text_band = if stacked { Rect::from_min_size(rect.min, egui::vec2(rect.width(), t::RC_HEAD_H)) } else { rect };
    let x = if text_fits { kb::recovered_glyph(&p, text_band) } else { rect.left() };
    let top = if stacked && text_fits {
        rect.bottom() - t::SB_RECOV_PAD_R - t::SB_BTN_H
    } else {
        rect.center().y - t::SB_BTN_H / 2.0
    };
    let review_r =
        Rect::from_min_size(egui::pos2(rect.right() - t::SB_RECOV_PAD_R - rw, top), egui::vec2(rw, t::SB_BTN_H));
    let later_r =
        Rect::from_min_size(egui::pos2(review_r.left() - t::RC_BTN_GAP - lw, top), egui::vec2(lw, t::SB_BTN_H));
    let (ghost, solid, on) = (kb::ButtonKind::Ghost, kb::ButtonKind::Solid, Availability::Enabled);
    if button(ui, ids::later(), later_r, parts.later, ghost, on, LATER) {
        actions.push(StartAction::Later);
    }
    let solid_name = parts.review.text().to_owned();
    let review = button(ui, solid_id, review_r, parts.review, solid, on, &solid_name);
    if !text_fits {
        return review;
    }
    // the sentence on one baseline; in a box too small for it the headline is elided
    let right = if stacked { rect.right() - t::SB_RECOV_PAD_R } else { later_r.left() - t::RC_TEXT_BTN_GAP };
    let line = roles::BODY_MEDIUM.line;
    let head = if parts.head.size().x > right - x {
        text_elided(ui, &parts.headline, roles::BODY_MEDIUM, (right - x).max(0.0))
    } else {
        parts.head
    };
    let hr = kb::galley_in_line(&p, x, text_band.center().y - line / 2.0, line, head.clone(), t::TEXT);
    if let Some(sub) = parts.sub.filter(|s| hr.right() + t::SB_RECOV_TEXT_GAP + s.size().x <= right + 0.5) {
        let baseline = hr.top() + kb::first_baseline(&head);
        kb::galley_at_baseline(&p, hr.right() + t::SB_RECOV_TEXT_GAP, baseline, sub, t::MUTED);
    }
    review
}

/// The Review panel: header (glyph · title · count · ×), the rows, the footer.
fn panel(ui: &mut Ui, rect: Rect, rows: &[ReviewRow], footer: &str, actions: &mut Vec<StartAction>) {
    let p = ui.painter().clone();
    kb::recovery_box(&p, rect);
    let inner = rect.shrink(t::KIT_STROKE);
    // Keep a scrolling viewport even when the Board cannot fit the usual header and footer.
    let row_room = t::SB_BTN_H.min(inner.height() / 2.0);
    // header
    let head = Rect::from_min_size(
        inner.min,
        egui::vec2(inner.width(), (t::RC_HEAD_H - t::KIT_STROKE).min(inner.height() - row_room)),
    );
    let icon_x = inner.left() + t::RC_PAD_X;
    Icon::History.paint(&p, egui::pos2(icon_x + t::ICON_MD / 2.0, head.center().y), t::ICON_MD, t::TEXT);
    let close = Rect::from_min_size(
        egui::pos2(head.right() - t::SB_RECOV_PAD_R - t::ICON_BTN_W, head.center().y - t::ICON_BTN_H / 2.0),
        egui::vec2(t::ICON_BTN_W, t::ICON_BTN_H),
    );
    let count = text(ui, &rows.len().to_string(), roles::MONO);
    let tx = icon_x + t::ICON_MD + t::RC_HEAD_ICON_GAP;
    let right = close.left() - t::RC_COUNT_GAP;
    let title =
        text_elided(ui, PANEL_TITLE, roles::PANEL_TITLE, (right - count.size().x - t::RC_COUNT_GAP - tx).max(0.0));
    let line = roles::PANEL_TITLE.line;
    let tr = kb::galley_in_line(&p, tx, head.center().y - line / 2.0, line, title.clone(), t::TEXT);
    kb::galley_at_baseline(&p, tr.right() + t::RC_COUNT_GAP, tr.top() + kb::first_baseline(&title), count, t::MUTED);
    let close = close.intersect(head);
    let response = ui.interact(close, ids::close(), Sense::CLICK);
    Icon::Remove.paint(&p, close.center(), t::ICON_LG, if response.hovered() { t::TEXT } else { t::MUTED });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, LATER));
    if focused(ui.ctx()) == Some(ids::close()) {
        kb::focus_ring(&p, close, t::R, t::PANEL);
    }
    if response.clicked()
        || ui.ctx().data(|d| d.get_temp::<Id>(Id::new("recovery-keyboard-activate"))) == Some(ids::close())
    {
        actions.push(StartAction::Later);
    }
    p.hline(inner.x_range(), head.bottom() - t::KIT_STROKE / 2.0, t::hairline());
    // rows (they scroll only when the box is too short for all of them)
    let foot = Rect::from_min_max(
        egui::pos2(inner.left(), (inner.bottom() - t::RC_FOOT_H + t::KIT_STROKE).max(head.bottom() + row_room)),
        inner.max,
    );
    let body = Rect::from_min_max(egui::pos2(inner.left(), head.bottom()), egui::pos2(inner.right(), foot.top()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
        ui.set_clip_rect(body.intersect(rect));
        egui::ScrollArea::vertical().id_salt("recovery-review-rows").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            for r in rows {
                let (row_rect, _) = ui.allocate_exact_size(
                    egui::vec2(body.width(), if compact_rows(body.width()) { t::RC_STACK_ROW_H } else { t::RC_ROW_H }),
                    Sense::hover(),
                );
                row(ui, row_rect, r, actions);
            }
        });
    });
    // footer: the recovery setting, as the host reports it
    if foot.height() < roles::MICRO.line {
        return;
    }
    let sx = inner.left() + t::RC_PAD_X;
    Icon::Shield.paint(&p, egui::pos2(sx + t::SB_ICON_SMALL / 2.0, foot.center().y), t::SB_ICON_SMALL, t::MUTED);
    let fx = sx + t::SB_ICON_SMALL + t::RC_FOOT_ICON_GAP;
    let note = text_elided(ui, footer, roles::MICRO, (foot.right() - t::RC_PAD_X - fx).max(0.0));
    kb::galley_in_line(&p, fx, foot.center().y - roles::MICRO.line / 2.0, roles::MICRO.line, note, t::MUTED);
}

/// One copy: name over "when" (or the problem) · folder; Discard (Ghost) and Restore (Solid) right.
fn row(ui: &mut Ui, rect: Rect, r: &ReviewRow, actions: &mut Vec<StartAction>) {
    let p = ui.painter().clone();
    let restore_l = text(ui, RESTORE, roles::SMALL_MEDIUM);
    let discard_l = text(ui, DISCARD, roles::SMALL_MEDIUM);
    let (rw, dw) = (kb::pill_width(&restore_l, t::SB_BTN_PAD), kb::pill_width(&discard_l, t::SB_BTN_PAD));
    let stacked = compact_rows(rect.width());
    let top = if stacked { rect.bottom() - t::RC_BTN_GAP - t::SB_BTN_H } else { rect.center().y - t::SB_BTN_H / 2.0 };
    let restore = Rect::from_min_size(
        egui::pos2(rect.right() - t::SB_RECOV_PAD_R + t::KIT_STROKE - rw, top),
        egui::vec2(rw, t::SB_BTN_H),
    );
    let discard =
        Rect::from_min_size(egui::pos2(restore.left() - t::RC_BTN_GAP - dw, top), egui::vec2(dw, t::SB_BTN_H));
    let busy = r.busy.then_some(Availability::Busy(WORKING));
    let restore_av = busy.unwrap_or(r.problem.as_deref().map_or(Availability::Enabled, Availability::Disabled));
    let discard_av = busy.unwrap_or(Availability::Enabled);
    let ghost = kb::ButtonKind::Ghost;
    if button(ui, ids::discard(&r.rid), discard, discard_l, ghost, discard_av, DISCARD) {
        actions.push(StartAction::DiscardRecovery(r.rid.clone()));
    }
    let solid = kb::ButtonKind::Solid;
    if button(ui, ids::restore(&r.rid), restore, restore_l, solid, restore_av, RESTORE) {
        actions.push(StartAction::Recover(r.rid.clone()));
    }
    // text column: from the pad to a pad short of Discard
    let x = rect.left() + t::RC_PAD_X;
    let right = if stacked { rect.right() - t::RC_PAD_X } else { discard.left() - t::RC_PAD_X };
    let width = (right - x).max(0.0);
    let name = text_elided(ui, &r.name, roles::BODY_MEDIUM, width);
    kb::galley_in_line(&p, x, rect.top() + t::RC_NAME_TOP, t::RC_NAME_LINE, name, t::TEXT);
    let when = text_elided(ui, r.problem.as_deref().unwrap_or(&r.when), roles::SMALL, width);
    let wr = kb::galley_in_line(&p, x, rect.top() + t::RC_DETAIL_TOP, t::RC_DETAIL_LINE, when.clone(), t::MUTED);
    let fx = wr.right() + t::RC_PATH_GAP;
    if let Some(folder) = r.folder.as_deref().filter(|_| fx < right) {
        let g = path_galley(ui, folder, right - fx);
        kb::galley_at_baseline(&p, fx, wr.top() + kb::first_baseline(&when), g, t::MUTED);
    }
    p.hline(rect.x_range(), rect.bottom() - t::KIT_STROKE / 2.0, t::hairline());
}
