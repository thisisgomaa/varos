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
        "{created?: Unix seconds|null (default now, recorded in the journal)} create a free canvas with zero artboards; add artboards later with the Artboard tool",
        always,
        new_board
    )]
}

fn new_board(s: &mut Session, p: &Value) -> Result<Value> {
    if !p.is_null() && p.as_object().is_none_or(|o| o.keys().any(|k| k != "created")) {
        return Err(bad("file.newBoard", "expected {created?: Unix seconds|null}"));
    }
    let mut doc = s.default_settings().document();
    doc.artboards.clear();
    doc.title = s.next_untitled();
    doc.grid = s.prefs.grid();
    doc.metadata.created = clock_date(s, "created", date_param(p, "created", "file.newBoard")?);
    let index = s.add_document(doc, None);
    Ok(json!({"index": index, "artboards": 0}))
}

/// Default export bounds on a free canvas; an explicitly selected artboard still validates.
pub(crate) fn export_art(doc: &Document, p: &Value) -> bool {
    doc.artboards.is_empty() && !["artboard", "artboards", "range"].iter().any(|k| p.get(k).is_some_and(|v| !v.is_null()))
}

/// A PDF needs a finite page. Adapt only the output copy, leaving the editable native
/// document (including its empty artboard list) in the PDF's editing attachment.
pub(crate) fn pdf_document(cmd: &str, doc: &Document, p: &Value) -> Result<Option<Document>> {
    if !export_art(doc, p)
        && !(doc.artboards.is_empty()
            && p.get("range").and_then(Value::as_str) == Some("all")
            && ["artboard", "artboards"].iter().all(|k| p.get(k).is_none_or(Value::is_null)))
    {
        return Ok(None);
    }
    let rect = vectorcraft_render::encode::art_bounds(doc).ok_or_else(|| bad(cmd, "nothing to export: the document has no visible art"))?;
    let mut output = doc.clone();
    output.artboards.push(vectorcraft_doc::Artboard { id: 1, name: String::new(), rect, show_center_mark: false, show_cross_hairs: false });
    Ok(Some(output))
}

#[cfg(test)]
mod tests;
