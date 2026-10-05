//! Recent-files list (work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3.4).
//!
//! Atomic JSON with a version field (`{"version":2,"items":[…]}`), newest first, capped at
//! [`MAX_RECENTS`], deduplicated by file identity.
//!
//! Store version 2 (Start v2 lane L2, 2026-10-04) caches each board's summary — name, description,
//! tags, artboard count — plus the file's modified time and a thumbnail-key slot (filled by the
//! thumbnail lane), written only at the successful Opened/Saved sites, so Home draws its cards without
//! ever parsing a `.vrs`. A version-1 list upgrades in memory on load (no summary cached yet: `board`
//! is `None` until the file is next opened or saved) and is written as version 2 on the next change.
//! Any other version is left untouched on disk, as before.
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
use varos_core::board;
use varos_core::model::Document;

use super::checksum::new_nonce;
use super::durable::{self, FsPort, WriteError, WriteOutcome};

/// Newest entries kept; older ones fall off the end.
pub const MAX_RECENTS: usize = 20;

const RECENTS_VERSION: u32 = 2;
/// The previous store format: same items without the board cache; upgraded in memory on load.
const RECENTS_V1: u32 = 1;

/// What Home shows about a board, cached from the document at a successful open or save.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardSummary {
    /// The board's own name; empty = use the file stem (`varos_core::board::display_name`).
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub artboards: u32,
}
impl BoardSummary {
    /// The summary of `doc` as it is on disk (the caller passes the loaded / just-written document).
    pub fn of(doc: &Document) -> Self {
        BoardSummary {
            name: doc.name.clone(),
            description: doc.description.clone(),
            tags: doc.tags.clone(),
            artboards: u32::try_from(doc.artboards.len()).unwrap_or(u32::MAX),
        }
    }
    /// The same bounds the format enforces; a hand-edited list that breaks them loses its cache entry
    /// (it is rebuilt at the next open/save) instead of drawing an unbounded card.
    fn is_valid(&self) -> bool {
        board::check_name(&self.name).is_ok()
            && board::check_description(&self.description).is_ok()
            && board::check_tags(&self.tags).is_ok()
    }
}

/// One remembered document. `file_id` is the file's device/inode (unix; `None` on platforms or
/// filesystems that do not expose one, or before the caller has ever opened/stat'd the file).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    /// The display name (`varos_core::board::display_name`: the cached board name, else the file stem).
    pub name: String,
    /// Unix seconds.
    pub last_opened: u64,
    #[serde(default)]
    pub file_id: Option<(u64, u64)>,
    /// The cached board summary; `None` = not known yet (an entry upgraded from store version 1).
    #[serde(default)]
    pub board: Option<BoardSummary>,
    /// The file's modified time (unix seconds) at the last successful open/save; for an upgraded
    /// version-1 entry, its `last_opened`.
    #[serde(default)]
    pub modified: u64,
    /// The thumbnail key slot (thumbnail lane L3). Cleared whenever the cached content changes.
    #[serde(default)]
    pub thumb: Option<String>,
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

fn display_name(path: &Path, board: Option<&BoardSummary>) -> String {
    let name = board::display_name(board.map_or("", |b| b.name.as_str()), Some(path), None);
    if name == "Untitled" {
        path.to_string_lossy().into_owned() // a path with no file name at all
    } else {
        name
    }
}

impl Recents {
    /// Every entry, newest first.
    pub fn entries(&self) -> &[RecentEntry] {
        &self.entries
    }

    /// Record a successful Open or Save: the file moves to (or is inserted at) the front. Only
    /// caller of this method: the Opened/Saved outcome sites (no other event reaches Recent).
    /// Always reports a change, since even a re-open of the top entry bumps `last_opened`.
    ///
    /// Without a summary (a re-open of an already-open tab, which may hold unsaved edits) the entry
    /// keeps whatever board summary, modified time and thumbnail it had cached.
    pub fn record(&mut self, path: &Path, file_id: Option<(u64, u64)>, now: u64) -> bool {
        let old = self
            .entries
            .iter()
            .position(|e| same_file(&e.path, e.file_id, path, file_id))
            .map(|i| self.entries.remove(i));
        let (board, modified, thumb) = old.map_or((None, now, None), |e| (e.board, e.modified, e.thumb));
        self.entries.insert(
            0,
            RecentEntry {
                path: path.to_path_buf(),
                name: display_name(path, board.as_ref()),
                last_opened: now,
                file_id,
                board,
                modified,
                thumb,
            },
        );
        self.entries.truncate(MAX_RECENTS);
        true
    }

    /// [`Recents::record`] with the board summary of the document just opened or saved and the
    /// file's modified time (unix seconds). The thumbnail slot is cleared when either changed.
    pub fn record_board(
        &mut self,
        path: &Path,
        file_id: Option<(u64, u64)>,
        now: u64,
        board: BoardSummary,
        modified: u64,
    ) -> bool {
        self.record(path, file_id, now);
        self.set_board(path, board, modified);
        true
    }

    /// Update the cached summary of the entry at `path` (after Locate's relocate, or a save).
    /// `false` when `path` is not in the list.
    pub fn set_board(&mut self, path: &Path, board: BoardSummary, modified: u64) -> bool {
        let Some(e) = self.entries.iter_mut().find(|e| e.path == path) else {
            return false;
        };
        if e.board.as_ref() != Some(&board) || e.modified != modified {
            e.thumb = None;
        }
        e.name = display_name(path, Some(&board));
        e.board = Some(board);
        e.modified = modified;
        true
    }

    /// The thumbnail lane's hook: store (or clear) the thumbnail key of the entry at `path`.
    pub fn set_thumb(&mut self, path: &Path, thumb: Option<String>) -> bool {
        match self.entries.iter_mut().find(|e| e.path == path) {
            Some(e) if e.thumb != thumb => {
                e.thumb = thumb;
                true
            }
            _ => false,
        }
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
                e.name = display_name(new_path, e.board.as_ref());
                e.file_id = new_key;
                e.last_opened = now;
                true
            }
            None => false,
        }
    }

    /// Save atomically as `{"version":2,"items":[…]}`.
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
/// untouched** — `load` never writes, so a future (or, before any migration exists, an older)
/// Varos build's data is never silently overwritten.
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
        Err(_) => return corrupt(fs, path),
    };
    if probe.version != RECENTS_VERSION && probe.version != RECENTS_V1 {
        return (Recents::default(), Some(version_mismatch_warning(probe.version)));
    }
    match serde_json::from_slice::<OnDisk>(&bytes) {
        Ok(doc) => (Recents { entries: doc.items.into_iter().map(|e| upgrade(e, probe.version)).collect() }, None),
        Err(_) => corrupt(fs, path),
    }
}

/// Bring a loaded item to the current shape: a version-1 item has no board cache (and its `name` was
/// the file name WITH extension); any item recomputes its display name and drops an invalid cache.
fn upgrade(mut e: RecentEntry, version: u32) -> RecentEntry {
    if version == RECENTS_V1 {
        e.board = None;
        e.modified = e.last_opened;
        e.thumb = None;
    }
    if e.board.as_ref().is_some_and(|b| !b.is_valid()) {
        e.board = None;
        e.thumb = None;
    }
    e.name = display_name(&e.path, e.board.as_ref());
    e
}

/// `probe.version` is only ever compared for equality against the known versions by the caller, so
/// this wording has to say which direction the mismatch actually goes (code review P2: a bare `!=`
/// mislabels an older file as "newer" the moment the version is ever bumped). Only version 1 is
/// upgraded; any other unknown version is left unchanged, and this makes the warning read correctly.
fn version_mismatch_warning(found: u32) -> String {
    let word = if found > RECENTS_VERSION { "a newer" } else { "an older" };
    format!("The recent documents list was saved by {word} version of Varos (format {found}); it was left unchanged.")
}

/// Move a corrupt `recent.json` aside to `<name>.bad` so the bytes are never silently discarded —
/// but only when no earlier corruption is already parked there: a second crash/bad-write must not
/// erase the forensic evidence of the first (code review P3). The second corrupt file is then left
/// at `path`; the next `load()` reports it as missing-or-corrupt again rather than losing it.
fn corrupt(fs: &dyn FsPort, path: &Path) -> (Recents, Option<String>) {
    let bad = bad_path(path);
    if fs.metadata(&bad).is_err() {
        let _ = fs.rename(path, &bad);
    }
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
        assert_eq!(names, vec!["b", "a"], "the display rule: no board name cached → the file stem");
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
        assert_eq!(r.entries()[1].name, "a");
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
        let warning = warning.expect("a warning is reported");
        assert!(warning.contains("newer"), "99 > 1: correctly worded as newer — {warning:?}");
        // `load` never writes on this path: byte-identical, and no `.bad` sibling appears.
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!d.join("recent.json.bad").exists());
    }

    #[test]
    fn older_version_is_worded_as_older_not_newer() {
        // Code review P2: `!=` alone mislabels an older file as "newer" once RECENTS_VERSION is
        // ever bumped past 1. `version:0` is already older than today's `RECENTS_VERSION == 1`.
        let d = TestDir::new("recents-oldver");
        let path = d.join("recent.json");
        let original: &[u8] = br#"{"version":0,"items":[]}"#;
        std::fs::write(&path, original).unwrap();
        let (r, warning) = load(&RealFs, &path);
        assert!(r.entries().is_empty());
        let warning = warning.expect("a warning is reported");
        assert!(warning.contains("older"), "0 < 1: correctly worded as older — {warning:?}");
        assert!(!warning.contains("newer"), "{warning:?}");
        assert_eq!(std::fs::read(&path).unwrap(), original, "left unchanged either way");
    }

    #[test]
    fn second_corruption_preserves_the_first_bad_file() {
        let d = TestDir::new("recents-doublecorrupt");
        let path = d.join("recent.json");
        let bad = d.join("recent.json.bad");
        std::fs::write(&path, b"first corrupt bytes").unwrap();
        let (_, warning1) = load(&RealFs, &path);
        assert!(warning1.is_some());
        assert_eq!(std::fs::read(&bad).unwrap(), b"first corrupt bytes");

        // A second bad write lands at the same path before anyone looks at the first `.bad`.
        std::fs::write(&path, b"second corrupt bytes").unwrap();
        let (r, warning2) = load(&RealFs, &path);
        assert!(r.entries().is_empty());
        assert!(warning2.is_some());
        assert_eq!(std::fs::read(&bad).unwrap(), b"first corrupt bytes", "the first crash's evidence survives");
        assert_eq!(std::fs::read(&path).unwrap(), b"second corrupt bytes", "the second is left where it is, not lost");
    }

    fn summary(name: &str, tags: &[&str], artboards: u32) -> BoardSummary {
        BoardSummary {
            name: name.into(),
            description: format!("{name} description"),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            artboards,
        }
    }

    #[test]
    fn record_board_caches_the_summary_and_a_plain_record_keeps_it() {
        let mut r = Recents::default();
        let p = Path::new("/docs/logo.vrs");
        r.record_board(p, id(1), 100, summary("شعار", &["client", "عربي"], 2), 90);
        let e = &r.entries()[0];
        assert_eq!(e.name, "شعار", "the board's own name is the display name");
        assert_eq!(e.board, Some(summary("شعار", &["client", "عربي"], 2)));
        assert_eq!(e.modified, 90);
        assert!(r.set_thumb(p, Some("thumb-key-1".into())));
        assert!(!r.set_thumb(p, Some("thumb-key-1".into())), "unchanged → no change reported");

        // focusing an already-open tab re-records WITHOUT a summary: the cache stays as it was
        r.record(Path::new("/docs/other.vrs"), id(2), 150);
        r.record(p, id(1), 200);
        let e = &r.entries()[0];
        assert_eq!((e.last_opened, e.modified), (200, 90));
        assert_eq!(e.board.as_ref().map(|b| b.artboards), Some(2));
        assert_eq!(e.thumb.as_deref(), Some("thumb-key-1"), "nothing changed on disk → thumbnail kept");

        // a save with new content: summary replaced, the stale thumbnail key dropped
        r.record_board(p, id(1), 300, summary("", &[], 0), 290);
        let e = &r.entries()[0];
        assert_eq!(e.name, "logo", "an empty board name falls back to the file stem");
        assert_eq!(e.board, Some(summary("", &[], 0)));
        assert_eq!(e.thumb, None);
        // relocate keeps the cache and re-derives the name from the new path
        r.record_board(p, id(1), 310, summary("", &[], 1), 290);
        assert!(r.relocate(p, Path::new("/moved/brand.vrs"), id(7), 400));
        assert_eq!(r.entries()[0].name, "brand");
        assert_eq!(r.entries()[0].board.as_ref().map(|b| b.artboards), Some(1));
        assert!(!r.set_board(Path::new("/never.vrs"), summary("x", &[], 0), 1));
    }

    #[test]
    fn summary_of_a_document() {
        let mut doc = varos_core::board::new_board_with_preset(varos_core::board::PresetId::A4, None);
        doc.name = "Poster".into();
        doc.description = "A4 poster".into();
        doc.tags = vec!["print".into()];
        assert_eq!(
            BoardSummary::of(&doc),
            BoardSummary {
                name: "Poster".into(),
                description: "A4 poster".into(),
                tags: vec!["print".into()],
                artboards: 1
            }
        );
        assert_eq!(BoardSummary::of(&varos_core::board::new_board()).artboards, 0, "a free canvas");
    }

    #[test]
    fn version_1_list_upgrades_in_memory_and_saves_as_version_2() {
        let d = TestDir::new("recents-v1-upgrade");
        let path = d.join("recent.json");
        let v1: &[u8] = br#"{"version":1,"items":[{"path":"/docs/b.vrs","name":"b.vrs","last_opened":200,"file_id":[1,2]},{"path":"/docs/a.vrs","name":"a.vrs","last_opened":100}]}"#;
        std::fs::write(&path, v1).unwrap();
        let (mut r, warning) = load(&RealFs, &path);
        assert_eq!(warning, None, "a known older list is upgraded, not refused");
        assert_eq!(std::fs::read(&path).unwrap(), v1, "loading never writes");
        let names: Vec<&str> = r.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["b", "a"]);
        let e = &r.entries()[0];
        assert_eq!((e.board.clone(), e.modified, e.thumb.clone(), e.file_id), (None, 200, None, Some((1, 2))));
        // the next change writes version 2, which reloads identically
        r.record_board(Path::new("/docs/a.vrs"), None, 300, summary("A", &["t"], 3), 250);
        r.save(&RealFs, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"version\": 2"), "{text}");
        let (again, warning) = load(&RealFs, &path);
        assert_eq!(warning, None);
        assert_eq!(again, r);
    }

    #[test]
    fn an_out_of_bounds_cached_summary_is_dropped_on_load() {
        let d = TestDir::new("recents-bad-cache");
        let path = d.join("recent.json");
        let long = "x".repeat(121);
        let text = format!(
            r#"{{"version":2,"items":[{{"path":"/d/a.vrs","name":"{long}","last_opened":1,"board":{{"name":"{long}","description":"","tags":[],"artboards":1}},"modified":1,"thumb":"k"}}]}}"#
        );
        std::fs::write(&path, &text).unwrap();
        let (r, warning) = load(&RealFs, &path);
        assert_eq!(warning, None);
        let e = &r.entries()[0];
        assert_eq!((e.board.clone(), e.thumb.clone(), e.name.as_str()), (None, None, "a"));
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
