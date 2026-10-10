//! THE field edit transaction (UI_SYSTEM §K3, piece P2) — implemented once, used by every text and
//! number field in the app. A field edit is a small session that opens when the field takes the
//! keyboard and ends exactly once:
//!
//! | event while editing                    | valid text                 | text that does not parse     |
//! |----------------------------------------|----------------------------|------------------------------|
//! | Enter · Tab / ⇧Tab · click elsewhere   | commit, close              | keep the keyboard + reason   |
//! | lifecycle command / canvas press       | commit first ([`settle`])  | the command does not run     |
//! | the field stops being drawn            | commit ([`end_frame`])     | revert silently              |
//! | Esc                                    | revert, close              | revert, close                |
//!
//! Unchanged text never commits (no undo step, nothing dirty). The session lives in egui temp memory
//! keyed by the field's id (the caller salts ids per document), so it survives the frame in which egui
//! already dropped the focus — the frame a click-away is seen in (the old `name_field` rebuilt its
//! buffer from the model there and committed the ORIGINAL text).
//!
//! Kit rule (K1): no binary types. A field reports values; the caller turns them into commands. The
//! value a commit would carry right now is reported every frame as [`Edit::pending`], so the host can
//! commit an open field before a command without running another frame.
// ---- Lane F: text adapters ----
use crate::shell::kit::text::ShapedResponse as _;
// ---- end Lane F ----
use crate::shell::kit::text::ShapedPainter as _;
use std::ops::RangeInclusive;

use egui::text::{CCursor, CCursorRange};
use egui::{Align2, FontId, Id, Key, Modifiers, Rect, Response, Sense, Stroke, StrokeKind, TextStyle, Ui};

use crate::shell::tokens as t;

/// The reason a number field gives for text that does not parse.
pub const NOT_A_NUMBER: &str = "Type a number";
/// The reason a name field gives for a name that is empty after trimming.
pub const EMPTY_NAME: &str = "A name can't be empty";

/// What one field did this frame. Each value is reported at most once.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit<T> {
    pub id: Id,
    /// Where the field sits (its box).
    pub rect: Rect,
    /// Committed now (Enter, Tab, click elsewhere): changed AND valid. One undo step for the caller.
    pub commit: Option<T>,
    /// Number fields only: an arrow-key step or a scrub, applied at once (today one step each — the
    /// one-undo arrow/scrub session is U3-T).
    pub live: Option<T>,
    /// While editing: the value a commit would carry now (valid and changed), for [`settle`] /
    /// [`end_frame`] commits that happen between frames.
    pub pending: Option<T>,
    /// The session ended this frame (commit, unchanged close, Esc) — transient editors close.
    pub closed: bool,
    /// A session is open after this frame.
    pub editing: bool,
}
impl<T> Edit<T> {
    fn new(id: Id, rect: Rect) -> Self {
        Edit { id, rect, commit: None, live: None, pending: None, closed: false, editing: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Validity {
    Unchanged,
    Changed,
    Invalid(&'static str),
}

#[derive(Clone, Debug)]
struct Session {
    /// The text at focus-in: Esc returns here, "unchanged" compares with it.
    orig: String,
    buf: String,
    /// As of the field's last frame — what [`settle`] / [`end_frame`] decide on.
    validity: Validity,
    /// Shown under the field after a refused commit; cleared by the next valid keystroke or Esc.
    reason: Option<&'static str>,
    /// Opened, but the text edit has not held the keyboard yet (a number field's click-to-type frame,
    /// a transient editor's opening frame): not having focus is not a blur yet.
    arm: bool,
    /// Number fields: the precise value behind arrow steps (FB4) — "42" + 0.1 still shows "42".
    acc: Option<f32>,
    /// The egui pass the field was last drawn in. [`end_frame`] keeps the session only if that is the
    /// pass the frame ENDED with (egui may re-run a frame: a field drawn in a discarded first pass and
    /// gone from the second is gone).
    pass: u64,
}
impl Session {
    fn new(text: &str) -> Self {
        Session {
            orig: text.into(),
            buf: text.into(),
            validity: Validity::Unchanged,
            reason: None,
            arm: true,
            acc: None,
            pass: 0,
        }
    }
}

fn session_key(id: Id) -> Id {
    id.with("varos-kit-field")
}
fn open_key() -> Id {
    Id::new("varos-kit-field-open")
}
fn order_key() -> Id {
    Id::new("varos-kit-field-order")
}
fn group_key() -> Id {
    Id::new("varos-kit-field-group")
}
fn tab_key() -> Id {
    Id::new("varos-kit-field-tab-to")
}

fn load(ctx: &egui::Context, id: Id) -> Option<Session> {
    ctx.data(|d| d.get_temp::<Session>(session_key(id)))
}
fn store(ctx: &egui::Context, id: Id, s: Session) {
    ctx.data_mut(|d| {
        d.insert_temp(session_key(id), s);
        let open = d.get_temp_mut_or_default::<Vec<Id>>(open_key());
        if !open.contains(&id) {
            open.push(id);
        }
    });
}
/// Close the session on `id` and give up its keyboard.
fn end(ctx: &egui::Context, id: Id) {
    super::text::editor::clear(ctx, id);
    ctx.data_mut(|d| {
        d.remove::<Session>(session_key(id));
        d.get_temp_mut_or_default::<Vec<Id>>(open_key()).retain(|o| *o != id);
    });
    ctx.memory_mut(|m| m.surrender_focus(id));
}
fn open_ids(ctx: &egui::Context) -> Vec<Id> {
    ctx.data(|d| d.get_temp::<Vec<Id>>(open_key())).unwrap_or_default()
}
/// Another field still holds text that does not parse: it keeps the keyboard (K3 "keep focus").
fn blocker(ctx: &egui::Context, me: Id) -> Option<Id> {
    open_ids(ctx)
        .into_iter()
        .filter(|o| *o != me)
        .find(|o| load(ctx, *o).is_some_and(|s| matches!(s.validity, Validity::Invalid(_))))
}

fn select_all(ctx: &egui::Context, id: Id, text: &str) {
    super::text::editor::select_all(ctx, id, text);
    let mut st = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(text.chars().count()))));
    st.store(ctx, id);
}

/// Open a session on `id` showing `text`, all selected (typing replaces it), unless another field
/// holds invalid text — then that one gets the keyboard back and this one does not open.
fn begin(ctx: &egui::Context, id: Id, text: &str) -> Option<Session> {
    if let Some(b) = blocker(ctx, id) {
        ctx.memory_mut(|m| m.request_focus(b));
        return None;
    }
    select_all(ctx, id, text);
    ctx.memory_mut(|m| m.request_focus(id));
    let s = Session { pass: ctx.cumulative_pass_nr(), ..Session::new(text) };
    store(ctx, id, s.clone());
    Some(s)
}

/// Esc while a session is open. egui already dropped the keyboard at the start of the frame (a text
/// edit does not filter Esc), so focus cannot be asked — the session's existence is the edit.
fn escaped(ui: &Ui) -> bool {
    ui.input(|i| i.key_pressed(Key::Escape))
}

/// Tab / ⇧Tab while this field holds the keyboard. egui's own walk is stopped (it lands on any
/// focusable widget, buttons included); the session commits and hands the keyboard to the next FIELD
/// of the same group. `Some(true)` = backwards.
fn take_tab(ui: &Ui, id: Id) -> Option<bool> {
    if !ui.memory(|m| m.has_focus(id)) {
        return None;
    }
    // ⇧Tab first: `consume_key` ignores an extra Shift
    let back = ui.input_mut(|i| {
        if i.consume_key(Modifiers::SHIFT, Key::Tab) {
            Some(true)
        } else if i.consume_key(Modifiers::NONE, Key::Tab) {
            Some(false)
        } else {
            None
        }
    })?;
    ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
    Some(back)
}

// ── Tab order: fields register in draw order, per group (a panel) ──

#[derive(Clone, Default)]
struct Order {
    pass: u64,
    cur: Vec<(Id, Id)>,
    prev: Vec<(Id, Id)>,
}

fn register(ui: &Ui, id: Id) {
    let pass = ui.ctx().cumulative_pass_nr();
    let group = ui.data(|d| d.get_temp::<Id>(group_key())).unwrap_or(Id::NULL);
    ui.data_mut(|d| {
        let o = d.get_temp_mut_or_default::<Order>(order_key());
        if o.pass != pass {
            std::mem::swap(&mut o.prev, &mut o.cur); // reuse both lists: no allocation per frame
            o.cur.clear();
            o.pass = pass;
        }
        if !o.cur.iter().any(|(_, f)| *f == id) {
            o.cur.push((group, id));
        }
    });
}

/// The field after (or before) `id` in its group, wrapping; `None` when it is alone.
fn next_field(ctx: &egui::Context, id: Id, back: bool) -> Option<Id> {
    let o = ctx.data(|d| d.get_temp::<Order>(order_key())).unwrap_or_default();
    // the last complete pass, unless the field is new this pass
    let list = if o.prev.iter().any(|(_, f)| *f == id) { o.prev } else { o.cur };
    let group = list.iter().find(|(_, f)| *f == id)?.0;
    let ring: Vec<Id> = list.iter().filter(|(g, _)| *g == group).map(|(_, f)| *f).collect();
    let (at, n) = (ring.iter().position(|f| *f == id)?, ring.len());
    (n > 1).then(|| ring[if back { (at + n - 1) % n } else { (at + 1) % n }])
}

fn take_tab_to(ctx: &egui::Context, id: Id) -> bool {
    ctx.data_mut(|d| {
        let hit = d.get_temp::<Id>(tab_key()) == Some(id);
        if hit {
            d.remove::<Id>(tab_key());
        }
        hit
    })
}

/// Fields drawn inside `add` form one Tab ring (K3: "focus to the next / previous field of the same
/// panel, wrapping; never a button").
pub fn group<R>(ui: &mut Ui, group: impl std::hash::Hash + std::fmt::Debug, add: impl FnOnce(&mut Ui) -> R) -> R {
    let key = group_key();
    let old = ui.data(|d| d.get_temp::<Id>(key));
    ui.data_mut(|d| d.insert_temp(key, Id::new(group)));
    let r = add(ui);
    ui.data_mut(|d| match old {
        Some(o) => {
            d.insert_temp(key, o);
        }
        None => d.remove::<Id>(key),
    });
    r
}

/// The refused-commit reason, under the field on PANEL in `ERROR` at the small text size (K3 rule 3).
/// Painted above the panels so it never pushes the layout.
fn paint_reason(ui: &Ui, id: Id, rect: Rect, reason: &str) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, id.with("reason")));
    let galley = painter.layout_no_wrap(reason.to_string(), TextStyle::Small.resolve(ui.style()), t::ERROR);
    let pad = egui::vec2(t::KIT_PAD / 2.0, t::KIT_TEXT_GAP / 2.0);
    let bg =
        Rect::from_min_size(rect.left_bottom() + egui::vec2(0.0, t::KIT_TEXT_GAP / 2.0), galley.size() + pad * 2.0);
    painter.rect(bg, t::r_ctrl(), t::PANEL, Stroke::new(t::KIT_STROKE, t::ERROR), StrokeKind::Inside);
    painter.shaped_galley(bg.min + pad, galley, t::ERROR);
}

/// One frame of an open session after its text edit ran: validate the text and end the session on
/// Enter / Tab / blur. Returns the session if it is still open.
fn finish<T: PartialEq>(
    ctx: &egui::Context,
    id: Id,
    mut s: Session,
    buf: String,
    tab: Option<bool>,
    parse: &dyn Fn(&str) -> Result<T, &'static str>,
    out: &mut Edit<T>,
) -> Option<Session> {
    s.pass = ctx.cumulative_pass_nr(); // drawn in this pass
    let focused = ctx.memory(|m| m.has_focus(id));
    if focused {
        s.arm = false;
    } else if s.arm {
        ctx.memory_mut(|m| m.request_focus(id)); // still opening: take the keyboard (once it lands, arm ends)
    }
    let typed = buf != s.buf;
    s.buf = buf;
    let parsed = parse(&s.buf);
    s.validity = match &parsed {
        _ if s.orig.is_empty() && s.buf.trim().is_empty() => Validity::Unchanged,
        Err(reason) => Validity::Invalid(reason),
        Ok(v) if parse(&s.orig).as_ref().ok() == Some(v) => Validity::Unchanged,
        Ok(_) => Validity::Changed,
    };
    if typed && parsed.is_ok() {
        s.reason = None;
    }
    let ending = tab.is_some() || (!focused && !s.arm);
    if !ending {
        if s.validity == Validity::Changed {
            out.pending = parsed.ok();
        }
        out.editing = true;
        store(ctx, id, s.clone());
        return Some(s);
    }
    if let Validity::Invalid(reason) = s.validity {
        s.reason = Some(reason);
        ctx.memory_mut(|m| m.request_focus(id));
        out.editing = true;
        store(ctx, id, s.clone());
        return Some(s);
    }
    if s.validity == Validity::Changed {
        out.commit = parsed.ok();
    }
    end(ctx, id);
    out.closed = true;
    if let Some(next) = tab.and_then(|back| next_field(ctx, id, back)) {
        ctx.data_mut(|d| d.insert_temp(tab_key(), next));
    }
    None
}

/// A single-line text field.
pub struct TextField<'a> {
    pub id: Id,
    pub rect: Rect,
    /// The model's value — shown while not editing, and the text a session starts from.
    pub value: &'a str,
    pub font: FontId,
    /// Paint the box (SURFACE + hairline, ACCENT while editing); `false` = bare text on the caller's row.
    pub framed: bool,
    /// A transient editor (Layers rename, on-canvas artboard name): editing starts as soon as it is
    /// drawn, all selected; it is not a Tab stop. The caller stops drawing it once `closed`.
    pub open: bool,
    pub hint: &'a str,
}

/// A text field under the K3 law. `parse` decides what the text means (and its reason when it means
/// nothing); "unchanged" is `parse(text) == parse(text at focus-in)`.
pub fn text_field<T: PartialEq>(
    ui: &mut Ui,
    f: TextField<'_>,
    parse: impl Fn(&str) -> Result<T, &'static str>,
) -> Edit<T> {
    edit_text(ui, f, 1, parse)
}

/// A wrapping text box `rows` lines tall (the board description) under the same K3 law. Its text holds
/// no line breaks, so Enter commits exactly as in [`text_field`]; long text wraps inside the box.
pub fn text_area<T: PartialEq>(
    ui: &mut Ui,
    f: TextField<'_>,
    rows: usize,
    parse: impl Fn(&str) -> Result<T, &'static str>,
) -> Edit<T> {
    edit_text(ui, f, rows.max(1), parse)
}

fn edit_text<T: PartialEq>(
    ui: &mut Ui,
    f: TextField<'_>,
    rows: usize,
    parse: impl Fn(&str) -> Result<T, &'static str>,
) -> Edit<T> {
    let ctx = ui.ctx().clone();
    let id = f.id;
    if !f.open {
        register(ui, id);
    }
    let mut out = Edit::new(id, f.rect);
    let mut sess = load(&ctx, id);
    if sess.is_some() && escaped(ui) {
        end(&ctx, id);
        sess = None;
        out.closed = true;
    } else if sess.is_none() && (f.open || take_tab_to(&ctx, id)) {
        sess = begin(&ctx, id, f.value);
    }
    let tab = if sess.is_some() { take_tab(ui, id) } else { None };
    if f.framed {
        let border = if sess.is_some() { t::ACCENT } else { t::LINE };
        ui.painter().rect(f.rect, t::r_ctrl(), t::SURFACE, Stroke::new(t::KIT_STROKE, border), StrokeKind::Middle);
    }
    let mut buf = sess.as_ref().map_or_else(|| f.value.to_string(), |s| s.buf.clone());
    let inner = if f.framed { f.rect.shrink2(egui::vec2(t::FIELD_INSET_X, t::FIELD_INSET_Y)) } else { f.rect };
    // a multi-row box: Enter is the commit key (it would otherwise insert a line break the text may not hold)
    let enter =
        rows > 1 && ctx.memory(|m| m.has_focus(id)) && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    super::text::editor::show(ui, id, inner, &mut buf, f.font.clone(), f.hint, rows > 1);
    // ---- Lane G (integration w3: the hint is spoken as painted, catalog-resolved; the value is authored) ----
    super::super::accessibility::emit(
        ui,
        id,
        if rows > 1 { accesskit::Role::MultilineTextInput } else { accesskit::Role::TextInput },
        &crate::i18n::translate(ui.ctx(), if f.hint.is_empty() { "Text" } else { f.hint }),
        Some(&buf),
        ui.is_enabled(),
    );
    if enter {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
    let sess = match sess {
        Some(s) => Some(s),
        // egui gave the field the keyboard (a click into it): the session starts from the shown value
        None if !out.closed && ctx.memory(|m| m.has_focus(id)) => match blocker(&ctx, id) {
            Some(b) => {
                ctx.memory_mut(|m| {
                    m.surrender_focus(id);
                    m.request_focus(b);
                });
                None
            }
            None => Some(Session { arm: false, ..Session::new(f.value) }),
        },
        None => None,
    };
    if let Some(s) = sess {
        if let Some(s) = finish(&ctx, id, s, buf, tab, &parse, &mut out) {
            if let Some(reason) = s.reason {
                paint_reason(ui, id, f.rect, reason);
            }
        }
    }
    out
}

/// A number field's prefix: a compact letter (X/Y/W/H) or a small grey icon (rotation/opacity/stroke).
pub enum Label<'a> {
    Letter(&'a str),
    Icon(Option<&'a egui::TextureHandle>),
}

pub struct NumberField<'a> {
    pub id: Id,
    pub width: f32,
    pub label: Label<'a>,
    /// Tooltip — for a disabled field, its reason.
    pub tip: &'a str,
    pub value: f32,
    pub decimals: usize,
    /// Scrub speed (value per point dragged).
    pub speed: f32,
    /// Out-of-range numbers clamp; they are not invalid (K3 rule 2).
    pub range: RangeInclusive<f32>,
    /// Explicit kit disabled state: no egui opacity fade, focus, click or scrub; `tip` remains visible.
    pub disabled: bool,
}

/// THE number field: a label column, then a box holding the value centred. The whole box is one target:
/// drag to scrub (↔), click to type (value pre-selected). While typing: ↑/↓ ±1, ⇧ ±10, Ctrl ±0.1 (A20);
/// commits by the K3 law.
pub fn number_field(ui: &mut Ui, f: NumberField<'_>) -> Edit<f32> {
    number_labeled(ui, f, false, None)
}
/// Compact picker value: no label column, right-aligned numeric ink; identical K3 behaviour.
/// An optional arrow step overrides the modifier ladder (e.g. one Web-safe step).
pub fn number_value(ui: &mut Ui, f: NumberField<'_>, arrow_step: Option<f32>) -> Edit<f32> {
    number_labeled(ui, f, true, arrow_step)
}
// ---- Lane G: preserve semantics after egui's editing TextEdit updates the same node ----
fn number_labeled(ui: &mut Ui, f: NumberField<'_>, compact: bool, arrow_step: Option<f32>) -> Edit<f32> {
    let (id, name, enabled) = (f.id, f.tip, !f.disabled && ui.is_enabled());
    let out = number_field_with(ui, f, compact, arrow_step);
    super::super::accessibility::emit(ui, id, accesskit::Role::SpinButton, name, None, enabled);
    out
}
fn number_field_with(ui: &mut Ui, f: NumberField<'_>, compact: bool, arrow_step: Option<f32>) -> Edit<f32> {
    let ctx = ui.ctx().clone();
    let id = f.id;
    if !f.disabled {
        register(ui, id);
    }
    let (lo, hi) = (*f.range.start(), *f.range.end());
    let (row, _) = ui.allocate_exact_size(egui::vec2(f.width, t::FIELD_H), Sense::hover());
    let p = ui.painter().clone();
    let labw = if compact { 0.0 } else { t::FIELD_LABEL_W };
    let ink = if f.disabled { t::DISABLED } else { t::MUTED };
    match f.label {
        Label::Letter(s) => {
            let at = egui::pos2(row.left() + labw - t::NUM_LABEL_RIGHT_INSET, row.center().y);
            p.shaped_chrome(at, Align2::RIGHT_CENTER, s, FontId::proportional(t::FIELD_LABEL_TEXT), ink);
        }
        Label::Icon(Some(tex)) => {
            let at = egui::pos2(row.left() + t::NUM_ICON_CENTER_X, row.center().y);
            let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            p.image(tex.id(), Rect::from_center_size(at, egui::Vec2::splat(t::ICON_SM)), uv, ink);
        }
        Label::Icon(None) => {}
    }
    let bx = Rect::from_min_max(
        egui::pos2(row.left() + labw + if compact { 0.0 } else { t::FIELD_LABEL_BOX_GAP }, row.top()),
        row.max,
    );
    let mut out = Edit::new(id, bx);
    let decimals = f.decimals;
    let fmt = move |v: f32| format!("{v:.decimals$}");
    let shown = fmt(f.value);
    // ---- Lane G ----
    super::super::accessibility::emit(ui, id, accesskit::Role::SpinButton, f.tip, Some(&shown), !f.disabled);
    ui.ctx().accesskit_node_builder(id, |node| {
        node.set_numeric_value(f.value as f64);
        node.set_min_numeric_value(lo as f64);
        node.set_max_numeric_value(hi as f64);
    });
    let mut sess = load(&ctx, id);
    if sess.is_some() && (f.disabled || escaped(ui)) {
        end(&ctx, id);
        sess = None;
        out.closed = true;
    } else if sess.is_none() && !f.disabled && take_tab_to(&ctx, id) {
        sess = begin(&ctx, id, &shown);
    }
    let Some(mut s) = sess else {
        idle_box(ui, &f, bx, &shown, &mut out, compact);
        return out;
    };
    let tab = take_tab(ui, id);
    p.rect(bx, t::r_ctrl(), t::INPUT_WELL, Stroke::new(t::KIT_STROKE, t::ACCENT), StrokeKind::Middle); // the dark input well
    let mut buf = s.buf.clone();
    if ctx.memory(|m| m.has_focus(id)) {
        let dv = ui.input_mut(|i| {
            // A20: Shift = 10 leap · Ctrl = fine (0.1) · plain = 1 — a clear keyboard step ladder
            // (Shift and Ctrl first: `consume_key` ignores an extra Shift)
            let steps: [(Modifiers, Key, f32); 6] = [
                (Modifiers::SHIFT, Key::ArrowUp, 10.0),
                (Modifiers::SHIFT, Key::ArrowDown, -10.0),
                (Modifiers::CTRL, Key::ArrowUp, 0.1),
                (Modifiers::CTRL, Key::ArrowDown, -0.1),
                (Modifiers::NONE, Key::ArrowUp, 1.0),
                (Modifiers::NONE, Key::ArrowDown, -1.0),
            ];
            steps
                .into_iter()
                .filter(|(m, k, _)| i.consume_key(*m, *k))
                .map(|(_, _, d)| arrow_step.map_or(d, |step| d.signum() * step))
                .sum::<f32>()
        });
        if dv != 0.0 {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
            // FB4: step a PRECISE value, not the rounded text — trusted while it still renders to what
            // is shown (else the user typed something new and the text wins)
            let base = s.acc.filter(|a| fmt(*a) == buf).unwrap_or_else(|| buf.trim().parse::<f32>().unwrap_or(f.value));
            let nv = (base + dv).clamp(lo, hi);
            let text = fmt(nv);
            select_all(&ctx, id, &text); // the next keystroke still replaces (same as click-to-type)
            s.acc = Some(nv);
            // the step is applied at once (`live`); it is the new baseline — a blur after it commits
            // nothing more, and Esc returns here (the one-undo arrow session is U3-T)
            s.orig = text.clone();
            s.buf = text.clone();
            buf = text;
            out.live = Some(nv);
        }
    }
    if ctx.memory(|m| m.has_focus(id)) {
        ui.input_mut(|i| {
            for event in &mut i.events {
                if let egui::Event::Text(s) | egui::Event::Paste(s) | egui::Event::Ime(egui::ImeEvent::Commit(s)) =
                    event
                {
                    *s = crate::i18n::latin_digits(s);
                }
            }
        });
    }
    super::text::editor::show(
        ui,
        id,
        bx.shrink2(egui::vec2(t::NUM_INSET_X, t::FIELD_INSET_Y)),
        &mut buf,
        t::numeric_value(t::NUM_TEXT),
        "",
        false,
    );
    ctx.memory_mut(|m| {
        m.set_focus_lock_filter(
            id,
            egui::EventFilter { vertical_arrows: true, horizontal_arrows: true, ..Default::default() },
        )
    });
    let acc = s.acc;
    let parse = move |txt: &str| -> Result<f32, &'static str> {
        let typed = txt.trim().parse::<f32>().ok().filter(|v| v.is_finite()).ok_or(NOT_A_NUMBER)?;
        // commit the precise accumulator while it still matches the text, so fine steps survive (FB4)
        Ok(acc.filter(|a| fmt(*a) == txt).unwrap_or(typed).clamp(lo, hi))
    };
    if let Some(s) = finish(&ctx, id, s, buf, tab, &parse, &mut out) {
        if let Some(reason) = s.reason {
            paint_reason(ui, id, bx, reason);
        }
    }
    out
}

/// The number field at rest: hover, scrub and click-to-type.
fn idle_box(ui: &mut Ui, f: &NumberField<'_>, bx: Rect, shown: &str, out: &mut Edit<f32>, compact: bool) {
    let (lo, hi) = (*f.range.start(), *f.range.end());
    let p = ui.painter().clone();
    let sense = if f.disabled { Sense::hover() } else { Sense::click_and_drag() };
    let resp: Response = ui.interact(bx, f.id, sense);
    if f.disabled {
        p.rect(bx, t::r_ctrl(), egui::Color32::TRANSPARENT, Stroke::new(t::KIT_STROKE, t::LINE), StrokeKind::Middle);
        p.shaped_text(
            if compact { bx.right_center() - egui::vec2(t::NUM_INSET_X, 0.0) } else { bx.center() },
            if compact { Align2::RIGHT_CENTER } else { Align2::CENTER_CENTER },
            shown,
            t::numeric_value(t::NUM_TEXT),
            t::DISABLED,
        );
        if !f.tip.is_empty() {
            resp.shaped_hover_text(f.tip).shaped_disabled_hover_text(f.tip);
        }
        return;
    }
    let hot = resp.hovered() || resp.dragged();
    if hot {
        p.rect(bx, t::r_ctrl(), t::HOVER, Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Middle);
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    } else {
        p.rect_filled(bx, t::r_ctrl(), t::SURFACE);
    }
    p.shaped_text(
        if compact { bx.right_center() - egui::vec2(t::NUM_INSET_X, 0.0) } else { bx.center() },
        if compact { Align2::RIGHT_CENTER } else { Align2::CENTER_CENTER },
        shown,
        t::numeric_value(t::NUM_TEXT),
        t::TEXT,
    );
    if resp.dragged() {
        let dx = resp.drag_delta().x;
        if dx != 0.0 {
            // A20: the mouse has its own modifier feel — Shift scrubs coarse (×5), Ctrl fine (÷5)
            let mods = ui.input(|i| i.modifiers);
            let mult = if mods.shift {
                5.0
            } else if mods.ctrl {
                0.2
            } else {
                1.0
            };
            out.live = Some((f.value + dx * f.speed * mult).clamp(lo, hi));
        }
    }
    if resp.clicked() {
        out.editing = begin(ui.ctx(), f.id, shown).is_some(); // the text edit claims focus next frame
    }
    if !f.tip.is_empty() {
        // disabled fields carry their REASON in `tip`, so show it in both states
        resp.shaped_hover_text(f.tip).shaped_disabled_hover_text(f.tip);
    }
}

/// A live-bound text field that commits nothing to a document (Layers search): every keystroke is the
/// value; Esc returns to the text at focus-in; Enter / Tab / blur just close.
pub fn search_field(ui: &mut Ui, id: Id, rect: Rect, text: &mut String, font: FontId, hint: &str) -> Response {
    let ctx = ui.ctx().clone();
    register(ui, id);
    let mut sess = load(&ctx, id);
    if let Some(s) = sess.as_ref().filter(|_| escaped(ui)) {
        *text = s.orig.clone();
        end(&ctx, id);
        sess = None;
    } else if sess.is_none() && take_tab_to(&ctx, id) {
        sess = begin(&ctx, id, text.as_str());
    }
    let tab = if sess.is_some() { take_tab(ui, id) } else { None };
    let resp = super::text::editor::show(ui, id, rect, text, font, hint, false);
    let sess = sess.or_else(|| ctx.memory(|m| m.has_focus(id)).then(|| Session { arm: false, ..Session::new(text) }));
    if let Some(s) = sess {
        let s = Session { buf: text.clone(), ..s };
        let mut out = Edit::new(id, rect);
        finish(&ctx, id, s, text.clone(), tab, &|_| Ok(()), &mut out);
    }
    resp
}

/// The outcome of [`settle`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Settled {
    /// Fields whose valid changed text is committed now: the caller applies the value each last
    /// reported as [`Edit::pending`].
    pub commits: Vec<Id>,
    /// A field holds text that does not parse: it keeps the keyboard and shows its reason, nothing
    /// else was touched, and the command that asked must not run.
    pub blocked: Option<Id>,
}

/// K3 before a lifecycle command (tab switch, Save, Close, Quit) or a canvas press: every open field is
/// finished as its last frame left it — valid changed text commits, unchanged text just closes.
pub fn settle(ctx: &egui::Context) -> Settled {
    let open = open_ids(ctx);
    let sessions: Vec<(Id, Session)> = open.iter().filter_map(|id| load(ctx, *id).map(|s| (*id, s))).collect();
    if let Some((id, mut s)) = sessions.iter().find(|(_, s)| matches!(s.validity, Validity::Invalid(_))).cloned() {
        if let Validity::Invalid(reason) = s.validity {
            s.reason = Some(reason);
        }
        store(ctx, id, s);
        ctx.memory_mut(|m| m.request_focus(id));
        return Settled { commits: vec![], blocked: Some(id) };
    }
    let mut out = Settled::default();
    for id in open {
        if load(ctx, id).is_some_and(|s| s.validity == Validity::Changed) {
            out.commits.push(id);
        }
        end(ctx, id);
    }
    out
}

/// After every frame (`run_ui` returned) — the ONE place the K3 invariant is enforced: an edit lives
/// only while its field was drawn in the pass the frame ended with. Any other edit (its panel closed,
/// hidden behind a tab, Home, the selection moved on, a mid-frame layout change) ends in THIS frame:
/// valid changed text commits (returned — the caller applies that field's last `pending` value);
/// anything else, invalid text included, reverts silently, its reason gone and nothing left to hold
/// a command (K3: "the field is gone; no trap").
pub fn end_frame(ctx: &egui::Context) -> Vec<Id> {
    let mut commits = vec![];
    let last = ctx.cumulative_pass_nr().saturating_sub(1); // the pass the frame ended with
    for id in open_ids(ctx) {
        match load(ctx, id) {
            Some(s) if s.pass == last => {}
            Some(s) => {
                if s.validity == Validity::Changed {
                    commits.push(id);
                }
                end(ctx, id);
            }
            None => end(ctx, id),
        }
    }
    commits
}

/// The reason a field shows under itself right now (a refused commit), if any.
pub fn reason(ctx: &egui::Context, id: Id) -> Option<&'static str> {
    load(ctx, id).and_then(|s| s.reason)
}

/// A field edit is open somewhere.
pub fn any_open(ctx: &egui::Context) -> bool {
    !open_ids(ctx).is_empty()
}

/// A field holds text that does not parse: it owns the keyboard, and the user's presses and commands
/// wait for it (K3 — fixed text commits, Esc reverts).
pub fn blocked(ctx: &egui::Context) -> bool {
    open_ids(ctx).into_iter().any(|id| load(ctx, id).is_some_and(|s| matches!(s.validity, Validity::Invalid(_))))
}

/// The HOME the fields being drawn now belong to (the [`group`] around them — a panel, the control
/// bar). Mirrored fields (the bar's X and the Properties X) put it in their ids, so they never share
/// an edit session.
pub fn home(ui: &Ui) -> Id {
    ui.data(|d| d.get_temp::<Id>(group_key())).unwrap_or(Id::NULL)
}

#[cfg(test)]
mod disabled_tests {
    use super::*;

    fn field(id: &'static str, disabled: bool) -> NumberField<'static> {
        NumberField {
            id: Id::new(id),
            width: 80.0,
            label: Label::Letter(id),
            tip: id,
            value: 12.0,
            decimals: 0,
            speed: 1.0,
            range: 0.0..=100.0,
            disabled,
        }
    }

    fn draw(ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<Edit<f32>> {
        let mut edits = vec![];
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 240.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            group(ui, "disabled-tab", |ui| {
                edits.push(number_field(ui, field("A", false)));
                edits.push(number_field(ui, field("Disabled", true)));
                edits.push(number_field(ui, field("B", false)));
            });
        });
        edits
    }

    #[test]
    fn tab_ring_skips_a_disabled_number_field() {
        let ctx = egui::Context::default();
        crate::shell::fonts::install(&ctx);
        crate::shell::tokens::apply(&ctx);
        let _ = draw(&ctx, vec![]);
        assert!(begin(&ctx, Id::new("A"), "12").is_some());
        let _ = draw(&ctx, vec![]);
        let edits = draw(
            &ctx,
            vec![egui::Event::Key {
                key: Key::Tab,
                physical_key: Some(Key::Tab),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(!edits[1].editing && edits[2].editing, "Tab must bypass Disabled and open B");
    }
}
