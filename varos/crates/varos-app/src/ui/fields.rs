//! The app side of the K3 field law (UI_SYSTEM §K3, piece P2). Every panel field is a `kit::field`
//! field; these helpers turn what it reports into `Op`s, so no panel carries commit logic of its own:
//!
//! * `live` (arrow step, scrub) → the op, applied this frame;
//! * `commit` (Enter, Tab, click elsewhere) → `Op::Field(op)`, applied BEFORE the frame's other ops —
//!   the edit happened before the click that ended it, so a click that also changes the selection
//!   (a Layers row) cannot pull the commit onto the new target;
//! * `pending` (what a commit would carry now) → `Op::FieldPending`, kept by `Ui` between frames, so a
//!   lifecycle command (tab switch, ⌘S, Close, Quit), a canvas press, or the field disappearing can
//!   commit it without another frame ([`settle`], [`finish_frame`]).
use varos_app::shell::kit::field::{self as kf, Edit, Label, NumberField, TextField};
use varos_app::shell::tokens as t;
use varos_core::editor::Editor;

use super::{apply_ops, doc_id, Op};
use crate::app_command::SessionId;

/// The open field's pending commit, kept between frames.
pub(super) struct Pending {
    doc: Option<SessionId>,
    id: egui::Id,
    op: Op,
}

fn route<T>(e: Edit<T>, ops: &mut Vec<Op>, mk: impl Fn(T) -> Op) {
    if let Some(v) = e.live {
        ops.push(mk(v));
    }
    if let Some(v) = e.commit {
        ops.push(Op::Field(Box::new(mk(v))));
    }
    if let Some(v) = e.pending {
        ops.push(Op::FieldPending(e.id, Box::new(mk(v))));
    }
}

fn number<'a>(ui: &egui::Ui, w: f32, label: Label<'a>, tip: &'a str, value: f32, decimals: usize) -> NumberField<'a> {
    NumberField {
        id: doc_id(ui, ("numf", kf::home(ui), tip)), // the home keeps mirrored fields apart (bar · pane)
        width: w,
        label,
        tip,
        value,
        decimals,
        speed: 1.0,
        range: -1.0e6..=1.0e6,
    }
}

/// A document number field: its edits become `mk(value)`. `tip` names the field (and, when the field is
/// disabled, says why); `speed` = scrub value per point; out-of-range values clamp.
#[allow(clippy::too_many_arguments)] // a field's look + behaviour knobs, as the kit takes them
pub(super) fn num(
    ui: &mut egui::Ui,
    w: f32,
    label: Label<'_>,
    tip: &str,
    value: f32,
    decimals: usize,
    speed: f32,
    range: std::ops::RangeInclusive<f32>,
    ops: &mut Vec<Op>,
    mk: impl Fn(f32) -> Op,
) {
    let f = NumberField { speed, range, ..number(ui, w, label, tip, value, decimals) };
    let e = kf::number_field(ui, f);
    #[cfg(test)]
    tests::probe(tip, e.rect);
    route(e, ops, mk);
}

/// A number field inside the colour picker: its value goes to the picker (whose OK/Cancel is the
/// document step), so a step, a scrub and a commit all just return the new value.
pub(super) fn num_value(
    ui: &mut egui::Ui,
    w: f32,
    label: Label<'_>,
    tip: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
) -> Option<f32> {
    let e = kf::number_field(ui, NumberField { range, ..number(ui, w, label, tip, value, 0) });
    e.live.or(e.commit)
}

/// A name is its text without edge whitespace / invisible marks; empty is not a name.
fn parse_name(s: &str) -> Result<String, &'static str> {
    let v = varos_core::command::clean_name(s);
    if v.is_empty() {
        Err(kf::EMPTY_NAME)
    } else {
        Ok(v.to_string())
    }
}

/// A boxed name field bound to a model name (the artboard name in the Properties pane).
pub(super) fn name(ui: &mut egui::Ui, w: f32, value: &str, id_src: &str, ops: &mut Vec<Op>, mk: impl Fn(String) -> Op) {
    let id = doc_id(ui, ("abname", kf::home(ui), id_src));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, t::TEXT_FIELD_H), egui::Sense::hover());
    let font = egui::FontId::proportional(t::FIELD_TEXT);
    let f = TextField { id, rect, value, font, framed: true, open: false, hint: "" };
    #[cfg(test)]
    tests::probe(id_src, rect);
    route(kf::text_field(ui, f, parse_name), ops, mk);
}

/// A transient rename editor (a Layers row, the on-canvas artboard name) seeded with `seed`: it
/// opens editing, all selected; returns true once it closed (commit, unchanged, Esc).
#[allow(clippy::too_many_arguments)]
pub(super) fn rename(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    seed: &str,
    size: f32,
    framed: bool,
    ops: &mut Vec<Op>,
    mk: impl Fn(String) -> Op,
) -> bool {
    let font = egui::FontId::proportional(size);
    let e = kf::text_field(ui, TextField { id, rect, value: seed, font, framed, open: true, hint: "" }, parse_name);
    let closed = e.closed;
    route(e, ops, mk);
    closed
}

/// The colour picker's hex field: a colour on commit, the reason while the text is not one.
pub(super) fn hex(ui: &mut egui::Ui, w: f32, shown: &str) -> Option<varos_core::geom::Rgba> {
    let id = doc_id(ui, "cm-hex");
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, t::FIELD_H), egui::Sense::hover());
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let f = TextField { id, rect, value: shown, font, framed: true, open: false, hint: "" };
    // a colour is its value: "fff" and "FFFFFF" are the same, unchanged
    let parse = |s: &str| super::parse_hex(s).map(|c| c.map(|v| (v * 255.0).round() as u8)).ok_or("Type a hex colour");
    kf::text_field(ui, f, parse).commit.map(|c| c.map(|v| v as f32 / 255.0))
}

/// The Layers search: live, commits nothing to the document; Esc restores the text it had.
pub(super) fn search(ui: &mut egui::Ui, rect: egui::Rect, text: &mut String) {
    let id = doc_id(ui, "lay-search");
    kf::search_field(ui, id, rect, text, egui::TextStyle::Button.resolve(ui.style()), "Search");
}

/// After `Ui::run`'s frame: keep this frame's pending commit, commit a field that was not drawn
/// (selection / panel changed) from its last pending value, and put field commits first.
pub(super) fn finish_frame(
    ctx: &egui::Context,
    doc: Option<SessionId>,
    ops: &mut Vec<Op>,
    pending: &mut Option<Pending>,
) {
    let (marks, rest): (Vec<Op>, Vec<Op>) = ops.drain(..).partition(|op| matches!(op, Op::FieldPending(..)));
    *ops = rest;
    let fresh = marks.into_iter().last().and_then(|op| match op {
        Op::FieldPending(id, op) => Some(Pending { doc, id, op: *op }),
        _ => None,
    });
    for id in kf::end_frame(ctx) {
        if let Some(p) = pending.take_if(|p| p.id == id && p.doc == doc) {
            ops.insert(0, Op::Field(Box::new(p.op)));
        }
    }
    if kf::blocked(ctx) {
        // K3: a field holds text that does not parse — this frame's presses (a button, a row, a tool)
        // are ignored; the field keeps the keyboard and its reason (no flash: L4)
        ops.retain(|op| matches!(op, Op::Field(_)));
    }
    ops.sort_by_key(|op| !matches!(op, Op::Field(_))); // stable: field commits first, in order
    *pending = fresh;
}

/// K3 before a lifecycle command or a canvas press: commit the open field into `ed` (its last pending
/// value; unchanged text just closes). `false` = its text does not parse: it keeps the keyboard and
/// its reason, nothing ran, and the command must not run either.
pub(super) fn settle(
    ctx: &egui::Context,
    doc: Option<SessionId>,
    pending: &mut Option<Pending>,
    ed: &mut Editor,
) -> bool {
    let settled = kf::settle(ctx);
    if settled.blocked.is_some() {
        return false;
    }
    let mut ops = vec![];
    for id in settled.commits {
        if let Some(p) = pending.take_if(|p| p.id == id && p.doc == doc) {
            ops.push(p.op);
        }
    }
    *pending = None;
    apply_ops(ed, ops);
    true
}

#[cfg(test)]
pub(super) mod tests;
