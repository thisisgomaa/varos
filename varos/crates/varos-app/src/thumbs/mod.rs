//! Asynchronous Start-card thumbnail cache. It never reads or writes a `.vrs` file.

pub use varos_raster as raster;

use crate::workspace::FileKey;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use varos_app::storage::paths::AppLayout;
use varos_core::model::Document;

const LIMIT: usize = 200;
const QUEUE_LIMIT: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThumbKey(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    Fresh(PathBuf),
    Stale(PathBuf),
}

#[derive(Clone, Debug)]
pub struct ThumbDone {
    pub key: ThumbKey,
    pub path: Option<PathBuf>,
    pub mtime: SystemTime,
}

struct Request {
    key: ThumbKey,
    snapshot: Arc<Document>,
    mtime: SystemTime,
    panic_for_test: bool,
    asset: Option<varos_raster::export::Asset>,
}

/// One worker with at most 16 distinct keys queued. Re-requests replace the per-key latest slot.
pub struct ThumbService {
    root: PathBuf,
    wake: Option<mpsc::SyncSender<()>>,
    done: mpsc::Receiver<ThumbDone>,
    latest: Arc<Mutex<HashMap<ThumbKey, Request>>>,
    active: Arc<Mutex<HashSet<ThumbKey>>>,
    cancelled: Arc<Mutex<HashSet<ThumbKey>>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ThumbService {
    pub fn new() -> Option<Self> {
        Self::at(AppLayout::current()?.thumbs())
    }

    pub fn at(root: PathBuf) -> Option<Self> {
        clean_orphan_temps(&root);
        let (wake_tx, wake_rx) = mpsc::sync_channel::<()>(1);
        let (done_tx, done) = mpsc::channel();
        let latest = Arc::new(Mutex::new(HashMap::<ThumbKey, Request>::new()));
        let active = Arc::new(Mutex::new(HashSet::new()));
        let cancelled = Arc::new(Mutex::new(HashSet::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (worker_latest, worker_active, worker_cancelled, worker_shutdown) =
            (latest.clone(), active.clone(), cancelled.clone(), shutdown.clone());
        let worker_root = root.clone();
        let thread = std::thread::Builder::new()
            .name("varos-thumbs".into())
            .spawn(move || {
                while wake_rx.recv().is_ok() {
                    loop {
                        if worker_shutdown.load(Ordering::Acquire) {
                            worker_latest.lock().unwrap().clear();
                            return;
                        }
                        let Some(req) = take_request(&worker_latest, &worker_active) else { break };
                        let result = catch_unwind(AssertUnwindSafe(|| {
                            if req.panic_for_test {
                                panic!("injected thumbnail panic");
                            }
                            render_write(&worker_root, &req)
                        }));
                        let mut path = result.ok().and_then(Result::ok);
                        if worker_cancelled.lock().unwrap().remove(&req.key) {
                            delete_key(&worker_root, &req.key);
                            path = None;
                        }
                        worker_active.lock().unwrap().remove(&req.key);
                        let _ = done_tx.send(ThumbDone { key: req.key, path, mtime: req.mtime });
                    }
                }
            })
            .ok()?;
        Some(Self { root, wake: Some(wake_tx), done, latest, active, cancelled, shutdown, thread: Some(thread) })
    }

    pub fn request(&self, key: ThumbKey, snapshot: Arc<Document>, mtime: SystemTime) {
        self.enqueue(Request { key, snapshot, mtime, panic_for_test: false, asset: None });
    }

    /// Export cards reuse the same bounded thumbnail worker, PNG cache and eviction policy.
    pub fn request_export(&self, key: ThumbKey, asset: varos_raster::export::Asset, mtime: SystemTime) -> bool {
        if matches!(self.lookup(&key, mtime), Some(Lookup::Fresh(_))) {
            return true;
        }
        if self.latest.lock().map_or(true, |pending| pending.len() >= QUEUE_LIMIT) {
            return false;
        }
        self.enqueue(Request { key, snapshot: asset.doc.clone(), mtime, panic_for_test: false, asset: Some(asset) });
        true
    }

    fn enqueue(&self, req: Request) {
        if self.shutdown.load(Ordering::Acquire) {
            return;
        }
        self.cancelled.lock().unwrap().remove(&req.key);
        let mut latest = self.latest.lock().unwrap();
        if !latest.contains_key(&req.key) && latest.len() >= QUEUE_LIMIT {
            return;
        }
        latest.insert(req.key.clone(), req);
        drop(latest);
        if let Some(wake) = &self.wake {
            let _ = wake.try_send(());
        }
    }

    /// Returns cached pixels immediately; stale pixels remain displayable while Home requests refresh.
    /// (The app reads through [`ThumbService::index`] — Home looks up on its decode worker — so only
    /// the tests call this one.)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn lookup(&self, key: &ThumbKey, source_mtime: SystemTime) -> Option<Lookup> {
        self.index().lookup(key, source_mtime)
    }

    /// A cheap, `Send` handle that only answers [`ThumbService::lookup`] — for the Start page's decode
    /// worker, which looks files up off the UI thread.
    pub fn index(&self) -> ThumbIndex {
        ThumbIndex { root: self.root.clone() }
    }

    pub fn try_completions(&self) -> Vec<ThumbDone> {
        self.done.try_iter().collect()
    }

    /// Integrator: call from `recent_files.rs::RecentStore::remove_recent`.
    pub fn forget(&self, key: &ThumbKey) {
        self.latest.lock().unwrap().remove(key);
        if self.active.lock().unwrap().contains(key) {
            self.cancelled.lock().unwrap().insert(key.clone());
        }
        delete_key(&self.root, key);
    }

    /// Integrator: call from `recent_files.rs::RecentStore::clear_recent`.
    pub fn clear(&self) {
        let mut cancelled = self.cancelled.lock().unwrap();
        cancelled.extend(self.latest.lock().unwrap().keys().cloned());
        cancelled.extend(self.active.lock().unwrap().iter().cloned());
        self.latest.lock().unwrap().clear();
        drop(cancelled);
        clear_cache(&self.root);
    }
}

impl Drop for ThumbService {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.latest.lock().unwrap().clear();
        self.wake = None;
        self.thread.take(); // detach: a current render may finish; queued renders cannot start
    }
}

/// The cache's read side (see [`ThumbService::index`]).
#[derive(Clone, Debug)]
pub struct ThumbIndex {
    root: PathBuf,
}
impl ThumbIndex {
    pub fn lookup(&self, key: &ThumbKey, source_mtime: SystemTime) -> Option<Lookup> {
        let path = cache_path(&self.root, key);
        if !path.is_file() {
            return None;
        }
        match read_mtime(&mtime_path(&self.root, key)) {
            Some(stored) if stored == mtime_value(source_mtime) => Some(Lookup::Fresh(path)),
            _ => Some(Lookup::Stale(path)),
        }
    }
}
/// Home's thumbnails come from here: the card's key (the board path, as Recent stores it) and its
/// cached modified time (unix seconds — the same clock `RecentStore` hands `request`).
impl varos_app::start_page::ThumbSource for ThumbIndex {
    fn find(&self, key: &varos_app::start::ThumbKey, modified: u64) -> Option<(PathBuf, bool)> {
        match self.lookup(&ThumbKey(key.0.clone()), unix(modified))? {
            Lookup::Fresh(path) => Some((path, true)),
            Lookup::Stale(path) => Some((path, false)),
        }
    }
}
/// Recent's cached modified time (unix seconds) as the cache's mtime stamp.
pub fn unix(secs: u64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_secs(secs)
}

/// In `Lifecycle::save_done`, call after `fingerprint` and before `mark_saved_snapshot`:
/// `thumbs::on_saved(&service, &key, flight.doc.clone(), mtime)`.
pub fn on_saved(service: &ThumbService, file: &FileKey, snapshot: Arc<Document>, mtime: SystemTime) {
    service.request(key_for_file(file), snapshot, mtime);
}

pub fn key_for_file(file: &FileKey) -> ThumbKey {
    ThumbKey(file.path.to_string_lossy().into_owned())
}

fn take_request(latest: &Mutex<HashMap<ThumbKey, Request>>, active: &Mutex<HashSet<ThumbKey>>) -> Option<Request> {
    let mut latest = latest.lock().unwrap();
    let key = latest.keys().next()?.clone();
    let request = latest.remove(&key)?;
    active.lock().unwrap().insert(key);
    Some(request)
}

fn hash_name(key: &ThumbKey) -> String {
    struct Fnv(u64);
    impl Hasher for Fnv {
        fn finish(&self) -> u64 {
            self.0
        }
        fn write(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
            }
        }
    }
    let mut h = Fnv(0xcbf29ce484222325);
    key.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn cache_path(root: &Path, key: &ThumbKey) -> PathBuf {
    root.join(format!("{}.png", hash_name(key)))
}
fn mtime_path(root: &Path, key: &ThumbKey) -> PathBuf {
    root.join(format!("{}.mtime", hash_name(key)))
}
fn mtime_value(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}
fn read_mtime(path: &Path) -> Option<u128> {
    fs::read_to_string(path).ok()?.parse().ok()
}

fn render_write(root: &Path, req: &Request) -> Result<PathBuf, String> {
    let result = render_write_inner(root, req);
    if let Err(reason) = &result {
        static LOGGED: AtomicBool = AtomicBool::new(false);
        if !LOGGED.swap(true, Ordering::Relaxed) {
            eprintln!("thumbnail cache unavailable: {reason}");
        }
    }
    result
}

fn render_write_inner(root: &Path, req: &Request) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let bytes = if let Some(asset) = &req.asset {
        let options = varos_raster::export::Options {
            format: varos_raster::export::Format::Png,
            scale: (varos_app::shell::tokens::EXPORT_CARD_W / asset.page.rect[2].max(asset.page.rect[3])).min(64.0),
            ..Default::default()
        };
        if let Some(index) = asset.page.artboard {
            let side = varos_app::shell::tokens::EXPORT_CARD_W as u32;
            raster::rasterize_artboard(asset.doc.clone(), index, [side, side])
                .ok_or("Invalid thumbnail page.")?
                .encode_png()?
        } else {
            varos_raster::export::encode(asset, &options, &AtomicBool::new(false))?.bytes
        }
    } else {
        raster::rasterize(req.snapshot.clone(), [raster::WIDTH, raster::HEIGHT]).encode_png()?
    };
    let path = cache_path(root, &req.key);
    write_atomic(root, &path, &bytes)?;
    write_atomic(root, &mtime_path(root, &req.key), mtime_value(req.mtime).to_string().as_bytes())?;
    evict(root, LIMIT);
    Ok(path)
}

fn write_atomic(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = root.join(format!(".{}.tmp", varos_app::storage::checksum::new_nonce()));
    fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        e.to_string()
    })
}

fn delete_key(root: &Path, key: &ThumbKey) {
    let _ = fs::remove_file(cache_path(root, key));
    let _ = fs::remove_file(mtime_path(root, key));
}

fn clear_cache(root: &Path) {
    let Ok(read) = fs::read_dir(root) else { return };
    for entry in read.flatten() {
        if matches!(entry.path().extension().and_then(|x| x.to_str()), Some("png" | "mtime" | "tmp")) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn clean_orphan_temps(root: &Path) {
    let Ok(read) = fs::read_dir(root) else { return };
    for entry in read.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "tmp")) {
        let _ = fs::remove_file(entry.path());
    }
}

fn evict(root: &Path, limit: usize) {
    let Ok(read) = fs::read_dir(root) else { return };
    let mut files: Vec<_> = read
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .map(|e| {
            let stem = e.path().file_stem().and_then(|x| x.to_str()).unwrap_or_default().to_owned();
            (read_mtime(&root.join(format!("{stem}.mtime"))).unwrap_or(0), stem, e.path())
        })
        .collect();
    files.sort_by_key(|x| x.0);
    let remove = files.len().saturating_sub(limit);
    for (_, stem, path) in files.into_iter().take(remove) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(root.join(format!("{stem}.mtime")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    struct TestDir(PathBuf);
    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("varos-{name}-{}", varos_app::storage::checksum::new_nonce()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn wait_for(service: &ThumbService, mtime: SystemTime) -> ThumbDone {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(done) = service.try_completions().into_iter().find(|d| d.mtime == mtime) {
                return done;
            }
            assert!(Instant::now() < until, "thumbnail worker timed out");
            std::thread::yield_now();
        }
    }

    #[test]
    fn request_writes_png_and_reports_freshness() {
        let dir = TestDir::new("thumb-request");
        let service = ThumbService::at(dir.join("Thumbs")).unwrap();
        let key = ThumbKey("/boards/a.vrs".into());
        let mtime = UNIX_EPOCH + Duration::from_secs(7);
        assert_eq!(service.lookup(&key, mtime), None);
        service.request(key.clone(), Arc::new(Document::default()), mtime);
        assert!(wait_for(&service, mtime).path.is_some());
        assert!(matches!(service.lookup(&key, mtime), Some(Lookup::Fresh(_))));
        assert!(matches!(service.lookup(&key, mtime + Duration::from_secs(1)), Some(Lookup::Stale(_))));
    }

    #[test]
    fn newest_duplicate_request_wins_without_sleep_race() {
        let dir = TestDir::new("thumb-coalesce");
        let service = ThumbService::at(dir.join("Thumbs")).unwrap();
        let key = ThumbKey("same".into());
        let old = UNIX_EPOCH + Duration::from_secs(1);
        let new = UNIX_EPOCH + Duration::from_secs(2);
        service.request(key.clone(), Arc::new(Document::default()), old);
        service.request(key.clone(), Arc::new(Document::default()), new);
        assert_eq!(wait_for(&service, new).mtime, new);
        assert!(matches!(service.lookup(&key, new), Some(Lookup::Fresh(_))));
    }

    #[test]
    fn panic_does_not_leave_key_pending() {
        let dir = TestDir::new("thumb-panic");
        let service = ThumbService::at(dir.join("Thumbs")).unwrap();
        let key = ThumbKey("panic".into());
        service.enqueue(Request {
            key: key.clone(),
            snapshot: Arc::new(Document::default()),
            mtime: UNIX_EPOCH,
            panic_for_test: true,
            asset: None,
        });
        let _ = wait_for(&service, UNIX_EPOCH);
        let next = UNIX_EPOCH + Duration::from_secs(1);
        service.request(key, Arc::new(Document::default()), next);
        assert!(wait_for(&service, next).path.is_some());
    }

    #[test]
    fn drop_discards_a_full_queue_promptly() {
        let dir = TestDir::new("thumb-drop");
        let service = ThumbService::at(dir.join("Thumbs")).unwrap();
        {
            let mut latest = service.latest.lock().unwrap();
            for i in 0..QUEUE_LIMIT {
                let key = ThumbKey(i.to_string());
                latest.insert(
                    key.clone(),
                    Request {
                        key,
                        snapshot: Arc::new(Document::default()),
                        mtime: UNIX_EPOCH,
                        panic_for_test: false,
                        asset: None,
                    },
                );
            }
            assert_eq!(latest.len(), QUEUE_LIMIT);
        }
        let start = Instant::now();
        drop(service);
        assert!(start.elapsed() < Duration::from_millis(100), "drop took {:?}", start.elapsed());
    }

    #[test]
    fn forget_and_clear_delete_private_cache_files() {
        let dir = TestDir::new("thumb-forget");
        let service = ThumbService::at(dir.join("Thumbs")).unwrap();
        let (a, b) = (ThumbKey("a".into()), ThumbKey("b".into()));
        service.request(a.clone(), Arc::new(Document::default()), UNIX_EPOCH);
        let _ = wait_for(&service, UNIX_EPOCH);
        service.forget(&a);
        assert_eq!(service.lookup(&a, UNIX_EPOCH), None);
        let later = UNIX_EPOCH + Duration::from_secs(1);
        service.request(b.clone(), Arc::new(Document::default()), later);
        let _ = wait_for(&service, later);
        service.clear();
        assert_eq!(service.lookup(&b, later), None);
    }

    #[test]
    fn eviction_keeps_two_hundred_newest_source_mtimes() {
        let dir = TestDir::new("thumb-evict");
        let root = dir.join("Thumbs");
        fs::create_dir_all(&root).unwrap();
        for i in 0..205 {
            let key = ThumbKey(format!("key-{i}"));
            fs::write(cache_path(&root, &key), b"x").unwrap();
            fs::write(mtime_path(&root, &key), i.to_string()).unwrap();
        }
        evict(&root, LIMIT);
        let count = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .count();
        assert_eq!(count, LIMIT);
        for i in 0..5 {
            assert!(!cache_path(&root, &ThumbKey(format!("key-{i}"))).exists());
        }
        assert!(cache_path(&root, &ThumbKey("key-204".into())).exists());
    }

    #[test]
    fn startup_cleans_orphan_temps_and_cache_failure_is_tolerated() {
        let dir = TestDir::new("thumb-fail");
        let root = dir.join("Thumbs");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".orphan.tmp"), b"x").unwrap();
        let service = ThumbService::at(root.clone()).unwrap();
        assert!(!root.join(".orphan.tmp").exists());
        drop(service);
        let blocked = dir.join("file");
        fs::write(&blocked, b"not a directory").unwrap();
        let service = ThumbService::at(blocked.join("Thumbs")).unwrap();
        service.request(ThumbKey("x".into()), Arc::new(Document::default()), UNIX_EPOCH);
        assert!(wait_for(&service, UNIX_EPOCH).path.is_none());
    }
}
