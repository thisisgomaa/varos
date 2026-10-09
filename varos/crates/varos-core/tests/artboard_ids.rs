//! Format 4 (2026-10-07, Bridge slice 3): stable artboard ids. Allocation from the document counter
//! with the editor's high-water discipline, the v3→v4 migration, and the refusals around it.
use serde_json::{json, Value};
use varos_core::board::{new_board_with_preset, PresetId};
use varos_core::editor::{Editor, ToolKind};
use varos_core::format::{
    check_structure, decode_model, encode_model, migrate_v3_to_v4, validate, Invalid, Limits, LoadError,
    ARTBOARD_ID_VERSION, FORMAT_VERSION, MIGRATION_NOTICE,
};
use varos_core::model::{Artboard, Document};
use varos_core::EditCommand;

fn ids(d: &Document) -> Vec<u32> {
    d.artboards.iter().map(|a| a.id).collect()
}
fn board(x: f32) -> Artboard {
    Artboard { x, y: 0.0, w: 100.0, h: 100.0, ..Artboard::default() }
}
fn dec(v: &Value) -> Result<varos_core::format::Loaded, LoadError> {
    decode_model(v.to_string().as_bytes(), None, &Limits::DEFAULT)
}
/// A v4 blob of `d` re-stamped as an older format, with the keys that format never wrote removed.
fn older(d: &Document, version: u32) -> Value {
    let mut v: Value = serde_json::from_str(&encode_model(d, &Limits::DEFAULT).unwrap()).unwrap();
    v["varos"] = json!(version);
    for b in v["doc"]["artboards"].as_array_mut().unwrap() {
        b.as_object_mut().unwrap().remove("id");
    }
    if version < 3 {
        for k in ["name", "description", "tags"] {
            v["doc"].as_object_mut().unwrap().remove(k);
        }
    }
    v
}

#[test]
fn format_4_is_current_and_artboard_ids_arrive_with_it() {
    assert_eq!(FORMAT_VERSION, varos_core::format::COLOUR_VERSION);
    assert_eq!(ARTBOARD_ID_VERSION, 4);
}

#[test]
fn presets_and_editor_creation_sites_allocate_unique_ids_from_the_counter() {
    let d = new_board_with_preset(PresetId::Story);
    assert_eq!(ids(&d), vec![2], "the preset page takes the next id after Layer 1");
    assert_eq!((d.artboards[0].w, d.artboards[0].h), (1080.0, 1920.0));
    let mut ed = Editor::new();
    ed.replace_doc(d);
    ed.execute_ui(EditCommand::AddArtboard);
    ed.execute_ui(EditCommand::DuplicateArtboard(0));
    ed.execute_ui(EditCommand::SetArtboardCount(5));
    let all = ids(&ed.doc);
    assert_eq!(all.len(), 5);
    let mut sorted = all.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 5, "unique: {all:?}");
    assert!(all.iter().all(|&id| id > 0 && id <= ed.doc.ids), "{all:?} from the counter {}", ed.doc.ids);
    assert_eq!(all[0], 2, "existing pages keep their id");
    // the duplicate (inserted after its source) got the fresh id, not the source's
    assert_ne!(all[1], all[0]);
    check_structure(&ed.doc, &Limits::DEFAULT).unwrap();
    validate(&ed.doc, &Limits::DEFAULT).unwrap();
}

#[test]
fn drawing_a_page_with_the_artboard_tool_gives_it_an_id_at_commit() {
    let mut ed = Editor::new();
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Artboard);
    ed.pointer_down([10.0, 10.0]);
    ed.pointer_move([200.0, 300.0]);
    ed.pointer_up();
    assert_eq!(ed.doc.artboards.len(), 1);
    assert!(ed.doc.artboards[0].id > 0);
    validate(&ed.doc, &Limits::DEFAULT).unwrap();
}

#[test]
fn undo_never_lets_a_new_page_reuse_a_removed_pages_id() {
    let mut ed = Editor::new();
    ed.execute_ui(EditCommand::AddArtboard);
    let first = ed.doc.artboards[0].id;
    ed.execute_ui(EditCommand::AddArtboard);
    let second = ed.doc.artboards[1].id;
    ed.undo(); // the second page is gone, the counter rolls back with the snapshot
    assert_eq!(ids(&ed.doc), vec![first]);
    ed.execute_ui(EditCommand::AddArtboard);
    let third = ed.doc.artboards[1].id;
    assert!(third > second, "high-water: {third} must not reuse {second}");
    ed.undo();
    ed.redo();
    assert_eq!(ids(&ed.doc), vec![first, third], "redo restores the same page with the same id");
}

#[test]
fn assign_is_idempotent_and_repairs_only_zero_and_repeated_ids() {
    let mut d = Document { artboards: vec![board(0.0), board(200.0)], ..Document::default() };
    assert!(d.assign_artboard_ids());
    let first = ids(&d);
    assert!(!d.assign_artboard_ids(), "nothing left to assign");
    assert_eq!(ids(&d), first);
    let dup = d.artboards[0].clone();
    d.artboards.insert(1, dup); // a clone inserted after its source (the duplicate gestures)
    assert!(d.assign_artboard_ids());
    assert_eq!(d.artboards[0].id, first[0], "the source keeps its id");
    assert_eq!(d.artboards[2].id, first[1]);
    assert!(d.artboards[1].id > first[1], "the copy gets a fresh one");
    assert_eq!(d.artboard_index(first[1]), Some(2));
    assert_eq!(d.artboard_index(0), None);
}

#[test]
fn v3_files_migrate_with_ids_in_artboard_order_and_the_notice() {
    let mut d = Document { artboards: vec![board(0.0), board(200.0), board(400.0)], ..Document::default() };
    d.active = 2;
    d.ids = 40;
    let v = older(&d, 3);
    assert!(v["doc"]["artboards"][0].get("id").is_none());
    let l = dec(&v).unwrap();
    assert_eq!((l.source_version, l.migrated, l.notice()), (3, true, Some(MIGRATION_NOTICE)));
    // the saved v4 blob had ids 41..43 and counter 43; stripped, the counter stays 43 → 44, 45, 46
    assert_eq!(ids(&l.doc), vec![44, 45, 46]);
    assert_eq!(l.doc.ids, 46);
    assert_eq!(l.doc.active, 2);
    // the same file always migrates to the same ids (pure, deterministic)
    assert_eq!(dec(&v).unwrap().doc, l.doc);
    // v1 and v2 run every step and get ids too
    for version in [1, 2] {
        let l = dec(&older(&d, version)).unwrap();
        assert!(l.migrated && l.doc.artboards.iter().all(|a| a.id > 0), "v{version}");
    }
    // the named step on a decoded v3 document
    let mut raw = d.clone();
    raw.artboards.iter_mut().for_each(|a| a.id = 0);
    raw.active = 9; // a stale index a v3 reader clamped on read
    let m = migrate_v3_to_v4(raw, &Limits::DEFAULT).unwrap();
    assert_eq!(ids(&m), vec![41, 42, 43]);
    assert_eq!(m.active, 2, "clamped into range");
}

#[test]
fn an_artboard_id_in_a_v1_v2_or_v3_file_is_refused_before_typed_decoding() {
    let d = Document { artboards: vec![board(0.0), board(200.0)], ..Document::default() };
    for version in [1, 2, 3] {
        for value in [json!(7), json!(0), json!(null), json!("x"), json!({"deep": [1, [2, {"a": null}]]})] {
            let mut v = older(&d, version);
            v["doc"]["artboards"][1]["id"] = value.clone();
            match dec(&v) {
                Err(LoadError::Invalid(Invalid::FieldNotInFormat { field, version: found })) => {
                    assert_eq!((field, found), ("artboard id", version), "{value}")
                }
                other => panic!("v{version} id={value}: expected FieldNotInFormat, got {other:?}"),
            }
        }
    }
    // a board key in a v2 file is still reported as the board key (the older refusal wins)
    let mut v = older(&d, 2);
    v["doc"]["name"] = json!("x");
    v["doc"]["artboards"][0]["id"] = json!(3);
    assert!(matches!(dec(&v), Err(LoadError::Invalid(Invalid::FieldNotInFormat { field: "name", version: 2 }))));
    let e = LoadError::Invalid(Invalid::FieldNotInFormat { field: "artboard id", version: 3 });
    assert!(e.to_string().contains("artboard id, which format 3 files cannot contain"), "{e}");
    // a non-object artboard list shape is left to the typed decode
    let mut v = older(&d, 3);
    v["doc"]["artboards"] = json!(5);
    assert!(matches!(dec(&v), Err(LoadError::Malformed { .. })));
}

#[test]
fn v4_refuses_missing_duplicate_ids_and_a_dangling_active_index() {
    let d = Document { artboards: vec![board(0.0), board(200.0)], ..Document::default() };
    let good: Value = serde_json::from_str(&encode_model(&d, &Limits::DEFAULT).unwrap()).unwrap();
    assert!(dec(&good).is_ok());
    let a = good["doc"]["artboards"][0]["id"].as_u64().unwrap() as u32;

    let mut v = good.clone();
    v["doc"]["artboards"][1]["id"] = json!(a);
    assert_eq!(dec(&v).unwrap_err(), LoadError::Invalid(Invalid::DuplicateId { kind: "artboard", id: a }));

    let mut v = good.clone();
    v["doc"]["artboards"][1].as_object_mut().unwrap().remove("id");
    assert_eq!(dec(&v).unwrap_err(), LoadError::Invalid(Invalid::MissingArtboardId { index: 1 }));
    let mut v = good.clone();
    v["doc"]["artboards"][0]["id"] = json!(0);
    assert_eq!(dec(&v).unwrap_err(), LoadError::Invalid(Invalid::MissingArtboardId { index: 0 }));

    let mut v = good.clone();
    v["doc"]["active"] = json!(2);
    assert_eq!(dec(&v).unwrap_err(), LoadError::Invalid(Invalid::ActiveArtboardOutOfRange { active: 2, count: 2 }));
    let mut v = good.clone();
    v["doc"]["artboards"] = json!([]);
    v["doc"]["active"] = json!(1);
    assert_eq!(dec(&v).unwrap_err(), LoadError::Invalid(Invalid::ActiveArtboardOutOfRange { active: 1, count: 0 }));
    assert!(LoadError::Invalid(Invalid::MissingArtboardId { index: 1 }).to_string().contains("artboard 2 has no id"));
}

#[test]
fn save_assigns_missing_ids_and_clamps_active_on_a_clone_only() {
    let mut d = Document { artboards: vec![board(0.0), board(200.0)], ..Document::default() };
    d.active = 7;
    let before = d.clone();
    let blob = encode_model(&d, &Limits::DEFAULT).unwrap();
    assert_eq!(d, before, "the caller's document is never mutated");
    let back = decode_model(blob.as_bytes(), None, &Limits::DEFAULT).unwrap().doc;
    assert!(back.artboards.iter().all(|a| a.id > 0));
    assert_eq!(back.active, 1);
    // a duplicate non-zero id is a broken document, refused on save rather than silently renumbered
    let mut dup = d.clone();
    dup.artboards[0].id = 9;
    dup.artboards[1].id = 9;
    dup.ids = 9;
    assert!(matches!(
        encode_model(&dup, &Limits::DEFAULT).unwrap_err().0,
        LoadError::Invalid(Invalid::DuplicateId { kind: "artboard", id: 9 })
    ));
}

#[test]
fn id_headroom_counts_the_ids_the_migration_will_allocate() {
    let mut d = Document { artboards: vec![board(0.0), board(200.0)], ..Document::default() };
    d.ids = u32::MAX - 3; // Layer replacement + 2 artboards + the next object would overflow
    assert_eq!(check_structure(&d, &Limits::DEFAULT).unwrap_err(), LoadError::Invalid(Invalid::IdExhausted));
    d.ids = u32::MAX - 4;
    check_structure(&d, &Limits::DEFAULT).unwrap();
}
