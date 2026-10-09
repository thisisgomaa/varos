//! Per-user shell persistence; no document, Start page or OS-window state enters this file.
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::checksum::new_nonce;
use super::durable::{self, FsPort, WriteError};
use crate::shell::ShellState;

const SETTLE: Duration = Duration::from_secs(1);

/// Additive v1 shell preference. Position is relative to the Board hole, in points.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PickerLayout {
    pub open: bool,
    pub position: Option<[f32; 2]>,
    pub drawer_open: bool,
    pub drawer_tab: u8,
    pub mode: PickerMode,
    pub harmony: HarmonyRule,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickerMode {
    #[default]
    Hsb,
    Hsl,
    Rgb,
    Cmyk,
    Lab,
    Web,
}

/// Original modal hue rules; Shades uses the original brightness progression.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarmonyRule {
    #[default]
    Complementary,
    Analogous,
    Split,
    Triadic,
    Tetradic,
    Square,
    Mono,
    Shades,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    #[serde(default)]
    pub picker: PickerLayout,
    pub tree: serde_json::Value,
    pub show_rail: bool,
    pub show_control_bar: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            picker: PickerLayout::default(),
            tree: ShellState::standard().layout_value(),
            show_rail: true,
            show_control_bar: true,
        }
    }
}

impl Layout {
    fn valid(&self) -> bool {
        self.picker.drawer_tab < 3
            && self.picker.position.is_none_or(|p| p.iter().all(|v| v.is_finite()))
            && ShellState::from_layout_value(self.tree.clone()).is_some()
    }

    fn hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Value's object keys are sorted; boxtree also sorts its invisible tile set.
        serde_json::to_vec(self).expect("layout serializes").hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    app_build: String,
    layout: Layout,
}

/// A one-second trailing debounce; after three failed writes, sleep until the next real change.
/// Observe runs after UI passes; its deadline wakes a host pass without drawing another frame.
pub struct LayoutStore {
    path: Option<PathBuf>,
    saved_hash: u64,
    observed_hash: u64,
    pending: Option<Layout>,
    deadline: Option<Instant>,
    failures: u8,
}

impl LayoutStore {
    pub fn load(fs: &dyn FsPort, path: Option<PathBuf>, reset: bool) -> (Self, Layout) {
        let mut layout = Layout::default();
        if let Some(path) = &path {
            if reset {
                let _ = fs.remove_file(path);
            } else {
                match fs.read(path) {
                    Ok(bytes) => match serde_json::from_slice::<Envelope>(&bytes) {
                        Ok(doc) if doc.version == 1 && doc.layout.valid() => {
                            layout = doc.layout;
                            layout.tree =
                                ShellState::from_layout_value(layout.tree).expect("validated tree").layout_value();
                        }
                        _ => {
                            let bad = path.with_extension("json.bad");
                            // Preserve an earlier diagnostic before parking this invalid file.
                            if fs.metadata(&bad).is_ok() {
                                let _ = fs.rename(&bad, &path.with_extension(format!("json.bad.{}", new_nonce())));
                            }
                            let _ = fs.rename(path, &bad);
                        }
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => {} // Unreadable is not corrupt: leave the file and diagnostics untouched.
                }
            }
        }
        let hash = layout.hash();
        (Self { path, saved_hash: hash, observed_hash: hash, pending: None, deadline: None, failures: 0 }, layout)
    }

    pub fn observe(&mut self, layout: Layout, now: Instant) {
        let hash = layout.hash();
        if hash == self.observed_hash {
            return;
        }
        self.observed_hash = hash;
        self.failures = 0;
        if hash == self.saved_hash || self.path.is_none() {
            self.pending = None;
            self.deadline = None;
        } else {
            self.pending = Some(layout);
            self.deadline = Some(now + SETTLE);
        }
    }

    pub fn next_wake(&self) -> Option<Instant> {
        self.deadline
    }

    /// Returns true only when a write actually replaced the file.
    pub fn tick(&mut self, fs: &dyn FsPort, now: Instant) -> Result<bool, WriteError> {
        if !self.deadline.is_some_and(|at| at <= now) {
            return Ok(false);
        }
        self.flush(fs, now)
    }

    /// Quit bypasses the settle deadline, but still never rewrites an unchanged layout.
    pub fn flush(&mut self, fs: &dyn FsPort, now: Instant) -> Result<bool, WriteError> {
        if self.failures >= 3 {
            return Ok(false);
        }
        let (Some(path), Some(layout)) = (&self.path, &self.pending) else { return Ok(false) };
        let doc = Envelope { version: 1, app_build: env!("CARGO_PKG_VERSION").into(), layout: layout.clone() };
        let bytes = serde_json::to_vec_pretty(&doc).expect("layout serializes");
        let result = (|| {
            if let Some(parent) = path.parent() {
                fs.create_dir_all(parent).map_err(WriteError::Create)?;
            }
            durable::write_replace(fs, path, &bytes, &new_nonce())
        })();
        if let Err(error) = result {
            self.failures += 1;
            self.deadline = (self.failures < 3).then_some(now + SETTLE);
            if self.failures == 3 {
                eprintln!("Layout write failed three times; waiting for a layout change: {error}");
            }
            return Err(error);
        }
        self.failures = 0;
        self.saved_hash = self.observed_hash;
        self.pending = None;
        self.deadline = None;
        Ok(true)
    }

    /// Reset is deliberately remembered as the baseline: rendering/quit must not recreate the file.
    pub fn reset(&mut self, fs: &dyn FsPort) -> Layout {
        if let Some(path) = &self.path {
            let _ = fs.remove_file(path);
        }
        let layout = Layout::default();
        self.saved_hash = layout.hash();
        self.observed_hash = self.saved_hash;
        self.pending = None;
        self.deadline = None;
        self.failures = 0;
        layout
    }
}

#[cfg(test)]
mod tests;
