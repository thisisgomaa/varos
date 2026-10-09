use super::*;

pub(crate) struct Snap {
    pub(crate) scale_strokes: bool,
    pub(crate) tool: ToolKind,
    pub(crate) name: String,
    pub(crate) sel: bool,
    pub(crate) direct: bool, // Astra F07: no object selection, but a Direct selection (anchors / Direct path) is measured
    pub(crate) drawing: bool, // Pen mid-draft (an open path is active) — drives the "Drawing path…" status (P9)
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) world_w: f32, // A7: the WORLD AABB dims — the 9-pt refpoint X/Y offsets from these (NOT the local W/H,
    pub(crate) world_h: f32, // which are what W/H display), so X/Y is correct for a non-top-left refpoint on a rotated object
    pub(crate) rot: f32,
    pub(crate) fill: Option<Rgba>,
    pub(crate) stroke: Option<Rgba>,
    pub(crate) fill_mixed: bool,
    pub(crate) stroke_mixed: bool,
    pub(crate) sw: f32,
    pub(crate) stroke_style: varos_core::stroke::StrokeStyle,
    pub(crate) stroke_style_mixed: bool,
    pub(crate) stroke_mixed_fields: [bool; 6],
    pub(crate) stroke_error: Option<String>,
    pub(crate) stroke_open: bool,
    pub(crate) stroke_closed: bool,
    pub(crate) stroke_no_tangent: bool,
    pub(crate) opacity: f32,
    pub(crate) clip_exempt: bool, // A30: the selection's clip unit is released from artboard clip
    pub(crate) any_clip: bool,    // any board clips → the "Clip to artboard" toggle is relevant to show
    pub(crate) paint: PaintTarget, // which target has focus (the rail control + X)
    pub(crate) recent: Vec<Rgba>,
    pub(crate) board_colors: Vec<Rgba>,
    pub(crate) doc_colors: Vec<Rgba>, // the picker's swatch strips (MRU + derived document scan)
    pub(crate) has_paint: bool, // a representative path exists (object OR Direct/anchor selection) → show paint, not Document
    // ── the Board section (Start v2 L5) ──
    pub(crate) board_name: String,
    pub(crate) board_description: String,
    pub(crate) board_tags: Vec<String>,
    // ── Document settings (shown when there is truly nothing to inspect — Pain A15) ──
    pub(crate) units_label: &'static str,
    pub(crate) artboards: usize,
    pub(crate) active_artboard: usize,
    pub(crate) artboard_names: Vec<String>,
    pub(crate) guides_locked: bool,
    pub(crate) snap_config: varos_core::model::SnapConfig,
    pub(crate) snap_enabled: bool,
    pub(crate) guides_on: bool, // ruler guides visible (= !guides_hidden)
    pub(crate) rulers_on: bool,
    pub(crate) pathfinder: Result<(), &'static str>, // the boolean buttons' availability + reason (core decides)
}
fn paint_mixed(ed: &Editor, target: PaintTarget) -> bool {
    let mut paints = ed.selected_pids().into_iter().filter_map(|pid| ed.doc.pidx(pid)).map(|pi| {
        let p = &ed.doc.paths[pi];
        match target {
            PaintTarget::Fill => &p.fill,
            PaintTarget::Stroke => &p.stroke,
        }
    });
    paints.next().is_some_and(|first| paints.any(|paint| paint != first))
}
impl Snap {
    pub(crate) fn stroke_field_mixed(&self, field: StrokeField) -> bool {
        self.stroke_mixed_fields[StrokeField::ALL
            .iter()
            .position(|f| std::mem::discriminant(f) == std::mem::discriminant(&field))
            .unwrap_or(0)]
    }

    pub(crate) fn target_mixed(&self, target: PaintTarget) -> bool {
        match target {
            PaintTarget::Fill => self.fill_mixed,
            PaintTarget::Stroke => self.stroke_mixed,
        }
    }
    pub(crate) fn read(ed: &Editor) -> Self {
        let n = ed.objsel.len();
        // Pen mid-draft. The Pen deselects other art when a draft starts (core `pen.rs`), so the selection
        // below already IS the draft (its anchors) — only the header name says what is going on (FB6 nit).
        let drawing = ed.tool == ToolKind::Pen && ed.active.is_some();
        // A7 Stage 5: X/Y = the WORLD AABB top-left (matches `obj_bbox`); W/H = the TRUE un-rotated size
        // (the LOCAL bbox), so a rotated object reports its own dimensions, not its axis-aligned envelope.
        // Astra F07: with NO object selection, a Direct selection (grabbed anchors, or a Direct path-level
        // selection) is measured by `direct_bbox` — a single anchor reads X/Y = its position, W = H = 0 —
        // instead of the zeros that made a successful anchor edit look unselected.
        let direct_bb = if n == 0 { ed.direct_bbox() } else { None };
        let direct = direct_bb.is_some();
        let (sel, x, y, w, h, world_w, world_h) = match (ed.obj_bbox(), ed.obj_local_dims()) {
            (Some((x0, y0, x1, y1)), Some((lw, lh))) if n > 0 => (true, x0, y0, lw, lh, x1 - x0, y1 - y0),
            _ => match direct_bb {
                Some((x0, y0, x1, y1)) => (false, x0, y0, x1 - x0, y1 - y0, x1 - x0, y1 - y0),
                None => (false, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            },
        };
        // fill/stroke/weight/opacity follow the EFFECTIVE paint selection (object sel, a Direct path-level
        // selection, or a selected anchor's path) — not objsel alone, so the Direct tool shows real colours.
        let repr = ed.repr_path();
        let (fill, stroke, sw, opacity) = match repr {
            Some(pi) => {
                let p = &ed.doc.paths[pi];
                (p.fill.solid(), p.stroke.solid(), p.stroke_width, p.opacity)
            }
            None => (ed.cur_fill, ed.cur_stroke, ed.cur_sw, 1.0),
        };
        let name = if drawing {
            "Drawing path\u{2026}".into()
        } else if n == 0 {
            // "Anchor" / "N anchors" / the path's name (or "Path") — see `Editor::direct_label`.
            ed.direct_label().unwrap_or_else(|| "No selection".into())
        } else if n == 1 {
            repr.and_then(|pi| ed.doc.paths[pi].name.clone()).unwrap_or_else(|| "Path".into())
        } else {
            format!("{n} objects")
        };
        let inspection = ed.stroke_inspection.read(ed);
        Snap {
            scale_strokes: ed.select_transform.scale_strokes,
            tool: ed.tool,
            name,
            sel,
            direct,
            drawing,
            x,
            y,
            w,
            h,
            world_w,
            world_h,
            rot: ed.obj_angle.to_degrees(),
            fill,
            stroke,
            fill_mixed: paint_mixed(ed, PaintTarget::Fill),
            stroke_mixed: paint_mixed(ed, PaintTarget::Stroke),
            sw,
            stroke_style: inspection.style,
            stroke_error: ed.stroke_error.clone(),
            stroke_mixed_fields: inspection.fields,
            stroke_style_mixed: inspection.mixed,
            stroke_open: inspection.open,
            stroke_no_tangent: inspection.no_tangent,
            stroke_closed: inspection.closed,
            opacity,
            clip_exempt: ed.sel_clip_exempt(),
            any_clip: ed.doc.artboards.iter().any(|a| a.clip),
            paint: ed.paint,
            recent: ed.recent_colors.clone(),
            board_colors: vec![],
            doc_colors: ed.document_colors(),
            has_paint: repr.is_some(),
            board_name: ed.doc.name.clone(),
            board_description: ed.doc.description.clone(),
            board_tags: ed.doc.tags.clone(),
            units_label: ed.doc.units.display.suffix(),
            artboards: ed.doc.artboards.len(),
            active_artboard: ed.doc.active,
            artboard_names: ed.doc.artboards.iter().map(|a| a.name.clone()).collect(),
            guides_locked: ed.doc.guides_locked,
            snap_config: ed.doc.snap,
            snap_enabled: ed.doc.snap.enabled,
            guides_on: !ed.guides_hidden,
            rulers_on: ed.show_rulers,
            pathfinder: ed.pathfinder_enabled(),
        }
    }
}

/// Read-only snapshot of the ACTIVE artboard for the artboard property panel.
pub(crate) struct AbSnap {
    pub(crate) id: u32,
    pub(crate) count: usize,
    pub(crate) active: usize,
    pub(crate) name: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) color: Option<Rgba>,
    pub(crate) clip: bool,
    pub(crate) move_art: bool,
}
impl AbSnap {
    pub(crate) fn read(ed: &Editor) -> Self {
        let count = ed.doc.artboards.len();
        let active = if count == 0 { 0 } else { ed.doc.active.min(count - 1) };
        let ab = ed.doc.active_artboard();
        AbSnap {
            id: ab.map_or(0, |a| a.id),
            count,
            active,
            name: ab.map(|a| a.name.clone()).unwrap_or_default(),
            x: ab.map(|a| a.x).unwrap_or(0.0),
            y: ab.map(|a| a.y).unwrap_or(0.0),
            w: ab.map(|a| a.w).unwrap_or(0.0),
            h: ab.map(|a| a.h).unwrap_or(0.0),
            color: ab.and_then(|a| a.page_color),
            clip: ab.map(|a| a.clip).unwrap_or(false),
            move_art: ed.doc.move_art_with_ab,
        }
    }
}

/// One artboard's on-canvas label info (for the name + ⋮ chrome painted over the board).
pub(crate) struct AbInfo {
    pub(crate) i: usize,
    pub(crate) name: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) transparent: bool,
    pub(crate) clip: bool,
    pub(crate) hidden: bool,
}
pub(crate) fn ab_infos(ed: &Editor) -> Vec<AbInfo> {
    ed.doc
        .artboards
        .iter()
        .enumerate()
        .map(|(i, a)| AbInfo {
            i,
            name: a.name.clone(),
            x: a.x,
            y: a.y,
            w: a.w,
            h: a.h,
            transparent: a.page_color.is_none(),
            clip: a.clip,
            hidden: a.hidden,
        })
        .collect()
}
