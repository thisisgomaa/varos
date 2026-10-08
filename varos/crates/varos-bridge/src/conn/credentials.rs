//! Per-user raw identity seeds in private files (ADR-0011 Amendment 2).
//! No Keychain access, prompts or alternate credential backend.
use super::hex;
use crate::Error;
use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex};

pub const HOST_ACCOUNT: &str = "host";
pub fn agent_account(profile_id: &str) -> String {
    format!("agent:{profile_id}")
}

pub trait CredentialStore: Send + Sync {
    /// `Ok(None)`: no such credential. `Err`: the store is locked/unavailable.
    fn load(&self, account: &str) -> Result<Option<[u8; 32]>, Error>;
    fn store(&self, account: &str, secret: &[u8; 32]) -> Result<(), Error>;
    fn delete(&self, account: &str) -> Result<(), Error>;
}
pub fn unavailable(reason: impl Into<String>) -> Error {
    Error::new("credential_unavailable", reason)
}

/// Load a signing key, creating (and storing) a fresh one from OS randomness if absent.
pub fn load_or_create(store: &dyn CredentialStore, account: &str) -> Result<SigningKey, Error> {
    if let Some(secret) = store.load(account)? {
        return Ok(SigningKey::from_bytes(&secret));
    }
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| unavailable(format!("OS randomness unavailable: {e}")))?;
    store.store(account, &secret)?;
    // Read back so a store that silently drops writes cannot hand out an unpersisted identity.
    match store.load(account)? {
        Some(stored) => Ok(SigningKey::from_bytes(&stored)),
        _ => Err(unavailable("credential store did not persist the new key")),
    }
}
pub fn load_existing(store: &dyn CredentialStore, account: &str) -> Result<Option<SigningKey>, Error> {
    Ok(store.load(account)?.map(|s| SigningKey::from_bytes(&s)))
}

/// SHA-256 of the raw 32-byte public key, lowercase hex (64 chars).
pub fn fingerprint(key: &VerifyingKey) -> String {
    hex(&Sha256::digest(key.as_bytes()))
}
/// Short human comparison form: `ab12 cd34 ef56 7890`.
pub fn short(fingerprint: &str) -> String {
    fingerprint
        .as_bytes()
        .chunks(4)
        .take(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn public_hex(key: &VerifyingKey) -> String {
    hex(key.as_bytes())
}
pub fn parse_public(text: &str) -> Result<VerifyingKey, Error> {
    let bytes: [u8; 32] = super::unhex(text)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| Error::new("identity_invalid", "public key must be 64 hex digits"))?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| Error::new("identity_invalid", "not a valid Ed25519 public key"))
}

/// In-memory store for tests and fakes. Never used by the shipped binaries' default path.
#[derive(Default)]
pub struct MemoryStore {
    items: Mutex<HashMap<String, [u8; 32]>>,
    locked: std::sync::atomic::AtomicBool,
}
impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
    /// Simulate an unavailable credential store.
    pub fn set_locked(&self, locked: bool) {
        self.locked.store(locked, std::sync::atomic::Ordering::Release);
    }
    fn check(&self) -> Result<(), Error> {
        if self.locked.load(std::sync::atomic::Ordering::Acquire) {
            Err(unavailable("credential store is locked"))
        } else {
            Ok(())
        }
    }
}
impl CredentialStore for MemoryStore {
    fn load(&self, account: &str) -> Result<Option<[u8; 32]>, Error> {
        self.check()?;
        Ok(self.items.lock().unwrap().get(account).copied())
    }
    fn store(&self, account: &str, secret: &[u8; 32]) -> Result<(), Error> {
        self.check()?;
        self.items.lock().unwrap().insert(account.into(), *secret);
        Ok(())
    }
    fn delete(&self, account: &str) -> Result<(), Error> {
        self.check()?;
        self.items.lock().unwrap().remove(account);
        Ok(())
    }
}

/// Per-user file store. Writes create identities once; an existing identity always wins.
/// Unix files are 0600 inside a 0700 directory. Same-uid processes can read these keys.
pub struct FileKeyStore {
    directory: Result<std::path::PathBuf, String>,
}
impl FileKeyStore {
    pub fn new(directory: impl Into<std::path::PathBuf>) -> Self {
        Self { directory: Ok(directory.into()) }
    }
    fn directory(&self) -> Result<&std::path::Path, Error> {
        self.directory.as_deref().map_err(|e| unavailable(e.clone()))
    }
    fn path(&self, account: &str) -> Result<std::path::PathBuf, Error> {
        let name = if account == HOST_ACCOUNT {
            "host".to_string()
        } else if let Some(id) = account.strip_prefix("agent:").filter(|id| super::handshake::valid_profile_id(id)) {
            format!("agent-{id}")
        } else {
            return Err(unavailable("invalid key account; use host or agent:<16-hex profile id>"));
        };
        Ok(self.directory()?.join(format!("{name}.key")))
    }
    fn check_directory(&self, create: bool) -> Result<bool, Error> {
        let dir = self.directory()?;
        // Check state before recursive creation can follow a symlinked ancestor.
        let state = dir.parent().ok_or_else(|| unavailable("key directory needs a state parent"))?;
        match std::fs::symlink_metadata(state) {
            Ok(m) if !m.is_dir() || m.file_type().is_symlink() => {
                return Err(unavailable("state parent must be a real private directory"))
            }
            Ok(_) => {
                #[cfg(unix)]
                super::fsutil::check_private_dir(state).map_err(|e| self.io_error(e))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(self.io_error(e)),
        }
        let result = if create {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                // Missing Bridge state must also be private: profiles.json shares its parent.
                std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(dir)
            }
        } else {
            Ok(())
        };
        result.map_err(|e| self.io_error(e))?;
        match std::fs::symlink_metadata(dir) {
            Err(e) if !create && e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(self.io_error(e)),
            Ok(m) if !m.is_dir() || m.file_type().is_symlink() => {
                return Err(unavailable(
                    "key parent must be a real directory; replace the symlink with a private directory",
                ));
            }
            Ok(_) => {}
        }
        #[cfg(unix)]
        super::fsutil::check_private_dir(dir).map_err(|e| self.io_error(e))?;
        Ok(true)
    }
    fn io_error(&self, e: impl std::fmt::Display) -> Error {
        unavailable(format!(
            "key store: {e}; use a real directory owned by this user (chmod 700) and regular key files (chmod 600)"
        ))
    }
}
impl CredentialStore for FileKeyStore {
    fn load(&self, account: &str) -> Result<Option<[u8; 32]>, Error> {
        use std::io::Read;
        let path = self.path(account)?;
        if !self.check_directory(false)? {
            return Ok(None);
        }
        #[cfg(unix)]
        let opened = super::fsutil::open_private(&path);
        #[cfg(not(unix))]
        let opened = (|| {
            let m = std::fs::symlink_metadata(&path)?;
            if !m.is_file() || m.file_type().is_symlink() {
                return Err(std::io::Error::other("key must be a regular file, not a symlink"));
            }
            std::fs::File::open(&path)
        })();
        let file = match opened {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(self.io_error(e)),
        };
        let mut bytes = Vec::new();
        file.take(33).read_to_end(&mut bytes).map_err(|e| self.io_error(e))?;
        bytes.try_into().map(Some).map_err(|_| {
            unavailable(
                "key must contain exactly 32 bytes; restore the key from backup or remove it to create a new identity",
            )
        })
    }
    fn store(&self, account: &str, secret: &[u8; 32]) -> Result<(), Error> {
        use std::io::Write;
        let path = self.path(account)?;
        self.check_directory(true)?;
        // Serializes the existence check and rename on Unix. On Windows, MoveFileW
        // refuses an existing destination, arbitrating concurrent creators.
        #[cfg(unix)]
        let _lock = super::fsutil::lock(&self.directory()?.join(".create.lock")).map_err(|e| self.io_error(e))?;
        if self.load(account)?.is_some() {
            return Ok(());
        }
        let tmp = self.directory()?.join(format!(".{}.tmp", super::random_hex(16)?));
        let result = (|| -> std::io::Result<()> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true); // O_EXCL: never reuse a temporary name.
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
            }
            let mut file = options.open(&tmp)?;
            file.write_all(secret)?;
            file.sync_all()?;
            #[cfg(not(windows))]
            {
                std::fs::rename(&tmp, &path)?;
                #[cfg(unix)]
                std::fs::File::open(self.directory().map_err(|e| std::io::Error::other(e.reason))?)?.sync_all()?;
                Ok(())
            }
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                #[link(name = "kernel32")]
                unsafe extern "system" {
                    fn MoveFileW(existing: *const u16, new: *const u16) -> i32;
                }
                let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
                let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                // SAFETY: both pointers refer to live NUL-terminated UTF-16 paths.
                if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } == 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            }
        })();
        let _ = std::fs::remove_file(&tmp);
        if let Err(e) = result {
            if self.load(account)?.is_some() {
                return Ok(());
            }
            return Err(self.io_error(e));
        }
        Ok(())
    }
    fn delete(&self, account: &str) -> Result<(), Error> {
        let path = self.path(account)?;
        if !self.check_directory(false)? {
            return Ok(());
        }
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(self.io_error(e)),
        }
    }
}

/// File keys on every platform; resolution errors remain typed and never prompt.
pub fn platform_store() -> Box<dyn CredentialStore> {
    match super::Paths::resolve() {
        Ok(paths) => Box::new(FileKeyStore::new(paths.state.join("keys"))),
        Err(e) => Box::new(FileKeyStore { directory: Err(e.reason) }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("varos-keys-{}", super::super::random_hex(16).unwrap())))
        }
        fn store(&self) -> FileKeyStore {
            FileKeyStore::new(self.0.join("keys"))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn file_round_trip_and_accounts_isolated() {
        let t = Temp::new();
        let store = t.store();
        let agent = agent_account("0123456789abcdef");
        let host = load_or_create(&store, HOST_ACCOUNT).unwrap();
        let key = load_or_create(&store, &agent).unwrap();
        assert_ne!(host.verifying_key(), key.verifying_key());
        assert_eq!(load_existing(&store, HOST_ACCOUNT).unwrap().unwrap().verifying_key(), host.verifying_key());
        assert_eq!(std::fs::read(t.0.join("keys/host.key")).unwrap().len(), 32);
        store.delete(HOST_ACCOUNT).unwrap();
        store.delete(HOST_ACCOUNT).unwrap();
        assert!(store.load(HOST_ACCOUNT).unwrap().is_none());
        assert!(store.load(&agent).unwrap().is_some());
        assert_eq!(store.load("agent:../../escape").unwrap_err().code, "credential_unavailable");
    }
    #[test]
    fn atomic_creators_share_one_identity() {
        let t = Temp::new();
        let store = std::sync::Arc::new(t.store());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|n| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    assert!(store.load(HOST_ACCOUNT).unwrap().is_none());
                    barrier.wait();
                    store.store(HOST_ACCOUNT, &[n; 32]).unwrap();
                    store.load(HOST_ACCOUNT).unwrap().unwrap()
                })
            })
            .collect();
        let keys: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(keys[0], keys[1]);
        assert!([[0; 32], [1; 32]].contains(&keys[0]));
    }
    #[cfg(unix)]
    #[test]
    fn refuses_public_mode_and_symlink_parent() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let t = Temp::new();
        let store = t.store();
        load_or_create(&store, HOST_ACCOUNT).unwrap();
        assert_eq!(std::fs::metadata(t.0.join("keys")).unwrap().permissions().mode() & 0o777, 0o700);
        let path = t.0.join("keys/host.key");
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let e = store.load(HOST_ACCOUNT).unwrap_err();
        assert_eq!(e.code, "credential_unavailable");
        assert!(e.reason.contains("chmod 600"));
        symlink(t.0.join("keys"), t.0.join("linked")).unwrap();
        let linked = FileKeyStore::new(t.0.join("linked"));
        let e = linked.load(HOST_ACCOUNT).unwrap_err();
        assert_eq!(e.code, "credential_unavailable");
        assert!(e.reason.contains("replace the symlink"));
        assert!(linked.store(HOST_ACCOUNT, &[0; 32]).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_state_before_creating_keys() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let t = Temp::new();
        std::fs::create_dir(&t.0).unwrap();
        std::fs::set_permissions(&t.0, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir(t.0.join("real")).unwrap();
        symlink(t.0.join("real"), t.0.join("state")).unwrap();
        let store = FileKeyStore::new(t.0.join("state/keys"));
        assert_eq!(store.load(HOST_ACCOUNT).unwrap_err().code, "credential_unavailable");
        assert_eq!(store.store(HOST_ACCOUNT, &[0; 32]).unwrap_err().code, "credential_unavailable");
        assert!(!t.0.join("real/keys").exists());
    }
    #[cfg(all(unix, debug_assertions))]
    #[test]
    fn platform_store_relocates_keys_with_override() {
        const CHILD: &str = "VAROS_KEY_OVERRIDE_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let paths = super::super::Paths::resolve().unwrap();
            let store = platform_store();
            store.store(HOST_ACCOUNT, &[42; 32]).unwrap();
            assert_eq!(std::fs::read(paths.state.join("keys/host.key")).unwrap(), [42; 32]);
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        let t = Temp::new();
        std::fs::create_dir(&t.0).unwrap();
        std::fs::set_permissions(&t.0, std::fs::Permissions::from_mode(0o700)).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "conn::credentials::tests::platform_store_relocates_keys_with_override"])
            .env(CHILD, "1")
            .env(super::super::HOME_OVERRIDE, &t.0)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
