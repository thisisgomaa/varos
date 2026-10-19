//! Varos free canvases use the native document's existing empty artboard list.
use super::*;
use serde_json::json;
use vectorcraft_doc::Document;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "file.newBoard",
        "New board",
        [],
        None,
        "{} create a free canvas with zero artboards; add artboards later with the Artboard tool",
        always,
        new_board
    )]
}

fn new_board(s: &mut Session, p: &Value) -> Result<Value> {
    if !p.is_null() && p.as_object().is_none_or(|o| !o.is_empty()) {
        return Err(bad("file.newBoard", "expected an empty object"));
    }
    let mut doc = Document::new(612.0, 792.0);
    doc.artboards.clear();
    doc.title = s.next_untitled();
    doc.grid = s.prefs.grid();
    doc.units = s.default_units();
    doc.metadata.created = vectorcraft_doc::metadata::now_unix();
    let index = s.add_document(doc, None);
    Ok(json!({"index": index, "artboards": 0}))
}

/// Default export bounds on a free canvas; an explicitly selected artboard still validates.
pub(crate) fn export_art(doc: &Document, p: &Value) -> bool {
    doc.artboards.is_empty() && !["artboard", "artboards", "range"].iter().any(|k| p.get(k).is_some_and(|v| !v.is_null()))
}

/// A PDF needs a finite page. Adapt only the output copy, leaving the editable native
/// document (including its empty artboard list) in the PDF's editing attachment.
pub(crate) fn pdf_document(doc: &Document, p: &Value) -> Result<Option<Document>> {
    if !export_art(doc, p) {
        return Ok(None);
    }
    let rect =
        vectorcraft_render::encode::art_bounds(doc).ok_or_else(|| bad("document.exportPdf", "nothing to export: the document has no visible art"))?;
    let mut output = doc.clone();
    output.artboards.push(vectorcraft_doc::Artboard { id: 1, name: String::new(), rect, show_center_mark: false, show_cross_hairs: false });
    Ok(Some(output))
}

#[cfg(test)]
mod tests;
