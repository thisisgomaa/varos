//! E2's host-owned Recent adapter. Model refresh and disk probes occur at lifecycle boundaries,
//! never during paint. The document store remains the sole reader/writer of documents.
use crate::{lifecycle::DocStore, thumbs, workspace::FileKey};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use varos_app::{
    start::StartModel,
    storage::{
        durable::{FsPort, RealFs},
        paths::{self, AppLayout},
        recents::{self, BoardSummary, Recents},
    },
};
use varos_core::model::Document;

pub struct RecentStore<S> {
    pub inner: S,
    pub recents: Recents,
    pub warning: Option<String>,
    destination: Option<PathBuf>,
    /// Bumps on every Recent change; part of Start's rebuild key (rebuild only when it moved).
    generation: u64,
    /// Start v2 thumbnails (lane L3's worker + cache); `None` = no cache dir → placeholders only.
    thumbs: Option<thumbs::ThumbService>,
}
impl<S: DocStore> RecentStore<S> {
    pub fn new(inner: S) -> Self {
        Self::at(inner, paths::data_root().map(|root| AppLayout { root }.recents()))
    }
    fn at(inner: S, destination: Option<PathBuf>) -> Self {
        let (recents, warning) = destination.as_deref().map(|p| recents::load(&RealFs, p)).unwrap_or_else(|| {
            (
                Recents::default(),
                Some(
                    "Recent documents are available for this session only: the app data folder is unavailable.".into(),
                ),
            )
        });
        // Unknown versions/read errors/corruption are never overwritten after a warning.
        let destination = if warning.is_some() { None } else { destination };
        Self { inner, recents, warning, destination, generation: 0, thumbs: None }
    }
    /// Install the thumbnail service (startup). A cache that cannot be written never surfaces to the
    /// user: the render fails on the worker, the card keeps its typographic placeholder.
    pub fn set_thumbs(&mut self, service: Option<thumbs::ThumbService>) {
        self.thumbs = service;
    }
    /// The read side Home's page looks thumbnails up with (off the UI thread).
    pub fn thumb_index(&self) -> Option<thumbs::ThumbIndex> {
        self.thumbs.as_ref().map(thumbs::ThumbService::index)
    }
    /// Landed renders since the last call: each written thumbnail's key is cached on its Recent entry
    /// (the generation moves, Home rebuilds). Returns the keys whose pixels changed. Never blocks.
    pub fn poll_thumbs(&mut self) -> Vec<varos_app::start::ThumbKey> {
        let Some(service) = &self.thumbs else {
            return vec![];
        };
        let mut landed = vec![];
        for done in service.try_completions() {
            let Some(_) = done.path else {
                continue; // the cache could not be written: the placeholder stays, no message
            };
            let path = PathBuf::from(&done.key.0);
            // only a board still in Recent whose cached time is the one rendered keeps the key
            let current =
                self.recents.entries().iter().any(|e| e.path == path && thumbs::unix(e.modified) == done.mtime);
            if current && self.recents.set_thumb(&path, Some(done.key.0.clone())) {
                self.persist();
            }
            landed.push(varos_app::start::ThumbKey(done.key.0));
        }
        landed
    }
    /// The cache key of a board in Recent (lane L3's rule: the file key's path).
    fn thumb_key(&self, path: &Path) -> thumbs::ThumbKey {
        thumbs::key_for_file(&self.key(path))
    }
    /// Start's model. `missing` is the host's cached, non-blocking probe (never a UI-thread stat).
    pub fn model(
        &self,
        recovery: Vec<varos_app::start::RecoveryRow>,
        missing: impl FnMut(&Path) -> bool,
    ) -> StartModel {
        StartModel::build(&self.recents, now(), missing, recovery)
    }
    /// The paths currently in Recent (the existence probe prunes to these).
    pub fn recent_paths(&self) -> Vec<PathBuf> {
        self.recents.entries().iter().map(|e| e.path.clone()).collect()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    fn persist(&mut self) {
        self.generation += 1;
        let Some(path) = &self.destination else {
            return;
        };
        let result =
            path.parent().map_or(Ok(()), |p| RealFs.create_dir_all(p)).map_err(|e| e.to_string()).and_then(|_| {
                self.recents.save(&RealFs, path).map_err(|e| format!("{e:?}")).and_then(|outcome| match outcome {
                    varos_app::storage::durable::WriteOutcome::Durable => Ok(()),
                    varos_app::storage::durable::WriteOutcome::ReplacedUnconfirmed(e) => Err(e.to_string()),
                })
            });
        if result.is_err() {
            self.warning = Some("Couldn't save the recent documents list. Your documents are safe; Recent is available for this session.".into());
        } else {
            self.warning = None;
        }
    }
}
fn now() -> u64 {
    unix_secs(std::time::SystemTime::now())
}
fn unix_secs(t: std::time::SystemTime) -> u64 {
    t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
impl<S: DocStore> DocStore for RecentStore<S> {
    fn load(&mut self, path: &Path) -> Result<Document, String> {
        self.inner.load(path)
    }
    fn load_with_notice(&mut self, path: &Path) -> Result<(Document, Option<&'static str>), String> {
        self.inner.load_with_notice(path)
    }
    fn save(&mut self, doc: &Document, path: &Path) -> Result<crate::lifecycle::SaveOutcome, String> {
        self.inner.save(doc, path)
    }
    fn fingerprint(&self, path: &Path) -> Option<varos_app::storage::durable::Fingerprint> {
        self.inner.fingerprint(path)
    }
    fn key(&self, path: &Path) -> FileKey {
        self.inner.key(path)
    }
    fn exists(&self, path: &Path) -> bool {
        self.inner.exists(path)
    }
    // An export is never a Recent entry: both pass straight through, nothing is recorded.
    fn write_export(&mut self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        self.inner.write_export(path, bytes)
    }
    fn read_existing(&mut self, path: &Path) -> Option<Vec<u8>> {
        self.inner.read_existing(path)
    }
    fn remember(&mut self, path: &Path, old: Option<&Path>, board: Option<&BoardSummary>) {
        let key = self.key(path);
        if let Some(old) = old {
            // Preserve the broken entry's position, even when Locate chooses an already-recent file.
            let duplicates: Vec<_> = self
                .recents
                .entries()
                .iter()
                .filter(|e| {
                    e.path != old
                        && (e.path == key.path || matches!((e.file_id, key.dev_ino), (Some(a), Some(b)) if a == b))
                })
                .map(|e| e.path.clone())
                .collect();
            for path in duplicates {
                self.recents.remove(&path);
            }
            if !self.recents.relocate(old, &key.path, key.dev_ino, now()) {
                self.recents.record(&key.path, key.dev_ino, now());
            }
        } else {
            self.recents.record(&key.path, key.dev_ino, now());
        }
        if let Some(board) = board {
            // a lifecycle boundary (never paint): one metadata read for the card's date
            let modified = self.inner.fingerprint(&key.path).and_then(|f| f.modified).map_or_else(now, unix_secs);
            self.recents.set_board(&key.path, board.clone(), modified);
        }
        self.persist();
    }
    fn remove_recent(&mut self, path: &Path) {
        if let Some(service) = &self.thumbs {
            service.forget(&self.thumb_key(path));
        }
        if self.recents.remove(path) {
            self.persist();
        }
    }
    fn clear_recent(&mut self) {
        if let Some(service) = &self.thumbs {
            service.clear();
        }
        self.recents.clear();
        self.persist();
    }
    fn rendered(&mut self, path: &Path, snapshot: Arc<Document>) {
        let (Some(service), Some(entry)) = (&self.thumbs, self.recents.entries().iter().find(|e| e.path == path))
        else {
            return;
        };
        // the cache is stamped with Recent's cached modified time: Home looks it up with the same value
        thumbs::on_saved(service, &self.key(path), snapshot, thumbs::unix(entry.modified));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_ports::DiskStore;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("varos-e2-{}", varos_app::storage::checksum::new_nonce()));
            std::fs::create_dir_all(&p).unwrap();
            Self(std::fs::canonicalize(p).unwrap())
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn recent_persistence_reload_remove_and_clear_never_write_documents() {
        let dir = Temp::new();
        let dest = dir.0.join("recent.json");
        let a = dir.0.join("a.vrs");
        let b = dir.0.join("b.vrs");
        std::fs::write(&a, b"document a").unwrap();
        std::fs::write(&b, b"document b").unwrap();
        let mut store = RecentStore::at(DiskStore, Some(dest.clone()));
        store.remember(&a, None, None);
        store.remember(&b, None, None);
        store.remember(&a, None, None);
        assert!(store.warning.is_none());
        let mut store = RecentStore::at(DiskStore, Some(dest.clone()));
        assert_eq!(store.recents.entries().len(), 2);
        assert_eq!(store.recents.entries()[0].path, a);
        store.remove_recent(&a);
        assert_eq!(RecentStore::at(DiskStore, Some(dest.clone())).recents.entries().len(), 1);
        store.clear_recent();
        assert!(RecentStore::at(DiskStore, Some(dest)).recents.entries().is_empty());
        assert_eq!(std::fs::read(a).unwrap(), b"document a");
        assert_eq!(std::fs::read(b).unwrap(), b"document b");
    }
    #[test]
    fn unknown_version_and_unwritable_storage_keep_recent_in_memory_without_overwriting() {
        let dir = Temp::new();
        let dest = dir.0.join("recent.json");
        let future = br#"{"version":99,"items":[]}"#;
        std::fs::write(&dest, future).unwrap();
        let mut store = RecentStore::at(DiskStore, Some(dest.clone()));
        store.remember(&dir.0.join("a.vrs"), None, None);
        assert!(store.warning.is_some());
        assert_eq!(store.recents.entries().len(), 1);
        assert_eq!(std::fs::read(&dest).unwrap(), future);
        let parent = dir.0.join("not-a-directory");
        std::fs::write(&parent, b"keep").unwrap();
        let mut store = RecentStore::at(DiskStore, Some(parent.join("recent.json")));
        store.remember(&dir.0.join("b.vrs"), None, None);
        assert!(store.warning.is_some());
        assert_eq!(store.recents.entries().len(), 1);
        assert_eq!(std::fs::read(parent).unwrap(), b"keep");
    }
    #[test]
    fn native_recent_menu_is_the_first_ten_start_entries_and_locate_keeps_its_slot() {
        let dir = Temp::new();
        let mut store = RecentStore::at(DiskStore, None);
        for i in 0..20 {
            store.remember(&dir.0.join(format!("{i}.vrs")), None, None);
        }
        let rows = crate::chrome::recent_menu(&store.recents);
        assert_eq!(rows.len(), 10);
        assert_eq!(
            rows.iter().map(|(_, p)| p).collect::<Vec<_>>(),
            store.model(Vec::new(), |_| false).rows().iter().take(10).map(|r| &r.path).collect::<Vec<_>>()
        );
        let old = store.recents.entries()[5].path.clone();
        let new = dir.0.join("found.vrs");
        store.remember(&new, Some(&old), None);
        assert_eq!(store.recents.entries()[5].path, new);
        assert_eq!(store.recents.entries().len(), 20);
    }

    // ── Start v2 thumbnails (lane L3's service, wired here) ──

    fn board(name: &str) -> (Document, BoardSummary) {
        let doc = Document { name: name.into(), ..Default::default() };
        let summary = BoardSummary::of(&doc);
        (doc, summary)
    }
    /// Poll until the worker landed (bounded: the render is milliseconds).
    fn settle_thumbs(store: &mut RecentStore<DiskStore>) -> Vec<varos_app::start::ThumbKey> {
        for _ in 0..500 {
            let landed = store.poll_thumbs();
            if !landed.is_empty() {
                return landed;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        vec![]
    }
    fn saved(store: &mut RecentStore<DiskStore>, path: &Path, name: &str) {
        let (doc, summary) = board(name);
        std::fs::write(path, b"board").unwrap();
        store.remember(path, None, Some(&summary)); // what `save_done` does where the save lands …
        store.rendered(path, Arc::new(doc)); // … then the thumbnail hook
    }

    #[test]
    fn a_saved_board_gets_a_thumbnail_key_and_its_png_exists() {
        let dir = Temp::new();
        let mut store = RecentStore::at(DiskStore, Some(dir.0.join("recent.json")));
        store.set_thumbs(thumbs::ThumbService::at(dir.0.join("thumbs")));
        let a = dir.0.join("a.vrs");
        saved(&mut store, &a, "Logo");
        let landed = settle_thumbs(&mut store);
        assert_eq!(landed, [varos_app::start::ThumbKey(a.to_string_lossy().into_owned())]);
        let entry = &store.recents.entries()[0];
        assert_eq!(entry.thumb.as_deref(), Some(a.to_string_lossy().as_ref()), "Recent caches the key");
        let model = store.model(vec![], |_| false);
        assert_eq!(model.cards()[0].thumb, Some(varos_app::start::ThumbKey(a.to_string_lossy().into_owned())));
        let index = store.thumb_index().unwrap();
        use varos_app::start_page::ThumbSource;
        let (png, fresh) = index.find(&model.cards()[0].thumb.clone().unwrap(), entry.modified).expect("a PNG");
        assert!(png.is_file() && fresh, "the PNG exists and is current for the cached time");
        assert_eq!(store.warning, None);
    }

    #[test]
    fn remove_deletes_the_png_and_clear_deletes_all() {
        let dir = Temp::new();
        let cache = dir.0.join("thumbs");
        let mut store = RecentStore::at(DiskStore, Some(dir.0.join("recent.json")));
        store.set_thumbs(thumbs::ThumbService::at(cache.clone()));
        let (a, b, c) = (dir.0.join("a.vrs"), dir.0.join("b.vrs"), dir.0.join("c.vrs"));
        for (path, name) in [(&a, "A"), (&b, "B"), (&c, "C")] {
            saved(&mut store, path, name);
            assert!(!settle_thumbs(&mut store).is_empty());
        }
        let pngs = || {
            std::fs::read_dir(&cache)
                .unwrap()
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
                .count()
        };
        assert_eq!(pngs(), 3);
        store.remove_recent(&a);
        assert_eq!(pngs(), 2, "Remove from Recent deletes that board's PNG");
        store.clear_recent();
        assert_eq!(pngs(), 0, "Clear Recent deletes them all");
    }

    #[test]
    fn a_cache_that_cannot_be_written_leaves_the_placeholder_and_no_message() {
        let dir = Temp::new();
        let blocked = dir.0.join("not-a-dir");
        std::fs::write(&blocked, b"a file where the cache folder should be").unwrap();
        let mut store = RecentStore::at(DiskStore, Some(dir.0.join("recent.json")));
        store.set_thumbs(thumbs::ThumbService::at(blocked.join("thumbs")));
        let a = dir.0.join("a.vrs");
        saved(&mut store, &a, "Logo");
        for _ in 0..50 {
            assert!(store.poll_thumbs().is_empty(), "a failed render lands nothing");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(store.recents.entries()[0].thumb, None, "no key: the card keeps its placeholder");
        assert_eq!(store.warning, None, "the user sees no error");
    }
}
