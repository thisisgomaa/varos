//! The editor: transient interaction state + shared operations + the drag/undo engine.
//! Tools (see `tools/`) define what a *press* does; the shared move/up engine handles the drag.

use std::collections::HashSet;
use crate::geom::*;
use crate::model::*;
use crate::tools;
use crate::boolean::{run_boolean_curves, BoolOp, ResultShape, Seg};

pub const DRAG_THRESH: f32 = 4.0;
pub const CLOSE_R: f32 = 11.0;
pub const ANCHOR_R: f32 = 12.0;
pub const HANDLE_R: f32 = 11.0;
pub const EDGE_R: f32 = 8.0;
pub const HANDLE_LEN: f32 = 45.0;

#[derive(Clone, Copy, Default)]
pub struct Mods { pub shift: bool, pub alt: bool, pub ctrl: bool }

#[derive(Clone, Copy, PartialEq)]
pub enum PaintTarget { Fill, Stroke }

#[derive(Clone, Copy, PartialEq)]
pub enum ToolKind { Object, Direct, Pen, Rect, Ellipse, Triangle, Polygon, Convert, Eyedropper }

/// What the Pen tool would do at the cursor right now — drives the contextual pen cursor (Illustrator
/// shows pen+×/+/−/○/continue). Computed by `Editor::pen_hint`, mirrors `tools::pen` down logic.
#[derive(Clone, Copy, PartialEq)]
pub enum PenHint { New, Draw, Add, Delete, Close, Connect }
impl ToolKind {
    pub fn is_shape(self) -> bool { matches!(self, ToolKind::Rect | ToolKind::Ellipse | ToolKind::Triangle | ToolKind::Polygon) }
    pub fn shape(self) -> ShapeKind {
        match self { ToolKind::Ellipse => ShapeKind::Ellipse, ToolKind::Triangle => ShapeKind::Triangle,
                     ToolKind::Polygon => ShapeKind::Polygon, _ => ShapeKind::Rect }
    }
}

pub enum Drag {
    None,
    PenNew { aid: u32, down: Pt, broken: bool },
    PenClose { aid: u32, down: Pt, broken: bool },
    Anchors { start: Pt, items: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    Handle { aid: u32, out: bool, couple: bool, opp_len: f32, grab: Pt },
    Segment { pid: u32, i: usize, down: Pt, a_out0: Option<Pt>, b_in0: Option<Pt>, ap0: Pt, bp0: Pt, straight: bool },
    Shape { start: Pt, pid: u32, kind: ShapeKind },
    Marquee { start: Pt, base: Vec<u32> },
    ObjMarquee { start: Pt, base: Vec<u32> },
    DupPending { srcs: Vec<u32>, down: Pt, object: bool },
    Object { down: Pt, base: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    // scale works in the frame's LOCAL (un-rotated) space; opp_l/cen_l/h0_l are local handle coords
    Scale { handle: u8, angle: f32, opp_l: Pt, cen_l: Pt, h0_l: Pt, base: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    Rotate { center: Pt, start: f32, a0: f32, base: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    ConvPull { aid: u32, down: Pt },
}

/// What a press on the object-selection bounding box hit.
#[derive(Clone, Copy)]
pub enum TfHit { Scale(u8), Rotate(u8) } // u8 = handle index 0..7 (corners 0-3, edge mids 4-7)

#[derive(Clone, Copy)]
pub enum ZOrder { Front, Forward, Backward, Back }
#[derive(Clone, Copy)]
pub enum AlignMode { Left, CenterH, Right, Top, Middle, Bottom }
#[derive(Clone, Copy)]
pub enum DistAxis { Horizontal, Vertical }

/// Build an anchor from a boolean-result endpoint + its two raw handle points.
/// Handles coincident with the point → None (straight). Smooth when both handles are collinear & opposite.
fn make_anchor(id: u32, p: Pt, hin_raw: Pt, hout_raw: Pt) -> Anchor {
    let din = sub(hin_raw, p); let dout = sub(hout_raw, p);
    let (lin, lout) = (length(din), length(dout));
    let deg = 1e-3;
    let hin = if lin > deg { Some(hin_raw) } else { None };
    let hout = if lout > deg { Some(hout_raw) } else { None };
    let smooth = matches!((hin, hout), (Some(_), Some(_)))
        && (din[0]*dout[1] - din[1]*dout[0]).abs() <= 1e-3 * lin * lout   // ~collinear
        && (din[0]*dout[0] + din[1]*dout[1]) < 0.0;                       // pointing apart (mirror handles)
    Anchor { id, p, hin, hout, smooth }
}
/// A closed anchor contour → its cubic segments (start, c1, c2, end) for the boolean engine.
fn contour_segs(anchors: &[Anchor]) -> Vec<Seg> {
    let n = anchors.len();
    if n < 3 { return vec![]; }
    (0..n).map(|i| { let x = &anchors[i]; let y = &anchors[(i+1) % n]; (x.p, x.hout.unwrap_or(x.p), y.hin.unwrap_or(y.p), y.p) }).collect()
}

pub struct Editor {
    pub doc: Document,
    pub tool: ToolKind,
    pub gesture: ToolKind,
    pub active: Option<u32>,
    pub selected: HashSet<u32>,
    pub objsel: HashSet<u32>,
    pub dsel_path: Option<u32>, // direct-mode path-level selection: anchors shown hollow, whole-path moves

    pub drag: Drag,
    pub cursor: Pt,
    pub ppu: f32,            // pixels-per-unit (view zoom) — so grab tolerances stay constant on screen
    pub obj_angle: f32,      // orientation of the object-selection transform frame (rotates with the selection)
    pub hover_path: Option<u32>,
    pub mods: Mods,
    pub cur_fill: Option<Rgba>, pub cur_stroke: Option<Rgba>, pub cur_sw: f32, pub paint: PaintTarget,
    pub dirty: bool,
    undo: Vec<Document>, redo: Vec<Document>, pending: Option<Document>,
}

impl Editor {
    pub fn new() -> Self {
        Editor { doc: Document::default(), tool: ToolKind::Pen, gesture: ToolKind::Pen, active: None,
                 selected: HashSet::new(), objsel: HashSet::new(), dsel_path: None, drag: Drag::None, cursor: [0.0, 0.0],
                 ppu: 1.0, obj_angle: 0.0, hover_path: None, mods: Mods::default(),
                 cur_fill: Some([0.95, 0.95, 0.96, 1.0]), cur_stroke: Some([0.12, 0.12, 0.13, 1.0]), cur_sw: 2.0, paint: PaintTarget::Fill,
                 dirty: false, undo: vec![], redo: vec![], pending: None }
    }

    // ---------- selection-aware queries ----------
    pub fn is_editable(&self, pid: u32) -> bool {
        self.active == Some(pid) || self.objsel.contains(&pid)
            || self.doc.paths.iter().find(|p| p.id == pid).map_or(false, |p| p.anchors.iter().chain(p.holes.iter().flatten()).any(|a| self.selected.contains(&a.id)))
    }
    pub fn path_shown(&self, pid: u32) -> bool {
        self.active == Some(pid) || self.hover_path == Some(pid) || self.objsel.contains(&pid) || self.dsel_path == Some(pid)
            || self.doc.paths.iter().find(|p| p.id == pid).map_or(false, |p| p.anchors.iter().chain(p.holes.iter().flatten()).any(|a| self.selected.contains(&a.id)))
    }
    pub fn nearest_anchor(&self, pos: Pt, r: f32, shown_only: bool) -> Option<u32> {
        let r = r / self.ppu;
        let mut best: Option<(u32, f32)> = None;
        for p in &self.doc.paths {
            if shown_only && !self.path_shown(p.id) { continue; }
            for a in p.anchors.iter().chain(p.holes.iter().flatten()) {   // outer + hole anchors
                let d = dist(pos, a.p);
                if d <= r && best.map_or(true, |(_, bd)| d < bd) { best = Some((a.id, d)); }
            }
        }
        best.map(|(id, _)| id)
    }
    pub fn path_under(&self, pos: Pt) -> Option<u32> {
        let edge_r = EDGE_R / self.ppu;
        let mut best: Option<(u32, f32)> = None;
        for pi in 0..self.doc.paths.len() {
            if let Some((_, _, d)) = self.doc.nearest_seg(pi, pos) {
                if d <= edge_r && best.map_or(true, |(_, bd)| d < bd) { best = Some((self.doc.paths[pi].id, d)); }
            }
        }
        if let Some((id, _)) = best { return Some(id); }
        for pi in 0..self.doc.paths.len() { if self.doc.point_in_path(pi, pos) { return Some(self.doc.paths[pi].id); } }
        None
    }
    pub fn handle_hit(&self, pos: Pt) -> Option<u32> {
        let hr = HANDLE_R / self.ppu;
        for &aid in &self.selected {
            if let Some(a) = self.doc.anchor(aid) {
                for h in [a.hin, a.hout].into_iter().flatten() { if dist(pos, h) <= hr { return Some(aid); } }
            }
        }
        if let Some(ap) = self.active { if let Some(pi) = self.doc.pidx(ap) {
            for a in &self.doc.paths[pi].anchors { for h in [a.hin, a.hout].into_iter().flatten() { if dist(pos, h) <= hr { return Some(a.id); } } }
        }}
        None
    }
    pub fn which_handle(&self, aid: u32, pos: Pt) -> bool {
        let a = self.doc.anchor(aid).unwrap();
        let dout = a.hout.map_or(f32::MAX, |h| dist(pos, h));
        let din = a.hin.map_or(f32::MAX, |h| dist(pos, h));
        dout <= din
    }
    pub fn tangent(&self, pi: usize, ai: usize) -> Pt {
        let p = &self.doc.paths[pi]; let n = p.anchors.len();
        let prev = if ai > 0 { Some(&p.anchors[ai-1]) } else if p.closed { Some(&p.anchors[n-1]) } else { None };
        let next = if ai < n-1 { Some(&p.anchors[ai+1]) } else if p.closed { Some(&p.anchors[0]) } else { None };
        match (prev, next) {
            (Some(a), Some(b)) => norm(sub(b.p, a.p)),
            (None, Some(b)) => norm(sub(b.p, p.anchors[ai].p)),
            (Some(a), None) => norm(sub(p.anchors[ai].p, a.p)),
            _ => [1.0, 0.0],
        }
    }

    // ---------- object-selection transform frame (rotates with the selection) ----------
    /// Axis-aligned bbox of the object selection (used to refit a fresh, un-rotated frame).
    pub fn obj_bbox(&self) -> Option<(f32, f32, f32, f32)> {
        if self.objsel.is_empty() { return None; }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &pid in &self.objsel { if let Some(pi) = self.doc.pidx(pid) {
            for q in self.doc.outline(pi, 8) { x0 = x0.min(q[0]); y0 = y0.min(q[1]); x1 = x1.max(q[0]); y1 = y1.max(q[1]); }
        }}
        if x0 <= x1 { Some((x0, y0, x1, y1)) } else { None }
    }
    /// Selection bbox in the frame's LOCAL space (outline un-rotated by obj_angle about origin).
    pub fn obj_local_bbox(&self) -> Option<(f32, f32, f32, f32)> {
        if self.objsel.is_empty() { return None; }
        let th = -self.obj_angle;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &pid in &self.objsel { if let Some(pi) = self.doc.pidx(pid) {
            for q in self.doc.outline(pi, 8) { let r = rotate_about(q, [0.0, 0.0], th); x0 = x0.min(r[0]); y0 = y0.min(r[1]); x1 = x1.max(r[0]); y1 = y1.max(r[1]); }
        }}
        if x0 <= x1 { Some((x0, y0, x1, y1)) } else { None }
    }
    /// 8 handle positions in local space: corners 0-3 (TL,TR,BR,BL), edge mids 4-7 (T,R,B,L).
    pub fn bbox_handles(bb: (f32, f32, f32, f32)) -> [Pt; 8] {
        let (x0, y0, x1, y1) = bb; let (mx, my) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        [[x0,y0],[x1,y0],[x1,y1],[x0,y1],[mx,y0],[x1,my],[mx,y1],[x0,my]]
    }
    /// The 8 handles in WORLD space (rotated by obj_angle).
    pub fn frame_handles(&self) -> Option<[Pt; 8]> {
        let l = Self::bbox_handles(self.obj_local_bbox()?);
        let th = self.obj_angle; let mut o = [[0.0f32; 2]; 8];
        for i in 0..8 { o[i] = rotate_about(l[i], [0.0, 0.0], th); }
        Some(o)
    }
    /// The 4 frame corners in WORLD space (for drawing the oriented rectangle).
    pub fn frame_corners(&self) -> Option<[Pt; 4]> {
        let (x0, y0, x1, y1) = self.obj_local_bbox()?;
        let th = self.obj_angle;
        Some([rotate_about([x0,y0],[0.0,0.0],th), rotate_about([x1,y0],[0.0,0.0],th),
              rotate_about([x1,y1],[0.0,0.0],th), rotate_about([x0,y1],[0.0,0.0],th)])
    }
    fn opposite(i: u8) -> u8 { match i { 0=>2, 1=>3, 2=>0, 3=>1, 4=>6, 5=>7, 6=>4, 7=>5, _=>0 } }
    fn objsel_base(&self) -> Vec<(u32, Pt, Option<Pt>, Option<Pt>)> {
        let mut base = vec![];
        for &pid in &self.objsel { if let Some(pi) = self.doc.pidx(pid) {
            for a in self.doc.paths[pi].anchors.iter().chain(self.doc.paths[pi].holes.iter().flatten()) { base.push((a.id, a.p, a.hin, a.hout)); }
        } }
        base
    }
    /// Does a path touch / fall inside a marquee rect? (vertex inside, or rect-centre inside a closed path)
    pub fn path_in_rect(&self, pi: usize, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
        let poly = self.doc.outline(pi, 16);
        if poly.is_empty() { return false; }
        if poly.iter().any(|p| p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1) { return true; }
        self.doc.point_in_path(pi, [(x0 + x1) * 0.5, (y0 + y1) * 0.5])
    }
    /// Did a press land on a transform handle (scale) or a corner's rotate ring (just outside)?
    pub fn transform_hit(&self, pos: Pt) -> Option<TfHit> {
        if self.tool != ToolKind::Object { return None; }
        let hs = self.frame_handles()?;
        let r = 7.0 / self.ppu;
        let mut best: Option<(u8, f32)> = None;
        for (i, h) in hs.iter().enumerate() { let d = dist(pos, *h); if d <= r && best.map_or(true, |(_, bd)| d < bd) { best = Some((i as u8, d)); } }
        if let Some((i, _)) = best { return Some(TfHit::Scale(i)); }
        // rotate: near a corner, OUTSIDE the (rotated) frame, AND over empty space — so clicking
        // (or shift-clicking) another nearby object selects it instead of rotating this one.
        let bb = self.obj_local_bbox()?;
        let lp = rotate_about(pos, [0.0, 0.0], -self.obj_angle);
        if (lp[0] < bb.0 || lp[0] > bb.2 || lp[1] < bb.1 || lp[1] > bb.3) && self.path_under(pos).is_none() {
            let ring = 22.0 / self.ppu;
            for i in 0..4u8 { if dist(pos, hs[i as usize]) <= ring { return Some(TfHit::Rotate(i)); } }
        }
        None
    }
    pub fn start_transform(&mut self, hit: TfHit, pos: Pt) {
        let bb = match self.obj_local_bbox() { Some(b) => b, None => return };
        let cen_l = [(bb.0 + bb.2) * 0.5, (bb.1 + bb.3) * 0.5];
        let base = self.objsel_base();
        match hit {
            TfHit::Scale(i) => {
                let l = Self::bbox_handles(bb);
                self.drag = Drag::Scale { handle: i, angle: self.obj_angle, opp_l: l[Self::opposite(i) as usize], cen_l, h0_l: l[i as usize], base };
            }
            TfHit::Rotate(_) => {
                let center = rotate_about(cen_l, [0.0, 0.0], self.obj_angle);
                let start = (pos[1] - center[1]).atan2(pos[0] - center[0]);
                self.drag = Drag::Rotate { center, start, a0: self.obj_angle, base };
            }
        }
    }
    fn translate_path(&mut self, pi: usize, d: Pt) {
        for a in &mut self.doc.paths[pi].anchors { a.p = add(a.p, d); a.hin = a.hin.map(|h| add(h, d)); a.hout = a.hout.map(|h| add(h, d)); }
        for h in &mut self.doc.paths[pi].holes { for a in h { a.p = add(a.p, d); a.hin = a.hin.map(|x| add(x, d)); a.hout = a.hout.map(|x| add(x, d)); } }
    }

    // ---------- Pathfinder (boolean ops) ----------
    /// Build one path as cubic contours (outer + hole contours) for the boolean engine.
    fn path_to_segs(&self, pi: usize) -> Vec<Vec<Seg>> {
        let p = &self.doc.paths[pi];
        let mut shape: Vec<Vec<Seg>> = vec![];
        let outer = contour_segs(&p.anchors); if !outer.is_empty() { shape.push(outer); }
        for hole in &p.holes { let c = contour_segs(hole); if !c.is_empty() { shape.push(c); } }
        shape
    }
    fn segs_to_anchors(&mut self, segs: &[Seg]) -> Vec<Anchor> {
        let m = segs.len();
        let mut out = Vec::with_capacity(m);
        for i in 0..m {
            let p = segs[i].3;                 // anchor = this segment's endpoint
            let hin_raw = segs[i].2;           // hin  = this segment's 2nd control point
            let hout_raw = segs[(i+1)%m].1;    // hout = next segment's 1st control point
            let id = self.doc.nid();
            out.push(make_anchor(id, p, hin_raw, hout_raw));
        }
        out
    }
    pub fn pathfinder(&mut self, op: BoolOp) {
        if self.objsel.len() < 2 { return; }
        // participants: closed paths with area, in document (z) order — bottom→top
        let sel: Vec<usize> = (0..self.doc.paths.len())
            .filter(|&pi| self.objsel.contains(&self.doc.paths[pi].id) && self.doc.paths[pi].closed && self.doc.paths[pi].anchors.len() >= 3)
            .collect();
        if sel.len() < 2 { return; }
        let bot = &self.doc.paths[sel[0]];
        let (fill, stroke, sw) = (bot.fill, bot.stroke, bot.stroke_width); // result inherits bottom-most paint
        let shapes: Vec<Vec<Vec<Seg>>> = sel.iter().map(|&pi| self.path_to_segs(pi)).filter(|s| !s.is_empty()).collect();
        if shapes.len() < 2 { return; }
        let result: Vec<ResultShape> = run_boolean_curves(op, &shapes);
        self.begin();
        let del: HashSet<u32> = sel.iter().map(|&pi| self.doc.paths[pi].id).collect();
        self.doc.paths.retain(|p| !del.contains(&p.id));
        let mut new_ids = vec![];
        for rs in &result {
            if rs.outer.len() < 2 { continue; }
            let anchors = self.segs_to_anchors(&rs.outer);
            if anchors.len() < 3 { continue; }
            let mut holes: Vec<Vec<Anchor>> = vec![];   // holes are editable anchor contours now
            for h in &rs.holes { let hc = self.segs_to_anchors(h); if hc.len() >= 3 { holes.push(hc); } }
            let id = self.doc.nid();
            self.doc.paths.push(Path { id, anchors, closed: true, fill, stroke, stroke_width: sw, holes });
            new_ids.push(id);
        }
        self.objsel = new_ids.into_iter().collect();
        self.tool = ToolKind::Object;        // land in the selection tool with the result framed & ready to move
        self.selected.clear(); self.active = None; self.dsel_path = None; self.obj_angle = 0.0;
        self.dirty = true; self.commit();
    }

    // ---------- arrange (z-order) / align / distribute (operate on the object selection) ----------
    pub fn arrange(&mut self, op: ZOrder) {
        if self.objsel.is_empty() { return; }
        self.begin();
        let sel = self.objsel.clone();
        match op {
            ZOrder::Front => { let (s, r): (Vec<_>, Vec<_>) = std::mem::take(&mut self.doc.paths).into_iter().partition(|p| sel.contains(&p.id)); self.doc.paths = r; self.doc.paths.extend(s); }
            ZOrder::Back  => { let (s, r): (Vec<_>, Vec<_>) = std::mem::take(&mut self.doc.paths).into_iter().partition(|p| sel.contains(&p.id)); self.doc.paths = s; self.doc.paths.extend(r); }
            ZOrder::Forward => { let n = self.doc.paths.len(); for i in (0..n.saturating_sub(1)).rev() {
                if sel.contains(&self.doc.paths[i].id) && !sel.contains(&self.doc.paths[i+1].id) { self.doc.paths.swap(i, i+1); } } }
            ZOrder::Backward => { let n = self.doc.paths.len(); for i in 1..n {
                if sel.contains(&self.doc.paths[i].id) && !sel.contains(&self.doc.paths[i-1].id) { self.doc.paths.swap(i, i-1); } } }
        }
        self.dirty = true; self.commit();
    }
    pub fn align(&mut self, mode: AlignMode) {
        if self.objsel.len() < 2 { return; }
        let (bx0, by0, bx1, by1) = match self.obj_bbox() { Some(b) => b, None => return };
        let pids: Vec<u32> = self.objsel.iter().copied().collect();
        self.begin();
        for pid in pids { if let Some(pi) = self.doc.pidx(pid) {
            let b = self.doc.outline_bbox(pi);
            let d = match mode {
                AlignMode::Left    => [bx0 - b.0, 0.0],
                AlignMode::Right   => [bx1 - b.2, 0.0],
                AlignMode::CenterH => [(bx0 + bx1) * 0.5 - (b.0 + b.2) * 0.5, 0.0],
                AlignMode::Top     => [0.0, by0 - b.1],
                AlignMode::Bottom  => [0.0, by1 - b.3],
                AlignMode::Middle  => [0.0, (by0 + by1) * 0.5 - (b.1 + b.3) * 0.5],
            };
            self.translate_path(pi, d);
        }}
        self.obj_angle = 0.0;
        self.dirty = true; self.commit();
    }
    pub fn distribute(&mut self, axis: DistAxis) {
        if self.objsel.len() < 3 { return; }
        let mut items: Vec<(u32, f32)> = self.objsel.iter().filter_map(|&pid| self.doc.pidx(pid).map(|pi| {
            let b = self.doc.outline_bbox(pi);
            (pid, match axis { DistAxis::Horizontal => (b.0 + b.2) * 0.5, DistAxis::Vertical => (b.1 + b.3) * 0.5 })
        })).collect();
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let n = items.len();
        let (c0, c1) = (items[0].1, items[n-1].1);
        let step = (c1 - c0) / (n as f32 - 1.0);
        self.begin();
        for k in 1..n-1 {
            let d = c0 + step * k as f32 - items[k].1;
            if let Some(pi) = self.doc.pidx(items[k].0) {
                self.translate_path(pi, match axis { DistAxis::Horizontal => [d, 0.0], DistAxis::Vertical => [0.0, d] });
            }
        }
        self.obj_angle = 0.0;
        self.dirty = true; self.commit();
    }

    // ---------- grouping (Ctrl+G / Ctrl+Shift+G) ----------
    pub fn group_selection(&mut self) {
        if self.objsel.len() < 2 { return; }
        self.begin();
        let pids: Vec<u32> = self.objsel.iter().copied().collect();
        if self.doc.group(&pids).is_some() { self.obj_angle = 0.0; self.dirty = true; }
        self.commit();
    }
    pub fn ungroup_selection(&mut self) {
        if self.objsel.is_empty() { return; }
        self.begin();
        let pids: Vec<u32> = self.objsel.iter().copied().collect();
        self.doc.ungroup(&pids);
        self.obj_angle = 0.0; self.dirty = true;
        self.commit();
    }

    // ---------- history ----------
    pub fn begin(&mut self) { self.pending = Some(self.doc.clone()); self.dirty = false; }
    pub fn commit(&mut self) {
        self.doc.sync_groups();   // drop membership for deleted paths / empty groups
        if self.dirty { if let Some(p) = self.pending.take() { self.undo.push(p); if self.undo.len() > 200 { self.undo.remove(0); } self.redo.clear(); } }
        self.pending = None; self.dirty = false;
    }
    pub fn undo(&mut self) { if let Some(s) = self.undo.pop() { self.redo.push(self.doc.clone()); self.doc = s; self.clear_transient(); } }
    pub fn redo(&mut self) { if let Some(s) = self.redo.pop() { self.undo.push(self.doc.clone()); self.doc = s; self.clear_transient(); } }
    fn clear_transient(&mut self) { self.selected.clear(); self.objsel.clear(); self.dsel_path = None; self.active = None; self.drag = Drag::None; }

    // ---------- shared mutating ops (used by tools) ----------
    pub fn reverse(&mut self, pi: usize) {
        self.doc.paths[pi].anchors.reverse();
        for a in &mut self.doc.paths[pi].anchors { std::mem::swap(&mut a.hin, &mut a.hout); }
    }
    pub fn delete_anchor(&mut self, aid: u32) {
        if let Some((pi, ai)) = self.doc.aidx(aid) {
            self.doc.paths[pi].anchors.remove(ai);
            self.selected.remove(&aid);
            if self.doc.paths[pi].anchors.len() < 2 { self.doc.paths[pi].closed = false; }
            if self.doc.paths[pi].anchors.is_empty() { let pid = self.doc.paths[pi].id; self.doc.paths.remove(pi); if self.active == Some(pid) { self.active = None; } }
        }
    }
    pub fn add_anchor(&mut self, pi: usize, i: usize, t: f32) -> u32 {
        let n = self.doc.paths[pi].anchors.len();
        let a = self.doc.paths[pi].anchors[i].clone();
        let b = self.doc.paths[pi].anchors[(i + 1) % n].clone();
        let nid = self.doc.nid();
        if a.hout.is_none() && b.hin.is_none() {
            let pt = cubic(a.p, a.p, b.p, b.p, t);
            self.doc.paths[pi].anchors.insert(i + 1, Anchor { id: nid, p: pt, hin: None, hout: None, smooth: false });
        } else {
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            let l = |u: Pt, v: Pt| [u[0] + (v[0]-u[0])*t, u[1] + (v[1]-u[1])*t];
            let (q0, q1, q2) = (l(p0, p1), l(p1, p2), l(p2, p3));
            let (r0, r1) = (l(q0, q1), l(q1, q2));
            let mid = l(r0, r1);
            self.doc.paths[pi].anchors[i].hout = Some(q0);
            let bi = (i + 1) % n;
            self.doc.paths[pi].anchors[bi].hin = Some(q2);
            self.doc.paths[pi].anchors.insert(i + 1, Anchor { id: nid, p: mid, hin: Some(r0), hout: Some(r1), smooth: true });
        }
        nid
    }
    pub fn toggle_type(&mut self, aid: u32) {
        let (pi, ai) = match self.doc.aidx(aid) { Some(x) => x, None => return }; // outer-ring only (hole convert deferred)
        if self.doc.paths[pi].anchors[ai].smooth {
            let a = &mut self.doc.paths[pi].anchors[ai]; a.smooth = false; a.hin = None; a.hout = None;
        } else {
            let dir = self.tangent(pi, ai);
            let p = self.doc.paths[pi].anchors[ai].p;
            let a = &mut self.doc.paths[pi].anchors[ai];
            a.smooth = true; a.hout = Some(add(p, scale(dir, HANDLE_LEN))); a.hin = Some(add(p, scale(dir, -HANDLE_LEN)));
        }
    }
    pub fn resume(&mut self, pid: u32, end_aid: u32) {
        let pi = self.doc.pidx(pid).unwrap();
        let last = self.doc.paths[pi].anchors.last().map(|a| a.id);
        if last != Some(end_aid) { self.reverse(pi); }
        self.active = Some(pid);
        self.selected.clear();
        for a in &self.doc.paths[pi].anchors { self.selected.insert(a.id); }
    }
    pub fn join(&mut self, act: u32, other: u32, end_aid: u32, pos: Pt) {
        let oi = self.doc.pidx(other).unwrap();
        if self.doc.paths[oi].anchors.first().map(|a| a.id) != Some(end_aid) { self.reverse(oi); }
        let moved: Vec<Anchor> = self.doc.paths[oi].anchors.clone();
        self.doc.paths.remove(oi);
        let ai = self.doc.pidx(act).unwrap();
        for a in moved { self.doc.paths[ai].anchors.push(a); }
        self.selected.clear();
        for a in &self.doc.paths[ai].anchors { self.selected.insert(a.id); }
        self.drag = Drag::PenNew { aid: end_aid, down: pos, broken: false };
    }
    pub fn begin_anchor_drag(&mut self, pos: Pt) {
        let items = self.selected.iter().filter_map(|&aid| self.doc.anchor(aid).map(|a| (aid, a.p, a.hin, a.hout))).collect();
        self.drag = Drag::Anchors { start: pos, items };
    }
    pub fn start_segment(&mut self, pid: u32, i: usize, pos: Pt) {
        let pi = self.doc.pidx(pid).unwrap(); let n = self.doc.paths[pi].anchors.len();
        let a = self.doc.paths[pi].anchors[i].clone(); let b = self.doc.paths[pi].anchors[(i+1)%n].clone();
        self.drag = Drag::Segment { pid, i, down: pos, a_out0: a.hout, b_in0: b.hin, ap0: a.p, bp0: b.p, straight: a.hout.is_none() && b.hin.is_none() };
    }
    pub fn shape_anchors(&mut self, kind: ShapeKind, start: Pt, cur: Pt) -> Vec<Anchor> {
        let mut dx = cur[0]-start[0]; let mut dy = cur[1]-start[1];
        if self.mods.shift { let s = dx.abs().max(dy.abs()); dx = if dx<0.0 {-s} else {s}; dy = if dy<0.0 {-s} else {s}; }
        let (a, b) = if self.mods.alt { ([start[0]-dx, start[1]-dy], [start[0]+dx, start[1]+dy]) } else { (start, [start[0]+dx, start[1]+dy]) };
        self.doc.build_shape(kind, a, b)
    }

    // ---------- input dispatch ----------
    pub fn eff_tool(&self) -> ToolKind {
        if self.mods.ctrl { ToolKind::Direct } else if self.tool == ToolKind::Pen && self.mods.alt { ToolKind::Convert } else { self.tool }
    }

    /// What the Pen tool would do at `pos` (world) — for the contextual pen cursor. Mirrors `tools::pen`.
    pub fn pen_hint(&self, pos: Pt) -> PenHint {
        if let Some(aid) = self.nearest_anchor(pos, ANCHOR_R, true) {
            if let Some((pi, ai)) = self.doc.aidx(aid) {
                let p = &self.doc.paths[pi];
                let (pid, n) = (p.id, p.anchors.len());
                let is_end = !p.closed && (ai == 0 || ai == n - 1);
                let tip = self.active.and_then(|ap| self.doc.pidx(ap)).and_then(|i| self.doc.paths[i].anchors.last().map(|a| a.id));
                if is_end {
                    return match self.active {
                        Some(act) if act == pid => if Some(aid) != tip { PenHint::Close } else { PenHint::Draw },
                        _ => PenHint::Connect, // join another path, or resume a selected one
                    };
                }
                if self.is_editable(pid) { return PenHint::Delete; }
            }
            return PenHint::Draw;
        }
        if let Some(pid) = self.path_under(pos) {
            if self.is_editable(pid) {
                if let Some(pi) = self.doc.pidx(pid) {
                    if let Some((_, _, d)) = self.doc.nearest_seg(pi, pos) { if d <= EDGE_R { return PenHint::Add; } }
                }
            }
        }
        if self.active.is_some() { PenHint::Draw } else { PenHint::New }
    }
    pub fn pointer_down(&mut self, pos: Pt) {
        self.cursor = pos;
        self.begin();
        self.gesture = self.eff_tool();
        tools::get(self.gesture).down(self, pos);
    }
    pub fn pointer_up(&mut self) {
        if let Drag::Shape { pid, .. } = self.drag {
            if let Some(pi) = self.doc.pidx(pid) { let b = self.doc.bbox(pi); if (b.2-b.0) < 2.0 && (b.3-b.1) < 2.0 { self.doc.paths.remove(pi); self.dirty = false; } }
        }
        if let Drag::PenClose { .. } = self.drag { self.active = None; }
        self.drag = Drag::None;
        self.commit();
    }

    pub fn pointer_move(&mut self, pos: Pt) {
        self.cursor = pos;
        if matches!(self.drag, Drag::None) { self.hover_path = self.path_under(pos); }
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::PenNew { aid, down, mut broken } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if self.mods.alt { broken = true; }
                    if let Some((pi, ai)) = self.doc.aidx(aid) {
                        let p = self.doc.paths[pi].anchors[ai].p;
                        let q = if self.mods.shift { add(p, snap45(sub(pos, p))) } else { pos };
                        let a = &mut self.doc.paths[pi].anchors[ai];
                        a.hout = Some(q);
                        if broken { a.smooth = false; } else { a.smooth = true; a.hin = Some(mirror(p, q)); }
                    }
                }
                self.drag = Drag::PenNew { aid, down, broken };
            }
            Drag::PenClose { aid, down, mut broken } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if self.mods.alt { broken = true; }
                    if let Some((pi, ai)) = self.doc.aidx(aid) {
                        let p = self.doc.paths[pi].anchors[ai].p;
                        let a = &mut self.doc.paths[pi].anchors[ai];
                        a.hout = Some(pos);
                        if broken { a.smooth = false; } else { a.smooth = true; a.hin = Some(mirror(p, pos)); }
                    }
                }
                self.drag = Drag::PenClose { aid, down, broken };
            }
            Drag::Anchors { start, items } => {
                let mut d = sub(pos, start);
                if self.mods.shift { d = snap45(d); }
                for (aid, p0, hin0, hout0) in &items {
                    if let Some(a) = self.doc.anchor_mut(*aid) {   // outer OR hole anchor
                        a.p = add(*p0, d); a.hin = hin0.map(|h| add(h, d)); a.hout = hout0.map(|h| add(h, d));
                    }
                }
                self.drag = Drag::Anchors { start, items };
                self.dirty = true;
            }
            Drag::Handle { aid, out, couple, opp_len, grab } => {
                if let Some(p) = self.doc.anchor(aid).map(|a| a.p) {
                    let mut q = add(pos, grab);
                    if self.mods.shift { q = add(p, snap45(sub(q, p))); }
                    let opp = add(p, scale(norm(sub(p, q)), opp_len));
                    if let Some(a) = self.doc.anchor_mut(aid) {
                        if out { a.hout = Some(q); } else { a.hin = Some(q); }
                        if couple { if out { a.hin = Some(opp); } else { a.hout = Some(opp); } }
                    }
                }
                self.drag = Drag::Handle { aid, out, couple, opp_len, grab };
                self.dirty = true;
            }
            Drag::Segment { pid, i, down, a_out0, b_in0, ap0, bp0, straight } => {
                let d = sub(pos, down);
                if let Some(pi) = self.doc.pidx(pid) {
                    let n = self.doc.paths[pi].anchors.len(); let bi = (i + 1) % n;
                    if straight { self.doc.paths[pi].anchors[i].p = add(ap0, d); self.doc.paths[pi].anchors[bi].p = add(bp0, d); }
                    else { self.doc.paths[pi].anchors[i].hout = Some(add(a_out0.unwrap_or(ap0), d)); self.doc.paths[pi].anchors[bi].hin = Some(add(b_in0.unwrap_or(bp0), d)); }
                }
                self.drag = Drag::Segment { pid, i, down, a_out0, b_in0, ap0, bp0, straight };
                self.dirty = true;
            }
            Drag::Shape { start, pid, kind } => {
                let anchors = self.shape_anchors(kind, start, pos);
                if let Some(pi) = self.doc.pidx(pid) { self.doc.paths[pi].anchors = anchors; }
                self.drag = Drag::Shape { start, pid, kind };
                self.dirty = true;
            }
            Drag::Marquee { start, base } => {
                let (x0, y0) = (start[0].min(pos[0]), start[1].min(pos[1]));
                let (x1, y1) = (start[0].max(pos[0]), start[1].max(pos[1]));
                self.selected = base.iter().copied().collect();
                for p in &self.doc.paths { for a in p.anchors.iter().chain(p.holes.iter().flatten()) { if a.p[0] >= x0 && a.p[0] <= x1 && a.p[1] >= y0 && a.p[1] <= y1 { self.selected.insert(a.id); } } }
                self.drag = Drag::Marquee { start, base };
            }
            Drag::ObjMarquee { start, base } => {
                let (x0, y0) = (start[0].min(pos[0]), start[1].min(pos[1]));
                let (x1, y1) = (start[0].max(pos[0]), start[1].max(pos[1]));
                self.objsel = base.iter().copied().collect();
                for pi in 0..self.doc.paths.len() {
                    if self.path_in_rect(pi, x0, y0, x1, y1) { self.objsel.insert(self.doc.paths[pi].id); }
                }
                // a marquee that catches any group member selects the whole group
                let expanded: Vec<u32> = self.objsel.iter().flat_map(|&p| self.doc.group_members(p)).collect();
                self.objsel.extend(expanded);
                self.drag = Drag::ObjMarquee { start, base };
            }
            Drag::DupPending { srcs, down, object } => {
                if dist(pos, down) < DRAG_THRESH { self.drag = Drag::DupPending { srcs, down, object }; }
                else {
                    let cids: Vec<u32> = self.doc.dup_paths(&srcs); // clones + mirrors group structure
                    if object {
                        self.objsel.clear();
                        let mut base = vec![];
                        for &cid in &cids { self.objsel.insert(cid); if let Some(pi) = self.doc.pidx(cid) { for a in self.doc.paths[pi].anchors.iter().chain(self.doc.paths[pi].holes.iter().flatten()) { base.push((a.id, a.p, a.hin, a.hout)); } } }
                        self.drag = Drag::Object { down, base };
                    } else {
                        self.selected.clear();
                        for &cid in &cids { if let Some(pi) = self.doc.pidx(cid) { for a in &self.doc.paths[pi].anchors { self.selected.insert(a.id); } } }
                        self.begin_anchor_drag(down);
                    }
                    self.dirty = true;
                    self.pointer_move(pos);
                }
            }
            Drag::Object { down, base } => {
                let mut d = sub(pos, down); if self.mods.shift { d = snap45(d); }
                for (aid, p0, hin0, hout0) in &base {
                    if let Some(a) = self.doc.anchor_mut(*aid) {
                        a.p = add(*p0, d); a.hin = hin0.map(|h| add(h, d)); a.hout = hout0.map(|h| add(h, d));
                    }
                }
                self.drag = Drag::Object { down, base };
                self.dirty = true;
            }
            Drag::Scale { handle, angle, opp_l, cen_l, h0_l, base } => {
                let pivot = if self.mods.alt { cen_l } else { opp_l };       // Alt → scale from centre
                let cx = handle <= 3 || handle == 5 || handle == 7;          // controls x (local axis)
                let cy = handle <= 3 || handle == 4 || handle == 6;          // controls y (local axis)
                let lp = rotate_about(pos, [0.0, 0.0], -angle);              // cursor → local (un-rotated) space
                let (dx0, dy0) = (h0_l[0] - pivot[0], h0_l[1] - pivot[1]);
                let mut sx = if cx && dx0.abs() > 1e-3 { (lp[0]-pivot[0])/dx0 } else { 1.0 };
                let mut sy = if cy && dy0.abs() > 1e-3 { (lp[1]-pivot[1])/dy0 } else { 1.0 };
                if self.mods.shift && cx && cy { let m = sx.abs().max(sy.abs()); sx = m.copysign(sx); sy = m.copysign(sy); }
                let tf = |w: Pt| {                                          // un-rotate → scale on local axes → re-rotate
                    let q = rotate_about(w, [0.0, 0.0], -angle);
                    let q2 = [pivot[0]+(q[0]-pivot[0])*sx, pivot[1]+(q[1]-pivot[1])*sy];
                    rotate_about(q2, [0.0, 0.0], angle)
                };
                for (aid, p0, hin0, hout0) in &base {
                    if let Some(a) = self.doc.anchor_mut(*aid) {
                        a.p = tf(*p0); a.hin = hin0.map(tf); a.hout = hout0.map(tf);
                    }
                }
                self.drag = Drag::Scale { handle, angle, opp_l, cen_l, h0_l, base };
                self.dirty = true;
            }
            Drag::Rotate { center, start, a0, base } => {
                let cur = (pos[1]-center[1]).atan2(pos[0]-center[0]);
                let mut d = cur - start;
                if self.mods.shift { let step = std::f32::consts::FRAC_PI_4; d = (d/step).round()*step; }
                for (aid, p0, hin0, hout0) in &base {
                    if let Some(a) = self.doc.anchor_mut(*aid) {
                        a.p = rotate_about(*p0, center, d);
                        a.hin = hin0.map(|h| rotate_about(h, center, d));
                        a.hout = hout0.map(|h| rotate_about(h, center, d));
                    }
                }
                self.obj_angle = a0 + d;                                     // frame rotates with the selection
                self.drag = Drag::Rotate { center, start, a0, base };
                self.dirty = true;
            }
            Drag::ConvPull { aid, down } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if let Some((pi, ai)) = self.doc.aidx(aid) {
                        let p = self.doc.paths[pi].anchors[ai].p;
                        let q = if self.mods.shift { add(p, snap45(sub(pos, p))) } else { pos };
                        let a = &mut self.doc.paths[pi].anchors[ai];
                        a.smooth = true; a.hout = Some(q); a.hin = Some(mirror(p, q));
                    }
                    self.dirty = true;
                }
                self.drag = Drag::ConvPull { aid, down };
            }
            Drag::None => {}
        }
    }

    // ---------- tool/keys ----------
    pub fn set_tool(&mut self, t: ToolKind) {
        if t == ToolKind::Object {
            // promote anchor-selection to object-selection (Illustrator A→V), then drop anchor sel
            let pids: Vec<u32> = self.selected.iter().filter_map(|&aid| self.doc.aidx(aid).map(|(pi, _)| self.doc.paths[pi].id)).collect();
            for pid in pids { for m in self.doc.group_members(pid) { self.objsel.insert(m); } } // promote whole groups
            self.selected.clear();
            self.obj_angle = 0.0;
        }
        self.tool = t;
        if t != ToolKind::Pen { self.active = None; }
        self.dsel_path = None;
        self.drag = Drag::None;
    }
    pub fn escape(&mut self) { self.active = None; self.selected.clear(); self.objsel.clear(); self.dsel_path = None; self.obj_angle = 0.0; self.drag = Drag::None; }
    pub fn nudge(&mut self, dx: f32, dy: f32) {
        if self.selected.is_empty() { return; }
        self.begin();
        let ids: Vec<u32> = self.selected.iter().copied().collect();
        for aid in ids { if let Some(a) = self.doc.anchor_mut(aid) {
            a.p = add(a.p, [dx, dy]); a.hin = a.hin.map(|h| add(h, [dx, dy])); a.hout = a.hout.map(|h| add(h, [dx, dy]));
        }}
        self.dirty = true; self.commit();
    }
    pub fn delete_selected(&mut self) {
        self.begin();
        if !self.selected.is_empty() { let ids: Vec<u32> = self.selected.iter().copied().collect(); for aid in ids { self.delete_anchor(aid); } }
        else if !self.objsel.is_empty() { let pids: Vec<u32> = self.objsel.iter().copied().collect(); for pid in pids { if let Some(pi) = self.doc.pidx(pid) { self.doc.paths.remove(pi); } } self.objsel.clear(); }
        self.dirty = true; self.commit();
    }
    pub fn double_click(&mut self, pos: Pt) {
        if let Some(pid) = self.path_under(pos) {
            self.set_tool(ToolKind::Direct);   // set_tool clears dsel_path, so set it after
            self.selected.clear();
            self.dsel_path = Some(pid);         // path-level select: anchors show hollow, grab one to edit it
        }
    }

    // ---------- paint (fill / stroke) ----------
    pub fn swap_paint(&mut self) { self.paint = if self.paint == PaintTarget::Fill { PaintTarget::Stroke } else { PaintTarget::Fill }; }
    pub fn apply_paint(&mut self, color: Option<Rgba>) {
        match self.paint { PaintTarget::Fill => self.cur_fill = color, PaintTarget::Stroke => self.cur_stroke = color }
        let mut pids: HashSet<u32> = self.objsel.clone();
        for &aid in &self.selected { if let Some(pid) = self.doc.pid_of_anchor(aid) { pids.insert(pid); } }
        if pids.is_empty() { return; }
        self.begin();
        for pid in pids { if let Some(pi) = self.doc.pidx(pid) {
            match self.paint { PaintTarget::Fill => self.doc.paths[pi].fill = color, PaintTarget::Stroke => self.doc.paths[pi].stroke = color }
        }}
        self.dirty = true; self.commit();
    }
    fn selected_pids(&self) -> HashSet<u32> {
        let mut pids: HashSet<u32> = self.objsel.clone();
        for &aid in &self.selected { if let Some(pid) = self.doc.pid_of_anchor(aid) { pids.insert(pid); } }
        pids
    }
    fn apply_current(&mut self) {
        let (f, st) = (self.cur_fill, self.cur_stroke);
        let pids = self.selected_pids(); if pids.is_empty() { return; }
        self.begin();
        for q in pids { if let Some(pi) = self.doc.pidx(q) { self.doc.paths[pi].fill = f; self.doc.paths[pi].stroke = st; } }
        self.dirty = true; self.commit();
    }
    pub fn swap_colors(&mut self) { std::mem::swap(&mut self.cur_fill, &mut self.cur_stroke); self.apply_current(); }
    pub fn default_paint(&mut self) { self.cur_fill = Some([0.95, 0.95, 0.96, 1.0]); self.cur_stroke = Some([0.12, 0.12, 0.13, 1.0]); self.apply_current(); }
    pub fn bump_stroke(&mut self, delta: f32) {
        self.cur_sw = (self.cur_sw + delta).max(0.5);
        let w = self.cur_sw; let pids = self.selected_pids(); if pids.is_empty() { return; }
        self.begin();
        for q in pids { if let Some(pi) = self.doc.pidx(q) { self.doc.paths[pi].stroke_width = w; } }
        self.dirty = true; self.commit();
    }
    pub fn eyedrop(&mut self, pid: u32) {
        let (f, st, sw) = if let Some(pi) = self.doc.pidx(pid) { let p = &self.doc.paths[pi]; (p.fill, p.stroke, p.stroke_width) } else { return };
        self.cur_fill = f; self.cur_stroke = st; self.cur_sw = sw;
        let pids = self.selected_pids(); if pids.is_empty() { return; }
        self.begin();
        for q in pids { if let Some(pi) = self.doc.pidx(q) { self.doc.paths[pi].fill = f; self.doc.paths[pi].stroke = st; self.doc.paths[pi].stroke_width = sw; } }
        self.dirty = true; self.commit();
    }
}
