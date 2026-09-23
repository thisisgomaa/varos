//! P11.2 pins (docs/foundation/P11_2_PERF.md), headless — no GPU, no window.
//! (a) path-level view culling drops a path wholly outside the frame and keeps (and correctly cuts) a
//!     partly visible one; (b) culling never drops a selected path's handles/markers that are in view;
//! (c) the cross-frame flatten cache returns exactly a fresh flatten and invalidates on any geometry edit
//!     (even one that does not bump `rev`) and on a zoom-bucket change.

use std::sync::Arc;
use varos_core::editor::{Editor, ToolKind};
use varos_core::flatten::{bucket_ppu, flatten_path, zoom_bucket, FlattenCache};
use varos_core::geom::{dist, point_in_poly, Pt, View};
use varos_core::model::{Anchor, Artboard, Document, Path, Xform};
use varos_core::scene::{build_scene, build_scene_in_view, Group, Prim, Scene, VIEW_PAD_PX};

const FRAME: [u32; 2] = [800, 600];

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn square(id: u32, base: u32, x0: f32, y0: f32, s: f32) -> Path {
    Path::new(
        id,
        vec![anc(base, x0, y0), anc(base + 1, x0 + s, y0), anc(base + 2, x0 + s, y0 + s), anc(base + 3, x0, y0 + s)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        Some([0.1, 0.1, 0.1, 1.0]),
        4.0,
    )
}
/// A smooth closed curve (a `count`-anchor circle of cubic arcs).
fn circle(id: u32, base: u32, c: Pt, r: f32, count: usize) -> Path {
    let step = std::f32::consts::TAU / count as f32;
    let k = r * (4.0 / 3.0) * (step * 0.25).tan();
    let anchors = (0..count)
        .map(|i| {
            let (s, co) = (i as f32 * step).sin_cos();
            let p = [c[0] + co * r, c[1] + s * r];
            let t = [-s, co];
            Anchor {
                id: base + i as u32,
                p,
                hin: Some([p[0] - t[0] * k, p[1] - t[1] * k]),
                hout: Some([p[0] + t[0] * k, p[1] + t[1] * k]),
                smooth: true,
            }
        })
        .collect();
    Path::new(id, anchors, true, Some([0.2, 0.5, 0.9, 1.0]), Some([0.0, 0.0, 0.0, 1.0]), 3.0)
}
fn editor(paths: Vec<Path>) -> Editor {
    let mut ed = Editor::new();
    ed.doc = Document::default();
    ed.doc.paths = paths;
    ed.doc.ids = 10_000;
    ed.doc.sync_tree();
    ed
}
fn fills(scene: &Scene) -> Vec<&Vec<Vec<Pt>>> {
    scene
        .content
        .iter()
        .flat_map(|g| g.prims())
        .filter_map(|p| match p {
            Prim::Fill { rings, .. } => Some(rings),
            _ => None,
        })
        .collect()
}
fn strokes(prims: &[Prim]) -> Vec<&Vec<Pt>> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Stroke { pts, .. } => Some(pts),
            _ => None,
        })
        .collect()
}
fn content_prims(scene: &Scene) -> Vec<&Prim> {
    scene.content.iter().flat_map(|g| g.prims()).collect()
}
fn in_frame(p: Pt) -> bool {
    p[0] >= 0.0 && p[0] <= FRAME[0] as f32 && p[1] >= 0.0 && p[1] <= FRAME[1] as f32
}
fn seg_dist(p: Pt, a: Pt, b: Pt) -> f32 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 <= 1e-12 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) };
    dist(p, [a[0] + d[0] * t, a[1] + d[1] * t])
}
fn polyline_dist(p: Pt, runs: &[&Vec<Pt>]) -> f32 {
    runs.iter().flat_map(|r| r.windows(2).map(move |w| seg_dist(p, w[0], w[1]))).fold(f32::MAX, f32::min)
}
/// Even-odd inside test over several rings (outer + holes), as the stencil fill paints it.
fn even_odd(rings: &[Vec<Pt>], p: Pt) -> bool {
    rings.iter().filter(|r| r.len() >= 3 && point_in_poly(r, p)).count() % 2 == 1
}
/// A grid of sample points strictly inside the frame.
fn frame_samples(step: f32) -> Vec<Pt> {
    let mut out = Vec::new();
    let mut y = step * 0.5 + 0.013;
    while y < FRAME[1] as f32 {
        let mut x = step * 0.5 + 0.017;
        while x < FRAME[0] as f32 {
            out.push([x, y]);
            x += step;
        }
        y += step;
    }
    out
}

// ─────────────────────────── (a) path-level culling + ring/edge clipping ───────────────────────────

#[test]
fn a_path_wholly_outside_the_view_is_culled_while_a_partly_visible_one_is_kept() {
    let inside = square(1, 100, 100.0, 100.0, 80.0);
    let outside = square(2, 200, 2_000.0, 100.0, 80.0); // far right of an 800×600 frame
    let partial = square(3, 300, 700.0, 250.0, 400.0); // straddles the right edge
    let ed = editor(vec![inside, outside, partial]);

    let full = build_scene(&ed, 1.0);
    let cut = build_scene_in_view(&ed, View::identity(), FRAME);

    assert_eq!(fills(&full).len(), 3, "uncut scene has all three fills");
    let cut_fills = fills(&cut);
    assert_eq!(cut_fills.len(), 2, "the off-screen square is culled; the other two are kept");
    for rings in &cut_fills {
        for p in rings.iter().flatten() {
            assert!(p[0] < 1_000.0, "nothing from the off-screen square survives: {p:?}");
        }
    }
    // the wholly-visible square is byte-identical to the uncut scene
    assert_eq!(cut_fills[0], fills(&full)[0]);
    // the partly visible one is cut to the grown view rect (frame + pad + half the stroke width)
    let limit = FRAME[0] as f32 + VIEW_PAD_PX + 4.0 * 0.5;
    assert!(cut_fills[1][0].iter().all(|p| p[0] <= limit + 1e-3), "partial fill ring is cut at the view edge");
    assert!(cut_fills[1][0].iter().any(|p| (p[0] - limit).abs() < 1e-3), "…and actually reaches the cut line");
}

#[test]
fn view_clipping_leaves_every_on_screen_pixel_unchanged_for_fills_holes_and_strokes() {
    // 4000%-style case: a big curved compound shape with a hole, only partly on screen at high zoom.
    let mut shape = circle(1, 100, [300.0, 200.0], 120.0, 24);
    let hole = circle(2, 200, [330.0, 200.0], 40.0, 12);
    shape.holes.push(hole.anchors);
    let ed = editor(vec![shape]);
    let zoom = 40.0;
    // centre the frame on the shape's right edge (x = 420) and the hole's right edge region
    let view = View { pan: [400.0 - 420.0 * zoom, 300.0 - 200.0 * zoom], zoom };

    let full = build_scene(&ed, zoom);
    let cut = build_scene_in_view(&ed, view, FRAME);

    let full_rings = fills(&full)[0].clone();
    let cut_rings = fills(&cut)[0].clone();
    let n_full: usize = full_rings.iter().map(Vec::len).sum();
    let n_cut: usize = cut_rings.iter().map(Vec::len).sum();
    assert!(n_cut * 5 < n_full, "ring clipping drops the off-screen edges ({n_cut} vs {n_full} points)");

    // Fill: even-odd coverage of every sample point in the frame is identical.
    for s in frame_samples(13.0) {
        let w = view.s2w(s);
        assert_eq!(even_odd(&full_rings, w), even_odd(&cut_rings, w), "fill coverage differs at screen {s:?}");
    }
    // Stroke: every in-frame point within reach of the band is equally far from the cut runs.
    let full_prims: Vec<Prim> = full.content.iter().flat_map(|g| g.prims().iter().cloned()).collect();
    let cut_prims: Vec<Prim> = cut.content.iter().flat_map(|g| g.prims().iter().cloned()).collect();
    let (fs, cs) = (strokes(&full_prims), strokes(&cut_prims));
    let fs_points: usize = fs.iter().map(|r| r.len()).sum();
    let cs_points: usize = cs.iter().map(|r| r.len()).sum();
    assert!(cs_points * 5 < fs_points, "stroke runs are cut too ({cs_points} vs {fs_points})");
    let reach = 3.0 * 0.5 + 2.0 / zoom;
    for s in frame_samples(20.0) {
        let w = view.s2w(s);
        let d_full = polyline_dist(w, &fs);
        if d_full <= reach {
            let d_cut = polyline_dist(w, &cs);
            assert!((d_full - d_cut).abs() < 1e-3, "stroke band differs at screen {s:?}: {d_full} vs {d_cut}");
        }
    }
}

#[test]
fn a_view_showing_everything_gives_the_uncut_scene_exactly() {
    let mut ed = editor(vec![square(1, 100, 50.0, 50.0, 80.0), circle(2, 200, [400.0, 300.0], 90.0, 16)]);
    ed.tool = ToolKind::Direct;
    ed.objsel.insert(2);
    let ids: Vec<u32> = ed.doc.paths[1].anchors.iter().map(|a| a.id).collect();
    ed.selected.extend(ids);
    let full = build_scene(&ed, 1.0);
    let cut = build_scene_in_view(&ed, View::identity(), FRAME);
    assert_eq!(full.content, cut.content, "content identical when nothing is off screen");
    assert_eq!(full.overlay, cut.overlay, "overlay identical when nothing is off screen");
}

#[test]
fn artboard_clip_and_view_clip_compose() {
    // a page with clip ON, larger than the frame; the art overflows both the page and the frame.
    let mut ed = editor(vec![square(1, 100, 500.0, 100.0, 1_000.0)]);
    ed.doc.artboards =
        vec![Artboard { x: 0.0, y: 0.0, w: 1_200.0, h: 900.0, name: "A".into(), clip: true, ..Artboard::default() }];
    ed.doc.sync_tree();
    let full = build_scene(&ed, 1.0);
    let cut = build_scene_in_view(&ed, View::identity(), FRAME);
    let art = |s: &Scene| -> Vec<Vec<Vec<Pt>>> {
        fills(s).into_iter().filter(|r| r[0].len() >= 3 && r[0][0][0] >= 499.0).cloned().collect()
    };
    let (af, ac) = (art(&full), art(&cut));
    assert_eq!(af.len(), 1);
    assert_eq!(ac.len(), 1);
    for s in frame_samples(11.0) {
        assert_eq!(even_odd(&af[0], s), even_odd(&ac[0], s), "page∩view cut changed coverage at {s:?}");
    }
    // stroke runs keep the PAGE rect as their scissor (A2), not the view rect
    for p in content_prims(&cut) {
        if let Prim::Stroke { clip, .. } = p {
            assert_eq!(*clip, Some([0.0, 0.0, 1_200.0, 900.0]));
        }
    }
}

#[test]
fn a_clip_mask_wholly_off_screen_clips_its_members_to_nothing() {
    let mut ed = editor(vec![square(10, 100, 100.0, 100.0, 50.0), square(11, 110, 3_000.0, 100.0, 50.0)]);
    ed.doc.clip_group(&[10, 11], 11).unwrap();
    let cut = build_scene_in_view(&ed, View::identity(), FRAME);
    let clip = cut.content.iter().find_map(|g| match g {
        Group::Clip { mask_rings, members } => Some((mask_rings, members)),
        _ => None,
    });
    let (mask_rings, _) = clip.expect("the visible member still emits its clip group");
    assert!(mask_rings.is_empty(), "an off-screen mask contributes no ring ⇒ members draw nowhere");
}

// ─────────────────────────── (b) handles & markers of a selected path ───────────────────────────

fn marks(overlay: &[Prim]) -> Vec<(u8, Pt)> {
    overlay
        .iter()
        .filter_map(|p| match p {
            Prim::Disc { c, .. } => Some((0, *c)),
            Prim::Square { c, .. } => Some((1, *c)),
            _ => None,
        })
        .collect()
}

#[test]
fn culling_never_drops_a_selected_paths_handles_or_markers_that_are_in_view() {
    let mut ed = editor(vec![circle(1, 100, [300.0, 200.0], 150.0, 60)]);
    ed.tool = ToolKind::Direct;
    ed.objsel.insert(1);
    let ids: Vec<u32> = ed.doc.paths[0].anchors.iter().map(|a| a.id).collect();
    ed.selected.extend(ids);
    let zoom = 8.0;
    let view = View { pan: [400.0 - 450.0 * zoom, 300.0 - 200.0 * zoom], zoom };

    let full = build_scene(&ed, zoom);
    let cut = build_scene_in_view(&ed, view, FRAME);
    let (full_marks, cut_marks) = (marks(&full.overlay), marks(&cut.overlay));
    let visible: Vec<&(u8, Pt)> = full_marks.iter().filter(|(_, c)| in_frame(view.w2s(*c))).collect();
    assert!(!visible.is_empty(), "the test view does show some handles/markers");
    for m in &visible {
        assert!(cut_marks.contains(m), "an in-view handle/marker was culled: {m:?}");
    }
    assert!(
        cut_marks.len() < full_marks.len(),
        "off-screen marks are culled ({} of {})",
        cut_marks.len(),
        full_marks.len()
    );

    // every handle LINE that crosses the frame survives
    let lines = |o: &[Prim]| -> Vec<Vec<Pt>> {
        o.iter()
            .filter_map(|p| match p {
                Prim::Stroke { pts, width, .. } if *width == 1.0 && pts.len() == 2 => Some(pts.clone()),
                _ => None,
            })
            .collect()
    };
    let cut_lines = lines(&cut.overlay);
    for l in lines(&full.overlay) {
        let (a, b) = (view.w2s(l[0]), view.w2s(l[1]));
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        if in_frame(a) || in_frame(b) || in_frame(mid) {
            assert!(cut_lines.contains(&l), "an in-view handle line was culled: {l:?}");
        }
    }
}

#[test]
fn a_handle_reaching_into_view_from_an_off_screen_anchor_is_kept() {
    // anchor far left of the frame, its out-handle reaching into it
    let mut a0 = anc(100, -300.0, 300.0);
    a0.hout = Some([60.0, 300.0]);
    let path = Path::new(1, vec![a0, anc(101, -300.0, 900.0)], false, None, Some([0.0, 0.0, 0.0, 1.0]), 1.0);
    let mut ed = editor(vec![path]);
    ed.tool = ToolKind::Direct;
    ed.selected.insert(100);
    let cut = build_scene_in_view(&ed, View::identity(), FRAME);
    assert!(
        cut.overlay.iter().any(|p| matches!(p, Prim::Disc { c, .. } if *c == [60.0, 300.0])),
        "the in-view handle disc is kept"
    );
    assert!(
        cut.overlay
            .iter()
            .any(|p| matches!(p, Prim::Stroke { pts, .. } if *pts == vec![[-300.0, 300.0], [60.0, 300.0]])),
        "its handle line (anchor off screen) is kept"
    );
}

// ─────────────────────────── (c) the cross-frame flatten cache ───────────────────────────

#[test]
fn zoom_buckets_are_quarter_octaves_never_coarser_than_the_zoom() {
    for b in [-8, -4, 0, 4, 8, 12] {
        assert_eq!(bucket_ppu(b), 2f32.powi(b / 4), "integer octaves are exact");
        assert_eq!(zoom_bucket(bucket_ppu(b)), b, "an exact octave is its own bucket");
    }
    let mut ppu = 0.02;
    while ppu < 64.0 {
        let rep = bucket_ppu(zoom_bucket(ppu));
        assert!(rep >= ppu * (1.0 - 1e-3), "bucket ppu {rep} is coarser than {ppu}");
        assert!(rep <= ppu * 1.19, "bucket ppu {rep} is more than a quarter octave above {ppu}");
        ppu *= 1.037;
    }
}

#[test]
fn the_cache_returns_exactly_a_fresh_flatten_and_reuses_it() {
    let doc = editor(vec![circle(1, 100, [0.0, 0.0], 100.0, 9)]).doc;
    let mut cache = FlattenCache::default();
    let g = cache.geometry(&doc, 0, 3.0);
    assert_eq!(*g, flatten_path(&doc, 0, 3.0));
    assert_eq!(g.outline, doc.world_outline_px(0, bucket_ppu(zoom_bucket(3.0))), "same as the model flatten");
    assert_eq!(cache.stats(), (0, 1));

    let again = cache.geometry(&doc, 0, 3.0);
    assert!(Arc::ptr_eq(&g, &again), "an unchanged path is not re-flattened");
    let same_bucket = cache.geometry(&doc, 0, 3.2); // 3.0 and 3.2 share a quarter-octave bucket
    assert!(Arc::ptr_eq(&g, &same_bucket));
    assert_eq!(cache.stats(), (2, 1));

    let other_bucket = cache.geometry(&doc, 0, 4.0);
    assert!(!Arc::ptr_eq(&g, &other_bucket), "a new zoom bucket re-flattens");
    assert_eq!(*other_bucket, flatten_path(&doc, 0, 4.0));
    assert_eq!(cache.stats(), (2, 2));
}

#[test]
fn the_cache_invalidates_on_every_kind_of_geometry_edit() {
    let mut doc = editor(vec![circle(1, 100, [0.0, 0.0], 100.0, 9)]).doc;
    let mut cache = FlattenCache::default();
    let expect_miss = |doc: &Document, cache: &mut FlattenCache, what: &str| {
        let (_, misses) = cache.stats();
        let g = cache.geometry(doc, 0, 2.0);
        assert_eq!(cache.stats().1, misses + 1, "{what}: must miss");
        assert_eq!(*g, flatten_path(doc, 0, 2.0), "{what}: must equal a fresh flatten");
        let bbox = cache.bbox(doc, 0);
        assert_eq!(bbox, varos_core::flatten::control_bbox(doc, 0), "{what}: bbox refreshed");
    };
    expect_miss(&doc, &mut cache, "first");
    doc.paths[0].anchors[3].p[0] += 5.0;
    expect_miss(&doc, &mut cache, "anchor moved");
    doc.paths[0].anchors[4].hout = Some([1.0, 2.0]);
    expect_miss(&doc, &mut cache, "handle moved");
    doc.paths[0].holes.push(vec![anc(900, -10.0, -10.0), anc(901, 10.0, -10.0), anc(902, 0.0, 10.0)]);
    expect_miss(&doc, &mut cache, "hole added");
    doc.paths[0].closed = false;
    expect_miss(&doc, &mut cache, "opened");
    let leaf = doc.node_of_path(1).expect("the path has a leaf node");
    doc.set_node_xform(leaf, Xform { rot: 0.5, piv: [0.0, 0.0] });
    expect_miss(&doc, &mut cache, "unit rotated");
    // a paint-only change does not touch geometry → still a hit
    doc.paths[0].fill = Default::default();
    let (hits, misses) = cache.stats();
    cache.geometry(&doc, 0, 2.0);
    assert_eq!(cache.stats(), (hits + 1, misses), "paint change reuses the flatten");
}

#[test]
fn a_live_gesture_edit_without_a_rev_bump_never_draws_stale_geometry() {
    let mut ed = editor(vec![circle(1, 100, [300.0, 200.0], 100.0, 8)]);
    let rev = ed.rev;
    let first = build_scene_in_view(&ed, View::identity(), FRAME);
    // mutate the live document the way a drag preview does — no commit, no rev bump
    for a in &mut ed.doc.paths[0].anchors {
        a.p[0] += 40.0;
        a.hin = a.hin.map(|h| [h[0] + 40.0, h[1]]);
        a.hout = a.hout.map(|h| [h[0] + 40.0, h[1]]);
    }
    assert_eq!(ed.rev, rev);
    let moved = build_scene_in_view(&ed, View::identity(), FRAME);
    let fresh = flatten_path(&ed.doc, 0, 1.0);
    assert_eq!(fills(&moved)[0][0], fresh.outline, "the moved outline, not the cached one");
    assert_ne!(fills(&first)[0][0], fills(&moved)[0][0]);
}

#[test]
fn the_editor_cache_reuses_geometry_across_frames_and_evicts_deleted_paths() {
    let mut ed = editor(vec![circle(1, 100, [100.0, 100.0], 50.0, 8), circle(2, 200, [400.0, 300.0], 50.0, 8)]);
    let _ = build_scene_in_view(&ed, View::identity(), FRAME);
    assert_eq!(ed.flatten_cache.lock().stats(), (0, 2));
    // a pan within the same zoom: both paths still visible, nothing re-flattened
    let _ = build_scene_in_view(&ed, View { pan: [10.0, -5.0], zoom: 1.0 }, FRAME);
    assert_eq!(ed.flatten_cache.lock().stats(), (2, 2));
    ed.doc.paths.remove(0);
    let _ = build_scene_in_view(&ed, View::identity(), FRAME);
    assert_eq!(ed.flatten_cache.lock().len(), 1, "the deleted path's entry is evicted");
    // a culled path is never flattened
    ed.flatten_cache.lock().clear();
    let _ = build_scene_in_view(&ed, View { pan: [-5_000.0, 0.0], zoom: 1.0 }, FRAME);
    assert_eq!(ed.flatten_cache.lock().stats(), (0, 0), "an off-screen path costs no flatten");
}

// ─────────────────────────── review fixes (Codex P1-1, P1-2) ───────────────────────────

/// The review's P1-1 scene: two crossing, unfilled, 50%-red strokes separated in z by an opaque filled
/// rectangle that lies wholly off screen. Public so the renderer test can build the same document.
fn crossing_translucent_strokes() -> Editor {
    let red = Some([1.0, 0.0, 0.0, 0.5]);
    let a = Path::new(1, vec![anc(100, 100.0, 100.0), anc(101, 500.0, 400.0)], false, None, red, 6.0);
    let rect = Path::new(
        2,
        vec![anc(200, 3_000.0, 0.0), anc(201, 3_100.0, 0.0), anc(202, 3_100.0, 100.0), anc(203, 3_000.0, 100.0)],
        true,
        Some([0.2, 0.2, 0.2, 1.0]),
        None,
        1.0,
    );
    let b = Path::new(3, vec![anc(300, 100.0, 400.0), anc(301, 500.0, 100.0)], false, None, red, 6.0);
    editor(vec![a, rect, b])
}

/// How many single-stencil coverage batches the renderer will form: within each group's prims, a
/// maximal run of consecutive translucent strokes of ONE colour is marked and covered once (tess.rs).
fn coverage_batches(scene: &Scene) -> usize {
    fn count(prims: &[Prim]) -> usize {
        let mut n = 0;
        let mut prev: Option<[f32; 4]> = None;
        for p in prims {
            match p {
                Prim::Stroke { color, .. } if color[3] < 0.999 => {
                    if prev != Some(*color) {
                        n += 1;
                    }
                    prev = Some(*color);
                }
                _ => prev = None,
            }
        }
        n
    }
    fn walk(groups: &[Group]) -> usize {
        groups
            .iter()
            .map(|g| match g {
                Group::Clip { members, .. } => walk(members),
                g => count(g.prims()),
            })
            .sum()
    }
    walk(&scene.content)
}

#[test]
fn culling_an_object_never_merges_the_translucent_strokes_on_either_side_of_it() {
    let ed = crossing_translucent_strokes();
    let full = build_scene(&ed, 1.0);
    let cut = build_scene_in_view(&ed, View::identity(), FRAME);
    assert_eq!(fills(&cut).len(), 0, "the separating rectangle is culled");
    assert_eq!(coverage_batches(&full), 2, "uncut: one coverage per stroke (the crossing paints twice)");
    assert_eq!(coverage_batches(&cut), 2, "culled: still one coverage per stroke — never merged");
}

#[test]
fn two_adjacent_translucent_strokes_of_one_colour_stay_separate_objects() {
    // no separator at all: object boundaries still split the coverage (the crossing is 75%, not 50%)
    let mut ed = crossing_translucent_strokes();
    ed.doc.paths.remove(1);
    assert_eq!(coverage_batches(&build_scene(&ed, 1.0)), 2);
    // …while ONE object's outer + hole rings keep sharing a single coverage (paints once)
    let mut ring = circle(1, 100, [300.0, 300.0], 100.0, 8);
    ring.fill = Default::default();
    ring.stroke = varos_core::model::Paint::from_opt(Some([0.0, 0.0, 1.0, 0.5]));
    ring.holes.push(circle(2, 200, [300.0, 300.0], 40.0, 8).anchors);
    let one = editor(vec![ring]);
    assert_eq!(coverage_batches(&build_scene(&one, 1.0)), 1);
}

/// Treatment of each painted object: I = isolated, K = knockout, O = anything in an opaque run.
fn treatments(groups: &[Group]) -> Vec<char> {
    groups
        .iter()
        .flat_map(|g| match g {
            Group::Clip { members, .. } => treatments(members),
            Group::Isolated { .. } => vec!['I'],
            Group::Knockout(_) => vec!['K'],
            Group::Opaque(_) => vec!['O'],
        })
        .collect()
}

/// The review's P1-2 scene: a 50%-opacity rectangle (opaque fill + 2-unit opaque stroke) larger than the
/// 800×600 frame, inside a larger clipping mask.
fn masked_translucent_overflowing_rect() -> Editor {
    let mut rect = square(10, 100, -100.0, -100.0, 1.0);
    for (a, p) in rect.anchors.iter_mut().zip([[-100.0, -100.0], [900.0, -100.0], [900.0, 700.0], [-100.0, 700.0]]) {
        a.p = p;
    }
    rect.stroke_width = 2.0;
    rect.opacity = 0.5;
    let mask = square(11, 110, -500.0, -500.0, 2_000.0);
    let mut ed = editor(vec![rect, mask]);
    ed.doc.clip_group(&[10, 11], 11).unwrap();
    ed
}

#[test]
fn an_off_screen_stroke_never_changes_an_objects_opacity_treatment() {
    let ed = masked_translucent_overflowing_rect();
    let full = treatments(&build_scene(&ed, 1.0).content);
    assert_eq!(full, vec!['I'], "uncut: fill + stroke at 50% = one isolated layer");
    // pan 0: every stroke run is off screen AND outside the padding → cut away entirely
    let at0 = build_scene_in_view(&ed, View::identity(), FRAME);
    assert!(strokes(&content_prims_owned(&at0)).is_empty(), "pan 0: no stroke run survives the view cut");
    // pan 70: the left edge enters the padding (still off screen) → one run survives
    let at70 = build_scene_in_view(&ed, View { pan: [70.0, 0.0], zoom: 1.0 }, FRAME);
    assert!(!strokes(&content_prims_owned(&at70)).is_empty(), "pan 70: the left edge's run is in the pad");
    assert_eq!(treatments(&at0.content), full, "pan 0 keeps the isolated treatment");
    assert_eq!(treatments(&at70.content), full, "pan 70 keeps the isolated treatment");
}

#[test]
fn an_off_screen_stroke_never_changes_the_treatment_without_a_mask_either() {
    let mut ed = masked_translucent_overflowing_rect();
    ed.doc = {
        let mut d = Document::default();
        d.paths.push(ed.doc.paths[ed.doc.pidx(10).unwrap()].clone());
        d.ids = 10_000;
        d.sync_tree();
        d
    };
    let full = treatments(&build_scene(&ed, 1.0).content);
    assert_eq!(full, vec!['I']);
    for pan in [0.0, 35.0, 70.0] {
        let cut = build_scene_in_view(&ed, View { pan: [pan, 0.0], zoom: 1.0 }, FRAME);
        assert_eq!(treatments(&cut.content), full, "pan {pan}: same treatment as uncut");
    }
    // translucent STROKE on an opaque fill: knockout stays knockout when the band is cut away
    ed.doc.paths[0].opacity = 1.0;
    ed.doc.paths[0].stroke = varos_core::model::Paint::from_opt(Some([0.0, 0.0, 0.0, 0.5]));
    assert_eq!(treatments(&build_scene(&ed, 1.0).content), vec!['K']);
    assert_eq!(treatments(&build_scene_in_view(&ed, View::identity(), FRAME).content), vec!['K']);
}

fn content_prims_owned(scene: &Scene) -> Vec<Prim> {
    fn walk(groups: &[Group], out: &mut Vec<Prim>) {
        for g in groups {
            match g {
                Group::Clip { members, .. } => walk(members, out),
                g => out.extend(g.prims().iter().cloned()),
            }
        }
    }
    let mut out = Vec::new();
    walk(&scene.content, &mut out);
    out
}
