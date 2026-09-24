//! Recovery-copy scheduler (work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3.6, piece D).
//!
//! Pure: no clock, no I/O, no threads. The host passes `now: Instant` in, runs the returned
//! [`Action`]s as jobs on the [`super::io_worker::IoWorker`], and feeds each job's [`Completion`]
//! back through [`Scheduler::on_complete`]. Per-session state lives in [`SessionRecovery`], which the
//! host embeds in each document session (so a closed session's state disappears with it).
//!
//! Rules (one session; sessions never affect each other):
//! - The first content change since the last successful copy sets `next_deadline = now + 30 s`.
//!   Later edits **do not** move it. The change signal is the editor's `rev` (compared with `==`
//!   only) together with `clean = !session.is_dirty()`.
//! - At the deadline: a gesture still open (`transaction_open`) ⇒ [`Action::Waiting`] (no copy of a
//!   half-finished edit); otherwise one [`Action::Snapshot`].
//! - A clean session never snapshots; if it has copies, it gets one [`Action::Retire`].
//! - One job in flight per session: nothing more is issued until its completion arrives. A
//!   completion that does not match the job in flight (stale, out of order, or another session's)
//!   is ignored.
//! - A change made while a retire is in flight still starts its 30 s at the change; the retire's
//!   completion does not restart it.
//! - A failure keeps the previous copy, records the reason, and backs off 30 s;
//!   [`Scheduler::retry_now`] (the Retry button) lifts the back-off at once.
//! - After a successful copy, if the document moved on meanwhile, the next copy is due 30 s after
//!   the previous one was issued.
//! - Disabled ⇒ no actions at all (existing copies stay); completions are still recorded.
//! - [`Scheduler::next_wake`] is the earliest moment an action could become due, for
//!   `ControlFlow::WaitUntil`. It never reports a session that is waiting on a gesture or on a job
//!   (those wake the loop through input / the worker's wake), so the loop never spins on a past
//!   deadline.
use std::time::{Duration, Instant};

use super::recovery::{fresh_rid, Generation};

/// Time between the first unprotected change and its recovery copy (owner decision D2), and the
/// back-off after a failure.
pub const RECOVERY_INTERVAL: Duration = Duration::from_secs(30);

/// Which recovery job a session has on the worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Snapshot,
    Retire,
}

/// The one job a session has in flight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InFlight {
    pub kind: JobKind,
    /// The job's ticket; a snapshot also asks the store for this generation number.
    pub seq: u64,
    /// The editor `rev` the job was issued at (the content a snapshot captures).
    pub rev: u64,
    pub issued_at: Instant,
}

/// Per-session recovery state. **Frozen here for S3-F1/F2**: F1 embeds it as
/// `DocumentSession::recovery`, F2 builds it with [`SessionRecovery::adopted`] for a recovered
/// orphan. Fields are public so status text can read them; only the [`Scheduler`] writes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRecovery {
    /// `Recovery/<rid>/` of this session. Replaced by a fresh rid after a successful retire (the
    /// store refuses a retired rid).
    pub rid: String,
    /// Ticket for the next job (and the generation number a snapshot asks for).
    pub next_seq: u64,
    /// Editor `rev` captured by the last successful copy (`None` = no copy of this content yet).
    pub last_snapshot_rev: Option<u64>,
    /// This rid may hold data on disk (set when a snapshot is issued, cleared by a successful
    /// retire), so a clean session must retire it.
    pub has_copies: bool,
    pub in_flight: Option<InFlight>,
    /// When the pending change is due for a copy (first change + 30 s; not moved by later edits).
    pub next_deadline: Option<Instant>,
    /// After a failure, no job before this moment (cleared by [`Scheduler::retry_now`]).
    pub backoff_until: Option<Instant>,
    /// The deadline passed while a gesture was open ("Recovery waiting for edit to finish").
    pub waiting: bool,
    /// The newest generation written for this rid (for "Recovery copy saved at 14:32").
    pub last_ok: Option<Generation>,
    /// The latest failure's plain-English reason; cleared by the next success.
    pub last_err: Option<String>,
}

impl SessionRecovery {
    /// A new session: fresh rid, nothing on disk.
    pub fn new(rid: String) -> Self {
        SessionRecovery {
            rid,
            next_seq: 1,
            last_snapshot_rev: None,
            has_copies: false,
            in_flight: None,
            next_deadline: None,
            backoff_until: None,
            waiting: false,
            last_ok: None,
            last_err: None,
        }
    }

    /// A session recovered from an orphan (F2): keeps writing into the adopted `rid` after its
    /// newest generation; `rev` is the editor's rev right after loading (so the loaded content is
    /// not copied again until it changes).
    pub fn adopted(rid: String, loaded: Generation, rev: u64) -> Self {
        SessionRecovery {
            next_seq: loaded.seq.saturating_add(1),
            last_snapshot_rev: Some(rev),
            has_copies: true,
            last_ok: Some(loaded),
            ..SessionRecovery::new(rid)
        }
    }

    fn issue<K>(&mut self, kind: JobKind, sid: K, rev: u64, now: Instant) -> Action<K> {
        let seq = self.next_seq;
        // Saturating: a wrap would reuse old generation names; the store refuses a seq this large.
        self.next_seq = self.next_seq.saturating_add(1);
        self.in_flight = Some(InFlight { kind, seq, rev, issued_at: now });
        self.waiting = false;
        let rid = self.rid.clone();
        match kind {
            JobKind::Snapshot => {
                self.has_copies = true;
                Action::Snapshot { sid, rid, seq }
            }
            JobKind::Retire => Action::Retire { sid, rid, seq },
        }
    }
}

/// One session as seen by [`Scheduler::observe`].
pub struct Probe<'a, K> {
    pub sid: K,
    pub recovery: &'a mut SessionRecovery,
    /// `editor.rev` — a change signal, compared with `==` only.
    pub rev: u64,
    /// `!session.is_dirty()`.
    pub clean: bool,
    /// `editor.transaction_open()` — a gesture is mid-way.
    pub transaction_open: bool,
}

/// What the host must do. Snapshot/Retire become one worker job each, carrying `seq` back in its
/// [`Completion`]; the snapshot job clones the document at this moment (a settled boundary).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action<K> {
    /// `RecoveryStore::write_generation(meta{rid}, seq, doc_to_blob(doc), now_unix)`.
    Snapshot { sid: K, rid: String, seq: u64 },
    /// `RecoveryStore::retire(rid)`.
    Retire { sid: K, rid: String, seq: u64 },
    /// The copy is due but a gesture is open (status: "Recovery waiting for edit to finish").
    Waiting { sid: K },
}

/// What a job did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Done {
    /// The generation the store actually wrote (its `seq` may be above the one asked for).
    Snapshot(Generation),
    Retired,
}

/// A finished recovery job, returned by the worker. `result`'s error is the plain-English reason
/// (`SnapError::reason()`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion<K> {
    pub sid: K,
    pub seq: u64,
    pub result: Result<Done, String>,
}

/// App-wide scheduling policy plus the cached wake time of the latest [`Scheduler::observe`].
#[derive(Clone, Debug)]
pub struct Scheduler {
    interval: Duration,
    enabled: bool,
    wake: Option<Instant>,
}

impl Scheduler {
    pub fn new(interval: Duration) -> Self {
        Scheduler { interval, enabled: true, wake: None }
    }

    /// The Recovery on/off setting. Turning it off stops new jobs and keeps existing copies.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.wake = None;
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Look at every open session once (call it in `AboutToWait`, after draining completions) and
    /// return the jobs to submit. Also recomputes [`Self::next_wake`].
    pub fn observe<'a, K: Copy>(
        &mut self,
        now: Instant,
        probes: impl IntoIterator<Item = Probe<'a, K>>,
    ) -> Vec<Action<K>> {
        self.wake = None;
        let mut out = Vec::new();
        if !self.enabled {
            return out;
        }
        for p in probes {
            let r = p.recovery;
            if let Some(job) = &r.in_flight {
                // A change during a retire starts its deadline now, not when the retire completes.
                if job.kind == JobKind::Retire && !p.clean && r.last_snapshot_rev != Some(p.rev) {
                    r.next_deadline.get_or_insert(now + self.interval);
                }
                continue; // one job per session; its completion wakes the loop
            }
            let ready_at = r.backoff_until.filter(|&b| b > now);
            if p.clean {
                r.next_deadline = None;
                r.waiting = false;
                if r.has_copies {
                    match ready_at {
                        Some(b) => self.wake_at(b),
                        None => out.push(r.issue(JobKind::Retire, p.sid, p.rev, now)),
                    }
                }
                continue;
            }
            if r.last_snapshot_rev == Some(p.rev) {
                r.next_deadline = None; // the newest copy already holds this content
                r.waiting = false;
                continue;
            }
            let deadline = *r.next_deadline.get_or_insert(now + self.interval);
            let due = ready_at.map_or(deadline, |b| b.max(deadline));
            if due > now {
                self.wake_at(due);
            } else if p.transaction_open {
                r.waiting = true;
                out.push(Action::Waiting { sid: p.sid });
            } else {
                out.push(r.issue(JobKind::Snapshot, p.sid, p.rev, now));
            }
        }
        out
    }

    /// Record a finished job for the session `sid`, whose state is `r`. Returns `false` (and changes
    /// nothing) for a completion that does not match that session's job in flight — another
    /// session's (every session's tickets start at 1), stale, duplicated or out of order.
    pub fn on_complete<K: PartialEq>(
        &self,
        now: Instant,
        sid: K,
        r: &mut SessionRecovery,
        done: Completion<K>,
    ) -> bool {
        if done.sid != sid {
            return false;
        }
        let Some(job) = r.in_flight.clone().filter(|j| j.seq == done.seq) else { return false };
        let kind_matches = matches!(
            (job.kind, &done.result),
            (JobKind::Snapshot, Ok(Done::Snapshot(_))) | (JobKind::Retire, Ok(Done::Retired)) | (_, Err(_))
        );
        if !kind_matches {
            return false;
        }
        r.in_flight = None;
        match done.result {
            Ok(Done::Snapshot(generation)) => {
                r.next_seq = r.next_seq.max(generation.seq.saturating_add(1));
                r.last_snapshot_rev = Some(job.rev);
                r.last_ok = Some(generation);
                r.last_err = None;
                r.backoff_until = None;
                // If the document moved on meanwhile, the next copy is due one interval after this
                // one was issued; `observe` drops the deadline when nothing changed.
                r.next_deadline = Some(job.issued_at + self.interval);
            }
            Ok(Done::Retired) => {
                // The store refuses a retired rid: continue in a fresh one. `next_deadline` is kept:
                // a change made during the retire stays due 30 s after that change.
                r.rid = fresh_rid();
                r.has_copies = false;
                r.last_snapshot_rev = None;
                r.last_ok = None;
                r.last_err = None;
                r.backoff_until = None;
            }
            Err(reason) => {
                r.last_err = Some(reason);
                r.backoff_until = Some(now + self.interval);
            }
        }
        true
    }

    /// The Retry button: lift a failure's back-off so the next [`Self::observe`] acts at once.
    pub fn retry_now(&mut self, now: Instant, r: &mut SessionRecovery) {
        r.backoff_until = None;
        if r.next_deadline.is_some_and(|d| d > now) {
            r.next_deadline = Some(now);
        }
        self.wake_at(now);
    }

    /// The session is closing (Don't Save, close after save, quit): retire its copies regardless of
    /// the one-in-flight rule — the worker is FIFO, so the retire runs after any job already queued.
    /// `None` when nothing could be on disk. The session is gone afterwards, so its completion is
    /// dropped by the host.
    pub fn retire_for_close<K>(&self, sid: K, r: &mut SessionRecovery, now: Instant) -> Option<Action<K>> {
        (r.has_copies || r.in_flight.is_some()).then(|| {
            let rev = r.in_flight.as_ref().map_or(0, |j| j.rev);
            r.issue(JobKind::Retire, sid, rev, now)
        })
    }

    /// Earliest moment a session could need a job (valid after the latest [`Self::observe`]); `None`
    /// ⇒ `ControlFlow::Wait`.
    pub fn next_wake(&self) -> Option<Instant> {
        self.wake
    }

    fn wake_at(&mut self, t: Instant) {
        self.wake = Some(self.wake.map_or(t, |w| w.min(t)));
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::new(RECOVERY_INTERVAL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    /// A fake session: the scheduler's inputs plus its embedded recovery state.
    struct Sess {
        id: u32,
        rev: u64,
        clean: bool,
        open: bool,
        rec: SessionRecovery,
    }

    impl Sess {
        fn new(id: u32) -> Self {
            Sess { id, rev: 0, clean: true, open: false, rec: SessionRecovery::new(format!("rid{id}")) }
        }
        fn edit(&mut self) {
            self.rev += 1;
            self.clean = false;
        }
        fn probe(&mut self) -> Probe<'_, u32> {
            Probe {
                sid: self.id,
                rev: self.rev,
                clean: self.clean,
                transaction_open: self.open,
                recovery: &mut self.rec,
            }
        }
    }

    fn observe(s: &mut Scheduler, now: Instant, sessions: &mut [&mut Sess]) -> Vec<Action<u32>> {
        s.observe(now, sessions.iter_mut().map(|x| x.probe()))
    }

    fn one(s: &mut Scheduler, now: Instant, x: &mut Sess) -> Vec<Action<u32>> {
        observe(s, now, &mut [x])
    }

    fn gen(seq: u64) -> Generation {
        Generation { seq, file: format!("snap-{seq}.json"), bytes: 10, crc32: 0, saved_at: 1_000 + seq }
    }

    fn ok_snapshot(sid: u32, seq: u64) -> Completion<u32> {
        Completion { sid, seq, result: Ok(Done::Snapshot(gen(seq))) }
    }

    fn snap_seq(a: &[Action<u32>]) -> Option<u64> {
        match a {
            [Action::Snapshot { seq, .. }] => Some(*seq),
            _ => None,
        }
    }

    #[test]
    fn no_snapshot_before_30s() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        assert!(one(&mut s, t0, &mut a).is_empty(), "a clean new document needs nothing");
        a.edit();
        assert!(one(&mut s, t0, &mut a).is_empty());
        assert_eq!(s.next_wake(), Some(t0 + 30 * S));
        assert!(one(&mut s, t0 + 29 * S, &mut a).is_empty());
        assert_eq!(one(&mut s, t0 + 30 * S, &mut a), vec![Action::Snapshot { sid: 1, rid: "rid1".into(), seq: 1 }]);
        assert!(a.rec.has_copies);
    }

    #[test]
    fn deadline_not_reset_by_later_edits() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.edit();
        one(&mut s, t0, &mut a);
        for sec in [5, 12, 20, 29] {
            a.edit();
            assert!(one(&mut s, t0 + sec * S, &mut a).is_empty());
            assert_eq!(a.rec.next_deadline, Some(t0 + 30 * S), "edit at {sec}s must not move the deadline");
        }
        assert_eq!(snap_seq(&one(&mut s, t0 + 30 * S, &mut a)), Some(1));
        // After the copy, a document that kept changing is due 30 s after that copy was issued.
        assert!(s.on_complete(t0 + 31 * S, a.id, &mut a.rec, ok_snapshot(1, 1)));
        a.edit();
        assert!(one(&mut s, t0 + 45 * S, &mut a).is_empty());
        assert_eq!(snap_seq(&one(&mut s, t0 + 60 * S, &mut a)), Some(2));
    }

    #[test]
    fn clean_document_never_snapshots_and_retires_existing() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        for sec in [0, 30, 300] {
            a.rev += 1; // content changes that end clean (e.g. undo back to saved)
            assert!(one(&mut s, t0 + sec * S, &mut a).is_empty());
        }
        assert_eq!(s.next_wake(), None);
        // Copies exist, then the user saves: exactly one retire, then a fresh rid.
        a.edit();
        one(&mut s, t0, &mut a);
        one(&mut s, t0 + 30 * S, &mut a);
        s.on_complete(t0 + 31 * S, a.id, &mut a.rec, ok_snapshot(1, 1));
        a.clean = true;
        let acts = one(&mut s, t0 + 32 * S, &mut a);
        assert_eq!(acts, vec![Action::Retire { sid: 1, rid: "rid1".into(), seq: 2 }]);
        assert!(one(&mut s, t0 + 33 * S, &mut a).is_empty(), "retire in flight: nothing more");
        assert!(s.on_complete(t0 + 33 * S, a.id, &mut a.rec, Completion { sid: 1, seq: 2, result: Ok(Done::Retired) }));
        assert!(!a.rec.has_copies && a.rec.last_ok.is_none());
        assert_ne!(a.rec.rid, "rid1", "a retired rid is never reused");
        assert!(one(&mut s, t0 + 400 * S, &mut a).is_empty());
    }

    #[test]
    fn in_transaction_defers_with_waiting_action() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.edit();
        one(&mut s, t0, &mut a);
        a.open = true;
        assert_eq!(one(&mut s, t0 + 30 * S, &mut a), vec![Action::Waiting { sid: 1 }]);
        assert!(a.rec.waiting && a.rec.in_flight.is_none());
        assert_eq!(s.next_wake(), None, "no timer while waiting: the gesture's end wakes the loop");
        assert_eq!(one(&mut s, t0 + 40 * S, &mut a), vec![Action::Waiting { sid: 1 }]);
        a.open = false;
        a.rev += 1;
        assert_eq!(snap_seq(&one(&mut s, t0 + 41 * S, &mut a)), Some(1));
        assert!(!a.rec.waiting);
    }

    #[test]
    fn one_in_flight_per_session() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.edit();
        one(&mut s, t0, &mut a);
        assert_eq!(snap_seq(&one(&mut s, t0 + 30 * S, &mut a)), Some(1));
        for sec in [31, 60, 120, 600] {
            a.edit();
            assert!(one(&mut s, t0 + sec * S, &mut a).is_empty(), "job 1 still in flight at {sec}s");
        }
        assert_eq!(s.next_wake(), None);
        s.on_complete(t0 + 601 * S, a.id, &mut a.rec, ok_snapshot(1, 1));
        assert_eq!(snap_seq(&one(&mut s, t0 + 601 * S, &mut a)), Some(2), "overdue content goes at once");
    }

    #[test]
    fn independent_sessions_have_independent_deadlines() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let (mut a, mut b) = (Sess::new(1), Sess::new(2));
        a.edit();
        observe(&mut s, t0, &mut [&mut a, &mut b]);
        b.edit();
        observe(&mut s, t0 + 10 * S, &mut [&mut a, &mut b]);
        assert_eq!(a.rec.next_deadline, Some(t0 + 30 * S));
        assert_eq!(b.rec.next_deadline, Some(t0 + 40 * S));
        let acts = observe(&mut s, t0 + 30 * S, &mut [&mut a, &mut b]);
        assert_eq!(acts, vec![Action::Snapshot { sid: 1, rid: "rid1".into(), seq: 1 }]);
        // A's job in flight does not hold B back, and B's failure does not touch A.
        let acts = observe(&mut s, t0 + 40 * S, &mut [&mut a, &mut b]);
        assert_eq!(acts, vec![Action::Snapshot { sid: 2, rid: "rid2".into(), seq: 1 }]);
        s.on_complete(t0 + 41 * S, b.id, &mut b.rec, Completion { sid: 2, seq: 1, result: Err("disk full".into()) });
        s.on_complete(t0 + 41 * S, a.id, &mut a.rec, ok_snapshot(1, 1));
        assert!(a.rec.last_err.is_none() && a.rec.backoff_until.is_none());
        assert_eq!(b.rec.last_err.as_deref(), Some("disk full"));
    }

    #[test]
    fn out_of_order_completion_is_ignored() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.edit();
        one(&mut s, t0, &mut a);
        one(&mut s, t0 + 30 * S, &mut a);
        assert!(s.on_complete(t0 + 31 * S, a.id, &mut a.rec, ok_snapshot(1, 1)));
        a.edit();
        assert_eq!(snap_seq(&one(&mut s, t0 + 60 * S, &mut a)), Some(2));
        let before = a.rec.clone();
        // A stale (older) completion, a future one and a duplicate change nothing.
        assert!(!s.on_complete(t0 + 61 * S, a.id, &mut a.rec, ok_snapshot(1, 1)));
        assert!(!s.on_complete(t0 + 61 * S, a.id, &mut a.rec, ok_snapshot(1, 7)));
        let wrong_kind = Completion { sid: 1, seq: 2, result: Ok(Done::Retired) };
        assert!(!s.on_complete(t0 + 61 * S, a.id, &mut a.rec, wrong_kind));
        assert_eq!(a.rec, before);
        assert!(s.on_complete(t0 + 62 * S, a.id, &mut a.rec, ok_snapshot(1, 2)));
        assert!(!s.on_complete(t0 + 63 * S, a.id, &mut a.rec, ok_snapshot(1, 2)), "a duplicate is ignored");
        assert_eq!(a.rec.last_ok, Some(gen(2)));
        assert_eq!(a.rec.last_snapshot_rev, Some(a.rev));
    }

    #[test]
    fn failure_backs_off_30s_and_retry_is_immediate() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.edit();
        one(&mut s, t0, &mut a);
        one(&mut s, t0 + 30 * S, &mut a);
        let fail = |seq| Completion { sid: 1, seq, result: Err("The disk is full.".to_string()) };
        assert!(s.on_complete(t0 + 31 * S, a.id, &mut a.rec, fail(1)));
        assert_eq!(a.rec.last_err.as_deref(), Some("The disk is full."));
        assert_eq!(a.rec.last_snapshot_rev, None, "the failed copy protects nothing");
        assert!(one(&mut s, t0 + 32 * S, &mut a).is_empty());
        assert_eq!(s.next_wake(), Some(t0 + 61 * S), "back-off is 30 s from the failure");
        assert!(one(&mut s, t0 + 60 * S, &mut a).is_empty());
        assert_eq!(snap_seq(&one(&mut s, t0 + 61 * S, &mut a)), Some(2));
        // Second failure; Retry acts at once.
        s.on_complete(t0 + 62 * S, a.id, &mut a.rec, fail(2));
        assert!(one(&mut s, t0 + 63 * S, &mut a).is_empty());
        s.retry_now(t0 + 63 * S, &mut a.rec);
        assert_eq!(s.next_wake(), Some(t0 + 63 * S));
        assert_eq!(snap_seq(&one(&mut s, t0 + 63 * S, &mut a)), Some(3));
        s.on_complete(t0 + 64 * S, a.id, &mut a.rec, ok_snapshot(1, 3));
        assert!(a.rec.last_err.is_none());
        // A failed retire backs off the same way.
        a.clean = true;
        assert!(matches!(one(&mut s, t0 + 65 * S, &mut a)[..], [Action::Retire { seq: 4, .. }]));
        s.on_complete(t0 + 65 * S, a.id, &mut a.rec, fail(4));
        assert!(one(&mut s, t0 + 66 * S, &mut a).is_empty());
        assert!(matches!(one(&mut s, t0 + 95 * S, &mut a)[..], [Action::Retire { seq: 5, .. }]));
    }

    #[test]
    fn disabled_emits_nothing() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let (mut dirty, mut clean) = (Sess::new(1), Sess::new(2));
        clean.rec.has_copies = true; // copies from before the switch was turned off
        dirty.edit();
        s.set_enabled(false);
        for sec in [0, 30, 60, 3600] {
            assert!(observe(&mut s, t0 + sec * S, &mut [&mut dirty, &mut clean]).is_empty());
            assert_eq!(s.next_wake(), None);
        }
        assert!(clean.rec.has_copies, "existing copies stay");
        assert!(dirty.rec.in_flight.is_none() && dirty.rec.next_deadline.is_none());
        s.set_enabled(true);
        assert_eq!(observe(&mut s, t0 + 3601 * S, &mut [&mut dirty, &mut clean]).len(), 1, "clean one retires");
        assert_eq!(s.next_wake(), Some(t0 + 3631 * S), "dirty one starts a fresh 30 s");
    }

    #[test]
    fn next_wake_is_earliest_deadline() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let (mut a, mut b, mut c) = (Sess::new(1), Sess::new(2), Sess::new(3));
        assert!(observe(&mut s, t0, &mut [&mut a, &mut b, &mut c]).is_empty());
        assert_eq!(s.next_wake(), None, "nothing pending ⇒ ControlFlow::Wait");
        b.edit();
        observe(&mut s, t0 + 5 * S, &mut [&mut a, &mut b, &mut c]);
        a.edit();
        observe(&mut s, t0 + 12 * S, &mut [&mut a, &mut b, &mut c]);
        assert_eq!(s.next_wake(), Some(t0 + 35 * S));
        // B goes in flight: the earliest remaining is A's.
        observe(&mut s, t0 + 35 * S, &mut [&mut a, &mut b, &mut c]);
        assert_eq!(s.next_wake(), Some(t0 + 42 * S));
        // C fails and backs off to before A's deadline.
        c.edit();
        c.rec.next_deadline = Some(t0);
        c.rec.backoff_until = Some(t0 + 40 * S);
        observe(&mut s, t0 + 36 * S, &mut [&mut a, &mut b, &mut c]);
        assert_eq!(s.next_wake(), Some(t0 + 40 * S));
    }

    #[test]
    fn adopted_session_continues_after_the_loaded_generation() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.rev = 1;
        a.clean = false; // a recovered session is dirty and pathless
        a.rec = SessionRecovery::adopted("orphan".into(), gen(7), 1);
        assert!(one(&mut s, t0, &mut a).is_empty(), "the loaded content is already on disk");
        a.edit();
        one(&mut s, t0, &mut a);
        assert_eq!(one(&mut s, t0 + 30 * S, &mut a), vec![Action::Snapshot { sid: 1, rid: "orphan".into(), seq: 8 }]);
    }

    #[test]
    fn edit_during_retire_keeps_the_30s_deadline_from_the_edit() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.edit();
        one(&mut s, t0, &mut a);
        one(&mut s, t0 + 30 * S, &mut a);
        assert!(s.on_complete(t0 + 31 * S, a.id, &mut a.rec, ok_snapshot(1, 1)));
        a.clean = true; // saved: the copies retire
        assert!(matches!(one(&mut s, t0 + 100 * S, &mut a)[..], [Action::Retire { seq: 2, .. }]));
        a.edit(); // a change while the (slow) retire is still running
        assert!(one(&mut s, t0 + 101 * S, &mut a).is_empty());
        assert_eq!(a.rec.next_deadline, Some(t0 + 131 * S), "the 30 s start at the edit");
        assert!(s.on_complete(
            t0 + 126 * S,
            a.id,
            &mut a.rec,
            Completion { sid: 1, seq: 2, result: Ok(Done::Retired) }
        ));
        assert!(one(&mut s, t0 + 126 * S, &mut a).is_empty());
        assert_eq!(s.next_wake(), Some(t0 + 131 * S), "not 30 s after the retire finished");
        let fresh = a.rec.rid.clone();
        assert_eq!(one(&mut s, t0 + 131 * S, &mut a), vec![Action::Snapshot { sid: 1, rid: fresh, seq: 3 }]);
    }

    #[test]
    fn completion_for_another_session_is_ignored() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let (mut a, mut b) = (Sess::new(1), Sess::new(2));
        a.edit();
        b.edit();
        observe(&mut s, t0, &mut [&mut a, &mut b]);
        let acts = observe(&mut s, t0 + 30 * S, &mut [&mut a, &mut b]);
        assert_eq!(acts.len(), 2, "both sessions have job seq 1 in flight");
        let before = b.rec.clone();
        // A's completion (same seq, same kind) handed to B's state protects nothing in B.
        assert!(!s.on_complete(t0 + 31 * S, b.id, &mut b.rec, ok_snapshot(1, 1)));
        assert_eq!(b.rec, before);
        assert!(b.rec.in_flight.is_some() && b.rec.last_snapshot_rev.is_none());
        assert!(s.on_complete(t0 + 31 * S, a.id, &mut a.rec, ok_snapshot(1, 1)));
        assert!(s.on_complete(t0 + 31 * S, b.id, &mut b.rec, ok_snapshot(2, 1)));
    }

    #[test]
    fn huge_generation_seq_never_panics_or_wraps() {
        let t0 = Instant::now();
        let mut s = Scheduler::default();
        let mut a = Sess::new(1);
        a.rev = 1;
        a.clean = false;
        let top =
            Generation { seq: u64::MAX, file: format!("snap-{}.json", u64::MAX), bytes: 10, crc32: 0, saved_at: 1 };
        a.rec = SessionRecovery::adopted("orphan".into(), top.clone(), 1);
        assert_eq!(a.rec.next_seq, u64::MAX, "saturates instead of overflowing");
        a.edit();
        one(&mut s, t0, &mut a);
        assert_eq!(snap_seq(&one(&mut s, t0 + 30 * S, &mut a)), Some(u64::MAX));
        assert_eq!(a.rec.next_seq, u64::MAX, "never wraps back to a low (reused) generation name");
        let done = Completion { sid: 1, seq: u64::MAX, result: Ok(Done::Snapshot(top)) };
        assert!(s.on_complete(t0 + 31 * S, a.id, &mut a.rec, done));
        assert_eq!(a.rec.next_seq, u64::MAX);
    }

    #[test]
    fn retire_for_close_queues_behind_a_job_in_flight() {
        let t0 = Instant::now();
        let s = Scheduler::default();
        let mut fresh = SessionRecovery::new("r0".into());
        assert_eq!(s.retire_for_close(0u32, &mut fresh, t0), None, "nothing on disk to retire");
        let mut a = Sess::new(1);
        let mut sch = Scheduler::default();
        a.edit();
        one(&mut sch, t0, &mut a);
        one(&mut sch, t0 + 30 * S, &mut a);
        assert_eq!(
            s.retire_for_close(1, &mut a.rec, t0 + 31 * S),
            Some(Action::Retire { sid: 1, rid: "rid1".into(), seq: 2 })
        );
    }
}
