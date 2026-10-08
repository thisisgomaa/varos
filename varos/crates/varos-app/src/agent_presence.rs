//! Host-owned, transient canvas feedback. No Bridge calls, wire fields, document edits or timers.
//! A logical client session spans many one-call sockets; inactivity is its only existing end signal.
use crate::{app_command::SessionId, workspace::Workspace};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashSet},
    time::{Duration, Instant},
};
use varos_bridge::{
    dto::{Edit, Operation},
    ipc::Pending,
    Reply, Request,
};
use varos_core::{
    editor::Editor,
    model::{Document, NodeKind},
};

const HOLD: Duration = Duration::from_secs(4);
const STAGGER: Duration = Duration::from_millis(25);
const REVEAL_CAP: Duration = Duration::from_millis(1500);
const FADE: Duration = Duration::from_millis(900);
const FINISH: Duration = Duration::from_millis(120);

#[derive(Clone, Debug)]
struct Reveal {
    id: String,
    board: SessionId,
    at: Instant,
    end: Instant,
}
#[derive(Clone, Debug)]
struct Session {
    label: String,
    profile_id: String,
    board: SessionId,
    last_artboard: Option<u32>,
    in_flight: bool,
    last_edit: Instant,
    reveal: Vec<Reveal>,
}
#[derive(Default)]
pub(crate) struct Presence {
    sessions: BTreeMap<String, Session>,
    geometry: RefCell<Geometry>,
}
#[derive(Default, Debug)]
pub(crate) struct Frame {
    pub pages: Vec<Page>,
    pub objects: Vec<Object>,
    pub repaint_after: Option<Duration>,
    pub expiry_after: Option<Duration>,
    pub pending: Vec<([f32; 4], Duration)>,
}
#[derive(Debug)]
pub(crate) struct Page {
    pub bounds: [f32; 4],
    pub labels: Vec<String>,
    pub name: String,
}
#[derive(Debug)]
pub(crate) struct Object {
    pub bounds: [f32; 4],
    pub alpha: f32,
}
/// Revision-keyed world bounds/visibility. Core visibility can inspect curved page membership;
/// do it once per revision, never at the display rate. Human previews invalidate this cache.
#[derive(Default)]
struct Geometry {
    key: Option<(SessionId, u64)>,
    paths: BTreeMap<u32, usize>,
    bounds: BTreeMap<u32, Option<[f32; 4]>>,
    objects: BTreeMap<String, (Vec<u32>, Option<[f32; 4]>)>,
}
impl Geometry {
    fn prepare(&mut self, board: SessionId, ed: &Editor) {
        if self.key != Some((board, ed.rev)) || ed.transaction_open() {
            self.key = (!ed.transaction_open()).then_some((board, ed.rev));
            self.paths = ed.doc.paths.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
            self.bounds.clear();
            self.objects.clear();
        }
    }
    fn object(&mut self, ed: &Editor, id: &str) -> &(Vec<u32>, Option<[f32; 4]>) {
        if !self.objects.contains_key(id) {
            let pids = object_paths(&ed.doc, id);
            let mut union: Option<[f32; 4]> = None;
            for pid in &pids {
                let b = self.bounds.entry(*pid).or_insert_with(|| {
                    let &i = self.paths.get(pid)?;
                    if ed.doc.paths[i].anchors.is_empty() || ed.doc.eff_hidden(*pid) {
                        return None;
                    }
                    let (x0, y0, x1, y1) = ed.doc.bbox(i);
                    Some([x0, y0, x1, y1])
                });
                if let Some(b) = b {
                    union = Some(match union {
                        None => *b,
                        Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])],
                    });
                }
            }
            self.objects.insert(id.into(), (pids, union.filter(|b| b.iter().all(|v| v.is_finite()))));
        }
        &self.objects[id]
    }
}
thread_local! { static PRESENCE: RefCell<Presence> = RefCell::new(Presence::default()); }

pub(crate) fn sanitize_label(label: &str) -> String {
    let label: String = label
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '\u{200b}'..='\u{200f}' | '\u{061c}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'))
        .take(24)
        .collect();
    let label = label.trim();
    if label.is_empty() {
        "Agent".into()
    } else {
        label.into()
    }
}
fn stagger(n: usize) -> Duration {
    if n == 0 {
        Duration::ZERO
    } else {
        STAGGER.min(REVEAL_CAP / n as u32)
    }
}
impl Presence {
    #[allow(clippy::too_many_arguments)]
    fn begin(
        &mut self,
        client: &str,
        label: &str,
        profile_id: &str,
        board: SessionId,
        artboard: Option<u32>,
        now: Instant,
    ) {
        let mut reveal = self.sessions.remove(client).map_or_else(Vec::new, |s| s.reveal);
        // New work collapses the old stagger and gives it a short final fade, without extending it.
        for r in &mut reveal {
            r.at = r.at.min(now);
            r.end = r.end.min(now + FINISH);
        }
        reveal.retain(|r| r.end > now);
        self.sessions.insert(
            client.into(),
            Session {
                label: sanitize_label(label),
                profile_id: profile_id.into(),
                board,
                last_artboard: artboard,
                in_flight: true,
                last_edit: now,
                reveal,
            },
        );
    }
    fn done(
        &mut self,
        client: &str,
        ids: Vec<String>,
        removed: &BTreeSet<String>,
        artboard: Option<u32>,
        now: Instant,
    ) {
        let Some(s) = self.sessions.get_mut(client) else { return };
        s.in_flight = false;
        s.last_edit = now;
        s.last_artboard = artboard;
        let board = s.board;
        // Deleted objects never paint, including highlights belonging to another agent.
        for session in self.sessions.values_mut() {
            for r in &mut session.reveal {
                r.at = r.at.min(now);
                r.end = r.end.min(now + FINISH);
            }
            session.reveal.retain(|r| r.board != board || !removed.contains(&r.id));
        }
        let s = self.sessions.get_mut(client).expect("session");
        let step = stagger(ids.len());
        let incoming: HashSet<_> = ids.iter().collect();
        s.reveal.retain(|r| r.board != s.board || !incoming.contains(&r.id));
        for (i, id) in ids.into_iter().enumerate() {
            let at = now + step * i as u32;
            // One outline per object even if touched repeatedly in this or overlapping batches.
            s.reveal.push(Reveal { id, board: s.board, at, end: at + FADE });
        }
    }
    fn retain(&mut self, ws: &Workspace, now: Instant) {
        self.sessions.retain(|_, s| ws.get(s.board).is_some() && (s.in_flight || now < s.last_edit + HOLD));
        for s in self.sessions.values_mut() {
            s.reveal.retain(|r| r.end > now && ws.get(r.board).is_some());
        }
    }
    fn frame(&self, board: SessionId, ed: &Editor, now: Instant) -> Frame {
        let mut frame = Frame::default();
        if self.sessions.is_empty() {
            return frame;
        }
        let mut selected = ed.selected_pids();
        selected.extend(ed.active);
        let mut pages: BTreeMap<u32, Vec<(&str, &str)>> = BTreeMap::new();
        let mut objects: BTreeMap<&str, ([f32; 4], f32)> = BTreeMap::new();
        let mut geometry = self.geometry.borrow_mut();
        geometry.prepare(board, ed);
        for s in self.sessions.values().filter(|s| s.in_flight || now < s.last_edit + HOLD) {
            if s.board == board {
                if let Some(id) = s.last_artboard.filter(|id| ed.doc.artboards.iter().any(|a| a.id == *id && !a.hidden))
                {
                    pages.entry(id).or_default().push((&s.profile_id, &s.label));
                    if !s.in_flight {
                        let delay = (s.last_edit + HOLD).saturating_duration_since(now);
                        frame.expiry_after = Some(frame.expiry_after.map_or(delay, |d| d.min(delay)));
                    }
                }
            }
            for r in s.reveal.iter().filter(|r| r.board == board && r.end > now) {
                let (pids, bounds) = geometry.object(ed, &r.id);
                let Some(bounds) = *bounds else { continue };
                if pids.iter().any(|p| selected.contains(p)) {
                    continue;
                }
                if r.at > now {
                    let delay = r.at - now;
                    frame.pending.push((bounds, delay));
                    continue;
                }
                let alpha = (r.end - now).as_secs_f32() / (r.end - r.at).as_secs_f32();
                objects.entry(&r.id).and_modify(|(_, a)| *a = a.max(alpha)).or_insert((bounds, alpha));
                // Zero delay is bounded by the existing display-refresh pacing in main.rs.
            }
        }
        for (id, mut labels) in pages {
            labels.sort(); // stable stacking by identity, independent of request arrival order
            let a = ed.doc.artboards.iter().find(|a| a.id == id).expect("visible page");
            frame.pages.push(Page {
                bounds: [a.x, a.y, a.x + a.w, a.y + a.h],
                name: a.name.clone(),
                labels: labels.into_iter().map(|(_, l)| l.to_owned()).collect(),
            });
        }
        frame.objects.extend(objects.into_values().map(|(bounds, alpha)| Object { bounds, alpha }));
        frame.repaint_after = repaint_after(&frame, |_| true);
        frame
    }
}
/// Pure visibility-gated deadline: only visible fading outlines drive animation frames.
/// Static pages and future reveals schedule one-shot deadlines; idle/offscreen work sleeps.
pub(crate) fn repaint_after(frame: &Frame, visible: impl Fn([f32; 4]) -> bool) -> Option<Duration> {
    if frame.objects.iter().any(|o| visible(o.bounds)) {
        return Some(Duration::ZERO);
    }
    frame
        .pending
        .iter()
        .filter(|(b, _)| visible(*b))
        .map(|(_, d)| *d)
        .chain(frame.pages.iter().any(|p| visible(p.bounds)).then_some(frame.expiry_after).flatten())
        .min()
}
fn object_paths(doc: &Document, id: &str) -> Vec<u32> {
    if let Some(id) = id.strip_prefix("path:").and_then(|s| s.parse().ok()) {
        vec![id]
    } else if let Some(id) = id.strip_prefix("node:").and_then(|s| s.parse().ok()) {
        doc.node(id).filter(|n| n.kind != NodeKind::Layer).map_or_else(Vec::new, |_| doc.node_paths(id))
    } else {
        vec![]
    }
}
fn page_id(id: &str) -> Option<u32> {
    id.strip_prefix("artboard:")?.parse().ok()
}
fn target_page(edit: &Edit, doc: &Document, locals: &serde_json::Value, before: Option<&Document>) -> Option<u32> {
    let ops = varos_bridge::expanded_ops(edit);
    let old: BTreeSet<_> = before.into_iter().flat_map(|d| d.artboards.iter().map(|a| a.id)).collect();
    let mut created: BTreeSet<_> =
        before.into_iter().flat_map(|_| doc.artboards.iter().filter(|a| !old.contains(&a.id)).map(|a| a.id)).collect();
    let mut target = None;
    for op in &ops {
        let id = match op {
            Operation::AddArtboard { local, .. } | Operation::DuplicateArtboard { local, .. } => {
                let id =
                    local.as_ref().and_then(|l| locals[l].as_str()).and_then(page_id).or_else(|| created.pop_first());
                if let Some(id) = id {
                    created.remove(&id);
                }
                if matches!(op, Operation::DuplicateArtboard { .. }) && id.is_none() {
                    op.artboard().and_then(page_id)
                } else {
                    id
                }
            }
            _ => {
                let id = match op {
                    Operation::Align { target, .. } => Some(target.as_str()),
                    _ => op.artboard(),
                };
                id.and_then(|id| {
                    let id = locals[id].as_str().unwrap_or(id);
                    page_id(id).or_else(|| {
                        id.strip_prefix('a')?
                            .split('@')
                            .next()?
                            .parse::<usize>()
                            .ok()
                            .and_then(|i| doc.artboards.get(i).map(|a| a.id))
                    })
                })
            }
        };
        if let Some(id) = id.filter(|id| doc.artboards.iter().any(|a| a.id == *id)) {
            target = Some(id);
        }
    }
    target.or_else(|| doc.active_artboard().map(|a| a.id))
}

pub(crate) struct Accepted {
    client: String,
    board: SessionId,
    rev: u64,
    previous: Option<Session>,
}
/// Runs at the existing FIFO dispatch boundary. Failures restore the prior transient state.
pub(crate) fn accept(request: &Pending, ws: &Workspace, now: Instant) -> Option<Accepted> {
    let Request::Edit(edit) = &request.request else { return None };
    let board = SessionId(edit.board.strip_prefix('b')?.parse().ok()?);
    let s = ws.get(board)?;
    if s.editor.rev != edit.expected_rev {
        return None;
    } // includes idempotent receipt replay
    let client = request.context.client.clone();
    let (profile, label) = request
        .file_audit
        .as_ref()
        .map(|(_, e)| (e.agent.as_str(), e.label.as_str()))
        .unwrap_or((client.as_str(), "Agent"));
    let artboard = target_page(edit, &s.editor.doc, &serde_json::Value::Null, None);
    let previous = PRESENCE.with(|p| {
        let mut p = p.borrow_mut();
        let previous = p.sessions.get(&client).cloned();
        p.begin(&client, label, profile, board, artboard, now);
        previous
    });
    Some(Accepted { client, board, rev: s.editor.rev, previous })
}
pub(crate) fn complete(accepted: Option<Accepted>, reply: &Reply, request: &Request, ws: &Workspace, now: Instant) {
    let Some(a) = accepted else { return };
    let after = ws.get(a.board).map(|s| &s.editor);
    PRESENCE.with(|p| {
        let mut p = p.borrow_mut();
        let Some(ed) = after.filter(|ed| reply.ok && ed.rev > a.rev && reply.undo_steps > 0) else {
            p.sessions.remove(&a.client);
            if let Some(previous) = a.previous {
                p.sessions.insert(a.client, previous);
            }
            return;
        };
        let Request::Edit(edit) = request else { return };
        let Some(before) = ed.history_preview(false) else { return };
        let receipt = reply.result.as_ref().unwrap_or(&serde_json::Value::Null);
        let (ids, removed) = receipt_order(edit, receipt, before, &ed.doc);
        let page = target_page(edit, &ed.doc, &receipt["locals"], Some(before));
        p.done(&a.client, ids, &removed, page, now);
    });
}
/// Receipts are sorted by public id, not op order. Reorder using the already received operations.
/// Oversized receipts omit ids; the host's two documents provide the same information locally.
fn receipt_order(
    edit: &Edit,
    receipt: &serde_json::Value,
    before: &Document,
    after: &Document,
) -> (Vec<String>, BTreeSet<String>) {
    let ops = varos_bridge::expanded_ops(edit);
    let mut touched: BTreeSet<String> = ["created", "changed"]
        .into_iter()
        .flat_map(|k| receipt[k].as_array().into_iter().flatten())
        .filter_map(|o| o["id"].as_str().map(str::to_owned))
        .collect();
    let mut removed: BTreeSet<String> =
        receipt["removed"].as_array().into_iter().flatten().filter_map(|o| o.as_str().map(str::to_owned)).collect();
    if receipt["more"] == true {
        // Only on a capped receipt; geometry equality also detects edits keeping the same bounds.
        let old = varos_core::bridge::elements(before, true);
        let new = varos_core::bridge::elements(after, true);
        touched.extend(new.iter().filter(|(id, v)| old.get(*id) != Some(*v)).map(|(id, _)| id.clone()));
        removed.extend(old.keys().filter(|id| !new.contains_key(*id)).cloned());
    }
    // Layers are structural; unchanged group records only report child-derived bounds.
    // Keep explicitly changed/created groups, but suppress a bounds-only ancestor of a target.
    let candidates = touched.clone();
    touched.retain(|id| {
        let Some(nid) = id.strip_prefix("node:").and_then(|s| s.parse().ok()) else { return true };
        let Some(node) = after.node(nid) else { return false };
        node.kind != NodeKind::Layer
            && !(node.kind == NodeKind::Group
                && before.node(nid) == Some(node)
                && after.node_paths(nid).iter().any(|pid| candidates.contains(&format!("path:{pid}"))))
    });
    let creations = creations_by_operation(edit, receipt, before, after);
    let owners: BTreeMap<_, _> =
        creations.iter().enumerate().flat_map(|(i, ids)| ids.iter().map(move |id| (id.as_str(), i))).collect();
    let mut ordered = Vec::new();
    let mut add = |id: String, op_index: usize| {
        // A changed ancestor belongs beside the operation that touched its subtree, not at the tail.
        let pids = object_paths(after, &id);
        let mut related = vec![id];
        for pid in pids {
            related.push(format!("path:{pid}"));
            let mut parent = after.node_of_path(pid).and_then(|n| after.node(n)).and_then(|n| n.parent);
            while let Some(id) = parent {
                related.push(format!("node:{id}"));
                parent = after.node(id).and_then(|n| n.parent);
            }
        }
        for id in related {
            if owners.get(id.as_str()).is_none_or(|owner| *owner <= op_index)
                && touched.remove(&id)
                && !removed.contains(&id)
            {
                ordered.push(id);
            }
        }
    };
    for (op_index, (op, creations)) in ops.iter().zip(&creations).enumerate() {
        for id in creations {
            add(id.clone(), op_index);
        }
        for id in op.ids() {
            let id = receipt["locals"][id].as_str().unwrap_or(id);
            add(id.into(), op_index);
        }
    }
    ordered.extend(touched.into_iter().filter(|id| !removed.contains(id)));
    (ordered, removed)
}
/// Allocation blocks are recoverable without replaying any edits: the core's one monotonic id
/// arena ends at Document::ids. Walk backwards, peeling fixed-size creation blocks; a duplicate
/// owns the entire variable-size block between its page id and the next creation. This also
/// handles empty duplicates and creations deleted later in the same atomic batch.
fn creations_by_operation(
    edit: &Edit,
    receipt: &serde_json::Value,
    before: &Document,
    after: &Document,
) -> Vec<Vec<String>> {
    let ops = varos_bridge::expanded_ops(edit);
    let old_pages: BTreeSet<_> = before.artboards.iter().map(|a| a.id).collect();
    let mut pages: BTreeSet<_> = after.artboards.iter().filter(|a| !old_pages.contains(&a.id)).map(|a| a.id).collect();
    pages.extend(
        ops.iter()
            .filter_map(Operation::artboard)
            .filter_map(|id| page_id(receipt["locals"][id].as_str().unwrap_or(id)))
            .filter(|id| !old_pages.contains(id)),
    );
    let paths: BTreeSet<_> = after.paths.iter().map(|p| p.id).collect();
    let groups: BTreeSet<_> = after.nodes.iter().filter(|n| n.kind == NodeKind::Group).map(|n| n.id).collect();
    let mut cursor = after.ids;
    let mut out = vec![vec![]; ops.len()];
    for (i, op) in ops.iter().enumerate().rev() {
        match op {
            Operation::AddShape { local, bounds, radius, .. } => {
                let count = shape_anchor_count(*bounds, *radius);
                let id = local
                    .as_ref()
                    .and_then(|l| receipt["locals"][l].as_str())
                    .and_then(|s| s.strip_prefix("path:")?.parse().ok())
                    .unwrap_or_else(|| cursor.saturating_sub(count + 1));
                out[i].push(format!("path:{id}"));
                cursor = id.saturating_sub(1);
            }
            Operation::AddPath { local, anchors, .. } => {
                let id = local
                    .as_ref()
                    .and_then(|l| receipt["locals"][l].as_str())
                    .and_then(|s| s.strip_prefix("path:")?.parse().ok())
                    .unwrap_or_else(|| cursor.saturating_sub(anchors.len() as u32 + 1));
                out[i].push(format!("path:{id}"));
                cursor = id.saturating_sub(1);
            }
            Operation::Group { local, .. } => {
                let id = local
                    .as_ref()
                    .and_then(|l| receipt["locals"][l].as_str())
                    .and_then(|s| s.strip_prefix("node:")?.parse().ok())
                    .unwrap_or(cursor);
                out[i].push(format!("node:{id}"));
                cursor = id.saturating_sub(1);
            }
            Operation::AddArtboard { local, .. } | Operation::DuplicateArtboard { local, .. } => {
                let page = local.as_ref().and_then(|l| receipt["locals"][l].as_str()).and_then(page_id).or_else(|| {
                    if matches!(op, Operation::AddArtboard { .. }) {
                        Some(cursor)
                    } else {
                        pages.range(..=cursor).next_back().copied()
                    }
                });
                if let Some(page) = page {
                    pages.remove(&page);
                    if matches!(op, Operation::DuplicateArtboard { with_art: true, .. }) {
                        out[i].extend(paths.range(page..=cursor).map(|id| format!("path:{id}")));
                        out[i].extend(groups.range(page..=cursor).map(|id| format!("node:{id}")));
                    }
                    cursor = page.saturating_sub(1);
                }
            }
            _ => {}
        }
    }
    out
}
/// Bridge rounded rectangles merge consecutive coincident corner endpoints. Count that fixed
/// allocation without generating geometry or running another edit; ordinary rect/ellipse = 4.
fn shape_anchor_count([x, y, w, h]: [f32; 4], radius: Option<f32>) -> u32 {
    let Some(r) = radius.filter(|r| *r != 0.0) else { return 4 };
    let r = r.min(w.min(h) / 2.0);
    let points = [
        [x + r, y],
        [x + w - r, y],
        [x + w, y + r],
        [x + w, y + h - r],
        [x + w - r, y + h],
        [x + r, y + h],
        [x, y + h - r],
        [x, y + r],
    ];
    1 + points.windows(2).filter(|p| p[0] != p[1]).count() as u32
}
pub(crate) fn frame(board: Option<SessionId>, ed: &Editor, now: Instant) -> Frame {
    board.map_or_else(Frame::default, |board| PRESENCE.with(|p| p.borrow().frame(board, ed, now)))
}
pub(crate) fn retain(ws: &Workspace, now: Instant) {
    PRESENCE.with(|p| p.borrow_mut().retain(ws, now));
}
pub(crate) fn clear() {
    PRESENCE.with(|p| *p.borrow_mut() = Presence::default());
}

#[cfg(test)]
mod tests;
