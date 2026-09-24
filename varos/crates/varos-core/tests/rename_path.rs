//! QW3 / Astra F10: a `<Path>` row in the Layers panel shows `Path::name`, so renaming a path must write
//! `Path::name` — the leaf node's own name is never displayed (`RenameNode` on a path leaf changed nothing
//! visible and still dirtied the document).

use varos_core::editor::Editor;
use varos_core::model::{Anchor, Path};
use varos_core::EditCommand;

fn anchor(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}

/// One open path (id 1) with the auto-name (`name: None`, shown as `<Path>`).
fn one_path() -> Editor {
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
    let mut ed = one_path();
    let leaf = ed.doc.node_of_path(1).expect("the path has a leaf node");
    let leaf_name = ed.doc.node(leaf).unwrap().name.clone();
    ed.execute(EditCommand::RenamePath { path: 1, name: "  Logo  ".into() });
    assert_eq!(ed.doc.paths[0].name.as_deref(), Some("Logo"), "trimmed, stored on the path");
    assert_eq!(ed.doc.node(leaf).unwrap().name, leaf_name, "the leaf node's name is not the displayed one");
}

#[test]
fn rename_path_is_one_undo_step() {
    let mut ed = one_path();
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    assert_eq!(ed.rev, 1, "one edit");
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths[0].name, None, "undo restores the auto-name");
    ed.execute(EditCommand::Redo);
    assert_eq!(ed.doc.paths[0].name.as_deref(), Some("Logo"));
}

#[test]
fn empty_name_keeps_the_old_one() {
    let mut ed = one_path();
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    let rev = ed.rev;
    for empty in ["", "   ", "\t"] {
        ed.execute(EditCommand::RenamePath { path: 1, name: empty.into() });
        assert_eq!(ed.doc.paths[0].name.as_deref(), Some("Logo"), "{empty:?} must keep the old name");
    }
    assert_eq!(ed.rev, rev, "an emptied field is not an edit");
}

#[test]
fn no_op_rename_does_not_dirty() {
    let mut ed = one_path();
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    let rev = ed.rev;
    ed.execute(EditCommand::RenamePath { path: 1, name: "Logo".into() });
    ed.execute(EditCommand::RenamePath { path: 1, name: " Logo ".into() });
    ed.execute(EditCommand::RenamePath { path: 99, name: "Ghost".into() }); // no such path
    assert_eq!(ed.rev, rev, "an unchanged name (or a missing path) leaves the document clean");
    // …and it pushed no history: one Undo goes straight back to the auto-name
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths[0].name, None);
}
