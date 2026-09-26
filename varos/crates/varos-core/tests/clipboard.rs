//! Edit ▸ Cut / Copy / Paste / Paste in Place (Astra F04) — headless. Everything goes through the
//! `EditCommand` boundary exactly as the app's ⌘C / ⌘X / ⌘V / ⇧⌘V keys do. Pure data, no GPU.
//!
//! Run with:  cargo test -p varos-core --test clipboard

use std::collections::HashSet;

use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, GroupRole, Node, NodeKind, Path, Xform};
use varos_core::EditCommand;

/// A filled axis-aligned square (anchor ids base..base+3).
fn sq(id: u32, base: u32, x: f32, y: f32, s: f32) -> Path {
    let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
    Path::new(
        id,
        vec![a(base, [x, y]), a(base + 1, [x + s, y]), a(base + 2, [x + s, y + s]), a(base + 3, [x, y + s])],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

/// Editor with squares 10 (0,0 · 40) and 11 (100,0 · 20) on Layer 1, no artboards, nothing selected.
fn two_squares() -> Editor {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.doc.paths.push(sq(10, 100, 0.0, 0.0, 40.0));
    ed.doc.paths.push(sq(11, 110, 100.0, 0.0, 20.0));
    ed.doc.ids = 200;
    ed.doc.sync_tree();
    ed
}

fn select(ed: &mut Editor, pids: &[u32]) {
    ed.objsel = pids.iter().copied().collect();
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

fn first_anchor(ed: &Editor, pid: u32) -> [f32; 2] {
    ed.doc.paths.iter().find(|p| p.id == pid).unwrap().anchors[0].p
}

fn add_layer(ed: &mut Editor, name: &str) -> u32 {
    let id = ed.doc.nid();
    ed.doc.nodes.push(Node {
        id,
        kind: NodeKind::Layer,
        name: name.into(),
        parent: None,
        children: vec![],
        hidden: false,
        locked: false,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
        role: GroupRole::Normal,
        mask_child: None,
    });
    ed.doc.roots.insert(0, id);
    id
}

#[test]
fn copy_leaves_the_document_and_rev_untouched() {
    let mut ed = two_squares();
    select(&mut ed, &[10]);
    let before = ed.doc.clone();
    let rev = ed.rev;
    ed.execute(EditCommand::Copy);
    assert_eq!(ed.rev, rev, "copy is not an edit — no rev bump (no unsaved-changes star)");
    assert_eq!(ed.doc, before, "copy never writes the document");
    assert!(!ed.dirty);
    assert_eq!(ed.clipboard().len(), 1);
    let (x0, y0, x1, y1) = ed.clipboard().bounds().unwrap();
    assert!(near([x0, y0], [0.0, 0.0]) && near([x1, y1], [40.0, 40.0]), "bounds = the copied square");
    // undo has nothing to undo: copy left no history entry
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.rev, rev);
}

#[test]
fn copy_with_nothing_selected_keeps_the_previous_clipboard() {
    let mut ed = two_squares();
    select(&mut ed, &[11]);
    ed.execute(EditCommand::Copy);
    ed.objsel.clear();
    ed.execute(EditCommand::Copy);
    assert_eq!(ed.clipboard().len(), 1, "an empty Copy does not wipe the clipboard (Illustrator)");
}

#[test]
fn cut_removes_the_selection_and_one_undo_restores_it() {
    let mut ed = two_squares();
    select(&mut ed, &[10]);
    let before = ed.doc.clone();
    ed.execute(EditCommand::Cut);
    assert!(ed.doc.pidx(10).is_none(), "the cut path is gone");
    assert!(ed.doc.pidx(11).is_some(), "the unselected path stays");
    assert!(ed.objsel.is_empty());
    assert_eq!(ed.rev, 1, "cut is ONE history step");
    assert_eq!(ed.clipboard().len(), 1, "and the cut art is on the clipboard");
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc, before, "one undo restores the document exactly");
    // the clipboard outlives the undo — paste still works
    ed.execute(EditCommand::Paste { offset: None });
    assert_eq!(ed.doc.paths.len(), 3);
}

#[test]
fn cut_then_paste_in_place_puts_it_back_where_it_was() {
    let mut ed = two_squares();
    select(&mut ed, &[10]);
    ed.execute(EditCommand::Cut);
    ed.execute(EditCommand::Paste { offset: None });
    assert_eq!(ed.doc.paths.len(), 2);
    let pasted = *ed.objsel.iter().next().unwrap();
    assert_ne!(pasted, 10, "a paste always mints a new id");
    assert_eq!(first_anchor(&ed, pasted), [0.0, 0.0]);
}

#[test]
fn paste_with_offset_lands_there_and_selects_the_copy() {
    let mut ed = two_squares();
    select(&mut ed, &[10, 11]);
    ed.execute(EditCommand::Copy);
    // the app's view-centre maths: target centre − clipboard centre
    let c = ed.clipboard().center().unwrap();
    assert!(near(c, [60.0, 20.0]), "centre of the two squares' union, got {c:?}");
    let target = [500.0, 300.0];
    let rev = ed.rev;
    ed.execute(EditCommand::Paste { offset: Some([target[0] - c[0], target[1] - c[1]]) });
    assert_eq!(ed.rev, rev + 1, "paste is ONE history step");
    assert_eq!(ed.doc.paths.len(), 4);
    assert_eq!(ed.objsel.len(), 2, "exactly the pasted copies are selected");
    assert!(!ed.objsel.contains(&10) && !ed.objsel.contains(&11));
    let (x0, y0, x1, y1) = ed.obj_bbox().unwrap();
    assert!(near([(x0 + x1) * 0.5, (y0 + y1) * 0.5], target), "the pasted art is centred on the target");
    // the originals did not move
    assert_eq!(first_anchor(&ed, 10), [0.0, 0.0]);
    assert_eq!(first_anchor(&ed, 11), [100.0, 0.0]);
    // one undo removes the whole paste
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths.len(), 2);
    assert!(ed.objsel.is_empty(), "the pruned selection doesn't dangle at the undone copies");
}

#[test]
fn paste_in_place_keeps_the_coordinates() {
    let mut ed = two_squares();
    select(&mut ed, &[11]);
    ed.execute(EditCommand::Copy);
    ed.execute(EditCommand::Paste { offset: None });
    let pasted = *ed.objsel.iter().next().unwrap();
    let orig = ed.doc.paths.iter().find(|p| p.id == 11).unwrap();
    let copy = ed.doc.paths.iter().find(|p| p.id == pasted).unwrap();
    let pts = |p: &Path| p.anchors.iter().map(|a| a.p).collect::<Vec<_>>();
    assert_eq!(pts(orig), pts(copy), "Paste in Place: same geometry, same place");
    assert_eq!(orig.fill, copy.fill);
    // it lands in FRONT of everything on the layer
    assert_eq!(ed.doc.paths.last().unwrap().id, pasted);
}

#[test]
fn pasting_twice_gives_two_independent_copies() {
    let mut ed = two_squares();
    select(&mut ed, &[10]);
    ed.execute(EditCommand::Copy);
    ed.execute(EditCommand::Paste { offset: Some([10.0, 0.0]) });
    let a = *ed.objsel.iter().next().unwrap();
    ed.execute(EditCommand::Paste { offset: Some([20.0, 0.0]) });
    let b = *ed.objsel.iter().next().unwrap();
    assert_ne!(a, b);
    assert_eq!(ed.doc.paths.len(), 4);
    // every path and anchor id in the document is unique
    let mut pids = HashSet::new();
    let mut aids = HashSet::new();
    for p in &ed.doc.paths {
        assert!(pids.insert(p.id), "duplicate path id {}", p.id);
        for an in p.anchors.iter().chain(p.holes.iter().flatten()) {
            assert!(aids.insert(an.id), "duplicate anchor id {}", an.id);
        }
    }
    // editing one copy leaves the other alone
    let ai = ed.doc.pidx(a).unwrap();
    ed.doc.paths[ai].anchors[0].p = [-99.0, -99.0];
    assert_eq!(first_anchor(&ed, b), [20.0, 0.0]);
    assert_eq!(first_anchor(&ed, 10), [0.0, 0.0]);
}

#[test]
fn group_and_clip_mask_structure_survive_copy_paste() {
    let mut ed = two_squares();
    ed.doc.paths.push(sq(12, 120, 5.0, 5.0, 10.0)); // mask for the clip
    ed.doc.sync_tree();
    let clip = ed.doc.clip_group(&[10, 12], 12).expect("clip group");
    let outer = ed.doc.group(&[10, 11, 12]).expect("outer group"); // nest: outer { clip{10,12}, 11 }
    ed.doc.sync_tree();
    select(&mut ed, &[10, 11, 12]);
    ed.execute(EditCommand::Copy);
    ed.execute(EditCommand::Paste { offset: Some([0.0, 200.0]) });
    let new: Vec<u32> = ed.objsel.iter().copied().collect();
    assert_eq!(new.len(), 3);

    // one new top-level group, directly on the layer, not the original
    let tops: HashSet<u32> = new.iter().filter_map(|&p| ed.doc.top_group_of_path(p)).collect();
    assert_eq!(tops.len(), 1, "all pasted paths share ONE top group");
    let new_outer = *tops.iter().next().unwrap();
    assert_ne!(new_outer, outer);
    assert_eq!(ed.doc.node(new_outer).unwrap().parent, Some(ed.doc.active_layer));
    // inside it: a Clip group with its OWN mask, plus the loose square
    let kids = ed.doc.node(new_outer).unwrap().children.clone();
    assert_eq!(kids.len(), 2);
    let new_clip = *kids.iter().find(|&&k| ed.doc.node(k).unwrap().role == GroupRole::Clip).expect("clip kept");
    assert_ne!(new_clip, clip);
    let mc = ed.doc.node(new_clip).unwrap().mask_child.expect("the pasted clip has a mask");
    assert!(ed.doc.node(new_clip).unwrap().children.contains(&mc));
    let mask_pid = ed.doc.node_paths(mc)[0];
    assert!(new.contains(&mask_pid), "the pasted clip is masked by the pasted mask, not the original");
    assert_eq!(first_anchor(&ed, mask_pid), [5.0, 205.0]);
    // the original structure is untouched
    assert_eq!(ed.doc.node(clip).unwrap().role, GroupRole::Clip);
    assert_eq!(ed.doc.top_group_of_path(10), Some(outer));
}

#[test]
fn a_live_rotation_travels_with_the_paste() {
    let mut ed = two_squares();
    let leaf = ed.doc.node_of_path(10).unwrap();
    ed.doc.set_node_xform(leaf, Xform { rot: 0.5, piv: [20.0, 20.0] });
    select(&mut ed, &[10]);
    ed.execute(EditCommand::Copy);
    let before = ed.clipboard().bounds().unwrap();
    ed.execute(EditCommand::Paste { offset: Some([100.0, 50.0]) });
    let pid = *ed.objsel.iter().next().unwrap();
    let xf = ed.doc.unit_xform(pid);
    assert!((xf.rot - 0.5).abs() < 1e-6, "the copy keeps its live angle");
    assert_eq!(xf.piv, [120.0, 70.0], "the pivot moves with the geometry");
    let after = ed.obj_bbox().unwrap();
    assert!((after.0 - (before.0 + 100.0)).abs() < 1e-3 && (after.1 - (before.1 + 50.0)).abs() < 1e-3);
}

#[test]
fn empty_clipboard_paste_is_a_no_op() {
    let mut ed = two_squares();
    select(&mut ed, &[10]);
    let before = ed.doc.clone();
    ed.execute(EditCommand::Paste { offset: Some([5.0, 5.0]) });
    ed.execute(EditCommand::Paste { offset: None });
    assert_eq!(ed.rev, 0, "nothing to paste → no history, no rev bump");
    assert_eq!(ed.doc, before);
    assert!(ed.objsel.contains(&10), "the selection is left alone");
}

#[test]
fn empty_selection_cut_is_a_no_op() {
    let mut ed = two_squares();
    ed.execute(EditCommand::Cut);
    assert_eq!(ed.rev, 0);
    assert!(ed.clipboard().is_empty());
}

#[test]
fn paste_goes_to_the_active_layer() {
    let mut ed = two_squares();
    let l1 = ed.doc.active_layer;
    select(&mut ed, &[10]);
    ed.execute(EditCommand::Copy);
    let l2 = add_layer(&mut ed, "Layer 2");
    ed.doc.active_layer = l2;
    ed.execute(EditCommand::Paste { offset: None });
    let pid = *ed.objsel.iter().next().unwrap();
    let leaf = ed.doc.node_of_path(pid).unwrap();
    assert_eq!(ed.doc.layer_ancestor(leaf), l2, "the paste lands on the ACTIVE layer");
    assert_eq!(ed.doc.layer_ancestor(ed.doc.node_of_path(10).unwrap()), l1, "the original stays put");
    assert_eq!(ed.doc.paths.last().unwrap().id, pid, "Layer 2 is on top → the copy is front-most");
}

#[test]
fn multi_item_paste_keeps_relative_z_order() {
    let mut ed = two_squares(); // 10 behind 11
    select(&mut ed, &[10, 11]);
    ed.execute(EditCommand::Copy);
    ed.execute(EditCommand::Paste { offset: Some([0.0, 100.0]) });
    let order: Vec<u32> = ed.doc.paths.iter().map(|p| p.id).collect();
    assert_eq!(&order[..2], &[10, 11], "originals stay behind");
    // the copy of 10 (at y=100) is behind the copy of 11
    assert_eq!(first_anchor(&ed, order[2]), [0.0, 100.0]);
    assert_eq!(first_anchor(&ed, order[3]), [100.0, 100.0]);
}

#[test]
fn paste_from_another_tool_shows_the_selection_on_the_selection_tool() {
    let mut ed = two_squares();
    select(&mut ed, &[10]);
    ed.execute(EditCommand::Copy);
    ed.set_tool(ToolKind::Pen);
    ed.execute(EditCommand::Paste { offset: None });
    assert!(ed.tool == ToolKind::Object, "pasting from the Pen tool hands over to Selection");
    assert_eq!(ed.objsel.len(), 1);
}

#[test]
fn the_clipboard_survives_opening_another_document() {
    let mut ed = two_squares();
    select(&mut ed, &[11]);
    ed.execute(EditCommand::Copy);
    ed.replace_doc(Default::default());
    ed.execute(EditCommand::Paste { offset: None });
    assert_eq!(ed.doc.paths.len(), 1);
    assert_eq!(ed.doc.paths[0].anchors[0].p, [100.0, 0.0]);
}

// ---------- P18 (Codex review of the Astra batch, 2026-09-26): Cut must never delete hidden/locked art ----
// Direct-select a path, hide or lock it (itself, or a parent group / layer), then ⌘X: the stale Direct
// selection used to survive the hide/lock and Cut deleted the invisible / protected art. The invariant is
// "nothing hidden or locked is ever selected"; Cut/Copy also filter defensively.

fn path_exists(ed: &Editor, pid: u32) -> bool {
    ed.doc.pidx(pid).is_some()
}

/// Square 10 Direct-selected at path level (dsel) AND one of its anchors grabbed, squares grouped.
fn direct_selected() -> (Editor, u32) {
    let mut ed = two_squares();
    let gid = ed.doc.group(&[10, 11]).expect("the squares group");
    ed.set_tool(ToolKind::Direct);
    ed.dsel_path = Some(10);
    ed.selected.insert(101);
    (ed, gid)
}

#[test]
fn hiding_or_locking_drops_the_direct_selection_so_cut_cannot_delete_it() {
    type Op = fn(&mut Editor, u32);
    let ops: [(&str, Op); 8] = [
        ("hide path (panel eye)", |ed, _| {
            let leaf = ed.doc.node_of_path(10).unwrap();
            ed.execute(EditCommand::ToggleNodeHidden(leaf));
        }),
        ("lock path (panel padlock)", |ed, _| {
            let leaf = ed.doc.node_of_path(10).unwrap();
            ed.execute(EditCommand::ToggleNodeLocked(leaf));
        }),
        ("hide path (set_hidden)", |ed, _| ed.set_hidden(10, true)),
        ("lock path (set_locked)", |ed, _| ed.set_locked(10, true)),
        ("hide parent group", |ed, gid| ed.execute(EditCommand::ToggleNodeHidden(gid))),
        ("lock parent group", |ed, gid| ed.execute(EditCommand::ToggleNodeLocked(gid))),
        ("hide parent layer", |ed, _| {
            let layer = ed.doc.layer_ancestor(ed.doc.node_of_path(10).unwrap());
            ed.execute(EditCommand::ToggleNodeHidden(layer));
        }),
        ("lock parent layer", |ed, _| {
            let layer = ed.doc.layer_ancestor(ed.doc.node_of_path(10).unwrap());
            ed.execute(EditCommand::ToggleNodeLocked(layer));
        }),
    ];
    for (what, op) in ops {
        let (mut ed, gid) = direct_selected();
        op(&mut ed, gid);
        assert!(ed.doc.eff_hidden(10) || ed.doc.eff_locked(10), "{what}: fixture — the path is now inert");
        assert_eq!(ed.dsel_path, None, "{what}: the Direct path selection is dropped");
        assert!(!ed.selected.contains(&101), "{what}: its grabbed anchor is dropped too");
        ed.execute(EditCommand::Cut);
        assert!(path_exists(&ed, 10), "{what}: ⌘X must not delete hidden/locked art");
    }
}

#[test]
fn hiding_a_board_drops_its_art_from_the_direct_selection() {
    let mut ed = two_squares();
    ed.doc.artboards = vec![varos_core::model::Artboard { x: 0.0, y: 0.0, w: 60.0, h: 60.0, ..Default::default() }];
    ed.set_tool(ToolKind::Direct);
    ed.dsel_path = Some(10);
    ed.execute(EditCommand::ToggleArtboardHidden(0));
    assert_eq!(ed.dsel_path, None, "board-hidden art leaves the Direct selection");
    ed.execute(EditCommand::ToggleArtboardHidden(0));
    ed.dsel_path = Some(10);
    ed.execute(EditCommand::ToggleArtboardLocked(0));
    assert_eq!(ed.dsel_path, None, "board-locked art leaves the Direct selection");
    ed.execute(EditCommand::Cut);
    assert!(path_exists(&ed, 10));
}

#[test]
fn cut_and_copy_skip_hidden_or_locked_paths_even_if_still_selected() {
    // Defence in depth: even if some future path leaves a hidden/locked path in a selection set, Cut and
    // Copy refuse it — they only take what is visible and editable.
    for lock in [false, true] {
        let mut ed = two_squares();
        {
            let i = ed.doc.pidx(10).unwrap();
            if lock {
                ed.doc.paths[i].locked = true;
            } else {
                ed.doc.paths[i].hidden = true;
            }
        }
        ed.dsel_path = Some(10); // forced past the ops, straight into the state
        ed.objsel.insert(10);
        let rev = ed.rev;
        ed.execute(EditCommand::Copy);
        assert!(ed.clipboard().is_empty(), "lock={lock}: nothing copyable");
        ed.execute(EditCommand::Cut);
        assert!(path_exists(&ed, 10), "lock={lock}: Cut refuses the inert path");
        assert_eq!(ed.rev, rev, "lock={lock}: no edit, no undo step");
    }
}

#[test]
fn redo_of_hide_prunes_a_selection_restored_after_undo() {
    let mut ed = two_squares();
    let leaf = ed.doc.node_of_path(10).unwrap();
    ed.execute(EditCommand::ToggleNodeHidden(leaf));
    ed.execute(EditCommand::Undo);
    ed.objsel.insert(10);
    ed.dsel_path = Some(10);
    ed.selected.insert(100);
    ed.execute(EditCommand::Redo);
    assert!(ed.doc.eff_hidden(10));
    assert!(!ed.objsel.contains(&10) && ed.dsel_path.is_none() && !ed.selected.contains(&100));
}

#[test]
fn moving_selected_art_onto_a_locked_board_prunes_it() {
    let mut ed = two_squares();
    ed.doc.artboards = vec![
        varos_core::model::Artboard { x: 0.0, y: 0.0, w: 60.0, h: 60.0, ..Default::default() },
        varos_core::model::Artboard { x: 200.0, y: 0.0, w: 60.0, h: 60.0, locked: true, ..Default::default() },
    ];
    ed.objsel.insert(10);
    let leaf = ed.doc.node_of_path(10).unwrap();
    ed.execute(EditCommand::MoveLayerToBoard { sources: vec![leaf], source_board: Some(0), target_board: 1 });
    assert!(ed.doc.eff_locked(10), "the moved path now belongs to the locked board");
    assert!(!ed.objsel.contains(&10), "central command pruning drops it");
}

#[test]
fn delete_defensively_refuses_hidden_or_locked_paths_and_anchors() {
    for locked in [false, true] {
        let mut ed = two_squares();
        let i = ed.doc.pidx(10).unwrap();
        ed.doc.paths[i].hidden = !locked;
        ed.doc.paths[i].locked = locked;
        ed.objsel.insert(10);
        ed.selected.insert(100);
        let rev = ed.rev;
        ed.execute(EditCommand::DeleteSelected);
        assert!(path_exists(&ed, 10), "locked={locked}: inert art survives Delete");
        assert_eq!(ed.doc.anchor(100).unwrap().p, [0.0, 0.0]);
        assert_eq!(ed.rev, rev, "locked={locked}: no deletion means no history step");
    }
}

#[test]
fn cut_of_a_group_carries_its_hidden_member_and_flag() {
    let mut ed = two_squares();
    let group = ed.doc.group(&[10, 11]).unwrap();
    ed.layer_select_set(&[group]);
    let hidden_leaf = ed.doc.node_of_path(11).unwrap();
    ed.execute(EditCommand::ToggleNodeHidden(hidden_leaf));
    ed.execute(EditCommand::Cut);
    assert!(!path_exists(&ed, 10) && !path_exists(&ed, 11), "the whole selected group is cut");
    ed.execute(EditCommand::Paste { offset: None });
    let hidden_copy = ed
        .doc
        .paths
        .iter()
        .map(|path| path.id)
        .find(|pid| ![10, 11].contains(pid) && ed.doc.eff_hidden(*pid))
        .expect("hidden member pasted");
    assert!(ed.doc.node(ed.doc.node_of_path(hidden_copy).unwrap()).unwrap().hidden, "node eye flag survives");
}

#[test]
fn copy_of_a_clip_group_carries_its_hidden_mask_and_stays_clip() {
    let mut ed = two_squares();
    let clip = ed.doc.clip_group(&[10, 11], 11).unwrap();
    ed.layer_select_set(&[clip]);
    let mask_leaf = ed.doc.node_of_path(11).unwrap();
    ed.execute(EditCommand::ToggleNodeHidden(mask_leaf));
    ed.execute(EditCommand::Copy);
    ed.execute(EditCommand::Paste { offset: Some([0.0, 100.0]) });
    let copied_clip = ed.objsel.iter().find_map(|&pid| ed.doc.clip_group_of(pid)).expect("copy stays clipped");
    let copied_mask = ed.doc.node(copied_clip).unwrap().mask_child.unwrap();
    assert!(ed.doc.node(copied_mask).unwrap().hidden, "copied mask keeps its node eye flag");
}

#[test]
fn copy_of_a_direct_group_member_does_not_carry_a_hidden_sibling() {
    let mut ed = two_squares();
    ed.doc.group(&[10, 11]).unwrap();
    let hidden_leaf = ed.doc.node_of_path(11).unwrap();
    ed.execute(EditCommand::ToggleNodeHidden(hidden_leaf));
    let direct_leaf = ed.doc.node_of_path(10).unwrap();
    ed.layer_select_set(&[direct_leaf]);
    ed.execute(EditCommand::Copy);
    assert_eq!(ed.clipboard().len(), 1, "a directly selected leaf is not a whole-group structural selection");
}

#[test]
fn delete_of_a_group_keeps_its_locked_member() {
    let mut ed = two_squares();
    let group = ed.doc.group(&[10, 11]).unwrap();
    ed.layer_select_set(&[group]);
    let locked_leaf = ed.doc.node_of_path(11).unwrap();
    ed.execute(EditCommand::ToggleNodeLocked(locked_leaf));
    ed.execute(EditCommand::DeleteSelected);
    assert!(!path_exists(&ed, 10), "unlocked group member is deleted");
    assert!(path_exists(&ed, 11), "locked group member wins and remains");
}
