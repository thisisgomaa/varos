//! F1: host-owned recovery coordination. The worker only receives owned documents and metadata.
//!
//! The host also owns THE one background I/O worker for the whole app: S6's manual saves and PDF
//! exports (`file_jobs`) run on the same FIFO thread as the recovery copies (`Finished::File`), so a
//! Quit's drain (`shutdown`) finishes an in-flight export exactly like a recovery copy.
use crate::{
    app_command::{AppCommand, SessionId},
    workspace::{DocumentSession, Workspace},
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use varos_app::storage::{
    durable::{RealFs, WriteOutcome},
    io_worker::IoWorker,
    paths::{self, AppLayout},
    recovery::{OrphanEntry, OrphanState, RecoveryStore, SessionMeta},
    scheduler::{Action, Completion, Done, Probe, Scheduler, SessionRecovery},
    settings::{self, Settings},
    time_text,
};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct RecoveryUi {
    pub enabled: bool,
    pub status: String,
    pub detail: String,
    pub last_copy: String,
    pub retry: bool,
    pub sid: Option<SessionId>,
    pub banner: bool,
    pub recovered_notice: Option<String>,
}
enum Finished {
    Recovery(Completion<SessionId>),
    Settings(Result<(), String>),
    Scanned(Vec<OrphanEntry>),
    ScanFailed,
    Loaded(String, Result<Box<crate::workspace::RecoveredDocument>, String>),
    Discarded(String, Result<(), String>),
    /// A manual save / PDF export (`file_jobs`), handed to the lifecycle as `AppCommand::FileDone`.
    File(crate::file_jobs::FileDone),
}
pub struct RecoveryHost {
    scheduler: Scheduler,
    worker: Option<IoWorker<Finished>>,
    store: Option<Arc<RecoveryStore>>,
    settings_path: Option<PathBuf>,
    pub warning: Option<String>,
    orphans: Vec<OrphanEntry>,
    busy: std::collections::HashSet<String>,
    ready: Vec<crate::workspace::RecoveredDocument>,
    deferred: bool,
    changed: bool,
    /// Why the Recovery switch cannot be persisted (settings unreadable / no data folder): a
    /// change then applies to this session only, and the user is told so.
    settings_unsaved: Option<String>,
    /// File-job results received but not yet applied (oldest first).
    file_done: std::collections::VecDeque<crate::file_jobs::FileDone>,
    /// Recovery results received while the host waited for a file job; `observe` handles them first.
    held: Vec<Finished>,
    /// The command held back for an in-flight save (`host::run_command`).
    pub save_wait: crate::host::SaveWait,
}
impl RecoveryHost {
    pub fn new(wake: Box<dyn Fn() + Send>) -> Self {
        let mut host = Self::at(paths::data_root().map(|root| AppLayout { root }), wake);
        host.begin_scan();
        host
    }
    fn at(layout: Option<AppLayout>, wake: Box<dyn Fn() + Send>) -> Self {
        let mut host = Self {
            scheduler: Scheduler::default(),
            worker: None,
            store: None,
            settings_path: None,
            warning: None,
            orphans: Vec::new(),
            busy: Default::default(),
            ready: Vec::new(),
            deferred: false,
            changed: false,
            settings_unsaved: None,
            file_done: Default::default(),
            held: Vec::new(),
            save_wait: Default::default(),
        };
        // The worker runs whether or not recovery storage is available: saves and exports use it too.
        let worker_error = match IoWorker::spawn(wake) {
            Ok(worker) => {
                host.worker = Some(worker);
                None
            }
            Err(e) => Some(format!("Recovery writer unavailable: {e}")),
        };
        let Some(layout) = layout else {
            host.warning = Some("Recovery unavailable: the app data folder is unavailable.".into());
            host.settings_unsaved = Some("the app data folder is unavailable.".into());
            return host;
        };
        let (settings, warning) = settings::load(&RealFs, &layout.settings());
        host.scheduler.set_enabled(settings.recovery_enabled);
        if warning.is_none() {
            host.settings_path = Some(layout.settings());
        }
        host.settings_unsaved.clone_from(&warning);
        host.warning = warning;
        match RecoveryStore::open(Arc::new(RealFs), layout.recovery()) {
            Ok(store) => host.store = Some(Arc::new(store)),
            Err(e) => {
                host.warning = Some(e.reason());
                return host;
            }
        }
        if worker_error.is_some() {
            host.warning = worker_error;
        }
        host
    }

    /// Pull every finished job off the worker without blocking: file-job results go to the file
    /// queue ([`Self::take_file_done`]), the rest wait for `observe`.
    fn pump(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        for done in worker.try_completions() {
            self.sort(done);
        }
    }
    fn sort(&mut self, done: Finished) {
        match done {
            Finished::File(d) => self.file_done.push_back(d),
            other => self.held.push(other),
        }
    }
    /// The file-job results received so far (oldest first), for the host to apply.
    pub fn take_file_done(&mut self) -> Vec<crate::file_jobs::FileDone> {
        self.pump();
        self.file_done.drain(..).collect()
    }
    /// A file-job result is waiting to be applied.
    pub fn has_file_done(&self) -> bool {
        !self.file_done.is_empty()
    }
    /// One launch scan, on the same FIFO as the writer; its store holds all orphan claims.
    fn begin_scan(&mut self) {
        let (Some(store), Some(worker)) = (&self.store, &self.worker) else {
            return;
        };
        let store = Arc::clone(store);
        let job = Box::new(move || {
            store.cleanup_completed();
            Finished::Scanned(store.scan())
        });
        if worker.submit(job, Finished::ScanFailed).is_err() {
            self.warning = Some("Recovery scan unavailable.".into());
        }
    }
    pub fn rows(&self) -> Vec<varos_app::start::RecoveryRow> {
        self.orphans
            .iter()
            .map(|row| varos_app::start::RecoveryRow {
                rid: row.rid.clone(),
                name: row.display_name.clone(),
                original_dir: row
                    .original_path
                    .as_deref()
                    .and_then(std::path::Path::parent)
                    .map(|p| p.display().to_string()),
                saved_at_text: row
                    .saved_at
                    .map(|t| format!("Saved {}", time_text::clock_hhmm(t)))
                    .unwrap_or_else(|| "Save time unavailable".into()),
                problem: match &row.state {
                    OrphanState::Ready => None,
                    OrphanState::Damaged(reason) => Some(reason.clone()),
                    OrphanState::NewerFormat(_) => Some("This copy needs a newer version of Varos.".into()),
                },
                busy: self.busy.contains(&row.rid),
            })
            .collect()
    }
    pub fn start_warning(&self, recent: Option<&str>) -> Option<String> {
        match (recent, self.warning.as_deref()) {
            (Some(a), Some(b)) => Some(format!("{a}\n{b}")),
            (a, b) => a.or(b).map(str::to_owned),
        }
    }
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
    pub fn take_recovered(&mut self) -> Vec<crate::workspace::RecoveredDocument> {
        std::mem::take(&mut self.ready)
    }
    fn row_error(&mut self, rid: &str, reason: String) {
        if let Some(row) = self.orphans.iter_mut().find(|row| row.rid == rid) {
            row.state = OrphanState::Damaged(reason);
        }
    }
    pub fn handle_read(&mut self, cmd: &AppCommand, dialogs: &mut dyn crate::lifecycle::Dialogs) -> bool {
        match cmd {
            AppCommand::ReviewRecovery => {
                self.deferred = false;
                false
            } // lifecycle settles and shows Home
            AppCommand::SetRecoveryEnabled(_) => {
                if let Some(reason) = &self.settings_unsaved {
                    // The switch still applies for this session (`handle`); say it won't persist.
                    dialogs.notice("Recovery", &format!("Recovery setting could not be saved: {reason}"));
                }
                false
            }
            AppCommand::DeferRecovery => {
                self.deferred = true;
                self.changed = true;
                true
            }
            AppCommand::Recover(rid) | AppCommand::DiscardRecovery(rid) => {
                let Some(row) = self.orphans.iter().find(|row| &row.rid == rid).cloned() else {
                    return true;
                };
                if self.busy.contains(rid) {
                    return true;
                }
                let discard = matches!(cmd, AppCommand::DiscardRecovery(_));
                if discard {
                    if !dialogs.confirm_discard_recovery(&row.display_name) {
                        return true;
                    }
                } else if !matches!(row.state, OrphanState::Ready) {
                    return true;
                }
                let (Some(store), Some(worker)) = (&self.store, &self.worker) else {
                    return true;
                };
                let store = Arc::clone(store);
                let rid = rid.clone();
                let panic_result = if discard {
                    Finished::Discarded(rid.clone(), Err("Couldn't discard this copy.".into()))
                } else {
                    Finished::Loaded(rid.clone(), Err("Couldn't read this copy.".into()))
                };
                self.busy.insert(rid.clone());
                self.changed = true;
                let job = Box::new(move || {
                    if discard {
                        return Finished::Discarded(rid.clone(), store.retire(&rid).map_err(|e| e.reason()));
                    }
                    let result = store
                        .load_best_decoded(&rid, |blob| {
                            let text =
                                std::str::from_utf8(blob).map_err(|_| "This recovery copy is damaged.".to_string())?;
                            // No format notice is dropped here: recovery copies are always written by
                            // this build's `doc_to_blob` (current v2), so no migration or released-mask
                            // repair — and therefore no notice — can arise when one is read back.
                            varos_core::file::doc_from_blob(text)
                        })
                        .map_err(|e| e.reason())
                        .map(|(loaded, doc)| {
                            Box::new(crate::workspace::RecoveredDocument {
                                doc,
                                rid: rid.clone(),
                                generation: loaded.generation.clone(),
                                source: crate::workspace::RecoveredSource {
                                    name: row.display_name,
                                    original_path: row.original_path,
                                    saved_at: loaded.generation.saved_at,
                                    fell_back: loaded.fell_back,
                                },
                            })
                        });
                    Finished::Loaded(rid, result)
                });
                if worker.submit(job, panic_result).is_err() {
                    self.busy.remove(&row.rid);
                    self.row_error(&row.rid, "Recovery worker stopped. Your copy is kept.".into());
                }
                true
            }
            _ => false,
        }
    }
    pub fn next_wake(&self) -> Option<Instant> {
        self.scheduler.next_wake()
    }
    pub fn handle(&mut self, cmd: &AppCommand, ws: &mut Workspace, now: Instant) -> bool {
        match cmd {
            AppCommand::SetRecoveryEnabled(enabled) => {
                self.scheduler.set_enabled(*enabled);
                if let (Some(path), Some(worker)) = (self.settings_path.clone(), &self.worker) {
                    let settings = Settings { recovery_enabled: *enabled };
                    let job = Box::new(move || {
                        Finished::Settings(settings.save(&RealFs, &path).map_err(|e| e.reason()).and_then(|outcome| {
                            match outcome {
                                WriteOutcome::Durable => Ok(()),
                                WriteOutcome::ReplacedUnconfirmed(e) => Err(e.to_string()),
                            }
                        }))
                    });
                    if worker.submit(job, Finished::Settings(Err("Settings writer failed.".into()))).is_err() {
                        self.warning = Some("Couldn't save the Recovery setting.".into());
                    }
                }
                true
            }
            AppCommand::RetryRecovery(id) => {
                if let Some(s) = ws.get_mut(*id) {
                    self.scheduler.retry_now(now, &mut s.recovery);
                }
                true
            }
            _ => false,
        }
    }
    pub fn observe(&mut self, ws: &mut Workspace, now: Instant) {
        {
            self.pump();
            let completed = std::mem::take(&mut self.held);
            for done in completed {
                match done {
                    Finished::File(d) => self.file_done.push_back(d), // `pump` sorted these already
                    Finished::ScanFailed => {
                        self.warning = Some("Recovery scan failed. Existing copies are kept.".into());
                        self.changed = true;
                    }
                    Finished::Scanned(rows) => {
                        self.orphans = rows;
                        self.changed = true;
                    }
                    Finished::Loaded(rid, result) => {
                        self.busy.remove(&rid);
                        match result {
                            Ok(copy) => {
                                self.orphans.retain(|row| row.rid != rid);
                                self.ready.push(*copy);
                            }
                            Err(reason) => self.row_error(&rid, reason),
                        }
                        self.changed = true;
                    }
                    Finished::Discarded(rid, result) => {
                        self.busy.remove(&rid);
                        match result {
                            Ok(()) => self.orphans.retain(|row| row.rid != rid),
                            Err(reason) => self.row_error(&rid, reason),
                        }
                        self.changed = true;
                    }
                    Finished::Recovery(done) => {
                        if let Some(s) = ws.get_mut(done.sid) {
                            self.scheduler.on_complete(now, s.id, &mut s.recovery, done);
                        }
                    }
                    Finished::Settings(result) => {
                        if let Err(e) = result {
                            self.warning = Some(format!("Couldn't save the Recovery setting. {e}"));
                        }
                    }
                }
            }
        }
        if self.store.is_none() || self.worker.is_none() {
            return;
        }
        // All probes must be observed together so next_wake remains the earliest session deadline.
        let probes = ws.sessions_mut().iter_mut().map(|s| {
            let clean = !s.is_dirty();
            Probe {
                sid: s.id,
                rev: s.editor.rev,
                clean,
                transaction_open: s.editor.transaction_open(),
                recovery: &mut s.recovery,
            }
        });
        let actions = self.scheduler.observe(now, probes);
        for action in actions {
            self.submit(action, ws);
        }
    }
    fn submit(&mut self, action: Action<SessionId>, ws: &mut Workspace) {
        let (Some(store), Some(worker)) = (&self.store, &self.worker) else {
            return;
        };
        let store = Arc::clone(store);
        let (sid, seq, job): (_, _, varos_app::storage::io_worker::Job<Finished>) = match action {
            Action::Waiting { .. } => return,
            Action::Snapshot { sid, rid, seq } => {
                let Some(s) = ws.get(sid) else {
                    return;
                };
                let meta = SessionMeta {
                    rid,
                    display_name: s.recovered.as_ref().map(|r| r.name.clone()).unwrap_or_else(|| s.display_name()),
                    untitled_number: s.untitled,
                    original_path: s
                        .path
                        .clone()
                        .or_else(|| s.recovered.as_ref().and_then(|r| r.original_path.clone())),
                    source_fingerprint: s.source_fingerprint,
                    recovered: s.recovered.is_some(),
                };
                let doc = s.editor.doc.clone();
                (
                    sid,
                    seq,
                    Box::new(move || {
                        let result = varos_core::file::doc_to_blob(&doc).and_then(|blob| {
                            store
                                .write_generation(&meta, seq, blob.as_bytes(), unix_now())
                                .map(Done::Snapshot)
                                .map_err(|e| e.reason())
                        });
                        Finished::Recovery(Completion { sid, seq, result })
                    }),
                )
            }
            Action::Retire { sid, rid, seq } => (
                sid,
                seq,
                Box::new(move || {
                    Finished::Recovery(Completion {
                        sid,
                        seq,
                        result: store.retire(&rid).map(|_| Done::Retired).map_err(|e| e.reason()),
                    })
                }),
            ),
        };
        if worker
            .submit(job, Finished::Recovery(Completion { sid, seq, result: Err("Recovery writer failed.".into()) }))
            .is_err()
        {
            if let Some(s) = ws.get_mut(sid) {
                self.scheduler.on_complete(
                    Instant::now(),
                    sid,
                    &mut s.recovery,
                    Completion { sid, seq, result: Err("Recovery writer stopped.".into()) },
                );
            }
        }
    }
    /// Capture before dispatch. Retire only sessions that actually disappeared, or a committed Quit.
    pub fn before_close(ws: &Workspace) -> Vec<(SessionId, SessionRecovery)> {
        ws.visible_tabs().iter().filter_map(|tab| ws.get(tab.id).map(|s| (s.id, s.recovery.clone()))).collect()
    }
    pub fn after_dispatch(
        &mut self,
        before: Vec<(SessionId, SessionRecovery)>,
        ws: &mut Workspace,
        exit: bool,
        now: Instant,
    ) {
        let open = ws.visible_tabs();
        for (sid, mut recovery) in before {
            if exit || !open.iter().any(|tab| tab.id == sid) {
                if let Some(action) = self.scheduler.retire_for_close(sid, &mut recovery, now) {
                    self.submit(action, ws);
                }
            }
        }
    }
    /// Quit: close the worker's queue and run everything already on it — the final recovery retires
    /// AND any in-flight PDF export (Quit waits for it) — then join the thread.
    pub fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            for done in worker.shutdown() {
                if let Finished::Recovery(Completion { result: Err(reason), .. }) = done {
                    eprintln!("Recovery cleanup: {reason}");
                }
            }
        }
    }
    pub fn presentation(&self, session: Option<&DocumentSession>) -> RecoveryUi {
        let mut ui = RecoveryUi {
            enabled: self.scheduler.enabled(),
            sid: session.map(|s| s.id),
            banner: !self.deferred && !self.orphans.is_empty(),
            ..Default::default()
        };
        if let Some(source) = session.and_then(|s| s.recovered.as_ref()) {
            ui.recovered_notice = Some(format!(
                "Recovered “{}” from {}. Save this copy to keep it. Your original file has not been changed.{}",
                source.name,
                time_text::clock_hhmm(source.saved_at),
                if source.fell_back { " The newest copy was damaged; the previous copy was used." } else { "" }
            ));
        }
        if let Some(warning) = &self.warning {
            ui.detail = warning.clone();
        }
        if self.worker.is_none() || self.store.is_none() {
            ui.status = "Recovery unavailable. Save your document regularly.".into();
        } else if !ui.enabled {
            ui.status = "Recovery is off. Save regularly to keep your work.".into();
        } else if let Some(s) = session {
            let r = &s.recovery;
            if let Some(g) = &r.last_ok {
                ui.last_copy = format!("Last recovery copy: {}", time_text::clock_hhmm(g.saved_at));
                if let Some(store) = &self.store {
                    ui.last_copy.push_str(&format!(" · {}", store.dir().display()));
                }
            }
            ui.status = if r.last_err.is_some() {
                ui.retry = true;
                ui.detail = r.last_err.clone().unwrap_or_default();
                "Recovery copy couldn't be saved. Save your document now.".into()
            } else if r.waiting {
                "Recovery waiting for edit to finish".into()
            } else if r
                .in_flight
                .as_ref()
                .is_some_and(|job| job.kind == varos_app::storage::scheduler::JobKind::Snapshot)
            {
                "Saving recovery copy…".into()
            } else if let Some(g) = &r.last_ok {
                format!("Recovery copy saved at {}", time_text::clock_hhmm(g.saved_at))
            } else {
                "Recovery on · copies every 30 seconds after changes".into()
            };
        }
        ui
    }
}
impl crate::host::FileJobs for RecoveryHost {
    fn submit(&mut self, job: crate::file_jobs::FileJob) -> Result<(), crate::file_jobs::FileJob> {
        let Some(worker) = &self.worker else {
            return Err(job);
        };
        let if_panicked = Finished::File(crate::file_jobs::FileDone::panicked(&job));
        // Shared with the job so a stopped worker hands the job back for an inline run.
        let slot = Arc::new(std::sync::Mutex::new(Some(job)));
        let mine = Arc::clone(&slot);
        let run = Box::new(move || {
            let job = mine.lock().ok().and_then(|mut j| j.take()).expect("the job runs once");
            // The plain disk store: the Recent entry of a save is recorded when its result is applied.
            Finished::File(crate::file_jobs::execute(job, &mut crate::file_ports::DiskStore))
        });
        match worker.submit(run, if_panicked) {
            Ok(()) => Ok(()),
            Err(_) => Err(slot.lock().ok().and_then(|mut j| j.take()).expect("a refused job never ran")),
        }
    }

    fn save_wait(&mut self) -> &mut crate::host::SaveWait {
        &mut self.save_wait
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    use varos_app::storage::{checksum::new_nonce, scheduler::RECOVERY_INTERVAL};
    use varos_core::editor::ToolKind;

    struct Rig {
        host: RecoveryHost,
        ws: Workspace,
        wake: mpsc::Receiver<()>,
        layout: AppLayout,
        now: Instant,
    }
    impl Rig {
        fn new() -> Self {
            let layout = AppLayout { root: std::env::temp_dir().join(format!("varos-f1-{}", new_nonce())) };
            let (tx, wake) = mpsc::channel();
            let host = RecoveryHost::at(
                Some(layout.clone()),
                Box::new(move || {
                    let _ = tx.send(());
                }),
            );
            assert!(host.warning.is_none(), "{:?}", host.warning);
            Self { host, ws: Workspace::new(), wake, layout, now: Instant::now() }
        }
        fn edit(&mut self) {
            let ed = &mut self.ws.active_mut().unwrap().editor;
            ed.set_tool(ToolKind::Rect);
            ed.pointer_down([50.0, 50.0]);
            ed.pointer_move([100.0, 100.0]);
            ed.pointer_up();
        }
        fn complete(&mut self) {
            self.wake.recv_timeout(Duration::from_secs(5)).expect("worker completion");
            self.host.observe(&mut self.ws, self.now);
        }
        fn snapshot(&mut self) -> String {
            self.edit();
            self.host.observe(&mut self.ws, self.now);
            assert_eq!(self.host.next_wake(), Some(self.now + RECOVERY_INTERVAL));
            self.now += RECOVERY_INTERVAL;
            self.host.observe(&mut self.ws, self.now);
            assert_eq!(self.host.presentation(self.ws.active()).status, "Saving recovery copy…");
            self.complete();
            assert!(self.ws.active().unwrap().is_dirty_exact());
            let s = self.ws.active().unwrap();
            let blob = self.host.store.as_ref().unwrap().load_best(&s.recovery.rid).unwrap().blob;
            let copy = varos_core::file::doc_from_blob(std::str::from_utf8(&blob).unwrap()).unwrap();
            assert_eq!(copy, s.editor.doc);
            s.recovery.rid.clone()
        }
    }
    impl Drop for Rig {
        fn drop(&mut self) {
            self.host.shutdown();
            let _ = std::fs::remove_dir_all(&self.layout.root);
        }
    }

    #[test]
    fn snapshot_never_touches_source_save_retires_and_later_edit_is_protected() {
        let mut r = Rig::new();
        let source = r.layout.root.join("original.vrs");
        std::fs::write(&source, b"original bytes").unwrap();
        r.ws.active_mut().unwrap().path = Some(source.clone());
        let rid = r.snapshot();
        assert_eq!(std::fs::read(&source).unwrap(), b"original bytes");
        let s = r.ws.active_mut().unwrap();
        s.mark_saved(source.clone(), crate::file_ports::file_key(&source));
        r.host.observe(&mut r.ws, r.now);
        r.complete();
        assert!(!r.layout.recovery().join(&rid).exists());
        assert_ne!(r.snapshot(), rid);
    }

    #[test]
    fn save_with_recovery_switched_off_still_retires_the_sessions_copies() {
        let mut r = Rig::new();
        let source = r.layout.root.join("original.vrs");
        std::fs::write(&source, b"original bytes").unwrap();
        r.ws.active_mut().unwrap().path = Some(source.clone());
        let rid = r.snapshot();
        r.host.handle(&AppCommand::SetRecoveryEnabled(false), &mut r.ws, r.now);
        r.complete(); // the settings write
        assert!(r.layout.recovery().join(&rid).exists(), "switching off keeps existing copies");
        let s = r.ws.active_mut().unwrap();
        s.mark_saved(source.clone(), crate::file_ports::file_key(&source));
        r.host.observe(&mut r.ws, r.now);
        r.complete();
        assert!(!r.layout.recovery().join(&rid).exists(), "a clean save retires the stale copy");
        r.edit();
        r.now += RECOVERY_INTERVAL;
        r.host.observe(&mut r.ws, r.now);
        assert!(r.ws.active().unwrap().recovery.in_flight.is_none(), "but no NEW copy while off");
    }

    #[test]
    fn unreadable_settings_switch_applies_for_the_session_and_says_it_was_not_saved() {
        let layout = AppLayout { root: std::env::temp_dir().join(format!("varos-f1-{}", new_nonce())) };
        std::fs::create_dir_all(layout.settings().parent().unwrap()).unwrap();
        std::fs::write(layout.settings(), br#"{"version":99,"recovery_enabled":true}"#).unwrap();
        let mut host = RecoveryHost::at(Some(layout.clone()), Box::new(|| {}));
        assert!(host.warning.is_some());
        let mut ws = Workspace::new();
        let mut dialogs = Dialog::default();
        let cmd = AppCommand::SetRecoveryEnabled(false);
        assert!(!host.handle_read(&cmd, &mut dialogs) && host.handle(&cmd, &mut ws, Instant::now()));
        assert!(!host.presentation(ws.active()).enabled, "the switch still applies to this session");
        assert_eq!(dialogs.notices.len(), 1);
        assert!(dialogs.notices[0].starts_with("Recovery setting could not be saved: "), "{:?}", dialogs.notices);
        assert_eq!(
            std::fs::read(layout.settings()).unwrap(),
            br#"{"version":99,"recovery_enabled":true}"#,
            "the unreadable file is not overwritten"
        );
        host.shutdown();
        let _ = std::fs::remove_dir_all(&layout.root);
    }

    #[test]
    fn canceled_quit_keeps_copy_committed_discard_and_quit_retire_even_when_disabled() {
        for quit in [false, true] {
            let mut r = Rig::new();
            let rid = r.snapshot();
            let before = RecoveryHost::before_close(&r.ws);
            r.host.after_dispatch(before, &mut r.ws, false, r.now); // canceled quit: no tabs removed
            assert!(r.layout.recovery().join(&rid).exists());
            r.host.handle(&AppCommand::SetRecoveryEnabled(false), &mut r.ws, r.now);
            r.complete();
            assert!(r.layout.recovery().join(&rid).exists());
            assert!(!settings::load(&RealFs, &r.layout.settings()).0.recovery_enabled);
            let protected_rev = r.ws.active().unwrap().recovery.last_snapshot_rev;
            r.edit();
            r.now += RECOVERY_INTERVAL;
            r.host.observe(&mut r.ws, r.now);
            assert_eq!(r.host.next_wake(), None);
            assert_eq!(r.ws.active().unwrap().recovery.last_snapshot_rev, protected_rev);
            assert!(r.ws.active().unwrap().recovery.in_flight.is_none());
            let before = RecoveryHost::before_close(&r.ws);
            if !quit {
                r.ws.remove(r.ws.active_id().unwrap());
            }
            r.host.after_dispatch(before, &mut r.ws, quit, r.now);
            r.host.shutdown();
            assert!(!r.layout.recovery().join(&rid).exists());
        }
    }

    #[test]
    fn open_transaction_waits_and_failed_copy_offers_retry_then_recovers() {
        use varos_app::storage::durable::{Fault, FaultFs, Step};
        let mut r = Rig::new();
        r.host.store = Some(Arc::new(
            RecoveryStore::open(Arc::new(FaultFs::new(vec![Fault::at(Step::Rename)])), r.layout.recovery()).unwrap(),
        ));
        r.edit();
        r.host.observe(&mut r.ws, r.now);
        r.ws.active_mut().unwrap().editor.begin();
        r.now += RECOVERY_INTERVAL;
        r.host.observe(&mut r.ws, r.now);
        assert_eq!(r.host.presentation(r.ws.active()).status, "Recovery waiting for edit to finish");
        assert_eq!(r.host.next_wake(), None);
        r.ws.active_mut().unwrap().editor.commit();
        r.host.observe(&mut r.ws, r.now);
        r.complete();
        let ui = r.host.presentation(r.ws.active());
        assert!(ui.retry);
        assert_eq!(ui.status, "Recovery copy couldn't be saved. Save your document now.");
        let id = r.ws.active_id().unwrap();
        r.host.handle(&AppCommand::RetryRecovery(id), &mut r.ws, r.now);
        r.host.observe(&mut r.ws, r.now);
        r.complete();
        assert!(!r.host.presentation(r.ws.active()).retry);
        assert!(r.ws.active().unwrap().recovery.last_ok.is_some());
    }

    #[test]
    fn closing_with_snapshot_in_flight_retires_after_the_writer_finishes() {
        let mut r = Rig::new();
        r.edit();
        r.host.observe(&mut r.ws, r.now);
        r.now += RECOVERY_INTERVAL;
        r.host.observe(&mut r.ws, r.now);
        let s = r.ws.active().unwrap();
        let (id, rid) = (s.id, s.recovery.rid.clone());
        assert!(s.recovery.in_flight.is_some());
        let before = RecoveryHost::before_close(&r.ws);
        r.ws.remove(id);
        r.host.after_dispatch(before, &mut r.ws, false, r.now);
        r.host.shutdown();
        assert!(!r.layout.recovery().join(rid).exists());
    }
    #[derive(Default)]
    struct Dialog {
        discard: bool,
        prompts: usize,
        save: Option<PathBuf>,
        suggestion: Option<(String, Option<PathBuf>)>,
        notices: Vec<String>,
    }
    impl crate::lifecycle::Dialogs for Dialog {
        fn confirm_discard_recovery(&mut self, _: &str) -> bool {
            self.prompts += 1;
            self.discard
        }
        fn pick_open(&mut self) -> Vec<PathBuf> {
            panic!("unexpected open")
        }
        fn pick_save(&mut self, name: &str, dir: Option<&std::path::Path>) -> Option<PathBuf> {
            self.suggestion = Some((name.into(), dir.map(std::path::Path::to_path_buf)));
            self.save.clone()
        }
        fn ask_save_changes(&mut self, _: &str, _: Option<(usize, usize)>) -> crate::lifecycle::SaveDecision {
            crate::lifecycle::SaveDecision::Cancel
        }
        fn save_failed(&mut self, _: &str, reason: &str) -> crate::lifecycle::SaveFailChoice {
            panic!("unexpected save error: {reason}")
        }
        fn open_failed(&mut self, _: &str, _: &str) {
            panic!("unexpected open error")
        }
        fn confirm_replace(&mut self, _: &str) -> bool {
            false
        }
        fn notice(&mut self, _: &str, body: &str) {
            self.notices.push(body.into());
        }
    }
    impl Rig {
        fn seed(&self, generations: u64, original: Option<PathBuf>) -> String {
            let store = RecoveryStore::open(Arc::new(RealFs), self.layout.recovery()).unwrap();
            let rid = varos_app::storage::recovery::fresh_rid();
            let meta = SessionMeta {
                rid: rid.clone(),
                display_name: "Logo.vrs".into(),
                original_path: original,
                ..Default::default()
            };
            let blob = varos_core::file::doc_to_blob(&varos_core::model::Document::default()).unwrap();
            for seq in 1..=generations {
                store.write_generation(&meta, seq, blob.as_bytes(), 100 + seq).unwrap();
            }
            rid // dropping this store releases the simulated crashed process's claim
        }
        fn scan(&mut self) {
            self.host.begin_scan();
            self.complete();
        }
        fn recover(&mut self, rid: &str) -> SessionId {
            let mut dialogs = Dialog::default();
            self.host.handle_read(&AppCommand::Recover(rid.into()), &mut dialogs);
            assert!(self.host.rows().iter().any(|row| row.rid == rid && row.busy));
            // A second click during the job cannot queue a second installation.
            self.host.handle_read(&AppCommand::Recover(rid.into()), &mut dialogs);
            self.complete();
            let mut copies = self.host.take_recovered();
            assert_eq!(copies.len(), 1);
            let copy = copies.pop().unwrap();
            crate::lifecycle::Lifecycle {
                ws: &mut self.ws,
                dialogs: &mut dialogs,
                store: &mut crate::file_ports::DiskStore,
                jobs: None,
            }
            .run(AppCommand::InstallRecovered(Box::new(copy)));
            self.ws.active_id().unwrap()
        }
    }

    #[test]
    fn launch_claims_orphans_recovery_is_pathless_dirty_and_never_writes_original() {
        let mut r = Rig::new();
        let original = r.layout.root.join("Logo.vrs");
        std::fs::write(&original, b"the original stays intact").unwrap();
        let before = std::fs::metadata(&original).unwrap().modified().unwrap();
        let rid = r.seed(1, Some(original.clone()));
        r.scan();
        assert_eq!(r.host.rows().len(), 1);
        assert!(r.host.presentation(r.ws.active()).banner);
        let other = RecoveryStore::open(Arc::new(RealFs), r.layout.recovery()).unwrap();
        assert!(other.scan().is_empty(), "another process cannot claim these copies");
        let id = r.recover(&rid);
        let s = r.ws.get(id).unwrap();
        assert!(s.path.is_none() && s.key.is_none() && s.is_dirty_exact() && !s.is_pristine());
        assert_eq!(s.display_name(), "Logo.vrs (Recovered)");
        assert_eq!(s.recovery.rid, rid);
        assert!(r.host.rows().is_empty());
        assert!(r
            .host
            .presentation(Some(s))
            .recovered_notice
            .unwrap()
            .contains("Your original file has not been changed"));
        assert_eq!(std::fs::read(&original).unwrap(), b"the original stays intact");
        assert_eq!(std::fs::metadata(&original).unwrap().modified().unwrap(), before);
        // Merely opening even an empty recovery copy must never retire it.
        r.host.observe(&mut r.ws, r.now);
        assert!(r.layout.recovery().join(&rid).exists());
        assert!(r.ws.active().unwrap().recovery.in_flight.is_none());
    }

    #[test]
    fn fallback_is_explained_and_new_edits_keep_the_claimed_session() {
        let mut r = Rig::new();
        let rid = r.seed(2, None);
        std::fs::write(r.layout.recovery().join(&rid).join("snap-2.json"), b"damaged").unwrap();
        r.scan();
        r.recover(&rid);
        assert!(r.host.presentation(r.ws.active()).recovered_notice.unwrap().contains("previous copy was used"));
        assert_eq!(r.ws.active().unwrap().recovery.last_ok.as_ref().unwrap().seq, 1);
        r.edit();
        r.host.observe(&mut r.ws, r.now);
        r.now += RECOVERY_INTERVAL;
        r.host.observe(&mut r.ws, r.now);
        r.complete();
        assert_eq!(r.ws.active().unwrap().recovery.rid, rid);
        assert!(r.ws.active().unwrap().recovery.last_ok.as_ref().unwrap().seq > 2);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(r.layout.recovery().join(&rid).join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["recovered"], true);
        assert_eq!(manifest["display_name"], "Logo.vrs");
    }

    #[test]
    fn recovered_save_as_suggests_safe_name_and_retires_only_after_durable_save() {
        let mut r = Rig::new();
        let original = r.layout.root.join("Logo.vrs");
        std::fs::write(&original, b"original").unwrap();
        let rid = r.seed(1, Some(original.clone()));
        r.scan();
        let id = r.recover(&rid);
        let mut dialog = Dialog::default();
        crate::lifecycle::Lifecycle {
            ws: &mut r.ws,
            dialogs: &mut dialog,
            store: &mut crate::file_ports::DiskStore,
            jobs: None,
        }
        .run(AppCommand::Save(id));
        assert_eq!(dialog.suggestion, Some(("Logo-recovered.vrs".into(), Some(r.layout.root.clone()))));
        assert!(r.ws.get(id).unwrap().is_dirty_exact());
        let dest = r.layout.root.join("Logo-recovered.vrs");
        dialog.save = Some(dest.clone());
        crate::lifecycle::Lifecycle {
            ws: &mut r.ws,
            dialogs: &mut dialog,
            store: &mut crate::file_ports::DiskStore,
            jobs: None,
        }
        .run(AppCommand::Save(id));
        assert!(!r.ws.get(id).unwrap().is_dirty_exact());
        assert!(r.ws.get(id).unwrap().recovered.is_none());
        r.host.observe(&mut r.ws, r.now);
        r.complete();
        assert!(!r.layout.recovery().join(rid).exists());
        assert!(varos_pdf::load_vrs(&dest).is_ok());
        assert_eq!(std::fs::read(original).unwrap(), b"original");
    }

    #[test]
    fn later_retains_copies_and_relaunch_offers_again() {
        let mut r = Rig::new();
        let rid = r.seed(1, None);
        r.scan();
        r.host.handle_read(&AppCommand::DeferRecovery, &mut Dialog::default());
        assert!(!r.host.presentation(r.ws.active()).banner);
        assert_eq!(r.host.rows().len(), 1, "Home still owns the choices");
        let before = RecoveryHost::before_close(&r.ws);
        r.host.after_dispatch(before, &mut r.ws, true, r.now);
        r.host.shutdown();
        r.host.store.take();
        assert!(r.layout.recovery().join(rid).exists());
        let (tx, rx) = mpsc::channel();
        r.host = RecoveryHost::at(
            Some(r.layout.clone()),
            Box::new(move || {
                let _ = tx.send(());
            }),
        );
        r.wake = rx;
        r.scan();
        assert_eq!(r.host.rows().len(), 1);
        assert!(r.host.presentation(r.ws.active()).banner);
    }

    #[test]
    fn damaged_and_newer_copies_stay_listed_and_discard_requires_confirmation() {
        let mut r = Rig::new();
        let damaged = r.seed(1, None);
        let newer = r.seed(1, None);
        std::fs::write(r.layout.recovery().join(&damaged).join("snap-1.json"), b"bad").unwrap();
        std::fs::write(r.layout.recovery().join(&newer).join("manifest.json"), br#"{"manifest_version":999}"#).unwrap();
        r.scan();
        assert_eq!(r.host.rows().len(), 2);
        assert!(r.host.rows().iter().all(|row| row.problem.is_some()));
        let mut dialog = Dialog::default();
        r.host.handle_read(&AppCommand::Recover(damaged.clone()), &mut dialog);
        assert!(r.host.take_recovered().is_empty());
        r.host.handle_read(&AppCommand::DiscardRecovery(damaged.clone()), &mut dialog);
        assert_eq!(dialog.prompts, 1);
        assert_eq!(r.host.rows().len(), 2);
        assert!(r.layout.recovery().join(&damaged).exists());
        dialog.discard = true;
        r.host.handle_read(&AppCommand::DiscardRecovery(damaged.clone()), &mut dialog);
        r.complete();
        assert_eq!(r.host.rows().len(), 1);
        assert!(!r.layout.recovery().join(damaged).exists());
        assert!(r.layout.recovery().join(newer).exists());
    }

    #[test]
    fn failed_discard_keeps_row_and_bytes_with_reason() {
        use varos_app::storage::durable::{Fault, FaultFs, Step};
        let mut r = Rig::new();
        let rid = r.seed(1, None);
        r.host.store = Some(Arc::new(
            RecoveryStore::open(Arc::new(FaultFs::new(vec![Fault::at(Step::Rename)])), r.layout.recovery()).unwrap(),
        ));
        r.scan();
        let before = std::fs::read(r.layout.recovery().join(&rid).join("snap-1.json")).unwrap();
        let mut dialog = Dialog { discard: true, ..Default::default() };
        r.host.handle_read(&AppCommand::DiscardRecovery(rid.clone()), &mut dialog);
        r.complete();
        assert_eq!(r.host.rows().len(), 1);
        assert!(r.host.rows()[0].problem.is_some());
        assert_eq!(std::fs::read(r.layout.recovery().join(rid).join("snap-1.json")).unwrap(), before);
    }

    #[test]
    fn unavailable_storage_does_not_block_new_documents() {
        let host = RecoveryHost::at(None, Box::new(|| {}));
        let mut ws = Workspace::new();
        ws.new_untitled();
        assert!(host.rows().is_empty());
        assert!(host.start_warning(None).unwrap().contains("unavailable"));
        assert!(host.presentation(ws.active()).status.contains("unavailable"));
        assert!(ws.active().is_some());
    }
    #[test]
    fn valid_checksum_invalid_model_falls_back_or_stays_listed_with_reason() {
        for has_previous in [false, true] {
            let mut r = Rig::new();
            let rid = if has_previous { r.seed(1, None) } else { varos_app::storage::recovery::fresh_rid() };
            {
                let store = RecoveryStore::open(Arc::new(RealFs), r.layout.recovery()).unwrap();
                let meta = SessionMeta { rid: rid.clone(), display_name: "Logo.vrs".into(), ..Default::default() };
                store.write_generation(&meta, 2, br#"{"varos":2,"doc":{"paths":"invalid"}}"#, 200).unwrap();
            }
            r.scan();
            r.host.handle_read(&AppCommand::Recover(rid.clone()), &mut Dialog::default());
            r.complete();
            let loaded = r.host.take_recovered();
            if has_previous {
                assert_eq!(loaded.len(), 1);
                assert!(loaded[0].source.fell_back);
                assert_eq!(loaded[0].generation.seq, 1);
            } else {
                assert!(loaded.is_empty());
                assert_eq!(r.host.rows().len(), 1);
                assert!(r.host.rows()[0].problem.is_some());
            }
            assert!(r.layout.recovery().join(rid).exists());
        }
    }

    /// A two-board document and an export job for it to `dest` (all visible boards).
    fn export_job(dest: PathBuf) -> crate::file_jobs::FileJob {
        let mut doc = varos_core::model::Document::default();
        for x in [0.0, 300.0] {
            doc.artboards.push(varos_core::model::Artboard {
                x,
                y: 0.0,
                w: 200.0,
                h: 100.0,
                name: "B".into(),
                ..Default::default()
            });
        }
        let plan = varos_pdf::plan_pdf_export(&doc, varos_pdf::ExportScope::AllVisibleArtboards).unwrap();
        crate::file_jobs::FileJob::Export(crate::file_jobs::ExportJob {
            sid: SessionId(1),
            dest,
            doc: Arc::new(doc),
            plan,
            replace_confirmed: false,
        })
    }

    #[test]
    fn export_runs_on_the_shared_worker_even_without_recovery_storage() {
        use crate::host::FileJobs;
        let dir = std::env::temp_dir().join(format!("varos-s6-{}", new_nonce()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("Logo.pdf");
        let mut host = RecoveryHost::at(None, Box::new(|| {}));
        assert!(
            FileJobs::submit(&mut host, export_job(dest.clone())).is_ok(),
            "the worker exists without a data folder"
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut got = host.take_file_done();
        while got.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            got = host.take_file_done();
        }
        assert_eq!(got.len(), 1, "the export's result arrives through the worker");
        let done = got.remove(0);
        assert!(
            matches!(&done, crate::file_jobs::FileDone::Exported(d) if d.result == crate::file_jobs::ExportResult::Exported),
            "{done:?}"
        );
        let bytes = std::fs::read(&dest).unwrap();
        assert_eq!(lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(), 2, "one page per board");
        assert!(!varos_pdf::has_embedded_model(&bytes));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quit_waits_for_an_in_flight_export() {
        use crate::host::FileJobs;
        let dir = std::env::temp_dir().join(format!("varos-s6-{}", new_nonce()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("Late.pdf");
        let mut host = RecoveryHost::at(None, Box::new(|| {}));
        // a slow job ahead keeps the export queued while Quit begins
        let (release, gate) = mpsc::channel::<()>();
        host.worker
            .as_ref()
            .unwrap()
            .submit(
                Box::new(move || {
                    let _ = gate.recv_timeout(Duration::from_secs(10));
                    Finished::ScanFailed
                }),
                Finished::ScanFailed,
            )
            .unwrap();
        FileJobs::submit(&mut host, export_job(dest.clone())).unwrap();
        let opener = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let _ = release.send(());
        });
        assert!(!dest.exists(), "still queued when Quit starts");
        host.shutdown();
        opener.join().unwrap();
        let bytes = std::fs::read(&dest).expect("Quit waited for the export to finish");
        assert_eq!(lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
