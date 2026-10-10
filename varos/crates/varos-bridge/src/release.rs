//! Lane G: visible updates. Pure signed-manifest and sheet contracts; no install operation.
use crate::{Error, Host, Reply};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub const MAX_MANIFEST: usize = 262_144;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api: String,
    pub action: Action,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Check {},
    Download {},
    CrashLogs {},
    CrashReport {},
    Close {},
    VerifyManifest { envelope: String, public_key: String, current: String },
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    pub notes: String,
    pub download_url: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: String,
    signature: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Sheet {
    Checking,
    Current(Manifest),
    Available(Manifest),
    Failed(String),
    Crash(String),
}
/// HTTPS destinations only, no credentials or whitespace/control characters. curl receives a separate arg.
pub fn https(url: &str) -> Result<(), String> {
    let tail = url.strip_prefix("https://").ok_or("Use an HTTPS URL")?;
    let authority = tail.split('/').next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains(['@', '?', '#', '\\'])
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("Invalid HTTPS destination".into());
    }
    Ok(())
}
pub fn verify(bytes: &[u8], key: &str, current: &str) -> Result<Sheet, String> {
    if bytes.len() > MAX_MANIFEST {
        return Err("Manifest exceeds size limit".into());
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| "Invalid signed manifest envelope")?;
    let key: [u8; 32] =
        STANDARD.decode(key).map_err(|_| "Invalid public key")?.try_into().map_err(|_| "Invalid public key length")?;
    let signature =
        Signature::from_slice(&STANDARD.decode(&envelope.signature).map_err(|_| "Invalid signature encoding")?)
            .map_err(|_| "Invalid signature length")?;
    VerifyingKey::from_bytes(&key)
        .map_err(|_| "Invalid public key")?
        .verify_strict(envelope.payload.as_bytes(), &signature)
        .map_err(|_| "Manifest signature rejected")?;
    let manifest: Manifest = serde_json::from_str(&envelope.payload).map_err(|_| "Invalid manifest payload")?;
    https(&manifest.download_url)?;
    if manifest.notes.len() > 65_536 {
        return Err("Release notes exceed size limit".into());
    }
    if newer(&manifest.version, current)? {
        Ok(Sheet::Available(manifest))
    } else {
        Ok(Sheet::Current(manifest))
    }
}
pub fn newer(candidate: &str, current: &str) -> Result<bool, String> {
    let parse = |s: &str| semver::Version::parse(s).map_err(|_| "Invalid semantic version".to_string());
    Ok(parse(candidate)?.cmp_precedence(&parse(current)?).is_gt())
}
pub fn dispatch(host: &mut dyn Host, request: &Request) -> Result<Reply, Error> {
    if request.api != "1.2" {
        return Err(Error::new("unsupported", "release requires API 1.2"));
    }
    if let Action::VerifyManifest { envelope, public_key, current } = &request.action {
        let sheet = verify(envelope.as_bytes(), public_key, current).map_err(|e| Error::new("invalid_argument", e))?;
        return Ok(Reply::success(json!(sheet)));
    }
    host.release(request)
}
pub fn schema(verb: Option<&str>) -> Result<Value, Error> {
    if verb.is_some() {
        return Err(Error::new("invalid_argument", "release uses action.kind; request its tool schema"));
    }
    Ok(
        json!({"type":"object","additionalProperties":false,"required":["api","action"],"properties":{"api":{"const":"1.2"},"action":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"enum":["check","download","crash_logs","crash_report","close"]}}},{"type":"object","additionalProperties":false,"required":["kind","envelope","public_key","current"],"properties":{"kind":{"const":"verify_manifest"},"envelope":{"type":"string","maxLength":MAX_MANIFEST},"public_key":{"type":"string"},"current":{"type":"string"}}}]}}}),
    )
}
/// Detailed discovery lives outside the capped inline tools/list projection.
pub fn discovery() -> Value {
    json!({"name":"release","id":"app.release","description":"Signed updates/crash reports.","actions":["check","download","crash_logs","crash_report","close","verify_manifest"],"enabled":false,"disabled_reason":"needs_arguments"})
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn signed() -> (Vec<u8>, String) {
        let key = SigningKey::from_bytes(&[42; 32]);
        let payload = json!({"version":"1.2.0","notes":"إصلاحات الوصول","download_url":"https://example.org/download"})
            .to_string();
        let sig = key.sign(payload.as_bytes());
        (
            serde_json::to_vec(&json!({"payload":payload,"signature":STANDARD.encode(sig.to_bytes())})).unwrap(),
            STANDARD.encode(key.verifying_key().as_bytes()),
        )
    }
    #[test]
    fn signatures_and_sheet_states() {
        let (bytes, key) = signed();
        assert!(matches!(verify(&bytes, &key, "1.0.0"), Ok(Sheet::Available(_))));
        assert!(matches!(verify(&bytes, &key, "1.2.0"), Ok(Sheet::Current(_))));
        let mut tampered: Value = serde_json::from_slice(&bytes).unwrap();
        tampered["payload"] = json!("{}");
        assert!(verify(&serde_json::to_vec(&tampered).unwrap(), &key, "1.0.0").is_err());
        assert!(verify(&bytes, &STANDARD.encode([7; 32]), "1.0.0").is_err());
        assert!(verify(&vec![0; MAX_MANIFEST + 1], &key, "1.0.0").is_err());
        assert!(verify(b"{}", &key, "1.0.0").is_err());
    }
    #[test]
    fn version_precedence() {
        assert!(newer("1.10.0", "1.9.0").unwrap());
        assert!(newer("1.0.0", "1.0.0-beta.2").unwrap());
        assert!(!newer("1.0.0+build2", "1.0.0+build1").unwrap());
        assert!(!newer("1.0.0-beta", "1.0.0").unwrap());
        assert!(newer("bad", "1.0.0").is_err());
    }
    #[test]
    fn discovery_and_decoder() {
        assert!(crate::mcp::schema("release", None).is_ok());
        assert!(crate::mcp::list_verbs().to_string().contains("release"));
        assert!(!crate::mcp::tools_for("1.2")["tools"].as_array().unwrap().iter().any(|v| v["name"] == "release"));
        assert!(serde_json::from_value::<Request>(json!({"api":"1.2","action":{"kind":"install"}})).is_err());
        for kind in ["check", "download", "crash_logs", "crash_report", "close"] {
            assert!(serde_json::from_value::<Request>(json!({"api":"1.2","action":{"kind":kind}})).is_ok());
        }
    }
    #[test]
    fn destinations() {
        for bad in ["http://a", "https://", "https://a@b/x", "https://a/\n", "file:///tmp/a"] {
            assert!(https(bad).is_err());
        }
    }
}
