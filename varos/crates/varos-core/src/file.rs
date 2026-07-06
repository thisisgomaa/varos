//! `.vrs` on disk — the serde spine written as VERSIONED JSON: `{"varos": 1, "doc": {…}}`.
//! This is the 🔖 slice's format: readable, diffable, and additive-tolerant (the model's
//! `#[serde(default)]` hygiene means older files keep loading as fields are added). The FINAL
//! container decision (PDF-native with the model embedded — docs/SAVE_EXPORT_PLAN.md §4) is
//! untouched: this exact blob later rides inside that container, so nothing here is throwaway.

use crate::model::Document;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Bump when the wrapper itself changes shape (model evolution is handled by serde defaults).
pub const VRS_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct VrsFile {
    varos: u32,
    doc: Document,
}

/// The versioned model blob — the exact payload the PDF container embeds (and the legacy raw-JSON
/// `.vrs` body). One serializer, two homes.
pub fn doc_to_blob(doc: &Document) -> Result<String, String> {
    serde_json::to_string(&VrsFile { varos: VRS_VERSION, doc: doc.clone() })
        .map_err(|e| format!("serialize failed: {e}"))
}
/// Parse a model blob back (version-checked header-first — see `load_vrs`).
pub fn doc_from_blob(body: &str) -> Result<Document, String> {
    #[derive(Deserialize)]
    struct VrsHead {
        varos: u32,
    } // serde skips unknown fields, so this reads ANY .vrs generation
    let head: VrsHead = serde_json::from_str(body).map_err(|e| format!("not a valid .vrs model: {e}"))?;
    if head.varos > VRS_VERSION {
        return Err(format!("this file was saved by a newer Varos (v{}) — please update", head.varos));
    }
    let f: VrsFile = serde_json::from_str(body).map_err(|e| format!("not a valid .vrs model: {e}"))?;
    let mut doc = f.doc;
    doc.sync_tree(); // legacy files: registry → tree + wrap tree-less paths into Layer 1 (z preserved)
    Ok(doc)
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

/// Read a raw-JSON `.vrs` back into a Document. The version is checked FIRST, on a header-only parse —
/// a file written by a newer Varos (whose Document shape we may not know) gets the clear "newer"
/// message, never a confusing field-level parse error. Never corrupt, never guess.
pub fn load_vrs(path: &Path) -> Result<Document, String> {
    let body = std::fs::read_to_string(path).map_err(|e| format!("read failed: {e}"))?;
    doc_from_blob(&body)
}
