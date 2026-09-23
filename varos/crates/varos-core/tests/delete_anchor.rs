//! A32 — deleting anchor points OPENS the path the Illustrator way. Removing a point must never
//! silently re-link its two neighbours with a phantom segment: a closed ring opens at the hole, and
//! deleting an interior point of an open path splits it in two. Pure editor topology, no UI.
//!
//! Run with:  cargo test -p varos-core --test delete_anchor

use std::collections::HashSet;
use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, Path, ShapeKind};
use varos_core::scene::{build_scene, Prim};
use varos_core::EditCommand;

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

// ---------- 2026-09-23 re-verification of A32 against the full acceptance list ----------
// These go through `EditCommand::DeleteSelected` — the exact path the Delete/Backspace key takes in the
// app — and use real 4-anchor ellipses from the Ellipse tool's geometry (smooth points with handles).

/// A closed 4-anchor ellipse exactly as the Ellipse tool builds it; returns (path id, anchor ids in order).
fn ellipse(ed: &mut Editor, x0: f32, y0: f32, x1: f32, y1: f32) -> (u32, Vec<u32>) {
    let anchors = ed.doc.build_shape(ShapeKind::Ellipse, [x0, y0], [x1, y1]);
    let ids = anchors.iter().map(|a| a.id).collect();
    let pid = ed.doc.nid();
    ed.doc.paths.push(Path::new(pid, anchors, true, Some([0.5, 0.5, 0.5, 1.0]), Some([0.0, 0.0, 0.0, 1.0]), 1.0));
    (pid, ids)
}

fn direct_editor() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.ids = 500;
    ed.set_tool(ToolKind::Direct);
    ed
}

/// Every drawn segment after the delete must be a segment that existed BEFORE it (either direction).
/// A re-linked pair of former non-neighbours is exactly the A32 bug.
fn assert_no_invented_segment(before: &[Path], after: &[Path]) {
    let mut original: HashSet<(u32, u32)> = HashSet::new();
    for p in before {
        let n = p.anchors.len();
        let segs = if p.closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (p.anchors[i].id, p.anchors[(i + 1) % n].id);
            original.insert((a, b));
            original.insert((b, a));
        }
    }
    for p in after {
        let n = p.anchors.len();
        let segs = if p.closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (p.anchors[i].id, p.anchors[(i + 1) % n].id);
            assert!(original.contains(&(a, b)), "delete invented a segment {a}→{b} that never existed (path {p:?})");
        }
    }
}

#[test]
fn deleting_one_anchor_of_a_closed_circle_leaves_an_open_three_anchor_arc() {
    let mut ed = direct_editor();
    let (_, ids) = ellipse(&mut ed, 0.0, 0.0, 100.0, 100.0); // top, right, bottom, left
    let before = ed.doc.paths.clone();
    ed.selected.insert(ids[0]); // delete TOP — its neighbours are LEFT (ids[3]) and RIGHT (ids[1])
    ed.execute(EditCommand::DeleteSelected);

    assert_eq!(ed.doc.paths.len(), 1);
    let p = &ed.doc.paths[0];
    assert!(!p.closed, "the circle must OPEN, not re-close over the gap");
    assert_eq!(p.anchors.len(), 3, "4 − 1 = 3 anchors");
    let order: Vec<u32> = p.anchors.iter().map(|a| a.id).collect();
    assert_eq!(order, vec![ids[1], ids[2], ids[3]], "open arc runs right → bottom → left; the gap is at the top");
    // The three surviving arcs are untouched — every remaining anchor keeps its exact handles.
    for a in &p.anchors {
        let orig = before[0].anchors.iter().find(|o| o.id == a.id).unwrap();
        assert_eq!(a, orig, "a surviving anchor must not be altered by the delete");
    }
    assert_no_invented_segment(&before, &ed.doc.paths);

    // What the user SEES: the stroke is an open polyline from RIGHT to LEFT — nothing drawn across the top.
    ed.doc.artboards.clear();
    let scene = build_scene(&ed, 1.0);
    let strokes: Vec<&Vec<[f32; 2]>> = scene
        .content
        .iter()
        .flat_map(|g| g.prims())
        .filter_map(|p| if let Prim::Stroke { pts, .. } = p { Some(pts) } else { None })
        .collect();
    assert_eq!(strokes.len(), 1, "one stroke for the one path");
    let (first, last) = (strokes[0][0], *strokes[0].last().unwrap());
    let d = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    assert!(
        d(first, [100.0, 50.0]) < 0.01 && d(last, [0.0, 50.0]) < 0.01,
        "stroke must run right→left: {first:?}..{last:?}"
    );
    assert!(strokes[0].iter().all(|q| q[1] >= 50.0 - 0.01), "no stroke point may cross the deleted top arc");
}

#[test]
fn one_delete_across_two_circles_opens_both_and_one_undo_restores_both() {
    let mut ed = direct_editor();
    let (_, a) = ellipse(&mut ed, 0.0, 0.0, 100.0, 100.0);
    let (_, b) = ellipse(&mut ed, 60.0, 20.0, 160.0, 120.0); // overlapping the first
    let before = ed.doc.paths.clone();
    ed.selected.insert(a[1]);
    ed.selected.insert(b[3]);
    ed.execute(EditCommand::DeleteSelected);

    assert_eq!(ed.doc.paths.len(), 2);
    for p in &ed.doc.paths {
        assert!(!p.closed, "each circle opens where its anchor was deleted");
        assert_eq!(p.anchors.len(), 3);
    }
    assert_no_invented_segment(&before, &ed.doc.paths);
    let after = ed.doc.paths.clone();

    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths, before, "ONE undo restores BOTH circles exactly (closed, 4 anchors, same handles)");
    ed.execute(EditCommand::Undo); // nothing earlier on the stack → still the original (the delete was ONE step)
    assert_eq!(ed.doc.paths, before);
    ed.execute(EditCommand::Redo);
    assert_eq!(ed.doc.paths, after, "redo re-applies the whole multi-object delete");
}

#[test]
fn deleting_several_anchors_on_three_overlapping_ellipses_never_relinks() {
    // The reported repro: 3 overlapping ellipses, several anchors picked across all of them, one Delete.
    let mut ed = direct_editor();
    let (_, e1) = ellipse(&mut ed, 0.0, 0.0, 120.0, 80.0);
    let (_, e2) = ellipse(&mut ed, 60.0, 30.0, 180.0, 110.0);
    let (_, e3) = ellipse(&mut ed, 30.0, 60.0, 150.0, 140.0);
    let before = ed.doc.paths.clone();
    // e1: two ADJACENT anchors; e2: one anchor; e3: two OPPOSITE anchors.
    for id in [e1[0], e1[1], e2[2], e3[0], e3[2]] {
        ed.selected.insert(id);
    }
    ed.execute(EditCommand::DeleteSelected);

    assert!(ed.doc.paths.iter().all(|p| !p.closed), "no path may stay/come back closed after losing anchors");
    assert_no_invented_segment(&before, &ed.doc.paths);
    let with = |id: u32| ed.doc.paths.iter().find(|p| p.anchors.iter().any(|a| a.id == id)).unwrap();
    // e1 lost top+right → the single surviving arc bottom→left (one segment).
    assert_eq!(with(e1[2]).anchors.iter().map(|a| a.id).collect::<Vec<_>>(), vec![e1[2], e1[3]]);
    // e2 lost bottom → open arc left → top → right.
    assert_eq!(with(e2[0]).anchors.len(), 3);
    // e3 lost top+bottom → every segment touched a deleted anchor: only lone points remain, no segment.
    assert!(with(e3[1]).anchors.len() == 1 && with(e3[3]).anchors.len() == 1);
    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths, before, "one undo restores all three ellipses");
}
