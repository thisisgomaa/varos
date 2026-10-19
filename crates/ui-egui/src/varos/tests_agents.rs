//! Varos agents (`varos::agents`): control requests carry their agent into the undo history and
//! the activity log; the person's own steps stay untagged. Headless: requests go through
//! `drain_control` with a default egui context, as the frame drains them.

use std::sync::mpsc::Sender;

use serde_json::{Value, json};
use varos_agents::{ActorKey, Since};
use vectorcraft_engine::Session;

use crate::control::ControlRequest;
use crate::{Services, VectorcraftApp};

fn app() -> (VectorcraftApp, Sender<ControlRequest>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let app = VectorcraftApp::new(Session::new(), Services::default()).with_control(rx);
    (app, tx)
}

/// One request from `actor` (`None`: a client that names none), drained as a frame drains it.
fn request(app: &mut VectorcraftApp, tx: &Sender<ControlRequest>, actor: Option<Value>, method: &str, params: Value) -> Value {
    let (req, reply) = ControlRequest::new(method, params);
    tx.send(req.with_actor(actor)).unwrap();
    app.drain_control(&egui::Context::default());
    reply.try_recv().unwrap()
}

fn claude(session: &str) -> Option<Value> {
    Some(json!({"session": session, "profile": "claude-code", "label": "Claude Code"}))
}

fn rect(x: i32) -> Value {
    json!({"command": "shape.rectangle", "params": {"x": x, "y": 10, "width": 20, "height": 20}})
}

fn new_doc(app: &mut VectorcraftApp) {
    app.run("file.new", json!({"width": 200, "height": 200, "units": "Points", "artboards": 1})).unwrap();
}

fn uid(app: &VectorcraftApp) -> u64 {
    app.session.active().unwrap().uid
}

fn last_actor(app: &VectorcraftApp) -> Option<u64> {
    app.session.active().unwrap().history.undo.last().unwrap().actor
}

#[test]
fn an_agents_edit_is_tagged_with_the_agent() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let r = request(&mut app, &tx, claude("s1"), "engine.execute", rect(10));
    assert_eq!(r["ok"], true, "{r}");
    let key = last_actor(&app).expect("the step names its agent");
    assert_eq!(app.agents.registry.label(ActorKey(key)), "Claude Code");
    // The history stops recording for the agent once its request is answered.
    assert_eq!(app.session.active().unwrap().history.actor, None);
    // The person's next step is theirs.
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 10, "height": 10})).unwrap();
    assert_eq!(last_actor(&app), None);
}

#[test]
fn undo_and_redo_keep_who_made_the_step() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    request(&mut app, &tx, claude("s1"), "engine.execute", rect(10));
    let key = last_actor(&app);
    assert!(key.is_some());
    // The person undoes the agent's step, then redoes it.
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().history.redo.last().unwrap().actor, key);
    app.run("edit.redo", json!({})).unwrap();
    assert_eq!(last_actor(&app), key);
}

#[test]
fn an_atomic_batch_is_one_step_of_its_agent() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let steps_before = app.session.active().unwrap().history.undo.len();
    let agent = claude("s1");
    request(&mut app, &tx, agent.clone(), "engine.execute", json!({"command": "edit.undoGroup.begin", "params": {"label": "Poster"}}));
    request(&mut app, &tx, agent.clone(), "engine.execute", rect(10));
    request(&mut app, &tx, agent.clone(), "engine.execute", rect(40));
    request(&mut app, &tx, agent, "engine.execute", json!({"command": "edit.undoGroup.end", "params": {}}));
    let st = app.session.active().unwrap();
    assert_eq!(st.history.undo.len(), steps_before + 1);
    assert_eq!(st.history.undo.last().unwrap().label, "Poster");
    assert!(st.history.undo.last().unwrap().actor.is_some());
}

#[test]
fn two_sessions_of_one_agent_are_told_apart() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    request(&mut app, &tx, claude("s1"), "engine.execute", rect(10));
    let first = last_actor(&app).unwrap();
    request(&mut app, &tx, claude("s2"), "engine.execute", rect(40));
    let second = last_actor(&app).unwrap();
    assert_ne!(first, second);
    assert_eq!(app.agents.registry.label(ActorKey(first)), "Claude Code");
    assert_eq!(app.agents.registry.label(ActorKey(second)), "Claude Code 2");
    // The first session again keeps its name.
    request(&mut app, &tx, claude("s1"), "engine.execute", rect(70));
    assert_eq!(last_actor(&app), Some(first));
}

#[test]
fn a_client_that_names_no_agent_is_still_an_agent() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    request(&mut app, &tx, None, "engine.execute", rect(10));
    let key = last_actor(&app).expect("tagged");
    assert_eq!(app.agents.registry.label(ActorKey(key)), "Agent");
}

#[test]
fn the_activity_log_has_every_revision_while_an_agent_is_around() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let doc = uid(&app);
    let start = app.session.active().unwrap().revision;
    request(&mut app, &tx, claude("s1"), "engine.execute", rect(10));
    let after_agent = app.session.active().unwrap().revision;
    let created = match app.agents.activity.since(doc, start, after_agent) {
        Since::Changes(c) => c.created,
        Since::ResyncRequired => panic!("the agent's revision is logged"),
    };
    assert_eq!(created.len(), 1);
    // The person draws; the next frame logs it as theirs.
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 10, "height": 10})).unwrap();
    app.drain_control(&egui::Context::default());
    let now = app.session.active().unwrap().revision;
    let Since::Changes(c) = app.agents.activity.since(doc, start, now) else { panic!("every revision is logged") };
    assert_eq!(c.created.len(), 2);
    let entries: Vec<_> = app.agents.activity.entries(doc).collect();
    assert!(entries.first().unwrap().actor.is_some());
    assert_eq!(entries.last().unwrap().actor, None, "the person's step is theirs");
    assert_eq!(entries.first().unwrap().label, "Claude Code");
    assert_eq!(entries.first().unwrap().step, "Rectangle");
}

#[test]
fn a_failed_request_records_nothing() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let steps = app.session.active().unwrap().history.undo.len();
    let r = request(&mut app, &tx, claude("s1"), "engine.execute", json!({"command": "no.such.command", "params": {}}));
    assert_eq!(r["ok"], false);
    assert_eq!(app.session.active().unwrap().history.undo.len(), steps);
    assert_eq!(app.session.active().unwrap().history.actor, None);
    assert_eq!(app.agents.activity.entries(uid(&app)).count(), 0);
    assert!(!app.agents.presence.is_active(), "nothing changed: nothing lights up");
}

/// The presence the canvas draws, for the active document, at `now`.
fn presence(app: &mut VectorcraftApp, now: std::time::Instant) -> varos_agents::Frame {
    let st = app.session.active().unwrap();
    let selected = st.selection.objects.iter().copied().collect();
    app.agents.presence.frame(st.uid, st.revision, &st.doc, &selected, now)
}

#[test]
fn an_agents_work_lights_its_artboard_and_its_objects() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    app.run("select.none", json!({})).unwrap();
    let r = request(&mut app, &tx, claude("s1"), "engine.execute", rect(10));
    let id = r["result"]["id"].as_u64().unwrap();
    // The rectangle command selects what it draws: the person's selection wins over orange, so
    // clear it to see the agent's outline.
    app.run("select.none", json!({})).unwrap();
    let f = presence(&mut app, std::time::Instant::now());
    assert_eq!(f.pages.len(), 1, "{f:?}");
    assert_eq!(f.pages[0].labels, vec!["Claude Code".to_string()]);
    assert_eq!(f.pages[0].artboard, app.session.active().unwrap().doc.artboards[0].id);
    assert_eq!(f.objects.iter().map(|o| o.id.0).collect::<Vec<_>>(), vec![id]);
    // After the hold, nothing is left (the frame's next look drops it).
    app.agents.presence.retain(|_| true, std::time::Instant::now() + varos_agents::presence::HOLD);
    assert!(!app.agents.presence.is_active());
}

#[test]
fn the_persons_own_edits_never_light_up() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    // An agent has been around (registered), but the edit is the person's.
    request(&mut app, &tx, claude("s1"), "document.inspect", json!({}));
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 10, "height": 10})).unwrap();
    app.run("select.none", json!({})).unwrap();
    app.drain_control(&egui::Context::default());
    assert!(presence(&mut app, std::time::Instant::now()).objects.is_empty());
}

#[test]
fn history_rows_name_the_agents_steps() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 10, "height": 10})).unwrap();
    request(&mut app, &tx, claude("s1"), "engine.execute", rect(10));
    request(&mut app, &tx, claude("s2"), "engine.execute", rect(40));
    app.run("edit.undo", json!({})).unwrap();
    let st = app.session.active().unwrap();
    let rows = crate::varos::agents::history_labels(&app, &st.history);
    let n = st.history.undo.len();
    // "Open", the person's steps, the first agent's step, then the undone (redo) step of the second.
    assert_eq!(rows.len(), 1 + n + 1);
    assert_eq!(rows[0], None);
    assert_eq!(rows[n - 1], None, "the person's rectangle");
    assert_eq!(rows[n], Some("Claude Code".to_string()));
    assert_eq!(rows[n + 1], Some("Claude Code 2".to_string()));
    let orange = crate::varos::theme::AGENT;
    assert_eq!(crate::varos::agents::history_icon(rows.get(n), egui::Color32::WHITE), orange);
    assert_eq!(crate::varos::agents::history_icon(rows.first(), egui::Color32::WHITE), egui::Color32::WHITE);
}

/// An agent that came in through Varos' always-on port (the transport marks it `guarded`).
fn guarded() -> Option<Value> {
    Some(json!({"session": "g1", "profile": "claude-code", "label": "Claude Code", "guarded": true}))
}

#[test]
fn agents_through_the_varos_port_never_overwrite_or_write_outside_home() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let dir = std::env::temp_dir().join(format!("varos-ui-guard-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let existing = dir.join("taken.png");
    std::fs::write(&existing, b"keep me").unwrap();
    let path = |p: &std::path::Path| p.to_string_lossy().into_owned();
    let r = request(&mut app, &tx, guarded(), "ui.render", json!({"path": path(&existing)}));
    assert!(r["error"].as_str().unwrap().contains("never overwrite"), "{r}");
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep me");
    // The temporary folder is outside the home folder.
    let r = request(
        &mut app,
        &tx,
        guarded(),
        "engine.execute",
        json!({"command": "file.saveAs", "params": {"path": path(&dir.join("new.vectorcraft"))}}),
    );
    assert!(r["error"].as_str().unwrap().contains("home folder"), "{r}");
    let r = request(&mut app, &tx, guarded(), "app.export", json!({"path": "poster.pdf", "format": "pdf"}));
    assert!(r["error"].as_str().unwrap().contains("absolute"), "{r}");
    assert!(!dir.join("new.vectorcraft").exists());
    // Exporting without a path (the data comes back) and reading are not guarded.
    let r = request(&mut app, &tx, guarded(), "document.inspect", json!({}));
    assert_eq!(r["ok"], true, "{r}");
}

#[test]
fn upstreams_control_port_is_not_guarded() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let dir = std::env::temp_dir().join(format!("varos-ui-unguarded-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let existing = dir.join("taken.png");
    std::fs::write(&existing, b"x").unwrap();
    for actor in [claude("s1"), None] {
        let r = request(&mut app, &tx, actor, "ui.render", json!({"path": existing.to_string_lossy()}));
        let error = r["error"].as_str().unwrap_or("");
        assert!(!error.contains("never overwrite") && !error.contains("home folder"), "{r}");
    }
}
