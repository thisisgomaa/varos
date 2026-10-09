//! The PURE PDF export (DFS S6): a deliverable for sharing. It carries the visible artwork and nothing
//! else — no embedded model, no /AF, no /Names, no /VAROS_* keys, no Info dictionary, no object or
//! board names, and nothing hidden. Pages are PLANNED first (`plan_pdf_export`) so the app can show
//! the page count, or a plain-English reason when a scope cannot be exported, before any bytes exist.
//!
//! Masks: clip-group members are written inside a PDF clip (`q … W* n … Q`, MASKS_PLAN Stage 5), in
//! the shared page loop (`crate::write`), so the export and the native `.vrs` pages both match the canvas.

use std::fmt;
use std::sync::atomic::AtomicBool;

use varos_core::model::{Artboard, Document, GroupRole, Paint};
use varos_core::Rgba;

use crate::write::{drawable, mask_paths, write_pages};

/// Which pages an export produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportScope {
    /// Every visible artboard, in document order (the default when the document has boards).
    AllVisibleArtboards,
    /// The active artboard only.
    ActiveArtboard,
    /// One page fitted to the visible artwork — only for a document without artboards.
    ArtworkBounds,
    /// Export Selection… (slice 0.6): one transparent page fitted to the SELECTED visible artwork,
    /// with or without artboards. It needs the selection, so it is planned by
    /// [`plan_selection_export`]; `plan_pdf_export` (which has no selection) answers `NoSelection`.
    Selection,
}

/// One output page: a world rect `[x, y, w, h]` (points, Y down) and its background (`None` = transparent).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSpec {
    pub rect: [f32; 4],
    pub background: Option<Rgba>,
    /// Resolved source-board bleed; bounds/selection pages have no board bleed.
    pub bleed: f32,
    /// Canonical document setup edges: top, right, bottom, left.
    pub bleed_edges: [f32; 4],
}
impl PageSpec {
    /// The page an artboard prints as.
    pub fn of_board(ab: &Artboard) -> PageSpec {
        PageSpec {
            rect: [ab.x, ab.y, ab.w, ab.h],
            background: ab.page_color,
            bleed: ab.bleed.max(0.0),
            bleed_edges: varos_core::document_setup::bleed(ab),
        }
    }
}

/// The pages a scope will export, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportPlan {
    pub scope: ExportScope,
    pub pages: Vec<PageSpec>,
}
impl ExportPlan {
    /// How many pages the PDF will have.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

/// Why a scope cannot be exported. `reason()` is the user-facing copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportUnavailable {
    NoVisibleArtboards,
    ActiveArtboardHidden,
    NotBoardless,
    NeedsArtboards,
    NothingToExport,
    NoSelection,
}
impl ExportUnavailable {
    pub fn reason(&self) -> &'static str {
        match self {
            ExportUnavailable::NoVisibleArtboards => "Every artboard is hidden. Show an artboard to export it.",
            ExportUnavailable::ActiveArtboardHidden => "The active artboard is hidden. Show it to export it.",
            ExportUnavailable::NotBoardless => "Artwork bounds is only for documents without artboards.",
            ExportUnavailable::NeedsArtboards => "This document has no artboards. Export its artwork bounds instead.",
            ExportUnavailable::NothingToExport => "There is no visible artwork to export.",
            ExportUnavailable::NoSelection => "Select something to export it.",
        }
    }
}
impl fmt::Display for ExportUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason())
    }
}

/// Why `export_pdf_bytes` produced no bytes. (Writing the bytes to disk is the caller's job — S6-B maps
/// its own file errors; this crate never touches the destination.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    Cancelled,
    InvalidDocument(String),
    LimitExceeded,
    Unavailable(ExportUnavailable),
}
impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::InvalidDocument(e) => write!(f, "Cannot export this board: {e}"),
            ExportError::LimitExceeded => f.write_str("limit_exceeded: stroke geometry budget"),
            ExportError::Cancelled => f.write_str("The export was cancelled."),
            ExportError::Unavailable(u) => f.write_str(u.reason()),
        }
    }
}

/// The scope an Export home opens on: all visible artboards when there are boards, else the artwork bounds.
pub fn default_scope(doc: &Document) -> ExportScope {
    if doc.artboards.is_empty() {
        ExportScope::ArtworkBounds
    } else {
        ExportScope::AllVisibleArtboards
    }
}

/// Plan the pages `scope` exports, or say why it can't. Never a dummy page.
pub fn plan_pdf_export(doc: &Document, scope: ExportScope) -> Result<ExportPlan, ExportUnavailable> {
    // ---- Lane B w3-effects ----
    let resolved = varos_core::effects_document::document(doc).map_err(|_| ExportUnavailable::NothingToExport)?;
    let doc = resolved.as_ref();
    // ---- end Lane B w3-effects ----
    // ---- Lane G ----
    let outlined;
    let doc = if doc.text_boxes.is_empty() {
        doc
    } else {
        outlined = varos_text_layout::outline_document(doc).map_err(|_| ExportUnavailable::NothingToExport)?;
        &outlined
    };
    let pages: Vec<PageSpec> = match scope {
        ExportScope::AllVisibleArtboards => {
            if doc.artboards.is_empty() {
                return Err(ExportUnavailable::NeedsArtboards);
            }
            let vis: Vec<PageSpec> = doc.artboards.iter().filter(|a| !a.hidden).map(PageSpec::of_board).collect();
            if vis.is_empty() {
                return Err(ExportUnavailable::NoVisibleArtboards);
            }
            vis
        }
        ExportScope::ActiveArtboard => {
            let ab = doc.active_artboard().ok_or(ExportUnavailable::NeedsArtboards)?;
            if ab.hidden {
                return Err(ExportUnavailable::ActiveArtboardHidden);
            }
            vec![PageSpec::of_board(ab)]
        }
        ExportScope::ArtworkBounds => {
            if !doc.artboards.is_empty() {
                return Err(ExportUnavailable::NotBoardless);
            }
            vec![bounds_page(doc, Reach::HalfStroke).ok_or(ExportUnavailable::NothingToExport)?]
        }
        ExportScope::Selection => return Err(ExportUnavailable::NoSelection),
    };
    Ok(ExportPlan { scope, pages })
}

/// Export Selection… (Illustrator): the document NARROWED to the selected paths `selected` — every
/// other path is hidden in the copy, mask geometry stays (it still shapes its clip, as on the canvas),
/// and a selected path that paints nothing (opacity 0, or no visible fill and stroke) is hidden too so
/// it never stretches the page — and its plan: one transparent page fitted to what is left (the
/// `ArtworkBounds` rule: hidden art skipped, padded by half the stroke width, a clipped member only
/// where it meets its mask), whether or not the document has artboards. Export the returned document
/// with the returned plan; the caller's document is untouched.
pub fn plan_selection_export(
    doc: &Document,
    selected: &std::collections::HashSet<u32>,
) -> Result<(Document, ExportPlan), ExportUnavailable> {
    if selected.is_empty() {
        return Err(ExportUnavailable::NoSelection);
    }
    let mut narrowed = doc.clone();
    // mask geometry is looked up only when a clip exists (`is_mask_source` scans the node arena)
    let masks: std::collections::HashSet<u32> = if doc.nodes.iter().any(|n| n.role == GroupRole::Clip) {
        doc.paths.iter().map(|p| p.id).filter(|&id| doc.is_mask_source(id)).collect()
    } else {
        Default::default()
    };
    for p in &mut narrowed.paths {
        let unselected = !selected.contains(&p.id) && !masks.contains(&p.id);
        if unselected || (!masks.contains(&p.id) && paints_nothing(p)) {
            p.hidden = true;
        }
    }
    varos_core::images::hide_unselected(&mut narrowed, selected);
    // the page reaches as far as the selection PAINTS: the outline grown by the shared painted extent
    // (`varos_core::geom::painted_padding` — the one rule cull and hit-test use too)
    let page = bounds_page(&narrowed, Reach::Painted).ok_or(ExportUnavailable::NothingToExport)?;
    Ok((narrowed, ExportPlan { scope: ExportScope::Selection, pages: vec![page] }))
}

/// Write the pure PDF for `plan`: its pages and a bare catalog, nothing else. `cancel` is checked
/// before every page. Deterministic: the same document and plan give the same bytes.
pub fn export_pdf_bytes(doc: &Document, plan: &ExportPlan, cancel: &AtomicBool) -> Result<Vec<u8>, ExportError> {
    export_pdf_bytes_with_report(doc, plan, cancel).map(|(output, _)| output)
}

/// Export bytes together with explicit diagnostics.
pub fn export_pdf_bytes_with_report(
    doc: &Document,
    plan: &ExportPlan,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, varos_core::ExportReport), ExportError> {
    if plan.pages.is_empty() {
        return Err(ExportError::Unavailable(ExportUnavailable::NothingToExport));
    }
    let bytes = write_pages(doc, &plan.pages, None, cancel)?;
    let mut report = varos_core::ExportReport::default();
    // ---- w3-cmyk ----
    report.notes.extend(crate::colour_management::notes(doc));
    // ---- Lane G ----
    if !doc.text_boxes.is_empty() {
        report.notes.extend(varos_text_layout::export_notes(doc).map_err(ExportError::InvalidDocument)?);
    }
    for p in &doc.paths {
        if [p.appearance().fill(), p.appearance().stroke()]
            .iter()
            .any(|p| matches!(p.resolved(doc), varos_core::model::Paint::Gradient(_)))
        {
            report.notes.push(varos_core::ExportNote {
                kind: "gradient_sampled".into(),
                object_id: Some(p.id),
                message:
                    "PDF: axial/radial shading uses a 4096-sample, 16-bit function; midpoint and spread are sampled"
                        .into(),
            });
        }
        if !p.stroke_style.is_default()
            || matches!(p.appearance().stroke().resolved(doc), varos_core::model::Paint::Gradient(_))
        {
            let coverage = varos_core::stroke::evaluate(p, 0.01, &|| cancel.load(std::sync::atomic::Ordering::Relaxed))
                .map_err(stroke_error)?;
            report.notes.extend(coverage.report.notes);
            if !crate::write::native_stroke(p)
                || matches!(p.appearance().stroke().resolved(doc), varos_core::model::Paint::Gradient(_))
            {
                report.notes.push(varos_core::ExportNote {
                    kind: "stroke_baked".into(),
                    object_id: Some(p.id),
                    message: "PDF: aligned, fitted, dotted, degenerate or arrowed coverage".into(),
                });
            }
        }
    }
    Ok((bytes, report))
}

/// Does this file carry an embedded Varos model (a native `.vrs` container)? Bounded byte scan for
/// `/VAROS_Model` or `model.varos.json`; no PDF parse (the bytes come from a file the user picked, so
/// nothing here may allocate or recurse on its content). It feeds a warning only, so a false positive
/// just asks one extra question. At most the first `HAS_MODEL_SCAN_CAP` bytes are scanned. Cost: about
/// 0.4 s at the full 256 MiB cap (release build, measured in the S6-A code review), so callers must run
/// it off the UI thread (S6-B: inside the export job).
pub fn has_embedded_model(bytes: &[u8]) -> bool {
    let hay = &bytes[..bytes.len().min(HAS_MODEL_SCAN_CAP)];
    [b"/VAROS_Model".as_slice(), b"model.varos.json".as_slice()]
        .iter()
        .any(|needle| hay.windows(needle.len()).any(|w| w == *needle))
}
/// The scan cap for `has_embedded_model` — the same 256 MiB the Export flow reads a destination up to.
pub const HAS_MODEL_SCAN_CAP: usize = 256 * 1024 * 1024;

/// How far past its outline a path reaches on a bounds page ([`bounds_page`]).
#[derive(Clone, Copy)]
enum Reach {
    HalfStroke,
    Painted,
}

/// A path that leaves no mark: fully transparent (opacity 0), or neither a visible fill nor a visible
/// stroke. Exhaustive over `Paint`, so a new paint kind must decide here.
fn paints_nothing(p: &varos_core::model::Path) -> bool {
    let alpha = |paint: &Paint| match paint {
        Paint::None => 0.0,
        // ---- w3-cmyk ----
        Paint::Managed(c) => c.alpha,
        Paint::Solid(c) => c[3],
        Paint::Gradient(g) => g.stops.iter().map(|s| s.colour[3] * s.opacity).fold(0., f32::max),
        Paint::SwatchRef { .. } => 1.,
    };
    let stroke = if p.stroke_width > 0.0 { alpha(p.appearance().stroke()) } else { 0.0 };
    p.opacity <= 0.0 || (alpha(p.appearance().fill()) <= 0.0 && stroke <= 0.0)
}

/// The boardless page: the union of `doc.outline_bbox` (the canvas's own WORLD extent, xform-aware,
/// curves flattened — not the control-point hull) over every drawn path, padded by half its stroke; a
/// clip member counts only where it overlaps its mask's outline box. Transparent background. `None`
/// when nothing visible draws.
///
/// `reach` = how far past its outline a path counts: half its drawn stroke (Artwork bounds, whose
/// pages are pinned) or its painted extent (`varos_core::geom::painted_padding`, Export Selection…).
fn bounds_page(doc: &Document, reach: Reach) -> Option<PageSpec> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (pi, p) in doc.paint_list() {
        let Some(d) = drawable(doc, pi, p) else { continue };
        let (bx0, by0, bx1, by1) = doc.outline_bbox(pi);
        let pad = match reach {
            Reach::HalfStroke => d.pad,
            Reach::Painted => varos_core::geom::painted_padding(p).max(d.pad),
        };
        let mut b = [bx0 - pad, by0 - pad, bx1 + pad, by1 + pad];
        if let Some(c) = d.clip {
            let mut m = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for (mp, _, _) in mask_paths(doc, c) {
                let Some(mi) = doc.pidx(mp.id) else { continue };
                let (mx0, my0, mx1, my1) = doc.outline_bbox(mi);
                m = [m[0].min(mx0), m[1].min(my0), m[2].max(mx1), m[3].max(my1)];
            }
            b = [b[0].max(m[0]), b[1].max(m[1]), b[2].min(m[2]), b[3].min(m[3])];
            if b[0] > b[2] || b[1] > b[3] {
                continue;
            }
        }
        x0 = x0.min(b[0]);
        y0 = y0.min(b[1]);
        x1 = x1.max(b[2]);
        y1 = y1.max(b[3]);
    }
    for i in &doc.images {
        if let Some(b) = varos_core::images::visible_bounds(doc, i) {
            x0 = x0.min(b.0);
            y0 = y0.min(b.1);
            x1 = x1.max(b.2);
            y1 = y1.max(b.3);
        }
    }
    if x0 > x1 || y0 > y1 {
        return None;
    }
    // a degenerate (zero-width or zero-height) extent still gets a real page, at least 1 pt each way
    Some(PageSpec {
        rect: [x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)],
        background: None,
        bleed: 0.0,
        bleed_edges: [0.0; 4],
    })
}

pub(crate) fn stroke_error(e: varos_core::stroke::StrokeError) -> ExportError {
    match e {
        varos_core::stroke::StrokeError::LimitExceeded => ExportError::LimitExceeded,
        varos_core::stroke::StrokeError::Cancelled => ExportError::Cancelled,
        _ => ExportError::InvalidDocument(e.to_string()),
    }
}
