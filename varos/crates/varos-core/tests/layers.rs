//! Layers Stage A — the scene TREE (D2), headless. The invariant under test: structure lives in the
//! node tree, the flat path list re-flattens to traversal order, and the canvas behaves EXACTLY as
//! before with one implicit "Layer 1".

use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, Document, DropPos, GroupRole, Node, NodeKind, PaintRole, Path, Xform};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn tri(id: u32, base: u32, x: f32) -> Path {
    Path::new(
        id,
        vec![anc(base, x, 0.0), anc(base + 1, x + 20.0, 0.0), anc(base + 2, x, 20.0)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

#[test]
fn a_fresh_document_has_layer_1() {
    let d = Document::default();
    assert_eq!(d.roots.len(), 1);
    let root = d.node(d.roots[0]).unwrap();
    assert!(matches!(root.kind, NodeKind::Layer));
    assert_eq!(root.name, "Layer 1");
    assert_eq!(d.active_layer, d.roots[0]);
}

#[test]
fn new_paths_are_adopted_into_the_active_layer_in_z_order() {
    let mut d = Document::default();
    d.paths.push(tri(10, 1, 0.0));
    d.paths.push(tri(11, 4, 40.0)); // pushed later = front-most (the old flat rule)
    d.ids = 20;
    d.sync_tree();
    let layer = d.node(d.active_layer).unwrap();
    assert_eq!(layer.children.len(), 2, "both paths adopted");
    // children are FRONT-first: child 0 must be the leaf of path 11
    let front = d.node(layer.children[0]).unwrap();
    assert!(matches!(front.kind, NodeKind::Path(11)), "front child = the later push");
    // and the flat storage still reads back→front: [10, 11]
    let order: Vec<u32> = d.paths.iter().map(|p| p.id).collect();
    assert_eq!(order, vec![10, 11], "flatten preserves the old z exactly");
}

#[test]
fn legacy_registry_files_migrate_with_z_preserved() {
    // a pre-tree document: raw paths + the old groups/group_of registry, NO nodes
    let mut d = Document::default();
    d.nodes.clear();
    d.roots.clear();
    d.active_layer = 0; // simulate a legacy file exactly
    d.paths.push(tri(1, 1, 0.0));
    d.paths.push(tri(2, 4, 30.0));
    d.paths.push(tri(3, 7, 60.0));
    d.groups.push(varos_core::model::Group { id: 50, name: "Old G".into(), parent: None });
    d.group_of.insert(2, 50);
    d.group_of.insert(3, 50);
    d.ids = 50;
    d.sync_tree();
    assert!(d.groups.is_empty() && d.group_of.is_empty(), "the registry is consumed");
    let order: Vec<u32> = d.paths.iter().map(|p| p.id).collect();
    assert_eq!(order, vec![1, 2, 3], "migration must not reorder the artwork");
    let mut m = d.group_members(2);
    m.sort();
    assert_eq!(m, vec![2, 3], "the old group survives as a tree Group");
    assert_eq!(d.node(d.top_group_of_path(2).unwrap()).unwrap().name, "Old G", "its name survives");
}

#[test]
fn hiding_or_locking_a_container_cascades() {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    ed.doc.paths.push(tri(1, 1, 0.0));
    ed.doc.paths.push(tri(2, 4, 40.0));
    ed.doc.ids = 10;
    ed.doc.sync_tree();
    ed.objsel.insert(1);
    ed.objsel.insert(2);
    ed.group_selection();
    let gid = ed.doc.top_group_of_path(1).unwrap();
    // hide the GROUP node → both paths effectively hidden (and unclickable)
    if let Some(i) = ed.doc.nodes.iter().position(|n| n.id == gid) {
        ed.doc.nodes[i].hidden = true;
    }
    assert!(ed.doc.eff_hidden(1) && ed.doc.eff_hidden(2), "container eye cascades to children");
    assert!(ed.path_under([5.0, 5.0]).is_none(), "hidden-by-cascade is not clickable");
    if let Some(i) = ed.doc.nodes.iter().position(|n| n.id == gid) {
        ed.doc.nodes[i].hidden = false;
        ed.doc.nodes[i].locked = true;
    }
    assert!(ed.doc.eff_locked(1), "container lock cascades");
    assert!(ed.path_under([5.0, 5.0]).is_none(), "locked-by-cascade is not clickable");
}

#[test]
fn arrange_moves_units_within_their_own_parent() {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    ed.doc.paths.push(tri(1, 1, 0.0));
    ed.doc.paths.push(tri(2, 4, 40.0));
    ed.doc.paths.push(tri(3, 7, 80.0)); // front-most
    ed.doc.ids = 10;
    ed.doc.sync_tree();
    ed.set_tool(ToolKind::Object);
    ed.objsel.insert(1); // the back-most
    ed.arrange(varos_core::editor::ZOrder::Front);
    let order: Vec<u32> = ed.doc.paths.iter().map(|p| p.id).collect();
    assert_eq!(order, vec![2, 3, 1], "Bring to Front puts 1 above its siblings");
    ed.arrange(varos_core::editor::ZOrder::Backward);
    let order: Vec<u32> = ed.doc.paths.iter().map(|p| p.id).collect();
    assert_eq!(order, vec![2, 1, 3], "Send Backward steps down exactly one sibling");
}

#[test]
fn drag_drop_reorders_nests_and_refuses_cycles() {
    let mut d = Document::default();
    d.paths.push(tri(1, 1, 0.0));
    d.paths.push(tri(2, 4, 40.0));
    d.paths.push(tri(3, 7, 80.0)); // front-most
    d.ids = 10;
    d.sync_tree();
    let layer = d.active_layer;
    let leaf = |d: &Document, pid: u32| d.node_of_path(pid).unwrap();

    // reorder: drop path 1 (back) BEFORE path 3 (rows are front-first, so Before = above = more front)
    // → 1 becomes front-most; flat z back→front = [2,3,1]
    let (n1, n3) = (leaf(&d, 1), leaf(&d, 3));
    assert!(d.move_node_to(n1, n3, DropPos::Before));
    assert_eq!(d.paths.iter().map(|p| p.id).collect::<Vec<_>>(), vec![2, 3, 1], "reordered by drag");

    // nest: make a Group, then drag path 2's leaf INTO it
    d.group(&[1, 3]).unwrap();
    let g = d.top_group_of_path(1).unwrap();
    let n2 = leaf(&d, 2);
    assert!(d.move_node_to(n2, g, DropPos::Into), "drop a leaf INTO a group nests it");
    assert_eq!(d.node(n2).unwrap().parent, Some(g), "the leaf is now a child of the group");

    // cycle guard: a group can't be dropped into its own descendant
    assert!(!d.move_node_to(g, n2, DropPos::Into), "dropping a container into its own child is refused");
    // into-a-leaf is refused
    assert!(!d.move_node_to(leaf(&d, 1), n2, DropPos::Into), "can't nest INTO a Path leaf");
    // a Layer can't be dropped into a Group
    assert!(!d.move_node_to(layer, g, DropPos::Into), "a Layer can't nest inside a Group");
}

#[test]
fn selection_square_moves_and_copies_art_across_layers() {
    // B4: drag a row's select-square → move the canvas selection onto another layer; Alt = duplicate.
    let mut d = Document::default();
    d.paths.push(tri(1, 1, 0.0));
    d.paths.push(tri(2, 4, 40.0));
    d.paths.push(tri(3, 7, 80.0)); // all on Layer 1
    d.ids = 10;
    d.sync_tree();
    let l1 = d.active_layer;
    // add Layer 2 on top
    let l2 = {
        let id = d.ids + 1;
        d.ids = id;
        d.nodes.push(Node {
            id,
            kind: NodeKind::Layer,
            name: "Layer 2".into(),
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
        d.roots.insert(0, id);
        id
    };

    // move paths 1 & 3 INTO Layer 2 (keep relative z: 1 behind 3)
    let landed = d.move_paths_to(&[1, 3], l2, DropPos::Into, false);
    assert_eq!(landed, vec![1, 3], "the same two paths moved (no copy)");
    assert_eq!(d.node_paths(l2), vec![1, 3], "node_paths is back→front: 1 stays behind 3 in Layer 2");
    assert_eq!(d.node_paths(l1), vec![2], "only path 2 is left on Layer 1");
    assert_eq!(
        d.paths.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![2, 1, 3],
        "flat z: L1 art below L2 art, 1 behind 3"
    );

    // Alt-copy path 2 into Layer 2 → a NEW path id, original stays on Layer 1
    let before = d.paths.len();
    let landed = d.move_paths_to(&[2], l2, DropPos::Into, true);
    assert_eq!(landed.len(), 1);
    assert!(landed[0] != 2 && d.pidx(2).is_some(), "copy makes a fresh id and leaves the original");
    assert_eq!(d.paths.len(), before + 1, "exactly one new path");
    assert_eq!(d.node_paths(l1), vec![2], "the original is still on Layer 1");
    assert!(d.node_paths(l2).contains(&landed[0]), "the copy landed on Layer 2");

    // Into a Path leaf is refused (only containers accept Into)
    let leaf2 = d.node_of_path(2).unwrap();
    assert!(d.move_paths_to(&[landed[0]], leaf2, DropPos::Into, false).is_empty(), "can't drop art INTO a leaf");
}

#[test]
fn alt_drag_on_canvas_leaves_a_moved_copy() {
    // Ahmed's bug report: "Alt+click+drag doesn't copy anything." Prove the editor path at the model level.
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    ed.doc.paths.push(tri(1, 1, 0.0)); // a triangle around (0,0)-(20,20)
    ed.doc.ids = 3;
    ed.doc.sync_tree();
    ed.set_tool(ToolKind::Object);
    ed.objsel.insert(1);
    ed.mods.alt = true;
    let before = ed.doc.paths.len();
    ed.pointer_down([5.0, 5.0]); // Alt on the object → DupPending
    ed.pointer_move([60.0, 60.0]); // past DRAG_THRESH → duplicate + drag the COPY
    ed.pointer_up();
    assert_eq!(ed.doc.paths.len(), before + 1, "Alt-drag must leave a copy behind");
    // the original stays put; the copy is the one that moved and is now selected
    let orig = ed.doc.paths.iter().find(|p| p.id == 1).unwrap();
    assert_eq!(orig.anchors[0].p, [0.0, 0.0], "the ORIGINAL never moves under Alt-drag");
    assert_eq!(ed.objsel.len(), 1, "the copy is the new selection");
    assert!(!ed.objsel.contains(&1), "selection moved off the original onto the copy");
}

#[test]
fn a_locked_layer_is_truly_immovable() {
    // Ahmed's "lock is fake" bug: after locking, the object still dragged. Lock must block hit-test,
    // marquee AND the transform-frame grab (drop it from the selection at gesture start).
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    ed.doc.paths.push(tri(1, 1, 0.0));
    ed.doc.ids = 3;
    ed.doc.sync_tree();
    ed.set_tool(ToolKind::Object);
    ed.objsel.insert(1); // selected first…
    let layer = ed.doc.active_layer;
    ed.layer_toggle_locked(layer); // …then the layer is locked
    assert!(!ed.objsel.contains(&1), "locking drops the object from the selection");
    assert!(ed.path_under([5.0, 5.0]).is_none(), "a locked object is not clickable");
    // even if it lingered in the selection, a drag can't move it
    ed.objsel.insert(1);
    let before = ed.doc.paths[0].anchors[0].p;
    ed.pointer_down([5.0, 5.0]);
    ed.pointer_move([50.0, 50.0]);
    ed.pointer_up();
    assert_eq!(ed.doc.paths[0].anchors[0].p, before, "a locked object never moves");
}

#[test]
fn paint_list_is_exactly_doc_paths_until_masks_land() {
    // LAYERS_VISION §5: the indirection lands as a byte-for-byte no-op — same paths, same order,
    // same Vec indices — so scene build / export / snap migrate onto it with ZERO behaviour change.
    // (The mask-era test — "a MaskSource is excluded from paint but present for hit-test" — arrives
    // with clip groups; this golden is what it will diff against.)
    let mut d = Document::default();
    d.paths.push(tri(1, 1, 0.0));
    d.paths.push(tri(2, 4, 40.0));
    d.paths.push(tri(3, 7, 80.0));
    d.ids = 10;
    d.sync_tree();
    let via: Vec<(usize, u32)> = d.paint_list().map(|(pi, p)| (pi, p.id)).collect();
    let raw: Vec<(usize, u32)> = d.paths.iter().enumerate().map(|(pi, p)| (pi, p.id)).collect();
    assert_eq!(via, raw, "paint_list = doc.paths, index-preserving, while no mask exists");
    assert!(d.paths.iter().all(|p| d.paint_role(p.id) == PaintRole::Normal), "all roles Normal before masks");
}

#[test]
fn ctrl_click_toggles_a_row_even_with_locked_members() {
    // 07-04 review bug #1: `all_in` counted locked/hidden paths (which can never be in objsel),
    // so Ctrl+click could only ever ADD on a mixed-lock row — deselect was unreachable.
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.ppu = 1.0;
    ed.doc.paths.push(tri(1, 1, 0.0));
    ed.doc.paths.push(tri(2, 4, 40.0));
    ed.doc.paths.push(tri(3, 7, 80.0));
    ed.doc.ids = 12;
    ed.doc.sync_tree();
    let layer = ed.doc.active_layer;
    ed.set_locked(3, true); // one locked member → a mixed row

    ed.layer_toggle(layer); // Ctrl+click: select what's selectable
    assert_eq!(ed.objsel.len(), 2, "only the unlocked art is selected");
    assert!(ed.objsel.contains(&1) && ed.objsel.contains(&2) && !ed.objsel.contains(&3));
    ed.layer_toggle(layer); // Ctrl+click again: must DESELECT
    assert!(ed.objsel.is_empty(), "a mixed-lock row toggles OFF once its selectable art is all in");

    // and a fully-locked row toggles nothing at all
    ed.set_locked(1, true);
    ed.set_locked(2, true);
    ed.layer_toggle(layer);
    assert!(ed.objsel.is_empty(), "a fully-locked row can't grab the selection");
}

#[test]
fn a_second_layer_receives_new_drawings() {
    let mut d = Document::default();
    d.paths.push(tri(1, 1, 0.0));
    d.ids = 10;
    d.sync_tree();
    // add "Layer 2" on top and make it active (what the panel's + button will do)
    let l2 = {
        let id = d.ids + 1;
        d.ids = id;
        d.nodes.push(Node {
            id,
            kind: NodeKind::Layer,
            name: "Layer 2".into(),
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
        d.roots.insert(0, id); // above Layer 1
        id
    };
    d.active_layer = l2;
    d.paths.push(tri(2, 4, 40.0));
    d.sync_tree();
    assert_eq!(d.node(l2).unwrap().children.len(), 1, "the new drawing landed on the ACTIVE layer");
    let order: Vec<u32> = d.paths.iter().map(|p| p.id).collect();
    assert_eq!(order, vec![1, 2], "Layer 2 sits above Layer 1 → its content is front-most");
}
