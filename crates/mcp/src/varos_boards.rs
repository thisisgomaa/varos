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
use crate::tools::{ToolResult, call_tool as upstream_call_tool};

const LIST_BOARDS: &str = "list_boards";
const CAPABILITIES: &str = "capabilities";

/// Tools that act on the window (synthetic input, captures, menus): their board must be the one
/// in front, else `target_changed`.
const FRONT_ONLY_TOOLS: &[&str] = &["pointer_gesture", "press_key", "type_text", "ui_screenshot", "invoke_menu"];

/// Tools that need no board: the catalogue, the lists, and opening a file (the new board is in
/// the reply's `varos.opened`).
const BOARD_FREE_TOOLS: &[&str] =
    &[LIST_BOARDS, CAPABILITIES, "command_list", "list_commands", "ui_inspect", "inspect_ui", "select_tool", "open_panel", "open_file"];

/// Tools whose description says what saving or exporting needs.
const WRITERS: &[&str] = &["save_file", "export", "screenshot"];

const RULE: &str = "All verbs are board-scoped: pass `board` (a stable id from list_boards) with every call that reads or changes a document, and `revision` too when saving or exporting (save_file, export, command_run file.save / file.saveAs / document.export*, screenshot or render to a path). Varos never falls back to the tab in front. Object ids come back as \"<board>:<id>\" references; pass them as they are. Every reply carries `varos: {board, revision, front, opened}`: use that revision next. The person's tab is never switched: work on a background board runs there and the person keeps their view.";

/// Add `board` and `revision` to every tool and append the Varos tools (`tool_definitions`).
pub(crate) fn extend(defs: &mut Vec<Value>) {
    for t in defs.iter_mut() {
        let name = t["name"].as_str().unwrap_or("").to_string();
        if let Some(props) = t.get_mut("inputSchema").and_then(|s| s.get_mut("properties")).and_then(Value::as_object_mut) {
            props.insert("board".into(), board_prop(&name));
            props.insert("revision".into(), revision_prop(&name));
        }
        if WRITERS.contains(&name.as_str())
            && let Some(Value::String(d)) = t.get_mut("description")
        {
            d.push_str(" In Varos pass `board` and its current `revision` (list_boards or the last reply): a board that changed since you read it is never written (stale_revision).");
        }
        if FRONT_ONLY_TOOLS.contains(&name.as_str())
            && let Some(Value::String(d)) = t.get_mut("description")
        {
            d.push_str(" In Varos this acts on the window: its `board` must be the one in front, else target_changed.");
        }
    }
    let empty = json!({"type": "object", "properties": {}, "additionalProperties": false});
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
        "boardFree": BOARD_FREE_TOOLS,
        "errors": errors,
    })
}

/// Run one tool with its board: `board` / `revision` taken out of the arguments and sent with
/// every line it makes; references in, references out ([`boards::qualify`]).
pub fn call_tool(b: &mut dyn Backend, name: &str, args: &Value) -> ToolResult {
    if name == CAPABILITIES {
        return ToolResult::json(&capabilities(b.board_scoped()));
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
        let r = b.call(boards::LIST, json!({}));
        let _ = b.set_target(None);
        return match r {
            Ok(v) => ToolResult::json(&v),
            Err(e) => ToolResult::error(format!("{name}: {e}")),
        };
    }
    let mut args = Value::Object(args);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Server, tool_definitions};

    /// A stand-in for Varos with two boards: it records every line's method, params and target,
    /// answers inspect / create calls with plain ids, and says where each ran.
    struct FakeVaros {
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
                boards::LIST => {
                    Ok(json!({"boards": [{"board": boards::board_id(1), "front": true}, {"board": boards::board_id(2), "front": false}]}))
                }
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
        FakeVaros { target: None, seen: vec![], last: Value::Null }
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
}
