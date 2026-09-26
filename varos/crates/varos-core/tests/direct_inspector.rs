//! Astra F07 — a Direct-Selection edit must be MEASURED by the inspector. With nothing selected, press A and
//! grab an anchor straight off a path: the anchor moves (modeless — keep that), and the context bar / X·Y·W·H
//! must name it and show real numbers, not "No selection" and zeros. The X/Y/W/H fields then edit the
//! selected anchors through `EditCommand::SetObjectBounds`. Pure logic, no GPU.

use varos_core::command::EditCommand;
use varos_core::editor::{Editor, ToolKind};
use varos_core::geom::Pt;
use varos_core::model::{Anchor, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
/// A 100×40 filled rect at (10, 20): path id 10, anchors 1 (TL), 2 (TR), 3 (BR), 4 (BL). Nothing selected.
fn one_rect() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.push(Path::new(
        10,
        vec![anc(1, 10.0, 20.0), anc(2, 110.0, 20.0), anc(3, 110.0, 60.0), anc(4, 10.0, 60.0)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    ));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.doc.snap.enabled = false; // deterministic geometry
    ed.doc.sync_tree();
    ed
}
fn world(ed: &Editor, aid: u32) -> Pt {
    let pid = ed.doc.pid_of_anchor(aid).unwrap();
    ed.doc.unit_xform(pid).apply(ed.doc.anchor(aid).unwrap().p)
}
fn close(a: Pt, b: Pt) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}
fn set_bounds(ed: &mut Editor, x: Option<f32>, y: Option<f32>, w: Option<f32>, h: Option<f32>, ax: f32, ay: f32) {
    ed.execute(EditCommand::SetObjectBounds { x, y, width: w, height: h, anchor_x: ax, anchor_y: ay });
}

#[test]
fn nothing_selected_still_reads_as_no_selection() {
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Direct);
    assert_eq!(ed.direct_bbox(), None, "nothing selected → no Direct bounds (the panel keeps its zeros)");
    assert_eq!(ed.direct_label(), None, "nothing selected → the UI falls back to \"No selection\"");
    let rev = ed.rev;
    set_bounds(&mut ed, Some(500.0), Some(500.0), None, None, 0.0, 0.0);
    assert_eq!(ed.rev, rev, "editing X/Y with nothing selected is a no-op with no undo step");
    assert!(close(world(&ed, 1), [10.0, 20.0]));
}

#[test]
fn directly_grabbed_anchor_is_named_and_measured() {
    // The Astra repro: nothing selected, press A, grab the BR anchor and drag it +30,+5.
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Direct);
    ed.pointer_down([110.0, 60.0]);
    ed.pointer_move([140.0, 65.0]);
    ed.pointer_up();
    assert!(close(world(&ed, 3), [140.0, 65.0]), "the modeless anchor drag still works");
    assert!(ed.objsel.is_empty(), "no object selection — the F07 condition");

    assert_eq!(ed.direct_label().as_deref(), Some("Anchor"));
    let (x0, y0, x1, y1) = ed.direct_bbox().expect("a grabbed anchor has real bounds");
    assert!(close([x0, y0], [140.0, 65.0]), "X/Y = the anchor's position, got ({x0},{y0})");
    assert!((x1 - x0).abs() < 1e-6 && (y1 - y0).abs() < 1e-6, "one anchor → W = H = 0");
}

#[test]
fn several_anchors_report_their_point_bounds() {
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Direct);
    ed.selected.extend([2, 3]); // the right edge
    assert_eq!(ed.direct_label().as_deref(), Some("2 anchors"));
    let (x0, y0, x1, y1) = ed.direct_bbox().unwrap();
    assert!(close([x0, y0], [110.0, 20.0]) && close([x1, y1], [110.0, 60.0]), "bbox of the two points");
}

#[test]
fn path_level_direct_selection_reports_the_path() {
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Direct);
    ed.pointer_down([60.0, 40.0]); // click the FILL → path-level Direct selection (anchors hollow)
    ed.pointer_up();
    assert_eq!(ed.dsel_path, Some(10), "fill click = path-level Direct selection");
    assert_eq!(ed.direct_label().as_deref(), Some("Path"), "unnamed path → \"Path\"");
    let (x0, y0, x1, y1) = ed.direct_bbox().unwrap();
    assert!(close([x0, y0], [10.0, 20.0]) && close([x1, y1], [110.0, 60.0]), "the whole path's bounds");

    ed.doc.paths[0].name = Some("Crescent".into());
    assert_eq!(ed.direct_label().as_deref(), Some("Crescent"), "a named path shows its name");
}

#[test]
fn an_object_selection_wins_over_the_direct_measure() {
    let mut ed = one_rect();
    ed.selected.insert(3);
    ed.objsel.insert(10);
    assert_eq!(ed.direct_bbox(), None, "objects selected → the object bounds drive the panel, not Direct");
    assert_eq!(ed.direct_label(), None);
}

#[test]
fn x_y_fields_move_a_single_anchor_as_one_undo_step() {
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Direct);
    ed.selected.insert(3);
    ed.doc.anchor_mut(3).unwrap().hin = Some([110.0, 50.0]); // a handle must travel with its anchor
    let rev = ed.rev;
    set_bounds(&mut ed, Some(200.0), None, None, None, 0.0, 0.0);
    set_bounds(&mut ed, None, Some(-5.0), None, None, 0.0, 0.0);
    assert!(close(world(&ed, 3), [200.0, -5.0]), "X/Y land the anchor exactly, got {:?}", world(&ed, 3));
    assert_eq!(ed.doc.anchor(3).unwrap().hin, Some([200.0, -15.0]), "its handle moved by the same delta");
    for (aid, p) in [(1, [10.0, 20.0]), (2, [110.0, 20.0]), (4, [10.0, 60.0])] {
        assert!(close(world(&ed, aid), p), "unselected anchor {aid} must not move");
    }
    assert_eq!(ed.rev, rev + 2, "each field edit is one undo step");
    ed.undo();
    ed.undo();
    assert!(close(world(&ed, 3), [110.0, 60.0]), "undo restores the anchor");
    assert!(ed.selected.contains(&3), "and keeps it selected");
}

#[test]
fn w_field_scales_selected_anchors_about_the_reference_point() {
    let mut ed = one_rect();
    ed.selected.extend([1, 2]); // top edge, x 10..110
    set_bounds(&mut ed, None, None, Some(50.0), None, 0.0, 0.0); // ref = left
    assert!(close(world(&ed, 1), [10.0, 20.0]) && close(world(&ed, 2), [60.0, 20.0]), "left anchor fixed");
    set_bounds(&mut ed, None, None, Some(100.0), None, 1.0, 0.0); // ref = right
    assert!(close(world(&ed, 1), [-40.0, 20.0]) && close(world(&ed, 2), [60.0, 20.0]), "right anchor fixed");
    assert!(close(world(&ed, 3), [110.0, 60.0]), "unselected anchors untouched");
}

#[test]
fn zero_extent_w_h_edit_is_a_true_noop() {
    // One anchor has W = H = 0: nothing to scale. The UI disables those fields; the core must still never
    // do anything odd (or record an undo step) if one arrives.
    let mut ed = one_rect();
    ed.selected.insert(3);
    let rev = ed.rev;
    set_bounds(&mut ed, None, None, Some(40.0), Some(40.0), 0.5, 0.5);
    assert_eq!(ed.rev, rev, "no undo step for a no-op");
    assert!(close(world(&ed, 3), [110.0, 60.0]));
}

#[test]
fn rotated_unit_anchor_is_measured_and_moved_in_world() {
    // A marquee-selected anchor of a LIVE-rotated unit (not baked): the panel reads its WORLD position and
    // X/Y put it exactly there in world (the unit is baked first, as nudge/drag do).
    let mut ed = one_rect();
    ed.objsel.insert(10);
    ed.set_obj_rotation(30.0);
    ed.objsel.clear();
    ed.selected.insert(3);
    let u = ed.doc.unit_of(10).unwrap();
    assert!(!ed.doc.node_xform(u).is_identity(), "fixture: the rotation is live");
    let w3 = world(&ed, 3);
    let (x0, y0, _, _) = ed.direct_bbox().unwrap();
    assert!(close([x0, y0], w3), "X/Y read the WORLD position {w3:?}, got ({x0},{y0})");

    let others: Vec<Pt> = [1, 2, 4].iter().map(|&a| world(&ed, a)).collect();
    set_bounds(&mut ed, Some(w3[0] + 25.0), Some(w3[1]), None, None, 0.0, 0.0);
    assert!(close(world(&ed, 3), [w3[0] + 25.0, w3[1]]), "moved +25 along WORLD x, got {:?}", world(&ed, 3));
    for (i, &a) in [1u32, 2, 4].iter().enumerate() {
        assert!(close(world(&ed, a), others[i]), "anchor {a} kept its world position");
    }
}

// ---------- P17 (Codex review of the Astra batch, 2026-09-26): compound-path HOLE anchors ----------
// The inspector used to filter the grabbed anchors through the OUTER-ring-only lookup, so a hole-only
// Direct selection read as "nothing" (zeros, edits a no-op) and a mixed outer+hole selection measured and
// moved only the outer points.

/// A 100×100 square at the origin (anchors 1..4) with a 30×30 square HOLE at (30,30) (anchors 5..8).
fn donut() -> Editor {
    let mut ed = Editor::new();
    let mut p = Path::new(
        10,
        vec![anc(1, 0.0, 0.0), anc(2, 100.0, 0.0), anc(3, 100.0, 100.0), anc(4, 0.0, 100.0)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    );
    p.holes = vec![vec![anc(5, 30.0, 30.0), anc(6, 60.0, 30.0), anc(7, 60.0, 60.0), anc(8, 30.0, 60.0)]];
    ed.doc.paths.push(p);
    ed.doc.ids = 8;
    ed.ppu = 1.0;
    ed.doc.snap.enabled = false;
    ed.doc.sync_tree();
    ed.set_tool(ToolKind::Direct);
    ed
}

#[test]
fn hole_only_direct_selection_is_measured_and_moved() {
    let mut ed = donut();
    ed.selected.extend([5, 6, 7, 8]);
    assert_eq!(ed.direct_label().as_deref(), Some("4 anchors"), "hole anchors count as anchors");
    let (x0, y0, x1, y1) = ed.direct_bbox().expect("a hole-only selection has real bounds");
    assert!(close([x0, y0], [30.0, 30.0]) && close([x1, y1], [60.0, 60.0]), "got ({x0},{y0})-({x1},{y1})");

    let rev = ed.rev;
    set_bounds(&mut ed, Some(40.0), Some(35.0), None, None, 0.0, 0.0);
    assert_eq!(ed.rev, rev + 1, "one undo step");
    for (aid, p) in [(5, [40.0, 35.0]), (6, [70.0, 35.0]), (7, [70.0, 65.0]), (8, [40.0, 65.0])] {
        assert!(close(world(&ed, aid), p), "hole anchor {aid} moved to {p:?}, got {:?}", world(&ed, aid));
    }
    for (aid, p) in [(1, [0.0, 0.0]), (2, [100.0, 0.0]), (3, [100.0, 100.0]), (4, [0.0, 100.0])] {
        assert!(close(world(&ed, aid), p), "outer anchor {aid} must not move");
    }
    set_bounds(&mut ed, None, None, Some(60.0), None, 0.0, 0.0); // W 30 → 60 about the left edge
    assert!(close(world(&ed, 6), [100.0, 35.0]) && close(world(&ed, 5), [40.0, 35.0]), "W scales the hole");

    ed.undo();
    ed.undo();
    assert!(close(world(&ed, 5), [30.0, 30.0]), "undo restores the hole");
    assert!(ed.selected.contains(&5), "…and keeps the hole anchor selected");
}

#[test]
fn mixed_outer_and_hole_selection_moves_both_as_one_step() {
    let mut ed = donut();
    ed.selected.extend([3, 5]); // outer BR (100,100) + hole TL (30,30)
    assert_eq!(ed.direct_label().as_deref(), Some("2 anchors"));
    let (x0, y0, x1, y1) = ed.direct_bbox().unwrap();
    assert!(close([x0, y0], [30.0, 30.0]) && close([x1, y1], [100.0, 100.0]), "bbox spans both rings");

    let rev = ed.rev;
    set_bounds(&mut ed, Some(40.0), Some(45.0), None, None, 0.0, 0.0);
    assert_eq!(ed.rev, rev + 1, "ONE undo step for the pair");
    assert!(close(world(&ed, 5), [40.0, 45.0]), "hole anchor moved, got {:?}", world(&ed, 5));
    assert!(close(world(&ed, 3), [110.0, 115.0]), "outer anchor moved, got {:?}", world(&ed, 3));
    assert!(close(world(&ed, 6), [60.0, 30.0]) && close(world(&ed, 1), [0.0, 0.0]), "others untouched");
    ed.undo();
    assert!(close(world(&ed, 5), [30.0, 30.0]) && close(world(&ed, 3), [100.0, 100.0]), "one undo restores both");
}

#[test]
fn hole_anchor_selection_promotes_to_its_object_on_v() {
    let mut ed = donut();
    ed.selected.insert(7);
    ed.set_tool(ToolKind::Object);
    assert!(ed.objsel.contains(&10), "A→V promotes a hole anchor to its path, like an outer one");
}
