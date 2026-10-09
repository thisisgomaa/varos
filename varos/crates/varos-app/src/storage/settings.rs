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
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(flatten)]
    pub preferences: crate::storage::preferences::Preferences,
    /// Autosave/recovery snapshots (§3.5/§3.6). On by default.
    pub recovery_enabled: bool,
    #[serde(default)]
    pub paste_remembers_layers: bool,
    #[serde(default = "enabled")]
    pub autosave_enabled: bool,
    #[serde(default = "default_interval")]
    pub autosave_interval_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            preferences: crate::storage::preferences::Preferences::default(),
            recovery_enabled: true,
            paste_remembers_layers: false,
            autosave_enabled: true,
            autosave_interval_seconds: AUTOSAVE_DEFAULT_SECONDS,
        }
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
        crate::storage::preferences::validate(self)
            .map_err(|e| WriteError::Write(io::Error::new(io::ErrorKind::InvalidInput, e)))?;
        let doc = OnDisk {
            preferences: self.preferences,
            version: SETTINGS_VERSION,
            recovery_enabled: self.recovery_enabled,
            paste_remembers_layers: self.paste_remembers_layers,
            autosave_enabled: self.autosave_enabled,
            autosave_interval_seconds: self.autosave_interval_seconds,
        };
        // Additive lane settings: preserve keys owned by sibling lanes at the FIFO writer.
        let mut value = match fs.read(path) {
            Ok(bytes) => {
                let existing = super::settings_codec::envelope(&bytes)
                    .map_err(|e| WriteError::Write(io::Error::new(io::ErrorKind::InvalidData, e)))?;
                let version = existing.get("version").and_then(serde_json::Value::as_u64);
                if version != Some(1) && version != Some(2) {
                    return Err(WriteError::Write(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Unsupported settings version; writes locked",
                    )));
                }
                let (_, invalid) = super::settings_codec::decode(&bytes)
                    .map_err(|e| WriteError::Write(io::Error::new(io::ErrorKind::InvalidData, e)))?;
                if !invalid.is_empty() {
                    return Err(WriteError::Write(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Preserve and repair invalid settings before replacement",
                    )));
                }
                serde_json::Value::Object(existing)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => serde_json::json!({}),
            Err(e) => return Err(WriteError::Write(e)),
        };
        let owned =
            serde_json::to_value(doc).map_err(|e| WriteError::Write(io::Error::new(io::ErrorKind::InvalidData, e)))?;
        if let (Some(existing), Some(owned)) = (value.as_object_mut(), owned.as_object()) {
            existing.extend(owned.clone());
        }
        let bytes = serde_json::to_vec_pretty(&value)
            .map_err(|e| WriteError::Write(io::Error::new(io::ErrorKind::InvalidData, e)))?;
        if bytes.len() > super::settings_codec::MAX_BYTES {
            return Err(WriteError::Write(io::Error::new(io::ErrorKind::InvalidData, "Settings exceed 64 KiB")));
        }
        durable::write_replace(fs, path, &bytes, &new_nonce())
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    #[serde(flatten)]
    preferences: crate::storage::preferences::Preferences,
    version: u32,
    recovery_enabled: bool,
    #[serde(default)]
    paste_remembers_layers: bool,
    #[serde(default = "enabled")]
    autosave_enabled: bool,
    #[serde(default = "default_interval")]
    autosave_interval_seconds: u64,
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
    let map = match super::settings_codec::envelope(&bytes) {
        Ok(map) => map,
        Err(_) => return corrupt(fs, path),
    };
    let Some(version) = map.get("version").and_then(serde_json::Value::as_u64).and_then(|v| u32::try_from(v).ok())
    else {
        return corrupt(fs, path);
    };
    if version != 1 && version != SETTINGS_VERSION {
        return (Settings::default(), Some(version_mismatch_warning(version)));
    }
    match super::settings_codec::decode(&bytes) {
        Ok((settings, invalid)) => (
            settings,
            if invalid.is_empty() {
                None
            } else {
                Some(format!("Invalid settings retained on disk: {}", invalid.join(", ")))
            },
        ),
        Err(_) => corrupt(fs, path),
    }
}

/// See `recents::version_mismatch_warning` — same code review P2 fix: word the direction
/// correctly instead of a bare `!=` that would call an older file "newer" once `SETTINGS_VERSION`
/// is ever bumped past 1. Version 1 is migrated; unsupported versions are left untouched.
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
    fn settings_writer_preserves_sibling_keys_and_refuses_changed_version() {
        let d = TestDir::new("settings-siblings");
        let path = d.join("settings.json");
        std::fs::write(&path, br#"{"version":1,"recovery_enabled":true,"sibling":{"enabled":true}}"#).unwrap();
        Settings { recovery_enabled: false, paste_remembers_layers: true, ..Settings::default() }
            .save(&RealFs, &path)
            .unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["sibling"], serde_json::json!({"enabled":true}));
        assert_eq!(saved["paste_remembers_layers"], true);
        let future = br#"{"version":3,"recovery_enabled":true,"sibling":42}"#;
        std::fs::write(&path, future).unwrap();
        assert!(Settings::default().save(&RealFs, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), future);
    }

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
        let s = Settings {
            preferences: crate::storage::preferences::Preferences::default(),
            recovery_enabled: false,
            paste_remembers_layers: true,
            autosave_enabled: false,
            autosave_interval_seconds: 300,
            ..Settings::default()
        };
        s.save(&RealFs, &path).unwrap();
        let (loaded, warning) = load(&RealFs, &path);
        assert!(warning.is_none());
        assert_eq!(loaded, s);
    }

    #[test]
    fn old_v1_settings_default_paste_remembers_layers_off() {
        let d = TestDir::new("settings-old-paste");
        let path = d.join("settings.json");
        std::fs::write(&path, br#"{"version":1,"recovery_enabled":false}"#).unwrap();
        let (s, warning) = load(&RealFs, &path);
        assert!(warning.is_none());
        assert!(!s.recovery_enabled && !s.paste_remembers_layers);
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
        std::fs::write(
            &path,
            br#"{"version":1,"recovery_enabled":false,"paste_remembers_layers":true,"window_memory":{"x":12}}"#,
        )
        .unwrap();
        let (s, w) = load(&RealFs, &path);
        assert!(w.is_none());
        assert!(!s.recovery_enabled);
        assert!(s.paste_remembers_layers);
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
