//! Temporary owner-root policy for Bridge file destinations. No root means no destinations.
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
    if forbidden(path) {
        return Err(Error::new("scope_refused", "protected destination"));
    }
    Ok(())
}
fn in_app_bundle(path: &Path, exe: &Path) -> bool {
    exe.ancestors().any(|a| a.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")) && path.starts_with(a))
}
pub fn forbidden(path: &Path) -> bool {
    path == Path::new("/")
        || ["/System", "/Applications", "/Library", "/dev", "/proc", "/sys"].iter().any(|p| path.starts_with(p))
        || path.as_os_str().to_string_lossy().split('/').any(|p| p.starts_with('.'))
        || path.components().any(|c| match c {
            std::path::Component::Normal(n) => n.to_string_lossy().starts_with('.'),
            std::path::Component::ParentDir | std::path::Component::CurDir => true,
            _ => false,
        })
        || std::env::var_os("HOME").is_some_and(|h| path.starts_with(PathBuf::from(h).join("Library")))
        || std::env::current_exe().ok().is_some_and(|exe| in_app_bundle(path, &exe))
}
/// Resolve an existing file or a new filename in an existing directory. Canonical parents stop
/// symlink escapes; a dangling symlink is refused rather than interpreted as a new file.
#[cfg(unix)]
pub fn destination(path: &Path, roots: &[PathBuf], backing: &[PathBuf]) -> Result<PathBuf, Error> {
    if forbidden(path) {
        return Err(Error::new("scope_refused", "protected destination"));
    }
    let refuse = |reason| Error::new("scope_refused", reason);
    if !path.is_absolute() {
        return Err(refuse("destination must be absolute"));
    }
    let dest = match std::fs::symlink_metadata(path) {
        Ok(_) => path.canonicalize().map_err(|_| Error::new("io_error", "destination cannot be resolved"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .ok_or_else(|| Error::new("io_error", "destination directory must exist"))?;
            parent.join(path.file_name().ok_or_else(|| refuse("destination needs filename"))?)
        }
        Err(_) => return Err(Error::new("io_error", "destination cannot be inspected")),
    };
    if ["/dev", "/proc", "/sys"].iter().any(|p| dest.starts_with(p)) {
        return Err(refuse("device and virtual filesystem destinations are refused"));
    }
    if forbidden(&dest)
        || !roots
            .iter()
            .filter_map(|r| r.canonicalize().ok().filter(|now| now == r))
            .any(|r| dest.starts_with(&r) && dest != r)
    {
        return Err(refuse("destination outside owner-granted file roots"));
    }
    for source in backing {
        if source.canonicalize().ok().as_ref() == Some(&dest) || same_file(source, &dest) {
            return Err(refuse("destination would overwrite an open backing file"));
        }
    }
    Ok(dest)
}
#[cfg(not(unix))]
pub fn destination(_path: &Path, _roots: &[PathBuf], _backing: &[PathBuf]) -> Result<PathBuf, Error> {
    Err(Error::new("unsupported", "Bridge file hosting is unavailable on this platform"))
}
#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (std::fs::metadata(a), std::fs::metadata(b)) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    false
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
        if let Some(home) = std::env::var_os("HOME") {
            let library = PathBuf::from(home).join("Library");
            assert!(forbidden(&library));
            assert!(forbidden(&library.join("a.vrs")));
        }
        let exe = Path::new("/opt/Varos.app/Contents/MacOS/varos");
        assert!(in_app_bundle(Path::new("/opt/Varos.app"), exe));
        assert!(in_app_bundle(Path::new("/opt/Varos.app/copy.vrs"), exe));
        assert!(!in_app_bundle(Path::new("/opt/Other.app/copy.vrs"), exe));
        assert!(validate_path(Path::new("/tmp/A.PDF"), "pdf").is_ok());
        assert!(validate_path(Path::new("/tmp/A.VrS"), "vrs").is_ok());
    }
    #[test]
    fn roots_backing_and_symlink_escape_refused() {
        let root = std::env::temp_dir().join(format!("bridge-roots-{}", crate::conn::random_hex(8).unwrap()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let granted = root.join("granted");
        let outside = root.join("outside");
        std::fs::create_dir(&granted).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let backing = granted.join("open.vrs");
        std::fs::write(&backing, b"source").unwrap();
        assert!(destination(&granted.join("new.pdf"), std::slice::from_ref(&granted), std::slice::from_ref(&backing))
            .is_ok());
        for path in [&outside.join("out.pdf"), &granted.join("../outside/out.pdf"), &backing] {
            assert_eq!(
                destination(path, std::slice::from_ref(&granted), std::slice::from_ref(&backing)).unwrap_err().code,
                "scope_refused"
            );
        }
        assert!(destination(&granted.join("out.pdf"), &[], &[]).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, granted.join("escape")).unwrap();
            assert!(destination(&granted.join("escape/out.pdf"), std::slice::from_ref(&granted), &[]).is_err());
            std::os::unix::fs::symlink(outside.join("missing"), granted.join("dangling")).unwrap();
            assert!(destination(&granted.join("dangling"), std::slice::from_ref(&granted), &[]).is_err());
            std::fs::hard_link(&backing, granted.join("alias.vrs")).unwrap();
            assert!(destination(
                &granted.join("alias.vrs"),
                std::slice::from_ref(&granted),
                std::slice::from_ref(&backing)
            )
            .is_err());
        }
        #[cfg(unix)]
        {
            std::fs::rename(&granted, root.join("original-grant")).unwrap();
            std::os::unix::fs::symlink(&outside, &granted).unwrap();
            assert!(destination(&granted.join("new.pdf"), std::slice::from_ref(&granted), &[]).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
