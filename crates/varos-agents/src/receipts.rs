//! Idempotent request ids, per agent (a port of Varos Bridge's receipts, ADR-0009 §3).
//!
//! A mutation carries `request_id` `r1`, `r2`, … increasing per agent session. After a successful
//! commit the same id with the same payload returns the original receipt instead of running again
//! (a retry after a timeout never applies an edit twice); the same id with another payload is
//! refused. A failure before commit keeps no receipt, so the same id may be retried. The last
//! [`Receipts::KEEP`] receipts per session are kept; older ids are refused, never rerun.

use std::collections::{BTreeMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde_json::Value;

use crate::actor::ActorKey;

/// Why a request id was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReceiptError {
    /// Not `r` followed by a positive integer.
    InvalidId,
    /// This id already committed with a different payload.
    PayloadMismatch,
    /// Older than the kept receipts: it may have run; inspect the revision instead of retrying.
    TooOld,
    /// Too many agent sessions sent mutations this launch.
    TooManyClients,
}

impl ReceiptError {
    /// The stable error code an agent sees.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidId | Self::PayloadMismatch => "invalid_argument",
            Self::TooOld => "not_found",
            Self::TooManyClients => "limit_exceeded",
        }
    }

    /// A readable reason.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::InvalidId => "request_id must be r followed by a positive integer",
            Self::PayloadMismatch => "request_id reused with a different payload",
            Self::TooOld => "request_id older than the retained receipt window; inspect the revision",
            Self::TooManyClients => "too many agent sessions this launch",
        }
    }
}

/// What to do with a mutation.
#[derive(Clone, Debug, PartialEq)]
pub enum Admission {
    /// Run it; on success, [`Receipts::commit`] the reply with this sequence number and digest.
    Fresh { seq: u64, digest: u64 },
    /// Already committed: return this reply, run nothing.
    Replay(Value),
}

#[derive(Debug)]
struct Kept {
    seq: u64,
    digest: u64,
    reply: Value,
}

#[derive(Debug, Default)]
struct Client {
    high: u64,
    kept: VecDeque<Kept>,
}

#[derive(Debug, Default)]
pub struct Receipts {
    clients: BTreeMap<ActorKey, Client>,
}

impl Receipts {
    /// Receipts kept per agent session.
    pub const KEEP: usize = 128;
    /// Agent sessions that may send mutations per launch.
    pub const MAX_CLIENTS: usize = 64;

    pub fn new() -> Self {
        Self::default()
    }

    /// Decide on a mutation from `actor` with `request_id` and `payload` (the whole request).
    pub fn admit(&self, actor: ActorKey, request_id: &str, payload: &Value) -> Result<Admission, ReceiptError> {
        let seq = sequence(request_id).ok_or(ReceiptError::InvalidId)?;
        let digest = digest(payload);
        let Some(client) = self.clients.get(&actor) else {
            if self.clients.len() >= Self::MAX_CLIENTS {
                return Err(ReceiptError::TooManyClients);
            }
            return Ok(Admission::Fresh { seq, digest });
        };
        if let Some(k) = client.kept.iter().find(|k| k.seq == seq) {
            return if k.digest == digest { Ok(Admission::Replay(k.reply.clone())) } else { Err(ReceiptError::PayloadMismatch) };
        }
        if seq <= client.high {
            return Err(ReceiptError::TooOld);
        }
        Ok(Admission::Fresh { seq, digest })
    }

    /// Keep the reply of a committed mutation admitted as [`Admission::Fresh`]. A later id
    /// retires the earlier unused ones.
    pub fn commit(&mut self, actor: ActorKey, seq: u64, digest: u64, reply: Value) {
        if !self.clients.contains_key(&actor) && self.clients.len() >= Self::MAX_CLIENTS {
            return;
        }
        let client = self.clients.entry(actor).or_default();
        client.high = client.high.max(seq);
        client.kept.push_back(Kept { seq, digest, reply });
        while client.kept.len() > Self::KEEP {
            client.kept.pop_front();
        }
    }

    /// The kept reply of `actor`'s `request_id` (`request_status`).
    pub fn status(&self, actor: ActorKey, request_id: &str) -> Option<&Value> {
        let seq = sequence(request_id)?;
        self.clients.get(&actor)?.kept.iter().find(|k| k.seq == seq).map(|k| &k.reply)
    }
}

/// `r12` → 12; anything else (`r0`, `r012`, `12`, `rx`) → `None`.
fn sequence(id: &str) -> Option<u64> {
    let digits = id.strip_prefix('r')?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|n| *n > 0)
}

/// A digest of `v` that ignores object key order (so it doesn't depend on how the JSON map
/// keeps its keys).
fn digest(v: &Value) -> u64 {
    let mut h = DefaultHasher::new();
    canonical(v, &mut h);
    h.finish()
}

fn canonical(v: &Value, h: &mut DefaultHasher) {
    match v {
        Value::Null => 0u8.hash(h),
        Value::Bool(b) => (1u8, b).hash(h),
        Value::Number(n) => (2u8, n.to_string()).hash(h),
        Value::String(s) => (3u8, s).hash(h),
        Value::Array(a) => {
            (4u8, a.len()).hash(h);
            for x in a {
                canonical(x, h);
            }
        }
        Value::Object(m) => {
            (5u8, m.len()).hash(h);
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            for k in keys {
                k.hash(h);
                if let Some(x) = m.get(k) {
                    canonical(x, h);
                }
            }
        }
    }
}
