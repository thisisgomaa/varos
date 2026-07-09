//! A7 Stages 4–7 — rotation is a LIVE, persistent per-object transform, consistent across every
//! subsystem (render is proven by the scene/golden tests; here we prove hit-test, frame, snap/marquee,
//! the panel readout, Expand and the composition math). Pure logic, no GPU (the math-test rule).
//!
//! THE TRIPWIRE (`rotated_object_is_consistent_everywhere`) is the split-brain guard: one rotated fixture,
//! cross-checked so that what's DRAWN, what's HIT, what the panel READS and what Expand BAKES all agree.

use varos_core::editor::{Editor, ToolKind};
use varos_core::geom::{rotate_about, Pt};
use varos_core::model::{Anchor, Path, Xform};

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
