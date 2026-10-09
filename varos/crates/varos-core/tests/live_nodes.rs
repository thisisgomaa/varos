use std::sync::{atomic::AtomicBool, Arc};
use varos_core::{
    live::{self, Action, Axis, Envelope, Kind, Orientation, Repeat, Warp},
    model::{Anchor, NodeKind, Path},
    EditCommand, Editor, ToolKind,
};
fn rectangle(ed: &mut Editor, x: f32) -> u32 {
    let id = ed.doc.nid();
    let mut anchors = Vec::new();
    for p in [[x, 0.], [x + 10., 0.], [x + 10., 10.], [x, 10.]] {
        anchors.push(Anchor { id: ed.doc.nid(), p, hin: None, hout: None, smooth: false });
    }
    ed.doc.paths.push(Path::new(id, anchors, true, Some([1., 0., 0., 1.]), None, 0.));
    ed.doc.sync_tree();
    id
}
fn setup(kind: Kind) -> (Editor, u32, Vec<u32>) {
    let mut ed = Editor::new();
    let a = rectangle(&mut ed, 0.);
    let mut paths = vec![a];
    if matches!(kind, Kind::Blend { .. }) {
        paths.push(rectangle(&mut ed, 100.));
    }
    ed.try_execute(EditCommand::Live(Action::Make { paths: paths.clone(), kind })).unwrap();
    let node = live::selected_node(&ed).unwrap();
    (ed, node, paths)
}
fn evaluated(ed: &Editor, node: u32) -> Arc<Vec<Path>> {
    let n = ed.doc.node(node).unwrap();
    let NodeKind::Live(kind) = n.kind else { panic!() };
    ed.live_cache.evaluate(&ed.doc, n, kind).unwrap()
}
#[test]
fn blend_geometry_colour_spine_and_orientation_goldens() {
    let (mut ed, n, ids) = setup(Kind::Blend { spine: None, steps: 1, orientation: Orientation::Page });
    let i = ed.doc.pidx(ids[1]).unwrap();
    ed.doc.paths[i].fill = varos_core::model::Paint::Solid([0., 0., 1., 1.]);
    ed.doc.paths[i].opacity = 0.5;
    ed.doc.paths[i].stroke_width = 10.;
    let p = evaluated(&ed, n);
    assert_eq!(p.len(), 3);
    assert_eq!(p[1].anchors[0].p, [50., 0.]);
    assert_eq!(p[1].fill, varos_core::model::Paint::Solid([0.5, 0., 0.5, 1.]));
    assert_eq!(p[1].opacity, 0.75);
    assert_eq!(p[1].stroke_width, 5.);
    let id = ed.doc.nid();
    let a = Anchor { id: ed.doc.nid(), p: [5., 5.], hin: None, hout: None, smooth: false };
    let b = Anchor { id: ed.doc.nid(), p: [5., 105.], hin: None, hout: None, smooth: false };
    ed.doc.paths.push(Path::new(id, vec![a, b], false, None, None, 0.));
    ed.doc.sync_tree();
    live::execute(&mut ed, Action::Spine { node: n, path: id }).unwrap();
    let p = evaluated(&ed, n);
    assert!((p[1].anchors[0].p[1] - 50.).abs() < 0.001);
    assert!((p[1].anchors[0].p[0]).abs() < 0.001);
    live::execute(
        &mut ed,
        Action::Options { node: n, kind: Kind::Blend { spine: Some(id), steps: 1, orientation: Orientation::Path } },
    )
    .unwrap();
    let p = evaluated(&ed, n);
    assert!((p[1].anchors[0].p[0] - 10.).abs() < 0.001);
}
#[test]
fn repeat_goldens_grid_radial_and_mirror() {
    for (kind, count, last) in [
        (Kind::Repeat { repeat: Repeat::Grid { rows: 2, cols: 2, gap: [2., 3.] } }, 4, [12., 13.]),
        (Kind::Repeat { repeat: Repeat::Mirror { axis: Axis::Vertical } }, 2, [20., 0.]),
        (Kind::Repeat { repeat: Repeat::Radial { count: 2, radius: 20. } }, 2, [-10., 10.]),
    ] {
        let (ed, n, _) = setup(kind);
        let p = evaluated(&ed, n);
        assert_eq!(p.len(), count);
        for (i, v) in last.iter().enumerate() {
            assert!((p[count - 1].anchors[0].p[i] - v).abs() < 0.001);
        }
    }
}
#[test]
fn envelope_mesh_and_warp_goldens() {
    let (ed, n, _) =
        setup(Kind::Envelope { envelope: Envelope::Mesh { points: [[0., 0.], [20., 0.], [0., 10.], [20., 20.]] } });
    let p = evaluated(&ed, n);
    assert_eq!(p[0].anchors[128].p, [20., 20.]);
    let (ed, n, _) = setup(Kind::Envelope { envelope: Envelope::Warp { preset: Warp::Arc, bend: 0.5 } });
    let p = evaluated(&ed, n);
    assert_eq!(p[0].anchors[32].p, [5., -5.]);
}
#[test]
fn per_node_cache_reuses_arc_and_invalidates_source_and_options() {
    let (mut ed, n, ids) = setup(Kind::Repeat { repeat: Repeat::Grid { rows: 1, cols: 2, gap: [0., 0.] } });
    let first = evaluated(&ed, n);
    let second = evaluated(&ed, n);
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(ed.live_cache.stats().evaluations, 1);
    ed.doc.paths.iter_mut().find(|p| p.id == ids[0]).unwrap().anchors[0].p[0] = -1.;
    let third = evaluated(&ed, n);
    assert!(!Arc::ptr_eq(&first, &third));
    assert_eq!(third[0].anchors[0].p[0], -1.);
    live::execute(
        &mut ed,
        Action::Options { node: n, kind: Kind::Repeat { repeat: Repeat::Grid { rows: 2, cols: 2, gap: [0., 0.] } } },
    )
    .unwrap();
    assert_eq!(evaluated(&ed, n).len(), 4);
    assert_eq!(ed.live_cache.stats().evaluations, 3);
}
#[test]
fn make_expand_release_and_options_are_undoable() {
    let (mut ed, n, _) = setup(Kind::Blend { spine: None, steps: 2, orientation: Orientation::Page });
    let before = ed.doc.clone();
    live::execute(&mut ed, Action::Expand { node: n }).unwrap();
    assert_eq!(ed.doc.paths.len(), 4);
    assert!(!live::has_live(&ed.doc));
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, before);
    live::execute(&mut ed, Action::Release { node: n }).unwrap();
    assert_eq!(ed.doc.paths.len(), 2);
    assert_eq!(ed.doc.node(n).unwrap().kind, NodeKind::Group);
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, before);
    ed.execute(EditCommand::Undo).unwrap();
    assert!(!live::has_live(&ed.doc));
    assert_eq!(ed.doc.paths.len(), 2);
}
#[test]
fn refusal_is_atomic_and_older_api_cannot_replay_live() {
    let (mut ed, n, _) = setup(Kind::Repeat { repeat: Repeat::Mirror { axis: Axis::Vertical } });
    let before = ed.doc.clone();
    let depth = ed.history_depths();
    assert!(live::execute(
        &mut ed,
        Action::Options {
            node: n,
            kind: Kind::Repeat { repeat: Repeat::Grid { rows: 1000, cols: 1000, gap: [0., 0.] } }
        }
    )
    .is_err());
    assert_eq!(ed.doc, before);
    assert_eq!(ed.history_depths(), depth);
    assert!(varos_core::bridge::parse_batch(
        br#"{"api":"0.1","commands":[{"Live":{"action":"isolate","node":null}}]}"#
    )
    .is_err());
}
#[test]
fn tool_two_clicks_and_double_click_isolates_editable_sources() {
    let mut ed = Editor::new();
    let a = rectangle(&mut ed, 0.);
    rectangle(&mut ed, 100.);
    ed.set_tool(ToolKind::Blend);
    ed.pointer_down([5., 5.]);
    ed.pointer_up();
    ed.pointer_down([105., 5.]);
    ed.pointer_up();
    assert!(live::has_live(&ed.doc));
    let n = live::selected_node(&ed).unwrap();
    ed.set_tool(ToolKind::Object);
    ed.double_click([5., 5.]);
    assert_eq!(ed.select_transform.isolation, Some(n));
    assert!(ed.in_isolation(a));
    live::execute(&mut ed, Action::Isolate { node: None }).unwrap();
    assert_eq!(ed.select_transform.isolation, None);
}
#[test]
fn svg_exports_evaluated_copies_and_whole_board_bounds() {
    let (ed, n, _) = setup(Kind::Repeat { repeat: Repeat::Grid { rows: 1, cols: 3, gap: [10., 0.] } });
    let plan = varos_core::svg::plan_svg_export(&ed.doc, varos_core::svg::ExportScope::WholeBoard).unwrap();
    for (a, b) in plan.pages[0].rect.iter().zip([0., 0., 50., 10.]) {
        assert!((*a - b).abs() < 0.001);
    }
    let files = varos_core::svg::export_svg_files(&ed.doc, &plan, &AtomicBool::new(false)).unwrap();
    let text = String::from_utf8(files[0].bytes.clone()).unwrap();
    assert_eq!(text.matches("<path ").count(), 3);
    assert_eq!(evaluated(&ed, n).len(), 3);
}

#[test]
fn unequal_anchor_counts_subdivide_cubics_and_curved_envelope_bakes() {
    let mut ed = Editor::new();
    let a = rectangle(&mut ed, 0.);
    let b = rectangle(&mut ed, 100.);
    let i = ed.doc.pidx(b).unwrap();
    ed.doc.paths[i].anchors.remove(2);
    live::execute(
        &mut ed,
        Action::Make { paths: vec![a, b], kind: Kind::Blend { spine: None, steps: 1, orientation: Orientation::Page } },
    )
    .unwrap();
    let node = live::selected_node(&ed).unwrap();
    let p = evaluated(&ed, node);
    assert_eq!(p.len(), 3);
    assert_eq!(p[0].anchors.len(), 12);
    assert_eq!(p[2].anchors.len(), 12);
    let (mut ed, _, ids) = setup(Kind::Envelope { envelope: Envelope::Warp { preset: Warp::Flag, bend: 0.2 } });
    let n = live::selected_node(&ed).unwrap();
    let i = ed.doc.pidx(ids[0]).unwrap();
    ed.doc.paths[i].anchors[0].hout = Some([3., -5.]);
    let p = evaluated(&ed, n);
    assert!(p[0].anchors.iter().all(|a| a.hin.is_none() && a.hout.is_none()));
    live::execute(&mut ed, Action::Expand { node: n }).unwrap();
    assert!(!live::has_live(&ed.doc));
}
#[test]
fn derived_copy_hit_and_selection_bounds_follow_geometry() {
    let (mut ed, n, _) = setup(Kind::Repeat { repeat: Repeat::Grid { rows: 1, cols: 3, gap: [10., 0.] } });
    let before = ed.live_cache.stats();
    assert!(ed.path_under([45., 5.]).is_some());
    let bb = ed.obj_bbox().unwrap();
    assert!((bb.2 - 50.).abs() < 0.001);
    let evals = ed.live_cache.stats().evaluations;
    varos_core::build_scene(&ed, 1.);
    varos_core::build_scene(&ed, 2.);
    assert_eq!(ed.live_cache.stats().evaluations, evals.max(before.evaluations));
    ed.set_tool(ToolKind::Object);
    ed.double_click([45., 5.]);
    assert_eq!(ed.select_transform.isolation, Some(n));
    assert!(ed.path_under([45., 5.]).is_none());
}

#[test]
fn copying_live_sources_preserves_kind_and_delete_prunes_live_container() {
    let (mut ed, n, _) = setup(Kind::Repeat { repeat: Repeat::Mirror { axis: Axis::Horizontal } });
    ed.try_execute(EditCommand::Copy).unwrap();
    ed.try_execute(EditCommand::Paste { offset: Some([100., 100.]) }).unwrap();
    assert_eq!(ed.doc.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Live(_))).count(), 2);
    assert!(live::selected_node(&ed).is_some());
    ed.try_execute(EditCommand::DeleteSelected).unwrap();
    assert_eq!(ed.doc.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Live(_))).count(), 1);
    assert!(ed.doc.node(n).is_some());
}

#[test]
fn idle_scene_and_selection_frames_reuse_document_evaluation() {
    let (ed, _, _) = setup(Kind::Repeat { repeat: Repeat::Radial { count: 12, radius: 40. } });
    varos_core::build_scene(&ed, 1.);
    let before = ed.live_cache.stats();
    for _ in 0..5 {
        varos_core::build_scene(&ed, 1.);
        ed.obj_bbox();
    }
    let after = ed.live_cache.stats();
    assert_eq!(after.document_evaluations, before.document_evaluations);
    assert_eq!(after.evaluations, before.evaluations);
    assert!(after.document_hits >= before.document_hits + 10);
}

#[test]
fn isolated_source_nudge_recomputes_and_undo_restores_invalid_gestures_rollback() {
    let (mut ed, n, ids) = setup(Kind::Blend { spine: None, steps: 1, orientation: Orientation::Page });
    live::execute(&mut ed, Action::Isolate { node: Some(n) }).unwrap();
    ed.set_tool(ToolKind::Direct);
    ed.objsel.clear();
    let pi = ed.doc.pidx(ids[0]).unwrap();
    ed.selected.insert(ed.doc.paths[pi].anchors[0].id);
    let before = ed.doc.clone();
    let derived = ed
        .live_cache
        .evaluate(
            &ed.doc,
            ed.doc.node(n).unwrap(),
            Kind::Blend { spine: None, steps: 1, orientation: Orientation::Page },
        )
        .unwrap();
    ed.try_execute(EditCommand::Nudge { x: 4., y: 2. }).unwrap();
    ed.commit();
    assert_ne!(ed.doc, before);
    let changed = evaluated(&ed, n);
    assert_ne!(changed[1].anchors[0].p, derived[1].anchors[0].p);
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, before);
    let history = ed.history_depths();
    ed.begin();
    ed.doc.paths[pi].anchors.clear();
    ed.dirty = true;
    ed.commit();
    assert_eq!(ed.doc, before);
    assert_eq!(ed.history_depths(), history);
    assert!(ed.last_error.is_some());
}

#[test]
fn repeated_invalid_source_command_returns_error_and_preserves_document_and_history() {
    let (mut ed, n, ids) = setup(Kind::Blend { spine: None, steps: 1, orientation: Orientation::Page });
    live::execute(&mut ed, Action::Isolate { node: Some(n) }).unwrap();
    ed.set_tool(ToolKind::Direct);
    ed.objsel.clear();
    let pi = ed.doc.pidx(ids[0]).unwrap();
    ed.selected = ed.doc.paths[pi].anchors.iter().map(|a| a.id).collect();
    let before = ed.doc.clone();
    let history = ed.history_depths();
    let error = ed.execute(EditCommand::DeleteSelected).unwrap_err();
    ed.last_error = Some(error.clone());
    assert_eq!(ed.execute(EditCommand::DeleteSelected), Err(error));
    assert_eq!(ed.doc, before);
    assert_eq!(ed.history_depths(), history);
}

#[test]
fn replacement_spine_restores_root_ownership_and_roundtrips() {
    let (mut ed, node, _) = setup(Kind::Blend { spine: None, steps: 1, orientation: Orientation::Page });
    // Exercise the legal root-level container rather than the default layer host.
    let parent = ed.doc.node(node).unwrap().parent.unwrap();
    ed.doc.nodes.iter_mut().find(|n| n.id == parent).unwrap().children.retain(|id| *id != node);
    ed.doc.nodes.iter_mut().find(|n| n.id == node).unwrap().parent = None;
    ed.doc.roots.push(node);
    let mut spines = Vec::new();
    for x in [20., 30.] {
        let id = rectangle(&mut ed, x);
        let i = ed.doc.pidx(id).unwrap();
        ed.doc.paths[i].closed = false;
        ed.doc.paths[i].anchors.truncate(2);
        live::execute(&mut ed, Action::Spine { node, path: id }).unwrap();
        spines.push(id);
    }
    let released = ed.doc.node_of_path(spines[0]).unwrap();
    assert_eq!(ed.doc.node(released).unwrap().parent, None);
    assert!(ed.doc.roots.contains(&released));
    let limits = varos_core::format::Limits::default();
    let bytes = varos_core::format::encode_model(&ed.doc, &limits).unwrap();
    assert!(varos_core::format::decode_model(bytes.as_bytes(), None, &limits).is_ok());
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc.node(released).unwrap().parent, Some(node));
}

#[test]
fn make_preserves_global_fill_and_stroke_links_through_release() {
    use varos_core::{model::Paint, swatches::Swatch};
    let mut ed = Editor::new();
    let path = rectangle(&mut ed, 0.);
    let swatch = ed.doc.nid();
    ed.doc.swatches.push(Swatch {
        id: swatch,
        name: "Global".into(),
        paint: Paint::Solid([1., 0., 0., 1.]),
        global: true,
        group: String::new(),
    });
    let i = ed.doc.pidx(path).unwrap();
    ed.doc.paths[i].fill = Paint::SwatchRef { id: swatch };
    ed.doc.paths[i].stroke = Paint::SwatchRef { id: swatch };
    live::execute(
        &mut ed,
        Action::Make {
            paths: vec![path],
            kind: Kind::Repeat { repeat: Repeat::Grid { rows: 1, cols: 2, gap: [0., 0.] } },
        },
    )
    .unwrap();
    let node = live::selected_node(&ed).unwrap();
    let red = evaluated(&ed, node);
    ed.doc.swatches[0].paint = Paint::Solid([0., 0., 1., 1.]);
    let blue = evaluated(&ed, node);
    assert!(!Arc::ptr_eq(&red, &blue));
    assert!(blue
        .iter()
        .all(|p| p.fill == Paint::Solid([0., 0., 1., 1.]) && p.stroke == Paint::Solid([0., 0., 1., 1.])));
    live::execute(&mut ed, Action::Release { node }).unwrap();
    assert_eq!(ed.doc.paths[i].fill, Paint::SwatchRef { id: swatch });
    assert_eq!(ed.doc.paths[i].stroke, Paint::SwatchRef { id: swatch });
}

#[test]
fn mesh_transform_copy_and_offset_paste_follow_visible_geometry() {
    use varos_core::{clipboard::Clipboard, select_transform::Transform};
    let (mut ed, node, ids) =
        setup(Kind::Envelope { envelope: Envelope::Mesh { points: [[0., 0.], [10., 0.], [0., 10.], [10., 10.]] } });
    ed.execute_targeted_batch(vec![varos_core::bridge::TargetEdit::Move { paths: ids.clone(), delta: [100., 50.] }])
        .unwrap();
    assert_eq!(evaluated(&ed, node)[0].anchors[0].p, [100., 50.]);
    let clip = Clipboard::capture(&ed.doc, &ids);
    let pasted = clip.paste_into(&mut ed.doc, [20., 30.]);
    let pasted_node = ed.doc.unit_of(pasted[0]).unwrap();
    assert_eq!(evaluated(&ed, pasted_node)[0].anchors[0].p, [120., 80.]);
    ed.objsel = ids.iter().copied().collect();
    ed.try_execute(EditCommand::Transform(Transform {
        scale: [2., 3.],
        origin: Some([100., 50.]),
        movement: [5., 7.],
        copy: true,
        ..Default::default()
    }))
    .unwrap();
    let copied = live::selected_node(&ed).unwrap();
    assert_ne!(copied, node);
    assert_eq!(evaluated(&ed, copied)[0].anchors[0].p, [105., 57.]);
    assert_eq!(evaluated(&ed, copied)[0].anchors[128].p, [125., 87.]);
    assert_eq!(evaluated(&ed, node)[0].anchors[0].p, [100., 50.]);
}

#[test]
fn multisource_mesh_gesture_restarts_from_snapshot_and_undo_restores() {
    use varos_core::select_transform::Transform;
    let mut ed = Editor::new();
    let a = rectangle(&mut ed, 0.);
    let b = rectangle(&mut ed, 10.);
    live::execute(
        &mut ed,
        Action::Make {
            paths: vec![a, b],
            kind: Kind::Envelope { envelope: Envelope::Mesh { points: [[0., 0.], [20., 0.], [0., 10.], [20., 10.]] } },
        },
    )
    .unwrap();
    let node = live::selected_node(&ed).unwrap();
    let before = ed.doc.clone();
    ed.try_execute(EditCommand::TransformBegin).unwrap();
    for delta in [[10., 20.], [30., 40.]] {
        ed.try_execute(EditCommand::TransformLive(Transform { movement: delta, ..Default::default() })).unwrap();
    }
    assert_eq!(evaluated(&ed, node)[0].anchors[0].p, [30., 40.]);
    ed.try_execute(EditCommand::TransformCommit).unwrap();
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, before);
    ed.execute(EditCommand::Redo).unwrap();
    assert_eq!(evaluated(&ed, node)[0].anchors[0].p, [30., 40.]);
}
