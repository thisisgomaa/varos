//! Lane A headless behaviour/undo/format/heat contracts.
use varos_core::{
    appearance::{base_stack, EntryOpts, Look, StackItem},
    appearance_edits::{AppearanceEdit as A, MaskEdit as M},
    format::{self, Limits},
    model::{Anchor, Document, GroupRole, Paint, Path},
    EditCommand, Editor, Group, View,
};
fn square(id: u32, x: f32, size: f32) -> Path {
    Path::new(
        id,
        [[x, 0.], [x + size, 0.], [x + size, size], [x, size]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([1., 0., 0., 1.]),
        None,
        2.,
    )
}
fn editor() -> Editor {
    let mut ed = Editor::new();
    let mut d = Document { paths: vec![square(10, 0., 40.), square(20, 10., 20.)], ids: 1000, ..Default::default() };
    d.sync_tree();
    ed.replace_doc(d);
    ed
}
#[test]
fn stack_entries_reorder_delete_and_expand_are_individually_undoable() {
    let mut ed = editor();
    let original = ed.doc.clone();
    ed.try_execute(EditCommand::Appearance(A::AddFill { path: 10, paint: Paint::Solid([0., 0., 1., 1.]) })).unwrap();
    assert_eq!(ed.doc.paths[0].stack.len(), 3);
    let added = ed.doc.clone();
    ed.try_execute(EditCommand::Appearance(A::SetEntry {
        path: 10,
        index: 2,
        paint: Paint::Solid([0., 1., 0., 1.]),
        opacity: 0.3,
        visible: false,
    }))
    .unwrap();
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, added);
    ed.try_execute(EditCommand::Appearance(A::Reorder { path: 10, from: 2, to: 0 })).unwrap();
    assert!(matches!(ed.doc.paths[0].stack[0], StackItem::Fill { .. }));
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, added);
    ed.try_execute(EditCommand::Appearance(A::Delete { path: 10, index: 2 })).unwrap();
    assert!(ed.doc.paths[0].stack.is_empty());
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, added);
    ed.try_execute(EditCommand::Appearance(A::Expand { path: 10 })).unwrap();
    assert!(ed.doc.pidx(10).is_none());
    assert!(ed.doc.paths.iter().all(|p| p.stack.is_empty()));
    format::encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, added);
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, original);
}
#[test]
fn extra_stroke_width_does_not_change_base_and_bad_stack_is_atomic() {
    let mut ed = editor();
    ed.try_execute(EditCommand::Appearance(A::AddStroke {
        path: 10,
        paint: Paint::Solid([0., 0., 0., 1.]),
        width: 24.,
    }))
    .unwrap();
    assert_eq!(ed.doc.paths[0].stroke_width, 2.);
    assert!(varos_core::geom::painted_padding(&ed.doc.paths[0]) >= 12.);
    let before = ed.doc.clone();
    let mut stack = base_stack();
    stack.push(StackItem::Fill { paint: Paint::None, opts: EntryOpts { opacity: f32::NAN, ..Default::default() } });
    assert!(ed.try_execute(EditCommand::Appearance(A::SetStack { path: 10, stack })).is_err());
    assert_eq!(ed.doc, before);
    assert!(ed.try_execute(EditCommand::Appearance(A::Delete { path: 10, index: 0 })).is_err());
}
#[test]
fn row_masks_desugar_nest_switch_release_and_undo() {
    let mut ed = editor();
    let content = ed.doc.node_of_path(10).unwrap();
    let mask = ed.doc.node_of_path(20).unwrap();
    let before = ed.doc.clone();
    ed.try_execute(EditCommand::Mask(M::Add { node: content, mask, alpha: true })).unwrap();
    let group = ed.doc.node(content).unwrap().parent.unwrap();
    assert_eq!(ed.doc.node(group).unwrap().role, GroupRole::MaskAlpha);
    assert_eq!(ed.doc.node(group).unwrap().mask_child, Some(mask));
    assert!(ed.doc.is_mask_source(20));
    ed.try_execute(EditCommand::Mask(M::Mode { node: group, alpha: false })).unwrap();
    assert_eq!(ed.doc.node(group).unwrap().role, GroupRole::Clip);
    let clipped = ed.doc.clone();
    ed.try_execute(EditCommand::Mask(M::Release { node: group })).unwrap();
    assert_eq!(ed.doc.node(group).unwrap().role, GroupRole::Normal);
    assert!(!ed.doc.is_mask_source(20));
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, clipped);
    ed.try_execute(EditCommand::Mask(M::Begin { node: group, alpha: true })).unwrap();
    let target = ed.doc.active_layer;
    assert!(ed.doc.node(target).is_some());
    assert!(ed.doc.node(target).unwrap().children.is_empty());
    format::encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
    for _ in 0..3 {
        ed.execute(EditCommand::Undo).unwrap();
    }
    assert_eq!(ed.doc, before);
    assert!(ed.try_execute(EditCommand::Mask(M::Add { node: content, mask: content, alpha: true })).is_err());
}
#[test]
fn group_look_rejects_leaf_and_undoes_once() {
    let mut ed = editor();
    let leaf = ed.doc.node_of_path(10).unwrap();
    assert!(ed.try_execute(EditCommand::Appearance(A::SetLook { node: leaf, look: Some(Look::default()) })).is_err());
    let before = ed.doc.clone();
    ed.try_execute(EditCommand::Appearance(A::SetLook { node: 1, look: Some(Look { opacity: 0.5, isolate: true }) }))
        .unwrap();
    let scene = varos_core::build_scene_for_export(&ed, 1.);
    assert!(matches!(scene.content[0], Group::Composite { opacity: 0.5, .. }));
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, before);
}
#[test]
fn plain_v9_bytes_and_migration_are_identity_and_old_eras_refuse_keys() {
    let v5 = include_str!("fixtures/v5/plain.json").trim();
    let v9 = v5.replacen("\"varos\":5", "\"varos\":9", 1);
    let loaded = format::decode_model(v9.as_bytes(), None, &Limits::DEFAULT).unwrap();
    let encoded = format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
    assert_eq!(encoded.replacen("\"varos\":10", "\"varos\":9", 1), v9);
    assert_eq!(format::migrate::migrate_v9_to_v10(loaded.doc.clone(), &Limits::DEFAULT).unwrap(), loaded.doc);
    let mut old: serde_json::Value = serde_json::from_str(&v9).unwrap();
    old["doc"]["paths"][0]["stack"] = serde_json::json!([]);
    assert!(format::decode_model(&serde_json::to_vec(&old).unwrap(), None, &Limits::DEFAULT).is_err());
    old["varos"] = serde_json::json!(10);
    assert!(format::decode_model(&serde_json::to_vec(&old).unwrap(), None, &Limits::DEFAULT).is_ok());
}
#[test]
fn scene_signature_and_pan_heat_cache_cover_live_entry_changes() {
    let mut ed = editor();
    ed.try_execute(EditCommand::Appearance(A::AddFill { path: 10, paint: Paint::Solid([0., 0., 1., 1.]) })).unwrap();
    let before = varos_core::scene::scene_signature(&ed, View::identity(), [100, 100]);
    varos_core::build_scene_for_export(&ed, 1.);
    let builds = ed.appearance_cache.builds();
    for pan in 0..25 {
        varos_core::build_scene_in_view(&ed, View { pan: [pan as f32, 0.], zoom: 1. }, [100, 100]);
    }
    assert_eq!(ed.appearance_cache.builds(), builds);
    ed.doc.paths[0].stack[2].opts_mut().opacity = 0.4;
    assert_ne!(before, varos_core::scene::scene_signature(&ed, View::identity(), [100, 100]));
    varos_core::build_scene_for_export(&ed, 1.);
    assert_eq!(ed.appearance_cache.builds(), builds + 1);
}

#[test]
fn scrub_is_one_undo_and_return_to_origin_has_no_history() {
    let mut ed = editor();
    let before = ed.doc.clone();
    let rev = ed.rev;
    ed.begin();
    for opacity in [0.2, 0.4, 0.7] {
        ed.try_execute(EditCommand::Appearance(A::SetEntry {
            path: 10,
            index: 0,
            paint: Paint::Solid([1., 0., 0., 1.]),
            opacity,
            visible: true,
        }))
        .unwrap();
    }
    ed.finish_document_setup();
    assert_eq!(ed.rev, rev + 1);
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, before);
    let rev = ed.rev;
    ed.begin();
    for opacity in [0.3, 1.] {
        ed.try_execute(EditCommand::Appearance(A::SetEntry {
            path: 10,
            index: 0,
            paint: Paint::Solid([1., 0., 0., 1.]),
            opacity,
            visible: true,
        }))
        .unwrap();
    }
    ed.finish_document_setup();
    assert_eq!(ed.rev, rev);
    assert_eq!(ed.doc, before);
}
#[test]
fn rotated_mask_reparenting_preserves_world_geometry_and_layer_masks_stay_valid() {
    let mut ed = editor();
    let node = ed.doc.node_of_path(10).unwrap();
    let mask = ed.doc.node_of_path(20).unwrap();
    ed.doc.set_node_xform(node, varos_core::model::Xform { rot: 0.4, piv: [0., 0.] });
    ed.doc.set_node_xform(mask, varos_core::model::Xform { rot: -0.2, piv: [10., 0.] });
    let before = ed.doc.clone();
    let world = before.paths.iter().map(|p| before.unit_xform(p.id).apply(p.anchors[0].p)).collect::<Vec<_>>();
    ed.try_execute(EditCommand::Mask(M::Add { node, mask, alpha: true })).unwrap();
    for (p, w) in ed.doc.paths.iter().zip(world) {
        let a = ed.doc.unit_xform(p.id).apply(p.anchors[0].p);
        assert!(a.iter().zip(w).all(|(a, b)| (a - b).abs() < 1e-4));
    }
    format::encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(before, ed.doc);
    ed.try_execute(EditCommand::Mask(M::Begin { node: 1, alpha: true })).unwrap();
    format::encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
    assert_eq!(ed.doc.node(1).unwrap().kind, varos_core::model::NodeKind::Layer);
}
#[test]
fn clipboard_and_duplication_preserve_stack_group_look_and_alpha_role() {
    let mut ed = editor();
    ed.try_execute(EditCommand::Appearance(A::AddFill { path: 10, paint: Paint::Solid([0., 0., 1., 1.]) })).unwrap();
    let node = ed.doc.node_of_path(10).unwrap();
    let mask = ed.doc.node_of_path(20).unwrap();
    ed.try_execute(EditCommand::Mask(M::Add { node, mask, alpha: true })).unwrap();
    let group = ed.doc.node(node).unwrap().parent.unwrap();
    ed.try_execute(EditCommand::Appearance(A::SetLook {
        node: group,
        look: Some(Look { opacity: 0.6, isolate: true }),
    }))
    .unwrap();
    let clipboard = varos_core::clipboard::Clipboard::capture(&ed.doc, &[10, 20]);
    let mut dest = Document::default();
    let ids = clipboard.paste_into(&mut dest, [0., 0.]);
    assert_eq!(ids.len(), 2);
    format::encode_model(&dest, &Limits::DEFAULT).unwrap();
    let masked = dest.nodes.iter().find(|n| n.role == GroupRole::MaskAlpha).unwrap();
    assert_eq!(masked.look, Some(Look { opacity: 0.6, isolate: true }));
    assert!(dest.paths.iter().any(|p| p.stack.len() == 3));
}
