//! The PURE PDF export (DFS S6): a deliverable for sharing. It carries the visible artwork and nothing
//! else — no embedded model, no /AF, no /Names, no /VAROS_* keys, no Info dictionary, no object or
//! board names, and nothing hidden. Pages are PLANNED first (`plan_pdf_export`) so the app can show
//! the page count, or a plain-English reason when a scope cannot be exported, before any bytes exist.
//!
//! Masks: the writer does not emit PDF clipping yet, so a clip group's members would come out
//! UNCLIPPED (the canvas clips them). Rather than silently misrender — and expose the art the mask
//! hides — a page that would draw any clip-group member is planned as `PlannedPage::Refused`, and
//! `export_pdf_bytes` refuses the whole export while any page is refused (never a silent subset).

use std::fmt;
use std::sync::atomic::AtomicBool;

use varos_core::model::{Anchor, Artboard, Document, Xform};
use varos_core::Rgba;

use crate::write::{drawable, drawn_on, write_pages};

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

/// A planned page: exportable, or refused with a plain-English reason (the page is still listed so
/// the page count and the refusal are honest about what the scope covers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlannedPage {
    Ready(PageSpec),
    Refused { spec: PageSpec, reason: &'static str },
}
impl PlannedPage {
    pub fn spec(&self) -> &PageSpec {
        match self {
            PlannedPage::Ready(spec) | PlannedPage::Refused { spec, .. } => spec,
        }
    }
    pub fn is_ready(&self) -> bool {
        matches!(self, PlannedPage::Ready(_))
    }
}

/// The pages a scope will export, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportPlan {
    pub scope: ExportScope,
    pub pages: Vec<PlannedPage>,
}
impl ExportPlan {
    /// Pages the scope covers (refused ones included).
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    /// Why this plan cannot be exported as a whole, if it can't (today: a page with a clipping mask).
    pub fn refusal(&self) -> Option<ExportUnavailable> {
        self.pages.iter().any(|p| !p.is_ready()).then_some(ExportUnavailable::ClipMasksNotSupported)
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
    ClipMasksNotSupported,
}
impl ExportUnavailable {
    pub fn reason(&self) -> &'static str {
        match self {
            ExportUnavailable::NoVisibleArtboards => "Every artboard is hidden. Show an artboard to export it.",
            ExportUnavailable::ActiveArtboardHidden => "The active artboard is hidden. Show it to export it.",
            ExportUnavailable::NotBoardless => "Artwork bounds is only for documents without artboards.",
            ExportUnavailable::NeedsArtboards => "This document has no artboards. Export its artwork bounds instead.",
            ExportUnavailable::NothingToExport => "There is no visible artwork to export.",
            ExportUnavailable::ClipMasksNotSupported => "Clipping masks can't be exported to PDF yet.",
        }
    }
}
impl fmt::Display for ExportUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason())
    }
}

/// Why `export_pdf_bytes` produced no bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    Cancelled,
    Unavailable(ExportUnavailable),
    Write(String),
}
impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Cancelled => f.write_str("The export was cancelled."),
            ExportError::Unavailable(u) => f.write_str(u.reason()),
            ExportError::Write(e) => f.write_str(e),
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

/// Plan the pages `scope` exports. Scope-level impossibilities are `Err`; a page that would need a
/// clipping mask is planned as `PlannedPage::Refused` (see `ExportPlan::refusal`). Never a dummy page.
pub fn plan_pdf_export(doc: &Document, scope: ExportScope) -> Result<ExportPlan, ExportUnavailable> {
    let specs: Vec<PageSpec> = match scope {
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
    Ok(ExportPlan { scope, pages: specs.into_iter().map(|s| plan_page(doc, s)).collect() })
}

/// Write the pure PDF for `plan`. Refuses (never silently drops pages) while any page needs a clipping
/// mask — re-checked here against `doc`, so a hand-built or stale plan cannot leak unclipped art.
/// `cancel` is checked before every page. Deterministic: the same document and plan give the same bytes.
pub fn export_pdf_bytes(doc: &Document, plan: &ExportPlan, cancel: &AtomicBool) -> Result<Vec<u8>, ExportError> {
    if plan.pages.is_empty() {
        return Err(ExportError::Unavailable(ExportUnavailable::NothingToExport));
    }
    let specs: Vec<PageSpec> = plan.pages.iter().map(|p| *p.spec()).collect();
    if let Some(u) = plan.refusal() {
        return Err(ExportError::Unavailable(u));
    }
    if specs.iter().any(|s| !plan_page(doc, *s).is_ready()) {
        return Err(ExportError::Unavailable(ExportUnavailable::ClipMasksNotSupported));
    }
    write_pages(doc, &specs, None, cancel)
}

/// Does this file carry an embedded Varos model (a native `.vrs` container)? Checks the catalog's
/// `/VAROS_Model` key and the `/Names` → `/EmbeddedFiles` tree for the model's file name. Anything
/// that is not a readable PDF answers `false`.
pub fn has_embedded_model(bytes: &[u8]) -> bool {
    let Ok(pdf) = lopdf::Document::load_mem(bytes) else { return false };
    let Ok(catalog) = pdf.catalog() else { return false };
    if catalog.has(b"VAROS_Model") {
        return true;
    }
    let tree = catalog
        .get_deref(b"Names", &pdf)
        .and_then(|o| o.as_dict())
        .and_then(|names| names.get_deref(b"EmbeddedFiles", &pdf))
        .and_then(|o| o.as_dict());
    match tree {
        Ok(root) => name_tree_has(&pdf, root, b"model.varos.json", 0),
        Err(_) => false,
    }
}

/// Walk a PDF name tree (leaf `/Names` pairs, inner `/Kids`) looking for `key`. Depth-capped.
fn name_tree_has(pdf: &lopdf::Document, node: &lopdf::Dictionary, key: &[u8], depth: u32) -> bool {
    if depth > 32 {
        return false;
    }
    if let Ok(arr) = node.get_deref(b"Names", pdf).and_then(|o| o.as_array()) {
        if arr.chunks(2).any(|kv| kv.first().and_then(|k| k.as_str().ok()) == Some(key)) {
            return true;
        }
    }
    if let Ok(kids) = node.get_deref(b"Kids", pdf).and_then(|o| o.as_array()) {
        for kid in kids {
            if let Ok(d) = pdf.dereference(kid).and_then(|(_, o)| o.as_dict()) {
                if name_tree_has(pdf, d, key, depth + 1) {
                    return true;
                }
            }
        }
    }
    false
}

/// A page is refused when anything the writer would draw on it is a clip-group member (a mask source
/// never paints — `paint_list` already drops it — so any clip ancestor here means "needs clipping").
fn plan_page(doc: &Document, spec: PageSpec) -> PlannedPage {
    if drawn_on(doc, &spec).any(|d| doc.clip_group_of(d.p.id).is_some()) {
        PlannedPage::Refused { spec, reason: ExportUnavailable::ClipMasksNotSupported.reason() }
    } else {
        PlannedPage::Ready(spec)
    }
}

/// The boardless page: the union of every drawn path's exact WORLD extent (curve extrema, not
/// handles), padded by half its stroke, with a transparent background. `None` when nothing visible draws.
fn artwork_bounds_page(doc: &Document) -> Option<PageSpec> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (_, p) in doc.paint_list() {
        let Some(d) = drawable(doc, p) else { continue };
        let mut own = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        ring_extent(&p.anchors, p.closed, &d.xf, &mut own);
        for hole in &p.holes {
            ring_extent(hole, true, &d.xf, &mut own);
        }
        x0 = x0.min(own[0] - d.pad);
        y0 = y0.min(own[1] - d.pad);
        x1 = x1.max(own[2] + d.pad);
        y1 = y1.max(own[3] + d.pad);
    }
    if x0 > x1 || y0 > y1 {
        return None;
    }
    // a degenerate (zero-width or zero-height) extent still gets a real page, at least 1 pt each way
    Some(PageSpec { rect: [x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)], background: None })
}

/// Grow `b` = [x0, y0, x1, y1] by the exact extent of one ring in WORLD space: every anchor plus each
/// cubic segment's interior extrema (roots of its derivative). Rotation is affine, so the control
/// points are mapped first and the extrema are taken on the world curve.
fn ring_extent(anchors: &[Anchor], closed: bool, xf: &Xform, b: &mut [f32; 4]) {
    let mut grow = |q: [f32; 2]| {
        b[0] = b[0].min(q[0]);
        b[1] = b[1].min(q[1]);
        b[2] = b[2].max(q[0]);
        b[3] = b[3].max(q[1]);
    };
    let n = anchors.len();
    if n == 0 {
        return;
    }
    grow(xf.apply(anchors[0].p));
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (a, c) = (&anchors[i], &anchors[(i + 1) % n]);
        let pts = [a.p, a.hout.unwrap_or(a.p), c.hin.unwrap_or(c.p), c.p].map(|q| xf.apply(q));
        grow(pts[3]);
        for t in cubic_extrema_t(&pts) {
            let mt = 1.0 - t;
            let w = [mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t];
            grow([0, 1].map(|k| w.iter().zip(&pts).map(|(w, p)| w * p[k]).sum()));
        }
    }
}

/// Parameters in (0, 1) where either coordinate of the cubic has a zero derivative.
fn cubic_extrema_t(p: &[[f32; 2]; 4]) -> Vec<f32> {
    let mut out = Vec::new();
    for axis in [0, 1] {
        // B'(t) / 3 = a·t² + b·t + c
        let [p0, p1, p2, p3] = p.map(|q| q[axis]);
        let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
        let b = 2.0 * (p0 - 2.0 * p1 + p2);
        let c = p1 - p0;
        if a.abs() < 1e-9 {
            if b.abs() > 1e-9 {
                out.push(-c / b);
            }
        } else {
            let disc = b * b - 4.0 * a * c;
            if disc >= 0.0 {
                let r = disc.sqrt();
                out.push((-b + r) / (2.0 * a));
                out.push((-b - r) / (2.0 * a));
            }
        }
    }
    out.retain(|t| *t > 0.0 && *t < 1.0);
    out
}
