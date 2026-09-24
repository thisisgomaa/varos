//! The one background I/O thread (work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3.6, piece D).
//!
//! Serializing, CRC, `F_FULLFSYNC` and renames can take tens of milliseconds and grow with the
//! document, so they never run inside the event loop. The worker is a **generic job runner**: a job
//! is a boxed closure that returns the host's completion value `T`. Why a closure rather than a
//! typed `Job::{Snapshot, Retire, Export}` enum: the worker then needs to know nothing about
//! `RecoveryStore`, documents or PDF — F1 builds its recovery closures over
//! `RecoveryStore::{write_generation, retire}`, and S6 reuses the same thread for Export by
//! submitting its own closure (the host's `T` gains an `Export` variant) instead of this module
//! growing a placeholder variant, a second thread or a second writer.
//!
//! - FIFO on one thread: jobs run in submission order, so writes to one destination are serialized.
//! - Each finished job's value goes back over a channel and `wake` is called (the host sets it to an
//!   `EventLoopProxy::send_event` closure; tests count calls). The host drains with
//!   [`IoWorker::try_completions`] in `AboutToWait`.
//! - Errors are values: a job reports its failure inside `T`. A job that panics anyway (a bug) is
//!   caught; the worker delivers the `if_panicked` value given at submission and keeps running, so a
//!   session's "one job in flight" never waits forever. (The app's panic hook still runs first.)
//! - [`IoWorker::shutdown`] (quit) closes the queue, runs every job already submitted — the final
//!   retires included — then joins the thread. Dropping the worker does the same.
//! - The worker never touches an `Editor`: a snapshot job owns a cloned `Document`.
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

/// One unit of background work; its return value is the completion the host receives.
pub type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

/// [`IoWorker::submit`] after the worker has stopped (its thread is gone); the job did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerStopped;

impl std::fmt::Display for WorkerStopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the background writer has stopped")
    }
}

impl std::error::Error for WorkerStopped {}

/// The background I/O thread. `T` = the host's completion type.
pub struct IoWorker<T: Send + 'static> {
    queue: Option<Sender<(Job<T>, T)>>,
    done: Receiver<T>,
    thread: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> IoWorker<T> {
    /// Start the thread. `wake` is called after every finished job (from the worker thread).
    pub fn spawn(wake: Box<dyn Fn() + Send>) -> io::Result<Self> {
        let (queue, jobs) = channel::<(Job<T>, T)>();
        let (done_tx, done) = channel::<T>();
        let thread = std::thread::Builder::new().name("varos-io".to_string()).spawn(move || {
            // Ends when every sender is gone (shutdown/drop) and the queue is empty: FIFO drain.
            for (job, if_panicked) in jobs {
                let value = catch_unwind(AssertUnwindSafe(job)).unwrap_or(if_panicked);
                if done_tx.send(value).is_err() {
                    continue; // the host is gone; keep draining so queued retires still run
                }
                wake();
            }
        })?;
        Ok(IoWorker { queue: Some(queue), done, thread: Some(thread) })
    }

    /// Queue `job` behind every job already submitted. `if_panicked` is delivered instead if the job
    /// panics (it should carry the job's identity and a failure).
    pub fn submit(&self, job: Job<T>, if_panicked: T) -> Result<(), WorkerStopped> {
        let queue = self.queue.as_ref().ok_or(WorkerStopped)?;
        queue.send((job, if_panicked)).map_err(|_| WorkerStopped)
    }

    /// Every completion delivered so far, in finishing (= submission) order. Never blocks.
    pub fn try_completions(&self) -> Vec<T> {
        self.done.try_iter().collect()
    }

    /// Close the queue, run everything already submitted, join the thread, and return the
    /// completions not yet collected.
    pub fn shutdown(mut self) -> Vec<T> {
        self.stop();
        self.try_completions()
    }

    fn stop(&mut self) {
        self.queue = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl<T: Send + 'static> Drop for IoWorker<T> {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::durable::{Fault, FaultFs, RealFs, Step};
    use crate::storage::recovery::{fresh_rid, RecoveryStore, SessionMeta};
    use crate::storage::scheduler::{Completion, Done};
    use crate::storage::testdir::TestDir;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    const WAIT: Duration = Duration::from_secs(10);

    fn blob(n: u64) -> Vec<u8> {
        format!(r#"{{"varos":1,"doc":{{"n":{n}}}}}"#).into_bytes()
    }

    fn meta(rid: &str) -> SessionMeta {
        SessionMeta { rid: rid.to_string(), display_name: "Logo".to_string(), ..SessionMeta::default() }
    }

    /// A worker whose wake bumps a counter and pings a channel the test can wait on.
    fn worker<T: Send + 'static>() -> (IoWorker<T>, Arc<AtomicUsize>, mpsc::Receiver<()>) {
        let count = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let c = count.clone();
        let w = IoWorker::spawn(Box::new(move || {
            c.fetch_add(1, Ordering::SeqCst);
            let _ = tx.send(());
        }))
        .unwrap();
        (w, count, rx)
    }

    /// The jobs F1 will build: a snapshot and a retire over the shared store.
    fn snapshot_job(
        store: &Arc<RecoveryStore>,
        sid: u32,
        rid: &str,
        seq: u64,
    ) -> (Job<Completion<u32>>, Completion<u32>) {
        let (store, rid) = (store.clone(), rid.to_string());
        let job: Job<Completion<u32>> = Box::new(move || Completion {
            sid,
            seq,
            result: store
                .write_generation(&meta(&rid), seq, &blob(seq), 100 + seq)
                .map(Done::Snapshot)
                .map_err(|e| e.reason()),
        });
        (job, Completion { sid, seq, result: Err("internal error".to_string()) })
    }

    fn retire_job(
        store: &Arc<RecoveryStore>,
        sid: u32,
        rid: &str,
        seq: u64,
    ) -> (Job<Completion<u32>>, Completion<u32>) {
        let (store, rid) = (store.clone(), rid.to_string());
        let job: Job<Completion<u32>> = Box::new(move || Completion {
            sid,
            seq,
            result: store.retire(&rid).map(|()| Done::Retired).map_err(|e| e.reason()),
        });
        (job, Completion { sid, seq, result: Err("internal error".to_string()) })
    }

    fn submit(w: &IoWorker<Completion<u32>>, (job, if_panicked): (Job<Completion<u32>>, Completion<u32>)) {
        w.submit(job, if_panicked).unwrap();
    }

    #[test]
    fn jobs_run_fifo_and_wake_is_called() {
        let d = TestDir::new("io-fifo");
        let store = Arc::new(RecoveryStore::open(Arc::new(RealFs), d.join("Recovery")).unwrap());
        let (w, woken, pings) = worker::<Completion<u32>>();
        let (a, b) = (fresh_rid(), fresh_rid());
        submit(&w, snapshot_job(&store, 1, &a, 1));
        submit(&w, snapshot_job(&store, 2, &b, 1));
        submit(&w, snapshot_job(&store, 1, &a, 2));
        submit(&w, snapshot_job(&store, 1, &a, 3));
        submit(&w, retire_job(&store, 2, &b, 2));
        let mut got = Vec::new();
        while got.len() < 5 {
            pings.recv_timeout(WAIT).expect("wake after each job");
            got.extend(w.try_completions());
        }
        let order: Vec<(u32, u64)> = got.iter().map(|c| (c.sid, c.seq)).collect();
        assert_eq!(order, vec![(1, 1), (2, 1), (1, 2), (1, 3), (2, 2)], "finishing order = submission order");
        assert!(got.iter().all(|c| c.result.is_ok()), "{got:?}");
        assert_eq!(woken.load(Ordering::SeqCst), 5);
        // The later generations of `a` really landed in order (two kept), and `b` is retired.
        let best = store.load_best(&a).unwrap();
        assert_eq!((best.generation.seq, best.blob), (3, blob(3)));
        assert!(!store.dir().join(&b).exists());
        assert!(w.try_completions().is_empty());
        assert!(w.shutdown().is_empty());
    }

    #[test]
    fn shutdown_drains_pending_retires() {
        let d = TestDir::new("io-drain");
        let store = Arc::new(RecoveryStore::open(Arc::new(RealFs), d.join("Recovery")).unwrap());
        let rids: Vec<String> = (0..3).map(|_| fresh_rid()).collect();
        for rid in &rids {
            store.write_generation(&meta(rid), 1, &blob(1), 101).unwrap();
        }
        let (w, woken, _pings) = worker::<Completion<u32>>();
        // A slow first job holds the queue, so the retires are still pending when quit begins.
        let (release, gate) = mpsc::channel::<()>();
        w.submit(
            Box::new(move || {
                let _ = gate.recv_timeout(WAIT);
                Completion { sid: 9, seq: 0, result: Ok(Done::Retired) }
            }),
            Completion { sid: 9, seq: 0, result: Err("internal error".to_string()) },
        )
        .unwrap();
        for (i, rid) in rids.iter().enumerate() {
            submit(&w, retire_job(&store, i as u32, rid, 1));
        }
        let opener = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let _ = release.send(());
        });
        let left = w.shutdown();
        opener.join().unwrap();
        let sids: Vec<u32> = left.iter().map(|c| c.sid).collect();
        assert_eq!(sids, vec![9, 0, 1, 2], "every queued job ran before shutdown returned");
        assert!(left.iter().all(|c| c.result.is_ok()), "{left:?}");
        assert_eq!(woken.load(Ordering::SeqCst), 4);
        let remaining: Vec<_> = std::fs::read_dir(store.dir()).unwrap().flatten().collect();
        assert!(remaining.is_empty(), "all copies retired: {remaining:?}");
    }

    #[test]
    fn worker_error_becomes_completion_not_panic() {
        let d = TestDir::new("io-err");
        let fs = Arc::new(FaultFs::new(Vec::new()));
        let store = Arc::new(RecoveryStore::open(fs.clone(), d.join("Recovery")).unwrap());
        let rid = fresh_rid();
        store.write_generation(&meta(&rid), 1, &blob(1), 101).unwrap();
        fs.push(Fault::at(Step::Rename).on("manifest").kind(io::ErrorKind::StorageFull));
        let (w, woken, pings) = worker::<Completion<u32>>();
        submit(&w, snapshot_job(&store, 1, &rid, 2));
        // A job that panics (a bug) still yields its failure completion, and the thread survives.
        w.submit(
            Box::new(|| panic!("injected worker bug")),
            Completion { sid: 1, seq: 3, result: Err("internal error".to_string()) },
        )
        .unwrap();
        submit(&w, snapshot_job(&store, 1, &rid, 4));
        let mut got = Vec::new();
        while got.len() < 3 {
            pings.recv_timeout(WAIT).expect("the worker keeps running");
            got.extend(w.try_completions());
        }
        assert_eq!(got[0].seq, 2);
        assert_eq!(got[0].result, Err("The disk is full.".to_string()));
        assert_eq!(got[1], Completion { sid: 1, seq: 3, result: Err("internal error".to_string()) });
        assert!(matches!(&got[2].result, Ok(Done::Snapshot(g)) if g.seq == 4), "{:?}", got[2]);
        assert_eq!(woken.load(Ordering::SeqCst), 3);
        // The failure broke nothing: the next generation landed.
        let gens: Vec<u64> = store.load_best(&rid).map(|l| l.generation.seq).into_iter().collect();
        assert_eq!(gens, vec![4]);
        drop(w);
    }
}
