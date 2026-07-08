//! P10 — undo/redo must KEEP the selection (pruned to what still exists), not wipe it. Pure logic.

use varos_core::editor::Editor;
use varos_core::model::{Anchor, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn sq(id: u32, ids: [u32; 4], x: f32, y: f32, s: f32) -> Path {
    Path::new(
        id,
        vec![anc(ids[0], x, y), anc(ids[1], x + s, y), anc(ids[2], x + s, y + s), anc(ids[3], x, y + s)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

#[test]
fn undo_keeps_a_still_present_selection() {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(sq(1, [1, 2, 3, 4], 0.0, 0.0, 20.0));
    ed.doc.ids = 4;
    ed.objsel.insert(1);

    // an undoable edit that does NOT remove path 1 (recolour it)
    ed.begin();
    ed.doc.paths[0].opacity = 0.5;
    ed.dirty = true; // real edits set this; commit only records history when dirty
    ed.commit();

    ed.undo();
    assert!(ed.objsel.contains(&1), "undo must keep path 1 selected (it still exists in the restored doc)");
}

#[test]
fn undo_prunes_selection_of_a_path_that_no_longer_exists() {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(sq(1, [1, 2, 3, 4], 0.0, 0.0, 20.0));
    ed.doc.ids = 4;
    ed.objsel.insert(1);

    // snapshot taken here has ONLY path 1; then we add path 2 and select it too
    ed.begin();
    ed.doc.paths.push(sq(2, [5, 6, 7, 8], 40.0, 0.0, 20.0));
    ed.doc.ids = 8;
    ed.objsel.insert(2);
    ed.dirty = true; // real edits set this; commit only records history when dirty
    ed.commit();

    ed.undo(); // restores the doc WITHOUT path 2
    assert!(ed.objsel.contains(&1), "path 1 stays selected");
    assert!(!ed.objsel.contains(&2), "path 2 vanished on undo → its selection is pruned, never left dangling");
}
