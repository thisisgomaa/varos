use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, Path};

fn anchor(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}

fn selected_square() -> Editor {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.doc.snap.enabled = false;
    ed.ppu = 1.0;
    ed.doc.paths.push(Path::new(
        1,
        vec![anchor(1, 0.0, 0.0), anchor(2, 20.0, 0.0), anchor(3, 20.0, 20.0), anchor(4, 0.0, 20.0)],
        true,
        Some([1.0, 0.0, 0.0, 1.0]),
        None,
        1.0,
    ));
    ed.doc.ids = 4;
    ed.doc.sync_tree();
    ed.objsel.insert(1);
    ed.set_tool(ToolKind::Object);
    ed
}

#[test]
fn completed_drag_commits_once_then_undo_and_redo_restore_each_state() {
    let mut ed = selected_square();
    let original = ed.doc.clone();

    ed.pointer_down([5.0, 5.0]);
    ed.pointer_move([15.0, 5.0]);
    ed.pointer_up();

    let moved = ed.doc.clone();
    assert_eq!(moved.paths[0].anchors[0].p, [10.0, 0.0]);
    assert_eq!(ed.rev, 1, "pointer-up commits the whole drag as one revision");

    ed.undo();
    assert_eq!(ed.doc, original);
    assert_eq!(ed.rev, 2);

    ed.redo();
    assert_eq!(ed.doc, moved);
    assert_eq!(ed.rev, 3);
}

#[test]
fn undo_during_drag_preserves_the_known_stale_pending_snapshot_behavior() {
    let mut ed = selected_square();
    ed.begin();
    ed.doc.paths[0].opacity = 0.5;
    ed.dirty = true;
    ed.commit();
    let pre_drag = ed.doc.clone();

    ed.pointer_down([5.0, 5.0]);
    ed.pointer_move([15.0, 5.0]);
    assert_eq!(ed.doc.paths[0].anchors[0].p, [10.0, 0.0]);

    // Known defect, intentionally characterized rather than fixed in F3:
    // docs/audits/2026-07-11-CLAUDE-COUNTER-REVIEW.md section 4.6.
    ed.undo();
    assert_eq!(ed.doc.paths[0].opacity, 1.0, "mid-drag undo reaches behind the live gesture");
    assert_eq!(ed.doc.paths[0].anchors[0].p, [0.0, 0.0]);

    ed.pointer_up();
    let revision_after_release = ed.rev;
    ed.redo();
    assert_eq!(ed.rev, revision_after_release, "release cleared the redo entry created by mid-drag undo");

    ed.undo();
    assert_eq!(
        ed.doc, pre_drag,
        "the next undo restores the stale pre-drag snapshot, which is newer than the prior state"
    );
}
