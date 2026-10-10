//! Lane C opt-in discovery and atomic command parity on a headless host.
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
    s.handle(h, &Context { client: "lane-c".into(), epoch: "test".into() }, request, &AtomicBool::new(false))
}
#[test]
fn colour_management_api_gating_discovery_atomicity_and_undo() {
    assert!(varos_bridge::mcp::schema("edit", Some("colour_management")).unwrap().to_string().contains("spot"));
    assert!(varos_bridge::mcp::list_verbs().to_string().contains("colour_management"));
    for api in ["1.0", "1.1", "1.2"] {
        let (mut s, mut h, id) = setup();
        let before = h.0.doc.clone();
        let reply = call(
            &mut s,
            &mut h,
            api,
            json!([{"verb":"colour_management","ids":[],"command":{"action":"mode","mode":"Cmyk"}}]),
        );
        assert_eq!(reply.ok, api == "1.2", "{reply:?}");
        if reply.ok {
            h.0.execute(EditCommand::Undo).unwrap();
        }
        assert!(h.0.doc.content_eq(&before));
        let reply = call(
            &mut s,
            &mut h,
            api,
            json!([{"verb":"colour_management","ids":[format!("path:{id}")],"command":{"action":"paint","target":"Fill","colour":{"colour":{"model":"gray","value":0.2},"alpha":1}}}]),
        );
        assert_eq!(reply.ok, api == "1.2", "{reply:?}");
    }
}
#[test]
fn invalid_profile_refuses_and_preview_is_view_only() {
    let (mut s, mut h, _) = setup();
    let before = h.0.doc.clone();
    let reply = call(
        &mut s,
        &mut h,
        "1.2",
        json!([{"verb":"colour_management","ids":[],"command":{"action":"profile","profile":{"name":"bad","data":"0001"}}}]),
    );
    assert!(!reply.ok);
    assert!(h.0.doc.content_eq(&before));
    let reply = call(
        &mut s,
        &mut h,
        "1.2",
        json!([{"verb":"colour_management","ids":[],"command":{"action":"overprint","enabled":true}}]),
    );
    assert!(reply.ok, "{reply:?}");
    assert!(h.0.colour_preview.overprint);
    assert!(h.0.doc.content_eq(&before));
}
