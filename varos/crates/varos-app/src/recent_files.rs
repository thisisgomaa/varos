//! E2's host-owned Recent adapter. Model refresh and disk probes occur at lifecycle boundaries,
//! never during paint. The document store remains the sole reader/writer of documents.
use crate::{lifecycle::DocStore, workspace::FileKey};
use std::path::{Path, PathBuf};
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
        Self { inner, recents, warning, destination, generation: 0 }
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
        if self.recents.remove(path) {
            self.persist();
        }
    }
    fn clear_recent(&mut self) {
        self.recents.clear();
        self.persist();
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
}
