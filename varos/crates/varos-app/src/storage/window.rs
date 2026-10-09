//! Physical-pixel window geometry, independent from shell layout and documents.
use super::{
    checksum::new_nonce,
    durable::{self, FsPort, WriteError},
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Geometry {
    pub position: [i32; 2],
    pub size: [u32; 2],
    pub maximized: bool,
    pub fullscreen: bool,
}
impl Default for Geometry {
    fn default() -> Self {
        Self { position: [0, 0], size: [1460, 860], maximized: false, fullscreen: false }
    }
}
impl Geometry {
    pub fn valid(self) -> bool {
        self.size.iter().all(|v| (320..=32768).contains(v))
            && self.position.iter().all(|v| v.unsigned_abs() <= 1_000_000)
    }
    /// Choose the screen with greatest intersection, else primary (first). Clamp the whole window.
    pub fn clamp(mut self, screens: &[[i32; 4]]) -> Self {
        let selected = screens.iter().filter(|s| s[2] > 0 && s[3] > 0).max_by_key(|s| {
            let w = (i64::from(self.position[0]) + i64::from(self.size[0])).min(i64::from(s[0]) + i64::from(s[2]))
                - i64::from(self.position[0].max(s[0]));
            let h = (i64::from(self.position[1]) + i64::from(self.size[1])).min(i64::from(s[1]) + i64::from(s[3]))
                - i64::from(self.position[1].max(s[1]));
            w.max(0) * h.max(0)
        });
        let selected = selected
            .filter(|s| {
                self.position[0] < s[0] + s[2]
                    && self.position[1] < s[1] + s[3]
                    && i64::from(self.position[0]) + i64::from(self.size[0]) > i64::from(s[0])
                    && i64::from(self.position[1]) + i64::from(self.size[1]) > i64::from(s[1])
            })
            .or_else(|| screens.first());
        if let Some(s) = selected.filter(|s| s[2] > 0 && s[3] > 0) {
            self.size[0] = self.size[0].min(s[2] as u32);
            self.size[1] = self.size[1].min(s[3] as u32);
            self.position[0] = self.position[0].clamp(s[0], s[0] + s[2] - self.size[0] as i32);
            self.position[1] = self.position[1].clamp(s[1], s[1] + s[3] - self.size[1] as i32);
        }
        self
    }
}
fn legacy(bytes: &[u8]) -> Option<Geometry> {
    let text = std::str::from_utf8(bytes).ok()?;
    let fields = text.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 || !["0", "1"].contains(&fields[0]) {
        return None;
    }
    let g = Geometry {
        maximized: fields[0] == "1",
        position: [fields[1].parse().ok()?, fields[2].parse().ok()?],
        size: [fields[3].parse().ok()?, fields[4].parse().ok()?],
        fullscreen: false,
    };
    g.valid().then_some(g)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    geometry: Geometry,
}
pub struct WindowStore {
    path: Option<PathBuf>,
    saved: Option<Geometry>,
    pending: Option<Geometry>,
    deadline: Option<Instant>,
    failures: u8,
}
impl WindowStore {
    pub fn load(fs: &dyn FsPort, path: Option<PathBuf>) -> (Self, Option<Geometry>) {
        let mut saved = None;
        let mut writable = true;
        let mut migrated = false;
        if let Some(path) = &path {
            if let Ok(bytes) = fs.read(path) {
                match serde_json::from_slice::<Envelope>(&bytes) {
                    Ok(e) if e.version == 1 && e.geometry.valid() => saved = Some(e.geometry),
                    Ok(e) if e.version != 1 => {
                        writable = false;
                    } // Future versions are not corruption.
                    _ => {
                        let bad = path.with_extension("json.bad");
                        if fs.metadata(&bad).is_ok() {
                            let _ = fs.rename(&bad, &path.with_extension(format!("json.bad.{}", new_nonce())));
                        }
                        let _ = fs.rename(path, &bad);
                    }
                }
            }
        }
        if let Some(path) = &path {
            if fs.read(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                && !fs.metadata(&path.with_extension("json.bad")).is_ok()
            {
                saved = fs.read(&path.with_file_name("window.txt")).ok().and_then(|b| legacy(&b));
                migrated = saved.is_some();
            }
        }
        (
            Self {
                path: path.filter(|_| writable),
                saved: if migrated { None } else { saved },
                pending: None,
                deadline: None,
                failures: 0,
            },
            saved,
        )
    }
    pub fn observe(&mut self, g: Geometry, now: Instant) {
        if !g.valid() || self.pending.or(self.saved) == Some(g) {
            return;
        }
        self.failures = 0;
        self.pending = (self.saved != Some(g)).then_some(g);
        self.deadline = self.pending.filter(|_| self.path.is_some()).map(|_| now + Duration::from_secs(1));
    }
    pub fn next_wake(&self) -> Option<Instant> {
        self.deadline
    }
    pub fn tick(&mut self, fs: &dyn FsPort, now: Instant) -> Result<bool, WriteError> {
        if self.deadline.is_some_and(|d| d <= now) {
            self.flush(fs, now)
        } else {
            Ok(false)
        }
    }
    pub fn flush(&mut self, fs: &dyn FsPort, now: Instant) -> Result<bool, WriteError> {
        let (Some(path), Some(geometry)) = (&self.path, self.pending) else { return Ok(false) };
        if self.failures >= 3 {
            return Ok(false);
        }
        let bytes = serde_json::to_vec_pretty(&Envelope { version: 1, geometry })
            .map_err(|e| WriteError::Create(std::io::Error::other(e)))?;
        let result = (|| {
            if let Some(parent) = path.parent() {
                fs.create_dir_all(parent).map_err(WriteError::Create)?;
            }
            durable::write_replace(fs, path, &bytes, &new_nonce())
        })();
        if let Err(e) = result {
            self.failures += 1;
            self.deadline = (self.failures < 3).then_some(now + Duration::from_secs(1));
            return Err(e);
        }
        self.saved = Some(geometry);
        self.pending = None;
        self.deadline = None;
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{durable::RealFs, testdir::TestDir};
    #[test]
    fn persistence_debounce_and_quarantine() {
        let d = TestDir::new("window");
        let p = d.join("window.json");
        let now = Instant::now();
        let (mut store, old) = WindowStore::load(&RealFs, Some(p.clone()));
        assert!(old.is_none());
        let g = Geometry { position: [-100, 50], maximized: true, fullscreen: true, ..Default::default() };
        store.observe(g, now);
        assert!(!store.tick(&RealFs, now).unwrap());
        assert!(store.tick(&RealFs, now + Duration::from_secs(2)).unwrap());
        assert_eq!(WindowStore::load(&RealFs, Some(p.clone())).1, Some(g));
        std::fs::write(&p, b"broken").unwrap();
        assert!(WindowStore::load(&RealFs, Some(p.clone())).1.is_none());
        assert!(p.with_extension("json.bad").exists());
    }
    #[test]
    fn unplugged_monitor_and_negative_screen() {
        let g = Geometry { position: [8000, -2000], size: [2000, 1500], ..Default::default() };
        assert_eq!(g.clamp(&[[0, 0, 1280, 800]]).size, [1280, 800]);
        assert_eq!(g.clamp(&[[0, 0, 1280, 800]]).position, [0, 0]);
        let g = Geometry { position: [-1200, 20], size: [800, 600], ..Default::default() };
        assert_eq!(g.clamp(&[[0, 0, 1920, 1080], [-1280, 0, 1280, 800]]), g);
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::storage::{durable::RealFs, testdir::TestDir};
    #[test]
    fn future_version_is_retained_and_legacy_is_imported() {
        let d = TestDir::new("window-versions");
        let p = d.join("window.json");
        let now = Instant::now();
        std::fs::write(&p, br#"{"version":99,"geometry":{}}"#).unwrap();
        let (mut store, _) = WindowStore::load(&RealFs, Some(p.clone()));
        store.observe(Geometry::default(), now);
        assert!(!store.flush(&RealFs, now).unwrap());
        assert_eq!(std::fs::read(&p).unwrap(), br#"{"version":99,"geometry":{}}"#);
        std::fs::remove_file(&p).unwrap();
        std::fs::write(d.join("window.txt"), b"1 -1200 40 1000 700").unwrap();
        let (mut store, g) = WindowStore::load(&RealFs, Some(p.clone()));
        let g = g.unwrap();
        assert!(g.maximized);
        assert_eq!(g.position, [-1200, 40]);
        store.observe(g, now);
        assert!(store.flush(&RealFs, now).unwrap());
        assert!(p.exists());
        assert!(legacy(b"1 0 0 5 10").is_none());
        assert!(legacy(b"no 0 0 1000 700").is_none());
    }
}
