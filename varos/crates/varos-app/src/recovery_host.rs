//! F1: host-owned recovery coordination. The worker only receives owned documents and metadata.
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
    recovery::{RecoveryStore, SessionMeta},
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
}
enum Finished {
    Recovery(Completion<SessionId>),
    Settings(Result<(), String>),
}
pub struct RecoveryHost {
    scheduler: Scheduler,
    worker: Option<IoWorker<Finished>>,
    store: Option<Arc<RecoveryStore>>,
    settings_path: Option<PathBuf>,
    pub warning: Option<String>,
}
impl RecoveryHost {
    pub fn new(wake: Box<dyn Fn() + Send>) -> Self {
        Self::at(paths::data_root().map(|root| AppLayout { root }), wake)
    }
    fn at(layout: Option<AppLayout>, wake: Box<dyn Fn() + Send>) -> Self {
        let mut host =
            Self { scheduler: Scheduler::default(), worker: None, store: None, settings_path: None, warning: None };
        let Some(layout) = layout else {
            host.warning = Some("Recovery unavailable: the app data folder is unavailable.".into());
            return host;
        };
        let (settings, warning) = settings::load(&RealFs, &layout.settings());
        host.scheduler.set_enabled(settings.recovery_enabled);
        if warning.is_none() {
            host.settings_path = Some(layout.settings());
        }
        host.warning = warning;
        match RecoveryStore::open(Arc::new(RealFs), layout.recovery()) {
            Ok(store) => host.store = Some(Arc::new(store)),
            Err(e) => {
                host.warning = Some(e.reason());
                return host;
            }
        }
        match IoWorker::spawn(wake) {
            Ok(worker) => host.worker = Some(worker),
            Err(e) => host.warning = Some(format!("Recovery writer unavailable: {e}")),
        }
        host
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
        if let Some(worker) = &self.worker {
            for done in worker.try_completions() {
                match done {
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
                    display_name: s.display_name(),
                    untitled_number: s.untitled,
                    original_path: s.path.clone(),
                    source_fingerprint: s.source_fingerprint,
                    recovered: false,
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
        let mut ui = RecoveryUi { enabled: self.scheduler.enabled(), sid: session.map(|s| s.id), ..Default::default() };
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
}
