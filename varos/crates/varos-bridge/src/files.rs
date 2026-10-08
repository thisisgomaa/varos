//! Mistake-guards for open local trust: passwd home, cloud drives and external volumes; new names only.
use crate::Error;
use std::path::{Path, PathBuf};

/// Checks that need no filesystem access, also applied at host acceptance.
pub fn validate_path(path: &Path, extension: &str) -> Result<(), Error> {
    if !path.is_absolute() || path.file_name().is_none() || path.as_os_str().to_string_lossy().ends_with('/') {
        return Err(Error::new("invalid_argument", "absolute destination with a filename required"));
    }
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)) {
        return Err(Error::new("invalid_argument", format!("destination must be .{extension}")));
    }
    let home =
        crate::conn::fsutil::user_home_dir().map_err(|_| Error::new("scope_refused", "user home unavailable"))?;
    if forbidden_for_home(path, &home) || !contained(path, &home) {
        return Err(Error::new("scope_refused", "protected destination"));
    }
    Ok(())
}
fn in_app_bundle(path: &Path, exe: &Path) -> bool {
    exe.ancestors().any(|a| a.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")) && path.starts_with(a))
}
fn cloud_drive(path: &Path, home: &Path) -> bool {
    path.starts_with(home.join("Library/Mobile Documents"))
        || path.strip_prefix(home.join("Library/CloudStorage")).is_ok_and(|p| p.components().count() >= 2)
}
fn contained(path: &Path, home: &Path) -> bool {
    home != Path::new("/")
        && ((path.starts_with(home) && (!path.starts_with(home.join("Library")) || cloud_drive(path, home)))
            || path.strip_prefix("/Volumes").is_ok_and(|p| p.components().count() >= 2))
}
fn forbidden_for_home(path: &Path, home: &Path) -> bool {
    path == Path::new("/")
        || ["/System", "/Applications", "/Library", "/dev", "/proc", "/sys"].iter().any(|p| path.starts_with(p))
        || path.as_os_str().to_string_lossy().split('/').any(|p| p.starts_with('.'))
        || path.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::CurDir))
        || (path.starts_with(home.join("Library")) && !cloud_drive(path, home))
        || std::env::current_exe().ok().is_some_and(|exe| in_app_bundle(path, &exe))
}
pub fn forbidden(path: &Path) -> bool {
    crate::conn::fsutil::user_home_dir().map_or(true, |home| forbidden_for_home(path, &home))
}
/// New names only, under the user's home, cloud drives or external volumes. No owner grant.
#[cfg(unix)]
pub fn destination(path: &Path, home: &Path) -> Result<PathBuf, Error> {
    let refuse = |reason| Error::new("scope_refused", reason);
    if !path.is_absolute() || forbidden_for_home(path, home) || !contained(path, home) {
        return Err(refuse("protected destination"));
    }
    match std::fs::symlink_metadata(path) {
        Ok(_) => return Err(Error::new("save_conflict", "destination exists; choose a fresh filename")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(Error::new("io_error", "destination cannot be inspected")),
    }
    let parent = path.parent().ok_or_else(|| refuse("destination needs parent"))?;
    let canonical = parent.canonicalize().map_err(|_| Error::new("io_error", "destination directory must exist"))?;
    // Resolve aliases before containment checks; the writer pins this canonical directory.
    let dest = canonical.join(path.file_name().ok_or_else(|| refuse("destination needs filename"))?);
    let home = home.canonicalize().map_err(|_| refuse("home cannot be resolved"))?;
    if forbidden_for_home(&dest, &home) || !contained(&dest, &home) {
        return Err(refuse("destination must be under user home, an external volume or a cloud drive"));
    }
    Ok(dest)
}
#[cfg(not(unix))]
pub fn destination(_path: &Path, _home: &Path) -> Result<PathBuf, Error> {
    Err(Error::new("unsupported", "Bridge file hosting is unavailable on this platform"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn protected_paths_and_extensions() {
        for path in [
            "/",
            "/System",
            "/System/a.vrs",
            "/Applications",
            "/Applications/a.vrs",
            "/Library",
            "/Library/a.vrs",
            "/tmp/.hidden/a.vrs",
            "/tmp/.a.vrs",
            "/tmp/./a.vrs",
            "/tmp/../a.vrs",
        ] {
            assert!(forbidden(Path::new(path)), "{path}");
        }
        if let Ok(home) = crate::conn::fsutil::user_home_dir() {
            let library = home.join("Library");
            assert!(forbidden(&library));
            assert!(forbidden(&library.join("a.vrs")));
        }
        let exe = Path::new("/opt/Varos.app/Contents/MacOS/varos");
        assert!(in_app_bundle(Path::new("/opt/Varos.app"), exe));
        assert!(in_app_bundle(Path::new("/opt/Varos.app/copy.vrs"), exe));
        assert!(!in_app_bundle(Path::new("/opt/Other.app/copy.vrs"), exe));
        let home = crate::conn::fsutil::user_home_dir().unwrap();
        assert!(validate_path(&home.join("A.PDF"), "pdf").is_ok());
        assert!(validate_path(&home.join("A.VrS"), "vrs").is_ok());
        for root in [
            "Library/Mobile Documents",
            "Library/CloudStorage/Dropbox",
            "Library/CloudStorage/GoogleDrive",
            "Library/CloudStorage/OneDrive",
        ] {
            assert!(validate_path(&home.join(root).join("new.pdf"), "pdf").is_ok());
        }
        assert!(validate_path(Path::new("/Volumes/External/new.pdf"), "pdf").is_ok());
        assert_eq!(
            validate_path(&home.join("Library/Application Support/new.pdf"), "pdf").unwrap_err().code,
            "scope_refused"
        );
        for path in ["/tmp/A.pdf", "/private/var/A.pdf", "/Volumes/A.pdf"] {
            assert_eq!(validate_path(Path::new(path), "pdf").unwrap_err().code, "scope_refused");
        }
        assert!(!contained(Path::new("/anything/a.pdf"), Path::new("/")));
    }
    #[test]
    fn fake_home_needs_no_grant_and_refuses_overwrite_and_links() {
        let home = std::env::temp_dir().join(format!("bridge-home-{}", crate::conn::random_hex(8).unwrap()));
        std::fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        std::fs::create_dir(home.join("Library")).unwrap();
        assert!(destination(&home.join("new.vrs"), &home).is_ok());
        assert!(destination(&home.join("new.pdf"), &home).is_ok());
        assert!(destination(&home.join("Library/new.vrs"), &home).is_err());
        for root in [
            "Library/Mobile Documents",
            "Library/CloudStorage/Dropbox",
            "Library/CloudStorage/GoogleDrive",
            "Library/CloudStorage/OneDrive",
        ] {
            std::fs::create_dir_all(home.join(root)).unwrap();
            assert!(destination(&home.join(root).join("new.pdf"), &home).is_ok());
        }
        std::fs::create_dir_all(home.join("Library/Application Support")).unwrap();
        assert_eq!(
            destination(&home.join("Library/Application Support/new.pdf"), &home).unwrap_err().code,
            "scope_refused"
        );
        assert!(destination(&home.join(".hidden.vrs"), &home).is_err());
        let backing = home.join("open.vrs");
        std::fs::write(&backing, b"original").unwrap();
        std::fs::hard_link(&backing, home.join("alias.vrs")).unwrap();
        std::os::unix::fs::symlink(&backing, home.join("link.vrs")).unwrap();
        std::os::unix::fs::symlink(&home, home.join("parent-link")).unwrap();
        for path in [backing, home.join("alias.vrs"), home.join("link.vrs")] {
            assert_eq!(destination(&path, &home).unwrap_err().code, "save_conflict");
        }
        assert!(destination(&home.join("parent-link/new.pdf"), &home).is_ok());
        std::os::unix::fs::symlink(home.parent().unwrap(), home.join("escape")).unwrap();
        assert!(destination(&home.join("escape/new.pdf"), &home).is_err());
        assert!(destination(&home.parent().unwrap().join("outside.pdf"), &home).is_err());
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(all(test, unix))]
mod environment_tests {
    use super::*;
    #[test]
    fn passwd_home_ignores_unset_or_forged_home() {
        // Run in a subprocess so environment mutation cannot race other tests.
        const CHILD: &str = "VAROS_HOME_POLICY_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let home = crate::conn::fsutil::user_home_dir().unwrap();
            assert!(validate_path(&home.join("new.pdf"), "pdf").is_ok());
            assert_eq!(
                validate_path(&home.join("Library/Application Support/new.pdf"), "pdf").unwrap_err().code,
                "scope_refused"
            );
            assert!(destination(&home.join(format!("bridge-home-test-{}.pdf", std::process::id())), &home).is_ok());
            return;
        }
        for forged in [None, Some("/")] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child
                .args(["--exact", "files::environment_tests::passwd_home_ignores_unset_or_forged_home"])
                .env(CHILD, "1");
            if let Some(value) = forged {
                child.env("HOME", value);
            } else {
                child.env_remove("HOME");
            }
            assert!(child.status().unwrap().success());
        }
    }
}
