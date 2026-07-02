//! The 🔖 slice, headless: a document written to a real `.vrs` on disk comes back IDENTICAL after
//! reload (draw → colour → save → reopen), a newer-versioned file is refused with a clear error, and
//! File ▸ Open resets history + transient state. Pure model/IO — allowed per the math-test rule.

use varos_core::editor::{Editor, ToolKind};
use varos_core::file::{load_vrs, save_vrs};
use varos_core::model::{Anchor, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor { Anchor { id, p: [x, y], hin: None, hout: None, smooth: false } }
fn tmp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("varos-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn vrs_round_trips_identically_on_disk() {
    let mut ed = Editor::new();
    ed.doc.paths.push(Path::new(1, vec![anc(1, 0.0, 0.0), anc(2, 80.0, 0.0), anc(3, 40.0, 60.0)],
                                true, Some([0.2, 0.7, 0.3, 1.0]), Some([0.0, 0.0, 0.0, 0.5]), 4.0));
    ed.doc.paths[0].opacity = 0.8;
    ed.doc.paths[0].name = Some("Hero triangle".into());
    ed.doc.ids = 3;

    let p = tmp("roundtrip.vrs");
    save_vrs(&ed.doc, &p).expect("save writes the file");
    let loaded = load_vrs(&p).expect("load reads it back");
    assert_eq!(loaded, ed.doc, "the reloaded document must equal the saved one, field for field");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn newer_version_is_refused_not_guessed() {
    let p = tmp("future.vrs");
    std::fs::write(&p, r#"{"varos": 9999, "doc": {}}"#).unwrap();
    let err = load_vrs(&p).expect_err("a future version must not half-parse");
    assert!(err.contains("newer"), "the error names the problem, got: {err}");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn garbage_is_a_clear_error() {
    let p = tmp("garbage.vrs");
    std::fs::write(&p, "not json at all").unwrap();
    assert!(load_vrs(&p).is_err(), "garbage must fail loudly, never yield an empty doc");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn open_resets_history_and_selection() {
    let mut ed = Editor::new();
    ed.doc.paths.push(Path::new(1, vec![anc(1, 0.0, 0.0), anc(2, 10.0, 0.0), anc(3, 0.0, 10.0)],
                                true, Some([1.0, 0.0, 0.0, 1.0]), None, 1.0));
    ed.doc.ids = 3;
    ed.set_tool(ToolKind::Object);
    ed.objsel.insert(1);
    ed.begin(); ed.doc.paths[0].opacity = 0.5; ed.dirty = true; ed.commit();   // one undoable change
    let rev_before = ed.rev;
    assert!(rev_before > 0, "a committed change bumps rev");

    let fresh = varos_core::model::Document::default();
    ed.replace_doc(fresh.clone());
    assert_eq!(ed.doc, fresh, "the loaded document replaces the old one");
    assert!(ed.objsel.is_empty() && ed.selected.is_empty(), "selection resets on open");
    assert!(ed.rev > rev_before, "open bumps rev so the app can re-baseline saved_rev");
    ed.undo();
    assert_eq!(ed.doc, fresh, "history is cleared — undo can't resurrect the previous file");
}
