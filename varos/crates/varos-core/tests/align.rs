//! A4 — align has a TARGET mode (Auto/Selection/Artboard) with a smart default. Headless, no GPU:
//! we drive `Editor::align` directly and read outline bounds. Proves Selection mode keeps the
//! historic "align objects to each other" behaviour, Artboard mode snaps each top-level item onto
//! the active page edges (works for a single object, moves a group as one), and Auto picks between
//! them by selection count — falling back to Selection when there is no active board.

use varos_core::editor::{AlignMode, AlignTarget, Editor};
use varos_core::model::{Anchor, Artboard, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
/// An axis-aligned filled square with a stable id/anchor-id block.
fn rect(id: u32, base: u32, x: f32, y: f32, w: f32, h: f32) -> Path {
    Path::new(
        id,
        vec![anc(base, x, y), anc(base + 1, x + w, y), anc(base + 2, x + w, y + h), anc(base + 3, x, y + h)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}
fn board(x: f32, y: f32, w: f32, h: f32) -> Artboard {
    Artboard { x, y, w, h, name: "A".into(), ..Artboard::default() }
}
/// The object's world-space bounds (x0, y0, x1, y1) after an op.
fn bbox(ed: &Editor, id: u32) -> (f32, f32, f32, f32) {
    ed.doc.outline_bbox(ed.doc.pidx(id).unwrap())
}
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.1
}

/// A 200×200 page at the origin + two disjoint squares:
/// A = (30,40)+20², B = (100,90)+20². Combined selection bbox = (30,40,120,110).
fn scene() -> Editor {
    let mut ed = Editor::new();
    ed.ppu = 1.0;
    ed.doc.artboards = vec![board(0.0, 0.0, 200.0, 200.0)];
    ed.doc.active = 0;
    ed.doc.paths.push(rect(1, 1, 30.0, 40.0, 20.0, 20.0));
    ed.doc.paths.push(rect(2, 10, 100.0, 90.0, 20.0, 20.0));
    ed.doc.ids = 20;
    ed.doc.sync_tree();
    ed
}

#[test]
fn align_to_selection_aligns_two_objects_to_each_other() {
    let mut ed = scene();
    ed.objsel.insert(1);
    ed.objsel.insert(2);
    ed.align(AlignMode::Left, AlignTarget::Selection);
    // both left edges collapse onto the selection's leftmost edge (30), NOT the page edge (0)
    assert!(near(bbox(&ed, 1).0, 30.0), "A keeps the shared left edge, got {:?}", bbox(&ed, 1));
    assert!(near(bbox(&ed, 2).0, 30.0), "B slides left onto A's edge, got {:?}", bbox(&ed, 2));

    // top-align the same pair → both top edges meet at the selection's top (40)
    ed.align(AlignMode::Top, AlignTarget::Selection);
    assert!(near(bbox(&ed, 1).1, 40.0));
    assert!(near(bbox(&ed, 2).1, 40.0), "B rises onto the shared top edge, got {:?}", bbox(&ed, 2));
}

#[test]
fn align_to_artboard_puts_the_object_on_the_board_edge() {
    let mut ed = scene();
    ed.objsel.insert(1); // a single object — Artboard mode still acts (Selection mode would not)

    ed.align(AlignMode::Left, AlignTarget::Artboard);
    assert!(near(bbox(&ed, 1).0, 0.0), "left edge lands on the page's left (x=0), got {:?}", bbox(&ed, 1));

    ed.align(AlignMode::Right, AlignTarget::Artboard);
    assert!(near(bbox(&ed, 1).2, 200.0), "right edge lands on the page's right (x=200), got {:?}", bbox(&ed, 1));

    ed.align(AlignMode::Bottom, AlignTarget::Artboard);
    assert!(near(bbox(&ed, 1).3, 200.0), "bottom edge lands on the page's bottom (y=200), got {:?}", bbox(&ed, 1));

    ed.align(AlignMode::Middle, AlignTarget::Artboard);
    let b = bbox(&ed, 1);
    assert!(near((b.1 + b.3) * 0.5, 100.0), "vertical centre lands on the page centre (y=100), got {b:?}");
}

#[test]
fn auto_picks_artboard_for_a_single_object() {
    let mut ed = scene();
    ed.objsel.insert(1); // ONE object + a page present → Auto resolves to Artboard
    ed.align(AlignMode::Left, AlignTarget::Auto);
    assert!(
        near(bbox(&ed, 1).0, 0.0),
        "Auto with a single object aligns it to the page edge (x=0), got {:?}",
        bbox(&ed, 1)
    );
}

#[test]
fn auto_picks_selection_for_multiple_objects() {
    let mut ed = scene();
    ed.objsel.insert(1);
    ed.objsel.insert(2); // TWO objects → Auto resolves to Selection (align to each other)
    ed.align(AlignMode::Left, AlignTarget::Auto);
    // Selection edge is 30 (leftmost object), distinct from the page edge 0 — proves it chose Selection
    assert!(near(bbox(&ed, 1).0, 30.0), "A stays on the shared left edge, got {:?}", bbox(&ed, 1));
    assert!(near(bbox(&ed, 2).0, 30.0), "B aligns to A, not the page, got {:?}", bbox(&ed, 2));
}

#[test]
fn artboard_target_without_a_board_falls_back_to_selection() {
    let mut ed = scene();
    ed.doc.artboards.clear(); // no active artboard → Artboard/Auto must degrade, never panic
    ed.objsel.insert(1);
    ed.objsel.insert(2);
    ed.align(AlignMode::Left, AlignTarget::Artboard);
    assert!(near(bbox(&ed, 1).0, 30.0), "fell back to Selection (edge 30), got {:?}", bbox(&ed, 1));
    assert!(near(bbox(&ed, 2).0, 30.0), "fell back to Selection (edge 30), got {:?}", bbox(&ed, 2));
}

#[test]
fn auto_single_object_without_a_board_is_a_safe_no_op() {
    let mut ed = scene();
    ed.doc.artboards.clear();
    ed.objsel.insert(1); // single object, no board → Auto→Selection→needs ≥2 → nothing moves, no panic
    let before = bbox(&ed, 1);
    ed.align(AlignMode::Left, AlignTarget::Auto);
    assert_eq!(bbox(&ed, 1), before, "a lone object with no page and no peers stays put");
}

#[test]
fn align_to_artboard_moves_a_group_as_one_unit() {
    let mut ed = scene();
    ed.doc.group(&[1, 2]).unwrap(); // A+B become ONE top-level unit
    ed.objsel.insert(1);
    ed.objsel.insert(2); // selecting a group puts all its members in objsel
    ed.align(AlignMode::Left, AlignTarget::Artboard);
    // the GROUP's combined left edge (30) lands on the page edge (0): everything shifts by -30,
    // keeping the members' relative offset (B was 70 to the right of A, still is).
    assert!(near(bbox(&ed, 1).0, 0.0), "the group's left edge lands on the page, got {:?}", bbox(&ed, 1));
    assert!(near(bbox(&ed, 2).0, 70.0), "the second member keeps its offset (moved as one), got {:?}", bbox(&ed, 2));
}
