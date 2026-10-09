//! Bridge slice 3: artboards as persistent `artboard:N` identities — page verbs, the active-artboard
//! rule, id-addressed align and page snapshots. Headless fake host; no GPU renderer or event loop.
use base64::Engine;
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Request, Service};
use varos_core::{editor::Editor, model::Artboard};

struct FakeHost {
    editor: Editor,
}
impl FakeHost {
    /// An empty new board (no pages), as `new_board` makes it.
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
fn call(s: &mut Service, h: &mut FakeHost, tool: &str, arguments: Value) -> Reply {
    s.handle(h, &ctx(false), req(tool, arguments), &AtomicBool::new(false))
}
fn edit(s: &mut Service, h: &mut FakeHost, id: &str, rev: u64, ops: Value) -> Reply {
    call(s, h, "edit", json!({"api":"1.0","request_id":id,"board":"b1","expected_rev":rev,"ops":ops}))
}
fn ids(h: &FakeHost) -> Vec<u32> {
    h.editor.doc.artboards.iter().map(|a| a.id).collect()
}
fn png_size(reply: &Reply) -> (u32, u32) {
    let png = base64::engine::general_purpose::STANDARD
        .decode(reply.result.as_ref().unwrap()["png"].as_str().unwrap())
        .unwrap();
    let reader = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
    (reader.info().width, reader.info().height)
}

/// The owner's ask: "make an Instagram Story artboard and design on it" — one request, one undo step,
/// identical through the CLI decoder and the MCP binding, frozen as the README example.
#[test]
fn instagram_story_batch_is_one_undo_step_with_mcp_cli_parity() {
    let arguments: Value = serde_json::from_str(include_str!("fixtures/story-request-1.0.json")).unwrap();
    let cli =
        varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":"edit","arguments":arguments})).unwrap()).unwrap();
    let (mut h, mut m) = (FakeHost::empty(), FakeHost::empty());
    let (mut s, mut ms) = (Service::new("test-epoch".into()), Service::new("test-epoch".into()));
    let r = s.handle(&mut h, &ctx(false), cli, &AtomicBool::new(false));
    let mr = ms.handle(&mut m, &ctx(false), req("edit", arguments), &AtomicBool::new(false));
    assert!(r.ok, "{r:?}");
    assert_eq!(r, mr, "CLI and MCP decode to the same receipt");
    let mcp = varos_bridge::mcp::tool_result(&mr);
    assert_eq!(mcp["structuredContent"], json!(r));
    assert_eq!(varos_bridge::service::compact(&r), mcp["content"][0]["text"]);
    if std::env::var_os("VAROS_BLESS_BRIDGE_FIXTURES").is_some() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/story-result-1.0.json");
        std::fs::write(path, serde_json::to_string_pretty(&json!(r)).unwrap() + "\n").unwrap();
    }
    assert_eq!(json!(r), serde_json::from_str::<Value>(include_str!("fixtures/story-result-1.0.json")).unwrap());
    assert_eq!(r.undo_steps, 1);
    let result = r.result.as_ref().unwrap();
    let story = result["locals"]["$story"].as_str().unwrap();
    assert_eq!(result["artboards_created"], json!([story]));
    assert_eq!(result["active_artboard"], json!(story));
    let id: u32 = story.strip_prefix("artboard:").unwrap().parse().unwrap();
    let page = &h.editor.doc.artboards[0];
    assert_eq!(
        (page.id, page.name.as_str(), page.x, page.y, page.w, page.h),
        (id, "Instagram Story", 0.0, 0.0, 1080.0, 1920.0)
    );
    // the sun is centred on the page
    let sun: u32 = result["locals"]["$sun"].as_str().unwrap().strip_prefix("path:").unwrap().parse().unwrap();
    let b = h.editor.doc.outline_bbox(h.editor.doc.pidx(sun).unwrap());
    assert!(((b.0 + b.2) / 2.0 - 540.0).abs() < 0.01 && ((b.1 + b.3) / 2.0 - 960.0).abs() < 0.01, "{b:?}");
    let group: u32 = result["locals"]["$design"].as_str().unwrap().strip_prefix("node:").unwrap().parse().unwrap();
    assert_eq!(h.editor.doc.node(group).unwrap().name, "Story design");
    assert_eq!(h.editor.doc.path_boards(h.editor.doc.pidx(sun).unwrap()), vec![0], "the art stands on the page");

    // a snapshot of that page alone is the page's 9:16 shape, not the fitted board preview
    let snap = call(&mut s, &mut h, "snapshot", json!({"board":"b1","rev":2,"artboard":story}));
    assert!(snap.ok, "{snap:?}");
    assert_eq!(snap.undo_steps, 0);
    assert_eq!(png_size(&snap), (576, 1024));
    assert_eq!(snap.result.as_ref().unwrap()["artboard"], json!(story));
    let small =
        call(&mut s, &mut h, "snapshot", json!({"board":"b1","rev":2,"artboard":story,"width":300,"height":300}));
    assert_eq!(png_size(&small), (169, 300));
    if let Some(path) = std::env::var_os("VAROS_BRIDGE_STORY_PNG") {
        let png = base64::engine::general_purpose::STANDARD
            .decode(snap.result.as_ref().unwrap()["png"].as_str().unwrap())
            .unwrap();
        std::fs::write(path, png).unwrap();
    }

    // one human undo removes the page AND the design; redo restores the same page id
    h.editor.undo();
    assert!(h.editor.doc.artboards.is_empty() && h.editor.doc.paths.is_empty());
    assert!(!h.editor.history_available(false));
    h.editor.redo();
    assert_eq!(ids(&h), vec![id]);
}

#[test]
fn describe_reports_persistent_ids_the_deprecated_alias_and_the_active_page() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let summary = call(&mut s, &mut h, "describe", json!({"board":"b1"}));
    let v = summary.result.unwrap();
    assert_eq!(v["active_artboard"], "artboard:2");
    assert_eq!(v["artboards"][1]["id"], "artboard:3");
    assert_eq!(v["artboards"][1]["ref"], "a1@1", "the revision-bound alias stays for one slice");
    let detail = call(&mut s, &mut h, "describe", json!({"board":"b1","fields":["artboards"]})).result.unwrap();
    assert_eq!(detail["artboards"][0]["id"], "artboard:2");
    // human edits never renumber: rename B, the id stays
    h.editor.execute_ui(varos_core::EditCommand::RenameArtboard { index: 1, name: "Cover".into() });
    let v = call(&mut s, &mut h, "describe", json!({"board":"b1","fields":["artboards"]})).result.unwrap();
    assert_eq!(
        (v["artboards"][1]["id"].as_str(), v["artboards"][1]["name"].as_str()),
        (Some("artboard:3"), Some("Cover"))
    );
}

#[test]
fn page_verbs_succeed_by_id_and_each_batch_is_one_step() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let r = edit(
        &mut s,
        &mut h,
        "r1",
        1,
        json!([
            {"verb":"add_artboard","bounds":[400,0,300,500],"local":"$c"},
            {"verb":"add_artboard","preset":"a4"},
            {"verb":"resize_artboard","id":"artboard:3","bounds":[200,10,150,120]},
            {"verb":"rename_artboard","id":"$c","name":"  Third  "}
        ]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(r.undo_steps, 1);
    let d = &h.editor.doc;
    assert_eq!(d.artboards.len(), 4);
    assert_eq!((d.artboards[1].x, d.artboards[1].y, d.artboards[1].w, d.artboards[1].h), (200.0, 10.0, 150.0, 120.0));
    assert_eq!(d.artboards[2].name, "Third", "names are cleaned like object names");
    assert_eq!((d.artboards[3].w, d.artboards[3].h), (595.0, 842.0));
    assert_eq!(d.artboards[3].x, 760.0, "a preset without origin goes right of the right-most page (700 + 60)");
    assert_eq!(d.artboards[3].name, "Artboard 4");
    assert_eq!(d.active, 0, "adding pages never changes the human's active page");
    let created = r.result.as_ref().unwrap()["artboards_created"].as_array().unwrap().len();
    assert_eq!(created, 2);
    // set_active alone is navigation: no revision, no undo step, but describe and the receipt show it
    let rev = h.editor.rev;
    let r = edit(&mut s, &mut h, "r2", rev, json!([{"verb":"set_active_artboard","id":"artboard:3"}]));
    assert!(r.ok, "{r:?}");
    assert_eq!((r.undo_steps, h.editor.rev, h.editor.doc.active), (0, rev, 1));
    assert_eq!(r.result.as_ref().unwrap()["active_artboard"], "artboard:3");
    assert_eq!(h.editor.absel.iter().copied().collect::<Vec<_>>(), vec![1]);
    let v = call(&mut s, &mut h, "describe", json!({"board":"b1"})).result.unwrap();
    assert_eq!(v["active_artboard"], "artboard:3");
}

#[test]
fn every_page_verb_refuses_with_a_typed_error_at_its_index_and_rolls_back() {
    let cases: Vec<(Value, &str)> = vec![
        (json!({"verb":"add_artboard","bounds":[0,0,10,10],"preset":"story"}), "invalid_argument"),
        (json!({"verb":"add_artboard"}), "invalid_argument"),
        (json!({"verb":"add_artboard","bounds":[0,0,10,10],"origin":[0,0]}), "invalid_argument"),
        (json!({"verb":"add_artboard","bounds":[0,0,0.5,10]}), "invalid_argument"),
        (json!({"verb":"add_artboard","bounds":[1e38,0,3e38,10]}), "invalid_argument"),
        (json!({"verb":"add_artboard","bounds":[0,0,10,10],"name":"   "}), "invalid_argument"),
        (json!({"verb":"add_artboard","bounds":[0,0,10,10],"local":"$new"}), "invalid_argument"), // duplicate local
        (json!({"verb":"resize_artboard","id":"artboard:99","bounds":[0,0,10,10]}), "not_found"),
        (json!({"verb":"resize_artboard","id":"artboard:03","bounds":[0,0,10,10]}), "invalid_argument"),
        (json!({"verb":"resize_artboard","id":"path:10","bounds":[0,0,10,10]}), "invalid_argument"),
        (json!({"verb":"resize_artboard","id":"$missing","bounds":[0,0,10,10]}), "not_found"),
        (json!({"verb":"rename_artboard","id":"artboard:3","name":""}), "invalid_argument"),
        (json!({"verb":"delete_artboard","id":"artboard:2"}), "invalid_argument"), // the active page
        (json!({"verb":"set_active_artboard","id":"artboard:0"}), "invalid_argument"),
        (json!({"verb":"move","ids":["$new"],"delta":[1,1]}), "invalid_argument"), // a page is not an object
    ];
    for (op, code) in cases {
        let mut h = FakeHost::two_pages();
        let mut s = Service::new("test-epoch".into());
        let before = h.editor.doc.clone();
        let floor = h.editor.doc.ids;
        // op 0 succeeds in the stage, op 1 fails: nothing at all is published
        let r =
            edit(&mut s, &mut h, "r1", 1, json!([{"verb":"add_artboard","bounds":[0,300,50,50],"local":"$new"}, op]));
        let e = r.error.unwrap_or_else(|| panic!("{op}: expected {code}"));
        assert_eq!((e.code.as_str(), e.op_index), (code, Some(1)), "{op}: {}", e.reason);
        assert_eq!(h.editor.doc, before, "{op}: rolled back");
        assert_eq!((h.editor.rev, h.editor.doc.ids, r.undo_steps), (1, floor, 0), "{op}");
        assert!(!h.editor.history_available(false), "{op}");
    }
    // a locked page cannot be resized or deleted; renaming it is fine
    let mut h = FakeHost::two_pages();
    h.editor.doc.artboards[1].locked = true;
    let mut s = Service::new("test-epoch".into());
    for (i, op) in [json!({"verb":"resize_artboard","id":"artboard:3","bounds":[0,0,10,10]})].into_iter().enumerate() {
        let r = edit(&mut s, &mut h, &format!("r{}", i + 1), 1, json!([op]));
        assert_eq!(r.error.unwrap().code, "locked_target");
    }
    assert!(edit(&mut s, &mut h, "r1", 1, json!([{"verb":"rename_artboard","id":"artboard:3","name":"L"}])).ok);
    // the format's 1,000-page cap
    let mut h = FakeHost::empty();
    for i in 0..1000 {
        h.editor.doc.artboards.push(Artboard {
            id: 10_000 + i,
            x: i as f32 * 20.0,
            w: 10.0,
            h: 10.0,
            ..Artboard::default()
        });
    }
    h.editor.doc.ids = 20_000;
    let mut s = Service::new("test-epoch".into());
    let r = edit(&mut s, &mut h, "r1", 1, json!([{"verb":"add_artboard","bounds":[0,0,10,10]}]));
    assert_eq!(r.error.unwrap().code, "limit_exceeded");
}

#[test]
fn deleting_pages_follows_the_active_rule_without_a_grant() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    // Non-active page B is deleted directly; artwork remains and undo restores it.
    let ops = json!([{"verb":"delete_artboard","id":"artboard:3"}]);
    let args = json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":ops});
    let r = s.handle(&mut h, &ctx(false), req("edit", args), &AtomicBool::new(false));
    assert!(r.ok, "{r:?}");
    assert_eq!(ids(&h), vec![2]);
    assert_eq!(h.editor.doc.paths.len(), 1, "the page's artwork stays (as the panel's delete)");
    assert_eq!(r.result.as_ref().unwrap()["artboards_removed"], json!(["artboard:3"]));

    // the active page: refused alone, accepted after an explicit set_active in the same batch
    let mut h = FakeHost::two_pages();
    h.editor.absel.insert(0);
    let mut s = Service::new("test-epoch".into());
    let ops = json!([{"verb":"set_active_artboard","id":"artboard:3"},{"verb":"delete_artboard","id":"artboard:2"}]);
    let args = json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":ops});
    let r = s.handle(&mut h, &ctx(true), req("edit", args), &AtomicBool::new(false));
    assert!(r.ok, "{r:?}");
    assert_eq!((ids(&h), h.editor.doc.active), (vec![3], 0), "B is now first and active");
    assert_eq!(h.editor.absel.iter().copied().collect::<Vec<_>>(), vec![0], "human page selection re-pointed by id");
    assert_eq!(r.undo_steps, 1);
    h.editor.undo();
    assert_eq!((ids(&h), h.editor.doc.active), (vec![2, 3], 0), "undo restores the page and the old active");

    // the last page: deleting it leaves a free canvas
    let mut h = FakeHost::empty();
    let mut s = Service::new("test-epoch".into());
    assert!(edit(&mut s, &mut h, "r1", 1, json!([{"verb":"add_artboard","preset":"square"}])).ok);
    let ops = json!([{"verb":"delete_artboard","id":"artboard:2"}]);
    let args = json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":2,"ops":ops});
    assert!(s.handle(&mut h, &ctx(true), req("edit", args), &AtomicBool::new(false)).ok);
    assert!(h.editor.doc.artboards.is_empty() && h.editor.doc.active == 0);
    varos_core::format::validate(&h.editor.doc, &varos_core::format::Limits::DEFAULT).unwrap();
}

#[test]
fn new_page_ids_are_never_reused_after_undo() {
    let mut h = FakeHost::empty();
    let mut s = Service::new("test-epoch".into());
    let r = edit(&mut s, &mut h, "r1", 1, json!([{"verb":"add_artboard","preset":"square","local":"$a"}]));
    let first = r.result.unwrap()["locals"]["$a"].clone();
    h.editor.undo();
    let rev = h.editor.rev;
    let r = edit(&mut s, &mut h, "r2", rev, json!([{"verb":"add_artboard","preset":"square","local":"$a"}]));
    let second = r.result.unwrap()["locals"]["$a"].clone();
    assert_ne!(first, second, "high-water allocation: {first} vs {second}");
}

#[test]
fn align_targets_pages_by_id_local_or_the_deprecated_alias() {
    for target in ["artboard:3", "a1@1"] {
        let mut h = FakeHost::two_pages();
        let mut s = Service::new("test-epoch".into());
        let r =
            edit(&mut s, &mut h, "r1", 1, json!([{"verb":"align","ids":["path:10"],"mode":"left","target":target}]));
        assert!(r.ok, "{target}: {r:?}");
        let b = h.editor.doc.outline_bbox(0);
        assert!((b.0 - 200.0).abs() < 1e-3, "{target}: aligned to page B's left edge, got {b:?}");
        assert_eq!(h.editor.doc.active, 0, "{target}: the active page is untouched");
    }
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let r = edit(
        &mut s,
        &mut h,
        "r1",
        1,
        json!([{"verb":"add_artboard","bounds":[500,500,100,100],"local":"$p"},{"verb":"align","ids":["path:10"],"mode":"bottom","target":"$p"}]),
    );
    assert!(r.ok, "{r:?}");
    assert!((h.editor.doc.outline_bbox(0).3 - 600.0).abs() < 1e-3);
    for (target, code) in [("artboard:99", "not_found"), ("a0@7", "revision_conflict"), ("$nope", "not_found")] {
        let mut h = FakeHost::two_pages();
        let mut s = Service::new("test-epoch".into());
        let r =
            edit(&mut s, &mut h, "r1", 1, json!([{"verb":"align","ids":["path:10"],"mode":"left","target":target}]));
        assert_eq!(r.error.unwrap().code, code, "{target}");
    }
}

/// Review P2: with pages A/B/C (C active), `delete_artboard A` would shift the stage so the deprecated
/// `a1@rev` (B at that revision) indexes C while its revision check still passes. Any batch mixing
/// page verbs with the alias is refused up front, naming the alias op; the stable id has no such hole.
#[test]
fn legacy_alias_is_refused_in_batches_with_page_verbs() {
    let three = || {
        let mut h = FakeHost::two_pages();
        let id = h.editor.doc.nid();
        h.editor.doc.artboards.push(Artboard {
            id,
            x: 400.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
            name: "C".into(),
            ..Artboard::default()
        });
        h.editor.doc.active = 2;
        h
    };
    let mut h = three();
    let before = h.editor.doc.clone();
    let mut s = Service::new("test-epoch".into());
    let ops = json!([
        {"verb":"delete_artboard","id":"artboard:2"},
        {"verb":"align","ids":["path:10"],"mode":"left","target":"a1@1"}
    ]);
    let args = json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":ops});
    // refused before staging or the destructive challenge, even with the owner's grant enabled
    let r = s.handle(&mut h, &ctx(true), req("edit", args), &AtomicBool::new(false));
    let e = r.error.unwrap();
    assert_eq!((e.code.as_str(), e.op_index), ("invalid_argument", Some(1)), "{}", e.reason);
    assert!(e.reason.contains("artboard:N"), "{}", e.reason);
    assert!(e.digest.is_none(), "no confirmation is offered for a refused batch");
    assert_eq!(h.editor.doc, before);
    assert_eq!((h.editor.rev, r.undo_steps), (1, 0));
    // any page verb counts, in either order
    let r = edit(
        &mut s,
        &mut h,
        "r1",
        1,
        json!([{"verb":"align","ids":["path:10"],"mode":"left","target":"a1@1"},{"verb":"rename_artboard","id":"artboard:3","name":"B2"}]),
    );
    assert_eq!(r.error.unwrap().op_index, Some(0));
    // the same intent with the stable id aligns to B, not C
    let mut h = three();
    let mut s = Service::new("test-epoch".into());
    let ops = json!([
        {"verb":"delete_artboard","id":"artboard:2"},
        {"verb":"align","ids":["path:10"],"mode":"left","target":"artboard:3"}
    ]);
    let args = json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":ops});
    let r = s.handle(&mut h, &ctx(true), req("edit", args), &AtomicBool::new(false));
    assert!(r.ok, "{r:?}");
    assert!((h.editor.doc.outline_bbox(0).0 - 200.0).abs() < 1e-3, "aligned to B (x 200), not C (x 400)");
    assert_eq!(ids(&h).len(), 2);
    assert_eq!(ids(&h)[0], 3, "A (artboard:2) removed, B kept");
    assert_eq!(h.editor.doc.artboards[h.editor.doc.active].name, "C", "C stays active");
}

#[test]
fn page_snapshots_refuse_unknown_or_malformed_pages() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let ok =
        call(&mut s, &mut h, "snapshot", json!({"board":"b1","rev":1,"artboard":"artboard:3","width":64,"height":64}));
    assert_eq!(png_size(&ok), (64, 64));
    let board = call(&mut s, &mut h, "snapshot", json!({"board":"b1","rev":1}));
    assert_eq!(png_size(&board), (544, 246), "without a page: the slice-2 board preview, unchanged");
    let missing = call(&mut s, &mut h, "snapshot", json!({"board":"b1","rev":1,"artboard":"artboard:99"}));
    assert_eq!(missing.error.unwrap().code, "not_found");
    let bad = varos_bridge::mcp::decode_tool("snapshot", json!({"board":"b1","rev":1,"artboard":"a0@1"})).unwrap();
    let r = s.handle(&mut h, &ctx(false), bad, &AtomicBool::new(false));
    assert_eq!(r.error.unwrap().code, "invalid_argument");
}

#[test]
fn capabilities_and_mcp_schemas_advertise_every_page_verb() {
    let mut h = FakeHost::empty();
    let mut s = Service::new("test-epoch".into());
    let caps = call(&mut s, &mut h, "capabilities", json!({"api":"1.0"})).result.unwrap();
    for verb in ["add_artboard", "resize_artboard", "rename_artboard", "delete_artboard", "set_active_artboard"] {
        assert!(caps["edit_verbs"].as_array().unwrap().contains(&json!(verb)), "{verb}");
    }
    assert_eq!(caps["writable_vrs"], json!([4]));
    assert_eq!(caps["readable_vrs"], json!([1, 2, 3, 4]));
    assert_eq!(caps["artboard_presets"]["story"], json!([1080, 1920]));
    assert_eq!(caps["limits"]["geometry_page_bytes"], 16 * 1024);
    assert_eq!(caps["limits"]["geometry_typical_anchors_per_page"], 300);
    assert_eq!(caps["limits"]["geometry_anchor_pagination"], false);
    assert!(caps["deprecated"]["aN@rev"].is_string());
    let tools = varos_bridge::mcp::tools();
    let edit = tools["tools"].as_array().unwrap().iter().find(|t| t["name"] == "edit").unwrap();
    let verbs: Vec<_> = edit["inputSchema"]["$defs"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(|s| s["properties"]["verb"]["const"].as_str().map(str::to_owned))
        .collect();
    for verb in varos_bridge::EDIT_VERBS {
        assert!(verbs.contains(&verb.to_string()), "schema for {verb}");
    }
    let snapshot = tools["tools"].as_array().unwrap().iter().find(|t| t["name"] == "snapshot").unwrap();
    assert!(snapshot["inputSchema"]["properties"]["artboard"].is_object());
    // unknown fields stay refused on the new verbs too
    let e = varos_bridge::mcp::decode_tool(
        "edit",
        json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"delete_artboard","id":"artboard:2","confirm":true}]}),
    )
    .unwrap_err();
    assert_eq!((e.code.as_str(), e.op_index), ("invalid_argument", Some(0)));
}

#[test]
fn slice4_reorder_duplicate_color_clip_and_rollback() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let source = ids(&h)[0];
    let other = ids(&h)[1];
    let before = h.editor.doc.clone();
    let rev = h.editor.rev;
    let r = edit(
        &mut s,
        &mut h,
        "r1",
        rev,
        json!([
            {"verb":"duplicate_artboard","id":format!("artboard:{source}"),"with_art":true,"offset":[400,50],"local":"$copy"},
            {"verb":"reorder_artboard","id":"$copy","position":0},
            {"verb":"set_artboard_color","id":"$copy","color":"#00FF0080"},
            {"verb":"set_artboard_clip","id":"$copy","clip":false}
        ]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(r.undo_steps, 1);
    let copy = &h.editor.doc.artboards[0];
    let new_id = copy.id;
    assert_ne!(new_id, source);
    assert_ne!(new_id, other);
    assert_eq!(copy.x, 400.0);
    assert!(!copy.clip);
    assert!(copy.page_color.unwrap()[1] > 0.99);
    assert_eq!(h.editor.doc.active_artboard().unwrap().id, source);
    assert_eq!(h.editor.doc.paths.len(), 2);
    let cloned = h.editor.doc.paths.last().unwrap();
    let pid = cloned.id;
    assert_eq!(cloned.anchors[0].p, [410.0, 60.0]);
    assert_ne!(pid, 10);
    h.editor.undo();
    assert!(h.editor.doc.content_eq(&before));
    for invalid in [
        json!({"verb":"reorder_artboard","id":format!("artboard:{source}"),"position":9}),
        json!({"verb":"duplicate_artboard","id":"artboard:9999","with_art":true}),
        json!({"verb":"duplicate_artboard","id":format!("artboard:{source}"),"with_art":true,"offset":[3.4e38,3.4e38]}),
        json!({"verb":"set_artboard_color","id":format!("artboard:{source}"),"color":"bad"}),
        json!({"verb":"set_artboard_clip","id":"artboard:9999","clip":false}),
    ] {
        let rev = h.editor.rev;
        let r = edit(
            &mut s,
            &mut h,
            "r2",
            rev,
            json!([{"verb":"set_artboard_clip","id":format!("artboard:{source}"),"clip":false},invalid]),
        );
        assert!(!r.ok, "{r:?}");
        assert_eq!(r.error.unwrap().op_index, Some(1));
        assert!(h.editor.doc.content_eq(&before));
    }
    let rev = h.editor.rev;
    let r = edit(
        &mut s,
        &mut h,
        "r2",
        rev,
        json!([{"verb":"duplicate_artboard","id":format!("artboard:{source}"),"with_art":false}]),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(h.editor.doc.paths.len(), 1);
    assert!(h.editor.doc.artboards.iter().map(|a| a.id).max().unwrap() > new_id.max(pid));
}

#[test]
fn duplicate_group_and_clip_allocates_fresh_tree_ids() {
    for clipped in [false, true] {
        let mut h = FakeHost::two_pages();
        let source = ids(&h)[0];
        h.editor
            .try_execute_created(varos_core::EditCommand::AddShape {
                kind: varos_core::model::ShapeKind::Rect,
                bounds: [10.0, 10.0, 25.0, 25.0],
                parent: None,
                fill: Some([1.0; 4]),
                stroke: None,
                stroke_width: 0.0,
                opacity: 1.0,
                name: None,
            })
            .unwrap();
        h.editor.objsel = h.editor.doc.paths.iter().map(|p| p.id).collect();
        h.editor.group_selection();
        let group = h.editor.doc.nodes.iter_mut().find(|n| n.kind == varos_core::model::NodeKind::Group).unwrap();
        let group_id = group.id;
        if clipped {
            group.role = varos_core::model::GroupRole::Clip;
            group.mask_child = Some(group.children[0]);
        }
        let old_nodes: std::collections::HashSet<_> = h.editor.doc.nodes.iter().map(|n| n.id).collect();
        let old_paths: std::collections::HashSet<_> = h.editor.doc.paths.iter().map(|p| p.id).collect();
        let rev = h.editor.rev;
        let mut s = Service::new("test-epoch".into());
        let r = edit(
            &mut s,
            &mut h,
            "r1",
            rev,
            json!([{"verb":"duplicate_artboard","id":format!("artboard:{source}"),"with_art":true,"offset":null}]),
        );
        assert!(r.ok, "{r:?}");
        let copy = h
            .editor
            .doc
            .nodes
            .iter()
            .find(|n| n.kind == varos_core::model::NodeKind::Group && n.id != group_id)
            .unwrap();
        assert!(!old_nodes.contains(&copy.id));
        assert!(copy.children.iter().all(|id| !old_nodes.contains(id)));
        assert_eq!(copy.role == varos_core::model::GroupRole::Clip, clipped);
        if clipped {
            assert!(copy.children.contains(&copy.mask_child.unwrap()));
            assert!(!old_nodes.contains(&copy.mask_child.unwrap()));
        }
        assert_eq!(h.editor.doc.paths.iter().filter(|p| !old_paths.contains(&p.id)).count(), 2);
    }
}

#[test]
fn construction_api_12_all_verbs_and_legacy_rejection() {
    let operations = [
        json!({"verb":"pathfinder","operation":"divide"}),
        json!({"verb":"pathfinder","operation":"trim"}),
        json!({"verb":"pathfinder","operation":"merge"}),
        json!({"verb":"pathfinder","operation":"crop"}),
        json!({"verb":"pathfinder","operation":"outline"}),
        json!({"verb":"pathfinder","operation":"minus_back"}),
        json!({"verb":"shape_builder","points":[[15,20],[45,20]],"delete":false}),
        json!({"verb":"shape_builder","points":[[15,20]],"delete":true}),
        json!({"verb":"scissors","segment":0,"t":0.5}),
        json!({"verb":"knife","points":[[0,20],[60,20]]}),
        json!({"verb":"eraser","points":[[20,0],[20,40]],"radius":2}),
        json!({"verb":"divide_objects_below"}),
    ];
    for operation in operations {
        for api in ["1.0", "1.1", "1.2"] {
            let mut h = FakeHost::two_pages();
            let second = h
                .editor
                .try_execute_created(varos_core::EditCommand::AddShape {
                    kind: varos_core::model::ShapeKind::Rect,
                    bounds: [20., 10., 20., 20.],
                    parent: None,
                    fill: Some([0., 1., 0., 1.]),
                    stroke: None,
                    stroke_width: 0.,
                    opacity: 1.,
                    name: None,
                })
                .unwrap();
            let before = h.editor.doc.clone();
            let rev = h.editor.rev;
            let mut op = operation.clone();
            op["ids"] = if op["verb"] == "scissors" {
                json!(["path:10"])
            } else if op["verb"] == "divide_objects_below" {
                json!([format!("path:{second}")])
            } else {
                json!(["path:10", format!("path:{second}")])
            };
            let mut s = Service::new("test-epoch".into());
            let decoded = varos_bridge::mcp::decode_tool(
                "edit",
                json!({"api":api,"board":"b1","request_id":"r1","expected_rev":rev,"ops":[op]}),
            );
            if api != "1.2" {
                assert!(decoded.is_err());
                assert!(h.editor.doc.content_eq(&before));
                continue;
            }
            let reply = s.handle(&mut h, &ctx(false), decoded.unwrap(), &AtomicBool::new(false));
            assert!(reply.ok, "{operation}, {api}: {reply:?}");
            if api == "1.2" {
                assert_eq!(h.editor.rev, rev + 1);
                h.editor.execute(varos_core::EditCommand::Undo);
                assert!(h.editor.doc.content_eq(&before));
            } else {
                assert!(h.editor.doc.content_eq(&before));
            }
        }
    }
}
#[test]
fn api_11_repeat_cannot_smuggle_construction_and_12_capabilities_are_opt_in() {
    let mut h = FakeHost::two_pages();
    let mut s = Service::new("test-epoch".into());
    let rev = h.editor.rev;
    assert!(varos_bridge::mcp::decode_tool("edit",json!({"api":"1.1","board":"b1","request_id":"r1","expected_rev":rev,"ops":[{"verb":"repeat","count":1,"dx":0,"dy":0,"ops":[{"verb":"knife","ids":["path:10"],"points":[[0,20],[50,20]]}]}]})).is_err());
    let reply = call(&mut s, &mut h, "capabilities", json!({"api":"1.2"}));
    assert!(reply.ok);
    assert!(reply.result.unwrap()["edit_verbs"].as_array().unwrap().contains(&json!("pathfinder")));
    let reply = call(&mut s, &mut h, "capabilities", json!({"api":"1.1"}));
    assert!(!reply.result.unwrap()["edit_verbs"].as_array().unwrap().contains(&json!("pathfinder")));
}

#[test]
fn construction_schema_is_explicit_opt_in_and_keeps_the_size_ratchet() {
    let old = varos_bridge::mcp::tools();
    assert_eq!(old, varos_bridge::mcp::tools_for_api("1.0"));
    assert_eq!(old, varos_bridge::mcp::tools_for_api("1.1"));
    let list = varos_bridge::mcp::tools_for_api("1.2");
    let bytes = serde_json::to_vec(&list).unwrap().len();
    assert!(bytes <= 24_000, "1.2 tools/list grew to {bytes} bytes");
    let edit = list["tools"].as_array().unwrap().iter().find(|t| t["name"] == "edit").unwrap();
    assert!(edit["inputSchema"]["$defs"]["pathfinder"]["properties"]["operation"]["enum"]
        .as_array()
        .unwrap()
        .contains(&json!("divide")));
}
