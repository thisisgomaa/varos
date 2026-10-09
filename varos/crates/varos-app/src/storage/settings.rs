//! App-wide recovery and file-autosave preferences (settings v2, migrates v1). Same atomic-JSON-with-a-version-field contract as [`super::recents`]
//! (missing → default; corrupt → default + warning, bad bytes kept aside as `settings.json.bad`;
//! unrecognised version → default + warning, file left untouched).
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::checksum::new_nonce;
use super::durable::{self, FsPort, WriteError, WriteOutcome};

const SETTINGS_VERSION: u32 = 2;

pub const AUTOSAVE_DEFAULT_SECONDS: u64 = 120;
pub fn valid_autosave_interval(seconds: u64) -> bool {
    (30..=1800).contains(&seconds)
}
fn default_interval() -> u64 {
    AUTOSAVE_DEFAULT_SECONDS
}
fn enabled() -> bool {
    true
}

/// App-wide settings, persisted across launches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Autosave/recovery snapshots (§3.5/§3.6). On by default.
    pub recovery_enabled: bool,
    #[serde(default = "enabled")]
    pub autosave_enabled: bool,
    #[serde(default = "default_interval")]
    pub autosave_interval_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { recovery_enabled: true, autosave_enabled: true, autosave_interval_seconds: AUTOSAVE_DEFAULT_SECONDS }
    }
}

impl Settings {
    /// Save v2 atomically, retaining additive keys owned by other settings lanes.
    pub fn save(&self, fs: &dyn FsPort, path: &Path) -> Result<WriteOutcome, WriteError> {
        if !valid_autosave_interval(self.autosave_interval_seconds) {
            return Err(WriteError::Create(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Autosave interval must be 30–1800 seconds",
            )));
        }
        let existing = match fs.read(path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(WriteError::Create(e)),
        };
        if let Some(bytes) = &existing {
            if serde_json::from_slice::<VersionProbe>(bytes).is_ok_and(|p| p.version > SETTINGS_VERSION) {
                return Err(WriteError::Create(io::Error::other("Newer settings were left unchanged")));
            }
        }
        let doc = OnDisk {
            version: SETTINGS_VERSION,
            recovery_enabled: self.recovery_enabled,
            autosave_enabled: self.autosave_enabled,
            autosave_interval_seconds: self.autosave_interval_seconds,
        };
        let mut value = existing
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        let known = serde_json::to_value(doc).map_err(|e| WriteError::Create(io::Error::other(e)))?;
        if let Some(known) = known.as_object() {
            value.extend(known.clone());
        }
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| WriteError::Create(io::Error::other(e)))?;
        durable::write_replace(fs, path, &bytes, &new_nonce())
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    recovery_enabled: bool,
    #[serde(default = "enabled")]
    autosave_enabled: bool,
    #[serde(default = "default_interval")]
    autosave_interval_seconds: u64,
}

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

fn bad_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".bad");
    PathBuf::from(s)
}

/// Missing file → default (Recovery on), no warning. Corrupt JSON → default + warning, moved aside
/// to `<name>.bad`. A version this build does not recognise → default + warning, file untouched.
pub fn load(fs: &dyn FsPort, path: &Path) -> (Settings, Option<String>) {
    let bytes = match fs.read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (Settings::default(), None),
        Err(e) => return (Settings::default(), Some(format!("Couldn't read settings: {}", durable::io_reason(&e)))),
    };
    let probe: VersionProbe = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => return corrupt(fs, path),
    };
    if probe.version != 1 && probe.version != SETTINGS_VERSION {
        return (Settings::default(), Some(version_mismatch_warning(probe.version)));
    }
    match serde_json::from_slice::<OnDisk>(&bytes) {
        Ok(doc) if valid_autosave_interval(doc.autosave_interval_seconds) => (
            Settings {
                recovery_enabled: doc.recovery_enabled,
                autosave_enabled: doc.autosave_enabled,
                autosave_interval_seconds: doc.autosave_interval_seconds,
            },
            None,
        ),
        Ok(_) => corrupt(fs, path),
        Err(_) => corrupt(fs, path),
    }
}

/// See `recents::version_mismatch_warning` — same code review P2 fix: word the direction
/// correctly instead of a bare `!=` that would call an older file "newer" once `SETTINGS_VERSION`
/// is ever bumped past 1. Neither direction is migrated; only the wording changes.
fn version_mismatch_warning(found: u32) -> String {
    let word = if found > SETTINGS_VERSION { "a newer" } else { "an older" };
    format!("Settings were saved by {word} version of Varos (format {found}); they were left unchanged.")
}

/// Move a corrupt `settings.json` aside to `<name>.bad`, but only when no earlier corruption is
/// already parked there — a second crash/bad-write must not erase the first one's evidence (code
/// review P3, mirrors `recents::corrupt`).
fn corrupt(fs: &dyn FsPort, path: &Path) -> (Settings, Option<String>) {
    let bad = bad_path(path);
    if fs.metadata(&bad).is_err() {
        let _ = fs.rename(path, &bad);
    }
    (Settings::default(), Some("Settings were damaged and have been reset.".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::durable::RealFs;
    use crate::storage::testdir::TestDir;

    #[test]
    fn settings_default_recovery_on() {
        assert!(Settings::default().recovery_enabled);
        let d = TestDir::new("settings-missing");
        let (s, warning) = load(&RealFs, &d.join("settings.json"));
        assert!(s.recovery_enabled);
        assert!(warning.is_none());
    }

    #[test]
    fn settings_round_trip() {
        let d = TestDir::new("settings-roundtrip");
        let path = d.join("settings.json");
        let s = Settings { recovery_enabled: false, autosave_enabled: false, autosave_interval_seconds: 300 };
        s.save(&RealFs, &path).unwrap();
        let (loaded, warning) = load(&RealFs, &path);
        assert!(warning.is_none());
        assert_eq!(loaded, s);
    }

    #[test]
    fn version_mismatch_is_worded_for_the_right_direction() {
        let d = TestDir::new("settings-verdir");
        let newer = d.join("newer.json");
        std::fs::write(&newer, br#"{"version":99,"recovery_enabled":true}"#).unwrap();
        let (_, w) = load(&RealFs, &newer);
        let w = w.expect("a warning is reported");
        assert!(w.contains("newer") && !w.contains("older"), "{w:?}");
        assert_eq!(std::fs::read(&newer).unwrap(), br#"{"version":99,"recovery_enabled":true}"#);

        let older = d.join("older.json");
        std::fs::write(&older, br#"{"version":0,"recovery_enabled":true}"#).unwrap();
        let (_, w) = load(&RealFs, &older);
        let w = w.expect("a warning is reported");
        assert!(w.contains("older") && !w.contains("newer"), "{w:?}");
        assert_eq!(
            std::fs::read(&older).unwrap(),
            br#"{"version":0,"recovery_enabled":true}"#,
            "left unchanged either way"
        );
    }

    #[test]
    fn second_corruption_preserves_the_first_bad_file() {
        let d = TestDir::new("settings-doublecorrupt");
        let path = d.join("settings.json");
        let bad = d.join("settings.json.bad");
        std::fs::write(&path, b"first corrupt bytes").unwrap();
        load(&RealFs, &path);
        assert_eq!(std::fs::read(&bad).unwrap(), b"first corrupt bytes");

        std::fs::write(&path, b"second corrupt bytes").unwrap();
        let (s, warning) = load(&RealFs, &path);
        assert!(s.recovery_enabled, "safe default while corrupt");
        assert!(warning.is_some());
        assert_eq!(std::fs::read(&bad).unwrap(), b"first corrupt bytes", "the first crash's evidence survives");
        assert_eq!(std::fs::read(&path).unwrap(), b"second corrupt bytes");
    }
}

#[cfg(test)]
mod autosave_tests {
    use super::*;
    use crate::storage::{durable::RealFs, testdir::TestDir};
    #[test]
    fn migration_bounds_and_future_preservation() {
        let d = TestDir::new("autosave-settings");
        let path = d.join("settings.json");
        std::fs::write(&path, br#"{"version":1,"recovery_enabled":false,"window_memory":{"x":12}}"#).unwrap();
        let (s, w) = load(&RealFs, &path);
        assert!(w.is_none());
        assert!(!s.recovery_enabled);
        assert!(s.autosave_enabled);
        assert_eq!(s.autosave_interval_seconds, 120);
        s.save(&RealFs, &path).unwrap();
        assert_eq!(load(&RealFs, &path).0, s);
        let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored["window_memory"]["x"], 12);
        for n in [0, 29, 1801, u64::MAX] {
            let invalid = Settings { autosave_interval_seconds: n, ..s };
            assert!(invalid.save(&RealFs, &path).is_err());
        }
        let future = br#"{"version":99,"recovery_enabled":false,"future":42}"#;
        std::fs::write(&path, future).unwrap();
        assert!(s.save(&RealFs, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), future);
    }
}
