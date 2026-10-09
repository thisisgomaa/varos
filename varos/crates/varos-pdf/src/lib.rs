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
mod clipboard;
mod export;
mod image_write;
pub mod images;
pub use clipboard::{clipboard_vectors, ClipboardVectors};
mod options;
pub use options::{export_pdf_with_options, PdfBoxes, PdfMarks, PdfOptions, PdfPreset};
mod read;
pub use read::{load_vrs_bytes, load_vrs_checked};
mod write;
pub use export::{
    default_scope, export_pdf_bytes, export_pdf_bytes_with_report, has_embedded_model, plan_pdf_export,
    plan_selection_export, ExportError, ExportPlan, ExportScope, ExportUnavailable, PageSpec, HAS_MODEL_SCAN_CAP,
};
pub use write::write_pdf;

// ───────────────────────────── public API (drop-in for varos_core::file) ─────────────────────────────

/// Save the document as a `.vrs` PDF container, atomically.
pub fn save_vrs(doc: &Document, path: &FsPath) -> Result<(), String> {
    write_atomic(path, &write_pdf_checked(doc, &Limits::DEFAULT)?)
}

/// The save-side gate (A1): a file Varos writes can always be reopened under the SAME `limits` the
/// reader enforces (one source — `Limits::DEFAULT` in the app), and it is decided before anything
/// reaches the disk. A refusal is a plain sentence.
pub fn write_pdf_checked(doc: &Document, limits: &Limits) -> Result<Vec<u8>, String> {
    write_pdf_checked_report(doc, limits).map(|(bytes, _)| bytes).map_err(|e| e.reason)
}

/// What the save gate measured. `decoded` = the full reopen (`load_vrs_bytes`) ran because a
/// count came within [`SAVE_CHECK_MARGIN`] of its limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveCheck {
    pub objects: usize,
    pub tokens: usize,
    /// Length of the embedded model stream (unfiltered: also its decoded length).
    pub model_bytes: usize,
    pub decoded: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveCheckError {
    pub reason: String,
    pub decoded: bool,
}

/// A count at or above this fraction of its limit triggers the full reopen check (10 % headroom).
pub const SAVE_CHECK_MARGIN: f64 = 0.9;

/// [`write_pdf_checked`] with its measurements, for tests and diagnostics.
///
/// Cheap by design: the model is encoded under `limits` (model-level limits refuse here); the
/// writer counts the indirect objects it emits; the direct-object tokens are counted by the reader's
/// own preflight lexer on the emitted dictionaries (no lopdf parse, no model decode). Anything over
/// a limit is refused from those counts. Only when a count is within 10 % of its limit does the full
/// `load_vrs_bytes` decode run as the final word — rare, and never on an ordinary save.
///
/// Every refusal the reader (`read.rs` + `varos_core::format::decode_model`) can raise, and where the
/// writer mirrors it:
///
/// | Reader refusal | Writer-side check |
/// |---|---|
/// | `FileBytes` (`max_file_bytes`) | `bytes.len()` here; ≥ 90 % → full decode |
/// | `PdfObjects`: xref entries (preflight), lopdf object count | writer's own object count + 1 (free entry 0) here; ≥ 90 % → full decode |
/// | direct object complexity (`max_pdf_tokens`) | reader's preflight lexer (`read::pdf_tokens`) here; ≥ 90 % → full decode |
/// | `PdfDepth`, footer/xref/trailer shape, `/Prev` `/XRefStm` `/Encrypt`, indirect or wrong stream `/Length`, oversized trailer/xref lines | the same preflight (`read::pdf_tokens` runs it with only the object/token budgets lifted) |
/// | `ModelBytes` (`max_model_bytes`) | `encode_model(doc, limits)` (exact blob length); ≥ 90 % → full decode |
/// | `DecodedStreams` (`max_decoded_stream_bytes`, the model stream) | model length here; ≥ 90 % → full decode |
/// | model `/Filter` ("model encoding") | never emitted: the writer stores the model unfiltered |
/// | missing catalog / `VAROS_Model` / `NoEmbeddedModel`, embedded-files tree budget | never emitted: the writer always writes the catalog with `/VAROS_Model` (the name-tree walk is a fallback the reader never reaches for our files) |
/// | `InvalidVersion`, `VersionMismatch`, `NewerVersion` | writer emits `VRS_VERSION` in both catalog and model |
/// | `Nodes`, `Paths`, `Anchors`, `Artboards`, `TreeDepth`, `Invalid(..)`, malformed JSON | `encode_model(doc, limits)`: the same structure check + validation on the normalized model, plus a serde read-back |
/// | lopdf strict parse failure (malformed structure) | not a limit: `pdf-writer` output is well-formed; any such bug is caught by the near-limit decode and the round-trip tests |
pub fn write_pdf_checked_report(doc: &Document, limits: &Limits) -> Result<(Vec<u8>, SaveCheck), SaveCheckError> {
    use varos_core::format::{LimitKind, LoadError, SaveRefused};
    let refuse = |e: LoadError, decoded| SaveCheckError { reason: SaveRefused(e).to_string(), decoded };
    let too_large =
        |limit, found: usize, max: usize| LoadError::TooLarge { limit, found: found as u64, max: max as u64 };
    let (bytes, objects, model_bytes) =
        write::write_native_counted(doc, limits).map_err(|reason| SaveCheckError { reason, decoded: false })?;
    // The reader counts the xref table, whose entry 0 is the free head: objects + 1.
    let entries = objects + 1;
    if entries > limits.max_pdf_objects {
        return Err(refuse(too_large(LimitKind::PdfObjects, entries, limits.max_pdf_objects), false));
    }
    if bytes.len() as u64 > limits.max_file_bytes {
        let e =
            LoadError::TooLarge { limit: LimitKind::FileBytes, found: bytes.len() as u64, max: limits.max_file_bytes };
        return Err(refuse(e, false));
    }
    // The reader bounds the model stream by both the model cap (enforced by `encode_model`) and the
    // decoded-stream budget.
    if model_bytes > limits.max_decoded_stream_bytes {
        let e = too_large(LimitKind::DecodedStreams, model_bytes, limits.max_decoded_stream_bytes);
        return Err(refuse(e, false));
    }
    let tokens = read::pdf_tokens(&bytes, limits).map_err(|e| refuse(e, false))?;
    if tokens > limits.max_pdf_tokens() {
        let e = LoadError::UnsupportedPdf("direct object complexity limit".into());
        return Err(refuse(e, false));
    }
    let near = |found: f64, max: f64| found >= max * SAVE_CHECK_MARGIN;
    let decoded = near(entries as f64, limits.max_pdf_objects as f64)
        || near(tokens as f64, limits.max_pdf_tokens() as f64)
        || near(bytes.len() as f64, limits.max_file_bytes as f64)
        || near(model_bytes as f64, limits.max_model_bytes as f64)
        || near(model_bytes as f64, limits.max_decoded_stream_bytes as f64);
    if decoded {
        load_vrs_bytes(&bytes, limits).map_err(|e| refuse(e, true))?;
    }
    Ok((bytes, SaveCheck { objects, tokens, model_bytes, decoded }))
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

mod gradient;
pub mod package;
