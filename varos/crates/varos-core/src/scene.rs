//! The HARD SEAM: the core describes WHAT to draw as render-agnostic primitives.
//! No wgpu, no triangles, no NDC here — a renderer turns these into pixels however it likes.
//!
//! Two buckets:  `content` = the artwork (fills + real strokes) → scales with zoom.
//!               `overlay` = editing chrome (anchors/handles/skeleton/marquee) → CONSTANT screen size.

use std::collections::HashSet;
use crate::geom::{cubic, Pt, Rgba};
use crate::editor::{Drag, Editor, ToolKind};
use crate::model::Document;

pub const ACCENT: Rgba = [0.047, 0.549, 0.914, 1.0];
pub const ACCENT_FILL: Rgba = [0.047, 0.549, 0.914, 0.14];
pub const HANDLE_COL: Rgba = [0.498, 0.737, 0.941, 1.0];
pub const WHITE: Rgba = [0.96, 0.96, 0.96, 1.0];

pub enum Prim {
    Fill { rings: Vec<Vec<Pt>>, color: Rgba },  // outer ring + hole rings — filled even-odd (holes cut through)
    Stroke { pts: Vec<Pt>, width: f32, color: Rgba },
    Dashed { pts: Vec<Pt>, width: f32, color: Rgba },
    Square { c: Pt, half: f32, color: Rgba },
    Disc { c: Pt, r: f32, color: Rgba },
    Tri { a: Pt, b: Pt, c: Pt, color: Rgba },   // a single filled triangle (icons)
}

#[derive(Default)]
pub struct Scene {
    pub content: Vec<Prim>, // artwork: scales with zoom
    pub overlay: Vec<Prim>, // editing chrome: constant screen size, positions follow the view
}

pub fn build_scene(ed: &Editor, ppu: f32) -> Scene {
    let mut s = Scene::default();

    // ---- CONTENT (scales with zoom) ----  `ppu` = zoom → curves stay smooth at any zoom
    for pi in 0..ed.doc.paths.len() {
        let p = &ed.doc.paths[pi];
        if p.closed && p.anchors.len() >= 3 { if let Some(c) = p.fill {
            let mut rings = vec![ed.doc.outline_px(pi, ppu)];
            for hole in &p.holes { rings.push(Document::ring_px(hole, true, ppu)); }
            s.content.push(Prim::Fill { rings, color: c });
        } }
    }
    for pi in 0..ed.doc.paths.len() {
        let p = &ed.doc.paths[pi];
        if p.anchors.len() >= 2 { if let Some(c) = p.stroke {
            s.content.push(Prim::Stroke { pts: ed.doc.outline_px(pi, ppu), width: p.stroke_width, color: c });
            for hole in &p.holes { let mut r = Document::ring_px(hole, true, ppu); if let Some(&f) = r.first() { r.push(f); } s.content.push(Prim::Stroke { pts: r, width: p.stroke_width, color: c }); }
        } }
    }

    // ---- OVERLAY (constant screen size) ----
    // editing skeleton: a thin accent outline for any path being hovered/selected/drawn
    for pi in 0..ed.doc.paths.len() {
        if ed.doc.paths[pi].anchors.len() >= 2 && ed.path_shown(ed.doc.paths[pi].id) {
            s.overlay.push(Prim::Stroke { pts: ed.doc.outline_px(pi, ppu), width: 1.2, color: ACCENT });
            for hole in &ed.doc.paths[pi].holes { let mut r = Document::ring_px(hole, true, ppu); if let Some(&f) = r.first() { r.push(f); } s.overlay.push(Prim::Stroke { pts: r, width: 1.2, color: ACCENT }); }
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
    // pen rubber-band: dashed, curved
    if ed.tool == ToolKind::Pen {
        if let Some(ap) = ed.active {
            if matches!(ed.drag, Drag::None) {
                if let Some(pi) = ed.doc.pidx(ap) {
                    if let Some(last) = ed.doc.paths[pi].anchors.last() {
                        let c1 = last.hout.unwrap_or(last.p);
                        let mut pts = Vec::with_capacity(49);
                        for k in 0..=48 { pts.push(cubic(last.p, c1, ed.cursor, ed.cursor, k as f32 / 48.0)); }
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
        s.overlay.push(Prim::Stroke { pts: vec![[x0,y0],[x1,y0],[x1,y1],[x0,y1],[x0,y0]], width: 1.0, color: ACCENT });
    }
    // object marquee (dragging the black arrow over empty space)
    if let Drag::ObjMarquee { start, .. } = &ed.drag {
        let c = ed.cursor;
        let (x0, y0) = (start[0].min(c[0]), start[1].min(c[1]));
        let (x1, y1) = (start[0].max(c[0]), start[1].max(c[1]));
        s.overlay.push(Prim::Stroke { pts: vec![[x0,y0],[x1,y0],[x1,y1],[x0,y1],[x0,y0]], width: 1.0, color: ACCENT });
    }
    // handles: like Illustrator, show them only when a SINGLE anchor is selected
    // (selecting many anchors / the whole path shows just the square markers, no handle clutter).
    let mut show: HashSet<u32> = HashSet::new();
    if ed.selected.len() == 1 && ed.tool != ToolKind::Object {
        for p in &ed.doc.paths {
            // every contour: the outer ring + each hole ring (holes are always closed)
            for (contour, closed) in std::iter::once((&p.anchors, p.closed)).chain(p.holes.iter().map(|h| (h, true))) {
                let n = contour.len();
                for (ai, a) in contour.iter().enumerate() {
                    if ed.selected.contains(&a.id) {
                        show.insert(a.id);
                        if ai > 0 { show.insert(contour[ai-1].id); } else if closed { show.insert(contour[n-1].id); }
                        if ai < n-1 { show.insert(contour[ai+1].id); } else if closed { show.insert(contour[0].id); }
                    }
                }
            }
        }
    }
    if ed.tool == ToolKind::Pen { if let Some(ap) = ed.active { if let Some(pi) = ed.doc.pidx(ap) { for a in &ed.doc.paths[pi].anchors { show.insert(a.id); } } } }
    for p in &ed.doc.paths { for a in p.anchors.iter().chain(p.holes.iter().flatten()) { if show.contains(&a.id) {
        for h in [a.hin, a.hout].into_iter().flatten() { s.overlay.push(Prim::Stroke { pts: vec![a.p, h], width: 1.0, color: HANDLE_COL }); }
    }}}
    for p in &ed.doc.paths { for a in p.anchors.iter().chain(p.holes.iter().flatten()) { if show.contains(&a.id) {
        for h in [a.hin, a.hout].into_iter().flatten() { s.overlay.push(Prim::Disc { c: h, r: 4.0, color: HANDLE_COL }); }
    }}}
    // anchor markers (only on shown paths; not in object mode) — outer + hole anchors
    for p in &ed.doc.paths {
        if !ed.path_shown(p.id) || ed.tool == ToolKind::Object { continue; }
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            let sel = ed.selected.contains(&a.id);
            if a.smooth {
                if sel { s.overlay.push(Prim::Disc { c: a.p, r: 5.0, color: ACCENT }); }
                else { s.overlay.push(Prim::Disc { c: a.p, r: 5.5, color: ACCENT }); s.overlay.push(Prim::Disc { c: a.p, r: 4.0, color: WHITE }); }
            } else if sel { s.overlay.push(Prim::Square { c: a.p, half: 4.5, color: ACCENT }); }
            else { s.overlay.push(Prim::Square { c: a.p, half: 5.0, color: ACCENT }); s.overlay.push(Prim::Square { c: a.p, half: 3.6, color: WHITE }); }
        }
    }
    s
}
