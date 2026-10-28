//! Varos boards (`varos::boards`): an agent's request acts on the board it names, in the one UI
//! turn that runs it, and the person's tab is never switched (VQA-11). Headless: requests go
//! through `drain_control` with a default egui context, as the frame drains them.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use serde_json::{Value, json};
use varos_agents::boards::board_id;
use vectorcraft_engine::Session;

use crate::control::ControlRequest;
use crate::{Services, VectorcraftApp};
use varos_agents::boards::board_id as id_of;

fn app() -> (VectorcraftApp, Sender<ControlRequest>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let services = Services { write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))), ..Services::default() };
    (VectorcraftApp::new(Session::new(), services).with_control(rx).with_agent_guards(), tx)
}

/// An agent through Varos' agents port, naming `target` (`{board, revision?}`) or none.
fn agent(target: Option<Value>) -> Option<Value> {
    let mut a = json!({"session": "b1", "profile": "codex", "label": "Codex", "guarded": true});
    if let Some(t) = target {
        a["target"] = t;
    }
    Some(a)
}

fn send(tx: &Sender<ControlRequest>, actor: Option<Value>, method: &str, params: Value) -> std::sync::mpsc::Receiver<Value> {
    let (req, reply) = ControlRequest::new(method, params);
    tx.send(req.with_actor(actor)).unwrap();
    reply
}

fn request(app: &mut VectorcraftApp, tx: &Sender<ControlRequest>, actor: Option<Value>, method: &str, params: Value) -> Value {
    let reply = send(tx, actor, method, params);
    app.drain_control(&egui::Context::default());
    reply.try_recv().unwrap()
}

/// Two open boards, A then B; B (made last) is in front, as the person left it.
fn two_boards(app: &mut VectorcraftApp) -> (u64, u64) {
    app.run("file.new", json!({"width": 200, "height": 200, "units": "Points", "artboards": 1})).unwrap();
    let a = app.session.active().unwrap().uid;
    app.run("file.new", json!({"width": 300, "height": 300, "units": "Points", "artboards": 1})).unwrap();
    let b = app.session.active().unwrap().uid;
    (a, b)
}

fn front(app: &VectorcraftApp) -> u64 {
    app.session.active().unwrap().uid
}

fn doc(app: &VectorcraftApp, uid: u64) -> Arc<vectorcraft_doc::Document> {
    app.session.documents().iter().find(|d| d.uid == uid).unwrap().doc.clone()
}

fn index_of(app: &VectorcraftApp, uid: u64) -> usize {
    app.session.documents().iter().position(|d| d.uid == uid).unwrap()
}

fn rect(x: i32) -> Value {
    json!({"command": "shape.rectangle", "params": {"x": x, "y": 10, "width": 20, "height": 20}})
}

fn on(uid: u64) -> Option<Value> {
    Some(json!({ "board": board_id(uid) }))
}

fn error(r: &Value) -> &str {
    r["error"].as_str().unwrap_or("")
}

fn list(app: &mut VectorcraftApp, tx: &Sender<ControlRequest>) -> Value {
    request(app, tx, agent(None), varos_agents::boards::LIST, json!({}))["result"].clone()
}

fn revision(app: &mut VectorcraftApp, tx: &Sender<ControlRequest>, uid: u64) -> u64 {
    let l = list(app, tx);
    l["boards"].as_array().unwrap().iter().find(|b| b["board"] == json!(board_id(uid))).unwrap()["revision"].as_u64().unwrap()
}

/// The agent edits board A while the person is on board B: the art lands on A only, B is
/// untouched, and B stays in front. The step is the agent's, in A's History.
#[test]
fn an_edit_on_a_background_board_lands_there_and_the_persons_tab_stays() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    let before_b = doc(&app, b);
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", rect(10));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(front(&app), b, "the person's tab is given back");
    assert!(Arc::ptr_eq(&doc(&app, b), &before_b), "board B is untouched");
    let st = &app.session.documents()[index_of(&app, a)];
    assert!(st.history.undo.last().unwrap().actor.is_some(), "the step on A is the agent's");
    assert_eq!(r["varos"]["board"], json!(board_id(a)));
    assert_eq!(r["varos"]["front"], json!(board_id(b)));
    assert!(r["varos"]["revision"].is_u64());
    // The object it made is on A.
    let id = r["result"]["id"].as_u64().expect("the new object's id");
    assert!(doc(&app, a).node(vectorcraft_doc::NodeId(id)).is_some());
    // Reading A from behind B: A's tree, B still in front.
    let r = request(&mut app, &tx, agent(on(a)), "document.inspect", json!({"depth": 0}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(front(&app), b);
}

/// VQA-11: the person switches tabs between two queued agent commands. A document command still
/// lands on the board it names, never on the new front; agents may not drive the window at all.
#[test]
fn switching_the_tab_between_two_queued_commands_never_misroutes() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    app.session.set_active(index_of(&app, a));
    // Two commands queued for A; the person clicks tab B before they run.
    let first = send(&tx, agent(on(a)), "engine.execute", rect(10));
    let second = send(&tx, agent(on(a)), "engine.execute", rect(40));
    app.session.set_active(index_of(&app, b));
    let (doc_a, doc_b) = (doc(&app, a), doc(&app, b));
    app.drain_control(&egui::Context::default());
    for reply in [first, second] {
        let r = reply.try_recv().unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["varos"]["board"], json!(board_id(a)));
    }
    assert!(!Arc::ptr_eq(&doc(&app, a), &doc_a), "they ran on A");
    assert!(Arc::ptr_eq(&doc(&app, b), &doc_b), "B is untouched");
    assert_eq!(front(&app), b, "the person's choice stands");
    // Keys and text from an agent never go to the window, whichever board is in front.
    for method in ["ui.key", "ui.text", "ui.click", "ui.pointer"] {
        let r = request(&mut app, &tx, agent(on(b)), method, json!({"key": "Delete", "text": "x", "events": []}));
        assert!(error(&r).starts_with("person_state"), "{method}: {r}");
    }
    assert!(app.synthetic.is_empty());
}

/// Save and export need the board and the revision the agent read: a missing revision, an old
/// one, an unknown board or one of an earlier launch are refused before anything is written, and
/// the file written is the named board's, whatever is in front.
#[test]
fn save_and_export_need_the_board_and_its_current_revision() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    let home = std::env::temp_dir().join(format!("varos-ui-boards-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("Desktop")).unwrap();
    let home = std::fs::canonicalize(home).unwrap();
    app.agents.places = Some(varos_agents::guard::Places { home: home.clone(), app: None });
    // A has a 77 pt wide rectangle, B (in front) has none.
    app.session.set_active(index_of(&app, a));
    app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 77, "height": 20})).unwrap();
    app.session.set_active(index_of(&app, b));
    let out = home.join("Desktop/a.svg");
    let export = || json!({"command": "document.export", "params": {"path": out.to_string_lossy(), "format": "svg"}});
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", export());
    assert!(error(&r).starts_with("board_required"), "no revision: {r}");
    let rev = revision(&mut app, &tx, a);
    let r = request(&mut app, &tx, agent(Some(json!({"board": board_id(a), "revision": rev + 5}))), "engine.execute", export());
    assert!(error(&r).starts_with("stale_revision"), "{r}");
    let r = request(&mut app, &tx, agent(Some(json!({"board": board_id(999), "revision": rev}))), "engine.execute", export());
    assert!(error(&r).starts_with("board_not_found"), "{r}");
    let other_launch = if varos_agents::boards::launch() == "00000000" { format!("11111111-{a}") } else { format!("00000000-{a}") };
    let r = request(&mut app, &tx, agent(Some(json!({"board": other_launch, "revision": rev}))), "engine.execute", export());
    assert!(error(&r).starts_with("session_reset"), "{r}");
    // The person edits A after the agent read it: the revision the agent has is old.
    app.session.set_active(index_of(&app, a));
    app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 5, "height": 5})).unwrap();
    app.session.set_active(index_of(&app, b));
    let r = request(&mut app, &tx, agent(Some(json!({"board": board_id(a), "revision": rev}))), "engine.execute", export());
    assert!(error(&r).starts_with("stale_revision"), "{r}");
    assert!(!out.exists(), "nothing was written");
    // The person selecting things doesn't move the revision.
    let rev = revision(&mut app, &tx, a);
    app.run("select.all", json!({})).unwrap();
    assert_eq!(revision(&mut app, &tx, a), rev);
    // With the current revision: A is written, from behind B.
    let r = request(&mut app, &tx, agent(Some(json!({"board": board_id(a), "revision": rev}))), "engine.execute", export());
    assert_eq!(r["ok"], true, "{r}");
    let svg = std::fs::read_to_string(&out).unwrap();
    assert!(svg.contains("width=\"200\"") && svg.contains("L87 10"), "board A (200 pt, the 77 pt rectangle): {svg}");
    assert_eq!(front(&app), b);
    // A save of a background board needs its revision too.
    let saved = home.join("Desktop/a.vectorcraft");
    let save = json!({"path": saved.to_string_lossy()});
    let r = request(&mut app, &tx, agent(on(a)), "app.save", save.clone());
    assert!(error(&r).starts_with("board_required"), "{r}");
    let r = request(&mut app, &tx, agent(Some(json!({"board": board_id(a), "revision": rev}))), "app.save", save);
    assert_eq!(r["ok"], true, "{r}");
    assert!(saved.exists());
    let st = &app.session.documents()[index_of(&app, a)];
    assert_eq!(st.path.as_deref(), Some(saved.to_string_lossy().as_ref()), "board A took the file");
    assert_eq!(app.session.documents()[index_of(&app, b)].path, None, "board B did not");
    assert_eq!(front(&app), b);
}

/// Through Varos' agents port a request that touches a board must name it while one is open;
/// the lists and making a new board don't, and a new board opens behind the person's.
#[test]
fn no_board_named_is_refused_while_a_board_is_open() {
    let (mut app, tx) = app();
    let (_, b) = two_boards(&mut app);
    let before = doc(&app, b);
    let r = request(&mut app, &tx, agent(None), "engine.execute", rect(10));
    assert!(error(&r).starts_with("board_required"), "{r}");
    assert!(Arc::ptr_eq(&doc(&app, b), &before), "nothing ran");
    assert_eq!(request(&mut app, &tx, agent(None), "engine.commands", json!({}))["ok"], true);
    let r = request(&mut app, &tx, agent(None), "engine.execute", json!({"command": "file.new", "params": {"width": 100, "height": 100}}));
    assert_eq!(r["ok"], true, "{r}");
    let opened = r["varos"]["opened"].as_array().unwrap().clone();
    assert_eq!(opened.len(), 1, "{r}");
    assert_eq!(r["varos"]["board"], opened[0], "the reply names the new board");
    assert_eq!(front(&app), b, "it opened behind the person's board");
    // Upstream's control port (not guarded) keeps upstream's behaviour.
    let r = request(&mut app, &tx, None, "engine.execute", rect(10));
    assert_eq!(r["ok"], true, "{r}");
}

/// `varos.boards`: stable ids (the document's own, not its tab index), titles, files, unsaved
/// changes, revisions and which one is in front. Ids survive other tabs closing; indices don't.
#[test]
fn list_boards_ids_are_stable_when_other_tabs_close() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    let c = front(&app);
    let l = list(&mut app, &tx);
    let boards = l["boards"].as_array().unwrap();
    assert_eq!(boards.len(), 3);
    assert_eq!(l["front"], json!(board_id(c)));
    for (entry, uid) in boards.iter().zip([a, b, c]) {
        assert_eq!(entry["board"], json!(board_id(uid)));
        assert!(entry["title"].as_str().unwrap().starts_with("Untitled"), "{entry}");
        assert!(entry["dirty"].is_boolean() && entry["revision"].is_u64() && entry["path"].is_null(), "{entry}");
        assert_eq!(entry["front"], uid == c);
    }
    // The person closes the first tab: the others move down an index, their ids stay.
    app.session.execute("file.close", &json!({"index": 0})).unwrap();
    app.sync_views();
    // (Closing a tab puts the one at its index in front; the person goes back to C.)
    app.session.set_active(index_of(&app, c));
    let l = list(&mut app, &tx);
    let ids: Vec<&Value> = l["boards"].as_array().unwrap().iter().map(|e| &e["board"]).collect();
    assert_eq!(ids, vec![&json!(board_id(b)), &json!(board_id(c))]);
    // The closed board is gone for good.
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", rect(10));
    assert!(error(&r).starts_with("board_not_found"), "{r}");
    // An edit of B by the agent, from behind C, still finds B at its new index.
    let r = request(&mut app, &tx, agent(on(b)), "engine.execute", rect(10));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(front(&app), c);
}

/// Agents never switch the person's tab, and never name a document by tab index.
#[test]
fn an_agent_never_switches_the_persons_tab() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    let r = request(&mut app, &tx, agent(on(b)), "engine.execute", json!({"command": "document.activate", "params": {"index": 0}}));
    assert!(error(&r).starts_with("target_changed"), "{r}");
    let batch = json!({"command": "command.batch", "params": {"commands": [{"command": "document.activate", "params": {"index": 0}}, rect(5)]}});
    assert!(error(&request(&mut app, &tx, agent(on(b)), "engine.execute", batch)).starts_with("target_changed"));
    let r = request(&mut app, &tx, agent(on(b)), "engine.execute", json!({"command": "file.close", "params": {"index": 0}}));
    assert!(error(&r).starts_with("board_required"), "{r}");
    assert_eq!(front(&app), b);
    assert_eq!(app.session.documents().len(), 2);
    // Closing a background board by its id: it closes, the person's stays in front.
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", json!({"command": "file.close", "params": {}}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(app.session.documents().len(), 1);
    assert_eq!(front(&app), b);
}

/// While the person drags or types on their board, another board is not lent the front (that
/// would end their gesture): `person_busy`, nothing changes. On their own board the agent's
/// request runs as before.
#[test]
fn the_persons_gesture_is_never_ended_by_an_agent() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    app.session.begin_interaction("Move").unwrap();
    let before = doc(&app, a);
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", rect(10));
    assert!(error(&r).starts_with("person_busy"), "{r}");
    assert!(Arc::ptr_eq(&doc(&app, a), &before));
    assert!(app.session.active().unwrap().interaction.is_some(), "the person's gesture goes on");
    assert_eq!(front(&app), b);
    app.session.cancel_interaction().unwrap();
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", rect(10));
    assert_eq!(r["ok"], true, "{r}");
}

/// A dialog a request would open on a board lent the front belongs to the window, which goes back
/// to the person's board: it is dismissed and the request answers `target_changed`.
#[test]
fn a_dialog_on_a_background_board_is_dismissed() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    // Upstream's port naming a board (unguarded: dialogs are allowed there).
    let actor = Some(json!({"session": "c1", "target": {"board": board_id(a)}}));
    let r = request(&mut app, &tx, actor, "engine.execute", json!({"command": "ui.newSwatch", "params": {}}));
    assert!(error(&r).starts_with("target_changed"), "{r}");
    assert!(error(&r).contains("dialog"), "{r}");
    assert!(app.ui.dialog.is_none(), "no dialog left on the person's window");
    assert_eq!(front(&app), b);
}

fn ids(r: &Value) -> u64 {
    r["result"]["id"].as_u64().expect("an object id")
}

/// Fix 1: a batch stays on the board it names. From an agent, a step that closes, opens, makes or
/// switches a board is refused before anything runs (and closing all boards is refused outright);
/// from any client naming a board, the engine is pinned to it: a step that would run on another
/// board fails and the steps after it don't run.
#[test]
fn a_batch_never_leaves_its_board() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    let (doc_a, doc_b) = (doc(&app, a), doc(&app, b));
    let escape = json!({"command": "command.batch", "params": {"commands": [{"command": "file.close", "params": {}}, rect(10)]}});
    for target in [a, b] {
        let r = request(&mut app, &tx, agent(on(target)), "engine.execute", escape.clone());
        assert!(error(&r).starts_with("target_changed"), "{r}");
    }
    assert_eq!(app.session.documents().len(), 2);
    assert!(Arc::ptr_eq(&doc(&app, a), &doc_a) && Arc::ptr_eq(&doc(&app, b), &doc_b), "nothing ran");
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", json!({"command": "file.closeAll", "params": {}}));
    assert!(error(&r).starts_with("person_state"), "{r}");
    assert_eq!(app.session.documents().len(), 2);
    // A client of upstream's port naming board A: the batch closes A, then the pinned engine
    // refuses to draw on B.
    let client = Some(json!({"session": "c1", "target": {"board": board_id(a)}}));
    let r = request(&mut app, &tx, client, "engine.execute", escape);
    assert!(r.to_string().contains("target_changed"), "{r}");
    assert!(Arc::ptr_eq(&doc(&app, b), &doc_b), "B was not drawn on");
    // The failed batch is rolled back: A is open again.
    assert_eq!(app.session.documents().len(), 2, "{r}");
    assert!(app.session.documents().iter().any(|d| d.uid == a));
}

/// Fix 2: synthetic input queued for a board is delivered only while that board is in front. The
/// person switches tabs before the host injects it: it is dropped (consumed from the queue, never
/// put into the frame's input), and the next reply says so.
#[test]
fn queued_synthetic_input_never_reaches_another_board() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    app.session.set_active(index_of(&app, a));
    // Upstream's port, naming board A (agents may not send input at all).
    let client = || Some(json!({"session": "c1", "target": {"board": board_id(a)}}));
    let r = request(&mut app, &tx, client(), "ui.key", json!({"key": "Delete"}));
    assert_eq!(r["ok"], true, "{r}");
    assert!(!app.synthetic.is_empty());
    // The person clicks tab B (with an object selected there) before the frame that injects it.
    app.session.set_active(index_of(&app, b));
    app.run("shape.rectangle", json!({"x": 5, "y": 5, "width": 10, "height": 10})).unwrap();
    let doc_b = doc(&app, b);
    let ctx = egui::Context::default();
    let mut raw = egui::RawInput::default();
    app.raw_input_hook(&ctx, &mut raw);
    assert!(!raw.events.iter().any(|e| matches!(e, egui::Event::Key { .. })), "no key reached B: {:?}", raw.events);
    assert!(app.synthetic.is_empty(), "the input was consumed (dropped)");
    assert!(Arc::ptr_eq(&doc(&app, b), &doc_b));
    let r = request(&mut app, &tx, client(), "document.inspect", json!({"depth": 0}));
    let dropped = &r["varos"]["dropped"][0];
    assert_eq!(dropped["board"], json!(id_of(a)), "{r}");
    assert!(dropped["error"].as_str().unwrap().starts_with("target_changed"), "{r}");
    // With its board still in front, the same input is delivered.
    app.session.set_active(index_of(&app, a));
    let r = request(&mut app, &tx, client(), "ui.key", json!({"key": "Escape"}));
    assert_eq!(r["ok"], true, "{r}");
    let mut raw = egui::RawInput::default();
    app.raw_input_hook(&ctx, &mut raw);
    assert!(raw.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Escape, .. })), "{:?}", raw.events);
}

/// Fix 3: a window capture admitted for board A, with the person on board B by the time it would
/// be taken, is refused and writes no file.
#[test]
fn a_window_capture_never_captures_another_board() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    app.session.set_active(index_of(&app, a));
    let home = std::env::temp_dir().join(format!("varos-ui-capture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("Desktop")).unwrap();
    let home = std::fs::canonicalize(home).unwrap();
    app.agents.places = Some(varos_agents::guard::Places { home: home.clone(), app: None });
    let shot = home.join("Desktop/a.png");
    let rev = revision(&mut app, &tx, a);
    let reply = send(&tx, agent(Some(json!({"board": board_id(a), "revision": rev}))), "ui.screenshot", json!({"path": shot.to_string_lossy()}));
    // A context with a window size, as after a drawn frame.
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 100.0))), ..Default::default() });
    let _ = ctx.end_pass();
    app.drain_control(&ctx);
    assert!(reply.try_recv().is_err(), "it waits for a frame");
    app.session.set_active(index_of(&app, b));
    let start = std::time::Instant::now();
    let r = loop {
        app.issue_screenshots(&ctx);
        app.collect_screenshots(&ctx);
        if let Ok(r) = reply.try_recv() {
            break r;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(5), "no answer");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(error(&r).contains("document changed"), "{r}");
    assert!(!shot.exists(), "no file");
    // Captures of a board not in front are refused at once.
    let rev = revision(&mut app, &tx, a);
    let r =
        request(&mut app, &tx, agent(Some(json!({"board": board_id(a), "revision": rev}))), "ui.screenshot", json!({"path": shot.to_string_lossy()}));
    assert!(error(&r).starts_with("target_changed"), "{r}");
}

/// Fix 4: an agent never changes the person's tool, selection, current layer or view. It has a
/// selection of its own per board, kept between its requests; the person's comes back after each.
#[test]
fn an_agent_never_changes_the_persons_tool_selection_or_view() {
    let (mut app, tx) = app();
    let (a, _) = two_boards(&mut app);
    app.session.set_active(index_of(&app, a));
    let mine = ids(&json!({"result": app.run("shape.rectangle", json!({"x": 5, "y": 5, "width": 10, "height": 10})).unwrap()}));
    let persons = app.session.active().unwrap().selection.clone();
    assert_eq!(persons.objects, vec![vectorcraft_doc::NodeId(mine)]);
    app.sync_views();
    let view = app.views[index_of(&app, a)];
    let tool = app.session.tool_id();
    // The agent draws on the person's own board: the new object is the agent's selection, not
    // the person's.
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", rect(60));
    let its = ids(&r);
    assert_eq!(app.session.active().unwrap().selection, persons, "the person's selection is back");
    // Its next request acts on its own selection: deleting it removes its object, not the person's.
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", json!({"command": "edit.clear", "params": {}}));
    assert_eq!(r["ok"], true, "{r}");
    let d = doc(&app, a);
    assert!(d.node(vectorcraft_doc::NodeId(its)).is_none(), "its object went");
    assert!(d.node(vectorcraft_doc::NodeId(mine)).is_some(), "the person's stayed");
    assert_eq!(app.session.active().unwrap().selection, persons);
    // Select All from the agent selects for the agent only.
    request(&mut app, &tx, agent(on(a)), "engine.execute", json!({"command": "select.all", "params": {}}));
    assert_eq!(app.session.active().unwrap().selection, persons);
    // Tool, view and panel commands are the person's.
    for (method, params) in [
        ("ui.tool.select", json!({"tool": "pen"})),
        ("engine.execute", json!({"command": "tool.setOption", "params": {"tool": "pen", "key": "x", "value": 1}})),
        ("engine.execute", json!({"command": "view.zoomIn", "params": {}})),
        ("engine.execute", json!({"command": "window.panel", "params": {"panel": "layers"}})),
        ("ui.set", json!({"panel": "layers"})),
    ] {
        let r = request(&mut app, &tx, agent(on(a)), method, params);
        assert!(error(&r).starts_with("person_state"), "{method}: {r}");
    }
    assert_eq!(app.session.tool_id(), tool);
    let now = app.views[index_of(&app, a)];
    assert_eq!((now.zoom, now.center), (view.zoom, view.center), "the person's view");
}

/// Fix 5: a command that would open a dialog is refused for an agent before it runs, and the
/// person's own open dialog is never replaced, closed or answered.
#[test]
fn dialogs_never_open_for_an_agent_and_the_persons_dialog_stays() {
    let (mut app, tx) = app();
    let (a, _) = two_boards(&mut app);
    app.ui.dialog = Some(crate::state::Dialog::new("rectangle", json!({"width": 50, "height": 50})));
    for command in ["ui.newSwatch", "ui.swatchOptions"] {
        let r = request(&mut app, &tx, agent(on(a)), "engine.execute", json!({"command": command, "params": {"name": "x"}}));
        assert!(error(&r).starts_with("person_state") || error(&r).starts_with("needs_dialog"), "{command}: {r}");
    }
    for method in ["ui.dialog.confirm", "ui.dialog.cancel", "ui.dialog.set"] {
        let r = request(&mut app, &tx, agent(on(a)), method, json!({}));
        assert!(error(&r).starts_with("person_state"), "{method}: {r}");
    }
    for command in ["file.documentSetup", "type.findFont", "object.slice.divide", "file.exportAs"] {
        let r = request(&mut app, &tx, agent(on(a)), "engine.execute", json!({"command": command, "params": {}}));
        assert!(error(&r).starts_with("needs_dialog"), "{command}: {r}");
    }
    // An ordinary edit while the person's dialog is open leaves it as it was.
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", rect(10));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("rectangle"), "the person's dialog stays");
}

/// Fix 5's catalogue: every command the UI handles itself that opens a dialog when run with no
/// options is refused for agents before it runs (by family or by [`super::boards::DIALOG_COMMANDS`]).
/// A dialog opener upstream adds later fails this test until it is listed.
#[test]
fn every_dialog_opening_command_is_refused_up_front() {
    let mut ids: Vec<String> = crate::menus::UI_COMMANDS.iter().map(|c| c.0.to_string()).collect();
    ids.extend(vectorcraft_engine::command_specs().iter().map(|s| s.id.to_string()));
    ids.sort();
    ids.dedup();
    let mut open = vec![];
    for id in &ids {
        let (req, _reply) = ControlRequest::new("engine.execute", json!({"command": id, "params": {}}));
        let req = req.with_actor(agent(None));
        if super::boards::refused_for_agents(&req).is_some() {
            continue;
        }
        let (mut app, _tx) = app();
        app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::menus::run_ui_command(&mut app, id, &json!({}))));
        if ran.is_ok() && app.ui.dialog.is_some() {
            open.push(id.clone());
        }
    }
    assert!(open.is_empty(), "open a dialog but are not refused for agents; add them to DIALOG_COMMANDS: {open:?}");
}

/// Fix 6: a batch never writes a file: the export would carry the revision checked before the
/// batch's own edits, and a later failing step could not take the file back.
#[test]
fn a_batch_that_writes_a_file_is_refused() {
    let (mut app, tx) = app();
    let (a, _) = two_boards(&mut app);
    let out = std::env::temp_dir().join(format!("varos-batch-write-{}.svg", std::process::id()));
    let rev = revision(&mut app, &tx, a);
    let before = doc(&app, a);
    let batch = json!({"command": "command.batch", "params": {"commands": [
        rect(10),
        {"command": "document.export", "params": {"path": out.to_string_lossy(), "format": "svg"}},
    ]}});
    let r = request(&mut app, &tx, agent(Some(json!({"board": board_id(a), "revision": rev}))), "engine.execute", batch);
    assert!(error(&r).starts_with("write_in_batch"), "{r}");
    assert!(Arc::ptr_eq(&doc(&app, a), &before), "nothing ran");
    assert!(!out.exists());
}

#[test]
fn pages_target_edits_another_canvas_and_restores_the_persons_page() {
    let (mut app, tx) = app();
    app.run("file.new", json!({"width":200,"height":200})).unwrap();
    app.run("page.add", json!({"name":"Second"})).unwrap();
    let uid = app.session.active().unwrap().uid;
    let page = app.session.project().unwrap().pages()[1].id.clone();
    let before = app.session.doc().unwrap().doc.node_count();
    let result = request(&mut app, &tx, agent(Some(json!({"board":board_id(uid),"page":page}))), "engine.execute", rect(20));
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(app.session.project().unwrap().active_page(), 0);
    assert_eq!(app.session.doc().unwrap().doc.node_count(), before);
    assert!(app.session.project().unwrap().pages()[1].doc.doc.node_count() > before);
    let old = request(&mut app, &tx, agent(None), varos_agents::boards::LIST, json!({}));
    assert!(old["result"]["boards"][0].get("pages").is_none());
    let new = request(&mut app, &tx, agent(None), varos_agents::boards::LIST, json!({"pages":true}));
    assert_eq!(new["result"]["boards"][0]["pages"][1]["page_name"], "Second");
    let missing = request(&mut app, &tx, agent(Some(json!({"board":board_id(uid),"page":"missing"}))), "engine.execute", rect(20));
    assert_eq!(missing["ok"], false);
    assert_eq!(app.session.project().unwrap().active_page(), 0);
}
