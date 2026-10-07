//! `CredentialStore` (ADR-0011 §3, §8): private keys live in the platform store, referenced by an
//! opaque account name. A locked/unavailable store is `credential_unavailable` — never a
//! plaintext-file fallback. Only public keys and fingerprints ever leave this module.
use super::hex;
use crate::Error;
use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex};

pub const KEYCHAIN_SERVICE: &str = "com.varos.bridge";
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
        Some(stored) if stored == secret => Ok(SigningKey::from_bytes(&secret)),
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
    /// Simulate a locked Keychain.
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

/// macOS login Keychain (generic password items). Each item's ACL trusts the creating binary;
/// a rebuilt/re-signed binary may get a macOS "allow access" prompt — the C1 signing gate.
#[cfg(target_os = "macos")]
pub struct KeychainStore;
/// `errSecItemNotFound` (Security/SecBase.h).
#[cfg(target_os = "macos")]
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
#[cfg(target_os = "macos")]
impl CredentialStore for KeychainStore {
    fn load(&self, account: &str) -> Result<Option<[u8; 32]>, Error> {
        use security_framework::passwords::get_generic_password;
        match get_generic_password(KEYCHAIN_SERVICE, account) {
            Ok(bytes) => bytes
                .try_into()
                .map(Some)
                .map_err(|_| unavailable(format!("Keychain item {account} is not a 32-byte key"))),
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(e) => Err(unavailable(format!("Keychain read failed ({}): {e}", e.code()))),
        }
    }
    fn store(&self, account: &str, secret: &[u8; 32]) -> Result<(), Error> {
        security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, account, secret)
            .map_err(|e| unavailable(format!("Keychain write failed ({}): {e}", e.code())))
    }
    fn delete(&self, account: &str) -> Result<(), Error> {
        match security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, account) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(e) => Err(unavailable(format!("Keychain delete failed ({}): {e}", e.code()))),
        }
    }
}

/// Platforms without an implemented store (Windows Credential Manager/DPAPI is planned, not built).
pub struct UnavailableStore;
impl CredentialStore for UnavailableStore {
    fn load(&self, _: &str) -> Result<Option<[u8; 32]>, Error> {
        Err(unavailable("no credential store is implemented on this platform yet"))
    }
    fn store(&self, _: &str, _: &[u8; 32]) -> Result<(), Error> {
        Err(unavailable("no credential store is implemented on this platform yet"))
    }
    fn delete(&self, _: &str) -> Result<(), Error> {
        Err(unavailable("no credential store is implemented on this platform yet"))
    }
}

/// The platform default: Keychain on macOS, unavailable elsewhere.
pub fn platform_store() -> Box<dyn CredentialStore> {
    #[cfg(target_os = "macos")]
    {
        Box::new(KeychainStore)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(UnavailableStore)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_once_then_reuse_and_locked_store_is_unavailable_not_plaintext() {
        let store = MemoryStore::new();
        let a = load_or_create(&store, "agent:x").unwrap();
        let b = load_or_create(&store, "agent:x").unwrap();
        assert_eq!(a.verifying_key(), b.verifying_key());
        assert_eq!(fingerprint(&a.verifying_key()).len(), 64);
        store.set_locked(true);
        let e = load_or_create(&store, "agent:y").unwrap_err();
        assert_eq!(e.code, "credential_unavailable");
        assert_eq!(UnavailableStore.load("host").unwrap_err().code, "credential_unavailable");
    }
    /// Touches the real login Keychain with a throwaway item; run by hand on macOS only.
    #[cfg(target_os = "macos")]
    #[ignore = "writes and deletes a throwaway item in the user's login Keychain"]
    #[test]
    fn keychain_round_trip() {
        let account = format!("test:{}", super::super::random_hex(8).unwrap());
        let key = load_or_create(&KeychainStore, &account).unwrap();
        assert_eq!(load_existing(&KeychainStore, &account).unwrap().unwrap().verifying_key(), key.verifying_key());
        KeychainStore.delete(&account).unwrap();
        assert!(KeychainStore.load(&account).unwrap().is_none());
    }
}
