use serde_json::json;
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Service};
use varos_core::{
    model::{Anchor, Path},
    Editor,
};
struct TestHost(Editor);
impl Host for TestHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Effects".into(),
            rev: self.0.rev,
            dirty: true,
            active: true,
            backing_file: None,
        }]
    }
    fn prepare(&mut self, board: &str, _mutation: bool) -> Result<(), Error> {
        if board == "b1" {
            Ok(())
        } else {
            Err(Error::new("not_found", "board"))
        }
    }
    fn access(&mut self, _board: &str) -> Result<BoardAccess<'_>, Error> {
        Ok(BoardAccess { editor: &mut self.0, dirty: true })
    }
}
fn host() -> TestHost {
    let mut e = Editor::new();
    e.doc.paths.push(Path::new(
        10,
        [[20., 20.], [120., 20.], [120., 100.], [20., 100.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: 11 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([0., 0.5, 1., 1.]),
        Some([1., 0., 0., 1.]),
        10.,
    ));
    e.doc.ids = 30;
    e.doc.sync_tree();
    TestHost(e)
}
#[test]
fn effects_are_progressively_discovered_and_cap_is_preserved() {
    let list = varos_bridge::mcp::tools_for_api("1.2");
    let bytes = serde_json::to_vec(&list).unwrap().len();
    assert!(bytes <= 24000, "API 1.2 {bytes} B");
    let verbs = varos_bridge::mcp::list_verbs().to_string();
    for verb in ["live_effects", "width_profile", "expand_live", "width_tool"] {
        assert!(verbs.contains(verb));
        let schema = varos_bridge::mcp::schema("edit", Some(verb)).unwrap();
        assert!(schema.to_string().contains(verb));
    }
    let edit = list["tools"].as_array().unwrap().iter().find(|row| row["name"] == "edit").unwrap();
    assert!(!edit["description"].as_str().unwrap().contains("live_effects"));
    for api in ["1.0", "1.1"] {
        assert!(!varos_bridge::mcp::tools_for_api(api).to_string().contains("live_effects"));
    }
}
#[test]
fn bridge_live_effects_width_expand_and_tool_are_undoable_or_transient() {
    let mut h = host();
    let mut service = Service::new("test".into());
    let ctx = Context { client: "effects-test".into(), epoch: "test".into() };
    for (i,op) in [
 json!({"verb":"live_effects","ids":["path:10"],"effects":[{"type":"offset","delta":5,"join":"Miter","miter":10}]}),
 json!({"verb":"width_profile","ids":["path:10"],"profile":{"points":[[0,0,0],[0.5,1,1],[1,0,0]]}}),
 json!({"verb":"expand_live","ids":["path:10"]}),
 json!({"verb":"width_tool"})
 ].into_iter().enumerate() {
 let request=varos_bridge::mcp::decode_tool("edit",json!({"api":"1.2","board":"b1","request_id":format!("r{}",i+1),"expected_rev":h.0.rev,"ops":[op]})).unwrap();
 let reply=service.handle(&mut h,&ctx,request,&AtomicBool::new(false));assert!(reply.error.is_none(),"{:?}",reply.error);
 }
    assert!(h.0.doc.paths[0].effects.is_empty());
    assert!(h.0.doc.paths[0].stroke_style.width_profile.is_some());
    assert!(h.0.tool == varos_core::ToolKind::Width);
    h.0.undo();
    assert!(!h.0.doc.paths[0].effects.is_empty());
}
#[test]
fn legacy_apis_refuse_effect_verbs_and_invalid_batch_is_atomic() {
    let effect = json!({"verb":"live_effects","ids":["path:10"],"effects":[{"type":"offset","delta":5,"join":"Miter","miter":10}]});
    for api in ["1.0", "1.1"] {
        assert!(varos_bridge::mcp::decode_tool(
            "edit",
            json!({"api":api,"board":"b1","request_id":"r1","expected_rev":0,"ops":[effect.clone()]})
        )
        .is_err());
    }
    let mut h = host();
    let doc = h.0.doc.clone();
    let ctx = Context { client: "effects-test".into(), epoch: "test".into() };
    let mut service = Service::new("test".into());
    let request=varos_bridge::mcp::decode_tool("edit",json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":0,"ops":[{"verb":"width_tool"},effect,{"verb":"width_profile","ids":["path:10"],"profile":{"points":[[0,1,1],[1,-1,1]]}}]})).unwrap();
    let reply = service.handle(&mut h, &ctx, request, &AtomicBool::new(false));
    assert!(reply.error.is_some());
    assert_eq!(h.0.doc, doc);
    assert!(h.0.tool == varos_core::ToolKind::Object);
}
