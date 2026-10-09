//! Owning-thread capture and completion policy. No Recent side effects, no forced settlement.
use crate::{
    app_command::SessionId,
    autosave_io,
    file_jobs::{next_ticket, SaveInFlight},
    lifecycle::{Effect, SaveOutcome},
    workspace::Workspace,
};
use std::{sync::Arc, time::Instant};
use varos_app::storage::{autosave::Probe, publication::Gate, settings::Settings};

pub fn editor_busy(ed: &varos_core::editor::Editor) -> bool {
    ed.transaction_open()
        || ed.origin_preview.is_some()
        || ed.guide_preview.is_some()
        || (ed.tool == varos_core::editor::ToolKind::Pen && ed.active.is_some())
        || !matches!(ed.drag, varos_core::editor::Drag::None)
        || !matches!(ed.ab_drag, varos_core::editor::AbDrag::None)
}

pub fn observe(
    ws: &mut Workspace,
    now: Instant,
    settings: Settings,
    blocked: bool,
    gate: &Arc<Gate>,
) -> (Option<Instant>, Option<autosave_io::Job>) {
    let busy = blocked
        || ws.sessions().iter().any(|s| s.saving.is_some() || !s.exports.is_empty() || s.recovery.in_flight.is_some());
    let mut wake = None;
    let mut captured = None;
    for s in ws.sessions_mut() {
        let ed = &s.editor;
        let probe = Probe {
            revision: ed.rev,
            dirty: s.is_dirty(),
            path: s.path.is_some() && s.source_fingerprint.is_some(),
            transaction: ed.transaction_open(),
            gesture: editor_busy(ed),
            file_job: busy || captured.is_some(),
            read_only: s.save_unconfirmed,
            ..Default::default()
        };
        if let Some(at) = s.autosave.observe(now, settings, probe) {
            if at <= now {
                let (Some(dest), Some(expected)) = (s.path.clone(), s.source_fingerprint) else { continue };
                let doc = Arc::new(s.editor.doc.clone());
                let ticket = next_ticket();
                s.saving =
                    Some(SaveInFlight { ticket, dest: dest.clone(), doc: doc.clone(), follow_up: false, started: now });
                s.autosave.ticket = Some(ticket);
                s.autosave.status = "Autosaving…".into();
                captured = Some(autosave_io::Job { sid: s.id, ticket, dest, doc, expected, permit: gate.capture() });
                continue;
            }
            wake = Some(wake.map_or(at, |old: Instant| old.min(at)));
        }
    }
    (wake, captured)
}
pub fn complete(ws: &mut Workspace, done: autosave_io::Done, now: Instant) -> Effect {
    let mut effect = Effect::default();
    let Some(s) = ws.get_mut(done.sid) else {
        return effect;
    };
    if s.autosave.ticket != Some(done.ticket) {
        return effect;
    }
    let Some(flight) = s.saving.take_if(|f| f.ticket == done.ticket && f.dest == done.dest) else { return effect };
    s.autosave.ticket = None;
    match done.result {
        Ok((SaveOutcome::Durable, Some(baseline))) => {
            let mut key = s.key.clone().unwrap_or_else(|| crate::workspace::FileKey {
                path: done.dest.clone(),
                dev_ino: None,
                name_id: None,
            });
            key.dev_ino = baseline.identity;
            s.mark_saved_snapshot(done.dest, key, Arc::unwrap_or_clone(flight.doc));
            s.source_fingerprint = Some(baseline);
            s.recovery.retain_after_autosave = true;
            s.autosave.status =
                format!("Autosaved {}", varos_app::storage::time_text::clock_hhmm(crate::recovery_host::unix_now()));
            if !s.is_dirty_exact() {
                s.autosave.reset();
            }
        }
        Ok((_, baseline)) => {
            // The replacement is ours even when directory sync could not confirm durability.
            // Keep the old content checkpoint, but do not mislabel our bytes as another app's edit.
            if baseline.is_some() {
                s.source_fingerprint = baseline;
            }
            s.save_unconfirmed = true;
            s.autosave.paused = true;
            s.autosave.status = "Save needs confirmation".into();
            s.autosave.confirmation = true;
        }
        Err(reason) if reason == "conflict" => {
            s.autosave.paused = true;
            s.autosave.conflict = true;
            s.autosave.status = "Autosave paused — file changed in another app".into();
        }
        Err(reason) => {
            s.autosave.status = if reason == "superseded" || reason == "busy" {
                String::new()
            } else {
                format!("Couldn't autosave — {reason}")
            };
            s.autosave.backoff(now);
        }
    }
    if flight.follow_up {
        effect.follow_up_saves.push(s.id);
    }
    effect
}
pub fn pending_conflict(ws: &mut Workspace) -> Option<SessionId> {
    let s = ws.sessions_mut().iter_mut().find(|s| s.autosave.conflict)?;
    s.autosave.conflict = false;
    Some(s.id)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::lifecycle::DocStore;
    use varos_app::storage::{
        checksum::new_nonce,
        durable::{fingerprint, RealFs},
    };
    fn setup() -> (std::path::PathBuf, Workspace, SessionId, Arc<Gate>) {
        let root = std::env::temp_dir().join(format!("auto-host-{}", new_nonce()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let path = root.join("board.vrs");
        let doc = varos_core::board::new_board();
        crate::file_ports::DiskStore.save(&doc, &path).unwrap();
        let mut ws = Workspace::new();
        let id = ws.add_loaded(doc, path.clone(), crate::file_ports::file_key(&path));
        ws.get_mut(id).unwrap().source_fingerprint = fingerprint(&RealFs, &path);
        edit(&mut ws, id, "first");
        (root, ws, id, Arc::new(Gate::default()))
    }
    fn edit(ws: &mut Workspace, id: SessionId, value: &str) {
        let ed = &mut ws.get_mut(id).unwrap().editor;
        ed.begin();
        ed.doc.description = value.into();
        ed.dirty = true;
        ed.commit();
    }
    fn due(ws: &mut Workspace, gate: &Arc<Gate>, t: Instant) -> autosave_io::Job {
        let settings = Settings::default();
        assert!(observe(ws, t, settings, false, gate).1.is_none());
        observe(ws, t + std::time::Duration::from_secs(120), settings, false, gate).1.unwrap()
    }
    #[test]
    fn durable_checkpoint_retains_history_and_recovery() {
        let (root, mut ws, id, gate) = setup();
        let t = Instant::now();
        let job = due(&mut ws, &gate, t);
        let saved = job.doc.clone();
        complete(&mut ws, job.run(), t);
        let s = ws.get_mut(id).unwrap();
        assert!(!s.is_dirty_exact());
        assert!(s.recovery.retain_after_autosave);
        s.editor.undo();
        assert!(s.is_dirty_exact());
        s.editor.redo();
        assert!(!s.is_dirty_exact());
        assert_eq!(&s.editor.doc, &*saved);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn contention_and_superseded_completion_defer_expired_capture() {
        for invalidated in [false, true] {
            let (root, mut ws, id, gate) = setup();
            let t = Instant::now();
            let job = due(&mut ws, &gate, t);
            let before = std::fs::read(&job.dest).unwrap();
            let lock = gate.lock.lock().unwrap();
            if invalidated {
                gate.invalidate();
            }
            let done = job.run();
            assert_eq!(done.result, Err(if invalidated { "superseded" } else { "busy" }.into()));
            drop(lock);
            let now = t + std::time::Duration::from_secs(120);
            complete(&mut ws, done, now);
            for offset in [0, 1, 29] {
                let (wake, job) =
                    observe(&mut ws, now + std::time::Duration::from_secs(offset), Settings::default(), false, &gate);
                assert!(job.is_none());
                assert_eq!(wake, Some(now + std::time::Duration::from_secs(30)));
            }
            assert_eq!(std::fs::read(ws.get(id).unwrap().path.as_ref().unwrap()).unwrap(), before);
            let job = observe(&mut ws, now + std::time::Duration::from_secs(30), Settings::default(), false, &gate)
                .1
                .unwrap();
            complete(&mut ws, job.run(), now);
            assert!(!ws.get(id).unwrap().is_dirty_exact());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn later_edits_and_stale_tickets_cannot_claim_clean() {
        let (root, mut ws, id, gate) = setup();
        let t = Instant::now();
        let job = due(&mut ws, &gate, t);
        let done = job.run();
        edit(&mut ws, id, "later");
        let mut stale = done.clone();
        stale.ticket += 1;
        complete(&mut ws, stale, t);
        assert!(ws.get(id).unwrap().saving.is_some());
        complete(&mut ws, done, t);
        assert!(ws.get(id).unwrap().is_dirty_exact());
        assert!(ws.get(id).unwrap().saving.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn worker_conflict_latches_once_and_preserves_dirty() {
        let (root, mut ws, id, gate) = setup();
        let t = Instant::now();
        let job = due(&mut ws, &gate, t);
        std::fs::write(&job.dest, b"external").unwrap();
        complete(&mut ws, job.run(), t);
        assert_eq!(pending_conflict(&mut ws), Some(id));
        assert_eq!(pending_conflict(&mut ws), None);
        assert!(ws.get(id).unwrap().is_dirty_exact());
        assert!(observe(&mut ws, t + std::time::Duration::from_secs(600), Settings::default(), false, &gate)
            .0
            .is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn blocked_capture_waits_for_event_and_settings_restart_interval() {
        let (root, mut ws, id, gate) = setup();
        let t = Instant::now();
        let settings = Settings::default();
        observe(&mut ws, t, settings, false, &gate);
        assert!(observe(&mut ws, t + std::time::Duration::from_secs(120), settings, true, &gate).1.is_none());
        ws.get_mut(id).unwrap().autosave.reset();
        assert_eq!(
            observe(&mut ws, t + std::time::Duration::from_secs(121), settings, false, &gate).0,
            Some(t + std::time::Duration::from_secs(241))
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn background_tabs_keep_independent_deadlines_and_recovery_has_priority() {
        let (root, mut ws, id, gate) = setup();
        let t = Instant::now();
        let settings = Settings::default();
        let path = root.join("second.vrs");
        let doc = varos_core::board::new_board();
        crate::file_ports::DiskStore.save(&doc, &path).unwrap();
        let second = ws.add_loaded(doc, path.clone(), crate::file_ports::file_key(&path));
        ws.get_mut(second).unwrap().source_fingerprint = fingerprint(&RealFs, &path);
        observe(&mut ws, t, settings, false, &gate);
        edit(&mut ws, second, "second tab");
        observe(&mut ws, t + std::time::Duration::from_secs(60), settings, false, &gate);
        let (_, job) = observe(&mut ws, t + std::time::Duration::from_secs(120), settings, false, &gate);
        let job = job.unwrap();
        assert_eq!(job.sid, id);
        assert_eq!(ws.get(second).unwrap().autosave.deadline, Some(t + std::time::Duration::from_secs(180)));
        complete(&mut ws, job.run(), t + std::time::Duration::from_secs(120));
        let mut scheduler = varos_app::storage::scheduler::Scheduler::default();
        let s = ws.get_mut(second).unwrap();
        let rev = s.editor.rev;
        let start = t + std::time::Duration::from_secs(140);
        scheduler.observe(
            start,
            [varos_app::storage::scheduler::Probe {
                sid: second,
                rev,
                clean: false,
                transaction_open: false,
                recovery: &mut s.recovery,
            }],
        );
        scheduler.observe(
            start + std::time::Duration::from_secs(30),
            [varos_app::storage::scheduler::Probe {
                sid: second,
                rev,
                clean: false,
                transaction_open: false,
                recovery: &mut s.recovery,
            }],
        );
        assert!(s.recovery.in_flight.is_some());
        assert!(observe(&mut ws, t + std::time::Duration::from_secs(180), settings, false, &gate).1.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failures_and_unconfirmed_replacement_never_move_checkpoint() {
        for result in [Err("disk full".into()), Ok((SaveOutcome::ReplacedUnconfirmed("sync failed".into()), None))] {
            let (root, mut ws, id, gate) = setup();
            let t = Instant::now();
            let job = due(&mut ws, &gate, t);
            let before = std::fs::read(&job.dest).unwrap();
            complete(
                &mut ws,
                autosave_io::Done { sid: id, ticket: job.ticket, dest: job.dest.clone(), result: result.clone() },
                t,
            );
            let s = ws.get(id).unwrap();
            assert!(s.is_dirty_exact());
            assert!(s.saving.is_none());
            if result.is_ok() {
                assert!(s.save_unconfirmed);
                assert!(s.autosave.paused);
                assert!(s.autosave.confirmation);
            } else {
                assert_eq!(s.autosave.deadline, Some(t + std::time::Duration::from_secs(120)));
            }
            assert_eq!(std::fs::read(&job.dest).unwrap(), before);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn bridge_atomic_commit_resets_idle_rejected_batch_does_not() {
        struct Ui;
        impl crate::host::DocUi for Ui {
            fn settle(&mut self, _: &mut varos_core::editor::Editor) -> bool {
                true
            }
            fn document_switched(&mut self) {}
        }
        let (root, mut ws, id, gate) = setup();
        let t = Instant::now();
        let ed = &mut ws.get_mut(id).unwrap().editor;
        ed.begin();
        let anchors = [[0., 0.], [10., 0.], [10., 10.], [0., 10.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| varos_core::model::Anchor { id: 11 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect();
        ed.doc.paths.push(varos_core::model::Path::new(10, anchors, true, None, None, 1.));
        ed.doc.ids = 15;
        ed.dirty = true;
        ed.commit();
        crate::bridge_host::initialize("auto-test".into());
        let settings = Settings::default();
        observe(&mut ws, t, settings, false, &gate);
        for (index, path, ok) in [(0, "path:10", true), (1, "path:999", false)] {
            let rev = ws.get(id).unwrap().editor.rev;
            let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"api":"1.0","request_id":format!("r{}",index+1),"board":format!("b{}",id.0),"expected_rev":rev,"ops":[{"verb":"move","ids":[path],"delta":[10,0]}]})).unwrap();
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            crate::bridge_host::run(
                varos_bridge::ipc::Pending {
                    context: varos_bridge::Context { client: "test".into(), epoch: "auto-test".into() },
                    request,
                    cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                    reply: tx,
                    file_audit: None,
                },
                &mut ws,
                &mut Ui,
            );
            let reply = rx.recv().unwrap();
            assert_eq!(reply.ok, ok, "{reply:?}");
            observe(&mut ws, t + std::time::Duration::from_secs(10 + index), settings, false, &gate);
            assert_eq!(ws.get(id).unwrap().autosave.deadline, Some(t + std::time::Duration::from_secs(130)));
            assert!(!ws.get(id).unwrap().editor.transaction_open());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
