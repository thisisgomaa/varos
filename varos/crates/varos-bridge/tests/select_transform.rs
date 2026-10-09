//! API 1.2 integration, using a headless host.
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::{model::ShapeKind, EditCommand, Editor};
struct TestHost(Editor);
impl Host for TestHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Tools".into(),
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
fn setup() -> (Service, TestHost, u32, u32) {
    let mut e = Editor::new();
    let mut ids = vec![];
    for x in [0., 100.] {
        ids.push(
            e.try_execute_created(EditCommand::AddShape {
                kind: ShapeKind::Rect,
                bounds: [x, 0., 10., 20.],
                parent: None,
                fill: Some([1., 0., 0., 1.]),
                stroke: None,
                stroke_width: 1.,
                opacity: 1.,
                name: None,
            })
            .unwrap(),
        );
    }
    (Service::new("test".into()), TestHost(e), ids[0], ids[1])
}
fn call(s: &mut Service, h: &mut TestHost, api: &str, id: &str, ops: Value) -> Reply {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let rid = format!("r{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let _ = id;
    let request = match varos_bridge::mcp::decode_tool(
        "edit",
        json!({"api":api,"board":"b1","request_id":rid,"expected_rev":h.0.rev,"ops":ops}),
    ) {
        Ok(r) => r,
        Err(e) => return Reply::failure(e),
    };
    s.handle(h, &Context { client: "tools-test".into(), epoch: "test".into() }, request, &AtomicBool::new(false))
}
#[test]
fn transform_opt_in_atomic_copy_geometry() {
    for api in ["1.0", "1.1", "1.2"] {
        let (mut s, mut h, a, _) = setup();
        let before = h.0.doc.clone();
        let r = call(
            &mut s,
            &mut h,
            api,
            "transform",
            json!([{"verb":"transform","ids":[format!("path:{a}")],"spec":{"reflect":90,"origin":[0,0],"copy":true}}]),
        );
        assert_eq!(r.ok, api == "1.2", "{r:?}");
        if api == "1.2" {
            assert_eq!(h.0.doc.paths.len(), 3);
            assert!(h.0.doc.paths.iter().any(|p| p.anchors[1].p[0] < -9.));
            h.0.undo();
        }
        assert_eq!(h.0.doc, before);
    }
}
#[test]
fn wand_eyedropper_isolation_and_layer_verbs() {
    let (mut s, mut h, a, b) = setup();
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "wand",
        json!([{"verb":"magic_wand","ids":[format!("path:{a}")],"options":{},"mode":"set"}]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(h.0.objsel.len(), 2);
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "subtract",
        json!([{"verb":"magic_wand","ids":[format!("path:{a}")],"options":{},"mode":"subtract"}]),
    );
    assert!(r.ok, "{r:?}");
    assert!(h.0.objsel.is_empty());
    let i = h.0.doc.pidx(b).unwrap();
    h.0.doc.paths[i].opacity = 0.5;
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "sample",
        json!([{"verb":"eyedropper","ids":[format!("path:{a}")],"source":format!("path:{b}"),"options":{"opacity":true},"colour_only":false}]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(h.0.doc.paths[h.0.doc.pidx(a).unwrap()].opacity, 0.5);
    h.0.execute(EditCommand::SelectPaths(vec![a, b]));
    h.0.execute(EditCommand::GroupSelection);
    let group = h.0.doc.top_group_of_path(a).unwrap();
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "isolate",
        json!([{"verb":"isolation","ids":[format!("node:{group}")],"exit":false}]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(h.0.select_transform.isolation, Some(group));
    let r = call(&mut s, &mut h, "1.2", "exit", json!([{"verb":"isolation","ids":[],"exit":true}]));
    assert!(r.ok, "{r:?}");
    assert!(h.0.select_transform.isolation.is_none());
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "collect",
        json!([{"verb":"layers","ids":[format!("node:{group}")],"action":"collect"}]),
    );
    assert!(r.ok, "{r:?}");
    assert!(h.0.doc.roots.iter().any(|n| h.0.doc.node_paths(*n).len() == 2));
}
#[test]
fn invalid_new_operation_rolls_back_batch() {
    let (mut s, mut h, a, _) = setup();
    let before = h.0.doc.clone();
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "invalid",
        json!([{"verb":"transform","ids":[format!("path:{a}")],"spec":{"angle":30}},{"verb":"transform","ids":[format!("path:{a}")],"spec":{"shear":90}}]),
    );
    assert!(!r.ok);
    assert_eq!(h.0.doc, before);
}
#[test]
fn capabilities_12_advertise_opt_in_only() {
    let (mut s, mut h, _, _) = setup();
    let request = varos_bridge::mcp::decode_tool("capabilities", json!({"api":"1.2"})).unwrap();
    let r = s.handle(
        &mut h,
        &Context { client: "tools-test".into(), epoch: "test".into() },
        request,
        &AtomicBool::new(false),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(
        r.result.unwrap()["slice4a_verbs"],
        json!(["transform", "magic_wand", "eyedropper", "isolation", "layers", "tool_options"])
    );
}

#[test]
fn option_verb_is_opt_in_and_publishes_preferences_without_undo() {
    let (mut s, mut h, _, _) = setup();
    let rev = h.0.rev;
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "options",
        json!([{"verb":"tool_options","wand":{"weight":3},"eyedropper":{"fill":false,"opacity":true}}]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(h.0.select_transform.wand.weight, 3.);
    assert!(!h.0.select_transform.pick.fill);
    assert_eq!(h.0.rev, rev);
}

#[test]
fn new_verbs_resolve_request_local_ids_in_same_atomic_batch() {
    let (mut s, mut h, _, _) = setup();
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        "locals",
        json!([{"verb":"add_shape","kind":"rect","bounds":[0,0,10,20],"fill":"#FF0000FF","local":"$new"},{"verb":"transform","ids":["$new"],"spec":{"movement":[50,0]}}]),
    );
    assert!(r.ok, "{r:?}");
    assert!(h.0.doc.paths.iter().any(|p| p.anchors[0].p == [50., 0.]));
}
