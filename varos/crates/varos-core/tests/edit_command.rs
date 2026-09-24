use varos_core::editor::{Editor, PaintTarget, ToolKind};
use varos_core::model::{Anchor, Paint, Path};
use varos_core::EditCommand;

fn anchor(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}

fn selected_square() -> Editor {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    ed.doc.paths.push(Path::new(
        1,
        vec![anchor(1, 0.0, 0.0), anchor(2, 20.0, 0.0), anchor(3, 20.0, 20.0), anchor(4, 0.0, 20.0)],
        true,
        Some([1.0, 0.0, 0.0, 1.0]),
        Some([0.0, 0.0, 0.0, 1.0]),
        3.0,
    ));
    ed.doc.ids = 4;
    ed.doc.sync_tree();
    ed.objsel.insert(1);
    ed
}

#[test]
fn stroke_width_command_owns_history_and_undo_redo() {
    let mut ed = selected_square();

    ed.execute(EditCommand::SetStrokeWidth(-4.0));
    assert_eq!(ed.doc.paths[0].stroke_width, 0.0);
    assert_eq!(ed.rev, 1);

    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths[0].stroke_width, 3.0);
    assert_eq!(ed.rev, 2);

    ed.execute(EditCommand::Redo);
    assert_eq!(ed.doc.paths[0].stroke_width, 0.0);
    assert_eq!(ed.rev, 3);
}

#[test]
fn nonundoable_document_settings_keep_their_current_revision_policy() {
    let mut ed = selected_square();
    ed.doc.snap.enabled = false;

    ed.execute(EditCommand::SetRulerOrigin([13.0, 17.0]));
    ed.execute(EditCommand::ToggleSnapping);
    assert!(ed.doc.snap.enabled);
    let mut config = ed.doc.snap;
    config.smart = false;
    ed.execute(EditCommand::SetSnapConfig(config));

    assert_eq!(ed.doc.ruler_origin, [13.0, 17.0]);
    assert_eq!(ed.origin_preview, Some([13.0, 17.0]));
    assert_eq!(ed.doc.snap, config);
    assert_eq!(ed.rev, 0, "these serialized mode changes are intentionally non-undoable today");
}

#[test]
fn paint_command_sets_the_target_and_commits_the_selected_path() {
    let mut ed = selected_square();

    ed.execute(EditCommand::ApplyPaint { target: PaintTarget::Stroke, color: None });

    assert!(ed.paint == PaintTarget::Stroke);
    assert_eq!(ed.doc.paths[0].stroke, Paint::None);
    assert_eq!(ed.rev, 1);
}

fn pen_click(ed: &mut Editor, p: [f32; 2]) {
    ed.pointer_down(p);
    ed.pointer_up();
}

#[test]
fn stroke_width_on_a_selection_carries_to_the_next_pen_path() {
    // Astra 09-24: after giving a stroke width 80, the next Pen stroke came out at 2. Setting the weight
    // on a SELECTED path must also make it the current weight (Illustrator: last-used appearance).
    let mut ed = selected_square();
    assert_eq!(ed.cur_sw, 2.0);
    ed.execute(EditCommand::SetStrokeWidth(80.0));
    assert_eq!(ed.doc.paths[0].stroke_width, 80.0, "the selection gets the weight");
    assert_eq!(ed.cur_sw, 80.0, "…and it becomes the current weight");

    ed.escape();
    ed.set_tool(ToolKind::Pen);
    pen_click(&mut ed, [100.0, 100.0]);
    pen_click(&mut ed, [200.0, 100.0]);
    assert_eq!(ed.doc.paths.last().unwrap().stroke_width, 80.0, "the next Pen path draws at 80, not 2");
}

#[test]
fn stroke_width_while_drawing_lands_on_the_pen_path() {
    // While the Pen is mid-path the inspector shows the in-progress path (its anchor is selected), so the
    // weight field must change THAT path — it used to read `objsel` only and leave it at 2.
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Pen);
    pen_click(&mut ed, [0.0, 0.0]);
    pen_click(&mut ed, [100.0, 0.0]);
    assert!(ed.objsel.is_empty() && ed.active.is_some(), "mid-draw: no object selection, an active path");
    let rev0 = ed.rev;

    ed.execute(EditCommand::SetStrokeWidth(80.0));

    let pi = ed.repr_path().expect("the inspector shows the in-progress path");
    assert_eq!(ed.doc.paths[pi].stroke_width, 80.0, "the in-progress Pen path takes the weight");
    assert_eq!(ed.cur_sw, 80.0);
    assert_eq!(ed.rev, rev0 + 1, "one committed, undoable edit");
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths[pi].stroke_width, 2.0);
}

// ── QW3 / Astra F10: a `<Path>` row in the Layers panel shows `Path::name`, so renaming a path must write
// `Path::name` — the leaf node's own name is never displayed (`RenameNode` on a path leaf changed nothing
// visible and still dirtied the document). ──

/// One open path (id 1) with the auto-name (`name: None`, shown as `<Path>`).
fn one_open_path() -> Editor {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.doc.paths.push(Path::new(
        1,
        vec![anchor(2, 0.0, 0.0), anchor(3, 30.0, 10.0)],
        false,
        None,
        Some([0.0, 0.0, 0.0, 1.0]),
        2.0,
    ));
    ed.doc.ids = 10;
    ed.doc.sync_tree();
    ed
}

#[test]
fn rename_path_writes_the_name_the_layers_row_reads() {
    let mut ed = one_open_path();
    let leaf = ed.doc.node_of_path(1).expect("the path has a leaf node");
    let leaf_name = ed.doc.node(leaf).unwrap().name.clone();
    ed.execute(EditCommand::RenamePath { path: 1, name: "  Logo  ".into() });
    assert_eq!(ed.doc.paths[0].name.as_deref(), Some("Logo"), "trimmed, stored on the path");
    assert_eq!(ed.doc.node(leaf).unwrap().name, leaf_name, "the leaf node's name is not the displayed one");
}

#[test]
fn rename_path_is_one_undo_step() {
    let mut ed = one_open_path();
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    assert_eq!(ed.rev, 1, "one edit");
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths[0].name, None, "undo restores the auto-name");
    ed.execute(EditCommand::Redo);
    assert_eq!(ed.doc.paths[0].name.as_deref(), Some("Logo"));
}

#[test]
fn rename_path_empty_name_keeps_the_old_one() {
    let mut ed = one_open_path();
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    let rev = ed.rev;
    // blank, or made only of invisible direction/format marks (RLM, ALM, LRM, RLO, ZWSP, BOM, isolates)
    for empty in
        ["", "   ", "\t", "\u{200F}", "\u{061C} \u{200E}", "\u{202E}\u{202A}", "\u{200B}\u{FEFF}", "\u{2067}\u{2069}"]
    {
        ed.execute(EditCommand::RenamePath { path: 1, name: empty.into() });
        assert_eq!(ed.doc.paths[0].name.as_deref(), Some("Logo"), "{empty:?} must keep the old name");
    }
    assert_eq!(ed.rev, rev, "an emptied field is not an edit");
}

#[test]
fn rename_path_trims_invisible_marks_at_the_edges_only() {
    let mut ed = one_open_path();
    ed.execute(EditCommand::RenamePath { path: 1, name: "\u{200F} شعار \u{200F}".into() });
    assert_eq!(ed.doc.paths[0].name.as_deref(), Some("شعار"));
    ed.execute(EditCommand::RenamePath { path: 1, name: "شعار\u{200F}2".into() });
    assert_eq!(ed.doc.paths[0].name.as_deref(), Some("شعار\u{200F}2"), "a mark inside the name is kept");
}

#[test]
fn rename_path_no_op_does_not_dirty() {
    let mut ed = one_open_path();
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    let rev = ed.rev;
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    ed.execute(EditCommand::RenamePath { path: 1, name: " Logo\u{200E}".into() });
    ed.execute(EditCommand::RenamePath { path: 99, name: "Ghost".into() }); // no such path
    assert_eq!(ed.rev, rev, "an unchanged name (or a missing path) leaves the document clean");
    // …and it pushed no history: one Undo goes straight back to the auto-name
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths[0].name, None);
}

// ── QW3 review P2-1 (PAINS_LOG FB6 nit): a Pen draft deselects other art (Illustrator), so a selection
// left over from before the Pen can no longer be the hidden target of the inspector's fields. ──

#[test]
fn pen_new_draft_deselects_leftover_objects() {
    let mut ed = selected_square();
    let square: Vec<_> = ed.doc.paths[0].anchors.iter().map(|a| a.p).collect();
    ed.set_tool(ToolKind::Pen);
    pen_click(&mut ed, [100.0, 100.0]);
    pen_click(&mut ed, [160.0, 100.0]);
    assert!(ed.active.is_some(), "mid-draft");
    assert!(ed.objsel.is_empty() && ed.dsel_path.is_none(), "the old selection is dropped when the draft starts");
    assert_eq!(ed.obj_angle, 0.0);
    // the inspector's X field acts on the draft, never on the old square
    ed.execute(EditCommand::SetObjectBounds {
        x: Some(500.0),
        y: None,
        width: None,
        height: None,
        anchor_x: 0.0,
        anchor_y: 0.0,
    });
    let after: Vec<_> = ed.doc.paths[0].anchors.iter().map(|a| a.p).collect();
    assert_eq!(after, square, "a mid-draft X edit moved the old object");
    let draft = ed.doc.paths.iter().find(|p| Some(p.id) == ed.active).unwrap();
    assert_eq!(draft.anchors[0].p[0], 500.0, "…it moved the draft");
}

#[test]
fn pen_resume_deselects_leftover_objects() {
    let mut ed = selected_square();
    ed.doc.paths.push(Path::new(
        9,
        vec![anchor(10, 100.0, 0.0), anchor(11, 150.0, 0.0)],
        false,
        None,
        Some([0.0, 0.0, 0.0, 1.0]),
        2.0,
    ));
    ed.doc.ids = 11;
    ed.doc.sync_tree();
    ed.set_tool(ToolKind::Pen);
    ed.pointer_move([150.0, 0.0]); // hovering reveals path 9's anchors…
    pen_click(&mut ed, [150.0, 0.0]); // …and a click on its open end resumes it
    assert_eq!(ed.active, Some(9), "resumed");
    assert!(ed.objsel.is_empty(), "resuming a path drops the leftover square selection");
}
