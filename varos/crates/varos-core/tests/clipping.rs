use varos_core::{
    model::{GroupRole, Paint, ShapeKind},
    EditCommand, Editor,
};
fn scene() -> (Editor, Vec<u32>) {
    let mut ed = Editor::new();
    let mut ids = vec![];
    for x in [0., 10.] {
        ids.push(
            ed.try_execute_created(EditCommand::AddShape {
                kind: ShapeKind::Rect,
                bounds: [x, x, 40., 40.],
                parent: None,
                fill: Some([1.; 4]),
                stroke: Some([0., 0., 0., 1.]),
                stroke_width: 2.,
                opacity: 1.,
                name: None,
            })
            .unwrap(),
        );
    }
    ed.try_execute(EditCommand::SelectPaths(ids.clone())).unwrap();
    (ed, ids)
}
#[test]
fn make_release_round_trip_discards_top_paint_and_each_is_one_step() {
    let (mut ed, ids) = scene();
    let original = ed.doc.clone();
    let rev = ed.rev;
    assert!(ed.clip_make_enabled());
    assert!(!ed.clip_release_enabled());
    ed.try_execute(EditCommand::ClipMake).unwrap();
    assert_eq!(ed.rev, rev + 1);
    let g = ed.doc.nodes.iter().find(|n| n.role == GroupRole::Clip).unwrap();
    let gid = g.id;
    assert_eq!(ed.doc.node_paths(g.mask_child.unwrap()), vec![ids[1]]);
    let mask = &ed.doc.paths[ed.doc.pidx(ids[1]).unwrap()];
    assert_eq!(mask.fill, Paint::None);
    assert_eq!(mask.stroke, Paint::None);
    assert!(ed.clip_release_enabled());
    assert!(!ed.clip_make_enabled());
    let clipped = ed.doc.clone();
    ed.try_execute(EditCommand::ClipRelease).unwrap();
    assert_eq!(ed.rev, rev + 2);
    assert_eq!(ed.doc.node(gid).unwrap().role, GroupRole::Normal);
    assert!(!ed.clip_release_enabled());
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, clipped);
    ed.execute(EditCommand::Undo).unwrap();
    assert_eq!(ed.doc, original);
    ed.execute(EditCommand::Redo).unwrap();
    assert_eq!(ed.doc, clipped);
}
#[test]
fn enable_rules_refuse_empty_single_locked_and_partial_selections() {
    let (mut ed, ids) = scene();
    ed.escape();
    assert!(!ed.clip_make_enabled());
    assert!(!ed.clip_release_enabled());
    ed.try_execute(EditCommand::SelectPaths(vec![ids[0]])).unwrap();
    assert!(!ed.clip_make_enabled());
    ed.try_execute(EditCommand::SelectPaths(ids.clone())).unwrap();
    ed.doc.paths[0].locked = true;
    assert!(!ed.clip_make_enabled());
    ed.doc.paths[0].locked = false;
    ed.try_execute(EditCommand::ClipMake).unwrap();
    ed.escape();
    assert!(!ed.clip_release_enabled());
    let group = ed.doc.top_group_of_path(ids[0]).unwrap();
    ed.layer_select_set(&[group]);
    assert!(ed.clip_release_enabled());
}
