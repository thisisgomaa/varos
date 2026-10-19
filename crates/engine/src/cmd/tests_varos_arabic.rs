//! Varos: `text.styleHarakat` and the Arabic new-type leading ([`super`]).

use serde_json::json;
use vectorcraft_color::Paint;
use vectorcraft_doc::{NodeId, NodeKind, TextObject};

use crate::Session;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 400})).unwrap();
    s
}

fn create(s: &mut Session, text: &str) -> u64 {
    s.execute("text.create", &json!({"x": 20, "y": 60, "text": text})).unwrap()["id"].as_u64().unwrap()
}

fn text(s: &Session, id: u64) -> TextObject {
    match &s.doc().unwrap().doc.node(NodeId(id)).unwrap().kind {
        NodeKind::Text(t) => (**t).clone(),
        _ => panic!("not text"),
    }
}

/// Each character's fill, in order.
fn fills(t: &TextObject) -> Vec<(char, Paint)> {
    t.runs.iter().flat_map(|r| r.text.chars().map(move |c| (c, r.style.fill.clone()))).collect()
}

const VOCALIZED: &str = "بِسْمِ اللَّهِ";

#[test]
fn style_harakat_fills_only_the_harakat_and_is_one_step() {
    let mut s = session();
    let id = create(&mut s, VOCALIZED);
    let letters = text(&s, id).runs[0].style.fill.clone();
    let steps = s.active().unwrap().history.undo.len();
    let r = s.execute("text.styleHarakat", &json!({"fill": "#C0392B"})).unwrap();
    assert_eq!(r["harakat"], 6);
    assert_eq!(s.active().unwrap().history.undo.len(), steps + 1, "one step");
    let t = text(&s, id);
    assert_eq!(t.plain_text(), VOCALIZED, "the text is untouched");
    for (c, fill) in fills(&t) {
        if vectorcraft_text::harakat::is_harakat(c) {
            assert_ne!(fill, letters, "{c:?} coloured");
            assert!(matches!(fill, Paint::Solid { .. }));
        } else {
            assert_eq!(fill, letters, "{c:?} keeps its letter colour");
        }
    }
    // Hidden: no fill. Back: the letters' fill.
    s.execute("text.styleHarakat", &json!({"fill": "none"})).unwrap();
    assert!(fills(&text(&s, id)).iter().filter(|(c, _)| vectorcraft_text::harakat::is_harakat(*c)).all(|(_, f)| *f == Paint::None));
    s.execute("text.styleHarakat", &json!({"fill": "inherit"})).unwrap();
    assert!(fills(&text(&s, id)).iter().all(|(_, f)| *f == letters));
    // Undo goes back step by step.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(fills(&text(&s, id)).iter().filter(|(c, _)| vectorcraft_text::harakat::is_harakat(*c)).all(|(_, f)| *f == Paint::None));
}

#[test]
fn style_harakat_takes_a_range_and_checks_its_params() {
    let mut s = session();
    let id = create(&mut s, VOCALIZED);
    // Only the first word.
    let first = "بِسْمِ".len();
    s.execute("text.styleHarakat", &json!({"id": id, "start": 0, "end": first, "fill": "#1E8449"})).unwrap();
    let t = text(&s, id);
    let coloured: Vec<usize> = {
        let mut at = 0;
        let mut v = vec![];
        for r in &t.runs {
            if r.style.fill != t.runs[0].style.fill {
                v.extend(r.text.char_indices().map(|(i, _)| at + i));
            }
            at += r.text.len();
        }
        v
    };
    assert_eq!(coloured.len(), 3);
    assert!(coloured.iter().all(|&b| b < first));
    assert!(s.execute("text.styleHarakat", &json!({})).is_err(), "fill is required");
    assert!(s.execute("text.styleHarakat", &json!({"fill": "nope"})).is_err());
    // Latin type has none: nothing to colour, and nothing changes.
    let latin = create(&mut s, "Typography");
    let before = text(&s, latin);
    let r = s.execute("text.styleHarakat", &json!({"ids": [latin], "fill": "#C0392B"})).unwrap();
    assert_eq!(r["harakat"], 0);
    assert_eq!(text(&s, latin).runs, before.runs);
}

#[test]
fn new_arabic_type_takes_150_percent_auto_leading() {
    let mut s = session();
    let arabic = create(&mut s, "بسم الله الرحمن الرحيم");
    assert_eq!(text(&s, arabic).para.justification.auto_leading, 150.0);
    let latin = create(&mut s, "Typography");
    assert_eq!(text(&s, latin).para.justification.auto_leading, 120.0);
    // The Type tool: empty new type, then Arabic typed into it.
    let typed = create(&mut s, "");
    assert_eq!(text(&s, typed).para.justification.auto_leading, 120.0);
    s.execute("text.editRange", &json!({"id": typed, "start": 0, "end": 0, "insert": "سلام"})).unwrap();
    assert_eq!(text(&s, typed).para.justification.auto_leading, 150.0);
    // Existing type keeps its leading: Arabic added to Latin type, or leading set by hand.
    s.execute("text.editRange", &json!({"id": latin, "start": 0, "end": 0, "insert": "سلام "})).unwrap();
    assert_eq!(text(&s, latin).para.justification.auto_leading, 120.0);
    s.execute("text.setFormat", &json!({"ids": [arabic], "autoLeading": 130})).unwrap();
    s.execute("text.editRange", &json!({"id": arabic, "start": 0, "end": 0, "insert": "و"})).unwrap();
    assert_eq!(text(&s, arabic).para.justification.auto_leading, 130.0);
}

/// Each character's fill at byte `at` of `t`.
fn fill_at(t: &TextObject, at: usize) -> Paint {
    let mut start = 0;
    for r in &t.runs {
        if start <= at && at < start + r.text.len() {
            return r.style.fill.clone();
        }
        start += r.text.len();
    }
    panic!("no character at {at}");
}

/// "inherit" from the second mark of a letter's harakat (its fatha and shadda) takes the letter's
/// fill, not the first mark's.
#[test]
fn inherit_from_the_second_mark_takes_the_letters_fill() {
    let mut s = session();
    let id = create(&mut s, VOCALIZED);
    let letters = text(&s, id).runs[0].style.fill.clone();
    s.execute("text.styleHarakat", &json!({"fill": "#C0392B"})).unwrap();
    // اللَّهِ: two marks on the second lam.
    let pair = vectorcraft_text::harakat::harakat_ranges(VOCALIZED, 0..VOCALIZED.len()).into_iter().find(|r| r.len() == 4).unwrap();
    let (first, second) = (pair.start, pair.start + 2);
    let red = fill_at(&text(&s, id), second);
    assert_ne!(red, letters);
    let r = s.execute("text.styleHarakat", &json!({"id": id, "start": second, "end": pair.end, "fill": "inherit"})).unwrap();
    assert_eq!(r["harakat"], 1);
    let t = text(&s, id);
    assert_eq!(fill_at(&t, second), letters, "the second mark takes its letter's fill");
    assert_eq!(fill_at(&t, first), red, "the first mark, outside the range, keeps its colour");
}

/// The range is normalised as `edit::style_range` takes one: reversed, past the end and inside a
/// character all colour what the plain range does; a negative offset is an error.
#[test]
fn style_harakat_normalises_its_range() {
    let first = "بِسْمِ".len(); // ب 0, kasra 2, س 4, sukun 6, م 8, kasra 10
    let coloured = |p: serde_json::Value| {
        let mut s = session();
        let id = create(&mut s, VOCALIZED);
        let mut p = p;
        p["id"] = json!(id);
        p["fill"] = json!("#1E8449");
        let r = s.execute("text.styleHarakat", &p).unwrap();
        (r["harakat"].as_u64().unwrap(), text(&s, id).runs)
    };
    let plain = coloured(json!({"start": 0, "end": first}));
    assert_eq!(plain.0, 3);
    assert_eq!(coloured(json!({"start": first, "end": 0})), plain, "reversed");
    assert_eq!(coloured(json!({"start": 1, "end": first})), plain, "a start inside ب");
    assert_eq!(coloured(json!({"start": first - 1, "end": 1})).0, 2, "reversed, both inside characters: the last kasra left out");
    assert_eq!(coloured(json!({"start": 0, "end": 7})).0, 1, "an end inside the sukun leaves it out");
    let all = coloured(json!({}));
    assert_eq!(coloured(json!({"start": 0, "end": 1_000_000})), all, "past the end");
    assert_eq!(coloured(json!({"start": 1_000_000})).0, 0, "wholly past the end");
    let mut s = session();
    let id = create(&mut s, VOCALIZED);
    assert!(s.execute("text.styleHarakat", &json!({"id": id, "start": -1, "end": 4, "fill": "#1E8449"})).is_err(), "negative");
    assert!(s.execute("text.styleHarakat", &json!({"id": id, "start": 0, "end": -4, "fill": "#1E8449"})).is_err(), "negative end");
}

/// Only type made in this session gets the Arabic leading when its first text arrives: an
/// existing object emptied by deletion keeps its own, whatever is typed into it next.
#[test]
fn an_emptied_object_keeps_its_leading() {
    let mut s = session();
    let leading = |s: &Session, id| text(s, id).para.justification.auto_leading;
    // Latin type, emptied, then Arabic typed: 120 % stays.
    let latin = create(&mut s, "Typography");
    s.execute("text.editRange", &json!({"id": latin, "start": 0, "end": "Typography".len(), "insert": ""})).unwrap();
    assert_eq!(text(&s, latin).plain_text(), "");
    s.execute("text.editRange", &json!({"id": latin, "start": 0, "end": 0, "insert": "سلام"})).unwrap();
    assert_eq!(leading(&s, latin), 120.0);
    // New empty type that took Latin first, then was emptied and took Arabic: 120 % stays.
    let typed = create(&mut s, "");
    s.execute("text.editRange", &json!({"id": typed, "start": 0, "end": 0, "insert": "ab"})).unwrap();
    s.execute("text.editRange", &json!({"id": typed, "start": 0, "end": 2, "insert": ""})).unwrap();
    s.execute("text.editRange", &json!({"id": typed, "start": 0, "end": 0, "insert": "سلام"})).unwrap();
    assert_eq!(leading(&s, typed), 120.0);
    // Arabic type made with its text, emptied and typed into again: its own leading stays.
    let arabic = create(&mut s, "سلام");
    s.execute("text.setFormat", &json!({"ids": [arabic], "autoLeading": 120})).unwrap();
    s.execute("text.editRange", &json!({"id": arabic, "start": 0, "end": "سلام".len(), "insert": ""})).unwrap();
    s.execute("text.editRange", &json!({"id": arabic, "start": 0, "end": 0, "insert": "سلام"})).unwrap();
    assert_eq!(leading(&s, arabic), 120.0);
}

/// The Type tool: new empty type, typed into keystroke by keystroke (each a preview against the
/// session's snapshot), keeps 150 % to the end of the session and after it; area type too.
#[test]
fn new_type_typed_in_one_session_takes_the_arabic_leading() {
    let mut s = session();
    let id = create(&mut s, "");
    s.begin_interaction("Typing").unwrap();
    let mut typed = String::new();
    for c in "سلام".chars() {
        typed.push(c);
        s.preview("text.editRange", &json!({"id": id, "start": 0, "end": 0, "insert": typed})).unwrap();
        assert_eq!(text(&s, id).para.justification.auto_leading, 150.0, "{typed}");
    }
    s.commit_interaction().unwrap();
    assert_eq!(text(&s, id).para.justification.auto_leading, 150.0);
    assert_eq!(text(&s, id).plain_text(), "سلام");
    // A later edit of the typed text changes nothing.
    s.execute("text.setFormat", &json!({"ids": [id], "autoLeading": 120})).unwrap();
    s.execute("text.editRange", &json!({"id": id, "start": 0, "end": 0, "insert": "و"})).unwrap();
    assert_eq!(text(&s, id).para.justification.auto_leading, 120.0);
    // Area type made empty from a path, then Arabic typed.
    let rect = s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 200, "height": 100})).unwrap()["id"].as_u64().unwrap();
    let area = s.execute("text.createInPath", &json!({"path": rect})).unwrap()["id"].as_u64().unwrap();
    assert_eq!(text(&s, area).para.justification.auto_leading, 120.0);
    s.execute("text.editRange", &json!({"id": area, "start": 0, "end": 0, "insert": "سلام"})).unwrap();
    assert_eq!(text(&s, area).para.justification.auto_leading, 150.0);
}
