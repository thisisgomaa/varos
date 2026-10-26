//! Varos: agents on the same document as the person at the keyboard (docs/varos/BACKLOG.md item 8).
//!
//! Any agent the user runs may read, edit and save through the control channel, with no pairing,
//! approval or OS credential prompt (owner decision 2026-10-08). This module is the desktop glue
//! around each control request ([`before`] / [`after`], hooked in `VectorcraftApp::drain_control`):
//!
//! - the request's actor (sent by the MCP proxy, `{session, profile, label}`) gets a key and a label
//!   ("Claude Code", "Claude Code 2");
//! - the undo steps the request records carry that key (`Session::set_actor`), in whichever
//!   document it edits (one it opens or makes included), so the History panel can show which steps
//!   an agent made, and undo/redo keep it;
//! - what the request changed is diffed from the document before and after and logged by revision
//!   ([`varos_agents::Activity`]); while an agent is around, the person's own changes are logged too
//!   ([`observe`]), so `describe since` sees every revision;
//! - the on-canvas presence (orange artboard outline, objects appearing one by one) is derived from
//!   the same diff: the agent sends nothing for it and spends no tokens;
//! - requests that came through Varos' agents port are held to the mistake guards on the files
//!   they write ([`guard`] on the parameters, then [`Pending`]'s write guard at every file the
//!   request really writes).
//!
//! The logic lives in the pure `varos-agents` crate; this file only reads and writes the app.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Stroke, StrokeKind};
use serde_json::Value;
use varos_agents::activity::Entry;
use varos_agents::guard::Places;
use varos_agents::presence::{Token, repaint_after, target_artboard};
use varos_agents::{Activity, ActorKey, Change, Presence, Receipts, Registry, diff};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_engine::file_access;
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
    /// Where the file guards draw their lines; `None`: the user's home ([`Places::current`]).
    /// Tests put a folder of their own here.
    pub places: Option<Places>,
    /// The boards' content revisions, which agents name to save and export
    /// ([`super::boards`]).
    pub revisions: varos_agents::boards::Revisions,
}

/// What [`before`] noted about a request, for [`after`].
pub(crate) struct Pending {
    actor: ActorKey,
    label: String,
    profile: String,
    /// Every open document's uid, revision and content when the request began.
    docs: Vec<(u64, u64, Arc<Document>)>,
    /// The presence begun on the active document, with its uid.
    token: Option<(u64, Token)>,
    /// The write guard in force while a guarded request runs, and what it replaced.
    writes: Option<file_access::WriteGuardScope>,
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

/// An agent's connection closed ([`super::AGENT_LEFT`]): its session's label number is free for
/// the next agent with its label (owner: "Claude Code", not "Claude Code 7").
pub(crate) fn left(app: &mut VectorcraftApp, req: &ControlRequest) {
    let (session, ..) = actor_fields(req.actor.as_ref());
    app.agents.registry.depart(&session);
}

/// A control request is about to run: register its agent, record the steps of every document
/// for it, light up the artboard it works on, and put the write guard in force for a guarded one.
pub(crate) fn before(app: &mut VectorcraftApp, req: &ControlRequest) -> Option<Pending> {
    let now = Instant::now();
    let (session, profile, label) = actor_fields(req.actor.as_ref());
    let actor = app.agents.registry.admit(&session, &profile, &label, now);
    observe(app);
    let artboard = view_artboard(app);
    app.session.set_actor(Some(actor.0));
    let label = app.agents.registry.label(actor);
    app.agents.labels.insert(actor.0, label.clone());
    let profile = app.agents.registry.get(actor).map(|a| a.profile.clone()).unwrap_or_default();
    let docs = app.session.documents().iter().map(|st| (st.uid, st.revision, st.doc.clone())).collect();
    let token = app.session.active().map(|st| st.uid).map(|uid| (uid, app.agents.presence.begin(actor, &label, &profile, uid, artboard, now)));
    let writes = is_guarded(req).then(|| {
        // Only an actual save of the active document may write over its own file: never a
        // template, an export or a print that lands on the same name.
        let saves = command_of(req).is_some_and(|(command, _)| SAVES_OWN_FILE.contains(&command));
        let own = app.session.active().and_then(|d| d.path.clone()).filter(|_| saves);
        file_access::begin_write_guard(write_guard(places(app), own))
    });
    Some(Pending { actor, label, profile, docs, token, writes })
}

/// Run `f` (writing a file a guarded request asked for, later than the request: a window
/// capture) under that request's write guard ([`file_access::write_guard`] while it ran).
pub(crate) fn with_request_guard<T>(guard: Option<file_access::WriteGuard>, f: impl FnOnce() -> T) -> T {
    struct End(Option<file_access::WriteGuardScope>);
    impl Drop for End {
        fn drop(&mut self) {
            if let Some(scope) = self.0.take() {
                file_access::end_write_guard(scope);
            }
        }
    }
    let _end = End(guard.map(file_access::begin_write_guard));
    f()
}

/// The check at every file a guarded request writes (`varos_agents::guard::Writes`).
fn write_guard(places: Option<Places>, own_file: Option<String>) -> file_access::WriteGuard {
    match places {
        Some(places) => {
            let writes = varos_agents::guard::Writes::new(places, own_file);
            file_access::WriteGuard::new(move |path| writes.check(path))
        }
        None => file_access::WriteGuard::new(|_| Err(NO_HOME.to_string())),
    }
}

const NO_HOME: &str = "agents can't write files here: the home folder is unknown";

/// Is a request of an agent through Varos' agents port running on this thread (its write guard in
/// force: [`before`])? Work it starts runs at once rather than in the background, so what fails is
/// its reply, never a dialog that interrupts the person (owner 2026-10-19).
pub(crate) fn guarded_request_running() -> bool {
    file_access::write_guard().is_some()
}

/// The request ran (or failed): end its write guard, stop recording for its agent, log what it
/// changed in each document, and reveal it on the canvas.
pub(crate) fn after(app: &mut VectorcraftApp, pending: Option<Pending>) {
    let Some(mut p) = pending else { return };
    if let Some(writes) = p.writes.take() {
        file_access::end_write_guard(writes);
    }
    app.session.set_actor(None);
    let now = Instant::now();
    let active = app.session.active().map(|st| st.uid);
    // What changed in every document open now: those the request began with, and those it opened
    // or made (logged from an empty document).
    let mut changes: Vec<(u64, Change)> = vec![];
    for st in app.session.documents() {
        let (from, before) = match p.docs.iter().find(|(uid, ..)| *uid == st.uid) {
            Some((_, revision, _)) if *revision == st.revision => continue,
            Some((_, revision, doc)) => (*revision, doc.clone()),
            None => (0, empty_document()),
        };
        let change = if Arc::ptr_eq(&before, &st.doc) { Change::default() } else { diff(&before, &st.doc) };
        let step = st.history.undo.last().filter(|e| e.actor == Some(p.actor.0)).map(|e| e.label.clone()).unwrap_or_default();
        let entry = Entry { from, to: st.revision, actor: Some(p.actor), label: p.label.clone(), step, change: change.clone(), at: now };
        app.agents.activity.record(st.uid, entry);
        app.agents.seen.insert(st.uid, (st.revision, st.doc.clone()));
        changes.push((st.uid, change));
    }
    // The canvas shows one document's work: the one now in front if it changed, else the one the
    // request began on, else any it changed.
    let began_on = p.token.as_ref().map(|(uid, _)| *uid);
    let shown = [active, began_on].into_iter().flatten().find(|uid| changes.iter().any(|(u, _)| u == uid)).or(changes.first().map(|(u, _)| *u));
    let Some(shown) = shown else {
        if let Some((_, token)) = p.token {
            app.agents.presence.abort(token);
        }
        return;
    };
    let Some(change) = changes.into_iter().find_map(|(uid, change)| (uid == shown).then_some(change)) else { return };
    let fallback = if active == Some(shown) { view_artboard(app) } else { None };
    let Some(doc) = app.session.documents().iter().find(|st| st.uid == shown).map(|st| st.doc.clone()) else { return };
    let artboard = target_artboard(&doc, &change, None, fallback);
    let token = match p.token {
        Some((uid, token)) if uid == shown => token,
        other => {
            if let Some((_, token)) = other {
                app.agents.presence.abort(token);
            }
            app.agents.presence.begin(p.actor, &p.label, &p.profile, shown, artboard, now)
        }
    };
    app.agents.presence.finish(token, &change, artboard, now);
}

/// A document with nothing in it: what a document the request opened or made is diffed from.
fn empty_document() -> Arc<Document> {
    let mut d = Document::new(1.0, 1.0);
    d.layers.clear();
    d.artboards.clear();
    Arc::new(d)
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

/// Did the request come through Varos' agents port (its transport marks its actor `guarded`)?
pub(crate) fn is_guarded(req: &ControlRequest) -> bool {
    req.actor.as_ref().and_then(|a| a.get("guarded")) == Some(&Value::Bool(true))
}

/// The mistake guards on the files an agent writes (`varos_agents::guard`), on the request's
/// parameters before anything runs, for requests that came through Varos' agents port. Never
/// overwrite a file (except saving the document onto its own), only under home or on an external
/// drive, no hidden folders, no new folder trees; a set of files only into an empty folder. The
/// steps of `command.batch` and `action.batch` are checked as well. `Some(refusal)` runs nothing;
/// requests from upstream's `--control` port and the person's own actions are never guarded. What
/// the parameters can't tell (names a command makes up, a replaced extension) is checked again at
/// every file the request writes ([`before`]'s write guard).
pub(crate) fn guard(app: &VectorcraftApp, req: &ControlRequest) -> Option<crate::control::Outcome> {
    if !is_guarded(req) {
        return None;
    }
    let (command, params) = command_of(req)?;
    let own = app.session.active().and_then(|d| d.path.clone());
    check_command(command, params, places(app).as_ref(), own.as_deref(), 0).err().map(crate::control::err)
}

/// The command a request runs and its params (a control method stands for itself).
pub(crate) fn command_of(req: &ControlRequest) -> Option<(&str, &Value)> {
    let p = &req.params;
    Some(match req.method.as_str() {
        "engine.execute" | "ui.menu.invoke" | "command" => {
            (p.get("command").or_else(|| p.get("id")).and_then(Value::as_str)?, p.get("params").unwrap_or(&Value::Null))
        }
        method => (method, p),
    })
}

/// Where the guards draw their lines.
fn places(app: &VectorcraftApp) -> Option<Places> {
    app.agents.places.clone().or_else(Places::current)
}

/// Parameter keys that name a file or folder. A command that takes one is guarded as one that
/// writes there, unless [`READS`] says it only reads.
pub(crate) const PATH_KEYS: &[&str] =
    &["path", "paths", "file", "files", "folder", "folders", "destinationFolder", "linksFolder", "userFolder", "location", "dir", "directory"];

/// Commands that take a [`PATH_KEYS`] parameter but write nothing at it: they read the file or
/// folder (open, place, import, relink, load, list, find), or the key names something that isn't a
/// file written as given (a recovery file in the app's own folder, a node, a setting stored in the
/// document). Every other command taking one is guarded, also one upstream adds later (the
/// registry is read, and a test lists any command that is in neither group).
pub(crate) const READS: &[&str] = &[
    "color.loadProfile",
    "dataset.import",
    "document.dxfInfo",
    "document.open",
    "document.pdfInfo",
    "file.list",
    "file.newFromTemplate",
    "file.place",
    "file.place.info",
    "file.place.queue",
    "file.recovery.discard",
    "file.recovery.restore",
    "flattener.presets.import",
    "graphicStyle.loadLibrary",
    "library.import",
    "links.findFiles",
    "links.relink",
    "pdf.preset.import",
    "perspective.presets.import",
    "plugin.install",
    "plugin.reload",
    "print.presets.import",
    "script.check",
    "shortcutSet.read",
    "svgInteractivity.addFile",
    "svgInteractivity.removeFile",
    "swatch.library.load",
    "text.addFontFiles",
    "text.createInPath",
    "text.findFontFiles",
];

/// Control methods and UI commands outside the engine's command registry that write a file at
/// their `path`: File › Save (`file.save`, which the registry doesn't hold: the app saves) and the
/// app's own export commands are refused up front for an existing file, like the registry's.
pub(crate) const UI_WRITERS: &[&str] = &[
    "app.save",
    "app.export",
    "ui.render",
    "ui.screenshot",
    "file.save",
    "file.exportAs",
    "file.export.svg",
    "file.export.png",
    "file.export.pdf",
    "file.exportForScreens",
    "file.saveForWeb",
];

/// Commands that write over the document's own file when it is the path they are given.
pub(crate) const SAVES_OWN_FILE: &[&str] = &["app.save", "document.save", "file.save"];

/// Commands that write a set of files with names they make up into the `folder` they are given.
const WRITES_A_SET: &[&str] = &["assets.export", "document.exportForScreens", "file.exportForScreens"];

/// Commands that give the path they are given an extension of their own.
const RENAMES_ITS_FILE: &[&str] = &["file.saveAsTemplate"];

/// Does `command` (a command id, or a control method) write files at a path it is given?
pub(crate) fn writes_files(command: &str) -> bool {
    if UI_WRITERS.contains(&command) {
        return true;
    }
    if READS.contains(&command) {
        return false;
    }
    vectorcraft_engine::find_command(command).is_some_and(|spec| param_keys(spec.params, 0).iter().any(|k| PATH_KEYS.contains(k)))
}

/// The keys a params description names, those of the commands its `…as <command>` and `…of
/// <command>` entries name included.
pub(crate) fn param_keys(doc: &'static str, depth: usize) -> Vec<&'static str> {
    let Ok(parsed) = vectorcraft_engine::cmd::params::doc_keys(doc) else { return vec![] };
    let mut keys = parsed.keys;
    if depth < 4 {
        for more in parsed.more {
            let words: Vec<&str> = more.split_whitespace().collect();
            for pair in words.windows(2) {
                if matches!(pair[0], "as" | "of")
                    && let Some(spec) = vectorcraft_engine::find_command(pair[1].trim_matches(|c: char| !c.is_ascii_alphanumeric()))
                {
                    keys.extend(param_keys(spec.params, depth + 1));
                }
            }
        }
    }
    keys
}

/// Steps nested deeper than this are refused (batches don't nest; this bounds a crafted request).
const MAX_NESTING: usize = 4;

/// A refusal to the agent: English protocol data, as every control reply is (`control.rs`), never
/// a status-bar message, so it isn't in the interface catalogs.
fn refusal(why: String) -> Result<(), String> {
    Err(why)
}

/// A batch step's command and params.
fn step(s: &Value) -> (&str, &Value) {
    (s.get("command").and_then(Value::as_str).unwrap_or(""), s.get("params").unwrap_or(&Value::Null))
}

/// The guards on one command and the steps it runs (`command.batch`, `action.batch`).
pub(crate) fn check_command(command: &str, params: &Value, places: Option<&Places>, own: Option<&str>, depth: usize) -> Result<(), String> {
    if depth > MAX_NESTING {
        return refusal("agents' batches don't nest this deep; nothing was run".into());
    }
    let steps = || params.get("commands").and_then(Value::as_array).into_iter().flatten();
    match command {
        "command.batch" => {
            for s in steps() {
                let (c, p) = step(s);
                check_command(c, p, places, own, depth + 1)?;
            }
        }
        "action.batch" => {
            // It plays its steps once per file (or data set) and names the files it saves itself.
            let destination = params.get("destination").and_then(Value::as_str).unwrap_or("none");
            if destination != "none" || params.get("destinationFolder").is_some_and(|f| !f.is_null()) {
                return refusal(
                    "agents run action.batch only with destination `none` (Save and Close writes over the source files, a folder destination names files itself); save or export each result with a request of its own"
                        .into(),
                );
            }
            for s in steps() {
                let (c, p) = step(s);
                if writes_files(c) && PATH_KEYS.iter().any(|k| p.get(*k).is_some_and(|v| !v.is_null())) {
                    return refusal(format!(
                        "action.batch would run `{c}` once per file, writing the same path again; save or export with a request of its own"
                    ));
                }
                check_command(c, p, places, own, depth + 1)?;
            }
            return Ok(());
        }
        _ => {}
    }
    if !writes_files(command) {
        return Ok(());
    }
    let mut given = PATH_KEYS.iter().filter_map(|k| params.get(*k).map(|v| (*k, v))).peekable();
    if given.peek().is_none() {
        // Nothing named: the data comes back to the agent (or the document's own file is saved).
        return Ok(());
    }
    let places = places.ok_or_else(|| NO_HOME.to_string())?;
    for (key, value) in given {
        let names: Vec<&str> = match value {
            Value::String(s) => vec![s.as_str()],
            Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
            _ => continue,
        };
        for name in names {
            match key {
                "path" | "paths" | "file" | "files" => {
                    if RENAMES_ITS_FILE.contains(&command) {
                        varos_agents::guard::check_file_place(name, places)?;
                    } else {
                        let own = own.filter(|_| SAVES_OWN_FILE.contains(&command));
                        varos_agents::guard::check_new_file(name, places, own)?;
                    }
                }
                "folder" if WRITES_A_SET.contains(&command) => varos_agents::guard::check_empty_folder(name, places)?,
                _ => varos_agents::guard::check_folder(name, places)?,
            }
        }
    }
    Ok(())
}

impl VectorcraftApp {
    /// Varos: the host's file writers (Save, Export, renders written by the app) check every path
    /// they write against the write guard of the agent request that started them
    /// (`file_access::check_write`), as [`VectorcraftApp::with_automation_roots`] does for roots.
    /// Without a guarded request in force, the check passes everything: the person's own saves
    /// are never guarded.
    pub fn with_agent_guards(mut self) -> Self {
        let s = &mut self.services;
        if let Some(mut write) = s.write.take() {
            s.write = Some(Box::new(move |p: &str, b: &[u8]| file_access::check_write(p).and_then(|()| write(p, b))));
        }
        if let Some(write) = s.write_shared.take() {
            s.write_shared = Some(Arc::new(move |p: &str, b: &[u8]| file_access::check_write(p).and_then(|()| write(p, b))));
        }
        if let Some(stream) = s.stream_shared.take() {
            s.stream_shared = Some(Arc::new(move |p: &str, fill: &mut dyn FnMut(&mut dyn std::io::Write) -> std::io::Result<()>| {
                file_access::check_write(p).and_then(|()| stream(p, fill))
            }));
        }
        self
    }
}
