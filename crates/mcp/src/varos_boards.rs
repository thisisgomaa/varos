//! Varos: every tool call names the board (open document) it acts on, so an agent's work can't
//! land on another document when the person switches tabs (production test VQA-11; the rules:
//! `varos_agents::boards`, the app side: `crates/ui-egui/src/varos/boards.rs`).
//!
//! Every tool takes `board` (a stable id from `list_boards`) and `revision` (the board's content
//! revision; required by saves and exports). They are taken out of the arguments here and sent
//! with every control line the tool makes (`target`), so a tool that makes several calls makes
//! them all on its board. The app resolves the board by id inside the UI turn that runs each
//! line; it never falls back to the tab in front.
//!
//! Object ids leave as references that carry their board (`"<board>:<id>"`) and are turned back
//! into plain ids on the way in; one of another board is refused as `wrong_board` before
//! anything is sent. Every result says where it ran: `varos: {board, revision, front, opened}`.
//!
//! Two tools of our own: `capabilities` (the rules, the error codes, the verbs that need their
//! board in front) and `list_boards`.

use serde_json::{Map, Value, json};
use varos_agents::boards::{self, BOARD_REQUIRED, ERRORS, error};

use crate::backend::Backend;
use crate::resources::ReadError;
use crate::tools::{ToolResult, call_tool as upstream_call_tool};

const LIST_BOARDS: &str = "list_boards";
const CAPABILITIES: &str = "capabilities";

/// Tools that capture the window: their board must be the one in front, else `target_changed`.
const FRONT_ONLY_TOOLS: &[&str] = &["ui_screenshot"];

/// Tools that drive the person's window, tool or input: refused for agents (`person_state`).
const PERSONS_TOOLS: &[&str] = &["pointer_gesture", "press_key", "type_text", "select_tool", "open_panel"];

/// Tools that need no board: the catalogue, the lists, and opening a file (the new board is in
/// the reply's `varos.opened`).
const BOARD_FREE_TOOLS: &[&str] = &[LIST_BOARDS, CAPABILITIES, "command_list", "list_commands", "ui_inspect", "inspect_ui", "open_file"];

/// Resources that read no board: the command catalogue.
const BOARD_FREE_RESOURCES: &[&str] = &["vectorcraft://commands", "vectorcraft://command/"];

/// Where a board's resources live: `vectorcraft://board/<board>/<resource>`.
const BOARD_URI: &str = "vectorcraft://board/";

/// Tools whose description says what saving or exporting needs.
const WRITERS: &[&str] = &["save_file", "export", "screenshot"];

const RULE: &str = "All verbs are board-scoped: pass `board` (a stable id from list_boards) with every call that reads or changes a document, and `revision` too when saving or exporting (save_file, export, command_run file.save / file.saveAs / document.export*, screenshot or render to a path). Varos never falls back to the tab in front. Object ids come back as \"<board>:<id>\" references; pass them as they are. Every reply carries `varos: {board, revision, front, opened}`: use that revision next. The person's tab, tool, selection, view and dialogs are never touched: each agent has its own selection per board, so act on explicit ids. Batches stay on their board and never save or export; save or export in a call of its own. Resources of a board are read as vectorcraft://board/<board>/document, .../object/<id>, .../swatch/<name>.";

/// Add `board` and `revision` to every tool and append the Varos tools (`tool_definitions`).
pub(crate) fn extend(defs: &mut Vec<Value>) {
    for t in defs.iter_mut() {
        let name = t["name"].as_str().unwrap_or("").to_string();
        if let Some(props) = t.get_mut("inputSchema").and_then(|s| s.get_mut("properties")).and_then(Value::as_object_mut) {
            props.insert("board".into(), board_prop(&name));
            props.insert("revision".into(), revision_prop(&name));
            props.insert("page".into(), json!({"type":"string", "description":"Optional stable page_id within board; select/edit/inspect that canvas. Omitted: its active page."}));
        }
        if WRITERS.contains(&name.as_str())
            && let Some(Value::String(d)) = t.get_mut("description")
        {
            d.push_str(" In Varos pass `board` and its current `revision` (list_boards or the last reply): a board that changed since you read it is never written (stale_revision).");
        }
        if FRONT_ONLY_TOOLS.contains(&name.as_str())
            && let Some(Value::String(d)) = t.get_mut("description")
        {
            d.push_str(" In Varos this captures the window: its `board` must be the one in front, else target_changed.");
        }
        if PERSONS_TOOLS.contains(&name.as_str())
            && let Some(Value::String(d)) = t.get_mut("description")
        {
            d.push_str(" Not for agents in Varos: it drives the person's own window, tool or input (person_state).");
        }
    }
    let empty = json!({"type": "object", "properties": {"pages":{"type":"boolean","description":"Opt in to project/page discovery; default responses remain unchanged."}}, "additionalProperties": false});
    defs.push(json!({
        "name": CAPABILITIES,
        "title": "Varos capabilities",
        "description": "Call first. The rules every call follows in Varos (all verbs are board-scoped: pass board + revision), the typed error codes and what to do on each, and the verbs that need their board in front.",
        "inputSchema": empty.clone(),
        "annotations": {"title": "Varos capabilities", "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false},
    }));
    defs.push(json!({
        "name": LIST_BOARDS,
        "title": "List boards",
        "description": "The open boards (documents) of Varos: stable `board` id (survives other tabs closing or moving), title, file path, unsaved changes (`dirty`), content `revision`, and which one is in `front` of the person. Pass the id as `board` to every other tool.",
        "inputSchema": empty,
        "annotations": {"title": "List boards", "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false},
    }));
}

fn board_prop(tool: &str) -> Value {
    let what = if BOARD_FREE_TOOLS.contains(&tool) {
        "Varos: the board (stable id from list_boards) this call acts on; not needed here"
    } else {
        "Varos: the board (stable id from list_boards) this call acts on; required whenever a board is open"
    };
    json!({"type": "string", "description": what})
}

fn revision_prop(tool: &str) -> Value {
    let what = if WRITERS.contains(&tool) {
        "Varos: the board's content revision you last read (list_boards or a reply's varos.revision); required to save or export, else board_required; refused as stale_revision when the board changed since"
    } else {
        "Varos: the board's content revision you last read; when given, the call is refused as stale_revision if the board changed since. Required for saves and exports"
    };
    json!({"type": "integer", "minimum": 0, "description": what})
}

/// What `capabilities` answers.
pub(crate) fn capabilities(scoped: bool) -> Value {
    let errors: Map<String, Value> = ERRORS.iter().map(|(code, what)| ((*code).to_string(), json!(what))).collect();
    json!({
        "api": boards::API,
        "boardScoped": scoped,
        "rule": RULE,
        "boardIds": "\"<launch>-<n>\": stable while Varos runs, whatever happens to other tabs; an id from before a restart is refused as session_reset",
        "objectRefs": "\"<board>:<id>\": object ids carry their board; one of another board is refused as wrong_board",
        "revision": "the board's content revision: it moves when the art changes (anyone's edit, undo), not when the person selects or scrolls",
        "frontOnly": FRONT_ONLY_TOOLS,
        "personsOnly": PERSONS_TOOLS,
        "resources": "vectorcraft://board/<board>/document, vectorcraft://board/<board>/document/json, vectorcraft://board/<board>/object/<id or <board>:<id>>, vectorcraft://board/<board>/swatch/<name>, vectorcraft://board/<board>/effect/<id>; vectorcraft://commands and vectorcraft://command/<id> need no board",
        "boardFree": BOARD_FREE_TOOLS,
        "errors": errors,
    })
}

/// Run one tool with its board: `board` / `revision` taken out of the arguments and sent with
/// every line it makes; references in, references out ([`boards::qualify`]).
pub fn call_tool(b: &mut dyn Backend, name: &str, args: &Value) -> ToolResult {
    if name == CAPABILITIES {
        let mut value = capabilities(b.board_scoped());
        if args.get("pages").and_then(Value::as_bool) == Some(true) {
            value["pages"] = json!({"list":"list_boards {pages:true} adds pages: [{page_id,page_name,active,index,artboards}] per board", "target":"optional page: stable page_id on select/edit/inspect tools; omitted means active page", "commands":["page.list","page.add","page.rename","page.duplicate","page.delete","page.move","page.switch"], "isolation":"selection and history belong to the page; agent calls restore the person's page and view"});
        }
        return ToolResult::json(&value);
    }
    let (args, board, revision) = match split(args) {
        Ok(parts) => parts,
        Err(e) => return ToolResult::error(format!("{name}: {e}")),
    };
    if name == LIST_BOARDS {
        if !b.board_scoped() {
            return ToolResult::error(format!("{name}: boards are a Varos desktop feature; this server runs {}", b.describe()));
        }
        // The list names no board itself.
        if let Err(e) = b.set_target(Some(Value::Null)) {
            return ToolResult::error(format!("{name}: {e}"));
        }
        let r = b.call(boards::LIST, Value::Object(args));
        let _ = b.set_target(None);
        return match r {
            Ok(v) => ToolResult::json(&v),
            Err(e) => ToolResult::error(format!("{name}: {e}")),
        };
    }
    let mut args = Value::Object(args);
    let page = args.as_object_mut().and_then(|a| a.remove("page"));
    if page.as_ref().is_some_and(|p| p.as_str().is_none()) {
        return ToolResult::error("page must be a stable page_id");
    }
    if page.is_some() && !b.board_scoped() {
        return ToolResult::error("page targets require a board-scoped Varos connection; use page.switch in headless sessions");
    }
    let board = match boards::unqualify(&mut args, board.as_deref()) {
        Ok(board) => board,
        Err(e) => return ToolResult::error(format!("{name}: {e}")),
    };
    if board.is_none() && revision.is_some() {
        return ToolResult::error(format!("{name}: {}", error(BOARD_REQUIRED, "`revision` goes with the `board` it is of. Nothing was run.")));
    }
    let target = match &board {
        Some(board) => {
            let mut t = json!({ "board": board });
            if let Some(page) = &page {
                t["page"] = page.clone();
            }
            if let Some(r) = revision {
                t["revision"] = json!(r);
            }
            Some(t)
        }
        // Explicitly none on a Varos board-scoped backend (never the board last named).
        None if b.board_scoped() => Some(Value::Null),
        None => None,
    };
    if let Err(e) = b.set_target(target) {
        return ToolResult::error(format!("{name}: {e}"));
    }
    let mut r = upstream_call_tool(b, name, &args);
    let ran = b.last_board();
    let _ = b.set_target(None);
    if b.board_scoped() {
        decorate(&mut r, &ran);
    }
    r
}

/// A tool's arguments without `board` and `revision`, and those two.
type Split = (Map<String, Value>, Option<String>, Option<u64>);

/// The arguments without `board` and `revision`, and those two.
fn split(args: &Value) -> Result<Split, String> {
    let mut a = match args {
        Value::Object(o) => o.clone(),
        Value::Null => Map::new(),
        _ => return Err("tool arguments must be a JSON object".into()),
    };
    let board = match a.remove("board") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s),
        Some(_) => return Err(error(BOARD_REQUIRED, "`board` must be a board id string from list_boards. Nothing was run.")),
    };
    let revision = match a.remove("revision") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().ok_or_else(|| error(BOARD_REQUIRED, "`revision` must be a non-negative integer. Nothing was run."))?),
    };
    Ok((a, board, revision))
}

/// Object ids in the result become references of the board it ran on, and the result says where
/// it ran (`varos`).
fn decorate(r: &mut ToolResult, ran: &Value) {
    if r.is_error || !ran.is_object() {
        return;
    }
    let board = ran.get("board").and_then(Value::as_str).map(str::to_string);
    let opened = ran.get("opened").and_then(Value::as_array).is_some_and(|o| !o.is_empty());
    if board.is_none() && !opened {
        return;
    }
    for block in r.content.iter_mut().filter(|c| c["type"] == "text") {
        let Some(text) = block["text"].as_str() else { continue };
        let Ok(mut v) = serde_json::from_str::<Value>(text) else { continue };
        if let Some(board) = &board {
            boards::qualify(&mut v, board);
        }
        let v = match v {
            Value::Object(mut o) => {
                o.insert("varos".into(), ran.clone());
                Value::Object(o)
            }
            other => json!({"result": other, "varos": ran}),
        };
        block["text"] = json!(serde_json::to_string_pretty(&v).unwrap_or_default());
        return;
    }
    r.content.push(json!({"type": "text", "text": json!({"varos": ran}).to_string()}));
}

/// `resources/read` with its board: on a Varos board-scoped backend a board's resource is
/// `vectorcraft://board/<board>/<resource>` (its object ids as plain numbers or `<board>:<id>`
/// references of that board), read on that board only, its ids qualified; the board-free forms
/// stay as they are, and the old board-less form of a board's resource is refused with the new
/// form (a resource never follows the board of another call, nor the tab in front).
pub(crate) fn read_resource(b: &mut dyn Backend, uri: &str) -> Result<Value, ReadError> {
    if !b.board_scoped() {
        return crate::resources::read(b, uri);
    }
    let Some(rest) = uri.strip_prefix(BOARD_URI) else {
        if BOARD_FREE_RESOURCES.iter().any(|free| uri == *free || (free.ends_with('/') && uri.starts_with(free))) {
            let _ = b.set_target(Some(Value::Null));
            let r = crate::resources::read(b, uri);
            let _ = b.set_target(None);
            return r;
        }
        let tail = uri.strip_prefix("vectorcraft://").unwrap_or(uri);
        return Err(ReadError::Invalid(error(
            BOARD_REQUIRED,
            format!("in Varos a board's resources name the board: read {BOARD_URI}<board>/{tail} (board ids from list_boards)"),
        )));
    };
    let (board, tail) = rest.split_once('/').unwrap_or((rest, ""));
    if boards::parse_board(board).is_none() {
        return Err(ReadError::Invalid(error(
            BOARD_REQUIRED,
            format!("`{board}` is not a board id; read {BOARD_URI}<board>/... with an id from list_boards"),
        )));
    }
    // An object reference in the URI must be of this board.
    let tail = match tail.strip_prefix("object/") {
        Some(id) => {
            let id = id.replace("%3A", ":").replace("%3a", ":");
            match boards::parse_object_ref(&id) {
                Some((of, n)) if of == board => format!("object/{n}"),
                Some((of, _)) => {
                    return Err(ReadError::Invalid(error(
                        boards::WRONG_BOARD,
                        format!("object `{id}` belongs to board `{of}`, not to board `{board}` the URI names"),
                    )));
                }
                None => format!("object/{id}"),
            }
        }
        None => tail.to_string(),
    };
    b.set_target(Some(json!({ "board": board }))).map_err(ReadError::Backend)?;
    let r = crate::resources::read(b, &format!("vectorcraft://{tail}"));
    let ran = b.last_board();
    let _ = b.set_target(None);
    let mut v = r?;
    boards::qualify(&mut v, board);
    if let Value::Object(o) = &mut v
        && ran.is_object()
    {
        o.insert("varos".into(), ran);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Server, tool_definitions};

    /// A stand-in for Varos with two boards: it records every line's method, params and target,
    /// answers inspect / create calls with plain ids, and says where each ran.
    struct FakeVaros {
        frozen_ids: bool,
        target: Option<Value>,
        seen: Vec<(String, Value, Option<Value>)>,
        last: Value,
    }

    impl Backend for FakeVaros {
        fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
            self.seen.push((method.to_string(), params.clone(), self.target.clone()));
            let board = self.target.as_ref().and_then(|t| t.get("board")).cloned();
            self.last = json!({"board": board, "revision": 7, "front": boards::board_id(1), "opened": []});
            match method {
                boards::LIST if self.frozen_ids => Ok(json!({"boards":[{"board":"fixture-1","front":true},{"board":"fixture-2","front":false}]})),
                boards::LIST => {
                    Ok(json!({"boards": [{"board": boards::board_id(1), "front": true}, {"board": boards::board_id(2), "front": false}]}))
                }
                "engine.commands" => Ok(json!([])),
                _ if board.is_none() => Err(error(BOARD_REQUIRED, "name it")),
                "document.inspect" => Ok(json!({"layers": [{"id": 3, "children": [{"id": 4, "parent": 3}]}], "selection": [4]})),
                _ => Ok(json!({"id": 9})),
            }
        }
        fn has_ui(&self) -> bool {
            true
        }
        fn describe(&self) -> String {
            "fake Varos".into()
        }
        fn set_target(&mut self, target: Option<Value>) -> Result<(), String> {
            self.target = target.filter(|t| !t.is_null());
            Ok(())
        }
        fn last_board(&self) -> Value {
            self.last.clone()
        }
        fn board_scoped(&self) -> bool {
            true
        }
    }

    fn fake() -> FakeVaros {
        FakeVaros { frozen_ids: false, target: None, seen: vec![], last: Value::Null }
    }

    fn json_of(r: &ToolResult) -> Value {
        serde_json::from_str(r.content[0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn every_line_of_a_tool_carries_its_board_and_ids_come_back_qualified() {
        let mut b = fake();
        let a = boards::board_id(1);
        let r = call_tool(&mut b, "doc_inspect", &json!({"board": a, "depth": 1}));
        assert!(!r.is_error, "{r:?}");
        let v = json_of(&r);
        assert_eq!(v["layers"][0]["children"][0]["parent"], json!(format!("{a}:3")));
        assert_eq!(v["selection"], json!([format!("{a}:4")]));
        assert_eq!(v["varos"]["board"], json!(a));
        assert_eq!(v["varos"]["revision"], 7);
        assert_eq!(b.seen[0].2, Some(json!({"board": a})), "the line names the board");
        assert_eq!(b.seen[0].1, json!({"depth": 1}), "board/revision never reach the command");
        assert_eq!(b.target, None, "the next call starts with no board");
        // A multi-call tool: every line on the same board, with the revision.
        b.seen.clear();
        let r = call_tool(&mut b, "set_paint", &json!({"board": a, "revision": 7, "fill": "#ff0000", "ids": [format!("{a}:4")]}));
        assert!(!r.is_error, "{r:?}");
        assert!(!b.seen.is_empty());
        for (_, params, target) in &b.seen {
            assert_eq!(target, &Some(json!({"board": a, "revision": 7})));
            assert!(!params.to_string().contains(&a), "references are plain ids on the way in: {params}");
        }
    }

    #[test]
    fn default_page_contract_is_byte_frozen() {
        let mut b = fake();
        b.frozen_ids = true;
        let actual = serde_json::to_vec_pretty(&json!({
            "capabilities": call_tool(&mut b, CAPABILITIES, &json!({})).to_value(),
            "list_boards": call_tool(&mut b, LIST_BOARDS, &json!({})).to_value(),
        }))
        .unwrap();
        assert_eq!(actual, include_bytes!("../fixtures/pages-default.json"));
    }

    #[test]
    fn pages_are_opt_in_and_targets_travel_with_each_control_line() {
        let mut b = fake();
        let baseline = capabilities(true);
        assert_eq!(json_of(&call_tool(&mut b, CAPABILITIES, &json!({}))), baseline);
        let extended = json_of(&call_tool(&mut b, CAPABILITIES, &json!({"pages":true})));
        assert!(extended["pages"]["target"].is_string());
        let board = boards::board_id(1);
        let r = call_tool(&mut b, "set_paint", &json!({"board":board,"page":"page-2","fill":"#ff0000","ids":[4]}));
        assert!(!r.is_error, "{r:?}");
        assert!(b.seen.iter().all(|(_, params, t)| t == &Some(json!({"board":board,"page":"page-2"})) && params.get("page").is_none()));
    }

    #[test]
    fn an_id_of_board_a_on_board_b_is_wrong_board_and_nothing_is_sent() {
        let mut b = fake();
        let (a, other) = (boards::board_id(1), boards::board_id(2));
        let r = call_tool(&mut b, "set_paint", &json!({"board": other, "fill": "#ff0000", "ids": [format!("{a}:4")]}));
        assert!(r.is_error);
        assert!(r.content[0]["text"].as_str().unwrap().contains("wrong_board"), "{r:?}");
        assert!(b.seen.is_empty(), "nothing reached the app");
        // References alone name their board.
        let r = call_tool(&mut b, "set_paint", &json!({"fill": "#ff0000", "ids": [format!("{a}:4")]}));
        assert!(!r.is_error, "{r:?}");
        assert!(b.seen.iter().all(|(_, _, t)| t == &Some(json!({"board": a}))));
    }

    #[test]
    fn without_a_board_the_app_is_asked_with_none_and_says_board_required() {
        let mut b = fake();
        let r = call_tool(&mut b, "doc_inspect", &json!({}));
        assert!(r.is_error);
        assert!(r.content[0]["text"].as_str().unwrap().contains("board_required"), "{r:?}");
        assert_eq!(b.seen[0].2, None, "no board sent: never the one last named, never the tab in front");
        let r = call_tool(&mut b, "save_file", &json!({"revision": 3}));
        assert!(r.content[0]["text"].as_str().unwrap().contains("board_required"), "{r:?}");
    }

    #[test]
    fn capabilities_and_list_boards_tell_the_rule() {
        let mut b = fake();
        let caps = json_of(&call_tool(&mut b, CAPABILITIES, &json!({})));
        assert_eq!(caps["api"], boards::API);
        assert!(caps["rule"].as_str().unwrap().contains("board-scoped"));
        for code in ["board_required", "target_changed", "stale_revision", "wrong_board", "board_not_found", "session_reset", "person_busy"] {
            assert!(caps["errors"][code].is_string(), "{code}");
        }
        let list = json_of(&call_tool(&mut b, LIST_BOARDS, &json!({})));
        assert_eq!(list["boards"].as_array().unwrap().len(), 2);
        // Every tool takes board and revision; the Varos tools are listed.
        let defs = tool_definitions();
        for t in defs.iter().filter(|t| t["name"] != LIST_BOARDS && t["name"] != CAPABILITIES) {
            assert_eq!(t["inputSchema"]["properties"]["board"]["type"], "string", "{}", t["name"]);
        }
        assert!(defs.iter().any(|t| t["name"] == LIST_BOARDS) && defs.iter().any(|t| t["name"] == CAPABILITIES));
        let save = defs.iter().find(|t| t["name"] == "save_file").unwrap();
        assert!(save["description"].as_str().unwrap().contains("revision"));
    }

    /// Upstream's headless backend has no boards: a board named there is refused, not ignored.
    #[test]
    fn a_backend_without_boards_refuses_a_board() {
        let mut s = Server::new(Box::new(crate::Headless::with_document()));
        let r = crate::tools::call_tool(&mut crate::Headless::with_document(), "doc_inspect", &json!({}));
        assert!(!r.is_error);
        let line = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "doc_inspect", "arguments": {"board": boards::board_id(1)}}});
        let reply: Value = serde_json::from_str(&s.handle_line(&line.to_string()).unwrap()).unwrap();
        assert_eq!(reply["result"]["isError"], true, "{reply}");
        assert!(reply.to_string().contains("desktop Varos"), "{reply}");
    }

    /// Fix 7: a board's resources name their board in the URI and are read on that board only,
    /// with ids qualified; the old board-less form is refused (it never follows the board of the
    /// last tool call), and a reference of another board in the URI is wrong_board.
    #[test]
    fn resources_name_their_board() {
        let mut b = fake();
        let (a, other) = (boards::board_id(1), boards::board_id(2));
        let v = read_resource(&mut b, &format!("vectorcraft://board/{a}/object/42")).unwrap();
        assert_eq!(v["id"], json!(format!("{a}:9")), "ids qualified: {v}");
        assert_eq!(b.seen.last().unwrap().2, Some(json!({"board": a})));
        assert_eq!(b.target, None, "the board is not kept for later calls");
        // A reference of this board in the URI is fine; one of another board is not.
        assert!(read_resource(&mut b, &format!("vectorcraft://board/{a}/object/{a}%3A42")).is_ok());
        let seen = b.seen.len();
        let e = read_resource(&mut b, &format!("vectorcraft://board/{a}/object/{other}:42")).unwrap_err();
        assert!(matches!(&e, ReadError::Invalid(m) if m.starts_with("wrong_board")), "{e:?}");
        assert_eq!(b.seen.len(), seen, "nothing was sent");
        // A tool call on another board, then the old form: refused, never board B's object 42.
        let _ = call_tool(&mut b, "doc_inspect", &json!({"board": other}));
        let e = read_resource(&mut b, "vectorcraft://object/42").unwrap_err();
        assert!(
            matches!(&e, ReadError::Invalid(m) if m.starts_with("board_required") && m.contains("vectorcraft://board/<board>/object/42")),
            "{e:?}"
        );
        // The command catalogue needs no board.
        assert!(read_resource(&mut b, "vectorcraft://commands").is_ok());
        assert_eq!(b.seen.last().unwrap().2, None);
    }
}
