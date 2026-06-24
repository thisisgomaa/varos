// Varos — Phase 0 spike, ported to full parity with pen-spike.html.
// Self-drawn wgpu GPU canvas in a winit desktop window. Stable IDs (no index-shift bugs).
// Cursors (the white+outline set) deferred for now.

#![allow(dead_code)]
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowBuilder},
};

// ===================== vertex / shader =====================
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex { pos: [f32; 2], color: [f32; 4] }

const SHADER: &str = r#"
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var o: VsOut; o.clip = vec4<f32>(pos, 0.0, 1.0); o.color = color; return o;
}
@fragment fn fs(in: VsOut) -> @location(0) vec4<f32> { return in.color; }
"#;

// ===================== math =====================
type P = [f32; 2];
fn sub(a: P, b: P) -> P { [a[0]-b[0], a[1]-b[1]] }
fn add(a: P, b: P) -> P { [a[0]+b[0], a[1]+b[1]] }
fn scale(a: P, k: f32) -> P { [a[0]*k, a[1]*k] }
fn dist(a: P, b: P) -> f32 { ((a[0]-b[0]).powi(2) + (a[1]-b[1]).powi(2)).sqrt() }
fn length(v: P) -> f32 { (v[0]*v[0] + v[1]*v[1]).sqrt() }
fn norm(v: P) -> P { let m = length(v).max(1e-4); [v[0]/m, v[1]/m] }
fn mirror(p: P, q: P) -> P { [2.0*p[0]-q[0], 2.0*p[1]-q[1]] }
fn cubic(p0: P, p1: P, p2: P, p3: P, t: f32) -> P {
    let u = 1.0 - t;
    [u*u*u*p0[0] + 3.0*u*u*t*p1[0] + 3.0*u*t*t*p2[0] + t*t*t*p3[0],
     u*u*u*p0[1] + 3.0*u*u*t*p1[1] + 3.0*u*t*t*p2[1] + t*t*t*p3[1]]
}
fn snap45(v: P) -> P {
    let a = v[1].atan2(v[0]); let step = std::f32::consts::FRAC_PI_4;
    let s = (a / step).round() * step; let m = length(v);
    [s.cos()*m, s.sin()*m]
}

// ===================== model =====================
#[derive(Clone)]
struct Anchor { id: u32, p: P, hin: Option<P>, hout: Option<P>, smooth: bool }
#[derive(Clone)]
struct Path { id: u32, anchors: Vec<Anchor>, closed: bool }

#[derive(Clone, Copy, PartialEq)]
enum Tool { Object, Direct, Pen, Rect, Ellipse, Triangle, Polygon, Convert }
fn is_shape(t: Tool) -> bool { matches!(t, Tool::Rect | Tool::Ellipse | Tool::Triangle | Tool::Polygon) }

enum Drag {
    None,
    PenNew { aid: u32, down: P, broken: bool },
    PenClose { aid: u32, down: P, broken: bool },
    Anchors { start: P, items: Vec<(u32, P, Option<P>, Option<P>)>, constrain: bool },
    Handle { aid: u32, out: bool, couple: bool, opp_len: f32, grab: P },
    Segment { pid: u32, i: usize, down: P, a_out0: Option<P>, b_in0: Option<P>, ap0: P, bp0: P, straight: bool },
    Shape { start: P, pid: u32, kind: Tool },
    Marquee { start: P, base: Vec<u32> },
    DupPending { src_pid: u32, down: P, object: bool },
    Object { down: P, base: Vec<(u32, P, Option<P>, Option<P>)> },
    ConvPull { aid: u32, down: P },
}

const DRAG_THRESH: f32 = 4.0;
const CLOSE_R: f32 = 11.0;
const ANCHOR_R: f32 = 12.0;
const HANDLE_R: f32 = 11.0;
const EDGE_R: f32 = 8.0;
const HANDLE_LEN: f32 = 45.0;
const K: f32 = 0.5522847;

const BG: [f32; 4] = [0.117, 0.117, 0.117, 1.0];
const ACCENT: [f32; 4] = [0.047, 0.549, 0.914, 1.0];
const ACCENT_FILL: [f32; 4] = [0.047, 0.549, 0.914, 0.14];
const HANDLE_COL: [f32; 4] = [0.498, 0.737, 0.941, 1.0];
const WHITE: [f32; 4] = [0.96, 0.96, 0.96, 1.0];
const GREEN: [f32; 4] = [0.204, 0.78, 0.349, 1.0];
const BTN_BG: [f32; 4] = [0.18, 0.18, 0.18, 1.0];
const GRID: [f32; 4] = [0.16, 0.16, 0.16, 1.0];

struct App {
    doc: Vec<Path>,
    tool: Tool,
    gesture: Tool,
    active: Option<u32>,            // path id the pen is extending
    selected: HashSet<u32>,         // anchor ids (white arrow)
    objsel: HashSet<u32>,           // path ids (black arrow)
    drag: Drag,
    cursor: P,
    hover_path: Option<u32>,
    shift: bool, alt: bool, ctrl: bool,
    ids: u32,
    undo: Vec<Vec<Path>>, redo: Vec<Vec<Path>>, pending: Option<Vec<Path>>, dirty: bool,
    last_click: Option<(Instant, P)>,
}

impl App {
    fn new() -> Self {
        App { doc: vec![], tool: Tool::Pen, gesture: Tool::Pen, active: None,
              selected: HashSet::new(), objsel: HashSet::new(), drag: Drag::None,
              cursor: [0.0, 0.0], hover_path: None, shift: false, alt: false, ctrl: false,
              ids: 1, undo: vec![], redo: vec![], pending: None, dirty: false, last_click: None }
    }
    fn nid(&mut self) -> u32 { let i = self.ids; self.ids += 1; i }

    // ---- lookups (id -> index) ----
    fn pidx(&self, pid: u32) -> Option<usize> { self.doc.iter().position(|p| p.id == pid) }
    fn aidx(&self, aid: u32) -> Option<(usize, usize)> {
        for (pi, p) in self.doc.iter().enumerate() {
            if let Some(ai) = p.anchors.iter().position(|a| a.id == aid) { return Some((pi, ai)); }
        }
        None
    }
    fn anchor(&self, aid: u32) -> Option<&Anchor> { self.aidx(aid).map(|(pi, ai)| &self.doc[pi].anchors[ai]) }

    fn is_editable(&self, pid: u32) -> bool {
        self.active == Some(pid) || self.objsel.contains(&pid)
            || self.doc.iter().find(|p| p.id == pid).map_or(false, |p| p.anchors.iter().any(|a| self.selected.contains(&a.id)))
    }
    fn path_shown(&self, pid: u32) -> bool {
        self.active == Some(pid) || self.hover_path == Some(pid) || self.objsel.contains(&pid)
            || self.doc.iter().find(|p| p.id == pid).map_or(false, |p| p.anchors.iter().any(|a| self.selected.contains(&a.id)))
    }

    // ---- geometry hit-testing ----
    fn nearest_anchor(&self, pos: P, r: f32, shown_only: bool) -> Option<u32> {
        let mut best: Option<(u32, f32)> = None;
        for p in &self.doc {
            if shown_only && !self.path_shown(p.id) { continue; }
            for a in &p.anchors {
                let d = dist(pos, a.p);
                if d <= r && best.map_or(true, |(_, bd)| d < bd) { best = Some((a.id, d)); }
            }
        }
        best.map(|(id, _)| id)
    }
    // nearest point on a path's outline: (segment index, t, distance)
    fn nearest_seg(&self, pi: usize, pos: P) -> Option<(usize, f32, f32)> {
        let p = &self.doc[pi]; let n = p.anchors.len();
        if n < 2 { return None; }
        let segs = if p.closed { n } else { n - 1 };
        let mut best: Option<(usize, f32, f32)> = None;
        for i in 0..segs {
            let a = &p.anchors[i]; let b = &p.anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            for k in 0..=24 {
                let t = k as f32 / 24.0;
                let d = dist(cubic(p0, p1, p2, p3, t), pos);
                if best.map_or(true, |(_, _, bd)| d < bd) { best = Some((i, t, d)); }
            }
        }
        best
    }
    fn point_in_path(&self, pi: usize, pt: P) -> bool {
        let p = &self.doc[pi];
        if !p.closed || p.anchors.len() < 3 { return false; }
        // sample outline into a polygon, even-odd ray test
        let mut poly: Vec<P> = Vec::new();
        let n = p.anchors.len();
        for i in 0..n {
            let a = &p.anchors[i]; let b = &p.anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            for k in 0..8 { poly.push(cubic(p0, p1, p2, p3, k as f32 / 8.0)); }
        }
        let mut inside = false; let m = poly.len(); let mut j = m - 1;
        for i in 0..m {
            let (a, b) = (poly[i], poly[j]);
            if (a[1] > pt[1]) != (b[1] > pt[1]) {
                let x = (b[0]-a[0]) * (pt[1]-a[1]) / (b[1]-a[1]) + a[0];
                if pt[0] < x { inside = !inside; }
            }
            j = i;
        }
        inside
    }
    // which path is under the cursor (edge within EDGE_R, or inside a closed fill) — for hover & V/A body clicks
    fn path_under(&self, pos: P) -> Option<u32> {
        // prefer edge hit
        let mut best: Option<(u32, f32)> = None;
        for pi in 0..self.doc.len() {
            if let Some((_, _, d)) = self.nearest_seg(pi, pos) {
                if d <= EDGE_R && best.map_or(true, |(_, bd)| d < bd) { best = Some((self.doc[pi].id, d)); }
            }
        }
        if let Some((id, _)) = best { return Some(id); }
        // else fill
        for pi in 0..self.doc.len() { if self.point_in_path(pi, pos) { return Some(self.doc[pi].id); } }
        None
    }

    // ---- undo ----
    fn begin(&mut self) { self.pending = Some(self.doc.clone()); self.dirty = false; }
    fn commit(&mut self) {
        if self.dirty { if let Some(p) = self.pending.take() { self.undo.push(p); if self.undo.len() > 200 { self.undo.remove(0); } self.redo.clear(); } }
        self.pending = None; self.dirty = false;
    }
    fn undo(&mut self) { if let Some(s) = self.undo.pop() { self.redo.push(self.doc.clone()); self.doc = s; self.selected.clear(); self.objsel.clear(); self.active = None; self.drag = Drag::None; } }
    fn redo(&mut self) { if let Some(s) = self.redo.pop() { self.undo.push(self.doc.clone()); self.doc = s; self.selected.clear(); self.objsel.clear(); self.active = None; self.drag = Drag::None; } }

    fn eff_tool(&self) -> Tool {
        if self.ctrl { Tool::Direct } else if self.tool == Tool::Pen && self.alt { Tool::Convert } else { self.tool }
    }

    // ---- pointer down ----
    fn down(&mut self, pos: P) {
        self.begin();
        self.gesture = self.eff_tool();
        match self.gesture {
            Tool::Pen => self.pen_down(pos),
            Tool::Direct => self.dir_down(pos),
            Tool::Convert => self.conv_down(pos),
            Tool::Object => self.obj_down(pos),
            t if is_shape(t) => self.shape_down(pos),
            _ => {}
        }
    }

    fn pen_down(&mut self, pos: P) {
        // hit an anchor?
        if let Some(aid) = self.nearest_anchor(pos, ANCHOR_R, true) {
            let (pi, ai) = self.aidx(aid).unwrap();
            let pid = self.doc[pi].id; let n = self.doc[pi].anchors.len();
            let is_end = !self.doc[pi].closed && (ai == 0 || ai == n - 1);
            let tip = self.active.and_then(|ap| self.pidx(ap)).and_then(|i| self.doc[i].anchors.last().map(|a| a.id));
            if is_end {
                if let Some(act) = self.active {
                    if act == pid {
                        if Some(aid) != tip { // close (curved if dragged)
                            if let Some(i) = self.pidx(pid) { self.doc[i].closed = true; }
                            self.dirty = true;
                            self.drag = Drag::PenClose { aid, down: pos, broken: false };
                        }
                        return;
                    } else { self.join(act, pid, aid, pos); self.dirty = true; return; }
                } else { self.resume(pid, aid); return; }
            }
            // middle anchor -> delete (only if editable)
            if self.is_editable(pid) { self.delete_anchor(aid); self.dirty = true; }
            return;
        }
        // on a segment & editable -> add anchor
        if let Some(pid) = self.path_under(pos) {
            if self.is_editable(pid) {
                if let Some(pi) = self.pidx(pid) {
                    if let Some((i, t, d)) = self.nearest_seg(pi, pos) {
                        if d <= EDGE_R { let nid = self.add_anchor(pi, i, t); self.selected.insert(nid); self.dirty = true; return; }
                    }
                }
            }
        }
        // else: extend / start a path
        let pid = match self.active { Some(i) => i, None => {
            let id = self.nid(); self.doc.push(Path { id, anchors: vec![], closed: false }); self.active = Some(id); self.selected.clear(); id
        }};
        let aid = self.nid();
        let pi = self.pidx(pid).unwrap();
        self.doc[pi].anchors.push(Anchor { id: aid, p: pos, hin: None, hout: None, smooth: false });
        self.selected.insert(aid);
        self.dirty = true;
        self.drag = Drag::PenNew { aid, down: pos, broken: false };
    }

    fn resume(&mut self, pid: u32, end_aid: u32) {
        let pi = self.pidx(pid).unwrap();
        let last = self.doc[pi].anchors.last().map(|a| a.id);
        if last != Some(end_aid) { self.reverse(pi); }
        self.active = Some(pid);
        self.selected.clear();
        for a in &self.doc[pi].anchors { self.selected.insert(a.id); }
    }
    fn join(&mut self, act: u32, other: u32, end_aid: u32, pos: P) {
        let oi = self.pidx(other).unwrap();
        if self.doc[oi].anchors.first().map(|a| a.id) != Some(end_aid) { self.reverse(oi); }
        let moved: Vec<Anchor> = self.doc[oi].anchors.clone();
        self.doc.remove(oi);
        let ai = self.pidx(act).unwrap();
        for a in moved { self.doc[ai].anchors.push(a); }
        self.selected.clear();
        for a in &self.doc[ai].anchors { self.selected.insert(a.id); }
        self.drag = Drag::PenNew { aid: end_aid, down: pos, broken: false };
    }
    fn reverse(&mut self, pi: usize) {
        self.doc[pi].anchors.reverse();
        for a in &mut self.doc[pi].anchors { std::mem::swap(&mut a.hin, &mut a.hout); }
    }
    fn delete_anchor(&mut self, aid: u32) {
        if let Some((pi, ai)) = self.aidx(aid) {
            self.doc[pi].anchors.remove(ai);
            self.selected.remove(&aid);
            if self.doc[pi].anchors.len() < 2 { self.doc[pi].closed = false; }
            if self.doc[pi].anchors.is_empty() { let pid = self.doc[pi].id; self.doc.remove(pi); if self.active == Some(pid) { self.active = None; } }
        }
    }
    fn add_anchor(&mut self, pi: usize, i: usize, t: f32) -> u32 {
        let n = self.doc[pi].anchors.len();
        let a = self.doc[pi].anchors[i].clone();
        let b = self.doc[pi].anchors[(i + 1) % n].clone();
        let nid = self.nid();
        if a.hout.is_none() && b.hin.is_none() {
            let pt = cubic(a.p, a.p, b.p, b.p, t);
            self.doc[pi].anchors.insert(i + 1, Anchor { id: nid, p: pt, hin: None, hout: None, smooth: false });
        } else {
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            // De Casteljau split
            let l = |u: P, v: P| [u[0] + (v[0]-u[0])*t, u[1] + (v[1]-u[1])*t];
            let (q0, q1, q2) = (l(p0, p1), l(p1, p2), l(p2, p3));
            let (r0, r1) = (l(q0, q1), l(q1, q2));
            let mid = l(r0, r1);
            self.doc[pi].anchors[i].hout = Some(q0);
            let bi = (i + 1) % n;
            self.doc[pi].anchors[bi].hin = Some(q2);
            self.doc[pi].anchors.insert(i + 1, Anchor { id: nid, p: mid, hin: Some(r0), hout: Some(r1), smooth: true });
        }
        nid
    }

    fn conv_down(&mut self, pos: P) {
        // handle -> break
        if let Some(aid) = self.handle_hit(pos) {
            let (pi, ai) = self.aidx(aid).unwrap();
            self.doc[pi].anchors[ai].smooth = false;
            self.selected.insert(aid);
            let out = self.which_handle(aid, pos);
            let hp = self.handle_pos(aid, out).unwrap();
            self.dirty = true;
            self.drag = Drag::Handle { aid, out, couple: false, opp_len: 0.0, grab: sub(hp, pos) };
            return;
        }
        if let Some(aid) = self.nearest_anchor(pos, ANCHOR_R, true) {
            let (pi, ai) = self.aidx(aid).unwrap();
            self.selected.clear(); self.selected.insert(aid);
            if self.doc[pi].anchors[ai].smooth {
                self.toggle_type(aid); self.dirty = true;
            } else {
                self.drag = Drag::ConvPull { aid, down: pos };
            }
            return;
        }
        if let Some(pid) = self.path_under(pos) { if let Some(pi) = self.pidx(pid) {
            if let Some((i, _, d)) = self.nearest_seg(pi, pos) { if d <= EDGE_R { self.start_segment(pid, i, pos); } }
        }}
    }
    fn toggle_type(&mut self, aid: u32) {
        let (pi, ai) = self.aidx(aid).unwrap();
        if self.doc[pi].anchors[ai].smooth {
            let a = &mut self.doc[pi].anchors[ai]; a.smooth = false; a.hin = None; a.hout = None;
        } else {
            let dir = self.tangent(pi, ai);
            let p = self.doc[pi].anchors[ai].p;
            let a = &mut self.doc[pi].anchors[ai];
            a.smooth = true;
            a.hout = Some(add(p, scale(dir, HANDLE_LEN)));
            a.hin = Some(add(p, scale(dir, -HANDLE_LEN)));
        }
    }
    fn tangent(&self, pi: usize, ai: usize) -> P {
        let p = &self.doc[pi]; let n = p.anchors.len();
        let prev = if ai > 0 { Some(&p.anchors[ai-1]) } else if p.closed { Some(&p.anchors[n-1]) } else { None };
        let next = if ai < n-1 { Some(&p.anchors[ai+1]) } else if p.closed { Some(&p.anchors[0]) } else { None };
        match (prev, next) {
            (Some(a), Some(b)) => norm(sub(b.p, a.p)),
            (None, Some(b)) => norm(sub(b.p, p.anchors[ai].p)),
            (Some(a), None) => norm(sub(p.anchors[ai].p, a.p)),
            _ => [1.0, 0.0],
        }
    }

    fn handle_hit(&self, pos: P) -> Option<u32> {
        // handles shown for: selected anchors (+ their drawn handles). check those.
        for &aid in &self.selected {
            if let Some(a) = self.anchor(aid) {
                for h in [a.hin, a.hout].into_iter().flatten() { if dist(pos, h) <= HANDLE_R { return Some(aid); } }
            }
        }
        // also active pen path anchors
        if let Some(ap) = self.active { if let Some(pi) = self.pidx(ap) {
            for a in &self.doc[pi].anchors { for h in [a.hin, a.hout].into_iter().flatten() { if dist(pos, h) <= HANDLE_R { return Some(a.id); } } }
        }}
        None
    }
    fn which_handle(&self, aid: u32, pos: P) -> bool {
        let a = self.anchor(aid).unwrap();
        let dout = a.hout.map_or(f32::MAX, |h| dist(pos, h));
        let din = a.hin.map_or(f32::MAX, |h| dist(pos, h));
        dout <= din
    }
    fn handle_pos(&self, aid: u32, out: bool) -> Option<P> {
        let a = self.anchor(aid)?; if out { a.hout } else { a.hin }
    }

    fn dir_down(&mut self, pos: P) {
        // handle FIRST — so Alt over a handle = BREAK it (must beat the Alt-duplicate below)
        if let Some(aid) = self.handle_hit(pos) {
            let out = self.which_handle(aid, pos);
            let a = self.anchor(aid).unwrap().clone();
            let hp = if out { a.hout } else { a.hin }.unwrap();
            let couple = !self.alt && a.hin.is_some() && a.hout.is_some() && {
                let vi = sub(a.hin.unwrap(), a.p); let vo = sub(a.hout.unwrap(), a.p);
                let mut d = (vi[1].atan2(vi[0]) - vo[1].atan2(vo[0])).abs();
                if d > std::f32::consts::PI { d = 2.0*std::f32::consts::PI - d; }
                d > std::f32::consts::PI - 0.15
            };
            let opp = if out { a.hin } else { a.hout };
            let opp_len = opp.map_or(0.0, |o| dist(a.p, o));
            if self.alt { let (pi, ai) = self.aidx(aid).unwrap(); self.doc[pi].anchors[ai].smooth = false; }
            self.selected.insert(aid);
            self.dirty = true;
            self.drag = Drag::Handle { aid, out, couple, opp_len, grab: sub(hp, pos) };
            return;
        }
        // Alt + anchor/path => duplicate on drag (handles already handled above)
        if self.alt {
            if let Some(aid) = self.nearest_anchor(pos, ANCHOR_R, true) {
                let (pi, _) = self.aidx(aid).unwrap(); let pid = self.doc[pi].id;
                if !self.shift { self.selected.clear(); } self.selected.insert(aid);
                self.drag = Drag::DupPending { src_pid: pid, down: pos, object: false }; return;
            }
            if let Some(pid) = self.path_under(pos) {
                self.drag = Drag::DupPending { src_pid: pid, down: pos, object: false }; return;
            }
        }
        // anchor
        if let Some(aid) = self.nearest_anchor(pos, ANCHOR_R, true) {
            if self.shift { if self.selected.contains(&aid) { self.selected.remove(&aid); } else { self.selected.insert(aid); } }
            else if !self.selected.contains(&aid) { self.selected.clear(); self.selected.insert(aid); }
            self.begin_anchor_drag(pos);
            return;
        }
        // path body
        if let Some(pid) = self.path_under(pos) {
            if let Some(pi) = self.pidx(pid) {
                let edge = self.nearest_seg(pi, pos).map_or(false, |(_, _, d)| d <= EDGE_R);
                if edge {
                    let (i, _, _) = self.nearest_seg(pi, pos).unwrap();
                    self.start_segment(pid, i, pos); return;
                }
            }
            if !self.shift { self.selected.clear(); }
            if let Some(pi) = self.pidx(pid) { for a in &self.doc[pi].anchors { self.selected.insert(a.id); } }
            self.begin_anchor_drag(pos); return;
        }
        // empty -> marquee
        if !self.shift { self.selected.clear(); }
        let base: Vec<u32> = self.selected.iter().copied().collect();
        self.drag = Drag::Marquee { start: pos, base };
    }
    fn begin_anchor_drag(&mut self, pos: P) {
        let items: Vec<(u32, P, Option<P>, Option<P>)> = self.selected.iter().filter_map(|&aid| {
            self.anchor(aid).map(|a| (aid, a.p, a.hin, a.hout))
        }).collect();
        self.drag = Drag::Anchors { start: pos, items, constrain: false };
    }
    fn start_segment(&mut self, pid: u32, i: usize, pos: P) {
        let pi = self.pidx(pid).unwrap(); let n = self.doc[pi].anchors.len();
        let a = self.doc[pi].anchors[i].clone(); let b = self.doc[pi].anchors[(i+1)%n].clone();
        self.drag = Drag::Segment { pid, i, down: pos, a_out0: a.hout, b_in0: b.hin, ap0: a.p, bp0: b.p, straight: a.hout.is_none() && b.hin.is_none() };
    }

    fn obj_down(&mut self, pos: P) {
        if let Some(pid) = self.path_under(pos) {
            if self.alt { self.drag = Drag::DupPending { src_pid: pid, down: pos, object: true }; return; }
            if !self.shift && !self.objsel.contains(&pid) { self.objsel.clear(); }
            self.objsel.insert(pid);
            let mut base = vec![];
            for &p in &self.objsel { if let Some(pi) = self.pidx(p) { for a in &self.doc[pi].anchors { base.push((a.id, a.p, a.hin, a.hout)); } } }
            self.drag = Drag::Object { down: pos, base };
            return;
        }
        self.objsel.clear();
    }

    fn shape_down(&mut self, pos: P) {
        self.selected.clear(); self.objsel.clear();
        let id = self.nid();
        let kind = self.gesture;
        let anchors = build_shape(self, kind, pos, pos);
        self.doc.push(Path { id, anchors, closed: true });
        self.dirty = true;
        self.drag = Drag::Shape { start: pos, pid: id, kind };
    }

    // ---- pointer move ----
    fn moved(&mut self, pos: P) {
        self.cursor = pos;
        if matches!(self.drag, Drag::None) { self.hover_path = self.path_under(pos); }
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::PenNew { aid, down, mut broken } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if self.alt { broken = true; }
                    if let Some((pi, ai)) = self.aidx(aid) {
                        let q = if self.shift { add(self.doc[pi].anchors[ai].p, snap45(sub(pos, self.doc[pi].anchors[ai].p))) } else { pos };
                        let p = self.doc[pi].anchors[ai].p;
                        let a = &mut self.doc[pi].anchors[ai];
                        a.hout = Some(q);
                        if broken { a.smooth = false; } else { a.smooth = true; a.hin = Some(mirror(p, q)); }
                    }
                }
                self.drag = Drag::PenNew { aid, down, broken };
            }
            Drag::PenClose { aid, down, mut broken } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if self.alt { broken = true; }
                    if let Some((pi, ai)) = self.aidx(aid) {
                        let p = self.doc[pi].anchors[ai].p;
                        let a = &mut self.doc[pi].anchors[ai];
                        a.hout = Some(pos);
                        if broken { a.smooth = false; } else { a.smooth = true; a.hin = Some(mirror(p, pos)); }
                    }
                }
                self.drag = Drag::PenClose { aid, down, broken };
            }
            Drag::Anchors { start, items, constrain } => {
                let mut d = sub(pos, start);
                if self.shift { d = snap45(d); }
                for (aid, p0, hin0, hout0) in &items {
                    if let Some((pi, ai)) = self.aidx(*aid) {
                        let a = &mut self.doc[pi].anchors[ai];
                        a.p = add(*p0, d);
                        a.hin = hin0.map(|h| add(h, d));
                        a.hout = hout0.map(|h| add(h, d));
                    }
                }
                self.drag = Drag::Anchors { start, items, constrain };
                self.dirty = true;
            }
            Drag::Handle { aid, out, couple, opp_len, grab } => {
                if let Some((pi, ai)) = self.aidx(aid) {
                    let p = self.doc[pi].anchors[ai].p;
                    let mut q = add(pos, grab);
                    if self.shift { q = add(p, snap45(sub(q, p))); }
                    {
                        let a = &mut self.doc[pi].anchors[ai];
                        if out { a.hout = Some(q); } else { a.hin = Some(q); }
                    }
                    if couple {
                        let opp = add(p, scale(norm(sub(p, q)), opp_len));
                        let a = &mut self.doc[pi].anchors[ai];
                        if out { a.hin = Some(opp); } else { a.hout = Some(opp); }
                    }
                }
                self.drag = Drag::Handle { aid, out, couple, opp_len, grab };
                self.dirty = true;
            }
            Drag::Segment { pid, i, down, a_out0, b_in0, ap0, bp0, straight } => {
                let d = sub(pos, down);
                if let Some(pi) = self.pidx(pid) {
                    let n = self.doc[pi].anchors.len();
                    let bi = (i + 1) % n;
                    if straight {
                        self.doc[pi].anchors[i].p = add(ap0, d);
                        self.doc[pi].anchors[bi].p = add(bp0, d);
                    } else {
                        self.doc[pi].anchors[i].hout = Some(add(a_out0.unwrap_or(ap0), d));
                        self.doc[pi].anchors[bi].hin = Some(add(b_in0.unwrap_or(bp0), d));
                    }
                }
                self.drag = Drag::Segment { pid, i, down, a_out0, b_in0, ap0, bp0, straight };
                self.dirty = true;
            }
            Drag::Shape { start, pid, kind } => {
                let (sh, al) = (self.shift, self.alt);
                let anchors = build_shape_mod(self, kind, start, pos, sh, al);
                if let Some(pi) = self.pidx(pid) { self.doc[pi].anchors = anchors; }
                self.drag = Drag::Shape { start, pid, kind };
                self.dirty = true;
            }
            Drag::Marquee { start, base } => {
                let (x0, y0) = (start[0].min(pos[0]), start[1].min(pos[1]));
                let (x1, y1) = (start[0].max(pos[0]), start[1].max(pos[1]));
                self.selected = base.iter().copied().collect();
                for p in &self.doc { for a in &p.anchors {
                    if a.p[0] >= x0 && a.p[0] <= x1 && a.p[1] >= y0 && a.p[1] <= y1 { self.selected.insert(a.id); }
                }}
                self.drag = Drag::Marquee { start, base };
            }
            Drag::DupPending { src_pid, down, object } => {
                if dist(pos, down) < DRAG_THRESH { self.drag = Drag::DupPending { src_pid, down, object }; }
                else {
                    let clone = self.clone_path(src_pid);
                    let cid = clone.id; self.doc.push(clone);
                    if object { self.objsel.clear(); self.objsel.insert(cid);
                        let mut base = vec![]; if let Some(pi) = self.pidx(cid) { for a in &self.doc[pi].anchors { base.push((a.id, a.p, a.hin, a.hout)); } }
                        self.drag = Drag::Object { down, base };
                    } else { self.selected.clear();
                        if let Some(pi) = self.pidx(cid) { for a in &self.doc[pi].anchors { self.selected.insert(a.id); } }
                        self.begin_anchor_drag(down);
                    }
                    self.dirty = true;
                    // apply this move immediately
                    self.moved(pos);
                }
            }
            Drag::Object { down, base } => {
                let mut d = sub(pos, down); if self.shift { d = snap45(d); }
                for (aid, p0, hin0, hout0) in &base {
                    if let Some((pi, ai)) = self.aidx(*aid) {
                        let a = &mut self.doc[pi].anchors[ai];
                        a.p = add(*p0, d); a.hin = hin0.map(|h| add(h, d)); a.hout = hout0.map(|h| add(h, d));
                    }
                }
                self.drag = Drag::Object { down, base };
                self.dirty = true;
            }
            Drag::ConvPull { aid, down } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if let Some((pi, ai)) = self.aidx(aid) {
                        let p = self.doc[pi].anchors[ai].p;
                        let q = if self.shift { add(p, snap45(sub(pos, p))) } else { pos };
                        let a = &mut self.doc[pi].anchors[ai];
                        a.smooth = true; a.hout = Some(q); a.hin = Some(mirror(p, q));
                    }
                    self.dirty = true;
                }
                self.drag = Drag::ConvPull { aid, down };
            }
            Drag::None => {}
        }
    }

    fn up(&mut self) {
        if let Drag::Shape { pid, .. } = self.drag {
            if let Some(pi) = self.pidx(pid) {
                let b = bbox(&self.doc[pi]);
                if (b.2 - b.0) < 2.0 && (b.3 - b.1) < 2.0 { self.doc.remove(pi); self.dirty = false; }
            }
        }
        if let Drag::PenClose { .. } = self.drag { self.active = None; }
        self.drag = Drag::None;
        self.commit();
    }

    fn clone_path(&mut self, pid: u32) -> Path {
        let pi = self.pidx(pid).unwrap();
        let src = self.doc[pi].clone();
        let id = self.nid();
        let anchors = src.anchors.iter().map(|a| Anchor { id: self.ids_next(), p: a.p, hin: a.hin, hout: a.hout, smooth: a.smooth }).collect();
        Path { id, anchors, closed: src.closed }
    }
    fn ids_next(&mut self) -> u32 { let i = self.ids; self.ids += 1; i }

    fn nudge(&mut self, dx: f32, dy: f32) {
        if self.selected.is_empty() { return; }
        self.begin();
        let ids: Vec<u32> = self.selected.iter().copied().collect();
        for aid in ids { if let Some((pi, ai)) = self.aidx(aid) {
            let a = &mut self.doc[pi].anchors[ai];
            a.p = add(a.p, [dx, dy]); a.hin = a.hin.map(|h| add(h, [dx, dy])); a.hout = a.hout.map(|h| add(h, [dx, dy]));
        }}
        self.dirty = true; self.commit();
    }
    fn delete_selected(&mut self) {
        self.begin();
        if !self.selected.is_empty() {
            let ids: Vec<u32> = self.selected.iter().copied().collect();
            for aid in ids { self.delete_anchor(aid); }
        } else if !self.objsel.is_empty() {
            let pids: Vec<u32> = self.objsel.iter().copied().collect();
            for pid in pids { if let Some(pi) = self.pidx(pid) { self.doc.remove(pi); } }
            self.objsel.clear();
        }
        self.dirty = true; self.commit();
    }

    fn set_tool(&mut self, t: Tool) {
        self.tool = t;
        if t != Tool::Pen { self.active = None; }
        self.drag = Drag::None;
    }
    fn key(&mut self, code: KeyCode) {
        match code {
            KeyCode::KeyV => self.set_tool(Tool::Object),
            KeyCode::KeyA => self.set_tool(Tool::Direct),
            KeyCode::KeyP => self.set_tool(Tool::Pen),
            KeyCode::KeyM => self.set_tool(Tool::Rect),
            KeyCode::KeyL => self.set_tool(Tool::Ellipse),
            KeyCode::Escape | KeyCode::Enter => { self.active = None; self.selected.clear(); self.objsel.clear(); self.drag = Drag::None; }
            KeyCode::Delete | KeyCode::Backspace => self.delete_selected(),
            KeyCode::ArrowLeft => { let s = if self.shift {10.0} else {1.0}; self.nudge(-s, 0.0); }
            KeyCode::ArrowRight => { let s = if self.shift {10.0} else {1.0}; self.nudge(s, 0.0); }
            KeyCode::ArrowUp => { let s = if self.shift {10.0} else {1.0}; self.nudge(0.0, -s); }
            KeyCode::ArrowDown => { let s = if self.shift {10.0} else {1.0}; self.nudge(0.0, s); }
            _ => {}
        }
    }
    fn double_click(&mut self, pos: P) {
        if let Some(pid) = self.path_under(pos) {
            self.set_tool(Tool::Direct);
            self.selected.clear();
            if let Some(pi) = self.pidx(pid) { for a in &self.doc[pi].anchors { self.selected.insert(a.id); } }
        }
    }
}

// ===================== shapes =====================
fn build_shape(app: &mut App, kind: Tool, a: P, b: P) -> Vec<Anchor> {
    let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
    let (x1, y1) = (a[0].max(b[0]), a[1].max(b[1]));
    let (cx, cy, rx, ry) = ((x0+x1)/2.0, (y0+y1)/2.0, (x1-x0)/2.0, (y1-y0)/2.0);
    let mut mk = |p: P, hin: Option<P>, hout: Option<P>, smooth: bool| Anchor { id: app.nid(), p, hin, hout, smooth };
    match kind {
        Tool::Rect => vec![mk([x0,y0],None,None,false), mk([x1,y0],None,None,false), mk([x1,y1],None,None,false), mk([x0,y1],None,None,false)],
        Tool::Triangle => vec![mk([cx,y0],None,None,false), mk([x1,y1],None,None,false), mk([x0,y1],None,None,false)],
        Tool::Polygon => {
            let n = 6; let mut out = vec![];
            for i in 0..n { let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
                out.push(mk([cx + rx*ang.cos(), cy + ry*ang.sin()], None, None, false)); }
            out
        }
        _ => { // ellipse
            let (kx, ky) = (K*rx, K*ry);
            vec![
                mk([cx,y0], Some([cx-kx,y0]), Some([cx+kx,y0]), true),
                mk([x1,cy], Some([x1,cy-ky]), Some([x1,cy+ky]), true),
                mk([cx,y1], Some([cx+kx,y1]), Some([cx-kx,y1]), true),
                mk([x0,cy], Some([x0,cy+ky]), Some([x0,cy-ky]), true),
            ]
        }
    }
}
fn build_shape_mod(app: &mut App, kind: Tool, start: P, cur: P, shift: bool, alt: bool) -> Vec<Anchor> {
    let mut dx = cur[0]-start[0]; let mut dy = cur[1]-start[1];
    if shift { let s = dx.abs().max(dy.abs()); dx = if dx<0.0 {-s} else {s}; dy = if dy<0.0 {-s} else {s}; }
    let (a, b) = if alt { ([start[0]-dx, start[1]-dy], [start[0]+dx, start[1]+dy]) } else { (start, [start[0]+dx, start[1]+dy]) };
    build_shape(app, kind, a, b)
}
fn bbox(path: &Path) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for a in &path.anchors { for q in [Some(a.p), a.hin, a.hout].into_iter().flatten() {
        x0 = x0.min(q[0]); y0 = y0.min(q[1]); x1 = x1.max(q[0]); y1 = y1.max(q[1]); } }
    (x0, y0, x1, y1)
}

// ===================== geometry build -> triangles =====================
fn ndc(p: P, w: f32, h: f32) -> [f32; 2] { [p[0]/w*2.0 - 1.0, 1.0 - p[1]/h*2.0] }
fn tri(v: &mut Vec<Vertex>, a: P, b: P, c: P, col: [f32;4], w: f32, h: f32) {
    v.push(Vertex { pos: ndc(a,w,h), color: col }); v.push(Vertex { pos: ndc(b,w,h), color: col }); v.push(Vertex { pos: ndc(c,w,h), color: col });
}
fn quad(v: &mut Vec<Vertex>, p0: P, p1: P, p2: P, p3: P, col: [f32;4], w: f32, h: f32) { tri(v,p0,p1,p2,col,w,h); tri(v,p0,p2,p3,col,w,h); }
fn line(v: &mut Vec<Vertex>, a: P, b: P, width: f32, col: [f32;4], w: f32, h: f32) {
    let d = sub(b, a); let l = length(d).max(1e-3); let n = [-d[1]/l*width/2.0, d[0]/l*width/2.0];
    quad(v, add(a,n), add(b,n), sub(b,n), sub(a,n), col, w, h);
}
fn sq(v: &mut Vec<Vertex>, c: P, half: f32, col: [f32;4], w: f32, h: f32) {
    quad(v, [c[0]-half,c[1]-half],[c[0]+half,c[1]-half],[c[0]+half,c[1]+half],[c[0]-half,c[1]+half], col, w, h);
}
fn disc(v: &mut Vec<Vertex>, c: P, r: f32, col: [f32;4], w: f32, h: f32) {
    let segs = 18;
    for i in 0..segs { let a0 = i as f32/segs as f32*std::f32::consts::TAU; let a1 = (i+1) as f32/segs as f32*std::f32::consts::TAU;
        tri(v, c, [c[0]+a0.cos()*r, c[1]+a0.sin()*r], [c[0]+a1.cos()*r, c[1]+a1.sin()*r], col, w, h); }
}
// dashed cubic preview (pen rubber-band): even arc-length dashes (5 on / 4 off), independent of sampling
fn dashed_cubic(v: &mut Vec<Vertex>, p0: P, p1: P, p2: P, p3: P, width: f32, col: [f32;4], w: f32, h: f32) {
    let steps = 72;
    let mut pts: Vec<P> = Vec::with_capacity(steps + 1);
    for s in 0..=steps { pts.push(cubic(p0, p1, p2, p3, s as f32 / steps as f32)); }
    let (dash, gap) = (5.0f32, 4.0f32); let period = dash + gap;
    let mut acc = 0.0f32;
    for i in 0..pts.len() - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let seglen = dist(a, b); if seglen < 1e-4 { continue; }
        let dir = [(b[0] - a[0]) / seglen, (b[1] - a[1]) / seglen];
        let mut s = 0.0f32;
        while s < seglen {
            let phase = (acc + s) % period;
            if phase < dash {
                let e = (s + (dash - phase)).min(seglen);
                line(v, [a[0] + dir[0]*s, a[1] + dir[1]*s], [a[0] + dir[0]*e, a[1] + dir[1]*e], width, col, w, h);
                s = e;
            } else {
                s += period - phase;
            }
        }
        acc += seglen;
    }
}

// grid (drawn first / behind)
fn build_bg(w: f32, h: f32) -> Vec<Vertex> {
    let mut v = Vec::new();
    let step = 24.0; let mut gx = step;
    while gx < w { let mut gy = step; while gy < h { sq(&mut v, [gx, gy], 1.0, GRID, w, h); gy += step; } gx += step; }
    v
}
// per closed path: a triangle-fan (for the stencil pass) + a bbox cover quad (for the cover pass).
// even-odd stencil-then-cover fills ANY polygon correctly (concave / self-intersecting).
fn build_fills(app: &App, w: f32, h: f32) -> (Vec<Vertex>, Vec<((u32, u32), (u32, u32))>) {
    let mut v = Vec::new(); let mut ranges = Vec::new();
    for path in &app.doc {
        let a = &path.anchors;
        if !path.closed || a.len() < 3 { continue; }
        let n = a.len(); let mut poly: Vec<P> = vec![];
        for i in 0..n { let p = &a[i]; let q = &a[(i+1)%n];
            let (c1, c2) = (p.hout.unwrap_or(p.p), q.hin.unwrap_or(q.p));
            for s in 0..12 { poly.push(cubic(p.p, c1, c2, q.p, s as f32/12.0)); } }
        let fan_start = v.len() as u32;
        for i in 1..poly.len()-1 { tri(&mut v, poly[0], poly[i], poly[i+1], ACCENT_FILL, w, h); }
        let fan_len = v.len() as u32 - fan_start;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &poly { x0=x0.min(p[0]); y0=y0.min(p[1]); x1=x1.max(p[0]); y1=y1.max(p[1]); }
        let cov_start = v.len() as u32;
        quad(&mut v, [x0,y0],[x1,y0],[x1,y1],[x0,y1], ACCENT_FILL, w, h);
        let cov_len = v.len() as u32 - cov_start;
        ranges.push(((fan_start, fan_len), (cov_start, cov_len)));
    }
    (v, ranges)
}

fn build(app: &App, w: f32, h: f32) -> Vec<Vertex> {
    let mut v = Vec::new();
    // path strokes (grid + fills are drawn in separate passes)
    for path in &app.doc {
        let a = &path.anchors; if a.len() < 2 { continue; }
        let n = a.len(); let segs = if path.closed { n } else { n-1 };
        for i in 0..segs { let p = &a[i]; let q = &a[(i+1)%n];
            let (c1, c2) = (p.hout.unwrap_or(p.p), q.hin.unwrap_or(q.p));
            let steps = 24; let mut prev = p.p;
            for s in 1..=steps { let t = s as f32/steps as f32; let pt = cubic(p.p, c1, c2, q.p, t); line(&mut v, prev, pt, 2.0, ACCENT, w, h); prev = pt; }
        }
    }
    // object-selection bbox
    if app.tool == Tool::Object { for &pid in &app.objsel { if let Some(pi) = app.pidx(pid) { if !app.doc[pi].anchors.is_empty() {
        let b = bbox(&app.doc[pi]);
        for (s, e) in [([b.0,b.1],[b.2,b.1]), ([b.2,b.1],[b.2,b.3]), ([b.2,b.3],[b.0,b.3]), ([b.0,b.3],[b.0,b.1])] {
            line(&mut v, [s[0]-2.0,s[1]-2.0], [e[0]+2.0,e[1]+2.0], 1.0, HANDLE_COL, w, h);
        }
    }}}}
    // pen rubber-band: a DASHED, CURVED preview (leaves the last anchor along its out-handle, ends at the cursor)
    if app.tool == Tool::Pen { if let Some(ap) = app.active { if matches!(app.drag, Drag::None) {
        if let Some(pi) = app.pidx(ap) { if let Some(last) = app.doc[pi].anchors.last() {
            let c1 = last.hout.unwrap_or(last.p);
            dashed_cubic(&mut v, last.p, c1, app.cursor, app.cursor, 1.5, ACCENT, w, h);
        } }
    }}}
    // marquee
    if let Drag::Marquee { start, .. } = &app.drag {
        let (x0, y0) = (start[0].min(app.cursor[0]), start[1].min(app.cursor[1]));
        let (x1, y1) = (start[0].max(app.cursor[0]), start[1].max(app.cursor[1]));
        for (s, e) in [([x0,y0],[x1,y0]), ([x1,y0],[x1,y1]), ([x1,y1],[x0,y1]), ([x0,y1],[x0,y0])] { line(&mut v, s, e, 1.0, ACCENT, w, h); }
    }
    // which handle sides to show: selected anchors + their neighbours, plus active pen path
    let mut show_anchor: HashSet<u32> = HashSet::new();   // anchors whose handles to draw
    for path in &app.doc {
        let n = path.anchors.len();
        for (ai, a) in path.anchors.iter().enumerate() {
            if app.selected.contains(&a.id) {
                show_anchor.insert(a.id);
                if ai > 0 { show_anchor.insert(path.anchors[ai-1].id); } else if path.closed { show_anchor.insert(path.anchors[n-1].id); }
                if ai < n-1 { show_anchor.insert(path.anchors[ai+1].id); } else if path.closed { show_anchor.insert(path.anchors[0].id); }
            }
        }
    }
    if app.tool == Tool::Pen { if let Some(ap) = app.active { if let Some(pi) = app.pidx(ap) { for a in &app.doc[pi].anchors { show_anchor.insert(a.id); } } } }
    // draw handles
    for path in &app.doc { for a in &path.anchors { if show_anchor.contains(&a.id) {
        for hp in [a.hin, a.hout].into_iter().flatten() { line(&mut v, a.p, hp, 1.0, HANDLE_COL, w, h); }
    }}}
    for path in &app.doc { for a in &path.anchors { if show_anchor.contains(&a.id) {
        for hp in [a.hin, a.hout].into_iter().flatten() { disc(&mut v, hp, 4.0, HANDLE_COL, w, h); }
    }}}
    // anchor markers (only for shown paths)
    for path in &app.doc {
        if !app.path_shown(path.id) || app.tool == Tool::Object { continue; }
        for a in &path.anchors {
            let sel = app.selected.contains(&a.id);
            if a.smooth {
                if sel { disc(&mut v, a.p, 5.0, ACCENT, w, h); } else { disc(&mut v, a.p, 5.5, ACCENT, w, h); disc(&mut v, a.p, 4.0, WHITE, w, h); }
            } else if sel { sq(&mut v, a.p, 4.5, ACCENT, w, h); } else { sq(&mut v, a.p, 5.0, ACCENT, w, h); sq(&mut v, a.p, 3.6, WHITE, w, h); }
        }
    }
    // toolbar
    for (i, t) in TOOLBAR.iter().enumerate() {
        let bx = 10.0 + i as f32 * 42.0; let by = 10.0; let active = app.tool == *t;
        sq(&mut v, [bx+17.0, by+17.0], 17.0, if active { ACCENT } else { BTN_BG }, w, h);
        let ic = if active { WHITE } else { ACCENT };
        draw_icon(&mut v, *t, bx, by, ic, if active {ACCENT} else {BTN_BG}, w, h);
    }
    v
}

const TOOLBAR: [Tool; 6] = [Tool::Object, Tool::Direct, Tool::Pen, Tool::Rect, Tool::Ellipse, Tool::Triangle];
fn draw_icon(v: &mut Vec<Vertex>, t: Tool, bx: f32, by: f32, ic: [f32;4], bg: [f32;4], w: f32, h: f32) {
    match t {
        Tool::Object => tri(v, [bx+11.0,by+9.0],[bx+11.0,by+25.0],[bx+24.0,by+19.0], ic, w, h),
        Tool::Direct => { tri(v, [bx+11.0,by+9.0],[bx+11.0,by+25.0],[bx+24.0,by+19.0], ic, w, h); tri(v, [bx+12.5,by+12.0],[bx+12.5,by+22.0],[bx+20.0,by+18.0], bg, w, h); }
        Tool::Pen => { line(v, [bx+10.0,by+25.0],[bx+24.0,by+11.0], 2.5, ic, w, h); sq(v, [bx+11.0,by+24.0], 2.0, ic, w, h); }
        Tool::Rect => { sq(v, [bx+17.0,by+17.0], 8.0, ic, w, h); sq(v, [bx+17.0,by+17.0], 5.5, bg, w, h); }
        Tool::Ellipse => { disc(v, [bx+17.0,by+17.0], 8.0, ic, w, h); disc(v, [bx+17.0,by+17.0], 5.5, bg, w, h); }
        Tool::Triangle => { tri(v, [bx+17.0,by+9.0],[bx+25.0,by+25.0],[bx+9.0,by+25.0], ic, w, h); tri(v, [bx+17.0,by+13.0],[bx+22.0,by+23.0],[bx+12.0,by+23.0], bg, w, h); }
        _ => {}
    }
}
fn button_hit(pos: P) -> Option<Tool> {
    for (i, t) in TOOLBAR.iter().enumerate() {
        let bx = 10.0 + i as f32 * 42.0;
        if pos[0] >= bx && pos[0] <= bx+34.0 && pos[1] >= 10.0 && pos[1] <= 44.0 { return Some(*t); }
    }
    None
}

// ===================== wgpu =====================
const DS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const VATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
    wgpu::VertexAttribute { offset: 8, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
];

struct Gpu {
    window: Arc<Window>, surface: wgpu::Surface<'static>, device: wgpu::Device, queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipe_main: wgpu::RenderPipeline, pipe_stencil: wgpu::RenderPipeline, pipe_cover: wgpu::RenderPipeline,
    msaa: wgpu::TextureView, ds: wgpu::TextureView, samples: u32,
    bg_buf: wgpu::Buffer, bg_cap: u64, fill_buf: wgpu::Buffer, fill_cap: u64, fg_buf: wgpu::Buffer, fg_cap: u64,
}

fn make_attach(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, samples: u32, format: wgpu::TextureFormat, label: &str) -> wgpu::TextureView {
    device.create_texture(&wgpu::TextureDescriptor { label: Some(label),
        size: wgpu::Extent3d { width: config.width.max(1), height: config.height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: samples, dimension: wgpu::TextureDimension::D2,
        format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[] }).create_view(&Default::default())
}
fn make_pipe(device: &wgpu::Device, layout: &wgpu::PipelineLayout, shader: &wgpu::ShaderModule,
             format: wgpu::TextureFormat, samples: u32, color: bool, stencil: wgpu::StencilState) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None, layout: Some(layout),
        vertex: wgpu::VertexState { module: shader, entry_point: "vs", buffers: &[wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VATTRS }] },
        fragment: Some(wgpu::FragmentState { module: shader, entry_point: "fs", targets: &[Some(wgpu::ColorTargetState {
            format, blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: if color { wgpu::ColorWrites::ALL } else { wgpu::ColorWrites::empty() } })] }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState { format: DS_FORMAT, depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always, stencil, bias: wgpu::DepthBiasState::default() }),
        multisample: wgpu::MultisampleState { count: samples, ..Default::default() }, multiview: None,
    })
}

impl Gpu {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..Default::default() });
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false,
        }).await.expect("no GPU adapter");
        eprintln!("[varos] adapter: {:?} | backend: {:?}", adapter.get_info().name, adapter.get_info().backend);
        let samples = 4u32;
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: None, required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::downlevel_defaults(),
        }, None).await.expect("no device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format, width: size.width.max(1), height: size.height.max(1),
            present_mode: if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) { wgpu::PresentMode::Mailbox }
                          else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) { wgpu::PresentMode::Immediate }
                          else { wgpu::PresentMode::Fifo },
            alpha_mode: caps.alpha_modes[0], view_formats: vec![], desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);
        eprintln!("[varos] present mode: {:?} | format: {:?} | samples: {}", config.present_mode, config.format, samples);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[], push_constant_ranges: &[] });
        let inv = wgpu::StencilFaceState { compare: wgpu::CompareFunction::Always, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::Invert };
        let cov = wgpu::StencilFaceState { compare: wgpu::CompareFunction::NotEqual, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::Zero };
        let st_fan = wgpu::StencilState { front: inv, back: inv, read_mask: 0xff, write_mask: 0xff };
        let st_cov = wgpu::StencilState { front: cov, back: cov, read_mask: 0xff, write_mask: 0xff };
        let pipe_main = make_pipe(&device, &layout, &shader, config.format, samples, true, wgpu::StencilState::default());
        let pipe_stencil = make_pipe(&device, &layout, &shader, config.format, samples, false, st_fan);
        let pipe_cover = make_pipe(&device, &layout, &shader, config.format, samples, true, st_cov);
        let msaa = make_attach(&device, &config, samples, config.format, "msaa");
        let ds = make_attach(&device, &config, samples, DS_FORMAT, "ds");
        let mkbuf = |cap: u64| device.create_buffer(&wgpu::BufferDescriptor { label: Some("v"), size: cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let (bg_cap, fill_cap, fg_cap) = (1u64 << 18, 1u64 << 21, 1u64 << 22);
        let (bg_buf, fill_buf, fg_buf) = (mkbuf(bg_cap), mkbuf(fill_cap), mkbuf(fg_cap));
        Gpu { window, surface, device, queue, config, pipe_main, pipe_stencil, pipe_cover, msaa, ds, samples,
              bg_buf, bg_cap, fill_buf, fill_cap, fg_buf, fg_cap }
    }
    fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w; self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            self.msaa = make_attach(&self.device, &self.config, self.samples, self.config.format, "msaa");
            self.ds = make_attach(&self.device, &self.config, self.samples, DS_FORMAT, "ds");
        }
    }
    fn upload(device: &wgpu::Device, queue: &wgpu::Queue, buf: &mut wgpu::Buffer, cap: &mut u64, verts: &[Vertex]) -> u32 {
        let bytes: &[u8] = bytemuck::cast_slice(verts);
        if bytes.len() as u64 > *cap {
            *cap = (bytes.len() as u64).next_power_of_two().max(1024);
            *buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("v"), size: *cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !bytes.is_empty() { queue.write_buffer(buf, 0, bytes); }
        verts.len() as u32
    }
    fn render(&mut self, app: &App) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => { self.surface.configure(&self.device, &self.config); return; }
            Err(_) => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let (fw, fh) = (self.config.width as f32, self.config.height as f32);
        let bg = build_bg(fw, fh);
        let (fillv, franges) = build_fills(app, fw, fh);
        let fg = build(app, fw, fh);
        let nbg = Self::upload(&self.device, &self.queue, &mut self.bg_buf, &mut self.bg_cap, &bg);
        let nfill = Self::upload(&self.device, &self.queue, &mut self.fill_buf, &mut self.fill_cap, &fillv);
        let nfg = Self::upload(&self.device, &self.queue, &mut self.fg_buf, &mut self.fg_cap, &fg);
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.msaa, resolve_target: Some(&view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: BG[0] as f64, g: BG[1] as f64, b: BG[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.ds,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0), store: wgpu::StoreOp::Discard }) }),
                timestamp_writes: None, occlusion_query_set: None });
            rp.set_stencil_reference(0);
            if nbg > 0 { rp.set_pipeline(&self.pipe_main); rp.set_vertex_buffer(0, self.bg_buf.slice(..)); rp.draw(0..nbg, 0..1); }
            if nfill > 0 {
                rp.set_vertex_buffer(0, self.fill_buf.slice(..));
                for ((fs, fl), (cs, cl)) in &franges {
                    rp.set_pipeline(&self.pipe_stencil); rp.draw(*fs..*fs + *fl, 0..1);
                    rp.set_pipeline(&self.pipe_cover);   rp.draw(*cs..*cs + *cl, 0..1);
                }
            }
            if nfg > 0 { rp.set_pipeline(&self.pipe_main); rp.set_vertex_buffer(0, self.fg_buf.slice(..)); rp.draw(0..nfg, 0..1); }
        }
        self.queue.submit(Some(enc.finish()));
        frame.present();
    }
}

fn title_for(t: Tool) -> &'static str {
    match t { Tool::Pen=>"Varos — Pen (P)", Tool::Direct=>"Varos — White arrow (A)", Tool::Object=>"Varos — Black arrow (V)",
              Tool::Rect=>"Varos — Rectangle (M)", Tool::Ellipse=>"Varos — Ellipse (L)", Tool::Triangle=>"Varos — Triangle", Tool::Polygon=>"Varos — Polygon", Tool::Convert=>"Varos — Convert" }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().with_title("Varos — Pen (P)")
        .with_inner_size(winit::dpi::LogicalSize::new(1180.0, 800.0)).build(&event_loop).unwrap());
    let mut gpu = pollster::block_on(Gpu::new(window.clone()));
    let mut app = App::new();
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, window_id } = event {
            if window_id != gpu.window.id() { return; }
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => { gpu.resize(size.width, size.height); gpu.window.request_redraw(); }
                WindowEvent::CursorMoved { position, .. } => { let PhysicalPosition { x, y } = position; app.moved([x as f32, y as f32]); gpu.window.request_redraw(); }
                WindowEvent::ModifiersChanged(m) => { app.shift = m.state().shift_key(); app.alt = m.state().alt_key(); app.ctrl = m.state().control_key() || m.state().super_key(); }
                WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                    match state {
                        ElementState::Pressed => {
                            // double-click?
                            let now = Instant::now();
                            let dbl = app.last_click.map_or(false, |(t, p)| now.duration_since(t).as_millis() < 350 && dist(p, app.cursor) < 6.0);
                            app.last_click = Some((now, app.cursor));
                            if let Some(t) = button_hit(app.cursor) { app.set_tool(t); }
                            else if dbl && (app.tool == Tool::Object || app.tool == Tool::Direct) { app.double_click(app.cursor); }
                            else { app.down(app.cursor); }
                        }
                        ElementState::Released => app.up(),
                    }
                    gpu.window.set_title(title_for(app.tool));
                    gpu.window.request_redraw();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Pressed {
                        if let PhysicalKey::Code(code) = event.physical_key {
                            if app.ctrl && code == KeyCode::KeyZ { if app.shift { app.redo(); } else { app.undo(); } }
                            else if app.ctrl && code == KeyCode::KeyY { app.redo(); }
                            else if !app.ctrl { app.key(code); }
                            gpu.window.set_title(title_for(app.tool));
                            gpu.window.request_redraw();
                        }
                    }
                }
                WindowEvent::RedrawRequested => gpu.render(&app),
                _ => {}
            }
        }
    }).unwrap();
}
