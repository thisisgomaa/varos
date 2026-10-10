//! Headless API 1.2 creation/options/source-isolation/release/expand/spine contracts.
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::{
    model::{NodeKind, ShapeKind},
    EditCommand, Editor,
};
struct TestHost(Editor);
impl Host for TestHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Live".into(),
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
fn setup() -> (Service, TestHost, Vec<String>) {
    let mut ed = Editor::new();
    let mut ids = Vec::new();
    for x in [0., 100.] {
        ids.push(format!(
            "path:{}",
            ed.try_execute_created(EditCommand::AddShape {
                kind: ShapeKind::Rect,
                bounds: [x, 0., 10., 10.],
                parent: None,
                fill: Some([1., 0., 0., 1.]),
                stroke: None,
                stroke_width: 0.,
                opacity: 1.,
                name: None
            })
            .unwrap()
        ));
    }
    (Service::new("test".into()), TestHost(ed), ids)
}
fn call(s: &mut Service, h: &mut TestHost, api: &str, ops: Value) -> Reply {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let req = varos_bridge::mcp::decode_tool(
        "edit",
        json!({"api":api,"board":"b1","request_id":format!("r{}",NEXT.fetch_add(1,Ordering::Relaxed)+1),"expected_rev":h.0.rev,"ops":ops}),
    );
    match req {
        Err(e) => Reply::failure(e),
        Ok(req) => {
            s.handle(h, &Context { client: "lane-e".into(), epoch: "test".into() }, req, &AtomicBool::new(false))
        }
    }
}
#[test]
fn discovery_is_complete_and_adds_zero_inline_bytes() {
    let table = varos_bridge::mcp::tools_for_api("1.2");
    let bytes = table.to_string();
    assert!(bytes.len() <= 24_000, "{}", bytes.len());
    let listed = varos_bridge::mcp::list_verbs().to_string();
    println!("API 1.2 tools/list={} B; list_verbs={} B", bytes.len(), listed.len());
    assert!(listed.len() < 16_000);
    for verb in ["live_make", "live_options", "live_release", "live_expand", "live_isolate", "live_spine"] {
        assert!(varos_bridge::mcp::schema("edit", Some(verb)).unwrap().to_string().contains(verb));
        assert!(varos_bridge::mcp::list_verbs().to_string().contains(verb));
        assert!(!bytes.contains(&format!("\"{verb}\"")));
        for api in ["1.0", "1.1"] {
            assert!(!varos_bridge::mcp::tools_for_api(api).to_string().contains(verb));
        }
    }
}
#[test]
fn wire_behaviours_are_atomic_undoable_and_api12_only() {
    for api in ["1.0", "1.1", "1.2"] {
        let (mut s, mut h, ids) = setup();
        let before = h.0.doc.clone();
        let kind = json!({"effect":"blend","spine":null,"steps":2,"orientation":"page"});
        let r = call(&mut s, &mut h, api, json!([{"verb":"live_make","ids":ids,"kind":kind}]));
        if api != "1.2" {
            assert!(!r.ok);
            assert_eq!(h.0.doc, before);
            continue;
        }
        assert!(r.ok, "{r:?}");
        let n = h.0.doc.nodes.iter().find(|n| matches!(n.kind, NodeKind::Live(_))).unwrap().id;
        let node = format!("node:{n}");
        let r = call(
            &mut s,
            &mut h,
            api,
            json!([{"verb":"live_options","node":node,"kind":{"effect":"blend","spine":null,"steps":5,"orientation":"page"}}]),
        );
        assert!(r.ok, "{r:?}");
        let opts = h.0.doc.clone();
        let r = call(&mut s, &mut h, api, json!([{"verb":"live_isolate","node":node}]));
        assert!(r.ok, "{r:?}");
        assert_eq!(h.0.select_transform.isolation, Some(n));
        assert_eq!(h.0.doc, opts);
        let r = call(&mut s, &mut h, api, json!([{"verb":"live_isolate","node":null}]));
        assert!(r.ok, "{r:?}");
        let r = call(&mut s, &mut h, api, json!([{"verb":"live_expand","node":node}]));
        assert!(r.ok, "{r:?}");
        assert_eq!(h.0.doc.paths.len(), 7);
        h.0.execute(EditCommand::Undo).unwrap();
        assert_eq!(h.0.doc, opts);
        let r = call(&mut s, &mut h, api, json!([{"verb":"live_release","node":node}]));
        assert!(r.ok, "{r:?}");
        assert_eq!(h.0.doc.node(n).unwrap().kind, NodeKind::Group);
        h.0.execute(EditCommand::Undo).unwrap();
        let snapshot = h.0.doc.clone();
        let depth = h.0.history_depths();
        let r = call(
            &mut s,
            &mut h,
            api,
            json!([{"verb":"live_options","node":node,"kind":{"effect":"blend","spine":null,"steps":4,"orientation":"page"}},{"verb":"live_expand","node":"node:999999"}]),
        );
        assert!(!r.ok);
        assert_eq!(h.0.doc, snapshot);
        assert_eq!(h.0.history_depths(), depth);
    }
}
#[test]
fn wire_spine_is_owned_and_unknown_options_refuse() {
    let (mut s, mut h, ids) = setup();
    assert!(call(&mut s,&mut h,"1.2",json!([{"verb":"live_make","ids":ids,"kind":{"effect":"blend","spine":null,"steps":2,"orientation":"page"}}])).ok);
    let n = h.0.doc.nodes.iter().find(|n| matches!(n.kind, NodeKind::Live(_))).unwrap().id;
    let mut p = h.0.doc.paths[0].clone();
    p.id = h.0.doc.nid();
    p.closed = false;
    p.anchors.truncate(2);
    for a in &mut p.anchors {
        a.id = h.0.doc.nid();
    }
    let pid = p.id;
    h.0.doc.paths.push(p);
    h.0.doc.sync_tree();
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        json!([{"verb":"live_spine","node":format!("node:{n}"),"path":format!("path:{pid}")}]),
    );
    assert!(r.ok, "{r:?}");
    assert!(h.0.doc.node(n).unwrap().children.contains(&h.0.doc.node_of_path(pid).unwrap()));
    assert!(varos_bridge::mcp::decode_tool("edit",json!({"api":"1.2","board":"b1","request_id":"unknown","expected_rev":h.0.rev,"ops":[{"verb":"live_release","node":format!("node:{n}"),"extra":1}]})).is_err());
}
