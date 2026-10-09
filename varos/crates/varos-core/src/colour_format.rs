//! Lane C: pre-v12 bodies cannot carry new colour keys, even default-looking values.
use crate::format::{Invalid, LoadError, COLOUR_VERSION};
pub fn refuse(bytes: &[u8], version: u32) -> Result<(), LoadError> {
    if version >= COLOUR_VERSION {
        return Ok(());
    }
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| LoadError::malformed(&e))?;
    let doc = &v["doc"];
    fn managed(p: &serde_json::Value) -> bool {
        match p {
            serde_json::Value::Object(m) => m.get("type").is_some_and(|t| t == "managed") || m.values().any(managed),
            serde_json::Value::Array(a) => a.iter().any(managed),
            _ => false,
        }
    }
    if doc.get("colour_mode").is_some() || doc.get("output_profile").is_some() {
        return Err(Invalid::FieldNotInFormat { field: "colour_mode/output_profile", version }.into());
    }
    if managed(doc) {
        return Err(Invalid::FieldNotInFormat { field: "managed colour", version }.into());
    }
    Ok(())
}
