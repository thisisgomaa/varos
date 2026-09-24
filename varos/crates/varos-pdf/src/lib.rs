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

use varos_core::file::{doc_from_blob, write_atomic};
use varos_core::model::Document;

// The write side lives in `write.rs` (the shared page loop + the native container) and `export.rs`
// (the pure PDF export: planning + a model-free writer). This file keeps the public entry points and
// the read side.
mod export;
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
    let bytes = std::fs::read(path).map_err(|e| format!("read failed: {e}"))?;
    if bytes.starts_with(b"%PDF-") {
        doc_from_blob(&extract_model(&bytes)?)
    } else {
        let s = String::from_utf8(bytes).map_err(|_| "not a valid .vrs".to_string())?;
        doc_from_blob(&s)
    }
}

// ───────────────────────────── read: PDF bytes → model blob ─────────────────────────────

fn extract_model(bytes: &[u8]) -> Result<String, String> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| format!("not a readable PDF: {e}"))?;
    let catalog = doc.catalog().map_err(|e| format!("no PDF catalog: {e}"))?;

    let stream_bytes = |obj: &lopdf::Object| -> Result<Vec<u8>, String> {
        let (_, o) = doc.dereference(obj).map_err(|e| e.to_string())?;
        let s = o.as_stream().map_err(|e| e.to_string())?;
        Ok(s.decompressed_content().unwrap_or_else(|_| s.content.clone()))
    };

    // fast path: the private catalog key Varos writes
    if let Ok(obj) = catalog.get(b"VAROS_Model") {
        let data = stream_bytes(obj)?;
        return String::from_utf8(data).map_err(|_| "embedded model is not UTF-8".into());
    }
    // fallback: /Names → /EmbeddedFiles name tree → FileSpec /EF /F (survives third-party re-saves better)
    fn collect(doc: &lopdf::Document, node: &lopdf::Dictionary, out: &mut Vec<lopdf::Object>) {
        if let Ok(pairs) = node.get(b"Names") {
            if let Ok((_, o)) = doc.dereference(pairs) {
                if let Ok(arr) = o.as_array() {
                    for kv in arr.chunks(2) {
                        if let [_k, v] = kv {
                            out.push(v.clone());
                        }
                    }
                }
            }
        } else if let Ok(kids) = node.get(b"Kids") {
            if let Ok((_, o)) = doc.dereference(kids) {
                if let Ok(arr) = o.as_array() {
                    for kid in arr {
                        if let Ok((_, kd)) = doc.dereference(kid) {
                            if let Ok(d) = kd.as_dict() {
                                collect(doc, d, out);
                            }
                        }
                    }
                }
            }
        }
    }
    let names = catalog
        .get_deref(b"Names", &doc)
        .and_then(|o| o.as_dict())
        .map_err(|_| "no embedded Varos model in this PDF".to_string())?;
    let root = names
        .get_deref(b"EmbeddedFiles", &doc)
        .and_then(|o| o.as_dict())
        .map_err(|_| "no embedded Varos model in this PDF".to_string())?;
    let mut specs = Vec::new();
    collect(&doc, root, &mut specs);
    for spec in specs {
        let Ok((_, so)) = doc.dereference(&spec) else { continue };
        let Ok(sd) = so.as_dict() else { continue };
        let Ok(ef) = sd.get_deref(b"EF", &doc).and_then(|o| o.as_dict()) else { continue };
        if let Ok(f) = ef.get(b"F").or_else(|_| ef.get(b"UF")) {
            if let Ok(data) = stream_bytes(f) {
                if let Ok(s) = String::from_utf8(data) {
                    return Ok(s);
                }
            }
        }
    }
    Err("no embedded Varos model in this PDF".into())
}
