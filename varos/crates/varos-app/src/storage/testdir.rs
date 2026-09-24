//! Test-only scratch directory: unique under `std::env::temp_dir()`, removed on `Drop`.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TestDir {
    path: PathBuf,
}

impl TestDir {
    /// A fresh, empty directory named after `tag`, the process id and a per-process counter.
    pub(crate) fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("varos-test-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create test dir");
        TestDir { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// Every entry name in the directory, sorted (for "no temp file left behind" checks).
    pub(crate) fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.path)
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        // Read-only files (permission tests) can block removal on some platforms: clear the flag first.
        if let Ok(rd) = std::fs::read_dir(&self.path) {
            for e in rd.flatten() {
                if let Ok(m) = std::fs::symlink_metadata(e.path()) {
                    let mut p = m.permissions();
                    if !m.file_type().is_symlink() && p.readonly() {
                        #[allow(clippy::permissions_set_readonly_false)]
                        p.set_readonly(false);
                        let _ = std::fs::set_permissions(e.path(), p);
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
