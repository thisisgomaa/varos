use varos_core::editor::{Editor, PaintTarget};
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
