//! ADR-0011 C1 — connection and trust: discovery registry, file-backed identity,
//! signed handshake, open local trust, capability reporting, audit and auto-attach.
//!
//! Everything here is host/adapter code. `varos-core` stays pure; no editing happens here.
//! Layout (macOS):
//! - runtime (per-user OS temp dir from `confstr(_CS_DARWIN_USER_TEMP_DIR)`, never `$TMPDIR`):
//!   `<temp>/varos-bridge/{hosts/<instance>/endpoint.json + b.sock}`
//! - state (persistent): `~/Library/Application Support/Varos/bridge/{profiles.json, audit/audit.log}`
//! - secrets: `<state>/keys/{host.key,agent-<profile-id>.key}` (raw seeds, Unix 0600/0700).
pub mod attach;
pub mod audit;
pub mod credentials;
pub mod fsutil;
pub mod handshake;
pub mod manage;
pub mod register;
pub mod registry;
pub mod trust;

use crate::Error;
use std::path::PathBuf;

/// Connection protocol, independent of Bridge `API`, MCP revision and `.vrs` (ADR-0011 §5).
pub const CONNECTION: &str = "1.0";
/// Registry record schema version (ADR-0011 §2).
pub const DISCOVERY_VERSION: u32 = 1;
/// Override for tests (debug builds only): relocates discovery, state and file keys.
/// The directory must already exist, be owned by this user and be mode 0700.
pub const HOME_OVERRIDE: &str = "VAROS_BRIDGE_HOME";

/// Where discovery (runtime) and trust (state) data live for this user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    /// `<os-user-temp>/varos-bridge`
    pub runtime: PathBuf,
    /// persistent per-user state directory (public keys, scopes, profile references)
    pub state: PathBuf,
}
impl Paths {
    pub fn under(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        Self { runtime: dir.join("run"), state: dir.join("state") }
    }
    pub fn hosts(&self) -> PathBuf {
        self.runtime.join("hosts")
    }
    /// Persistent and owner-only (the macOS temp folder may be cleaned after a few days).
    pub fn audit(&self) -> PathBuf {
        self.state.join("audit")
    }
    /// Resolve from the platform APIs (or the explicit test override).
    pub fn resolve() -> Result<Self, Error> {
        // Test/dev only: a release build ignores the override (loudly) so an inherited
        // environment variable can never relocate a shipped app's trust data.
        let over = std::env::var_os(HOME_OVERRIDE);
        if over.is_some() && !cfg!(debug_assertions) {
            eprintln!("[varos-bridge] WARNING: {HOME_OVERRIDE} is set but ignored in release builds");
        }
        if let Some(dir) = over.filter(|_| cfg!(debug_assertions)) {
            let dir = PathBuf::from(dir);
            fsutil::check_private_dir(&dir).map_err(|e| {
                Error::new(
                    "unsupported",
                    format!("{HOME_OVERRIDE} must be an existing owner-only (0700) directory: {e}"),
                )
            })?;
            return Ok(Self::under(dir));
        }
        let temp = fsutil::user_temp_dir().map_err(|e| Error::new("unsupported", e.to_string()))?;
        let state = fsutil::app_support_dir().map_err(|e| Error::new("unsupported", e.to_string()))?;
        Ok(Self { runtime: temp.join("varos-bridge"), state })
    }
}

/// Lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}
pub fn random_hex(bytes: usize) -> Result<String, Error> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| Error::new("unsupported", format!("OS randomness unavailable: {e}")))?;
    Ok(hex(&buf))
}
pub fn is_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
/// Display-only client label: printable ASCII, bounded. It is a claim, never an identity.
pub fn clean_label(label: &str) -> String {
    let cleaned: String =
        label.chars().filter(|c| c.is_ascii_alphanumeric() || " ._-()".contains(*c)).take(64).collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "unnamed-agent".into()
    } else {
        cleaned
    }
}
