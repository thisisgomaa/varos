//! Varos: every agent operation names the board (the open document) it acts on, by a stable id,
//! and can't land on another one (production test VQA-11: with the person switching tabs, agent
//! commands that followed "the active document" hit the wrong one, and an export was written
//! from an unintended document).
//!
//! - A **board id** is `"<launch>-<uid>"`: this launch's random tag ([`launch`]) and the open
//!   document's process-unique uid (`DocState::uid`, which survives other tabs closing or moving;
//!   a tab index does not). An id from an earlier launch is refused as [`SESSION_RESET`], so a
//!   restarted Varos can never take it for one of its own documents.
//! - A board's **revision** ([`Revisions`]) moves only when its content changes (a new document
//!   value), not when the person selects or scrolls. Save and export need the revision the agent
//!   last read ([`STALE_REVISION`] otherwise).
//! - **Object references** given to agents carry their board: `"<board>:<id>"` ([`qualify`]). One
//!   applied to another board is refused as [`WRONG_BOARD`] ([`unqualify`]).
//!
//! The rules are applied by the app, inside the one UI-thread turn that runs the request
//! (`crates/ui-egui/src/varos/boards.rs`), and by the MCP proxy for the references
//! (`crates/mcp/src/varos_boards.rs`). This module holds what both share: ids, error codes, the
//! reference rewriting and the revision counter. Pure logic.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use serde_json::Value;
use vectorcraft_doc::Document;

mod requests;
pub use requests::unqualify_tool;

/// The contract agents discover through `capabilities`.
pub const API: &str = "varos-boards/1";

/// The control method that lists the open boards.
pub const LIST: &str = "varos.boards";

/// No board named, where one must be (or a save/export without its revision).
pub const BOARD_REQUIRED: &str = "board_required";
/// The named board isn't open (closed, or never was).
pub const BOARD_NOT_FOUND: &str = "board_not_found";
/// The board id is from an earlier launch of Varos.
pub const SESSION_RESET: &str = "session_reset";
/// The verb acts on the window, which shows another board now (the person switched tabs), or the
/// request would have moved off its board. Nothing was run or kept.
pub const TARGET_CHANGED: &str = "target_changed";
/// The board changed since the revision the agent gave.
pub const STALE_REVISION: &str = "stale_revision";
/// An object reference of one board was applied to another.
pub const WRONG_BOARD: &str = "wrong_board";
/// The person is in the middle of a gesture or typing; working on another board now would end it.
pub const PERSON_BUSY: &str = "person_busy";
/// The verb would change the person's own state: their tool, view, window, panels, dialogs or
/// input. Agents work on boards through document verbs.
pub const PERSON_STATE: &str = "person_state";
/// The command needs a dialog. Agents pass every option in the call; a dialog never opens for
/// them and the person's dialog is never touched.
pub const NEEDS_DIALOG: &str = "needs_dialog";
/// A batch writes a file. Saves and exports go in a request of their own, after the batch, with
/// the revision it answered.
pub const WRITE_IN_BATCH: &str = "write_in_batch";
/// A save or an export couldn't write its file: the folder or volume takes none (a read-only disk,
/// no permission). Nothing was written; the board keeps its file and stays modified.
pub const READ_ONLY_LOCATION: &str = "read_only_location";

/// Every error code with what it means and what the agent does next, for `capabilities`.
pub const ERRORS: &[(&str, &str)] = &[
    (BOARD_REQUIRED, "name the board (list_boards); save, save_as and exports also need its current revision. Nothing was run."),
    (BOARD_NOT_FOUND, "that board is not open any more; call list_boards. Nothing was run."),
    (SESSION_RESET, "Varos restarted since the board id was read; call list_boards again. Nothing was run."),
    (
        TARGET_CHANGED,
        "the verb needs its board in front of the person and another board is in front now (the person switched tabs), or the request would have left its board. Nothing was run or kept; read the board again and retry.",
    ),
    (STALE_REVISION, "the board changed since the revision you gave; read it again (doc_inspect / list_boards) and retry. Nothing was run."),
    (WRONG_BOARD, "an object reference (\"<board>:<id>\") belongs to another board than the one named. Nothing was run."),
    (PERSON_BUSY, "the person is dragging or typing; retry in a moment. Nothing was run."),
    (
        PERSON_STATE,
        "the verb would change the person's tool, view, window, panels, dialogs or input (or close their boards); agents work through document verbs on explicit ids. Nothing was run.",
    ),
    (NEEDS_DIALOG, "the command opens a dialog; pass every option in the call (command_list shows them). Nothing was run."),
    (
        WRITE_IN_BATCH,
        "a batch may not save or export: run the batch, then save or export in a call of its own with the revision the batch answered. Nothing was run.",
    ),
    (
        READ_ONLY_LOCATION,
        "the file's folder or volume can't be written (a read-only disk, no permission); save_as to a folder that can be written. Nothing was written.",
    ),
];

/// A refusal to the agent: `"<code>: <why>"`, as the proxy's other typed errors.
pub fn error(code: &str, why: impl std::fmt::Display) -> String {
    format!("{code}: {why}")
}

/// The code of an error message made by [`error`], if it is one of [`ERRORS`].
pub fn code_of(message: &str) -> Option<&'static str> {
    let head = message.split(':').next()?;
    ERRORS.iter().map(|(code, _)| *code).find(|code| *code == head)
}

/// This launch's tag: 8 random hex digits, made once.
pub fn launch() -> &'static str {
    static LAUNCH: OnceLock<String> = OnceLock::new();
    // No random source (the web, which has no agents port): a fixed tag.
    LAUNCH.get_or_init(|| crate::auth::random_hex(4).unwrap_or_else(|_| "00000000".to_string()))
}

/// The board id of the open document with uid `uid`, in this launch.
pub fn board_id(uid: u64) -> String {
    format!("{}-{uid}", launch())
}

/// A board id taken apart: `(launch tag, uid)`. `None` when it isn't shaped like one.
pub fn parse_board(id: &str) -> Option<(&str, u64)> {
    let (tag, uid) = id.split_once('-')?;
    (tag.len() == 8 && tag.bytes().all(|b| b.is_ascii_hexdigit()) && !uid.is_empty() && uid.bytes().all(|b| b.is_ascii_digit()))
        .then(|| uid.parse().ok().map(|uid| (tag, uid)))
        .flatten()
}

/// The uid a board id names in this launch, or the refusal ([`BOARD_NOT_FOUND`] for a malformed
/// id, [`SESSION_RESET`] for another launch's). Whether it is open is the caller's question.
pub fn resolve(board: &Value) -> Result<u64, String> {
    let Some(id) = board.as_str() else {
        return Err(error(BOARD_REQUIRED, "`board` must be a board id string from list_boards"));
    };
    match parse_board(id) {
        Some((tag, uid)) if tag == launch() => Ok(uid),
        Some(_) => Err(error(SESSION_RESET, format!("board `{id}` is from an earlier launch of Varos; call list_boards again"))),
        None => Err(error(BOARD_NOT_FOUND, format!("`{id}` is not a board id; call list_boards"))),
    }
}

/// The reference agents get for object `id` of `board`.
pub fn object_ref(board: &str, id: u64) -> String {
    format!("{board}:{id}")
}

/// An object reference taken apart: `(board, id)`. `None` when `s` isn't shaped like one.
pub fn parse_object_ref(s: &str) -> Option<(&str, u64)> {
    let (board, id) = s.rsplit_once(':')?;
    parse_board(board)?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())).then(|| id.parse().ok().map(|id| (board, id))).flatten()
}

/// Keys whose numbers (or arrays of numbers) are object ids in what commands answer: the node
/// ids of `document.inspect` (`id`, `parent`, the ancestors in `path`, `selection`), the ids
/// creation commands answer (`id`, `ids`, `created`, `objects`). `objects` and `selected`
/// carry ids only as lists: their scalars can be counts, point indexes, booleans or names.
/// Artboard ids under `id` are qualified too: they are per board as well.
pub const ID_KEYS: &[&str] = &["id", "ids", "created", "parent", "path", "selection", "objects", "selected"];

/// Turn the object ids in `v` (numbers under [`ID_KEYS`]) into references of `board`.
pub fn qualify(v: &mut Value, board: &str) {
    match v {
        Value::Object(map) => {
            for (k, child) in map.iter_mut() {
                if id_field(k, child) {
                    qualify_numbers(child, board);
                }
                qualify(child, board);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| qualify(item, board)),
        _ => {}
    }
}

fn id_field(key: &str, value: &Value) -> bool {
    ID_KEYS.contains(&key) && (!matches!(key, "objects" | "selected") || value.is_array())
}

fn qualify_numbers(v: &mut Value, board: &str) {
    match v {
        Value::Number(n) => {
            if let Some(id) = n.as_u64() {
                *v = Value::String(object_ref(board, id));
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut().filter(|i| i.is_u64()) {
                qualify_numbers(item, board);
            }
        }
        _ => {}
    }
}

/// Decode references only at id leaves of a reference record (the fields of [`ID_KEYS`], plus
/// `target`/`targets`). Other strings, including strings in arrays, are literal. With `board`
/// given, a reference of another board is refused as [`WRONG_BOARD`]. Otherwise references
/// imply one board. MCP requests use [`unqualify_tool`] to disambiguate overloaded fields such
/// as a file `path` versus a text path id, and to leave arbitrary authored parameter maps alone.
pub fn unqualify(v: &mut Value, board: Option<&str>) -> Result<Option<String>, String> {
    let mut named: Option<String> = board.map(str::to_string);
    walk_refs(v, &mut named, board.is_some(), false)?;
    Ok(named)
}

fn walk_refs(v: &mut Value, named: &mut Option<String>, given: bool, is_id: bool) -> Result<(), String> {
    match v {
        Value::String(s) if is_id => {
            if let Some((b, id)) = parse_object_ref(s) {
                match named.as_deref() {
                    Some(n) if n != b => {
                        let why = if given {
                            format!("object `{s}` belongs to board `{b}`, not to board `{n}` named by this call")
                        } else {
                            format!("objects of two boards (`{n}` and `{b}`) in one call")
                        };
                        return Err(error(WRONG_BOARD, why));
                    }
                    Some(_) => {}
                    None => *named = Some(b.to_string()),
                }
                *v = Value::from(id);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_refs(item, named, given, is_id)?;
            }
        }
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let is_id = id_field(key, child) || matches!(key.as_str(), "target" | "targets");
                walk_refs(child, named, given, is_id)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Board revisions that move only with content: a board's number goes up when its document value
/// is not the one last seen (an edit, an undo to another state). Selecting, scrolling or saving
/// leave it. Unchanged documents are one pointer comparison. Starts at 1.
type PageContent = Vec<(String, String, Arc<Document>)>;

#[derive(Default)]
pub struct Revisions {
    seen: HashMap<u64, (u64, Arc<Document>)>,
    projects: HashMap<u64, (u64, PageContent)>,
}

impl Revisions {
    /// The revision of board `uid` whose content is `doc` now.
    pub fn of(&mut self, uid: u64, doc: &Arc<Document>) -> u64 {
        let entry = self.seen.entry(uid).or_insert_with(|| (1, doc.clone()));
        if !Arc::ptr_eq(&entry.1, doc) {
            entry.0 += 1;
            entry.1 = doc.clone();
        }
        entry.0
    }

    /// Project revision excludes active page and selection/view state.
    pub fn of_pages(&mut self, uid: u64, pages: PageContent) -> u64 {
        let entry = self.projects.entry(uid).or_insert_with(|| (1, pages.clone()));
        if entry.1.len() != pages.len() || !entry.1.iter().zip(&pages).all(|(a, b)| a.0 == b.0 && a.1 == b.1 && Arc::ptr_eq(&a.2, &b.2)) {
            entry.0 += 1;
            entry.1 = pages;
        }
        entry.0
    }
    /// Forget the boards that closed.
    pub fn retain(&mut self, open: impl Fn(u64) -> bool) {
        self.seen.retain(|uid, _| open(*uid));
        self.projects.retain(|uid, _| open(*uid));
    }
}
