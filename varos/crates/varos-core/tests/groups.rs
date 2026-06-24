//! Headless tests for the grouping model (hierarchy on top of the flat path list).
//! Pure data — no GPU, no UI. Verifies group/ungroup, whole-group membership, z-order
//! contiguity, and the post-delete reconciliation done by `sync_groups`.
//!
//! Run with:  cargo test -p varos-core --test groups

use varos_core::model::{Document, Path, ShapeKind};

/// Add a rectangle path to the doc; return its id.
fn add_rect(doc: &mut Document, x0: f32, y0: f32, x1: f32, y1: f32) -> u32 {
    let anchors = doc.build_shape(ShapeKind::Rect, [x0, y0], [x1, y1]);
    let id = doc.nid();
    doc.paths.push(Path {
        id,
        anchors,
        closed: true,
        fill: Some([0.0, 0.0, 0.0, 1.0]),
        stroke: None,
        stroke_width: 1.0,
        holes: vec![],
    });
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
