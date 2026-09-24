//! Recovery snapshot store (work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3.5, piece C).
//!
//! Layout: `Recovery/<rid>/{session.lock, manifest.json, snap-<seq>.json}`, one folder per editing
//! session (`rid` = a fresh nonce per session).
//! - **Model-only blobs.** A snapshot is exactly the bytes of `varos_core::file::doc_to_blob(doc)`
//!   (no PDF, never the user's original file), so recovery decodes through the same function Open
//!   uses. The store itself is bytes-level: it never sees a `Document`.
//! - **Manifest published last.** `write_generation` writes the blob (`durable::write_replace`),
//!   then the manifest (`write_replace` again). Until the manifest's rename lands, the previous
//!   manifest and both of its generations are untouched; a new blob never overwrites a listed file
//!   because its `seq` is always above every listed one. At most two generations (N and N-1) are
//!   kept; older blobs are removed only after the new manifest is confirmed durable.
//! - **Checked on read.** The manifest records each blob's size and CRC-32. `load_best` returns the
//!   newest generation that matches and parses, else the previous one (`fell_back`), else
//!   `Damaged`. An unreadable manifest is `Damaged` too (no guessing from loose blobs). Damaged and
//!   newer-format folders are **kept**, never age-deleted: only [`RecoveryStore::retire`] (explicit
//!   Discard, clean save, Don't Save, normal quit) or its `.discarded-<nonce>` tombstone removes data.
//! - **One lock per session.** The live session holds `session.lock` (`File::try_lock`, an OS lock
//!   released automatically when the process dies). `scan` try-locks every folder: success means an
//!   orphan left by a dead process, and the lock taken there IS this process's claim on it (kept
//!   until Recover/Discard or exit); a folder locked elsewhere belongs to a live session and is
//!   skipped. std's `try_lock` is `flock` on Unix and `LockFileEx` on Windows; both conflict between
//!   two opens inside one process, so "two stores in one process" behaves like two processes.
//! - **Retire** = atomic `rename(<rid>, <rid>.discarded-<nonce>)`, then best-effort removal. A rid
//!   retired in this process is refused afterwards: the session must take a fresh rid, so a
//!   snapshot can never land in a folder that a later cleanup deletes.
//! - **Only under `Recovery/`.** All file I/O goes through [`FsPort`] with paths built from the
//!   recovery folder plus a validated rid and fixed file names — except the lock file, which is
//!   opened with `std::fs` because `FsPort` (frozen after piece A) has no lock operation.
use std::collections::{HashMap, HashSet};
use std::fs::{File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use super::checksum::{crc32, new_nonce};
use super::durable::{io_reason, write_replace, Fingerprint, FsPort, WriteError, WriteOutcome};

/// Largest snapshot blob accepted (the spec's provisional model limit; S5 may change it).
pub const MAX_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
/// The manifest format this build writes and reads.
pub const MANIFEST_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "manifest.json";
pub const LOCK_FILE: &str = "session.lock";
/// Marker inside a retired folder's name: `<rid>.discarded-<nonce>`.
pub const DISCARDED_MARK: &str = ".discarded-";
/// Generations kept per session (N and N-1).
pub const GENERATIONS_KEPT: usize = 2;

/// One snapshot as listed in the manifest (newest first).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generation {
    pub seq: u64,
    /// Always `snap-<seq>.json`; anything else read from disk is treated as damage.
    pub file: String,
    pub bytes: u64,
    pub crc32: u32,
    /// Unix seconds.
    pub saved_at: u64,
}

/// `Recovery/<rid>/manifest.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_version: u32,
    pub rid: String,
    pub display_name: String,
    pub untitled_number: Option<u32>,
    pub original_path: Option<PathBuf>,
    pub source_fingerprint: Option<Fingerprint>,
    pub recovered: bool,
    /// `varos_core::file::VRS_VERSION` of the blobs.
    pub model_version: u32,
    /// Unix seconds of the session's first generation.
    pub created: u64,
    /// Newest first, at most [`GENERATIONS_KEPT`].
    pub generations: Vec<Generation>,
}

/// What the session tells the store about itself on every write (rewritten into the manifest, so a
/// recovered session's later writes update the name/flags without a separate adopt step).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionMeta {
    pub rid: String,
    pub display_name: String,
    pub untitled_number: Option<u32>,
    pub original_path: Option<PathBuf>,
    pub source_fingerprint: Option<Fingerprint>,
    pub recovered: bool,
}

/// How a scanned orphan can be offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrphanState {
    /// A generation verifies (it may be the previous one; `load_best` says so).
    Ready,
    /// Nothing verifies, or the manifest can't be read. Kept, with a plain-English reason.
    Damaged(String),
    /// Written by a newer Varos (the newer manifest or model version). Kept, not opened.
    NewerFormat(u32),
}

/// A recovery folder left by a dead session, claimed by this process during [`RecoveryStore::scan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrphanEntry {
    pub rid: String,
    pub display_name: String,
    pub original_path: Option<PathBuf>,
    /// Unix seconds of the generation Recover would use (else the newest listed one).
    pub saved_at: Option<u64>,
    pub state: OrphanState,
}

/// The result of [`RecoveryStore::load_best`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    pub blob: Vec<u8>,
    pub generation: Generation,
    /// The newest generation was damaged; the previous one was used.
    pub fell_back: bool,
}

/// Why a recovery operation failed.
#[derive(Debug)]
pub enum SnapError {
    /// The recovery folder can't be created or written ("Recovery unavailable").
    Unavailable(io::Error),
    /// The blob is over [`MAX_SNAPSHOT_BYTES`]; nothing was written.
    TooLarge {
        bytes: u64,
        max: u64,
    },
    /// Not a plain session name (empty, a separator, a dot…); refused before any I/O.
    InvalidRid,
    /// Another live session (another process, or another store) holds this folder's lock.
    Locked,
    /// This rid was retired in this process; the session must take a fresh rid.
    Retired,
    /// No such recovery session (or it has no published manifest).
    NotFound,
    /// The recovery data can't be used; it is kept.
    Damaged(String),
    /// Written by a newer Varos (manifest or model version); kept.
    NewerFormat(u32),
    /// A durable write failed; the previous manifest and its generations are intact.
    Write(WriteError),
    Io(io::Error),
}

impl SnapError {
    /// Plain-English reason for the user-facing copy.
    pub fn reason(&self) -> String {
        match self {
            SnapError::Unavailable(e) => format!("Recovery is unavailable. {}", io_reason(e)),
            SnapError::TooLarge { bytes, max } => format!(
                "The document is too large for a recovery copy ({} MB; the limit is {} MB).",
                bytes.div_ceil(1024 * 1024),
                max / (1024 * 1024)
            ),
            SnapError::InvalidRid => "The recovery copy has an invalid name.".to_string(),
            SnapError::Locked => "Another Varos window is using this recovery copy.".to_string(),
            SnapError::Retired => "This recovery copy was discarded.".to_string(),
            SnapError::NotFound => "The recovery copy no longer exists.".to_string(),
            SnapError::Damaged(r) => r.clone(),
            SnapError::NewerFormat(_) => "This recovery copy was made by a newer version of Varos.".to_string(),
            SnapError::Write(e) => e.reason(),
            SnapError::Io(e) => io_reason(e),
        }
    }
}

impl std::fmt::Display for SnapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason())
    }
}

impl std::error::Error for SnapError {}

/// A held `session.lock`. `claimed` = taken by `scan` for an orphan and not yet written to by this
/// process (still listed by later scans); `false` = one of this process's live sessions.
struct Held {
    _file: File,
    claimed: bool,
}

/// The recovery folder of one app process. `Send + Sync`: the I/O worker shares it via `Arc`.
pub struct RecoveryStore {
    fs: Arc<dyn FsPort>,
    dir: PathBuf,
    locks: Mutex<HashMap<String, Held>>,
    retired: Mutex<HashSet<String>>,
}

/// A rid is one plain name fragment: 1–64 ASCII letters, digits, `-` or `_` (so it can never be
/// `..`, contain a separator, or look like a tombstone or a hidden temp).
pub fn valid_rid(rid: &str) -> bool {
    !rid.is_empty() && rid.len() <= 64 && rid.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A fresh session id (a 128-bit nonce as 32 hex chars).
pub fn fresh_rid() -> String {
    new_nonce()
}

fn snap_file(seq: u64) -> String {
    format!("snap-{seq}.json")
}

fn lock_guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Open (creating if needed) `dir/session.lock` and try to take its exclusive lock.
/// `Ok(None)` = held by someone else.
fn try_take_lock(dir: &Path) -> io::Result<Option<File>> {
    let f =
        std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join(LOCK_FILE))?;
    match f.try_lock() {
        Ok(()) => Ok(Some(f)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

/// Parse a manifest read from disk. Untrusted: versions are checked head-first, and every listed
/// file name must be exactly `snap-<seq>.json`.
fn parse_manifest(bytes: &[u8]) -> Result<Manifest, SnapError> {
    #[derive(Deserialize)]
    struct Head {
        manifest_version: u32,
    }
    let damaged = || SnapError::Damaged("The recovery record can't be read.".to_string());
    let head: Head = serde_json::from_slice(bytes).map_err(|_| damaged())?;
    if head.manifest_version > MANIFEST_VERSION {
        return Err(SnapError::NewerFormat(head.manifest_version));
    }
    let m: Manifest = serde_json::from_slice(bytes).map_err(|_| damaged())?;
    if m.model_version > varos_core::file::VRS_VERSION {
        return Err(SnapError::NewerFormat(m.model_version));
    }
    if m.generations.is_empty() || m.generations.iter().any(|g| g.file != snap_file(g.seq)) {
        return Err(damaged());
    }
    Ok(m)
}

impl RecoveryStore {
    /// Create the recovery folder if needed and prove it is writable (a hidden probe file is created
    /// and removed). Any failure ⇒ [`SnapError::Unavailable`] ("Recovery unavailable"; editing goes on).
    pub fn open(fs: Arc<dyn FsPort>, recovery_dir: PathBuf) -> Result<Self, SnapError> {
        fs.create_dir_all(&recovery_dir).map_err(SnapError::Unavailable)?;
        match fs.metadata(&recovery_dir) {
            Ok(m) if m.is_dir => {}
            Ok(_) => return Err(SnapError::Unavailable(io::Error::other("the recovery location is not a folder"))),
            Err(e) => return Err(SnapError::Unavailable(e)),
        }
        let probe = recovery_dir.join(format!(".probe-{}", new_nonce()));
        let w = fs.create_new(&probe).map_err(SnapError::Unavailable)?;
        drop(w);
        let _ = fs.remove_file(&probe);
        Ok(RecoveryStore {
            fs,
            dir: recovery_dir,
            locks: Mutex::new(HashMap::new()),
            retired: Mutex::new(HashSet::new()),
        })
    }

    /// The `Recovery/` folder this store works in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Hold `rid`'s lock: create its folder and take the lock, or keep the one `scan` already
    /// claimed (the claim becomes a live session once a generation is published).
    fn ensure_live(&self, rid: &str, dir: &Path) -> Result<(), SnapError> {
        let mut locks = lock_guard(&self.locks);
        if locks.contains_key(rid) {
            return Ok(());
        }
        self.fs.create_dir_all(dir).map_err(SnapError::Io)?;
        match try_take_lock(dir) {
            Ok(Some(f)) => {
                locks.insert(rid.to_string(), Held { _file: f, claimed: false });
                Ok(())
            }
            Ok(None) => Err(SnapError::Locked),
            Err(e) => Err(SnapError::Io(e)),
        }
    }

    /// Read `<dir>/manifest.json`: `Ok(None)` when none was ever published.
    fn read_manifest(&self, dir: &Path) -> Result<Option<Manifest>, SnapError> {
        match self.fs.read(&dir.join(MANIFEST_FILE)) {
            Ok(bytes) => parse_manifest(&bytes).map(Some),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SnapError::Damaged(format!("The recovery record can't be read. {}", io_reason(&e)))),
        }
    }

    /// Write one generation for the session `meta.rid` (see the module docs for the order).
    ///
    /// The on-disk sequence is `max(seq, newest listed + 1)`, so a restarted counter (a recovered
    /// session) can never overwrite a listed blob; the returned [`Generation`] carries the real one.
    /// On any error the previous manifest and both of its generations are intact.
    pub fn write_generation(
        &self,
        meta: &SessionMeta,
        seq: u64,
        blob: &[u8],
        now: u64,
    ) -> Result<Generation, SnapError> {
        let rid = meta.rid.as_str();
        if !valid_rid(rid) {
            return Err(SnapError::InvalidRid);
        }
        if blob.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(SnapError::TooLarge { bytes: blob.len() as u64, max: MAX_SNAPSHOT_BYTES });
        }
        if lock_guard(&self.retired).contains(rid) {
            return Err(SnapError::Retired);
        }
        let dir = self.dir.join(rid);
        self.ensure_live(rid, &dir)?;
        // A present-but-unreadable (or newer) manifest is refused rather than overwritten: its data is kept.
        let prev = self.read_manifest(&dir)?;
        let prev_gens: &[Generation] = prev.as_ref().map(|m| m.generations.as_slice()).unwrap_or(&[]);
        let seq = match prev_gens.iter().map(|g| g.seq).max() {
            Some(newest) if seq <= newest => newest + 1,
            _ => seq,
        };

        // 1. The blob (a new, unlisted name).
        let file = snap_file(seq);
        let blob_path = dir.join(&file);
        write_replace(self.fs.as_ref(), &blob_path, blob, &new_nonce()).map_err(SnapError::Write)?;
        let generation = Generation { seq, file, bytes: blob.len() as u64, crc32: crc32(blob), saved_at: now };

        // 2. The manifest, published last: the new generation first, the previous one kept.
        let mut generations = vec![generation.clone()];
        generations.extend(prev_gens.iter().take(GENERATIONS_KEPT - 1).cloned());
        let manifest = Manifest {
            manifest_version: MANIFEST_VERSION,
            rid: rid.to_string(),
            display_name: meta.display_name.clone(),
            untitled_number: meta.untitled_number,
            original_path: meta.original_path.clone(),
            source_fingerprint: meta.source_fingerprint,
            recovered: meta.recovered,
            model_version: varos_core::file::VRS_VERSION,
            created: prev.as_ref().map_or(now, |m| m.created),
            generations,
        };
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| SnapError::Io(io::Error::other(e)))?;
        match write_replace(self.fs.as_ref(), &dir.join(MANIFEST_FILE), &bytes, &new_nonce()) {
            Err(e) => {
                // Unlisted, so harmless if the removal fails too (swept by a later write or scan).
                let _ = self.fs.remove_file(&blob_path);
                Err(SnapError::Write(e))
            }
            Ok(outcome) => {
                // A claimed orphan that now carries this session's generation is a live session.
                if let Some(h) = lock_guard(&self.locks).get_mut(rid) {
                    h.claimed = false;
                }
                // 3. Only now drop what the new manifest no longer lists — unless the new manifest
                // may not survive a power loss, in which case the old one's blobs are kept.
                if matches!(outcome, WriteOutcome::Durable) {
                    self.sweep(&dir, &manifest);
                }
                Ok(generation)
            }
        }
    }

    /// Remove every `snap-*.json` the manifest doesn't list, and hidden temps left by crashed writes
    /// (only this process writes into a folder it holds the lock for).
    fn sweep(&self, dir: &Path, manifest: &Manifest) {
        let Ok(entries) = self.fs.read_dir(dir) else { return };
        for p in entries {
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
            let unlisted_snap = name.starts_with("snap-")
                && name.ends_with(".json")
                && !manifest.generations.iter().any(|g| g.file == name);
            let temp = name.starts_with('.') && name.ends_with(".varos-tmp");
            if unlisted_snap || temp {
                let _ = self.fs.remove_file(&p);
            }
        }
    }

    /// Verify one listed generation: exact size, CRC-32 and a readable model head.
    fn verify(&self, dir: &Path, g: &Generation) -> Result<Vec<u8>, String> {
        if g.bytes > MAX_SNAPSHOT_BYTES {
            return Err("The recovery copy is larger than allowed.".to_string());
        }
        let path = dir.join(&g.file);
        match self.fs.metadata(&path) {
            Ok(m) if m.len == g.bytes && !m.is_dir => {}
            Ok(_) => return Err("The recovery copy is incomplete.".to_string()),
            Err(e) => return Err(format!("The recovery copy can't be read. {}", io_reason(&e))),
        }
        let blob = self.fs.read(&path).map_err(|e| format!("The recovery copy can't be read. {}", io_reason(&e)))?;
        if blob.len() as u64 != g.bytes || crc32(&blob) != g.crc32 {
            return Err("The recovery copy is damaged.".to_string());
        }
        #[derive(Deserialize)]
        struct Head {
            #[allow(dead_code)]
            varos: u32,
        }
        serde_json::from_slice::<Head>(&blob).map_err(|_| "The recovery copy is damaged.".to_string())?;
        Ok(blob)
    }

    /// The newest generation of `manifest` that verifies, and whether it was not the newest.
    fn best(&self, dir: &Path, manifest: &Manifest) -> Result<Loaded, SnapError> {
        let mut last_reason = String::new();
        for (i, g) in manifest.generations.iter().enumerate() {
            match self.verify(dir, g) {
                Ok(blob) => return Ok(Loaded { blob, generation: g.clone(), fell_back: i > 0 }),
                Err(r) => last_reason = r,
            }
        }
        Err(SnapError::Damaged(if manifest.generations.len() > 1 {
            "Both recovery copies are damaged.".to_string()
        } else {
            last_reason
        }))
    }

    /// The best usable generation of `rid`: the newest that verifies, else the previous one
    /// (`fell_back = true`). Nothing usable ⇒ `Damaged`/`NewerFormat` (the data is kept).
    pub fn load_best(&self, rid: &str) -> Result<Loaded, SnapError> {
        if !valid_rid(rid) {
            return Err(SnapError::InvalidRid);
        }
        let dir = self.dir.join(rid);
        let manifest = self.read_manifest(&dir)?.ok_or(SnapError::NotFound)?;
        self.best(&dir, &manifest)
    }

    /// Orphans left by dead sessions, each claimed by this process (its lock stays held until
    /// Recover writes into it, [`Self::retire`] discards it, or the store is dropped). Folders locked
    /// elsewhere (live sessions) and this process's own live sessions are skipped; orphans claimed by
    /// an earlier scan are listed again. A claimed folder that never published a manifest holds
    /// nothing recoverable and is removed. Every generation is verified (read + CRC), so `state` is
    /// what Recover will meet. Sorted newest first.
    pub fn scan(&self) -> Vec<OrphanEntry> {
        let Ok(mut entries) = self.fs.read_dir(&self.dir) else { return Vec::new() };
        entries.sort();
        let mut out = Vec::new();
        for path in entries {
            let Some(rid) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
            if !valid_rid(&rid) || !self.fs.metadata(&path).is_ok_and(|m| m.is_dir) {
                continue;
            }
            {
                let mut locks = lock_guard(&self.locks);
                match locks.get(&rid) {
                    Some(h) if !h.claimed => continue, // one of our live sessions
                    Some(_) => {}                      // claimed by an earlier scan
                    None => match try_take_lock(&path) {
                        Ok(Some(f)) => {
                            locks.insert(rid.clone(), Held { _file: f, claimed: true });
                        }
                        Ok(None) | Err(_) => continue, // a live session elsewhere (or no access)
                    },
                }
            }
            let (display_name, original_path, saved_at, state) = match self.read_manifest(&path) {
                Ok(None) => {
                    // Nothing was ever published here: release the claim and clean up.
                    lock_guard(&self.locks).remove(&rid);
                    let _ = self.remove_tree(&path);
                    continue;
                }
                Ok(Some(m)) => match self.best(&path, &m) {
                    Ok(l) => (m.display_name, m.original_path, Some(l.generation.saved_at), OrphanState::Ready),
                    Err(e) => (m.display_name, m.original_path, m.generations.first().map(|g| g.saved_at), state_of(e)),
                },
                Err(e) => ("Unknown document".to_string(), None, None, state_of(e)),
            };
            out.push(OrphanEntry { rid, display_name, original_path, saved_at, state });
        }
        out.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
        out
    }

    /// Discard `rid`: atomic rename to `<rid>.discarded-<nonce>`, then best-effort removal (a
    /// leftover tombstone is removed by [`Self::cleanup_completed`]). Afterwards `rid` is refused by
    /// `write_generation` — the session takes a fresh rid. A rid that has no folder is simply marked
    /// retired. Refused with `Locked` if another live session holds the folder.
    pub fn retire(&self, rid: &str) -> Result<(), SnapError> {
        if !valid_rid(rid) {
            return Err(SnapError::InvalidRid);
        }
        let dir = self.dir.join(rid);
        let mut locks = lock_guard(&self.locks);
        let mut held = locks.remove(rid);
        if held.is_none() {
            match self.fs.metadata(&dir) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    lock_guard(&self.retired).insert(rid.to_string());
                    return Ok(());
                }
                _ => {}
            }
            match try_take_lock(&dir) {
                Ok(Some(f)) => held = Some(Held { _file: f, claimed: false }),
                Ok(None) => return Err(SnapError::Locked),
                Err(e) => return Err(SnapError::Io(e)),
            }
        }
        // Windows cannot rename a folder while a file inside it is open: let go of the lock first.
        // Unix keeps it across the rename so no other process can claim the folder mid-retire.
        #[cfg(windows)]
        drop(held.take());
        let tomb = self.dir.join(format!("{rid}{DISCARDED_MARK}{}", new_nonce()));
        if let Err(e) = self.fs.rename(&dir, &tomb) {
            if e.kind() == io::ErrorKind::NotFound {
                lock_guard(&self.retired).insert(rid.to_string());
                return Ok(());
            }
            // Keep (Windows: take back) the lock; the session goes on using this folder.
            let held = held.or_else(|| try_take_lock(&dir).ok().flatten().map(|f| Held { _file: f, claimed: false }));
            if let Some(h) = held {
                locks.insert(rid.to_string(), h);
            }
            return Err(SnapError::Io(e));
        }
        drop(held);
        drop(locks);
        lock_guard(&self.retired).insert(rid.to_string());
        let _ = self.fs.sync_dir(&self.dir);
        let _ = self.remove_tree(&tomb);
        Ok(())
    }

    /// Remove `*.discarded-*` tombstones left by an interrupted [`Self::retire`] (best effort). Never
    /// touches a session folder, damaged or not.
    pub fn cleanup_completed(&self) {
        let Ok(entries) = self.fs.read_dir(&self.dir) else { return };
        for p in entries {
            let is_tomb = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                n.split_once(DISCARDED_MARK).is_some_and(|(rid, nonce)| valid_rid(rid) && !nonce.is_empty())
            });
            if is_tomb {
                let _ = self.remove_tree(&p);
            }
        }
    }

    /// Recursive removal through the port (session folders are flat; nested folders are handled
    /// anyway). Only ever called on a path inside the recovery folder.
    fn remove_tree(&self, path: &Path) -> io::Result<()> {
        debug_assert!(path.starts_with(&self.dir) && path != self.dir);
        for p in self.fs.read_dir(path)? {
            if self.fs.metadata(&p).is_ok_and(|m| m.is_dir) {
                self.remove_tree(&p)?;
            } else {
                self.fs.remove_file(&p)?;
            }
        }
        self.fs.remove_dir(path)
    }
}

fn state_of(e: SnapError) -> OrphanState {
    match e {
        SnapError::NewerFormat(v) => OrphanState::NewerFormat(v),
        other => OrphanState::Damaged(other.reason()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::durable::{Fault, FaultFs, Op, RealFs, Step};
    use crate::storage::testdir::TestDir;

    fn blob(n: u64) -> Vec<u8> {
        format!(r#"{{"varos":1,"doc":{{"n":{n}}}}}"#).into_bytes()
    }

    fn meta(rid: &str, name: &str) -> SessionMeta {
        SessionMeta {
            rid: rid.to_string(),
            display_name: name.to_string(),
            original_path: Some(PathBuf::from(format!("/Users/a/{name}.vrs"))),
            ..SessionMeta::default()
        }
    }

    fn real_store(d: &TestDir) -> RecoveryStore {
        RecoveryStore::open(Arc::new(RealFs), d.join("Recovery")).unwrap()
    }

    fn fault_store(d: &TestDir) -> (Arc<FaultFs>, RecoveryStore) {
        let fs = Arc::new(FaultFs::new(Vec::new()));
        let store = RecoveryStore::open(fs.clone(), d.join("Recovery")).unwrap();
        (fs, store)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    fn read_manifest(store: &RecoveryStore, rid: &str) -> Manifest {
        serde_json::from_slice(&std::fs::read(store.dir().join(rid).join(MANIFEST_FILE)).unwrap()).unwrap()
    }

    fn seqs(store: &RecoveryStore, rid: &str) -> Vec<u64> {
        read_manifest(store, rid).generations.iter().map(|g| g.seq).collect()
    }

    /// Every file in the session folder with its bytes (to prove "byte-identical" after a failure).
    fn snapshot_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
        names(dir).into_iter().map(|n| (n.clone(), std::fs::read(dir.join(&n)).unwrap())).collect()
    }

    fn flip_last_byte(path: &Path) {
        let mut b = std::fs::read(path).unwrap();
        let i = b.len() - 2;
        b[i] ^= 0x01;
        std::fs::write(path, b).unwrap();
    }

    #[test]
    fn first_generation_publishes_manifest_last() {
        let d = TestDir::new("rec-first");
        let (fs, store) = fault_store(&d);
        let rid = fresh_rid();
        // The manifest's rename fails, and so does the clean-up of the new blob.
        fs.push(Fault::at(Step::Rename).on(MANIFEST_FILE));
        fs.push(Fault::at(Step::Remove).on("snap-1.json"));
        let err = store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 100).unwrap_err();
        assert!(matches!(err, SnapError::Write(WriteError::Replace(_))), "{err:?}");
        assert_eq!(fs.pending(), 0);
        let session = store.dir().join(&rid);
        assert_eq!(names(&session), vec![LOCK_FILE.to_string(), "snap-1.json".to_string()], "no manifest published");
        // Blob renamed into place strictly before the manifest rename was attempted.
        let renames: Vec<PathBuf> = fs.log().into_iter().filter(|(op, _)| *op == Op::Rename).map(|(_, p)| p).collect();
        let blob_at = renames.iter().position(|p| p.ends_with("snap-1.json")).unwrap();
        let man_at = renames.iter().position(|p| p.ends_with(MANIFEST_FILE)).unwrap();
        assert!(blob_at < man_at, "{renames:?}");
        assert!(matches!(store.load_best(&rid), Err(SnapError::NotFound)));
        // The session dies; the next launch sees no generation and cleans the unpublished blob.
        drop(store);
        let next = real_store(&d);
        assert!(next.scan().is_empty());
        assert!(!session.exists(), "unpublished folder cleaned: {:?}", names(next.dir()));
    }

    #[test]
    fn keeps_exactly_two_generations() {
        let d = TestDir::new("rec-two");
        let store = real_store(&d);
        let rid = fresh_rid();
        for seq in 1..=5 {
            let g = store.write_generation(&meta(&rid, "Logo"), seq, &blob(seq), 100 + seq).unwrap();
            assert_eq!(g.seq, seq);
        }
        assert_eq!(seqs(&store, &rid), vec![5, 4]);
        assert_eq!(names(&store.dir().join(&rid)), vec![MANIFEST_FILE, LOCK_FILE, "snap-4.json", "snap-5.json"]);
        let l = store.load_best(&rid).unwrap();
        assert_eq!((l.blob, l.generation.seq, l.fell_back), (blob(5), 5, false));
        let m = read_manifest(&store, &rid);
        assert_eq!((m.created, m.model_version, m.manifest_version), (101, varos_core::file::VRS_VERSION, 1));
    }

    #[test]
    fn failed_generation_keeps_previous_two_intact() {
        let d = TestDir::new("rec-fail");
        let (fs, store) = fault_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102).unwrap();
        let session = store.dir().join(&rid);
        let before = snapshot_of(&session);
        let faults = [
            Fault::at(Step::Create).on("snap-3"),
            Fault::at(Step::Write { after: 5 }).on("snap-3"),
            Fault::at(Step::Sync).on("snap-3"),
            Fault::at(Step::Rename).on("snap-3"),
            Fault::at(Step::Create).on(MANIFEST_FILE),
            Fault::at(Step::Write { after: 5 }).on(MANIFEST_FILE),
            Fault::at(Step::Rename).on(MANIFEST_FILE),
        ];
        for f in faults {
            let label = format!("{f:?}");
            fs.push(f);
            assert!(store.write_generation(&meta(&rid, "Logo"), 3, &blob(3), 103).is_err(), "{label}");
            assert_eq!(fs.pending(), 0, "{label}");
            assert_eq!(snapshot_of(&session), before, "previous generations changed after {label}");
            let l = store.load_best(&rid).unwrap();
            assert_eq!((l.generation.seq, l.fell_back), (2, false), "{label}");
        }
        // And the next attempt succeeds normally.
        store.write_generation(&meta(&rid, "Logo"), 3, &blob(3), 103).unwrap();
        assert_eq!(seqs(&store, &rid), vec![3, 2]);
    }

    #[test]
    fn disk_full_is_reported_and_previous_survives() {
        let d = TestDir::new("rec-full");
        let (fs, store) = fault_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        for target in ["snap-2", MANIFEST_FILE] {
            fs.push(Fault::at(Step::Write { after: 4 }).on(target).kind(io::ErrorKind::StorageFull));
            let err = store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102).unwrap_err();
            assert_eq!(err.reason(), "The disk is full.", "{target}: {err:?}");
            let l = store.load_best(&rid).unwrap();
            assert_eq!((l.blob, l.generation.seq), (blob(1), 1), "{target}");
            let left: Vec<String> = names(&store.dir().join(&rid));
            assert_eq!(left, vec![MANIFEST_FILE, LOCK_FILE, "snap-1.json"], "{target}: nothing half-written left");
        }
    }

    #[test]
    fn corrupt_newest_falls_back_with_flag() {
        let d = TestDir::new("rec-fallback");
        let store = real_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102).unwrap();
        let newest = store.dir().join(&rid).join("snap-2.json");
        // Same size, one bit flipped: only the CRC catches it.
        flip_last_byte(&newest);
        let l = store.load_best(&rid).unwrap();
        assert_eq!((l.blob, l.generation.seq, l.fell_back), (blob(1), 1, true));
        // The hand test's damage: truncated to zero bytes (`: > snap-N.json`).
        std::fs::write(&newest, b"").unwrap();
        let l = store.load_best(&rid).unwrap();
        assert_eq!((l.generation.seq, l.fell_back), (1, true));
        // A dead session's scan reports it Ready, timed at the generation Recover will use.
        drop(store);
        let next = real_store(&d);
        let orphans = next.scan();
        assert_eq!(orphans.len(), 1);
        assert_eq!((orphans[0].state.clone(), orphans[0].saved_at), (OrphanState::Ready, Some(101)));
    }

    #[test]
    fn both_corrupt_is_damaged_and_retained() {
        let d = TestDir::new("rec-both");
        let store = real_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102).unwrap();
        let session = store.dir().join(&rid);
        flip_last_byte(&session.join("snap-1.json"));
        std::fs::write(session.join("snap-2.json"), b"").unwrap();
        let err = store.load_best(&rid).unwrap_err();
        assert!(matches!(&err, SnapError::Damaged(r) if r == "Both recovery copies are damaged."), "{err:?}");
        drop(store);
        let next = real_store(&d);
        next.cleanup_completed();
        let orphans = next.scan();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].state, OrphanState::Damaged("Both recovery copies are damaged.".into()));
        assert_eq!(orphans[0].display_name, "Logo");
        next.cleanup_completed();
        assert_eq!(next.scan().len(), 1, "still listed");
        assert_eq!(names(&session), vec![MANIFEST_FILE, LOCK_FILE, "snap-1.json", "snap-2.json"], "retained");
    }

    #[test]
    fn unreadable_manifest_is_damaged_and_retained() {
        let d = TestDir::new("rec-badman");
        let store = real_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        let session = store.dir().join(&rid);
        std::fs::write(session.join(MANIFEST_FILE), b"{ not json").unwrap();
        assert!(matches!(store.load_best(&rid), Err(SnapError::Damaged(_))));
        // A live session refuses to overwrite it (its data is kept).
        assert!(matches!(store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102), Err(SnapError::Damaged(_))));
        drop(store);
        let next = real_store(&d);
        next.cleanup_completed();
        let orphans = next.scan();
        assert_eq!(orphans.len(), 1);
        assert!(matches!(&orphans[0].state, OrphanState::Damaged(r) if r == "The recovery record can't be read."));
        assert_eq!((orphans[0].display_name.as_str(), orphans[0].saved_at), ("Unknown document", None));
        next.cleanup_completed();
        assert_eq!(names(&session), vec![MANIFEST_FILE, LOCK_FILE, "snap-1.json"], "blob kept, no blob-scan guessing");
    }

    #[test]
    fn newer_manifest_version_is_retained_not_deleted() {
        let d = TestDir::new("rec-newer");
        let store = real_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        let session = store.dir().join(&rid);
        // A newer Varos wrote this (different shape entirely).
        std::fs::write(session.join(MANIFEST_FILE), br#"{"manifest_version":7,"something":"new"}"#).unwrap();
        assert!(matches!(store.load_best(&rid), Err(SnapError::NewerFormat(7))));
        drop(store);
        let next = real_store(&d);
        next.cleanup_completed();
        let orphans = next.scan();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].state, OrphanState::NewerFormat(7));
        next.cleanup_completed();
        assert_eq!(names(&session), vec![MANIFEST_FILE, LOCK_FILE, "snap-1.json"]);
        // A newer model version in a v1 manifest is also held back, not opened.
        let mut m = Manifest {
            manifest_version: 1,
            rid: rid.clone(),
            display_name: "Logo".into(),
            untitled_number: None,
            original_path: None,
            source_fingerprint: None,
            recovered: false,
            model_version: 1,
            created: 1,
            generations: vec![],
        };
        m.model_version = varos_core::file::VRS_VERSION + 1;
        m.generations = vec![Generation { seq: 1, file: "snap-1.json".into(), bytes: 0, crc32: 0, saved_at: 1 }];
        std::fs::write(session.join(MANIFEST_FILE), serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(
            matches!(next.load_best(&rid), Err(SnapError::NewerFormat(v)) if v == varos_core::file::VRS_VERSION + 1)
        );
    }

    #[test]
    fn live_session_is_not_an_orphan() {
        let d = TestDir::new("rec-live");
        let a = real_store(&d);
        let rid = fresh_rid();
        a.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        // A second store in the same process stands in for a second process: flock/LockFileEx
        // conflict between two opens.
        let b = real_store(&d);
        assert!(b.scan().is_empty(), "a live session elsewhere is skipped");
        assert!(a.scan().is_empty(), "our own live session is not an orphan");
        assert!(matches!(b.retire(&rid), Err(SnapError::Locked)));
        assert!(matches!(b.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102), Err(SnapError::Locked)));
        assert_eq!(seqs(&a, &rid), vec![1]);
    }

    #[test]
    fn dead_session_becomes_orphan() {
        let d = TestDir::new("rec-dead");
        let a = real_store(&d);
        let rid = fresh_rid();
        let mut m = meta(&rid, "Logo");
        m.untitled_number = Some(3);
        a.write_generation(&m, 1, &blob(1), 101).unwrap();
        a.write_generation(&m, 2, &blob(2), 102).unwrap();
        drop(a); // the process died: the OS released its lock
        let b = real_store(&d);
        let orphans = b.scan();
        assert_eq!(
            orphans,
            vec![OrphanEntry {
                rid: rid.clone(),
                display_name: "Logo".into(),
                original_path: Some(PathBuf::from("/Users/a/Logo.vrs")),
                saved_at: Some(102),
                state: OrphanState::Ready,
            }]
        );
        let l = b.load_best(&rid).unwrap();
        assert_eq!((l.blob, l.fell_back), (blob(2), false));
        assert_eq!(read_manifest(&b, &rid).untitled_number, Some(3));
    }

    #[test]
    fn scan_claims_orphan_exclusively() {
        let d = TestDir::new("rec-claim");
        let a = real_store(&d);
        let (r1, r2) = (fresh_rid(), fresh_rid());
        a.write_generation(&meta(&r1, "One"), 1, &blob(1), 101).unwrap();
        a.write_generation(&meta(&r2, "Two"), 1, &blob(2), 202).unwrap();
        drop(a);
        let b = real_store(&d);
        let found: Vec<String> = b.scan().into_iter().map(|o| o.display_name).collect();
        assert_eq!(found, vec!["Two", "One"], "newest first");
        let c = real_store(&d);
        assert!(c.scan().is_empty(), "b's claims hold");
        assert_eq!(b.scan().len(), 2, "b still lists its claims");
        b.retire(&r1).unwrap(); // Discard one
        drop(b); // b exits; its remaining claim is released
        let found: Vec<String> = c.scan().into_iter().map(|o| o.display_name).collect();
        assert_eq!(found, vec!["Two"]);
    }

    #[test]
    fn recovered_session_keeps_writing_same_rid() {
        let d = TestDir::new("rec-adopt");
        let a = real_store(&d);
        let rid = fresh_rid();
        a.write_generation(&meta(&rid, "Logo"), 6, &blob(6), 106).unwrap();
        a.write_generation(&meta(&rid, "Logo"), 7, &blob(7), 107).unwrap();
        drop(a);
        let b = real_store(&d);
        assert_eq!(b.scan().len(), 1);
        assert_eq!(b.load_best(&rid).unwrap().blob, blob(7));
        // The recovered session's scheduler restarts at seq 1; the store never overwrites a listed blob.
        let mut m = meta(&rid, "Logo (Recovered)");
        m.recovered = true;
        m.original_path = None;
        let g = b.write_generation(&m, 1, &blob(100), 200).unwrap();
        assert_eq!(g.seq, 8);
        assert_eq!(seqs(&b, &rid), vec![8, 7], "the source stays as generation N-1");
        let man = read_manifest(&b, &rid);
        assert_eq!(
            (man.display_name.as_str(), man.recovered, man.original_path, man.created),
            ("Logo (Recovered)", true, None, 106)
        );
        assert!(b.scan().is_empty(), "now a live session of this process, not an orphan");
        let c = real_store(&d);
        assert!(c.scan().is_empty(), "and still locked against other processes");
    }

    #[test]
    fn retire_renames_to_discarded_then_cleanup_removes_dir() {
        let d = TestDir::new("rec-retire");
        let (fs, store) = fault_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102).unwrap();
        store.retire(&rid).unwrap();
        let renamed: Vec<PathBuf> = fs.log().into_iter().filter(|(op, _)| *op == Op::Rename).map(|(_, p)| p).collect();
        let n = renamed.len();
        assert_eq!(renamed[n - 2], store.dir().join(&rid));
        let tomb = renamed[n - 1].file_name().unwrap().to_string_lossy().into_owned();
        assert!(tomb.starts_with(&format!("{rid}{DISCARDED_MARK}")), "{tomb}");
        assert!(names(store.dir()).is_empty(), "fully removed: {:?}", names(store.dir()));
        // An interrupted retire leaves only the tombstone; cleanup removes it.
        let rid2 = fresh_rid();
        store.write_generation(&meta(&rid2, "Other"), 1, &blob(1), 101).unwrap();
        fs.push(Fault::at(Step::Remove).on(MANIFEST_FILE));
        store.retire(&rid2).unwrap();
        let left = names(store.dir());
        assert_eq!(left.len(), 1);
        assert!(left[0].starts_with(&format!("{rid2}{DISCARDED_MARK}")), "{left:?}");
        assert!(store.scan().is_empty(), "a tombstone is never offered");
        store.cleanup_completed();
        assert!(names(store.dir()).is_empty());
        // Retiring something that never wrote is fine.
        store.retire(&fresh_rid()).unwrap();
    }

    #[test]
    fn failed_retire_rename_keeps_the_session_and_its_lock() {
        let d = TestDir::new("rec-retire-fail");
        let (fs, store) = fault_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        fs.push(Fault::at(Step::Rename).on(DISCARDED_MARK));
        assert!(matches!(store.retire(&rid), Err(SnapError::Io(_))));
        assert!(real_store(&d).scan().is_empty(), "still locked by this session");
        store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102).unwrap();
        assert_eq!(seqs(&store, &rid), vec![2, 1]);
    }

    #[test]
    fn session_takes_a_fresh_rid_after_retire() {
        let d = TestDir::new("rec-fresh");
        let store = real_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        store.retire(&rid).unwrap();
        assert!(matches!(store.write_generation(&meta(&rid, "Logo"), 2, &blob(2), 102), Err(SnapError::Retired)));
        assert!(!store.dir().join(&rid).exists());
        let fresh = fresh_rid();
        assert_ne!(fresh, rid);
        store.write_generation(&meta(&fresh, "Logo"), 2, &blob(2), 102).unwrap();
        assert_eq!(store.load_best(&fresh).unwrap().blob, blob(2));
    }

    #[test]
    fn snapshot_after_partial_retire_survives_cleanup() {
        let d = TestDir::new("rec-partial");
        let (fs, store) = fault_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        // The rename lands; removing the tombstone's contents fails.
        fs.push(Fault::at(Step::Remove).on("snap-1.json"));
        store.retire(&rid).unwrap();
        assert_eq!(fs.pending(), 0);
        // The user keeps editing: the session writes into a fresh rid.
        let rid2 = fresh_rid();
        store.write_generation(&meta(&rid2, "Logo"), 2, &blob(2), 102).unwrap();
        store.cleanup_completed();
        assert_eq!(names(store.dir()), vec![rid2.clone()], "tombstone gone, new session kept");
        assert_eq!(store.load_best(&rid2).unwrap().blob, blob(2));
        // Next launch (after a crash) still offers it.
        drop(store);
        let next = real_store(&d);
        next.cleanup_completed();
        let orphans = next.scan();
        assert_eq!(orphans.len(), 1);
        assert_eq!((orphans[0].rid.as_str(), &orphans[0].state), (rid2.as_str(), &OrphanState::Ready));
    }

    #[test]
    fn never_writes_outside_recovery_dir() {
        let d = TestDir::new("rec-contained");
        // The user's document sits next to the recovery folder.
        let original = d.join("Original.vrs");
        std::fs::write(&original, b"the user's real file").unwrap();
        let before = std::fs::metadata(&original).unwrap().modified().unwrap();
        let (fs, store) = fault_store(&d);
        let recovery = store.dir().to_path_buf();
        let mut m = meta("placeholder", "Original");
        m.original_path = Some(original.clone());
        let (r1, r2) = (fresh_rid(), fresh_rid());
        for (i, rid) in [&r1, &r2].into_iter().enumerate() {
            m.rid = rid.clone();
            for seq in 1..=3 {
                store.write_generation(&m, seq, &blob(seq + i as u64), 100 + seq).unwrap();
            }
        }
        store.load_best(&r1).unwrap();
        store.retire(&r1).unwrap();
        drop(store);
        let again = RecoveryStore::open(fs.clone(), recovery.clone()).unwrap();
        again.cleanup_completed();
        assert_eq!(again.scan().len(), 1);
        again.load_best(&r2).unwrap();
        m.rid = r2.clone();
        again.write_generation(&m, 1, &blob(9), 200).unwrap();
        again.retire(&r2).unwrap();
        // Hostile names never reach the disk.
        assert!(matches!(again.retire("../Original.vrs"), Err(SnapError::InvalidRid)));
        m.rid = "..".into();
        assert!(matches!(again.write_generation(&m, 1, &blob(1), 1), Err(SnapError::InvalidRid)));
        assert!(matches!(again.load_best("a/b"), Err(SnapError::InvalidRid)));

        let log = fs.log();
        assert!(log.len() > 50, "the log recorded the work: {}", log.len());
        for (op, p) in &log {
            assert!(p.starts_with(&recovery), "{op:?} touched {} outside {}", p.display(), recovery.display());
        }
        // The lock files (opened with std, not the port) live inside too: the parent folder holds
        // exactly what it held before, and the original is untouched.
        assert_eq!(names(d.path()), vec!["Original.vrs", "Recovery"]);
        assert_eq!(std::fs::read(&original).unwrap(), b"the user's real file");
        assert_eq!(std::fs::metadata(&original).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn hostile_manifest_file_names_are_damage_not_paths() {
        let d = TestDir::new("rec-hostile");
        let store = real_store(&d);
        let rid = fresh_rid();
        store.write_generation(&meta(&rid, "Logo"), 1, &blob(1), 101).unwrap();
        let path = store.dir().join(&rid).join(MANIFEST_FILE);
        let mut m = read_manifest(&store, &rid);
        m.generations[0].file = "../../Original.vrs".into();
        std::fs::write(&path, serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(matches!(store.load_best(&rid), Err(SnapError::Damaged(_))));
    }

    #[test]
    fn too_large_snapshot_is_refused() {
        let d = TestDir::new("rec-large");
        let (fs, store) = fault_store(&d);
        let rid = fresh_rid();
        let ops_before = fs.log().len();
        let big = vec![b' '; MAX_SNAPSHOT_BYTES as usize + 1];
        let err = store.write_generation(&meta(&rid, "Huge"), 1, &big, 101).unwrap_err();
        assert!(
            matches!(err, SnapError::TooLarge { bytes, max } if bytes == MAX_SNAPSHOT_BYTES + 1 && max == MAX_SNAPSHOT_BYTES)
        );
        assert_eq!(err.reason(), "The document is too large for a recovery copy (33 MB; the limit is 32 MB).");
        assert_eq!(fs.log().len(), ops_before, "refused before any I/O");
        assert!(names(store.dir()).is_empty());
        // Exactly at the cap is accepted (as bytes; a real blob this size is valid JSON too).
        let mut at_cap = blob(1);
        at_cap.resize(MAX_SNAPSHOT_BYTES as usize, b' ');
        store.write_generation(&meta(&rid, "Huge"), 1, &at_cap, 101).unwrap();
        assert_eq!(store.load_best(&rid).unwrap().blob.len() as u64, MAX_SNAPSHOT_BYTES);
    }

    #[test]
    fn real_document_blob_round_trips_through_the_open_decoder() {
        let d = TestDir::new("rec-doc");
        let store = real_store(&d);
        let rid = fresh_rid();
        let doc = varos_core::model::Document::default();
        let body = varos_core::file::doc_to_blob(&doc).unwrap();
        store.write_generation(&meta(&rid, "Logo"), 1, body.as_bytes(), 101).unwrap();
        let l = store.load_best(&rid).unwrap();
        assert_eq!(l.blob, body.as_bytes());
        varos_core::file::doc_from_blob(std::str::from_utf8(&l.blob).unwrap()).unwrap();
    }

    #[test]
    fn open_reports_unavailable_storage() {
        let d = TestDir::new("rec-unavail");
        let fs = Arc::new(FaultFs::new(vec![Fault::at(Step::CreateDir).kind(io::ErrorKind::PermissionDenied)]));
        let err = RecoveryStore::open(fs, d.join("Recovery")).err().unwrap();
        assert_eq!(err.reason(), "Recovery is unavailable. Varos isn't allowed to write there.");
        let fs = Arc::new(FaultFs::new(vec![Fault::at(Step::Create).kind(io::ErrorKind::ReadOnlyFilesystem)]));
        assert!(matches!(RecoveryStore::open(fs, d.join("Recovery")), Err(SnapError::Unavailable(_))));
        assert_eq!(names(&d.join("Recovery")), Vec::<String>::new(), "the probe leaves nothing behind");
        // A file where the folder should be.
        std::fs::write(d.join("NotADir"), b"x").unwrap();
        assert!(matches!(RecoveryStore::open(Arc::new(RealFs), d.join("NotADir")), Err(SnapError::Unavailable(_))));
    }

    #[test]
    fn store_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<RecoveryStore>();
    }
}
