//! Varos: agents on the same document as the person at the keyboard (docs/varos/BACKLOG.md item 8).
//!
//! Any agent the user runs may read, edit and save through the control channel, with no pairing,
//! approval or OS credential prompt (owner decision 2026-10-08). This module is the desktop glue
//! around each control request ([`before`] / [`after`], hooked in `VectorcraftApp::drain_control`):
//!
//! - the request's actor (sent by the MCP proxy, `{session, profile, label}`) gets a key and a label
//!   ("Claude Code", "Claude Code 2");
//! - the undo steps the request records carry that key (`History::actor`), so the History panel can
//!   show which steps an agent made, and undo/redo keep it;
//! - what the request changed is diffed from the document before and after and logged by revision
//!   ([`varos_agents::Activity`]); while an agent is around, the person's own changes are logged too
//!   ([`observe`]), so `describe since` sees every revision;
//! - the on-canvas presence (orange artboard outline, objects appearing one by one) is derived from
//!   the same diff: the agent sends nothing for it and spends no tokens.
//!
//! The logic lives in the pure `varos-agents` crate; this file only reads and writes the app.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Stroke, StrokeKind};
use serde_json::Value;
use varos_agents::activity::Entry;
use varos_agents::presence::{Token, repaint_after, target_artboard};
use varos_agents::{Activity, ActorKey, Presence, Receipts, Registry, diff};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::Rect;

use super::theme::{
    AGENT, AGENT_LABEL_GAP, AGENT_LABEL_H, AGENT_LABEL_MIN_PAGE_W, AGENT_LABEL_PAD, AGENT_LABEL_TEXT, AGENT_OBJECT_STROKE, AGENT_PAGE_STROKE,
};
use crate::VectorcraftApp;
use crate::canvas::Xf;
use crate::control::ControlRequest;
use crate::theme::Tokens;

/// An agent session not heard from for this long is forgotten: its label number is free again.
const IDLE: Duration = Duration::from_secs(10 * 60);

/// The session and label of a request that names none (a raw control-channel client).
pub(crate) const UNNAMED_SESSION: &str = "control";

/// The agents' state, owned by the app.
#[derive(Default)]
pub(crate) struct Agents {
    pub registry: Registry,
    pub presence: Presence,
    pub activity: Activity,
    #[allow(dead_code)] // Varos: the edit verbs (BACKLOG item 8, step S4) use the receipts.
    pub receipts: Receipts,
    /// The last revision and document seen of each open document, by document uid.
    seen: HashMap<u64, (u64, Arc<Document>)>,
    /// Every agent's label as it was when it worked, by key: History rows keep naming a session
    /// the registry has forgotten. One short string per agent session this launch.
    labels: HashMap<u64, String>,
}

/// What [`before`] noted about a request, for [`after`].
pub(crate) struct Pending {
    actor: ActorKey,
    label: String,
    uid: u64,
    revision: u64,
    doc: Arc<Document>,
    token: Token,
}

/// The actor's `{session, profile, label}` strings; missing ones are empty.
fn actor_fields(actor: Option<&Value>) -> (String, String, String) {
    let field = |k: &str| actor.and_then(|a| a.get(k)).and_then(Value::as_str).unwrap_or("").to_string();
    let session = field("session");
    let session = if session.is_empty() { UNNAMED_SESSION.to_string() } else { session };
    (session, field("profile"), field("label"))
}

/// Log the changes made since the last look by anyone but an agent request (the person, a
/// timer, a background job), while agent sessions are registered. Cheap when nothing changed:
/// one revision comparison per open document.
pub(crate) fn observe(app: &mut VectorcraftApp) {
    let now = Instant::now();
    app.agents.registry.forget_idle(now, IDLE);
    let agents = &mut app.agents;
    if agents.registry.is_empty() {
        agents.seen.clear();
        return;
    }
    let mut open = Vec::with_capacity(app.session.documents().len());
    for st in app.session.documents() {
        open.push(st.uid);
        match agents.seen.get(&st.uid) {
            Some((rev, _)) if *rev == st.revision => {}
            Some((rev, doc)) => {
                let change = if Arc::ptr_eq(doc, &st.doc) { Default::default() } else { diff(doc, &st.doc) };
                let step = st.history.undo.last().map(|e| e.label.clone()).unwrap_or_default();
                agents.activity.record(st.uid, Entry { from: *rev, to: st.revision, actor: None, label: String::new(), step, change, at: now });
                agents.seen.insert(st.uid, (st.revision, st.doc.clone()));
            }
            None => {
                agents.seen.insert(st.uid, (st.revision, st.doc.clone()));
            }
        }
    }
    agents.seen.retain(|uid, _| open.contains(uid));
    agents.activity.retain(|uid| open.contains(&uid));
    agents.presence.retain(|uid| open.contains(&uid), now);
}

/// A control request is about to run: register its agent, mark the active document's history as
/// recording for it, and light up the artboard it works on.
pub(crate) fn before(app: &mut VectorcraftApp, req: &ControlRequest) -> Option<Pending> {
    let now = Instant::now();
    let (session, profile, label) = actor_fields(req.actor.as_ref());
    let actor = app.agents.registry.admit(&session, &profile, &label, now);
    observe(app);
    let artboard = view_artboard(app);
    let st = app.session.active_mut()?;
    st.history.actor = Some(actor.0);
    let (uid, revision, doc) = (st.uid, st.revision, st.doc.clone());
    let label = app.agents.registry.label(actor);
    app.agents.labels.insert(actor.0, label.clone());
    let profile = app.agents.registry.get(actor).map(|a| a.profile.clone()).unwrap_or_default();
    let token = app.agents.presence.begin(actor, &label, &profile, uid, artboard, now);
    Some(Pending { actor, label, uid, revision, doc, token })
}

/// The request ran (or failed): stop recording for its agent, log what it changed and reveal it.
pub(crate) fn after(app: &mut VectorcraftApp, pending: Option<Pending>) {
    let Some(p) = pending else { return };
    let now = Instant::now();
    let fallback = view_artboard(app);
    let Some(st) = app.session.document_mut(p.uid) else {
        app.agents.presence.abort(p.token);
        return;
    };
    st.history.actor = None;
    if st.revision == p.revision {
        app.agents.presence.abort(p.token);
        return;
    }
    let change = if Arc::ptr_eq(&p.doc, &st.doc) { Default::default() } else { diff(&p.doc, &st.doc) };
    let step = st.history.undo.last().filter(|e| e.actor == Some(p.actor.0)).map(|e| e.label.clone()).unwrap_or_default();
    let (revision, doc) = (st.revision, st.doc.clone());
    let artboard = target_artboard(&doc, &change, None, fallback);
    app.agents
        .activity
        .record(p.uid, Entry { from: p.revision, to: revision, actor: Some(p.actor), label: p.label, step, change: change.clone(), at: now });
    app.agents.seen.insert(p.uid, (revision, doc));
    app.agents.presence.finish(p.token, &change, artboard, now);
}

/// The id of the artboard the view shows, if any.
fn view_artboard(app: &VectorcraftApp) -> Option<u32> {
    let doc = &app.session.active()?.doc;
    let index = app.view().map_or(0, |v| v.artboard);
    doc.artboards.get(index).or(doc.artboards.first()).map(|a| a.id)
}

/// The agent labels of the History panel's rows, in its order: the "Open" row, the undo steps
/// oldest first, then the redo steps nearest first. `None` for the person's steps.
pub(crate) fn history_labels(app: &VectorcraftApp, history: &vectorcraft_engine::History) -> Vec<Option<String>> {
    let label = |e: &vectorcraft_engine::HistoryEntry| {
        e.actor.map(|key| app.agents.labels.get(&key).cloned().unwrap_or_else(|| app.agents.registry.label(ActorKey(key))))
    };
    std::iter::once(None).chain(history.undo.iter().map(label)).chain(history.redo.iter().rev().map(label)).collect()
}

/// The icon colour of a History row: orange for an agent's step.
pub(crate) fn history_icon(agent: Option<&Option<String>>, color: Color32) -> Color32 {
    if matches!(agent, Some(Some(_))) { AGENT } else { color }
}

/// An agent's name at the right end of its History row, in orange.
pub(crate) fn paint_history_agent(ui: &egui::Ui, row: egui::Rect, agent: Option<&Option<String>>) {
    if let Some(Some(label)) = agent {
        ui.painter().text(
            row.right_center() - egui::vec2(8.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            label,
            egui::FontId::proportional(AGENT_LABEL_TEXT),
            AGENT,
        );
    }
}

/// Draw the agents' presence over the canvas: the artboards they work on outlined with their
/// names above, and the objects they created or changed outlined while they fade. Nothing while
/// no agent is working. Asks for the next frame only while something visible changes.
pub(crate) fn paint(app: &mut VectorcraftApp, painter: &egui::Painter, xf: &Xf) {
    if !app.agents.presence.is_active() {
        return;
    }
    let Some(st) = app.session.active() else { return };
    let now = Instant::now();
    let selected: HashSet<NodeId> = st.selection.objects.iter().copied().collect();
    let frame = app.agents.presence.frame(st.uid, st.revision, &st.doc, &selected, now);
    let t = Tokens::get(painter.ctx());
    let clip = painter.clip_rect();
    let on_screen = |r: Rect| clip.intersects(xf.rect_to_screen(r).expand(AGENT_LABEL_H * 3.0));
    for page in &frame.pages {
        if !on_screen(page.rect) {
            continue;
        }
        painter.add(egui::Shape::closed_line(xf.quad(page.rect), Stroke::new(AGENT_PAGE_STROKE, AGENT)));
        let r = xf.rect_to_screen(page.rect);
        if r.width() < AGENT_LABEL_MIN_PAGE_W {
            continue;
        }
        // Name chips above the top-right corner, stacked upward; the artboard's own name stays
        // readable at the top left.
        let mut top = r.top() - AGENT_LABEL_GAP;
        for label in &page.labels {
            let galley = painter.layout_no_wrap(label.clone(), egui::FontId::proportional(AGENT_LABEL_TEXT), t.text);
            let size = egui::vec2(galley.size().x + AGENT_LABEL_PAD * 2.0, AGENT_LABEL_H);
            let chip = egui::Rect::from_min_size(egui::pos2(r.right() - size.x, top - AGENT_LABEL_H), size);
            painter.rect_filled(chip, CornerRadius::same(3), t.panel);
            painter.rect_stroke(chip, CornerRadius::same(3), Stroke::new(1.0, AGENT), StrokeKind::Inside);
            painter.galley(egui::pos2(chip.left() + AGENT_LABEL_PAD, chip.center().y - galley.size().y / 2.0), galley, t.text);
            top -= AGENT_LABEL_H + AGENT_LABEL_GAP;
        }
    }
    for object in &frame.objects {
        if on_screen(object.rect) {
            painter.add(egui::Shape::closed_line(xf.quad(object.rect), Stroke::new(AGENT_OBJECT_STROKE, AGENT.gamma_multiply(object.alpha))));
        }
    }
    if let Some(delay) = repaint_after(&frame, on_screen) {
        painter.ctx().request_repaint_after(delay);
    }
}

/// The mistake guards on the files an agent writes (`varos_agents::guard`), for requests that came
/// through Varos' always-on agents port (their actor is marked `guarded`). Never overwrite a file
/// (except saving the document onto its own), only under home or on an external drive, no hidden
/// folders, no new folders. `Some(refusal)` runs nothing; requests from upstream's `--control` port
/// and the person's own actions are never guarded.
pub(crate) fn guard(app: &VectorcraftApp, req: &ControlRequest) -> Option<crate::control::Outcome> {
    if req.actor.as_ref().and_then(|a| a.get("guarded")) != Some(&Value::Bool(true)) {
        return None;
    }
    let p = &req.params;
    let (command, params) = match req.method.as_str() {
        "engine.execute" | "ui.menu.invoke" | "command" => {
            (p.get("command").or_else(|| p.get("id")).and_then(Value::as_str)?, p.get("params").unwrap_or(&Value::Null))
        }
        method => (method, p),
    };
    if !writes_files(command) {
        return None;
    }
    let Some(places) = varos_agents::guard::Places::current() else {
        return Some(crate::control::err("agents can't write files here: the home folder is unknown"));
    };
    let own = app.session.active().and_then(|d| d.path.clone()).filter(|_| matches!(command, "file.save" | "app.save"));
    let checks = [
        params.get("path").and_then(Value::as_str).map(|path| varos_agents::guard::check_new_file(path, &places, own.as_deref())),
        params.get("folder").and_then(Value::as_str).map(|folder| varos_agents::guard::check_folder(folder, &places)),
    ];
    checks.into_iter().flatten().find_map(Result::err).map(crate::control::err)
}

/// Commands and methods that write a file at a `path` (or into a `folder`) they are given.
fn writes_files(command: &str) -> bool {
    matches!(command, "app.save" | "app.export" | "ui.screenshot" | "ui.render")
        || ["file.save", "file.export", "file.package", "document.export"].iter().any(|p| command.starts_with(p))
        || command.contains(".export")
        || command.contains("save")
        || command.contains("Save")
}
