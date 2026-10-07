//! Per-user discovery registry (ADR-0011 §2): one owner-only entry directory per live host,
//! `hosts/<instance-id>/endpoint.json` + `b.sock`. A record only *locates* a candidate; it never
//! authenticates or authorizes one. No credential, board name, file path or document data.
use super::{fsutil, is_hex, Paths, DISCOVERY_VERSION};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const RECORD_FILE: &str = "endpoint.json";
pub const SOCKET_FILE: &str = "b.sock";
/// Scan bound: never walk more than this many host entries.
pub const MAX_ENTRIES: usize = 64;
/// Hard cap on names read from the directory before sorting.
pub const MAX_LISTED: usize = 4096;
/// Usable `sun_path` bytes (macOS: 104 including the NUL; Linux: 108).
#[cfg(target_os = "macos")]
pub const MAX_SOCKET_PATH: usize = 103;
#[cfg(not(target_os = "macos"))]
pub const MAX_SOCKET_PATH: usize = 107;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRange {
    pub min: String,
    pub max: String,
}
/// Additive fields from newer hosts are tolerated; required fields are validated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub discovery_version: u32,
    pub instance_id: String,
    pub epoch: String,
    pub pid: u32,
    /// Process start identity (guards pid reuse).
    pub start: String,
    /// `desktop` or `headless`.
    pub mode: String,
    pub socket: PathBuf,
    pub connection: VersionRange,
    pub api: String,
    pub app_build: String,
    pub host_fingerprint: String,
}
impl Record {
    pub fn validate(&self, entry_dir: &Path) -> Result<(), String> {
        if self.discovery_version != DISCOVERY_VERSION {
            return Err(format!("unsupported discovery_version {}", self.discovery_version));
        }
        if !is_hex(&self.instance_id, 16) || !is_hex(&self.epoch, 64) || !is_hex(&self.host_fingerprint, 64) {
            return Err("malformed instance/epoch/fingerprint".into());
        }
        if entry_dir.file_name().and_then(|n| n.to_str()) != Some(self.instance_id.as_str()) {
            return Err("record instance does not match its directory".into());
        }
        if self.socket != entry_dir.join(SOCKET_FILE) {
            return Err("socket must live inside its own entry directory".into());
        }
        if !matches!(self.mode.as_str(), "desktop" | "headless") {
            return Err("unknown host mode".into());
        }
        if self.app_build.len() > 128 || !self.app_build.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
            return Err("malformed app_build".into());
        }
        if self.start.len() > 64
            || self.api.len() > 16
            || self.connection.min.len() > 16
            || self.connection.max.len() > 16
        {
            return Err("oversized field".into());
        }
        Ok(())
    }
    /// Non-secret candidate summary for `ambiguous_target` (no names, paths or epoch).
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"instance":self.instance_id,"pid":self.pid,"mode":self.mode,"app_build":self.app_build})
    }
}

/// Process start identity, or None if the process is gone / cannot be inspected.
#[cfg(target_os = "macos")]
pub fn process_start(pid: u32) -> Option<String> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: info is a correctly sized, writable proc_bsdinfo for PROC_PIDTBSDINFO.
    let n = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    (n == size).then(|| format!("{}.{:06}", info.pbi_start_tvsec, info.pbi_start_tvusec))
}
#[cfg(target_os = "linux")]
pub fn process_start(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Field 22 (starttime) follows the parenthesised comm, which may contain spaces.
    stat.rsplit_once(')')?.1.split_whitespace().nth(19).map(str::to_owned)
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn process_start(_pid: u32) -> Option<String> {
    None
}
pub fn alive(record: &Record) -> bool {
    process_start(record.pid).as_deref() == Some(record.start.as_str())
}

/// Validate a socket path against the platform `sun_path` limit.
pub fn check_socket_path(path: &Path) -> Result<(), Error> {
    let len = path.as_os_str().len();
    if len > MAX_SOCKET_PATH {
        return Err(Error::new(
            "unsupported",
            format!("socket path is {len} bytes; the platform limit is {MAX_SOCKET_PATH}: {}", path.display()),
        ));
    }
    Ok(())
}

/// A host's own registry entry. Removal is identity-checked against the published record.
#[derive(Debug)]
pub struct Entry {
    pub dir: PathBuf,
    pub record: Record,
}
impl Entry {
    /// Reserve `hosts/<instance>/` (0700, must not exist) and return the socket path to bind.
    pub fn reserve(paths: &Paths, instance_id: &str) -> Result<PathBuf, Error> {
        let io = |e: std::io::Error| Error::new("unsupported", format!("registry: {e}"));
        fsutil::ensure_private_dir(&paths.runtime).map_err(io)?;
        fsutil::ensure_private_dir(&paths.hosts()).map_err(io)?;
        let dir = paths.hosts().join(instance_id);
        let socket = dir.join(SOCKET_FILE);
        check_socket_path(&socket)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(&dir).map_err(io)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir(&dir).map_err(io)?;
        fsutil::check_private_dir(&dir).map_err(io)?;
        Ok(socket)
    }
    /// Publish the record by atomic rename (after the socket is bound and 0600).
    pub fn publish(paths: &Paths, record: Record) -> Result<Self, Error> {
        let dir = paths.hosts().join(&record.instance_id);
        record.validate(&dir).map_err(|e| Error::new("invalid_argument", e))?;
        let bytes = serde_json::to_vec(&record).expect("record serializes");
        fsutil::write_atomic(&dir.join(RECORD_FILE), &bytes)
            .map_err(|e| Error::new("unsupported", format!("registry publish: {e}")))?;
        Ok(Self { dir, record })
    }
    /// Remove only this host's identity-checked entry; never recursive.
    pub fn remove(&self) {
        let path = self.dir.join(RECORD_FILE);
        let ours =
            fsutil::read_private(&path).ok().and_then(|b| serde_json::from_slice::<Record>(&b).ok()).is_some_and(|r| {
                r.instance_id == self.record.instance_id && r.pid == self.record.pid && r.epoch == self.record.epoch
            });
        if ours {
            let _ = fsutil::remove_owned(&path);
        }
        let _ = fsutil::remove_owned(&self.dir.join(SOCKET_FILE));
        let _ = std::fs::remove_dir(&self.dir);
    }
}

/// Live, validated records plus bounded diagnostics for ignored entries.
#[derive(Debug, Default)]
pub struct Scan {
    pub live: Vec<Record>,
    pub stale: Vec<PathBuf>,
    pub diagnostics: Vec<String>,
}
pub fn scan(paths: &Paths) -> Scan {
    scan_with(paths, alive)
}
pub fn scan_with(paths: &Paths, is_alive: impl Fn(&Record) -> bool) -> Scan {
    let mut out = Scan::default();
    let hosts = paths.hosts();
    if fsutil::check_private_dir(&paths.runtime).is_err() || fsutil::check_private_dir(&hosts).is_err() {
        return out;
    }
    let Ok(entries) = std::fs::read_dir(&hosts) else { return out };
    // Sort before bounding, so which entries are examined never depends on directory order.
    let mut names: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).take(MAX_LISTED).collect();
    names.sort();
    if names.len() > MAX_ENTRIES {
        out.diagnostics.push(format!("more than {MAX_ENTRIES} registry entries; extra entries ignored"));
        names.truncate(MAX_ENTRIES);
    }
    for dir in names {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if fsutil::check_private_dir(&dir).is_err() {
            out.diagnostics.push(format!("{name}: not an owner-only directory; ignored"));
            continue;
        }
        let record = fsutil::read_private(&dir.join(RECORD_FILE))
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice::<Record>(&b).map_err(|e| e.to_string()))
            .and_then(|r| r.validate(&dir).map(|()| r));
        match record {
            Ok(r) if is_alive(&r) => out.live.push(r),
            Ok(_) => {
                out.diagnostics.push(format!("{name}: host process is gone (stale entry)"));
                out.stale.push(dir);
            }
            Err(e) => {
                out.diagnostics.push(format!("{name}: {e}"));
                // A half-published or malformed entry whose directory is old is stale too.
                let old = std::fs::symlink_metadata(&dir)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > std::time::Duration::from_secs(600));
                if old {
                    out.stale.push(dir);
                }
            }
        }
    }
    out.diagnostics.truncate(16);
    out
}
/// Bounded stale cleanup: only our two known file names, then a non-recursive rmdir.
pub fn cleanup_stale(scan: &Scan, max: usize) -> usize {
    let mut removed = 0;
    for dir in scan.stale.iter().take(max) {
        let _ = fsutil::remove_owned(&dir.join(RECORD_FILE));
        let _ = fsutil::remove_owned(&dir.join(SOCKET_FILE));
        if std::fs::remove_dir(dir).is_ok() {
            removed += 1;
        }
    }
    removed
}
