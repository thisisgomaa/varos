//! Connection 1.0 handshake (ADR-0011 §2, §3, §5). Pure message logic; sockets live in `ipc`.
//!
//! 1. agent → `client_hello { connection: [offered], api, nonce }`
//! 2. host  → `host_challenge { connection, api, instance_id, epoch, host_key, nonce, signature }`
//!    The host signs (offered, chosen, api, instance, epoch, host key, both nonces): proves the
//!    host key, freshness (agent nonce), and binds epoch + negotiated versions (no downgrade).
//! 3. agent → `client_proof { profile_id, agent_key, session, label, signature }`
//!    The agent signs the same binding plus its profile/session: proves possession over the
//!    host's fresh nonce. The host consumes that nonce once.
//! 4. host  → `Reply` (welcome with scopes, or `pairing_required` / refusal), then one call frame.
use super::{credentials, hex, is_hex, random_hex, CONNECTION};
use crate::{Error, API};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

const HOST_CONTEXT: &[u8] = b"varos-bridge connection/1 host-challenge\0";
const AGENT_CONTEXT: &[u8] = b"varos-bridge connection/1 agent-proof\0";
pub const SUPPORTED: &[&str] = &[CONNECTION];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Frame {
    ClientHello {
        connection: Vec<String>,
        api: String,
        nonce: String,
    },
    HostChallenge {
        connection: String,
        api: String,
        instance_id: String,
        epoch: String,
        host_key: String,
        nonce: String,
        signature: String,
    },
    ClientProof {
        profile_id: String,
        agent_key: String,
        session: String,
        label: String,
        signature: String,
    },
}

/// Offered versions as one unambiguous transcript field: count, then each length-prefixed.
fn encode_offered(versions: &[String]) -> String {
    let mut out = format!("{};", versions.len());
    for v in versions {
        out.push_str(&format!("{}:{v}", v.len()));
    }
    out
}
fn transcript(context: &[u8], fields: &[&str]) -> Vec<u8> {
    let mut out = context.to_vec();
    for f in fields {
        out.extend_from_slice(&(f.len() as u32).to_be_bytes());
        out.extend_from_slice(f.as_bytes());
    }
    out
}
fn parse_signature(text: &str) -> Result<Signature, Error> {
    let bytes: [u8; 64] = super::unhex(text)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| Error::new("identity_invalid", "signature must be 128 hex digits"))?;
    Ok(Signature::from_bytes(&bytes))
}
pub fn valid_profile_id(id: &str) -> bool {
    is_hex(id, 16)
}

/// Host state for one connection. The host nonce can be consumed exactly once.
#[derive(Debug)]
pub struct HostSide {
    offered: String,
    instance_id: String,
    epoch: String,
    host_key: String,
    client_nonce: String,
    host_nonce: String,
    consumed: bool,
}
#[derive(Clone, Debug)]
pub struct VerifiedAgent {
    pub profile_id: String,
    pub key: VerifyingKey,
    pub fingerprint: String,
    pub session: String,
    pub label: String,
}
impl HostSide {
    pub fn challenge(key: &SigningKey, instance_id: &str, epoch: &str, hello: &Frame) -> Result<(Self, Frame), Error> {
        let Frame::ClientHello { connection, api, nonce } = hello else {
            return Err(Error::new("unsupported", "client_hello required (legacy token Hello is not accepted here)"));
        };
        if !is_hex(nonce, 64) {
            return Err(Error::new("invalid_argument", "nonce must be 64 lowercase hex digits"));
        }
        if connection.len() > 8 || connection.iter().any(|v| v.len() > 16) {
            return Err(Error::new("invalid_argument", "connection version list too long"));
        }
        if !connection.iter().any(|v| v == CONNECTION) {
            return Err(Error::new("unsupported", format!("connection {CONNECTION} required")));
        }
        if api != API {
            return Err(Error::new("unsupported", "Bridge API must be 1.0"));
        }
        let state = Self {
            offered: encode_offered(connection),
            instance_id: instance_id.into(),
            epoch: epoch.into(),
            host_key: credentials::public_hex(&key.verifying_key()),
            client_nonce: nonce.clone(),
            host_nonce: random_hex(32)?,
            consumed: false,
        };
        let signature = key.sign(&state.host_transcript());
        let frame = Frame::HostChallenge {
            connection: CONNECTION.into(),
            api: API.into(),
            instance_id: state.instance_id.clone(),
            epoch: state.epoch.clone(),
            host_key: state.host_key.clone(),
            nonce: state.host_nonce.clone(),
            signature: hex(&signature.to_bytes()),
        };
        Ok((state, frame))
    }
    fn host_transcript(&self) -> Vec<u8> {
        transcript(
            HOST_CONTEXT,
            &[
                &self.offered,
                CONNECTION,
                API,
                &self.instance_id,
                &self.epoch,
                &self.host_key,
                &self.client_nonce,
                &self.host_nonce,
            ],
        )
    }
    /// Verify the agent's possession proof. Single use: a second proof on this state fails.
    pub fn verify(&mut self, proof: &Frame) -> Result<VerifiedAgent, Error> {
        if self.consumed {
            return Err(Error::new("identity_invalid", "handshake nonce already used"));
        }
        self.consumed = true;
        let Frame::ClientProof { profile_id, agent_key, session, label, signature } = proof else {
            return Err(Error::new("identity_invalid", "client_proof required"));
        };
        if !valid_profile_id(profile_id) || !is_hex(session, 64) || label.len() > 64 {
            return Err(Error::new("identity_invalid", "malformed profile, session or label"));
        }
        let key = credentials::parse_public(agent_key)?;
        let message = agent_transcript(
            &self.offered,
            &self.instance_id,
            &self.epoch,
            &self.host_key,
            &self.client_nonce,
            &self.host_nonce,
            profile_id,
            agent_key,
            session,
            label,
        );
        key.verify_strict(&message, &parse_signature(signature)?)
            .map_err(|_| Error::new("identity_invalid", "agent key proof failed"))?;
        Ok(VerifiedAgent {
            profile_id: profile_id.clone(),
            fingerprint: credentials::fingerprint(&key),
            key,
            session: session.clone(),
            label: super::clean_label(label),
        })
    }
}
#[allow(clippy::too_many_arguments)]
fn agent_transcript(
    offered: &str,
    instance_id: &str,
    epoch: &str,
    host_key: &str,
    client_nonce: &str,
    host_nonce: &str,
    profile_id: &str,
    agent_key: &str,
    session: &str,
    label: &str,
) -> Vec<u8> {
    transcript(
        AGENT_CONTEXT,
        &[
            offered,
            CONNECTION,
            API,
            instance_id,
            epoch,
            host_key,
            client_nonce,
            host_nonce,
            profile_id,
            agent_key,
            session,
            label,
        ],
    )
}

/// What the agent expects of the host it located in the registry.
#[derive(Clone, Debug)]
pub struct Expect {
    pub instance_id: String,
    pub epoch: String,
    pub host_fingerprint: String,
}
/// Agent side for one connection.
pub struct AgentSide {
    nonce: String,
    offered: String,
}
impl AgentSide {
    pub fn hello() -> Result<(Self, Frame), Error> {
        let nonce = random_hex(32)?;
        let offered: Vec<String> = SUPPORTED.iter().map(|s| s.to_string()).collect();
        let state = Self { nonce: nonce.clone(), offered: encode_offered(&offered) };
        Ok((state, Frame::ClientHello { connection: offered, api: API.into(), nonce }))
    }
    /// Verify host identity, freshness, epoch and negotiated versions; then sign our proof.
    pub fn respond(
        &self,
        challenge: &Frame,
        expect: &Expect,
        agent: &SigningKey,
        profile_id: &str,
        session: &str,
        label: &str,
    ) -> Result<Frame, Error> {
        let Frame::HostChallenge { connection, api, instance_id, epoch, host_key, nonce, signature } = challenge else {
            return Err(Error::new("identity_invalid", "host_challenge expected"));
        };
        if connection != CONNECTION || api != API {
            return Err(Error::new("unsupported", "host negotiated an unsupported connection/API version"));
        }
        let key = credentials::parse_public(host_key)?;
        let message = transcript(
            HOST_CONTEXT,
            &[&self.offered, CONNECTION, API, instance_id, epoch, host_key, &self.nonce, nonce],
        );
        key.verify_strict(&message, &parse_signature(signature)?)
            .map_err(|_| Error::new("host_identity_mismatch", "host signature does not verify"))?;
        let fingerprint = credentials::fingerprint(&key);
        if fingerprint != expect.host_fingerprint {
            return Err(Error::new("host_identity_mismatch", "host key differs from its registry record"));
        }
        // A host key that differs from the one pinned at pairing is NOT a dead end (ADR-0011
        // §3.5 "asks again"): the proof below only binds this host's fresh nonce, and the host's
        // trust store answers `pairing_required` (naming the key change) instead of granting.
        if instance_id != &expect.instance_id {
            return Err(Error::new("host_identity_mismatch", "host instance differs from the registry record"));
        }
        if epoch != &expect.epoch {
            return Err(Error::new("session_reset", "host epoch changed; re-discover and resync"));
        }
        if !is_hex(nonce, 64) {
            return Err(Error::new("identity_invalid", "malformed host nonce"));
        }
        let agent_key = credentials::public_hex(&agent.verifying_key());
        let label = super::clean_label(label);
        let message = agent_transcript(
            &self.offered,
            instance_id,
            epoch,
            host_key,
            &self.nonce,
            nonce,
            profile_id,
            &agent_key,
            session,
            &label,
        );
        Ok(Frame::ClientProof {
            profile_id: profile_id.into(),
            agent_key,
            session: session.into(),
            label,
            signature: hex(&agent.sign(&message).to_bytes()),
        })
    }
}
