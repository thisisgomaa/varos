//! Headless slice 0.8 contracts. No renderer, window or event loop.
use varos_core::{
    editor::{
        wave::{ObjectAction as O, Same, Selection as S},
        AlignMode, AlignTarget, DistAxis,
    },
    model::{Anchor, Paint, Path},
    EditCommand as C, Editor,
};
fn editor() -> Editor {
    let mut e = Editor::new();
    e.doc.paths.clear();
    e.doc.artboards.clear();
    for (id, x, w) in [(10, 0.0, 10.0), (20, 40.0, 20.0), (30, 100.0, 30.0)] {
        let a = |n, p| Anchor { id: id + n, p, hin: None, hout: None, smooth: false };
        e.doc.paths.push(Path::new(
            id,
            vec![a(1, [x, 0.0]), a(2, [x + w, 0.0]), a(3, [x + w, 10.0]), a(4, [x, 10.0])],
            true,
            Some([1.0, 0.0, 0.0, 1.0]),
            None,
            1.0,
        ));
    }
    e.doc.ids = 100;
    e.doc.sync_tree();
    e
}
fn select(e: &mut Editor, ids: &[u32]) {
    e.try_execute(C::SelectPaths(ids.to_vec())).unwrap();
}
#[test]
fn all_deselect_reselect_inverse_are_transient() {
    let mut e = editor();
    let rev = e.rev;
    e.try_execute(C::Selection(S::All)).unwrap();
    assert_eq!(e.objsel.len(), 3);
    e.try_execute(C::Selection(S::Deselect)).unwrap();
    assert!(e.objsel.is_empty());
    e.try_execute(C::Selection(S::Reselect)).unwrap();
    assert_eq!(e.objsel.len(), 3);
    select(&mut e, &[10]);
    e.try_execute(C::Selection(S::Inverse)).unwrap();
    assert_eq!(e.objsel, [20, 30].into_iter().collect());
    assert_eq!(e.rev, rev);
}
#[test]
fn next_above_below_are_paint_order_and_skip_inert_paths() {
    let mut e = editor();
    select(&mut e, &[10]);
    e.doc.paths[1].locked = true;
    e.try_execute(C::Selection(S::Above)).unwrap();
    assert_eq!(e.objsel, [30].into_iter().collect());
    e.try_execute(C::Selection(S::Below)).unwrap();
    assert_eq!(e.objsel, [10].into_iter().collect());
    e.try_execute(C::Selection(S::Below)).unwrap();
    assert_eq!(e.objsel, [10].into_iter().collect());
}
#[test]
fn all_on_artboard_uses_geometry_membership() {
    let mut e = editor();
    e.doc.artboards.push(varos_core::model::Artboard {
        id: 200,
        x: 0.0,
        y: 0.0,
        w: 25.0,
        h: 25.0,
        ..Default::default()
    });
    e.try_execute(C::Selection(S::Artboard)).unwrap();
    assert_eq!(e.objsel, [10].into_iter().collect());
}
#[test]
fn same_matches_each_requested_paint_attribute() {
    for mode in [Same::Fill, Same::Stroke, Same::FillStroke, Same::StrokeWeight, Same::Opacity, Same::Appearance] {
        let mut e = editor();
        e.doc.paths[1].fill = Paint::Solid([0.0, 1.0, 0.0, 1.0]);
        e.doc.paths[1].stroke = Paint::Solid([0.0, 0.0, 0.0, 1.0]);
        e.doc.paths[1].stroke_width = 2.0;
        e.doc.paths[1].opacity = 0.5;
        select(&mut e, &[10]);
        e.doc.paths[2].hidden = true;
        e.try_execute(C::Selection(S::Same(mode))).unwrap();
        assert_eq!(e.objsel, [10].into_iter().collect(), "{mode:?}");
    }
}
#[test]
fn selection_lock_and_hide_are_one_undo_step() {
    for action in [O::Lock, O::Hide] {
        let mut e = editor();
        select(&mut e, &[10, 20]);
        let rev = e.rev;
        e.try_execute(C::Object(action)).unwrap();
        assert_eq!(e.rev, rev + 1);
        assert!(e.objsel.is_empty());
        assert!(!e.doc.paths[2].hidden && !e.doc.paths[2].locked);
        e.execute(C::Undo);
        assert!(e.doc.paths.iter().all(|p| !p.hidden && !p.locked));
    }
}
#[test]
fn unlock_and_show_all_clear_ancestor_flags_and_undo() {
    for action in [O::UnlockAll, O::ShowAll] {
        let mut e = editor();
        let layer = e.doc.active_layer;
        e.doc.nodes.iter_mut().find(|n| n.id == layer).unwrap().locked = true;
        e.doc.nodes.iter_mut().find(|n| n.id == layer).unwrap().hidden = true;
        e.try_execute(C::Object(action)).unwrap();
        let n = e.doc.node(layer).unwrap();
        if action == O::UnlockAll {
            assert!(!n.locked && n.hidden);
        } else {
            assert!(!n.hidden && n.locked);
        }
        e.execute(C::Undo);
        assert!(e.doc.node(layer).unwrap().locked && e.doc.node(layer).unwrap().hidden);
    }
}
#[test]
fn reverse_and_add_anchors_are_undoable() {
    for action in [O::Reverse, O::AddAnchors] {
        let mut e = editor();
        select(&mut e, &[10]);
        let before = e.doc.paths[0].clone();
        e.try_execute(C::Object(action)).unwrap();
        let p = &e.doc.paths[0];
        if action == O::Reverse {
            assert_eq!(p.anchors[0], before.anchors[3]);
        } else {
            assert_eq!(p.anchors.len(), 8);
            assert_eq!(p.anchors[1].p, [5.0, 0.0]);
        }
        e.execute(C::Undo);
        assert_eq!(e.doc.paths[0], before);
    }
}
#[test]
fn average_preserves_handle_offsets_and_is_undoable() {
    let mut e = editor();
    e.doc.paths[0].anchors[0].hout = Some([2.0, 0.0]);
    let before = e.doc.paths[0].clone();
    e.try_execute(C::SelectAnchors(vec![11, 12])).unwrap();
    e.try_execute(C::Object(O::Average)).unwrap();
    assert_eq!(e.doc.paths[0].anchors[0].p, [5.0, 0.0]);
    assert_eq!(e.doc.paths[0].anchors[0].hout, Some([7.0, 0.0]));
    e.execute(C::Undo);
    assert_eq!(e.doc.paths[0], before);
}
#[test]
fn join_closes_single_path_and_connects_multiple_paths() {
    let mut e = editor();
    e.doc.paths[0].closed = false;
    select(&mut e, &[10]);
    e.try_execute(C::Object(O::Join)).unwrap();
    assert!(e.doc.paths[0].closed);
    e.execute(C::Undo);
    assert!(!e.doc.paths[0].closed);
    e.doc.paths[1].closed = false;
    select(&mut e, &[10, 20]);
    e.try_execute(C::Object(O::Join)).unwrap();
    assert_eq!(e.doc.paths.len(), 2);
    assert_eq!(e.doc.paths[0].anchors.len(), 8);
    e.execute(C::Undo);
    assert_eq!(e.doc.paths.len(), 3);
}
#[test]
fn compound_make_release_preserve_anchor_ids_and_undo() {
    let mut e = editor();
    select(&mut e, &[10, 20]);
    let before = e.doc.clone();
    e.try_execute(C::Object(O::CompoundMake)).unwrap();
    assert_eq!(e.doc.paths.len(), 2);
    assert_eq!(e.doc.paths[0].holes[0][0].id, 21);
    e.try_execute(C::Object(O::CompoundRelease)).unwrap();
    assert_eq!(e.doc.paths.len(), 3);
    assert!(e.doc.paths.iter().all(|p| p.holes.is_empty()));
    e.execute(C::Undo);
    assert_eq!(e.doc.paths.len(), 2);
    e.execute(C::Undo);
    assert_eq!(e.doc, before);
}
#[test]
fn clean_up_removes_unpainted_paths_and_keeps_locked_art() {
    let mut e = editor();
    e.doc.paths[0].fill = Paint::None;
    e.doc.paths[1].fill = Paint::None;
    e.doc.paths[1].locked = true;
    e.try_execute(C::Object(O::CleanUp)).unwrap();
    assert_eq!(e.doc.paths.iter().map(|p| p.id).collect::<Vec<_>>(), vec![20, 30]);
    e.execute(C::Undo);
    assert_eq!(e.doc.paths.len(), 3);
}
#[test]
fn distribute_all_six_modes_keep_endpoints_fixed() {
    for mode in
        [AlignMode::Left, AlignMode::CenterH, AlignMode::Right, AlignMode::Top, AlignMode::Middle, AlignMode::Bottom]
    {
        let mut e = editor();
        e.doc.paths[1].anchors.iter_mut().for_each(|a| a.p[1] += 40.0);
        e.doc.paths[2].anchors.iter_mut().for_each(|a| a.p[1] += 100.0);
        select(&mut e, &[10, 20, 30]);
        let first = e.doc.paths[0].clone();
        let last = e.doc.paths[2].clone();
        e.try_execute(C::DistributeMode(mode)).unwrap();
        assert_eq!(e.doc.paths[0], first);
        assert_eq!(e.doc.paths[2], last);
        let coord = |i| {
            let b = e.doc.outline_bbox(i);
            match mode {
                AlignMode::Left => b.0,
                AlignMode::CenterH => (b.0 + b.2) * 0.5,
                AlignMode::Right => b.2,
                AlignMode::Top => b.1,
                AlignMode::Middle => (b.1 + b.3) * 0.5,
                AlignMode::Bottom => b.3,
            }
        };
        assert!((coord(1) - (coord(0) + coord(2)) * 0.5).abs() < 0.001);
    }
}
#[test]
fn spacing_and_align_keep_key_fixed() {
    let mut e = editor();
    select(&mut e, &[10, 20, 30]);
    e.try_execute(C::SetKeyObject(Some(20))).unwrap();
    e.try_execute(C::DistributeSpacing { axis: DistAxis::Horizontal, gap: 5.0 }).unwrap();
    assert!((e.doc.outline_bbox(1).0 - 40.0).abs() < 0.001);
    assert!((e.doc.outline_bbox(0).2 - 35.0).abs() < 0.001);
    assert!((e.doc.outline_bbox(2).0 - 65.0).abs() < 0.001);
    let key = e.doc.paths[1].clone();
    e.try_execute(C::Align { mode: AlignMode::Left, target: AlignTarget::KeyObject }).unwrap();
    assert_eq!(e.doc.paths[1], key);
    assert!((e.doc.outline_bbox(0).0 - 40.0).abs() < 0.001);
    assert!((e.doc.outline_bbox(2).0 - 40.0).abs() < 0.001);
}
#[test]
fn expand_transform_bakes_and_undo_restores_it() {
    let mut e = editor();
    select(&mut e, &[10]);
    e.try_execute(C::SetObjectRotation(30.0)).unwrap();
    let before = e.doc.clone();
    e.try_execute(C::Object(O::ExpandTransform)).unwrap();
    assert!(e.doc.node_xform(e.doc.unit_of(10).unwrap()).is_identity());
    e.execute(C::Undo);
    assert_eq!(e.doc, before);
}
#[test]
fn invalid_numeric_commands_are_rejected_without_mutation() {
    let mut e = editor();
    select(&mut e, &[10, 20, 30]);
    let before = e.doc.clone();
    assert!(e.try_execute(C::DistributeSpacing { axis: DistAxis::Horizontal, gap: f32::NAN }).is_err());
    assert_eq!(e.doc, before);
    assert!(e.try_execute(C::SetKeyObject(Some(999))).is_err());
}
#[test]
fn layer_create_sublayer_and_send_are_valid_and_undoable() {
    let mut e = editor();
    let old = e.doc.active_layer;
    let before = e.doc.clone();
    e.try_execute(C::Object(O::NewLayer)).unwrap();
    let layer = e.doc.active_layer;
    assert_ne!(layer, old);
    assert!(e.doc.roots.contains(&layer));
    e.try_execute(C::Object(O::NewSublayer)).unwrap();
    let sub = e.doc.active_layer;
    assert_eq!(e.doc.node(sub).unwrap().parent, Some(layer));
    select(&mut e, &[10]);
    e.try_execute(C::Object(O::SendToCurrentLayer)).unwrap();
    assert_eq!(e.doc.layer_ancestor(e.doc.node_of_path(10).unwrap()), sub);
    varos_core::format::check_structure(&e.doc, &varos_core::format::Limits::DEFAULT).unwrap();
    varos_core::format::validate(&e.doc, &varos_core::format::Limits::DEFAULT).unwrap();
    e.execute(C::Undo);
    e.execute(C::Undo);
    e.execute(C::Undo);
    assert_eq!(e.doc, before);
}
#[test]
fn spacing_preserves_group_member_offsets() {
    let mut e = editor();
    select(&mut e, &[10, 20]);
    e.try_execute(C::GroupSelection).unwrap();
    select(&mut e, &[10, 20, 30]);
    let dx = e.doc.paths[1].anchors[0].p[0] - e.doc.paths[0].anchors[0].p[0];
    e.try_execute(C::DistributeSpacing { axis: DistAxis::Horizontal, gap: 8.0 }).unwrap();
    assert_eq!(e.doc.paths[1].anchors[0].p[0] - e.doc.paths[0].anchors[0].p[0], dx);
    assert!((e.doc.outline_bbox(2).0 - e.doc.outline_bbox(1).2 - 8.0).abs() < 0.001);
}
#[test]
fn add_anchor_points_splits_compound_holes() {
    let mut e = editor();
    select(&mut e, &[10, 20]);
    e.try_execute(C::Object(O::CompoundMake)).unwrap();
    let before = e.doc.clone();
    e.try_execute(C::Object(O::AddAnchors)).unwrap();
    assert_eq!(e.doc.paths[0].anchors.len(), 8);
    assert_eq!(e.doc.paths[0].holes[0].len(), 8);
    varos_core::format::check_structure(&e.doc, &varos_core::format::Limits::DEFAULT).unwrap();
    varos_core::format::validate(&e.doc, &varos_core::format::Limits::DEFAULT).unwrap();
    e.execute(C::Undo);
    assert_eq!(e.doc, before);
}
#[test]
fn headless_batch_replays_command_wiring_and_one_undo() {
    let mut e = editor();
    let before = e.doc.clone();
    let batch=varos_core::bridge::parse_batch(br#"{"api":"0.1","commands":[{"Selection":"all"},{"Object":"lock"},{"Object":"unlock_all"},{"Selection":"all"},{"Object":"reverse"}]}"#).unwrap();
    e.execute_batch(batch).unwrap();
    assert_ne!(e.doc, before);
    e.execute(C::Undo);
    assert_eq!(e.doc, before);
}
#[test]
fn option_direct_click_climbs_exactly_one_nested_group() {
    let mut e = editor();
    select(&mut e, &[10, 20]);
    e.execute(C::GroupSelection);
    let inner = e.doc.top_group_of_path(10).unwrap();
    select(&mut e, &[10, 20, 30]);
    e.execute(C::GroupSelection);
    let outer = e.doc.top_group_of_path(10).unwrap();
    e.escape();
    e.try_execute(C::Selection(S::Group(10))).unwrap();
    assert_eq!(e.objsel, [10].into_iter().collect());
    e.try_execute(C::Selection(S::Group(10))).unwrap();
    assert_eq!(e.doc.node_paths(inner).len(), 2);
    assert_eq!(e.objsel, [10, 20].into_iter().collect());
    e.try_execute(C::Selection(S::Group(10))).unwrap();
    assert_eq!(e.objsel, e.doc.node_paths(outer).into_iter().collect());
    assert_eq!(e.objsel.len(), 3);
    e.try_execute(C::Selection(S::Group(10))).unwrap();
    assert_eq!(e.objsel, e.doc.node_paths(outer).into_iter().collect());
}
#[test]
fn lasso_world_anchor_object_additive_and_inert_contracts() {
    let mut e = editor();
    let polygon = vec![[-1.0, -1.0], [11.0, -1.0], [11.0, 11.0], [-1.0, 11.0]];
    e.try_execute(C::Lasso { points: polygon.clone(), objects: false, additive: false }).unwrap();
    assert_eq!(e.selected, [11, 12, 13, 14].into_iter().collect());
    select(&mut e, &[30]);
    e.try_execute(C::Lasso { points: polygon.clone(), objects: true, additive: true }).unwrap();
    assert_eq!(e.objsel, [10, 30].into_iter().collect());
    e.doc.paths[0].locked = true;
    e.try_execute(C::Lasso { points: polygon, objects: true, additive: false }).unwrap();
    assert!(e.objsel.is_empty());
    assert!(e.try_execute(C::Lasso { points: vec![[f32::NAN, 0.0]; 3], objects: false, additive: false }).is_err());
}
#[test]
fn explicit_anchor_add_delete_preserve_curve_and_undo() {
    let mut e = editor();
    e.try_execute(C::InsertAnchor { path: 10, segment: 0, t: 0.5 }).unwrap();
    assert_eq!(e.doc.paths[0].anchors.len(), 5);
    let id = *e.selected.iter().next().unwrap();
    assert_eq!(e.doc.anchor(id).unwrap().p, [5.0, 0.0]);
    e.try_execute(C::DeleteAnchor(id)).unwrap();
    assert_eq!(e.doc.paths[0].anchors.len(), 4);
    e.execute(C::Undo);
    assert_eq!(e.doc.paths[0].anchors.len(), 5);
    e.execute(C::Undo);
    assert_eq!(e.doc.paths[0].anchors.len(), 4);
    assert!(e.try_execute(C::InsertAnchor { path: 10, segment: 99, t: 0.5 }).is_err());
}
#[test]
fn paste_remembers_source_layers_and_recreates_missing_ancestry() {
    let mut e = editor();
    let source = e.doc.active_layer;
    select(&mut e, &[10]);
    e.execute(C::Copy);
    e.execute(C::Object(O::NewLayer));
    let destination = e.doc.active_layer;
    e.execute(C::SetPasteRemembersLayers(false));
    e.execute(C::Paste { offset: None });
    let first = *e.objsel.iter().next().unwrap();
    assert_eq!(e.doc.node(e.doc.node_of_path(first).unwrap()).unwrap().parent, Some(destination));
    e.execute(C::SetPasteRemembersLayers(true));
    e.execute(C::Paste { offset: None });
    let second = *e.objsel.iter().next().unwrap();
    assert_eq!(e.doc.node(e.doc.node_of_path(second).unwrap()).unwrap().parent, Some(source));
    let clip = e.clipboard().clone();
    let mut target = Editor::new();
    target.doc.nodes.iter_mut().find(|n| n.id == target.doc.active_layer).unwrap().name = "Different".into();
    let pasted = clip.paste_into_remembering_layers(&mut target.doc, [0.0, 0.0], true);
    let parent = target.doc.node(target.doc.node_of_path(pasted[0]).unwrap()).unwrap().parent.unwrap();
    assert_ne!(parent, target.doc.active_layer);
    assert_eq!(target.doc.node(parent).unwrap().name, e.doc.node(source).unwrap().name);
}
#[test]
fn reselect_restores_individual_anchors_without_promoting_paths() {
    let mut e = editor();
    e.try_execute(C::SelectAnchors(vec![11, 12])).unwrap();
    e.try_execute(C::Selection(S::Deselect)).unwrap();
    e.try_execute(C::Selection(S::Reselect)).unwrap();
    assert_eq!(e.selected, [11, 12].into_iter().collect());
    assert!(e.objsel.is_empty());
}
#[test]
fn explicit_anchor_tools_and_lasso_pointer_gestures_are_headless() {
    use varos_core::editor::ToolKind;
    let mut e = editor();
    e.ppu = 10.0;
    e.set_tool(ToolKind::AddAnchor);
    let rev = e.rev;
    e.pointer_down([5.0, 0.0]);
    e.pointer_up();
    assert_eq!(e.doc.paths[0].anchors.len(), 5);
    assert_eq!(e.rev, rev + 1);
    e.set_tool(ToolKind::DeleteAnchor);
    e.pointer_down([5.0, 0.0]);
    e.pointer_up();
    assert_eq!(e.doc.paths[0].anchors.len(), 4);
    assert_eq!(e.rev, rev + 2);
    e.escape();
    e.set_tool(ToolKind::Lasso);
    e.pointer_down([-1.0, -1.0]);
    for p in [[11.0, -1.0], [11.0, 11.0], [-1.0, 11.0]] {
        e.pointer_move(p);
    }
    e.pointer_up();
    assert_eq!(e.selected.len(), 4);
    assert_eq!(e.rev, rev + 2);
}
#[test]
fn anchor_point_type_is_undoable_and_rejects_unknown_anchor() {
    let mut e = editor();
    e.try_execute(C::AnchorType { anchor: 11, smooth: true }).unwrap();
    assert!(e.doc.anchor(11).unwrap().smooth);
    assert!(e.doc.anchor(11).unwrap().hin.is_some());
    e.try_execute(C::AnchorType { anchor: 11, smooth: false }).unwrap();
    assert!(e.doc.anchor(11).unwrap().hin.is_none());
    e.execute(C::Undo);
    assert!(e.doc.anchor(11).unwrap().smooth);
    assert!(e.try_execute(C::AnchorType { anchor: 999, smooth: true }).is_err());
}
