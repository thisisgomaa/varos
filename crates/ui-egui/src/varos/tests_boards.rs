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

/// VQA-11: the person switches tabs between two queued agent commands. A verb that acts on the
/// window (keys, text, clicks, captures) aborts with `target_changed` and changes nothing in
/// either board; a document verb still lands on the board it names, never the new front.
#[test]
fn switching_the_tab_between_two_queued_commands_never_misroutes() {
    let (mut app, tx) = app();
    let (a, b) = two_boards(&mut app);
    // The agent works on A, which is in front.
    app.session.set_active(index_of(&app, a));
    let r = request(&mut app, &tx, agent(on(a)), "ui.text", json!({"text": "x"}));
    assert_eq!(r["ok"], true, "{r}");
    app.synthetic.clear();
    // The next two commands are queued; the person clicks tab B before they run.
    let typed = send(&tx, agent(on(a)), "ui.text", json!({"text": "y"}));
    let key = send(&tx, agent(on(a)), "ui.key", json!({"key": "Delete"}));
    app.session.set_active(index_of(&app, b));
    let (doc_a, doc_b) = (doc(&app, a), doc(&app, b));
    app.drain_control(&egui::Context::default());
    for reply in [typed, key] {
        let r = reply.try_recv().unwrap();
        assert!(error(&r).starts_with("target_changed"), "{r}");
    }
    assert!(app.synthetic.is_empty(), "no input reached the window");
    assert!(Arc::ptr_eq(&doc(&app, a), &doc_a) && Arc::ptr_eq(&doc(&app, b), &doc_b), "nothing changed in either board");
    assert_eq!(front(&app), b, "the person's choice stands");
    // A document command queued the same way lands on A, never on B.
    let r = request(&mut app, &tx, agent(on(a)), "engine.execute", rect(40));
    assert_eq!(r["ok"], true, "{r}");
    assert!(!Arc::ptr_eq(&doc(&app, a), &doc_a), "it ran on A");
    assert!(Arc::ptr_eq(&doc(&app, b), &doc_b), "B is untouched");
    assert_eq!(front(&app), b);
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
