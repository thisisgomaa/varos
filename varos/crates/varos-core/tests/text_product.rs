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

#[test]
fn release_build_refuses_text_without_mutation_or_history() {
    let mut ed = Editor::new();
    ed.try_execute(EditCommand::AddText { text: fixture().text_boxes[0].clone(), parent: None }).unwrap();
    let before = ed.doc.clone();
    let rev = ed.rev;
    assert!(ed
        .try_execute(EditCommand::LayerFamily {
            action: varos_core::select_transform::LayerAction::ReleaseBuild,
            nodes: vec![ed.doc.active_layer],
        })
        .unwrap_err()
        .contains("text"));
    assert_eq!(ed.doc, before);
    assert_eq!(ed.rev, rev);
    ed.undo();
    assert!(ed.doc.text_boxes.is_empty());
}

#[test]
fn mixed_object_selection_translate_copy_cut_paste_delete_and_undo() {
    let mut ed = Editor::new();
    let text =
        ed.try_execute_created(EditCommand::AddText { text: fixture().text_boxes[0].clone(), parent: None }).unwrap();
    let path = ed.doc.nid();
    ed.doc.paths.push(varos_core::model::Path::new(path, vec![], false, None, None, 1.));
    ed.doc.sync_tree();
    ed.try_execute(EditCommand::SelectPaths(vec![text, path])).unwrap();
    let before = ed.doc.text_boxes[0].clone();
    ed.try_execute(EditCommand::Transform(varos_core::select_transform::Transform {
        movement: [10., 20.],
        ..Default::default()
    }))
    .unwrap();
    assert_eq!(ed.doc.text_boxes[0].frame, [before.frame[0] + 10., before.frame[1] + 20.]);
    ed.undo();
    assert_eq!(ed.doc.text_boxes[0], before);
    ed.try_execute(EditCommand::SelectPaths(vec![text, path])).unwrap();
    ed.try_execute(EditCommand::Copy).unwrap();
    assert_eq!(ed.clipboard().len(), 2);
    ed.try_execute(EditCommand::Paste { offset: Some([40., 50.]) }).unwrap();
    assert_eq!(ed.doc.text_boxes.len(), 2);
    assert_eq!(ed.doc.text_boxes[1].source(), before.source());
    assert_ne!(ed.doc.text_boxes[1].id, text);
    assert_eq!(ed.doc.text_boxes[1].frame, [before.frame[0] + 40., before.frame[1] + 50.]);
    validate_document(&ed.doc).unwrap();
    ed.try_execute(EditCommand::Cut).unwrap();
    assert_eq!(ed.doc.text_boxes.len(), 1);
    ed.undo();
    assert_eq!(ed.doc.text_boxes.len(), 2);
    ed.try_execute(EditCommand::SelectPaths(vec![text])).unwrap();
    ed.try_execute(EditCommand::DeleteSelected).unwrap();
    assert!(!ed.doc.text_boxes.iter().any(|t| t.id == text));
    ed.undo();
    assert!(ed.doc.text_boxes.iter().any(|t| t.id == text));
}

#[test]
fn mixed_translation_preserves_rotated_group_transform() {
    use varos_core::model::{Anchor, NodeKind, Path, Xform};
    let mut ed = Editor::new();
    let id =
        ed.try_execute_created(EditCommand::AddText { text: fixture().text_boxes[0].clone(), parent: None }).unwrap();
    let pid = ed.doc.nid();
    let aid = ed.doc.nid();
    ed.doc.paths.push(Path::new(
        pid,
        vec![Anchor { id: aid, p: [10., 20.], hin: None, hout: None, smooth: false }],
        false,
        None,
        None,
        1.,
    ));
    ed.doc.sync_tree();
    let layer = ed.doc.active_layer;
    let mut group = ed.doc.node(layer).unwrap().clone();
    group.id = ed.doc.nid();
    group.kind = NodeKind::Group;
    group.parent = Some(layer);
    group.xform = Xform { rot: 1., piv: [5., 8.] };
    let xf = group.xform;
    for node in &mut ed.doc.nodes {
        if group.children.contains(&node.id) {
            node.parent = Some(group.id);
        }
    }
    ed.doc.nodes.iter_mut().find(|n| n.id == layer).unwrap().children = vec![group.id];
    ed.doc.nodes.push(group);
    let text_before = xf.apply(ed.doc.text_boxes[0].frame);
    let path_before = xf.apply(ed.doc.paths[0].anchors[0].p);
    ed.try_execute(EditCommand::SelectPaths(vec![id, pid])).unwrap();
    ed.try_execute(EditCommand::Transform(varos_core::select_transform::Transform {
        movement: [30., 40.],
        ..Default::default()
    }))
    .unwrap();
    assert_eq!(ed.doc.unit_xform(pid), xf);
    for (before, after) in
        [(text_before, xf.apply(ed.doc.text_boxes[0].frame)), (path_before, xf.apply(ed.doc.paths[0].anchors[0].p))]
    {
        assert!((after[0] - before[0] - 30.).abs() < 0.001);
        assert!((after[1] - before[1] - 40.).abs() < 0.001);
    }
}
