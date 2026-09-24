//! QW1 (Astra F08) — a stroke is hit where it is PAINTED. Pure core, no UI / GPU.
//!  • click reach = `EDGE_R` SCREEN px (÷ ppu, zoom-constant) + half the painted stroke width;
//!  • the outline distance is measured to the TRUE curve, not to 25 samples on it;
//!  • a marquee must touch the painted band / fill — sitting in a hollow ring's hole catches nothing;
//!  • the Pen's add-anchor tolerance is screen px at every zoom (it used to be 8 WORLD units).
//!
//! Run with:  cargo test -p varos-core --test hit_thick_stroke

use varos_core::editor::{Drag, Editor, PenHint, TfHit, ToolKind, EDGE_R};
use varos_core::geom::{cubic, Pt, Rgba};
use varos_core::model::{Anchor, Path, K};

const INK: Rgba = [0.0, 0.0, 0.0, 1.0];
const GREY: Rgba = [0.5, 0.5, 0.5, 1.0];

fn corner(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn smooth(id: u32, p: Pt, hin: Pt, hout: Pt) -> Anchor {
    Anchor { id, p, hin: Some(hin), hout: Some(hout), smooth: true }
}
/// A 4-anchor bezier circle centred on (cx, cy) — the same construction the Ellipse tool uses.
fn circle(pid: u32, base: u32, c: Pt, r: f32, fill: Option<Rgba>, stroke: Option<Rgba>, sw: f32) -> Path {
    let (cx, cy, k) = (c[0], c[1], K * r);
    let anchors = vec![
        smooth(base, [cx, cy - r], [cx - k, cy - r], [cx + k, cy - r]),
        smooth(base + 1, [cx + r, cy], [cx + r, cy - k], [cx + r, cy + k]),
        smooth(base + 2, [cx, cy + r], [cx + k, cy + r], [cx - k, cy + r]),
        smooth(base + 3, [cx - r, cy], [cx - r, cy + k], [cx - r, cy - k]),
    ];
    Path::new(pid, anchors, true, fill, stroke, sw)
}
/// Corner-anchored rect over `[x0, y0, x1, y1]`, anchor ids 1..=4, stroke width 1.
fn rect(pid: u32, [x0, y0, x1, y1]: [f32; 4], fill: Option<Rgba>, stroke: Option<Rgba>) -> Path {
    let base = 1;
    Path::new(
        pid,
        vec![corner(base, x0, y0), corner(base + 1, x1, y0), corner(base + 2, x1, y1), corner(base + 3, x0, y1)],
        true,
        fill,
        stroke,
        1.0,
    )
}
fn editor_with(paths: Vec<Path>, ppu: f32) -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths = paths;
    ed.doc.ids = 1000;
    ed.ppu = ppu;
    ed
}
/// A point at `dist` from the centre along a 30° ray (between anchors, so it exercises a curve).
fn on_ray(dist: f32) -> Pt {
    let a = 30f32.to_radians();
    [dist * a.cos(), dist * a.sin()]
}

// ───────────────────────── click reach = painted band ─────────────────────────

#[test]
fn painted_band_click_selects_away_from_centreline() {
    // Astra's case: an 80-wide stroke on an r = 400 unfilled ring at 327 %. Reach = 8/3.27 + 40 ≈ 42.4.
    let ed = editor_with(vec![circle(1, 1, [0.0, 0.0], 400.0, None, Some(INK), 80.0)], 3.27);
    assert_eq!(ed.path_under(on_ray(435.0)), Some(1), "35 outside the centreline is on the painted band");
    assert_eq!(ed.path_under(on_ray(365.0)), Some(1), "…and so is 35 inside it");
    assert_eq!(ed.path_under(on_ray(445.0)), None, "45 outside is past the band + 8 screen px");
    assert_eq!(ed.path_under(on_ray(355.0)), None, "45 inside is in the ring's hollow → click-through");
    assert_eq!(ed.path_under([0.0, 0.0]), None, "the unfilled centre stays click-through");
}

#[test]
fn band_reach_is_screen_px_plus_half_width_at_any_zoom() {
    // The 8 px part is SCREEN px; the half-width part is WORLD (it is painted in world units).
    for ppu in [0.25f32, 1.0, 4.0, 40.0] {
        let ed = editor_with(vec![circle(1, 1, [0.0, 0.0], 400.0, None, Some(INK), 80.0)], ppu);
        let reach = EDGE_R / ppu + 40.0;
        assert_eq!(ed.path_under(on_ray(400.0 + reach * 0.98)), Some(1), "just inside the reach @ppu {ppu}");
        assert_eq!(ed.path_under(on_ray(400.0 + reach * 1.02)), None, "just past the reach @ppu {ppu}");
    }
}

#[test]
fn fill_only_shape_reach_unchanged() {
    // No painted stroke ⇒ no band, even if a stroke width is stored: reach stays EDGE_R / ppu.
    let mut p = rect(1, [0.0, 0.0, 100.0, 100.0], Some(GREY), None);
    p.stroke_width = 50.0;
    let ed = editor_with(vec![p], 2.0); // reach = 4 world
    assert_eq!(ed.path_under([103.9, 50.0]), Some(1), "3.9 outside the edge is within 8 screen px");
    assert_eq!(ed.path_under([104.1, 50.0]), None, "4.1 outside is not — the unpainted width adds nothing");
}

// ───────────────────────── true-curve distance ─────────────────────────

#[test]
fn centreline_between_flatten_samples_is_hit() {
    // Astra's W/H 20000 circle. The 25 samples per quarter sit ~654 units apart and the chord between two
    // of them cuts ~5 units inside the arc — both far more than the 8/3.27 + 0.5 ≈ 2.9 reach at 327 %.
    let r = 10_000.0;
    let ed = editor_with(vec![circle(1, 1, [0.0, 0.0], r, None, Some(INK), 1.0)], 3.27);
    let a = &ed.doc.paths[0].anchors;
    let (p0, p1, p2, p3) = (a[0].p, a[0].hout.unwrap(), a[1].hin.unwrap(), a[1].p);
    for k in [0usize, 5, 11, 23] {
        let t = (k as f32 + 0.5) / 24.0; // exactly midway between two samples
        let on_curve = cubic(p0, p1, p2, p3, t);
        let (si, st, d) = ed.doc.nearest_seg(0, on_curve).expect("a segment");
        assert_eq!(si, 0, "nearest segment for t = {t}");
        assert!(d < 0.05, "a point ON the drawn curve measures ~0 from it, got {d} (t = {t})");
        assert!((st - t).abs() < 1e-3, "t is recovered on the curve: {st} vs {t}");
        assert!(ed.doc.edge_dist(0, on_curve).is_some_and(|e| e < 0.05));
        assert_eq!(ed.path_under(on_curve), Some(1), "the centreline between samples is clickable (t = {t})");
    }
}

#[test]
fn big_circle_thick_band_between_samples_at_327_percent() {
    // Astra F08 item 14 pinned: the W/H 20000 circle with an 80-wide stroke at 327 %. Between two flatten
    // samples, 35 off the centreline (radially) is on the band; 45 off is past band + 8 screen px (≈42.4).
    let ed = editor_with(vec![circle(1, 1, [0.0, 0.0], 10_000.0, None, Some(INK), 80.0)], 3.27);
    let a = &ed.doc.paths[0].anchors;
    let (p0, p1, p2, p3) = (a[1].p, a[1].hout.unwrap(), a[2].hin.unwrap(), a[2].p);
    for k in [2usize, 12, 21] {
        let c = cubic(p0, p1, p2, p3, (k as f32 + 0.5) / 24.0);
        let len = (c[0] * c[0] + c[1] * c[1]).sqrt();
        let out = |o: f32| [c[0] * (len + o) / len, c[1] * (len + o) / len];
        assert_eq!(ed.path_under(out(35.0)), Some(1), "35 outside, between samples k = {k}");
        assert_eq!(ed.path_under(out(-35.0)), Some(1), "35 inside, between samples k = {k}");
        assert_eq!(ed.path_under(out(45.0)), None, "45 outside is past the reach, k = {k}");
        assert_eq!(ed.path_under(out(-45.0)), None, "45 inside is the hollow, k = {k}");
    }
}

// ───────────────────────── rotate ring vs the selection's own band ─────────────────────────

/// A selected 200×200 stroke-only square with an 80-wide stroke (its band paints ±40 past the outline).
fn selected_thick_square(ppu: f32) -> Editor {
    let mut sq = rect(1, [0.0, 0.0, 200.0, 200.0], None, Some(INK));
    sq.stroke_width = 80.0;
    let mut ed = editor_with(vec![sq], ppu);
    ed.set_tool(ToolKind::Object);
    ed.objsel.insert(1);
    ed.refresh_obj_angle();
    ed
}

#[test]
fn rotate_ring_still_works_over_the_selections_own_thick_band() {
    // The ring sits just outside a frame corner (7..22 screen px). With the band reach, the selected
    // square's own stroke is under that spot — it must not block its own rotate ring.
    for ppu in [1.0f32, 3.27] {
        let ed = selected_thick_square(ppu);
        let d = 10.0 / ppu; // 14 screen px diagonally off the top-left corner [0, 0]
        let pos = [-d, -d];
        assert_eq!(ed.path_under(pos), Some(1), "the selection's own band is under the ring @ppu {ppu}");
        assert!(
            matches!(ed.transform_hit(pos), Some(TfHit::Rotate(0))),
            "the corner rotate ring still grabs over the own band @ppu {ppu}"
        );
    }
}

#[test]
fn rotate_ring_yields_to_another_objects_band() {
    // Unchanged rule: when a DIFFERENT (unselected) object is under the cursor, the click selects it.
    let mut ed = selected_thick_square(1.0);
    let other = Path::new(2, vec![corner(11, -100.0, -40.0), corner(12, 100.0, -40.0)], false, None, Some(INK), 80.0);
    ed.doc.paths.push(other); // on top; its band paints y ∈ [-80, 0]
    assert_eq!(ed.path_under([-10.0, -10.0]), Some(2));
    assert!(ed.transform_hit([-10.0, -10.0]).is_none(), "another object's band wins over the rotate ring");
}

// ───────────────────────── Convert segment-grab tolerance ─────────────────────────

#[test]
fn convert_segment_grab_tolerance_is_screen_constant() {
    for ppu in [0.25f32, 40.0] {
        for (px, grabs) in [(6.0f32, true), (10.0, false)] {
            let mut ed = editor_with(
                vec![Path::new(1, vec![corner(1, 0.0, 0.0), corner(2, 1000.0, 0.0)], false, None, Some(INK), 1.0)],
                ppu,
            );
            ed.set_tool(ToolKind::Convert);
            ed.pointer_down([500.0, px / ppu]);
            assert_eq!(
                matches!(ed.drag, Drag::Segment { .. }),
                grabs,
                "Convert at {px} screen px off the segment @ppu {ppu}: grabs = {grabs}"
            );
            ed.pointer_up();
        }
    }
}

// ───────────────────────── marquee touches painted geometry ─────────────────────────

#[test]
fn hollow_ring_marquee_inside_does_not_catch_ring() {
    let ed = editor_with(vec![circle(1, 1, [0.0, 0.0], 400.0, None, Some(INK), 80.0)], 1.0);
    // wholly inside the hole (inner painted edge is at r = 360)
    assert!(!ed.path_in_rect(0, -200.0, -200.0, 200.0, 200.0), "a marquee in the hollow leaves the ring alone");
    // on the inner half of the band only (x 370..380 — never reaches the centreline at 400)
    assert!(ed.path_in_rect(0, 370.0, -5.0, 380.0, 5.0), "touching the inner band catches the ring");
    // on the outer half of the band only (x 415..435)
    assert!(ed.path_in_rect(0, 415.0, -5.0, 435.0, 5.0), "touching the outer band catches the ring");
    // just outside the painted band (x 445..460) — the marquee is exact, no screen slack
    assert!(!ed.path_in_rect(0, 445.0, -5.0, 460.0, 5.0), "past the painted band catches nothing");
    // a FILLED disc is still caught by a marquee inside its area
    let filled = editor_with(vec![circle(1, 1, [0.0, 0.0], 400.0, Some(GREY), None, 1.0)], 1.0);
    assert!(filled.path_in_rect(0, -20.0, -20.0, 20.0, 20.0), "inside a filled shape = touching its fill");
}

#[test]
fn hollow_ring_marquee_gesture_leaves_ring_unselected() {
    // End-to-end through the shipped pointer engine (Object tool): press in the hollow → marquee.
    let mut ed = editor_with(vec![circle(1, 1, [0.0, 0.0], 400.0, None, Some(INK), 80.0)], 1.0);
    ed.set_tool(ToolKind::Object);
    ed.pointer_down([-150.0, -150.0]);
    ed.pointer_move([150.0, 150.0]);
    ed.pointer_up();
    assert!(!ed.objsel.contains(&1), "a marquee inside the ring must not select it");
    // …and a marquee reaching onto the band does
    ed.pointer_down([-150.0, -150.0]);
    ed.pointer_move([150.0, 370.0]); // bottom edge y = 370 dips into the band (inner edge 360)
    ed.pointer_up();
    assert!(ed.objsel.contains(&1), "a marquee touching the painted band selects the ring");
}

#[test]
fn thin_marquee_crossing_long_edge_selects() {
    // A 1000-wide stroke-only rect: its straight top edge has NO vertex between the corners. A 4-wide
    // marquee straddling that edge mid-span touches the drawn stroke, so it must select.
    let ed = editor_with(vec![rect(1, [0.0, 0.0, 1000.0, 500.0], None, Some(INK))], 1.0);
    assert!(ed.path_in_rect(0, 530.0, -10.0, 534.0, 8.0), "thin marquee across the edge selects");
    assert!(!ed.path_in_rect(0, 530.0, -30.0, 534.0, -10.0), "the same marquee above the edge does not");
}

#[test]
fn marquee_touching_only_a_donut_hole_rim_selects_it() {
    // A FILLED donut (outer r 400, hole r 200, stroke 40). The hole's rim is painted too, so a marquee on
    // its inner band touches the path — even though the marquee's centre sits in the (unfilled) hole.
    let mut donut = circle(1, 1, [0.0, 0.0], 400.0, Some(GREY), Some(INK), 40.0);
    donut.holes = vec![circle(9, 20, [0.0, 0.0], 200.0, None, None, 1.0).anchors];
    let ed = editor_with(vec![donut], 1.0);
    assert!(ed.path_in_rect(0, 185.0, -5.0, 195.0, 5.0), "a marquee on the hole rim's inner band catches it");
    assert!(!ed.path_in_rect(0, -50.0, -50.0, 50.0, 50.0), "a marquee wholly inside the hole catches nothing");
}

// ───────────────────────── straight edges measure true distance too ─────────────────────────

#[test]
fn big_rect_edge_near_corner_centreline_is_hit() {
    // Review P1-1: a straight segment is the cubic (p0, p0, p3, p3). Full Newton stalled near its ends,
    // leaving a click ON the top edge of a 20000 rect up to ~22 units "away" — a miss at 327 %.
    let ed = editor_with(vec![rect(1, [0.0, 0.0, 20_000.0, 20_000.0], None, Some(INK))], 3.27);
    for i in 0..=400 {
        let x = i as f32 * 0.5; // x ∈ [0, 200], the first ~650 screen px beside the corner
        for q in [[x, 0.0], [20_000.0 - x, 0.0], [0.0, x]] {
            let d = ed.doc.edge_dist(0, q).unwrap();
            assert!(d < 0.1, "a point ON the edge measures ~0 from it: {q:?} → {d}");
            assert_eq!(ed.path_under(q), Some(1), "the edge near the corner is clickable at 327 %: {q:?}");
        }
    }
}

#[test]
fn pen_add_anchor_near_a_corner_lands_on_the_line() {
    // Same stall, seen through the Pen: the inserted anchor must sit ON the line under the cursor.
    let mut ed = editor_with(
        vec![Path::new(1, vec![corner(1, 0.0, 0.0), corner(2, 20_000.0, 0.0)], false, None, Some(INK), 1.0)],
        3.27,
    );
    ed.doc.snap.enabled = false;
    ed.set_tool(ToolKind::Pen);
    ed.objsel.insert(1);
    let x = 32.5; // where the old refinement was worst (~22 units off)
    ed.pointer_down([x, 1.0 / 3.27]); // 1 screen px below the line
    ed.pointer_up();
    assert_eq!(ed.doc.paths[0].anchors.len(), 3, "an anchor was added");
    let added = ed.doc.paths[0].anchors[1].p;
    assert!(added[1].abs() < 1e-3 && (added[0] - x).abs() < 0.1, "inserted on the line under the cursor: {added:?}");
}

// ───────────────────────── Pen add-anchor tolerance ─────────────────────────

fn pen_on_line(ppu: f32) -> Editor {
    let mut ed = editor_with(
        vec![Path::new(1, vec![corner(1, 0.0, 0.0), corner(2, 1000.0, 0.0)], false, None, Some(INK), 1.0)],
        ppu,
    );
    ed.doc.snap.enabled = false; // measure the Pen's own tolerance, not the snap pull
    ed.set_tool(ToolKind::Pen);
    ed.objsel.insert(1); // selected ⇒ editable ⇒ Pen may add anchors to it
    ed
}

#[test]
fn pen_add_anchor_tolerance_is_screen_constant() {
    for ppu in [0.25f32, 40.0] {
        // 6 screen px off the centreline → inside EDGE_R (8 px) → adds an anchor, at both zooms
        let mut ed = pen_on_line(ppu);
        let pos = [500.0, 6.0 / ppu];
        assert!(matches!(ed.pen_hint(pos), PenHint::Add), "hint promises Add at 6 px @ppu {ppu}");
        ed.pointer_down(pos);
        ed.pointer_up();
        assert_eq!(ed.doc.paths[0].anchors.len(), 3, "6 screen px adds an anchor @ppu {ppu}");
        let added = ed.doc.paths[0].anchors[1].p;
        assert!((added[0] - 500.0).abs() < 1.0 && added[1].abs() < 1e-3, "inserted ON the curve: {added:?}");

        // 10 screen px off → outside EDGE_R → no anchor is added to the line, at both zooms
        let mut ed = pen_on_line(ppu);
        let pos = [500.0, 10.0 / ppu];
        assert!(!matches!(ed.pen_hint(pos), PenHint::Add), "no Add hint at 10 px @ppu {ppu}");
        ed.pointer_down(pos);
        ed.pointer_up();
        assert_eq!(ed.doc.paths[0].anchors.len(), 2, "10 screen px must not add an anchor @ppu {ppu}");
    }
}

#[test]
fn pen_click_on_thick_band_off_centre_does_not_insert() {
    // The band is clickable (it selects / occludes), but add-anchor stays a CENTRELINE gesture.
    let mut ed = pen_on_line(1.0);
    ed.doc.paths[0].stroke_width = 80.0;
    let pos = [500.0, 30.0]; // on the painted band, 30 off the centreline (> 8 px)
    assert_eq!(ed.path_under(pos), Some(1), "the band is under the cursor");
    ed.pointer_down(pos);
    ed.pointer_up();
    assert_eq!(ed.doc.paths[0].anchors.len(), 2, "no anchor inserted off the curve");
}
