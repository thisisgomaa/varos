//! Trust store (ADR-0011 §3): owner-approved agent profiles with scopes, a revocation list and a
//! monotonic trust generation, in an owner-only `trust.json` under the state directory.
//! Public data only — private keys stay in the CredentialStore. Read failures deny.
//! Pending pairing requests are bounded, expiring, coalesced files in the runtime directory.
use super::{credentials, fsutil, handshake::VerifiedAgent, is_hex, now_secs, random_hex, Paths};
use crate::Error;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TRUST_FILE: &str = "trust.json";
pub const LOCK_FILE: &str = "trust.lock";
pub const MAX_APPROVED: usize = 64;
pub const MAX_REVOKED: usize = 256;
pub const MAX_PENDING: usize = 8;
pub const PAIRING_TTL_SECS: u64 = 600;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scopes {
    pub read: bool,
    pub edit: bool,
    pub destructive: bool,
    pub history: bool,
    #[serde(default)]
    pub files: bool,
}
impl Scopes {
    pub const DEFAULT_REQUEST: Self = Self { read: true, edit: true, destructive: false, history: false, files: false };
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut s = Self::default();
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part {
                "read" => s.read = true,
                "edit" => s.edit = true,
                "destructive" => s.destructive = true,
                "history" => s.history = true,
                "files" => s.files = true,
                other => {
                    return Err(Error::new(
                        "invalid_argument",
                        format!("unknown scope {other:?}; use read,edit,destructive,history,files"),
                    ))
                }
            }
        }
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if !self.read || ((self.destructive || self.history) && !self.edit) {
            return Err(Error::new("invalid_argument", "scopes need read; destructive and history also need edit"));
        }
        Ok(())
    }
    pub fn intersect(self, other: Self) -> Self {
        Self {
            read: self.read && other.read,
            edit: self.edit && other.edit,
            destructive: self.destructive && other.destructive,
            history: self.history && other.history,
            files: self.files && other.files,
        }
    }
    pub fn names(&self) -> String {
        [
            ("read", self.read),
            ("edit", self.edit),
            ("destructive", self.destructive),
            ("history", self.history),
            ("files", self.files),
        ]
        .iter()
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(",")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approved {
    pub profile_id: String,
    pub public_key: String,
    pub fingerprint: String,
    /// Unverified client label recorded at approval (display only).
    pub label: String,
    pub scopes: Scopes,
    /// Host key pinned through the owner ceremony.
    pub host_fingerprint: String,
    pub approved_at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revoked {
    pub profile_id: String,
    pub fingerprint: String,
    pub revoked_at: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustFile {
    pub version: u32,
    pub generation: u64,
    pub approved: Vec<Approved>,
    pub revoked: Vec<Revoked>,
}
fn trust_error(e: impl std::fmt::Display) -> Error {
    Error::new("trust_unavailable", format!("trust store unreadable; refusing: {e}"))
}
impl TrustFile {
    pub fn load(paths: &Paths) -> Result<Self, Error> {
        let path = paths.state.join(TRUST_FILE);
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self { version: 1, ..Self::default() });
            }
            _ => {}
        }
        fsutil::check_private_dir(&paths.state).map_err(trust_error)?;
        let file: Self =
            serde_json::from_slice(&fsutil::read_private(&path).map_err(trust_error)?).map_err(trust_error)?;
        if file.version != 1 || file.approved.len() > MAX_APPROVED || file.revoked.len() > MAX_REVOKED {
            return Err(trust_error("unsupported version or oversized store"));
        }
        Ok(file)
    }
    fn save(&self, paths: &Paths) -> Result<(), Error> {
        fsutil::write_atomic(&paths.state.join(TRUST_FILE), &serde_json::to_vec_pretty(self).expect("trust serializes"))
            .map_err(trust_error)
    }
    pub fn is_revoked(&self, profile_id: &str, fingerprint: &str) -> bool {
        self.revoked.iter().any(|r| r.profile_id == profile_id || r.fingerprint == fingerprint)
    }
    pub fn approved(&self, profile_id: &str) -> Option<&Approved> {
        self.approved.iter().find(|a| a.profile_id == profile_id)
    }
    /// Host-side decision for an authenticated agent key.
    pub fn decide(&self, agent: &VerifiedAgent, host_fingerprint: &str) -> Decision {
        if self.is_revoked(&agent.profile_id, &agent.fingerprint) {
            return Decision::Denied(Error::new("pairing_denied", "this agent profile was revoked by the owner"));
        }
        match self.approved(&agent.profile_id) {
            Some(a) if a.fingerprint != agent.fingerprint => Decision::Denied(Error::new(
                "identity_mismatch",
                "profile is paired with a different key; a copied profile reference is not an identity",
            )),
            Some(a) if a.host_fingerprint != host_fingerprint => Decision::NeedsPairing,
            Some(a) => Decision::Allowed { scopes: a.scopes, generation: self.generation },
            None => Decision::NeedsPairing,
        }
    }
}
#[derive(Debug)]
pub enum Decision {
    Allowed { scopes: Scopes, generation: u64 },
    NeedsPairing,
    Denied(Error),
}

/// Serialize a read-modify-write of the trust store against other hosts/CLIs.
fn transaction<T>(paths: &Paths, f: impl FnOnce(&mut TrustFile) -> Result<T, Error>) -> Result<T, Error> {
    fsutil::ensure_private_dir(&paths.state).map_err(trust_error)?;
    let _lock = fsutil::lock(&paths.state.join(LOCK_FILE)).map_err(trust_error)?;
    let mut file = TrustFile::load(paths)?;
    file.version = 1;
    let out = f(&mut file)?;
    file.generation += 1;
    file.save(paths)?;
    Ok(out)
}
/// Owner approval: bind the key, profile, host pin and selected scopes.
pub fn approve(paths: &Paths, request: &PairingRequest, scopes: Scopes) -> Result<Approved, Error> {
    scopes.validate()?;
    transaction(paths, |file| {
        if file.is_revoked(&request.profile_id, &request.fingerprint) {
            return Err(Error::new(
                "pairing_denied",
                "this profile/key was revoked; the agent must create a new profile",
            ));
        }
        file.approved.retain(|a| a.profile_id != request.profile_id);
        if file.approved.len() >= MAX_APPROVED {
            return Err(Error::new("limit_exceeded", "too many approved agents; revoke one first"));
        }
        let approved = Approved {
            profile_id: request.profile_id.clone(),
            public_key: request.public_key.clone(),
            fingerprint: request.fingerprint.clone(),
            label: request.label.clone(),
            scopes,
            host_fingerprint: request.host_fingerprint.clone(),
            approved_at: now_secs(),
        };
        file.approved.push(approved.clone());
        Ok(approved)
    })
}
/// Revoke a profile (and its key) permanently; the agent must pair a new profile.
pub fn revoke(paths: &Paths, profile_id: &str) -> Result<Revoked, Error> {
    transaction(paths, |file| {
        let fingerprint = file
            .approved(profile_id)
            .map(|a| a.fingerprint.clone())
            .ok_or_else(|| Error::new("not_found", format!("no approved agent profile {profile_id}")))?;
        file.approved.retain(|a| a.profile_id != profile_id);
        if file.revoked.len() >= MAX_REVOKED {
            file.revoked.remove(0);
        }
        let revoked = Revoked { profile_id: profile_id.into(), fingerprint, revoked_at: now_secs() };
        file.revoked.push(revoked.clone());
        Ok(revoked)
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingRequest {
    pub request_id: String,
    pub profile_id: String,
    pub public_key: String,
    pub fingerprint: String,
    /// Claimed, unverified client label.
    pub label: String,
    pub requested: Scopes,
    pub host_fingerprint: String,
    pub instance_id: String,
    pub created: u64,
    pub expires: u64,
    /// Short code derived from both keys so the owner can match agent and terminal.
    pub match_code: String,
}
impl PairingRequest {
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"request_id":self.request_id,"match_code":self.match_code,"agent_fingerprint":credentials::short(&self.fingerprint),"expires_in_secs":self.expires.saturating_sub(now_secs())})
    }
}
pub fn match_code(agent_fingerprint: &str, host_fingerprint: &str, request_id: &str) -> String {
    let h = super::hex(&Sha256::digest(format!("{agent_fingerprint}:{host_fingerprint}:{request_id}").as_bytes()));
    format!("{}-{}", h[..3].to_uppercase(), h[3..6].to_uppercase())
}
fn valid_request_id(id: &str) -> bool {
    is_hex(id, 8)
}
fn pairing_io(e: impl std::fmt::Display) -> Error {
    Error::new("trust_unavailable", format!("pairing store: {e}"))
}
/// Non-expired pending requests (expired files are removed). Bounded scan.
pub fn pending(paths: &Paths) -> Result<Vec<PairingRequest>, Error> {
    let dir = paths.pairing();
    if std::fs::symlink_metadata(&dir).is_err() {
        return Ok(vec![]);
    }
    fsutil::check_private_dir(&dir).map_err(pairing_io)?;
    let now = now_secs();
    let mut out = vec![];
    for entry in std::fs::read_dir(&dir).map_err(pairing_io)?.filter_map(Result::ok).take(64) {
        let path = entry.path();
        let Some(id) = path.file_stem().and_then(|s| s.to_str()).filter(|s| valid_request_id(s)) else { continue };
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let parsed = fsutil::read_private(&path).ok().and_then(|b| serde_json::from_slice::<PairingRequest>(&b).ok());
        match parsed {
            Some(r) if r.request_id == id && r.expires > now => out.push(r),
            _ => {
                let _ = fsutil::remove_owned(&path);
            }
        }
    }
    out.sort_by_key(|r| r.created);
    Ok(out)
}
/// Host side: record (or coalesce) a pending pairing for an unknown authenticated key.
pub fn request_pairing(
    paths: &Paths,
    agent: &VerifiedAgent,
    host_fingerprint: &str,
    instance_id: &str,
) -> Result<PairingRequest, Error> {
    fsutil::ensure_private_dir(&paths.runtime).map_err(pairing_io)?;
    fsutil::ensure_private_dir(&paths.pairing()).map_err(pairing_io)?;
    let existing = pending(paths)?;
    if let Some(r) =
        existing.iter().find(|r| r.fingerprint == agent.fingerprint && r.host_fingerprint == host_fingerprint)
    {
        return Ok(r.clone());
    }
    if existing.len() >= MAX_PENDING {
        return Err(Error::new("busy", "too many pending pairing requests; the owner must approve or deny some first"));
    }
    let request_id = random_hex(4)?;
    let now = now_secs();
    let request = PairingRequest {
        match_code: match_code(&agent.fingerprint, host_fingerprint, &request_id),
        request_id,
        profile_id: agent.profile_id.clone(),
        public_key: credentials::public_hex(&agent.key),
        fingerprint: agent.fingerprint.clone(),
        label: agent.label.clone(),
        requested: Scopes::DEFAULT_REQUEST,
        host_fingerprint: host_fingerprint.into(),
        instance_id: instance_id.into(),
        created: now,
        expires: now + PAIRING_TTL_SECS,
    };
    fsutil::write_atomic(
        &paths.pairing().join(format!("{}.json", request.request_id)),
        &serde_json::to_vec(&request).expect("pairing serializes"),
    )
    .map_err(pairing_io)?;
    Ok(request)
}
/// Owner side: find a pending request by id (expired requests are gone).
pub fn find_pending(paths: &Paths, request_id: &str) -> Result<PairingRequest, Error> {
    if !valid_request_id(request_id) {
        return Err(Error::new("invalid_argument", "pairing request ids are 8 lowercase hex digits"));
    }
    pending(paths)?.into_iter().find(|r| r.request_id == request_id).ok_or_else(|| {
        Error::new("not_found", format!("no pending pairing request {request_id} (it may have expired)"))
    })
}
pub fn remove_pending(paths: &Paths, request_id: &str) {
    if valid_request_id(request_id) {
        let _ = fsutil::remove_owned(&paths.pairing().join(format!("{request_id}.json")));
    }
}

#[cfg(test)]
mod slice4_scope_tests {
    use super::*;
    #[test]
    fn files_scope_defaults_off_and_intersects() {
        let old: Scopes =
            serde_json::from_str(r#"{"read":true,"edit":true,"destructive":false,"history":false}"#).unwrap();
        assert!(!old.files);
        const { assert!(!Scopes::DEFAULT_REQUEST.files) };
        let grant = Scopes::parse("read,edit,files").unwrap();
        assert!(grant.files);
        assert_eq!(grant.names(), "read,edit,files");
        assert!(!grant.intersect(old).files);
    }
}
