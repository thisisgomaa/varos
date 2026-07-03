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
pub const AB_HANDLE_R: f32 = 8.0;   // grab radius (screen px) for an artboard resize handle
pub const AB_GAP: f32 = 60.0;       // default world gap between auto-placed artboards

#[derive(Clone, Copy, Default)]
pub struct Mods { pub shift: bool, pub alt: bool, pub ctrl: bool }

#[derive(Clone, Copy, PartialEq)]
pub enum PaintTarget { Fill, Stroke }

#[derive(Clone, Copy, PartialEq)]
pub enum ToolKind { Object, Direct, Pen, Rect, Ellipse, Triangle, Polygon, Convert, Eyedropper, Artboard, Rotate, Scale }

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
    Guide { idx: usize },   // moving an existing ruler guide (idx into doc.guides)
    Shape { start: Pt, pid: u32, kind: ShapeKind },
    Marquee { start: Pt, base: Vec<u32> },
    ObjMarquee { start: Pt, base: Vec<u32> },
    DupPending { srcs: Vec<u32>, down: Pt, object: bool },
    Object { down: Pt, base: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    // scale works in the frame's LOCAL (un-rotated) space; opp_l/cen_l/h0_l are local handle coords
    Scale { handle: u8, angle: f32, opp_l: Pt, cen_l: Pt, h0_l: Pt, base: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    Rotate { center: Pt, start: f32, a0: f32, base: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    ScaleLive { pivot: Pt, down: Pt, base: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },   // Scale tool: about `pivot`, ratio from `down`
    TfPending { pivot: Pt, down: Pt },   // Rotate/Scale pressed: a plain click relocates the pivot, a drag transforms
    ConvPull { aid: u32, down: Pt },
}

/// What a press on the object-selection bounding box hit.
#[derive(Clone, Copy)]
pub enum TfHit { Scale(u8), Rotate(u8) } // u8 = handle index 0..7 (corners 0-3, edge mids 4-7)

/// The last transform, replayed by Transform Again (Ctrl+D) — move / rotate / scale / reflect.
#[derive(Clone, Copy)]
pub enum TfAgain {
    Move(Pt),
    Rotate { pivot: Pt, ang: f32 },
    Scale { pivot: Pt, sx: f32, sy: f32 },
}

/// Artboard-tool drag state — kept STRICTLY separate from `Drag` (the object/anchor engine) so the two
/// can never cross-grab (the no-cross-grab guarantee). `Move` carries the artwork base when "move artwork
/// with artboard" is on, so the page and the art on it translate together.
pub enum AbDrag {
    None,
    Move { grab: Pt, ox: f32, oy: f32, art: Vec<(u32, Pt, Option<Pt>, Option<Pt>)> },
    Resize { handle: u8, ox: f32, oy: f32, ow: f32, oh: f32 },
    Create { start: Pt },
}

/// What a press in the Artboard tool landed on: a resize handle of the ACTIVE page, the body of some
/// page (index), or nothing (empty board → drag to create a new page).
#[derive(Clone, Copy)]
pub enum AbHit { Handle(u8), Body(usize) }

/// A piece of snap feedback to draw this frame (world coords; the renderer styles/sizes it). Transient —
/// rebuilt every move, cleared on pointer-up, never serialized. (SNAP_TRANSFORM_SPEC §0.)
#[derive(Clone, Copy)]
pub enum SnapGuide {
    Line { a: Pt, b: Pt },   // an alignment extension line through the matched feature
    Gap { a: Pt, b: Pt },    // an equal-spacing bar between two points (Stage-1+: tick-capped)
    Point { p: Pt },         // a snapped target POINT (anchor/edge/midpoint) — constant-size marker
    PathHi { pid: u32 },     // highlight a whole path we snapped onto (Illustrator's "path" highlight)
}

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

/// Resize an artboard rect by dragging handle `h` (corners 0-3, edge mids 4-7) to `pos`, keeping the
/// opposite edge(s) fixed. `shift` constrains a CORNER drag to the original aspect ratio. Returns the
/// normalised (x, y, w, h) with a 1pt minimum so the page never collapses.
fn ab_resized(h: u8, ox: f32, oy: f32, ow: f32, oh: f32, pos: Pt, shift: bool) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (ox, oy, ox + ow, oy + oh);
    let (ml, mr) = (matches!(h, 0 | 3 | 7), matches!(h, 1 | 2 | 5));
    let (mt, mb) = (matches!(h, 0 | 1 | 4), matches!(h, 2 | 3 | 6));
    if ml { x0 = pos[0]; } if mr { x1 = pos[0]; }
    if mt { y0 = pos[1]; } if mb { y1 = pos[1]; }
    if shift && matches!(h, 0 | 1 | 2 | 3) && ow > 1e-3 && oh > 1e-3 {
        let (fx, fy) = (if ml { x1 } else { x0 }, if mt { y1 } else { y0 }); // the fixed (opposite) corner
        let s = ((x1 - x0).abs() / ow).max((y1 - y0).abs() / oh);
        let (w, ht) = (ow * s, oh * s);
        x0 = if ml { fx - w } else { fx }; x1 = if ml { fx } else { fx + w };
        y0 = if mt { fy - ht } else { fy }; y1 = if mt { fy } else { fy + ht };
    }
    (x0.min(x1), y0.min(y1), (x1 - x0).abs().max(1.0), (y1 - y0).abs().max(1.0))
}

/// (x, y, w, h) of the rect spanned by two corners; `square` forces equal sides (Shift while creating).
fn rect_from_corners(a: Pt, b: Pt, square: bool) -> (f32, f32, f32, f32) {
    let (mut dx, mut dy) = (b[0] - a[0], b[1] - a[1]);
    if square { let s = dx.abs().max(dy.abs()); dx = s.copysign(dx); dy = s.copysign(dy); }
    (a[0].min(a[0] + dx), a[1].min(a[1] + dy), dx.abs().max(1.0), dy.abs().max(1.0))
}

/// Axis-aligned bbox of a drag's base anchor set (the pre-drag selection extent), for snapping.
fn base_bbox(base: &[(u32, Pt, Option<Pt>, Option<Pt>)]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (_, p, _, _) in base { x0 = x0.min(p[0]); y0 = y0.min(p[1]); x1 = x1.max(p[0]); y1 = y1.max(p[1]); }
    if x0 <= x1 { (x0, y0, x1, y1) } else { (0.0, 0.0, 0.0, 0.0) }
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
    pub ab_drag: AbDrag,     // Artboard-tool drag (move/resize/create) — parallel to `drag`, never crosses it
    pub snap_guides: Vec<SnapGuide>,    // this frame's snap feedback (transient, never serialized)
    pub snap_hud: Option<(Pt, String)>, // live measurement label: (world anchor near the cursor, text)
    pub last_tf: Option<(TfAgain, bool)>, // (last transform, was_copy) — Transform Again (Ctrl+D)
    gesture_copy: bool,                 // transient: did the current gesture duplicate (Alt-drag)?
    gesture_delta: Pt,                  // transient: running net delta of the current Object drag
    gesture_tf: Option<TfAgain>,        // transient: the net rotate/scale/reflect this gesture (for Transform Again)
    pub cursor: Pt,
    pub ppu: f32,            // pixels-per-unit (view zoom) — so grab tolerances stay constant on screen
    pub obj_angle: f32,      // orientation of the object-selection transform frame (rotates with the selection)
    pub pivot: Option<Pt>,   // Rotate/Scale/Reflect transform origin (None ⇒ selection centre); set by a click
    pub hover_path: Option<u32>,
    pub show_rulers: bool,   // View ▸ Rulers (Ctrl+R). View pref, not serialized; default ON.
    pub guides_hidden: bool, // View ▸ Hide Guides (Ctrl+;). View pref, not serialized.
    pub origin_preview: Option<Pt>,  // while dragging the ruler corner: the live (snapped) zero-point → dashed crosshair
    pub guide_preview: Option<Guide>, // while dragging a NEW guide out of a ruler: the live (snapped) guide
    pub mods: Mods,
    pub cur_fill: Option<Rgba>, pub cur_stroke: Option<Rgba>, pub cur_sw: f32, pub paint: PaintTarget,
    pub recent_colors: Vec<Rgba>, // picker MRU — newest first, deduped, cap 12; ephemeral (not serialized)
    /// Document revision — bumps on every committed change, undo and redo. `rev != saved_rev` (held by
    /// the app) = unsaved changes. (`dirty` below is a PER-GESTURE flag for begin/commit, not this.)
    pub rev: u64,
    pub dirty: bool,
    undo: Vec<Document>, redo: Vec<Document>, pending: Option<Document>,
}

impl Editor {
    pub fn new() -> Self {
        Editor { doc: Document::default(), tool: ToolKind::Object, gesture: ToolKind::Object, active: None,
                 selected: HashSet::new(), objsel: HashSet::new(), dsel_path: None, drag: Drag::None, ab_drag: AbDrag::None,
                 snap_guides: vec![], snap_hud: None, last_tf: None, gesture_copy: false, gesture_delta: [0.0, 0.0], gesture_tf: None, cursor: [0.0, 0.0],
                 ppu: 1.0, obj_angle: 0.0, pivot: None, hover_path: None, show_rulers: true, guides_hidden: false,
                 origin_preview: None, guide_preview: None, mods: Mods::default(),
                 cur_fill: Some([0.95, 0.95, 0.96, 1.0]), cur_stroke: Some([0.12, 0.12, 0.13, 1.0]), cur_sw: 2.0, paint: PaintTarget::Fill,
                 recent_colors: vec![], rev: 0, dirty: false, undo: vec![], redo: vec![], pending: None }
    }

    // ---------- selection-aware queries ----------
    pub fn is_editable(&self, pid: u32) -> bool {
        self.active == Some(pid) || self.objsel.contains(&pid)
            || self.doc.paths.iter().find(|p| p.id == pid).map_or(false, |p| p.anchors.iter().chain(p.holes.iter().flatten()).any(|a| self.selected.contains(&a.id)))
    }
    pub fn path_shown(&self, pid: u32) -> bool {
        self.hover_path == Some(pid) || self.path_selected(pid)
    }
    /// Like `path_shown` but EXCLUDES mere hover — a path is "selected" only once you actually act on it
    /// (object/anchor/whole-path/pen-active). Anchor markers gate on this so hovering doesn't reveal points
    /// (Illustrator shows anchors only on real selection, never on hover).
    pub fn path_selected(&self, pid: u32) -> bool {
        self.active == Some(pid) || self.objsel.contains(&pid) || self.dsel_path == Some(pid)
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
            if self.doc.eff_hidden(self.doc.paths[pi].id) || self.doc.eff_locked(self.doc.paths[pi].id) { continue; } // not clickable (cascades)
            if let Some((_, _, d)) = self.doc.nearest_seg(pi, pos) {
                if d <= edge_r && best.map_or(true, |(_, bd)| d < bd) { best = Some((self.doc.paths[pi].id, d)); }
            }
        }
        if let Some((id, _)) = best { return Some(id); }
        for pi in 0..self.doc.paths.len() {
            if self.doc.eff_hidden(self.doc.paths[pi].id) || self.doc.eff_locked(self.doc.paths[pi].id) { continue; }
            if self.doc.point_in_path(pi, pos) { return Some(self.doc.paths[pi].id); }
        }
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
    /// The transform origin (world) for the Rotate/Scale/Reflect tools: the user-set pivot, or — until a
    /// click relocates it — the object-selection bbox centre.
    pub fn pivot_point(&self) -> Option<Pt> {
        self.pivot.or_else(|| self.obj_bbox().map(|(x0, y0, x1, y1)| [(x0 + x1) * 0.5, (y0 + y1) * 0.5]))
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
            self.doc.paths.push(Path { holes, ..Path::new(id, anchors, true, fill, stroke, sw) });
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
        // tree-side arrange: units move within their OWN parent (Illustrator scope) + re-flatten
        let (toward_front, extreme) = match op {
            ZOrder::Front => (true, true), ZOrder::Back => (false, true),
            ZOrder::Forward => (true, false), ZOrder::Backward => (false, false),
        };
        self.doc.arrange_units(&sel, toward_front, extreme);
        self.dirty = true; self.commit();
    }
    /// Align selected ANCHOR POINTS (Direct Selection) to a shared edge/centre of their bbox — handles
    /// move with their anchor so curves keep shape. Illustrator aligns points the same way.
    fn align_anchors(&mut self, mode: AlignMode) {
        let ids: Vec<u32> = self.selected.iter().copied().collect();
        if ids.len() < 2 { return; }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &id in &ids { if let Some(a) = self.doc.anchor(id) { x0 = x0.min(a.p[0]); y0 = y0.min(a.p[1]); x1 = x1.max(a.p[0]); y1 = y1.max(a.p[1]); } }
        if x0 > x1 { return; }
        self.begin();
        for &id in &ids { if let Some(a) = self.doc.anchor_mut(id) {
            let np = match mode {
                AlignMode::Left => [x0, a.p[1]], AlignMode::Right => [x1, a.p[1]], AlignMode::CenterH => [(x0 + x1) * 0.5, a.p[1]],
                AlignMode::Top => [a.p[0], y0], AlignMode::Bottom => [a.p[0], y1], AlignMode::Middle => [a.p[0], (y0 + y1) * 0.5],
            };
            let dx = [np[0] - a.p[0], np[1] - a.p[1]];
            a.p = np; a.hin = a.hin.map(|h| add(h, dx)); a.hout = a.hout.map(|h| add(h, dx));
        }}
        self.dirty = true; self.commit();
    }
    /// Distribute selected anchor points evenly along an axis (by position). Needs ≥3.
    fn distribute_anchors(&mut self, axis: DistAxis) {
        let mut items: Vec<(u32, f32)> = self.selected.iter().filter_map(|&id| self.doc.anchor(id).map(|a| (id, match axis { DistAxis::Horizontal => a.p[0], DistAxis::Vertical => a.p[1] }))).collect();
        if items.len() < 3 { return; }
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let n = items.len();
        let step = (items[n - 1].1 - items[0].1) / (n as f32 - 1.0);
        self.begin();
        for k in 1..n - 1 {
            let target = items[0].1 + step * k as f32;
            if let Some(a) = self.doc.anchor_mut(items[k].0) {
                let dx = match axis { DistAxis::Horizontal => [target - a.p[0], 0.0], DistAxis::Vertical => [0.0, target - a.p[1]] };
                a.p = add(a.p, dx); a.hin = a.hin.map(|h| add(h, dx)); a.hout = a.hout.map(|h| add(h, dx));
            }
        }
        self.dirty = true; self.commit();
    }
    pub fn align(&mut self, mode: AlignMode) {
        if !self.selected.is_empty() { self.align_anchors(mode); return; }   // Direct Selection: align the points
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
        if !self.selected.is_empty() { self.distribute_anchors(axis); return; }   // Direct Selection: distribute points
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
    /// Distribute the object selection so the GAP between successive bbox edges equals `gap` (px),
    /// ordered along the axis and anchored at the first (lowest) object. Illustrator "distribute spacing".
    pub fn distribute_spacing(&mut self, axis: DistAxis, gap: f32) {
        if self.objsel.len() < 2 { return; }
        let mut items: Vec<(u32, f32, f32)> = self.objsel.iter().filter_map(|&pid| self.doc.pidx(pid).map(|pi| {
            let b = self.doc.outline_bbox(pi);
            match axis { DistAxis::Horizontal => (pid, b.0, b.2 - b.0), DistAxis::Vertical => (pid, b.1, b.3 - b.1) }
        })).collect();
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        self.begin();
        let mut cur = items[0].1 + items[0].2 + gap;            // edge just after the first object
        for k in 1..items.len() {
            let (pid, lo, len) = items[k];
            let d = cur - lo;
            if let Some(pi) = self.doc.pidx(pid) {
                self.translate_path(pi, match axis { DistAxis::Horizontal => [d, 0.0], DistAxis::Vertical => [0.0, d] });
            }
            cur += len + gap;
        }
        self.obj_angle = 0.0;
        self.dirty = true; self.commit();
    }
    /// Mirror the object selection across its bbox centre (horizontal = flip left↔right).
    pub fn flip(&mut self, horizontal: bool) {
        let (x0, y0, x1, y1) = match self.obj_bbox() { Some(b) => b, None => return };
        let (cx, cy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        let base = self.objsel_base();
        self.begin();
        let tf = |p: Pt| if horizontal { [2.0 * cx - p[0], p[1]] } else { [p[0], 2.0 * cy - p[1]] };
        for (aid, p0, hin0, hout0) in &base {
            if let Some(a) = self.doc.anchor_mut(*aid) { a.p = tf(*p0); a.hin = hin0.map(tf); a.hout = hout0.map(tf); }
        }
        self.obj_angle = 0.0;
        self.dirty = true; self.commit();
    }
    /// Set the object selection's AXIS-ALIGNED bbox (any of x/y/w/h). `ax,ay` (0..1) is the reference
    /// point that stays fixed while w/h scale (the Transform 9-point selector). x/y set the bbox
    /// top-left absolutely. Drives the editable Transform X·Y·W·H fields. Resets frame angle.
    pub fn set_obj_bbox(&mut self, nx: Option<f32>, ny: Option<f32>, nw: Option<f32>, nh: Option<f32>, ax: f32, ay: f32) {
        let (x0, y0, x1, y1) = match self.obj_bbox() { Some(b) => b, None => return };
        let (w, h) = (x1 - x0, y1 - y0);
        let sx = if w.abs() > 1e-3 { nw.map(|v| (v / w).max(1e-3)).unwrap_or(1.0) } else { 1.0 };
        let sy = if h.abs() > 1e-3 { nh.map(|v| (v / h).max(1e-3)).unwrap_or(1.0) } else { 1.0 };
        let (fx, fy) = (x0 + w * ax, y0 + h * ay);              // reference point kept fixed under scale
        let nx0 = fx + (x0 - fx) * sx;                          // where the left edge lands after scaling
        let ny0 = fy + (y0 - fy) * sy;
        let tx = nx.map(|v| v - nx0).unwrap_or(0.0);            // then shift so the top-left equals x/y
        let ty = ny.map(|v| v - ny0).unwrap_or(0.0);
        if (sx - 1.0).abs() < 1e-5 && (sy - 1.0).abs() < 1e-5 && tx.abs() < 1e-4 && ty.abs() < 1e-4 { return; }
        let base = self.objsel_base();
        self.begin();
        let tf = |p: Pt| [fx + (p[0] - fx) * sx + tx, fy + (p[1] - fy) * sy + ty];
        for (aid, p0, hin0, hout0) in &base {
            if let Some(a) = self.doc.anchor_mut(*aid) { a.p = tf(*p0); a.hin = hin0.map(tf); a.hout = hout0.map(tf); }
        }
        self.obj_angle = 0.0;
        self.dirty = true; self.commit();
    }
    /// Rotate the object selection so its transform frame sits at `deg` degrees (about the frame centre).
    pub fn set_obj_rotation(&mut self, deg: f32) {
        let bb = match self.obj_local_bbox() { Some(b) => b, None => return };
        let cen_l = [(bb.0 + bb.2) * 0.5, (bb.1 + bb.3) * 0.5];
        let center = rotate_about(cen_l, [0.0, 0.0], self.obj_angle);
        let target = deg.to_radians();
        let d = target - self.obj_angle;
        if d.abs() < 1e-4 { return; }
        let base = self.objsel_base();
        self.begin();
        for (aid, p0, hin0, hout0) in &base {
            if let Some(a) = self.doc.anchor_mut(*aid) {
                a.p = rotate_about(*p0, center, d);
                a.hin = hin0.map(|h| rotate_about(h, center, d));
                a.hout = hout0.map(|h| rotate_about(h, center, d));
            }
        }
        self.obj_angle = target;
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

    /// Transform Again (Illustrator Ctrl+D): replay the last Object transform on the current selection. If
    /// the last gesture left a copy (Alt-drag duplicate), duplicate first then offset — so Ctrl+D Ctrl+D…
    /// step-and-repeats. One undo step each; updates the selection to the result so it chains.
    pub fn transform_again(&mut self) {
        let (tf, was_copy) = match self.last_tf { Some(t) => t, None => return };
        if self.objsel.is_empty() { return; }
        self.begin();
        if was_copy {
            let srcs: Vec<u32> = self.objsel.iter().copied().collect();
            let cids = self.doc.dup_paths(&srcs);
            self.objsel = cids.into_iter().collect();
        }
        // the point map for the remembered transform — Ctrl+D repeats rotate/scale/reflect, not just moves
        // (so rotate-a-copy then Ctrl+D+D… builds a radial pattern).
        let f: Box<dyn Fn(Pt) -> Pt> = match tf {
            TfAgain::Move(d) => Box::new(move |p| add(p, d)),
            TfAgain::Rotate { pivot, ang } => Box::new(move |p| rotate_about(p, pivot, ang)),
            TfAgain::Scale { pivot, sx, sy } => Box::new(move |p| [pivot[0] + (p[0]-pivot[0])*sx, pivot[1] + (p[1]-pivot[1])*sy]),
        };
        let base = self.objsel_base();
        for (aid, p0, hin0, hout0) in &base {
            if let Some(a) = self.doc.anchor_mut(*aid) { a.p = f(*p0); a.hin = hin0.map(|h| f(h)); a.hout = hout0.map(|h| f(h)); }
        }
        self.obj_angle = if let TfAgain::Rotate { ang, .. } = tf { self.obj_angle + ang } else { 0.0 };
        self.dirty = true; self.commit();
    }

    // ---------- artboards (the Artboard tool, Shift+O) ----------
    /// The 8 resize handles of an artboard rect (corners 0-3, edge mids 4-7) in world space.
    pub fn ab_handles(ab: &Artboard) -> [Pt; 8] { Self::bbox_handles(ab.rect()) }

    /// What a press at `pos` hits in the Artboard tool: a resize handle of the ACTIVE page, the body of
    /// some page (topmost first), or nothing (empty board → drag to create a page).
    pub fn ab_hit(&self, pos: Pt) -> Option<AbHit> {
        if let Some(ab) = self.doc.active_artboard() {
            let r = AB_HANDLE_R / self.ppu;
            for (i, h) in Self::ab_handles(ab).iter().enumerate() {
                if dist(pos, *h) <= r { return Some(AbHit::Handle(i as u8)); }
            }
        }
        for i in (0..self.doc.artboards.len()).rev() {
            if self.doc.artboards[i].contains(pos) { return Some(AbHit::Body(i)); }
        }
        None
    }

    /// Path ids whose outline bbox overlaps artboard `i` — the artwork that travels when the page moves.
    fn paths_on_ab(&self, i: usize) -> Vec<u32> {
        let (x0, y0, x1, y1) = match self.doc.artboards.get(i) { Some(a) => a.rect(), None => return vec![] };
        (0..self.doc.paths.len()).filter(|&pi| {
            if self.doc.eff_hidden(self.doc.paths[pi].id) || self.doc.eff_locked(self.doc.paths[pi].id) { return false; }
            let b = self.doc.outline_bbox(pi);
            b.0 <= x1 && b.2 >= x0 && b.1 <= y1 && b.3 >= y0
        }).map(|pi| self.doc.paths[pi].id).collect()
    }
    fn anchors_base(&self, pids: &[u32]) -> Vec<(u32, Pt, Option<Pt>, Option<Pt>)> {
        let mut base = vec![];
        for &pid in pids { if let Some(pi) = self.doc.pidx(pid) {
            for a in self.doc.paths[pi].anchors.iter().chain(self.doc.paths[pi].holes.iter().flatten()) { base.push((a.id, a.p, a.hin, a.hout)); }
        }}
        base
    }

    pub fn ab_down(&mut self, pos: Pt) {
        match self.ab_hit(pos) {
            Some(AbHit::Handle(h)) => {
                if let Some(ab) = self.doc.active_artboard() {
                    self.ab_drag = AbDrag::Resize { handle: h, ox: ab.x, oy: ab.y, ow: ab.w, oh: ab.h };
                }
            }
            Some(AbHit::Body(i)) => {
                self.doc.active = i;
                let (ox, oy) = (self.doc.artboards[i].x, self.doc.artboards[i].y);
                let art = if self.doc.move_art_with_ab { let p = self.paths_on_ab(i); self.anchors_base(&p) } else { vec![] };
                self.ab_drag = AbDrag::Move { grab: pos, ox, oy, art };
            }
            None => {
                // empty board → start a NEW page by dragging (resizing its BR corner). Tiny ⇒ dropped on up.
                let n = self.doc.artboards.len();
                self.doc.artboards.push(Artboard { x: pos[0], y: pos[1], w: 1.0, h: 1.0,
                    name: format!("Artboard {}", n + 1), ..Artboard::default() });
                self.doc.active = self.doc.artboards.len() - 1;
                self.ab_drag = AbDrag::Create { start: pos };
            }
        }
    }

    pub fn ab_move(&mut self, pos: Pt) {
        match std::mem::replace(&mut self.ab_drag, AbDrag::None) {
            AbDrag::Move { grab, ox, oy, art } => {
                let mut d = sub(pos, grab); if self.mods.shift { d = snap45(d); }
                if let Some(ab) = self.doc.active_artboard_mut() { ab.x = ox + d[0]; ab.y = oy + d[1]; }
                for (aid, p0, hin0, hout0) in &art {
                    if let Some(a) = self.doc.anchor_mut(*aid) { a.p = add(*p0, d); a.hin = hin0.map(|h| add(h, d)); a.hout = hout0.map(|h| add(h, d)); }
                }
                self.ab_drag = AbDrag::Move { grab, ox, oy, art };
                self.dirty = true;
            }
            AbDrag::Resize { handle, ox, oy, ow, oh } => {
                let (x, y, w, h) = ab_resized(handle, ox, oy, ow, oh, pos, self.mods.shift);
                if let Some(ab) = self.doc.active_artboard_mut() { ab.x = x; ab.y = y; ab.w = w; ab.h = h; }
                self.ab_drag = AbDrag::Resize { handle, ox, oy, ow, oh };
                self.dirty = true;
            }
            AbDrag::Create { start } => {
                let (x, y, w, h) = rect_from_corners(start, pos, self.mods.shift);
                if let Some(ab) = self.doc.active_artboard_mut() { ab.x = x; ab.y = y; ab.w = w; ab.h = h; }
                self.ab_drag = AbDrag::Create { start };
                self.dirty = true;
            }
            AbDrag::None => {}
        }
    }

    pub fn ab_up(&mut self) {
        if let AbDrag::Create { .. } = self.ab_drag {
            // a click or tiny drag didn't make a real page — drop it
            if let Some(ab) = self.doc.active_artboard() { if ab.w < 4.0 || ab.h < 4.0 {
                let i = self.doc.active; self.doc.artboards.remove(i);
                self.doc.active = self.doc.active.min(self.doc.artboards.len().saturating_sub(1));
                self.dirty = false;
            }}
        }
        self.ab_drag = AbDrag::None;
    }

    // ---- artboard edits driven by the panel / the on-canvas ⋮ menu (each one undo step) ----
    pub fn ab_set_active(&mut self, i: usize) { if i < self.doc.artboards.len() { self.doc.active = i; } }
    pub fn ab_set_rect(&mut self, i: usize, nx: Option<f32>, ny: Option<f32>, nw: Option<f32>, nh: Option<f32>) {
        self.begin();
        if let Some(ab) = self.doc.artboards.get_mut(i) {
            if let Some(v) = nx { ab.x = v; } if let Some(v) = ny { ab.y = v; }
            if let Some(v) = nw { ab.w = v.max(1.0); } if let Some(v) = nh { ab.h = v.max(1.0); }
        }
        self.dirty = true; self.commit();
    }
    pub fn ab_rename(&mut self, i: usize, name: String) {
        self.begin();
        let n = name.trim();
        if let Some(ab) = self.doc.artboards.get_mut(i) { if !n.is_empty() { ab.name = n.to_string(); } }
        self.dirty = true; self.commit();
    }
    pub fn ab_set_color(&mut self, i: usize, c: Option<Rgba>) {
        self.begin();
        if let Some(ab) = self.doc.artboards.get_mut(i) { ab.page_color = c; }
        self.dirty = true; self.commit();
    }
    pub fn ab_toggle_clip(&mut self, i: usize) {
        self.begin();
        if let Some(ab) = self.doc.artboards.get_mut(i) { ab.clip = !ab.clip; }
        self.dirty = true; self.commit();
    }
    pub fn ab_orient(&mut self, i: usize) {
        self.begin();
        if let Some(ab) = self.doc.artboards.get_mut(i) { std::mem::swap(&mut ab.w, &mut ab.h); }
        self.dirty = true; self.commit();
    }
    pub fn ab_set_move_art(&mut self, on: bool) { self.doc.move_art_with_ab = on; }   // a mode flag, not undoable
    /// Place a fresh page to the RIGHT of the right-most one (with a gap), copying the active page's size.
    pub fn ab_add(&mut self) {
        self.begin();
        let (x, y, w, h, n) = self.ab_next_slot();
        self.doc.artboards.push(Artboard { x, y, w, h, name: format!("Artboard {}", n), ..Artboard::default() });
        self.doc.active = self.doc.artboards.len() - 1;
        self.dirty = true; self.commit();
    }
    pub fn ab_duplicate(&mut self, i: usize) {
        self.begin();
        if let Some(src) = self.doc.artboards.get(i).cloned() {
            let mut c = src.clone(); c.x = src.x + src.w + AB_GAP; c.name = format!("{} copy", src.name);
            self.doc.artboards.insert(i + 1, c);
            self.doc.active = i + 1;
            self.dirty = true;
        }
        self.commit();
    }
    pub fn ab_delete(&mut self, i: usize) {
        if self.doc.artboards.len() <= 1 { return; }   // never delete the last page
        self.begin();
        if i < self.doc.artboards.len() { self.doc.artboards.remove(i); }
        self.doc.active = self.doc.active.min(self.doc.artboards.len() - 1);
        self.dirty = true; self.commit();
    }
    /// Set the page COUNT (≥1): append default pages to the right, or trim from the end.
    pub fn ab_set_count(&mut self, n: usize) {
        let n = n.max(1);
        if n == self.doc.artboards.len() { return; }
        self.begin();
        while self.doc.artboards.len() < n {
            let (x, y, w, h, k) = self.ab_next_slot();
            self.doc.artboards.push(Artboard { x, y, w, h, name: format!("Artboard {}", k), ..Artboard::default() });
        }
        while self.doc.artboards.len() > n { self.doc.artboards.pop(); }
        self.doc.active = self.doc.active.min(self.doc.artboards.len() - 1);
        self.dirty = true; self.commit();
    }
    /// (x, y, w, h, ordinal) for the next page placed to the right of the right-most one.
    fn ab_next_slot(&self) -> (f32, f32, f32, f32, usize) {
        let right = self.doc.artboards.iter().map(|a| a.x + a.w).fold(f32::MIN, f32::max);
        let (y, w, h) = self.doc.active_artboard().map(|a| (a.y, a.w, a.h)).unwrap_or((0.0, 1080.0, 1080.0));
        let x = if right > f32::MIN { right + AB_GAP } else { 0.0 };
        (x, y, w, h, self.doc.artboards.len() + 1)
    }

    // ---------- snapping (the SnapEngine — SNAP_TRANSFORM_SPEC) ----------
    /// All object + artboard X/Y target lines for snapping, each as `(coord, span_lo, span_hi)` (the span
    /// lets a guide reach both the moving and the target object). Excludes the current selection + hidden.
    fn snap_target_lines(&self) -> (Vec<(f32, f32, f32)>, Vec<(f32, f32, f32)>) {
        let cfg = &self.doc.snap;
        let (mut txl, mut tyl) = (vec![], vec![]);
        for pi in 0..self.doc.paths.len() {
            let p = &self.doc.paths[pi];
            if p.hidden || self.objsel.contains(&p.id) { continue; }
            let b = self.doc.outline_bbox(pi);
            if cfg.object_bounds { txl.push((b.0, b.1, b.3)); txl.push((b.2, b.1, b.3)); tyl.push((b.1, b.0, b.2)); tyl.push((b.3, b.0, b.2)); }
            if cfg.bbox_mids { txl.push(((b.0 + b.2) * 0.5, b.1, b.3)); tyl.push(((b.1 + b.3) * 0.5, b.0, b.2)); }
        }
        if cfg.artboard { if let Some(ab) = self.doc.active_artboard() {
            let (ax0, ay0, ax1, ay1) = ab.rect();
            txl.push((ax0, ay0, ay1)); txl.push((ax1, ay0, ay1)); tyl.push((ay0, ax0, ax1)); tyl.push((ay1, ax0, ax1));
            if cfg.artboard_mids { txl.push(((ax0 + ax1) * 0.5, ay0, ay1)); tyl.push(((ay0 + ay1) * 0.5, ax0, ax1)); }
        }}
        if cfg.guides && !self.guides_hidden {   // ruler guides are just more snap lines (infinite extent)
            for g in &self.doc.guides {
                if g.vertical { txl.push((g.pos, -1.0e6, 1.0e6)); } else { tyl.push((g.pos, -1.0e6, 1.0e6)); }
            }
        }
        (txl, tyl)
    }

    /// World spacing of the FINEST VISIBLE dot-grid level at the current zoom, so "Snap to Grid" lands
    /// exactly on the dots the user sees (mirrors the renderer's adaptive base-5 grid, tess.rs build_bg —
    /// TARGET 30px, MIN 9px). Zoom-dependent: zoom in → finer grid → snaps to the closer dots.
    pub fn adaptive_grid_step(&self) -> f32 {
        let zoom = self.ppu.max(1e-4);
        let k0 = ((30.0_f32 / zoom).max(1e-6).ln() / 5f32.ln()).floor();
        let step = 5f32.powf(k0);                                     // finest level
        if step * zoom < 9.0 { 5f32.powf(k0 + 1.0) } else { step }   // too dense to draw → next level up
    }

    /// Equal-spacing ("snap to gaps"): along `axis` (0 = X, 1 = Y), for the moving span `[m0,m1]` in the
    /// perpendicular band `[p0,p1]`, find the position that either CENTRES it between two neighbours OR
    /// makes its gap to a neighbour EQUAL an existing gap in that row (so you can extend an even row).
    /// Returns `(delta, guide-bars)` for the best in-tolerance candidate.
    fn equal_gap(&self, axis: usize, m0: f32, m1: f32, p0: f32, p1: f32, tol: f32) -> Option<(f32, Vec<SnapGuide>)> {
        let mut rows: Vec<(f32, f32)> = vec![];
        for pi in 0..self.doc.paths.len() {
            let p = &self.doc.paths[pi];
            if p.hidden || self.objsel.contains(&p.id) { continue; }
            let b = self.doc.outline_bbox(pi);
            let (b0, b1, q0, q1) = if axis == 0 { (b.0, b.2, b.1, b.3) } else { (b.1, b.3, b.0, b.2) };
            if q1 < p0 || q0 > p1 { continue; }            // not in the same row / column
            rows.push((b0, b1));
        }
        if rows.is_empty() { return None; }
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let below = rows.iter().filter(|r| r.1 <= m0 + tol).map(|r| r.1).fold(f32::MIN, f32::max);  // nearest low edge
        let above = rows.iter().filter(|r| r.0 >= m1 - tol).map(|r| r.0).fold(f32::MAX, f32::min);  // nearest high edge
        let len = m1 - m0;
        let mut gaps: Vec<f32> = vec![];
        for w in rows.windows(2) { let g = w[1].0 - w[0].1; if g > 0.5 { gaps.push(g); } }
        let mut cands: Vec<(f32, Vec<(f32, f32)>)> = vec![];                 // (target_m0, bar segments)
        if below > f32::MIN && above < f32::MAX {
            let t = below + ((above - below) - len) * 0.5;                  // centre between two neighbours
            cands.push((t, vec![(below, t), (t + len, above)]));
        }
        for &g in &gaps {                                                   // match an existing gap (extend the row)
            if below > f32::MIN { let t = below + g; cands.push((t, vec![(below, t)])); }
            if above < f32::MAX { let t = above - g - len; cands.push((t, vec![(t + len, above)])); }
        }
        let mut best: Option<(f32, Vec<(f32, f32)>)> = None;
        for (t, segs) in cands {
            let diff = t - m0;
            if diff.abs() <= tol && best.as_ref().map_or(true, |(bd, _)| diff.abs() < bd.abs()) { best = Some((diff, segs)); }
        }
        best.map(|(diff, segs)| {
            let pm = (p0 + p1) * 0.5;
            let guides = segs.into_iter().map(|(a, b)| if axis == 0 { SnapGuide::Gap { a: [a, pm], b: [b, pm] } } else { SnapGuide::Gap { a: [pm, a], b: [pm, b] } }).collect();
            (diff, guides)
        })
    }

    /// Snap a TRANSLATION drag (Object move): given the selection's pre-drag bbox and the proposed delta,
    /// return the adjusted delta + the alignment guides + a live HUD label. Pure (reads doc + ppu + cfg).
    /// Tolerance is SCREEN px (÷ ppu) so it feels identical at every zoom. No-op delta when snapping is off.
    pub fn snap_move(&self, bbox0: (f32, f32, f32, f32), d: Pt) -> (Pt, Vec<SnapGuide>, Option<(Pt, String)>) {
        let cfg = &self.doc.snap;
        let hud_at = self.cursor;
        let hud = |nx: f32, ny: f32| Some((hud_at, format!("X {:.0}   Y {:.0}", nx, ny)));
        if !cfg.enabled || !cfg.smart {
            return (d, vec![], hud(bbox0.0 + d[0], bbox0.1 + d[1]));
        }
        let tol = cfg.radius_px / self.ppu.max(1e-4);
        let (mx0, my0, mx1, my1) = (bbox0.0 + d[0], bbox0.1 + d[1], bbox0.2 + d[0], bbox0.3 + d[1]);
        let mxs = [mx0, (mx0 + mx1) * 0.5, mx1];   // moving left / centre / right
        let mys = [my0, (my0 + my1) * 0.5, my1];   // moving top / middle / bottom
        let (txl, tyl) = self.snap_target_lines();
        // best edge/centre snap per axis = smallest in-tolerance offset (axes are independent)
        let mut bx: Option<(f32, f32, f32, f32)> = None; let mut bx_center = false;   // (diff, coord, span_lo, span_hi)
        for (mi, &mx) in mxs.iter().enumerate() { for &(x, s0, s1) in &txl {
            let diff = x - mx;
            if diff.abs() <= tol && bx.map_or(true, |(bd, _, _, _)| diff.abs() < bd.abs()) {
                bx = Some((diff, x, my0.min(s0), my1.max(s1))); bx_center = mi == 1;
            }
        }}
        let mut by: Option<(f32, f32, f32, f32)> = None; let mut by_center = false;
        for (mi, &my) in mys.iter().enumerate() { for &(y, s0, s1) in &tyl {
            let diff = y - my;
            if diff.abs() <= tol && by.map_or(true, |(bd, _, _, _)| diff.abs() < bd.abs()) {
                by = Some((diff, y, mx0.min(s0), mx1.max(s1))); by_center = mi == 1;
            }
        }}
        let mut nd = d;
        let mut guides = vec![];
        let (mut sx_done, mut sy_done) = (false, false);
        if let Some((diff, x, s0, s1)) = bx { nd[0] += diff; sx_done = true; if cfg.alignment_guides { guides.push(SnapGuide::Line { a: [x, s0], b: [x, s1] }); } }
        if let Some((diff, y, s0, s1)) = by { nd[1] += diff; sy_done = true; if cfg.alignment_guides { guides.push(SnapGuide::Line { a: [s0, y], b: [s1, y] }); } }
        // centre-to-centre: both axes snapped via the moving CENTRE → mark the shared point, so you can tell
        // two objects sit on the same spot (Illustrator's snap indicator).
        if bx_center && by_center { if let (Some((_, x, _, _)), Some((_, y, _, _))) = (bx, by) {
            let r = 5.0;
            guides.push(SnapGuide::Line { a: [x - r, y], b: [x + r, y] });
            guides.push(SnapGuide::Line { a: [x, y - r], b: [x, y + r] });
        }}
        // equal-spacing ("snap to gaps") — only on an axis the edge/centre snap didn't already claim
        if cfg.gaps_and_sizes && cfg.equal_spacing {
            if !sx_done { if let Some((diff, mut g)) = self.equal_gap(0, mx0, mx1, my0, my1, tol) { nd[0] += diff; sx_done = true; guides.append(&mut g); } }
            if !sy_done { if let Some((diff, mut g)) = self.equal_gap(1, my0, my1, mx0, mx1, tol) { nd[1] += diff; sy_done = true; guides.append(&mut g); } }
        }
        // grid fallback (lowest priority) — snap the top-left edge to the VISIBLE adaptive dot grid
        if cfg.grid && (!sx_done || !sy_done) {
            let step = self.adaptive_grid_step();
            if !sx_done { nd[0] += (mx0 / step).round() * step - mx0; }
            if !sy_done { nd[1] += (my0 / step).round() * step - my0; }
        }
        (nd, guides, hud(bbox0.0 + nd[0], bbox0.1 + nd[1]))
    }

    /// Snap a single world point's X and/or Y to target lines (object/artboard edges & centres), with a
    /// grid / pixel fallback. Returns the snapped point + alignment guides. Drives the resize (Scale) handle.
    pub fn snap_xy(&self, w: Pt, want_x: bool, want_y: bool) -> (Pt, Vec<SnapGuide>) {
        let cfg = &self.doc.snap;
        if !cfg.enabled || !cfg.smart { return (w, vec![]); }
        let tol = cfg.radius_px / self.ppu.max(1e-4);
        let (txl, tyl) = self.snap_target_lines();
        let (mut nx, mut ny) = (w[0], w[1]);
        let mut guides = vec![];
        if want_x {
            let mut best: Option<(f32, f32, f32, f32)> = None;
            for &(x, s0, s1) in &txl { let diff = x - w[0]; if diff.abs() <= tol && best.map_or(true, |(bd, _, _, _)| diff.abs() < bd.abs()) { best = Some((diff, x, s0, s1)); } }
            if let Some((_, x, s0, s1)) = best { nx = x; if cfg.alignment_guides { guides.push(SnapGuide::Line { a: [x, s0.min(w[1])], b: [x, s1.max(w[1])] }); } }
            else if cfg.grid { let step = self.adaptive_grid_step(); nx = (w[0] / step).round() * step; }
        }
        if want_y {
            let mut best: Option<(f32, f32, f32, f32)> = None;
            for &(y, s0, s1) in &tyl { let diff = y - w[1]; if diff.abs() <= tol && best.map_or(true, |(bd, _, _, _)| diff.abs() < bd.abs()) { best = Some((diff, y, s0, s1)); } }
            if let Some((_, y, s0, s1)) = best { ny = y; if cfg.alignment_guides { guides.push(SnapGuide::Line { a: [s0.min(w[0]), y], b: [s1.max(w[0]), y] }); } }
            else if cfg.grid { let step = self.adaptive_grid_step(); ny = (w[1] / step).round() * step; }
        }
        ([nx, ny], guides)
    }

    /// Snap a FREE point — the ruler origin (0,0) being dragged from the corner — onto artboard
    /// corners/edges/centre, object bbox features, every object ANCHOR point, then the grid: per axis,
    /// within the screen-constant tolerance. Gated on the snap master only (works with Smart Guides off),
    /// so the zero-point lands crisply on a page corner, an anchor, or a grid dot — never on a stray pixel.
    pub fn snap_origin(&self, p: Pt) -> Pt {
        let cfg = &self.doc.snap;
        if !cfg.enabled { return p; }
        let tol = cfg.radius_px / self.ppu.max(1e-4);
        let (mut xs, mut ys): (Vec<f32>, Vec<f32>) = (vec![], vec![]);
        if let Some(ab) = self.doc.active_artboard() {
            let (x0, y0, x1, y1) = ab.rect();
            xs.extend([x0, x1, (x0 + x1) * 0.5]); ys.extend([y0, y1, (y0 + y1) * 0.5]);
        }
        for pi in 0..self.doc.paths.len() {
            let pp = &self.doc.paths[pi];
            if pp.hidden { continue; }
            let b = self.doc.outline_bbox(pi);
            xs.extend([b.0, b.2, (b.0 + b.2) * 0.5]); ys.extend([b.1, b.3, (b.1 + b.3) * 0.5]);
            for a in pp.anchors.iter().chain(pp.holes.iter().flatten()) { xs.push(a.p[0]); ys.push(a.p[1]); }
        }
        let pick = |v: f32, cands: &[f32]| -> Option<f32> {
            cands.iter().copied().filter(|c| (c - v).abs() <= tol)
                .min_by(|a, b| (a - v).abs().partial_cmp(&(b - v).abs()).unwrap_or(std::cmp::Ordering::Equal))
        };
        let mut nx = pick(p[0], &xs);
        let mut ny = pick(p[1], &ys);
        let step = self.adaptive_grid_step();   // grid fallback → the origin lands on the dots the ruler follows
        if nx.is_none() { let g = (p[0] / step).round() * step; if (g - p[0]).abs() <= tol { nx = Some(g); } }
        if ny.is_none() { let g = (p[1] / step).round() * step; if (g - p[1]).abs() <= tol { ny = Some(g); } }
        [nx.unwrap_or(p[0]), ny.unwrap_or(p[1])]
    }

    /// Snap the transform PIVOT (Rotate/Scale/Reflect origin). Prefers landing on a whole feature POINT —
    /// an object anchor, a bbox corner, a centre (object or page) — within tolerance, so it sits exactly on
    /// the point you aimed at; else falls back to the per-axis edge/grid snap (`snap_origin`).
    pub fn snap_pivot(&self, p: Pt) -> Pt {
        let cfg = &self.doc.snap;
        if !cfg.enabled { return p; }
        let tol = cfg.radius_px / self.ppu.max(1e-4);
        let mut pts: Vec<Pt> = vec![];
        if let Some(ab) = self.doc.active_artboard() {
            let (x0, y0, x1, y1) = ab.rect();
            pts.extend([[x0, y0], [x1, y0], [x1, y1], [x0, y1], [(x0 + x1) * 0.5, (y0 + y1) * 0.5]]);
        }
        for pi in 0..self.doc.paths.len() {
            let pp = &self.doc.paths[pi]; if pp.hidden { continue; }
            let b = self.doc.outline_bbox(pi);
            pts.extend([[b.0, b.1], [b.2, b.1], [b.2, b.3], [b.0, b.3], [(b.0 + b.2) * 0.5, (b.1 + b.3) * 0.5]]);
            for a in pp.anchors.iter().chain(pp.holes.iter().flatten()) { pts.push(a.p); }
        }
        let mut best: Option<(f32, Pt)> = None;
        for q in pts { let d = dist(p, q); if d <= tol && best.map_or(true, |(bd, _)| d < bd) { best = Some((d, q)); } }
        if let Some((_, q)) = best { return q; }
        self.snap_origin(p)   // no whole point in range → per-axis edges + grid
    }

    // ---------- ruler guides ----------
    /// Nearest guide line within the screen-constant grab tolerance, or None (also None when hidden/locked).
    pub fn guide_at(&self, pos: Pt) -> Option<usize> {
        if self.guides_hidden || self.doc.guides_locked { return None; }
        let tol = EDGE_R / self.ppu.max(1e-4);
        let mut best: Option<(f32, usize)> = None;
        for (i, g) in self.doc.guides.iter().enumerate() {
            let d = if g.vertical { (g.pos - pos[0]).abs() } else { (g.pos - pos[1]).abs() };
            if d <= tol && best.map_or(true, |(bd, _)| d < bd) { best = Some((d, i)); }
        }
        best.map(|(_, i)| i)
    }
    /// Snap ONE guide coordinate to artboard / object features / grid on its axis (never to a guide itself).
    fn snap_axis(&self, v: f32, vertical: bool) -> f32 {
        let cfg = &self.doc.snap;
        if !cfg.enabled { return v; }
        let tol = cfg.radius_px / self.ppu.max(1e-4);
        let mut cands: Vec<f32> = vec![];
        if let Some(ab) = self.doc.active_artboard() {
            let (x0, y0, x1, y1) = ab.rect();
            if vertical { cands.extend([x0, x1, (x0 + x1) * 0.5]); } else { cands.extend([y0, y1, (y0 + y1) * 0.5]); }
        }
        for pi in 0..self.doc.paths.len() {
            let p = &self.doc.paths[pi]; if p.hidden { continue; }
            let b = self.doc.outline_bbox(pi);
            if vertical { cands.extend([b.0, b.2, (b.0 + b.2) * 0.5]); } else { cands.extend([b.1, b.3, (b.1 + b.3) * 0.5]); }
            for a in p.anchors.iter().chain(p.holes.iter().flatten()) { cands.push(if vertical { a.p[0] } else { a.p[1] }); }
        }
        let mut best: Option<(f32, f32)> = None;
        for c in cands { let d = (c - v).abs(); if d <= tol && best.map_or(true, |(bd, _)| d < bd) { best = Some((d, c)); } }
        if let Some((_, c)) = best { return c; }
        let s = self.adaptive_grid_step(); let g = (v / s).round() * s; if (g - v).abs() <= tol { return g; }
        v
    }
    /// Ruler drag-out: set the live (snapped) preview of a NEW guide. `vertical` ⇒ from the LEFT ruler.
    pub fn set_guide_preview(&mut self, vertical: bool, world: Pt) {
        let v = if vertical { world[0] } else { world[1] };
        self.guide_preview = Some(Guide { vertical, pos: self.snap_axis(v, vertical) });
    }
    /// Drop the previewed guide into the document (undoable). No-op if there's no preview.
    pub fn commit_guide(&mut self) {
        if let Some(g) = self.guide_preview.take() { self.begin(); self.doc.guides.push(g); self.dirty = true; self.commit(); }
    }
    /// Cancel an in-progress ruler drag-out without placing a guide.
    pub fn cancel_guide(&mut self) { self.guide_preview = None; }

    /// Core GEOMETRY point-snap: find the nearest snap of any moving point (offset by `d`) onto a target
    /// anchor (key point), segment midpoint, or the nearest point on a path edge (vector geometry), within
    /// `tol`. Skips hidden paths, the moving object selection, and the path currently being edited (the one
    /// holding a selected anchor). Returns `(target, moved-point)` of the best snap.
    fn snap_points(&self, base_pts: &[Pt], d: Pt, tol: f32) -> Option<(Pt, Pt)> {
        let cfg = &self.doc.snap;
        let mut best: Option<(f32, Pt, Pt)> = None;        // (dist, target, moved)
        for mp in base_pts {
            let moved = add(*mp, d);
            for pi in 0..self.doc.paths.len() {
                let p = &self.doc.paths[pi];
                if p.hidden || self.objsel.contains(&p.id) { continue; }
                if p.anchors.iter().chain(p.holes.iter().flatten()).any(|a| self.selected.contains(&a.id)) { continue; }
                if cfg.key_points {
                    for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
                        let dd = dist(moved, a.p);
                        if dd <= tol && best.map_or(true, |(bd, _, _)| dd < bd) { best = Some((dd, a.p, moved)); }
                    }
                }
                if cfg.segment_mids {
                    let n = p.anchors.len();
                    let segs = if p.closed { n } else { n.saturating_sub(1) };
                    for i in 0..segs {
                        let (q, r) = (p.anchors[i].p, p.anchors[(i + 1) % n].p);
                        let mid = [(q[0] + r[0]) * 0.5, (q[1] + r[1]) * 0.5];
                        let dd = dist(moved, mid);
                        if dd <= tol && best.map_or(true, |(bd, _, _)| dd < bd) { best = Some((dd, mid, moved)); }
                    }
                }
                if cfg.object_geometry {
                    if let Some((si, t, dd)) = self.doc.nearest_seg(pi, moved) {
                        if dd <= tol && best.map_or(true, |(bd, _, _)| dd < bd) {
                            let n = p.anchors.len();
                            let a = &p.anchors[si]; let b = &p.anchors[(si + 1) % n];
                            let pt = cubic(a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p, t);
                            best = Some((dd, pt, moved));
                        }
                    }
                }
            }
        }
        best.map(|(_, t, moved)| (t, moved))
    }

    /// Geometry point-snap wrapper → `(delta-adjust, cross marker, target)`. Off when snapping or all three
    /// geometry sources are disabled.
    fn snap_to_points(&self, base_pts: &[Pt], d: Pt) -> Option<(Pt, Vec<SnapGuide>, Pt)> {
        let cfg = &self.doc.snap;
        // Point/geometry snapping is INDEPENDENT of Smart Guides (Illustrator's "Snap to Point" is its own
        // toggle, separate from alignment guides) — gate only on the master + the geometry sources.
        if !cfg.enabled || base_pts.is_empty() || (!cfg.key_points && !cfg.object_geometry && !cfg.segment_mids) { return None; }
        let tol = cfg.radius_px / self.ppu.max(1e-4);
        self.snap_points(base_pts, d, tol).map(|(t, moved)| (sub(t, moved), vec![SnapGuide::Point { p: t }], t))
    }

    /// Nearest point on path `pi`'s edges to `pos`, SKIPPING any segment that touches a currently-selected
    /// (dragged) anchor — so a dragged point/handle can snap to OTHER parts of the SAME path (incl. curves),
    /// not where it already sits. Returns (point, dist). Samples each cubic so curves snap, not just nodes.
    fn nearest_edge(&self, pi: usize, pos: Pt) -> Option<(Pt, f32)> {
        let p = &self.doc.paths[pi];
        let n = p.anchors.len();
        if n < 2 { return None; }
        let segs = if p.closed { n } else { n - 1 };
        let mut best: Option<(Pt, f32)> = None;
        for i in 0..segs {
            let (a, b) = (&p.anchors[i], &p.anchors[(i + 1) % n]);
            if self.selected.contains(&a.id) || self.selected.contains(&b.id) { continue; }
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            for k in 0..=24 {
                let pt = cubic(p0, p1, p2, p3, k as f32 / 24.0);
                let dd = dist(pt, pos);
                if best.map_or(true, |(_, bd)| dd < bd) { best = Some((pt, dd)); }
            }
        }
        best
    }

    /// Snap an ANCHOR drag (Direct/Pen) — the primary moving point. (1) An exact landing on the nearest
    /// anchor / segment-mid / path edge within tolerance → snap onto it + a marker. Otherwise (2) per-axis
    /// SMART GUIDES: align X (and/or Y) to the nearest other anchor — INCLUDING the same shape's other
    /// corners — or the artboard, drawing the guide line; (3) grid is the fallback. Independent of Smart
    /// Guides (this is "Snap to Point"); only the dragged anchors themselves are excluded as targets.
    pub fn snap_anchor(&self, base_pts: &[Pt], d: Pt) -> (Pt, Vec<SnapGuide>, Option<(Pt, String)>) {
        let cfg = &self.doc.snap;
        let first = base_pts.first().copied().unwrap_or(self.cursor);
        let hud = |p: Pt| Some((self.cursor, format!("X {:.0}   Y {:.0}", p[0], p[1])));
        if !cfg.enabled || base_pts.is_empty() { return (d, vec![], hud(add(first, d))); }
        let tol = cfg.radius_px / self.ppu.max(1e-4);
        let moved = add(first, d);

        // candidate target POINTS: every OTHER anchor (incl. the same shape's other corners, only the
        // dragged anchors excluded) + segment midpoints + the artboard corners/centre.
        let mut pts: Vec<Pt> = vec![];
        for p in &self.doc.paths {
            if p.hidden { continue; }
            if cfg.key_points {
                for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
                    if self.selected.contains(&a.id) { continue; }
                    pts.push(a.p);
                }
            }
            if cfg.segment_mids && !p.anchors.iter().any(|a| self.selected.contains(&a.id)) {
                let n = p.anchors.len();
                let segs = if p.closed { n } else { n.saturating_sub(1) };
                for i in 0..segs { let (q, r) = (p.anchors[i].p, p.anchors[(i + 1) % n].p); pts.push([(q[0] + r[0]) * 0.5, (q[1] + r[1]) * 0.5]); }
            }
        }
        if cfg.artboard { if let Some(ab) = self.doc.active_artboard() {
            let (x0, y0, x1, y1) = ab.rect();
            for c in [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [(x0 + x1) * 0.5, (y0 + y1) * 0.5]] { pts.push(c); }
        }}

        // 1) exact landing on the nearest point / edge (both axes) within tolerance
        let mut best: Option<(f32, Pt)> = None;
        let mut best_pid: Option<u32> = None;   // set only when the winner is a path EDGE (→ highlight it)
        for &tp in &pts { let dd = dist(moved, tp); if dd <= tol && best.map_or(true, |(bd, _)| dd < bd) { best = Some((dd, tp)); best_pid = None; } }
        if cfg.object_geometry {
            for pi in 0..self.doc.paths.len() {
                if self.doc.paths[pi].hidden { continue; }
                if let Some((pt, dd)) = self.nearest_edge(pi, moved) {
                    if dd <= tol && best.map_or(true, |(bd, _)| dd < bd) { best = Some((dd, pt)); best_pid = Some(self.doc.paths[pi].id); }
                }
            }
        }
        if let Some((_, t)) = best {
            let mut g = vec![SnapGuide::Point { p: t }];
            if let Some(pid) = best_pid { g.push(SnapGuide::PathHi { pid }); }   // light up the whole snapped path
            return (add(d, sub(t, moved)), g, hud(t));
        }

        // 2) per-axis smart-guide alignment to the nearest target X / Y
        let mut nd = d; let mut guides = vec![]; let (mut sx, mut sy) = (false, false);
        let mut bx: Option<(f32, Pt)> = None;
        for &tp in &pts { let diff = tp[0] - moved[0]; if diff.abs() <= tol && bx.map_or(true, |(bd, _)| diff.abs() < bd.abs()) { bx = Some((diff, tp)); } }
        if let Some((diff, tp)) = bx { nd[0] += diff; sx = true; if cfg.alignment_guides { guides.push(SnapGuide::Line { a: [tp[0], tp[1].min(moved[1])], b: [tp[0], tp[1].max(moved[1])] }); } }
        let mut by: Option<(f32, Pt)> = None;
        for &tp in &pts { let diff = tp[1] - moved[1]; if diff.abs() <= tol && by.map_or(true, |(bd, _)| diff.abs() < bd.abs()) { by = Some((diff, tp)); } }
        if let Some((diff, tp)) = by { nd[1] += diff; sy = true; if cfg.alignment_guides { guides.push(SnapGuide::Line { a: [tp[0].min(moved[0]), tp[1]], b: [tp[0].max(moved[0]), tp[1]] }); } }

        // 3) grid fallback
        if cfg.grid {
            let step = self.adaptive_grid_step();
            if !sx { nd[0] += (moved[0] / step).round() * step - moved[0]; }
            if !sy { nd[1] += (moved[1] / step).round() * step - moved[1]; }
        }
        (nd, guides, hud(add(first, nd)))
    }

    // ---------- history ----------
    pub fn begin(&mut self) { self.pending = Some(self.doc.clone()); self.dirty = false; }
    pub fn commit(&mut self) {
        self.doc.sync_tree();   // adopt new paths / prune dead + empty nodes / re-flatten z
        if self.dirty { if let Some(p) = self.pending.take() { self.undo.push(p); if self.undo.len() > 200 { self.undo.remove(0); } self.redo.clear(); self.rev += 1; } }
        self.pending = None; self.dirty = false;
    }
    pub fn undo(&mut self) { if let Some(s) = self.undo.pop() { self.redo.push(self.doc.clone()); self.doc = s; self.clear_transient(); self.rev += 1; } }
    pub fn redo(&mut self) { if let Some(s) = self.redo.pop() { self.undo.push(self.doc.clone()); self.doc = s; self.clear_transient(); self.rev += 1; } }
    fn clear_transient(&mut self) { self.selected.clear(); self.objsel.clear(); self.dsel_path = None; self.active = None; self.drag = Drag::None; }
    /// Swap in a freshly-loaded document (File ▸ Open): history, gesture and every transient selection
    /// state reset — the new file starts clean, on the same tool.
    pub fn replace_doc(&mut self, doc: Document) {
        self.doc = doc;
        self.doc.sync_tree();   // migrate legacy registries / adopt tree-less paths (old files)
        self.undo.clear(); self.redo.clear(); self.pending = None;
        self.clear_transient();
        self.pivot = None; self.hover_path = None; self.guide_preview = None; self.origin_preview = None;
        self.snap_guides.clear(); self.snap_hud = None; self.obj_angle = 0.0;
        self.rev += 1;
    }

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
        // grabbing a SEGMENT selects its two bordering anchors so their handles appear (Illustrator)
        self.selected.clear(); self.selected.insert(a.id); self.selected.insert(b.id);
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
        if self.tool == ToolKind::Artboard { return ToolKind::Artboard; }   // Artboard tool never morphs
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
        self.gesture_copy = false; self.gesture_delta = [0.0, 0.0]; self.gesture_tf = None;
        self.gesture = self.eff_tool();
        if self.gesture == ToolKind::Artboard { self.ab_down(pos); return; }
        // grab a ruler guide first (Selection / Direct tools) — drag to reposition it
        if matches!(self.gesture, ToolKind::Object | ToolKind::Direct) {
            if let Some(idx) = self.guide_at(pos) { self.drag = Drag::Guide { idx }; return; }
        }
        // Pen: snap the new anchor onto nearby points / path / grid before placing it
        let pos = if self.gesture == ToolKind::Pen { let (adj, _, _) = self.snap_anchor(&[pos], [0.0, 0.0]); add(pos, adj) } else { pos };
        self.cursor = pos;
        tools::get(self.gesture).down(self, pos);
    }
    pub fn pointer_up(&mut self) {
        self.snap_guides.clear(); self.snap_hud = None;   // snap feedback is per-gesture
        if self.tool == ToolKind::Artboard { self.ab_up(); self.commit(); return; }
        if let Drag::Shape { pid, .. } = self.drag {
            if let Some(pi) = self.doc.pidx(pid) { let b = self.doc.bbox(pi); if (b.2-b.0) < 2.0 && (b.3-b.1) < 2.0 { self.doc.paths.remove(pi); self.dirty = false; } }
        }
        if let Drag::PenClose { .. } = self.drag { self.active = None; }
        if matches!(self.drag, Drag::Object { .. }) && (self.gesture_delta[0] != 0.0 || self.gesture_delta[1] != 0.0) {
            self.last_tf = Some((TfAgain::Move(self.gesture_delta), self.gesture_copy));   // remember for Transform Again
        }
        if let Some(tf) = self.gesture_tf.take() { self.last_tf = Some((tf, self.gesture_copy)); }   // rotate/scale/reflect
        if let Drag::TfPending { down, .. } = self.drag {   // a click (no drag) relocates the pivot — snapped
            self.pivot = Some(self.snap_pivot(down));        // lands on an anchor / corner / centre / edge / grid
            self.dirty = false;
        }
        self.drag = Drag::None;
        self.commit();
    }

    pub fn pointer_move(&mut self, pos: Pt) {
        self.cursor = pos;
        if self.tool == ToolKind::Artboard {
            if !matches!(self.ab_drag, AbDrag::None) { self.ab_move(pos); }
            return;
        }
        if matches!(self.drag, Drag::None) { self.hover_path = self.path_under(pos); }
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::PenNew { aid, down, mut broken } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if self.mods.alt { broken = true; }
                    if let Some((pi, ai)) = self.doc.aidx(aid) {
                        let p = self.doc.paths[pi].anchors[ai].p;
                        let mut q = if self.mods.shift { add(p, snap45(sub(pos, p))) } else { pos };
                        let (adj, guides, hud) = self.snap_anchor(&[q], [0.0, 0.0]);   // snap the handle being pulled
                        q = add(q, adj); self.snap_guides = guides; self.snap_hud = hud;
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
                        let (adj, guides, hud) = self.snap_anchor(&[pos], [0.0, 0.0]);   // snap the closing handle
                        let q = add(pos, adj); self.snap_guides = guides; self.snap_hud = hud;
                        let a = &mut self.doc.paths[pi].anchors[ai];
                        a.hout = Some(q);
                        if broken { a.smooth = false; } else { a.smooth = true; a.hin = Some(mirror(p, q)); }
                    }
                }
                self.drag = Drag::PenClose { aid, down, broken };
            }
            Drag::Anchors { start, items } => {
                let mut d = sub(pos, start);
                if self.mods.shift { d = snap45(d); }
                let base_pts: Vec<Pt> = items.iter().map(|(_, p0, _, _)| *p0).collect();
                let (d, guides, hud) = self.snap_anchor(&base_pts, d);   // Direct/Pen → snap to points/edges (always active)
                self.snap_guides = guides; self.snap_hud = hud;
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
                    // snap the bezier handle to nearby anchors / path / grid (like any other point)
                    let (adj, guides, hud) = self.snap_anchor(&[q], [0.0, 0.0]);
                    q = add(q, adj); self.snap_guides = guides; self.snap_hud = hud;
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
            Drag::Guide { idx } => {
                if let Some(g) = self.doc.guides.get(idx).copied() {
                    let v = self.snap_axis(if g.vertical { pos[0] } else { pos[1] }, g.vertical);
                    if let Some(gm) = self.doc.guides.get_mut(idx) { gm.pos = v; }
                    let o = self.doc.ruler_origin;
                    self.snap_hud = Some((pos, if g.vertical { format!("X {:.0}", v - o[0]) } else { format!("Y {:.0}", v - o[1]) }));
                }
                self.drag = Drag::Guide { idx };
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
                let inside = |p: Pt| p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1;
                let mut sel: HashSet<u32> = base.iter().copied().collect();
                for p in &self.doc.paths {
                    if p.hidden { continue; }
                    // anchors whose POINT lands inside the marquee
                    for a in p.anchors.iter().chain(p.holes.iter().flatten()) { if inside(a.p) { sel.insert(a.id); } }
                    // …plus the two endpoints of any SEGMENT the marquee crosses, so their handles appear when
                    // the rect catches only the curve between anchors (Illustrator feel). ANCHOR PRIORITY: skip
                    // a segment if either endpoint is already inside — so a tight marquee on ONE anchor selects
                    // just it, never dragging in its neighbours via the two adjacent segments.
                    for (ring, closed) in std::iter::once((&p.anchors, p.closed)).chain(p.holes.iter().map(|h| (h, true))) {
                        let n = ring.len(); if n < 2 { continue; }
                        let segs = if closed { n } else { n - 1 };
                        for i in 0..segs {
                            let a = &ring[i]; let b = &ring[(i + 1) % n];
                            if inside(a.p) || inside(b.p) { continue; }   // an endpoint is captured → anchor wins
                            let (q0, q1, q2, q3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
                            if (0..=16).any(|k| inside(cubic(q0, q1, q2, q3, k as f32 / 16.0))) { sel.insert(a.id); sel.insert(b.id); }
                        }
                    }
                }
                self.selected = sel;
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
                    self.gesture_copy = true;                       // remember this gesture left a copy (Transform Again)
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
                // snap priority: the object's own anchors → vector geometry (most specific), else the bbox
                // edges/centres/spacing/grid. Always active per the magnet toggles (Ctrl only morphs the tool).
                let base_pts: Vec<Pt> = base.iter().map(|(_, p0, _, _)| *p0).collect();
                let (d, guides, hud) = if let Some((adj, g, t)) = self.snap_to_points(&base_pts, d) { (add(d, adj), g, Some((self.cursor, format!("X {:.0}   Y {:.0}", t[0], t[1])))) }
                    else { self.snap_move(base_bbox(&base), d) };
                self.snap_guides = guides; self.snap_hud = hud; self.gesture_delta = d;
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
                // snap the dragged handle to other geometry / grid (axis-aligned frames only)
                let (sp, guides) = if angle.abs() < 1e-3 { self.snap_xy(pos, cx, cy) } else { (pos, vec![]) };
                self.snap_guides = guides;
                let lp = rotate_about(sp, [0.0, 0.0], -angle);              // (snapped) cursor → local space
                let (dx0, dy0) = (h0_l[0] - pivot[0], h0_l[1] - pivot[1]);
                let mut sx = if cx && dx0.abs() > 1e-3 { (lp[0]-pivot[0])/dx0 } else { 1.0 };
                let mut sy = if cy && dy0.abs() > 1e-3 { (lp[1]-pivot[1])/dy0 } else { 1.0 };
                if self.mods.shift && cx && cy { let m = sx.abs().max(sy.abs()); sx = m.copysign(sx); sy = m.copysign(sy); }
                // live W×H readout (local bbox of the base × the scale)
                let (mut lx0, mut ly0, mut lx1, mut ly1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for (_, p0, _, _) in &base { let q = rotate_about(*p0, [0.0, 0.0], -angle); lx0 = lx0.min(q[0]); ly0 = ly0.min(q[1]); lx1 = lx1.max(q[0]); ly1 = ly1.max(q[1]); }
                self.snap_hud = Some((self.cursor, format!("W {:.0}   H {:.0}", (lx1 - lx0) * sx.abs(), (ly1 - ly0) * sy.abs())));
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
                self.snap_hud = Some((pos, format!("{:.1}\u{b0}", -d.to_degrees())));   // CCW-positive (Illustrator)
                self.gesture_tf = Some(TfAgain::Rotate { pivot: center, ang: d });
                self.drag = Drag::Rotate { center, start, a0, base };
                self.dirty = true;
            }
            Drag::TfPending { pivot, down } => {
                if dist(pos, down) >= DRAG_THRESH {
                    if self.mods.alt {                       // Alt-drag transforms a COPY (Illustrator)
                        let srcs: Vec<u32> = self.objsel.iter().copied().collect();
                        let cids = self.doc.dup_paths(&srcs);
                        self.gesture_copy = true;
                        self.objsel.clear();
                        for cid in cids { self.objsel.insert(cid); }
                    }
                    let base = self.objsel_base();
                    self.drag = match self.tool {
                        ToolKind::Scale => Drag::ScaleLive { pivot, down, base },
                        _ => { let start = (down[1] - pivot[1]).atan2(down[0] - pivot[0]);
                               Drag::Rotate { center: pivot, start, a0: self.obj_angle, base } }
                    };
                    self.pointer_move(pos);                  // apply this frame's transform immediately
                } else {
                    self.drag = Drag::TfPending { pivot, down };
                }
            }
            Drag::ScaleLive { pivot, down, base } => {
                let (dx0, dy0) = (down[0] - pivot[0], down[1] - pivot[1]);
                let (mut sx, mut sy) = (if dx0.abs() > 1e-3 { (pos[0]-pivot[0])/dx0 } else { 1.0 },
                                        if dy0.abs() > 1e-3 { (pos[1]-pivot[1])/dy0 } else { 1.0 });
                if self.mods.shift {   // uniform: project the drag onto the grab direction (signed)
                    let den = dx0*dx0 + dy0*dy0;
                    let s = if den > 1e-6 { ((pos[0]-pivot[0])*dx0 + (pos[1]-pivot[1])*dy0) / den } else { 1.0 };
                    sx = s; sy = s;
                }
                let sc = |p: Pt| [pivot[0] + (p[0]-pivot[0])*sx, pivot[1] + (p[1]-pivot[1])*sy];
                for (aid, p0, hin0, hout0) in &base {
                    if let Some(a) = self.doc.anchor_mut(*aid) { a.p = sc(*p0); a.hin = hin0.map(sc); a.hout = hout0.map(sc); }
                }
                self.snap_hud = Some((pos, format!("{:.0}%   {:.0}%", sx*100.0, sy*100.0)));
                self.gesture_tf = Some(TfAgain::Scale { pivot, sx, sy });
                self.drag = Drag::ScaleLive { pivot, down, base };
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
        if t == ToolKind::Artboard {
            // entering the Artboard tool drops artwork selection so the page chrome stands alone
            self.selected.clear(); self.objsel.clear(); self.dsel_path = None;
            self.obj_angle = 0.0; self.hover_path = None;
        }
        if matches!(t, ToolKind::Rotate | ToolKind::Scale) && self.objsel.is_empty() {
            // the transform tools act on whole objects — promote any anchor selection (coming from Direct)
            let pids: Vec<u32> = self.selected.iter().filter_map(|&aid| self.doc.aidx(aid).map(|(pi, _)| self.doc.paths[pi].id)).collect();
            for pid in pids { for m in self.doc.group_members(pid) { self.objsel.insert(m); } }
            self.selected.clear();
        }
        self.tool = t;
        self.pivot = None;   // each tool entry re-homes the transform origin to the selection centre
        if t != ToolKind::Pen { self.active = None; }
        self.dsel_path = None;
        self.drag = Drag::None;
        self.ab_drag = AbDrag::None;
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
    /// If a guide is being dragged, remove it (drag-to-ruler delete) — undoable. Returns true if it did.
    pub fn delete_dragged_guide(&mut self) -> bool {
        if let Drag::Guide { idx } = self.drag {
            if idx < self.doc.guides.len() { self.doc.guides.remove(idx); }
            self.dirty = true; self.drag = Drag::None; self.snap_hud = None; self.commit();
            return true;
        }
        false
    }
    pub fn double_click(&mut self, pos: Pt) {
        if self.guide_at(pos).is_some() {   // double-click a guide → delete it
            if let Some(idx) = self.guide_at(pos) { self.begin(); self.doc.guides.remove(idx); self.dirty = true; self.commit(); }
            return;
        }
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
        let pids = self.selected_pids();
        if pids.is_empty() { return; }
        self.begin();
        for pid in pids { if let Some(pi) = self.doc.pidx(pid) {
            match self.paint { PaintTarget::Fill => self.doc.paths[pi].fill = color, PaintTarget::Stroke => self.doc.paths[pi].stroke = color }
        }}
        self.dirty = true; self.commit();
    }
    /// Remember an applied colour in the picker's MRU strip: newest first, value-deduped, cap 12.
    pub fn push_recent(&mut self, c: Rgba) {
        let same = |a: &Rgba, b: &Rgba| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-4);
        self.recent_colors.retain(|r| !same(r, &c));
        self.recent_colors.insert(0, c);
        self.recent_colors.truncate(12);
    }
    /// The distinct colours the artwork uses RIGHT NOW (fills + strokes, first-appearance order, cap 12).
    /// Derived on demand — never stored (COLOR_SPEC Stage 1).
    pub fn document_colors(&self) -> Vec<Rgba> {
        let same = |a: &Rgba, b: &Rgba| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-4);
        let mut out: Vec<Rgba> = Vec::new();
        for p in &self.doc.paths {
            for c in [p.fill, p.stroke].into_iter().flatten() {
                if !out.iter().any(|r| same(r, &c)) {
                    out.push(c);
                    if out.len() >= 12 { return out; }
                }
            }
        }
        out
    }
    /// Paths targeted by inspector edits (paint / stroke-weight / opacity): object selection ∪ the paths of
    /// individually-selected anchors ∪ the Direct-tool path-level selection. Missing that last term was the
    /// bug where changing colour / removing stroke did nothing while the Direct-Selection tool was active.
    fn selected_pids(&self) -> HashSet<u32> {
        let mut pids: HashSet<u32> = self.objsel.clone();
        for &aid in &self.selected { if let Some(pid) = self.doc.pid_of_anchor(aid) { pids.insert(pid); } }
        if let Some(pid) = self.dsel_path { pids.insert(pid); }
        pids
    }
    /// The path whose fill/stroke/weight/opacity the inspector should DISPLAY (first of the same set).
    pub fn repr_path(&self) -> Option<usize> {
        self.objsel.iter().copied()
            .chain(self.dsel_path)
            .chain(self.selected.iter().filter_map(|&aid| self.doc.pid_of_anchor(aid)))
            .filter_map(|pid| self.doc.pidx(pid))
            .next()
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
    /// Object-level opacity (0..1) on the current selection (object sel ∪ paths of selected anchors).
    pub fn set_opacity(&mut self, o: f32) {
        let pids = self.selected_pids(); if pids.is_empty() { return; }
        let o = o.clamp(0.0, 1.0);
        self.begin();
        for q in pids { if let Some(pi) = self.doc.pidx(q) { self.doc.paths[pi].opacity = o; } }
        self.dirty = true; self.commit();
    }
    /// Layers: toggle a path's visibility. Hiding it drops it from the object selection.
    pub fn set_hidden(&mut self, pid: u32, hidden: bool) {
        self.begin();
        if let Some(pi) = self.doc.pidx(pid) { self.doc.paths[pi].hidden = hidden; }
        if hidden { self.objsel.remove(&pid); }
        self.dirty = true; self.commit();
    }
    /// Layers: toggle a path's lock. Locking it drops it from the object selection.
    pub fn set_locked(&mut self, pid: u32, locked: bool) {
        self.begin();
        if let Some(pi) = self.doc.pidx(pid) { self.doc.paths[pi].locked = locked; }
        if locked { self.objsel.remove(&pid); }
        self.dirty = true; self.commit();
    }
    /// Layers: inline rename. Empty/blank name clears back to the default label.
    pub fn rename_path(&mut self, pid: u32, name: String) {
        self.begin();
        let n = name.trim();
        if let Some(pi) = self.doc.pidx(pid) { self.doc.paths[pi].name = if n.is_empty() { None } else { Some(n.to_string()) }; }
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
