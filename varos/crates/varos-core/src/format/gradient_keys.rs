//! Next-format refusal runs before typed model decoding, including malformed tagged values.
use super::{Invalid, LoadError};
pub(super) fn refuse(bytes: &[u8], version: u32) -> Result<(), LoadError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| LoadError::malformed(&e))?;
    let doc = &value["doc"];
    if doc.get("swatches").is_some()
        || doc["paths"]
            .as_array()
            .is_some_and(|paths| paths.iter().any(|p| ["fill", "stroke"].iter().any(|slot| p[slot].is_object())))
    {
        Err(Invalid::FieldNotInFormat { field: "gradient paints / swatches", version }.into())
    } else {
        Ok(())
    }
}
