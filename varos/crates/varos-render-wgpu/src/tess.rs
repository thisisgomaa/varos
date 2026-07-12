//! Turn the core's render-agnostic `Scene` primitives into GPU triangles (pixel space → NDC on CPU).

use varos_core::geom::{Pt, View};
use varos_core::scene::{Group, Prim};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

fn dist(a: Pt, b: Pt) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}
fn ndc(p: Pt, w: f32, h: f32) -> [f32; 2] {
    [p[0] / w * 2.0 - 1.0, 1.0 - p[1] / h * 2.0]
}
fn tri(v: &mut Vec<Vertex>, a: Pt, b: Pt, c: Pt, col: [f32; 4], w: f32, h: f32) {
    v.push(Vertex { pos: ndc(a, w, h), color: col });
    v.push(Vertex { pos: ndc(b, w, h), color: col });
    v.push(Vertex { pos: ndc(c, w, h), color: col });
}
#[allow(clippy::too_many_arguments)] // 4 corners + paint + framebuffer — bundling would obscure it
fn quad(v: &mut Vec<Vertex>, p0: Pt, p1: Pt, p2: Pt, p3: Pt, col: [f32; 4], w: f32, h: f32) {
    tri(v, p0, p1, p2, col, w, h);
    tri(v, p0, p2, p3, col, w, h);
}
fn line(v: &mut Vec<Vertex>, a: Pt, b: Pt, width: f32, col: [f32; 4], w: f32, h: f32) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
    let n = [-d[1] / l * width / 2.0, d[0] / l * width / 2.0];
    quad(
        v,
        [a[0] + n[0], a[1] + n[1]],
        [b[0] + n[0], b[1] + n[1]],
        [b[0] - n[0], b[1] - n[1]],
        [a[0] - n[0], a[1] - n[1]],
        col,
        w,
        h,
    );
}
fn sq(v: &mut Vec<Vertex>, c: Pt, half: f32, col: [f32; 4], w: f32, h: f32) {
    quad(
        v,
        [c[0] - half, c[1] - half],
        [c[0] + half, c[1] - half],
        [c[0] + half, c[1] + half],
        [c[0] - half, c[1] + half],
        col,
        w,
        h,
    );
}
fn disc(v: &mut Vec<Vertex>, c: Pt, r: f32, col: [f32; 4], w: f32, h: f32) {
    static UNIT_RING: std::sync::OnceLock<Vec<Pt>> = std::sync::OnceLock::new();
    let ring = UNIT_RING.get_or_init(|| {
        (0..=24)
            .map(|i| {
                let angle = i as f32 / 24.0 * std::f32::consts::TAU;
                [angle.cos(), angle.sin()]
            })
            .collect()
    });
    v.reserve(24 * 3);
    for edge in ring.windows(2) {
        tri(
            v,
            c,
            [c[0] + edge[0][0] * r, c[1] + edge[0][1] * r],
            [c[0] + edge[1][0] * r, c[1] + edge[1][1] * r],
            col,
            w,
            h,
        );
    }
}
fn stroke_poly(v: &mut Vec<Vertex>, pts: &[Pt], width: f32, col: [f32; 4], w: f32, h: f32) {
    let join_count = if width >= 1.6 && pts.len() >= 2 {
        (1..pts.len() - 1).filter(|&i| round_join_needed(pts[i - 1], pts[i], pts[i + 1])).count() + 2
    } else {
        0
    };
    v.reserve(pts.len().saturating_sub(1) * 6 + join_count * 24 * 3);
    for i in 0..pts.len().saturating_sub(1) {
        line(v, pts[i], pts[i + 1], width, col, w, h);
    }
    // Round caps plus joins at actual direction changes. Adaptive curve subdivision emits many nearly
    // collinear points; a 24-triangle disc at every one was P11's high-zoom vertex explosion. Segment
    // quads already overlap cleanly below this threshold, while real corners retain the round join.
    if width >= 1.6 && pts.len() >= 2 {
        let r = width * 0.5;
        disc(v, pts[0], r, col, w, h);
        for i in 1..pts.len() - 1 {
            if round_join_needed(pts[i - 1], pts[i], pts[i + 1]) {
                disc(v, pts[i], r, col, w, h);
            }
        }
        disc(v, pts[pts.len() - 1], r, col, w, h);
    }
}

/// Five degrees is below a visually meaningful corner but above floating-point/tessellation drift.
fn round_join_needed(a: Pt, b: Pt, c: Pt) -> bool {
    const COS_5_DEG: f32 = 0.996_194_7;
    let incoming = [b[0] - a[0], b[1] - a[1]];
    let outgoing = [c[0] - b[0], c[1] - b[1]];
    let lengths = (incoming[0] * incoming[0] + incoming[1] * incoming[1])
        * (outgoing[0] * outgoing[0] + outgoing[1] * outgoing[1]);
    if lengths <= 1e-12 {
        return false;
    }
    let cosine = (incoming[0] * outgoing[0] + incoming[1] * outgoing[1]) / lengths.sqrt();
    cosine < COS_5_DEG
}
fn dashed_poly(v: &mut Vec<Vertex>, pts: &[Pt], width: f32, col: [f32; 4], w: f32, h: f32) {
    let (dash, gap) = (5.0f32, 4.0f32);
    let period = dash + gap;
    let mut acc = 0.0f32;
    for i in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[i], pts[i + 1]);
        let seglen = dist(a, b);
        if seglen < 1e-4 {
            continue;
        }
        let dir = [(b[0] - a[0]) / seglen, (b[1] - a[1]) / seglen];
        let mut s = 0.0f32;
        while s < seglen {
            let phase = (acc + s) % period;
            if phase < dash {
                let e = (s + (dash - phase)).min(seglen);
                line(
                    v,
                    [a[0] + dir[0] * s, a[1] + dir[1] * s],
                    [a[0] + dir[0] * e, a[1] + dir[1] * e],
                    width,
                    col,
                    w,
                    h,
                );
                s = e;
            } else {
                s += period - phase;
            }
        }
        acc += seglen;
    }
}

/// Infinite ADAPTIVE dot grid. The dots live in WORLD space (they pan & zoom with the board), and the
/// spacing snaps to base-5 "nice" levels (…1·5·25·125…) so the on-screen density stays comfortable at
/// any zoom. Two consecutive levels crossfade (the finer one fades out as it gets too dense) so moving
/// between scales is smooth, never a pop — giving a sense of depth and of where you are on the board.
/// This is also the spatial reference the future snapping system will lock onto.
pub fn build_bg(view: View, w: f32, h: f32) -> Vec<Vertex> {
    let mut v = Vec::new();
    let zoom = view.zoom.max(1e-4);
    const TARGET: f32 = 30.0; // desired screen px between dots
    const MIN_PX: f32 = 9.0; // skip a level finer than this (perf + anti-clutter)
    const BG: [f32; 3] = [0.078, 0.075, 0.075]; // board background (#141313)
    const DOT: [f32; 3] = [0.34, 0.34, 0.37]; // a dot at full strength (clearly visible on #141313)

    // base-5 level whose world step lands near TARGET px on screen
    let scale = (TARGET / zoom).max(1e-6);
    let level = scale.ln() / 5f32.ln();
    let k0 = level.floor();
    let t = level - k0; // 0..1 within the level
    let step_fine = 5f32.powf(k0);
    let step_coarse = 5f32.powf(k0 + 1.0);

    // visible world rect (+1 step padding so dots don't pop at the edges)
    let tl = view.s2w([0.0, 0.0]);
    let br = view.s2w([w, h]);
    let (wx0, wy0) = (tl[0].min(br[0]), tl[1].min(br[1]));
    let (wx1, wy1) = (tl[0].max(br[0]), tl[1].max(br[1]));

    let mut grid = |step: f32, alpha: f32| {
        if alpha < 0.04 || step * zoom < MIN_PX {
            return;
        }
        // composite the faded dot over the board once (no blend-state dependency): opaque colour.
        let col =
            [BG[0] + (DOT[0] - BG[0]) * alpha, BG[1] + (DOT[1] - BG[1]) * alpha, BG[2] + (DOT[2] - BG[2]) * alpha, 1.0];
        let mut gx = (wx0 / step).floor() * step;
        while gx <= wx1 {
            let mut gy = (wy0 / step).floor() * step;
            while gy <= wy1 {
                sq(&mut v, view.w2s([gx, gy]), 1.0, col, w, h);
                gy += step;
            }
            gx += step;
        }
    };
    // THREE levels crossfade with NO pop: the finest fades OUT as it gets too dense (1-t), the middle
    // is the steady full-strength anchor (1.0), and the next-coarser fades IN (t) so it's already there
    // when it becomes the new anchor. Every level enters/leaves through 0 → no appear/disappear snap.
    grid(step_fine, 1.0 - t);
    grid(step_coarse, 1.0);
    grid(5f32.powf(k0 + 2.0), t);
    v
}

/// One Fill prim's vertex ranges: `((fan_start, fan_len), (cover_start, cover_len))`.
pub type FillRanges = ((u32, u32), (u32, u32));
/// fills: per Fill prim → a triangle-fan (stencil) + a bbox cover quad. Points are mapped world→screen via `view`.
pub fn build_fills(prims: &[Prim], view: View, w: f32, h: f32) -> (Vec<Vertex>, Vec<FillRanges>) {
    let mut v = Vec::new();
    let mut ranges = Vec::new();
    for prim in prims {
        if let Prim::Fill { rings, color } = prim {
            // map every ring (outer + holes) to screen, then draw pivot-triangles for ALL edges into the
            // stencil with one global pivot — even-odd parity then cuts the holes in a single cover pass.
            let srings: Vec<Vec<Pt>> = rings.iter().map(|r| r.iter().map(|p| view.w2s(*p)).collect()).collect();
            let pivot = match srings.iter().find(|r| r.len() >= 3) {
                Some(r) => r[0],
                None => continue,
            };
            let fan_start = v.len() as u32;
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for r in &srings {
                if r.len() < 3 {
                    continue;
                }
                let n = r.len();
                for j in 0..n {
                    tri(&mut v, pivot, r[j], r[(j + 1) % n], *color, w, h);
                }
                for p in r {
                    x0 = x0.min(p[0]);
                    y0 = y0.min(p[1]);
                    x1 = x1.max(p[0]);
                    y1 = y1.max(p[1]);
                }
            }
            let fan_len = v.len() as u32 - fan_start;
            let cov_start = v.len() as u32;
            quad(&mut v, [x0, y0], [x1, y0], [x1, y1], [x0, y1], *color, w, h);
            let cov_len = v.len() as u32 - cov_start;
            ranges.push(((fan_start, fan_len), (cov_start, cov_len)));
        }
    }
    (v, ranges)
}

/// everything except fills (drawn on top). Points mapped world→screen via `view`; sizes/widths scaled by `size_scale`
/// (use view.zoom for artwork so strokes thicken with zoom, or 1.0 for overlay/UI to keep constant screen size).
pub fn build_fg(prims: &[Prim], view: View, size_scale: f32, w: f32, h: f32) -> Vec<Vertex> {
    let mut v = Vec::new();
    let z = size_scale;
    for prim in prims {
        match prim {
            Prim::Fill { .. } => {}
            // `clip` is honoured at DRAW time (a GPU scissor set around this stroke's Fg range) — the band
            // is tessellated here in full and trimmed to the page edge by the scissor. See build_content.
            Prim::Stroke { pts, width, color, .. } => {
                let sp: Vec<Pt> = pts.iter().map(|p| view.w2s(*p)).collect();
                stroke_poly(&mut v, &sp, width * z, *color, w, h);
            }
            Prim::Dashed { pts, width, color } => {
                let sp: Vec<Pt> = pts.iter().map(|p| view.w2s(*p)).collect();
                dashed_poly(&mut v, &sp, width * z, *color, w, h);
            }
            Prim::Square { c, half, color } => sq(&mut v, view.w2s(*c), half * z, *color, w, h),
            Prim::Disc { c, r, color } => disc(&mut v, view.w2s(*c), r * z, *color, w, h),
            Prim::Tri { a, b, c, color } => tri(&mut v, view.w2s(*a), view.w2s(*b), view.w2s(*c), *color, w, h),
        }
    }
    v
}

/// One draw step inside a group, in PAINT ORDER. `Fill` = a stencil fan + cover quad (ranges into the
/// shared fill buffer); `Fg` = a run of stroke/marker triangles (range into the shared fg buffer);
/// `StrokeCov` = a TRANSLUCENT stroke — its self-overlapping segment quads + join discs stencil-MARK the
/// covered pixels (colour writes off), then a bbox cover quad paints the whole band ONCE at the stroke
/// colour, so no pixel double-blends (an opaque stroke doesn't need this: overlap is invisible). Steps
/// are emitted per object — each object's fill directly before its own stroke — so an object above covers
/// the stroke of the one below (Illustrator stacking), instead of all strokes floating above all fills.
/// `Knockout` = one filled object with a translucent stroke: mark the band (stencil bit 0x80), even-odd
/// fan the fill (bit 0x01), paint the fill only where inside AND NOT under the band, then paint the band
/// once — so the stroke blends against what's BEHIND the object, never against its own fill.
pub enum Draw {
    Fill { fan: (u32, u32), cover: (u32, u32) },
    // `scissor` = a pixel-space rect [x, y, w, h] to confine this run to (A2: an artboard-clipped OPAQUE
    // stroke, so its extruded band is trimmed to the page edge, not just its centerline). `None` = draw
    // across the whole framebuffer (the usual case). A degenerate/off-screen clip resolves to `None` in
    // `scissor_px`, so a missed clip draws UNCLIPPED (overflowing) — never clipped-to-nothing. Fail-open.
    Fg { range: (u32, u32), scissor: Option<[u32; 4]> },
    StrokeCov { tris: (u32, u32), cover: (u32, u32) },
    Knockout { band: (u32, u32), fan: (u32, u32), fcover: (u32, u32), bcover: (u32, u32) },
}

/// Map a world-space clip rect `[x0,y0,x1,y1]` to an integer pixel scissor `[x, y, w, h]` on a `w`×`h`
/// framebuffer, via the canvas `view` (pure translate+scale → axis-aligned). Rounds OUTWARD (floor the
/// min, ceil the max) so the scissor is never tighter than the page — it only trims the band OVERHANG,
/// never in-page pixels. Clamped to the framebuffer so wgpu always gets a valid rect. Returns `None` when
/// the visible rect is empty/sub-pixel/off-screen: the caller then draws UNCLIPPED (fail-open — a missed
/// clip overflows, it never vanishes). Pure (no GPU) → unit-tested headlessly.
pub fn scissor_px(rect: [f32; 4], view: View, w: f32, h: f32) -> Option<[u32; 4]> {
    let a = view.w2s([rect[0], rect[1]]);
    let b = view.w2s([rect[2], rect[3]]);
    let (sx0, sx1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (sy0, sy1) = (a[1].min(b[1]), a[1].max(b[1]));
    let x0 = sx0.floor().clamp(0.0, w);
    let y0 = sy0.floor().clamp(0.0, h);
    let x1 = sx1.ceil().clamp(0.0, w);
    let y1 = sy1.ceil().clamp(0.0, h);
    // sub-pixel or empty after clamping ⇒ no scissor (never a zero-size rect that would clip to nothing)
    if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
        return None;
    }
    Some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
}

/// How to draw one content Group on the GPU. `Layer` is an isolated translucent object: render its draws
/// opaquely into an offscreen buffer, then composite `quad` (a fullscreen quad carrying its opacity) onto
/// the scene. `Clip` is a CLIPPING MASK (MASKS_PLAN §3.1): fan `mask_fan` into the dedicated clip stencil
/// bit `0x02`, replay `members` with the clip test, then `mask_clear` zeros `0x02` — all in ONE render
/// pass so the clip bit persists across the member draws (each scene pass clears the stencil at entry).
/// `mask_fan`/`mask_clear` are ranges into the shared FILL buffer (the ring fan + its bbox cover quad).
pub enum GroupDraw {
    Opaque { draws: Vec<Draw> },
    Layer { draws: Vec<Draw>, quad: (u32, u32) },
    Clip { mask_fan: (u32, u32), mask_clear: (u32, u32), members: Vec<Draw> },
}

/// One object's knockout steps: band triangles + band bbox cover from its stroke prims, and the fill's
/// fan + cover. The renderer stencils the band, fans the fill, paints fill-outside-band, then the band.
fn knock_draws(
    prims: &[Prim],
    view: View,
    zoom: f32,
    w: f32,
    h: f32,
    fillv: &mut Vec<Vertex>,
    fgv: &mut Vec<Vertex>,
) -> Vec<Draw> {
    let t0 = fgv.len() as u32;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut bcol = [0.0f32; 4];
    for p in prims {
        if let Prim::Stroke { pts, width, color, .. } = p {
            bcol = *color;
            let sp: Vec<Pt> = pts.iter().map(|q| view.w2s(*q)).collect();
            let r = width * zoom * 0.5 + 1.5;
            for q in &sp {
                x0 = x0.min(q[0] - r);
                y0 = y0.min(q[1] - r);
                x1 = x1.max(q[0] + r);
                y1 = y1.max(q[1] + r);
            }
            stroke_poly(fgv, &sp, width * zoom, bcol, w, h);
        }
    }
    let band = (t0, fgv.len() as u32 - t0);
    let (fv, fr) = build_fills(prims, view, w, h);
    let off = fillv.len() as u32;
    fillv.extend(fv);
    let (fan, fcover) =
        fr.first().map(|((fs, fl), (cs, cl))| ((*fs + off, *fl), (*cs + off, *cl))).unwrap_or(((0, 0), (0, 0)));
    let c0 = fgv.len() as u32;
    if band.1 > 0 {
        quad(fgv, [x0, y0], [x1, y0], [x1, y1], [x0, y1], bcol, w, h);
    }
    vec![Draw::Knockout { band, fan, fcover, bcover: (c0, fgv.len() as u32 - c0) }]
}

/// Does this (single-object) prim set need knockout? = has a fill AND a translucent stroke.
fn needs_knockout(prims: &[Prim]) -> bool {
    prims.iter().any(|p| matches!(p, Prim::Fill { .. }))
        && prims.iter().any(|p| matches!(p, Prim::Stroke { color, .. } if color[3] < 0.999))
}

/// Build ONE group's ordered draw steps (fill fan+cover, stroke fg, translucent-stroke mark+cover,
/// knockout) into the shared fill/fg buffers. Extracted so a clip's members reuse the EXACT same routing
/// (MASKS_PLAN §2.4). A `Clip` member has no direct draws (its paint is its own members) → empty here;
/// nested clips are a later stage.
fn group_draws(
    g: &Group,
    view: View,
    zoom: f32,
    w: f32,
    h: f32,
    fillv: &mut Vec<Vertex>,
    fgv: &mut Vec<Vertex>,
) -> Vec<Draw> {
    if matches!(g, Group::Clip { .. }) {
        return Vec::new();
    }
    // knockout objects (and isolated layers that contain a translucent stroke) take the dedicated path
    if matches!(g, Group::Knockout(_)) || matches!(g, Group::Isolated { prims, .. } if needs_knockout(prims)) {
        return knock_draws(g.prims(), view, zoom, w, h, fillv, fgv);
    }
    let prims = g.prims();
    let mut draws = Vec::new();
    let mut i = 0;
    while i < prims.len() {
        if matches!(prims[i], Prim::Fill { .. }) {
            // one fill → its own stencil+cover step (offset into the shared fill buffer)
            let (fv, fr) = build_fills(&prims[i..i + 1], view, w, h);
            let off = fillv.len() as u32;
            fillv.extend(fv);
            for ((fs, fl), (cs, cl)) in fr {
                draws.push(Draw::Fill { fan: (fs + off, fl), cover: (cs + off, cl) });
            }
            i += 1;
        } else {
            // a run of consecutive non-fill prims. Opaque ones coalesce into plain fg steps. A
            // TRANSLUCENT stroke (colour alpha < 1 — from the colour itself or folded object opacity)
            // must paint its overlapping quads + join discs EXACTLY ONCE → stencil-mark + cover step
            // (otherwise every overlap re-blends and the band turns into the blotchy "blur").
            let j = (i..prims.len()).find(|&k| matches!(prims[k], Prim::Fill { .. })).unwrap_or(prims.len());
            while i < j {
                if let Prim::Stroke { color, .. } = &prims[i] {
                    if color[3] < 0.999 {
                        let col = *color;
                        // an object's outer + hole rings share one colour → mark them together so
                        // even ring-vs-ring overlap of one object's stroke still paints once
                        let e = (i..j)
                            .find(|&k| !matches!(&prims[k], Prim::Stroke { color: c2, .. } if *c2 == col))
                            .unwrap_or(j);
                        let t0 = fgv.len() as u32;
                        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                        for p in &prims[i..e] {
                            if let Prim::Stroke { pts, width, .. } = p {
                                let sp: Vec<Pt> = pts.iter().map(|q| view.w2s(*q)).collect();
                                let r = width * zoom * 0.5 + 1.5;
                                for q in &sp {
                                    x0 = x0.min(q[0] - r);
                                    y0 = y0.min(q[1] - r);
                                    x1 = x1.max(q[0] + r);
                                    y1 = y1.max(q[1] + r);
                                }
                                stroke_poly(fgv, &sp, width * zoom, col, w, h);
                            }
                        }
                        let tris = (t0, fgv.len() as u32 - t0);
                        let c0 = fgv.len() as u32;
                        quad(fgv, [x0, y0], [x1, y0], [x1, y1], [x0, y1], col, w, h);
                        draws.push(Draw::StrokeCov { tris, cover: (c0, fgv.len() as u32 - c0) });
                        i = e;
                        continue;
                    }
                }
                // A2: an artboard-clipped OPAQUE stroke draws ALONE under a GPU scissor set to its page
                // rect, so the extruded band is trimmed to the page edge (not just its centerline). One
                // clipped stroke ⇒ one page rect (scene.rs emits one Stroke per rect), so one scissor is
                // unambiguous. Translucent bands stay on the centerline clip (the stencil StrokeCov path,
                // above) — a sub-half-width, semi-transparent overhang, left untouched to keep that path
                // stable. A degenerate clip ⇒ scissor None ⇒ drawn uncut (fail-open).
                if let Prim::Stroke { clip: Some(rect), .. } = &prims[i] {
                    let scissor = scissor_px(*rect, view, w, h);
                    let start = fgv.len() as u32;
                    fgv.extend(build_fg(&prims[i..=i], view, zoom, w, h));
                    let n = fgv.len() as u32 - start;
                    if n > 0 {
                        draws.push(Draw::Fg { range: (start, n), scissor });
                    }
                    i += 1;
                    continue;
                }
                // opaque strokes / dashes etc. — coalesce until the next translucent OR clipped stroke
                // (a clipped stroke needs its own scissored draw, so it can't share a coalesced range)
                let e = (i + 1..j)
                    .find(|&k| {
                        matches!(&prims[k], Prim::Stroke { color, clip, .. } if color[3] < 0.999 || clip.is_some())
                    })
                    .unwrap_or(j);
                let start = fgv.len() as u32;
                fgv.extend(build_fg(&prims[i..e], view, zoom, w, h));
                let n = fgv.len() as u32 - start;
                if n > 0 {
                    draws.push(Draw::Fg { range: (start, n), scissor: None });
                }
                i = e;
            }
        }
    }
    draws
}

/// The mask silhouette's stencil geometry: fan triangles (even-odd into clip bit `0x02`) + a bbox cover
/// quad (to zero `0x02` afterward), both appended to the shared FILL buffer. Reuses `build_fills`, whose
/// first range is exactly (fan, cover). Colour is irrelevant — these draw with colour writes OFF.
fn mask_ranges(
    mask_rings: &[Vec<Pt>],
    view: View,
    w: f32,
    h: f32,
    fillv: &mut Vec<Vertex>,
) -> ((u32, u32), (u32, u32)) {
    let prim = [Prim::Fill { rings: mask_rings.to_vec(), color: [0.0, 0.0, 0.0, 0.0] }];
    let (mv, mr) = build_fills(&prim, view, w, h);
    let off = fillv.len() as u32;
    fillv.extend(mv);
    match mr.first() {
        Some(&((fs, fl), (cs, cl))) => ((fs + off, fl), (cs + off, cl)),
        None => ((0, 0), (0, 0)),
    }
}

/// Tessellate every content Group into three shared vertex buffers — fills (fan+cover), strokes (fg), and
/// composite quads (op) — plus per-group draw steps that preserve the group's internal paint order.
/// Consecutive on-canvas groups (Opaque/Knockout) merge into one GroupDraw::Opaque → one render pass.
pub fn build_content(
    groups: &[Group],
    view: View,
    zoom: f32,
    w: f32,
    h: f32,
) -> (Vec<Vertex>, Vec<Vertex>, Vec<Vertex>, Vec<GroupDraw>) {
    let mut fillv = Vec::new();
    let mut fgv = Vec::new();
    let mut opv = Vec::new();
    let mut metas: Vec<GroupDraw> = Vec::new();
    for g in groups {
        // a CLIPPING MASK is its own self-contained render pass (mask fan → clip-tested members → clear):
        // it never coalesces with the opaque run, so its clip bit can't leak into neighbours.
        if let Group::Clip { mask_rings, members } = g {
            let (mask_fan, mask_clear) = mask_ranges(mask_rings, view, w, h, &mut fillv);
            let mut member_draws = Vec::new();
            for m in members {
                member_draws.extend(group_draws(m, view, zoom, w, h, &mut fillv, &mut fgv));
            }
            metas.push(GroupDraw::Clip { mask_fan, mask_clear, members: member_draws });
            continue;
        }
        let draws = group_draws(g, view, zoom, w, h, &mut fillv, &mut fgv);
        push_group(&mut metas, &mut opv, g, draws);
    }
    (fillv, fgv, opv, metas)
}

/// File a group's draws: isolated layers get their composite quad; on-canvas groups (Opaque/Knockout)
/// merge into the previous Opaque meta when adjacent — knockout steps are stencil-self-cleaning, so they
/// share a render pass with plain content (no extra pass per knockout object).
fn push_group(metas: &mut Vec<GroupDraw>, opv: &mut Vec<Vertex>, g: &Group, draws: Vec<Draw>) {
    match g {
        Group::Isolated { opacity, .. } => {
            let qs = opv.len() as u32;
            fullscreen_quad(opv, *opacity);
            metas.push(GroupDraw::Layer { draws, quad: (qs, opv.len() as u32 - qs) });
        }
        _ => {
            if let Some(GroupDraw::Opaque { draws: prev }) = metas.last_mut() {
                prev.extend(draws);
            } else {
                metas.push(GroupDraw::Opaque { draws });
            }
        }
    }
}

/// A full-NDC quad (two triangles); the object opacity rides in colour.a for the composite shader.
fn fullscreen_quad(v: &mut Vec<Vertex>, opacity: f32) {
    let col = [0.0, 0.0, 0.0, opacity];
    let (a, b, c, d) = ([-1.0f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]);
    for p in [a, b, c, a, c, d] {
        v.push(Vertex { pos: p, color: col });
    }
}

// CPU-pure tessellation tests (no GPU): lock the translucent-stroke routing — mark+cover paints the
// self-overlapping band exactly once; opaque strokes stay on the fast single-draw path.
#[cfg(test)]
mod tests {
    use super::*;
    use varos_core::scene::Group;

    fn stroke(alpha: f32) -> Prim {
        Prim::Stroke {
            pts: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
            width: 4.0,
            color: [0.0, 0.0, 0.0, alpha],
            clip: None,
        }
    }

    #[test]
    fn adaptive_subdivision_points_do_not_create_round_join_discs() {
        let mut vertices = Vec::new();
        stroke_poly(
            &mut vertices,
            &[[0.0, 0.0], [10.0, 0.1], [20.0, 0.3], [30.0, 0.6]],
            4.0,
            [0.0, 0.0, 0.0, 1.0],
            100.0,
            100.0,
        );
        // Three segment quads (18 vertices) plus two 24-triangle round caps (144 vertices).
        assert_eq!(vertices.len(), 162);
    }

    #[test]
    fn a_real_corner_keeps_its_round_join_disc() {
        let mut vertices = Vec::new();
        stroke_poly(&mut vertices, &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], 4.0, [0.0, 0.0, 0.0, 1.0], 100.0, 100.0);
        // Two segment quads (12) plus two caps and the 90-degree join (3 * 72).
        assert_eq!(vertices.len(), 228);
    }

    #[test]
    fn translucent_stroke_goes_through_mark_and_cover() {
        let g = [Group::Opaque(vec![stroke(0.5)])];
        let (_f, fgv, _o, metas) = build_content(&g, View::identity(), 1.0, 100.0, 100.0);
        let draws = match &metas[0] {
            GroupDraw::Opaque { draws } => draws,
            _ => panic!("opaque group expected"),
        };
        let (tris, cover) = match draws[0] {
            Draw::StrokeCov { tris, cover } => (tris, cover),
            _ => panic!("a translucent stroke must mark+cover (paint once)"),
        };
        assert!(tris.1 > 0 && cover.1 == 6, "mark triangles + one cover quad");
        assert!((fgv[cover.0 as usize].color[3] - 0.5).abs() < 1e-6, "the cover quad carries the stroke's alpha");
    }

    #[test]
    fn opaque_stroke_stays_on_the_fast_path() {
        let g = [Group::Opaque(vec![stroke(1.0)])];
        let (_f, _fg, _o, metas) = build_content(&g, View::identity(), 1.0, 100.0, 100.0);
        let draws = match &metas[0] {
            GroupDraw::Opaque { draws } => draws,
            _ => panic!("opaque group expected"),
        };
        match draws[0] {
            Draw::Fg { scissor, .. } => assert!(scissor.is_none(), "an unclipped stroke carries no scissor"),
            _ => panic!("an opaque stroke needs no stencil pass"),
        }
    }

    fn clipped_stroke(rect: [f32; 4]) -> Prim {
        Prim::Stroke {
            pts: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
            width: 4.0,
            color: [0.0, 0.0, 0.0, 1.0],
            clip: Some(rect),
        }
    }

    #[test]
    fn clipped_opaque_stroke_draws_alone_under_a_scissor() {
        // an opaque stroke carrying a page rect must draw ON ITS OWN with a scissor set to that rect,
        // so the extruded band (not just the centerline) is trimmed to the page edge.
        let g = [Group::Opaque(vec![clipped_stroke([2.0, 3.0, 40.0, 50.0])])];
        let (_f, _fg, _o, metas) = build_content(&g, View::identity(), 1.0, 100.0, 100.0);
        let draws = match &metas[0] {
            GroupDraw::Opaque { draws } => draws,
            _ => panic!("opaque group expected"),
        };
        match draws[0] {
            Draw::Fg { scissor: Some(s), .. } => assert_eq!(s, [2, 3, 38, 47], "scissor = the page rect in px"),
            _ => panic!("a clipped opaque stroke must carry a scissor"),
        }
    }

    #[test]
    fn clipped_stroke_does_not_coalesce_with_its_neighbour() {
        // a plain stroke followed by a clipped one → TWO Fg draws (the clipped one can't share the
        // coalesced range because it needs its own scissor). The first stays unclipped.
        let g = [Group::Opaque(vec![stroke(1.0), clipped_stroke([0.0, 0.0, 20.0, 20.0])])];
        let (_f, _fg, _o, metas) = build_content(&g, View::identity(), 1.0, 100.0, 100.0);
        let draws = match &metas[0] {
            GroupDraw::Opaque { draws } => draws,
            _ => panic!("opaque group expected"),
        };
        assert_eq!(draws.len(), 2, "clipped stroke splits off into its own draw");
        assert!(matches!(draws[0], Draw::Fg { scissor: None, .. }), "the plain stroke stays unclipped");
        assert!(matches!(draws[1], Draw::Fg { scissor: Some(_), .. }), "the clipped stroke is scissored");
    }

    #[test]
    fn scissor_px_maps_and_rounds_outward() {
        // identity view: world rect → the same pixel rect, rounded outward (fractional edges grow the box)
        assert_eq!(scissor_px([10.0, 20.0, 110.0, 120.0], View::identity(), 200.0, 200.0), Some([10, 20, 100, 100]));
        assert_eq!(scissor_px([10.4, 20.6, 30.2, 40.9], View::identity(), 200.0, 200.0), Some([10, 20, 21, 21]));
    }

    #[test]
    fn scissor_px_honours_the_view_transform() {
        // screen = world*zoom + pan; a 2× zoom with a pan shifts+scales the rect
        let v = View { pan: [5.0, 7.0], zoom: 2.0 };
        // x: 10..60 → 25..125, y: 0..10 → 7..27
        assert_eq!(scissor_px([10.0, 0.0, 60.0, 10.0], v, 300.0, 300.0), Some([25, 7, 100, 20]));
    }

    #[test]
    fn scissor_px_degenerate_or_offscreen_is_none() {
        // an empty (zero-width) rect and fully off-screen rects (left/above and right/below the window)
        // all disable the scissor → the stroke draws UNCLIPPED (fail-open: a missed clip overflows the
        // page, it never clips the stroke to nothing).
        assert_eq!(scissor_px([10.0, 20.0, 10.0, 120.0], View::identity(), 200.0, 200.0), None);
        assert_eq!(scissor_px([-100.0, -100.0, -50.0, -50.0], View::identity(), 200.0, 200.0), None);
        assert_eq!(scissor_px([300.0, 300.0, 400.0, 400.0], View::identity(), 200.0, 200.0), None);
    }

    #[test]
    fn scissor_px_clamps_to_the_framebuffer() {
        // a page bigger than the window → clamped to [0,w]×[0,h] so wgpu always gets a valid rect
        assert_eq!(scissor_px([-50.0, -50.0, 500.0, 500.0], View::identity(), 200.0, 150.0), Some([0, 0, 200, 150]));
    }

    #[test]
    fn knockout_object_emits_band_fan_and_two_covers() {
        let fill = Prim::Fill {
            rings: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]],
            color: [0.0, 1.0, 0.0, 1.0],
        };
        let g = [Group::Knockout(vec![fill, stroke(0.5)])];
        let (_f, fgv, _o, metas) = build_content(&g, View::identity(), 1.0, 100.0, 100.0);
        let draws = match &metas[0] {
            GroupDraw::Opaque { draws } => draws,
            _ => panic!("knockout draws inline (opaque pass)"),
        };
        let (band, fan, fcover, bcover) = match draws[0] {
            Draw::Knockout { band, fan, fcover, bcover } => (band, fan, fcover, bcover),
            _ => panic!("a filled object with a translucent stroke must knock out"),
        };
        assert!(band.1 > 0 && fan.1 > 0 && fcover.1 == 6 && bcover.1 == 6, "band tris + fill fan + both covers");
        assert!((fgv[bcover.0 as usize].color[3] - 0.5).abs() < 1e-6, "the band cover carries the stroke's alpha");
    }

    #[test]
    fn clip_group_emits_mask_fan_members_and_clear() {
        // MASKS_PLAN §3.1 / Stage 2: a clip group tessellates to a GroupDraw::Clip carrying a mask fan
        // (into clip bit 0x02), the clipped member draw steps, and a mask-bbox clear. A doc with no clip
        // never produces this variant (proven by every other test staying Opaque/Layer).
        let mask = vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]];
        let fill = Prim::Fill {
            rings: vec![vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]]],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let g = [Group::Clip { mask_rings: mask, members: vec![Group::Opaque(vec![fill])] }];
        let (_f, _fg, _o, metas) = build_content(&g, View::identity(), 1.0, 100.0, 100.0);
        let (mask_fan, mask_clear, members) = match &metas[0] {
            GroupDraw::Clip { mask_fan, mask_clear, members } => (mask_fan, mask_clear, members),
            _ => panic!("a clip group must emit GroupDraw::Clip"),
        };
        assert!(mask_fan.1 > 0, "the mask silhouette is fanned into the clip stencil bit");
        assert!(mask_clear.1 == 6, "the mask bbox clear is one quad (6 verts)");
        assert!(matches!(members.first(), Some(Draw::Fill { .. })), "the clipped member's fill is a Draw::Fill");
    }

    #[test]
    fn isolated_layer_with_translucent_stroke_knocks_out_inside_the_layer() {
        let fill = Prim::Fill {
            rings: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]],
            color: [0.0, 1.0, 0.0, 1.0],
        };
        let g = [Group::Isolated { opacity: 0.5, prims: vec![fill, stroke(0.5)] }];
        let (_f, _fg, _o, metas) = build_content(&g, View::identity(), 1.0, 100.0, 100.0);
        let draws = match &metas[0] {
            GroupDraw::Layer { draws, .. } => draws,
            _ => panic!("layer expected"),
        };
        assert!(matches!(draws[0], Draw::Knockout { .. }), "knockout also applies inside an isolated layer");
    }
}
