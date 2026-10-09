//! Pure v13 migration and older-era key refusal before typed decoding.
use crate::{
    format::{Invalid, Limits, LoadError},
    model::Document,
};
pub fn migrate_v12_to_v13(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    Ok(doc)
}
/// Lane-local placeholders. Integrator replaces these rows with sibling era migrations.
pub fn migrate_reserved_era(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    Ok(doc)
}
pub fn refuse_older_keys(bytes: &[u8], version: u32) -> Result<(), LoadError> {
    if version >= crate::format::LIVE_VERSION {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| LoadError::Malformed {
        line: e.line(),
        column: e.column(),
        detail: e.to_string(),
    })?;
    if value["doc"]["nodes"].as_array().is_some_and(|nodes| nodes.iter().any(|n| n["kind"].get("Live").is_some())) {
        return Err(Invalid::FieldNotInFormat { field: "nodes[].kind.Live", version }.into());
    }
    Ok(())
}
