//! Lane H: format 14 pure migration and fail-closed key gate.
use crate::{
    format::{Invalid, Limits, LoadError},
    model::Document,
};
pub const VERSION: u32 = 14;
/// Previous era's optional wave-3 keys are preserved; this step invents no text or font data.
pub fn migrate_v13_to_v14(doc: Document, _: &Limits) -> Result<Document, LoadError> {
    if !doc.typography.is_empty() {
        return Err(Invalid::FieldNotInFormat { field: "typography", version: 13 }.into());
    }
    Ok(doc)
}
pub(crate) fn refuse(json: &[u8], version: u32) -> Result<(), LoadError> {
    if version >= VERSION {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_slice(json).map_err(|e| LoadError::Malformed {
        line: e.line(),
        column: e.column(),
        detail: e.to_string(),
    })?;
    if value["doc"].get("typography").is_some() {
        return Err(Invalid::FieldNotInFormat { field: "typography", version }.into());
    }
    Ok(())
}
