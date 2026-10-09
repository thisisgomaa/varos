//! Autosave uses the manual DocStore::save encoder/gate and the pinned durable writer.
#[cfg(unix)]
use crate::lifecycle::DocStore;
use crate::{app_command::SessionId, lifecycle::SaveOutcome};
#[cfg(unix)]
use std::path::Path;
use std::{path::PathBuf, sync::Arc};
use varos_app::storage::{durable::Fingerprint, publication::Permit};
use varos_core::model::Document;

#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    pub sid: SessionId,
    pub ticket: u64,
    pub dest: PathBuf,
    pub result: Result<(SaveOutcome, Option<Fingerprint>), String>,
}
#[derive(Clone)]
pub struct Job {
    // ---- w2-images ----
    pub blobs: varos_core::images::BlobStore,
    pub sid: SessionId,
    pub ticket: u64,
    pub dest: PathBuf,
    pub doc: Arc<Document>,
    pub expected: Fingerprint,
    pub permit: Permit,
}
impl Job {
    pub fn failed(&self, reason: &str) -> Done {
        Done { sid: self.sid, ticket: self.ticket, dest: self.dest.clone(), result: Err(reason.into()) }
    }
    pub fn run(self) -> Done {
        let result = self.write();
        Done { sid: self.sid, ticket: self.ticket, dest: self.dest, result }
    }
    fn write(&self) -> Result<(SaveOutcome, Option<Fingerprint>), String> {
        if !self.permit.valid() {
            return Err("superseded".into());
        }
        #[cfg(unix)]
        {
            let fs = crate::bridge_fs::Pinned::new(&self.dest, Some(&self.expected), false)
                .map_err(|e| {
                    let e = e.bridge();
                    if e.code == "save_conflict" {
                        "conflict".to_string()
                    } else {
                        e.reason
                    }
                })?
                .with_permit(self.permit.clone())
                .map_err(|e| e.to_string())?;
            let mut store = GuardedStore(&fs);
            let outcome = if self.doc.images.is_empty() {
                store.save(&self.doc, &self.dest)
            } else {
                crate::image_io::save(&fs, &self.doc, &self.blobs, &self.dest).map(|(outcome, _)| outcome)
            }
            .map_err(|reason| {
                if !self.permit.valid() || fs.error().code == "cancelled" {
                    "superseded".into()
                } else if fs.error().code == "busy" {
                    "busy".into()
                } else if fs.error().code == "save_conflict" {
                    "conflict".into()
                } else {
                    reason
                }
            })?;
            Ok((outcome, fs.published()))
        }
        #[cfg(not(unix))]
        {
            let _ = (&self.doc, &self.blobs, &self.expected);
            Err("Automatic writes unavailable on this platform".into())
        }
    }
}
#[cfg(unix)]
struct GuardedStore<'a>(&'a dyn varos_app::storage::durable::FsPort);
#[cfg(unix)]
impl DocStore for GuardedStore<'_> {
    fn save(&mut self, doc: &Document, path: &Path) -> Result<SaveOutcome, String> {
        crate::file_ports::durable_save(self.0, doc, path, &varos_core::format::Limits::DEFAULT)
    }
    fn load(&mut self, path: &Path) -> Result<Document, String> {
        crate::file_ports::DiskStore.load(path)
    }
    fn key(&self, path: &Path) -> crate::workspace::FileKey {
        crate::file_ports::DiskStore.key(path)
    }
    fn exists(&self, path: &Path) -> bool {
        self.0.metadata(path).is_ok()
    }
}

/// Only proven equal decoded content is redundant after a crash; differing/damaged generations stay.
pub fn matches_backing(
    store: &varos_app::storage::recovery::RecoveryStore,
    row: &varos_app::storage::recovery::OrphanEntry,
) -> bool {
    let Some(path) = row.original_path.as_deref() else { return false };
    let before = varos_app::storage::durable::fingerprint(&varos_app::storage::durable::RealFs, path);
    if before.is_none() {
        return false;
    }
    let Ok(backing) = varos_pdf::load_vrs(path) else { return false };
    let Ok((loaded, recovered)) = store.load_best_decoded(&row.rid, |bytes| {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        varos_core::file::doc_from_blob(text)
    }) else {
        return false;
    };
    !loaded.fell_back
        && backing.content_eq(&recovered)
        && before == varos_app::storage::durable::fingerprint(&varos_app::storage::durable::RealFs, path)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use varos_app::storage::{
        checksum::new_nonce,
        durable::{fingerprint, RealFs},
        publication::Gate,
    };
    fn fixture() -> (PathBuf, Job) {
        let root = std::env::temp_dir().join(format!("autosave-{}", new_nonce()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let dest = root.join("board.vrs");
        let doc = Arc::new(varos_core::board::new_board());
        crate::file_ports::DiskStore.save(&doc, &dest).unwrap();
        let job = Job {
            blobs: Default::default(),
            sid: SessionId(1),
            ticket: 1,
            expected: fingerprint(&RealFs, &dest).unwrap(),
            dest,
            doc,
            permit: Arc::new(Gate::default()).capture(),
        };
        (root, job)
    }
    #[test]
    fn same_bytes_as_manual_save_and_published_baseline() {
        let (root, job) = fixture();
        let manual = root.join("manual.vrs");
        crate::file_ports::DiskStore.save(&job.doc, &manual).unwrap();
        let dest = job.dest.clone();
        let done = job.run();
        assert!(matches!(done.result, Ok((SaveOutcome::Durable, Some(_)))));
        assert_eq!(std::fs::read(&manual).unwrap(), std::fs::read(&dest).unwrap());
        assert_eq!(done.result.unwrap().1, fingerprint(&RealFs, &dest));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancelled_capture_and_same_size_same_time_external_edit_never_write() {
        let (root, job) = fixture();
        let before = std::fs::read(&job.dest).unwrap();
        job.permit.gate.invalidate();
        assert_eq!(job.clone().run().result, Err("superseded".into()));
        assert_eq!(std::fs::read(&job.dest).unwrap(), before);
        let mut job = job;
        job.permit = Arc::new(Gate::default()).capture();
        let mut external = before.clone();
        external[0] = b'!';
        std::fs::write(&job.dest, &external).unwrap();
        let f = std::fs::File::options().write(true).open(&job.dest).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(job.expected.modified.unwrap())).unwrap();
        assert_eq!(job.clone().run().result, Err("conflict".into()));
        assert_eq!(std::fs::read(&job.dest).unwrap(), external);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn deletion_replacement_aliases_readonly_and_final_boundary_refuse() {
        for mode in 0..6 {
            let (root, job) = fixture();
            let before = std::fs::read(&job.dest).unwrap();
            match mode {
                0 => {
                    std::fs::remove_file(&job.dest).unwrap();
                }
                1 => {
                    std::fs::remove_file(&job.dest).unwrap();
                    std::fs::write(&job.dest, &before).unwrap();
                }
                2 => {
                    std::fs::hard_link(&job.dest, root.join("alias")).unwrap();
                }
                3 => {
                    let other = root.join("other");
                    std::fs::rename(&job.dest, &other).unwrap();
                    std::os::unix::fs::symlink(&other, &job.dest).unwrap();
                }
                4 => {
                    let mut p = std::fs::metadata(&job.dest).unwrap().permissions();
                    p.set_readonly(true);
                    std::fs::set_permissions(&job.dest, p).unwrap();
                }
                _ => {
                    let fs = crate::bridge_fs::Pinned::new(&job.dest, Some(&job.expected), false)
                        .unwrap()
                        .with_permit(job.permit.clone())
                        .unwrap();
                    job.permit.gate.invalidate();
                    assert!(crate::file_ports::durable_save(
                        &fs,
                        &job.doc,
                        &job.dest,
                        &varos_core::format::Limits::DEFAULT
                    )
                    .is_err());
                    assert_eq!(std::fs::read(&job.dest).unwrap(), before);
                    std::fs::remove_dir_all(root).unwrap();
                    continue;
                }
            }
            assert!(job.run().result.is_err(), "mode {mode}");
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn recovery_deduplicates_only_decoded_equal_stable_backing_content() {
        use varos_app::storage::recovery::{fresh_rid, OrphanEntry, OrphanState, RecoveryStore, SessionMeta};
        let (root, job) = fixture();
        let store = RecoveryStore::open(Arc::new(RealFs), root.join("Recovery")).unwrap();
        let rid = fresh_rid();
        let meta = SessionMeta { rid: rid.clone(), original_path: Some(job.dest.clone()), ..Default::default() };
        let blob = varos_core::file::doc_to_blob(&job.doc).unwrap();
        store.write_generation(&meta, 1, blob.as_bytes(), 1).unwrap();
        let row = OrphanEntry {
            rid,
            display_name: "board".into(),
            original_path: Some(job.dest.clone()),
            saved_at: Some(1),
            created: Some(1),
            state: OrphanState::Ready,
        };
        assert!(matches_backing(&store, &row));
        store.write_generation(&meta, 2, b"undecodable newest generation", 2).unwrap();
        assert!(
            !matches_backing(&store, &row),
            "uncertain newest copies remain offered even if the fallback equals disk"
        );
        let mut changed = (*job.doc).clone();
        changed.description = "older differing copy".into();
        let blob = varos_core::file::doc_to_blob(&changed).unwrap();
        store.write_generation(&meta, 3, blob.as_bytes(), 3).unwrap();
        assert!(!matches_backing(&store, &row));
        std::fs::write(&job.dest, b"unreadable document").unwrap();
        assert!(!matches_backing(&store, &row));
        assert!(store.load_best(&row.rid).is_ok());
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
