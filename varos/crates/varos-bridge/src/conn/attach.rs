//! `--attach auto` (ADR-0011 §2): the agent-side proxy discovers, authenticates and attaches
//! on every tool call. MCP itself never waits on this — it initializes with static tools, and
//! tool calls return typed `host_not_running` / `pairing_required` / `ambiguous_target` /
//! `session_reset` errors instead of hanging.
use super::{
    credentials::{self, CredentialStore},
    fsutil, handshake,
    registry::{self, Record, Scan},
    trust::TrustFile,
    Paths,
};
use crate::{Error, Reply, Request};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub const MAX_CANDIDATES: usize = 8;
pub const PROFILES_FILE: &str = "profiles.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    Auto,
    Instance(String),
    /// Only a selector; verified against the record's start identity/epoch by the scan.
    Pid(u32),
}
/// Built once per tool call; boxing the record would only add noise.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Attach(Record),
    NotRunning,
    Ambiguous(Vec<Record>),
}
/// Deterministic choice. `paired` = host fingerprints this profile is paired with.
/// Never prefers newest, desktop over headless, or the last record.
pub fn select(live: &[Record], selector: &Selector, paired: &[String]) -> Selection {
    let matching: Vec<&Record> = live
        .iter()
        .filter(|r| match selector {
            Selector::Auto => true,
            Selector::Instance(id) => &r.instance_id == id,
            Selector::Pid(pid) => r.pid == *pid,
        })
        .collect();
    if matching.is_empty() {
        return Selection::NotRunning;
    }
    let eligible: Vec<&Record> = matching.iter().copied().filter(|r| paired.contains(&r.host_fingerprint)).collect();
    let pick = |v: Vec<&Record>| -> Selection {
        if v.len() == 1 {
            Selection::Attach(v[0].clone())
        } else {
            Selection::Ambiguous(v.into_iter().take(MAX_CANDIDATES).cloned().collect())
        }
    };
    if eligible.is_empty() {
        // A sole local candidate may enter pairing (no document access); several need the owner.
        pick(matching)
    } else {
        pick(eligible)
    }
}
pub fn host_not_running(selector: &Selector) -> Error {
    let which = match selector {
        Selector::Auto => String::new(),
        Selector::Instance(id) => format!(" matching --instance {id}"),
        Selector::Pid(pid) => format!(" matching --pid {pid}"),
    };
    Error::new(
        "host_not_running",
        format!("No running Varos{which} was found. Ask the owner to open Varos, then retry; nothing was changed. (No headless host or network fallback is started.)"),
    )
}
pub fn ambiguous(candidates: &[Record]) -> Error {
    let mut e = Error::new(
        "ambiguous_target",
        "Several Varos launches are running and none is chosen. Ask the owner which one to use: close the extra Varos, or pass --pid <pid> from the candidates below. Nothing was changed.",
    );
    e.candidates = candidates.iter().take(MAX_CANDIDATES).map(Record::summary).collect();
    e
}

/// Agent-side public profile references (no secrets): which Keychain profile a client label uses.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalProfile {
    pub profile_id: String,
    pub label: String,
    pub public_key: String,
    pub created: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalProfiles {
    pub version: u32,
    pub profiles: Vec<LocalProfile>,
}
impl LocalProfiles {
    pub fn load(paths: &Paths) -> Result<Self, Error> {
        let path = paths.state.join(PROFILES_FILE);
        if std::fs::symlink_metadata(&path).is_err() {
            return Ok(Self { version: 1, profiles: vec![] });
        }
        let bytes = fsutil::read_private(&path).map_err(|e| Error::new("trust_unavailable", e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| Error::new("trust_unavailable", e.to_string()))
    }
}
/// Resolve (or create) this client's profile key. `identity` is an explicit public reference.
pub fn resolve_profile(
    paths: &Paths,
    store: &dyn CredentialStore,
    trust: &TrustFile,
    label: &str,
    identity: Option<&str>,
) -> Result<(String, ed25519_dalek::SigningKey), Error> {
    if let Some(id) = identity {
        if !handshake::valid_profile_id(id) {
            return Err(Error::new("invalid_argument", "--identity must be a 16-hex profile id"));
        }
        let key = credentials::load_existing(store, &credentials::agent_account(id))?
            .ok_or_else(|| Error::new("identity_invalid", "no key for this --identity in the credential store"))?;
        return Ok((id.into(), key));
    }
    let label = super::clean_label(label);
    // Fast path: an owner-approved profile for this client needs no lock.
    if let Some((id, key, true)) = pick_profile(&LocalProfiles::load(paths)?, store, trust, &label)? {
        return Ok((id, key));
    }
    // Look again and create under the trust-store lock, so two MCP processes of the same
    // client starting together end up with one profile, not two.
    let io = |e: std::io::Error| Error::new("trust_unavailable", e.to_string());
    fsutil::ensure_private_dir(&paths.state).map_err(io)?;
    let _lock = fsutil::lock(&paths.state.join(super::trust::LOCK_FILE)).map_err(io)?;
    let mut profiles = LocalProfiles::load(paths)?;
    if let Some((id, key, _)) = pick_profile(&profiles, store, trust, &label)? {
        return Ok((id, key));
    }
    // New profile: key in the credential store first, then the public reference.
    let profile_id = super::random_hex(8)?;
    let key = credentials::load_or_create(store, &credentials::agent_account(&profile_id))?;
    profiles.version = 1;
    if profiles.profiles.len() >= 64 {
        profiles.profiles.remove(0);
    }
    profiles.profiles.push(LocalProfile {
        profile_id: profile_id.clone(),
        label,
        public_key: credentials::public_hex(&key.verifying_key()),
        created: super::now_secs(),
    });
    fsutil::write_atomic(&paths.state.join(PROFILES_FILE), &serde_json::to_vec_pretty(&profiles).expect("profiles"))
        .map_err(io)?;
    Ok((profile_id, key))
}
/// This label's best profile: an owner-approved one (true) before the newest unrevoked one (false).
/// Never chosen for broader grants; approval only breaks ties within the same client label.
fn pick_profile(
    profiles: &LocalProfiles,
    store: &dyn CredentialStore,
    trust: &TrustFile,
    label: &str,
) -> Result<Option<(String, ed25519_dalek::SigningKey, bool)>, Error> {
    let mut fallback = None;
    for p in profiles.profiles.iter().rev().filter(|p| p.label == label) {
        let Some(key) = credentials::load_existing(store, &credentials::agent_account(&p.profile_id))? else {
            continue;
        };
        let fp = credentials::fingerprint(&key.verifying_key());
        if trust.is_revoked(&p.profile_id, &fp) {
            continue;
        }
        if trust.approved(&p.profile_id).is_some_and(|a| a.fingerprint == fp) {
            return Ok(Some((p.profile_id.clone(), key, true)));
        }
        if fallback.is_none() {
            fallback = Some((p.profile_id.clone(), key, false));
        }
    }
    Ok(fallback)
}

type Scanner = Box<dyn Fn(&Paths) -> Scan + Send + Sync>;
/// The `--attach auto` transport.
pub struct AutoClient {
    paths: Result<Paths, Error>,
    store: Arc<dyn CredentialStore>,
    selector: Selector,
    identity: Option<String>,
    label: Mutex<String>,
    session: String,
    pinned: Mutex<Option<(String, String)>>,
    /// (label, profile id, key) resolved for this process: one credential-store read per session.
    profile: Mutex<Option<(String, String, ed25519_dalek::SigningKey)>>,
    scanner: Scanner,
}
impl AutoClient {
    /// Platform paths and credential store (Keychain on macOS).
    pub fn new(selector: Selector, identity: Option<String>, label: &str) -> Result<Self, Error> {
        Self::with(Paths::resolve(), Arc::from(credentials::platform_store()), selector, identity, label)
    }
    pub fn with(
        paths: Result<Paths, Error>,
        store: Arc<dyn CredentialStore>,
        selector: Selector,
        identity: Option<String>,
        label: &str,
    ) -> Result<Self, Error> {
        Ok(Self {
            paths,
            store,
            selector,
            identity,
            label: Mutex::new(super::clean_label(label)),
            session: super::random_hex(32)?,
            pinned: Mutex::new(None),
            profile: Mutex::new(None),
            scanner: Box::new(registry::scan),
        })
    }
    /// Replace the registry scan (tests: a fake registry).
    pub fn with_scanner(mut self, scanner: impl Fn(&Paths) -> Scan + Send + Sync + 'static) -> Self {
        self.scanner = Box::new(scanner);
        self
    }
    pub fn with_session(mut self, session: String) -> Result<Self, Error> {
        if !super::is_hex(&session, 64) {
            return Err(Error::new("invalid_argument", "client/session id must be 64 lowercase hex digits"));
        }
        self.session = session;
        Ok(self)
    }
    pub fn set_label(&self, label: &str) {
        *self.label.lock().unwrap() = super::clean_label(label);
    }
    pub fn pinned(&self) -> Option<(String, String)> {
        self.pinned.lock().unwrap().clone()
    }

    /// This client's profile, cached for the process; re-resolved after a label change or revocation.
    fn profile(&self, paths: &Paths, trust: &TrustFile) -> Result<(String, ed25519_dalek::SigningKey), Error> {
        let label = self.label.lock().unwrap().clone();
        let mut cache = self.profile.lock().unwrap();
        if let Some((l, id, key)) = cache.as_ref() {
            if l == &label && !trust.is_revoked(id, &credentials::fingerprint(&key.verifying_key())) {
                return Ok((id.clone(), key.clone()));
            }
        }
        let (id, key) = resolve_profile(paths, self.store.as_ref(), trust, &label, self.identity.as_deref())?;
        *cache = Some((label, id.clone(), key.clone()));
        Ok((id, key))
    }
    /// Decide the target for this call, applying the session pin (ADR-0011 §2).
    fn target(&self, paths: &Paths) -> Result<(Record, String, ed25519_dalek::SigningKey), Error> {
        let scan = (self.scanner)(paths);
        if scan.live.is_empty() {
            return Err(host_not_running(&self.selector));
        }
        let trust = TrustFile::load(paths)?;
        let (profile_id, key) = self.profile(paths, &trust)?;
        let fingerprint = credentials::fingerprint(&key.verifying_key());
        let pinned_host = trust
            .approved(&profile_id)
            .filter(|a| a.fingerprint == fingerprint && !trust.is_revoked(&profile_id, &fingerprint))
            .map(|a| a.host_fingerprint.clone());
        let paired: Vec<String> = pinned_host.iter().cloned().collect();
        let mut pin = self.pinned.lock().unwrap();
        // A live pinned instance keeps this session; never silently switch to another launch.
        if let Some((instance, epoch)) = pin.as_ref() {
            if let Some(r) = scan.live.iter().find(|r| &r.instance_id == instance) {
                if &r.epoch == epoch {
                    return Ok((r.clone(), profile_id, key));
                }
            }
        }
        let record = match select(&scan.live, &self.selector, &paired) {
            Selection::Attach(r) => r,
            Selection::NotRunning => return Err(host_not_running(&self.selector)),
            Selection::Ambiguous(c) => return Err(ambiguous(&c)),
        };
        let previous = pin.replace((record.instance_id.clone(), record.epoch.clone()));
        if previous.is_some() {
            return Err(Error::new(
                "session_reset",
                "Varos restarted (or the attached launch closed), so earlier board handles and revisions are invalid. Call list_boards and describe again before editing.",
            ));
        }
        Ok((record, profile_id, key))
    }
    pub fn call(&self, call_id: &str, request: Request) -> Reply {
        let paths = match &self.paths {
            Ok(p) => p.clone(),
            Err(e) => return Reply::failure(e.clone()),
        };
        let (record, profile_id, key) = match self.target(&paths) {
            Ok(t) => t,
            Err(e) => return Reply::failure(e),
        };
        let frame = crate::ipc::Frame::Call { call_id: call_id.into(), request };
        self.exchange(&record, &profile_id, &key, frame).unwrap_or_else(Reply::failure)
    }
    pub fn cancel(&self, call_id: &str) {
        let Ok(paths) = &self.paths else { return };
        let Some((instance, epoch)) = self.pinned() else { return };
        let scan = (self.scanner)(paths);
        let Some(record) = scan.live.into_iter().find(|r| r.instance_id == instance && r.epoch == epoch) else {
            return;
        };
        let Ok(trust) = TrustFile::load(paths) else { return };
        let Ok((profile_id, key)) = self.profile(paths, &trust) else { return };
        let _ = self.exchange(&record, &profile_id, &key, crate::ipc::Frame::Cancel { call_id: call_id.into() });
    }
    #[cfg(unix)]
    fn exchange(
        &self,
        record: &Record,
        profile_id: &str,
        key: &ed25519_dalek::SigningKey,
        frame: crate::ipc::Frame,
    ) -> Result<Reply, Error> {
        use crate::ipc::{read_frame, write_frame};
        let io = |e: std::io::Error| {
            Error::new("io_error", format!("attachment: {e}; query request_status before retrying an edit"))
        };
        let stream = crate::ipc::connect_checked(&record.socket).map_err(|e| {
            if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused) {
                host_not_running(&self.selector)
            } else {
                io(e)
            }
        })?;
        let mut writer = stream.try_clone().map_err(io)?;
        let mut reader = std::io::BufReader::new(stream);
        let (agent, hello) = handshake::AgentSide::hello()?;
        write_frame(&mut writer, &hello).map_err(io)?;
        let bytes =
            read_frame(&mut reader).map_err(io)?.ok_or_else(|| io(std::io::Error::other("host disconnected")))?;
        let challenge: handshake::Frame = match serde_json::from_slice(&bytes) {
            Ok(c) => c,
            Err(_) => {
                return serde_json::from_slice::<Reply>(&bytes).map_err(|e| io(std::io::Error::other(e)));
            }
        };
        let expect = handshake::Expect {
            instance_id: record.instance_id.clone(),
            epoch: record.epoch.clone(),
            host_fingerprint: record.host_fingerprint.clone(),
        };
        let label = self.label.lock().unwrap().clone();
        let proof = agent.respond(&challenge, &expect, key, profile_id, &self.session, &label)?;
        write_frame(&mut writer, &proof).map_err(io)?;
        let welcome: Reply = serde_json::from_slice(
            &read_frame(&mut reader).map_err(io)?.ok_or_else(|| io(std::io::Error::other("host disconnected")))?,
        )
        .map_err(|e| io(std::io::Error::other(e)))?;
        if !welcome.ok {
            return Ok(welcome);
        }
        write_frame(&mut writer, &frame).map_err(io)?;
        serde_json::from_slice(
            &read_frame(&mut reader).map_err(io)?.ok_or_else(|| io(std::io::Error::other("host disconnected")))?,
        )
        .map_err(|e| io(std::io::Error::other(e)))
    }
    #[cfg(not(unix))]
    fn exchange(
        &self,
        _: &Record,
        _: &str,
        _: &ed25519_dalek::SigningKey,
        _: crate::ipc::Frame,
    ) -> Result<Reply, Error> {
        Err(Error::new("unsupported", "native attachment is Unix-only; Windows is compile-only"))
    }
}
