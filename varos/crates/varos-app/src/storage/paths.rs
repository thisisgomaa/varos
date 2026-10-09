//! Shared app-data resolver, with app-owned named files.
#[cfg(test)]
use std::ffi::OsString;
use std::path::PathBuf;
pub use varos_bridge::storage_paths::{data_root, resolve_data_root, Os, DATA_DIR_ENV};

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

    pub fn window_json(&self) -> PathBuf {
        self.root.join("window.json")
    }
    pub fn templates(&self) -> PathBuf {
        self.root.join("Templates")
    }

    /// Per-user shell arrangement (independent of documents and window geometry).
    pub fn shell_layout(&self) -> PathBuf {
        self.root.join("layout.json")
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

    /// CPU-rendered Start-card thumbnails (never stored beside the user's document).
    pub fn thumbs(&self) -> PathBuf {
        self.root.join("Thumbs")
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
