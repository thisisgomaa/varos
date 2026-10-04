//! Board metadata (Start v2 lane L2): the bounds/cleaning table, the edit commands (one undo step,
//! dirty, no-op when unchanged), the display-name rule, the preset table, and format 3 on the wire
//! (v2→v3 migration, Arabic round-trip, refusals on load and save). Pure — no Renderer, no EventLoop.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use varos_core::board::{
    self, check_description, check_name, check_tags, display_name, new_board, new_board_with_preset, normalize_tags,
    preset, MetaError, PresetId, MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, MAX_TAGS, MAX_TAG_CHARS, PRESETS,
};
use varos_core::editor::Editor;
use varos_core::file::{doc_from_blob, doc_to_blob, load_vrs, save_vrs};
use varos_core::format::{
    decode_model, encode_model, migrate_v2_to_v3, Invalid, Limits, LoadError, SaveRefused, MIGRATION_NOTICE,
};
use varos_core::model::Document;
use varos_core::{EditCommand, Unit};

fn s(text: &str) -> String {
    text.to_string()
}
fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|t| t.to_string()).collect()
}
fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(rel)
}

// ───────────────────────────── bounds & cleaning ─────────────────────────────

#[test]
fn validation_table() {
    let long = |n: usize| "a".repeat(n);
    let arabic = |n: usize| "ع".repeat(n); // 2 bytes each: limits count characters, not bytes
    let name_cases: Vec<(String, Result<(), MetaError>)> = vec![
        (s(""), Ok(())),
        (long(MAX_NAME_CHARS), Ok(())),
        (arabic(MAX_NAME_CHARS), Ok(())),
        (long(MAX_NAME_CHARS + 1), Err(MetaError::NameTooLong { found: 121 })),
        (arabic(MAX_NAME_CHARS + 1), Err(MetaError::NameTooLong { found: 121 })),
        (s("two\nlines"), Err(MetaError::ControlCharacter { field: "name" })),
        (s("tab\there"), Err(MetaError::ControlCharacter { field: "name" })),
        (s("bell\u{7}"), Err(MetaError::ControlCharacter { field: "name" })),
        (s("شعار — Logo"), Ok(())),
    ];
    for (name, want) in name_cases {
        assert_eq!(check_name(&name), want, "{name:?}");
    }
    assert_eq!(check_description(&long(MAX_DESCRIPTION_CHARS)), Ok(()));
    assert_eq!(check_description(&long(501)), Err(MetaError::DescriptionTooLong { found: 501 }));
    assert_eq!(check_description("a\r\nb"), Err(MetaError::ControlCharacter { field: "description" }));

    let sixteen: Vec<String> = (0..MAX_TAGS).map(|i| format!("t{i}")).collect();
    let seventeen: Vec<String> = (0..=MAX_TAGS).map(|i| format!("t{i}")).collect();
    let tag_cases: Vec<(Vec<String>, Result<(), MetaError>)> = vec![
        (vec![], Ok(())),
        (sixteen, Ok(())),
        (seventeen, Err(MetaError::TooManyTags { found: 17 })),
        (vec![long(MAX_TAG_CHARS)], Ok(())),
        (vec![long(33)], Err(MetaError::TagTooLong { tag: long(33), found: 33 })),
        (tags(&[""]), Err(MetaError::UncleanTag { tag: s("") })),
        (tags(&[" client"]), Err(MetaError::UncleanTag { tag: s(" client") })),
        (tags(&["client", "CLIENT"]), Err(MetaError::DuplicateTag { tag: s("CLIENT") })),
        (tags(&["a\nb"]), Err(MetaError::ControlCharacter { field: "tags" })),
        (tags(&["عربي", "client"]), Ok(())),
    ];
    for (list, want) in tag_cases {
        assert_eq!(check_tags(&list), want, "{list:?}");
    }
    // the reasons are plain English, with the numbers
    assert_eq!(
        MetaError::NameTooLong { found: 121 }.to_string(),
        "the board name is 121 characters long; the limit is 120"
    );
    assert!(MetaError::TooManyTags { found: 17 }.to_string().contains("the limit is 16"));
}

#[test]
fn tags_are_trimmed_emptied_and_deduped_case_insensitively_in_order() {
    let typed = tags(&["  Client ", "", "  ", "logo", "client", "LOGO", "عربي", "\u{200F}عربي", "Personal"]);
    assert_eq!(normalize_tags(typed), tags(&["Client", "logo", "عربي", "Personal"]), "first spelling kept");
    // what normalize produces always passes the stored-form check (when within the count/length bounds)
    let cleaned = normalize_tags(tags(&[" a ", "A", "b"]));
    assert_eq!(check_tags(&cleaned), Ok(()));
    assert_eq!(board::clean_text("  \u{200E}Logo\u{200F} "), "Logo");
}

// ───────────────────────────── commands ─────────────────────────────

fn editor() -> Editor {
    let mut ed = Editor::new();
    ed.doc = new_board();
    ed
}

#[test]
fn set_board_name_is_one_undo_step_dirty_and_no_op_when_unchanged() {
    let mut ed = editor();
    let saved = ed.doc.clone();
    ed.execute(EditCommand::SetBoardName(s("  شعار المقهى  ")));
    assert_eq!(ed.doc.name, "شعار المقهى", "edges cleaned");
    assert_eq!(ed.rev, 1, "one undo step");
    assert!(!ed.doc.content_eq(&saved), "the name is content: the document is dirty");

    ed.execute(EditCommand::SetBoardName(s("شعار المقهى")));
    assert_eq!(ed.rev, 1, "unchanged → no undo step");
    ed.execute(EditCommand::SetBoardName(s(" شعار المقهى\u{200F}")));
    assert_eq!(ed.rev, 1, "unchanged after cleaning → no undo step");

    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.name, "");
    assert!(ed.doc.content_eq(&saved), "undo back to the checkpoint reads clean");
    ed.execute(EditCommand::Redo);
    assert_eq!(ed.doc.name, "شعار المقهى");

    // an empty name is a real edit (back to "use the file stem")
    ed.execute(EditCommand::SetBoardName(s("   ")));
    assert_eq!(ed.doc.name, "");
    assert_eq!(ed.rev, 4);
}

#[test]
fn set_board_description_and_tags_are_undoable_single_steps() {
    let mut ed = editor();
    ed.execute(EditCommand::SetBoardDescription(s(" Brand mark, round two. ")));
    assert_eq!(ed.doc.description, "Brand mark, round two.");
    assert_eq!(ed.rev, 1);
    ed.execute(EditCommand::SetBoardTags(tags(&[" client ", "Client", "", "عربي"])));
    assert_eq!(ed.doc.tags, tags(&["client", "عربي"]));
    assert_eq!(ed.rev, 2, "the whole tag list is ONE step");
    ed.execute(EditCommand::SetBoardTags(tags(&["CLIENT", "client", "عربي"])));
    assert_eq!(ed.doc.tags, tags(&["CLIENT", "عربي"]), "a respelling is a change");
    assert_eq!(ed.rev, 3);
    ed.execute(EditCommand::SetBoardTags(tags(&["CLIENT", "عربي", "  "])));
    assert_eq!(ed.rev, 3, "same list after cleaning → no-op");
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.tags, tags(&["client", "عربي"]));
    ed.execute(EditCommand::Undo);
    assert!(ed.doc.tags.is_empty());
    assert_eq!(ed.doc.description, "Brand mark, round two.", "undo walks back one step at a time");
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.description, "");
}

#[test]
fn checked_edits_return_the_reason_and_change_nothing() {
    let mut ed = editor();
    let cases: Vec<(Result<(), board::Reject>, MetaError)> = vec![
        (ed.try_set_board_name(&"a".repeat(121)), MetaError::NameTooLong { found: 121 }),
        (ed.try_set_board_name("line\nbreak"), MetaError::ControlCharacter { field: "name" }),
        (ed.try_set_board_description(&"d".repeat(501)), MetaError::DescriptionTooLong { found: 501 }),
        (ed.try_set_board_description("a\tb"), MetaError::ControlCharacter { field: "description" }),
        (ed.try_set_board_tags((0..17).map(|i| format!("t{i}")).collect()), MetaError::TooManyTags { found: 17 }),
        (ed.try_set_board_tags(vec!["x".repeat(33)]), MetaError::TagTooLong { tag: "x".repeat(33), found: 33 }),
        (ed.try_set_board_tags(vec![s("ok\u{7}")]), MetaError::ControlCharacter { field: "tags" }),
    ];
    for (got, want) in cases {
        assert_eq!(got, Err(want.clone()));
        assert!(!want.to_string().is_empty(), "a plain-English reason for the field");
    }
    assert_eq!(ed.rev, 0, "nothing landed, nothing to undo");
    assert_eq!(ed.doc, new_board());
    assert_eq!(
        ed.try_set_board_name(&"a".repeat(121)).unwrap_err().to_string(),
        "the board name is 121 characters long; the limit is 120"
    );
}

#[test]
fn checked_edits_with_valid_input_are_one_undo_step_each() {
    let mut ed = editor();
    assert_eq!(ed.try_set_board_name("  Logo  "), Ok(()));
    assert_eq!((ed.doc.name.as_str(), ed.rev), ("Logo", 1));
    assert_eq!(ed.try_set_board_name("Logo"), Ok(()), "unchanged is accepted…");
    assert_eq!(ed.rev, 1, "…as a no-op without an undo step");
    assert_eq!(ed.try_set_board_description("Round two"), Ok(()));
    assert_eq!(ed.rev, 2);
    // 17 typed tags that clean down to 16 are fine (bounds apply AFTER cleaning)
    let mut typed: Vec<String> = (0..16).map(|i| format!("t{i}")).collect();
    typed.push(s("T0"));
    assert_eq!(ed.try_set_board_tags(typed), Ok(()));
    assert_eq!((ed.doc.tags.len(), ed.rev), (16, 3));
    ed.execute(EditCommand::Undo);
    assert!(ed.doc.tags.is_empty());
    assert_eq!(ed.doc.description, "Round two");
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "SetBoard* carried an invalid value")]
fn the_replay_command_only_carries_valid_values() {
    editor().execute(EditCommand::SetBoardName("a".repeat(121)));
}

#[test]
fn tags_fold_fully_and_leave_arabic_alone() {
    assert_eq!(board::fold("Straße"), board::fold("STRASSE"), "ß = SS");
    assert_eq!(board::fold("σς"), board::fold("ΣΣ"), "σ/ς = Σ");
    assert_eq!(board::fold("café"), board::fold("CAFE\u{301}"), "NFC: é = e + combining acute");
    assert_eq!(board::fold("عربي"), "عربي", "Arabic has no case: unchanged");
    assert_ne!(board::fold("عربي"), board::fold("عربى"), "different Arabic letters stay different");
    assert_eq!(normalize_tags(tags(&["Straße", "STRASSE", "ΣΣ", "σς", "عربي"])), tags(&["Straße", "ΣΣ", "عربي"]));
    assert_eq!(check_tags(&tags(&["Straße", "STRASSE"])), Err(MetaError::DuplicateTag { tag: s("STRASSE") }));
}

#[test]
fn joiners_and_bidi_marks_are_allowed_inside_and_trimmed_only_at_the_edges() {
    // Persian "می‌خواهم" needs ZWNJ (U+200C) between می and خواهم; a family emoji is a ZWJ sequence
    let zwnj_name = "می\u{200C}خواهم";
    let family = "👨\u{200D}👩\u{200D}👧";
    let mut ed = editor();
    assert_eq!(ed.try_set_board_name(&format!("\u{200F} {zwnj_name}\u{200C} ")), Ok(()));
    assert_eq!(ed.doc.name, zwnj_name, "inner ZWNJ kept; edge marks and spaces trimmed");
    assert_eq!(ed.try_set_board_tags(vec![family.to_string(), s("\u{200E}rtl\u{200F}mark")]), Ok(()));
    assert_eq!(ed.doc.tags, [family, "rtl\u{200F}mark"], "inner ZWJ / RLM kept");
    // bounds count scalar values, not graphemes: the family emoji is 5 scalars
    assert_eq!(family.chars().count(), 5);
    let seven = std::iter::repeat_n(family, 7).collect::<String>(); // 35 scalars, 7 graphemes
    assert_eq!(check_tags(std::slice::from_ref(&seven)), Err(MetaError::TagTooLong { tag: seven, found: 35 }));
    // round-trip through the file
    let back = doc_from_blob(&doc_to_blob(&ed.doc).unwrap()).unwrap();
    assert_eq!((back.name, back.tags), (ed.doc.name.clone(), ed.doc.tags.clone()));
}

// ───────────────────────────── display name ─────────────────────────────

#[test]
fn display_name_rule() {
    let p = Path::new("/work/Logo.vrs");
    assert_eq!(display_name("شعار", Some(p), None), "شعار", "the board's own name wins");
    assert_eq!(display_name("", Some(p), None), "Logo", "else the file stem (no extension)");
    assert_eq!(display_name("", Some(Path::new("/work/.hidden")), None), ".hidden");
    assert_eq!(display_name("", Some(Path::new("/work/archive.v2.vrs")), None), "archive.v2");
    assert_eq!(display_name("", None, Some(3)), "Untitled-3", "else Untitled-N");
    assert_eq!(display_name("", None, None), "Untitled");
    assert_eq!(display_name("Named", None, Some(3)), "Named", "an unsaved board can be named");
}

// ───────────────────────────── new board & presets ─────────────────────────────

#[test]
fn new_board_is_a_free_canvas_with_zero_artboards() {
    let d = new_board();
    assert!(d.artboards.is_empty());
    assert!(d.active_artboard().is_none());
    assert!(d.name.is_empty() && d.description.is_empty() && d.tags.is_empty());
    assert_eq!(d, Document::default());
}

#[test]
fn presets_create_the_right_artboard() {
    let want = [
        (PresetId::Square, 1080.0, 1080.0, Unit::Px),
        (PresetId::Portrait, 1080.0, 1350.0, Unit::Px),
        (PresetId::Story, 1080.0, 1920.0, Unit::Px),
        (PresetId::A4, 595.0, 842.0, Unit::Pt),
        (PresetId::Custom, 1080.0, 1080.0, Unit::Px),
    ];
    assert_eq!(PRESETS.len(), want.len(), "one table, one row per preset");
    for (id, w, h, unit) in want {
        assert_eq!((preset(id).w, preset(id).h, preset(id).unit), (w, h, unit), "{id:?} table row");
        let d = new_board_with_preset(id, None);
        assert_eq!(d.artboards.len(), 1, "{id:?}");
        let ab = &d.artboards[0];
        assert_eq!((ab.x, ab.y, ab.w, ab.h), (0.0, 0.0, w, h), "{id:?}: at the origin, in points (72 ppi)");
        assert_eq!(d.active, 0);
        assert_eq!(d.units.display, unit, "{id:?}: the fields show the preset's unit");
        assert!(ab.clip, "an ordinary clipping page");
        let back = doc_from_blob(&doc_to_blob(&d).unwrap()).unwrap();
        assert_eq!(back, d, "{id:?}: saves and reopens");
    }
    let custom = new_board_with_preset(PresetId::Custom, Some((300.0, 250.0)));
    assert_eq!((custom.artboards[0].w, custom.artboards[0].h), (300.0, 250.0), "last-used custom size");
    for bad in [(0.0, 10.0), (-1.0, 10.0), (f32::NAN, 10.0), (10.0, f32::INFINITY)] {
        let d = new_board_with_preset(PresetId::Custom, Some(bad));
        assert_eq!((d.artboards[0].w, d.artboards[0].h), (1080.0, 1080.0), "{bad:?} falls back");
    }
    // the custom size is only read for Custom
    let sq = new_board_with_preset(PresetId::Square, Some((300.0, 250.0)));
    assert_eq!(sq.artboards[0].w, 1080.0);
}

// ───────────────────────────── format 3 on the wire ─────────────────────────────

fn meta_doc() -> Document {
    let mut d = new_board_with_preset(PresetId::Portrait, None);
    d.name = s("شعار المقهى — Café");
    d.description = s("الوصف بالعربي and English, 100% UTF-8 ✓");
    d.tags = tags(&["client", "عربي", "شخصي"]);
    d
}

#[test]
fn metadata_round_trips_through_blob_and_disk_including_arabic() {
    let d = meta_doc();
    let blob = doc_to_blob(&d).unwrap();
    assert!(blob.starts_with(r#"{"varos":3,"doc":{"name":"شعار المقهى — Café","description":"#), "{}", &blob[..80]);
    assert!(blob.contains(r#""tags":["client","عربي","شخصي"]"#), "stored as plain UTF-8, not escaped");
    assert_eq!(doc_from_blob(&blob).unwrap(), d);
    let p = std::env::temp_dir().join(format!("varos-board-meta-{}.vrs", std::process::id()));
    save_vrs(&d, &p).unwrap();
    let back = load_vrs(&p).unwrap();
    let _ = std::fs::remove_file(&p);
    assert_eq!(back, d);
    assert!(back.content_eq(&d));
}

#[test]
fn v2_files_migrate_to_v3_with_empty_metadata_and_a_notice() {
    for name in ["v2/v2_masked_rotated.vrs", "v2/v2_boardless.vrs"] {
        let bytes = std::fs::read(fixture(name)).unwrap();
        let l = decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
        assert_eq!(l.source_version, 2, "{name}");
        assert!(l.migrated, "{name}");
        assert_eq!(l.notice(), Some(MIGRATION_NOTICE), "{name}");
        assert!(l.doc.name.is_empty() && l.doc.description.is_empty() && l.doc.tags.is_empty(), "{name}");
        // the migrated v2 file saves as exactly the frozen v3 twin
        if name.ends_with("v2_boardless.vrs") {
            let frozen = std::fs::read(fixture("v3/v3_boardless.vrs")).unwrap();
            assert_eq!(doc_to_blob(&l.doc).unwrap().as_bytes(), frozen.as_slice());
        }
    }
    // the named step is the identity on an already-decoded v2 document
    let d = Document::default();
    assert_eq!(migrate_v2_to_v3(d.clone(), &Limits::DEFAULT).unwrap(), d);
    // v1 goes through both steps
    let v1 = std::fs::read(fixture("v1/v1_unicode_arabic.vrs")).unwrap();
    let l = decode_model(&v1, None, &Limits::DEFAULT).unwrap();
    assert!(l.migrated && l.source_version == 1 && l.doc.name.is_empty());
}

#[test]
fn frozen_v3_metadata_fixture_loads_its_exact_metadata() {
    let bytes = std::fs::read(fixture("v3/v3_board_meta.vrs")).unwrap();
    let l = decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
    assert_eq!((l.source_version, l.migrated, l.notice()), (3, false, None));
    assert_eq!(l.doc.name, "شعار المقهى — Café logo");
    assert_eq!(l.doc.description, "Brand mark, round two. نسخة ثانية للشعار.");
    assert_eq!(l.doc.tags, tags(&["client", "عربي", "logo"]));
    assert_eq!(doc_to_blob(&l.doc).unwrap().as_bytes(), bytes.as_slice(), "byte-stable");
}

fn stamped(d: &Document, version: u32) -> Value {
    json!({"varos": version, "doc": serde_json::to_value(d).unwrap()})
}
fn dec(v: &Value) -> Result<varos_core::format::Loaded, LoadError> {
    decode_model(v.to_string().as_bytes(), None, &Limits::DEFAULT)
}

#[test]
fn board_keys_in_a_v1_or_v2_file_are_refused_as_unknown() {
    for version in [1, 2] {
        for key in ["name", "description", "tags"] {
            let mut v = stamped(&Document::default(), version);
            for other in ["name", "description", "tags"] {
                if other != key {
                    v["doc"].as_object_mut().unwrap().remove(other);
                }
            }
            match dec(&v) {
                Err(LoadError::Invalid(Invalid::FieldNotInFormat { field, version: found })) => {
                    assert_eq!((field, found), (key, version));
                }
                other => panic!("v{version} {key}: expected FieldNotInFormat, got {other:?}"),
            }
            // the refusal comes BEFORE typed decoding: a wrong-typed or deeply nested value is still
            // "this format cannot carry the key", never a generic "damaged"
            for value in [json!(42), json!(null), json!({"a": [1, {"b": [[], {"c": "deep"}]}], "z": true})] {
                v["doc"][key] = value.clone();
                match dec(&v) {
                    Err(LoadError::Invalid(Invalid::FieldNotInFormat { field, .. })) => assert_eq!(field, key),
                    other => panic!("v{version} {key}={value}: expected FieldNotInFormat, got {other:?}"),
                }
            }
        }
        // the same keys absent: an ordinary older file
        let mut v = stamped(&Document::default(), version);
        for key in ["name", "description", "tags"] {
            v["doc"].as_object_mut().unwrap().remove(key);
        }
        assert!(dec(&v).unwrap().migrated);
    }
    let e = LoadError::Invalid(Invalid::FieldNotInFormat { field: "name", version: 2 });
    assert!(e.to_string().contains("board name, which format 2 files cannot contain"), "{e}");
    // a v3 file may omit them (reader relaxation: they default to empty)
    let mut v = stamped(&Document::default(), 3);
    v["doc"].as_object_mut().unwrap().remove("tags");
    assert!(dec(&v).unwrap().doc.tags.is_empty());
}

#[test]
fn over_bound_metadata_is_refused_on_load_and_on_save() {
    type Poke = fn(&mut Document);
    let cases: [(Poke, MetaError); 5] = [
        (|d| d.name = "n".repeat(121), MetaError::NameTooLong { found: 121 }),
        (|d| d.description = "d".repeat(501), MetaError::DescriptionTooLong { found: 501 }),
        (|d| d.tags = (0..17).map(|i| format!("t{i}")).collect(), MetaError::TooManyTags { found: 17 }),
        (|d| d.tags = vec![s("ok"), s("OK")], MetaError::DuplicateTag { tag: s("OK") }),
        (|d| d.name = s("a\u{0}b"), MetaError::ControlCharacter { field: "name" }),
    ];
    for (poke, want) in cases {
        let mut d = meta_doc();
        poke(&mut d);
        let before = d.clone();
        match encode_model(&d, &Limits::DEFAULT) {
            Err(SaveRefused(LoadError::Invalid(Invalid::Board(e)))) => assert_eq!(e, want),
            other => panic!("save of {want:?}: got {other:?}"),
        }
        assert_eq!(d, before, "a refused save never mutates the document");
        match dec(&stamped(&d, 3)) {
            Err(LoadError::Invalid(Invalid::Board(e))) => assert_eq!(e, want),
            other => panic!("load of {want:?}: got {other:?}"),
        }
    }
    let msg = SaveRefused(LoadError::Invalid(Invalid::Board(MetaError::NameTooLong { found: 121 }))).to_string();
    assert_eq!(
        msg,
        "This document can't be saved: the board name is 121 characters long; the limit is 120. It is still open."
    );
}
