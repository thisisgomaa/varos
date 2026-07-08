//! Input-feel logic — pure math / state, no UI or GPU (per the no-Renderer rule). Covers:
//!  • A10 — a drawing tool snaps its FIRST point (and shows a phantom target) before any anchor exists.
//!  • A12 — the Properties "Constrain W/H" lock holds the aspect ratio of a canvas scale drag, like Shift.
//! Everything drives the real `pointer_down/move/up` engine so the tests exercise the shipped path.

use varos_core::editor::{Drag, Editor, SnapGuide, ToolKind};
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
/// One 100×100 square with corner anchors 1..=4 at [0,0],[100,0],[100,100],[0,100]; zoom 1 (tol = 8 world).
fn one_rect() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.push(rect(10, [1, 2, 3, 4], 0.0, 0.0, 100.0, 100.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed
}

// ───────────────────────── A10: start-point snap ─────────────────────────

#[test]
fn shape_first_corner_snaps_to_a_nearby_anchor() {
    // Rectangle tool: press ~3.6px from the existing square's top-right corner [100,0]. Before A10 the
    // start corner landed on the raw cursor; now it snaps onto the anchor, so the new shape aligns.
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Rect);
    ed.pointer_down([103.0, -2.0]);
    match &ed.drag {
        Drag::Shape { start, .. } => assert!(
            (start[0] - 100.0).abs() < 0.01 && start[1].abs() < 0.01,
            "the shape's first corner should snap onto [100,0], got {:?}",
            start
        ),
        _ => panic!("Rect press should open a Shape drag"),
    }
    // and the freshly created path is seeded at the snapped corner, not the raw cursor
    let p = ed.doc.paths.last().unwrap();
    assert!((p.anchors[0].p[0] - 100.0).abs() < 0.01 && p.anchors[0].p[1].abs() < 0.01);
}

#[test]
fn shape_first_corner_stays_free_when_far() {
    // No target within tolerance → the corner lands exactly where pressed (no phantom pull).
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Rect);
    ed.pointer_down([500.0, 500.0]);
    match &ed.drag {
        Drag::Shape { start, .. } => {
            assert!(
                (start[0] - 500.0).abs() < 0.01 && (start[1] - 500.0).abs() < 0.01,
                "no snap when far, got {:?}",
                start
            )
        }
        _ => panic!("expected a Shape drag"),
    }
}

#[test]
fn pen_hover_shows_phantom_point_before_first_click() {
    // Pen, no path yet: hovering near the corner must surface the phantom target point so the user sees
    // where the first click will land (A10 hover feedback). No press has happened.
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Pen);
    ed.pointer_move([103.0, -2.0]);
    assert!(matches!(ed.drag, Drag::None), "hover must not start a drag");
    let point = ed.snap_guides.iter().find_map(|g| match g {
        SnapGuide::Point { p } => Some(*p),
        _ => None,
    });
    let p = point.expect("a phantom snap point should appear on hover near a target");
    assert!((p[0] - 100.0).abs() < 0.01 && p[1].abs() < 0.01, "phantom should sit on [100,0], got {:?}", p);
}

#[test]
fn pen_hover_over_empty_space_shows_no_phantom() {
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Pen);
    ed.pointer_move([500.0, 500.0]);
    assert!(ed.snap_guides.is_empty(), "far hover should show no snap feedback");
    assert!(ed.snap_hud.is_none());
}

#[test]
fn pen_first_click_lands_on_the_snapped_point() {
    // The placed point must match the phantom: press near [100,0] → the new anchor is exactly on it.
    let mut ed = one_rect();
    ed.set_tool(ToolKind::Pen);
    ed.pointer_down([103.0, -2.0]);
    let active = ed.active.expect("pen press starts a path");
    let placed = ed.doc.paths.iter().find(|p| p.id == active).unwrap().anchors.last().unwrap().p;
    assert!(
        (placed[0] - 100.0).abs() < 0.01 && placed[1].abs() < 0.01,
        "first pen point should snap onto [100,0], got {:?}",
        placed
    );
}

// ───────────────────────── A12: canvas constrain lock ─────────────────────────

/// A selected square on the Object tool, snapping OFF so only the aspect logic is under test.
fn scale_setup(constrain: bool, shift: bool) -> Editor {
    let mut ed = one_rect();
    ed.doc.snap.enabled = false; // isolate the aspect-lock from geometry snapping
    ed.set_tool(ToolKind::Object);
    ed.objsel.insert(10);
    ed.constrain_wh = constrain;
    ed.mods.shift = shift;
    // grab the bottom-right transform handle [100,100]; pivot = opposite corner [0,0]
    ed.pointer_down([100.0, 100.0]);
    assert!(matches!(ed.drag, Drag::Scale { .. }), "pressing the corner handle must start a Scale drag");
    ed
}

#[test]
fn scale_drag_with_constrain_locks_aspect() {
    let mut ed = scale_setup(true, false); // constrain ON, no Shift
                                           // drag the corner to [140,110] — 1.4× wide, 1.1× tall if free
    ed.pointer_move([140.0, 110.0]);
    let p3 = ed.doc.anchor(3).unwrap().p; // far corner, was [100,100]
    assert!(
        (p3[0] - 140.0).abs() < 0.5 && (p3[1] - 140.0).abs() < 0.5,
        "constrain should lock to the larger axis (1.4×) → far corner ~[140,140], got {:?}",
        p3
    );
    ed.pointer_up();
}

#[test]
fn scale_drag_without_constrain_is_free() {
    let mut ed = scale_setup(false, false); // no constrain, no Shift
    ed.pointer_move([140.0, 110.0]);
    let p3 = ed.doc.anchor(3).unwrap().p;
    assert!(
        (p3[0] - 140.0).abs() < 0.5 && (p3[1] - 110.0).abs() < 0.5,
        "without the lock the drag scales each axis freely → ~[140,110], got {:?}",
        p3
    );
    ed.pointer_up();
}

// ───────────────────────── A9: Space repositions during placement ─────────────────────────

/// (x0,y0,x1,y1) bbox of the last path's anchor points.
fn last_bbox(ed: &Editor) -> (f32, f32, f32, f32) {
    let p = ed.doc.paths.last().unwrap();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for a in &p.anchors {
        x0 = x0.min(a.p[0]);
        y0 = y0.min(a.p[1]);
        x1 = x1.max(a.p[0]);
        y1 = y1.max(a.p[1]);
    }
    (x0, y0, x1, y1)
}

#[test]
fn space_repositions_a_shape_in_progress() {
    let mut ed = Editor::new();
    ed.doc.snap.enabled = false;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Rect);
    ed.pointer_down([10.0, 10.0]); // start corner
    ed.pointer_move([60.0, 40.0]); // 50×30 rect from [10,10]
    let (x0, y0, x1, y1) = last_bbox(&ed);
    assert!((x0 - 10.0).abs() < 0.5 && (y0 - 10.0).abs() < 0.5 && (x1 - 60.0).abs() < 0.5 && (y1 - 40.0).abs() < 0.5);

    // hold Space and move +30 in x → the WHOLE rect slides, its 50×30 size unchanged
    ed.space = true;
    ed.pointer_move([90.0, 40.0]);
    let (x0, y0, x1, y1) = last_bbox(&ed);
    assert!(
        (x0 - 40.0).abs() < 0.5 && (y0 - 10.0).abs() < 0.5 && (x1 - 90.0).abs() < 0.5 && (y1 - 40.0).abs() < 0.5,
        "Space should translate the rect to [40,10]..[90,40] (size preserved), got {:?}",
        (x0, y0, x1, y1)
    );

    // release Space and keep dragging → it resumes SIZING from the repositioned corner [40,10]
    ed.space = false;
    ed.pointer_move([100.0, 60.0]);
    let (x0, y0, x1, y1) = last_bbox(&ed);
    assert!(
        (x0 - 40.0).abs() < 0.5 && (y0 - 10.0).abs() < 0.5 && (x1 - 100.0).abs() < 0.5 && (y1 - 60.0).abs() < 0.5,
        "after Space the drag grows from [40,10] to the cursor, got {:?}",
        (x0, y0, x1, y1)
    );
    ed.pointer_up();
}

#[test]
fn space_repositions_a_new_pen_anchor() {
    let mut ed = Editor::new();
    ed.doc.snap.enabled = false;
    ed.ppu = 1.0;
    ed.set_tool(ToolKind::Pen);
    ed.pointer_down([20.0, 20.0]); // place the first anchor (mouse still down)
    let aid = ed.doc.paths.last().unwrap().anchors.last().unwrap().id;
    ed.space = true;
    ed.pointer_move([50.0, 30.0]); // +30,+10 → the anchor follows
    let p = ed.doc.anchor(aid).unwrap().p;
    assert!(
        (p[0] - 50.0).abs() < 0.5 && (p[1] - 30.0).abs() < 0.5,
        "Space should carry the new pen anchor to [50,30], got {:?}",
        p
    );
    ed.space = false;
    ed.pointer_up();
}

#[test]
fn scale_drag_shift_still_locks_independently() {
    // Shift must keep working on its own, even with the constrain flag off.
    let mut ed = scale_setup(false, true);
    ed.pointer_move([140.0, 110.0]);
    let p3 = ed.doc.anchor(3).unwrap().p;
    assert!(
        (p3[0] - 140.0).abs() < 0.5 && (p3[1] - 140.0).abs() < 0.5,
        "Shift alone should still lock aspect → ~[140,140], got {:?}",
        p3
    );
    ed.pointer_up();
}
