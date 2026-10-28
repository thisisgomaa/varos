//! Varos: an agent's request acts on the board (open document) it names, never on "whichever tab
//! is in front" (production test VQA-11, docs/varos/lanes/agents2-report.txt).
//!
//! Upstream's control methods and commands act on the active document. A request that names a
//! board (`target: {board, revision?}` on its line, put into its actor by the transport) is run
//! like this, all inside the one UI-thread turn that runs it, so no tab click of the person can
//! come in between ([`enter`] / [`leave`] around `control::handle` in `drain_control`):
//!
//! 1. resolve the board id to the open document (`board_not_found`, `session_reset`); check the
//!    revision when one is given, and require it for anything that writes a file
//!    (`stale_revision`, `board_required`);
//! 2. if it isn't the board in front, lend it the front ([`vectorcraft_engine::Session::varos_lend_front`],
//!    which leaves the person's tool work alone; refused as `person_busy` while the person drags
//!    or types) and check it is in front;
//! 3. pin the engine to it ([`vectorcraft_engine::Session::varos_pin`]): every command, each step
//!    of a batch included, runs only while it is the active document;
//! 4. for an agent: the person's selection, current layer, highlighted layer rows, isolation and
//!    view of that board are put aside and the agent's own selection and current layer put in;
//!    the person's dialog is put aside too;
//! 5. run the request;
//! 6. check again: the request ended on its board (or a board it opened, or it closed its board);
//!    a dialog it opened is dismissed (`needs_dialog`). Otherwise `target_changed`;
//! 7. give the person back their selection, layer, view, dialog and tab, in the same turn: agents
//!    never change what the person sees or works with. The reply carries
//!    `varos: {board, revision, front, opened}`.
//!
//! Agents (Varos' agents port, `guarded`) never drive the person's window: synthetic input,
//! pointer gestures, tool, view, panel and window commands, dialogs and closing the person's boards
//! are refused (`person_state`), and so are commands that open a dialog (`needs_dialog`). A batch
//! stays on its board (no step may close, open, make or switch a board) and writes no file
//! (`write_in_batch`). Agents must name a board whenever one is open, except the lists and the
//! commands that open or make a board.
//!
//! Upstream's `--control` port keeps its behaviour unless a line names a board. A window capture
//! or synthetic input naming a board runs only while that board is in front; synthetic input that
//! is still queued when the person switches tabs is dropped, never delivered to the other board
//! ([`screen_synthetic`]).

use std::collections::VecDeque;

use serde_json::{Value, json};
use varos_agents::boards::{
    self, BOARD_NOT_FOUND, BOARD_REQUIRED, NEEDS_DIALOG, PERSON_BUSY, PERSON_STATE, STALE_REVISION, TARGET_CHANGED, WRITE_IN_BATCH, board_id, error,
};
use vectorcraft_doc::{NodeId, Selection};

use crate::VectorcraftApp;
use crate::control::{ControlRequest, Outcome};
use crate::state::{Dialog, View};
use crate::varos::agents::{PATH_KEYS, SAVES_OWN_FILE, command_of, is_guarded, writes_files};

/// What [`enter`] decided.
pub(crate) enum Entered {
    /// Run the request, then [`leave`] with this.
    Run(Box<Scope>),
    /// Answered here (a refusal, or the board list); nothing ran.
    Answered(Value),
}

/// A request in progress: whose front it was and which board it runs on.
pub(crate) struct Scope {
    page_before: Option<String>,
    page_target: Option<String>,
    /// The board in front when the request came (the person's), by uid.
    person: Option<u64>,
    /// The board the request names (uid).
    target: Option<u64>,
    /// The target was lent the front for this request.
    lent: bool,
    /// The engine is pinned to the target for this request.
    pinned: bool,
    /// The boards open when it began.
    open_before: Vec<u64>,
    /// A dialog was open when it began.
    dialog_before: bool,
    /// The target's revision when it began.
    revision_before: Option<u64>,
    /// A request of an agent through Varos' agents port: the person's front, state and dialog are
    /// given back after it whatever it did.
    agent: bool,
    /// The agent's session (its private selection is kept by session and board).
    session: String,
    /// Add `varos` (board, revision, front, opened) to the reply.
    report: bool,
    /// Synthetic input queued before the request ran ([`screen_synthetic`]).
    synthetic_before: usize,
    /// The person's dialog, put aside while an agent's request runs.
    persons_dialog: Option<Dialog>,
    /// The person's state of the target board, put aside while an agent's request runs.
    persons_state: Option<PersonState>,
    /// The tool in use when an agent's request began.
    tool_before: String,
    /// The request's command (or method), for messages.
    what: String,
}

/// What of a board is the person's own: given back after every agent request on it.
pub(crate) struct PersonState {
    uid: u64,
    selection: Selection,
    active_layer: Option<NodeId>,
    layer_rows: Vec<NodeId>,
    isolation: Option<NodeId>,
    view: Option<View>,
}

/// An agent's own selection and current layer on a board, kept between its requests.
#[derive(Clone, Default)]
pub(crate) struct Private {
    selection: Selection,
    active_layer: Option<NodeId>,
}

/// Control methods that read or change no open board.
const DOC_FREE_METHODS: &[&str] = &[
    "engine.commands",
    "ui.inspect",
    "ui.menu.list",
    "ui.contextMenu.list",
    "ui.tool.list",
    "ui.tool.select",
    "ui.set",
    "ui.resize",
    "ui.focus",
    "app.open",
    "app.quit",
];

/// Commands that make or open a board: they need none (the new one is in the reply's `opened`).
const OPENS_A_BOARD: &[&str] = &["file.new", "file.newBoard", "file.open", "document.open", "file.newFromTemplate"];

/// Commands that leave the board a request runs on: never a step of a batch that names one.
const CROSS_BOARD: &[&str] = &[
    "file.new",
    "file.newBoard",
    "file.open",
    "document.open",
    "file.newFromTemplate",
    "file.close",
    "file.closeAll",
    "document.close",
    "document.activate",
    "app.quit",
];

/// Control methods that act on the window, not on a document: only on the board in front.
const FRONT_ONLY: &[&str] = &[
    "ui.click",
    "ui.drag",
    "ui.key",
    "ui.text",
    "ui.move",
    "ui.wheel",
    "ui.pointer",
    "ui.screenshot",
    "ui.dialog.set",
    "ui.dialog.confirm",
    "ui.dialog.cancel",
];

/// Control methods that drive the person's window, tool or input: never for agents.
const PERSONS_METHODS: &[&str] = &[
    "ui.click",
    "ui.drag",
    "ui.key",
    "ui.text",
    "ui.move",
    "ui.wheel",
    "ui.pointer",
    "ui.tool.select",
    "ui.set",
    "ui.resize",
    "ui.focus",
    "ui.dialog.set",
    "ui.dialog.confirm",
    "ui.dialog.cancel",
    "app.quit",
];

/// Command id families that are the person's own interface (tool and its options, view, panels,
/// windows and workspaces, the UI's own commands): never for agents.
const PERSONS_FAMILIES: &[&str] = &["tool.", "view.", "window.", "workspace.", "ui."];

/// Commands that close the person's boards or the app: never for agents.
const PERSONS_COMMANDS: &[&str] = &["file.closeAll", "app.quit"];

/// Commands that are a dialog: never for agents. With [`DIALOG_WITHOUT_OPTIONS`], kept complete by
/// `tests_boards::every_dialog_opening_command_is_refused_up_front`, which runs every command the
/// UI handles itself with no options and lists those that open a dialog.
pub(crate) const DIALOG_COMMANDS: &[&str] = &["edit.keyboardShortcuts", "edit.preferences", "file.documentSetup", "file.newDialog", "type.findFont"];

/// Commands that open a dialog when given no options (the UI asks for them): refused for agents
/// unless the call passes its options. Options that still leave something to ask open a dialog,
/// which is dismissed and answered as `needs_dialog` ([`leave`]).
pub(crate) const DIALOG_WITHOUT_OPTIONS: &[&str] = &[
    "document.exportForOffice",
    "file.export.pdf",
    "file.export.svg",
    "file.exportAs",
    "file.exportForScreens",
    "file.print",
    "file.saveForWeb",
    "object.slice.divide",
];

/// Before a request runs: resolve its board and lend it the front, or answer it here.
pub(crate) fn enter(app: &mut VectorcraftApp, req: &ControlRequest) -> Entered {
    if req.method == boards::LIST {
        let mut value = list(app);
        if req.params.get("pages").and_then(Value::as_bool) == Some(true)
            && let Some(boards) = value["boards"].as_array_mut()
        {
            for (entry, project) in boards.iter_mut().zip(app.session.documents()) {
                entry["pages"] = project.list_pages()["pages"].clone();
            }
        }
        return Entered::Answered(json!({"ok": true, "result": value}));
    }
    let agent = is_guarded(req);
    let target = req.actor.as_ref().and_then(|a| a.get("target")).filter(|t| !t.is_null());
    let person = app.session.active().map(|d| d.uid);
    let open_before: Vec<u64> = app.session.documents().iter().map(|d| d.uid).collect();
    let session = req.actor.as_ref().and_then(|a| a.get("session")).and_then(Value::as_str).unwrap_or("").to_string();
    let mut scope = Box::new(Scope {
        page_before: None,
        page_target: None,
        person,
        target: None,
        lent: false,
        pinned: false,
        open_before,
        dialog_before: app.ui.dialog.is_some(),
        revision_before: None,
        agent,
        session,
        report: agent || target.is_some(),
        synthetic_before: app.synthetic.len(),
        persons_dialog: None,
        persons_state: None,
        tool_before: app.session.tool_id().to_string(),
        what: what(req),
    });
    if !scope.report {
        // Upstream's control port, no board named: upstream's behaviour.
        return Entered::Run(scope);
    }
    let refuse = |why: String| Entered::Answered(json!({"ok": false, "error": why}));
    if agent && let Some(why) = refused_for_agents(req) {
        return refuse(why);
    }
    let Some(board) = target.and_then(|t| t.get("board")).filter(|b| !b.is_null()) else {
        if agent && !scope.open_before.is_empty() && needs_board(req) {
            return refuse(error(
                BOARD_REQUIRED,
                format!("`{}` acts on a board: name it with `board` (from list_boards). Nothing was run.", what(req)),
            ));
        }
        if agent && opens_a_board(req) && app.session.varos_tool_working() {
            return refuse(error(PERSON_BUSY, "the person is dragging or typing; a new board now would end it. Retry in a moment. Nothing was run."));
        }
        begin(app, &mut scope);
        return Entered::Run(scope);
    };
    let uid = match boards::resolve(board) {
        Ok(uid) => uid,
        Err(why) => return refuse(why),
    };
    let Some(doc) = app.session.documents().iter().find(|d| d.uid == uid).map(|d| d.doc.clone()) else {
        return refuse(error(BOARD_NOT_FOUND, format!("board `{}` is not open; call list_boards. Nothing was run.", board_id(uid))));
    };
    let current = project_revision(app, uid).unwrap_or_else(|| app.agents.revisions.of(uid, &doc));
    match target.and_then(|t| t.get("revision")).filter(|r| !r.is_null()) {
        Some(given) => match given.as_u64() {
            Some(r) if r == current => {}
            Some(r) => {
                return refuse(error(
                    STALE_REVISION,
                    format!(
                        "board `{}` is at revision {current}, not {r}: it changed since you read it. Read it again and retry. Nothing was run.",
                        board_id(uid)
                    ),
                ));
            }
            None => return refuse(error(BOARD_REQUIRED, "`revision` must be the number list_boards or the last reply gave. Nothing was run.")),
        },
        None if agent && writes_out(req) => {
            return refuse(error(
                BOARD_REQUIRED,
                format!(
                    "`{}` writes a file: pass the board's current `revision` (list_boards or the last reply) with `board`, so a board that changed since you read it is never written. Nothing was run.",
                    what(req)
                ),
            ));
        }
        None => {}
    }
    scope.target = Some(uid);
    scope.revision_before = Some(current);
    if Some(uid) != person && front_only(req) {
        return refuse(error(
            TARGET_CHANGED,
            format!(
                "`{}` acts on the window, which shows board `{}` now, not `{}` (the person switched tabs). Agents never switch the person's tab; ask the person to bring the board to the front, or use a document verb. Nothing was run.",
                what(req),
                person.map(board_id).unwrap_or_default(),
                board_id(uid)
            ),
        ));
    }
    // Lending ends nothing of the person's only while their tool is idle; an agent's edit during
    // the person's gesture would become part of it, on any board.
    if (agent || Some(uid) != person) && app.session.varos_tool_working() {
        return refuse(error(PERSON_BUSY, "the person is dragging or typing; retry in a moment. Nothing was run."));
    }
    if Some(uid) != person {
        // Lend the front, and check it took: the rest of this turn runs on this board.
        if !app.session.varos_lend_front(uid) || app.session.active().map(|d| d.uid) != Some(uid) {
            give_back(app, &scope);
            return refuse(error(TARGET_CHANGED, format!("board `{}` could not be put to work. Nothing was run.", board_id(uid))));
        }
        scope.lent = true;
    }
    if let Some(project) = app.session.active() {
        scope.page_target = Some(project.page_id().to_owned());
    }
    if let Some(page) = target.and_then(|t| t.get("page")) {
        let result = app.session.project().and_then(|p| p.resolve_page(&json!({"page":page}), "page.target"));
        let index = match result {
            Ok(index) => index,
            Err(e) => {
                give_back(app, &scope);
                return refuse(e.to_string());
            }
        };
        scope.page_before = app.session.active().map(|p| p.page_id().to_owned());
        if let Err(e) = app.session.switch_page(index) {
            give_back(app, &scope);
            return refuse(e.to_string());
        }
        scope.page_target = app.session.active().map(|p| p.page_id().to_owned());
    }
    if !opens_a_board(req) {
        app.session.varos_pin(Some(uid));
        scope.pinned = true;
    }
    begin(app, &mut scope);
    Entered::Run(scope)
}

/// An agent's request is about to run: put the person's dialog aside, and on its board the
/// person's selection, layer, isolation and view, with the agent's own selection and layer in.
fn begin(app: &mut VectorcraftApp, scope: &mut Scope) {
    if !scope.agent {
        return;
    }
    scope.persons_dialog = app.ui.dialog.take();
    let Some(uid) = scope.target else { return };
    let index = app.session.documents().iter().position(|d| d.uid == uid);
    let view = index.and_then(|i| app.views.get(i)).copied();
    let private = app.agents.private.get(&(scope.session.clone(), uid, scope.page_target.clone().unwrap_or_default())).cloned().unwrap_or_default();
    let Some(st) = app.session.document_mut(uid) else { return };
    let mut selection = private.selection;
    selection.prune(&st.doc);
    let layer = private.active_layer.filter(|id| st.doc.node(*id).is_some());
    let saved = PersonState {
        uid,
        selection: std::mem::replace(&mut st.selection, selection),
        active_layer: if layer.is_some() { std::mem::replace(&mut st.active_layer, layer) } else { st.active_layer },
        layer_rows: std::mem::take(&mut st.layer_rows),
        isolation: st.isolation.take(),
        view,
    };
    scope.persons_state = Some(saved);
}

/// After the request ran: check it stayed on its board, give the person back their state and
/// board, and add where it ran to the reply.
pub(crate) fn leave(app: &mut VectorcraftApp, scope: Box<Scope>, answer: Result<Outcome, String>) -> Result<Outcome, String> {
    let mut scope = scope;
    let mut answer = answer;
    if scope.pinned {
        app.session.varos_pin(None);
    }
    // Synthetic input this request queued, with the board it is for.
    let queued = app.synthetic.len().saturating_sub(scope.synthetic_before);
    if queued > 0 {
        let board = scope.target.filter(|_| scope.report);
        app.agents.synthetic.push_back((board, queued));
    }
    let front = app.session.active().map(|d| d.uid);
    let open: Vec<u64> = app.session.documents().iter().map(|d| d.uid).collect();
    let opened: Vec<u64> = open.iter().copied().filter(|u| !scope.open_before.contains(u)).collect();
    let mut refusal = None;
    if let Some(uid) = scope.target {
        let still_open = open.contains(&uid);
        let on_new = front.is_some_and(|f| opened.contains(&f));
        if scope.lent && still_open && front != Some(uid) && !on_new {
            refusal = Some(error(
                TARGET_CHANGED,
                format!(
                    "the request left board `{}` for board `{}` (a command switched documents); what it did there is in that board's History. Name each board with its own call.",
                    board_id(uid),
                    front.map(board_id).unwrap_or_default()
                ),
            ));
        }
    }
    if scope.agent {
        // A dialog never stays open for an agent, and the person's is given back untouched.
        if app.ui.dialog.take().is_some() {
            refusal = Some(error(NEEDS_DIALOG, format!("`{}` opened a dialog, which was dismissed. Pass every option in the call.", scope.what)));
        }
        app.ui.dialog = scope.persons_dialog.take();
        // A tool the request switched to goes back to the person's.
        if app.session.tool_id() != scope.tool_before {
            let tool = scope.tool_before.clone();
            app.select_tool(&tool);
        }
        give_back_state(app, &mut scope);
    } else if scope.lent && !scope.dialog_before && app.ui.dialog.is_some() {
        // A dialog belongs to the window, which goes back to the person's board.
        app.ui.dialog = None;
        refusal = Some(error(
            TARGET_CHANGED,
            format!(
                "it opens a dialog, which needs its board in front, and board `{}` is not the person's; the dialog was dismissed. Pass every option in the call instead.",
                scope.target.map(board_id).unwrap_or_default()
            ),
        ));
    }
    if let (Some(uid), Some(page)) = (scope.target, &scope.page_before)
        && let Some(project) = app.session.document_mut(uid)
        && let Ok(index) = project.resolve_page(&json!({"page":page}), "page.restore")
    {
        // The validated index belongs to this project's unchanged page list.
        if let Err(e) = project.switch(index) {
            refusal = Some(e.to_string());
        }
    }
    give_back(app, &scope);
    app.agents.revisions.retain(|uid| open.contains(&uid));
    app.agents.private.retain(|(_, uid, _), _| open.contains(uid));
    if let Some(why) = refusal {
        return Ok(Outcome::Done(json!({"ok": false, "error": why, "varos": report(app, &scope, &opened)})));
    }
    if scope.report
        && let Ok(Outcome::Done(v)) = &mut answer
        && v.is_object()
    {
        v["varos"] = report(app, &scope, &opened);
    }
    answer
}

/// The person's selection, layer, rows, isolation and view of the target board back; the agent's
/// selection and layer kept for its next request there.
fn give_back_state(app: &mut VectorcraftApp, scope: &mut Scope) {
    let Some(saved) = scope.persons_state.take() else { return };
    let index = app.session.documents().iter().position(|d| d.uid == saved.uid);
    if let Some(project) = app.session.document_mut(saved.uid)
        && let Some(st) = scope.page_target.as_deref().and_then(|id| project.page_state_mut(id))
    {
        let private = Private { selection: st.selection.clone(), active_layer: st.active_layer };
        let mut selection = saved.selection;
        selection.prune(&st.doc);
        st.selection = selection;
        let exists = |id: &NodeId| st.doc.node(*id).is_some();
        st.active_layer = saved.active_layer.filter(exists).or(st.active_layer.filter(exists));
        st.layer_rows = saved.layer_rows.into_iter().filter(exists).collect();
        st.isolation = saved.isolation.filter(exists);
        app.agents.private.insert((scope.session.clone(), saved.uid, scope.page_target.clone().unwrap_or_default()), private);
    }
    if let (Some(i), Some(view)) = (index, saved.view)
        && let Some(v) = app.views.get_mut(i)
    {
        *v = view;
    }
}

/// Give the person's board the front back (also after a board the request made or opened came
/// to the front). When the person was on Home (no board), a board the agent made stays in front.
fn give_back(app: &mut VectorcraftApp, scope: &Scope) {
    if !(scope.lent || scope.agent) {
        return;
    }
    let Some(person) = scope.person else { return };
    if app.session.active().map(|d| d.uid) != Some(person) {
        app.session.varos_lend_front(person);
    }
}

/// Where the request ran: its board and that board's revision now, the board in front (the
/// person's), the boards it opened or made, and synthetic input dropped since the last report
/// because its board had left the front.
fn report(app: &mut VectorcraftApp, scope: &Scope, opened: &[u64]) -> Value {
    let board = scope.target.or(match opened {
        [one] => Some(*one),
        _ => None,
    });
    let revision = board.and_then(|uid| project_revision(app, uid));
    let mut v = json!({
        "board": board.map(board_id),
        "revision": revision,
        "front": app.session.active().map(|d| board_id(d.uid)),
        "opened": opened.iter().copied().map(board_id).collect::<Vec<_>>(),
    });
    if scope.page_before.is_some()
        && let (Some(uid), Some(id)) = (scope.target, &scope.page_target)
        && let Some(project) = app.session.documents().iter().find(|p| p.uid == uid)
    {
        if let Some(page) = project.pages().iter().find(|p| &p.id == id) {
            v["page_id"] = json!(page.id);
            v["page_name"] = json!(page.name);
            v["active"] = json!(project.page_id() == id);
        }
        v["pages"] = project.list_pages()["pages"].clone();
    }
    if !app.agents.dropped.is_empty() {
        v["dropped"] = Value::Array(std::mem::take(&mut app.agents.dropped));
    }
    v
}

/// Before the host injects the next synthetic input: input queued for a board that is no longer
/// in front (the person switched tabs) is dropped, never delivered to the board now in front. The
/// next reply says so (`varos.dropped`, `target_changed`). Called from `raw_input_hook`.
pub(crate) fn screen_synthetic(app: &mut VectorcraftApp) {
    let tags: &mut VecDeque<(Option<u64>, usize)> = &mut app.agents.synthetic;
    // What the host took since: from the front of the queue (or all of it, cleared).
    let tagged: usize = tags.iter().map(|t| t.1).sum();
    let mut taken = tagged.saturating_sub(app.synthetic.len());
    while taken > 0 {
        let Some(first) = tags.front_mut() else { break };
        let n = first.1.min(taken);
        first.1 -= n;
        taken -= n;
        if first.1 == 0 {
            tags.pop_front();
        }
    }
    let front = app.session.active().map(|d| d.uid);
    while let Some((Some(board), n)) = tags.front().copied() {
        if Some(board) == front {
            break;
        }
        tags.pop_front();
        let n = n.min(app.synthetic.len());
        app.synthetic.drain(..n);
        app.agents.dropped.push(json!({
            "board": board_id(board),
            "events": n,
            "error": error(TARGET_CHANGED, format!("synthetic input for board `{}` was not delivered: another board is in front now", board_id(board))),
        }));
    }
}

/// The open boards: stable id, title, file, unsaved changes, content revision, and which one is
/// in front. Ids don't change when other tabs close or move.
pub(crate) fn list(app: &mut VectorcraftApp) -> Value {
    let front = app.session.active().map(|d| d.uid);
    // Each board's entry, and its document for the revision (counted after the borrow ends).
    let entries: Vec<(u64, Value, std::sync::Arc<vectorcraft_doc::Document>)> = app
        .session
        .documents()
        .iter()
        .map(|d| {
            let entry = json!({
                "board": board_id(d.uid),
                "title": d.title(),
                "path": d.path,
                "dirty": d.is_dirty(),
                "front": Some(d.uid) == front,
                "artboards": d.doc.artboards.len(),
            });
            (d.uid, entry, d.doc.clone())
        })
        .collect();
    app.agents.revisions.retain(|uid| entries.iter().any(|e| e.0 == uid));
    let boards: Vec<Value> = entries
        .into_iter()
        .map(|(uid, mut entry, doc)| {
            entry["revision"] = json!(project_revision(app, uid).unwrap_or_else(|| app.agents.revisions.of(uid, &doc)));
            entry
        })
        .collect();
    json!({"api": boards::API, "boards": boards, "front": front.map(board_id)})
}

/// The request's command id (or its method), for messages.
fn what(req: &ControlRequest) -> String {
    command_of(req).map_or_else(|| req.method.clone(), |(c, _)| c.to_string())
}

/// The commands a request runs, with their batch depth: itself, and the steps of its batches.
fn commands(command: &str, params: &Value, out: &mut Vec<(String, Value, usize)>, depth: usize) {
    out.push((command.to_string(), params.clone(), depth));
    if depth < 4 && matches!(command, "command.batch" | "action.batch") {
        for step in params.get("commands").and_then(Value::as_array).into_iter().flatten() {
            let c = step.get("command").and_then(Value::as_str).unwrap_or("");
            commands(c, step.get("params").unwrap_or(&Value::Null), out, depth + 1);
        }
    }
}

/// Is the request a command run through the engine (not another control method)?
fn runs_a_command(req: &ControlRequest) -> bool {
    matches!(req.method.as_str(), "engine.execute" | "ui.menu.invoke" | "command")
}

fn request_commands(req: &ControlRequest) -> Vec<(String, Value, usize)> {
    let mut out = vec![];
    if let Some((c, p)) = command_of(req) {
        commands(c, p, &mut out, 0);
    }
    out
}

/// Does the request need a board named (when one is open)?
fn needs_board(req: &ControlRequest) -> bool {
    if DOC_FREE_METHODS.contains(&req.method.as_str()) {
        return false;
    }
    !opens_a_board(req)
}

/// Does the request only make or open a board?
fn opens_a_board(req: &ControlRequest) -> bool {
    match command_of(req) {
        Some((c, _)) => OPENS_A_BOARD.contains(&c) || req.method == "app.open",
        None => false,
    }
}

/// Does the request act on the window (only on the board in front)?
fn front_only(req: &ControlRequest) -> bool {
    FRONT_ONLY.contains(&req.method.as_str())
}

/// Does `command` with `params` write a file from the board: a save, or an export or render
/// given a path?
fn writes_a_file(command: &str, params: &Value) -> bool {
    writes_files(command) && (SAVES_OWN_FILE.contains(&command) || PATH_KEYS.iter().any(|k| params.get(*k).is_some_and(|v| !v.is_null())))
}

/// Does the request (or a step of it) write a file from the board?
pub(crate) fn writes_out(req: &ControlRequest) -> bool {
    request_commands(req).iter().any(|(c, p, _)| writes_a_file(c, p))
}

/// What agents are never allowed: driving the person's window, tool, view or dialogs, closing the
/// person's boards, switching the person's tab, naming a document by tab index (indices shift
/// when tabs close; the board id doesn't), commands that open a dialog, and batches that leave
/// their board or write a file.
pub(crate) fn refused_for_agents(req: &ControlRequest) -> Option<String> {
    if PERSONS_METHODS.contains(&req.method.as_str()) {
        return Some(error(
            PERSON_STATE,
            format!(
                "`{}` drives the person's window, tool or input; agents work through document verbs on explicit ids. Nothing was run.",
                req.method
            ),
        ));
    }
    let commands = request_commands(req);
    let batch = commands.iter().any(|(_, _, depth)| *depth > 0);
    for (c, p, depth) in &commands {
        let c = c.as_str();
        if c == "document.activate" {
            return Some(error(
                TARGET_CHANGED,
                "agents never change which board is in front; name the board you work on with `board` instead. Nothing was run.",
            ));
        }
        if *depth > 0 && CROSS_BOARD.contains(&c) {
            return Some(error(
                TARGET_CHANGED,
                format!(
                    "a batch stays on the board it names: `{c}` would close, open, make or switch a board; run it as a call of its own. Nothing was run."
                ),
            ));
        }
        if (runs_a_command(req) || *depth > 0) && (PERSONS_COMMANDS.contains(&c) || PERSONS_FAMILIES.iter().any(|f| c.starts_with(f))) {
            return Some(error(
                PERSON_STATE,
                format!("`{c}` changes the person's own interface (tool, view, panels, windows) or closes their boards. Nothing was run."),
            ));
        }
        if matches!(c, "file.close" | "document.close") && p.get("index").is_some_and(|i| !i.is_null()) {
            return Some(error(BOARD_REQUIRED, format!("`{c}` takes the board to close as `board`, not a tab index. Nothing was run.")));
        }
        let no_options = p.as_object().is_none_or(|o| o.is_empty());
        if DIALOG_COMMANDS.contains(&c) || (no_options && DIALOG_WITHOUT_OPTIONS.contains(&c)) {
            return Some(error(NEEDS_DIALOG, format!("`{c}` opens a dialog; agents pass every option in the call. Nothing was run.")));
        }
        if batch && *depth > 0 && writes_a_file(c, p) {
            return Some(error(
                WRITE_IN_BATCH,
                format!(
                    "`{c}` writes a file inside a batch; run the batch, then save or export in a call of its own with the revision it answered. Nothing was run."
                ),
            ));
        }
    }
    None
}

fn project_revision(app: &mut VectorcraftApp, uid: u64) -> Option<u64> {
    let project = app.session.documents().iter().find(|p| p.uid == uid)?;
    let pages = project.pages().iter().map(|p| (p.id.clone(), p.name.clone(), p.doc.doc.clone())).collect();
    Some(app.agents.revisions.of_pages(uid, pages))
}
