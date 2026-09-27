//! `.vrs` = a VALID PDF container (one page per artboard, artwork rendered by any viewer) with the
//! editable varos-core model embedded as an associated file — the `.ai` pattern (SAVE_EXPORT_PLAN §1).
//! Write = `pdf-writer`, read-back = `lopdf`. PDF semantics live ONLY in this crate (§0 hedge: the
//! container can be swapped with zero model rewrite).
//!
//! Mapping decisions (design-reviewed 2026-07-02):
//! - World→page flips CPU-side (X = wx−ab.x, Y = ab.y+ab.h−wy); identity CTM, MediaBox [0 0 w h].
//!   Cubics are affine-invariant → control points use the same map.
//! - Closed paths emit the WRAP-AROUND cubic explicitly before `h` (h alone would straighten it).
//! - Fills are `f*` (even-odd) with holes as subpaths of the SAME path; plain fill+stroke is one `B*`.
//! - Alphas: /ca = fill.a·opacity, /CA = stroke.a·opacity via pooled ExtGStates.
//! - Varos knockout semantics (translucent stroke must not blend over its own fill) map to a Form
//!   XObject transparency group /I true /K true — emitted ONLY when fill+stroke exist and the
//!   effective stroke alpha < 1. Known gap: pdf.js (Firefox) ignores /K → slightly darker band there.
//! - The model blob ({"varos":1,…}) embeds via EmbeddedFiles name tree + /AF (AFRelationship=Source)
//!   + catalog /VAROS_Model + /VAROS_SchemaVersion; read prefers /VAROS_Model, falls back to the tree.

use std::path::Path as FsPath;

use varos_core::file::write_atomic;
use varos_core::format::Limits;
use varos_core::model::Document;

// The write side lives in `write.rs` (the shared page loop + the native container) and `export.rs`
// (the pure PDF export: planning + a model-free writer). The bounded read side lives in read.rs;
// this file keeps the compatible public entry points.
mod export;
mod read;
pub use read::{load_vrs_bytes, load_vrs_checked};
mod write;
pub use export::{
    default_scope, export_pdf_bytes, has_embedded_model, plan_pdf_export, ExportError, ExportPlan, ExportScope,
    ExportUnavailable, PageSpec, HAS_MODEL_SCAN_CAP,
};
pub use write::write_pdf;

// ───────────────────────────── public API (drop-in for varos_core::file) ─────────────────────────────

/// Save the document as a `.vrs` PDF container, atomically.
pub fn save_vrs(doc: &Document, path: &FsPath) -> Result<(), String> {
    write_atomic(path, &write_pdf(doc)?)
}

/// Load a `.vrs`: a PDF container (the model blob is recovered from inside), or a legacy raw-JSON
/// `.vrs` from the first slice (sniffed by the missing `%PDF-` header).
pub fn load_vrs(path: &FsPath) -> Result<Document, String> {
    load_vrs_with_notice(path).map(|(doc, _)| doc)
}

/// Load with the migration notice retained for the application's open flow.
pub fn load_vrs_with_notice(path: &FsPath) -> Result<(Document, Option<&'static str>), String> {
    let loaded = load_vrs_checked(path, &Limits::DEFAULT).map_err(|e| e.to_string())?;
    let notice = loaded.notice();
    Ok((loaded.doc, notice))
}
