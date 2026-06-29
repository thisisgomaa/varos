//! Turn the core's render-agnostic `Scene` primitives into GPU triangles (pixel space → NDC on CPU).

use varos_core::geom::{Pt, View};
use varos_core::scene::Prim;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex { pub pos: [f32; 2], pub color: [f32; 4] }

fn dist(a: Pt, b: Pt) -> f32 { ((a[0]-b[0]).powi(2) + (a[1]-b[1]).powi(2)).sqrt() }
fn ndc(p: Pt, w: f32, h: f32) -> [f32; 2] { [p[0]/w*2.0 - 1.0, 1.0 - p[1]/h*2.0] }
fn tri(v: &mut Vec<Vertex>, a: Pt, b: Pt, c: Pt, col: [f32;4], w: f32, h: f32) {
    v.push(Vertex { pos: ndc(a,w,h), color: col }); v.push(Vertex { pos: ndc(b,w,h), color: col }); v.push(Vertex { pos: ndc(c,w,h), color: col });
}
fn quad(v: &mut Vec<Vertex>, p0: Pt, p1: Pt, p2: Pt, p3: Pt, col: [f32;4], w: f32, h: f32) { tri(v,p0,p1,p2,col,w,h); tri(v,p0,p2,p3,col,w,h); }
fn line(v: &mut Vec<Vertex>, a: Pt, b: Pt, width: f32, col: [f32;4], w: f32, h: f32) {
    let d = [b[0]-a[0], b[1]-a[1]]; let l = (d[0]*d[0]+d[1]*d[1]).sqrt().max(1e-3);
    let n = [-d[1]/l*width/2.0, d[0]/l*width/2.0];
    quad(v, [a[0]+n[0],a[1]+n[1]], [b[0]+n[0],b[1]+n[1]], [b[0]-n[0],b[1]-n[1]], [a[0]-n[0],a[1]-n[1]], col, w, h);
}
fn sq(v: &mut Vec<Vertex>, c: Pt, half: f32, col: [f32;4], w: f32, h: f32) {
    quad(v, [c[0]-half,c[1]-half],[c[0]+half,c[1]-half],[c[0]+half,c[1]+half],[c[0]-half,c[1]+half], col, w, h);
}
fn disc(v: &mut Vec<Vertex>, c: Pt, r: f32, col: [f32;4], w: f32, h: f32) {
    let segs = 24;
    for i in 0..segs {
        let a0 = i as f32/segs as f32*std::f32::consts::TAU;
        let a1 = (i+1) as f32/segs as f32*std::f32::consts::TAU;
        tri(v, c, [c[0]+a0.cos()*r, c[1]+a0.sin()*r], [c[0]+a1.cos()*r, c[1]+a1.sin()*r], col, w, h);
    }
}
fn stroke_poly(v: &mut Vec<Vertex>, pts: &[Pt], width: f32, col: [f32;4], w: f32, h: f32) {
    for i in 0..pts.len().saturating_sub(1) { line(v, pts[i], pts[i+1], width, col, w, h); }
    // round joins/caps: a disc at each vertex fills the gaps between butt-cap segment quads,
    // so thick strokes don't crack apart at corners. Only worth it once the stroke is thick enough.
    if width >= 1.6 && pts.len() >= 2 { let r = width * 0.5; for p in pts { disc(v, *p, r, col, w, h); } }
}
fn dashed_poly(v: &mut Vec<Vertex>, pts: &[Pt], width: f32, col: [f32;4], w: f32, h: f32) {
    let (dash, gap) = (5.0f32, 4.0f32); let period = dash + gap; let mut acc = 0.0f32;
    for i in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[i], pts[i+1]); let seglen = dist(a, b); if seglen < 1e-4 { continue; }
        let dir = [(b[0]-a[0])/seglen, (b[1]-a[1])/seglen]; let mut s = 0.0f32;
        while s < seglen {
            let phase = (acc + s) % period;
            if phase < dash { let e = (s + (dash - phase)).min(seglen);
                line(v, [a[0]+dir[0]*s,a[1]+dir[1]*s], [a[0]+dir[0]*e,a[1]+dir[1]*e], width, col, w, h); s = e;
            } else { s += period - phase; }
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
    const TARGET: f32 = 30.0;          // desired screen px between dots
    const MIN_PX: f32 = 9.0;           // skip a level finer than this (perf + anti-clutter)
    const BG: [f32; 3] = [0.078, 0.075, 0.075];   // board background (#141313)
    const DOT: [f32; 3] = [0.34, 0.34, 0.37];     // a dot at full strength (clearly visible on #141313)

    // base-5 level whose world step lands near TARGET px on screen
    let scale = (TARGET / zoom).max(1e-6);
    let level = scale.ln() / 5f32.ln();
    let k0 = level.floor();
    let t = level - k0;                            // 0..1 within the level
    let step_fine = 5f32.powf(k0);
    let step_coarse = 5f32.powf(k0 + 1.0);

    // visible world rect (+1 step padding so dots don't pop at the edges)
    let tl = view.s2w([0.0, 0.0]);
    let br = view.s2w([w, h]);
    let (wx0, wy0) = (tl[0].min(br[0]), tl[1].min(br[1]));
    let (wx1, wy1) = (tl[0].max(br[0]), tl[1].max(br[1]));

    let mut grid = |step: f32, alpha: f32| {
        if alpha < 0.04 || step * zoom < MIN_PX { return; }
        // composite the faded dot over the board once (no blend-state dependency): opaque colour.
        let col = [BG[0] + (DOT[0]-BG[0])*alpha, BG[1] + (DOT[1]-BG[1])*alpha, BG[2] + (DOT[2]-BG[2])*alpha, 1.0];
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

/// fills: per Fill prim → a triangle-fan (stencil) + a bbox cover quad. Points are mapped world→screen via `view`.
pub fn build_fills(prims: &[Prim], view: View, w: f32, h: f32) -> (Vec<Vertex>, Vec<((u32,u32),(u32,u32))>) {
    let mut v = Vec::new(); let mut ranges = Vec::new();
    for prim in prims {
        if let Prim::Fill { rings, color } = prim {
            // map every ring (outer + holes) to screen, then draw pivot-triangles for ALL edges into the
            // stencil with one global pivot — even-odd parity then cuts the holes in a single cover pass.
            let srings: Vec<Vec<Pt>> = rings.iter().map(|r| r.iter().map(|p| view.w2s(*p)).collect()).collect();
            let pivot = match srings.iter().find(|r| r.len() >= 3) { Some(r) => r[0], None => continue };
            let fan_start = v.len() as u32;
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for r in &srings {
                if r.len() < 3 { continue; }
                let n = r.len();
                for j in 0..n { tri(&mut v, pivot, r[j], r[(j+1) % n], *color, w, h); }
                for p in r { x0=x0.min(p[0]); y0=y0.min(p[1]); x1=x1.max(p[0]); y1=y1.max(p[1]); }
            }
            let fan_len = v.len() as u32 - fan_start;
            let cov_start = v.len() as u32;
            quad(&mut v, [x0,y0],[x1,y0],[x1,y1],[x0,y1], *color, w, h);
            let cov_len = v.len() as u32 - cov_start;
            ranges.push(((fan_start, fan_len), (cov_start, cov_len)));
        }
    }
    (v, ranges)
}

/// everything except fills (drawn on top). Points mapped world→screen via `view`; sizes/widths scaled by `size_scale`
/// (use view.zoom for artwork so strokes thicken with zoom, or 1.0 for overlay/UI to keep constant screen size).
pub fn build_fg(prims: &[Prim], view: View, size_scale: f32, w: f32, h: f32) -> Vec<Vertex> {
    let mut v = Vec::new(); let z = size_scale;
    for prim in prims {
        match prim {
            Prim::Fill { .. } => {}
            Prim::Stroke { pts, width, color } => { let sp: Vec<Pt> = pts.iter().map(|p| view.w2s(*p)).collect(); stroke_poly(&mut v, &sp, width*z, *color, w, h); }
            Prim::Dashed { pts, width, color } => { let sp: Vec<Pt> = pts.iter().map(|p| view.w2s(*p)).collect(); dashed_poly(&mut v, &sp, width*z, *color, w, h); }
            Prim::Square { c, half, color } => sq(&mut v, view.w2s(*c), half*z, *color, w, h),
            Prim::Disc { c, r, color } => disc(&mut v, view.w2s(*c), r*z, *color, w, h),
            Prim::Tri { a, b, c, color } => tri(&mut v, view.w2s(*a), view.w2s(*b), view.w2s(*c), *color, w, h),
        }
    }
    v
}
