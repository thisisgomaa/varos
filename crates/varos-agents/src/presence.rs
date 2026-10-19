//! On-canvas agent presence (owner, 2026-10-08): the artboard an agent works on gets an orange
//! outline with the agent's name, and the objects it creates or changes appear one by one with an
//! orange outline that fades. The person's own selection stays azure and wins over orange.
//!
//! Everything is derived on the host from the commands the agent sends and the document before and
//! after each one ([`crate::diff`]); the agent sends nothing for it, so it costs no tokens. Nothing
//! is drawn while no agent is working. This is a port of Varos' `varos-app/src/agent_presence.rs`
//! onto VectorCraft's document (node ids, artboard ids), without timers: the frontend asks
//! [`Presence::frame`] each frame and schedules the next one from [`Frame::repaint_after`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, Instant};

use vectorcraft_doc::{Document, Node, NodeId, NodeKind};
use vectorcraft_geom::Rect;

use crate::actor::{ActorKey, sanitize_label};
use crate::diff::Change;

/// How long the artboard outline stays after an agent's last edit.
pub const HOLD: Duration = Duration::from_secs(4);
/// The delay between two objects appearing.
pub const STAGGER: Duration = Duration::from_millis(25);
/// The longest a whole batch takes to appear, however many objects it has.
pub const REVEAL_CAP: Duration = Duration::from_millis(1500);
/// How long one object's outline takes to fade.
pub const FADE: Duration = Duration::from_millis(900);
/// The short final fade of outlines interrupted by newer work.
pub const FINISH: Duration = Duration::from_millis(120);

#[derive(Clone, Debug)]
struct Reveal {
    id: NodeId,
    board: u64,
    at: Instant,
    end: Instant,
}

#[derive(Clone, Debug)]
struct Session {
    label: String,
    profile: String,
    board: u64,
    artboard: Option<u32>,
    in_flight: bool,
    last_edit: Instant,
    reveal: Vec<Reveal>,
}

/// One artboard outlined for the agents working on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub artboard: u32,
    pub rect: Rect,
    pub name: String,
    /// The agents' labels, in a stable order (by profile, then label).
    pub labels: Vec<String>,
}

/// One object outlined, at `alpha` (1 when it just appeared, 0 when its fade ends).
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub id: NodeId,
    pub rect: Rect,
    pub alpha: f32,
}

/// What to draw this frame, in document coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub pages: Vec<Page>,
    pub objects: Vec<Object>,
    /// Objects that appear later: their box and how long until they do.
    pub pending: Vec<(Rect, Duration)>,
    /// When the first artboard outline whose agent finished goes away.
    pub expiry_after: Option<Duration>,
    /// When to draw again, if anything is visible at all ([`repaint_after`] with everything visible).
    pub repaint_after: Option<Duration>,
}

impl Frame {
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty() && self.objects.is_empty() && self.pending.is_empty()
    }
}

/// Where an object is drawn: its box, and the ids that make the person's selection win over it
/// (itself, its ancestors and its descendants).
#[derive(Clone, Debug)]
struct Placed {
    rect: Rect,
    members: Vec<NodeId>,
}

/// Object boxes for one revision of one document: looked up once per revision, not per frame.
#[derive(Default)]
struct Geometry {
    key: Option<(u64, u64)>,
    placed: HashMap<NodeId, Option<Placed>>,
}

impl Geometry {
    fn prepare(&mut self, board: u64, revision: u64) {
        if self.key != Some((board, revision)) {
            self.key = Some((board, revision));
            self.placed.clear();
        }
    }

    fn get(&mut self, doc: &Document, id: NodeId) -> Option<&Placed> {
        self.placed.entry(id).or_insert_with(|| place(doc, id)).as_ref()
    }
}

/// An object's box and selection members, or `None` for a layer, a hidden object (or one inside a
/// hidden container), an object with no box or one no longer in the document.
fn place(doc: &Document, id: NodeId) -> Option<Placed> {
    let path = doc.index_path(id)?;
    let mut members = Vec::with_capacity(path.len());
    let mut node: Option<&Node> = None;
    for depth in 1..=path.len() {
        let n = doc.node_at(&path[..depth])?;
        if !n.visible {
            return None;
        }
        members.push(n.id);
        node = Some(n);
    }
    let node = node?;
    if matches!(node.kind, NodeKind::Layer { .. }) {
        return None;
    }
    let rect = node.visual_bounds().filter(|r| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()))?;
    node.walk(&mut |d: &Node| {
        if d.id != id {
            members.push(d.id);
        }
    });
    Some(Placed { rect, members })
}

/// What [`Presence::begin`] replaced, so that [`Presence::abort`] can put it back.
#[derive(Debug)]
#[must_use = "finish or abort the request"]
pub struct Token {
    actor: ActorKey,
    previous: Option<Session>,
}

/// The presence of every agent session.
#[derive(Default)]
pub struct Presence {
    sessions: BTreeMap<ActorKey, Session>,
    geometry: Geometry,
}

fn stagger(n: usize) -> Duration {
    match u32::try_from(n) {
        Ok(0) => Duration::ZERO,
        Ok(n) => STAGGER.min(REVEAL_CAP / n),
        Err(_) => Duration::ZERO,
    }
}

impl Presence {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether any agent shows: callers skip the per-frame work when not.
    pub fn is_active(&self) -> bool {
        !self.sessions.is_empty()
    }

    /// An agent request starts on document `board`, probably on `artboard`: its artboard lights
    /// up at once. Outlines still fading from its earlier work finish quickly.
    pub fn begin(&mut self, actor: ActorKey, label: &str, profile: &str, board: u64, artboard: Option<u32>, now: Instant) -> Token {
        let previous = self.sessions.get(&actor).cloned();
        let mut reveal = previous.as_ref().map_or_else(Vec::new, |s| s.reveal.clone());
        for r in &mut reveal {
            r.at = r.at.min(now);
            r.end = r.end.min(now + FINISH);
        }
        reveal.retain(|r| r.end > now);
        self.sessions.insert(
            actor,
            Session { label: sanitize_label(label), profile: profile.to_string(), board, artboard, in_flight: true, last_edit: now, reveal },
        );
        Token { actor, previous }
    }

    /// The request failed or changed nothing: as if it never began.
    pub fn abort(&mut self, token: Token) {
        match token.previous {
            Some(previous) => {
                self.sessions.insert(token.actor, previous);
            }
            None => {
                self.sessions.remove(&token.actor);
            }
        }
    }

    /// The request committed `change`, on `artboard` (see [`target_artboard`]): its objects appear
    /// one by one, at most [`REVEAL_CAP`] for the whole batch; removed objects stop showing,
    /// whichever agent revealed them.
    pub fn finish(&mut self, token: Token, change: &Change, artboard: Option<u32>, now: Instant) {
        let actor = token.actor;
        let Some(board) = self.sessions.get(&actor).map(|s| s.board) else { return };
        let removed: HashSet<NodeId> = change.removed.iter().copied().collect();
        for s in self.sessions.values_mut() {
            for r in &mut s.reveal {
                r.at = r.at.min(now);
                r.end = r.end.min(now + FINISH);
            }
            s.reveal.retain(|r| r.board != board || !removed.contains(&r.id));
        }
        let Some(s) = self.sessions.get_mut(&actor) else { return };
        s.in_flight = false;
        s.last_edit = now;
        if artboard.is_some() {
            s.artboard = artboard;
        }
        let ids = change.touched();
        let incoming: HashSet<NodeId> = ids.iter().copied().collect();
        s.reveal.retain(|r| r.board != board || !incoming.contains(&r.id));
        let step = stagger(ids.len());
        for (i, id) in ids.into_iter().enumerate() {
            // step * i stays within REVEAL_CAP for i < n (and step is zero past u32::MAX objects).
            let at = now + step * u32::try_from(i).unwrap_or(u32::MAX);
            s.reveal.push(Reveal { id, board, at, end: at + FADE });
        }
    }

    /// Drop sessions whose document closed (`open(board)` false) or that finished more than
    /// [`HOLD`] ago, and outlines that ended.
    pub fn retain(&mut self, open: impl Fn(u64) -> bool, now: Instant) {
        self.sessions.retain(|_, s| open(s.board) && (s.in_flight || now < s.last_edit + HOLD));
        for s in self.sessions.values_mut() {
            s.reveal.retain(|r| r.end > now && open(r.board));
        }
    }

    /// What to draw over document `board` at `revision`, given the person's `selected` objects.
    pub fn frame(&mut self, board: u64, revision: u64, doc: &Document, selected: &HashSet<NodeId>, now: Instant) -> Frame {
        let mut frame = Frame::default();
        if self.sessions.is_empty() {
            return frame;
        }
        self.geometry.prepare(board, revision);
        let mut pages: BTreeMap<u32, Vec<(String, String)>> = BTreeMap::new();
        let mut objects: BTreeMap<NodeId, (Rect, f32)> = BTreeMap::new();
        for s in self.sessions.values().filter(|s| s.in_flight || now < s.last_edit + HOLD) {
            if s.board == board
                && let Some(id) = s.artboard.filter(|id| doc.artboards.iter().any(|a| a.id == *id))
            {
                pages.entry(id).or_default().push((s.profile.clone(), s.label.clone()));
                if !s.in_flight {
                    let delay = (s.last_edit + HOLD).saturating_duration_since(now);
                    frame.expiry_after = Some(frame.expiry_after.map_or(delay, |d| d.min(delay)));
                }
            }
            for r in s.reveal.iter().filter(|r| r.board == board && r.end > now) {
                let Some(placed) = self.geometry.get(doc, r.id) else { continue };
                if placed.members.iter().any(|m| selected.contains(m)) {
                    continue;
                }
                if r.at > now {
                    frame.pending.push((placed.rect, r.at - now));
                    continue;
                }
                let span = (r.end - r.at).as_secs_f32();
                let alpha = if span > 0.0 { ((r.end - now).as_secs_f32() / span).clamp(0.0, 1.0) } else { 0.0 };
                objects.entry(r.id).and_modify(|(_, a)| *a = a.max(alpha)).or_insert((placed.rect, alpha));
            }
        }
        for (id, mut labels) in pages {
            labels.sort();
            labels.dedup();
            if let Some(a) = doc.artboards.iter().find(|a| a.id == id) {
                frame.pages.push(Page { artboard: id, rect: a.rect, name: a.name.clone(), labels: labels.into_iter().map(|(_, l)| l).collect() });
            }
        }
        frame.objects = objects.into_iter().map(|(id, (rect, alpha))| Object { id, rect, alpha }).collect();
        frame.repaint_after = repaint_after(&frame, |_| true);
        frame
    }
}

/// When to draw again, counting only what `visible` (the part of the document on screen) shows:
/// at once while an outline is fading, else when the next object appears or the next artboard
/// outline expires; `None` when nothing visible changes (off-screen work sleeps).
pub fn repaint_after(frame: &Frame, visible: impl Fn(Rect) -> bool) -> Option<Duration> {
    if frame.objects.iter().any(|o| visible(o.rect)) {
        return Some(Duration::ZERO);
    }
    frame
        .pending
        .iter()
        .filter(|(r, _)| visible(*r))
        .map(|(_, d)| *d)
        .chain(frame.pages.iter().any(|p| visible(p.rect)).then_some(frame.expiry_after).flatten())
        .min()
}

/// The artboard a change happened on: `explicit` when the request named one that exists; else the
/// last artboard the change created or changed; else the artboard most covered by the objects it
/// created or changed; else `fallback` (the view's artboard), when it exists.
pub fn target_artboard(doc: &Document, change: &Change, explicit: Option<u32>, fallback: Option<u32>) -> Option<u32> {
    let exists = |id: u32| doc.artboards.iter().any(|a| a.id == id);
    if let Some(id) = explicit.filter(|id| exists(*id)) {
        return Some(id);
    }
    if let Some(id) = change.artboards_touched().into_iter().rev().find(|id| exists(*id)) {
        return Some(id);
    }
    let union = change
        .touched()
        .into_iter()
        .filter_map(|id| place(doc, id).map(|p| p.rect))
        .fold(None, |acc: Option<Rect>, r| Some(acc.map_or(r, |a| a.union(r))));
    if let Some(u) = union {
        let best =
            doc.artboards.iter().map(|a| (a.id, area(a.rect.intersect(u)))).filter(|(_, overlap)| *overlap > 0.0).max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((id, _)) = best {
            return Some(id);
        }
    }
    fallback.filter(|id| exists(*id))
}

fn area(r: Rect) -> f64 {
    (r.x1 - r.x0).max(0.0) * (r.y1 - r.y0).max(0.0)
}
