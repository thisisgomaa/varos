//! Headless tests for the grouping model (hierarchy on top of the flat path list).
//! Pure data — no GPU, no UI. Verifies group/ungroup, whole-group membership, z-order
//! contiguity, and the post-delete reconciliation done by `sync_groups`.
//!
//! Run with:  cargo test -p varos-core --test groups

use varos_core::editor::{Editor, Mods, ToolKind};
use varos_core::model::{Document, Path, ShapeKind};

/// Add a rectangle path to the doc; return its id.
fn add_rect(doc: &mut Document, x0: f32, y0: f32, x1: f32, y1: f32) -> u32 {
    let anchors = doc.build_shape(ShapeKind::Rect, [x0, y0], [x1, y1]);
    let id = doc.nid();
    doc.paths.push(Path::new(id, anchors, true, Some([0.0, 0.0, 0.0, 1.0]), None, 1.0));
    id
}

fn idx_of(doc: &Document, pid: u32) -> usize {
    doc.paths.iter().position(|p| p.id == pid).unwrap()
}

fn sorted(mut v: Vec<u32>) -> Vec<u32> {
    v.sort();
    v
}

#[test]
fn group_then_members_and_top() {
    let mut doc = Document::default();
    let a = add_rect(&mut doc, 0.0, 0.0, 10.0, 10.0);
    let b = add_rect(&mut doc, 20.0, 0.0, 30.0, 10.0);
    let c = add_rect(&mut doc, 40.0, 0.0, 50.0, 10.0);

    let g = doc.group(&[a, b]).expect("group should be created");
    assert_eq!(doc.top_group_of_path(a), Some(g));
    assert_eq!(doc.top_group_of_path(b), Some(g));
    assert_eq!(doc.top_group_of_path(c), None, "c was not grouped");

    assert_eq!(sorted(doc.group_members(a)), sorted(vec![a, b]));
    assert_eq!(sorted(doc.group_members(b)), sorted(vec![a, b]));
    assert_eq!(doc.group_members(c), vec![c], "ungrouped path is its own member set");
}

#[test]
fn group_needs_two() {
    let mut doc = Document::default();
    let a = add_rect(&mut doc, 0.0, 0.0, 10.0, 10.0);
    assert!(doc.group(&[a]).is_none(), "a single path can't form a group");
    assert_eq!(doc.top_group_of_path(a), None);
}

#[test]
fn group_makes_members_contiguous() {
    let mut doc = Document::default();
    let a = add_rect(&mut doc, 0.0, 0.0, 1.0, 1.0);
    let _b = add_rect(&mut doc, 2.0, 0.0, 3.0, 1.0);
    let c = add_rect(&mut doc, 4.0, 0.0, 5.0, 1.0);
    let _d = add_rect(&mut doc, 6.0, 0.0, 7.0, 1.0);
    // group two non-adjacent paths (a at idx0, c at idx2)
    doc.group(&[a, c]).unwrap();
    let (ia, ic) = (idx_of(&doc, a), idx_of(&doc, c));
    assert_eq!((ia as i32 - ic as i32).abs(), 1, "grouped members must be adjacent in z order");
    // the block should sit at the topmost member's original spot (index 2)
    assert_eq!(ia.max(ic), 2, "group rises to its front-most member's z");
}

#[test]
fn ungroup_clears_membership() {
    let mut doc = Document::default();
    let a = add_rect(&mut doc, 0.0, 0.0, 10.0, 10.0);
    let b = add_rect(&mut doc, 20.0, 0.0, 30.0, 10.0);
    doc.group(&[a, b]).unwrap();
    doc.ungroup(&[a]); // ungrouping via any member dissolves the whole group
    assert_eq!(doc.top_group_of_path(a), None);
    assert_eq!(doc.top_group_of_path(b), None);
    assert!(doc.groups.is_empty(), "the group registry should be empty after ungroup");
    assert_eq!(doc.group_members(a), vec![a]);
}

#[test]
fn nested_group_ungroup_peels_one_level() {
    // group A = {a1,a2}, group B = {b1,b2}, then group the two groups → outer.
    // A single ungroup must peel ONLY the outer group, leaving A and B intact (Illustrator).
    let mut doc = Document::default();
    let a1 = add_rect(&mut doc, 0.0, 0.0, 1.0, 1.0);
    let a2 = add_rect(&mut doc, 2.0, 0.0, 3.0, 1.0);
    let b1 = add_rect(&mut doc, 4.0, 0.0, 5.0, 1.0);
    let b2 = add_rect(&mut doc, 6.0, 0.0, 7.0, 1.0);
    let ga = doc.group(&[a1, a2]).unwrap();
    let gb = doc.group(&[b1, b2]).unwrap();
    let outer = doc.group(&[a1, a2, b1, b2]).unwrap(); // group the two groups

    // everything now resolves to the outer group
    for p in [a1, a2, b1, b2] { assert_eq!(doc.top_group_of_path(p), Some(outer)); }
    assert_eq!(sorted(doc.group_members(a1)), sorted(vec![a1, a2, b1, b2]));

    // peel one level
    doc.ungroup(&[a1, a2, b1, b2]);
    assert_eq!(doc.top_group_of_path(a1), Some(ga), "inner group A must survive");
    assert_eq!(doc.top_group_of_path(a2), Some(ga));
    assert_eq!(doc.top_group_of_path(b1), Some(gb), "inner group B must survive");
    assert_eq!(doc.top_group_of_path(b2), Some(gb));
    assert_eq!(sorted(doc.group_members(a1)), sorted(vec![a1, a2]), "A is its own unit again");
    assert_eq!(sorted(doc.group_members(b1)), sorted(vec![b1, b2]));
    assert!(doc.groups.iter().all(|g| g.id != outer), "the outer group is gone");

    // peel the next level → fully ungrouped
    doc.ungroup(&[a1]);
    assert_eq!(doc.top_group_of_path(a1), None);
    assert_eq!(doc.top_group_of_path(a2), None);
    assert_eq!(doc.top_group_of_path(b1), Some(gb), "ungrouping A must not touch B");
}

#[test]
fn duplicating_a_group_stays_grouped() {
    // Alt-drag copying a group must yield a NEW group, not loose paths.
    let mut doc = Document::default();
    let a = add_rect(&mut doc, 0.0, 0.0, 1.0, 1.0);
    let b = add_rect(&mut doc, 2.0, 0.0, 3.0, 1.0);
    let g = doc.group(&[a, b]).unwrap();
    let copies = doc.dup_paths(&[a, b]);
    assert_eq!(copies.len(), 2, "two paths copied");
    let cg = doc.top_group_of_path(copies[0]).expect("the copy must be grouped");
    assert_eq!(doc.top_group_of_path(copies[1]), Some(cg), "both copies in the same group");
    assert_ne!(cg, g, "the copy is a distinct group from the original");
    assert_eq!(doc.top_group_of_path(a), Some(g), "original group untouched");
    assert_eq!(sorted(doc.group_members(copies[0])), sorted(copies.clone()));
}

#[test]
fn duplicating_nested_group_preserves_nesting() {
    let mut doc = Document::default();
    let a1 = add_rect(&mut doc, 0.0, 0.0, 1.0, 1.0);
    let a2 = add_rect(&mut doc, 2.0, 0.0, 3.0, 1.0);
    let b1 = add_rect(&mut doc, 4.0, 0.0, 5.0, 1.0);
    let b2 = add_rect(&mut doc, 6.0, 0.0, 7.0, 1.0);
    doc.group(&[a1, a2]).unwrap();
    doc.group(&[b1, b2]).unwrap();
    let x = doc.group(&[a1, a2, b1, b2]).unwrap();
    let copies = doc.dup_paths(&[a1, a2, b1, b2]);
    assert_eq!(copies.len(), 4);
    // all copies under one new top group (≠ x)
    let ctop = doc.top_group_of_path(copies[0]).unwrap();
    for c in &copies { assert_eq!(doc.top_group_of_path(*c), Some(ctop)); }
    assert_ne!(ctop, x);
    // peeling one level off the copy must reveal TWO inner sub-groups (mirroring A and B)
    doc.ungroup(&copies);
    let tops: std::collections::HashSet<u32> =
        copies.iter().map(|c| doc.top_group_of_path(*c).unwrap()).collect();
    assert_eq!(tops.len(), 2, "the copy preserved its two inner sub-groups");
}

#[test]
fn re_grouping_single_group_is_noop() {
    let mut doc = Document::default();
    let a = add_rect(&mut doc, 0.0, 0.0, 1.0, 1.0);
    let b = add_rect(&mut doc, 2.0, 0.0, 3.0, 1.0);
    let g = doc.group(&[a, b]).unwrap();
    // selecting the whole group and pressing Ctrl+G again should NOT wrap it in a redundant layer
    assert!(doc.group(&[a, b]).is_none(), "re-grouping one existing group is a no-op");
    assert_eq!(doc.top_group_of_path(a), Some(g));
}

#[test]
fn shift_click_near_selection_corner_still_selects() {
    // Root cause of the grouping bug: with one shape selected, shift-clicking a CLOSE second
    // shape used to be stolen by the rotate-ring (22px outside the frame corner) → it rotated
    // the first shape instead of adding the second. Shift-click on an object must always select.
    let mut ed = Editor::new();
    let s1 = ed_rect(&mut ed, 0.0, 0.0, 40.0, 40.0);
    let s2 = ed_rect(&mut ed, 50.0, 0.0, 90.0, 40.0); // only a 10px gap → s2 sits inside s1's rotate ring
    ed.set_tool(ToolKind::Object);
    click(&mut ed, 20.0, 20.0, false); // select s1
    assert_eq!(ed.objsel.len(), 1);
    click(&mut ed, 52.0, 8.0, true);   // shift-click s2 near s1's (40,0) corner
    assert!(ed.objsel.contains(&s1) && ed.objsel.contains(&s2), "both shapes must be selected");
    assert_eq!(ed.objsel.len(), 2, "shift-click on a nearby object must select it, not rotate");
}

/// Add a path straight to an Editor's document (mirrors what the tools do).
fn ed_rect(ed: &mut Editor, x0: f32, y0: f32, x1: f32, y1: f32) -> u32 {
    let anchors = ed.doc.build_shape(ShapeKind::Rect, [x0, y0], [x1, y1]);
    let id = ed.doc.nid();
    ed.doc.paths.push(Path::new(id, anchors, true, Some([0.0, 0.0, 0.0, 1.0]), None, 1.0));
    id
}

/// Simulate a real Object-tool click (down+up) at a point, optionally with Shift held.
fn click(ed: &mut Editor, x: f32, y: f32, shift: bool) {
    ed.mods = Mods { shift, alt: false, ctrl: false };
    ed.pointer_down([x, y]);
    ed.pointer_up();
    ed.mods = Mods::default();
}

#[test]
fn app_flow_group_two_groups_then_ungroup() {
    // FAITHFUL reproduction of Ahmed's steps through the actual Object tool + selection,
    // not just the model methods: group A, group B, group both into X, then ungroup X once.
    // X must peel to leave BOTH A and B intact.
    // realistic scale: shapes ~40 units, spaced ~100 apart, so the 22px rotate-ring around a
    // selected shape never overlaps the next shape's click point (that's a test-scale artifact).
    let mut ed = Editor::new();
    let a1 = ed_rect(&mut ed, 0.0, 0.0, 40.0, 40.0);
    let _a2 = ed_rect(&mut ed, 100.0, 0.0, 140.0, 40.0);
    let b1 = ed_rect(&mut ed, 200.0, 0.0, 240.0, 40.0);
    let _b2 = ed_rect(&mut ed, 300.0, 0.0, 340.0, 40.0);
    ed.set_tool(ToolKind::Object);

    // group A (click a1, shift-click a2, Ctrl+G)
    click(&mut ed, 20.0, 20.0, false);
    click(&mut ed, 120.0, 20.0, true);
    ed.group_selection();
    let ga = ed.doc.top_group_of_path(a1).expect("A should be a group");

    // group B (click b1, shift-click b2, Ctrl+G)
    click(&mut ed, 220.0, 20.0, false);
    click(&mut ed, 320.0, 20.0, true);
    ed.group_selection();
    let gb = ed.doc.top_group_of_path(b1).expect("B should be a group");
    assert_ne!(ga, gb, "A and B are distinct groups");

    // select A and B, group into X (click A, shift-click B, Ctrl+G)
    click(&mut ed, 20.0, 20.0, false);
    click(&mut ed, 220.0, 20.0, true);
    ed.group_selection();
    let x = ed.doc.top_group_of_path(a1).expect("X should exist");
    assert_eq!(ed.doc.top_group_of_path(b1), Some(x), "both A and B now resolve to X");

    // select X (click any member) and ungroup once
    click(&mut ed, 20.0, 20.0, false);
    ed.ungroup_selection();

    assert_eq!(ed.doc.top_group_of_path(a1), Some(ga), "A must survive as a group");
    assert_eq!(ed.doc.top_group_of_path(b1), Some(gb), "B must survive as a group");
}

#[test]
fn sync_groups_drops_deleted_paths() {
    let mut doc = Document::default();
    let a = add_rect(&mut doc, 0.0, 0.0, 10.0, 10.0);
    let b = add_rect(&mut doc, 20.0, 0.0, 30.0, 10.0);
    doc.group(&[a, b]).unwrap();

    // delete path a directly (as delete_selected does), then reconcile
    doc.paths.retain(|p| p.id != a);
    doc.sync_groups();
    assert!(!doc.group_of.contains_key(&a), "membership for a deleted path must be dropped");
    // group still has b → stays alive
    assert!(doc.top_group_of_path(b).is_some());

    // delete b too → group becomes empty and is removed
    doc.paths.retain(|p| p.id != b);
    doc.sync_groups();
    assert!(doc.groups.is_empty(), "an empty group must be removed");
    assert!(doc.group_of.is_empty());
}
