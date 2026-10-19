//! Where a running Varos listens for agents, and the key that proves an agent runs as the same
//! user (ADR-0011 Amendments 2 and 3, owner decisions 2026-10-08 and 2026-10-19).
//!
//! Every launch of the desktop app listens on a fresh loopback port and writes one endpoint file,
//! `<agents dir>/endpoint-<pid>.json`, holding the port and a fresh random 256-bit token. The
//! folder is private to the user (0700) and the file too (0600), so only the user's own processes
//! can read the token; the MCP proxy reads it and sends it with every request. There is no pairing,
//! approval, password or keychain prompt, ever. The file is removed when the app quits; a file left
//! by a crash points at a port nothing answers on, and the proxy skips it.
//!
//! The honest limit: any process of the same user can read the file and edit documents. The safety
//! model is undo plus per-agent history, not a gate.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The format of the endpoint file.
pub const VERSION: u64 = 1;
/// Environment variable that moves the agents folder (tests, isolated profiles).
pub const DIR_ENV: &str = "VAROS_AGENTS_DIR";

/// What one endpoint file says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
    pub pid: u32,
    /// When the app started, milliseconds since 1970: the newest launch first when several run.
    pub started_ms: u64,
}

impl Endpoint {
    /// A new endpoint for this process on `port`, with a fresh token.
    pub fn new(port: u16) -> std::io::Result<Self> {
        let started_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        Ok(Self { port, token: new_token()?, pid: std::process::id(), started_ms })
    }

    pub fn to_json(&self) -> Value {
        json!({"version": VERSION, "port": self.port, "token": self.token, "pid": self.pid, "started_ms": self.started_ms})
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        if v.get("version")?.as_u64()? != VERSION {
            return None;
        }
        let token = v.get("token")?.as_str()?;
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            port: u16::try_from(v.get("port")?.as_u64()?).ok().filter(|p| *p > 0)?,
            token: token.to_string(),
            pid: u32::try_from(v.get("pid")?.as_u64()?).ok()?,
            started_ms: v.get("started_ms").and_then(Value::as_u64).unwrap_or(0),
        })
    }

    /// The loopback address to connect to.
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

/// The folder endpoint files live in: [`DIR_ENV`] when set, else `Varos/agents` in the user's
/// application-support folder (macOS), `%APPDATA%` (Windows) or `$XDG_CONFIG_HOME` / `~/.config`
/// (`varos/agents`, Linux and BSD).
pub fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os(DIR_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let home = || std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Application Support/Varos/agents"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").filter(|a| !a.is_empty()).map(|a| PathBuf::from(a).join("Varos").join("agents"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|x| !x.is_empty())
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".config")))
            .map(|c| c.join("varos").join("agents"))
    }
}

/// The endpoint file of process `pid` in `dir`.
pub fn file_for(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("endpoint-{pid}.json"))
}

/// Write `endpoint` into `dir` (created private to the user), atomically: a reader never sees a
/// half-written file. Returns the file's path, for [`remove`] at quit.
pub fn publish(dir: &Path, endpoint: &Endpoint) -> std::io::Result<PathBuf> {
    make_private_dir(dir)?;
    let path = file_for(dir, endpoint.pid);
    let tmp = dir.join(format!(".endpoint-{}.tmp", endpoint.pid));
    let _ = std::fs::remove_file(&tmp);
    {
        let mut f = private_new_file(&tmp)?;
        f.write_all(endpoint.to_json().to_string().as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Remove an endpoint file this process published.
pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// The endpoints published in `dir`, newest launch first. Unreadable, malformed or (on Unix)
/// not-private files are skipped: a file anyone could write is not trusted to name a port.
pub fn list(dir: &Path) -> Vec<Endpoint> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    if !is_private(dir) {
        return vec![];
    }
    let mut found: Vec<Endpoint> = entries
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with("endpoint-") && n.ends_with(".json")))
        .filter_map(|e| {
            let path = e.path();
            if !is_private(&path) || std::fs::symlink_metadata(&path).map_or(true, |m| !m.is_file()) {
                return None;
            }
            let mut text = String::new();
            std::fs::File::open(&path).ok()?.take(4096).read_to_string(&mut text).ok()?;
            Endpoint::from_json(&serde_json::from_str(&text).ok()?)
        })
        .collect();
    found.sort_by(|a, b| b.started_ms.cmp(&a.started_ms).then(b.pid.cmp(&a.pid)));
    found
}

/// 32 random bytes from the operating system, as 64 hex digits.
fn new_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    fill_random(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(unix)]
fn fill_random(bytes: &mut [u8]) -> std::io::Result<()> {
    std::fs::File::open("/dev/urandom")?.read_exact(bytes)
}

/// Without `/dev/urandom`: the standard library's hash keys, which it seeds from the operating
/// system's random source, mixed with the time.
#[cfg(not(unix))]
fn fill_random(bytes: &mut [u8]) -> std::io::Result<()> {
    use std::hash::{BuildHasher, Hasher};
    for (i, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_usize(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        for (b, v) in chunk.iter_mut().zip(h.finish().to_le_bytes()) {
            *b = v;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if let Ok(m) = std::fs::symlink_metadata(dir) {
        if !m.is_dir() {
            return Err(std::io::Error::other(format!("{} is not a folder", dir.display())));
        }
    } else {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn make_private_dir(dir: &Path) -> std::io::Result<()> {
    // The user's profile folder's own permissions protect it on Windows.
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn private_new_file(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn private_new_file(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new().write(true).create_new(true).open(path)
}

/// No access for group or others (Unix; on Windows the profile folder's permissions apply). In a
/// folder that is itself private nobody else can have put a file, so the mode is enough.
#[cfg(unix)]
fn is_private(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path).is_ok_and(|m| m.mode() & 0o077 == 0)
}

#[cfg(not(unix))]
fn is_private(_: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("varos-endpoint-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_published_endpoint_is_listed_and_private() {
        let dir = temp("publish").join("agents");
        let e = Endpoint::new(4242).unwrap();
        assert_eq!(e.token.len(), 64);
        assert_ne!(Endpoint::new(4242).unwrap().token, e.token, "every launch gets a fresh token");
        let path = publish(&dir, &e).unwrap();
        assert_eq!(list(&dir), vec![e.clone()]);
        assert_eq!(e.addr(), "127.0.0.1:4242");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        remove(&path);
        assert!(list(&dir).is_empty());
    }

    #[test]
    fn newest_launch_first_and_junk_skipped() {
        let dir = temp("order");
        let mut a = Endpoint::new(1000).unwrap();
        a.pid = 1;
        a.started_ms = 10;
        let mut b = Endpoint::new(2000).unwrap();
        b.pid = 2;
        b.started_ms = 20;
        publish(&dir, &a).unwrap();
        publish(&dir, &b).unwrap();
        std::fs::write(dir.join("endpoint-3.json"), "{not json").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join("endpoint-3.json"), std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert_eq!(list(&dir).iter().map(|e| e.port).collect::<Vec<_>>(), vec![2000, 1000]);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_others_can_read_or_write_is_not_trusted() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp("open");
        let e = Endpoint::new(3000).unwrap();
        let path = publish(&dir, &e).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(list(&dir).is_empty());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(list(&dir).is_empty(), "a folder others can list is not trusted either");
    }

    #[test]
    fn malformed_records_are_refused() {
        let ok = Endpoint::new(1).unwrap().to_json();
        assert!(Endpoint::from_json(&ok).is_some());
        for bad in [
            json!({"version": 2, "port": 1, "token": ok["token"], "pid": 1}),
            json!({"version": 1, "port": 0, "token": ok["token"], "pid": 1}),
            json!({"version": 1, "port": 70000, "token": ok["token"], "pid": 1}),
            json!({"version": 1, "port": 1, "token": "short", "pid": 1}),
            json!({"version": 1, "port": 1, "token": "z".repeat(64), "pid": 1}),
        ] {
            assert_eq!(Endpoint::from_json(&bad), None, "{bad}");
        }
    }
}
