//! Direct-tool segment grab: clicking on a path's OUTLINE (curve or straight, filled or not) must
//! grab that segment and select its two bordering anchors so their handles appear (Illustrator feel).
//! Pure dispatch logic, no UI — localises "I grabbed the path but no handles showed up".

use varos_core::editor::{Drag, Editor, ToolKind};
use varos_core::model::{Anchor, Path};

fn anc(id: u32, x: f32, y: f32, hin: Option<[f32; 2]>, hout: Option<[f32; 2]>) -> Anchor {
    Anchor { id, p: [x, y], hin, hout, smooth: hin.is_some() && hout.is_some() }
}

/// An OPEN, UNFILLED curve a→b that bulges down — like a pen-drawn stroke. Mid-segment ≈ [50,30].
fn curve_editor() -> Editor {
    let mut ed = Editor::new();
    let a = anc(1, 0.0, 0.0, None, Some([30.0, 40.0]));
    let b = anc(2, 100.0, 0.0, Some([70.0, 40.0]), None);
    ed.doc.paths.push(Path::new(10, vec![a, b], false, None, None, 1.0));
    ed.doc.ids = 2;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Direct);
    ed
}

#[test]
fn grab_curved_segment_selects_both_anchors() {
    let mut ed = curve_editor();
    ed.pointer_down([50.0, 30.0]); // on the curve, far from either anchor (>12)
    assert!(matches!(ed.drag, Drag::Segment { pid: 10, .. }), "grabbing the curve should start a Segment drag");
    assert!(
        ed.selected.contains(&1) && ed.selected.contains(&2),
        "both bordering anchors must be selected so their handles show, got {:?}",
        ed.selected
    );
}

#[test]
fn grab_straight_segment_of_rect_selects_both_corners() {
    // a filled rectangle: clicking the middle of an EDGE (not a corner) grabs that edge's two corners.
    let mut ed = Editor::new();
    let a = anc(1, 0.0, 0.0, None, None);
    let b = anc(2, 100.0, 0.0, None, None);
    let c = anc(3, 100.0, 80.0, None, None);
    let d = anc(4, 0.0, 80.0, None, None);
    ed.doc.paths.push(Path::new(10, vec![a, b, c, d], true, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Direct);
    ed.pointer_down([50.0, 0.0]); // middle of the TOP edge, 50 from each corner (>12) → segment, not anchor
    assert!(matches!(ed.drag, Drag::Segment { pid: 10, .. }), "grabbing the top edge should start a Segment drag");
    assert!(
        ed.selected.contains(&1) && ed.selected.contains(&2),
        "top-edge corners must be selected, got {:?}",
        ed.selected
    );
}

#[test]
fn marquee_across_a_segment_selects_its_endpoints() {
    // Illustrator: a marquee that catches NO anchor but CROSSES a curved segment selects that segment's
    // two endpoint anchors (so their handles appear). Curve a→b bulges down; mid ≈ [50,45].
    let mut ed = Editor::new();
    let a = anc(1, 0.0, 0.0, None, Some([33.0, 60.0]));
    let b = anc(2, 100.0, 0.0, Some([66.0, 60.0]), None);
    ed.doc.paths.push(Path::new(10, vec![a, b], false, None, None, 1.0));
    ed.doc.ids = 2;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Direct);
    ed.pointer_down([200.0, 200.0]); // empty space → marquee
    assert!(matches!(ed.drag, Drag::Marquee { .. }), "empty-space drag should be a marquee");
    ed.pointer_move([40.0, 40.0]); // rect [40,40]-[200,200]: contains the curve mid, neither anchor
    assert!(
        ed.selected.contains(&1) && ed.selected.contains(&2),
        "marquee crossing the curve must select both endpoints, got {:?}",
        ed.selected
    );
}

#[test]
fn tight_marquee_on_one_anchor_selects_only_it() {
    // ANCHOR PRIORITY: a small marquee around a single corner must select ONLY that corner — never its two
    // neighbours via the adjacent edges (the bug Ahmed flagged: "selecting a point auto-selects the two beside it").
    let mut ed = Editor::new();
    let a = anc(1, 0.0, 0.0, None, None);
    let b = anc(2, 100.0, 0.0, None, None);
    let c = anc(3, 100.0, 80.0, None, None);
    let d = anc(4, 0.0, 80.0, None, None);
    ed.doc.paths.push(Path::new(10, vec![a, b, c, d], true, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Direct);
    ed.pointer_down([110.0, 10.0]); // empty (14px from corner 2, >12) → marquee
    assert!(matches!(ed.drag, Drag::Marquee { .. }), "should be a marquee");
    ed.pointer_move([90.0, -10.0]); // rect [90,-10]-[110,10]: contains ONLY corner 2
    assert!(ed.selected.contains(&2), "the captured corner must be selected");
    assert!(
        !ed.selected.contains(&1) && !ed.selected.contains(&3),
        "neighbours must NOT be selected, got {:?}",
        ed.selected
    );
}

#[test]
fn hover_alone_does_not_select_a_path() {
    // Hovering must NOT reveal anchors — only real selection does (Illustrator). path_selected ignores hover.
    let mut ed = Editor::new();
    let a = anc(1, 0.0, 0.0, None, None);
    let b = anc(2, 100.0, 0.0, None, None);
    let c = anc(3, 100.0, 80.0, None, None);
    let d = anc(4, 0.0, 80.0, None, None);
    ed.doc.paths.push(Path::new(10, vec![a, b, c, d], true, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Direct);
    ed.hover_path = Some(10);
    assert!(!ed.path_selected(10), "hover must not count as selection (no anchors on hover)");
    assert!(ed.path_shown(10), "hover may still highlight the outline (path_shown true)");
}
