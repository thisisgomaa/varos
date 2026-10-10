//! Lane G: offline signed-manifest inspection, without a desktop/attached instance.
use std::io::Read;
pub fn verify(args: Vec<String>) -> Result<serde_json::Value, String> {
    let [path, key, current] = args.as_slice() else {
        return Err("verify-update <manifest.json> <base64-trusted-public-key> <current-semver>".into());
    };
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(varos_bridge::release::MAX_MANIFEST as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let sheet = varos_bridge::release::verify(&bytes, key, current)?;
    serde_json::to_value(sheet).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    #[test]
    fn usage_requires_explicit_key_and_version() {
        assert!(super::verify(vec![]).is_err());
    }
}
