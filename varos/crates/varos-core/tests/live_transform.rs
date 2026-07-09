//! A7 Stages 4–7 — rotation is a LIVE, persistent per-object transform, consistent across every
//! subsystem (render is proven by the scene/golden tests; here we prove hit-test, frame, snap/marquee,
//! the panel readout, Expand and the composition math). Pure logic, no GPU (the math-test rule).
//!
//! THE TRIPWIRE (`rotated_object_is_consistent_everywhere`) is the split-brain guard: one rotated fixture,
//! cross-checked so that what's DRAWN, what's HIT, what the panel READS and what Expand BAKES all agree.

use varos_core::editor::{Editor, ToolKind};
use varos_core::geom::{rotate_about, Pt};
use varos_core::model::{Anchor, Artboard, Path, Xform};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
/// A `w`×`h` filled rectangle at (x,y), path id 10, anchor ids 1..4.
fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    Path::new(
        10,
        vec![anc(1, x, y), anc(2, x + w, y), anc(3, x + w, y + h), anc(4, x, y + h)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}
/// Editor holding one selected 100×40 rect at the origin (unit id 10, leaf node materialised).
fn sel_rect() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.push(rect(0.0, 0.0, 100.0, 40.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.doc.snap.enabled = false; // deterministic geometry in the transform tests
    ed.doc.sync_tree(); // materialise the leaf node so `unit_of(10)` resolves
    ed.objsel.insert(10);
    ed.set_tool(ToolKind::Object);
    ed
}
fn unit(ed: &Editor) -> u32 {
    ed.doc.unit_of(10).unwrap()
}
fn local_anchors(ed: &Editor) -> Vec<Pt> {
    ed.doc.paths[0].anchors.iter().map(|a| a.p).collect()
}

// ───────────────────────── Stage 4: rotate WRITES the transform, not baked geometry ─────────────────────────

#[test]
fn rotate_composes_into_xform_anchors_unchanged_in_local() {
    let mut ed = sel_rect();
    let before = local_anchors(&ed);
    ed.set_obj_rotation(45.0);
    // the crux of A7: the stored (local) anchors are UNTOUCHED — rotation lives in the transform.
    assert_eq!(before, local_anchors(&ed), "rotate must NOT bake — local anchors stay put");
    let xf = ed.doc.node_xform(unit(&ed));
    assert!((xf.rot - 45f32.to_radians()).abs() < 1e-4, "the unit stores a live 45° rotation, got {}", xf.rot);
    // and the WORLD image equals the old baked geometry (corner [100,0] rotates about the centre [50,20]).
    let w2 = xf.apply(ed.doc.paths[0].anchors[1].p);
    let want = rotate_about([100.0, 0.0], [50.0, 20.0], 45f32.to_radians());
    assert!((w2[0] - want[0]).abs() < 1e-3 && (w2[1] - want[1]).abs() < 1e-3, "world corner {:?} vs {:?}", w2, want);
}

#[test]
fn rotation_persists_through_deselect_and_reselect() {
    let mut ed = sel_rect();
    ed.set_obj_rotation(30.0);
    assert!((ed.sel_stored_angle() - 30f32.to_radians()).abs() < 1e-4);
    // deselect everything, then re-select — the angle must come BACK (today it read 0: the A7 pain).
    ed.objsel.clear();
    ed.refresh_obj_angle();
    assert_eq!(ed.sel_stored_angle(), 0.0, "nothing selected → axis-aligned");
    ed.objsel.insert(10);
    ed.refresh_obj_angle();
    assert!((ed.sel_stored_angle() - 30f32.to_radians()).abs() < 1e-4, "reselect restores the stored angle");
    assert!((ed.obj_angle - 30f32.to_radians()).abs() < 1e-4, "the frame angle follows on reselect");
}

#[test]
fn rotate_drag_writes_a_live_transform() {
    // drive the real Rotate tool: pivot at [0,0], grab at 0° and drag to 90°.
    let mut ed = sel_rect();
    ed.set_tool(ToolKind::Rotate);
    ed.pointer_down([0.0, 0.0]);
    ed.pointer_up(); // click relocates the pivot to the corner
    ed.pointer_down([100.0, 0.0]);
    ed.pointer_move([0.0, 100.0]); // 0° → 90° about [0,0]
    ed.pointer_up();
    let xf = ed.doc.node_xform(unit(&ed));
    assert!(!xf.is_identity(), "the drag left a LIVE transform (not baked, not identity)");
    // stored anchors are still the ORIGINAL local rect (proof geometry was not baked)
    assert_eq!(local_anchors(&ed), vec![[0.0, 0.0], [100.0, 0.0], [100.0, 40.0], [0.0, 40.0]]);
    // world image of corner [100,0] rotated 90° about [0,0] is ~[0,100]
    let w = xf.apply([100.0, 0.0]);
    assert!(w[0].abs() < 0.5 && (w[1] - 100.0).abs() < 0.5, "world corner {:?}", w);
}

#[test]
fn move_keeps_rotation_live_and_translates_world() {
    let mut ed = sel_rect();
    ed.set_obj_rotation(30.0);
    let rot0 = ed.doc.node_xform(unit(&ed)).rot;
    let c0 = {
        let b = ed.obj_bbox().unwrap();
        [(b.0 + b.2) * 0.5, (b.1 + b.3) * 0.5]
    };
    // grab the shape (its centre [50,20] is the rotation pivot → still inside) and drag by (10,-5)
    ed.pointer_down([50.0, 20.0]);
    ed.pointer_move([60.0, 15.0]);
    ed.pointer_up();
    let rot1 = ed.doc.node_xform(unit(&ed)).rot;
    assert!((rot1 - rot0).abs() < 1e-5, "a move keeps the live rotation ({rot0} → {rot1})");
    let c1 = {
        let b = ed.obj_bbox().unwrap();
        [(b.0 + b.2) * 0.5, (b.1 + b.3) * 0.5]
    };
    assert!(
        (c1[0] - c0[0] - 10.0).abs() < 0.5 && (c1[1] - c0[1] + 5.0).abs() < 0.5,
        "world centre moved by the drag delta: {c0:?} → {c1:?}"
    );
}

// ───────────────────────── THE TRIPWIRE — one rotated fixture, every reader agrees ─────────────────────────

#[test]
fn rotated_object_is_consistent_everywhere() {
    let mut ed = sel_rect(); // 100×40 rect, centre [50,20]
    ed.set_obj_rotation(45.0);

    // (1) HIT-TEST: the drawn centre (the rotation pivot, unmoved) still hits the path.
    assert_eq!(ed.path_under([50.0, 20.0]), Some(10), "click at the drawn centre must hit the rotated path");
    // a point INSIDE the world AABB of the rotated shape but OUTSIDE the shape itself must MISS (proof the
    // hit-test is rotated, not reading local geometry): [50,65] sits in the AABB's top gap, off the diamond.
    assert_eq!(ed.path_under([50.0, 65.0]), None, "an AABB gap point (off the rotated diamond) is empty");

    // (2) WORLD BBOX matches the rotated extent (align/snap/board-membership box).
    let (x0, y0, x1, y1) = ed.obj_bbox().unwrap();
    let expect = (100.0 + 40.0) * std::f32::consts::FRAC_1_SQRT_2; // 45° AABB of a 100×40 rect
    assert!(
        (x1 - x0 - expect).abs() < 0.6 && (y1 - y0 - expect).abs() < 0.6,
        "world AABB {}×{} should be the rotated extent ≈ {expect}",
        x1 - x0,
        y1 - y0
    );

    // (3) PANEL reads the TRUE (local) W/H, not the world AABB.
    let (lw, lh) = ed.obj_local_dims().unwrap();
    assert!((lw - 100.0).abs() < 0.5 && (lh - 40.0).abs() < 0.5, "panel W/H = local dims, got {lw}×{lh}");

    // (4) the transform FRAME is oriented (corners are NOT axis-aligned) and hugs the world shape.
    let c = ed.frame_corners().unwrap();
    assert!((c[0][1] - c[1][1]).abs() > 1.0, "top edge of the frame is tilted (oriented, not axis-aligned)");
}

#[test]
fn marquee_selects_a_rotated_object_by_its_visual_bounds() {
    let mut ed = sel_rect();
    ed.set_obj_rotation(45.0);
    ed.objsel.clear();
    ed.refresh_obj_angle();
    // an object marquee sweeping the rotated shape's WORLD bounds must catch it.
    ed.pointer_down([-60.0, -60.0]); // empty space (outside the rotated rect)
    ed.pointer_move([160.0, 160.0]);
    ed.pointer_up();
    assert!(ed.objsel.contains(&10), "marquee over the visual bounds selects the rotated object");
}

// ───────────────────────── Stage 7: Expand bakes to identity, world-equivalent ─────────────────────────

#[test]
fn expand_bakes_to_identity_with_world_equivalent_geometry() {
    let mut ed = sel_rect();
    ed.set_obj_rotation(30.0);
    let xf = ed.doc.node_xform(unit(&ed));
    let world_before: Vec<Pt> = ed.doc.paths[0].anchors.iter().map(|a| xf.apply(a.p)).collect();
    ed.expand_transform();
    assert!(ed.doc.node_xform(unit(&ed)).is_identity(), "Expand resets the transform to identity");
    for (b, a) in world_before.iter().zip(ed.doc.paths[0].anchors.iter()) {
        assert!(
            (b[0] - a.p[0]).abs() < 1e-3 && (b[1] - a.p[1]).abs() < 1e-3,
            "after Expand the stored anchors ARE the world coordinates: {b:?} vs {:?}",
            a.p
        );
    }
    // idempotent: expanding an identity selection does nothing (no crash, still identity).
    ed.expand_transform();
    assert!(ed.doc.node_xform(unit(&ed)).is_identity());
}

#[test]
fn rotate_back_to_zero_bakes_cleanly_to_identity() {
    // the degenerate composition (total angle ≡ 0) must reduce to a pure translation → identity.
    let mut ed = sel_rect();
    ed.set_obj_rotation(30.0);
    assert!(!ed.doc.node_xform(unit(&ed)).is_identity());
    ed.set_obj_rotation(0.0); // back to axis-aligned
    assert!(ed.doc.node_xform(unit(&ed)).is_identity(), "rotating to 0° bakes the residual → identity");
    // geometry returned to the original axis-aligned rect (about its own centre → unmoved)
    let mut got = local_anchors(&ed);
    got.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut want = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 40.0], [0.0, 40.0]];
    want.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (g, w) in got.iter().zip(&want) {
        assert!((g[0] - w[0]).abs() < 1e-2 && (g[1] - w[1]).abs() < 1e-2, "back to origin rect: {g:?} vs {w:?}");
    }
}

// ───────────────────────── the composition math (pure, unit-level) ─────────────────────────

#[test]
fn then_rotate_composes_two_rotations_exactly() {
    let base = Xform { rot: 0.4, piv: [10.0, -5.0] };
    let about = [30.0, 20.0];
    let dtheta = 0.7;
    let composed = base.then_rotate(dtheta, about);
    // new.apply(p) must equal rotating base.apply(p) about `about` by dtheta, for arbitrary p.
    for p in [[0.0, 0.0], [100.0, 50.0], [-20.0, 80.0], [7.5, -12.25]] {
        let want = rotate_about(base.apply(p), about, dtheta);
        let got = composed.apply(p);
        assert!(
            (want[0] - got[0]).abs() < 2e-3 && (want[1] - got[1]).abs() < 2e-3,
            "compose mismatch for {p:?}: want {want:?} got {got:?}"
        );
    }
    assert!((composed.rot - (0.4 + 0.7)).abs() < 1e-5, "the composed angle is the sum");
}

#[test]
fn old_vrs_without_xform_loads_as_identity() {
    // a Node serialized WITHOUT the xform key (pre-A7 file) must deserialize to identity — the whole
    // suite proves un-rotated objects are byte-identical, this pins the serde default explicitly.
    let json = r#"{"id":5,"kind":{"Path":10},"name":"","parent":null,"children":[],
        "hidden":false,"locked":false,"color":null,"clip_exempt":false}"#;
    let n: varos_core::model::Node = serde_json::from_str(json).expect("legacy node loads");
    assert!(n.xform.is_identity(), "missing xform ⇒ identity (old files unchanged)");
}

// ───────────────────────── split-brain regressions (bake rotated units before world-space edits) ─────────────────────────

/// An OPEN 3-point path (id 10, anchors 1..3) — needed for the Pen resume/extend case.
fn sel_open_path() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.push(Path::new(
        10,
        vec![anc(1, 0.0, 0.0), anc(2, 100.0, 0.0), anc(3, 100.0, 60.0)],
        false, // open
        None,
        Some([0.0, 0.0, 0.0, 1.0]),
        2.0,
    ));
    ed.doc.ids = 3;
    ed.ppu = 1.0;
    ed.doc.snap.enabled = false;
    ed.doc.sync_tree();
    ed.objsel.insert(10);
    ed.set_tool(ToolKind::Object);
    ed
}

/// Two separate rects → two independent units (ids 10 & 20, anchors 1..4 and 5..8).
fn two_rects() -> Editor {
    let mut ed = Editor::new();
    let fill = Some([0.5, 0.5, 0.5, 1.0]);
    ed.doc.paths.push(Path::new(
        10,
        vec![anc(1, 0.0, 0.0), anc(2, 80.0, 0.0), anc(3, 80.0, 40.0), anc(4, 0.0, 40.0)],
        true,
        fill,
        None,
        1.0,
    ));
    ed.doc.paths.push(Path::new(
        20,
        vec![anc(5, 150.0, 0.0), anc(6, 230.0, 0.0), anc(7, 230.0, 40.0), anc(8, 150.0, 40.0)],
        true,
        fill,
        None,
        1.0,
    ));
    ed.doc.ids = 8;
    ed.ppu = 1.0;
    ed.doc.snap.enabled = false;
    ed.doc.sync_tree();
    ed.set_tool(ToolKind::Object);
    ed
}

#[test]
fn convert_handle_on_a_rotated_object_is_one_to_one() {
    // Bug 1: the Convert tool pulls a handle to the WORLD cursor. On a rotated unit the write must land the
    // handle exactly under the cursor (bake first) — not double-transformed.
    let mut ed = sel_rect();
    ed.set_obj_rotation(45.0);
    let a0_world = ed.doc.unit_xform(10).apply([0.0, 0.0]); // corner anchor id 1, in world
    ed.set_tool(ToolKind::Convert);
    ed.pointer_down(a0_world); // grabs the corner → Drag::ConvPull (after baking the unit)
    let target = [a0_world[0] + 25.0, a0_world[1] - 12.0];
    ed.pointer_move(target); // pull the out-handle to `target`
    ed.pointer_up();
    let hout = ed.doc.anchor(1).and_then(|a| a.hout).expect("convert pulled an out handle");
    let hout_world = ed.doc.unit_xform(10).apply(hout);
    assert!(
        (hout_world[0] - target[0]).abs() < 0.5 && (hout_world[1] - target[1]).abs() < 0.5,
        "the pulled handle sits under the cursor (1:1): world {hout_world:?} vs cursor {target:?}"
    );
}

#[test]
fn pen_resume_on_a_rotated_object_places_the_anchor_at_the_cursor() {
    // Bug 2: resuming a rotated open path and clicking a new point must place that anchor AT the cursor —
    // the raw world click must not be stored as a rotated-local coordinate.
    let mut ed = sel_open_path();
    ed.set_obj_rotation(30.0);
    // drop the object selection and enter the Pen with nothing active (so the endpoint click RESUMES).
    ed.objsel.clear();
    ed.selected.clear();
    ed.active = None;
    ed.refresh_obj_angle();
    ed.set_tool(ToolKind::Pen);
    let end_world = ed.doc.unit_xform(10).apply([100.0, 60.0]); // the open path's endpoint (anchor 3)
    ed.pointer_move(end_world); // hover the endpoint so the Pen "sees" it (path_shown gate), as in the app
    ed.pointer_down(end_world);
    ed.pointer_up(); // resume(10, 3) — bakes the unit, makes it active
    let click = [200.0, 150.0]; // a fresh world point, clear of the shape
    ed.pointer_down(click);
    ed.pointer_up(); // extend: push a new anchor
    let newp = ed.doc.paths[0].anchors.last().unwrap().p;
    let world = ed.doc.unit_xform(10).apply(newp);
    assert!(
        (world[0] - click[0]).abs() < 0.5 && (world[1] - click[1]).abs() < 0.5,
        "the extended anchor lands under the cursor: world {world:?} vs click {click:?}"
    );
}

#[test]
fn multi_unit_rotated_marquee_drag_moves_every_anchor_correctly() {
    // Bug 3: a Direct-tool marquee can select anchors across SEVERAL rotated units. Dragging one must move
    // every selected anchor by the same WORLD delta — the follow-up drag must bake ALL spanned units, not
    // just the grabbed one (else the other units' anchors move by a rotated delta = split-brain).
    let mut ed = two_rects();
    ed.objsel.clear();
    ed.objsel.insert(10);
    ed.refresh_obj_angle();
    ed.set_obj_rotation(30.0); // rotate unit 10
    ed.objsel.clear();
    ed.objsel.insert(20);
    ed.refresh_obj_angle();
    ed.set_obj_rotation(60.0); // rotate unit 20 by a DIFFERENT angle

    // marquee-select every anchor of both rotated rects.
    ed.objsel.clear();
    ed.selected.clear();
    ed.set_tool(ToolKind::Direct);
    ed.pointer_down([-200.0, -200.0]);
    ed.pointer_move([400.0, 400.0]);
    ed.pointer_up();
    let units_in_sel: std::collections::HashSet<u32> =
        ed.selected.iter().filter_map(|&a| ed.doc.pid_of_anchor(a).and_then(|p| ed.doc.unit_of(p))).collect();
    assert!(units_in_sel.len() >= 2, "the marquee spans multiple units (the split-brain condition)");

    // record every selected anchor's WORLD position, then drag one grabbed anchor by a delta.
    let world_before: Vec<(u32, Pt)> = ed
        .selected
        .iter()
        .map(|&aid| {
            let pid = ed.doc.pid_of_anchor(aid).unwrap();
            (aid, ed.doc.unit_xform(pid).apply(ed.doc.anchor(aid).unwrap().p))
        })
        .collect();
    let grab = ed.doc.unit_xform(10).apply(ed.doc.anchor(1).unwrap().p); // anchor 1 of unit 10, in world
    let delta = [17.0, -23.0];
    ed.pointer_down(grab);
    ed.pointer_move([grab[0] + delta[0], grab[1] + delta[1]]);
    ed.pointer_up();

    for (aid, wb) in &world_before {
        let pid = ed.doc.pid_of_anchor(*aid).unwrap();
        let wa = ed.doc.unit_xform(pid).apply(ed.doc.anchor(*aid).unwrap().p);
        assert!(
            (wa[0] - wb[0] - delta[0]).abs() < 0.5 && (wa[1] - wb[1] - delta[1]).abs() < 0.5,
            "anchor {aid} world moved {wb:?}→{wa:?}, want +{delta:?} (split-brain across units?)"
        );
    }
}

#[test]
fn panel_xy_with_a_nontop_left_refpoint_on_a_rotated_object_reads_and_writes() {
    // Bug 4: the panel X/Y reference-point offset must use the WORLD AABB dims (not the local W/H). Read and
    // write must agree: typing back the displayed value is a no-op, and a delta moves the object by it.
    let mut ed = sel_rect(); // 100×40
    ed.set_obj_rotation(45.0);
    let (ax, ay) = (1.0, 1.0); // bottom-right reference point

    let (x0, y0, x1, y1) = ed.obj_bbox().unwrap();
    let (lw, lh) = ed.obj_local_dims().unwrap();
    // the world AABB dims differ from the local W/H when rotated (so `s.x + ax*local_w` would be wrong).
    assert!(((x1 - x0) - lw).abs() > 1.0 && ((y1 - y0) - lh).abs() > 1.0, "world dims differ from local W/H");
    // READ: the reference point = world AABB bottom-right (offset by the WORLD dims).
    let refp = ed.obj_ref_xy(ax, ay).unwrap();
    assert!((refp[0] - x1).abs() < 1e-3 && (refp[1] - y1).abs() < 1e-3, "refpoint reads the world BR corner");

    // WRITE round-trip: re-entering the shown value must NOT move the object (read & write agree).
    ed.set_obj_bbox(Some(refp[0]), Some(refp[1]), None, None, ax, ay);
    let b2 = ed.obj_bbox().unwrap();
    assert!(
        (b2.0 - x0).abs() < 0.2 && (b2.1 - y0).abs() < 0.2 && (b2.2 - x1).abs() < 0.2 && (b2.3 - y1).abs() < 0.2,
        "typing the shown X/Y back is a no-op: {:?} vs {b2:?}",
        (x0, y0, x1, y1)
    );
    assert!((ed.doc.node_xform(unit(&ed)).rot - 45f32.to_radians()).abs() < 1e-4, "X/Y edit keeps the live rotation");

    // WRITE move: nudging the BR reference point by (+10,+6) shifts the whole world AABB by that.
    ed.set_obj_bbox(Some(refp[0] + 10.0), Some(refp[1] + 6.0), None, None, ax, ay);
    let b3 = ed.obj_bbox().unwrap();
    assert!(
        (b3.0 - (x0 + 10.0)).abs() < 0.3 && (b3.1 - (y0 + 6.0)).abs() < 0.3,
        "editing the BR reference point translates the object by the delta: {b3:?}"
    );
}

#[test]
fn rotate_drag_through_zero_keeps_rotating() {
    // Bug 5: during ONE rotate drag, an intermediate frame that hits total-angle ≡ 0 must NOT bake the
    // residual (which permanently loses the live rotation). Pre-rotate 90° about the centre, then drag about
    // a DIFFERENT pivot so the total angle passes through 0 mid-gesture and continues to −45°.
    let mut ed = sel_rect(); // 100×40, centre [50,20]
    ed.set_obj_rotation(90.0);
    ed.set_tool(ToolKind::Rotate);
    ed.pivot = Some([0.0, 0.0]); // rotate about the corner (≠ the 90° pre-rotation centre)
    ed.pointer_down([100.0, 0.0]); // start angle 0 about [0,0]
    ed.pointer_move([0.0, -100.0]); // d = −90° → TOTAL = 0 (the degenerate crossing, mid-drag)
    ed.pointer_move([-70.71, -70.71]); // d = −135° → TOTAL = −45° (kept rotating past zero)
    ed.pointer_up();

    // the geometry was NOT baked mid-drag (anchors are still the original local rect)…
    assert_eq!(
        local_anchors(&ed),
        vec![[0.0, 0.0], [100.0, 0.0], [100.0, 40.0], [0.0, 40.0]],
        "anchors stay the original local rect — no mid-drag bake"
    );
    // …and the live world image equals the exact composition base(90°@centre) ∘ (−135°@corner).
    let xf = ed.doc.node_xform(unit(&ed));
    assert!(!xf.is_identity(), "still live-rotated after crossing 0°");
    for lp in [[0.0, 0.0], [100.0, 0.0], [100.0, 40.0], [0.0, 40.0]] {
        let want = rotate_about(rotate_about(lp, [50.0, 20.0], 90f32.to_radians()), [0.0, 0.0], -135f32.to_radians());
        let got = xf.apply(lp);
        assert!(
            (got[0] - want[0]).abs() < 0.3 && (got[1] - want[1]).abs() < 0.3,
            "corner {lp:?}: live world {got:?} vs composed {want:?} (rotation lost at the zero-cross?)"
        );
    }
}

// ───────────────────── final split-brain closure — artboard-move · nudge · pen-segment-insert ─────────────────────

#[test]
fn moving_an_artboard_carries_its_rotated_art_one_to_one() {
    // Bug 6: with "move artwork with artboard" on, traveling art is captured as LOCAL anchors and translated
    // by the world delta. On a ROTATED unit that separates the art from its page (it moves by R·d, not d).
    // The page-move must carry the rotated unit's pivot by the SAME d (as `Drag::Object` does) so the art
    // tracks the page exactly and keeps its live rotation.
    let mut ed = Editor::new();
    ed.ppu = 1.0;
    ed.doc.snap.enabled = false;
    ed.doc.artboards = vec![Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, name: "A".into(), ..Artboard::default() }];
    assert!(ed.doc.move_art_with_ab, "precondition: move-art-with-artboard is on by default");
    // a 40×20 rect fully inside page A, at (30,30)
    ed.doc.paths.push(Path::new(
        10,
        vec![anc(1, 30.0, 30.0), anc(2, 70.0, 30.0), anc(3, 70.0, 50.0), anc(4, 30.0, 50.0)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    ));
    ed.doc.ids = 4;
    ed.doc.sync_tree();
    // rotate the object 90° about its own centre (a LIVE transform)
    ed.objsel.insert(10);
    ed.set_tool(ToolKind::Object);
    ed.set_obj_rotation(90.0);
    let u = ed.doc.unit_of(10).unwrap();
    let rot0 = ed.doc.node_xform(u).rot;
    assert!(!ed.doc.node_xform(u).is_identity(), "the object is live-rotated before the page move");
    let world_before: Vec<Pt> = (1..=4).map(|aid| ed.doc.unit_xform(10).apply(ed.doc.anchor(aid).unwrap().p)).collect();

    // switch to the Artboard tool and drag page A by (100,0)
    ed.objsel.clear();
    ed.set_tool(ToolKind::Artboard);
    ed.pointer_down([50.0, 50.0]); // inside page A's body → selects it and begins the move (captures the art)
    ed.pointer_move([150.0, 50.0]); // drag +100 in x
    ed.pointer_up();

    assert!((ed.doc.artboards[0].x - 100.0).abs() < 1e-3, "the page moved by (100,0)");
    // every anchor's WORLD position moved by EXACTLY (100,0) — not (0,-100) = R·d
    for (aid, wb) in (1..=4).zip(&world_before) {
        let wa = ed.doc.unit_xform(10).apply(ed.doc.anchor(aid).unwrap().p);
        assert!(
            (wa[0] - wb[0] - 100.0).abs() < 0.5 && (wa[1] - wb[1]).abs() < 0.5,
            "anchor {aid} world {wb:?}→{wa:?}, want +[100,0] (art dropped off its page?)"
        );
    }
    let rot1 = ed.doc.node_xform(ed.doc.unit_of(10).unwrap()).rot;
    assert!((rot1 - rot0).abs() < 1e-5, "the page move keeps the object's live rotation ({rot0} → {rot1})");
}

#[test]
fn nudge_moves_rotated_marquee_anchors_along_the_world_axis() {
    // Bug 7: a Direct-tool marquee selects anchors WITHOUT baking, so `nudge` writing a WORLD delta into
    // LOCAL storage slides a rotated unit's anchors along the rotated axis. `nudge` must bake every spanned
    // unit first (as `begin_anchor_drag` does) so the nudge is a true world translation.
    let mut ed = sel_rect(); // 100×40
    ed.set_obj_rotation(45.0);
    // Direct-tool marquee over the whole shape → selects the anchors, leaving the unit LIVE-rotated
    ed.objsel.clear();
    ed.selected.clear();
    ed.set_tool(ToolKind::Direct);
    ed.pointer_down([-200.0, -200.0]);
    ed.pointer_move([400.0, 400.0]);
    ed.pointer_up();
    assert!(!ed.selected.is_empty(), "the marquee selected anchors");
    assert!(
        !ed.doc.node_xform(unit(&ed)).is_identity(),
        "the marquee left the rotation live (unbaked) — the bug condition"
    );

    let world_before: Vec<(u32, Pt)> = ed
        .selected
        .iter()
        .map(|&aid| {
            let pid = ed.doc.pid_of_anchor(aid).unwrap();
            (aid, ed.doc.unit_xform(pid).apply(ed.doc.anchor(aid).unwrap().p))
        })
        .collect();
    ed.nudge(0.0, -1.0); // nudge up by one world unit
    for (aid, wb) in &world_before {
        let pid = ed.doc.pid_of_anchor(*aid).unwrap();
        let wa = ed.doc.unit_xform(pid).apply(ed.doc.anchor(*aid).unwrap().p);
        assert!(
            (wa[0] - wb[0]).abs() < 1e-3 && (wa[1] - wb[1] + 1.0).abs() < 1e-3,
            "anchor {aid} world {wb:?}→{wa:?}, want +[0,-1] (slid along the rotated axis?)"
        );
    }
}

#[test]
fn pen_click_on_a_rotated_segment_inserts_an_anchor_at_the_cursor() {
    // Bug 8: the Pen add-anchor-on-segment branch ran `nearest_seg` with a WORLD cursor against LOCAL
    // geometry, so clicking a rotated path's segment silently missed. Mapping the cursor into the unit's
    // local frame first makes the insert land under the click (keeps the path's live rotation).
    let mut ed = sel_rect(); // closed 100×40, anchors 1..4, objsel = {10} ⇒ editable
    ed.set_obj_rotation(45.0);
    ed.set_tool(ToolKind::Pen);
    let n_before = ed.doc.paths[0].anchors.len();
    // midpoint of the top edge (anchor 1 [0,0] → anchor 2 [100,0]) in LOCAL is [50,0]; click its WORLD image
    let click = ed.doc.unit_xform(10).apply([50.0, 0.0]);
    ed.pointer_down(click);
    ed.pointer_up();
    assert_eq!(ed.doc.paths[0].anchors.len(), n_before + 1, "clicking a rotated segment inserts an anchor");
    // the inserted anchor (the freshly-selected one) sits at the click in WORLD, near the edge midpoint
    let nid = *ed.selected.iter().next().expect("the inserted anchor is selected");
    let world = ed.doc.unit_xform(10).apply(ed.doc.anchor(nid).unwrap().p);
    assert!(
        (world[0] - click[0]).abs() < 0.5 && (world[1] - click[1]).abs() < 0.5,
        "the new anchor lands under the cursor: world {world:?} vs click {click:?}"
    );
    assert!(!ed.doc.node_xform(unit(&ed)).is_identity(), "the insert kept the path's live rotation (no bake)");
}
