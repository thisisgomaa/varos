//! Lane G: pure next-version migration and fail-closed legacy-key gate.
use crate::{
    format::{Invalid, Limits, LoadError, PRE_TEXT_FORMAT_VERSION, TEXT_FORMAT_VERSION},
    model::Document,
};
pub fn migrate_to_text_boxes(doc: Document, _: &Limits) -> Result<Document, LoadError> {
    // Legacy readers supplied the empty default. Migration never fabricates source/fonts.
    if !doc.text_boxes.is_empty() {
        return Err(Invalid::FieldNotInFormat { field: "text_boxes", version: PRE_TEXT_FORMAT_VERSION }.into());
    }
    Ok(doc)
}
pub(crate) fn refuse_legacy_text(json: &[u8], version: u32) -> Result<(), LoadError> {
    if version >= TEXT_FORMAT_VERSION {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_slice(json).map_err(|e| LoadError::Malformed {
        line: e.line(),
        column: e.column(),
        detail: e.to_string(),
    })?;
    let doc = &value["doc"];
    if doc.get("text_boxes").is_some()
        || doc["nodes"].as_array().is_some_and(|nodes| nodes.iter().any(|n| n["kind"].get("Text").is_some()))
    {
        return Err(Invalid::FieldNotInFormat { field: "text_boxes/Text", version }.into());
    }
    Ok(())
}
