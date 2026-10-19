//! Who an automation request comes from.
//!
//! Trust is the local user: any agent the user runs may read, edit and save, with no pairing,
//! approval or OS credential prompt (owner decision 2026-10-08). The identity here is only for
//! attribution: the orange label on the canvas and the agent's rows in the History panel. Labels
//! and profiles are claims made by the client, never proof, and are treated as untrusted text.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// One agent session, as the host numbers them. Stable for the life of the session (a reconnect of
/// the same session keeps it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorKey(pub u64);

/// Longest label kept, in characters.
pub const MAX_LABEL: usize = 24;
/// Longest profile id kept, in characters.
pub const MAX_PROFILE: usize = 64;
/// The label for a client that gives none (or only invisible characters).
pub const DEFAULT_LABEL: &str = "Agent";

/// A client-supplied label made safe to draw: control, zero-width and bidi-override characters
/// removed, trimmed, at most [`MAX_LABEL`] characters; empty becomes [`DEFAULT_LABEL`].
pub fn sanitize_label(label: &str) -> String {
    let label: String = label.chars().filter(|c| !invisible(*c)).take(MAX_LABEL).collect();
    let label = label.trim();
    if label.is_empty() { DEFAULT_LABEL.into() } else { label.into() }
}

fn invisible(c: char) -> bool {
    c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{061c}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

fn sanitize_profile(profile: &str) -> String {
    profile.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).take(MAX_PROFILE).collect()
}

/// What the registry knows about one session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorInfo {
    pub key: ActorKey,
    /// The session id the client chose (one per MCP server process), or a connection id.
    pub session: String,
    /// A stable profile id for grouping history (sanitized; may be empty).
    pub profile: String,
    /// The label as the client gave it, sanitized: "Claude Code".
    pub base_label: String,
    /// 1 for the first live session with this base label, 2 for the second…
    pub ordinal: u32,
    pub last_seen: Instant,
    /// Its connection closed ([`Registry::depart`]): its number is free for a new session with the
    /// same label. It keeps its key if it comes back (a reconnect), renumbered if its number was
    /// taken meanwhile.
    pub departed: bool,
}

impl ActorInfo {
    /// The label drawn on the canvas and in the History panel: "Claude Code", "Claude Code 2".
    pub fn label(&self) -> String {
        if self.ordinal <= 1 { self.base_label.clone() } else { format!("{} {}", self.base_label, self.ordinal) }
    }
}

/// The live agent sessions, numbered so that two sessions with the same label stay apart: a
/// label is numbered only while another session with it is connected ("Claude Code", "Claude Code
/// 2"); once they have both ended, the next is plain "Claude Code" again.
#[derive(Debug, Default)]
pub struct Registry {
    by_session: BTreeMap<String, ActorKey>,
    actors: BTreeMap<ActorKey, ActorInfo>,
    next: u64,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// The key for a request from `session`, registering it on first sight. A known session keeps
    /// its key and label; a new one with the same label as a connected session gets the smallest
    /// free ordinal ("Claude Code 2"). Sessions that departed don't hold their numbers.
    pub fn admit(&mut self, session: &str, profile: &str, label: &str, now: Instant) -> ActorKey {
        if let Some(key) = self.by_session.get(session).copied()
            && let Some(info) = self.actors.get(&key)
        {
            let (base, ordinal, departed) = (info.base_label.clone(), info.ordinal, info.departed);
            // Back after its connection closed: its number again, unless another took it.
            let ordinal = if departed && self.taken(&base, Some(key)).contains(&ordinal) { self.free(&base, Some(key)) } else { ordinal };
            if let Some(info) = self.actors.get_mut(&key) {
                info.last_seen = now;
                info.departed = false;
                info.ordinal = ordinal;
            }
            return key;
        }
        let base_label = sanitize_label(label);
        let ordinal = self.free(&base_label, None);
        self.next += 1;
        let key = ActorKey(self.next);
        self.by_session.insert(session.to_string(), key);
        self.actors.insert(
            key,
            ActorInfo { key, session: session.to_string(), profile: sanitize_profile(profile), base_label, ordinal, last_seen: now, departed: false },
        );
        key
    }

    /// The numbers the connected sessions labelled `base` hold (`but` aside).
    fn taken(&self, base: &str, but: Option<ActorKey>) -> Vec<u32> {
        self.actors.values().filter(|a| a.base_label == base && !a.departed && Some(a.key) != but).map(|a| a.ordinal).collect()
    }

    /// The smallest number no connected session labelled `base` holds (`but` aside).
    fn free(&self, base: &str, but: Option<ActorKey>) -> u32 {
        let taken = self.taken(base, but);
        (1..).find(|n| !taken.contains(n)).unwrap_or(1)
    }

    /// The connection of `session` closed (its agent ended, or the connection timed out): its
    /// number is free for the next session with its label. Its key and label stay until it is
    /// forgotten ([`Self::forget_idle`]), so a reconnect is still the same agent.
    pub fn depart(&mut self, session: &str) {
        if let Some(info) = self.by_session.get(session).and_then(|key| self.actors.get_mut(key)) {
            info.departed = true;
        }
    }

    pub fn get(&self, key: ActorKey) -> Option<&ActorInfo> {
        self.actors.get(&key)
    }

    /// The label for `key`, or the default label for a key no longer registered.
    pub fn label(&self, key: ActorKey) -> String {
        self.get(key).map_or_else(|| DEFAULT_LABEL.to_string(), ActorInfo::label)
    }

    /// Forget sessions not seen for `idle`. Their keys stay valid in history rows (they show the
    /// label recorded with the row); their ordinals become free again.
    pub fn forget_idle(&mut self, now: Instant, idle: Duration) {
        let gone: Vec<ActorKey> = self.actors.values().filter(|a| now.saturating_duration_since(a.last_seen) >= idle).map(|a| a.key).collect();
        for key in gone {
            if let Some(info) = self.actors.remove(&key) {
                self.by_session.remove(&info.session);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.actors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }
}
