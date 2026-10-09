//! The ONE app-data resolver (work order §3.2).
//!
//! Order: `VAROS_DATA_DIR` override → macOS `~/Library/Application Support/Varos` → Windows
//! `%APPDATA%\Varos` (today's `window.txt` home) → Linux `$XDG_DATA_HOME/varos`, else
//! `~/.local/share/varos`. Relative values are never used and there is **no** working-directory
//! fallback: `None` means "no app storage" (Recovery unavailable, Recent kept in memory only).
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// Environment variable that overrides the data root (tests and the unwritable-storage hand test).
pub const DATA_DIR_ENV: &str = "VAROS_DATA_DIR";

/// The platform whose storage convention applies. A parameter (not `cfg!`) so every branch is
/// unit-tested on any host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    /// The platform this binary was built for (anything that is not macOS/Windows uses the Linux rules).
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// Absolute-path test for `os`'s path syntax, independent of the host (a Windows `C:\…` value is
/// judged by Windows rules even when the test runs on Linux).
fn is_absolute_for(os: Os, p: &OsStr) -> bool {
    let s = p.to_string_lossy();
    match os {
        Os::Windows => {
            let b = s.as_bytes();
            // `C:\…` / `C:/…` (drive-absolute) or `\\server\share` / `//server/share` (UNC).
            (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
                || s.starts_with("\\\\")
                || s.starts_with("//")
        }
        Os::Mac | Os::Linux => s.starts_with('/'),
    }
}

/// Resolve the app-data root. Pure: `env` reads a variable, `home` is the user's home folder.
/// Empty variables count as unset. A relative `VAROS_DATA_DIR` is rejected outright (`None`) rather
/// than silently using the real per-user folder; a relative `XDG_DATA_HOME` is ignored as the XDG
/// spec requires.
pub fn resolve_data_root(os: Os, env: &dyn Fn(&str) -> Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    let var = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(v) = var(DATA_DIR_ENV) {
        return is_absolute_for(os, &v).then(|| PathBuf::from(v));
    }
    let home = home.filter(|h| is_absolute_for(os, h.as_os_str()));
    match os {
        Os::Mac => home.map(|h| h.join("Library").join("Application Support").join("Varos")),
        Os::Windows => var("APPDATA").filter(|a| is_absolute_for(os, a)).map(|a| PathBuf::from(a).join("Varos")),
        Os::Linux => match var("XDG_DATA_HOME").filter(|x| is_absolute_for(os, x)) {
            Some(x) => Some(PathBuf::from(x).join("varos")),
            None => home.map(|h| h.join(".local").join("share").join("varos")),
        },
    }
}

/// The data root for this process (real environment + real home folder).
pub fn data_root() -> Option<PathBuf> {
    let home = std::env::home_dir();
    resolve_data_root(Os::current(), &|k| std::env::var_os(k), home)
}
