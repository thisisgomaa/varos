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
pub(crate) struct Pending {
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
        disabled: false,
    }
}

/// A document number field: its edits become `mk(value)`. `tip` names the field (and, when the field is
/// disabled, says why); `speed` = scrub value per point; out-of-range values clamp.
#[allow(clippy::too_many_arguments)] // a field's look + behaviour knobs, as the kit takes them
pub(crate) fn num(
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
    num_body(ui, w, label, tip, value, decimals, speed, range, false, ops, mk);
}

/// The explicit kit disabled state: full-strength DISABLED ink, reason tooltip, no interaction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn num_disabled(
    ui: &mut egui::Ui,
    w: f32,
    label: Label<'_>,
    tip: &str,
    value: f32,
    decimals: usize,
    speed: f32,
    range: std::ops::RangeInclusive<f32>,
    disabled: bool,
    ops: &mut Vec<Op>,
    mk: impl Fn(f32) -> Op,
) {
    num_body(ui, w, label, tip, value, decimals, speed, range, disabled, ops, mk);
}

#[allow(clippy::too_many_arguments)]
fn num_body(
    ui: &mut egui::Ui,
    w: f32,
    label: Label<'_>,
    tip: &str,
    value: f32,
    decimals: usize,
    speed: f32,
    range: std::ops::RangeInclusive<f32>,
    disabled: bool,
    ops: &mut Vec<Op>,
    mk: impl Fn(f32) -> Op,
) {
    let f = NumberField { speed, range, disabled, ..number(ui, w, label, tip, value, decimals) };
    let e = kf::number_field(ui, f);
    #[cfg(test)]
    tests::probe(tip, e.rect);
    route(e, ops, mk);
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
pub(crate) fn name(ui: &mut egui::Ui, w: f32, value: &str, id_src: &str, ops: &mut Vec<Op>, mk: impl Fn(String) -> Op) {
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
pub(crate) fn rename(
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

/// The reason a board field shows when the core refuses its text (`varos_core::board`'s checks — the
/// same ones `Editor::try_set_board_*` runs). Static text; the limits are the core's constants (a test
/// holds them equal).
pub(crate) fn board_reason(e: &varos_core::board::MetaError) -> &'static str {
    use varos_core::board::MetaError as E;
    match e {
        E::NameTooLong { .. } => BOARD_NAME_TOO_LONG,
        E::DescriptionTooLong { .. } => BOARD_DESCRIPTION_TOO_LONG,
        E::TooManyTags { .. } => BOARD_TOO_MANY_TAGS,
        E::TagTooLong { .. } => BOARD_TAG_TOO_LONG,
        E::ControlCharacter { .. } | E::UncleanTag { .. } | E::DuplicateTag { .. } => BOARD_CONTROL,
    }
}
pub(crate) const BOARD_NAME_TOO_LONG: &str = "A board name is at most 120 characters";
pub(crate) const BOARD_DESCRIPTION_TOO_LONG: &str = "A description is at most 500 characters";
pub(crate) const BOARD_TOO_MANY_TAGS: &str = "A board has at most 16 tags";
pub(crate) const BOARD_TAG_TOO_LONG: &str = "A tag is at most 32 characters";
pub(crate) const BOARD_CONTROL: &str = "No line breaks or tabs";

fn board_text(
    typed: &str,
    check: fn(&str) -> Result<(), varos_core::board::MetaError>,
) -> Result<String, &'static str> {
    let text = varos_core::board::clean_text(typed);
    check(&text).map_err(|e| board_reason(&e))?;
    Ok(text)
}

/// The Board section's Name: empty is allowed (the board then shows its file name).
pub(crate) fn board_name(ui: &mut egui::Ui, w: f32, value: &str, ops: &mut Vec<Op>) {
    let id = doc_id(ui, ("board-name", kf::home(ui)));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, t::TEXT_FIELD_H), egui::Sense::hover());
    let f = TextField {
        id,
        rect,
        value,
        font: egui::FontId::proportional(t::FIELD_TEXT),
        framed: true,
        open: false,
        hint: "Untitled board",
    };
    #[cfg(test)]
    tests::probe("board name", rect);
    route(kf::text_field(ui, f, |s| board_text(s, varos_core::board::check_name)), ops, Op::BoardName);
}

/// The Board section's Description: three wrapping rows; Enter commits (the text has no line breaks).
pub(crate) fn board_description(ui: &mut egui::Ui, w: f32, value: &str, ops: &mut Vec<Op>) {
    let id = doc_id(ui, ("board-description", kf::home(ui)));
    let rows = t::BOARD_DESC_ROWS;
    let h = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(t::FIELD_TEXT))) * rows as f32
        + t::FIELD_INSET_Y * 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let f = TextField {
        id,
        rect,
        value,
        font: egui::FontId::proportional(t::FIELD_TEXT),
        framed: true,
        open: false,
        hint: "What is this board for?",
    };
    #[cfg(test)]
    tests::probe("board description", rect);
    let parse = |s: &str| board_text(s, varos_core::board::check_description);
    route(kf::text_area(ui, f, rows, parse), ops, Op::BoardDescription);
}

/// The Board section's Tags: the tags as removable chips, then an inline input inside one framed box.
/// The input is a kit text field whose value is the whole new tag list (`normalize_tags`: trimmed,
/// deduped by the core fold), so adding a tag the board already has is "unchanged" and adds nothing.
/// A comma in the typed text, or Enter, adds the tag and keeps the keyboard for the next one;
/// Backspace in an empty input removes the last tag; a chip's × removes that tag.
pub(crate) fn board_tags(ui: &mut egui::Ui, w: f32, tags: &[String], ops: &mut Vec<Op>) {
    use varos_app::shell::kit::board as kb;
    let id = doc_id(ui, ("board-tags", kf::home(ui)));
    let ctx = ui.ctx().clone();
    let typing = ctx.memory(|m| m.has_focus(id));
    // the input's text was empty at the end of the last frame (kept by the parse below)
    let empty_key = id.with("empty");
    let was_empty = ctx.data(|d| d.get_temp::<bool>(empty_key)).unwrap_or(true);
    let mut add_now = false;
    if typing {
        ui.input_mut(|i| {
            let mut comma = false;
            for e in &mut i.events {
                if let egui::Event::Text(text) = e {
                    if text.contains(',') {
                        *text = text.replace(',', "");
                        comma = true;
                    }
                }
            }
            if comma {
                let enter = |pressed| egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                };
                i.events.extend([enter(true), enter(false)]);
            }
            add_now = i.key_pressed(egui::Key::Enter);
        });
        if was_empty && !tags.is_empty() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace))
        {
            ops.push(Op::BoardTags(tags[..tags.len() - 1].to_vec()));
        }
    }
    // flow: chips, then the input on the last line (or a line of its own)
    let pad = t::BOARD_TAG_PAD;
    let line = t::BOARD_TAG_LINE;
    let inner_w = (w - pad * 2.0).max(t::BOARD_TAG_INPUT_MIN);
    let chips: Vec<(std::sync::Arc<egui::Galley>, f32)> = tags
        .iter()
        .map(|tag| {
            let g = ui.fonts_mut(|f| f.layout_no_wrap(tag.clone(), t::tag(), egui::Color32::PLACEHOLDER));
            let cw = kb::tag_chip_width(&g);
            (g, cw)
        })
        .collect();
    let (mut x, mut row) = (0.0_f32, 0usize);
    let mut spots = vec![];
    for (_, cw) in &chips {
        if x > 0.0 && x + cw > inner_w {
            x = 0.0;
            row += 1;
        }
        spots.push((x, row));
        x += cw + t::SB_PILL_GAP;
    }
    let (input_x, input_row) = if inner_w - x >= t::BOARD_TAG_INPUT_MIN { (x, row) } else { (0.0, row + 1) };
    let h = line * (input_row + 1) as f32 + pad * 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let border = if typing { t::ACCENT } else { t::LINE };
    ui.painter().rect(
        rect,
        t::r_ctrl(),
        t::SURFACE,
        egui::Stroke::new(t::KIT_STROKE, border),
        egui::StrokeKind::Middle,
    );
    let origin = rect.min + egui::vec2(pad, pad);
    for (i, ((g, cw), (cx, r))) in chips.into_iter().zip(spots).enumerate() {
        let top = origin.y + r as f32 * line + (line - t::SB_PILL_H) / 2.0;
        let chip = egui::Rect::from_min_size(egui::pos2(origin.x + cx, top), egui::vec2(cw, t::SB_PILL_H));
        let x = kb::tag_chip(ui, id.with(("chip", i)), chip, g, &tags[i]);
        #[cfg(test)]
        tests::probe("tag chip ×", x.response.rect);
        if x.activated {
            let mut rest = tags.to_vec();
            rest.remove(i);
            ops.push(Op::BoardTags(rest));
        }
    }
    let input = egui::Rect::from_min_max(
        egui::pos2(origin.x + input_x, origin.y + input_row as f32 * line),
        egui::pos2(rect.right() - pad, origin.y + (input_row + 1) as f32 * line),
    );
    #[cfg(test)]
    tests::probe("board tags", input);
    let hint = if tags.is_empty() { "Add a tag" } else { "" };
    let f = TextField {
        id,
        rect: input,
        value: "",
        font: egui::FontId::proportional(t::FIELD_TEXT),
        framed: false,
        open: false,
        hint,
    };
    let nonempty = std::cell::Cell::new(false);
    let parse = |typed: &str| {
        nonempty.set(nonempty.get() || !typed.trim().is_empty());
        let mut next = tags.to_vec();
        next.push(typed.to_string());
        let next = varos_core::board::normalize_tags(next);
        varos_core::board::check_tags(&next).map_err(|e| board_reason(&e))?;
        Ok(next)
    };
    let e = kf::text_field(ui, f, parse);
    ctx.data_mut(|d| d.insert_temp(empty_key, !nonempty.get()));
    if add_now && e.commit.is_some() {
        ctx.memory_mut(|m| m.request_focus(id)); // keep typing the next tag
    }
    route(e, ops, Op::BoardTags);
}

/// The Layers search: live, commits nothing to the document; Esc restores the text it had.
pub(crate) fn search(ui: &mut egui::Ui, rect: egui::Rect, text: &mut String) {
    let id = doc_id(ui, "lay-search");
    kf::search_field(ui, id, rect, text, egui::TextStyle::Button.resolve(ui.style()), "Search");
}

/// After `Ui::run`'s frame: keep this frame's pending commit, commit a field that was not drawn
/// (selection / panel changed) from its last pending value, and put field commits first.
pub(crate) fn finish_frame(
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
pub(crate) fn settle(
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
    if ed.select_transform.preview.is_some() {
        ed.execute(varos_core::EditCommand::TransformCommit);
    }
    true
}

#[cfg(test)]
pub(crate) mod tests;

impl crate::host::DocUi for crate::ui::Ui {
    fn queue_app_command(&mut self, cmd: crate::app_command::AppCommand) -> bool {
        self.app_cmds.push(cmd);
        true
    }
    fn settle(&mut self, ed: &mut Editor) -> bool {
        crate::ui::Ui::settle(self, ed)
    }
    fn settle_fields(&mut self, ed: &mut Editor) -> bool {
        self.commit_fields(ed)
    }
    fn cancel_picker_sample(&mut self, ed: &mut Editor) {
        if let Some(m) = &mut self.color_panel {
            if m.sample_active() {
                let mut ops = vec![];
                m.finish(&mut ops);
                apply_ops(ed, ops);
            }
        }
    }
    fn bridge_preview_active(&self) -> bool {
        self.color_panel.as_ref().is_some_and(|m| m.gesture_active())
            || self.lay_drag.is_some()
            || self.tab_drag_active()
            || self.ctx.input(|i| i.pointer.any_down())
    }
    fn field_has_focus(&self) -> bool {
        self.editing_field()
    }
    fn document_switched(&mut self) {
        crate::ui::Ui::document_switched(self)
    }
    fn export_event(&mut self, event: &crate::file_jobs::ExportEvent) -> bool {
        crate::export_ui::on_event(&mut self.export_sheet, event)
    }
}
