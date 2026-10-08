use super::*;
use varos_core::{
    model::{Artboard, ShapeKind},
    EditCommand,
};
fn scene(n: usize) -> (Workspace, SessionId, Vec<String>) {
    let mut ws = Workspace::new();
    let board = ws.active_id().unwrap();
    let ed = &mut ws.get_mut(board).unwrap().editor;
    ed.doc.artboards.push(Artboard { id: 11, w: 500.0, h: 500.0, ..Default::default() });
    ed.doc.artboards.push(Artboard { id: 22, x: 600.0, w: 500.0, h: 500.0, ..Default::default() });
    ed.doc.ids = 22; // reserve page identities in the same monotonic allocation arena
    for i in 0..n {
        ed.execute(EditCommand::AddShape {
            kind: ShapeKind::Rect,
            bounds: [i as f32 * 5.0, 0.0, 4.0, 4.0],
            parent: None,
            fill: None,
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
            name: None,
        });
    }
    let ids = ed.doc.paths.iter().map(|p| format!("path:{}", p.id)).collect();
    (ws, board, ids)
}
fn ed(ws: &Workspace, board: SessionId) -> &Editor {
    &ws.get(board).unwrap().editor
}
#[test]
fn transitions_in_flight_done_then_cleared_after_four_seconds() {
    let (ws, board, ids) = scene(1);
    let now = Instant::now();
    let mut p = Presence::default();
    p.begin("session", "Claude", "profile", board, Some(11), now);
    assert!(p.sessions["session"].in_flight);
    assert_eq!(p.frame(board, ed(&ws, board), now).pages.len(), 1);
    assert_eq!(p.frame(board, ed(&ws, board), now).repaint_after, None, "in-flight outline is static");
    p.done("session", ids, &BTreeSet::new(), Some(11), now);
    assert!(!p.sessions["session"].in_flight);
    p.retain(&ws, now + HOLD - Duration::from_nanos(1));
    assert_eq!(p.sessions.len(), 1);
    p.retain(&ws, now + HOLD);
    assert!(p.sessions.is_empty());
    assert!(p.frame(board, ed(&ws, board), now + HOLD).pages.is_empty());
}
#[test]
fn reveal_schedule_and_outline_count_for_one_fifty_four_hundred() {
    for n in [1, 50, 400] {
        let (ws, board, ids) = scene(n);
        let now = Instant::now();
        let mut p = Presence::default();
        p.begin("s", "Claude", "p", board, Some(11), now);
        p.done("s", ids, &BTreeSet::new(), Some(11), now);
        let step = stagger(n);
        assert_eq!(step, if n == 400 { Duration::from_micros(3750) } else { STAGGER });
        let reveal = &p.sessions["s"].reveal;
        assert_eq!(reveal[0].at, now);
        assert_eq!(reveal[n - 1].at, now + step * (n - 1) as u32);
        assert!(reveal[n - 1].at - now <= REVEAL_CAP);
        for millis in [0, 24, 25, 375, 899, 900, 1250, 1499, 2400] {
            let t = now + Duration::from_millis(millis);
            let expected = reveal.iter().filter(|r| r.at <= t && r.end > t).count();
            let f = p.frame(board, ed(&ws, board), t);
            assert_eq!(f.objects.len(), expected, "N={n}, t={millis}");
            assert!(f.objects.iter().all(|o| o.alpha > 0.0 && o.alpha <= 1.0));
        }
        let static_frame = p.frame(board, ed(&ws, board), now + Duration::from_millis(2400));
        assert_eq!(static_frame.repaint_after, Some(Duration::from_millis(1600)), "no frame chain during static hold");
    }
}
#[test]
fn selection_azure_wins_including_group_and_direct_selection() {
    let (mut ws, board, ids) = scene(2);
    let now = Instant::now();
    let mut p = Presence::default();
    p.begin("s", "Claude", "p", board, Some(11), now);
    p.done("s", ids, &BTreeSet::new(), Some(11), now);
    let editor = &mut ws.get_mut(board).unwrap().editor;
    editor.objsel.insert(editor.doc.paths[0].id);
    let f = p.frame(board, editor, now + STAGGER);
    assert_eq!(f.objects.len(), 1);
    editor.dsel_path = Some(editor.doc.paths[1].id);
    assert!(p.frame(board, editor, now + STAGGER).objects.is_empty());
    assert_eq!(
        p.frame(board, editor, now + STAGGER).repaint_after,
        Some(HOLD - STAGGER),
        "no fade repaints when every outline is suppressed"
    );
}
#[test]
fn no_session_draws_nothing_and_requests_no_repaint() {
    let (ws, board, _) = scene(1);
    let now = Instant::now();
    for i in 0..1000 {
        let t = now + Duration::from_millis(i * 37);
        let frame = Presence::default().frame(board, ed(&ws, board), t);
        assert!(frame.pages.is_empty() && frame.objects.is_empty());
        assert!(frame.repaint_after.is_none());
        let deadline = frame.repaint_after.map(|d| t + d);
        let plan = crate::pacing::plan(
            t,
            crate::pacing::paced(deadline, t, crate::pacing::refresh_interval(None)),
            &[],
            false,
        );
        assert!(!plan.redraw);
        assert_eq!(plan.flow, crate::pacing::Flow::Wait);
    }
}
#[test]
fn labels_are_bounded_unicode_without_controls() {
    assert_eq!(sanitize_label("\nClaude\t\0\u{202e}\u{2066}"), "Claude");
    assert_eq!(sanitize_label("\n\t"), "Agent");
    assert_eq!(sanitize_label(&"ك".repeat(50)).chars().count(), 24);
    assert!(!sanitize_label("Codex\n\r\u{007f}").chars().any(char::is_control));
}
#[test]
fn agents_stack_by_identity_and_different_pages_remain_independent() {
    let (ws, board, ids) = scene(1);
    let now = Instant::now();
    let mut p = Presence::default();
    for (client, label, profile, page) in
        [("s2", "Codex", "b", 11), ("s1", "Claude", "a", 11), ("s3", "Third", "c", 22)]
    {
        p.begin(client, label, profile, board, Some(page), now);
        p.done(client, ids.clone(), &BTreeSet::new(), Some(page), now);
    }
    let f = p.frame(board, ed(&ws, board), now);
    assert_eq!(f.pages.len(), 2);
    assert_eq!(f.pages[0].labels, vec!["Claude", "Codex"]);
    assert_eq!(f.pages[1].labels, vec!["Third"]);
    assert_eq!(f.objects.len(), 1, "overlapping agents share one outline per object");
    assert!(p.frame(SessionId(999), ed(&ws, board), now).pages.is_empty());
}
#[test]
fn new_batch_collapses_old_stagger_and_deleted_ids_disappear() {
    let (ws, board, ids) = scene(50);
    let now = Instant::now();
    let mut p = Presence::default();
    p.begin("s", "Claude", "p", board, Some(11), now);
    p.done("s", ids.clone(), &BTreeSet::new(), Some(11), now);
    let next = now + Duration::from_millis(100);
    p.begin("s", "Claude", "p", board, Some(22), next);
    assert!(p.sessions["s"].reveal.iter().all(|r| r.at <= next && r.end <= next + FINISH));
    let removed = BTreeSet::from([ids[0].clone()]);
    p.done("s", vec![ids[1].clone()], &removed, Some(22), next);
    assert_eq!(p.frame(board, ed(&ws, board), next).objects.len(), 49);
    assert_eq!(p.frame(board, ed(&ws, board), next + FINISH).objects.len(), 1);
    assert_eq!(p.sessions["s"].last_artboard, Some(22));
}
#[test]
fn document_close_and_host_reset_clear_state() {
    let (ws, board, _) = scene(0);
    let now = Instant::now();
    let mut p = Presence::default();
    p.begin("s", "Claude", "p", SessionId(999), Some(11), now);
    p.retain(&ws, now);
    assert!(p.sessions.is_empty());
    PRESENCE.with(|p| p.borrow_mut().begin("s", "Claude", "p", board, Some(11), now));
    clear();
    assert!(frame(Some(board), ed(&ws, board), now).pages.is_empty());
    assert!(frame(None, ed(&ws, board), now).repaint_after.is_none());
}
struct Fields;
impl crate::host::DocUi for Fields {
    fn settle(&mut self, _: &mut Editor) -> bool {
        true
    }
    fn document_switched(&mut self) {}
}
fn dispatch(ws: &mut Workspace, request: Request) -> Reply {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let mut audit = varos_bridge::conn::audit::Entry::event("call", "profile", "pending");
    audit.label = "Claude\n".into();
    crate::bridge_host::run(
        Pending {
            context: varos_bridge::Context { client: "profile:session".into(), epoch: "epoch".into() },
            request,
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            reply: tx,
            file_audit: Some((varos_bridge::conn::Paths::under("/tmp/presence-unused"), audit)),
        },
        ws,
        &mut Fields,
    );
    rx.recv().unwrap()
}
#[test]
fn fifo_host_edit_reveals_in_operation_order_and_keeps_one_undo_step() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, ids) = scene(2);
    let before = ed(&ws, board).doc.clone();
    let make = |request_id: &str, expected_rev: u64| {
        varos_bridge::mcp::decode_tool("edit",serde_json::json!({"api":"1.0","request_id":request_id,"board":format!("b{}",board.0),"expected_rev":expected_rev,"ops":[{"verb":"move","ids":[ids[1]],"delta":[10,0]},{"verb":"set_paint","ids":[ids[0]],"fill":"#FF0000FF"}]})).unwrap()
    };
    let request = make("r1", ed(&ws, board).rev);
    let reply = dispatch(&mut ws, request.clone());
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(reply.undo_steps, 1);
    PRESENCE.with(|p| {
        let p = p.borrow();
        let s = &p.sessions["profile:session"];
        assert_eq!(s.label, "Claude");
        assert_eq!(s.profile_id, "profile");
        assert!(!s.in_flight);
        assert_eq!(s.reveal.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec![ids[1].as_str(), ids[0].as_str()]);
    });
    let last = PRESENCE.with(|p| p.borrow().sessions["profile:session"].last_edit);
    assert!(dispatch(&mut ws, request).ok, "receipt replay");
    assert_eq!(PRESENCE.with(|p| p.borrow().sessions["profile:session"].last_edit), last);
    let invalid=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r2","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[{"verb":"move","ids":["path:999999"],"delta":[1,0]}]})).unwrap();
    assert!(!dispatch(&mut ws, invalid).ok);
    assert_eq!(PRESENCE.with(|p| p.borrow().sessions["profile:session"].last_edit), last);
    ws.get_mut(board).unwrap().editor.undo();
    assert_eq!(ed(&ws, board).doc, before);
}
#[test]
fn capped_receipt_reveals_all_four_hundred_targets_without_calls() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, mut ids) = scene(400);
    ids.reverse();
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[{"verb":"set_paint","ids":ids,"fill":"#FF0000FF"}]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(reply.result.as_ref().unwrap()["more"], true, "fixture exercises receipt cap");
    PRESENCE.with(|p| {
        let p = p.borrow();
        let s = &p.sessions["profile:session"];
        assert_eq!(s.reveal.len(), 400);
        assert_eq!(s.reveal.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), ids);
        assert!(s.reveal.last().unwrap().at - s.last_edit <= REVEAL_CAP);
    });
}

#[test]
fn group_selection_suppresses_both_group_and_descendant_highlights() {
    let (mut ws, board, ids) = scene(2);
    let now = Instant::now();
    let editor = &mut ws.get_mut(board).unwrap().editor;
    editor.objsel.extend(editor.doc.paths.iter().map(|p| p.id));
    editor.group_selection();
    let group = editor.doc.top_group_of_path(editor.doc.paths[0].id).unwrap();
    let mut p = Presence::default();
    p.begin("s", "Claude", "p", board, Some(11), now);
    p.done("s", vec![format!("node:{group}"), ids[0].clone(), ids[1].clone()], &BTreeSet::new(), Some(11), now);
    assert!(p.frame(board, editor, now + STAGGER * 3).objects.is_empty());
}
#[test]
fn temporary_deleted_creation_does_not_consume_surviving_creation_order() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, ids) = scene(1);
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[
        {"verb":"add_shape","kind":"rect","local":"$tmp","bounds":[30,30,10,10],"fill":"#FF0000FF"},
        {"verb":"delete","ids":["$tmp"]},
        {"verb":"add_shape","kind":"rect","bounds":[50,50,10,10],"fill":"#FF0000FF"},
        {"verb":"set_paint","ids":[ids[0]],"fill":"#FF0000FF"}
    ]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    let created = reply.result.as_ref().unwrap()["created"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|o| o["id"].as_str().filter(|id| id.starts_with("path:")))
        .unwrap();
    PRESENCE.with(|p| {
        let p = p.borrow();
        let reveal = &p.sessions["profile:session"].reveal;
        assert_eq!(reveal[0].id, created);
        assert!(reveal.iter().position(|r| r.id == created) < reveal.iter().position(|r| r.id == ids[0]));
    });
}
#[test]
fn duplicated_artwork_precedes_later_moves_and_target_is_the_new_page() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, ids) = scene(2);
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[
        {"verb":"duplicate_artboard","id":"artboard:11","with_art":true,"offset":[1200,0],"local":"$copy"},
        {"verb":"move","ids":[ids[0]],"delta":[0,10]},
        {"verb":"add_shape","kind":"rect","bounds":[100,100,10,10],"fill":"#FF0000FF"}
    ]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    let created: Vec<_> = reply.result.as_ref().unwrap()["created"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["id"].as_str().filter(|id| id.starts_with("path:")))
        .collect();
    let page = page_id(reply.result.as_ref().unwrap()["locals"]["$copy"].as_str().unwrap()).unwrap();
    PRESENCE.with(|p| {
        let p = p.borrow();
        let s = &p.sessions["profile:session"];
        assert_eq!(s.last_artboard, Some(page));
        let moved = s.reveal.iter().position(|r| r.id == ids[0]).unwrap();
        assert_eq!(created.len(), 3);
        for id in &created[..2] {
            assert!(s.reveal.iter().position(|r| r.id == *id).unwrap() < moved);
        }
        assert!(s.reveal.iter().position(|r| r.id == created[2]).unwrap() > moved);
    });
}
#[test]
fn page_creation_without_local_lights_created_page_not_active_page() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, _) = scene(0);
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[{"verb":"add_artboard","bounds":[1200,0,500,500]}]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    let page = page_id(reply.result.as_ref().unwrap()["artboards_created"][0].as_str().unwrap()).unwrap();
    assert_ne!(ed(&ws, board).doc.active_artboard().unwrap().id, page);
    PRESENCE.with(|p| assert_eq!(p.borrow().sessions["profile:session"].last_artboard, Some(page)));
}
#[test]
fn new_agent_batch_finishes_other_agents_reveal_quickly() {
    let (ws, board, ids) = scene(50);
    let now = Instant::now();
    let mut p = Presence::default();
    p.begin("a", "Claude", "a", board, Some(11), now);
    p.done("a", ids.clone(), &BTreeSet::new(), Some(11), now);
    let next = now + Duration::from_millis(100);
    p.begin("b", "Codex", "b", board, Some(22), next);
    p.done("b", vec![ids[0].clone()], &BTreeSet::new(), Some(22), next);
    assert!(p.sessions["a"].reveal.iter().all(|r| r.at <= next && r.end <= next + FINISH));
    assert_eq!(p.frame(board, ed(&ws, board), next + FINISH).objects.len(), 1);
}

#[test]
fn empty_duplicate_cannot_take_a_later_unnamed_creation() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, ids) = scene(1);
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[
        {"verb":"duplicate_artboard","id":"artboard:22","with_art":true,"offset":[1200,0]},
        {"verb":"move","ids":[ids[0]],"delta":[0,10]},
        {"verb":"add_shape","kind":"rect","bounds":[100,100,10,10],"fill":"#FF0000FF"}
    ]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    let new = reply.result.as_ref().unwrap()["created"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|o| o["id"].as_str().filter(|id| id.starts_with("path:")))
        .unwrap();
    PRESENCE.with(|p| {
        let p = p.borrow();
        let r = &p.sessions["profile:session"].reveal;
        assert!(r.iter().position(|r| r.id == ids[0]) < r.iter().position(|r| r.id == new));
    });
}
#[test]
fn reverse_allocation_blocks_handle_rounded_rects_paths_and_groups() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, ids) = scene(2);
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[
        {"verb":"add_shape","kind":"rect","bounds":[100,100,10,10],"radius":5,"fill":"#FF0000FF"},
        {"verb":"add_path","anchors":[{"p":[200,200]},{"p":[210,200]},{"p":[210,210]}],"closed":true,"fill":"#FF0000FF"},
        {"verb":"group","ids":ids},
        {"verb":"add_shape","kind":"rect","bounds":[300,300,10,10],"radius":2,"fill":"#FF0000FF"}
    ]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    let document = &ed(&ws, board).doc;
    let created: Vec<_> = document
        .paths
        .iter()
        .filter(|p| !ids.contains(&format!("path:{}", p.id)))
        .map(|p| format!("path:{}", p.id))
        .collect();
    let group = document
        .top_group_of_path(document.paths.iter().find(|p| ids.contains(&format!("path:{}", p.id))).unwrap().id)
        .unwrap();
    PRESENCE.with(|p| {
        let p = p.borrow();
        let r = &p.sessions["profile:session"].reveal;
        let at = |id: &str| r.iter().position(|r| r.id == id).unwrap();
        assert!(at(&created[0]) < at(&created[1]));
        assert!(at(&created[1]) < at(&format!("node:{group}")));
        assert!(at(&format!("node:{group}")) < at(&created[2]));
    });
}
#[test]
fn geometry_cache_refreshes_after_human_edit_and_hidden_page_schedules_nothing() {
    let (mut ws, board, ids) = scene(1);
    let now = Instant::now();
    let mut p = Presence::default();
    p.begin("s", "Claude", "p", board, Some(11), now);
    p.done("s", ids, &BTreeSet::new(), Some(11), now);
    let first = p.frame(board, ed(&ws, board), now).objects[0].bounds;
    let editor = &mut ws.get_mut(board).unwrap().editor;
    let pid = editor.doc.paths[0].id;
    editor.try_execute(EditCommand::SelectPaths(vec![pid])).unwrap();
    editor
        .try_execute(EditCommand::SetObjectBounds {
            x: Some(10.0),
            y: None,
            width: None,
            height: None,
            anchor_x: 0.0,
            anchor_y: 0.0,
        })
        .unwrap();
    editor.objsel.clear();
    let moved = p.frame(board, editor, now).objects[0].bounds;
    assert_eq!(moved[0], first[0] + 10.0);
    editor.execute(EditCommand::ToggleArtboardHidden(0));
    let f = p.frame(board, editor, now);
    assert!(f.pages.is_empty() && f.objects.is_empty());
    assert_eq!(f.repaint_after, None);
}

#[test]
fn deleted_duplicate_artwork_cannot_take_a_later_unnamed_creation() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, ids) = scene(1);
    let copied = format!("path:{}", ed(&ws, board).doc.ids + 2); // page id, then copied path id
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[
        {"verb":"duplicate_artboard","id":"artboard:11","with_art":true,"offset":[1200,0]},
        {"verb":"delete","ids":[copied]},
        {"verb":"add_shape","kind":"rect","bounds":[100,100,10,10],"fill":"#FF0000FF"},
        {"verb":"move","ids":[ids[0]],"delta":[0,10]}
    ]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    let new = reply.result.as_ref().unwrap()["created"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|o| o["id"].as_str().filter(|id| id.starts_with("path:")))
        .unwrap();
    PRESENCE.with(|p| {
        let p = p.borrow();
        let r = &p.sessions["profile:session"].reveal;
        assert_eq!(r[0].id, new);
        assert!(!r.iter().any(|r| r.id == copied));
        assert!(r.iter().position(|r| r.id == new) < r.iter().position(|r| r.id == ids[0]));
    });
}

#[test]
fn future_created_group_cannot_reveal_through_an_earlier_descendant_move() {
    crate::bridge_host::initialize("epoch".into());
    let (mut ws, board, ids) = scene(3);
    let request=varos_bridge::mcp::decode_tool("edit",serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":ed(&ws,board).rev,"ops":[
        {"verb":"move","ids":[ids[0]],"delta":[0,10]},
        {"verb":"move","ids":[ids[2]],"delta":[0,20]},
        {"verb":"group","ids":[ids[0],ids[1]],"local":"$group"}
    ]})).unwrap();
    let reply = dispatch(&mut ws, request);
    assert!(reply.ok, "{:?}", reply.error);
    let group = reply.result.as_ref().unwrap()["locals"]["$group"].as_str().unwrap();
    PRESENCE.with(|p| {
        let p = p.borrow();
        let r = &p.sessions["profile:session"].reveal;
        let at = |id: &str| r.iter().position(|r| r.id == id).unwrap();
        assert!(at(&ids[0]) < at(&ids[2]));
        assert!(at(&ids[2]) < at(group));
    });
}

#[test]
fn visibility_deadlines_sleep_offscreen_and_at_hold_even_with_queued_reveals() {
    let (ws, board, ids) = scene(50);
    let now = Instant::now();
    let mut p = Presence::default();
    p.begin("s", "Claude", "p", board, Some(11), now);
    p.done("s", ids, &BTreeSet::new(), Some(11), now);
    let f = p.frame(board, ed(&ws, board), now);
    assert_eq!(repaint_after(&f, |_| false), None);
    assert_eq!(repaint_after(&f, |_| true), Some(Duration::ZERO));
    assert_eq!(repaint_after(&f, |b| b[0] == 0.0 && b[2] == 500.0), Some(HOLD), "static page never requests ZERO");
    assert_eq!(repaint_after(&f, |b| b[0] == 5.0), Some(STAGGER), "visible future reveal is a one-shot deadline");
    let f = p.frame(board, ed(&ws, board), now + FADE);
    assert_eq!(repaint_after(&f, |b| b[0] == 0.0 && b[2] == 500.0), Some(HOLD - FADE));
    assert!(!p.sessions["s"].reveal.is_empty());
    let f = p.frame(board, ed(&ws, board), now + HOLD);
    assert_eq!(f.repaint_after, None);
    assert_eq!(repaint_after(&f, |_| true), None);
}
#[test]
fn invisible_unicode_marks_and_line_separators_are_stripped() {
    assert_eq!(
        sanitize_label("C\u{200b}\u{200c}\u{200d}\u{200e}\u{200f}\u{061c}\u{2028}\u{2029}\u{feff}odex"),
        "Codex"
    );
}
#[test]
fn child_bounds_do_not_reveal_unchanged_ancestor_group_or_layer() {
    let (mut ws, board, ids) = scene(2);
    let editor = &mut ws.get_mut(board).unwrap().editor;
    editor.objsel.extend(editor.doc.paths.iter().map(|p| p.id));
    editor.group_selection();
    let group = editor.doc.top_group_of_path(editor.doc.paths[0].id).unwrap();
    let before = editor.doc.clone();
    editor.objsel.clear();
    let request = varos_bridge::mcp::decode_tool("edit", serde_json::json!({"request_id":"r1","api":"1.0","board":format!("b{}",board.0),"expected_rev":editor.rev,"ops":[{"verb":"move","ids":[ids[0]],"delta":[10,0]}]})).unwrap();
    crate::bridge_host::initialize("epoch".into());
    let reply = dispatch(&mut ws, request.clone());
    assert!(reply.ok);
    let Request::Edit(edit) = request else { panic!() };
    let (order, _) = receipt_order(&edit, reply.result.as_ref().unwrap(), &before, &ed(&ws, board).doc);
    assert_eq!(order, vec![ids[0].clone()]);
    assert!(!order.contains(&format!("node:{group}")));
    assert!(object_paths(&ed(&ws, board).doc, "node:1").is_empty());
}
