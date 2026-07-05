//! The document data model: anchors, paths, the document. Plus pure geometry queries.
//! Stable u32 IDs (never Vec indices) so selection/active survive deletes & joins.

use crate::geom::*;
use crate::units::DocUnits;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// Pt = [f32;2] and Rgba = [f32;4] are type aliases for arrays, which serde already supports —
// no derive needed (or possible) on them; they round-trip as JSON number arrays.

pub const K: f32 = 0.5522847; // bezier circle constant (ellipse handles)

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub id: u32,
    pub p: Pt,
    pub hin: Option<Pt>,
    pub hout: Option<Pt>,
    pub smooth: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub fn new(
        id: u32,
        anchors: Vec<Anchor>,
        closed: bool,
        fill: Option<Rgba>,
        stroke: Option<Rgba>,
        stroke_width: f32,
    ) -> Path {
        Path {
            id,
            anchors,
            closed,
            fill,
            stroke,
            stroke_width,
            holes: vec![],
            opacity: 1.0,
            hidden: false,
            locked: false,
            name: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Triangle,
    Polygon,
}

/// LEGACY group registry entry (pre-tree files only): kept so old `.vrs` documents still deserialize;
/// `migrate_legacy()` converts the registry into tree nodes on load and clears it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: u32,
    pub name: String,
    pub parent: Option<u32>,
}

/// What a scene-graph node IS. `Path` leaves point at an entry in the flat `paths` storage.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Layer,
    Group,
    Path(u32),
}

/// Where a dragged row lands relative to the target row (the 3-zone drag model).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropPos {
    Before,
    Into,
    After,
}

/// One node of the REAL scene graph (the Layers system, D2). Structure lives here; geometry/appearance
/// stay in the flat `Vec<Path>` (which is kept re-flattened to tree order, so everything that reads
/// "vec order = z" keeps working untouched).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: u32,
    pub kind: NodeKind,
    /// Display name for Layer/Group rows (Path leaves show their Path.name / auto-name instead).
    #[serde(default)]
    pub name: String,
    /// Parent node id (None = a root Layer).
    #[serde(default)]
    pub parent: Option<u32>,
    /// Children, FRONT-FIRST (index 0 = top row of the panel = front-most on canvas).
    #[serde(default)]
    pub children: Vec<u32>,
    /// Node-level hide/lock (the panel's eye/padlock on containers) — cascades to descendants.
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    /// The layer colour strip (panel); None = default.
    #[serde(default)]
    pub color: Option<Rgba>,
}

/// A single artboard (a defined PAGE) on the infinite board, in world points (1pt = 1/72in). The
/// document holds a `Vec<Artboard>` + an `active` index (the Artboard system, Shift+O). An artboard is
/// page furniture — never a z-object and never *contains* artwork (the Illustrator model); which page an
/// object belongs to is decided by bounds overlap at export, not by containment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artboard {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub name: String,
    /// print bleed (pt) — the page is a PDF TrimBox; bleed extends the BleedBox outward. UI is later,
    /// but the field exists from day one so multi + bleed never force a format migration. Default 0.
    #[serde(default)]
    pub bleed: f32,
    /// page fill. `Some(rgba)` = a solid page colour (white default); `None` = TRANSPARENT (export with
    /// no background; on-canvas it shows just its edge). Changeable per artboard.
    #[serde(default = "white_page")]
    pub page_color: Option<Rgba>,
    /// clip artwork that overflows the page edge on-canvas (a per-artboard toggle). Default OFF
    /// (Illustrator: art bleeds past the edge freely; clipping is an export-time choice).
    #[serde(default)]
    pub clip: bool,
}
fn white_page() -> Option<Rgba> {
    Some([1.0, 1.0, 1.0, 1.0])
}
impl Default for Artboard {
    /// A square 1080×1080 white page at the origin (the locked default; a categorised new-file size
    /// modal comes later). px == pt at the default 72 ppi.
    fn default() -> Self {
        Artboard {
            x: 0.0,
            y: 0.0,
            w: 1080.0,
            h: 1080.0,
            name: "Artboard 1".into(),
            bleed: 0.0,
            page_color: white_page(),
            clip: false,
        }
    }
}
impl Artboard {
    /// (x0, y0, x1, y1) — the page rect in world points.
    pub fn rect(&self) -> (f32, f32, f32, f32) {
        (self.x, self.y, self.x + self.w, self.y + self.h)
    }
    pub fn contains(&self, p: Pt) -> bool {
        p[0] >= self.x && p[0] <= self.x + self.w && p[1] >= self.y && p[1] <= self.y + self.h
    }
}

fn one_artboard() -> Vec<Artboard> {
    vec![Artboard::default()]
}
fn yes() -> bool {
    true
}

/// Snapping configuration — the unified, Affinity-grade snap model (SNAP_TRANSFORM_SPEC §2). One config in
/// three groups, every field serde-defaulted (struct-level `#[serde(default)]` + an explicit `Default`) so
/// adding fields never breaks old `.varos` files. `smart` is the Ctrl+U master (Smart Guides). Stage-1 acts
/// on the ON-by-default fields; the rest are present-but-inert until their subsystem lands (grid/pixel/etc.).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapConfig {
    // ── BEHAVIOUR (global) ──
    pub enabled: bool,           // master kill switch (Affinity "Enable snapping")
    pub smart: bool,             // Smart Guides master (Ctrl+U)
    pub radius_px: f32,          // capture tolerance in SCREEN px (÷ ppu at use)
    pub candidate_max: usize,    // N strongest candidates kept per frame
    pub show_candidates: bool,   // draw the live alignment lines / pips / halo
    pub force_pixel_align: bool, // land the snapped result on whole device px (Stage 2)
    pub move_whole_px: bool,     // quantise the whole motion to integer px (Stage 2)

    // ── PAGE / ARTBOARD snapping ──
    pub grid: bool,          // construction grid intersections (Stage 2)
    pub grid_lines: bool,    // snap to a single grid LINE (one axis), not only intersections
    pub baseline_grid: bool, // typographic baseline grid (Stage 2/3, with text)
    pub guides: bool,        // user ruler guides
    pub artboard: bool,      // page / "spread" edges
    pub artboard_mids: bool, // page centre lines + centre point (dependent child of `artboard`)
    pub margins: bool,       // page margin box (Stage 2/3)
    pub margin_mids: bool,   // margin-box centre lines (Stage 2/3)

    // ── OBJECT snapping ──
    pub visible_only: bool,       // candidates from on-screen objects only
    pub object_bounds: bool,      // bbox edges + corners
    pub bbox_mids: bool,          // bbox edge-mids + centre (dependent child of `object_bounds`)
    pub gaps_and_sizes: bool,     // equal-spacing + equal-dimension snap (Affinity flagship)
    pub key_points: bool,         // anchors / handle ends (Illustrator "Snap to Point")
    pub object_geometry: bool,    // snap anywhere on a path edge (nearest-point-on-outline)
    pub segment_mids: bool,       // midpoint between two adjacent nodes (Corel/Inkscape)
    pub path_intersections: bool, // where two path edges cross (Inkscape/Corel diamond)
    pub pixel_bounds: bool,       // raster-selection bounds (Stage 3 — vector-only for now)

    // ── alignment-guide feedback ──
    pub alignment_guides: bool, // the H/V extension lines drawn from the matched feature
    pub equal_spacing: bool,    // equal-gap pips + `=` ticks
    pub equal_size: bool,       // match-width/height hints

    // ── grid spacing (so the engine has no magic constant) ──
    pub grid_spacing: f32, // world pt
}
impl Default for SnapConfig {
    /// Defaults follow the spec: Smart Guides + object/page snapping + feedback ON; grid/pixel/margins/
    /// baseline OFF (inert until their subsystem ships). Tolerance 8 screen px; candidate cap 8.
    fn default() -> Self {
        SnapConfig {
            enabled: true,
            smart: true,
            radius_px: 8.0,
            candidate_max: 8,
            show_candidates: true,
            force_pixel_align: false,
            move_whole_px: false,
            grid: false,
            grid_lines: true,
            baseline_grid: false,
            guides: true,
            artboard: true,
            artboard_mids: true,
            margins: false,
            margin_mids: false,
            visible_only: true,
            object_bounds: true,
            bbox_mids: true,
            gaps_and_sizes: true,
            key_points: true,
            object_geometry: true,
            segment_mids: true,
            path_intersections: true,
            pixel_bounds: false,
            alignment_guides: true,
            equal_spacing: true,
            equal_size: true,
            grid_spacing: 72.0,
        }
    }
}

/// A ruler guide: an infinite construction line. `vertical` ⇒ a vertical line at world x = `pos`
/// (dragged out of the LEFT ruler); else a horizontal line at world y = `pos` (dragged out of the TOP
/// ruler). Snapping locks onto it like any other target; persisted with the document.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub vertical: bool,
    pub pos: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub paths: Vec<Path>,
    /// LEGACY registry (pre-tree files). Deserialized for compatibility, converted by
    /// `migrate_legacy()`, then stays empty. New code never writes it.
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub group_of: HashMap<u32, u32>,
    /// THE SCENE GRAPH (Layers system, D2): a node arena + the root Layers, FRONT-FIRST. Structure is
    /// authoritative here; `paths` is storage kept re-flattened to traversal order by `sync_tree`.
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub roots: Vec<u32>,
    /// The ACTIVE layer (new objects land here — Illustrator). Fixed up by `sync_tree` if stale.
    #[serde(default)]
    pub active_layer: u32,
    pub ids: u32,
    /// Document measurement settings (ppi + display unit). `#[serde(default)]` so older `.varos`
    /// files written before this field still load.
    #[serde(default)]
    pub units: DocUnits,
    /// The artboards (pages). Always ≥1 — the model guarantees you can't delete the last one.
    #[serde(default = "one_artboard")]
    pub artboards: Vec<Artboard>,
    /// Index of the active artboard within `artboards` (the one the panel edits / new objects land on).
    #[serde(default)]
    pub active: usize,
    /// When moving an artboard with the Artboard tool, also move the artwork sitting on it. A toggle,
    /// default ON (matches Illustrator's control-bar default and Figma).
    #[serde(default = "yes")]
    pub move_art_with_ab: bool,
    /// Snapping config (the magnet menu / Ctrl+U). `#[serde(default)]` so older files still load.
    #[serde(default)]
    pub snap: SnapConfig,
    /// Ruler zero-point in WORLD coords. Numbers on the rulers read `world - ruler_origin` (top-left
    /// origin, Y-down). Defaults to [0,0] = the default artboard's top-left; drag from the ruler corner
    /// to set, double-click the corner to reset. `#[serde(default)]` so older `.varos` files still load.
    #[serde(default)]
    pub ruler_origin: Pt,
    /// User ruler guides (dragged out of the rulers). `#[serde(default)]` so older files still load.
    #[serde(default)]
    pub guides: Vec<Guide>,
    /// Guides locked (can't be grabbed/moved) — Illustrator's Alt+Ctrl+; . Persisted with the doc.
    #[serde(default)]
    pub guides_locked: bool,
}
impl Default for Document {
    fn default() -> Self {
        // a fresh document opens with one empty "Layer 1" (the roadmap's empty state)
        Document {
            paths: vec![],
            groups: vec![],
            group_of: HashMap::new(),
            nodes: vec![Node {
                id: 1,
                kind: NodeKind::Layer,
                name: "Layer 1".into(),
                parent: None,
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
            }],
            roots: vec![1],
            active_layer: 1,
            ids: 1,
            units: DocUnits::default(),
            artboards: one_artboard(),
            active: 0,
            move_art_with_ab: true,
            snap: SnapConfig::default(),
            ruler_origin: [0.0, 0.0],
            guides: vec![],
            guides_locked: false,
        }
    }
}

impl Document {
    pub fn nid(&mut self) -> u32 {
        self.ids += 1;
        self.ids
    }

    /// The active artboard (index clamped). None only if there are somehow zero — the model avoids that.
    pub fn active_artboard(&self) -> Option<&Artboard> {
        if self.artboards.is_empty() {
            return None;
        }
        self.artboards.get(self.active.min(self.artboards.len() - 1))
    }
    pub fn active_artboard_mut(&mut self) -> Option<&mut Artboard> {
        if self.artboards.is_empty() {
            return None;
        }
        let i = self.active.min(self.artboards.len() - 1);
        self.artboards.get_mut(i)
    }

    pub fn pidx(&self, pid: u32) -> Option<usize> {
        self.paths.iter().position(|p| p.id == pid)
    }
    pub fn aidx(&self, aid: u32) -> Option<(usize, usize)> {
        for (pi, p) in self.paths.iter().enumerate() {
            if let Some(ai) = p.anchors.iter().position(|a| a.id == aid) {
                return Some((pi, ai));
            }
        }
        None
    }
    /// Find an anchor by id across ALL contours (outer + holes). aidx covers only the outer ring.
    pub fn anchor(&self, aid: u32) -> Option<&Anchor> {
        for p in &self.paths {
            if let Some(a) = p.anchors.iter().find(|a| a.id == aid) {
                return Some(a);
            }
            for h in &p.holes {
                if let Some(a) = h.iter().find(|a| a.id == aid) {
                    return Some(a);
                }
            }
        }
        None
    }
    pub fn anchor_mut(&mut self, aid: u32) -> Option<&mut Anchor> {
        for pi in 0..self.paths.len() {
            if let Some(ai) = self.paths[pi].anchors.iter().position(|a| a.id == aid) {
                return Some(&mut self.paths[pi].anchors[ai]);
            }
            for hi in 0..self.paths[pi].holes.len() {
                if let Some(ai) = self.paths[pi].holes[hi].iter().position(|a| a.id == aid) {
                    return Some(&mut self.paths[pi].holes[hi][ai]);
                }
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
        if n < 2 {
            return None;
        }
        let segs = if p.closed { n } else { n - 1 };
        let mut best: Option<(usize, f32, f32)> = None;
        for i in 0..segs {
            let a = &p.anchors[i];
            let b = &p.anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            for k in 0..=24 {
                let t = k as f32 / 24.0;
                let d = dist(cubic(p0, p1, p2, p3, t), pos);
                if best.is_none_or(|(_, _, bd)| d < bd) {
                    best = Some((i, t, d));
                }
            }
        }
        best
    }

    /// Flatten any anchor ring into a polyline (steps points per segment). Shared by outer + holes.
    pub fn ring(anchors: &[Anchor], closed: bool, steps: usize) -> Vec<Pt> {
        let n = anchors.len();
        let mut poly = Vec::new();
        if n == 0 {
            return poly;
        }
        let segs = if closed { n } else { n - 1 };
        poly.push(anchors[0].p);
        for i in 0..segs {
            let a = &anchors[i];
            let b = &anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            for s in 1..=steps {
                poly.push(cubic(p0, p1, p2, p3, s as f32 / steps as f32));
            }
        }
        poly
    }
    /// Resolution-independent flatten: per-cubic step count adapts so chords stay ~4px on screen.
    pub fn ring_px(anchors: &[Anchor], closed: bool, ppu: f32) -> Vec<Pt> {
        let n = anchors.len();
        let mut poly = Vec::new();
        if n == 0 {
            return poly;
        }
        let segs = if closed { n } else { n - 1 };
        poly.push(anchors[0].p);
        for i in 0..segs {
            let a = &anchors[i];
            let b = &anchors[(i + 1) % n];
            let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
            let steps = if a.hout.is_none() && b.hin.is_none() {
                1
            } else {
                let clen = dist(p0, p1) + dist(p1, p2) + dist(p2, p3);
                (((clen * ppu) / 4.0).ceil() as usize).clamp(8, 256)
            };
            for s in 1..=steps {
                poly.push(cubic(p0, p1, p2, p3, s as f32 / steps as f32));
            }
        }
        poly
    }

    /// Outer outline of a path (steps per segment).
    pub fn outline(&self, pi: usize, steps: usize) -> Vec<Pt> {
        Self::ring(&self.paths[pi].anchors, self.paths[pi].closed, steps)
    }
    /// Resolution-independent outer outline (`ppu` = view zoom) — smooth at any zoom.
    pub fn outline_px(&self, pi: usize, ppu: f32) -> Vec<Pt> {
        Self::ring_px(&self.paths[pi].anchors, self.paths[pi].closed, ppu)
    }

    /// Visual (outline) bounding box of one path — used for align / distribute.
    pub fn outline_bbox(&self, pi: usize) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for q in self.outline(pi, 12) {
            x0 = x0.min(q[0]);
            y0 = y0.min(q[1]);
            x1 = x1.max(q[0]);
            y1 = y1.max(q[1]);
        }
        if x0 <= x1 {
            (x0, y0, x1, y1)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    pub fn point_in_path(&self, pi: usize, pt: Pt) -> bool {
        let p = &self.paths[pi];
        if !p.closed || p.anchors.len() < 3 {
            return false;
        }
        if !point_in_poly(&self.outline(pi, 8), pt) {
            return false;
        }
        // inside the outer ring — but a point inside a hole is NOT in the (even-odd) filled region
        for h in &p.holes {
            if h.len() >= 3 && point_in_poly(&Self::ring(h, true, 8), pt) {
                return false;
            }
        }
        true
    }

    pub fn bbox(&self, pi: usize) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for a in &self.paths[pi].anchors {
            for q in [Some(a.p), a.hin, a.hout].into_iter().flatten() {
                x0 = x0.min(q[0]);
                y0 = y0.min(q[1]);
                x1 = x1.max(q[0]);
                y1 = y1.max(q[1]);
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
            self.ids += 1;
            Anchor { id: self.ids, p, hin, hout, smooth }
        };
        match kind {
            ShapeKind::Rect => vec![
                mk([x0, y0], None, None, false),
                mk([x1, y0], None, None, false),
                mk([x1, y1], None, None, false),
                mk([x0, y1], None, None, false),
            ],
            ShapeKind::Triangle => {
                vec![mk([cx, y0], None, None, false), mk([x1, y1], None, None, false), mk([x0, y1], None, None, false)]
            }
            ShapeKind::Polygon => {
                let n = 6;
                let mut out = Vec::new();
                for i in 0..n {
                    let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
                    out.push(mk([cx + rx * ang.cos(), cy + ry * ang.sin()], None, None, false));
                }
                out
            }
            ShapeKind::Ellipse => {
                let (kx, ky) = (K * rx, K * ry);
                vec![
                    mk([cx, y0], Some([cx - kx, y0]), Some([cx + kx, y0]), true),
                    mk([x1, cy], Some([x1, cy - ky]), Some([x1, cy + ky]), true),
                    mk([cx, y1], Some([cx + kx, y1]), Some([cx - kx, y1]), true),
                    mk([x0, cy], Some([x0, cy + ky]), Some([x0, cy - ky]), true),
                ]
            }
        }
    }

    pub fn clone_path(&mut self, pid: u32) -> Path {
        let pi = self.pidx(pid).unwrap();
        let src = self.paths[pi].clone();
        let id = self.nid();
        let anchors = src
            .anchors
            .iter()
            .map(|a| {
                self.ids += 1;
                Anchor { id: self.ids, p: a.p, hin: a.hin, hout: a.hout, smooth: a.smooth }
            })
            .collect();
        let holes = src
            .holes
            .iter()
            .map(|h| {
                h.iter()
                    .map(|a| {
                        self.ids += 1;
                        Anchor { id: self.ids, p: a.p, hin: a.hin, hout: a.hout, smooth: a.smooth }
                    })
                    .collect()
            })
            .collect();
        Path {
            holes,
            opacity: src.opacity,
            hidden: src.hidden,
            locked: src.locked,
            name: src.name.clone(),
            ..Path::new(id, anchors, src.closed, src.fill, src.stroke, src.stroke_width)
        }
    }

    // ---------- THE SCENE GRAPH (Layers system, D2) ----------
    // Structure is authoritative in `nodes`/`roots` (FRONT-first children); `paths` is storage that
    // `sync_tree` keeps re-flattened to traversal order — so every consumer of "vec order = z"
    // (renderer, hit-testing, export) keeps working untouched.

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    fn node_mut(&mut self, id: u32) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
    /// The leaf node representing a path.
    pub fn node_of_path(&self, pid: u32) -> Option<u32> {
        self.nodes.iter().find(|n| matches!(n.kind, NodeKind::Path(p) if p == pid)).map(|n| n.id)
    }
    /// The HIGHEST Group ancestor of a path's leaf (stops at the Layer). None = ungrouped.
    pub fn top_group_of_path(&self, pid: u32) -> Option<u32> {
        let mut cur = self.node_of_path(pid)?;
        let mut top = None;
        for _ in 0..4096 {
            let n = self.node(cur)?;
            match n.parent {
                Some(p) => {
                    if matches!(self.node(p)?.kind, NodeKind::Group) {
                        top = Some(p);
                    }
                    cur = p;
                }
                None => break,
            }
        }
        top
    }
    /// All path ids in `nid`'s subtree, front-first (traversal order).
    fn collect_paths(&self, nid: u32, out: &mut Vec<u32>) {
        if let Some(n) = self.node(nid) {
            if let NodeKind::Path(p) = n.kind {
                out.push(p);
            }
            for &c in &n.children {
                self.collect_paths(c, out);
            }
        }
    }
    /// Every path id in the same top-level group as `pid` (z order, back→front). `[pid]` if ungrouped.
    pub fn group_members(&self, pid: u32) -> Vec<u32> {
        match self.top_group_of_path(pid) {
            None => vec![pid],
            Some(top) => {
                let mut v = vec![];
                self.collect_paths(top, &mut v);
                v.reverse();
                v
            }
        }
    }
    /// The selection UNIT a path belongs to: its top-level group node, else its own leaf node.
    fn unit_of(&self, pid: u32) -> Option<u32> {
        self.top_group_of_path(pid).or_else(|| self.node_of_path(pid))
    }
    /// Effective visibility: the path's own flag OR any ancestor container's (the panel eye cascade).
    pub fn eff_hidden(&self, pid: u32) -> bool {
        if self.pidx(pid).is_none_or(|i| self.paths[i].hidden) {
            return true;
        }
        let mut cur = self.node_of_path(pid);
        while let Some(id) = cur {
            let Some(n) = self.node(id) else { break };
            if n.hidden {
                return true;
            }
            cur = n.parent;
        }
        false
    }
    /// Effective lock: the path's own flag OR any ancestor container's (cascade).
    pub fn eff_locked(&self, pid: u32) -> bool {
        if self.pidx(pid).is_some_and(|i| self.paths[i].locked) {
            return true;
        }
        let mut cur = self.node_of_path(pid);
        while let Some(id) = cur {
            let Some(n) = self.node(id) else { break };
            if n.locked {
                return true;
            }
            cur = n.parent;
        }
        false
    }
    /// Group these paths into a new Group node (returns its id). The group lands at the FRONT-most
    /// member's slot in ITS parent (Illustrator: pulled to the top-most member — cross-layer selections
    /// collect onto the front-most member's layer). Grouping groups NESTS them, so a later single
    /// ungroup peels exactly one level.
    pub fn group(&mut self, pids: &[u32]) -> Option<u32> {
        use std::collections::HashSet;
        self.sync_tree(); // self-sufficient: adopt any raw pushes first (tests / direct callers)
        let mut seen = HashSet::new();
        let mut units: Vec<u32> = vec![];
        for &p in pids {
            if self.pidx(p).is_none() {
                continue;
            }
            if let Some(u) = self.unit_of(p) {
                if seen.insert(u) {
                    units.push(u);
                }
            }
        }
        if units.len() < 2 {
            return None;
        }
        // FRONT-first ordering of the units = descending front-most storage index of their contents
        let vec_pos = |d: &Document, u: u32| -> usize {
            let mut v = vec![];
            d.collect_paths(u, &mut v);
            v.iter().filter_map(|p| d.pidx(*p)).max().unwrap_or(0)
        };
        units.sort_by_key(|&u| std::cmp::Reverse(vec_pos(self, u)));
        let front = units[0];
        let host = self.node(front)?.parent?;
        let slot = self.node(host)?.children.iter().position(|&c| c == front)?;
        // where the group lands in host = slot minus the selected units sitting above it
        let idx = self.node(host)?.children.iter().take(slot).filter(|c| !units.contains(c)).count();
        let gid = self.nid();
        self.nodes.push(Node {
            id: gid,
            kind: NodeKind::Group,
            name: format!("Group {gid}"),
            parent: Some(host),
            children: vec![],
            hidden: false,
            locked: false,
            color: None,
        });
        for &u in &units {
            if let Some(par) = self.node(u).and_then(|n| n.parent) {
                if let Some(pn) = self.node_mut(par) {
                    pn.children.retain(|&c| c != u);
                }
            }
            if let Some(un) = self.node_mut(u) {
                un.parent = Some(gid);
            }
        }
        if let Some(hn) = self.node_mut(host) {
            hn.children.insert(idx, gid);
        }
        if let Some(gn) = self.node_mut(gid) {
            gn.children = units;
        }
        self.flatten();
        Some(gid)
    }
    /// Ungroup: peel exactly ONE level off the top-level group(s) the selection belongs to. The
    /// dissolved group's children rise into its parent AT ITS SLOT (z preserved); inner groups survive.
    pub fn ungroup(&mut self, pids: &[u32]) {
        use std::collections::HashSet;
        self.sync_tree();
        let tops: HashSet<u32> = pids.iter().filter_map(|&p| self.top_group_of_path(p)).collect();
        for top in tops {
            let Some(tn) = self.node(top).cloned() else { continue };
            let Some(host) = tn.parent else { continue };
            let Some(hidx) = self.node(host).and_then(|h| h.children.iter().position(|&c| c == top)) else { continue };
            for (k, &c) in tn.children.iter().enumerate() {
                if let Some(cn) = self.node_mut(c) {
                    cn.parent = Some(host);
                }
                if let Some(hn) = self.node_mut(host) {
                    hn.children.insert(hidx + 1 + k, c);
                }
            }
            if let Some(hn) = self.node_mut(host) {
                hn.children.retain(|&c| c != top);
            }
            self.nodes.retain(|n| n.id != top);
        }
        self.flatten();
    }
    /// Duplicate a set of paths, PRESERVING their group structure: the copies mirror the originals'
    /// subtree and land at the FRONT of the originals' container (single layer ⇒ the old "copy lands on
    /// top"). Returns the new path ids (in `srcs` order). Used by Alt-drag duplicate + Transform Again.
    pub fn dup_paths(&mut self, srcs: &[u32]) -> Vec<u32> {
        self.sync_tree(); // sources must be in the tree before mirroring their subtree
                          // 1) clone the paths (clone_path gives fresh anchor + path ids)
        let mut pmap: HashMap<u32, u32> = HashMap::new();
        let mut new_pids = vec![];
        for &s in srcs {
            if self.pidx(s).is_none() {
                continue;
            }
            let c = self.clone_path(s);
            pmap.insert(s, c.id);
            new_pids.push(c.id);
            self.paths.push(c);
        }
        // 2) every Group ancestor of the sources (innermost → up to the Layer)
        let mut gset: Vec<u32> = vec![];
        for &s in srcs {
            let mut cur = self.node_of_path(s).and_then(|n| self.node(n)).and_then(|n| n.parent);
            while let Some(g) = cur {
                let Some(gn) = self.node(g) else { break };
                if !matches!(gn.kind, NodeKind::Group) {
                    break;
                }
                if !gset.contains(&g) {
                    gset.push(g);
                }
                cur = gn.parent;
            }
        }
        // 3) mirrored Group nodes + copied leaf nodes
        let mut gmap: HashMap<u32, u32> = HashMap::new();
        for &og in &gset {
            let ng = self.nid();
            let name = self.node(og).map(|n| n.name.clone()).unwrap_or_default();
            self.nodes.push(Node {
                id: ng,
                kind: NodeKind::Group,
                name,
                parent: None,
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
            });
            gmap.insert(og, ng);
        }
        let mut leafmap: HashMap<u32, u32> = HashMap::new(); // old LEAF node id → new leaf node id
        for (&old_p, &new_p) in &pmap {
            if let Some(old_leaf) = self.node_of_path(old_p) {
                let nl = self.nid();
                self.nodes.push(Node {
                    id: nl,
                    kind: NodeKind::Path(new_p),
                    name: String::new(),
                    parent: None,
                    children: vec![],
                    hidden: false,
                    locked: false,
                    color: None,
                });
                leafmap.insert(old_leaf, nl);
            }
        }
        // 4) mirror children lists (originals' order restricted to copied nodes) + parent links
        for &og in &gset {
            let kids: Vec<u32> = self
                .node(og)
                .map(|n| {
                    n.children.iter().filter_map(|c| gmap.get(c).copied().or_else(|| leafmap.get(c).copied())).collect()
                })
                .unwrap_or_default();
            let ng = gmap[&og];
            for &k in &kids {
                if let Some(kn) = self.node_mut(k) {
                    kn.parent = Some(ng);
                }
            }
            if let Some(n) = self.node_mut(ng) {
                n.children = kids;
            }
        }
        // 5) copied roots (mirrored node whose ORIGINAL parent wasn't copied) → front of that parent
        let mut attach: Vec<(u32, u32)> = vec![];
        for &og in &gset {
            if let Some(op) = self.node(og).and_then(|n| n.parent) {
                if !gmap.contains_key(&op) {
                    attach.push((gmap[&og], op));
                }
            }
        }
        for (&old_leaf, &new_leaf) in &leafmap {
            if let Some(op) = self.node(old_leaf).and_then(|n| n.parent) {
                if !gmap.contains_key(&op) {
                    attach.push((new_leaf, op));
                }
            }
        }
        for (nn, host) in attach {
            if let Some(n) = self.node_mut(nn) {
                n.parent = Some(host);
            }
            if let Some(hn) = self.node_mut(host) {
                hn.children.insert(0, nn);
            }
        }
        self.flatten();
        new_pids
    }
    /// Arrange the selection's UNITS within their own parents (Illustrator scope: front/back relative
    /// to siblings). `extreme` = to Front/Back; otherwise one step Forward/Backward.
    pub fn arrange_units(&mut self, sel: &std::collections::HashSet<u32>, toward_front: bool, extreme: bool) {
        use std::collections::HashSet;
        self.sync_tree();
        let mut units: HashSet<u32> = HashSet::new();
        for &p in sel {
            if let Some(u) = self.unit_of(p) {
                units.insert(u);
            }
        }
        let parents: HashSet<u32> = units.iter().filter_map(|&u| self.node(u).and_then(|n| n.parent)).collect();
        for par in parents {
            let Some(pn) = self.node(par) else { continue };
            let mut kids = pn.children.clone(); // FRONT-first
            if extreme {
                let (s, r): (Vec<u32>, Vec<u32>) = kids.into_iter().partition(|c| units.contains(c));
                kids = if toward_front { s.into_iter().chain(r).collect() } else { r.into_iter().chain(s).collect() };
            } else if toward_front {
                for i in 1..kids.len() {
                    if units.contains(&kids[i]) && !units.contains(&kids[i - 1]) {
                        kids.swap(i, i - 1);
                    }
                }
            } else {
                for i in (0..kids.len().saturating_sub(1)).rev() {
                    if units.contains(&kids[i]) && !units.contains(&kids[i + 1]) {
                        kids.swap(i, i + 1);
                    }
                }
            }
            if let Some(pm) = self.node_mut(par) {
                pm.children = kids;
            }
        }
        self.flatten();
    }
    /// Detach a node from its parent (or roots) and drop it from the arena. Children are NOT touched —
    /// callers re-home them first when that matters.
    fn remove_node(&mut self, id: u32) {
        match self.node(id).and_then(|n| n.parent) {
            Some(par) => {
                if let Some(pn) = self.node_mut(par) {
                    pn.children.retain(|&c| c != id);
                }
            }
            None => self.roots.retain(|&r| r != id),
        }
        self.nodes.retain(|n| n.id != id);
    }
    /// Ensure `id` is linked into its parent's children (at the FRONT), attaching unattached ancestors
    /// first. Used by the legacy migration's back→front walk (reproduces flat z exactly).
    fn attach_front(&mut self, id: u32) {
        let Some(par) = self.node(id).and_then(|n| n.parent) else { return };
        if self.node(par).is_none_or(|p| p.children.contains(&id)) {
            return;
        }
        self.attach_front(par);
        if let Some(pn) = self.node_mut(par) {
            pn.children.insert(0, id);
        }
    }
    /// Convert the pre-tree registry (groups/group_of) into tree nodes ONCE (legacy files), then clear
    /// it. Walking storage back→front and front-inserting reproduces the original z exactly.
    pub fn migrate_legacy(&mut self) {
        if self.groups.is_empty() && self.group_of.is_empty() {
            return;
        }
        if self.roots.is_empty() {
            let id = self.nid();
            self.nodes.push(Node {
                id,
                kind: NodeKind::Layer,
                name: "Layer 1".into(),
                parent: None,
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
            });
            self.roots.push(id);
        }
        let host = self.roots[0];
        let legacy = std::mem::take(&mut self.groups);
        let memb = std::mem::take(&mut self.group_of);
        let mut gmap: HashMap<u32, u32> = HashMap::new();
        for g in &legacy {
            let id = self.nid();
            self.nodes.push(Node {
                id,
                kind: NodeKind::Group,
                name: g.name.clone(),
                parent: None,
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
            });
            gmap.insert(g.id, id);
        }
        for g in &legacy {
            let ng = gmap[&g.id];
            let np = g.parent.and_then(|p| gmap.get(&p).copied()).unwrap_or(host);
            if let Some(n) = self.node_mut(ng) {
                n.parent = Some(np);
            }
        }
        let order: Vec<u32> = self.paths.iter().map(|p| p.id).collect(); // back→front
        for pid in order {
            let parent = memb.get(&pid).and_then(|g| gmap.get(g).copied()).unwrap_or(host);
            let id = self.nid();
            self.nodes.push(Node {
                id,
                kind: NodeKind::Path(pid),
                name: String::new(),
                parent: Some(parent),
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
            });
            self.attach_front(id);
        }
    }
    /// Re-order the flat storage to tree traversal (back→front) — the invariant every "vec order = z"
    /// consumer relies on. Paths not yet in the tree keep their push order at the very front (they are
    /// adopted by the next `sync_tree`).
    pub fn flatten(&mut self) {
        let mut order: Vec<u32> = vec![];
        let roots = self.roots.clone();
        for r in roots {
            self.collect_paths(r, &mut order);
        }
        order.reverse();
        let pos: HashMap<u32, usize> = order.iter().enumerate().map(|(i, &p)| (p, i)).collect();
        let n = order.len();
        let mut indexed: Vec<(usize, Path)> = std::mem::take(&mut self.paths)
            .into_iter()
            .enumerate()
            .map(|(i, p)| (pos.get(&p.id).copied().unwrap_or(n + i), p))
            .collect();
        indexed.sort_by_key(|(k, _)| *k);
        self.paths = indexed.into_iter().map(|(_, p)| p).collect();
    }
    /// Reconcile the tree with storage after any mutation (runs at commit — sync_groups' successor):
    /// migrate legacy registries, prune leaves of deleted paths + emptied Groups, adopt new paths
    /// under the ACTIVE layer (at its front), guarantee ≥1 Layer + a valid active_layer, re-flatten.
    pub fn sync_tree(&mut self) {
        use std::collections::HashSet;
        self.migrate_legacy();
        let live: HashSet<u32> = self.paths.iter().map(|p| p.id).collect();
        let dead: Vec<u32> = self
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Path(p) if !live.contains(&p)))
            .map(|n| n.id)
            .collect();
        for d in dead {
            self.remove_node(d);
        }
        loop {
            let empty: Vec<u32> = self
                .nodes
                .iter()
                .filter(|n| matches!(n.kind, NodeKind::Group) && n.children.is_empty())
                .map(|n| n.id)
                .collect();
            if empty.is_empty() {
                break;
            }
            for e in empty {
                self.remove_node(e);
            }
        }
        if !self.roots.iter().any(|&r| self.node(r).is_some_and(|n| matches!(n.kind, NodeKind::Layer))) {
            let id = self.nid();
            self.nodes.push(Node {
                id,
                kind: NodeKind::Layer,
                name: "Layer 1".into(),
                parent: None,
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
            });
            self.roots.insert(0, id);
        }
        if self.node(self.active_layer).is_none_or(|n| !matches!(n.kind, NodeKind::Layer)) {
            self.active_layer = self
                .roots
                .iter()
                .copied()
                .find(|&r| self.node(r).is_some_and(|n| matches!(n.kind, NodeKind::Layer)))
                .unwrap_or(0);
        }
        let known: HashSet<u32> =
            self.nodes.iter().filter_map(|n| if let NodeKind::Path(p) = n.kind { Some(p) } else { None }).collect();
        let newbies: Vec<u32> = self.paths.iter().map(|p| p.id).filter(|p| !known.contains(p)).collect();
        let host = self.active_layer;
        for pid in newbies {
            // back→front; front-inserting each keeps their relative order
            let id = self.nid();
            self.nodes.push(Node {
                id,
                kind: NodeKind::Path(pid),
                name: String::new(),
                parent: Some(host),
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
            });
            if let Some(h) = self.node_mut(host) {
                h.children.insert(0, id);
            }
        }
        self.flatten();
    }

    // ---------- Layers panel API (structural ops the panel drives) ----------
    /// The Layer node an id lives under (walk parents to the enclosing Layer; returns nid if it IS one).
    pub fn layer_ancestor(&self, nid: u32) -> u32 {
        let mut cur = nid;
        for _ in 0..4096 {
            let Some(n) = self.node(cur) else { break };
            if matches!(n.kind, NodeKind::Layer) {
                return cur;
            }
            match n.parent {
                Some(p) => cur = p,
                None => break,
            }
        }
        cur
    }
    /// All path ids in a node's subtree, z order (back→front).
    pub fn node_paths(&self, nid: u32) -> Vec<u32> {
        let mut v = vec![];
        self.collect_paths(nid, &mut v);
        v.reverse();
        v
    }
    fn children_of(&self, parent: Option<u32>) -> Vec<u32> {
        match parent {
            Some(p) => self.node(p).map(|n| n.children.clone()).unwrap_or_default(),
            None => self.roots.clone(),
        }
    }
    fn is_descendant(&self, node: u32, ancestor: u32) -> bool {
        let mut cur = self.node(node).and_then(|n| n.parent);
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.node(c).and_then(|n| n.parent);
        }
        false
    }
    fn unlink(&mut self, id: u32) {
        match self.node(id).and_then(|n| n.parent) {
            Some(p) => {
                if let Some(n) = self.node_mut(p) {
                    n.children.retain(|&c| c != id);
                }
            }
            None => self.roots.retain(|&r| r != id),
        }
    }
    /// Drag & drop: move `src` (+ its subtree) relative to `target`. Returns false (no-op) for illegal
    /// drops — cycle (into self/own descendant), Into a leaf Path, or a Layer into a Group. Undoable via
    /// the caller's begin/commit; re-flattens z on success.
    pub fn move_node_to(&mut self, src: u32, target: u32, pos: DropPos) -> bool {
        if src == target || self.node(src).is_none() || self.node(target).is_none() {
            return false;
        }
        let src_is_layer = matches!(self.node(src).unwrap().kind, NodeKind::Layer);
        let (parent, mut index) = match pos {
            DropPos::Into => {
                if !matches!(self.node(target).unwrap().kind, NodeKind::Layer | NodeKind::Group) {
                    return false;
                }
                (Some(target), 0usize) // Illustrator drops at the FRONT (top) of the container
            }
            DropPos::Before | DropPos::After => {
                let par = self.node(target).unwrap().parent;
                let sib = self.children_of(par);
                let ti = sib.iter().position(|&c| c == target).unwrap_or(0);
                (par, if matches!(pos, DropPos::After) { ti + 1 } else { ti })
            }
        };
        // cycle guard: src can't become a child of itself or its own descendant
        if let Some(p) = parent {
            if p == src || self.is_descendant(p, src) {
                return false;
            }
        }
        // a Layer can nest in a Layer (sublayer) but never inside a Group
        if src_is_layer {
            if let Some(p) = parent {
                if matches!(self.node(p).unwrap().kind, NodeKind::Group) {
                    return false;
                }
            }
        }
        // same-parent reorder: removing src first shifts the insertion index down by one if src was above it
        let old_parent = self.node(src).unwrap().parent;
        if old_parent == parent {
            let sib = self.children_of(parent);
            if let Some(si) = sib.iter().position(|&c| c == src) {
                if si < index {
                    index -= 1;
                }
            }
        }
        self.unlink(src);
        match parent {
            Some(p) => {
                let n = self.node(p).map(|n| n.children.len()).unwrap_or(0);
                if let Some(pn) = self.node_mut(p) {
                    pn.children.insert(index.min(n), src);
                }
                if let Some(sn) = self.node_mut(src) {
                    sn.parent = Some(p);
                }
            }
            None => {
                let i = index.min(self.roots.len());
                self.roots.insert(i, src);
                if let Some(sn) = self.node_mut(src) {
                    sn.parent = None;
                }
            }
        }
        self.flatten();
        true
    }
    /// Drag-the-selection-square-to-move-art (Layers B4): move a SET of paths (the canvas selection) to a
    /// drop target, keeping their relative z. `copy` first flat-clones each into a fresh leaf (Alt-drag).
    /// Returns the pids now at the destination (`[]` on no-op) so the caller can reselect them. Paths are
    /// always leaves → no cycle/Layer guard; only `Into` must land on a container. An emptied source Group
    /// is dissolved by the next `sync_tree` (commit); source Layers are kept even when emptied.
    pub fn move_paths_to(&mut self, paths: &[u32], target: u32, pos: DropPos, copy: bool) -> Vec<u32> {
        let Some(tkind) = self.node(target).map(|n| n.kind) else { return vec![] };
        if matches!(pos, DropPos::Into) && !matches!(tkind, NodeKind::Layer | NodeKind::Group) {
            return vec![];
        }
        // sources present in the tree, ordered back→front so they keep relative z at the destination
        let mut pids: Vec<u32> = paths.iter().copied().filter(|p| self.pidx(*p).is_some()).collect();
        pids.sort_by_key(|pid| self.pidx(*pid).unwrap());
        pids.dedup();
        if pids.is_empty() {
            return vec![];
        }
        // Alt-copy: a flat clone into a new leaf — grouping is irrelevant once the art lands elsewhere
        let pids: Vec<u32> = if copy {
            pids.iter()
                .map(|&s| {
                    let c = self.clone_path(s);
                    let cid = c.id;
                    self.paths.push(c);
                    let nl = self.nid();
                    self.nodes.push(Node {
                        id: nl,
                        kind: NodeKind::Path(cid),
                        name: String::new(),
                        parent: None,
                        children: vec![],
                        hidden: false,
                        locked: false,
                        color: None,
                    });
                    cid
                })
                .collect()
        } else {
            pids
        };
        let nodes: Vec<u32> = pids.iter().filter_map(|&pid| self.node_of_path(pid)).collect();
        if nodes.is_empty() || nodes.contains(&target) {
            return vec![];
        }
        for &n in &nodes {
            self.unlink(n);
        }
        // destination parent + base index, resolved AFTER unlink so sibling indices are already settled
        let (parent, base) = match pos {
            DropPos::Into => (Some(target), 0usize),
            DropPos::Before | DropPos::After => {
                let par = self.node(target).and_then(|n| n.parent);
                let sib = self.children_of(par);
                let ti = sib.iter().position(|&c| c == target).unwrap_or(sib.len());
                (par, if matches!(pos, DropPos::After) { ti + 1 } else { ti })
            }
        };
        // front-first children: insert front-most first at `base` so back→front z survives and the whole
        // run lands together (Into → front of the container, Illustrator-style)
        for (i, &n) in nodes.iter().rev().enumerate() {
            let idx = base + i;
            match parent {
                Some(p) => {
                    let cn = self.node(p).map(|x| x.children.len()).unwrap_or(0);
                    if let Some(pn) = self.node_mut(p) {
                        pn.children.insert(idx.min(cn), n);
                    }
                    if let Some(sn) = self.node_mut(n) {
                        sn.parent = Some(p);
                    }
                }
                None => {
                    let cn = self.roots.len();
                    self.roots.insert(idx.min(cn), n);
                    if let Some(sn) = self.node_mut(n) {
                        sn.parent = None;
                    }
                }
            }
        }
        self.flatten();
        pids
    }
    pub fn set_node_name(&mut self, nid: u32, name: String) {
        if let Some(n) = self.node_mut(nid) {
            n.name = name;
        }
    }
    pub fn toggle_node_hidden(&mut self, nid: u32) {
        if let Some(n) = self.node_mut(nid) {
            n.hidden = !n.hidden;
        }
    }
    pub fn toggle_node_locked(&mut self, nid: u32) {
        if let Some(n) = self.node_mut(nid) {
            n.locked = !n.locked;
        }
    }
}
