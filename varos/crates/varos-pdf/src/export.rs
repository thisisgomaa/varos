//! The PURE PDF export (DFS S6): a deliverable for sharing. It carries the visible artwork and nothing
//! else — no embedded model, no /AF, no /Names, no /VAROS_* keys, no Info dictionary, no object or
//! board names, and nothing hidden. Pages are PLANNED first (`plan_pdf_export`) so the app can show
//! the page count, or a plain-English reason when a scope cannot be exported, before any bytes exist.
//!
//! Masks: clip-group members are written inside a PDF clip (`q … W* n … Q`, MASKS_PLAN Stage 5), in
//! the shared page loop (`crate::write`), so the export and the native `.vrs` pages both match the canvas.

use std::fmt;
use std::sync::atomic::AtomicBool;

use varos_core::model::{Artboard, Document};
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
}

/// One output page: a world rect `[x, y, w, h]` (points, Y down) and its background (`None` = transparent).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSpec {
    pub rect: [f32; 4],
    pub background: Option<Rgba>,
}
impl PageSpec {
    /// The page an artboard prints as.
    pub fn of_board(ab: &Artboard) -> PageSpec {
        PageSpec { rect: [ab.x, ab.y, ab.w, ab.h], background: ab.page_color }
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
}
impl ExportUnavailable {
    pub fn reason(&self) -> &'static str {
        match self {
            ExportUnavailable::NoVisibleArtboards => "Every artboard is hidden. Show an artboard to export it.",
            ExportUnavailable::ActiveArtboardHidden => "The active artboard is hidden. Show it to export it.",
            ExportUnavailable::NotBoardless => "Artwork bounds is only for documents without artboards.",
            ExportUnavailable::NeedsArtboards => "This document has no artboards. Export its artwork bounds instead.",
            ExportUnavailable::NothingToExport => "There is no visible artwork to export.",
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
    Unavailable(ExportUnavailable),
}
impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            vec![artwork_bounds_page(doc).ok_or(ExportUnavailable::NothingToExport)?]
        }
    };
    Ok(ExportPlan { scope, pages })
}

/// Write the pure PDF for `plan`: its pages and a bare catalog, nothing else. `cancel` is checked
/// before every page. Deterministic: the same document and plan give the same bytes.
pub fn export_pdf_bytes(doc: &Document, plan: &ExportPlan, cancel: &AtomicBool) -> Result<Vec<u8>, ExportError> {
    if plan.pages.is_empty() {
        return Err(ExportError::Unavailable(ExportUnavailable::NothingToExport));
    }
    write_pages(doc, &plan.pages, None, cancel)
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

/// The boardless page: the union of `doc.outline_bbox` (the canvas's own WORLD extent, xform-aware,
/// curves flattened — not the control-point hull) over every drawn path, padded by half its stroke; a
/// clip member counts only where it overlaps its mask's outline box. Transparent background. `None`
/// when nothing visible draws.
fn artwork_bounds_page(doc: &Document) -> Option<PageSpec> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (pi, p) in doc.paint_list() {
        let Some(d) = drawable(doc, pi, p) else { continue };
        let (bx0, by0, bx1, by1) = doc.outline_bbox(pi);
        let mut b = [bx0 - d.pad, by0 - d.pad, bx1 + d.pad, by1 + d.pad];
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
    if x0 > x1 || y0 > y1 {
        return None;
    }
    // a degenerate (zero-width or zero-height) extent still gets a real page, at least 1 pt each way
    Some(PageSpec { rect: [x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)], background: None })
}
