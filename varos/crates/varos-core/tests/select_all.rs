//! QW5 — Edit ▸ Select All (⌘A). Selection state only, like Deselect (`escape`): it takes what a
//! canvas click / marquee may take (visible, unlocked; whole groups) and never edits the document.
//! Pure logic, headless.

use varos_core::command::EditCommand;
use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, Artboard, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
/// A filled square, path id `id`, anchors `base..base+4`.
fn sq(id: u32, base: u32, x: f32) -> Path {
    Path::new(
        id,
        vec![anc(base, x, 0.0), anc(base + 1, x + 20.0, 0.0), anc(base + 2, x + 20.0, 20.0), anc(base + 3, x, 20.0)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

/// Five squares on a free canvas: 1 plain · 2 hidden · 3 locked · 4+5 grouped.
fn canvas() -> Editor {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    for (i, id) in (1..=5u32).enumerate() {
        ed.doc.paths.push(sq(id, 10 * id, 40.0 * i as f32));
    }
    ed.doc.ids = 60;
    ed.doc.sync_tree();
    ed.objsel.extend([4, 5]);
    ed.group_selection();
    ed.objsel.clear();
    let pi = |ed: &Editor, id| ed.doc.pidx(id).unwrap();
    let (h, l) = (pi(&ed, 2), pi(&ed, 3));
    ed.doc.paths[h].hidden = true;
    ed.doc.paths[l].locked = true;
    ed
}

fn sorted(v: impl IntoIterator<Item = u32>) -> Vec<u32> {
    let mut v: Vec<u32> = v.into_iter().collect();
    v.sort_unstable();
    v
}

#[test]
fn select_all_takes_visible_unlocked_paths_only() {
    let mut ed = canvas();
    ed.selected.insert(10); // a stale anchor selection is dropped
    ed.select_all();
    assert_eq!(sorted(ed.objsel.iter().copied()), [1, 4, 5], "hidden 2 and locked 3 are not selectable");
    assert!(ed.selected.is_empty(), "Select All in the Selection tool is an object selection");
    assert!(ed.dsel_path.is_none());

    // a locked / hidden CONTAINER cascades exactly as it does for a click
    let gid = ed.doc.top_group_of_path(4).unwrap();
    let gi = ed.doc.nodes.iter().position(|n| n.id == gid).unwrap();
    ed.doc.nodes[gi].locked = true;
    ed.escape();
    ed.select_all();
    assert_eq!(sorted(ed.objsel.iter().copied()), [1], "a locked group is skipped whole");
    ed.doc.nodes[gi].locked = false;
    ed.doc.nodes[gi].hidden = true;
    ed.select_all();
    assert_eq!(sorted(ed.objsel.iter().copied()), [1], "a hidden group is skipped whole");
}

#[test]
fn select_all_in_direct_selects_anchors() {
    let mut ed = canvas();
    ed.set_tool(ToolKind::Direct);
    ed.objsel.insert(1);
    ed.select_all();
    let want: Vec<u32> = [1u32, 4, 5].iter().flat_map(|&id| (10 * id)..(10 * id + 4)).collect();
    assert_eq!(sorted(ed.selected.iter().copied()), want, "every anchor of the visible, unlocked paths");
    assert!(ed.objsel.is_empty(), "the Direct tool selects anchors, not objects");
}

#[test]
fn select_all_in_artboard_tool_selects_boards() {
    let mut ed = canvas();
    ed.doc.artboards =
        (0..3).map(|i| Artboard { x: 200.0 * i as f32, y: 0.0, w: 100.0, h: 100.0, ..Artboard::default() }).collect();
    ed.doc.active = 1;
    ed.set_tool(ToolKind::Artboard);
    ed.select_all();
    assert_eq!(ed.absel.iter().copied().collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(ed.doc.active, 1, "the primary page stays the primary");
    assert!(ed.objsel.is_empty() && ed.selected.is_empty(), "artwork stays unselected in the Artboard tool");
    assert!((0..3).all(|i| ed.ab_is_selected(i)));
}

#[test]
fn select_all_ends_pen_draft() {
    let mut ed = canvas();
    ed.set_tool(ToolKind::Pen);
    ed.active = Some(1); // path 1 stands in for the path being drawn
    ed.select_all();
    assert_eq!(ed.active, None, "the Pen draft ends");
    assert_eq!(sorted(ed.objsel.iter().copied()), [1, 4, 5]);
    assert!(ed.tool == ToolKind::Pen, "Select All never switches tools");
}

#[test]
fn select_all_is_not_an_edit() {
    let mut ed = canvas();
    // one real edit first, so an extra history entry would show on undo
    ed.objsel.insert(1);
    ed.execute(EditCommand::SetOpacity(0.5));
    let rev = ed.rev;
    let doc_before = serde_json::to_string(&ed.doc).unwrap();

    ed.select_all();
    assert_eq!(ed.rev, rev, "Select All does not bump the document revision");
    assert!(!ed.dirty, "and does not mark a gesture dirty");
    assert_eq!(serde_json::to_string(&ed.doc).unwrap(), doc_before, "the document is untouched");

    ed.escape(); // Deselect (⇧⌘A) is the same kind of change
    assert_eq!(ed.rev, rev);

    ed.execute(EditCommand::Undo);
    let p1 = ed.doc.pidx(1).unwrap();
    assert_eq!(ed.doc.paths[p1].opacity, 1.0, "one ⌘Z undoes the real edit — Select All left no history step");
}
