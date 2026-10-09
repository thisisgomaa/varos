use varos_core::{
    geom::Pt,
    model::ShapeKind,
    planar::{self, PathfinderOp},
    EditCommand as C, Editor, ToolKind,
};
fn rect(ed: &mut Editor, x: f32) -> u32 {
    ed.try_execute_created(C::AddShape {
        kind: ShapeKind::Rect,
        bounds: [x, 0., 20., 20.],
        parent: None,
        fill: Some([1., 0., 0., 1.]),
        stroke: None,
        stroke_width: 0.,
        opacity: 0.7,
        name: Some("source".into()),
    })
    .unwrap()
}
fn selected() -> Editor {
    let mut ed = Editor::new();
    let a = rect(&mut ed, 0.);
    let b = rect(&mut ed, 10.);
    ed.execute(C::SelectPaths(vec![a, b]));
    ed
}
fn shape(p: &varos_core::model::Path) -> Vec<Vec<[f64; 2]>> {
    std::iter::once(&p.anchors)
        .chain(&p.holes)
        .map(|r| r.iter().map(|a| [a.p[0] as f64, a.p[1] as f64]).collect())
        .collect()
}
fn area(ed: &Editor) -> f64 {
    ed.doc.paths.iter().map(|p| planar::area(&shape(p))).sum()
}
#[test]
fn six_pathfinder_commands_undo_and_keep_appearance() {
    for op in [
        PathfinderOp::Divide,
        PathfinderOp::Trim,
        PathfinderOp::Merge,
        PathfinderOp::Crop,
        PathfinderOp::Outline,
        PathfinderOp::MinusBack,
    ] {
        let mut ed = selected();
        let before = ed.doc.clone();
        let rev = ed.rev;
        ed.try_execute(C::Pathfinder(op)).unwrap();
        assert_eq!(ed.rev, rev + 1);
        assert!(ed.doc.paths.iter().all(|p| p.opacity == 0.7 && p.name.as_deref() == Some("source")));
        if op == PathfinderOp::Outline {
            assert!(ed.doc.paths.iter().all(|p| !p.closed && p.fill == varos_core::model::Paint::None));
        } else {
            let want = if matches!(op, PathfinderOp::Crop | PathfinderOp::MinusBack) { 200. } else { 600. };
            assert!((area(&ed) - want).abs() < 0.001, "{op:?}");
        }
        ed.execute(C::Undo);
        assert!(ed.doc.content_eq(&before));
        ed.execute(C::Redo);
        assert_eq!(ed.rev, rev + 3);
    }
}
#[test]
fn shape_builder_merge_delete_and_fast_gesture() {
    for delete in [false, true] {
        let mut ed = selected();
        let before = ed.doc.clone();
        ed.try_execute(C::ShapeBuilder { points: vec![[-5., 10.], [35., 10.]], delete }).unwrap();
        if delete {
            assert!(ed.doc.paths.is_empty());
        } else {
            assert_eq!(ed.doc.paths.len(), 1);
            assert!((area(&ed) - 600.).abs() < 0.001);
        }
        ed.execute(C::Undo);
        assert!(ed.doc.content_eq(&before));
    }
    let mut ed = selected();
    ed.set_tool(ToolKind::ShapeBuilder);
    ed.pointer_down([5., 10.]);
    ed.pointer_move([25., 10.]);
    let highlight = ed.construction_highlight();
    assert_eq!(highlight.len(), 3);
    ed.pointer_up();
    assert_eq!(ed.doc.paths.len(), 1);
    ed.execute(C::Undo);
    assert_eq!(ed.doc.paths.len(), 2);
}
#[test]
fn scissors_circle_preserves_cubics_and_open_path_splits() {
    let mut ed = Editor::new();
    let pid = ed
        .try_execute_created(C::AddShape {
            kind: ShapeKind::Ellipse,
            bounds: [0., 0., 20., 20.],
            parent: None,
            fill: None,
            stroke: Some([0., 0., 0., 1.]),
            stroke_width: 1.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    let before = ed.doc.clone();
    ed.try_execute(C::Scissors { path: pid, segment: 0, t: 0.5 }).unwrap();
    let p = &ed.doc.paths[0];
    assert!(!p.closed);
    assert_eq!(p.anchors.len(), 6);
    assert_eq!(p.anchors[0].p, p.anchors[5].p);
    assert!(p.anchors.iter().any(|a| a.hout.is_some()));
    ed.execute(C::Undo);
    assert!(ed.doc.content_eq(&before));
    ed.execute(C::Scissors { path: pid, segment: 0, t: 0.5 });
    ed.execute(C::Scissors { path: pid, segment: 2, t: 0.5 });
    assert_eq!(ed.doc.paths.len(), 2);
}
#[test]
fn knife_freehand_partitions_fill_and_is_one_undo_step() {
    let mut ed = Editor::new();
    let pid = rect(&mut ed, 0.);
    ed.execute(C::SelectPaths(vec![pid]));
    let before = ed.doc.clone();
    let rev = ed.rev;
    ed.set_tool(ToolKind::Knife);
    ed.pointer_down([-2., 10.]);
    ed.pointer_move([10., 10.]);
    ed.pointer_move([22., 10.]);
    ed.pointer_up();
    assert_eq!(ed.doc.paths.len(), 2);
    assert!((area(&ed) - 400.).abs() < 0.001);
    assert_eq!(ed.rev, rev + 1);
    ed.execute(C::Undo);
    assert!(ed.doc.content_eq(&before));
}
#[test]
fn eraser_brush_subtracts_and_preserves_holes_undo() {
    let mut ed = Editor::new();
    let pid = rect(&mut ed, 0.);
    ed.execute(C::SelectPaths(vec![pid]));
    let before = ed.doc.clone();
    ed.try_execute(C::Eraser { points: vec![[10., -2.], [10., 22.]], radius: 2. }).unwrap();
    assert_eq!(ed.doc.paths.len(), 2);
    assert!((area(&ed) - 320.).abs() < 0.01);
    ed.execute(C::Undo);
    assert!(ed.doc.content_eq(&before));
    ed.execute(C::SelectPaths(vec![pid]));
    ed.execute(C::Eraser { points: vec![[10., 10.]], radius: 2. });
    assert_eq!(ed.doc.paths[0].holes.len(), 1);
}
#[test]
fn divide_objects_below_keeps_lower_paint_consumes_cutter_and_skips_locked() {
    let mut ed = selected();
    let cutter = ed.doc.paths.last().unwrap().id;
    let before = ed.doc.clone();
    ed.execute(C::SelectPaths(vec![cutter]));
    ed.try_execute(C::DivideObjectsBelow).unwrap();
    assert_eq!(ed.doc.paths.len(), 2);
    assert!((area(&ed) - 400.).abs() < 0.001);
    ed.execute(C::Undo);
    assert!(ed.doc.content_eq(&before));
}
#[test]
fn checked_tools_reject_invalid_geometry_without_mutation() {
    let mut ed = selected();
    let before = ed.doc.clone();
    let pid = ed.doc.paths[0].id;
    for c in [
        C::Scissors { path: pid, segment: 99, t: 0.5 },
        C::Scissors { path: pid, segment: 0, t: f32::NAN },
        C::Eraser { points: vec![[1., 1.]], radius: 0. },
        C::Knife { points: vec![[f32::INFINITY, 1.]] },
        C::ShapeBuilder { points: Vec::<Pt>::new(), delete: false },
    ] {
        assert!(ed.try_execute(c).is_err());
        assert!(ed.doc.content_eq(&before));
    }
}

#[test]
fn open_edges_split_faces_and_builder_keeps_the_cutting_path() {
    let mut ed = Editor::new();
    let square = rect(&mut ed, 0.);
    let anchors = vec![
        varos_core::model::Anchor { id: 0, p: [-5., 10.], hin: None, hout: None, smooth: false },
        varos_core::model::Anchor { id: 0, p: [25., 10.], hin: None, hout: None, smooth: false },
    ];
    let line = ed
        .try_execute_created(C::AddPath {
            anchors,
            closed: false,
            parent: None,
            fill: None,
            stroke: Some([0., 0., 0., 1.]),
            stroke_width: 1.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    ed.execute(C::SelectPaths(vec![square, line]));
    let fs = ed.construction_faces();
    assert_eq!(fs.len(), 2);
    assert!((fs.iter().map(|f| planar::area(&f.shape)).sum::<f64>() - 400.).abs() < 0.001);
    ed.execute(C::ShapeBuilder { points: vec![[10., 5.]], delete: true });
    assert_eq!(ed.doc.paths.len(), 2);
    assert!(ed.doc.pidx(line).is_some());
    assert!(
        (ed.doc.paths.iter().filter(|p| p.closed).map(|p| planar::area(&shape(p))).sum::<f64>() - 200.).abs() < 0.001
    );
}
#[test]
fn builder_hover_changes_scene_signature_and_overlay_without_gpu() {
    let mut ed = selected();
    ed.set_tool(ToolKind::ShapeBuilder);
    let view = varos_core::geom::View::identity();
    ed.pointer_move([5., 5.]);
    let first = varos_core::scene::scene_signature(&ed, view, [400, 400]);
    let scene = varos_core::build_scene(&ed, 1.0);
    assert!(scene.overlay.iter().any(|p| matches!(p, varos_core::scene::Prim::Fill { .. })));
    ed.pointer_move([100., 100.]);
    assert_ne!(first, varos_core::scene::scene_signature(&ed, view, [400, 400]));
}

#[test]
fn merge_discards_strokes_before_comparing_visible_paint() {
    let mut ed = selected();
    ed.doc.paths[0].stroke = varos_core::model::Paint::Solid([0., 0., 0., 1.]);
    ed.doc.paths[0].stroke_width = 5.;
    ed.execute(C::Pathfinder(PathfinderOp::Merge));
    assert_eq!(ed.doc.paths.len(), 1);
    assert_eq!(ed.doc.paths[0].stroke, varos_core::model::Paint::None);
    assert!((area(&ed) - 600.).abs() < 0.001);
}
#[test]
fn divide_below_leaves_locked_lower_objects_and_cutter_when_no_editable_hits() {
    let mut ed = selected();
    let cutter = ed.doc.paths[1].id;
    ed.doc.paths[0].locked = true;
    let before = ed.doc.clone();
    ed.execute(C::SelectPaths(vec![cutter]));
    let rev = ed.rev;
    ed.execute(C::DivideObjectsBelow);
    assert!(ed.doc.content_eq(&before));
    assert_eq!(ed.rev, rev);
}
#[test]
fn scissors_refuses_compound_cut_without_changing_hole_coverage_or_undo() {
    let mut ed = selected();
    let pid = ed.doc.paths[0].id;
    let hole = ed.doc.build_shape(ShapeKind::Rect, [2., 2.], [8., 8.]);
    ed.doc.paths[0].holes.push(hole);
    let before = ed.doc.clone();
    let rev = ed.rev;
    let contains = |ed: &Editor| planar::faces(&[shape(&ed.doc.paths[0])]).iter().any(|f| f.contains([5., 5.]));
    assert!(!contains(&ed));
    assert!(ed.try_execute(C::Scissors { path: pid, segment: 0, t: 0.5 }).is_err());
    ed.execute(C::Scissors { path: pid, segment: 0, t: 0.5 });
    assert!(!contains(&ed));
    assert!(ed.doc.content_eq(&before));
    assert_eq!(ed.rev, rev);
}

#[test]
fn alt_builder_drag_deletes_one_face_as_one_edit() {
    let mut ed = selected();
    let before = ed.doc.clone();
    let rev = ed.rev;
    ed.set_tool(ToolKind::ShapeBuilder);
    ed.mods.alt = true;
    ed.pointer_down([5., 5.]);
    ed.pointer_move([5., 15.]);
    ed.pointer_up();
    assert_eq!(ed.rev, rev + 1);
    assert!((area(&ed) - 400.).abs() < 0.001);
    ed.execute(C::Undo);
    assert!(ed.doc.content_eq(&before));
}

#[test]
fn scissors_canvas_hit_uses_refined_curve_parameter() {
    let mut ed = Editor::new();
    let pid = ed
        .try_execute_created(C::AddShape {
            kind: ShapeKind::Ellipse,
            bounds: [0., 0., 100., 100.],
            parent: None,
            fill: None,
            stroke: Some([0., 0., 0., 1.]),
            stroke_width: 1.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    let p = &ed.doc.paths[ed.doc.pidx(pid).unwrap()];
    let a = &p.anchors[0];
    let b = &p.anchors[1];
    let point = varos_core::geom::cubic(a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p, 0.371);
    ed.set_tool(ToolKind::Scissors);
    ed.pointer_down(point);
    ed.pointer_up();
    let p = &ed.doc.paths[ed.doc.pidx(pid).unwrap()];
    assert!(!p.closed);
    assert!(varos_core::geom::dist(p.anchors[0].p, point) < 0.001);
}

#[test]
fn builder_retains_distant_cubic_path_and_selection_verbatim() {
    let mut ed = selected();
    let ellipse = ed
        .try_execute_created(C::AddShape {
            kind: ShapeKind::Ellipse,
            bounds: [100., 100., 30., 30.],
            parent: None,
            fill: Some([0., 1., 0., 1.]),
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    let original = ed.doc.paths[ed.doc.pidx(ellipse).unwrap()].clone();
    let ids = ed.doc.paths.iter().map(|p| p.id).collect();
    ed.execute(C::SelectPaths(ids));
    ed.execute(C::ShapeBuilder { points: vec![[5., 5.]], delete: true });
    assert_eq!(ed.doc.paths[ed.doc.pidx(ellipse).unwrap()], original);
    assert!(ed.objsel.contains(&ellipse));
}
#[test]
fn builder_isolated_merge_is_noop_including_ids_revision_and_undo() {
    let mut ed = Editor::new();
    let pid = rect(&mut ed, 0.);
    ed.execute(C::SelectPaths(vec![pid]));
    let before = ed.doc.clone();
    let rev = ed.rev;
    ed.execute(C::ShapeBuilder { points: vec![[5., 5.]], delete: false });
    assert!(ed.doc.content_eq(&before));

    assert_eq!(ed.rev, rev);
    assert_eq!(ed.doc.ids, before.ids);
    ed.execute(C::Undo);
    assert!(ed.doc.paths.is_empty(), "undo must undo AddShape, with no intermediate no-op");
}
#[test]
fn construction_replacements_reject_mask_sources_atomically() {
    for command in [
        C::Pathfinder(PathfinderOp::Divide),
        C::ShapeBuilder { points: vec![[-5., 10.], [35., 10.]], delete: true },
        C::Knife { points: vec![[-5., 10.], [35., 10.]] },
        C::Eraser { points: vec![[5., 5.]], radius: 2. },
        C::DivideObjectsBelow,
    ] {
        let mut ed = selected();
        let ids: Vec<_> = ed.doc.paths.iter().map(|p| p.id).collect();
        let group = ed.doc.clip_group(&ids, ids[0]).unwrap();
        let mask = ed.doc.node_mask_child(group);
        let before = ed.doc.clone();
        let rev = ed.rev;
        ed.execute(command);
        assert!(ed.doc.content_eq(&before));
        assert_eq!(ed.doc.node_mask_child(group), mask);
        assert_eq!(ed.rev, rev);
    }
}
