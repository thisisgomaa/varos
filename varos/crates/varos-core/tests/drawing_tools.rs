use varos_core::{
    drawing::{self, Action, Options, Shape, ShapeSpec},
    EditCommand, Editor, ToolKind,
};
fn draw(e: &mut Editor, action: Action) {
    e.try_execute(EditCommand::Drawing(action)).unwrap();
}
fn line(e: &mut Editor, origin: [f32; 2], size: [f32; 2]) -> u32 {
    draw(e, Action::Shape { spec: ShapeSpec { kind: Shape::Line, origin, size, ..Default::default() } });
    e.doc.paths.last().unwrap().id
}
#[test]
fn every_shape_gesture_produces_anchors_and_one_undo() {
    for tool in [
        ToolKind::Rect,
        ToolKind::Ellipse,
        ToolKind::RoundedRect,
        ToolKind::Polygon,
        ToolKind::Star,
        ToolKind::Line,
        ToolKind::Arc,
        ToolKind::Spiral,
        ToolKind::RectGrid,
        ToolKind::PolarGrid,
    ] {
        let mut e = Editor::new();
        let before = e.doc.clone();
        e.set_tool(tool);
        e.pointer_down([100., 100.]);
        e.pointer_move([160., 140.]);
        if matches!(tool, ToolKind::Rect | ToolKind::Ellipse) {
            assert!(!e.doc.paths.is_empty());
        } else {
            assert!(!e.drawing.preview.is_empty());
            assert!(e.doc.paths.is_empty());
        }
        e.pointer_up();
        assert!(!e.doc.paths.is_empty());
        assert!(e.doc.paths.iter().all(|p| p.anchors.len() >= 2 && p.anchors.iter().all(|a| a.id != 0)));
        assert_eq!(e.rev, 1);
        e.undo();
        assert_eq!(e.doc, before);
    }
}
#[test]
fn click_opens_numeric_dialog_without_undo_and_grid_is_one_edit() {
    let mut e = Editor::new();
    e.set_tool(ToolKind::RectGrid);
    e.pointer_down([200., 100.]);
    e.pointer_up();
    assert!(e.drawing.dialog.is_some());
    assert!(!e.history_available(false));
    assert!(e.doc.paths.is_empty());
    let s = ShapeSpec { rows: 2, columns: 3, ..e.drawing.dialog.take().unwrap() };
    draw(&mut e, Action::Shape { spec: s });
    assert_eq!(e.doc.paths.len(), 7);
    assert_eq!(e.rev, 1);
}
#[test]
fn arrows_change_live_radius_sides_and_star_points() {
    let mut e = Editor::new();
    for tool in [ToolKind::RoundedRect, ToolKind::Polygon, ToolKind::Star] {
        e.set_tool(tool);
        e.pointer_down([0., 0.]);
        e.pointer_move([100., 100.]);
        let r = e.drawing.shape.radius;
        let n = e.drawing.shape.sides;
        assert!(drawing::arrow(&mut e, true));
        if tool == ToolKind::RoundedRect {
            assert_eq!(e.drawing.shape.radius, r + 1.);
        } else {
            assert_eq!(e.drawing.shape.sides, n + 1);
        }
        e.pointer_up();
    }
    assert!(!drawing::arrow(&mut e, true));
}
#[test]
fn shift_line_and_centre_rectangle_are_constrained() {
    let mut e = Editor::new();
    e.set_tool(ToolKind::Line);
    e.mods.shift = true;
    e.pointer_down([0., 0.]);
    e.pointer_move([100., 30.]);
    e.pointer_up();
    assert!(e.doc.paths[0].anchors[1].p[1].abs() < 0.001);
    e.set_tool(ToolKind::RoundedRect);
    e.mods.alt = true;
    e.pointer_down([200., 200.]);
    e.pointer_move([230., 220.]);
    e.pointer_up();
    let i = e.doc.paths.len() - 1;
    let b = e.doc.bbox(i);
    assert!((b.0 - 170.).abs() < 0.001 && (b.2 - 230.).abs() < 0.001);
    assert!((b.1 - 170.).abs() < 0.001 && (b.3 - 230.).abs() < 0.001);
}
#[test]
fn pencil_fitter_stroke_continues_either_endpoint_and_undo_restores() {
    let mut e = Editor::new();
    e.set_tool(ToolKind::Pencil);
    e.pointer_down([10., 10.]);
    for p in [[20., 20.], [30., 15.], [40., 10.]] {
        e.pointer_move(p);
    }
    e.pointer_up();
    assert_eq!(e.doc.paths.len(), 1);
    assert_eq!(e.rev, 1);
    assert!(e.doc.paths[0].anchors.iter().any(|a| a.hout.is_some()));
    let before = e.doc.clone();
    e.pointer_down([40., 10.]);
    e.pointer_move([55., 5.]);
    e.pointer_up();
    assert_eq!(e.doc.paths.len(), 1);
    assert_eq!(e.rev, 2);
    assert_eq!(e.doc.paths[0].anchors.last().unwrap().p, [55., 5.]);
    e.undo();
    assert_eq!(e.doc, before);
    e.pointer_down([10., 10.]);
    e.pointer_move([0., 0.]);
    e.pointer_up();
    assert_eq!(e.doc.paths.len(), 1);
    assert_eq!(e.doc.paths[0].anchors.last().unwrap().p, [0., 0.]);
}
#[test]
fn pencil_continues_rotated_endpoint_in_world_coordinates() {
    let mut e = Editor::new();
    let id = line(&mut e, [0., 0.], [100., 0.]);
    let node = e.doc.unit_of(id).unwrap();
    e.doc.set_node_xform(node, varos_core::model::Xform { rot: std::f32::consts::FRAC_PI_2, piv: [0., 0.] });
    draw(&mut e, Action::Pencil { points: vec![[0., 100.], [0., 120.]], options: Options::default() });
    assert_eq!(e.doc.paths.len(), 1);
    assert_eq!(e.doc.paths[0].anchors.last().unwrap().p, [0., 120.]);
}
#[test]
fn selected_smooth_stroke_is_local_and_undoable() {
    let mut e = Editor::new();
    draw(
        &mut e,
        Action::Curvature { points: vec![[0., 0.], [50., 20.], [100., 0.], [150., 20.], [200., 0.]], closed: false },
    );
    let id = e.doc.paths[0].id;
    e.doc.paths[0].anchors[1].hin = None;
    e.doc.paths[0].anchors[1].hout = None;
    let before = e.doc.clone();
    e.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
    draw(&mut e, Action::Smooth { points: vec![[49., 20.], [51., 20.]], options: Options::default() });
    assert!(e.doc.paths[0].anchors[1].hin.is_some());
    assert_eq!(e.doc.paths[0].anchors[3], before.paths[0].anchors[3]);
    e.undo();
    assert_eq!(e.doc, before);
}
#[test]
fn path_eraser_splits_a_long_segment_preserves_style_and_one_undo() {
    let mut e = Editor::new();
    let id = line(&mut e, [0., 0.], [100., 0.]);
    let before = e.doc.clone();
    e.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
    draw(
        &mut e,
        Action::PathErase {
            points: vec![[50., -10.], [50., 10.]],
            options: Options { brush_radius: 5., ..Default::default() },
        },
    );
    assert_eq!(e.doc.paths.len(), 2);
    assert!(e.doc.paths.iter().all(|p| p.stroke == before.paths[0].stroke && !p.closed));
    assert!(e.doc.paths.iter().flat_map(|p| &p.anchors).all(|a| (a.p[0] - 50.).abs() > 4.99));
    e.undo();
    assert_eq!(e.doc, before);
}
#[test]
fn eraser_miss_is_noop_and_locked_paths_are_inert() {
    let mut e = Editor::new();
    let id = line(&mut e, [0., 0.], [100., 0.]);
    e.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
    let before = e.doc.clone();
    let n = e.rev;
    draw(&mut e, Action::PathErase { points: vec![[0., 100.], [100., 100.]], options: Options::default() });
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, n);
    e.doc.paths[0].locked = true;
    e.doc.sync_tree();
    let before = e.doc.clone();
    draw(&mut e, Action::PathErase { points: vec![[50., -10.], [50., 10.]], options: Options::default() });
    assert_eq!(e.doc, before);
}
#[test]
fn join_stroke_joins_only_targeted_open_paths() {
    let mut e = Editor::new();
    let a = line(&mut e, [0., 0.], [40., 0.]);
    let b = line(&mut e, [42., 0.], [40., 0.]);
    let untouched = line(&mut e, [300., 0.], [50., 0.]);
    e.try_execute(EditCommand::SelectPaths(vec![a, b])).unwrap();
    let before = e.doc.clone();
    draw(&mut e, Action::Join { points: vec![[38., -2.], [44., 2.]], options: Options::default() });
    assert_eq!(e.doc.paths.len(), 2);
    assert!(e.doc.paths.iter().any(|p| p.id == untouched));
    e.undo();
    assert_eq!(e.doc, before);
}
#[test]
fn curvature_retains_through_points_finish_is_single_undo_cancel_noop() {
    let mut e = Editor::new();
    e.set_tool(ToolKind::Curvature);
    let points = vec![[10., 10.], [50., 20.], [100., 10.]];
    for p in &points {
        e.pointer_down(*p);
        e.pointer_up();
    }
    assert!(e.doc.paths.is_empty());
    drawing::finish(&mut e, false);
    assert_eq!(e.doc.paths[0].anchors.iter().map(|a| a.p).collect::<Vec<_>>(), points);
    assert_eq!(e.rev, 1);
    let before = e.doc.clone();
    e.pointer_down([200., 200.]);
    e.pointer_up();
    e.pointer_down([250., 250.]);
    e.pointer_up();
    e.escape();
    assert_eq!(e.doc, before);
    assert!(e.drawing.curvature.is_empty());
}
#[test]
fn invalid_options_and_geometry_are_refused_without_changes() {
    let mut e = Editor::new();
    let before = e.doc.clone();
    assert!(e
        .try_execute(EditCommand::Drawing(Action::Options {
            options: Options { fidelity: f32::NAN, ..Default::default() }
        }))
        .is_err());
    assert!(e
        .try_execute(EditCommand::Drawing(Action::Pencil { points: vec![[0., 0.]], options: Options::default() }))
        .is_err());
    assert_eq!(e.doc, before);
}

#[test]
fn star_option_drag_changes_inner_radius_with_fixed_outer_radius() {
    let mut e = Editor::new();
    e.set_tool(ToolKind::Star);
    e.pointer_down([0., 0.]);
    e.pointer_move([100., 0.]);
    let before = e.drawing.preview[0].anchors[0].p;
    e.mods.alt = true;
    e.pointer_move([120., 0.]);
    let after = e.drawing.preview[0].anchors[0].p;
    assert_eq!(before, after);
    assert!(e.drawing.shape.inner_ratio > 0.5);
    e.pointer_up();
}
#[test]
fn eraser_fragments_stay_in_the_original_rotated_group() {
    let mut e = Editor::new();
    let a = line(&mut e, [0., 0.], [100., 0.]);
    let b = line(&mut e, [0., 100.], [100., 0.]);
    let group = e.doc.group(&[a, b]).unwrap();
    e.doc.set_node_xform(group, varos_core::model::Xform { rot: std::f32::consts::FRAC_PI_2, piv: [0., 0.] });
    e.try_execute(EditCommand::SelectPaths(vec![a])).unwrap();
    let before = e.doc.clone();
    draw(
        &mut e,
        Action::PathErase {
            points: vec![[-10., 50.], [10., 50.]],
            options: Options { brush_radius: 5., ..Default::default() },
        },
    );
    assert_eq!(e.doc.node_paths(group).len(), 3);
    for id in e.doc.node_paths(group) {
        assert_eq!(e.doc.unit_of(id), Some(group));
    }
    e.undo();
    assert_eq!(e.doc, before);
}

#[test]
fn smooth_eraser_and_join_pointer_strokes_commit_once() {
    for tool in [ToolKind::Smooth, ToolKind::PathEraser, ToolKind::Join] {
        let mut e = Editor::new();
        let a = line(&mut e, [0., 0.], [40., 0.]);
        let b = line(&mut e, [42., 0.], [40., 0.]);
        if tool == ToolKind::Smooth {
            let anchor_id = e.doc.nid();
            e.doc.paths[0].anchors.insert(
                1,
                varos_core::model::Anchor { id: anchor_id, p: [20., 10.], hin: None, hout: None, smooth: false },
            );
        }
        e.try_execute(EditCommand::SelectPaths(vec![a, b])).unwrap();
        e.set_tool(tool);
        let before = e.doc.clone();
        let rev = e.rev;
        let points = if tool == ToolKind::Smooth { [[19., 10.], [21., 10.]] } else { [[39., -2.], [43., 2.]] };
        e.pointer_down(points[0]);
        e.pointer_move(points[1]);
        e.pointer_up();
        assert_eq!(e.rev, rev + 1);
        e.undo();
        assert_eq!(e.doc, before);
    }
}
#[test]
fn drawing_finish_never_commits_another_tool_transaction() {
    let mut e = Editor::new();
    e.begin();
    assert!(e.transaction_open());
    drawing::finish(&mut e, true);
    assert!(e.transaction_open());
    drawing::finish(&mut e, false);
    assert!(e.transaction_open());
}
#[test]
fn join_miss_does_not_bake_an_unrelated_transform() {
    let mut e = Editor::new();
    let a = line(&mut e, [0., 0.], [100., 0.]);
    let node = e.doc.unit_of(a).unwrap();
    e.doc.set_node_xform(node, varos_core::model::Xform { rot: 0.2, piv: [0., 0.] });
    e.try_execute(EditCommand::SelectPaths(vec![a])).unwrap();
    let before = e.doc.clone();
    let rev = e.rev;
    draw(&mut e, Action::Join { points: vec![[500., 500.], [510., 510.]], options: Options::default() });
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
}

#[test]
fn stationary_freehand_clicks_are_noops_and_close_the_gesture() {
    for tool in [ToolKind::Pencil, ToolKind::Smooth, ToolKind::PathEraser, ToolKind::Join] {
        let mut e = Editor::new();
        e.set_tool(tool);
        let before = e.doc.clone();
        e.pointer_down([10., 10.]);
        e.pointer_up();
        assert_eq!(e.doc, before);
        assert!(!e.transaction_open());
        assert!(!e.history_available(false));
        assert!(e.stroke_error.is_none());
    }
    let mut e = Editor::new();
    assert!(e
        .try_execute(EditCommand::Drawing(Action::Pencil {
            points: vec![[10., 10.], [10., 10.]],
            options: Options::default(),
        }))
        .is_err());
}

#[test]
fn shift_radial_drag_preserves_pointer_radius() {
    for tool in [ToolKind::Polygon, ToolKind::Star, ToolKind::Spiral] {
        let mut e = Editor::new();
        e.set_tool(tool);
        e.mods.shift = true;
        e.pointer_down([0., 0.]);
        e.pointer_move([80., 60.]);
        let p = &e.drawing.preview[0];
        let radius = varos_core::geom::length(p.anchors[0].p);
        assert!((radius - 100.).abs() < 0.001, "radius {radius}");
        e.pointer_up();
    }
}

#[test]
fn drawing_refuses_protected_destination_and_mask_replacement_before_history() {
    let mut e = Editor::new();
    let layer = e.doc.active_layer;
    let index = e.doc.nodes.iter().position(|n| n.id == layer).unwrap();
    e.doc.nodes[index].locked = true;
    let before = e.doc.clone();
    assert!(e.try_execute(EditCommand::Drawing(Action::Shape { spec: ShapeSpec::default() })).is_err());
    assert_eq!(e.doc, before);
    assert!(!e.history_available(false));
    e.doc.nodes[index].locked = false;
    let a = line(&mut e, [0., 0.], [100., 0.]);
    let b = line(&mut e, [0., 20.], [100., 0.]);
    e.try_execute(EditCommand::SelectPaths(vec![a, b])).unwrap();
    e.try_execute(EditCommand::ClipMake).unwrap();
    let mask = [a, b].into_iter().find(|id| e.doc.is_mask_source(*id)).unwrap();
    e.try_execute(EditCommand::SelectPaths(vec![mask])).unwrap();
    for action in [
        Action::PathErase { points: vec![[50., -50.], [50., 50.]], options: Options::default() },
        Action::Join { points: vec![[0., -50.], [100., 50.]], options: Options::default() },
    ] {
        let before = e.doc.clone();
        let rev = e.rev;
        assert!(e.try_execute(EditCommand::Drawing(action)).is_err());
        assert_eq!(e.doc, before);
        assert_eq!(e.rev, rev);
        assert!(!e.transaction_open());
    }
}

#[test]
fn pencil_continuation_stays_inside_isolation_and_offsets_join_handle() {
    let mut e = Editor::new();
    let a = line(&mut e, [0., 0.], [100., 0.]);
    let b = line(&mut e, [0., 40.], [100., 0.]);
    let outside = line(&mut e, [300., 0.], [100., 0.]);
    let group = e.doc.group(&[a, b]).unwrap();
    e.select_transform.isolation = Some(group);
    let source = e.doc.paths[e.doc.pidx(outside).unwrap()].clone();
    draw(&mut e, Action::Pencil { points: vec![[402., 0.], [450., 0.]], options: Options::default() });
    assert_eq!(e.doc.paths[e.doc.pidx(outside).unwrap()], source);
    draw(
        &mut e,
        Action::Pencil {
            points: vec![[102., 0.], [150., 0.]],
            options: Options { smoothness: 0., ..Default::default() },
        },
    );
    let p = &e.doc.paths[e.doc.pidx(a).unwrap()];
    assert_eq!(p.anchors[1].p, [100., 0.]);
    assert!((p.anchors[1].hout.unwrap()[0] - 116.).abs() < 0.001);
}

#[test]
fn join_brushed_right_endpoints_even_when_left_endpoints_are_nearer() {
    for right in [false, true] {
        let mut e = Editor::new();
        let a = line(&mut e, [0., 0.], [100., 0.]);
        let b = line(&mut e, [2., 0.], [101., 0.]);
        e.try_execute(EditCommand::SelectPaths(vec![a, b])).unwrap();
        let before = e.doc.clone();
        let points = if right { vec![[99., -1.], [104., 1.]] } else { vec![[-1., -1.], [3., 1.]] };
        draw(&mut e, Action::Join { points, options: Options::default() });
        assert_eq!(e.doc.paths.len(), 1);
        let p = &e.doc.paths[0];
        let endpoints = [p.anchors[0].p, p.anchors.last().unwrap().p];
        assert_eq!(endpoints, if right { [[0., 0.], [2., 0.]] } else { [[100., 0.], [103., 0.]] });
        assert_eq!(e.rev, 3);
        e.undo();
        assert_eq!(e.doc, before);
    }
}

#[test]
fn join_consumed_endpoint_does_not_expose_an_untouched_endpoint() {
    let mut e = Editor::new();
    let a = line(&mut e, [0., 0.], [100., 0.]);
    let b = line(&mut e, [2., 0.], [101., 0.]);
    let c = line(&mut e, [4., 0.], [102., 0.]);
    e.try_execute(EditCommand::SelectPaths(vec![a, b, c])).unwrap();
    draw(&mut e, Action::Join { points: vec![[99., -1.], [107., 1.]], options: Options::default() });
    // Each path had only one touched endpoint: one pair can join, the third stays open.
    assert_eq!(e.doc.paths.len(), 2);
    assert!(e.doc.paths.iter().all(|p| !p.closed));
    assert_eq!(e.doc.paths.iter().map(|p| p.anchors.len()).sum::<usize>(), 5);
}

#[test]
fn numeric_sheet_rolls_back_rectangle_and_ellipse_at_every_zoom() {
    for tool in [ToolKind::Rect, ToolKind::Ellipse] {
        for zoom in [0.1, 1., 10.] {
            let mut e = Editor::new();
            e.ppu = zoom;
            e.set_tool(tool);
            let before = e.doc.clone();
            e.pointer_down([200., 200.]);
            e.pointer_move([200. + 1.5 / zoom, 200. + 1.5 / zoom]);
            assert!(!e.doc.paths.is_empty());
            e.pointer_up();
            let spec = e.drawing.dialog.unwrap();
            assert_eq!(e.doc, before);
            assert!(!e.transaction_open());
            assert!(!e.history_available(false));
            drawing::finish(&mut e, true);
            assert_eq!(e.doc, before);
            draw(&mut e, Action::Shape { spec });
            assert_eq!(e.doc.paths.len(), 1);
            assert_eq!(e.rev, 1);
            e.undo();
            assert_eq!(e.doc, before);
        }
    }
}

#[test]
fn undo_and_redo_cancel_freehand_and_curvature_but_keep_preferences() {
    for tool in [ToolKind::Pencil, ToolKind::Curvature] {
        for redo in [false, true] {
            let mut e = Editor::new();
            line(&mut e, [0., 0.], [100., 0.]);
            if redo {
                e.undo();
            }
            let prefs = Options { fidelity: 3., smoothness: 0.7, ..Default::default() };
            e.drawing.options = prefs;
            e.drawing.shape.sides = 7;
            e.set_tool(tool);
            e.pointer_down([200., 200.]);
            e.pointer_move([220., 230.]);
            if tool == ToolKind::Curvature {
                e.pointer_down([240., 200.]);
            }
            if redo {
                e.redo();
            } else {
                e.undo();
            }
            let restored = e.doc.clone();
            let rev = e.rev;
            assert!(e.drawing.start.is_none());
            assert!(e.drawing.samples.is_empty());
            assert!(e.drawing.curvature.is_empty());
            assert!(e.drawing.preview.is_empty());
            assert_eq!(e.drawing.options, prefs);
            assert_eq!(e.drawing.shape.sides, 7);
            assert!(!e.transaction_open());
            e.pointer_move([260., 260.]);
            e.pointer_up();
            drawing::finish(&mut e, false);
            assert_eq!(e.doc, restored);
            assert_eq!(e.rev, rev);
        }
    }
}

#[test]
fn eraser_refuses_default_path_cap_without_document_or_history_changes() {
    let mut e = Editor::new();
    let id = line(&mut e, [0., 0.], [100., 0.]);
    e.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
    let source = e.doc.paths[0].clone();
    // Check runs before tree adoption/history; cap padding need not be adopted to reproduce the count.
    for _ in 1..varos_core::format::Limits::DEFAULT.max_paths {
        let mut p = source.clone();
        p.id = e.doc.nid();
        for a in &mut p.anchors {
            a.id = e.doc.nid();
        }
        e.doc.paths.push(p);
    }
    let before = e.doc.clone();
    let rev = e.rev;
    let result = e.try_execute(EditCommand::Drawing(Action::PathErase {
        points: vec![[50., -10.], [50., 10.]],
        options: Options { brush_radius: 5., ..Default::default() },
    }));
    assert!(result.unwrap_err().contains("limit"));
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
    assert!(!e.transaction_open());
}

/// Integration w2 (Lane D × gradients): a drawing tool shape inherits a current gradient fill.
#[test]
fn drawing_tools_inherit_current_gradient_paint() {
    use varos_core::{editor::PaintTarget, model::Paint};
    let mut ed = Editor::new();
    ed.set_current_paint(PaintTarget::Fill, Paint::Gradient(varos_core::gradient::Gradient::default()));
    let before = ed.doc.paths.len();
    ed.try_execute(EditCommand::Drawing(varos_core::drawing::Action::Curvature {
        points: vec![[0., 0.], [50., 0.], [50., 50.], [0., 50.]],
        closed: true,
    }))
    .unwrap();
    assert_eq!(ed.doc.paths.len(), before + 1);
    assert!(matches!(ed.doc.paths.last().unwrap().fill, Paint::Gradient(_)));
}
