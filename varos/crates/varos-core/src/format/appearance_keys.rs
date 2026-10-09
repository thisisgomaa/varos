//! Lane A: refuse appearance-era fields and alpha roles in older formats, before typed decode.
use super::{Invalid, LoadError};
pub(super) fn refuse(json: &[u8], version: u32) -> Result<(), LoadError> {
    let value: serde_json::Value = serde_json::from_slice(json).map_err(|e| LoadError::malformed(&e))?;
    let paths = value["doc"]["paths"].as_array();
    let nodes = value["doc"]["nodes"].as_array();
    let field = if paths.is_some_and(|ps| ps.iter().any(|p| p.get("stack").is_some())) {
        Some("stack")
    } else if nodes.is_some_and(|ns| ns.iter().any(|n| n.get("look").is_some())) {
        Some("look")
    } else if nodes.is_some_and(|ns| ns.iter().any(|n| n["role"] == "MaskAlpha")) {
        Some("MaskAlpha")
    } else {
        None
    };
    match field {
        Some(field) => Err(Invalid::FieldNotInFormat { field, version }.into()),
        None => Ok(()),
    }
}
