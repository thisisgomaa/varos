//! A32 — deleting anchor points OPENS the path the Illustrator way. Removing a point must never
//! silently re-link its two neighbours with a phantom segment: a closed ring opens at the hole, and
//! deleting an interior point of an open path splits it in two. Pure editor topology, no UI.
//!
//! Run with:  cargo test -p varos-core --test delete_anchor

use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, Path};
use varos_core::scene::{build_scene, Prim};

fn corner(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}

/// A closed square whose four corners carry `ids` (A B C D, counter-clockwise).
fn square(pid: u32, ids: [u32; 4]) -> Path {
    let a = corner(ids[0], 0.0, 0.0);
    let b = corner(ids[1], 10.0, 0.0);
    let c = corner(ids[2], 10.0, 10.0);
    let d = corner(ids[3], 0.0, 10.0);
    Path::new(pid, vec![a, b, c, d], true, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0)
}

#[test]
fn deleting_a_corner_opens_the_ring_at_the_hole() {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(square(100, [1, 2, 3, 4])); // A B C D, closed
    ed.doc.ids = 500;
    ed.set_tool(ToolKind::Direct);
    ed.selected.insert(2); // delete B — its neighbours are A(1) and C(3)
    ed.delete_selected();

    assert_eq!(ed.doc.paths.len(), 1, "one contour, still");
    let p = &ed.doc.paths[0];
    assert!(!p.closed, "the ring must OPEN, not stay closed with a phantom A-C segment");
    assert_eq!(p.anchors.len(), 3, "four corners minus one = three");
    let ids: Vec<u32> = p.anchors.iter().map(|a| a.id).collect();
    assert!(!ids.contains(&2), "the deleted corner is gone, got {ids:?}");
    // Opening at B puts the GAP between B's neighbours, so A and C become the two open ENDS —
    // proof that no segment bridges them (an endpoint has no wrap segment).
    let ends = [p.anchors.first().unwrap().id, p.anchors.last().unwrap().id];
    assert!(
        ends.contains(&1) && ends.contains(&3),
        "B's neighbours (A=1, C=3) must be the open endpoints, got ends {ends:?} of {ids:?}"
    );
}

#[test]
fn deleting_across_two_shapes_in_one_op_opens_both() {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(square(100, [1, 2, 3, 4]));
    ed.doc.paths.push(square(200, [11, 12, 13, 14]));
    ed.doc.ids = 500;
    ed.set_tool(ToolKind::Direct);
    ed.selected.insert(2); // a corner of shape 1
    ed.selected.insert(12); // a corner of shape 2 — one Delete acts on both
    ed.delete_selected();

    assert_eq!(ed.doc.paths.len(), 2, "both shapes survive as open contours");
    for p in &ed.doc.paths {
        assert!(!p.closed, "each shape must OPEN where its anchor was deleted");
        assert_eq!(p.anchors.len(), 3, "each shape drops exactly one corner");
    }
    let all: Vec<u32> = ed.doc.paths.iter().flat_map(|p| p.anchors.iter().map(|a| a.id)).collect();
    assert!(!all.contains(&2) && !all.contains(&12), "both deleted corners gone, got {all:?}");
}

#[test]
fn deleting_an_interior_point_of_an_open_path_splits_it_in_two() {
    // open A-B-C-D-E; delete the middle C → two open paths A-B and D-E (the hole is a real break).
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    let anchors = vec![
        corner(1, 0.0, 0.0),
        corner(2, 10.0, 0.0),
        corner(3, 20.0, 0.0), // C — interior
        corner(4, 30.0, 0.0),
        corner(5, 40.0, 0.0),
    ];
    ed.doc.paths.push(Path::new(100, anchors, false, None, Some([0.0, 0.0, 0.0, 1.0]), 1.0));
    ed.doc.ids = 500;
    ed.set_tool(ToolKind::Direct);
    ed.selected.insert(3);
    ed.delete_selected();

    assert_eq!(ed.doc.paths.len(), 2, "the interior hole splits the open path into two");
    let mut lens: Vec<usize> = ed.doc.paths.iter().map(|p| p.anchors.len()).collect();
    lens.sort();
    assert_eq!(lens, vec![2, 2], "A-B and D-E, two anchors each");
    for p in &ed.doc.paths {
        assert!(!p.closed, "the split pieces are open");
        assert!(!p.anchors.iter().any(|a| a.id == 3), "the deleted point is in neither piece");
    }
}

#[test]
fn deleting_an_open_endpoint_just_trims_it() {
    // open A-B-C; delete the last point C → still one open path A-B (no split, no re-link).
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    let anchors = vec![corner(1, 0.0, 0.0), corner(2, 10.0, 0.0), corner(3, 20.0, 0.0)];
    ed.doc.paths.push(Path::new(100, anchors, false, None, Some([0.0, 0.0, 0.0, 1.0]), 1.0));
    ed.doc.ids = 500;
    ed.set_tool(ToolKind::Direct);
    ed.selected.insert(3);
    ed.delete_selected();

    assert_eq!(ed.doc.paths.len(), 1, "trimming an endpoint keeps one contour");
    let p = &ed.doc.paths[0];
    assert!(!p.closed);
    assert_eq!(p.anchors.iter().map(|a| a.id).collect::<Vec<_>>(), vec![1, 2], "only C was trimmed");
}

/// FB2 — an OPEN compound path (outer + hole, e.g. a donut A32 already opened) whose outer is split by
/// deleting an interior anchor: each hole must travel to the fragment whose area actually holds it.
/// Left lobe = triangle (1,2,3); right lobe = triangle (5,6,7); anchor 4 bridges them and is deleted.
fn open_compound_with_hole_in(lobe_right: bool) -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    let outer = vec![
        corner(1, 0.0, 0.0),
        corner(2, 40.0, 0.0),
        corner(3, 20.0, 40.0),   // left lobe 1,2,3  (centre ~20,13)
        corner(4, 100.0, -60.0), // interior anchor we delete (bridges the lobes)
        corner(5, 160.0, 0.0),
        corner(6, 240.0, 0.0),
        corner(7, 200.0, 40.0), // right lobe 5,6,7 (centre ~200,13)
    ];
    let mut p = Path::new(100, outer, false, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0);
    // a small square hole inside whichever lobe the caller chose
    let cx = if lobe_right { 200.0 } else { 20.0 };
    p.holes = vec![vec![
        corner(20, cx - 6.0, 5.0),
        corner(21, cx + 6.0, 5.0),
        corner(22, cx + 6.0, 15.0),
        corner(23, cx - 6.0, 15.0),
    ]];
    ed.doc.paths.push(p);
    ed.doc.ids = 500;
    ed.delete_anchor(4); // split the outer at the bridging anchor
    ed
}

#[test]
fn a_split_sends_the_hole_to_the_fragment_that_contains_it() {
    let ed = open_compound_with_hole_in(true); // hole in the RIGHT lobe
    assert_eq!(ed.doc.paths.len(), 2, "the interior delete splits the outer into two fragments");
    let right = ed.doc.paths.iter().find(|p| p.anchors.iter().any(|a| a.id == 5)).unwrap();
    let left = ed.doc.paths.iter().find(|p| p.anchors.iter().any(|a| a.id == 1)).unwrap();
    assert_eq!(right.holes.len(), 1, "the hole follows the RIGHT fragment that actually contains it");
    assert!(left.holes.is_empty(), "the LEFT fragment must not inherit a hole it doesn't overlap (FB2)");
}

#[test]
fn a_split_keeps_a_left_lobe_hole_on_the_left() {
    let ed = open_compound_with_hole_in(false); // hole in the LEFT lobe
    let right = ed.doc.paths.iter().find(|p| p.anchors.iter().any(|a| a.id == 5)).unwrap();
    let left = ed.doc.paths.iter().find(|p| p.anchors.iter().any(|a| a.id == 1)).unwrap();
    assert_eq!(left.holes.len(), 1, "a hole in the left lobe stays on the left fragment");
    assert!(right.holes.is_empty(), "…and does not leak to the right");
}

#[test]
fn a_split_keeps_uneven_left_hole_on_the_head_fragment() {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    let outer = vec![
        corner(1, 0.0, 0.0),
        corner(2, 40.0, 0.0),
        corner(3, 20.0, 40.0),   // left lobe
        corner(4, 100.0, -60.0), // interior anchor we delete
        corner(5, 160.0, 0.0),
        corner(6, 240.0, 0.0),
        corner(7, 200.0, 40.0), // right lobe
    ];
    let mut p = Path::new(100, outer, false, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0);
    let mut hole = vec![corner(20, 14.0, 8.0), corner(21, 26.0, 8.0), corner(22, 26.0, 16.0), corner(23, 14.0, 16.0)];
    for i in 0..24 {
        let dx = (i % 6) as f32 * 0.02;
        let dy = (i / 6) as f32 * 0.02;
        hole.push(corner(24 + i, 200.0 + dx, 12.0 + dy));
    }
    p.holes = vec![hole];
    ed.doc.paths.push(p);
    ed.doc.ids = 500;

    ed.delete_anchor(4);

    assert_eq!(ed.doc.paths.len(), 2, "the interior delete splits the outer into two fragments");
    let right = ed.doc.paths.iter().find(|p| p.anchors.iter().any(|a| a.id == 5)).unwrap();
    let left = ed.doc.paths.iter().find(|p| p.anchors.iter().any(|a| a.id == 1)).unwrap();
    assert_eq!(left.holes.len(), 1, "the uneven hole must stay with the left/head fragment");
    assert!(right.holes.is_empty(), "the right sibling must not get a hole with left-lobe vertices");
}

#[test]
fn an_opened_shape_keeps_its_fill() {
    // Illustrator: an open path still fills (endpoints joined by an implied line). Opening a filled
    // square by deleting a corner must NOT drop the fill — the scene still emits a Fill prim.
    let mut ed = Editor::new();
    ed.doc.artboards.clear(); // no page fill in the scene → only our path's prims
    ed.doc.paths.clear();
    ed.doc.paths.push(square(100, [1, 2, 3, 4])); // grey fill, closed
    ed.doc.ids = 500;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Direct);
    ed.selected.insert(2);
    ed.delete_selected();

    assert!(!ed.doc.paths[0].closed, "precondition: the shape is now open");
    let scene = build_scene(&ed, 1.0);
    let fills = scene.content.iter().flat_map(|g| g.prims()).filter(|p| matches!(p, Prim::Fill { .. })).count();
    assert!(fills >= 1, "the opened shape must still fill (implied close), got {fills} Fill prims");
}
