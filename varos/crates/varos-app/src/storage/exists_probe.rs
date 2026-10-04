//! Start's "is this Recent file still there?" probe, off the UI thread.
//!
//! A `stat` on a sleeping network volume can block for seconds, so the event loop never asks the
//! disk itself. [`ExistsProbe::missing`] answers from a cache at once (an unknown path counts as
//! present until its answer arrives) and queues the path for one background thread; finished
//! answers come back over a channel, `wake` nudges the event loop, and the host drains them with
//! [`ExistsProbe::poll`] in `AboutToWait`. A changed answer bumps [`ExistsProbe::generation`] so
//! the host rebuilds Start once, showing the row as "Missing" without ever blocking a frame.
//!
//! Bounded: the cache and the in-flight set only ever hold paths of the CURRENT Recent list
//! ([`ExistsProbe::retain`] prunes on every rebuild and refresh; answers for pruned paths are
//! dropped), and the job queue holds at most [`MAX_QUEUED`] paths (a full queue just retries on a
//! later rebuild).
//!
//! Shutdown is intentionally NON-joining: the thread is detached (its `JoinHandle` is dropped at
//! spawn). Dropping the probe closes the queue, so an idle worker exits; a worker stuck inside
//! `exists` on a dead volume is never waited for — Quit returns at once and the process exit ends
//! the thread. The worker owns nothing that needs flushing (it only reads metadata).
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, sync_channel, Receiver, SyncSender};

/// Paths waiting for the worker. Recent holds at most 20 entries, so this is never the limit in
/// practice; it only caps what a worker stuck on a dead volume can accumulate.
pub const MAX_QUEUED: usize = 32;

pub struct ExistsProbe {
    known: HashMap<PathBuf, bool>,
    pending: HashSet<PathBuf>,
    jobs: Option<SyncSender<PathBuf>>,
    answers: Receiver<(PathBuf, bool)>,
    generation: u64,
}

impl ExistsProbe {
    /// One detached worker thread running `exists` for each queued path; `wake` runs after every answer.
    pub fn spawn(exists: impl Fn(&Path) -> bool + Send + 'static, wake: Box<dyn Fn() + Send>) -> Self {
        let (jobs, rx) = sync_channel::<PathBuf>(MAX_QUEUED);
        let (tx, answers) = channel();
        let spawned = std::thread::Builder::new().name("varos-exists-probe".into()).spawn(move || {
            for path in rx {
                let found = exists(&path);
                if tx.send((path, found)).is_err() {
                    break; // the probe was dropped
                }
                wake();
            }
        });
        Self {
            known: HashMap::new(),
            pending: HashSet::new(),
            // No thread: nothing is ever probed and every row reads as present (never a UI-thread stat).
            // The JoinHandle is dropped here on purpose: see the module note on non-joining shutdown.
            jobs: spawned.ok().map(|_| jobs),
            answers,
            generation: 0,
        }
    }

    /// The cached answer: `true` only once the background probe reported the file missing.
    /// Never touches the disk; an unknown path is queued and reads as present for now.
    pub fn missing(&mut self, path: &Path) -> bool {
        match self.known.get(path) {
            Some(exists) => !exists,
            None => {
                self.request(path);
                false
            }
        }
    }

    /// Forget every path that is no longer in `current` (the Recent list): cached answers and
    /// in-flight requests alike. Call before each Start rebuild.
    pub fn retain(&mut self, current: &[PathBuf]) {
        let keep: HashSet<&PathBuf> = current.iter().collect();
        self.known.retain(|p, _| keep.contains(p));
        self.pending.retain(|p| keep.contains(p));
    }

    /// Prune to `current`, then re-check each of its paths (e.g. the window regained focus). Cached
    /// answers stay visible until fresh ones arrive.
    pub fn refresh(&mut self, current: &[PathBuf]) {
        self.retain(current);
        for path in current {
            self.request(path);
        }
    }

    fn request(&mut self, path: &Path) {
        if self.pending.contains(path) {
            return;
        }
        if let Some(jobs) = &self.jobs {
            // Full (a worker stuck on a dead volume) or closed: not pending, so a later call retries.
            if jobs.try_send(path.to_path_buf()).is_ok() {
                self.pending.insert(path.to_path_buf());
            }
        }
    }

    /// Drain finished answers; `true` when any visible answer changed (generation bumped). Answers for
    /// paths pruned since they were asked are dropped.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((path, exists)) = self.answers.try_recv() {
            if !self.pending.remove(&path) {
                continue;
            }
            // An unknown path already reads as present, so only a change in what Start shows counts.
            let was_missing = self.known.insert(path, exists).is_some_and(|e| !e);
            changed |= was_missing == exists;
        }
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// Bumps whenever a cached answer changes; part of Start's rebuild key.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Probes still in flight.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Paths with a cached answer.
    pub fn cached(&self) -> usize {
        self.known.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    fn settle(probe: &mut ExistsProbe) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while probe.pending() > 0 && Instant::now() < deadline {
            probe.poll();
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_blocked_probe_never_blocks_the_caller_and_missing_shows_once_answered() {
        // The "network volume" is a gate the test holds closed: the probe thread waits on it.
        let gate = Arc::new(Mutex::new(()));
        let held = gate.lock().unwrap();
        let probe_gate = gate.clone();
        let mut probe = ExistsProbe::spawn(
            move |p| {
                let _wait = probe_gate.lock().unwrap();
                !p.ends_with("gone.vrs")
            },
            Box::new(|| {}),
        );
        let started = Instant::now();
        assert!(!probe.missing(Path::new("/net/gone.vrs")), "unknown reads as present for now");
        assert!(!probe.missing(Path::new("/net/here.vrs")));
        assert!(started.elapsed() < Duration::from_millis(100), "the caller never waits on the disk");
        assert!(!probe.poll());
        assert_eq!(probe.pending(), 2);
        drop(held);
        settle(&mut probe);
        assert!(probe.missing(Path::new("/net/gone.vrs")), "the answered probe shows the file as missing");
        assert!(!probe.missing(Path::new("/net/here.vrs")));
        assert_eq!(probe.generation(), 1);
    }

    #[test]
    fn removed_recents_are_pruned_and_never_requeued() {
        let asked = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let log = asked.clone();
        let mut probe = ExistsProbe::spawn(
            move |p| {
                log.lock().unwrap().push(p.to_path_buf());
                false
            },
            Box::new(|| {}),
        );
        let (a, b) = (PathBuf::from("/a.vrs"), PathBuf::from("/b.vrs"));
        probe.missing(&a);
        probe.missing(&b);
        settle(&mut probe);
        assert_eq!(probe.cached(), 2);
        // b leaves Recent: its answer is dropped, and a refresh only re-checks a.
        probe.refresh(std::slice::from_ref(&a));
        settle(&mut probe);
        assert_eq!(probe.cached(), 1);
        let asked = asked.lock().unwrap();
        assert_eq!(asked.iter().filter(|p| **p == b).count(), 1, "a removed recent is never asked again");
        assert_eq!(asked.iter().filter(|p| **p == a).count(), 2);
    }

    #[test]
    fn a_dead_volume_bounds_the_queue_and_never_delays_quit() {
        let gate = Arc::new(Mutex::new(()));
        let held = gate.lock().unwrap();
        let probe_gate = gate.clone();
        let mut probe = ExistsProbe::spawn(
            move |_| {
                let _stuck = probe_gate.lock().unwrap();
                true
            },
            Box::new(|| {}),
        );
        for i in 0..(MAX_QUEUED * 4) {
            probe.missing(Path::new(&format!("/dead/{i}.vrs")));
        }
        assert!(probe.pending() <= MAX_QUEUED + 1, "queue is bounded: {}", probe.pending());
        let started = Instant::now();
        drop(probe); // Quit: nothing joins the stuck worker
        assert!(started.elapsed() < Duration::from_millis(100));
        drop(held);
    }
}
