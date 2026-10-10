use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::{format::Limits, Editor};
struct TestHost {
    editor: Editor,
}
impl Host for TestHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Text".into(),
            rev: self.editor.rev,
            dirty: true,
            active: true,
            backing_file: None,
        }]
    }
    fn prepare(&mut self, _: &str, _: bool) -> Result<(), Error> {
        Ok(())
    }
    fn access(&mut self, _: &str) -> Result<BoardAccess<'_>, Error> {
        Ok(BoardAccess { editor: &mut self.editor, dirty: true })
    }
}
fn invoke(service: &mut Service, host: &mut TestHost, tool: &str, args: Value) -> Reply {
    let request = varos_bridge::mcp::decode_tool(tool, args).unwrap();
    service.handle(host, &Context { client: "text".into(), epoch: "test".into() }, request, &AtomicBool::new(false))
}
fn text() -> Value {
    let doc = varos_core::format::decode_model(
        include_bytes!("../../varos-core/tests/fixtures/text_next/mixed.json"),
        None,
        &Limits::DEFAULT,
    )
    .unwrap()
    .doc;
    serde_json::to_value(&doc.text_boxes[0]).unwrap()
}
#[test]
fn discover_add_describe_set_atomic_history_and_legacy_refusal() {
    let mut host = TestHost { editor: Editor::new() };
    let mut service = Service::new("test".into());
    let verbs = varos_bridge::mcp::list_verbs().to_string();
    assert!(verbs.contains("add_text") && verbs.contains("set_text"));
    for verb in ["add_text", "set_text"] {
        let schema = varos_bridge::mcp::schema("edit", Some(verb)).unwrap();
        assert!(schema.to_string().contains("line_height"));
    }
    let args = json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":0,"ops":[{"verb":"add_text","text":text(),"local":"$title"}]});
    let added = invoke(&mut service, &mut host, "edit", args.clone());
    assert!(added.ok, "{added:?}");
    assert_eq!(added.undo_steps, 1);
    assert_eq!(host.editor.doc.text_boxes.len(), 1);
    let described = invoke(&mut service, &mut host, "describe", json!({"api":"1.2","board":"b1","fields":["text"]}));
    assert!(described.ok, "{described:?}");
    let result = described.result.unwrap();
    let node = result["objects"].as_array().unwrap().iter().find(|v| v["kind"] == "text").unwrap()["id"].clone();
    let rev = host.editor.rev;
    let original = host.editor.doc.clone();
    let mut changed = text();
    changed["runs"][0]["text"] = json!("تحرير Text");
    let set = invoke(
        &mut service,
        &mut host,
        "edit",
        json!({"api":"1.2","board":"b1","request_id":"r2","expected_rev":rev,"ops":[{"verb":"set_text","node":node,"text":changed}]}),
    );
    assert!(set.ok, "{set:?}");
    host.editor.undo();
    assert_eq!(host.editor.doc.text_boxes, original.text_boxes);
    for api in ["1.0", "1.1"] {
        let mut args = args.clone();
        args["api"] = json!(api);
        args["request_id"] = json!(if api == "1.0" { "r3" } else { "r4" });
        args["expected_rev"] = json!(host.editor.rev);
        match varos_bridge::mcp::decode_tool("edit", args) {
            Err(_) => {}
            Ok(request) => {
                let reply = service.handle(
                    &mut host,
                    &Context { client: "text".into(), epoch: "test".into() },
                    request,
                    &AtomicBool::new(false),
                );
                assert!(!reply.ok);
            }
        }
    }
}

#[test]
fn typography_progressive_disclosure_api_gate_and_history() {
    let mut host = TestHost { editor: Editor::new() };
    let mut service = Service::new("test".into());
    let added = invoke(
        &mut service,
        &mut host,
        "edit",
        json!({"api":"1.2","board":"b1","request_id":"r10","expected_rev":0,"ops":[{"verb":"add_text","text":text()}]}),
    );
    assert!(added.ok, "{added:?}");
    let id = host.editor.doc.text_boxes[0].id;
    let schema = varos_bridge::mcp::schema("edit", Some("typography")).unwrap().to_string();
    assert!(schema.contains("define_character") && schema.contains("binding"));
    assert!(varos_bridge::mcp::list_verbs().to_string().contains("typography"));
    let args = json!({"api":"1.2","board":"b1","request_id":"r11","expected_rev":host.editor.rev,"ops":[{"verb":"typography","command":{"action":"features","text":id,"features":{"liga":1,"calt":1,"ss01":1}}}]});
    let reply = invoke(&mut service, &mut host, "edit", args.clone());
    assert!(reply.ok, "{reply:?}");
    assert_eq!(reply.undo_steps, 1);
    assert_eq!(host.editor.doc.typography.frames[&id].features["ss01"], 1);
    host.editor.undo();
    assert!(host.editor.doc.typography.is_empty());
    for (n, api) in ["1.0", "1.1"].iter().enumerate() {
        let mut args = args.clone();
        args["api"] = json!(api);
        args["expected_rev"] = json!(host.editor.rev);
        args["request_id"] = json!(format!("r{}", n + 12));
        if let Ok(request) = varos_bridge::mcp::decode_tool("edit", args) {
            let reply = service.handle(
                &mut host,
                &Context { client: "text".into(), epoch: "test".into() },
                request,
                &AtomicBool::new(false),
            );
            assert!(!reply.ok);
        }
    }
}
