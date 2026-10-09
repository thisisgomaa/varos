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
fn each_verb_disclosed_only_in_api12() {
    for verb in ["outline_stroke", "offset_path", "expand", "live_corners", "scale_strokes", "new_document"] {
        let schema = varos_bridge::mcp::schema("edit", Some(verb)).unwrap();
        assert!(schema.to_string().contains(verb));
        assert!(varos_bridge::mcp::list_verbs().to_string().contains(verb));
        for api in ["1.0", "1.1"] {
            assert!(!varos_bridge::mcp::tools_for_api(api).to_string().contains(&format!("\"const\":\"{verb}\"")));
        }
    }
    assert!(varos_bridge::mcp::schema("export_screens", None).is_ok());
}
#[test]
fn geometry_and_corners_legacy_refused_atomic_and_undo() {
    for verb in ["outline_stroke", "offset_path", "expand", "live_corners"] {
        for api in ["1.0", "1.1", "1.2"] {
            let (mut s, mut h, id) = setup();
            let before = h.0.doc.clone();
            let mut op = json!({"verb":verb,"ids":[format!("path:{id}")]});
            if verb == "offset_path" {
                op["delta"] = json!(10);
                op["join"] = json!("Miter");
                op["miter"] = json!(10);
            }
            if verb == "live_corners" {
                op["corners"] = json!(vec![json!({"radius":10,"kind":"round"}); 4]);
            }
            let reply = call(&mut s, &mut h, api, json!([op]));
            assert_eq!(reply.ok, api == "1.2", "{reply:?}");
            if api == "1.2" {
                assert_ne!(h.0.doc, before);
                h.0.execute(EditCommand::Undo).unwrap();
            }
            assert_eq!(h.0.doc, before);
        }
    }
}
#[test]
fn new_document_and_scale_strokes_bridge_commands() {
    let (mut s, mut h, id) = setup();
    let reply = call(
        &mut s,
        &mut h,
        "1.2",
        json!([{"verb":"scale_strokes","enabled":true},{"verb":"transform","ids":[format!("path:{id}")],"spec":{"scale":[2,2]}}]),
    );
    assert!(reply.ok, "{reply:?}");
    assert_eq!(h.0.doc.paths[0].stroke_width, 20.);
    let reply = call(&mut s, &mut h, "1.2", json!([{"verb":"new_document","settings":{"count":3,"bleed":3}}]));
    assert!(reply.ok, "{reply:?}");
    assert_eq!(h.0.doc.artboards.len(), 3);
}
#[test]
fn export_screens_alias_options_and_legacy_refusals() {
    let opts = json!({"screens":{"rows":[{"format":"png","scale":1,"suffix":""},{"format":"svg","svg":{"minify":true}}],"prefix":"icon-","subfolders":"format"}});
    let req=varos_bridge::mcp::decode_tool("export_screens",json!({"api":"1.2","board":"b1","request_id":"r","expected_rev":0,"path":"/tmp/export.png","scope":"all_visible_artboards","options":opts})).unwrap();
    assert!(matches!(req, varos_bridge::Request::ExportRaster(_)));
    for api in ["1.0", "1.1"] {
        assert!(varos_bridge::mcp::decode_tool("export_screens",json!({"api":api,"board":"b1","request_id":"r","expected_rev":0,"path":"/tmp/export.png","scope":"all_visible_artboards","options":opts})).is_err());
    }
}
