//! The HARD SEAM: the core describes WHAT to draw as render-agnostic primitives.
//! No wgpu, no triangles, no NDC here — a renderer turns these into pixels however it likes.
//!
//! Two buckets:  `content` = the artwork (fills + real strokes) → scales with zoom.
//!               `overlay` = editing chrome (anchors/handles/skeleton/marquee) → CONSTANT screen size.

use crate::editor::{Drag, Editor, SnapGuide, ToolKind};
use crate::geom::{cubic, dist, Pt, Rgba};
use crate::model::Document;
use std::collections::HashSet;

pub const ACCENT: Rgba = [0.047, 0.549, 0.914, 1.0];
pub const ACCENT_FILL: Rgba = [0.047, 0.549, 0.914, 0.14];
pub const HANDLE_COL: Rgba = [0.498, 0.737, 0.941, 1.0];
pub const WHITE: Rgba = [0.96, 0.96, 0.96, 1.0];
pub const PAPER: Rgba = [1.0, 1.0, 1.0, 1.0]; // artboard page fill (white) — default page colour
pub const AB_GHOST: Rgba = [1.0, 1.0, 1.0, 0.06]; // a TRANSPARENT page → a faint translucent white, so the
                                                  // page still reads on the dark board (not invisible)
pub const AB_EDGE: Rgba = [0.0, 0.0, 0.0, 0.34]; // page hairline on a WHITE page (clear dark line)
pub const AB_EDGE_T: Rgba = [1.0, 1.0, 1.0, 0.34]; // page hairline on a transparent page (light, on the dark board)
pub const SNAP_GUIDE: Rgba = [0.05, 0.92, 0.55, 1.0]; // smart-guide green (vivid; adjustable per Ahmed)
pub const SEG_HI: Rgba = [0.35, 0.80, 1.0, 1.0]; // grabbed/selected path segment — bright cyan (Illustrator feel)
pub const GUIDE: Rgba = [0.0, 0.72, 0.92, 0.9]; // ruler guide line — cyan (Illustrator default)

pub enum Prim {
    Fill { rings: Vec<Vec<Pt>>, color: Rgba }, // outer ring + hole rings — filled even-odd (holes cut through)
    Stroke { pts: Vec<Pt>, width: f32, color: Rgba },
    Dashed { pts: Vec<Pt>, width: f32, color: Rgba },
    Square { c: Pt, half: f32, color: Rgba },
    Disc { c: Pt, r: f32, color: Rgba },
    Tri { a: Pt, b: Pt, c: Pt, color: Rgba }, // a single filled triangle (icons)
}

/// A z-ordered draw group. `Opaque` runs paint straight onto the canvas. `Isolated` renders its prims
/// to an offscreen buffer OPAQUELY, then composites the whole buffer at `opacity` — Illustrator group
/// opacity, so an object's fill+stroke fade as ONE unit instead of double-blending each other.
/// `Knockout` = ONE object whose translucent stroke must KNOCK OUT the fill beneath it (Illustrator/PDF
/// knockout): the band blends against what's BEHIND the object — never against the object's own fill.
pub enum Group {
    Opaque(Vec<Prim>),
    Knockout(Vec<Prim>),
    Isolated { opacity: f32, prims: Vec<Prim> },
}
impl Group {
    pub fn prims(&self) -> &[Prim] {
        match self {
            Group::Opaque(p) | Group::Knockout(p) => p,
            Group::Isolated { prims, .. } => prims,
        }
    }
}

#[derive(Default)]
pub struct Scene {
    pub content: Vec<Group>, // artwork groups (z-ordered): opaque runs + isolated translucent layers
    pub overlay: Vec<Prim>,  // editing chrome: constant screen size, positions follow the view
}

/// Multiply a primitive's colour alpha — folds object-opacity into a single-primitive object (no overlap
/// to double-blend, so no isolated layer needed).
fn scale_alpha(p: &mut Prim, o: f32) {
    let c = match p {
        Prim::Fill { color, .. } => color,
        Prim::Stroke { color, .. } => color,
        Prim::Dashed { color, .. } => color,
        Prim::Square { color, .. } => color,
        Prim::Disc { color, .. } => color,
        Prim::Tri { color, .. } => color,
    };
    c[3] *= o;
}

pub fn build_scene(ed: &Editor, ppu: f32) -> Scene {
    let mut s = Scene::default();
    // content = z-ordered Groups. Opaque prims accumulate into the current run in PER-OBJECT paint order
    // (each object's fill immediately followed by its own stroke — Illustrator stacking: an object above
    // covers the stroke of the one below). A translucent fill+stroke object flushes the run and becomes
    // its own isolated layer (group opacity).
    let mut groups: Vec<Group> = Vec::new();
    let mut open: Vec<Prim> = Vec::new();

    // ---- ARTBOARDS (the pages) ---- DEFINED rectangles sitting on the infinite dotted board. Each is a
    // page-colour Fill (pushed first → over the grid, behind artwork) unless TRANSPARENT, plus a boundary
    // hairline in the overlay (constant screen width, never scales). In the Artboard tool the ACTIVE page
    // gets an accent border + 8 resize handles. No drop shadow for now (kept light).
    {
        let ab_tool = ed.tool == ToolKind::Artboard;
        for (i, ab) in ed.doc.artboards.iter().enumerate() {
            let (x0, y0, x1, y1) = ab.rect();
            let ring = vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
            // page fill: a solid colour, or — when transparent — a faint translucent white so the page
            // still reads on the dark board instead of vanishing into it.
            let paper = ab.page_color.unwrap_or(AB_GHOST);
            open.push(Prim::Fill { rings: vec![ring.clone()], color: paper });
            let active = ab_tool && i == ed.doc.active;
            let edge_col = if active {
                ACCENT
            } else if ab.page_color.is_none() {
                AB_EDGE_T
            } else {
                AB_EDGE
            };
            let mut edge = ring;
            edge.push([x0, y0]);
            // the page you're standing on gets a clearly heavier frame (~2× the others) + handles.
            s.overlay.push(Prim::Stroke { pts: edge, width: if active { 2.4 } else { 1.2 }, color: edge_col });
            if active {
                for h in Editor::bbox_handles((x0, y0, x1, y1)) {
                    s.overlay.push(Prim::Square { c: h, half: 5.0, color: ACCENT });
                    s.overlay.push(Prim::Square { c: h, half: 3.2, color: WHITE });
                }
            }
        }
    }

    // ---- CONTENT (scales with zoom): built into z-ordered Groups ----  `ppu` = zoom → curves stay smooth
    // Colour alpha is honoured as-is. OBJECT opacity < 1 is the special case: fill+stroke must composite as
    // ONE unit then fade together (Illustrator group opacity), so a translucent fill+stroke object becomes
    // an isolated layer. With only a fill OR only a stroke there is no overlap to double-blend, so we just
    // fold the opacity into that single colour's alpha and keep it in the fast opaque run.
    // CLIP: if any page has clip on, artwork overlapping it is cut to that page's rect. None ⇒ no clip.
    let any_clip = ed.doc.artboards.iter().any(|a| a.clip);
    let clip_rect = |pi: usize| -> Option<(f32, f32, f32, f32)> {
        if !any_clip {
            return None;
        }
        let b = ed.doc.outline_bbox(pi);
        ed.doc
            .artboards
            .iter()
            .filter(|a| a.clip)
            .map(|a| a.rect())
            .find(|r| r.0 <= b.2 && r.2 >= b.0 && r.1 <= b.3 && r.3 >= b.1)
    };
    let fill_prims = |pi: usize| -> Vec<Prim> {
        let p = &ed.doc.paths[pi];
        let mut out = Vec::new();
        if p.closed && p.anchors.len() >= 3 {
            if let Some(c) = p.fill {
                let mut rings = vec![ed.doc.outline_px(pi, ppu)];
                for hole in &p.holes {
                    rings.push(Document::ring_px(hole, true, ppu));
                }
                match clip_rect(pi) {
                    Some(r) => {
                        let clipped: Vec<Vec<Pt>> =
                            rings.iter().map(|ring| clip_poly_rect(ring, r)).filter(|ring| ring.len() >= 3).collect();
                        if clipped.first().is_some_and(|o| o.len() >= 3) {
                            out.push(Prim::Fill { rings: clipped, color: c });
                        }
                    }
                    None => out.push(Prim::Fill { rings, color: c }),
                }
            }
        }
        out
    };
    let stroke_prims = |pi: usize| -> Vec<Prim> {
        let p = &ed.doc.paths[pi];
        let mut out = Vec::new();
        if p.anchors.len() >= 2 {
            if let Some(c) = p.stroke {
                let clip = clip_rect(pi);
                let mut push = |pts: Vec<Pt>| match clip {
                    Some(r) => {
                        for run in clip_polyline_rect(&pts, r) {
                            if run.len() >= 2 {
                                out.push(Prim::Stroke { pts: run, width: p.stroke_width, color: c });
                            }
                        }
                    }
                    None => out.push(Prim::Stroke { pts, width: p.stroke_width, color: c }),
                };
                push(ed.doc.outline_px(pi, ppu));
                for hole in &p.holes {
                    let mut r = Document::ring_px(hole, true, ppu);
                    if let Some(&f) = r.first() {
                        r.push(f);
                    }
                    push(r);
                }
            }
        }
        out
    };
    // the CONTENT pass reads paint_list(), not doc.paths — the §5 indirection: a future mask/page
    // filter lands there once, and this loop (plus export + snap) follows for free
    for (pi, p) in ed.doc.paint_list() {
        if ed.doc.eff_hidden(p.id) {
            continue;
        } // cascades from layer/group eyes
        let o = p.opacity;
        let s_alpha = p.stroke.map_or(1.0, |c| c[3]);
        let mut fp = fill_prims(pi);
        let mut sp = stroke_prims(pi);
        if o < 0.999 && !fp.is_empty() && !sp.is_empty() {
            // isolated layer: flush the current opaque run, then emit the object as one unit (fill(s) then stroke(s))
            if !open.is_empty() {
                groups.push(Group::Opaque(std::mem::take(&mut open)));
            }
            let mut lp = fp;
            lp.append(&mut sp);
            groups.push(Group::Isolated { opacity: o, prims: lp });
        } else if !fp.is_empty() && !sp.is_empty() && s_alpha < 0.999 {
            // translucent stroke on a filled object → knockout: the band must blend against what's BEHIND
            // the object, never against the object's own fill (the fill is cut away under the band)
            if !open.is_empty() {
                groups.push(Group::Opaque(std::mem::take(&mut open)));
            }
            let mut lp = fp;
            lp.append(&mut sp);
            groups.push(Group::Knockout(lp));
        } else if o < 0.999 {
            // single-primitive translucent → fold opacity into the colour's own alpha, stay in the run
            for mut pr in fp.drain(..) {
                scale_alpha(&mut pr, o);
                open.push(pr);
            }
            for mut pr in sp.drain(..) {
                scale_alpha(&mut pr, o);
                open.push(pr);
            }
        } else {
            open.append(&mut fp);
            open.append(&mut sp);
        }
    }
    if !open.is_empty() {
        groups.push(Group::Opaque(open));
    }
    s.content = groups;

    // ---- OVERLAY (constant screen size) ----
    // ruler guides (cyan, full-extent world lines) + the live ruler drag-out preview — unless hidden
    if !ed.guides_hidden {
        const BIG: f32 = 1.0e5;
        for g in ed.doc.guides.iter().chain(ed.guide_preview.iter()) {
            let (a, b) = if g.vertical { ([g.pos, -BIG], [g.pos, BIG]) } else { ([-BIG, g.pos], [BIG, g.pos]) };
            s.overlay.push(Prim::Stroke { pts: vec![a, b], width: 1.0, color: GUIDE });
        }
    }
    // editing skeleton: a thin accent outline for any path being hovered/selected/drawn
    for pi in 0..ed.doc.paths.len() {
        if ed.doc.eff_hidden(ed.doc.paths[pi].id) {
            continue;
        } // cascades from layer/group eyes
        if ed.doc.paths[pi].anchors.len() >= 2 && ed.path_shown(ed.doc.paths[pi].id) {
            s.overlay.push(Prim::Stroke { pts: ed.doc.outline_px(pi, ppu), width: 1.7, color: ACCENT });
            for hole in &ed.doc.paths[pi].holes {
                let mut r = Document::ring_px(hole, true, ppu);
                if let Some(&f) = r.first() {
                    r.push(f);
                }
                s.overlay.push(Prim::Stroke { pts: r, width: 1.7, color: ACCENT });
            }
        }
    }
    // grabbed segment highlight: the moment you grab a path segment (Direct tool), the segment itself
    // lights up bright cyan — so you FEEL you caught the path, even on a straight edge with no handles.
    if let Drag::Segment { pid, i, .. } = ed.drag {
        if let Some(pi) = ed.doc.pidx(pid) {
            let p = &ed.doc.paths[pi];
            let n = p.anchors.len();
            if n >= 2 {
                let a = &p.anchors[i];
                let b = &p.anchors[(i + 1) % n];
                let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
                let pts: Vec<Pt> = (0..=24).map(|k| cubic(p0, p1, p2, p3, k as f32 / 24.0)).collect();
                s.overlay.push(Prim::Stroke { pts, width: 3.0, color: SEG_HI });
            }
        }
    }
    // object-selection transform frame (oriented — rotates with the selection) + 8 handles
    if ed.tool == ToolKind::Object && !matches!(ed.drag, Drag::ObjMarquee { .. }) {
        if let (Some(c), Some(hs)) = (ed.frame_corners(), ed.frame_handles()) {
            s.overlay.push(Prim::Stroke { pts: vec![c[0], c[1], c[2], c[3], c[0]], width: 1.0, color: ACCENT });
            for h in hs {
                s.overlay.push(Prim::Square { c: h, half: 4.0, color: ACCENT });
                s.overlay.push(Prim::Square { c: h, half: 2.6, color: WHITE });
            }
        }
    }
    // Rotate/Scale: the transform pivot (bullseye) — the origin a drag transforms around; click to move it
    if matches!(ed.tool, ToolKind::Rotate | ToolKind::Scale) && !ed.objsel.is_empty() {
        if let Some(c) = ed.pivot_point() {
            s.overlay.push(Prim::Disc { c, r: 6.0, color: WHITE });
            s.overlay.push(Prim::Disc { c, r: 4.5, color: ACCENT });
            s.overlay.push(Prim::Disc { c, r: 1.6, color: WHITE });
        }
    }
    // pen rubber-band: dashed, curved
    if ed.tool == ToolKind::Pen {
        if let Some(ap) = ed.active {
            if matches!(ed.drag, Drag::None) {
                if let Some(pi) = ed.doc.pidx(ap) {
                    if let Some(last) = ed.doc.paths[pi].anchors.last() {
                        let c1 = last.hout.unwrap_or(last.p);
                        let mut pts = Vec::with_capacity(49);
                        for k in 0..=48 {
                            pts.push(cubic(last.p, c1, ed.cursor, ed.cursor, k as f32 / 48.0));
                        }
                        s.overlay.push(Prim::Dashed { pts, width: 1.5, color: ACCENT });
                    }
                }
            }
        }
    }
    // marquee
    if let Drag::Marquee { start, .. } = &ed.drag {
        let c = ed.cursor;
        let (x0, y0) = (start[0].min(c[0]), start[1].min(c[1]));
        let (x1, y1) = (start[0].max(c[0]), start[1].max(c[1]));
        s.overlay.push(Prim::Stroke {
            pts: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]],
            width: 1.0,
            color: ACCENT,
        });
    }
    // object marquee (dragging the black arrow over empty space)
    if let Drag::ObjMarquee { start, .. } = &ed.drag {
        let c = ed.cursor;
        let (x0, y0) = (start[0].min(c[0]), start[1].min(c[1]));
        let (x1, y1) = (start[0].max(c[0]), start[1].max(c[1]));
        s.overlay.push(Prim::Stroke {
            pts: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]],
            width: 1.0,
            color: ACCENT,
        });
    }
    // handles: every SELECTED anchor shows its own direction handles (Illustrator). A whole-path / hover
    // selection shows none — grabbing a segment selects its two endpoints, so both reveal handles naturally.
    let mut show: HashSet<u32> = if ed.tool == ToolKind::Object { HashSet::new() } else { ed.selected.clone() };
    if ed.tool == ToolKind::Pen {
        if let Some(ap) = ed.active {
            if let Some(pi) = ed.doc.pidx(ap) {
                for a in &ed.doc.paths[pi].anchors {
                    show.insert(a.id);
                }
            }
        }
    }
    for p in &ed.doc.paths {
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            if show.contains(&a.id) {
                for h in [a.hin, a.hout].into_iter().flatten() {
                    s.overlay.push(Prim::Stroke { pts: vec![a.p, h], width: 1.0, color: HANDLE_COL });
                }
            }
        }
    }
    for p in &ed.doc.paths {
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            if show.contains(&a.id) {
                for h in [a.hin, a.hout].into_iter().flatten() {
                    s.overlay.push(Prim::Disc { c: h, r: 4.0, color: HANDLE_COL });
                }
            }
        }
    }
    // anchor markers — only on SELECTED paths (never on mere hover), and not in object mode — outer + hole anchors
    for p in &ed.doc.paths {
        if ed.doc.eff_hidden(p.id) || !ed.path_selected(p.id) || ed.tool == ToolKind::Object {
            continue;
        }
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            let sel = ed.selected.contains(&a.id);
            if a.smooth {
                if sel {
                    s.overlay.push(Prim::Disc { c: a.p, r: 5.0, color: ACCENT });
                } else {
                    s.overlay.push(Prim::Disc { c: a.p, r: 5.5, color: ACCENT });
                    s.overlay.push(Prim::Disc { c: a.p, r: 4.0, color: WHITE });
                }
            } else if sel {
                s.overlay.push(Prim::Square { c: a.p, half: 4.5, color: ACCENT });
            } else {
                s.overlay.push(Prim::Square { c: a.p, half: 5.0, color: ACCENT });
                s.overlay.push(Prim::Square { c: a.p, half: 3.6, color: WHITE });
            }
        }
    }

    // ---- SNAP GUIDES (smart-guide feedback) — world-positioned lines, drawn at constant screen width ----
    for g in &ed.snap_guides {
        match g {
            SnapGuide::Line { a, b } => {
                s.overlay.push(Prim::Stroke { pts: vec![*a, *b], width: 1.6, color: SNAP_GUIDE })
            }
            SnapGuide::Gap { a, b } => {
                s.overlay.push(Prim::Stroke { pts: vec![*a, *b], width: 1.6, color: SNAP_GUIDE });
                // perpendicular end-ticks so the gap reads as a distance bar
                let d = [b[0] - a[0], b[1] - a[1]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
                let n = [-d[1] / l * 4.0, d[0] / l * 4.0];
                for p in [a, b] {
                    s.overlay.push(Prim::Stroke {
                        pts: vec![[p[0] - n[0], p[1] - n[1]], [p[0] + n[0], p[1] + n[1]]],
                        width: 1.6,
                        color: SNAP_GUIDE,
                    });
                }
            }
            // a snapped POINT: a constant-screen-size green marker with a white core (clear at any zoom)
            SnapGuide::Point { p } => {
                s.overlay.push(Prim::Square { c: *p, half: 5.0, color: SNAP_GUIDE });
                s.overlay.push(Prim::Square { c: *p, half: 3.0, color: WHITE });
            }
            // a whole snapped PATH lights up (Illustrator's "path" highlight)
            SnapGuide::PathHi { pid } => {
                if let Some(pi) = ed.doc.pidx(*pid) {
                    s.overlay.push(Prim::Stroke { pts: ed.doc.outline_px(pi, ppu), width: 2.0, color: SNAP_GUIDE });
                }
            }
        }
    }
    // ---- CENTER POINT of the object selection (Illustrator "Show Center"), tied to the 9-pt reference ----
    if ed.tool == ToolKind::Object && !ed.objsel.is_empty() {
        if let Some((x0, y0, x1, y1)) = ed.obj_bbox() {
            s.overlay.push(Prim::Disc { c: [(x0 + x1) * 0.5, (y0 + y1) * 0.5], r: 2.5, color: ACCENT });
        }
    }

    s
}

// ───────────────────────── clip-to-artboard geometry (pure) ─────────────────────────

/// Sutherland–Hodgman: clip a polygon to an axis-aligned rect `(x0,y0,x1,y1)`. Returns the clipped
/// polygon (possibly empty). Used for filled artwork when its page has "clip to artboard" on.
fn clip_poly_rect(poly: &[Pt], (x0, y0, x1, y1): (f32, f32, f32, f32)) -> Vec<Pt> {
    let mut poly = poly.to_vec();
    // four half-planes: x>=x0, x<=x1, y>=y0, y<=y1   (which: 0 left, 1 right, 2 top, 3 bottom)
    for (which, v) in [(0u8, x0), (1, x1), (2, y0), (3, y1)] {
        if poly.is_empty() {
            break;
        }
        let inside = |p: &Pt| match which {
            0 => p[0] >= v,
            1 => p[0] <= v,
            2 => p[1] >= v,
            _ => p[1] <= v,
        };
        let cut = |a: &Pt, b: &Pt| -> Pt {
            if which <= 1 {
                let t = (v - a[0]) / (b[0] - a[0]);
                [v, a[1] + t * (b[1] - a[1])]
            } else {
                let t = (v - a[1]) / (b[1] - a[1]);
                [a[0] + t * (b[0] - a[0]), v]
            }
        };
        let input = std::mem::take(&mut poly);
        let n = input.len();
        for i in 0..n {
            let cur = input[i];
            let prev = input[(i + n - 1) % n];
            let (ci, pi) = (inside(&cur), inside(&prev));
            if ci {
                if !pi {
                    poly.push(cut(&prev, &cur));
                }
                poly.push(cur);
            } else if pi {
                poly.push(cut(&prev, &cur));
            }
        }
    }
    poly
}

/// Cohen–Sutherland region code of a point against a rect.
fn outcode(p: Pt, (x0, y0, x1, y1): (f32, f32, f32, f32)) -> u8 {
    let mut c = 0u8;
    if p[0] < x0 {
        c |= 1;
    } else if p[0] > x1 {
        c |= 2;
    }
    if p[1] < y0 {
        c |= 4;
    } else if p[1] > y1 {
        c |= 8;
    }
    c
}
/// Cohen–Sutherland: the portion of segment a→b inside the rect, or None if it misses entirely.
fn clip_seg_rect(mut a: Pt, mut b: Pt, r: (f32, f32, f32, f32)) -> Option<(Pt, Pt)> {
    let (x0, y0, x1, y1) = r;
    let (mut ca, mut cb) = (outcode(a, r), outcode(b, r));
    for _ in 0..8 {
        if ca | cb == 0 {
            return Some((a, b));
        }
        if ca & cb != 0 {
            return None;
        }
        let c = if ca != 0 { ca } else { cb };
        let p = if c & 8 != 0 {
            [a[0] + (b[0] - a[0]) * (y1 - a[1]) / (b[1] - a[1]), y1]
        } else if c & 4 != 0 {
            [a[0] + (b[0] - a[0]) * (y0 - a[1]) / (b[1] - a[1]), y0]
        } else if c & 2 != 0 {
            [x1, a[1] + (b[1] - a[1]) * (x1 - a[0]) / (b[0] - a[0])]
        } else {
            [x0, a[1] + (b[1] - a[1]) * (x0 - a[0]) / (b[0] - a[0])]
        };
        if c == ca {
            a = p;
            ca = outcode(a, r);
        } else {
            b = p;
            cb = outcode(b, r);
        }
    }
    None
}
/// Clip an (open or closed) polyline to a rect → the inside runs (a stroke can split into several).
fn clip_polyline_rect(pts: &[Pt], r: (f32, f32, f32, f32)) -> Vec<Vec<Pt>> {
    let mut runs: Vec<Vec<Pt>> = vec![];
    let mut cur: Vec<Pt> = vec![];
    for i in 0..pts.len().saturating_sub(1) {
        match clip_seg_rect(pts[i], pts[i + 1], r) {
            Some((a, b)) => {
                if cur.last().is_some_and(|l| dist(*l, a) < 1e-3) {
                    cur.push(b);
                } else {
                    if cur.len() >= 2 {
                        runs.push(std::mem::take(&mut cur));
                    } else {
                        cur.clear();
                    }
                    cur.push(a);
                    cur.push(b);
                }
            }
            None => {
                if cur.len() >= 2 {
                    runs.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
    }
    if cur.len() >= 2 {
        runs.push(cur);
    }
    runs
}
