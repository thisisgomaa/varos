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
//! 3. run the request;
//! 4. check again: the request ended on its board (or a board it opened, or it closed its board);
//!    a dialog it opened on a lent board is dismissed. Otherwise `target_changed`;
//! 5. give the person's board the front back, in the same turn: agents never change what the
//!    person sees. The reply carries `varos: {board, revision, front, opened}`.
//!
//! Verbs that act on the window itself (synthetic clicks, keys and text, pointer gestures,
//! dialogs, menus, window captures) can't be lent a board: they run only when their board is the
//! one in front, else `target_changed`. Requests through Varos' agents port (`guarded`) must name
//! a board whenever one is open, except the lists and the commands that open or make a board;
//! upstream's `--control` port keeps its behaviour unless a line names a board.

use serde_json::{Value, json};
use varos_agents::boards::{self, BOARD_NOT_FOUND, BOARD_REQUIRED, PERSON_BUSY, STALE_REVISION, TARGET_CHANGED, board_id, error};

use crate::VectorcraftApp;
use crate::control::{ControlRequest, Outcome};
use crate::varos::agents::{PATH_KEYS, SAVES_OWN_FILE, command_of, is_guarded, writes_files};

/// What [`enter`] decided.
pub(crate) enum Entered {
    /// Run the request, then [`leave`] with this.
    Run(Scope),
    /// Answered here (a refusal, or the board list); nothing ran.
    Answered(Value),
}

/// A request in progress: whose front it was and which board it runs on.
pub(crate) struct Scope {
    /// The board in front when the request came (the person's), by uid.
    person: Option<u64>,
    /// The board the request names (uid).
    target: Option<u64>,
    /// The target was lent the front for this request.
    lent: bool,
    /// The boards open when it began.
    open_before: Vec<u64>,
    /// A dialog was open when it began.
    dialog_before: bool,
    /// The target's revision when it began.
    revision_before: Option<u64>,
    /// A request of an agent through Varos' agents port: the person's front is given back after it
    /// whatever it did.
    agent: bool,
    /// Add `varos` (board, revision, front, opened) to the reply.
    report: bool,
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
    "ui.menu.invoke",
];

/// Before a request runs: resolve its board and lend it the front, or answer it here.
pub(crate) fn enter(app: &mut VectorcraftApp, req: &ControlRequest) -> Entered {
    if req.method == boards::LIST {
        return Entered::Answered(json!({"ok": true, "result": list(app)}));
    }
    let agent = is_guarded(req);
    let target = req.actor.as_ref().and_then(|a| a.get("target")).filter(|t| !t.is_null());
    let person = app.session.active().map(|d| d.uid);
    let open_before: Vec<u64> = app.session.documents().iter().map(|d| d.uid).collect();
    let mut scope = Scope {
        person,
        target: None,
        lent: false,
        open_before,
        dialog_before: app.ui.dialog.is_some(),
        revision_before: None,
        agent,
        report: agent || target.is_some(),
    };
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
        return Entered::Run(scope);
    };
    let uid = match boards::resolve(board) {
        Ok(uid) => uid,
        Err(why) => return refuse(why),
    };
    let Some(doc) = app.session.documents().iter().find(|d| d.uid == uid).map(|d| d.doc.clone()) else {
        return refuse(error(BOARD_NOT_FOUND, format!("board `{}` is not open; call list_boards. Nothing was run.", board_id(uid))));
    };
    let current = app.agents.revisions.of(uid, &doc);
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
    if Some(uid) == person {
        return Entered::Run(scope);
    }
    if front_only(req) {
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
    if app.session.varos_tool_working() {
        return refuse(error(PERSON_BUSY, "the person is dragging or typing on another board; retry in a moment. Nothing was run."));
    }
    // Lend the front, and check it took: the rest of this turn runs on this board.
    if !app.session.varos_lend_front(uid) || app.session.active().map(|d| d.uid) != Some(uid) {
        give_back(app, &scope);
        return refuse(error(TARGET_CHANGED, format!("board `{}` could not be put to work. Nothing was run.", board_id(uid))));
    }
    scope.lent = true;
    Entered::Run(scope)
}

/// After the request ran: check it stayed on its board, give the person's board the front back,
/// and add where it ran to the reply.
pub(crate) fn leave(app: &mut VectorcraftApp, scope: Scope, answer: Result<Outcome, String>) -> Result<Outcome, String> {
    let mut answer = answer;
    let front = app.session.active().map(|d| d.uid);
    let open: Vec<u64> = app.session.documents().iter().map(|d| d.uid).collect();
    let opened: Vec<u64> = open.iter().copied().filter(|u| !scope.open_before.contains(u)).collect();
    let mut changed_target = None;
    if let Some(uid) = scope.target {
        let still_open = open.contains(&uid);
        let on_new = front.is_some_and(|f| opened.contains(&f));
        if scope.lent && still_open && front != Some(uid) && !on_new {
            changed_target = Some(format!(
                "the request left board `{}` for board `{}` (a command switched documents); what it did there is in that board's History. Name each board with its own call.",
                board_id(uid),
                front.map(board_id).unwrap_or_default()
            ));
        }
    }
    if scope.lent && !scope.dialog_before && app.ui.dialog.is_some() {
        // A dialog belongs to the window, which goes back to the person's board.
        app.ui.dialog = None;
        let unchanged = scope.target.zip(scope.revision_before).is_some_and(|(uid, before)| {
            app.session
                .documents()
                .iter()
                .find(|d| d.uid == uid)
                .map(|d| d.doc.clone())
                .is_some_and(|doc| app.agents.revisions.of(uid, &doc) == before)
        });
        changed_target = Some(format!(
            "it opens a dialog, which needs its board in front, and board `{}` is not the person's; the dialog was dismissed{}. Pass every option in the call instead.",
            scope.target.map(board_id).unwrap_or_default(),
            if unchanged { " and nothing changed" } else { "" }
        ));
    }
    give_back(app, &scope);
    app.agents.revisions.retain(|uid| open.contains(&uid));
    if let Some(why) = changed_target {
        return Ok(Outcome::Done(json!({"ok": false, "error": error(TARGET_CHANGED, why), "varos": report(app, &scope, &opened)})));
    }
    if scope.report
        && let Ok(Outcome::Done(v)) = &mut answer
        && v.is_object()
    {
        v["varos"] = report(app, &scope, &opened);
    }
    answer
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
/// person's), and the boards it opened or made.
fn report(app: &mut VectorcraftApp, scope: &Scope, opened: &[u64]) -> Value {
    let board = scope.target.or(match opened {
        [one] => Some(*one),
        _ => None,
    });
    let revision = board.and_then(|uid| {
        let doc = app.session.documents().iter().find(|d| d.uid == uid).map(|d| d.doc.clone())?;
        Some(app.agents.revisions.of(uid, &doc))
    });
    json!({
        "board": board.map(board_id),
        "revision": revision,
        "front": app.session.active().map(|d| board_id(d.uid)),
        "opened": opened.iter().copied().map(board_id).collect::<Vec<_>>(),
    })
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
            entry["revision"] = json!(app.agents.revisions.of(uid, &doc));
            entry
        })
        .collect();
    json!({"api": boards::API, "boards": boards, "front": front.map(board_id)})
}

/// The request's command id (or its method), for messages.
fn what(req: &ControlRequest) -> String {
    command_of(req).map_or_else(|| req.method.clone(), |(c, _)| c.to_string())
}

/// The commands a request runs: itself, and the steps of its batches.
fn commands(command: &str, params: &Value, out: &mut Vec<(String, Value)>, depth: usize) {
    out.push((command.to_string(), params.clone()));
    if depth < 4 && matches!(command, "command.batch" | "action.batch") {
        for step in params.get("commands").and_then(Value::as_array).into_iter().flatten() {
            let c = step.get("command").and_then(Value::as_str).unwrap_or("");
            commands(c, step.get("params").unwrap_or(&Value::Null), out, depth + 1);
        }
    }
}

fn request_commands(req: &ControlRequest) -> Vec<(String, Value)> {
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

/// Does the request (or a step of it) write a file from the board: a save, or an export or
/// render given a path?
pub(crate) fn writes_out(req: &ControlRequest) -> bool {
    request_commands(req)
        .iter()
        .any(|(c, p)| writes_files(c) && (SAVES_OWN_FILE.contains(&c.as_str()) || PATH_KEYS.iter().any(|k| p.get(*k).is_some_and(|v| !v.is_null()))))
}

/// What agents are never allowed: switching the person's tab, or naming a document by tab index
/// (indices shift when tabs close; the board id doesn't).
fn refused_for_agents(req: &ControlRequest) -> Option<String> {
    for (c, p) in request_commands(req) {
        if c == "document.activate" {
            return Some(error(
                TARGET_CHANGED,
                "agents never change which board is in front; name the board you work on with `board` instead. Nothing was run.",
            ));
        }
        if matches!(c.as_str(), "file.close" | "document.close") && p.get("index").is_some_and(|i| !i.is_null()) {
            return Some(error(BOARD_REQUIRED, format!("`{c}` takes the board to close as `board`, not a tab index. Nothing was run.")));
        }
    }
    None
}
