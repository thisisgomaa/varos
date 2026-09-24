//! The durable replacement writer (work order §3.3).
//!
//! [`write_replace`] replaces a file so that, at every instant, the destination holds either the
//! complete old bytes or the complete new bytes — never a mix, never nothing:
//! 1. a symlinked destination is resolved to its target; a read-only destination is refused; a
//!    missing folder is reported;
//! 2. the bytes go to a unique hidden temp **in the same folder** (`.{name}.{nonce}.varos-tmp`,
//!    created with O_EXCL — two writes never share a temp);
//! 3. `write_all` → `sync_all` → close → copy the destination's permissions onto the temp.
//!    On Apple targets `File::sync_all` issues `fcntl(F_FULLFSYNC)` (Rust 1.94.1
//!    `library/std/src/sys/fs/unix.rs:1381-1388`, `os_fsync` under `cfg(target_vendor = "apple")`),
//!    so the bytes are on the platter, not just in the drive cache. Some volumes (network shares,
//!    some USB sticks) reject F_FULLFSYNC with ENOTSUP/ENOTTY/EINVAL; then, like SQLite's
//!    `full_fsync`, Varos retries with a plain `fsync` before calling it a failure;
//! 4. `rename(temp, dest)` — POSIX `rename(2)` is an atomic replace; on Windows `std::fs::rename`
//!    replaces an existing file, and a sharing violation (file open elsewhere) fails **before**
//!    anything is replaced;
//! 5. Unix: the folder is synced (same fallback) so the rename itself survives power loss; a failed
//!    folder sync never fails the write — it reports [`WriteOutcome::ReplacedUnconfirmed`]. Windows: skipped;
//! 6. any failure before step 4 removes the temp (best effort) and leaves the destination untouched.
//!    The destination is never deleted first.
//!
//! All I/O goes through [`FsPort`] so tests inject failures at every step ([`FaultFs`]).
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// A writer whose contents can be forced to stable storage.
pub trait SyncWrite: Write + Send {
    fn sync_all(&mut self) -> io::Result<()>;
}

impl SyncWrite for std::fs::File {
    fn sync_all(&mut self) -> io::Result<()> {
        full_sync(self)
    }
}

/// `ENOTTY` — 25 on both macOS and Linux (checked against `libc` on macOS below).
const ENOTTY: i32 = 25;
#[cfg(target_os = "macos")]
const _: () = assert!(ENOTTY == libc::ENOTTY);

/// Did a full sync fail only because the volume doesn't support it (ENOTSUP / ENOTTY / EINVAL)?
fn full_sync_unsupported(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput) || e.raw_os_error() == Some(ENOTTY)
}

/// Run the full sync; if the volume rejects it as unsupported, run the plain sync instead
/// (SQLite's `full_fsync` fallback). Any other full-sync error is returned as is.
fn sync_with_fallback(full: impl FnOnce() -> io::Result<()>, plain: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
    match full() {
        Err(e) if full_sync_unsupported(&e) => plain(),
        r => r,
    }
}

/// Plain `fsync(2)` (no F_FULLFSYNC), retried on EINTR like std does.
#[cfg(target_os = "macos")]
fn plain_fsync(f: &std::fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    loop {
        // SAFETY: `f` owns an open descriptor for the whole call; `fsync` only reads the fd number.
        if unsafe { libc::fsync(f.as_raw_fd()) } == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// `File::sync_all` (F_FULLFSYNC on macOS) with the plain-`fsync` fallback for volumes that reject it.
#[cfg(target_os = "macos")]
fn full_sync(f: &std::fs::File) -> io::Result<()> {
    sync_with_fallback(|| f.sync_all(), || plain_fsync(f))
}

/// Elsewhere `File::sync_all` already is the plain sync (`fsync` / `FlushFileBuffers`).
#[cfg(not(target_os = "macos"))]
fn full_sync(f: &std::fs::File) -> io::Result<()> {
    f.sync_all()
}

/// What [`FsPort::metadata`] reports (symlinks followed).
#[derive(Clone, Debug)]
pub struct FileMeta {
    pub len: u64,
    pub modified: Option<SystemTime>,
    pub is_dir: bool,
    pub readonly: bool,
    pub permissions: std::fs::Permissions,
}

/// The file-system operations storage code may perform. [`RealFs`] is `std::fs`; [`FaultFs`]
/// wraps it with injected failures and an operation log.
pub trait FsPort: Send + Sync {
    /// Create a new file, failing if it already exists (O_EXCL).
    fn create_new(&self, path: &Path) -> io::Result<Box<dyn SyncWrite>>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    /// Flush a directory's entries (Unix). May return `Unsupported`/`InvalidInput` on some systems.
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    /// Remove an EMPTY directory.
    fn remove_dir(&self, path: &Path) -> io::Result<()>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn metadata(&self, path: &Path) -> io::Result<FileMeta>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    /// Entries of a directory (full paths, unsorted).
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
    fn set_permissions(&self, path: &Path, perms: std::fs::Permissions) -> io::Result<()>;
    /// If `path` is a symlink, the path it finally points at (which may not exist yet); otherwise
    /// `path` unchanged.
    fn resolve_link(&self, path: &Path) -> io::Result<PathBuf>;
}

/// `std::fs`, unchanged.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealFs;

impl FsPort for RealFs {
    fn create_new(&self, path: &Path) -> io::Result<Box<dyn SyncWrite>> {
        let f = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(Box::new(f))
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        if cfg!(windows) {
            // A directory cannot be opened as a file on Windows; NTFS journals the rename itself.
            return Err(io::Error::from(io::ErrorKind::Unsupported));
        }
        full_sync(&std::fs::File::open(dir)?)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }
    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir(path)
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<FileMeta> {
        let m = std::fs::metadata(path)?;
        Ok(FileMeta {
            len: m.len(),
            modified: m.modified().ok(),
            is_dir: m.is_dir(),
            readonly: m.permissions().readonly(),
            permissions: m.permissions(),
        })
    }
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        std::fs::read_dir(path)?.map(|e| e.map(|e| e.path())).collect()
    }
    fn set_permissions(&self, path: &Path, perms: std::fs::Permissions) -> io::Result<()> {
        std::fs::set_permissions(path, perms)
    }
    fn resolve_link(&self, path: &Path) -> io::Result<PathBuf> {
        let mut p = path.to_path_buf();
        // Follow a chain of links by hand (not `canonicalize`) so a dangling link still names its target.
        for _ in 0..40 {
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink() => {
                    let target = std::fs::read_link(&p)?;
                    p = match p.parent() {
                        Some(dir) if target.is_relative() => dir.join(target),
                        _ => target,
                    };
                }
                Ok(_) => return Ok(p),
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(p),
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other("too many levels of symbolic links"))
    }
}

/// How a successful replacement ended.
#[derive(Debug)]
pub enum WriteOutcome {
    /// Bytes synced, rename done and (on Unix) the folder synced.
    Durable,
    /// The destination now has the new bytes, but the folder sync failed: the rename may not
    /// survive a power loss. Callers keep their safety net (dirty flag / recovery copy).
    ReplacedUnconfirmed(io::Error),
}

/// Why a replacement failed. In every case the destination still holds its old bytes.
#[derive(Debug)]
pub enum WriteError {
    /// The destination's folder does not exist (or is not a folder).
    Folder,
    /// The destination is marked read-only; Varos refuses to rename over it.
    ReadOnly,
    Create(io::Error),
    Write(io::Error),
    Sync(io::Error),
    Replace(io::Error),
}

impl WriteError {
    /// The underlying OS error, if any.
    pub fn io_error(&self) -> Option<&io::Error> {
        match self {
            WriteError::Folder | WriteError::ReadOnly => None,
            WriteError::Create(e) | WriteError::Write(e) | WriteError::Sync(e) | WriteError::Replace(e) => Some(e),
        }
    }

    /// Plain-English reason, used as `{reason}` in the spec's user-facing copy.
    pub fn reason(&self) -> String {
        match self {
            WriteError::Folder => "The folder no longer exists.".to_string(),
            WriteError::ReadOnly => "The file is read-only.".to_string(),
            WriteError::Create(e) | WriteError::Write(e) | WriteError::Sync(e) | WriteError::Replace(e) => io_reason(e),
        }
    }
}

/// Plain-English text for an OS error (shared with other storage modules).
pub fn io_reason(e: &io::Error) -> String {
    // Windows ERROR_SHARING_VIOLATION (32) / ERROR_LOCK_VIOLATION (33). Raw 32 means EPIPE on Unix.
    if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) {
        return "The file is open in another app.".to_string();
    }
    match e.kind() {
        io::ErrorKind::StorageFull => "The disk is full.".to_string(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            "Varos isn't allowed to write there.".to_string()
        }
        _ => {
            // The OS text without Rust's " (os error N)" suffix, as a sentence.
            let s = e.to_string();
            let s = match s.rfind(" (os error ") {
                Some(i) => s[..i].to_string(),
                None => s,
            };
            let mut c = s.chars();
            let mut out: String = match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => "Unknown error".to_string(),
            };
            if !out.ends_with('.') {
                out.push('.');
            }
            out
        }
    }
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason())
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.io_error().map(|e| e as _)
    }
}

/// The hidden same-folder temp name used for one write to `dest`.
/// The name part is capped at [`TEMP_NAME_MAX_BYTES`] (cut on a character boundary) so a long
/// document name plus the nonce never exceeds the 255-byte file-name limit.
pub fn temp_path(dest: &Path, nonce: &str) -> PathBuf {
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut cut = name.len().min(TEMP_NAME_MAX_BYTES);
    while !name.is_char_boundary(cut) {
        cut -= 1;
    }
    let dir = dest.parent().unwrap_or(Path::new(""));
    dir.join(format!(".{}.{nonce}.varos-tmp", &name[..cut]))
}

/// Longest slice of the destination's file name kept in a temp name (bytes). With the dots, a
/// 32-char nonce and `.varos-tmp` the temp name stays ≤ 173 bytes.
pub const TEMP_NAME_MAX_BYTES: usize = 128;

/// Replace `dest` with `bytes` durably (see the module docs for the exact steps).
/// `nonce` makes the temp name unique — pass [`super::checksum::new_nonce`]; it must be a plain
/// name fragment (no path separators).
pub fn write_replace(fs: &dyn FsPort, dest: &Path, bytes: &[u8], nonce: &str) -> Result<WriteOutcome, WriteError> {
    debug_assert!(!nonce.is_empty() && !nonce.contains(['/', '\\']), "nonce must be a plain name fragment");
    // 1. Resolve a symlink to its target (replace the target, keep the link).
    let dest = fs.resolve_link(dest).map_err(WriteError::Create)?;
    if dest.file_name().is_none() {
        return Err(WriteError::Folder);
    }
    let parent = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    match fs.metadata(&parent) {
        Ok(m) if !m.is_dir => return Err(WriteError::Folder),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(WriteError::Folder),
        _ => {}
    }
    let old_perms = match fs.metadata(&dest) {
        Ok(m) if m.readonly => return Err(WriteError::ReadOnly),
        Ok(m) if !m.is_dir => Some(m.permissions),
        _ => None,
    };

    // 2–3. Unique temp in the same folder: write, sync, close, copy permissions.
    let temp = temp_path(&dest, nonce);
    let mut w = fs.create_new(&temp).map_err(WriteError::Create)?;
    let fail = |w: Box<dyn SyncWrite>, err: WriteError| {
        drop(w);
        let _ = fs.remove_file(&temp);
        Err(err)
    };
    if let Err(e) = w.write_all(bytes).and_then(|_| w.flush()) {
        return fail(w, WriteError::Write(e));
    }
    if let Err(e) = w.sync_all() {
        return fail(w, WriteError::Sync(e));
    }
    drop(w);
    if let Some(p) = old_perms {
        let _ = fs.set_permissions(&temp, p); // best effort; a new file keeps default permissions
    }

    // 4. Atomic replace.
    if let Err(e) = fs.rename(&temp, &dest) {
        let _ = fs.remove_file(&temp);
        return Err(WriteError::Replace(e));
    }

    // 5. Make the rename itself durable (Unix). Windows has no directory sync.
    if cfg!(windows) {
        return Ok(WriteOutcome::Durable);
    }
    match fs.sync_dir(&parent) {
        Ok(()) => Ok(WriteOutcome::Durable),
        // Some file systems cannot sync a directory at all (even after the plain-fsync fallback);
        // the rename is then as durable as that volume allows.
        Err(e) if full_sync_unsupported(&e) => Ok(WriteOutcome::Durable),
        // Never fails the write: the new bytes are in place, only the confirmation is missing.
        Err(e) => Ok(WriteOutcome::ReplacedUnconfirmed(e)),
    }
}

/// Size + modification time of a file, for "was it changed by another app?" checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

/// The file's current [`Fingerprint`], or `None` when it cannot be read (missing, no access).
pub fn fingerprint(fs: &dyn FsPort, path: &Path) -> Option<Fingerprint> {
    let m = fs.metadata(path).ok()?;
    (!m.is_dir).then_some(Fingerprint { len: m.len, modified: m.modified })
}

// ─────────────────────────────────── fault injection ───────────────────────────────────

/// Where a [`Fault`] fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Create,
    /// The write fails once `after` bytes have been written (a partial write happens first).
    Write {
        after: usize,
    },
    /// Both the full sync and its plain-`fsync` fallback fail.
    Sync,
    /// Only the full sync (F_FULLFSYNC) fails; the plain-`fsync` fallback then runs if the error
    /// kind is `Unsupported`/`InvalidInput` (or raw ENOTTY).
    FullSync,
    Rename,
    SyncDir,
    Remove,
    RemoveDir,
    CreateDir,
    Read,
}

/// One planned failure. Fires once (the first matching operation), then is used up.
#[derive(Clone, Debug)]
pub struct Fault {
    pub step: Step,
    /// Only operations whose path contains this text match (`None` = any path). For a rename the
    /// source and destination are both checked.
    pub path_contains: Option<String>,
    pub kind: io::ErrorKind,
}

impl Fault {
    /// A fault at `step` on any path, with `ErrorKind::Other`.
    pub fn at(step: Step) -> Self {
        Fault { step, path_contains: None, kind: io::ErrorKind::Other }
    }
    pub fn on(mut self, path_contains: &str) -> Self {
        self.path_contains = Some(path_contains.to_string());
        self
    }
    pub fn kind(mut self, kind: io::ErrorKind) -> Self {
        self.kind = kind;
        self
    }
    fn matches_path(&self, paths: &[&Path]) -> bool {
        match &self.path_contains {
            None => true,
            Some(s) => paths.iter().any(|p| p.to_string_lossy().contains(s.as_str())),
        }
    }
    fn error(&self) -> io::Error {
        io::Error::new(self.kind, format!("injected fault at {:?}", self.step))
    }
}

/// Operation kinds recorded by [`FaultFs`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Create,
    Rename,
    SyncDir,
    Remove,
    RemoveDir,
    Read,
    Metadata,
    CreateDir,
    ReadDir,
    SetPermissions,
    ResolveLink,
}

/// [`RealFs`] plus planned failures and a log of every path touched. Lives in the library (not
/// `cfg(test)`) so every storage module's tests can use it.
///
/// `Write`/`Sync` faults are bound to a file when it is created (the plan is consulted at
/// `create_new`), so push them before the write starts.
#[derive(Default)]
pub struct FaultFs {
    inner: RealFs,
    plan: Mutex<Vec<Fault>>,
    log: Mutex<Vec<(Op, PathBuf)>>,
}

impl FaultFs {
    pub fn new(plan: Vec<Fault>) -> Self {
        FaultFs { inner: RealFs, plan: Mutex::new(plan), log: Mutex::new(Vec::new()) }
    }
    /// Add one more planned failure.
    pub fn push(&self, f: Fault) {
        self.plan.lock().unwrap_or_else(|e| e.into_inner()).push(f);
    }
    /// Faults not yet fired.
    pub fn pending(&self) -> usize {
        self.plan.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
    /// Every operation so far, in order. A rename logs its source, then its destination.
    pub fn log(&self) -> Vec<(Op, PathBuf)> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    /// Every path an operation touched, in order (duplicates kept).
    pub fn touched(&self) -> Vec<PathBuf> {
        self.log().into_iter().map(|(_, p)| p).collect()
    }
    fn record(&self, op: Op, p: &Path) {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).push((op, p.to_path_buf()));
    }
    /// Remove and return the first planned fault accepted by `pred` whose path filter matches.
    fn take(&self, pred: impl Fn(&Step) -> bool, paths: &[&Path]) -> Option<Fault> {
        let mut plan = self.plan.lock().unwrap_or_else(|e| e.into_inner());
        let i = plan.iter().position(|f| pred(&f.step) && f.matches_path(paths))?;
        Some(plan.remove(i))
    }
    fn check(&self, step: Step, paths: &[&Path]) -> io::Result<()> {
        match self.take(|s| *s == step, paths) {
            Some(f) => Err(f.error()),
            None => Ok(()),
        }
    }
}

struct FaultWriter {
    inner: Box<dyn SyncWrite>,
    written: usize,
    write_fault: Option<(usize, Fault)>,
    sync_fault: Option<Fault>,
    full_sync_fault: Option<Fault>,
}

impl Write for FaultWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some((after, f)) = &self.write_fault {
            let room = after.saturating_sub(self.written);
            if room == 0 {
                return Err(f.error());
            }
            if buf.len() > room {
                let n = self.inner.write(&buf[..room])?;
                self.written += n;
                return Ok(n);
            }
        }
        let n = self.inner.write(buf)?;
        self.written += n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl SyncWrite for FaultWriter {
    fn sync_all(&mut self) -> io::Result<()> {
        if let Some(f) = self.sync_fault.take() {
            return Err(f.error());
        }
        // The same fallback decision as the real macOS path; the inner sync plays "plain fsync".
        let full_fault = self.full_sync_fault.take();
        let inner = &mut self.inner;
        match full_fault {
            Some(f) => sync_with_fallback(|| Err(f.error()), || inner.sync_all()),
            None => inner.sync_all(),
        }
    }
}

impl FsPort for FaultFs {
    fn create_new(&self, path: &Path) -> io::Result<Box<dyn SyncWrite>> {
        self.record(Op::Create, path);
        self.check(Step::Create, &[path])?;
        let inner = self.inner.create_new(path)?;
        let write_fault = self.take(|s| matches!(s, Step::Write { .. }), &[path]).map(|f| match f.step {
            Step::Write { after } => (after, f),
            _ => unreachable!("filtered to Write"),
        });
        let sync_fault = self.take(|s| *s == Step::Sync, &[path]);
        let full_sync_fault = self.take(|s| *s == Step::FullSync, &[path]);
        Ok(Box::new(FaultWriter { inner, written: 0, write_fault, sync_fault, full_sync_fault }))
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.record(Op::Rename, from);
        self.record(Op::Rename, to);
        self.check(Step::Rename, &[from, to])?;
        self.inner.rename(from, to)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        self.record(Op::SyncDir, dir);
        self.check(Step::SyncDir, &[dir])?;
        self.inner.sync_dir(dir)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.record(Op::Remove, path);
        self.check(Step::Remove, &[path])?;
        self.inner.remove_file(path)
    }
    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        self.record(Op::RemoveDir, path);
        self.check(Step::RemoveDir, &[path])?;
        self.inner.remove_dir(path)
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.record(Op::Read, path);
        self.check(Step::Read, &[path])?;
        self.inner.read(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<FileMeta> {
        self.record(Op::Metadata, path);
        self.inner.metadata(path)
    }
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.record(Op::CreateDir, path);
        self.check(Step::CreateDir, &[path])?;
        self.inner.create_dir_all(path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        self.record(Op::ReadDir, path);
        self.inner.read_dir(path)
    }
    fn set_permissions(&self, path: &Path, perms: std::fs::Permissions) -> io::Result<()> {
        self.record(Op::SetPermissions, path);
        self.inner.set_permissions(path, perms)
    }
    fn resolve_link(&self, path: &Path) -> io::Result<PathBuf> {
        self.record(Op::ResolveLink, path);
        self.inner.resolve_link(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::checksum::new_nonce;
    use crate::storage::testdir::TestDir;

    const OLD: &[u8] = b"old bytes \x00\x01 that must survive";
    const NEW: &[u8] = b"brand new bytes, longer than the old ones to make partial writes visible";

    /// A folder with `doc.vrs` holding OLD.
    fn setup(tag: &str) -> (TestDir, PathBuf) {
        let d = TestDir::new(tag);
        let dest = d.join("doc.vrs");
        std::fs::write(&dest, OLD).unwrap();
        (d, dest)
    }

    fn temps(d: &TestDir) -> Vec<String> {
        d.names().into_iter().filter(|n| n.ends_with(".varos-tmp")).collect()
    }

    #[test]
    fn replace_writes_bytes_and_leaves_no_temp() {
        let (d, dest) = setup("dur-ok");
        let out = write_replace(&RealFs, &dest, NEW, &new_nonce()).unwrap();
        assert!(matches!(out, WriteOutcome::Durable), "{out:?}");
        assert_eq!(std::fs::read(&dest).unwrap(), NEW);
        assert_eq!(d.names(), vec!["doc.vrs".to_string()]);
        // A brand-new destination works too.
        let fresh = d.join("fresh.vrs");
        write_replace(&RealFs, &fresh, b"x", &new_nonce()).unwrap();
        assert_eq!(std::fs::read(&fresh).unwrap(), b"x");
        assert!(temps(&d).is_empty());
    }

    fn assert_fault_keeps_old(tag: &str, fault: Fault, expect: fn(&WriteError) -> bool) {
        let (d, dest) = setup(tag);
        let fs = FaultFs::new(vec![fault]);
        let err = write_replace(&fs, &dest, NEW, &new_nonce()).unwrap_err();
        assert!(expect(&err), "unexpected error {err:?}");
        assert_eq!(fs.pending(), 0, "the fault must have fired");
        assert_eq!(std::fs::read(&dest).unwrap(), OLD, "old file must be byte-identical");
        assert!(temps(&d).is_empty(), "temp left behind: {:?}", d.names());
    }

    #[test]
    fn fail_at_create_keeps_old_file_byte_identical() {
        assert_fault_keeps_old("dur-create", Fault::at(Step::Create), |e| matches!(e, WriteError::Create(_)));
    }

    #[test]
    fn fail_at_write_keeps_old_file_byte_identical() {
        // Fails after 7 bytes: a partial temp exists, then must be removed.
        assert_fault_keeps_old("dur-write", Fault::at(Step::Write { after: 7 }), |e| matches!(e, WriteError::Write(_)));
    }

    #[test]
    fn fail_at_sync_keeps_old_file_byte_identical() {
        assert_fault_keeps_old("dur-sync", Fault::at(Step::Sync), |e| matches!(e, WriteError::Sync(_)));
    }

    #[test]
    fn full_sync_unsupported_falls_back_to_plain_fsync() {
        // F_FULLFSYNC rejected by the volume (ENOTSUP / EINVAL / ENOTTY) ⇒ plain fsync ⇒ saved, durable.
        let enotty = || io::Error::from_raw_os_error(ENOTTY);
        assert!(full_sync_unsupported(&enotty()));
        for kind in [io::ErrorKind::Unsupported, io::ErrorKind::InvalidInput] {
            let (d, dest) = setup("dur-fullsync");
            let fs = FaultFs::new(vec![Fault::at(Step::FullSync).kind(kind)]);
            let out = write_replace(&fs, &dest, NEW, &new_nonce()).unwrap();
            assert!(matches!(out, WriteOutcome::Durable), "{kind:?}: {out:?}");
            assert_eq!(fs.pending(), 0);
            assert_eq!(std::fs::read(&dest).unwrap(), NEW);
            assert!(temps(&d).is_empty());
        }
        // Any other full-sync error (e.g. an I/O error) is NOT papered over: the write fails, old file kept.
        assert_fault_keeps_old("dur-fullsync-io", Fault::at(Step::FullSync), |e| matches!(e, WriteError::Sync(_)));
        // The fallback helper: plain sync runs only for "unsupported" errors, and its result is final.
        let ran = std::cell::Cell::new(false);
        let plain_ok = || {
            ran.set(true);
            Ok(())
        };
        assert!(sync_with_fallback(|| Err(enotty()), plain_ok).is_ok());
        assert!(ran.replace(false));
        let r = sync_with_fallback(|| Err(io::Error::other("eio")), || unreachable!("no fallback for real I/O errors"));
        assert!(r.is_err());
        let r = sync_with_fallback(|| Err(io::ErrorKind::Unsupported.into()), || Err(io::Error::other("plain failed")));
        assert_eq!(r.unwrap_err().to_string(), "plain failed");
    }

    #[test]
    fn fail_at_rename_keeps_old_file_byte_identical() {
        assert_fault_keeps_old("dur-rename", Fault::at(Step::Rename), |e| matches!(e, WriteError::Replace(_)));
    }

    #[test]
    fn failed_write_removes_temp() {
        let (d, dest) = setup("dur-cleanup");
        let fs = FaultFs::new(vec![Fault::at(Step::Write { after: 3 })]);
        let nonce = new_nonce();
        write_replace(&fs, &dest, NEW, &nonce).unwrap_err();
        let temp = temp_path(&dest, &nonce);
        assert!(fs.log().contains(&(Op::Create, temp.clone())), "temp was created");
        assert!(fs.log().contains(&(Op::Remove, temp.clone())), "temp removal was attempted");
        assert!(!temp.exists());
        assert_eq!(d.names(), vec!["doc.vrs".to_string()]);
    }

    #[test]
    fn dir_sync_failure_reports_replaced_unconfirmed() {
        let (d, dest) = setup("dur-dirsync");
        let fs = FaultFs::new(vec![Fault::at(Step::SyncDir)]);
        let out = write_replace(&fs, &dest, NEW, &new_nonce()).unwrap();
        assert!(matches!(out, WriteOutcome::ReplacedUnconfirmed(_)), "{out:?}");
        assert_eq!(std::fs::read(&dest).unwrap(), NEW, "the rename already happened");
        assert!(temps(&d).is_empty());
        // Unsupported/InvalidInput from the directory sync count as durable.
        for kind in [io::ErrorKind::Unsupported, io::ErrorKind::InvalidInput] {
            let fs = FaultFs::new(vec![Fault::at(Step::SyncDir).kind(kind)]);
            let out = write_replace(&fs, &dest, OLD, &new_nonce()).unwrap();
            assert!(matches!(out, WriteOutcome::Durable), "{kind:?}: {out:?}");
        }
    }

    #[test]
    fn disk_full_reason_is_plain_english() {
        let (d, dest) = setup("dur-full");
        let fs = FaultFs::new(vec![Fault::at(Step::Write { after: 10 }).kind(io::ErrorKind::StorageFull)]);
        let err = write_replace(&fs, &dest, NEW, &new_nonce()).unwrap_err();
        assert!(matches!(err, WriteError::Write(_)));
        assert_eq!(err.reason(), "The disk is full.");
        assert_eq!(std::fs::read(&dest).unwrap(), OLD);
        assert!(temps(&d).is_empty());
        let denied = WriteError::Create(io::Error::from(io::ErrorKind::PermissionDenied));
        assert_eq!(denied.reason(), "Varos isn't allowed to write there.");
        let rofs = WriteError::Replace(io::Error::from(io::ErrorKind::ReadOnlyFilesystem));
        assert_eq!(rofs.reason(), "Varos isn't allowed to write there.");
        assert_eq!(WriteError::Folder.reason(), "The folder no longer exists.");
        // Other errors: the OS text as a sentence, without the "(os error N)" tail.
        let other = WriteError::Sync(io::Error::other("device went away"));
        assert_eq!(other.reason(), "Device went away.");
    }

    #[test]
    fn read_only_destination_is_refused() {
        let (d, dest) = setup("dur-ro");
        let mut p = std::fs::metadata(&dest).unwrap().permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&dest, p).unwrap();
        let fs = FaultFs::new(vec![]);
        let err = write_replace(&fs, &dest, NEW, &new_nonce()).unwrap_err();
        assert!(matches!(err, WriteError::ReadOnly), "{err:?}");
        assert_eq!(err.reason(), "The file is read-only.");
        assert_eq!(std::fs::read(&dest).unwrap(), OLD);
        assert!(!fs.log().iter().any(|(op, _)| *op == Op::Create || *op == Op::Rename), "nothing written");
        assert_eq!(d.names(), vec!["doc.vrs".to_string()]);
    }

    #[test]
    fn missing_folder_is_reported() {
        let d = TestDir::new("dur-nofolder");
        let dest = d.join("gone").join("doc.vrs");
        let err = write_replace(&RealFs, &dest, NEW, &new_nonce()).unwrap_err();
        assert!(matches!(err, WriteError::Folder), "{err:?}");
        assert!(d.names().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destination_replaces_target_not_link() {
        let d = TestDir::new("dur-link");
        std::fs::create_dir(d.join("real")).unwrap();
        let target = d.join("real").join("doc.vrs");
        std::fs::write(&target, OLD).unwrap();
        let link = d.join("link.vrs");
        std::os::unix::fs::symlink(Path::new("real").join("doc.vrs"), &link).unwrap();
        write_replace(&RealFs, &link, NEW, &new_nonce()).unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link stays a link");
        assert_eq!(std::fs::read(&target).unwrap(), NEW, "the target got the bytes");
        assert!(temps(&d).is_empty());
        let real_names: Vec<String> = std::fs::read_dir(d.join("real"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into())
            .collect();
        assert_eq!(real_names, vec!["doc.vrs".to_string()], "temp lived next to the target and is gone");
    }

    #[cfg(unix)]
    #[test]
    fn permissions_are_preserved() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, dest) = setup("dur-perm");
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o640)).unwrap();
        write_replace(&RealFs, &dest, NEW, &new_nonce()).unwrap();
        assert_eq!(std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777, 0o640);
        assert_eq!(std::fs::read(&dest).unwrap(), NEW);
    }

    #[test]
    fn two_writes_use_distinct_temps() {
        let (d, dest) = setup("dur-twotemps");
        let (a, b) = (new_nonce(), new_nonce());
        assert_ne!(temp_path(&dest, &a), temp_path(&dest, &b));
        // The temp is a hidden sibling in the same folder, never the old fixed `.vrs.tmp`.
        let t = temp_path(&dest, &a);
        assert_eq!(t.parent(), dest.parent());
        let name = t.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name, format!(".doc.vrs.{a}.varos-tmp"));
        // A leftover temp from an earlier crashed write never blocks (or is clobbered by) a new write.
        std::fs::write(temp_path(&dest, &a), b"stale").unwrap();
        let fs = FaultFs::new(vec![]);
        write_replace(&fs, &dest, NEW, &b).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), NEW);
        assert_eq!(std::fs::read(temp_path(&dest, &a)).unwrap(), b"stale");
        let created: Vec<PathBuf> = fs.log().into_iter().filter(|(op, _)| *op == Op::Create).map(|(_, p)| p).collect();
        assert_eq!(created, vec![temp_path(&dest, &b)]);
        assert!(d.names().contains(&"doc.vrs".to_string()));
    }

    #[test]
    fn long_names_get_a_capped_temp_name() {
        let d = TestDir::new("dur-longname");
        // "a" + 116 Arabic letters (2 bytes each) + ".vrs" = 237 bytes: legal, but + nonce would exceed 255.
        // Byte 128 falls inside a letter, so the cut must step back to 127.
        let name = format!("a{}.vrs", "\u{645}".repeat(116));
        assert_eq!(name.len(), 237);
        let dest = d.join(&name);
        let nonce = new_nonce();
        let t = temp_path(&dest, &nonce);
        let tname = t.file_name().unwrap().to_string_lossy().into_owned();
        assert!(tname.len() <= 255, "temp name is {} bytes", tname.len());
        assert!(tname.starts_with(&format!(".a{}.", "\u{645}".repeat(63))), "cut on a character boundary");
        assert!(tname.ends_with(&format!(".{nonce}.varos-tmp")));
        write_replace(&RealFs, &dest, NEW, &nonce).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), NEW);
        assert_eq!(d.names(), vec![name]);
        // Short names are kept whole.
        assert_eq!(temp_path(Path::new("doc.vrs"), "n"), PathBuf::from(".doc.vrs.n.varos-tmp"));
    }

    #[test]
    fn fingerprint_tracks_len_and_mtime() {
        let (d, dest) = setup("dur-fp");
        let fp = fingerprint(&RealFs, &dest).unwrap();
        assert_eq!(fp.len, OLD.len() as u64);
        assert!(fp.modified.is_some());
        write_replace(&RealFs, &dest, NEW, &new_nonce()).unwrap();
        assert_ne!(fingerprint(&RealFs, &dest), Some(fp));
        assert_eq!(fingerprint(&RealFs, &d.join("missing.vrs")), None);
        assert_eq!(fingerprint(&RealFs, d.path()), None, "a folder has no fingerprint");
    }
}
