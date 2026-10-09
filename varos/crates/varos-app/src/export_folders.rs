//! Advanced Bridge sub-folders stay below the requested export directory.
//! Unix directory descriptors and O_NOFOLLOW match the incumbent pinned writer.
#[cfg(unix)]
pub fn ensure(root: &std::path::Path, folder: &std::path::Path) -> Result<(), String> {
    use std::{
        ffi::CString,
        fs::File,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
        path::Component,
    };
    fn owned(fd: i32) -> Result<File, String> {
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: successful open/openat hands out a new owned descriptor.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    fn name(value: &std::ffi::OsStr) -> Result<CString, String> {
        CString::new(value.as_bytes()).map_err(|_| "Invalid export directory".into())
    }
    if !root.is_absolute() {
        return Err("Export root must be absolute".into());
    }
    let relative = folder.strip_prefix(root).map_err(|_| "Export folder is outside the requested directory")?;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    // SAFETY: static NUL-terminated name and valid directory flags.
    let mut dir = owned(unsafe { libc::open(c"/".as_ptr(), flags) })?;
    for component in root.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(value) => {
                let value = name(value)?;
                // SAFETY: valid directory descriptor and owned NUL-terminated name.
                dir = owned(unsafe { libc::openat(dir.as_raw_fd(), value.as_ptr(), flags) })?;
            }
            _ => return Err("Invalid export root".into()),
        }
    }
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err("Invalid export sub-folder".into());
        };
        let value = name(value)?;
        // SAFETY: mkdirat is relative to the held directory; no symlink is followed.
        if unsafe { libc::mkdirat(dir.as_raw_fd(), value.as_ptr(), 0o755) } < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error.to_string());
            }
        }
        let parent = dir;
        // SAFETY: openat refuses an existing symlink or non-directory.
        dir = owned(unsafe { libc::openat(parent.as_raw_fd(), value.as_ptr(), flags) })?;
        parent.sync_all().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[cfg(not(unix))]
pub fn ensure(_: &std::path::Path, _: &std::path::Path) -> Result<(), String> {
    Err("Bridge file writes unavailable on this platform".into())
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn creates_subfolders_and_refuses_symlinks_and_parent_escape() {
        let dir = std::env::temp_dir().join(format!("lane-c-folders-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        ensure(&dir, &dir.join("svg")).unwrap();
        ensure(&dir, &dir.join("svg")).unwrap();
        std::os::unix::fs::symlink(dir.join("svg"), dir.join("link")).unwrap();
        assert!(ensure(&dir, &dir.join("link")).is_err());
        assert!(ensure(&dir, &dir.join("../outside")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// Authorize the existing root before making any Advanced-only directories.
/// Normalize every destination so the pinned writer sees the same canonical root.
pub fn prepare_screen(
    screen: &mut crate::file_jobs::ScreenJob,
    home: &std::path::Path,
) -> Result<(), varos_bridge::Error> {
    let Some(root) = screen.folder_root.clone() else {
        return Ok(());
    };
    let probe = root.join(format!("Varos-export-check-{}.svg", varos_app::storage::checksum::new_nonce()));
    let authorized = varos_bridge::files::destination(&probe, home)?;
    let canonical =
        authorized.parent().ok_or_else(|| varos_bridge::Error::new("invalid_argument", "Missing export root"))?;
    fn prepare(
        job: &mut crate::file_jobs::ScreenJob,
        root: &std::path::Path,
        canonical: &std::path::Path,
        home: &std::path::Path,
    ) -> Result<(), varos_bridge::Error> {
        let relative = job
            .job
            .dest
            .strip_prefix(root)
            .map_err(|_| varos_bridge::Error::new("scope_refused", "Export escaped the requested directory"))?;
        let dest = canonical.join(relative);
        if let Some(folder) = dest.parent() {
            ensure(canonical, folder).map_err(|e| varos_bridge::Error::new("io_error", e))?;
        }
        job.job.dest = varos_bridge::files::destination(&dest, home)?;
        job.folder_root = Some(canonical.to_path_buf());
        for extra in &mut job.additional_jobs {
            prepare(extra, root, canonical, home)?;
        }
        Ok(())
    }
    prepare(screen, &root, canonical, home)
}
