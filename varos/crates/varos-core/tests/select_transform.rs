use varos_core::{
    model::{NodeKind, ShapeKind},
    select_transform::*,
    EditCommand, Editor,
};
fn rect(ed: &mut Editor, x: f32) -> u32 {
    ed.try_execute_created(EditCommand::AddShape {
        kind: ShapeKind::Rect,
        bounds: [x, 0., 10., 20.],
        parent: None,
        fill: Some([1., 0., 0., 1.]),
        stroke: None,
        stroke_width: 1.,
        opacity: 1.,
        name: None,
    })
    .unwrap()
}
fn selected() -> (Editor, u32) {
    let mut e = Editor::new();
    let id = rect(&mut e, 0.);
    e.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
    (e, id)
}
fn point(e: &Editor, id: u32) -> [f32; 2] {
    e.doc.paths[e.doc.pidx(id).unwrap()].anchors[0].p
}
fn near(p: [f32; 2], q: [f32; 2]) {
    assert!((p[0] - q[0]).abs() < 0.001 && (p[1] - q[1]).abs() < 0.001, "{p:?} != {q:?}");
}
#[test]
fn reflect_axis_geometry_and_undo() {
    let (mut e, id) = selected();
    let before = e.doc.clone();
    e.try_execute(EditCommand::Transform(Transform {
        reflect: Some(90.),
        origin: Some([0., 0.]),
        ..Default::default()
    }))
    .unwrap();
    near(point(&e, id), [0., 0.]);
    near(e.doc.paths[0].anchors[1].p, [-10., 0.]);
    e.undo();
    assert_eq!(e.doc, before);
}
#[test]
fn shear_bakes_handles_and_holes() {
    let (mut e, id) = selected();
    let p = &mut e.doc.paths[0];
    p.anchors[0].hin = Some([1., 2.]);
    p.holes = vec![p.anchors.clone()];
    e.try_execute(EditCommand::Transform(Transform { shear: 45., origin: Some([0., 0.]), ..Default::default() }))
        .unwrap();
    near(e.doc.paths[0].anchors[0].hin.unwrap(), [3., 2.]);
    near(e.doc.paths[0].holes[0][2].p, [30., 20.]);
    assert!(e.doc.unit_xform(id).is_identity());
}
#[test]
fn preview_is_absolute_and_one_undo_step() {
    let (mut e, id) = selected();
    let before = e.doc.clone();
    let rev = e.rev;
    e.try_execute(EditCommand::TransformBegin).unwrap();
    for x in [10., 20., 30.] {
        e.try_execute(EditCommand::TransformLive(Transform { movement: [x, 0.], ..Default::default() })).unwrap();
    }
    near(point(&e, id), [30., 0.]);
    assert_eq!(e.rev, rev);
    e.try_execute(EditCommand::TransformCommit).unwrap();
    assert_eq!(e.rev, rev + 1);
    e.undo();
    assert_eq!(e.doc, before);
}
#[test]
fn preview_cancel_restores_without_history() {
    let (mut e, _) = selected();
    let before = e.doc.clone();
    let rev = e.rev;
    e.execute(EditCommand::TransformBegin);
    e.execute(EditCommand::TransformLive(Transform { scale: [2., 3.], copy: true, ..Default::default() }));
    e.execute(EditCommand::TransformCancel);
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
    assert!(!e.transaction_open());
}
#[test]
fn preview_copy_keeps_original_and_is_one_step() {
    let (mut e, id) = selected();
    let before = e.doc.clone();
    let rev = e.rev;
    e.execute(EditCommand::TransformBegin);
    e.execute(EditCommand::TransformLive(Transform { movement: [10., 0.], copy: true, ..Default::default() }));
    e.execute(EditCommand::TransformLive(Transform { movement: [20., 0.], copy: true, ..Default::default() }));
    e.execute(EditCommand::TransformCommit);
    assert_eq!(e.doc.paths.len(), 2);
    near(point(&e, id), [0., 0.]);
    assert_eq!(e.rev, rev + 1);
    e.undo();
    assert_eq!(e.doc, before);
}
#[test]
fn each_scales_about_separate_centres() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::Transform(Transform { each: true, scale: [2., 1.], ..Default::default() }));
    near(point(&e, a), [-5., 0.]);
    near(point(&e, b), [95., 0.]);
}
#[test]
fn each_random_is_seeded() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    let before = e.doc.clone();
    let s = Transform { each: true, random: true, angle: 90., movement: [100., 50.], ..Default::default() };
    e.execute(EditCommand::Transform(s));
    let after = e.doc.clone();
    e.replace_doc(before);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::Transform(s));
    assert_eq!(e.doc, after);
}
#[test]
fn wand_tolerance_modes_and_inert_paths() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    let c = rect(&mut e, 200.);
    let bi = e.doc.pidx(b).unwrap();
    e.doc.paths[bi].fill = varos_core::model::Paint::Solid([0.99, 0., 0., 1.]);
    let ci = e.doc.pidx(c).unwrap();
    e.doc.paths[ci].locked = true;
    e.execute(EditCommand::MagicWand {
        source: a,
        options: WandOptions { colour: 0.02, ..Default::default() },
        mode: SelectMode::Set,
    });
    assert_eq!(e.objsel.len(), 2);
    e.execute(EditCommand::MagicWand { source: a, options: WandOptions::default(), mode: SelectMode::Subtract });
    assert_eq!(e.objsel, [b].into_iter().collect());
}
#[test]
fn wand_weight_stroke_opacity() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    let bi = e.doc.pidx(b).unwrap();
    e.doc.paths[bi].opacity = 0.8;
    let o = WandOptions {
        pick: PickOptions { fill: false, stroke: true, weight: true, opacity: true },
        opacity: 0.1,
        ..Default::default()
    };
    e.execute(EditCommand::MagicWand { source: a, options: o, mode: SelectMode::Set });
    assert_eq!(e.objsel, [a].into_iter().collect());
}
#[test]
fn eyedropper_toggles_and_shift_colour_only() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    let i = e.doc.pidx(b).unwrap();
    e.doc.paths[i].opacity = 0.4;
    e.doc.paths[i].stroke_width = 9.;
    e.execute(EditCommand::Eyedropper {
        source: b,
        options: PickOptions { fill: false, stroke: false, weight: false, opacity: true },
        colour_only: false,
    });
    assert_eq!(e.doc.paths[e.doc.pidx(a).unwrap()].opacity, 0.4);
    assert_eq!(e.doc.paths[e.doc.pidx(a).unwrap()].stroke_width, 1.);
    e.execute(EditCommand::Eyedropper { source: b, options: PickOptions::default(), colour_only: true });
    assert_eq!(e.doc.paths[e.doc.pidx(a).unwrap()].stroke_width, 1.);
}
#[test]
fn isolation_confines_hit_selection_all_and_marquee() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    let c = rect(&mut e, 200.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::GroupSelection);
    let group = e.doc.top_group_of_path(a).unwrap();
    e.execute(EditCommand::Isolate(Some(group)));
    assert!(e.path_under([205., 5.]).is_none());
    assert!(e.try_execute(EditCommand::SelectPaths(vec![c])).is_err());
    e.select_all();
    assert_eq!(e.objsel.len(), 2);
    e.pointer_down([-20., -20.]);
    e.pointer_move([300., 50.]);
    e.pointer_up();
    assert!(!e.objsel.contains(&c));
    e.escape();
    assert!(e.select_transform.isolation.is_none());
}
#[test]
fn double_click_group_and_empty_exit() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::GroupSelection);
    e.double_click([5., 5.]);
    assert!(e.select_transform.isolation.is_some());
    e.double_click([400., 400.]);
    assert!(e.select_transform.isolation.is_none());
}
#[test]
fn reflect_pointer_axis_and_alt_dialog() {
    let (mut e, id) = selected();
    e.set_tool(varos_core::ToolKind::Reflect);
    e.pointer_down([0., 0.]);
    e.pointer_move([0., 100.]);
    e.pointer_up();
    near(e.doc.paths[e.doc.pidx(id).unwrap()].anchors[1].p, [-10., 0.]);
    e.mods.alt = true;
    e.pointer_down([30., 30.]);
    e.pointer_up();
    assert!(e.select_transform.dialog.is_some());
}
#[test]
fn free_transform_command_side_handle_shears() {
    let (mut e, _) = selected();
    e.set_tool(varos_core::ToolKind::FreeTransform);
    e.mods.ctrl = true;
    e.pointer_down([5., 0.]);
    e.pointer_move([15., 0.]);
    e.pointer_up();
    near(e.doc.paths[0].anchors[0].p, [10., 0.]);
    near(e.doc.paths[0].anchors[2].p, [10., 20.]);
}
#[test]
fn layer_collect_merge_flatten_and_undo() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    let before = e.doc.clone();
    let nodes = vec![e.doc.node_of_path(a).unwrap(), e.doc.node_of_path(b).unwrap()];
    e.execute(EditCommand::LayerFamily { action: LayerAction::Collect, nodes });
    assert_eq!(e.doc.node_paths(e.doc.active_layer).len(), 2);
    e.undo();
    assert_eq!(e.doc, before);
    e.execute(EditCommand::LayerFamily { action: LayerAction::Flatten, nodes: vec![] });
    assert_eq!(e.doc.nodes.iter().filter(|n| n.kind == NodeKind::Layer).count(), 1);
    assert_eq!(e.doc.paths.len(), 2);
}
#[test]
fn layer_release_sequence_build_preserve_order() {
    for build in [false, true] {
        let (mut e, _) = selected();
        rect(&mut e, 100.);
        let layer = e.doc.active_layer;
        let before = e.doc.clone();
        e.try_execute(EditCommand::LayerFamily {
            action: if build { LayerAction::ReleaseBuild } else { LayerAction::ReleaseSequence },
            nodes: vec![layer],
        })
        .unwrap();
        assert_eq!(e.doc.node(layer).unwrap().children.len(), 2);
        assert_eq!(e.doc.paths.len(), if build { 3 } else { 2 });
        e.undo();
        assert_eq!(e.doc, before);
    }
}
#[test]
fn layer_hide_lock_others_locate() {
    for action in [LayerAction::HideOthers, LayerAction::LockOthers] {
        let (mut e, a) = selected();
        let b = rect(&mut e, 100.);
        let other = e.doc.node_of_path(b).unwrap();
        e.execute(EditCommand::LayerFamily { action: LayerAction::Collect, nodes: vec![other] });
        let n = e.doc.node_of_path(a).unwrap();
        e.execute(EditCommand::LayerFamily { action, nodes: vec![n] });
        assert!(!e.doc.eff_hidden(a) && !e.doc.eff_locked(a));
        assert!(e.doc.eff_hidden(b) || e.doc.eff_locked(b));
        e.undo();
        assert!(!e.doc.eff_hidden(b) && !e.doc.eff_locked(b));
        e.execute(EditCommand::LayerFamily { action: LayerAction::Locate, nodes: vec![n] });
        assert_eq!(e.select_transform.located, Some(n));
    }
}
#[test]
fn invalid_transform_refused_without_mutation() {
    let (mut e, _) = selected();
    let before = e.doc.clone();
    assert!(e.try_execute(EditCommand::Transform(Transform { shear: 90., ..Default::default() })).is_err());
    assert_eq!(e.doc, before);
}

#[test]
fn isolation_direct_selection_and_scene_opacity() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    let c = rect(&mut e, 200.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::GroupSelection);
    let group = e.doc.top_group_of_path(a).unwrap();
    let signature = varos_core::scene::scene_signature(&e, varos_core::View::identity(), [800, 600]);
    e.execute(EditCommand::Isolate(Some(group)));
    assert_ne!(signature, varos_core::scene::scene_signature(&e, varos_core::View::identity(), [800, 600]));
    assert!(e.nearest_anchor([200., 0.], 20., false).is_none());
    e.set_tool(varos_core::ToolKind::Direct);
    e.pointer_down([-30., -30.]);
    e.pointer_move([300., 50.]);
    e.pointer_up();
    assert!(e.selected.iter().all(|id| e.doc.pid_of_anchor(*id) != Some(c)));
    let scene = varos_core::build_scene(&e, 1.);
    assert!(scene
        .content
        .iter()
        .flat_map(|g| g.prims())
        .any(|p| matches!(p,varos_core::Prim::Fill {color,..} if color[0]==1. && (color[3]-0.25).abs()<0.001)));
}
#[test]
fn transform_each_group_moves_rigidly_with_seeded_random() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::GroupSelection);
    e.execute(EditCommand::Transform(Transform {
        each: true,
        random: true,
        movement: [100., 50.],
        ..Default::default()
    }));
    let p = point(&e, a);
    let q = point(&e, b);
    near([q[0] - p[0], q[1] - p[1]], [100., 0.]);
}
#[test]
fn transform_copy_preserves_group_and_noop_preserves_rotation() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::GroupSelection);
    e.execute(EditCommand::SetObjectRotation(30.));
    let before = e.doc.clone();
    let rev = e.rev;
    e.execute(EditCommand::TransformBegin);
    e.execute(EditCommand::TransformLive(Transform::default()));
    e.execute(EditCommand::TransformCommit);
    assert_eq!(e.rev, rev);
    assert_eq!(e.doc, before);
    e.execute(EditCommand::Transform(Transform { copy: true, movement: [200., 0.], ..Default::default() }));
    assert_eq!(e.doc.paths.len(), 4);
    assert_eq!(e.doc.nodes.iter().filter(|n| n.kind == NodeKind::Group).count(), 2);
    e.undo();
    assert_eq!(e.doc, before);
}
#[test]
fn cli_batch_tools_require_12() {
    let bytes = br#"{"api":"1.2","commands":[{"Transform":{"shear":20}}]}"#;
    assert!(varos_core::bridge::parse_batch(bytes).is_ok());
    let old = br#"{"api":"0.1","commands":[{"Transform":{"shear":20}}]}"#;
    assert!(varos_core::bridge::parse_batch(old).is_err());
}
#[test]
fn merge_selected_layers_preserves_art_and_one_undo() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    let first = e.doc.active_layer;
    e.execute(EditCommand::LayerFamily { action: LayerAction::Collect, nodes: vec![e.doc.node_of_path(b).unwrap()] });
    let second = e.doc.active_layer;
    let before = e.doc.clone();
    let order: Vec<_> = e.doc.paths.iter().map(|p| p.id).collect();
    let rev = e.rev;
    e.try_execute(EditCommand::LayerFamily { action: LayerAction::Merge, nodes: vec![first, second] }).unwrap();
    assert_eq!(e.doc.paths.len(), 2);
    assert_eq!(e.doc.paths.iter().map(|p| p.id).collect::<Vec<_>>(), order);
    assert!(e.doc.pidx(a).is_some());
    assert_eq!(e.doc.nodes.iter().filter(|n| n.kind == NodeKind::Layer).count(), 1);
    assert_eq!(e.rev, rev + 1);
    e.undo();
    assert_eq!(e.doc, before);
}

#[test]
fn rotate_scale_dialog_math_uses_shared_origin() {
    let (mut e, id) = selected();
    e.try_execute(EditCommand::Transform(Transform {
        scale: [2., 3.],
        angle: 90.,
        origin: Some([0., 0.]),
        ..Default::default()
    }))
    .unwrap();
    near(e.doc.paths[e.doc.pidx(id).unwrap()].anchors[1].p, [0., 20.]);
    near(e.doc.paths[e.doc.pidx(id).unwrap()].anchors[2].p, [-60., 20.]);
}
#[test]
fn tool_options_are_commands_without_document_history() {
    let (mut e, _) = selected();
    let before = e.doc.clone();
    let rev = e.rev;
    e.try_execute(EditCommand::SetWandOptions(WandOptions { weight: 3., ..Default::default() })).unwrap();
    e.try_execute(EditCommand::SetEyedropperOptions(PickOptions {
        fill: false,
        stroke: true,
        weight: false,
        opacity: false,
    }))
    .unwrap();
    assert_eq!(e.select_transform.wand.weight, 3.);
    assert!(!e.select_transform.pick.fill);
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
    assert!(e.try_execute(EditCommand::SetWandOptions(WandOptions { colour: -1., ..Default::default() })).is_err());
}

#[test]
fn escape_cancels_pointer_preview_and_cannot_resume() {
    let (mut e, _) = selected();
    let before = e.doc.clone();
    e.set_tool(varos_core::ToolKind::Shear);
    e.pointer_down([0., 0.]);
    e.pointer_move([20., 20.]);
    e.escape();
    e.pointer_move([40., 40.]);
    e.pointer_up();
    assert_eq!(e.doc, before);
    assert!(!e.transaction_open());
}

#[test]
fn release_build_1000_paths_refused_before_mutation() {
    let (mut e, id) = selected();
    for _ in 1..1000 {
        let p = e.doc.clone_path(id);
        e.doc.paths.push(p);
    }
    e.doc.sync_tree();
    let before = e.doc.clone();
    let rev = e.rev;
    let layer = e.doc.active_layer;
    assert!(e.try_execute(EditCommand::LayerFamily { action: LayerAction::ReleaseBuild, nodes: vec![layer] }).is_err());
    e.execute(EditCommand::LayerFamily { action: LayerAction::ReleaseBuild, nodes: vec![layer] });
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
    assert!(!e.transaction_open());
}

#[test]
fn flatten_preserves_nested_hidden_locked_state_and_undo() {
    let (mut e, a) = selected();
    let b = rect(&mut e, 100.);
    e.execute(EditCommand::SelectPaths(vec![a, b]));
    e.execute(EditCommand::GroupSelection);
    let original_layer = e.doc.active_layer;
    e.execute(EditCommand::LayerFamily { action: LayerAction::Collect, nodes: vec![original_layer] });
    let outer = e.doc.active_layer;
    let c = rect(&mut e, 200.);
    e.doc.nodes.iter_mut().find(|n| n.id == original_layer).unwrap().hidden = true;
    e.doc.nodes.iter_mut().find(|n| n.id == outer).unwrap().locked = true;
    let before = e.doc.clone();
    let state: Vec<_> = [a, b, c].into_iter().map(|p| (p, e.doc.eff_hidden(p), e.doc.eff_locked(p))).collect();
    e.try_execute(EditCommand::LayerFamily { action: LayerAction::Flatten, nodes: vec![] }).unwrap();
    assert_eq!(e.doc.nodes.iter().filter(|n| n.kind == NodeKind::Layer).count(), 1);
    for (p, h, l) in state {
        assert_eq!((e.doc.eff_hidden(p), e.doc.eff_locked(p)), (h, l));
    }
    e.undo();
    assert_eq!(e.doc, before);
}

#[test]
fn noop_sampling_and_hide_lock_preserve_document_revision_and_redo() {
    for action in [LayerAction::HideOthers, LayerAction::LockOthers] {
        let (mut e, a) = selected();
        let b = rect(&mut e, 100.);
        e.execute(EditCommand::LayerFamily {
            action: LayerAction::Collect,
            nodes: vec![e.doc.node_of_path(b).unwrap()],
        });
        let n = e.doc.node_of_path(a).unwrap();
        e.execute(EditCommand::LayerFamily { action, nodes: vec![n] });
        e.execute(EditCommand::SelectPaths(vec![a]));
        e.execute(EditCommand::Transform(Transform { movement: [5., 0.], ..Default::default() }));
        e.undo();
        let before = e.doc.clone();
        let redo = e.history_preview(true).cloned();
        let undo = e.history_preview(false).cloned();
        let rev = e.rev;
        e.execute(EditCommand::LayerFamily { action, nodes: vec![n] });
        for options in
            [PickOptions { fill: false, stroke: false, weight: false, opacity: false }, PickOptions::default()]
        {
            e.execute(EditCommand::Eyedropper { source: a, options, colour_only: false });
        }
        assert_eq!(e.doc, before);
        assert_eq!(e.rev, rev);
        assert_eq!(e.history_preview(true), redo.as_ref());
        assert_eq!(e.history_preview(false), undo.as_ref());
    }
}
