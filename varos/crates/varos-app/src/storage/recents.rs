//! Recent-files list (work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3.4).
//!
//! Atomic JSON with a version field (`{"version":1,"items":[…]}`), newest first, capped at
//! [`MAX_RECENTS`], deduplicated by file identity.
//!
//! There is no `FileEvent` enum: [`Recents::record`] is called only at the Opened/Saved sites, so a
//! failed open, an export or a snapshot can never reach this list by construction (review finding
//! P2-7). Identity reuses S1's `FileKey::same_file` rule — equal `dev_ino` when both sides know it,
//! else exact path equality — instead of a second, lexical case-folding key, which would be wrong on
//! case-sensitive APFS and would ignore NFD names (review finding P2-7 / amendment finding 7). This
//! module does not derive that identity itself: the caller passes the `(path, dev_ino)` pair it
//! already has from S1's `DocStore::key`.
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::checksum::new_nonce;
use super::durable::{self, FsPort, WriteError, WriteOutcome};

/// Newest entries kept; older ones fall off the end.
pub const MAX_RECENTS: usize = 20;

const RECENTS_VERSION: u32 = 1;

/// One remembered document. `file_id` is the file's device/inode (unix; `None` on platforms or
/// filesystems that do not expose one, or before the caller has ever opened/stat'd the file).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    pub name: String,
    /// Unix seconds.
    pub last_opened: u64,
    #[serde(default)]
    pub file_id: Option<(u64, u64)>,
}

/// The Recent-documents list, newest first. Construction only ever grows the list through
/// [`Recents::record`] (Opened/Saved) or [`Recents::relocate`] (Locate, on an entry that already
/// exists) — never a bare push, so "recents is never polluted" is a property of the API, not of
/// caller discipline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recents {
    entries: Vec<RecentEntry>,
}

/// Same rule as `workspace::FileKey::same_file`: path-equal, OR both sides know a `dev_ino` and
/// those match. Path is checked first because a save replaces the inode (durable replace = new
/// temp file renamed over the target), so a key taken before a save and one taken after it still
/// match by path; the inode alone catches aliases (symlinks, case variants) of a live file.
fn same_file(a_path: &Path, a_id: Option<(u64, u64)>, b_path: &Path, b_id: Option<(u64, u64)>) -> bool {
    a_path == b_path || matches!((a_id, b_id), (Some(x), Some(y)) if x == y)
}

fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string_lossy().into_owned())
}

impl Recents {
    /// Every entry, newest first.
    pub fn entries(&self) -> &[RecentEntry] {
        &self.entries
    }

    /// Record a successful Open or Save: the file moves to (or is inserted at) the front. Only
    /// caller of this method: the Opened/Saved outcome sites (no other event reaches Recent).
    /// Always reports a change, since even a re-open of the top entry bumps `last_opened`.
    pub fn record(&mut self, path: &Path, file_id: Option<(u64, u64)>, now: u64) -> bool {
        if let Some(i) = self.entries.iter().position(|e| same_file(&e.path, e.file_id, path, file_id)) {
            self.entries.remove(i);
        }
        self.entries
            .insert(0, RecentEntry { path: path.to_path_buf(), name: display_name(path), last_opened: now, file_id });
        self.entries.truncate(MAX_RECENTS);
        true
    }

    /// Remove one entry (context menu "Remove from Recent"). Never touches the file on disk.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.path != path);
        self.entries.len() != before
    }

    /// Empty the list ("Clear Recent"). Never touches any file on disk.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// After Locate's Open picker has already succeeded: update the entry matching `old` in place
    /// (same position in the list — Locate fixes a broken entry, it does not re-sort the list) with
    /// the new path/identity and a bumped `last_opened`. `false` when `old` is not in the list.
    pub fn relocate(&mut self, old: &Path, new_path: &Path, new_key: Option<(u64, u64)>, now: u64) -> bool {
        match self.entries.iter_mut().find(|e| e.path == old) {
            Some(e) => {
                e.path = new_path.to_path_buf();
                e.name = display_name(new_path);
                e.file_id = new_key;
                e.last_opened = now;
                true
            }
            None => false,
        }
    }

    /// Save atomically as `{"version":1,"items":[…]}`.
    pub fn save(&self, fs: &dyn FsPort, path: &Path) -> Result<WriteOutcome, WriteError> {
        let doc = OnDisk { version: RECENTS_VERSION, items: self.entries.clone() };
        let bytes = serde_json::to_vec_pretty(&doc).expect("Recents always serializes");
        durable::write_replace(fs, path, &bytes, &new_nonce())
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    items: Vec<RecentEntry>,
}

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

fn bad_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".bad");
    PathBuf::from(s)
}

/// Missing file → empty list, no warning. Corrupt JSON → empty list + warning, and the bad bytes
/// are moved aside to `<name>.bad` (never silently discarded on first read). A version this build
/// does not recognise → empty list + warning, but the file on disk is left **completely
/// untouched** — `load` never writes, so a future Varos build's data is never silently overwritten.
pub fn load(fs: &dyn FsPort, path: &Path) -> (Recents, Option<String>) {
    let bytes = match fs.read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (Recents::default(), None),
        Err(e) => {
            return (
                Recents::default(),
                Some(format!("Couldn't read the recent documents list: {}", durable::io_reason(&e))),
            )
        }
    };
    let probe: VersionProbe = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => return corrupt(fs, path, &bytes),
    };
    if probe.version != RECENTS_VERSION {
        return (
            Recents::default(),
            Some(format!(
                "The recent documents list was saved by a newer version of Varos (format {}); it was left unchanged.",
                probe.version
            )),
        );
    }
    match serde_json::from_slice::<OnDisk>(&bytes) {
        Ok(doc) => (Recents { entries: doc.items }, None),
        Err(_) => corrupt(fs, path, &bytes),
    }
}

fn corrupt(fs: &dyn FsPort, path: &Path, bytes: &[u8]) -> (Recents, Option<String>) {
    let _ = bytes;
    let bad = bad_path(path);
    let _ = fs.remove_file(&bad);
    let _ = fs.rename(path, &bad);
    (Recents::default(), Some("The recent documents list was damaged and has been reset.".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::durable::{Fault, FaultFs, RealFs, Step};
    use crate::storage::testdir::TestDir;

    fn id(n: u64) -> Option<(u64, u64)> {
        Some((1, n))
    }

    #[test]
    fn opened_and_saved_record_newest_first() {
        let mut r = Recents::default();
        assert!(r.record(Path::new("/docs/a.vrs"), id(1), 100)); // "Opened"
        assert!(r.record(Path::new("/docs/b.vrs"), id(2), 200)); // "Saved"
        let names: Vec<&str> = r.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["b.vrs", "a.vrs"]);
        assert_eq!(r.entries()[0].last_opened, 200);
    }

    #[test]
    fn record_is_the_only_call_recents_never_polluted_by_construction() {
        // `relocate` requires an existing entry — it cannot seed the list on its own, so `record`
        // (called only at the Opened/Saved sites) is the sole way an entry is ever added.
        let mut r = Recents::default();
        assert!(!r.relocate(Path::new("/x"), Path::new("/y"), None, 1));
        assert!(r.entries().is_empty());
        r.record(Path::new("/docs/a.vrs"), None, 1);
        assert_eq!(r.entries().len(), 1);
    }

    #[test]
    fn dedupe_by_dev_ino_else_exact_path() {
        // Same path, identity unknown this time: still the one entry (path match).
        let mut r = Recents::default();
        r.record(Path::new("/docs/a.vrs"), id(1), 100);
        r.record(Path::new("/docs/a.vrs"), None, 200);
        assert_eq!(r.entries().len(), 1);
        assert_eq!(r.entries()[0].last_opened, 200);
        assert_eq!(r.entries()[0].file_id, None);

        // Different path, same inode (a symlink / case alias of the same file on disk): one entry,
        // and the newer path wins — this is `FileKey::same_file`, not a lexical/case-fold key.
        let mut r = Recents::default();
        r.record(Path::new("/docs/a.vrs"), id(1), 100);
        r.record(Path::new("/link/A.VRS"), id(1), 200);
        assert_eq!(r.entries().len(), 1);
        assert_eq!(r.entries()[0].path, Path::new("/link/A.VRS"));

        // Two genuinely different files never merge just because their names case-fold equal.
        let mut r = Recents::default();
        r.record(Path::new("/docs/a.vrs"), id(1), 100);
        r.record(Path::new("/docs/A.VRS"), id(2), 200);
        assert_eq!(r.entries().len(), 2);
    }

    #[test]
    fn cap_is_twenty() {
        let mut r = Recents::default();
        for i in 0..25u64 {
            r.record(&PathBuf::from(format!("/docs/{i}.vrs")), id(i), i);
        }
        assert_eq!(r.entries().len(), MAX_RECENTS);
        assert_eq!(r.entries().first().unwrap().last_opened, 24, "newest kept");
        assert_eq!(r.entries().last().unwrap().last_opened, 5, "oldest 5 fell off");
    }

    #[test]
    fn remove_and_clear_touch_only_the_list() {
        let d = TestDir::new("recents-remove");
        let doc = d.join("a.vrs");
        std::fs::write(&doc, b"doc bytes").unwrap();

        let mut r = Recents::default();
        r.record(&doc, None, 1);
        assert!(r.remove(&doc));
        assert!(r.entries().is_empty());
        assert!(!r.remove(&doc), "already gone ⇒ no change");
        assert!(doc.exists(), "remove never touches the file on disk");

        r.record(&doc, None, 2);
        r.clear();
        assert!(r.entries().is_empty());
        assert!(doc.exists(), "clear never touches the file on disk");
    }

    #[test]
    fn relocate_replaces_entry_in_place_and_bumps_time() {
        let mut r = Recents::default();
        r.record(Path::new("/docs/a.vrs"), id(1), 100); // ends at index 1 below
        r.record(Path::new("/docs/b.vrs"), id(2), 200); // index 0

        assert!(r.relocate(Path::new("/docs/a.vrs"), Path::new("/moved/a.vrs"), id(9), 300));
        assert_eq!(r.entries()[1].path, Path::new("/moved/a.vrs"), "position unchanged: in place");
        assert_eq!(r.entries()[1].name, "a.vrs");
        assert_eq!(r.entries()[1].file_id, id(9));
        assert_eq!(r.entries()[1].last_opened, 300, "time bumped");
        assert_eq!(r.entries()[0].path, Path::new("/docs/b.vrs"), "the other entry is untouched");

        assert!(!r.relocate(Path::new("/never/there.vrs"), Path::new("/x"), None, 1));
    }

    #[test]
    fn corrupt_file_loads_empty_with_warning_and_is_kept_as_bad() {
        let d = TestDir::new("recents-corrupt");
        let path = d.join("recent.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let (r, warning) = load(&RealFs, &path);
        assert!(r.entries().is_empty());
        assert!(warning.is_some());
        assert!(!path.exists(), "the corrupt file is moved aside, not left in place");
        assert_eq!(std::fs::read(d.join("recent.json.bad")).unwrap(), b"{ not json");
    }

    #[test]
    fn unknown_version_is_not_overwritten_silently() {
        let d = TestDir::new("recents-newver");
        let path = d.join("recent.json");
        let original: &[u8] = br#"{"version":99,"items":[],"future_field":true}"#;
        std::fs::write(&path, original).unwrap();
        let (r, warning) = load(&RealFs, &path);
        assert!(r.entries().is_empty());
        assert!(warning.is_some());
        // `load` never writes on this path: byte-identical, and no `.bad` sibling appears.
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!d.join("recent.json.bad").exists());
    }

    #[test]
    fn save_is_atomic_under_fault() {
        let d = TestDir::new("recents-save-fault");
        let path = d.join("recent.json");
        let mut r = Recents::default();
        r.record(Path::new("/docs/a.vrs"), id(1), 100);
        r.save(&RealFs, &path).unwrap();
        let before = std::fs::read(&path).unwrap();

        r.record(Path::new("/docs/b.vrs"), id(2), 200);
        let fs = FaultFs::new(vec![Fault::at(Step::Rename)]);
        assert!(r.save(&fs, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before, "the old recent.json is untouched");

        // A clean save round-trips.
        r.save(&RealFs, &path).unwrap();
        let (loaded, warning) = load(&RealFs, &path);
        assert!(warning.is_none());
        assert_eq!(loaded, r);
    }
}
