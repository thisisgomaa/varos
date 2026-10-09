use varos_core::{
    format::{self, Limits},
    text::*,
    EditCommand, Editor,
};
fn fixture() -> varos_core::model::Document {
    format::decode_model(include_bytes!("fixtures/text_next/mixed.json"), None, &Limits::DEFAULT).unwrap().doc
}
#[test]
fn source_roundtrip_and_undo_batches() {
    let d = fixture();
    let source = d.text_boxes[0].source();
    let blob = format::encode_model(&d, &Limits::DEFAULT).unwrap();
    let loaded = format::decode_model(blob.as_bytes(), None, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.doc.text_boxes, d.text_boxes);
    let mut ed = Editor::new();
    let mut t = d.text_boxes[0].clone();
    t.id = 0;
    let id = ed.try_execute_created(EditCommand::AddText { text: t, parent: None }).unwrap();
    let mut next = ed.doc.text_boxes[0].clone();
    next.runs[0].text.push_str(" — تحرير");
    ed.try_execute(EditCommand::SetText { id, text: next.clone() }).unwrap();
    ed.undo();
    assert_eq!(ed.doc.text_boxes[0].source(), source);
    ed.redo();
    assert_eq!(ed.doc.text_boxes[0], next);
    ed.undo();
    ed.undo();
    assert!(ed.doc.text_boxes.is_empty());
}
#[test]
fn refuses_legacy_keys_bad_tracking_and_newer_before_decode() {
    for data in [
        include_bytes!("fixtures/text_next/refuse_text_in_v5.json").as_slice(),
        include_bytes!("fixtures/text_next/refuse_arabic_tracking.json").as_slice(),
        include_bytes!("fixtures/text_next/refuse_newer.json").as_slice(),
    ] {
        assert!(format::decode_model(data, None, &Limits::DEFAULT).is_err());
    }
    let mut d = fixture();
    d.nodes.iter_mut().find(|n| matches!(n.kind, varos_core::model::NodeKind::Text(_))).unwrap().children.push(1);
    assert!(format::encode_model(&d, &Limits::DEFAULT).is_err());
}
#[test]
fn frozen_plain_migration_does_not_add_text_keys() {
    let v = include_bytes!("fixtures/v5/plain.json");
    let before: serde_json::Value = serde_json::from_slice(v).unwrap();
    let l = format::decode_model(v, None, &Limits::DEFAULT).unwrap();
    assert!(l.migrated);
    assert!(l.doc.text_boxes.is_empty());
    let after: serde_json::Value =
        serde_json::from_str(&format::encode_model(&l.doc, &Limits::DEFAULT).unwrap()).unwrap();
    assert!(after["doc"].get("text_boxes").is_none());
    assert_eq!(before["doc"]["paths"], after["doc"]["paths"]);
}
#[test]
fn refusal_is_atomic_and_noop_has_no_undo_step() {
    let mut ed = Editor::new();
    let t = fixture().text_boxes.remove(0);
    let id = ed.try_execute_created(EditCommand::AddText { text: t, parent: None }).unwrap();
    let before = ed.doc.clone();
    let rev = ed.rev;
    ed.try_execute(EditCommand::SetText { id, text: before.text_boxes[0].clone() }).unwrap();
    assert_eq!(rev, ed.rev);
    let mut t = before.text_boxes[0].clone();
    t.runs[0].style.letter_spacing = 1.;
    assert!(ed.try_execute(EditCommand::SetText { id, text: t }).is_err());
    assert_eq!(ed.doc, before);
    let mut t = before.text_boxes[0].clone();
    t.box_kind = TextBoxKind::Area([0., 0., -1., 30.]);
    assert!(t.validate().is_err());
}
#[test]
fn v5_reader_gate_refuses_next_writer() {
    // Frozen v5 version gate: inspect header before attempting typed data.
    let json: serde_json::Value = serde_json::from_slice(include_bytes!("fixtures/text_next/mixed.json")).unwrap();
    assert!(json["varos"].as_u64().unwrap() > 5);
}
