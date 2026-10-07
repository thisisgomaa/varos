//! Pinned-directory adapter for the existing durable writer. Bridge never follows a changed
//! parent or final symlink. Fresh destinations publish with linkat (no replacement).
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
        IoError,
        ScopeRefused,
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
                Self::ScopeRefused => varos_bridge::Error::new("scope_refused", "file grant revoked"),
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
        auth: Option<varos_bridge::ipc::Recheck>,
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
    impl Pinned {
        pub fn new(
            dest: &Path,
            expected: Option<&Fingerprint>,
            fresh: bool,
            auth: Option<&varos_bridge::ipc::Recheck>,
        ) -> Result<Self, PinnedError> {
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
                    return Err(PinnedError::IoError);
                }
            }
            let pinned = Self {
                failure: std::sync::Mutex::new(None),
                dir,
                parent: parent.to_owned(),
                dest: dest.to_owned(),
                expected: expected.cloned(),
                fresh,
                auth: auth.cloned(),
            };
            pinned.check_destination()?;
            Ok(pinned)
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
            self.failure.lock().unwrap().unwrap_or(PinnedError::IoError).bridge()
        }
        fn record(&self, error: PinnedError) -> io::Error {
            *self.failure.lock().unwrap() = Some(error);
            io::Error::other("pinned publication refused")
        }
        fn check_destination(&self) -> Result<(), PinnedError> {
            match self.open(&self.dest) {
                Err(e) if self.fresh && e.kind() == io::ErrorKind::NotFound => Ok(()),
                Ok(_) if self.fresh => Err(PinnedError::SaveConflict),
                Ok(f) => {
                    let m = f.metadata()?;
                    let got = Fingerprint { len: m.len(), modified: m.modified().ok() };
                    if !m.is_file() || Some(&got) != self.expected.as_ref() {
                        Err(PinnedError::SaveConflict)
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(if !self.fresh && e.kind() == io::ErrorKind::NotFound {
                    PinnedError::SaveConflict
                } else {
                    e.into()
                }),
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
            if let Some(auth) = &self.auth {
                let scopes = (auth.0)().map_err(|_| self.record(PinnedError::ScopeRefused))?;
                if !scopes.read || !scopes.files {
                    return Err(self.record(PinnedError::ScopeRefused));
                }
            }
            self.parent_unchanged()?;
            self.check_destination().map_err(|e| self.record(e))?;
            let from = self.leaf(from)?;
            let to = self.leaf(to)?;
            if self.fresh {
                // SAFETY: both names are restricted to the pinned directory; linkat refuses overwrite.
                check(unsafe {
                    libc::linkat(self.dir.as_raw_fd(), from.as_ptr(), self.dir.as_raw_fd(), to.as_ptr(), 0)
                })
                .map_err(|e| {
                    self.record(if e.kind() == io::ErrorKind::AlreadyExists {
                        PinnedError::SaveConflict
                    } else {
                        PinnedError::IoError
                    })
                })?;
                // SAFETY: deletes only our temporary name in the pinned directory.
                check(unsafe { libc::unlinkat(self.dir.as_raw_fd(), from.as_ptr(), 0) })
            } else {
                // SAFETY: atomic replacement stays in the pinned directory.
                check(unsafe { libc::renameat(self.dir.as_raw_fd(), from.as_ptr(), self.dir.as_raw_fd(), to.as_ptr()) })
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
        let fs = Pinned::new(&dest, None, true, None).unwrap();
        std::fs::rename(&parent, root.join("held")).unwrap();
        std::fs::create_dir(&parent).unwrap();
        assert!(write_replace(&fs, &dest, b"new", &new_nonce()).is_err());
        assert!(!root.join("held/fresh.vrs").exists());
        assert!(!dest.exists());
        let fs = Pinned::new(&dest, None, true, None).unwrap();
        std::fs::write(&dest, b"another writer").unwrap();
        assert!(write_replace(&fs, &dest, b"new", &new_nonce()).is_err());
        assert_eq!(fs.error().code, "save_conflict");
        assert_eq!(Pinned::new(&dest, None, true, None).err().unwrap().bridge().code, "save_conflict");
        assert_eq!(std::fs::read(&dest).unwrap(), b"another writer");
        let fp = fingerprint(&RealFs, &dest).unwrap();
        let fs = Pinned::new(&dest, Some(&fp), false, None).unwrap();
        std::fs::write(&dest, b"external changes").unwrap();
        assert!(write_replace(&fs, &dest, b"new", &new_nonce()).is_err());
        assert_eq!(fs.error().code, "save_conflict");
        assert_eq!(Pinned::new(&dest, Some(&fp), false, None).err().unwrap().bridge().code, "save_conflict");
        assert_eq!(std::fs::read(&dest).unwrap(), b"external changes");
        std::os::unix::fs::symlink(root.join("held"), root.join("link")).unwrap();
        assert!(Pinned::new(&root.join("link/new.vrs"), None, true, None).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn file_scope_revoked_after_encoding_refuses_publication() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let root = std::env::temp_dir().join(format!("bridge-revoke-{}", new_nonce()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let dest = root.join("new.vrs");
        let allow = Arc::new(AtomicBool::new(true));
        let grant = allow.clone();
        let auth = varos_bridge::ipc::Recheck(Arc::new(move || {
            Ok(varos_bridge::conn::trust::Scopes {
                read: true,
                files: grant.load(Ordering::Acquire),
                ..Default::default()
            })
        }));
        let fs = Pinned::new(&dest, None, true, Some(&auth)).unwrap();
        allow.store(false, Ordering::Release);
        assert!(write_replace(&fs, &dest, b"new", &new_nonce()).is_err());
        assert!(!dest.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
