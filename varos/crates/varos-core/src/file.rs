//! `.vrs` on disk — the serde spine written as VERSIONED JSON: `{"varos": 2, "doc": {…}}`.
//! This is the 🔖 slice's format: readable and diffable. Versioning, limits, migration and validation
//! live in [`crate::format`] (ADR-0008); this module keeps the original string-error API. The FINAL
//! container decision (PDF-native with the model embedded — docs/SAVE_EXPORT_PLAN.md §4) is
//! untouched: this exact blob later rides inside that container, so nothing here is throwaway.

use crate::format::{self, Limits};
use crate::model::Document;
use std::path::Path;

/// The format number this build writes — an alias of [`format::FORMAT_VERSION`] (ADR-0008: any change
/// to what the writer can emit raises it; the migration table in `format::migrate` reads older ones).
pub const VRS_VERSION: u32 = format::FORMAT_VERSION;

/// The versioned model blob — the exact payload the PDF container embeds (and the legacy raw-JSON
/// `.vrs` body). One serializer, two homes. Runs the save-side checks (`format::encode_model`), so a
/// document this build would refuse to reopen is refused here, readably, and nothing is written.
pub fn doc_to_blob(doc: &Document) -> Result<String, String> {
    format::encode_model(doc, &Limits::DEFAULT).map_err(|e| e.to_string())
}
/// Parse a model blob back through the full load pipeline (`format::decode_model`): version gate
/// before any typed decode, strict decode, structural precheck, v1→v2 migration, validation.
pub fn doc_from_blob(body: &str) -> Result<Document, String> {
    format::decode_model(body.as_bytes(), None, &Limits::DEFAULT).map(|l| l.doc).map_err(|e| e.to_string())
}

/// Write the document to `path` atomically: serialize, write a sibling temp file, rename over the
/// target — a crash mid-save can never leave a half-written `.vrs`.
pub fn save_vrs(doc: &Document, path: &Path) -> Result<(), String> {
    let body = doc_to_blob(doc)?;
    write_atomic(path, body.as_bytes())
}

/// Atomic byte write (temp sibling + rename) — shared by the raw-JSON path and the PDF container.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("vrs.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("write failed: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename failed: {e}"))
}

/// Read a raw-JSON `.vrs` back into a Document. The read is bounded (`Limits::max_file_bytes`), and the
/// version is checked FIRST, on a header-only parse — a file written by a newer Varos (whose Document
/// shape we may not know) gets the clear "newer" message, never a confusing field-level parse error.
/// Never corrupt, never guess.
pub fn load_vrs(path: &Path) -> Result<Document, String> {
    let bytes = format::read_bounded(path, &Limits::DEFAULT).map_err(|e| e.to_string())?;
    format::decode_model(&bytes, None, &Limits::DEFAULT).map(|l| l.doc).map_err(|e| e.to_string())
}
