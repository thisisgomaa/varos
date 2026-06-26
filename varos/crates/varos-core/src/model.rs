//! The document data model: anchors, paths, the document. Plus pure geometry queries.
//! Stable u32 IDs (never Vec indices) so selection/active survive deletes & joins.

use crate::geom::*;
use std::collections::HashMap;

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
    /// object-level opacity 0..1 (multiplies fill+stroke alpha at render). 1.0 = opaque.
    pub opacity: f32,
    /// Layers panel flags: hidden = not drawn / not hit-tested; locked = not selectable by click.
    pub hidden: bool,
    pub locked: bool,
    /// optional custom name (Layers inline rename). None ⇒ a default label ("Path", "Rectangle"…).
    pub name: Option<String>,
}

impl Path {
    /// Sensible defaults for the panel-era fields, so construction sites stay terse.
    pub fn new(id: u32, anchors: Vec<Anchor>, closed: bool, fill: Option<Rgba>, stroke: Option<Rgba>, stroke_width: f32) -> Path {
        Path { id, anchors, closed, fill, stroke, stroke_width, holes: vec![], opacity: 1.0, hidden: false, locked: false, name: None }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ShapeKind { Rect, Ellipse, Triangle, Polygon }

/// A group: a named container for paths. `parent` lets groups nest (unused in the flat v1).
#[derive(Clone)]
pub struct Group { pub id: u32, pub name: String, pub parent: Option<u32> }

#[derive(Default, Clone)]
pub struct Document {
    pub paths: Vec<Path>,
    /// Group registry. Membership lives in `group_of` so `Path` (and every Path-construction site)
    /// stays untouched. The flat path list is still the z-order / render source of truth.
    pub groups: Vec<Group>,
    /// path id → its innermost group id. Absent = ungrouped. Reconciled by `sync_groups`.
    pub group_of: HashMap<u32, u32>,
    pub ids: u32,
}

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
        Path { holes, opacity: src.opacity, hidden: src.hidden, locked: src.locked, name: src.name.clone(),
               ..Path::new(id, anchors, src.closed, src.fill, src.stroke, src.stroke_width) }
    }

    // ---------- groups (hierarchy on top of the flat path list) ----------
    /// The outermost group ancestor of a group id (walks `parent`, with a cycle guard).
    pub fn group_root(&self, gid: u32) -> u32 {
        let mut g = gid;
        for _ in 0..4096 {
            match self.groups.iter().find(|x| x.id == g).and_then(|grp| grp.parent) {
                Some(p) => g = p,
                None => break,
            }
        }
        g
    }
    /// The top-level group a path belongs to, or None if ungrouped.
    pub fn top_group_of_path(&self, pid: u32) -> Option<u32> {
        self.group_of.get(&pid).map(|&g| self.group_root(g))
    }
    /// Every path id in the same top-level group as `pid` (in z order). Just `[pid]` if ungrouped.
    pub fn group_members(&self, pid: u32) -> Vec<u32> {
        match self.top_group_of_path(pid) {
            None => vec![pid],
            Some(top) => self.paths.iter().map(|p| p.id)
                .filter(|&q| self.top_group_of_path(q) == Some(top)).collect(),
        }
    }
    /// Group these paths into a new group (returns its id). Members become contiguous in z order.
    /// Grouping items that are themselves groups NESTS them (the existing groups become children of
    /// the new one) — so a later single ungroup peels exactly one level, like Illustrator.
    pub fn group(&mut self, pids: &[u32]) -> Option<u32> {
        use std::collections::HashSet;
        let set: HashSet<u32> = pids.iter().copied().filter(|&p| self.pidx(p).is_some()).collect();
        // need ≥2 distinct top-level UNITS (a group counts once, a lone path counts once): so
        // re-grouping a single existing group is a no-op, and grouping 2 groups nests cleanly.
        let units: HashSet<u32> = set.iter().map(|&p| self.top_group_of_path(p).unwrap_or(p)).collect();
        if units.len() < 2 { return None; }
        let gid = self.nid();
        self.groups.push(Group { id: gid, name: format!("Group {gid}"), parent: None });
        // existing top-level groups in the selection become CHILDREN of the new group (nesting)…
        let tops: Vec<u32> = set.iter().filter_map(|&p| self.top_group_of_path(p)).collect();
        for t in tops { if let Some(g) = self.groups.iter_mut().find(|g| g.id == t) { g.parent = Some(gid); } }
        // …and lone (ungrouped) paths take the new group as their innermost.
        for &p in &set { if !self.group_of.contains_key(&p) { self.group_of.insert(p, gid); } }
        self.contiguous(&set);
        Some(gid)
    }
    /// Ungroup: peel exactly ONE level off the top-level group(s) the selection belongs to. The
    /// dissolved group's direct children move up to its parent (None ⇒ top-level); inner groups survive.
    pub fn ungroup(&mut self, pids: &[u32]) {
        use std::collections::HashSet;
        let tops: HashSet<u32> = pids.iter().filter_map(|&p| self.top_group_of_path(p)).collect();
        for top in tops {
            let parent = self.groups.iter().find(|g| g.id == top).and_then(|g| g.parent);
            for g in self.groups.iter_mut() { if g.parent == Some(top) { g.parent = parent; } } // child groups rise
            let direct: Vec<u32> = self.group_of.iter().filter(|(_, &g)| g == top).map(|(&p, _)| p).collect();
            for p in direct { match parent { Some(pg) => { self.group_of.insert(p, pg); } None => { self.group_of.remove(&p); } } }
            self.groups.retain(|g| g.id != top);
        }
    }
    /// Duplicate a set of paths, PRESERVING their group structure: the copies form parallel groups
    /// that mirror the originals' nesting. Returns the new path ids (in `srcs` order). Used by the
    /// Alt-drag duplicate so copying a group yields a group, not loose paths.
    pub fn dup_paths(&mut self, srcs: &[u32]) -> Vec<u32> {
        use std::collections::HashMap;
        // 1) clone the paths (clone_path gives fresh anchor + path ids)
        let mut pmap: HashMap<u32, u32> = HashMap::new();
        let mut new_pids = vec![];
        for &s in srcs {
            if self.pidx(s).is_none() { continue; }
            let c = self.clone_path(s);
            pmap.insert(s, c.id);
            new_pids.push(c.id);
            self.paths.push(c);
        }
        // 2) every old group in the chains of the sources (innermost → root)
        let mut old_groups: Vec<u32> = vec![];
        for &s in srcs {
            let mut g = self.group_of.get(&s).copied();
            while let Some(gid) = g {
                if !old_groups.contains(&gid) { old_groups.push(gid); }
                g = self.groups.iter().find(|x| x.id == gid).and_then(|x| x.parent);
            }
        }
        // 3) a parallel new group for each old one
        let mut gmap: HashMap<u32, u32> = HashMap::new();
        for &og in &old_groups {
            let ng = self.nid();
            let name = self.groups.iter().find(|x| x.id == og).map(|x| x.name.clone()).unwrap_or_else(|| format!("Group {ng}"));
            self.groups.push(Group { id: ng, name, parent: None });
            gmap.insert(og, ng);
        }
        // 4) mirror parent links inside the duplicated subtree
        for &og in &old_groups {
            let old_parent = self.groups.iter().find(|x| x.id == og).and_then(|x| x.parent);
            let new_parent = old_parent.and_then(|p| gmap.get(&p).copied());
            let ng = gmap[&og];
            if let Some(g) = self.groups.iter_mut().find(|x| x.id == ng) { g.parent = new_parent; }
        }
        // 5) attach each clone to its mirrored innermost group
        for (&old_p, &new_p) in &pmap {
            if let Some(&old_g) = self.group_of.get(&old_p) {
                if let Some(&new_g) = gmap.get(&old_g) { self.group_of.insert(new_p, new_g); }
            }
        }
        new_pids
    }
    /// Reorder `paths` so the given ids form one contiguous run ending at the topmost member's z
    /// position (Illustrator brings a group up to its front-most member). Inner order preserved.
    fn contiguous(&mut self, set: &std::collections::HashSet<u32>) {
        let n = self.paths.len();
        let top_idx = match (0..n).filter(|&i| set.contains(&self.paths[i].id)).max() { Some(i) => i, None => return };
        let above = (top_idx + 1..n).filter(|&i| !set.contains(&self.paths[i].id)).count();
        let taken = std::mem::take(&mut self.paths);
        let (block, mut rest): (Vec<Path>, Vec<Path>) = taken.into_iter().partition(|p| set.contains(&p.id));
        let split = rest.len() - above;
        let tail = rest.split_off(split);
        rest.extend(block);
        rest.extend(tail);
        self.paths = rest;
    }
    /// Reconcile group bookkeeping after path create/delete: drop membership for dead paths and
    /// remove groups that hold nothing. A group stays alive if it is some live path's innermost
    /// group OR an ancestor of such a group (so nesting levels aren't pruned away).
    pub fn sync_groups(&mut self) {
        use std::collections::HashSet;
        let live: HashSet<u32> = self.paths.iter().map(|p| p.id).collect();
        self.group_of.retain(|pid, _| live.contains(pid));
        let mut alive: HashSet<u32> = self.group_of.values().copied().collect();
        let mut frontier: Vec<u32> = alive.iter().copied().collect();
        while let Some(g) = frontier.pop() {
            if let Some(p) = self.groups.iter().find(|x| x.id == g).and_then(|x| x.parent) {
                if alive.insert(p) { frontier.push(p); }
            }
        }
        self.groups.retain(|g| alive.contains(&g.id));
    }
}
