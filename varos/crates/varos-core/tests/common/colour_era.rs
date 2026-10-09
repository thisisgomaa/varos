//! Historical future fixtures retain their bytes; advance only the test's in-memory header.
pub fn future(bytes: &[u8]) -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    v["varos"] = serde_json::json!(varos_core::format::FORMAT_VERSION + 1);
    serde_json::to_vec(&v).unwrap()
}
