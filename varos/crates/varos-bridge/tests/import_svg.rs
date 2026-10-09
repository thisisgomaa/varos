use serde_json::json;
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::{EditCommand, Editor};
struct ImportHost {
    editor: Editor,
    calls: usize,
}
impl Host for ImportHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Import".into(),
            rev: self.editor.rev,
            dirty: false,
            active: true,
            backing_file: None,
        }]
    }
    fn prepare(&mut self, _: &str, _: bool) -> Result<(), Error> {
        Ok(())
    }
    fn access(&mut self, _: &str) -> Result<BoardAccess<'_>, Error> {
        Ok(BoardAccess { editor: &mut self.editor, dirty: false })
    }
    fn file_effect(&mut self, verb: &str, request: &varos_bridge::dto::FileEffect) -> Result<Reply, Error> {
        assert_eq!(verb, "import_svg");
        assert_eq!(request.path.as_deref(), Some("/allowed/art.svg"));
        self.calls += 1;
        let mut artwork = Editor::new();
        artwork.execute_ui(EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Rect,
            bounds: [0., 0., 20., 20.],
            parent: None,
            fill: Some([1., 0., 0., 1.]),
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        });
        self.editor.execute_ui(EditCommand::PlaceArtwork(Box::new(artwork.doc)));
        Ok(Reply::success(json!({"rev":self.editor.rev,"report":{"paths":1,"loss_notes":["Text omitted"]}})))
    }
}
#[test]
fn import_is_12_only_revision_pinned_and_idempotent() {
    let mut host = ImportHost { editor: Editor::new(), calls: 0 };
    let mut service = Service::new("test".into());
    let context = Context { epoch: "test".into(), client: "test".into() };
    let cancel = AtomicBool::new(false);
    for api in ["1.0", "1.1"] {
        let req = varos_bridge::mcp::decode_tool(
            "import_svg",
            json!({"api":api,"board":"b1","expected_rev":host.editor.rev,"request_id":"r1","path":"/allowed/art.svg"}),
        )
        .unwrap();
        assert!(!service.handle(&mut host, &context, req, &cancel).ok);
    }
    assert_eq!(host.calls, 0);
    let req = varos_bridge::mcp::decode_tool(
        "import_svg",
        json!({"api":"1.2","board":"b1","expected_rev":host.editor.rev,"request_id":"r1","path":"/allowed/art.svg"}),
    )
    .unwrap();
    let before = host.editor.rev;
    let reply = service.handle(&mut host, &context, req.clone(), &cancel);
    assert!(reply.ok, "{reply:?}");
    assert_eq!(reply.result.as_ref().unwrap()["report"]["paths"], 1);
    assert_eq!(reply.rev, Some(host.editor.rev));
    assert_ne!(reply.rev, Some(before));
    assert_eq!(reply.undo_steps, 1);
    let result = reply.result.as_ref().unwrap();
    assert_eq!(result["rev"], host.editor.rev);
    assert_eq!(result["from"], before);
    assert_eq!(result["changed_document"], true);
    assert!(!result["created"].as_array().unwrap().is_empty());
    assert!(result["selection_count"].as_u64().unwrap() > 0);
    assert_eq!(
        serde_json::to_vec(&reply).unwrap(),
        serde_json::to_vec(&service.handle(&mut host, &context, req, &cancel)).unwrap()
    );
    assert_eq!(host.calls, 1);
    let status = varos_bridge::mcp::decode_tool("request_status", json!({"request_id":"r1"})).unwrap();
    let retained = service.handle(&mut host, &context, status, &cancel);
    assert_eq!(retained.result.as_ref().unwrap()["receipt"], serde_json::to_value(&reply).unwrap());
    let next = varos_bridge::mcp::decode_tool(
        "import_svg",
        json!({"api":"1.2","board":"b1","expected_rev":reply.rev,"request_id":"r2","path":"/allowed/art.svg"}),
    )
    .unwrap();
    let next_reply = service.handle(&mut host, &context, next, &cancel);
    assert!(next_reply.ok, "{next_reply:?}");
    assert_eq!(next_reply.rev, Some(host.editor.rev));
    assert_eq!(next_reply.undo_steps, 1);
    assert_eq!(host.calls, 2);
    assert_eq!(host.editor.doc.paths.len(), 2);
    host.editor.execute_ui(EditCommand::Undo);
    assert_eq!(host.editor.doc.paths.len(), 1);
    host.editor.execute_ui(EditCommand::Undo);
    assert!(host.editor.doc.paths.is_empty());
    let stale = varos_bridge::mcp::decode_tool(
        "import_svg",
        json!({"api":"1.2","board":"b1","expected_rev":999,"request_id":"r3","path":"/allowed/art.svg"}),
    )
    .unwrap();
    assert!(!service.handle(&mut host, &context, stale, &cancel).ok);
    assert_eq!(host.calls, 2);
}
#[test]
fn capability_optin_does_not_advertise_import_to_legacy_clients() {
    for api in ["1.0", "1.1", "1.2"] {
        let mut host = ImportHost { editor: Editor::new(), calls: 0 };
        let mut service = Service::new("test".into());
        let ctx = Context { epoch: "test".into(), client: "test".into() };
        let req = varos_bridge::mcp::decode_tool("capabilities", json!({"api":api})).unwrap();
        let reply = service.handle(&mut host, &ctx, req, &AtomicBool::new(false));
        assert!(reply.ok, "{reply:?}");
        assert_eq!(reply.result.unwrap()["tools"].as_array().unwrap().iter().any(|v| v == "import_svg"), api == "1.2");
    }
}
#[test]
fn sources_outside_files_scope_are_refused_before_reading() {
    for path in ["/tmp/a.svg", "/System/a.svg", "/Applications/a.svg", "relative.svg"] {
        assert!(varos_bridge::files::read_source(std::path::Path::new(path), "svg", 100).is_err());
    }
}
