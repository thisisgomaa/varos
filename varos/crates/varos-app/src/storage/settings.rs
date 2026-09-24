//! App-wide settings store (work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3.4): today just the
//! Recovery on/off switch. Same atomic-JSON-with-a-version-field contract as [`super::recents`]
//! (missing → default; corrupt → default + warning, bad bytes kept aside as `settings.json.bad`;
//! unrecognised version → default + warning, file left untouched).
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::checksum::new_nonce;
use super::durable::{self, FsPort, WriteError, WriteOutcome};

const SETTINGS_VERSION: u32 = 1;

/// App-wide settings, persisted across launches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Autosave/recovery snapshots (§3.5/§3.6). On by default.
    pub recovery_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { recovery_enabled: true }
    }
}

impl Settings {
    /// Save atomically as `{"version":1,"recovery_enabled":…}`.
    pub fn save(&self, fs: &dyn FsPort, path: &Path) -> Result<WriteOutcome, WriteError> {
        let doc = OnDisk { version: SETTINGS_VERSION, recovery_enabled: self.recovery_enabled };
        let bytes = serde_json::to_vec_pretty(&doc).expect("Settings always serializes");
        durable::write_replace(fs, path, &bytes, &new_nonce())
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    recovery_enabled: bool,
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
    if probe.version != SETTINGS_VERSION {
        return (
            Settings::default(),
            Some(format!(
                "Settings were saved by a newer version of Varos (format {}); they were left unchanged.",
                probe.version
            )),
        );
    }
    match serde_json::from_slice::<OnDisk>(&bytes) {
        Ok(doc) => (Settings { recovery_enabled: doc.recovery_enabled }, None),
        Err(_) => corrupt(fs, path),
    }
}

fn corrupt(fs: &dyn FsPort, path: &Path) -> (Settings, Option<String>) {
    let bad = bad_path(path);
    let _ = fs.remove_file(&bad);
    let _ = fs.rename(path, &bad);
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
        let s = Settings { recovery_enabled: false };
        s.save(&RealFs, &path).unwrap();
        let (loaded, warning) = load(&RealFs, &path);
        assert!(warning.is_none());
        assert_eq!(loaded, s);
    }
}
