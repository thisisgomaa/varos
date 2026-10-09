//! Phase 1 API 1.2 setup/info opt-in; headless host.
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Request, Service};
use varos_core::{editor::Editor, model::Artboard};

struct FakeHost {
    editor: Editor,
}
impl FakeHost {
    /// An empty new board (no pages), as `new_board` makes it.
    fn empty() -> Self {
        let mut editor = Editor::new();
        editor.replace_doc(varos_core::board::new_board());
        Self { editor }
    }
    /// Two 100×100 pages A (x 0) and B (x 200) with stable ids, and a 20×20 square on A.
    fn two_pages() -> Self {
        let mut doc = varos_core::board::new_board();
        for (x, name) in [(0.0, "A"), (200.0, "B")] {
            let id = doc.nid();
            doc.artboards.push(Artboard {
                id,
                x,
                y: 0.0,
                w: 100.0,
                h: 100.0,
                name: name.into(),
                ..Artboard::default()
            });
        }
        let a =
            |id: u32, x: f32, y: f32| varos_core::model::Anchor { id, p: [x, y], hin: None, hout: None, smooth: false };
        doc.paths.push(varos_core::model::Path::new(
            10,
            vec![a(11, 10.0, 10.0), a(12, 30.0, 10.0), a(13, 30.0, 30.0), a(14, 10.0, 30.0)],
            true,
            Some([1.0, 0.0, 0.0, 1.0]),
            None,
            0.0,
        ));
        doc.ids = 20;
        let mut editor = Editor::new();
        editor.replace_doc(doc);
        Self { editor }
    }
}
impl Host for FakeHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: String::new(),
            rev: self.editor.rev,
            dirty: true,
            active: true,
            backing_file: None,
        }]
    }
    fn prepare(&mut self, board: &str, _: bool) -> Result<(), Error> {
        if board != "b1" {
            return Err(Error::new("not_found", "closed"));
        }
        Ok(())
    }
    fn access(&mut self, board: &str) -> Result<BoardAccess<'_>, Error> {
        if board != "b1" {
            return Err(Error::new("not_found", "closed"));
        }
        Ok(BoardAccess { editor: &mut self.editor, dirty: true })
    }
}
fn ctx(_destructive: bool) -> Context {
    Context { client: "fixture".into(), epoch: "test-epoch".into() }
}
fn req(tool: &str, arguments: Value) -> Request {
    varos_bridge::mcp::decode_tool(tool, arguments).unwrap()
}

fn invoke(s: &mut Service, h: &mut FakeHost, tool: &str, args: Value) -> Reply {
    s.handle(h, &ctx(false), req(tool, args), &AtomicBool::new(false))
}
#[test]
fn setup_opt_in_invalid_atomic_and_history() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let rev = h.editor.rev;
    let args = json!({"api":"1.2","board":"b1","expected_rev":rev,"request_id":"r1","ops":[{"verb":"document_setup","field":"ppi","value":300}]});
    let mut legacy = args.clone();
    legacy["api"] = json!("1.1");
    assert!(varos_bridge::mcp::decode_tool("edit", legacy).is_err());
    let reply = invoke(&mut s, &mut h, "edit", args);
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(h.editor.doc.units.ppi, 300.0);
    h.editor.execute(varos_core::EditCommand::Undo).unwrap();
    assert_eq!(h.editor.doc.units.ppi, 72.0);
    let rev = h.editor.rev;
    let reply = invoke(
        &mut s,
        &mut h,
        "edit",
        json!({"api":"1.2","board":"b1","expected_rev":rev,"request_id":"r2","ops":[{"verb":"document_setup","field":"ppi","value":144},{"verb":"document_setup","field":"ppi","value":0}]}),
    );
    assert!(!reply.ok);
    assert_eq!(h.editor.doc.units.ppi, 72.0);
    let page = h.editor.doc.artboards[0].id;
    let rev = h.editor.rev;
    let reply = invoke(
        &mut s,
        &mut h,
        "edit",
        json!({"api":"1.2","board":"b1","expected_rev":rev,"request_id":"r3","ops":[{"verb":"document_setup","field":"bleed","artboard":format!("artboard:{page}"),"value":[1,2,3,4]}]}),
    );
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(varos_core::document_setup::bleed(&h.editor.doc.artboards[0]), [1.0, 2.0, 3.0, 4.0]);
}
#[test]
fn document_info_matches_legacy_counts() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let old = invoke(&mut s, &mut h, "describe", json!({"api":"1.0","board":"b1"}));
    let info = invoke(&mut s, &mut h, "describe", json!({"api":"1.2","board":"b1","fields":["document_info"]}));
    assert!(info.ok, "{:?}", info.error);
    assert_eq!(info.result.as_ref().unwrap()["counts"], old.result.as_ref().unwrap()["counts"]);
    assert_eq!(info.result.unwrap()["colours"].as_array().unwrap().len(), 2);
}
#[test]
fn schemas_are_opt_in() {
    assert_eq!(
        varos_bridge::mcp::tools_12()["tools"].as_array().unwrap().len(),
        varos_bridge::mcp::tools()["tools"].as_array().unwrap().len() + 11
    );
    let schema = varos_bridge::mcp::tools_12();
    let tools = schema["tools"].as_array().unwrap();
    let edit = tools.iter().find(|t| t["name"] == "edit").unwrap();
    assert!(edit["inputSchema"]["$defs"].get("document_setup").is_none());
    let setup = varos_bridge::mcp::schema("edit", Some("document_setup")).unwrap();
    assert_eq!(setup["required"], json!(["field", "value"]));
    let legacy = varos_bridge::mcp::tools();
    let legacy_edit = legacy["tools"].as_array().unwrap().iter().find(|t| t["name"] == "edit").unwrap();
    assert!(legacy_edit["inputSchema"]["$defs"].get("document_setup").is_none());
    for name in ["save_template", "new_from_template"] {
        let t = tools.iter().find(|t| t["name"] == name).unwrap();
        assert_eq!(t["inputSchema"]["properties"]["api"]["const"], "1.2");
        assert_eq!(t["inputSchema"]["required"], json!(["api", "board", "request_id", "expected_rev", "path"]));
    }
    let mut h = FakeHost::empty();
    let capabilities = invoke(&mut Service::new("test-epoch".into()), &mut h, "capabilities", json!({"api":"1.2"}));
    assert!(capabilities.ok);
    let caps = capabilities.result.unwrap();
    for tool in ["export_svg", "export_raster", "save_template", "new_from_template", "window_memory"] {
        assert_eq!(caps["tools"].as_array().unwrap().iter().filter(|v| **v == tool).count(), 1);
        assert_eq!(caps["api_by_tool"][tool], json!(["1.2"]));
    }
    assert_eq!(caps["edit_verbs"].as_array().unwrap().iter().filter(|v| **v == "document_setup").count(), 1);
}

#[test]
fn units_only_publishes_once_and_noop_has_no_history() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let rev = h.editor.rev;
    let args = |rev, id| json!({"api":"1.2","board":"b1","expected_rev":rev,"request_id":id,"ops":[{"verb":"document_setup","field":"units","value":"mm"}]});
    assert!(invoke(&mut s, &mut h, "edit", args(rev, "r1")).ok);
    assert_eq!(h.editor.doc.units.display, varos_core::Unit::Mm);
    assert_eq!(h.editor.rev, rev + 1);
    assert!(invoke(&mut s, &mut h, "edit", args(rev + 1, "r2")).ok);
    assert_eq!(h.editor.rev, rev + 1);
    h.editor.undo();
    assert_eq!(h.editor.doc.units.display, varos_core::Unit::Px);
    assert!(!h.editor.history_available(false));
    h.editor.redo();
    assert_eq!(h.editor.doc.units.display, varos_core::Unit::Mm);
}
