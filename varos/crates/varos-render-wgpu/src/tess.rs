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
/// The half-width offset of segment `a→b`: the vector from the centerline to one long side of its quad.
/// Used by dashed lines; solid strokes keep their segment frames in f64 below.
fn seg_normal(a: Pt, b: Pt, width: f32) -> Pt {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
    [-d[1] / l * width / 2.0, d[0] / l * width / 2.0]
}
fn line(v: &mut Vec<Vertex>, a: Pt, b: Pt, width: f32, col: [f32; 4], w: f32, h: f32) {
    let n = seg_normal(a, b, width);
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
// Keep the world→screen multiply/add, offsets and NDC conversion in f64. Rounding an
// absolute screen point to f32 before subtracting its neighbour loses the small turn.
type StrokePt = [f64; 2];
fn stroke_screen(p: Pt, view: View) -> StrokePt {
    [p[0] as f64 * view.zoom as f64 + view.pan[0] as f64, p[1] as f64 * view.zoom as f64 + view.pan[1] as f64]
}
fn stroke_vertex(p: StrokePt, col: [f32; 4], w: f32, h: f32) -> Vertex {
    Vertex { pos: [(p[0] / w as f64 * 2.0 - 1.0) as f32, (1.0 - p[1] / h as f64 * 2.0) as f32], color: col }
}
fn stroke_tri(v: &mut Vec<Vertex>, a: StrokePt, b: StrokePt, c: StrokePt, col: [f32; 4], w: f32, h: f32) {
    v.extend([a, b, c].map(|p| stroke_vertex(p, col, w, h)));
}
// Bounds use the same screen points and rounded screen width as stroke_poly. Do not
// round the centre, radius, padding or cover coordinates before the final NDC cast.
fn extend_stroke_bounds(bounds: &mut [f64; 4], pts: &[StrokePt], width: f32) {
    let r = width as f64 * 0.5 + 1.5;
    for p in pts {
        bounds[0] = bounds[0].min(p[0] - r);
        bounds[1] = bounds[1].min(p[1] - r);
        bounds[2] = bounds[2].max(p[0] + r);
        bounds[3] = bounds[3].max(p[1] + r);
    }
}
fn stroke_cover(v: &mut Vec<Vertex>, bounds: [f64; 4], col: [f32; 4], w: f32, h: f32) {
    let [x0, y0, x1, y1] = bounds;
    stroke_tri(v, [x0, y0], [x1, y0], [x1, y1], col, w, h);
    stroke_tri(v, [x0, y0], [x1, y1], [x0, y1], col, w, h);
}
#[derive(Clone, Copy)]
struct StrokeEnd {
    dir: StrokePt,
    normal: StrokePt,
    // The +/- normal quad corners, calculated in f64 and cast exactly once. The
    // fan reuses these emitted vertices, so shared edges coincide by construction.
    corners: [Vertex; 2],
}
impl StrokeEnd {
    /// The same end seen from the other side: heading reversed, normal negated, corners swapped. The
    /// corners are the SAME emitted vertices, so a fan built against it shares the quad's edge exactly.
    fn reversed(self) -> StrokeEnd {
        StrokeEnd {
            dir: [-self.dir[0], -self.dir[1]],
            normal: [-self.normal[0], -self.normal[1]],
            corners: [self.corners[1], self.corners[0]],
        }
    }
}
/// A round cap on `end` (the frame heading INTO the cap at `c`): a half-disc built as a 180°
/// `stroke_join` against the same frame reversed. Caps therefore get exactly the joins' screen-space
/// subdivision (sagitta <= JOIN_TOL_PX, at least 4 chords, at most JOIN_MAX_STEPS) and pivot on the
/// quad's own corners (no T-junction). At 180° the join's last chord ends ON its pivot; that final
/// zero-area triangle is dropped, because the previous chord already closes the half-disc.
#[allow(clippy::too_many_arguments)] // centre + frame + radius + paint + framebuffer
fn stroke_cap(v: &mut Vec<Vertex>, c: StrokePt, end: StrokeEnd, r: f64, col: [f32; 4], w: f32, h: f32) {
    let start = v.len();
    stroke_join(v, c, end, end.reversed(), r, col, w, h);
    let n = v.len();
    if n - start >= 6 && v[n - 1].pos == v[n - 3].pos {
        v.truncate(n - 3);
    }
}
fn stroke_poly(v: &mut Vec<Vertex>, pts: &[StrokePt], width: f32, col: [f32; 4], w: f32, h: f32) {
    if pts.len() < 2 {
        return;
    }
    let joins = width >= 1.6;
    let r = width as f64 * 0.5;
    v.reserve((pts.len() - 1) * 6 + if joins { pts.len() * 3 + 24 } else { 0 });
    // Only exact duplicates are skipped. Every nonzero segment gets a full-width quad
    // and a join, including segments shorter than 1e-3 px. Duplicates keep the same joint.
    let mut first: Option<StrokeEnd> = None;
    let mut prev: Option<StrokeEnd> = None;
    for edge in pts.windows(2) {
        let (a, b) = (edge[0], edge[1]);
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = d[0].hypot(d[1]);
        if len == 0.0 {
            continue;
        }
        let dir = [d[0] / len, d[1] / len];
        let n = [-dir[1] * r, dir[0] * r];
        let (ap, bp, bm, am) = (
            [a[0] + n[0], a[1] + n[1]],
            [b[0] + n[0], b[1] + n[1]],
            [b[0] - n[0], b[1] - n[1]],
            [a[0] - n[0], a[1] - n[1]],
        );
        let [ap, bp, bm, am] = [ap, bp, bm, am].map(|p| stroke_vertex(p, col, w, h));
        v.extend([ap, bp, bm, ap, bm, am]);
        let outgoing = StrokeEnd { dir, normal: n, corners: [ap, am] };
        if joins {
            if let Some(incoming) = prev {
                stroke_join(v, a, incoming, outgoing, r, col, w, h);
            }
        }
        first.get_or_insert(outgoing);
        prev = Some(StrokeEnd { dir, normal: n, corners: [bp, bm] });
    }
    if !joins {
        return;
    }
    let (c0, c1) = (pts[0], pts[pts.len() - 1]);
    match (first, prev) {
        // A closed ring arrives with its first point repeated at the end (`ring_px` ends the closing
        // segment exactly on anchor 0; hole rings push it again). Its seam is a JOIN, never two caps.
        // Two half-caps would also cover the seam; the join is the cheaper choice. An open path whose
        // ends happen to coincide gets the same coverage as any interior round join.
        (Some(first), Some(last)) if c0 == c1 => stroke_join(v, c0, last, first, r, col, w, h),
        (Some(first), Some(last)) => {
            stroke_cap(v, c0, first.reversed(), r, col, w, h);
            stroke_cap(v, c1, last, r, col, w, h);
        }
        // Every segment has zero length: two half-caps on an arbitrary axis keep the round dot.
        _ => {
            let corners = [[c0[0], c0[1] + r], [c0[0], c0[1] - r]].map(|p| stroke_vertex(p, col, w, h));
            let end = StrokeEnd { dir: [1.0, 0.0], normal: [0.0, r], corners };
            stroke_cap(v, c0, end, r, col, w, h);
            stroke_cap(v, c0, end.reversed(), r, col, w, h);
        }
    }
}

/// Maximum chord sagitta in screen pixels, until the bounded fan reaches its cap.
const JOIN_TOL_PX: f32 = 0.25;
const JOIN_MAX_STEPS: usize = 128;

/// Close every nonzero turn using the quad's already-emitted corners.
#[allow(clippy::too_many_arguments)] // two segment frames + radius + paint + framebuffer
fn stroke_join(
    v: &mut Vec<Vertex>,
    b: StrokePt,
    incoming: StrokeEnd,
    outgoing: StrokeEnd,
    r: f64,
    col: [f32; 4],
    w: f32,
    h: f32,
) {
    let (din, dout, nin) = (incoming.dir, outgoing.dir, incoming.normal);
    let cos = (din[0] * dout[0] + din[1] * dout[1]).clamp(-1.0, 1.0);
    let cross = din[0] * dout[1] - din[1] * dout[0];
    if cross == 0.0 && cos >= 0.0 {
        return;
    }
    let s = if cross > 0.0 { -1.0 } else { 1.0 };
    let outer = usize::from(cross > 0.0);
    let cin = incoming.corners[outer];
    let cout = outgoing.corners[outer];
    let theta = cross.abs().atan2(cos);
    // Unlike 1-cos(theta/2), this retains tiny angles at huge radii. Sagitta
    // chooses subdivision ONLY: no area/angle threshold may remove a closing wedge.
    let sagitta = 2.0 * r * (theta * 0.25).sin().powi(2);
    let steps = if theta <= std::f64::consts::FRAC_PI_4 && sagitta <= JOIN_TOL_PX as f64 {
        1
    } else {
        let max_step = 4.0 * ((JOIN_TOL_PX as f64 / (2.0 * r)).min(1.0).sqrt()).asin();
        (theta / max_step).ceil().max((theta / std::f64::consts::FRAC_PI_4).ceil()).max(1.0) as usize
    }
    .min(JOIN_MAX_STEPS);
    // Anchor on the incoming quad's INNER corner, not the centerline midpoint.
    // The latter is a T-junction: after f32 NDC rounding it need not lie on the
    // quad's end edge. Sharing that entire edge removes the radial crack; the
    // outgoing edge overlaps its quad. The fan stays inside the round-join disk.
    let pivot = incoming.corners[1 - outer];
    let mut prev = cin;
    // One sin_cos per fan, then rotate the offset step by step (f64, at most 128 steps: the drift is
    // ~1e-13 of r). The last vertex is always the quad's own corner, so the shared edge stays exact.
    let (sd, cd) = (-s * theta / steps as f64).sin_cos();
    let mut n = [nin[0] * s, nin[1] * s];
    for k in 1..=steps {
        let next = if k == steps {
            cout
        } else {
            n = [n[0] * cd - n[1] * sd, n[0] * sd + n[1] * cd];
            stroke_vertex([b[0] + n[0], b[1] + n[1]], col, w, h)
        };
        v.extend([pivot, prev, next]);
        prev = next;
    }
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
                let sp: Vec<StrokePt> = pts.iter().map(|p| stroke_screen(*p, view)).collect();
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
    let mut bounds = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let mut bcol = [0.0f32; 4];
    for p in prims {
        if let Prim::Stroke { pts, width, color, .. } = p {
            bcol = *color;
            let sp: Vec<StrokePt> = pts.iter().map(|q| stroke_screen(*q, view)).collect();
            let screen_width = width * zoom;
            extend_stroke_bounds(&mut bounds, &sp, screen_width);
            stroke_poly(fgv, &sp, screen_width, bcol, w, h);
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
        stroke_cover(fgv, bounds, bcol, w, h);
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
                        let mut bounds = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
                        for p in &prims[i..e] {
                            if let Prim::Stroke { pts, width, .. } = p {
                                let sp: Vec<StrokePt> = pts.iter().map(|q| stroke_screen(*q, view)).collect();
                                let screen_width = width * zoom;
                                extend_stroke_bounds(&mut bounds, &sp, screen_width);
                                stroke_poly(fgv, &sp, screen_width, col, w, h);
                            }
                        }
                        let tris = (t0, fgv.len() as u32 - t0);
                        let c0 = fgv.len() as u32;
                        stroke_cover(fgv, bounds, col, w, h);
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

    fn stroke_poly(v: &mut Vec<Vertex>, pts: &[Pt], width: f32, col: [f32; 4], w: f32, h: f32) {
        let pts: Vec<StrokePt> = pts.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
        super::stroke_poly(v, &pts, width, col, w, h);
    }

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
        // Three segment quads (18 vertices), two half-disc caps (4 chords = 3 triangles each at r = 2 px,
        // 18 vertices) and one bevel wedge (3 vertices) at each of the two gentle turns — never a disc there.
        assert_eq!(vertices.len(), 42);
    }

    #[test]
    fn a_real_corner_gets_a_round_outer_join() {
        let mut vertices = Vec::new();
        stroke_poly(&mut vertices, &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], 4.0, [0.0, 0.0, 0.0, 1.0], 100.0, 100.0);
        // Two segment quads (12), two 3-triangle half-disc caps (18) and the 90° join as a two-triangle
        // outer round sector (6): at r = 2 px a 45° chord already sits within 0.25 px of the circle.
        assert_eq!(vertices.len(), 36);
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

    /// Review P1-1, end to end on the CPU: two crossing 50%-red strokes separated in z by an opaque
    /// rectangle that is wholly off screen. Uncut, the rectangle's fill splits them into two coverage
    /// draws (the crossing paints twice). Culling the rectangle must NOT merge them into one.
    #[test]
    fn culling_keeps_one_coverage_draw_per_translucent_object() {
        use varos_core::editor::Editor;
        use varos_core::model::{Anchor, Path};
        use varos_core::scene::{build_scene, build_scene_in_view};
        let anc = |id: u32, x: f32, y: f32| Anchor { id, p: [x, y], hin: None, hout: None, smooth: false };
        let red = Some([1.0, 0.0, 0.0, 0.5]);
        let mut ed = Editor::new();
        ed.doc.paths = vec![
            Path::new(1, vec![anc(100, 100.0, 100.0), anc(101, 500.0, 400.0)], false, None, red, 6.0),
            Path::new(
                2,
                vec![
                    anc(200, 3_000.0, 0.0),
                    anc(201, 3_100.0, 0.0),
                    anc(202, 3_100.0, 100.0),
                    anc(203, 3_000.0, 100.0),
                ],
                true,
                Some([0.2, 0.2, 0.2, 1.0]),
                None,
                1.0,
            ),
            Path::new(3, vec![anc(300, 100.0, 400.0), anc(301, 500.0, 100.0)], false, None, red, 6.0),
        ];
        ed.doc.ids = 10_000;
        ed.doc.sync_tree();
        let view = View::identity();
        let coverage_draws = |groups: &[Group]| -> usize {
            let (_, _, _, metas) = build_content(groups, view, 1.0, 800.0, 600.0);
            metas
                .iter()
                .flat_map(|m| match m {
                    GroupDraw::Opaque { draws } | GroupDraw::Layer { draws, .. } => draws.iter(),
                    GroupDraw::Clip { members, .. } => members.iter(),
                })
                .filter(|d| matches!(d, Draw::StrokeCov { .. }))
                .count()
        };
        let full = build_scene(&ed, 1.0);
        let cut = build_scene_in_view(&ed, view, [800, 600]);
        assert_eq!(coverage_draws(&full.content), 2, "uncut: one coverage draw per stroke");
        assert_eq!(coverage_draws(&cut.content), 2, "culled: the strokes must not merge into one coverage");
    }

    // ---- Stroke-band integrity (regression: the "spokes" artifact, fix/stroke-fan-artifact) ----
    // P11.1 dropped every join under 5°, which left an open wedge (≈ r·θ wide) between consecutive
    // segment quads on the outer side of every curve point; with a thick stroke the outer half of the
    // band rendered as radial spokes. These tests look at the triangles themselves (no GPU): every point
    // inside the band must be covered, and no vertex may leave the band.

    fn to_px(p: [f32; 2], w: f32, h: f32) -> Pt {
        [(p[0] + 1.0) * 0.5 * w, (1.0 - p[1]) * 0.5 * h]
    }
    fn seg_dist(p: Pt, a: Pt, b: Pt) -> f32 {
        let d = [b[0] - a[0], b[1] - a[1]];
        let l2 = d[0] * d[0] + d[1] * d[1];
        let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) };
        dist(p, [a[0] + d[0] * t, a[1] + d[1] * t])
    }
    fn in_tri(p: Pt, t: &[Pt; 3]) -> bool {
        let s = |a: Pt, b: Pt| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let (d0, d1, d2) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
        !((d0 < 0.0 || d1 < 0.0 || d2 < 0.0) && (d0 > 0.0 || d1 > 0.0 || d2 > 0.0))
    }

    /// Check a stroke's triangles (`verts`, NDC on a `w`×`h` frame) against its screen-px centerline
    /// `runs` and half width `r`. Samples sit on each interior vertex's bisector, both sides, from 0.5r
    /// to r−0.5 px: exactly where an open join wedge is. Only on-screen samples count (view clipping only
    /// promises what is on screen). Returns (samples checked, triangles) so callers can sanity-check.
    fn assert_band_intact(label: &str, verts: &[Vertex], runs: &[Vec<Pt>], r: f32, w: f32, h: f32) -> (usize, usize) {
        assert!(verts.len().is_multiple_of(3), "{label}: whole triangles");
        assert!(verts.iter().all(|v| v.pos[0].is_finite() && v.pos[1].is_finite()), "{label}: NaN/inf vertex");
        let tris: Vec<[Pt; 3]> =
            verts.chunks(3).map(|c| [to_px(c[0].pos, w, h), to_px(c[1].pos, w, h), to_px(c[2].pos, w, h)]).collect();
        // 1. containment: every vertex within r of the centerline (+ f32 slack at 22 000 px coordinates)
        let tol = 0.05 + r * 1e-4;
        for t in &tris {
            for &q in t {
                let d = runs
                    .iter()
                    .flat_map(|run| run.windows(2).map(move |s| seg_dist(q, s[0], s[1])))
                    .fold(f32::MAX, f32::min);
                assert!(d <= r + tol, "{label}: vertex {q:?} is {:.3} px outside the band", d - r);
            }
        }
        // 2. no collapsed triangle: zero area along a long edge draws nothing (a broken-normal signature)
        for t in &tris {
            let l = dist(t[0], t[1]).max(dist(t[1], t[2])).max(dist(t[2], t[0]));
            let area =
                ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs() * 0.5;
            assert!(!(area < 1e-4 && l > 1.0), "{label}: collapsed triangle {t:?}");
        }
        // 3. coverage: no hole inside the band at any join
        let mut samples = 0;
        for run in runs {
            for i in 1..run.len().saturating_sub(1) {
                let (a, b, c) = (run[i - 1], run[i], run[i + 1]);
                let unit_n = |p: Pt, q: Pt| {
                    let l = dist(p, q).max(1e-6);
                    [-(q[1] - p[1]) / l, (q[0] - p[0]) / l]
                };
                let (n1, n2) = (unit_n(a, b), unit_n(b, c));
                let bis = [n1[0] + n2[0], n1[1] + n2[1]];
                let bl = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt();
                if bl < 1e-3 {
                    continue;
                }
                for side in [1.0f32, -1.0] {
                    for f in [0.5f32, 0.9, 0.97, 1.0] {
                        let d = (r * f).min(r - 0.5);
                        let q = [b[0] + side * bis[0] / bl * d, b[1] + side * bis[1] / bl * d];
                        if q[0] < 0.0 || q[1] < 0.0 || q[0] >= w || q[1] >= h {
                            continue;
                        }
                        samples += 1;
                        assert!(
                            tris.iter().any(|t| in_tri(q, t)),
                            "{label}: band point {q:?} ({:.0}% of the half width out, vertex {i}) is not covered",
                            d / r * 100.0
                        );
                    }
                }
            }
        }
        (samples, tris.len())
    }

    #[test]
    fn thick_arc_turning_either_way_has_no_wedge_gaps() {
        // a 300 px-radius arc flattened every 2° (under the old 5° join cut-off), 160 px wide, walked
        // turning one way and then the other: the open wedge flips sides, and both must be closed.
        let arc: Vec<Pt> = (0..=45)
            .map(|i| {
                let a = (i as f32 * 2.0).to_radians();
                [400.0 + 300.0 * a.cos(), 400.0 + 300.0 * a.sin()]
            })
            .collect();
        let mut reversed = arc.clone();
        reversed.reverse();
        for (label, pts) in [("one way", arc), ("the other way", reversed)] {
            let mut v = Vec::new();
            stroke_poly(&mut v, &pts, 160.0, [0.0, 0.0, 0.0, 1.0], 800.0, 800.0);
            let (samples, _) = assert_band_intact(label, &v, std::slice::from_ref(&pts), 80.0, 800.0, 800.0);
            assert!(samples > 300, "{label}: the check must actually sample the band ({samples})");
            // still no per-point disc explosion: 45 quads + 44 bevel wedges + 2 half-disc caps (r = 80:
            // 20 chords within 0.25 px = 19 triangles each)
            assert_eq!(v.len(), 45 * 6 + 44 * 3 + 2 * 19 * 3, "{label}: one bevel per gentle turn, no join discs");
        }
    }

    fn unit_deg(deg: f32) -> Pt {
        let a = deg.to_radians();
        [a.cos(), a.sin()]
    }

    /// Stroke a single turn `a → b → c` (heading `deg_in`, then `deg_out`) with half width `r` px. The
    /// ends sit 3r from the joint so the round end caps cannot reach it: only quads + join close the
    /// wedge. Returns the triangles in px and the joint.
    fn turn_tris(deg_in: f32, deg_out: f32, r: f32) -> (Vec<[Pt; 3]>, Pt) {
        let (din, dout) = (unit_deg(deg_in), unit_deg(deg_out));
        let l = 3.0 * r;
        let side = 2.0 * (l + r) + 10.0;
        let b = [side * 0.5, side * 0.5];
        let a = [b[0] - din[0] * l, b[1] - din[1] * l];
        let c = [b[0] + dout[0] * l, b[1] + dout[1] * l];
        let mut v = Vec::new();
        stroke_poly(&mut v, &[a, b, c], 2.0 * r, [0.0, 0.0, 0.0, 1.0], side, side);
        let tris = v
            .chunks(3)
            .map(|t| [to_px(t[0].pos, side, side), to_px(t[1].pos, side, side), to_px(t[2].pos, side, side)]);
        (tris.collect(), b)
    }

    /// Points of the outer join wedge at `b` that no triangle covers. The wedge is derived from the turn
    /// alone (not from the tessellator): centred on the outward direction `din − dout`, spanning the turn
    /// angle, sampled at 1, 2 and 3 px inside the band edge and at half depth, at nine angles across it.
    fn uncovered_wedge_points(tris: &[[Pt; 3]], b: Pt, deg_in: f32, deg_out: f32, r: f32) -> Vec<Pt> {
        let (din, dout) = (unit_deg(deg_in), unit_deg(deg_out));
        let theta = (din[0] * dout[0] + din[1] * dout[1]).clamp(-1.0, 1.0).acos();
        let out = [din[0] - dout[0], din[1] - dout[1]];
        let ol = (out[0] * out[0] + out[1] * out[1]).sqrt();
        if ol < 1e-6 {
            return Vec::new(); // straight: no wedge
        }
        let mid = [out[0] / ol, out[1] / ol];
        let mut bad = Vec::new();
        for t in [-0.97f32, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 0.97] {
            let (s, c) = (t * theta * 0.5).sin_cos();
            let dir = [mid[0] * c - mid[1] * s, mid[0] * s + mid[1] * c];
            for depth in [1.0f32, 2.0, 3.0, r * 0.5] {
                let q = [b[0] + dir[0] * (r - depth), b[1] + dir[1] * (r - depth)];
                if !tris.iter().any(|tr| in_tri(q, tr)) {
                    bad.push(q);
                }
            }
        }
        bad
    }

    /// Codex review of 045c956 (P2): width 80 at 4000%, heading 6° → 9°. The bevel's bulge exceeds the
    /// tolerance, and the old fallback swapped it for the inscribed 24-gon disc, which sits ~13.7 px
    /// inside the band midway between its vertices — leaving points 1–3 px inside the band uncovered.
    #[test]
    fn high_zoom_rotated_turn_is_covered_across_the_whole_wedge() {
        let r = 80.0 * 40.0 * 0.5;
        let (tris, b) = turn_tris(6.0, 9.0, r);
        let bad = uncovered_wedge_points(&tris, b, 6.0, 9.0, r);
        assert!(bad.is_empty(), "{} wedge points uncovered, e.g. {:?}", bad.len(), bad.first());
    }

    /// Sweep through the zoom range where a turn switches from a single bevel to a subdivided round
    /// join (width 80 at 10×…60× ⇒ r = 400…2400 px, plus 100% and 327%), for gentle to U-turns, turning
    /// both ways: the whole outer wedge must stay covered at every step.
    #[test]
    fn join_coverage_holds_across_the_round_join_threshold() {
        let turns = [(6.0f32, 9.0f32), (0.0, 1.0), (0.0, 5.0), (20.0, 50.0), (10.0, 100.0), (0.0, 179.0), (0.0, 180.0)];
        let mut zooms = vec![1.0f32, 3.27];
        zooms.extend((0..=20).map(|i| 10.0 + 2.5 * i as f32));
        for &zoom in &zooms {
            let r = 80.0 * zoom * 0.5;
            for &(a, b_deg) in &turns {
                for (din, dout) in [(a, b_deg), (a, a - (b_deg - a))] {
                    let (tris, b) = turn_tris(din, dout, r);
                    let bad = uncovered_wedge_points(&tris, b, din, dout, r);
                    assert!(
                        bad.is_empty(),
                        "zoom {zoom}, turn {din}° → {dout}°: {} wedge points uncovered, e.g. {:?}",
                        bad.len(),
                        bad.first()
                    );
                }
            }
        }
    }

    /// The round join is a fan anchored on the two quads' exact outer corners, and its outer edge (every
    /// chord) stays within JOIN_TOL_PX of the true circle — for thin to huge radii and small to U-turns.
    #[test]
    fn round_join_outer_edge_hugs_the_true_circle() {
        for r in [0.8f32, 2.0, 40.0, 131.0, 1600.0, 2400.0, 3300.0] {
            for (deg_in, deg_out) in [(6.0f32, 9.0f32), (0.0, 30.0), (0.0, -90.0), (15.0, 170.0), (0.0, 180.0)] {
                let (din, dout) = (unit_deg(deg_in), unit_deg(deg_out));
                let b = [5_000.0f32, 5_000.0];
                let nin = seg_normal([b[0] - din[0] * 10.0, b[1] - din[1] * 10.0], b, 2.0 * r);
                let nout = seg_normal(b, [b[0] + dout[0] * 10.0, b[1] + dout[1] * 10.0], 2.0 * r);
                let (w, h) = (10_000.0, 10_000.0);
                let mut v = Vec::new();
                let up = |p: Pt| [p[0] as f64, p[1] as f64];
                let end = |dir: Pt, n: Pt| StrokeEnd {
                    dir: up(dir),
                    normal: up(n),
                    corners: [-1.0, 1.0].map(|s| {
                        stroke_vertex(
                            [b[0] as f64 - n[0] as f64 * s, b[1] as f64 - n[1] as f64 * s],
                            [0.0, 0.0, 0.0, 1.0],
                            w,
                            h,
                        )
                    }),
                };
                stroke_join(&mut v, up(b), end(din, nin), end(dout, nout), r as f64, [0.0, 0.0, 0.0, 1.0], w, h);
                let label = format!("r {r}, turn {deg_in}° → {deg_out}°");
                assert!(!v.is_empty(), "{label}: a turn gets a join");
                let tris: Vec<[Pt; 3]> = v
                    .chunks(3)
                    .map(|t| [to_px(t[0].pos, w, h), to_px(t[1].pos, w, h), to_px(t[2].pos, w, h)])
                    .collect();
                let slack = 0.01; // NDC round trip at 5 000 px coordinates
                for t in &tris {
                    // The inner quad corner anchors the fan, sharing the whole cap edge.
                    assert!((dist(t[0], b) - r).abs() <= slack + r * 1e-6, "{label}: pivot on inner corner");
                    for q in [t[1], t[2]] {
                        assert!((dist(q, b) - r).abs() <= slack + r * 1e-6, "{label}: fan vertex off the circle");
                    }
                    let m = [(t[1][0] + t[2][0]) * 0.5, (t[1][1] + t[2][1]) * 0.5];
                    let bulge = r - dist(m, b);
                    assert!(bulge <= JOIN_TOL_PX + slack, "{label}: chord sits {bulge:.3} px inside the circle");
                }
                // the fan's first and last outer vertices are the quads' own outer corners (no crack)
                let cross = din[0] * dout[1] - din[1] * dout[0];
                let s = if cross > 0.0 { -1.0 } else { 1.0 };
                let first = v[1].pos;
                let last = v[v.len() - 1].pos;
                let precise_ndc = |n: Pt| {
                    let p = [b[0] as f64 + n[0] as f64 * s, b[1] as f64 + n[1] as f64 * s];
                    [(p[0] / w as f64 * 2.0 - 1.0) as f32, (1.0 - p[1] / h as f64 * 2.0) as f32]
                };
                assert_eq!(first, precise_ndc(nin), "{label}: fan starts on the incoming quad's corner");
                assert_eq!(last, precise_ndc(nout), "{label}: fan ends on the outgoing quad's corner");
            }
        }
    }

    /// The owner's case: a closed smooth ~480×400 path, 80.4 wide, at 100%, 327% and 4000%, both with the
    /// path wholly in view and with the view cutting through it (P11.2 view clipping in play).
    #[test]
    fn thick_curved_stroke_band_is_intact_in_and_out_of_view() {
        use varos_core::editor::Editor;
        use varos_core::model::{Anchor, Path};
        use varos_core::scene::build_scene_in_view;
        let radii = [240.0f32, 190.0, 225.0, 170.0, 235.0, 185.0, 210.0]; // convex and concave stretches
        let n = radii.len();
        let pts: Vec<Pt> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                [a.cos() * radii[i], a.sin() * radii[i] * 0.83]
            })
            .collect();
        let anchors = (0..n)
            .map(|i| {
                let (p, prev, next) = (pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]);
                let t = [(next[0] - prev[0]) / 6.0, (next[1] - prev[1]) / 6.0];
                Anchor {
                    id: 100 + i as u32,
                    p,
                    hin: Some([p[0] - t[0], p[1] - t[1]]),
                    hout: Some([p[0] + t[0], p[1] + t[1]]),
                    smooth: true,
                }
            })
            .collect();
        let mut ed = Editor::new();
        ed.doc.artboards.clear();
        let stroke_col = [0.122, 0.122, 0.129, 1.0];
        ed.doc.paths.push(Path::new(10, anchors, true, Some([0.945, 0.0, 0.0, 1.0]), Some(stroke_col), 80.4));
        ed.doc.ids = 10_000;
        ed.doc.sync_tree();
        for zoom in [1.0f32, 3.27, 40.0] {
            let ext = (560.0 * zoom).ceil() + 40.0; // the whole path plus its band
            let cases = [
                // a 1600×1000 window centred on world (200, 0): the right side of the path, cut by the view
                ("partial", [1600u32, 1000], [800.0 - 200.0 * zoom, 500.0]),
                ("full", [ext as u32, ext as u32], [ext * 0.5, ext * 0.5]),
            ];
            for (mode, frame, pan) in cases {
                let label = format!("{mode} view at zoom {zoom}");
                let view = View { pan, zoom };
                let (w, h) = (frame[0] as f32, frame[1] as f32);
                let scene = build_scene_in_view(&ed, view, frame);
                let runs: Vec<Vec<Pt>> = scene
                    .content
                    .iter()
                    .flat_map(|g| g.prims())
                    .filter_map(|p| match p {
                        Prim::Stroke { pts, .. } => Some(pts.iter().map(|q| view.w2s(*q)).collect()),
                        _ => None,
                    })
                    .collect();
                assert!(!runs.is_empty(), "{label}: the stroke is in view");
                let (_fill, fgv, _op, _metas) = build_content(&scene.content, view, zoom, w, h);
                assert!(fgv.iter().all(|v| v.color == stroke_col), "{label}: fg holds only this stroke");
                let (samples, tris) = assert_band_intact(&label, &fgv, &runs, 80.4 * zoom * 0.5, w, h);
                assert!(samples > 300 && tris > 0, "{label}: the band was actually sampled ({samples})");
            }
        }
    }

    // ---- QW4 (PAINS_LOG P13): round caps are half-discs on the joins' subdivision rule ----
    // The old caps were full 24-gon discs: inscribed, so at r = 1 600 px (width 80 at 4000%) they sat
    // up to r·(1 − cos 7.5°) ≈ 13.7 px inside the true circle. Closed rings now take a seam join
    // instead of two caps (a vertex-count choice: half-caps alone would also cover the seam).

    const CAP_FRAME: f32 = 10_000.0;

    /// One straight segment of half width `r` px heading `deg`, centred in a 10 000 px frame (far from
    /// the NDC origin, as in the join tests). Returns the mesh, both end points and the heading.
    fn capped_segment(deg: f32, r: f32) -> (Vec<Vertex>, Pt, Pt, Pt) {
        let d = unit_deg(deg);
        let (c, half) = (CAP_FRAME * 0.5, 1.25 * r);
        let a = [c - d[0] * half, c - d[1] * half];
        let b = [c + d[0] * half, c + d[1] * half];
        let mut v = Vec::new();
        stroke_poly(&mut v, &[a, b], 2.0 * r, [0.0, 0.0, 0.0, 1.0], CAP_FRAME, CAP_FRAME);
        (v, a, b, d)
    }
    /// A single segment's mesh is its quad (6 vertices), then the start cap, then the end cap.
    fn split_caps(v: &[Vertex]) -> (&[Vertex], &[Vertex]) {
        let caps = &v[6..];
        assert!(caps.len().is_multiple_of(6), "two equal caps of whole triangles");
        caps.split_at(caps.len() / 2)
    }
    fn px_tris(v: &[Vertex], w: f32, h: f32) -> Vec<[Pt; 3]> {
        v.chunks(3).map(|t| [to_px(t[0].pos, w, h), to_px(t[1].pos, w, h), to_px(t[2].pos, w, h)]).collect()
    }
    /// Points of the disc of radius `rad` around `c`, at `angles` directions spread over `span_deg`
    /// degrees centred on `out`, and at the given fractions of `rad`, that no triangle covers.
    fn uncovered_disc_points(tris: &[[Pt; 3]], c: Pt, out: Pt, span_deg: f32, rad: f32, fracs: &[f32]) -> Vec<Pt> {
        let mut bad = Vec::new();
        let steps = span_deg.round() as i32;
        for i in 0..=steps {
            let (s, co) = (i as f32 - steps as f32 * 0.5).to_radians().sin_cos();
            let dir = [out[0] * co - out[1] * s, out[0] * s + out[1] * co];
            for &f in fracs {
                let q = [c[0] + dir[0] * rad * f, c[1] + dir[1] * rad * f];
                if !tris.iter().any(|t| in_tri(q, t)) {
                    bad.push(q);
                }
            }
        }
        bad
    }

    /// Width 80 at 100% (r = 40 px) and 4000% (r = 1 600 px): each cap is a half-disc on the outward
    /// side whose every vertex lies on the true circle and whose every chord sits within JOIN_TOL_PX of
    /// it (the joins' rule), and the whole half-disc up to that tolerance is covered. Zoom 8250%
    /// (r = 3 300 px) is the largest cap that still meets 0.25 px with the full 128 chords: it pins
    /// that the incremental rotation in `stroke_join` does not drift over the longest fan.
    #[test]
    fn round_cap_hugs_true_circle_at_100_and_4000_percent() {
        for zoom in [1.0f32, 40.0, 82.5] {
            let r = 80.0 * zoom * 0.5;
            for deg in [0.0f32, 23.0, 90.0, 137.0, 180.0, 271.0] {
                let (v, a, b, d) = capped_segment(deg, r);
                let (start, end) = split_caps(&v);
                if zoom == 82.5 {
                    assert_eq!(start.len() / 3 + 1, JOIN_MAX_STEPS, "r {r}: the 128-chord case");
                }
                for (which, cap, c, out) in [("start", start, a, [-d[0], -d[1]]), ("end", end, b, d)] {
                    let label = format!("zoom {zoom}, heading {deg}°, {which} cap");
                    let tris = px_tris(cap, CAP_FRAME, CAP_FRAME);
                    let slack = 0.01; // NDC round trip at 5 000 px coordinates
                    for t in &tris {
                        for &q in t {
                            assert!((dist(q, c) - r).abs() <= slack, "{label}: vertex {q:?} off the true circle");
                            let along = (q[0] - c[0]) * out[0] + (q[1] - c[1]) * out[1];
                            assert!(along >= -slack, "{label}: vertex {q:?} behind the end (not a half-disc)");
                        }
                        let m = [(t[1][0] + t[2][0]) * 0.5, (t[1][1] + t[2][1]) * 0.5];
                        let bulge = r - dist(m, c);
                        assert!(bulge <= JOIN_TOL_PX + slack, "{label}: chord sits {bulge:.3} px inside the circle");
                    }
                    let inner = (r - JOIN_TOL_PX - 0.05) / r;
                    let bad = uncovered_disc_points(&tris, c, out, 178.0, r, &[0.25, 0.5, 0.9, inner]);
                    assert!(bad.is_empty(), "{label}: {} half-disc points uncovered, e.g. {:?}", bad.len(), bad[0]);
                }
            }
        }
    }

    /// Every cap triangle pivots on the end quad's own corner vertex and the arc starts on its other
    /// corner, bit for bit (the same emitted vertices), so the cap shares the quad's whole end edge.
    #[test]
    fn cap_shares_the_end_quad_corners_bit_exact() {
        for (deg, r) in [(0.0f32, 2.0f32), (37.0, 40.0), (200.0, 1600.0)] {
            let (v, ..) = capped_segment(deg, r);
            // The quad is [ap, bp, bm, ap, bm, am].
            let (ap, bp, bm, am) = (v[0].pos, v[1].pos, v[2].pos, v[5].pos);
            let (start, end) = split_caps(&v);
            for (cap, pivot, first_outer) in [(start, ap, am), (end, bm, bp)] {
                assert!(cap.chunks(3).all(|t| t[0].pos == pivot), "r {r}: every cap triangle pivots on the corner");
                assert_eq!(cap[1].pos, first_outer, "r {r}: the arc starts on the quad's other corner");
                assert!(
                    cap.chunks(3).all(|t| t[1].pos != pivot && t[2].pos != pivot),
                    "r {r}: no zero-area triangle ends on the pivot"
                );
            }
        }
    }

    /// Caps use the joins' step rule: at least 4 chords (1 per 45°), at most JOIN_MAX_STEPS.
    #[test]
    fn cap_step_count_is_bounded() {
        for r in [0.8f32, 2.0, 40.0, 1600.0, 3000.0, 1e5, 1e9] {
            let (v, ..) = capped_segment(0.0, r);
            let (start, end) = split_caps(&v);
            let chords = start.len() / 3 + 1;
            assert_eq!(start.len(), end.len());
            assert!((4..=JOIN_MAX_STEPS).contains(&chords), "r {r}: {chords} chords");
            if r <= 2.0 {
                assert_eq!(chords, 4, "r {r}: a thin stroke still gets 4 chords (1 per 45°)");
            }
            if r >= 1e5 {
                assert_eq!(chords, JOIN_MAX_STEPS, "r {r}: capped");
            }
        }
        // r = 1 600 px (width 80 at 4000%): the 0.25 px sagitta needs 89 chords, well under the cap.
        let (v, ..) = capped_segment(0.0, 1600.0);
        assert_eq!(split_caps(&v).0.len() / 3 + 1, 89);
    }

    /// Below 1.6 px wide, strokes stay bare quads: no joins and no caps, open or closed.
    #[test]
    fn thin_strokes_unchanged() {
        let col = [0.0, 0.0, 0.0, 1.0];
        let mut v = Vec::new();
        stroke_poly(&mut v, &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], 1.5, col, 100.0, 100.0);
        assert_eq!(v.len(), 12, "open, 1.5 px: two quads only");
        v.clear();
        stroke_poly(&mut v, &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]], 1.5, col, 100.0, 100.0);
        assert_eq!(v.len(), 24, "closed, 1.5 px: four quads only");
        v.clear();
        stroke_poly(&mut v, &[[0.0, 0.0], [10.0, 0.0]], 1.6, col, 100.0, 100.0);
        assert_eq!(v.len(), 6 + 2 * 9, "at 1.6 px the caps start: 4 chords = 3 triangles each");
    }

    /// A zero-length stroke (every point equal) is still a round dot.
    #[test]
    fn zero_length_stroke_is_still_a_round_dot() {
        let mut v = Vec::new();
        stroke_poly(&mut v, &[[50.0, 50.0], [50.0, 50.0]], 10.0, [0.0, 0.0, 0.0, 1.0], 100.0, 100.0);
        let tris = px_tris(&v, 100.0, 100.0);
        assert!(tris.iter().flatten().all(|q| dist(*q, [50.0, 50.0]) <= 5.0 + 1e-3), "inside the dot");
        let bad = uncovered_disc_points(&tris, [50.0, 50.0], [1.0, 0.0], 360.0, 5.0, &[0.3, 0.6, 0.9]);
        assert!(bad.is_empty(), "{} dot points uncovered, e.g. {:?}", bad.len(), bad.first());
    }

    fn one_path_editor(path: varos_core::model::Path) -> varos_core::editor::Editor {
        let mut ed = varos_core::editor::Editor::new();
        ed.doc.artboards.clear();
        ed.doc.paths.push(path);
        ed.doc.ids = 10_000;
        ed.doc.sync_tree();
        ed
    }
    fn blob_path(width: f32) -> varos_core::model::Path {
        use varos_core::model::{Anchor, Path};
        let radii = [240.0f32, 190.0, 225.0, 170.0, 235.0, 185.0, 210.0];
        let n = radii.len();
        let pts: Vec<Pt> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                [a.cos() * radii[i], a.sin() * radii[i] * 0.83]
            })
            .collect();
        let anchors = (0..n)
            .map(|i| {
                let (p, prev, next) = (pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]);
                let t = [(next[0] - prev[0]) / 6.0, (next[1] - prev[1]) / 6.0];
                Anchor {
                    id: 100 + i as u32,
                    p,
                    hin: Some([p[0] - t[0], p[1] - t[1]]),
                    hout: Some([p[0] + t[0], p[1] + t[1]]),
                    smooth: true,
                }
            })
            .collect();
        Path::new(10, anchors, true, None, Some([0.0, 0.0, 0.0, 1.0]), width)
    }

    /// How closed outlines arrive (the planner's risk): the scene hands every closed ring — outer
    /// outline, straight or curved, and hole rings — to the tessellator with its first point repeated
    /// exactly at the end, and an open path without it. So `stroke_poly`'s `c0 == c1` seam branch is
    /// the one real closed paths take.
    #[test]
    fn closed_rings_arrive_with_the_first_point_repeated() {
        use varos_core::model::{Anchor, Path};
        use varos_core::scene::build_scene;
        let anc = |id: u32, x: f32, y: f32| Anchor { id, p: [x, y], hin: None, hout: None, smooth: false };
        let square = |base: u32, s: f32, o: f32| {
            vec![anc(base, o, o), anc(base + 1, o + s, o), anc(base + 2, o + s, o + s), anc(base + 3, o, o + s)]
        };
        let mut holed = Path::new(20, square(200, 100.0, 0.0), true, None, Some([0.0, 0.0, 0.0, 1.0]), 8.0);
        holed.holes.push(square(300, 40.0, 30.0));
        let open = Path::new(30, square(400, 100.0, 0.0), false, None, Some([0.0, 0.0, 0.0, 1.0]), 8.0);
        for ppu in [1.0f32, 40.0] {
            for (label, path, closed_runs) in [("curved blob", blob_path(80.0), 1), ("square + hole", holed.clone(), 2)]
            {
                let scene = build_scene(&one_path_editor(path), ppu);
                let runs: Vec<&Vec<Pt>> = scene
                    .content
                    .iter()
                    .flat_map(|g| g.prims())
                    .filter_map(|p| match p {
                        Prim::Stroke { pts, .. } => Some(pts),
                        _ => None,
                    })
                    .collect();
                assert_eq!(runs.len(), closed_runs, "{label} at ppu {ppu}: one run per ring");
                for run in runs {
                    assert!(run.len() > 3 && run[0] == run[run.len() - 1], "{label} at ppu {ppu}: ring not repeated");
                }
            }
            let scene = build_scene(&one_path_editor(open.clone()), ppu);
            let open_runs: Vec<&Prim> = scene.content.iter().flat_map(|g| g.prims()).collect();
            assert!(
                matches!(open_runs.as_slice(), [Prim::Stroke { pts, .. }] if pts[0] != pts[pts.len() - 1]),
                "an open path stays open at ppu {ppu}"
            );
        }
    }

    /// A closed ring gets a seam JOIN at its repeated first point and no caps: the vertex count is
    /// exactly quads + one join per corner, it does not depend on which corner the ring starts at, and
    /// the round-join disc at the seam is covered.
    #[test]
    fn closed_ring_seam_is_joined_not_capped() {
        let (s0, s1) = (4_000.0f32, 4_400.0f32);
        let corners = [[s0, s0], [s1, s0], [s1, s1], [s0, s1]];
        let r = 10.0f32;
        for start in 0..4 {
            let ring: Vec<Pt> = (0..=4).map(|i| corners[(start + i) % 4]).collect();
            let mut v = Vec::new();
            stroke_poly(&mut v, &ring, 2.0 * r, [0.0, 0.0, 0.0, 1.0], CAP_FRAME, CAP_FRAME);
            // 4 quads (24) + 4 round 90° joins; at r = 10 a 0.25 px sagitta needs 4 chords (12 each).
            assert_eq!(v.len(), 24 + 4 * 12, "starting at corner {start}: quads + 4 joins, no caps");
            let tris = px_tris(&v, CAP_FRAME, CAP_FRAME);
            let seam = ring[0];
            let bad = uncovered_disc_points(&tris, seam, [1.0, 0.0], 360.0, r, &[0.25, 0.5, 0.9, 0.97]);
            assert!(bad.is_empty(), "seam at corner {start}: {} points uncovered, e.g. {:?}", bad.len(), bad.first());
        }
    }

    /// The same seam through the real scene: the curved blob, width 80, at 100% (whole ring in view:
    /// one closed run, so a seam join) and 4000% (the view cuts the ring and the seam stays in view:
    /// two open runs whose half-disc caps meet there). The disc around the seam is covered on screen.
    #[test]
    fn closed_path_seam_is_covered_in_and_out_of_view() {
        let ed = one_path_editor(blob_path(80.0));
        let seam = [240.0f32, 0.0]; // anchor 0 of the blob
        for (zoom, frame, n_runs, closed) in [(1.0f32, [1200u32, 1000u32], 1, true), (40.0, [1600, 1000], 2, false)] {
            let (runs, sampled, bad) = disc_coverage_in_scene(&ed, zoom, frame, seam, 40.0 * zoom);
            assert_eq!(runs.len(), n_runs, "zoom {zoom}: stroke runs");
            assert!(runs.iter().all(|run| (run[0] == run[run.len() - 1]) == closed), "zoom {zoom}: closed = {closed}");
            if !closed {
                // the cut left the seam as a run END on both sides: one run starts there, one ends there
                assert!(runs.iter().any(|run| run[0] == seam), "4000%: a run starts on the seam");
                assert!(runs.iter().any(|run| run[run.len() - 1] == seam), "4000%: a run ends on the seam");
            }
            assert!(sampled > 600, "zoom {zoom}: the seam disc was sampled ({sampled})");
            assert!(bad.is_empty(), "zoom {zoom}: {} seam points uncovered, e.g. {:?}", bad.len(), bad.first());
        }
    }

    /// Build `ed` through the real scene at `zoom`, centring world point `focus` in a `frame`, and
    /// return the stroke runs (world) and how many on-screen points of the disc of radius `r_px`
    /// around `focus` were sampled / left uncovered by the stroke triangles.
    fn disc_coverage_in_scene(
        ed: &varos_core::editor::Editor,
        zoom: f32,
        frame: [u32; 2],
        focus: Pt,
        r_px: f32,
    ) -> (Vec<Vec<Pt>>, usize, Vec<Pt>) {
        let (w, h) = (frame[0] as f32, frame[1] as f32);
        let view = View { pan: [w * 0.5 - focus[0] * zoom, h * 0.5 - focus[1] * zoom], zoom };
        let scene = varos_core::scene::build_scene_in_view(ed, view, frame);
        let runs = scene
            .content
            .iter()
            .flat_map(|g| g.prims())
            .filter_map(|p| match p {
                Prim::Stroke { pts, .. } => Some(pts.clone()),
                _ => None,
            })
            .collect();
        let (_, fgv, _, _) = build_content(&scene.content, view, zoom, w, h);
        let tris = px_tris(&fgv, w, h);
        let fracs = [0.1, 0.25, 0.5, 0.75, 0.9, 0.97];
        let on_screen: Vec<Pt> = uncovered_disc_points(&[], view.w2s(focus), [1.0, 0.0], 360.0, r_px, &fracs)
            .into_iter()
            .filter(|q| q[0] >= 0.0 && q[1] >= 0.0 && q[0] < w && q[1] < h)
            .collect();
        let bad = on_screen.iter().copied().filter(|q| !tris.iter().any(|t| in_tri(*q, t))).collect();
        (runs, on_screen.len(), bad)
    }

    /// Plan review P2 "QW4 seams": a closed RECTANGLE's seam is a 90° corner (anchor 0), at 4000%, with
    /// the view centred on that corner. When the whole ring fits the view's clip rect it arrives as one
    /// closed run and the seam gets a round join (width 4, r = 80 px; and width 80, r = 1 600 px, on a
    /// small rectangle). When the view cuts a big rectangle, two open runs end on the corner and their
    /// half-disc caps meet there. In every case the full round-corner disc is covered on screen.
    #[test]
    fn rectangle_seam_corner_is_round_at_4000_percent() {
        use varos_core::model::{Anchor, Path};
        let anc = |id: u32, x: f32, y: f32| Anchor { id, p: [x, y], hin: None, hout: None, smooth: false };
        let rect = |size: f32, width: f32| {
            let anchors = vec![anc(100, 0.0, 0.0), anc(101, size, 0.0), anc(102, size, size), anc(103, 0.0, size)];
            Path::new(10, anchors, true, Some([0.9, 0.9, 0.9, 1.0]), Some([0.0, 0.0, 0.0, 1.0]), width)
        };
        let seam = [0.0f32, 0.0];
        // (stroke width, rectangle side, frame, arrives closed?)
        for (width, size, frame, closed) in [
            (4.0f32, 20.0f32, [1800u32, 1800u32], true),
            (80.0, 20.0, [1600, 1000], true),
            (80.0, 200.0, [1600, 1000], false),
        ] {
            let label = format!("width {width}, side {size}");
            let r = width * 40.0 * 0.5;
            let (runs, sampled, bad) =
                disc_coverage_in_scene(&one_path_editor(rect(size, width)), 40.0, frame, seam, r);
            if closed {
                assert!(runs.len() == 1 && runs[0][0] == seam && runs[0][runs[0].len() - 1] == seam, "{label}: closed");
            } else {
                assert!(runs.iter().all(|r| r[0] != r[r.len() - 1]), "{label}: the view cut the ring open");
                assert!(runs.iter().any(|r| r[0] == seam), "{label}: a run starts on the seam");
                assert!(runs.iter().any(|r| r[r.len() - 1] == seam), "{label}: a run ends on the seam");
            }
            assert!(sampled > 600, "{label}: the corner disc was sampled ({sampled})");
            assert!(bad.is_empty(), "{label}: {} seam-corner points uncovered, e.g. {:?}", bad.len(), bad.first());
        }
    }

    /// Plan review P2: a zero-length open path (two anchors on one point) with round caps renders a
    /// full disc through the real scene, at 100% and 4000%. (A ONE-anchor path never reaches the
    /// tessellator: the scene emits strokes only for paths with >= 2 anchors, before and after QW4.)
    #[test]
    fn zero_length_path_renders_a_full_disc() {
        use varos_core::model::{Anchor, Path};
        let anc = |id: u32| Anchor { id, p: [10.0, 10.0], hin: None, hout: None, smooth: false };
        let ed =
            one_path_editor(Path::new(10, vec![anc(100), anc(101)], false, None, Some([0.0, 0.0, 0.0, 1.0]), 80.0));
        for (zoom, frame) in [(1.0f32, [400u32, 400u32]), (40.0, [1600, 1000])] {
            let r = 40.0 * zoom;
            let (runs, sampled, bad) = disc_coverage_in_scene(&ed, zoom, frame, [10.0, 10.0], r);
            assert_eq!(runs.len(), 1, "zoom {zoom}: the zero-length run reaches the tessellator");
            assert!(sampled > 600, "zoom {zoom}: the dot was sampled ({sampled})");
            assert!(bad.is_empty(), "zoom {zoom}: {} dot points uncovered, e.g. {:?}", bad.len(), bad.first());
        }
    }

    /// The perf harness's curved scenes, rebuilt here (same construction as
    /// `examples/perf_harness.rs`): stroke vertex counts must not grow past the numbers of record in
    /// `P11_2_PERF.md` (round 3: C 100 500 at 100%, D 3 387 at 4000%). Closed rings lose their two
    /// 72-vertex discs for one seam join; D's two view-cut runs get four half-disc caps instead of discs.
    #[test]
    fn harness_curved_scenes_stroke_vertices_do_not_grow() {
        use varos_core::editor::Editor;
        use varos_core::model::{Anchor, Path};
        use varos_core::scene::build_scene_in_view;
        let curved = |id: u32, base: u32, c: Pt, radius: f32, count: usize| {
            let step = std::f32::consts::TAU / count as f32;
            let handle = radius * (4.0 / 3.0) * (step * 0.25).tan();
            let anchors = (0..count)
                .map(|i| {
                    let (sin, cos) = (i as f32 * step).sin_cos();
                    let p = [c[0] + cos * radius, c[1] + sin * radius];
                    Anchor {
                        id: base + i as u32,
                        p,
                        hin: Some([p[0] + sin * handle, p[1] - cos * handle]),
                        hout: Some([p[0] - sin * handle, p[1] + cos * handle]),
                        smooth: true,
                    }
                })
                .collect();
            Path::new(id, anchors, true, Some([0.18, 0.55, 0.86, 1.0]), Some([0.04, 0.04, 0.05, 1.0]), 2.0)
        };
        let fg_vertices = |paths: Vec<Path>, view: View| {
            let mut ed = Editor::new();
            ed.doc.paths = paths;
            ed.doc.ids = 100_000;
            ed.doc.sync_tree();
            let scene = build_scene_in_view(&ed, view, [1920, 1080]);
            build_content(&scene.content, view, view.zoom, 1920.0, 1080.0).1.len()
        };
        let c_paths = (0..100u32)
            .map(|i| {
                curved(
                    40_000 + i,
                    50_000 + i * 12,
                    [60.0 + (i % 10) as f32 * 90.0, 60.0 + (i / 10) as f32 * 90.0],
                    32.0,
                    12,
                )
            })
            .collect();
        let c = fg_vertices(c_paths, View::identity());
        assert_eq!(c, 100_500 - 100 * (2 * 72 - 3), "C at 100%: each closed ring trades two discs for one seam bevel");
        let d_view = View { pan: [960.0 - 490.0 * 40.0, 540.0 - 220.0 * 40.0], zoom: 40.0 };
        let d = fg_vertices(vec![curved(10, 100, [320.0, 220.0], 170.0, 150)], d_view);
        assert_eq!(d, 3_387 - 4 * 72 + 4 * 14 * 3, "D at 4000%: four discs become four 15-chord half-discs");
    }
}

#[cfg(test)]
#[path = "tess_round3_tests.rs"]
mod round3_tests;
