//! Pinned-directory adapter for the existing durable writer. Bridge never follows a changed
//! parent or final symlink. Fresh destinations publish with linkat, or macOS exclusive rename (no replacement).
#[cfg(unix)]
mod unix {
    use std::{
        ffi::CString,
        fs::File,
        io::{self, Read},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::PermissionsExt},
        },
        path::{Path, PathBuf},
    };
    use varos_app::storage::durable::{FileMeta, Fingerprint, FsPort, SyncWrite};
    #[derive(Clone, Copy, Debug)]
    pub enum PinnedError {
        SaveConflict,
        Superseded,
        Busy,
        IoError,
        NetworkVolume,
    }
    impl From<io::Error> for PinnedError {
        fn from(_: io::Error) -> Self {
            Self::IoError
        }
    }
    impl PinnedError {
        pub fn bridge(self) -> varos_bridge::Error {
            match self {
                Self::SaveConflict => {
                    varos_bridge::Error::new("save_conflict", "destination exists or backing file changed")
                }
                Self::NetworkVolume => varos_bridge::Error::new("scope_refused", "network volume not supported"),
                Self::Busy => varos_bridge::Error::new("busy", "publication lock busy"),
                Self::Superseded => varos_bridge::Error::new("cancelled", "autosave superseded"),
                Self::IoError => varos_bridge::Error::new("io_error", "file IO failed"),
            }
        }
    }
    pub struct Pinned {
        failure: std::sync::Mutex<Option<PinnedError>>,
        dir: File,
        parent: PathBuf,
        dest: PathBuf,
        expected: Option<Fingerprint>,
        fresh: bool,
        permit: Option<varos_app::storage::publication::Permit>,
        published: std::sync::Mutex<Option<Fingerprint>>,
    }
    fn name(s: &std::ffi::OsStr) -> io::Result<CString> {
        CString::new(s.as_bytes()).map_err(|_| io::Error::other("invalid filename"))
    }
    fn fd(result: i32) -> io::Result<File> {
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: successful open/openat returns a new owned descriptor.
            Ok(unsafe { File::from_raw_fd(result) })
        }
    }
    fn check(result: i32) -> io::Result<()> {
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    /// Returns true when exclusive rename consumed the temporary name.
    #[cfg(target_os = "macos")]
    fn publish_fresh(
        link: impl FnOnce() -> io::Result<()>,
        rename_exclusive: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<bool> {
        match link() {
            Ok(()) => Ok(false),
            Err(e) if matches!(e.raw_os_error(), Some(libc::ENOTSUP | libc::EPERM | libc::EXDEV)) => {
                rename_exclusive()?;
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }
    #[cfg(all(test, target_os = "macos"))]
    #[test]
    fn unsupported_hardlinks_use_exclusive_rename() {
        use std::cell::Cell;
        let called = Cell::new(false);
        assert!(publish_fresh(
            || Err(io::Error::from_raw_os_error(libc::ENOTSUP)),
            || {
                called.set(true);
                Ok(())
            }
        )
        .unwrap());
        assert!(called.get());
        assert!(!publish_fresh(|| Ok(()), || panic!("link succeeded")).unwrap());
        let error = publish_fresh(
            || Err(io::Error::from_raw_os_error(libc::ENOTSUP)),
            || Err(io::Error::from_raw_os_error(libc::EEXIST)),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(publish_fresh(
            || Err(io::Error::from_raw_os_error(libc::EEXIST)),
            || panic!("must not retry collision")
        )
        .is_err());
    }
    impl Pinned {
        pub fn new(dest: &Path, expected: Option<&Fingerprint>, fresh: bool) -> Result<Self, PinnedError> {
            let parent = dest.parent().ok_or_else(|| io::Error::other("missing parent"))?;
            if !parent.is_absolute() {
                return Err(PinnedError::IoError);
            }
            let dir = Self::open_directory(parent)?;
            #[cfg(target_os = "macos")]
            {
                let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
                // SAFETY: writable statfs storage and a valid descriptor.
                check(unsafe { libc::fstatfs(dir.as_raw_fd(), stat.as_mut_ptr()) })?;
                // SAFETY: fstatfs succeeded and initialized stat.
                if unsafe { stat.assume_init() }.f_flags & libc::MNT_LOCAL as u32 == 0 {
                    return Err(PinnedError::NetworkVolume);
                }
            }
            let pinned = Self {
                failure: std::sync::Mutex::new(None),
                dir,
                parent: parent.to_owned(),
                dest: dest.to_owned(),
                expected: expected.cloned(),
                fresh,
                permit: None,
                published: std::sync::Mutex::new(None),
            };
            pinned.check_destination()?;
            Ok(pinned)
        }
        pub fn with_permit(mut self, permit: varos_app::storage::publication::Permit) -> io::Result<Self> {
            // Cooperative autosave hosts own the pinned directory until completion. Refuse unsupported coordination.
            // SAFETY: valid descriptor; nonblocking flock never changes directory contents.
            check(unsafe { libc::flock(self.dir.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) })?;
            self.permit = Some(permit);
            Ok(self)
        }
        pub fn published(&self) -> Option<Fingerprint> {
            self.published.lock().ok().and_then(|p| *p)
        }
        fn file_fingerprint(f: File) -> io::Result<Fingerprint> {
            use std::os::unix::fs::MetadataExt;
            let m = f.metadata()?;
            if !m.is_file() || m.nlink() != 1 || m.permissions().readonly() {
                return Err(io::Error::other("unsafe destination"));
            }
            let hash = varos_app::storage::durable::hash_regular_file(f.try_clone()?)?;
            let after = f.metadata()?;
            if m.len() != after.len() || m.modified().ok() != after.modified().ok() {
                return Err(io::Error::other("file changed while reading"));
            }
            Ok(Fingerprint {
                len: m.len(),
                modified: m.modified().ok(),
                hash,
                identity: Some((m.dev(), m.ino())),
                parent_identity: None,
            })
        }
        fn open_directory(parent: &Path) -> io::Result<File> {
            let root = CString::new("/").expect("literal");
            // SAFETY: C string is valid; flags open a directory without following symlinks.
            let mut dir =
                fd(unsafe { libc::open(root.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) })?;
            for part in parent.components() {
                if let std::path::Component::Normal(part) = part {
                    let part = name(part)?;
                    // SAFETY: valid directory descriptor and C string; returned descriptor is owned.
                    dir = fd(unsafe {
                        libc::openat(
                            dir.as_raw_fd(),
                            part.as_ptr(),
                            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                        )
                    })?;
                } else if !matches!(part, std::path::Component::RootDir) {
                    return Err(io::Error::other("noncanonical parent"));
                }
            }
            Ok(dir)
        }
        fn parent_unchanged(&self) -> io::Result<()> {
            use std::os::unix::fs::MetadataExt;
            let now = Self::open_directory(&self.parent)?.metadata()?;
            let held = self.dir.metadata()?;
            if now.dev() != held.dev() || now.ino() != held.ino() {
                return Err(io::Error::other("destination directory changed"));
            }
            Ok(())
        }
        fn leaf(&self, path: &Path) -> io::Result<CString> {
            if path.parent() != Some(self.parent.as_path()) {
                return Err(io::Error::other("operation outside pinned directory"));
            }
            name(path.file_name().ok_or_else(|| io::Error::other("missing filename"))?)
        }
        fn open(&self, path: &Path) -> io::Result<File> {
            let leaf = self.leaf(path)?;
            // SAFETY: descriptor and C string are valid; no final symlink is followed.
            fd(unsafe {
                libc::openat(
                    self.dir.as_raw_fd(),
                    leaf.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                )
            })
        }
        pub fn error(&self) -> varos_bridge::Error {
            self.failure.lock().ok().and_then(|error| *error).unwrap_or(PinnedError::IoError).bridge()
        }
        fn record(&self, error: PinnedError) -> io::Error {
            if let Ok(mut failure) = self.failure.lock() {
                *failure = Some(error);
            }
            io::Error::other("pinned publication refused")
        }
        fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint> {
            use std::os::unix::fs::MetadataExt;
            let mut got = Self::file_fingerprint(self.open(path)?)?;
            let parent = self.dir.metadata()?;
            got.parent_identity = Some((parent.dev(), parent.ino()));
            Ok(got)
        }
        fn check_destination(&self) -> Result<(), PinnedError> {
            match self.open(&self.dest) {
                Err(e) if self.fresh && e.kind() == io::ErrorKind::NotFound => Ok(()),
                Ok(_) if self.fresh => Err(PinnedError::SaveConflict),
                Ok(f) => {
                    let m = f.metadata()?;
                    let got = self.fingerprint(&self.dest).map_err(|_| PinnedError::SaveConflict)?;
                    if !m.is_file() || Some(&got) != self.expected.as_ref() {
                        Err(PinnedError::SaveConflict)
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(if !self.fresh { PinnedError::SaveConflict } else { e.into() }),
            }
        }
    }
    impl FsPort for Pinned {
        fn create_new(&self, path: &Path) -> io::Result<Box<dyn SyncWrite>> {
            let leaf = self.leaf(path)?;
            // SAFETY: valid directory and string; O_EXCL prevents following a destination link.
            let f = fd(unsafe {
                libc::openat(
                    self.dir.as_raw_fd(),
                    leaf.as_ptr(),
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    0o666,
                )
            })?;
            Ok(Box::new(f))
        }
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            if to != self.dest {
                return Err(io::Error::other("unexpected destination"));
            }
            // Hash the temp outside edit admission; preserve its identity as the completion baseline.
            let baseline = Some(self.fingerprint(from)?);
            self.check_destination().map_err(|e| self.record(e))?;
            let _publication = if let Some(p) = &self.permit {
                let guard = p.gate.lock.try_lock().map_err(|_| self.record(PinnedError::Busy))?;
                if !p.valid() {
                    return Err(self.record(PinnedError::Superseded));
                }
                Some(guard)
            } else {
                None
            };
            self.parent_unchanged().map_err(|_| self.record(PinnedError::SaveConflict))?;
            if let Some(expected) = &self.expected {
                use std::os::unix::fs::MetadataExt;
                let m = self
                    .open(&self.dest)
                    .and_then(|f| f.metadata())
                    .map_err(|_| self.record(PinnedError::SaveConflict))?;
                if expected.identity != Some((m.dev(), m.ino()))
                    || expected.len != m.len()
                    || expected.modified != m.modified().ok()
                    || m.nlink() != 1
                    || m.permissions().readonly()
                {
                    return Err(self.record(PinnedError::SaveConflict));
                }
            }
            let from = self.leaf(from)?;
            let to = self.leaf(to)?;
            if self.fresh {
                let link = || {
                    // SAFETY: names are restricted to the pinned directory; linkat refuses overwrite.
                    check(unsafe {
                        libc::linkat(self.dir.as_raw_fd(), from.as_ptr(), self.dir.as_raw_fd(), to.as_ptr(), 0)
                    })
                };
                #[cfg(target_os = "macos")]
                let published = publish_fresh(link, || {
                    // SAFETY: valid pinned descriptor and names; RENAME_EXCL never replaces an existing name.
                    check(unsafe {
                        libc::renameatx_np(
                            self.dir.as_raw_fd(),
                            from.as_ptr(),
                            self.dir.as_raw_fd(),
                            to.as_ptr(),
                            libc::RENAME_EXCL,
                        )
                    })
                });
                #[cfg(not(target_os = "macos"))]
                let published = link().map(|()| false);
                let renamed = published.map_err(|e| {
                    self.record(if e.kind() == io::ErrorKind::AlreadyExists {
                        PinnedError::SaveConflict
                    } else {
                        PinnedError::IoError
                    })
                })?;
                if let Ok(mut published) = self.published.lock() {
                    *published = baseline;
                }
                if renamed {
                    Ok(())
                } else {
                    // SAFETY: deletes only our temporary name in the pinned directory.
                    check(unsafe { libc::unlinkat(self.dir.as_raw_fd(), from.as_ptr(), 0) })
                }
            } else {
                // SAFETY: atomic replacement stays in the pinned directory.
                check(unsafe {
                    libc::renameat(self.dir.as_raw_fd(), from.as_ptr(), self.dir.as_raw_fd(), to.as_ptr())
                })?;
                if let Ok(mut published) = self.published.lock() {
                    *published = baseline;
                }
                Ok(())
            }
        }
        fn sync_dir(&self, _: &Path) -> io::Result<()> {
            let mut dir = self.dir.try_clone()?;
            SyncWrite::sync_all(&mut dir)
        }
        fn remove_file(&self, path: &Path) -> io::Result<()> {
            let leaf = self.leaf(path)?; /* SAFETY: leaf is in pinned directory. */
            check(unsafe { libc::unlinkat(self.dir.as_raw_fd(), leaf.as_ptr(), 0) })
        }
        fn metadata(&self, path: &Path) -> io::Result<FileMeta> {
            let m = if path == self.parent { self.dir.metadata()? } else { self.open(path)?.metadata()? };
            Ok(FileMeta {
                len: m.len(),
                modified: m.modified().ok(),
                is_dir: m.is_dir(),
                readonly: m.permissions().readonly(),
                permissions: m.permissions(),
            })
        }
        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            let mut bytes = vec![];
            self.open(path)?.read_to_end(&mut bytes)?;
            Ok(bytes)
        }
        fn set_permissions(&self, path: &Path, perms: std::fs::Permissions) -> io::Result<()> {
            let f = self.open(path)?; /* SAFETY: owned valid descriptor. */
            check(unsafe { libc::fchmod(f.as_raw_fd(), perms.mode() as libc::mode_t) })
        }
        fn resolve_link(&self, path: &Path) -> io::Result<PathBuf> {
            self.leaf(path)?;
            Ok(path.to_owned())
        }
        fn remove_dir(&self, _: &Path) -> io::Result<()> {
            Err(io::Error::other("unsupported"))
        }
        fn create_dir_all(&self, _: &Path) -> io::Result<()> {
            Err(io::Error::other("unsupported"))
        }
        fn read_dir(&self, _: &Path) -> io::Result<Vec<PathBuf>> {
            Err(io::Error::other("unsupported"))
        }
    }
}
#[cfg(unix)]
pub use unix::Pinned;

#[cfg(all(test, unix))]
mod tests {
    use super::Pinned;
    use varos_app::storage::{
        checksum::new_nonce,
        durable::{fingerprint, write_replace, RealFs},
    };
    #[test]
    fn pinned_writer_refuses_replace_races_and_parent_symlinks() {
        let root = std::env::temp_dir().join(format!("bridge-pin-{}", new_nonce()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let parent = root.join("granted");
        std::fs::create_dir(&parent).unwrap();
        let dest = parent.join("fresh.vrs");
        let fs = Pinned::new(&dest, None, true).unwrap();
        std::fs::rename(&parent, root.join("held")).unwrap();
        std::fs::create_dir(&parent).unwrap();
        assert!(write_replace(&fs, &dest, b"new", &new_nonce()).is_err());
        assert!(!root.join("held/fresh.vrs").exists());
        assert!(!dest.exists());
        let fs = Pinned::new(&dest, None, true).unwrap();
        std::fs::write(&dest, b"another writer").unwrap();
        assert!(write_replace(&fs, &dest, b"new", &new_nonce()).is_err());
        assert_eq!(fs.error().code, "save_conflict");
        assert_eq!(Pinned::new(&dest, None, true).err().unwrap().bridge().code, "save_conflict");
        assert_eq!(std::fs::read(&dest).unwrap(), b"another writer");
        let fp = fingerprint(&RealFs, &dest).unwrap();
        let fs = Pinned::new(&dest, Some(&fp), false).unwrap();
        std::fs::write(&dest, b"external changes").unwrap();
        assert!(write_replace(&fs, &dest, b"new", &new_nonce()).is_err());
        assert_eq!(fs.error().code, "save_conflict");
        assert_eq!(Pinned::new(&dest, Some(&fp), false).err().unwrap().bridge().code, "save_conflict");
        assert_eq!(std::fs::read(&dest).unwrap(), b"external changes");
        std::os::unix::fs::symlink(root.join("held"), root.join("link")).unwrap();
        assert!(Pinned::new(&root.join("link/new.vrs"), None, true).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
