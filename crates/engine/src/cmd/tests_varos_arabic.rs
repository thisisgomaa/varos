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
