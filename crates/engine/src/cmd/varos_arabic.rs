//! Varos: Arabic typography commands and new-type defaults.
//!
//! * `text.styleHarakat` fills the harakat (Arabic vowel and other combining marks) apart from
//!   their letters: a colour, `none` to hide them (they stay in the text, for search, copy and
//!   export), or `inherit` to give them their letters' fill again. Only paint changes, so the
//!   letters keep their joining and the marks their attachment (`vectorcraft_text::harakat`).
//! * New type whose first letter is Arabic takes 150 % Auto Leading ([`arabic_new_type_leading`]):
//!   at 120 % harakat above and below collide between lines.

use serde_json::{Value, json};
use vectorcraft_color::Paint;
use vectorcraft_doc::{NodeId, NodeKind, TextObject};
use vectorcraft_text::harakat;

use super::typecmd::{TextRange, refresh_bounds, text_targets};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "text.styleHarakat",
        "Style Harakat",
        [],
        None,
        "{fill: colour|\"none\"|\"inherit\", ids?|id?, start?: byte, end?: byte} fill the harakat (Arabic vowel and other combining marks) of the selected type, or of the range of `id`, apart from their letters: a colour, \"none\" to hide them (they stay in the text), or \"inherit\" to give each its letter's fill again; only paint changes, so letters keep their joining and marks their place → {ids, harakat: count}",
        has_doc,
        style_harakat
    )]
}

/// What `text.styleHarakat` fills the harakat with.
enum Fill {
    Paint(Paint),
    /// Each harakat its letter's fill (the fill of the character before its run of harakat).
    Inherit,
}

fn style_harakat(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "text.styleHarakat";
    let fill = match p.get("fill") {
        None | Some(Value::Null) => return Err(bad(C, "missing `fill`")),
        Some(Value::String(n)) if n.eq_ignore_ascii_case("none") => Fill::Paint(Paint::None),
        Some(Value::String(n)) if n.eq_ignore_ascii_case("inherit") => Fill::Inherit,
        Some(v) => Fill::Paint(Paint::solid(super::paint::new_color(s, p, color_value(v).ok_or_else(|| bad(C, "bad fill colour"))?))),
    };
    let range = TextRange::parse(p, C)?;
    let ids = text_targets(s, p, C)?;
    let mut count = 0;
    s.edit("Style Harakat", |d, _| {
        for id in &ids {
            let Some(NodeKind::Text(t)) = d.node_mut(*id).map(|n| &mut n.kind) else { continue };
            let plain = t.plain_text();
            let (a, b) = range.as_ref().map_or((0, plain.len()), |r| r.bytes(t));
            for r in harakat::harakat_ranges(&plain, a..b) {
                count += plain.get(r.clone()).map_or(0, |h| h.chars().count());
                let paint = match &fill {
                    Fill::Paint(p) => p.clone(),
                    Fill::Inherit => letter_fill(t, harakat_start(&plain, r.start)),
                };
                vectorcraft_text::edit::style_range(&mut t.runs, r.start, r.end, |st| st.fill = paint.clone());
            }
            refresh_bounds(t);
        }
        Ok(())
    })?;
    Ok(json!({ "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(), "harakat": count }))
}

/// Where the harakat sequence holding byte `at` of `text` starts: back over the harakat before
/// `at` (a range may start at the second mark of a letter's shadda and fatha).
fn harakat_start(text: &str, mut at: usize) -> usize {
    while let Some(c) = text.get(..at).and_then(|s| s.chars().next_back()).filter(|c| harakat::is_harakat(*c)) {
        at -= c.len_utf8();
    }
    at
}

/// The fill of the character before byte `at` of `t` (the letter a run of harakat sits on), else
/// of the first run.
fn letter_fill(t: &TextObject, at: usize) -> Paint {
    let mut start = 0;
    let mut found = None;
    for run in &t.runs {
        let end = start + run.text.len();
        if start < at && at <= end {
            found = Some(run.style.fill.clone());
        }
        start = end;
    }
    found.or_else(|| t.runs.first().map(|r| r.style.fill.clone())).unwrap_or(Paint::None)
}

/// Note `id` (in the active document), type just created empty: it is new type until text first
/// reaches it ([`typing_into_new_type`]). `None`: nothing to note.
pub(crate) fn note_new_type(s: &mut Session, id: Option<NodeId>) {
    let Some(id) = id else { return };
    let Ok(uid) = s.doc().map(|d| d.uid) else { return };
    // Forget the closed documents' objects.
    let open: Vec<u64> = s.documents().iter().map(|d| d.uid).collect();
    s.varos_new_type.retain(|(doc, _)| open.contains(doc));
    s.varos_new_type.insert((uid, id));
}

/// Text is about to be typed into `id` of the active document, empty before the edit (`empty`):
/// is it new type ([`note_new_type`]) getting its first text? An edit of `id` while it has text
/// ends its being new, so type emptied by deletion keeps its leading. A typing session previews
/// each keystroke against its snapshot (the empty new type), so the note outlives the edits that
/// start from empty text.
pub(crate) fn typing_into_new_type(s: &mut Session, id: NodeId, empty: bool) -> bool {
    let Ok(uid) = s.doc().map(|d| d.uid) else { return false };
    if empty {
        s.varos_new_type.contains(&(uid, id))
    } else {
        s.varos_new_type.remove(&(uid, id));
        false
    }
}

/// Varos: new type whose first letter is Arabic (`new_type`: type made in this session, empty
/// until this edit, or made with its text) takes 150 % Auto Leading in paragraphs still at the
/// default 120 %, so harakat above and below don't collide between lines. Existing type (an old
/// object emptied by deletion included) and Latin type are left alone.
pub(crate) fn arabic_new_type_leading(t: &mut TextObject, new_type: bool) {
    const ARABIC_AUTO_LEADING: f64 = 150.0;
    if !new_type || !vectorcraft_text::arabic_fonts::is_arabic_text(&t.plain_text()) {
        return;
    }
    let default = vectorcraft_doc::JustificationOptions::default().auto_leading;
    t.edit_paras(None, |para| {
        if para.justification.auto_leading == default {
            para.justification.auto_leading = ARABIC_AUTO_LEADING;
        }
    });
}

#[cfg(test)]
#[path = "tests_varos_arabic.rs"]
mod tests;
