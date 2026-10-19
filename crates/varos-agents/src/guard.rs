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
//! The desktop app checks these twice: once on the request's parameters before it runs anything
//! (a quick refusal, nothing half done), and again at every file the request actually writes
//! ([`Writes`]), with the names the command made up itself (Export for Screens' subfolders and
//! numbered files, a template's replaced extension, a batch's outputs).
//!
//! A path is checked, then written: another program swapping in a link between the two can still
//! win that race. The person at the keyboard is never guarded.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};

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
    check_file_place(path, places)
}

/// Where a file at `path` would go may take an agent's file: an absolute, plain path into an
/// existing folder where agents write. Whether a file is there already is not looked at: for a
/// command that changes the name it is given (Save as Template puts its own extension on it), the
/// check at the write itself ([`Writes`]) sees the real name.
pub fn check_file_place(path: &str, places: &Places) -> Result<(), String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("give an absolute path (agents' files are never relative): {path}"));
    }
    plain_components(p, path)?;
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
    allowed_place(&real.join("x"), places, path)?;
    // A visible link into a hidden folder is a hidden folder.
    plain_components(&real, path)
}

/// May an agent write a set of files whose names the command makes up (Export for Screens) into
/// the folder `path`? Only an existing, empty one (hidden files such as `.DS_Store` aside), so none
/// of the made-up names can be a file that is already there.
pub fn check_empty_folder(path: &str, places: &Places) -> Result<(), String> {
    check_folder(path, places)?;
    let mut entries = std::fs::read_dir(path).map_err(|e| format!("{path}: {e}"))?;
    if entries.any(|e| e.is_ok_and(|e| !e.file_name().to_string_lossy().starts_with('.'))) {
        return Err(format!(
            "{path} is not empty; agents export a set of files only into an empty folder (they never overwrite one): choose or make an empty folder"
        ));
    }
    Ok(())
}

/// The check on every file one agent request actually writes (see the module notes): what
/// [`check_new_file`] says for a new file, an existing folder only where agents may write, and the
/// files and folders this request made itself may be written again (a batch writing over its own
/// output, a folder a command made for its files).
#[derive(Debug)]
pub struct Writes {
    places: Places,
    own_file: Option<String>,
    made: Mutex<HashSet<PathBuf>>,
}

impl Writes {
    /// The check for one request; `own_file`: the document's own file, which a plain Save may write
    /// over.
    pub fn new(places: Places, own_file: Option<String>) -> Self {
        Self { places, own_file, made: Mutex::default() }
    }

    /// May the request write (a file or a new folder) at `path` now? `Err` says why not.
    pub fn check(&self, path: &str) -> Result<(), String> {
        let p = Path::new(path);
        if !p.is_absolute() {
            return Err(format!("give an absolute path (agents' files are never relative): {path}"));
        }
        plain_components(p, path)?;
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        if made.contains(p) {
            return Ok(());
        }
        // Links followed: a link to a folder is checked as that folder.
        if let Ok(m) = std::fs::metadata(p)
            && m.is_dir()
        {
            // An existing folder (making it again changes nothing; writing a file onto it fails).
            return check_folder(path, &self.places);
        }
        check_new_file(path, &self.places, self.own_file.as_deref())?;
        made.insert(p.to_path_buf());
        Ok(())
    }
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

    #[cfg(unix)]
    #[test]
    fn a_visible_link_into_a_hidden_folder_is_hidden() {
        let home = temp_home("hidden-link");
        std::os::unix::fs::symlink(home.join(".secret"), home.join("Desktop/plain")).unwrap();
        let places = Places { home: home.clone(), app: None };
        assert!(check_folder(&s(&home.join("Desktop/plain")), &places).unwrap_err().contains("hidden"));
        assert!(check_new_file(&s(&home.join("Desktop/plain/a.pdf")), &places, None).unwrap_err().contains("hidden"));
        assert!(Writes::new(places, None).check(&s(&home.join("Desktop/plain"))).unwrap_err().contains("hidden"));
    }

    #[test]
    fn a_set_of_files_goes_only_into_an_empty_folder() {
        let home = temp_home("empty");
        let places = Places { home: home.clone(), app: None };
        let out = home.join("Desktop/out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join(".DS_Store"), b"x").unwrap();
        assert_eq!(check_empty_folder(&s(&out), &places), Ok(()), "hidden files don't count");
        std::fs::write(out.join("icon.png"), b"x").unwrap();
        assert!(check_empty_folder(&s(&out), &places).unwrap_err().contains("not empty"));
        assert!(check_empty_folder(&s(&home.join("Desktop/missing")), &places).is_err());
    }

    /// The check at the files a request really writes: new files and one new folder level, again
    /// what it made itself, never something that was there, never a folder tree.
    #[test]
    fn writes_are_checked_where_they_happen() {
        let home = temp_home("writes");
        let places = Places { home: home.clone(), app: None };
        let desk = home.join("Desktop");
        let own = desk.join("doc.vectorcraft");
        std::fs::write(&own, b"doc").unwrap();
        std::fs::write(desk.join("taken.png"), b"x").unwrap();
        let w = Writes::new(places, Some(s(&own)));
        assert_eq!(w.check(&s(&own)), Ok(()), "a save onto the document's own file");
        assert!(w.check(&s(&desk.join("taken.png"))).unwrap_err().contains("never overwrite"));
        // Export for Screens: a subfolder of the destination, then files in it, twice.
        let sub = desk.join("1x");
        assert_eq!(w.check(&s(&sub)), Ok(()));
        std::fs::create_dir(&sub).unwrap();
        assert_eq!(w.check(&s(&sub.join("a.png"))), Ok(()));
        std::fs::write(sub.join("a.png"), b"x").unwrap();
        assert_eq!(w.check(&s(&sub.join("a.png"))), Ok(()), "its own new file again");
        assert_eq!(w.check(&s(&desk)), Ok(()), "an existing folder (create_dir_all of it is a no-op)");
        assert!(w.check(&s(&desk.join("a/b"))).unwrap_err().contains("doesn't exist"), "no folder trees");
        assert!(w.check("relative.png").unwrap_err().contains("absolute"));
        assert!(w.check(&s(&home.join("Library/Application Support/x.png"))).is_err());
        // Another request starts afresh: the file it didn't make is there now.
        let next = Writes::new(Places { home: home.clone(), app: None }, None);
        assert!(next.check(&s(&sub.join("a.png"))).unwrap_err().contains("never overwrite"));
    }
}
