//! Lane E API 1.2 view commands, discovery and atomicity.
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Request, Service};
use varos_core::{editor::Editor, model::Artboard};

struct FakeHost {
    editor: Editor,
}
impl FakeHost {
    /// An empty new board (no pages), as `new_board` makes it.
    #[allow(dead_code)]
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
fn view_depth_discovery_and_staged_publication() {
    let schema = varos_bridge::mcp::schema("edit", Some("view")).unwrap();
    assert!(schema.to_string().contains("navigator_zoom"));
    assert!(schema.to_string().contains("canvas_color"));
    assert!(varos_bridge::mcp::list_verbs().to_string().contains("view"));
    let mut h = FakeHost::two_pages();
    let mut service = Service::new("test-epoch".into());
    for (i, action) in [
        json!("outline"),
        json!("pixel_preview"),
        json!("trim"),
        json!("presentation"),
        json!("exit_presentation"),
        json!("snap_pixel"),
        json!("move_whole_pixel"),
        json!({"navigator_pan":{"center":[10,20]}}),
        json!({"navigator_zoom":{"percent":600}}),
        json!({"canvas_color":{"rgb":[20,19,19]}}),
        json!("transparency_grid"),
    ]
    .into_iter()
    .enumerate()
    {
        let rev = h.editor.rev;
        let r = invoke(
            &mut service,
            &mut h,
            "edit",
            json!({"api":"1.2","board":"b1","request_id":format!("r{}",i+1),"expected_rev":rev,"ops":[{"verb":"view","ids":[],"action":{"depth":action}}]}),
        );
        assert!(r.ok, "{r:?}");
    }
    assert!(h.editor.view_depth.outline && h.editor.view_depth.pixel_preview && h.editor.view_depth.trim);
    assert!(!h.editor.view_depth.presentation);
    assert!(h.editor.doc.snap.force_pixel_align && h.editor.doc.snap.move_whole_px);
    assert_eq!(h.editor.requested_pan, Some([10.0, 20.0]));
    assert_eq!(h.editor.requested_zoom, Some(600.0));
    assert_eq!(h.editor.requested_canvas, Some([20, 19, 19]));
}
#[test]
fn refused_batch_preserves_modes() {
    let mut h = FakeHost::two_pages();
    let mut service = Service::new("test-epoch".into());
    let rev = h.editor.rev;
    let r = invoke(
        &mut service,
        &mut h,
        "edit",
        json!({"api":"1.2","board":"b1","request_id":"r99","expected_rev":rev,"ops":[{"verb":"view","ids":[],"action":{"depth":"outline"}},{"verb":"view","ids":[],"action":{"depth":{"navigator_zoom":{"percent":0}}}}]}),
    );
    assert!(!r.ok);
    assert!(!h.editor.view_depth.outline);
}
