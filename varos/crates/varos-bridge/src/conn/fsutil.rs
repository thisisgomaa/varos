//! Owner-only filesystem helpers: private directories, no-follow creation, atomic publication.
//! Parent directories are owner-only (0700), so only this uid can race entries inside them;
//! every read re-checks type, owner and mode (ADR-0011 §2, §7 symlink threats).
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// Hard cap for any JSON record we read from these directories.
pub const MAX_RECORD: u64 = 64 * 1024;

#[cfg(unix)]
fn euid() -> u32 {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() }
}

/// The per-user temporary directory from the platform API, not the inherited `$TMPDIR`.
#[cfg(target_os = "macos")]
pub fn user_temp_dir() -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let mut buf = vec![0u8; 1024];
    // SAFETY: buf is valid for buf.len() bytes; confstr writes a NUL-terminated string.
    let n = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, buf.as_mut_ptr().cast(), buf.len()) };
    if n == 0 || n > buf.len() {
        return Err(io::Error::other("confstr(_CS_DARWIN_USER_TEMP_DIR) failed"));
    }
    buf.truncate(n - 1);
    let dir = PathBuf::from(std::ffi::OsString::from_vec(buf));
    check_private_dir(&dir)?;
    Ok(dir)
}
/// Linux: validated `XDG_RUNTIME_DIR`; no shared `/tmp` fallback (ADR-0011 §2).
#[cfg(all(unix, not(target_os = "macos")))]
pub fn user_temp_dir() -> io::Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "XDG_RUNTIME_DIR is not set"))?;
    check_private_dir(&dir)?;
    Ok(dir)
}
#[cfg(not(unix))]
pub fn user_temp_dir() -> io::Result<PathBuf> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "Bridge discovery is not implemented on this platform yet"))
}

/// Home directory from the password database (not `$HOME`).
#[cfg(unix)]
pub fn user_home_dir() -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut buf = vec![0u8; 16 * 1024];
    let mut out: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: all pointers reference live, correctly sized buffers for the duration of the call.
    let rc = unsafe { libc::getpwuid_r(euid(), &mut pwd, buf.as_mut_ptr().cast(), buf.len(), &mut out) };
    if rc != 0 || out.is_null() || pwd.pw_dir.is_null() {
        return Err(io::Error::other("home directory lookup failed"));
    }
    // SAFETY: pw_dir points into buf and is NUL-terminated per getpwuid_r's contract.
    let dir = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(dir.to_bytes())))
}
#[cfg(not(unix))]
pub fn user_home_dir() -> io::Result<PathBuf> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "Bridge trust store is not implemented on this platform yet"))
}

/// A real directory (not a link), owned by this user, no group/other permissions.
pub fn check_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path)?;
        if !m.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a directory", path.display()),
            ));
        }
        if m.uid() != euid() || m.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} must be owned by this user with mode 0700", path.display()),
            ));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(io::Error::new(io::ErrorKind::Unsupported, "owner-only directories are Unix-only in C1"))
    }
}
/// Create (0700) or validate a private directory. Missing ancestors are created normally;
/// only `path` itself must be private.
pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        match std::fs::symlink_metadata(path) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                match std::fs::DirBuilder::new().mode(0o700).create(path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e),
                }
            }
            Err(e) => return Err(e),
        }
    }
    check_private_dir(path)
}
/// An owner-only regular file (not a link).
pub fn check_private_file(path: &Path) -> io::Result<std::fs::Metadata> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path)?;
        if !m.file_type().is_file() || m.uid() != euid() || m.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} must be an owner-only regular file", path.display()),
            ));
        }
        Ok(m)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(io::Error::new(io::ErrorKind::Unsupported, "owner-only files are Unix-only in C1"))
    }
}
/// Open an owner-only regular file for reading with O_NOFOLLOW|O_NONBLOCK and check the *opened*
/// descriptor, so a swapped link fails and a planted FIFO cannot block the caller.
pub fn open_private(path: &Path) -> io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file =
            std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)?;
        check_opened(&file, path)?;
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        check_private_file(path)?;
        std::fs::File::open(path)
    }
}
/// Type/owner/mode check on an already-open descriptor.
pub fn check_opened(file: &std::fs::File, path: &Path) -> io::Result<std::fs::Metadata> {
    let m = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !m.file_type().is_file() || m.uid() != euid() || m.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} must be an owner-only regular file", path.display()),
            ));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(m)
}
/// Read a bounded owner-only regular file (see [`open_private`]).
pub fn read_private(path: &Path) -> io::Result<Vec<u8>> {
    let file = open_private(path)?;
    let mut bytes = vec![];
    file.take(MAX_RECORD + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} exceeds {MAX_RECORD} bytes", path.display()),
        ));
    }
    Ok(bytes)
}
/// Create a new 0600 file exclusively (never through an existing name or link).
pub fn create_private(path: &Path) -> io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(io::Error::new(io::ErrorKind::Unsupported, "owner-only files are Unix-only in C1"))
    }
}
/// Publish `bytes` at `path` by writing a fresh 0600 sibling and renaming over the name.
/// Rename replaces a directory entry (even a link) and never writes through it.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| io::Error::other("no parent directory"))?;
    check_private_dir(dir)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("no file name"))?.to_string_lossy();
    let mut nonce = [0u8; 6];
    getrandom::fill(&mut nonce).map_err(|e| io::Error::other(e.to_string()))?;
    let tmp = dir.join(format!(".{name}.{}.tmp", super::hex(&nonce)));
    let result = (|| {
        let mut f = create_private(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
/// Remove a regular file or socket we own; never follows or recurses.
pub fn remove_owned(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let m = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        let kind = m.file_type();
        if m.uid() != euid() || !(kind.is_file() || kind.is_socket() || kind.is_symlink()) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("refusing to remove {}", path.display()),
            ));
        }
        std::fs::remove_file(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Advisory exclusive lock on a private lock file (trust-store transactions).
pub struct Lock {
    #[cfg(unix)]
    _file: std::fs::File,
}
pub fn lock(path: &Path) -> io::Result<Lock> {
    #[cfg(unix)]
    {
        use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        check_opened(&file, path)?;
        // SAFETY: the fd is owned by `file`, which outlives the lock.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Lock { _file: file })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(io::Error::new(io::ErrorKind::Unsupported, "trust-store locking is Unix-only in C1"))
    }
}
