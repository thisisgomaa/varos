use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::Editor;
struct H(Editor);
impl Host for H {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Drawing".into(),
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
fn call(s: &mut Service, h: &mut H, api: &str, ops: Value) -> Reply {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    let request = varos_bridge::mcp::decode_tool(
        "edit",
        json!({"api":api,"board":"b1","request_id":format!("r{}",NEXT.fetch_add(1,Ordering::Relaxed)),"expected_rev":h.0.rev,"ops":ops}),
    );
    let request = match request {
        Ok(r) => r,
        Err(e) => return Reply::failure(e),
    };
    s.handle(h, &Context { client: "drawing-test".into(), epoch: "d".into() }, request, &AtomicBool::new(false))
}
#[test]
fn all_drawing_verbs_have_progressive_schemas_and_old_apis_refuse() {
    let names = ["shape_tool", "pencil", "smooth_path", "path_erase", "join_tool", "curvature", "drawing_options"];
    let list = varos_bridge::mcp::list_verbs().to_string();
    for name in names {
        assert!(list.contains(name));
        let schema = varos_bridge::mcp::schema("edit", Some(name)).unwrap();
        assert!(schema.is_object());
        assert!(schema.to_string().contains(name));
    }
    for api in ["1.0", "1.1"] {
        let mut h = H(Editor::new());
        let before = h.0.doc.clone();
        let mut s = Service::new("d".into());
        let r = call(&mut s, &mut h, api, json!([{"verb":"shape_tool","spec":{"kind":"star"}}]));
        assert!(!r.ok);
        assert_eq!(h.0.doc, before);
    }
}
#[test]
fn shapes_pencil_curvature_options_receipts_and_one_shared_undo() {
    let mut h = H(Editor::new());
    let before = h.0.doc.clone();
    let mut s = Service::new("d".into());
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        json!([
            {"verb":"shape_tool","spec":{"kind":"rectangular_grid","rows":2,"columns":3}},
            {"verb":"pencil","points":[[300,0],[350,20],[400,0]],"options":{}},
            {"verb":"curvature","points":[[500,0],[550,20],[600,0]],"closed":false},
            {"verb":"drawing_options","options":{"fidelity":3,"smoothness":0.8}}
        ]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(h.0.doc.paths.len(), 9);
    assert_eq!(h.0.rev, 1);
    assert_eq!(h.0.drawing.options.fidelity, 3.);
    h.0.undo();
    assert_eq!(h.0.doc, before);
}
#[test]
fn eraser_uses_explicit_ids_and_atomic_invalid_batch_rolls_back() {
    let mut h = H(Editor::new());
    let mut s = Service::new("d".into());
    let r = call(&mut s, &mut h, "1.2", json!([{"verb":"shape_tool","spec":{"kind":"line","size":[100,0]}}]));
    assert!(r.ok, "{r:?}");
    let id = h.0.doc.paths[0].id;
    let before = h.0.doc.clone();
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        json!([
            {"verb":"path_erase","ids":[format!("path:{id}")],"points":[[50,-10],[50,10]],"options":{"brush_radius":5}},
            {"verb":"pencil","points":[[100,100]],"options":{}}
        ]),
    );
    assert!(!r.ok);
    assert_eq!(h.0.doc, before);
    let r = call(
        &mut s,
        &mut h,
        "1.2",
        json!([{"verb":"path_erase","ids":[format!("path:{id}")],"points":[[50,-10],[50,10]],"options":{"brush_radius":5}}]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(h.0.doc.paths.len(), 2);
}
