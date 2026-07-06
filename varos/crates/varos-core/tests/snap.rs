//! SnapEngine logic tests — pure math, no UI (allowed per the math-test rule). Proves point/geometry
//! snapping actually fires, so a "snap not working" report can be localised to logic vs UI/feedback.

use varos_core::editor::{AlignMode, Editor};
use varos_core::model::{Anchor, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn rect(id: u32, ids: [u32; 4], x: f32, y: f32, w: f32, h: f32) -> Path {
    Path::new(
        id,
        vec![anc(ids[0], x, y), anc(ids[1], x + w, y), anc(ids[2], x + w, y + h), anc(ids[3], x, y + h)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}
fn two_rects() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.push(rect(10, [1, 2, 3, 4], 0.0, 0.0, 100.0, 100.0)); // A
    ed.doc.paths.push(rect(11, [5, 6, 7, 8], 200.0, 0.0, 100.0, 100.0)); // B
    ed.doc.ids = 8;
    ed.ppu = 1.0; // zoom 1 → tolerance = radius_px (8) world units
    ed
}

#[test]
fn anchor_snaps_to_other_object_anchor() {
    let mut ed = two_rects();
    ed.selected.insert(2); // dragging A's top-right anchor (world [100,0])
                           // propose moving it to [198,0] — 2 world from B's top-left [200,0] → should snap onto it
    let (nd, guides, _) = ed.snap_anchor(&[[100.0, 0.0]], [98.0, 0.0]);
    assert!((nd[0] - 100.0).abs() < 0.01, "x should snap so the anchor lands on 200, got delta {}", nd[0]);
    assert!(nd[1].abs() < 0.01, "y stays 0, got {}", nd[1]);
    assert!(!guides.is_empty(), "a snap marker should be emitted");
}

#[test]
fn anchor_snaps_to_path_edge() {
    let mut ed = two_rects();
    ed.selected.insert(2);
    // move A's top-right anchor to [198, 50] — near B's LEFT EDGE (x=200) at mid-height → geometry snap
    let (nd, guides, _) = ed.snap_anchor(&[[100.0, 0.0]], [98.0, 50.0]);
    assert!((nd[0] - 100.0).abs() < 0.01, "x should snap to B's left edge (200), got delta {}", nd[0]);
    assert!(!guides.is_empty());
}

#[test]
fn no_snap_when_far() {
    let mut ed = two_rects();
    ed.selected.insert(2);
    // moved = [120, 17] — far (>8) from every anchor / artboard edge on BOTH axes → no snap
    let (nd, guides, _) = ed.snap_anchor(&[[100.0, 0.0]], [20.0, 17.0]);
    assert!((nd[0] - 20.0).abs() < 0.01 && (nd[1] - 17.0).abs() < 0.01, "no snap → delta unchanged, got {:?}", nd);
    assert!(guides.is_empty());
}

#[test]
fn align_panel_aligns_selected_anchors() {
    // The Align panel must work on selected ANCHOR POINTS (Direct Selection), not only objects.
    let mut ed = two_rects();
    ed.selected.insert(1); // A top-left  [0,0]
    ed.selected.insert(2); // A top-right [100,0]
    ed.selected.insert(5); // B top-left  [200,0]
    ed.align(AlignMode::Left); // all x → min (0)
    for id in [1u32, 2, 5] {
        assert!((ed.doc.anchor(id).unwrap().p[0]).abs() < 0.01, "anchor {} x should be 0", id);
    }
}

#[test]
fn anchor_aligns_to_same_shape_corner() {
    // a SINGLE square: dragging the top-right corner should align (smart guide) to the other corners.
    let mut ed = Editor::new();
    ed.doc.paths.push(rect(10, [1, 2, 3, 4], 0.0, 0.0, 100.0, 100.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.selected.insert(2); // top-right corner [100,0]
                           // nudge it to [103, 60]: x=103 is 3 from bottom-right's x (100) → X aligns; y=60 far → free
    let (nd, guides, _) = ed.snap_anchor(&[[100.0, 0.0]], [3.0, 60.0]);
    assert!((nd[0] - 0.0).abs() < 0.01, "x should align back to 100 (same-shape corner), got delta {}", nd[0]);
    assert!(!guides.is_empty(), "an alignment guide should be drawn");
}
