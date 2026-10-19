//! Mistake guards on the files agents write (a port of Varos ADR-0011 Amendment 3's file rules).
//!
//! These are not permissions: every local agent may save and export without asking (owner decision
//! 2026-10-08). They stop the expensive mistakes an agent can make with a path:
//!
//! - never overwrite an existing file (except saving the document onto its own file);
//! - write only under the user's home folder or on an external volume (`/Volumes/<name>/…` on
//!   macOS), and inside `~/Library` only into the iCloud Drive and cloud-storage folders;
//! - no hidden (dot) folders or files, no `..`, no relative paths, nothing inside the running app;
//! - the folder must already exist (agents don't create folder trees), and it is checked again with
//!   its links followed.
//!
//! A path is checked, then written: another program swapping in a link between the two can still
//! win that race. The person at the keyboard is never guarded.

use std::path::{Component, Path, PathBuf};

/// Where the guard draws its lines; [`Places::current`] for the running process.
#[derive(Clone, Debug)]
pub struct Places {
    pub home: PathBuf,
    /// The running app's bundle or folder, which agents never write into.
    pub app: Option<PathBuf>,
}

impl Places {
    /// The user's home (`$HOME`, `%USERPROFILE%`) and the running executable's bundle.
    pub fn current() -> Option<Self> {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(PathBuf::from)?;
        let app = std::env::current_exe().ok().and_then(|exe| {
            // The `.app` bundle on macOS, else the executable's folder.
            let bundle = exe.ancestors().find(|a| a.extension().is_some_and(|x| x == "app")).map(Path::to_path_buf);
            bundle.or_else(|| exe.parent().map(Path::to_path_buf))
        });
        Some(Self { home, app })
    }
}

/// May an agent write a new file at `path`? `own_file`: the document's own file, which a plain
/// Save may write over. `Err` says why not, in words an agent can act on.
pub fn check_new_file(path: &str, places: &Places, own_file: Option<&str>) -> Result<(), String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("give an absolute path (agents' files are never relative): {path}"));
    }
    if own_file.is_some_and(|own| same_file(Path::new(own), p)) {
        return Ok(());
    }
    plain_components(p, path)?;
    if std::fs::symlink_metadata(p).is_ok() {
        return Err(format!("{path} already exists; agents never overwrite a file: choose a new name"));
    }
    let (Some(parent), Some(name)) = (p.parent(), p.file_name()) else {
        return Err(format!("not a file path: {path}"));
    };
    let real_parent = std::fs::canonicalize(parent).map_err(|_| format!("the folder of {path} doesn't exist; agents don't create folders"))?;
    if !real_parent.is_dir() {
        return Err(format!("the folder of {path} is not a folder"));
    }
    allowed_place(p, places, path)?;
    let real = real_parent.join(name);
    allowed_place(&real, places, path)?;
    plain_components(&real, path)
}

/// May an agent write files into the existing folder `path` (Export for Screens)?
pub fn check_folder(path: &str, places: &Places) -> Result<(), String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("give an absolute path (agents' files are never relative): {path}"));
    }
    plain_components(p, path)?;
    let real = std::fs::canonicalize(p).map_err(|_| format!("{path} doesn't exist; agents don't create folders"))?;
    if !real.is_dir() {
        return Err(format!("{path} is not a folder"));
    }
    allowed_place(p, places, path)?;
    allowed_place(&real.join("x"), places, path)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// No `..`, `.`, or hidden (dot) name anywhere in the path.
fn plain_components(p: &Path, shown: &str) -> Result<(), String> {
    for c in p.components() {
        match c {
            Component::ParentDir | Component::CurDir => return Err(format!("no `..` or `.` in agents' paths: {shown}")),
            Component::Normal(n) if n.to_string_lossy().starts_with('.') => {
                return Err(format!("agents don't write hidden files or into hidden folders: {shown}"));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Under home (but in `~/Library` only iCloud Drive and cloud storage) or on an external volume,
/// never the home folder itself, never inside the running app.
fn allowed_place(p: &Path, places: &Places, shown: &str) -> Result<(), String> {
    if let Some(app) = &places.app
        && p.starts_with(app)
    {
        return Err(format!("agents don't write inside the running app: {shown}"));
    }
    let home = std::fs::canonicalize(&places.home).unwrap_or_else(|_| places.home.clone());
    let under_home = |home: &Path| -> Option<bool> {
        let rest = p.strip_prefix(home).ok()?;
        let mut parts = rest.components();
        let first = parts.next()?;
        if first.as_os_str() != "Library" {
            return Some(true);
        }
        let second = parts.next()?;
        Some(second.as_os_str() == "Mobile Documents" || (second.as_os_str() == "CloudStorage" && parts.next().is_some()))
    };
    let home_ok = under_home(&places.home).or_else(|| under_home(&home));
    match home_ok {
        Some(true) => return Ok(()),
        Some(false) => return Err(format!("inside ~/Library agents write only to iCloud Drive and cloud storage folders: {shown}")),
        None => {}
    }
    if cfg!(target_os = "macos") && on_external_volume(p) {
        return Ok(());
    }
    Err(format!("agents write only under your home folder or on an external drive: {shown}"))
}

/// `/Volumes/<name>/<something>`.
fn on_external_volume(p: &Path) -> bool {
    p.strip_prefix("/Volumes").is_ok_and(|rest| rest.components().count() >= 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("varos-guard-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Desktop")).unwrap();
        std::fs::create_dir_all(base.join("Library/Application Support")).unwrap();
        std::fs::create_dir_all(base.join("Library/Mobile Documents")).unwrap();
        std::fs::create_dir_all(base.join("Library/CloudStorage/GoogleDrive-me")).unwrap();
        std::fs::create_dir_all(base.join(".secret")).unwrap();
        std::fs::canonicalize(base).unwrap()
    }

    fn s(p: &Path) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn new_files_under_home_are_fine_existing_ones_are_not() {
        let home = temp_home("new");
        let places = Places { home: home.clone(), app: None };
        let out = home.join("Desktop/poster.pdf");
        assert_eq!(check_new_file(&s(&out), &places, None), Ok(()));
        std::fs::write(&out, b"x").unwrap();
        assert!(check_new_file(&s(&out), &places, None).unwrap_err().contains("never overwrite"));
        // Saving the document onto its own file is fine.
        assert_eq!(check_new_file(&s(&out), &places, Some(&s(&out))), Ok(()));
    }

    #[test]
    fn library_only_cloud_folders() {
        let home = temp_home("lib");
        let places = Places { home: home.clone(), app: None };
        assert_eq!(check_new_file(&s(&home.join("Library/Mobile Documents/a.svg")), &places, None), Ok(()));
        assert_eq!(check_new_file(&s(&home.join("Library/CloudStorage/GoogleDrive-me/a.svg")), &places, None), Ok(()));
        assert!(check_new_file(&s(&home.join("Library/Application Support/a.svg")), &places, None).is_err());
    }

    #[test]
    fn hidden_relative_and_missing_folders_are_refused() {
        let home = temp_home("odd");
        let places = Places { home: home.clone(), app: None };
        assert!(check_new_file(&s(&home.join(".secret/a.pdf")), &places, None).unwrap_err().contains("hidden"));
        assert!(check_new_file(&s(&home.join("Desktop/.a.pdf")), &places, None).unwrap_err().contains("hidden"));
        assert!(check_new_file(&s(&home.join("Desktop/../Desktop/a.pdf")), &places, None).is_err());
        assert!(check_new_file("Desktop/a.pdf", &places, None).unwrap_err().contains("absolute"));
        assert!(check_new_file(&s(&home.join("Nope/a.pdf")), &places, None).unwrap_err().contains("doesn't exist"));
    }

    #[test]
    fn outside_home_and_inside_the_app_are_refused() {
        let home = temp_home("out");
        let places = Places { home: home.join("Desktop"), app: Some(home.join("Library")) };
        // The test's home is Desktop, so the folder above it is outside home.
        assert!(check_new_file(&s(&home.join("a.pdf")), &places, None).unwrap_err().contains("home folder"));
        assert!(check_new_file("/etc/a.pdf", &places, None).is_err());
        let places = Places { home: home.clone(), app: Some(home.join("Desktop")) };
        assert!(check_new_file(&s(&home.join("Desktop/a.pdf")), &places, None).unwrap_err().contains("running app"));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_home_is_followed_and_refused() {
        let home = temp_home("link");
        let outside = temp_home("link-outside");
        std::os::unix::fs::symlink(&outside, home.join("Desktop/away")).unwrap();
        let places = Places { home: home.clone(), app: None };
        assert!(check_new_file(&s(&home.join("Desktop/away/a.pdf")), &places, None).unwrap_err().contains("home folder"));
    }

    #[test]
    fn export_folders() {
        let home = temp_home("folder");
        let places = Places { home: home.clone(), app: None };
        assert_eq!(check_folder(&s(&home.join("Desktop")), &places), Ok(()));
        assert!(check_folder(&s(&home.join("Missing")), &places).is_err());
        assert!(check_folder(&s(&home.join(".secret")), &places).is_err());
    }
}
