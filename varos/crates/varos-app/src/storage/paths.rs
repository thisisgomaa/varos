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

/// Named files and folders under the data root. Only paths — nothing is created here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppLayout {
    pub root: PathBuf,
}

impl AppLayout {
    pub fn new(root: PathBuf) -> Self {
        AppLayout { root }
    }

    /// The layout under [`data_root`], or `None` when no usable data root exists.
    pub fn current() -> Option<Self> {
        data_root().map(AppLayout::new)
    }

    /// Remembered window geometry.
    pub fn window_state(&self) -> PathBuf {
        self.root.join("window.txt")
    }

    /// The crash log written by the panic hook / fatal path.
    pub fn crash_log(&self) -> PathBuf {
        self.root.join("Logs").join("crash.txt")
    }

    /// The Recent documents list.
    pub fn recents(&self) -> PathBuf {
        self.root.join("recent.json")
    }

    /// App-wide settings (Recovery on/off, …).
    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// The recovery-snapshot store.
    pub fn recovery(&self) -> PathBuf {
        self.root.join("Recovery")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let m: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), OsString::from(v))).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn resolver_mac_uses_application_support() {
        let env = env_of(&[("APPDATA", "/ignored"), ("XDG_DATA_HOME", "/ignored")]);
        let got = resolve_data_root(Os::Mac, &env, Some(PathBuf::from("/Users/ahmed")));
        assert_eq!(got, Some(PathBuf::from("/Users/ahmed").join("Library").join("Application Support").join("Varos")));
        // No home ⇒ no storage (never the working directory).
        assert_eq!(resolve_data_root(Os::Mac, &env, None), None);
    }

    #[test]
    fn resolver_windows_uses_appdata() {
        let appdata = r"C:\Users\ahmed\AppData\Roaming";
        let env = env_of(&[("APPDATA", appdata)]);
        let got = resolve_data_root(Os::Windows, &env, Some(PathBuf::from(r"C:\Users\ahmed")));
        assert_eq!(got, Some(PathBuf::from(appdata).join("Varos")));
        // The layout keeps today's window.txt location (%APPDATA%\Varos\window.txt).
        assert_eq!(
            AppLayout::new(got.unwrap()).window_state(),
            PathBuf::from(appdata).join("Varos").join("window.txt")
        );
        // Missing or relative APPDATA ⇒ None (the home folder is not guessed at on Windows).
        assert_eq!(resolve_data_root(Os::Windows, &env_of(&[]), Some(PathBuf::from(r"C:\Users\a"))), None);
        assert_eq!(resolve_data_root(Os::Windows, &env_of(&[("APPDATA", r"AppData\Roaming")]), None), None);
    }

    #[test]
    fn resolver_linux_xdg_then_local_share() {
        let home = Some(PathBuf::from("/home/ahmed"));
        let xdg = env_of(&[("XDG_DATA_HOME", "/data/xdg")]);
        assert_eq!(resolve_data_root(Os::Linux, &xdg, home.clone()), Some(PathBuf::from("/data/xdg").join("varos")));
        let none = env_of(&[]);
        assert_eq!(
            resolve_data_root(Os::Linux, &none, home.clone()),
            Some(PathBuf::from("/home/ahmed").join(".local").join("share").join("varos"))
        );
        // Relative or empty XDG_DATA_HOME is ignored (XDG spec) ⇒ ~/.local/share.
        for bad in ["relative/xdg", ""] {
            let env = env_of(&[("XDG_DATA_HOME", bad)]);
            assert_eq!(
                resolve_data_root(Os::Linux, &env, home.clone()),
                Some(PathBuf::from("/home/ahmed").join(".local").join("share").join("varos"))
            );
        }
    }

    #[test]
    fn resolver_override_wins_and_relative_is_rejected() {
        for os in [Os::Mac, Os::Windows, Os::Linux] {
            let abs = if os == Os::Windows { r"D:\VarosData" } else { "/tmp/varos-data" };
            let env = env_of(&[(DATA_DIR_ENV, abs), ("APPDATA", r"C:\AppData"), ("XDG_DATA_HOME", "/xdg")]);
            let home = Some(PathBuf::from(if os == Os::Windows { r"C:\Users\a" } else { "/home/a" }));
            assert_eq!(resolve_data_root(os, &env, home.clone()), Some(PathBuf::from(abs)), "{os:?}");
            // A relative override is refused — and does NOT fall through to the real per-user folder.
            let rel = env_of(&[(DATA_DIR_ENV, "data/here"), ("APPDATA", r"C:\AppData"), ("XDG_DATA_HOME", "/xdg")]);
            assert_eq!(resolve_data_root(os, &rel, home.clone()), None, "{os:?}");
            // An empty override counts as unset.
            let empty = env_of(&[(DATA_DIR_ENV, ""), ("APPDATA", r"C:\AppData"), ("XDG_DATA_HOME", "/xdg")]);
            assert!(resolve_data_root(os, &empty, home).is_some(), "{os:?}");
        }
        // UNC override on Windows is absolute.
        let unc = env_of(&[(DATA_DIR_ENV, r"\\server\share\varos")]);
        assert_eq!(resolve_data_root(Os::Windows, &unc, None), Some(PathBuf::from(r"\\server\share\varos")));
    }

    #[test]
    fn resolver_none_never_falls_back_to_cwd() {
        let none = env_of(&[]);
        for os in [Os::Mac, Os::Windows, Os::Linux] {
            assert_eq!(resolve_data_root(os, &none, None), None, "{os:?}");
            // A relative "home" is not a home: still None, never resolved against the working dir.
            assert_eq!(resolve_data_root(os, &none, Some(PathBuf::from("."))), None, "{os:?}");
            assert_eq!(resolve_data_root(os, &none, Some(PathBuf::from("home/a"))), None, "{os:?}");
        }
    }

    #[test]
    fn layout_names_are_fixed() {
        let l = AppLayout::new(PathBuf::from("/r"));
        let r = PathBuf::from("/r");
        assert_eq!(l.window_state(), r.join("window.txt"));
        assert_eq!(l.crash_log(), r.join("Logs").join("crash.txt"));
        assert_eq!(l.recents(), r.join("recent.json"));
        assert_eq!(l.settings(), r.join("settings.json"));
        assert_eq!(l.recovery(), r.join("Recovery"));
    }
}
