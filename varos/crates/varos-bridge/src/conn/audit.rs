//! Bounded owner-only audit log (ADR-0011 §3), in the persistent state folder: time, agent/session, request id, verb, opaque
//! board handle, revisions, result code and pairing/revocation events. Never names, paths,
//! prompts, geometry, payloads or credentials. Rotates at 10 MiB; rotated file deleted after
//! 30 days. Diagnostic, not tamper-proof against the account owner.
use super::{fsutil, now_secs, Paths};
use serde::Serialize;
use std::io::Write;

pub const AUDIT_FILE: &str = "audit.log";
pub const ROTATED_FILE: &str = "audit.log.1";
pub const LOCK_FILE: &str = "audit.lock";
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_AGE_SECS: u64 = 30 * 24 * 3600;

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct Entry {
    pub t: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket: Option<u64>,
    pub event: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub agent: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub session: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_rev: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_rev: Option<u64>,
    pub result: String,
}
impl Entry {
    pub fn event(event: &str, agent: &str, result: &str) -> Self {
        Self { t: now_secs(), event: event.into(), agent: agent.into(), result: result.into(), ..Self::default() }
    }
}
/// Append one entry. An error means the caller must deny new mutations.
pub fn append(paths: &Paths, entry: &Entry) -> std::io::Result<()> {
    let dir = paths.audit();
    fsutil::ensure_private_dir(&paths.state)?;
    fsutil::ensure_private_dir(&dir)?;
    // Rotation + append under one lock: concurrent hosts/CLIs never lose or interleave a rotation.
    let _lock = fsutil::lock(&dir.join(LOCK_FILE))?;
    let path = dir.join(AUDIT_FILE);
    let rotated = dir.join(ROTATED_FILE);
    if let Ok(m) = std::fs::symlink_metadata(&rotated) {
        let old = m.modified().ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a.as_secs() > MAX_AGE_SECS);
        if old {
            fsutil::remove_owned(&rotated)?;
        }
    }
    match fsutil::check_private_file(&path) {
        Ok(m) if m.len() >= MAX_BYTES => {
            std::fs::rename(&path, &rotated)?;
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut line = serde_json::to_vec(entry).map_err(std::io::Error::other)?;
    line.push(b'\n');
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)?
    };
    #[cfg(not(unix))]
    let mut file = std::fs::OpenOptions::new().append(true).create(true).open(&path)?;
    fsutil::check_opened(&file, &path)?;
    file.write_all(&line)
}
/// Most recent lines (bounded), for `varos-cli bridge agents audit`.
pub fn tail(paths: &Paths, max: usize) -> Vec<String> {
    let path = paths.audit().join(AUDIT_FILE);
    let mut text = String::new();
    let Ok(file) = fsutil::open_private(&path) else { return vec![] };
    if std::io::Read::read_to_string(&mut std::io::Read::take(file, MAX_BYTES + 1), &mut text).is_err() {
        return vec![];
    }
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines[lines.len().saturating_sub(max)..].to_vec()
}
