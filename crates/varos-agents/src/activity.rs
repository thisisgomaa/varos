//! A bounded log of what changed in each document, revision by revision, and who changed it.
//!
//! It answers two questions: "what changed since revision R" (an agent's `describe since`, so it
//! reads a diff instead of the whole document again) and "what did each agent do" (review). It
//! keeps ids and labels only, never document contents, and it lives only as long as the session
//! (owner decision 2026-10-19: in-session for v1). Undo steps may be trimmed by the history budget;
//! this log is trimmed separately, by [`Activity::MAX_ENTRIES`] and [`Activity::MAX_BYTES`].

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

use vectorcraft_doc::NodeId;

use crate::actor::ActorKey;
use crate::diff::Change;

/// One recorded change.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub from: u64,
    pub to: u64,
    /// The agent that made it; `None` for the person at the keyboard.
    pub actor: Option<ActorKey>,
    /// The agent's label when it was made (labels of forgotten sessions stay readable).
    pub label: String,
    /// The undo step's name ("Move", "Rectangle").
    pub step: String,
    pub change: Change,
    pub at: Instant,
}

impl Entry {
    fn bytes(&self) -> usize {
        let c = &self.change;
        let ids = c.created.len() + c.changed.len() + c.removed.len() + c.reordered.len();
        let boards = c.artboards_created.len() + c.artboards_changed.len() + c.artboards_removed.len();
        std::mem::size_of::<Self>() + ids * std::mem::size_of::<NodeId>() + boards * 4 + self.label.len() + self.step.len()
    }
}

/// The answer to "what changed since revision R".
#[derive(Clone, Debug, PartialEq)]
pub enum Since {
    /// The net change from R to the current revision (empty when R is current).
    Changes(Change),
    /// R is unknown, too old, or a revision in between was not observed: describe afresh.
    ResyncRequired,
}

#[derive(Default, Debug)]
struct Log {
    entries: VecDeque<Entry>,
    bytes: usize,
}

/// The logs of all open documents, by document id.
#[derive(Default, Debug)]
pub struct Activity {
    docs: BTreeMap<u64, Log>,
}

impl Activity {
    /// Most entries kept per document.
    pub const MAX_ENTRIES: usize = 128;
    /// About the most memory kept per document.
    pub const MAX_BYTES: usize = 8 << 20;

    pub fn new() -> Self {
        Self::default()
    }

    /// Record that document `doc` went from revision `entry.from` to `entry.to`. An entry that
    /// doesn't advance the revision is ignored. One larger than [`Activity::MAX_BYTES`] by itself
    /// is not kept: the gap it leaves makes every `since` across it [`Since::ResyncRequired`].
    pub fn record(&mut self, doc: u64, entry: Entry) {
        if entry.to <= entry.from {
            return;
        }
        if entry.bytes() > Self::MAX_BYTES {
            return;
        }
        let log = self.docs.entry(doc).or_default();
        log.bytes += entry.bytes();
        log.entries.push_back(entry);
        while log.entries.len() > Self::MAX_ENTRIES || (log.bytes > Self::MAX_BYTES && log.entries.len() > 1) {
            if let Some(e) = log.entries.pop_front() {
                log.bytes = log.bytes.saturating_sub(e.bytes());
            }
        }
    }

    /// The document closed: forget its log.
    pub fn close(&mut self, doc: u64) {
        self.docs.remove(&doc);
    }

    /// Keep only the logs of the documents `open` says are still open.
    pub fn retain(&mut self, open: impl Fn(u64) -> bool) {
        self.docs.retain(|doc, _| open(*doc));
    }

    /// The kept entries of `doc`, oldest first.
    pub fn entries(&self, doc: u64) -> impl Iterator<Item = &Entry> {
        self.docs.get(&doc).into_iter().flat_map(|l| l.entries.iter())
    }

    /// The kept entries `actor` made in `doc`, oldest first: what a review lists.
    pub fn by_actor(&self, doc: u64, actor: ActorKey) -> impl Iterator<Item = &Entry> {
        self.entries(doc).filter(move |e| e.actor == Some(actor))
    }

    /// The net change of `doc` from revision `since` to `current`. Every revision in between must
    /// have been recorded, one entry after the other; otherwise [`Since::ResyncRequired`].
    pub fn since(&self, doc: u64, since: u64, current: u64) -> Since {
        if since == current {
            return Since::Changes(Change::default());
        }
        if since > current {
            return Since::ResyncRequired;
        }
        let Some(log) = self.docs.get(&doc) else { return Since::ResyncRequired };
        let Some(start) = log.entries.iter().position(|e| e.from == since) else { return Since::ResyncRequired };
        let mut net = Net::default();
        let mut rev = since;
        for e in log.entries.iter().skip(start) {
            if e.from != rev {
                return Since::ResyncRequired;
            }
            net.add(&e.change);
            rev = e.to;
            if rev == current {
                return Since::Changes(net.finish());
            }
        }
        Since::ResyncRequired
    }
}

/// Several changes folded into one: created then removed is nothing, created then changed is
/// created, changed then removed is removed. A restack is kept unless its container was created or
/// removed in between (restacking back to the first order still counts: the log has ids, not the
/// orders themselves).
#[derive(Default)]
struct Net {
    created: BTreeSet<NodeId>,
    changed: BTreeSet<NodeId>,
    removed: BTreeSet<NodeId>,
    reordered: BTreeSet<NodeId>,
    layers_reordered: bool,
    boards_created: BTreeSet<u32>,
    boards_changed: BTreeSet<u32>,
    boards_removed: BTreeSet<u32>,
}

impl Net {
    fn add(&mut self, c: &Change) {
        fold(&mut self.created, &mut self.changed, &mut self.removed, &c.created, &c.changed, &c.removed);
        self.reordered.extend(&c.reordered);
        self.layers_reordered |= c.layers_reordered;
        fold(
            &mut self.boards_created,
            &mut self.boards_changed,
            &mut self.boards_removed,
            &c.artboards_created,
            &c.artboards_changed,
            &c.artboards_removed,
        );
    }

    fn finish(self) -> Change {
        // A container made or removed in between is reported as that, not as restacked.
        let reordered = self.reordered.iter().copied().filter(|id| !self.created.contains(id) && !self.removed.contains(id)).collect();
        Change {
            reordered,
            layers_reordered: self.layers_reordered,
            created: self.created.into_iter().collect(),
            changed: self.changed.into_iter().collect(),
            removed: self.removed.into_iter().collect(),
            artboards_created: self.boards_created.into_iter().collect(),
            artboards_changed: self.boards_changed.into_iter().collect(),
            artboards_removed: self.boards_removed.into_iter().collect(),
        }
    }
}

fn fold<T: Ord + Copy>(created: &mut BTreeSet<T>, changed: &mut BTreeSet<T>, removed: &mut BTreeSet<T>, c: &[T], ch: &[T], r: &[T]) {
    for id in c {
        // An id back after a removal (undo of a delete) is a change of something that existed.
        if removed.remove(id) {
            changed.insert(*id);
        } else {
            created.insert(*id);
        }
    }
    for id in ch {
        if !created.contains(id) {
            changed.insert(*id);
        }
    }
    for id in r {
        if !created.remove(id) {
            changed.remove(id);
            removed.insert(*id);
        }
    }
}
