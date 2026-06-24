//! The document data model: anchors, paths, the document. Plus pure geometry queries.
//! Stable u32 IDs (never Vec indices) so selection/active survive deletes & joins.

use crate::geom::*;

pub const K: f32 = 0.5522847; // bezier circle constant (ellipse handles)

#[derive(Clone)]
pub struct Anchor { pub id: u32, pub p: Pt, pub hin: Option<Pt>, pub hout: Option<Pt>, pub smooth: bool }

#[derive(Clone)]
pub struct Path {
    pub id: u32,
    pub anchors: Vec<Anchor>,
    pub closed: bool,
    pub fill: Option<Rgba>,
    pub stroke: Option<Rgba>,
    pub stroke_width: f32,
    /// extra hole contours (editable bezier anchors) — e.g. from boolean ops. A compound path: the
    /// outer `anchors` plus these inner rings, filled even-odd so holes cut through. Normally empty.
    pub holes: Vec<Vec<Anchor>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ShapeKind { Rect, Ellipse, Triangle, Polygon }

#[derive(Default, Clone)]
pub struct Document { pub paths: Vec<Path>, pub ids: u32 }

impl Document {
    pub fn nid(&mut self) -> u32 { self.ids += 1; self.ids }

    pub fn pidx(&self, pid: u32) -> Option<usize> { self.paths.iter().position(|p| p.id == pid) }
    pub fn aidx(&self, aid: u32) -> Option<(usize, usize)> {
        for (pi, p) in self.paths.iter().enumerate() {
            if let Some(ai) = p.anchors.iter().position(|a| a.id == aid) { return Some((pi, ai)); }
        }
        None
    }
    /// Find an anchor by id across ALL contours (outer + holes). aidx covers only the outer ring.
    pub fn anchor(&self, aid: u32) -> Option<&Anchor> {
        for p in &self.paths {
            if let Some(a) = p.anchors.iter().find(|a| a.id == aid) { return Some(a); }
            for h in &p.holes { if let Some(a) = h.iter().find(|a| a.id == aid) { return Some(a); } }
        }
        None
    }
    pub fn anchor_mut(&mut self, aid: u32) -> Option<&mut Anchor> {
        for pi in 0..self.paths.len() {
            if let Some(ai) = self.paths[pi].anchors.iter().position(|a| a.id == aid) { return Some(&mut self.paths[pi].anchors[ai]); }
            for hi in 0..self.paths[pi].holes.len() {
                if let Some(ai) = self.paths[pi].holes[hi].iter().position(|a| a.id == aid) { return Some(&mut self.paths[pi].holes[hi][ai]); }
            }
        }
        None
    }
    /// The id of the path owning an anchor (outer or hole).
    pub fn pid_of_anchor(&self, aid: u32) -> Option<u32> {
        self.paths.iter().find(|p| p.anchors.iter().chain(p.holes.iter().flatten()).any(|a| a.id == aid)).map(|p| p.id)
    }

    /// Nearest point on a path's outline → (segment index, t, distance).
    pub fn nearest_seg(&self, pi: usize, pos: Pt) -> Option<(usize, f32, f32)> {
        let p = &self.paths[pi];
        let n = p.anchors.len();
        if n < 2 { return None; }
        let segs = if p.closed { n } else { n - 1 };
        let mut best: Option<(usize, f32, f32)> = None;
        for i in 0..segs {
            let a = &p.anchors[i];
            let b = &p.anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            for k in 0..=24 {
                let t = k as f32 / 24.0;
                let d = dist(cubic(p0, p1, p2, p3, t), pos);
                if best.map_or(true, |(_, _, bd)| d < bd) { best = Some((i, t, d)); }
            }
        }
        best
    }

    /// Flatten any anchor ring into a polyline (steps points per segment). Shared by outer + holes.
    pub fn ring(anchors: &[Anchor], closed: bool, steps: usize) -> Vec<Pt> {
        let n = anchors.len();
        let mut poly = Vec::new();
        if n == 0 { return poly; }
        let segs = if closed { n } else { n - 1 };
        poly.push(anchors[0].p);
        for i in 0..segs {
            let a = &anchors[i]; let b = &anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            for s in 1..=steps { poly.push(cubic(p0, p1, p2, p3, s as f32 / steps as f32)); }
        }
        poly
    }
    /// Resolution-independent flatten: per-cubic step count adapts so chords stay ~4px on screen.
    pub fn ring_px(anchors: &[Anchor], closed: bool, ppu: f32) -> Vec<Pt> {
        let n = anchors.len();
        let mut poly = Vec::new();
        if n == 0 { return poly; }
        let segs = if closed { n } else { n - 1 };
        poly.push(anchors[0].p);
        for i in 0..segs {
            let a = &anchors[i]; let b = &anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            let steps = if a.hout.is_none() && b.hin.is_none() { 1 }
                else { let clen = dist(p0, p1) + dist(p1, p2) + dist(p2, p3); (((clen * ppu) / 4.0).ceil() as usize).clamp(8, 256) };
            for s in 1..=steps { poly.push(cubic(p0, p1, p2, p3, s as f32 / steps as f32)); }
        }
        poly
    }

    /// Outer outline of a path (steps per segment).
    pub fn outline(&self, pi: usize, steps: usize) -> Vec<Pt> { Self::ring(&self.paths[pi].anchors, self.paths[pi].closed, steps) }
    /// Resolution-independent outer outline (`ppu` = view zoom) — smooth at any zoom.
    pub fn outline_px(&self, pi: usize, ppu: f32) -> Vec<Pt> { Self::ring_px(&self.paths[pi].anchors, self.paths[pi].closed, ppu) }

    /// Visual (outline) bounding box of one path — used for align / distribute.
    pub fn outline_bbox(&self, pi: usize) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for q in self.outline(pi, 12) { x0 = x0.min(q[0]); y0 = y0.min(q[1]); x1 = x1.max(q[0]); y1 = y1.max(q[1]); }
        if x0 <= x1 { (x0, y0, x1, y1) } else { (0.0, 0.0, 0.0, 0.0) }
    }

    pub fn point_in_path(&self, pi: usize, pt: Pt) -> bool {
        let p = &self.paths[pi];
        if !p.closed || p.anchors.len() < 3 { return false; }
        if !point_in_poly(&self.outline(pi, 8), pt) { return false; }
        // inside the outer ring — but a point inside a hole is NOT in the (even-odd) filled region
        for h in &p.holes { if h.len() >= 3 && point_in_poly(&Self::ring(h, true, 8), pt) { return false; } }
        true
    }

    pub fn bbox(&self, pi: usize) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for a in &self.paths[pi].anchors {
            for q in [Some(a.p), a.hin, a.hout].into_iter().flatten() {
                x0 = x0.min(q[0]); y0 = y0.min(q[1]); x1 = x1.max(q[0]); y1 = y1.max(q[1]);
            }
        }
        (x0, y0, x1, y1)
    }

    /// Build a ready-made shape (returns anchors; closed by the caller).
    pub fn build_shape(&mut self, kind: ShapeKind, a: Pt, b: Pt) -> Vec<Anchor> {
        let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
        let (x1, y1) = (a[0].max(b[0]), a[1].max(b[1]));
        let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
        let mut mk = |p: Pt, hin: Option<Pt>, hout: Option<Pt>, smooth: bool| {
            self.ids += 1; Anchor { id: self.ids, p, hin, hout, smooth }
        };
        match kind {
            ShapeKind::Rect => vec![mk([x0,y0],None,None,false), mk([x1,y0],None,None,false), mk([x1,y1],None,None,false), mk([x0,y1],None,None,false)],
            ShapeKind::Triangle => vec![mk([cx,y0],None,None,false), mk([x1,y1],None,None,false), mk([x0,y1],None,None,false)],
            ShapeKind::Polygon => {
                let n = 6; let mut out = Vec::new();
                for i in 0..n {
                    let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
                    out.push(mk([cx + rx * ang.cos(), cy + ry * ang.sin()], None, None, false));
                }
                out
            }
            ShapeKind::Ellipse => {
                let (kx, ky) = (K * rx, K * ry);
                vec![
                    mk([cx,y0], Some([cx-kx,y0]), Some([cx+kx,y0]), true),
                    mk([x1,cy], Some([x1,cy-ky]), Some([x1,cy+ky]), true),
                    mk([cx,y1], Some([cx+kx,y1]), Some([cx-kx,y1]), true),
                    mk([x0,cy], Some([x0,cy+ky]), Some([x0,cy-ky]), true),
                ]
            }
        }
    }

    pub fn clone_path(&mut self, pid: u32) -> Path {
        let pi = self.pidx(pid).unwrap();
        let src = self.paths[pi].clone();
        let id = self.nid();
        let anchors = src.anchors.iter().map(|a| { self.ids += 1; Anchor { id: self.ids, p: a.p, hin: a.hin, hout: a.hout, smooth: a.smooth } }).collect();
        let holes = src.holes.iter().map(|h| h.iter().map(|a| { self.ids += 1; Anchor { id: self.ids, p: a.p, hin: a.hin, hout: a.hout, smooth: a.smooth } }).collect()).collect();
        Path { id, anchors, closed: src.closed, fill: src.fill, stroke: src.stroke, stroke_width: src.stroke_width, holes }
    }
}
