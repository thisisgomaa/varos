//! Lane A opt-in discovery and atomic command parity on a headless host.
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::{model::ShapeKind, EditCommand, Editor};
struct TestHost(Editor);
impl Host for TestHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Paths".into(),
            rev: self.0.rev,
            dirty: true,
            active: true,
            backing_file: None,
        }]
    }
    fn prepare(&mut self, _: &str, _: bool) -> Result<(), Error> {
        Ok(())
    }
    fn access(&mut self, _: &str) -> Result<BoardAccess<'_>, Error> {
        Ok(BoardAccess { editor: &mut self.0, dirty: true })
    }
}
fn setup() -> (Service, TestHost, u32) {
    let mut ed = Editor::new();
    let id = ed
        .try_execute_created(EditCommand::AddShape {
            kind: ShapeKind::Rect,
            bounds: [0., 0., 100., 80.],
            parent: None,
            fill: Some([0., 1., 0., 1.]),
            stroke: Some([1., 0., 0., 1.]),
            stroke_width: 10.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    (Service::new("test".into()), TestHost(ed), id)
}
fn call(s: &mut Service, h: &mut TestHost, api: &str, ops: Value) -> Reply {
    let request = match varos_bridge::mcp::decode_tool(
        "edit",
        json!({"api":api,"board":"b1","request_id":format!("r{}",h.0.rev),"expected_rev":h.0.rev,"ops":ops}),
    ) {
        Ok(request) => request,
        Err(error) => return Reply::failure(error),
    };
    s.handle(h, &Context { client: "lane-a".into(), epoch: "test".into() }, request, &AtomicBool::new(false))
}
#[test]
fn discovery_projects_new_verbs_and_full_schema_requires_parameters() {
    for verb in ["appearance", "mask"] {
        let schema = varos_bridge::mcp::schema("edit", Some(verb)).unwrap();
        assert!(schema["properties"]["edit"]["oneOf"].as_array().unwrap().iter().all(|v| v["required"]
            .as_array()
            .unwrap()
            .len()
            > 1));
        assert!(varos_bridge::mcp::list_verbs().to_string().contains(verb));
        for api in ["1.0", "1.1"] {
            assert!(!varos_bridge::mcp::tools_for_api(api).to_string().contains(&format!("\"const\":\"{verb}\"")));
        }
    }
    let bytes = serde_json::to_vec(&varos_bridge::mcp::tools_for_api("1.2")).unwrap();
    assert!(bytes.len() <= 24000);
    println!("API 1.2: {} B", bytes.len());
}
#[test]
fn api12_edits_and_mask_gestures_are_atomic_and_undoable() {
    for api in ["1.0", "1.1", "1.2"] {
        let (mut s, mut h, id) = setup();
        let original = h.0.doc.clone();
        let reply = call(
            &mut s,
            &mut h,
            api,
            json!([{"verb":"appearance","edit":{"action":"add_fill","path":id,"paint":[0,0,1,1]}}]),
        );
        assert_eq!(reply.ok, api == "1.2", "{reply:?}");
        if reply.ok {
            assert_eq!(h.0.doc.paths[0].stack.len(), 3);
            h.0.execute(EditCommand::Undo).unwrap();
        }
        assert_eq!(h.0.doc, original);
        let node = h.0.doc.node_of_path(id).unwrap();
        let reply =
            call(&mut s, &mut h, api, json!([{"verb":"mask","edit":{"action":"begin","node":node,"alpha":true}}]));
        assert_eq!(reply.ok, api == "1.2", "{reply:?}");
        if reply.ok {
            assert!(h.0.doc.nodes.iter().any(|n| n.role == varos_core::model::GroupRole::MaskAlpha));
            h.0.execute(EditCommand::Undo).unwrap();
        }
        assert_eq!(h.0.doc, original);
    }
    let (mut s, mut h, id) = setup();
    let before = h.0.doc.clone();
    let reply = call(
        &mut s,
        &mut h,
        "1.2",
        json!([{"verb":"appearance","edit":{"action":"add_fill","path":id,"paint":[0,0,1,1]}},{"verb":"appearance","edit":{"action":"delete","path":id,"index":0}}]),
    );
    assert!(!reply.ok);
    assert_eq!(before, h.0.doc);
}
#[test]
fn describe_appearance_only_on_api12() {
    let (mut service, mut host, id) = setup();
    for api in ["1.0", "1.1", "1.2"] {
        let req = varos_bridge::mcp::decode_tool(
            "describe",
            json!({"api":api,"board":"b1","ids":[format!("path:{id}")],"fields":["appearance"]}),
        )
        .unwrap();
        let reply = service.handle(
            &mut host,
            &Context { client: "lane-a".into(), epoch: "test".into() },
            req,
            &AtomicBool::new(false),
        );
        assert_eq!(reply.ok, api == "1.2", "{reply:?}");
    }
}
