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
    let app = VectorcraftApp::new(Session::new(), Services::default()).with_control(rx).with_agent_guards();
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

/// A home folder of the test's own (with a Desktop), as the guards' home.
fn test_home(app: &mut VectorcraftApp, tag: &str) -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("varos-ui-home-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("Desktop")).unwrap();
    let home = std::fs::canonicalize(home).unwrap();
    app.agents.places = Some(varos_agents::guard::Places { home: home.clone(), app: None });
    home
}

fn s(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A save hidden inside a batch is checked like one sent alone.
#[test]
fn the_steps_of_a_batch_are_guarded_too() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let home = test_home(&mut app, "batch");
    let taken = home.join("Desktop/taken.vectorcraft");
    std::fs::write(&taken, b"keep me").unwrap();
    let batch = json!({"command": "command.batch", "params": {"commands": [
        {"command": "shape.rectangle", "params": {"x": 0, "y": 0, "width": 5, "height": 5}},
        {"command": "file.saveCopy", "params": {"path": s(&taken)}},
    ]}});
    let r = request(&mut app, &tx, guarded(), "engine.execute", batch);
    assert!(r["error"].as_str().unwrap().contains("never overwrite"), "{r}");
    assert_eq!(std::fs::read(&taken).unwrap(), b"keep me");
    // Nested one more level, and through action.batch's steps.
    let nested = json!({"command": "command.batch", "params": {"commands": [
        {"command": "command.batch", "params": {"commands": [{"command": "file.print", "params": {"path": "/etc/varos.pdf"}}]}},
    ]}});
    assert!(request(&mut app, &tx, guarded(), "engine.execute", nested)["error"].as_str().unwrap().contains("home folder"));
    let action = |destination: &str, step: Value| json!({"command": "action.batch", "params": {"source": "datasets", "destination": destination, "commands": [step]}});
    let plain = json!({"command": "select.all", "params": {}});
    let r = request(&mut app, &tx, guarded(), "engine.execute", action("saveAndClose", plain.clone()));
    assert!(r["error"].as_str().unwrap().contains("destination `none`"), "{r}");
    let save = json!({"command": "document.export", "params": {"path": s(&home.join("Desktop/out.png"))}});
    let r = request(&mut app, &tx, guarded(), "engine.execute", action("none", save));
    assert!(r["error"].as_str().unwrap().contains("once per file"), "{r}");
}

/// Export for Screens names its files and subfolders itself: only into an empty folder.
#[test]
fn a_set_of_files_goes_only_into_an_empty_folder() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let home = test_home(&mut app, "screens");
    let out = home.join("Desktop/out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("Artboard 1.png"), b"keep me").unwrap();
    let screens = json!({"command": "document.exportForScreens", "params": {"folder": s(&out)}});
    let r = request(&mut app, &tx, guarded(), "engine.execute", screens);
    assert!(r["error"].as_str().unwrap().contains("not empty"), "{r}");
    assert_eq!(std::fs::read(out.join("Artboard 1.png")).unwrap(), b"keep me");
    // Into an empty one it runs, subfolders and all.
    let empty = home.join("Desktop/empty");
    std::fs::create_dir_all(&empty).unwrap();
    let screens = json!({"command": "document.exportForScreens", "params": {"folder": s(&empty)}});
    let r = request(&mut app, &tx, guarded(), "engine.execute", screens);
    assert_eq!(r["ok"], true, "{r}");
    assert!(std::fs::read_dir(&empty).unwrap().next().is_some(), "the files were written");
}

/// The name a command really writes is checked where it writes it: Save as Template puts its own
/// extension on the path it is given, and a file already there under that name is never replaced.
#[test]
fn the_written_name_is_checked_after_the_command_renames_it() {
    // The desktop's file writer, through which the app's own Save and Export write.
    let services = Services { write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))), ..Services::default() };
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = VectorcraftApp::new(Session::new(), services).with_control(rx).with_agent_guards();
    new_doc(&mut app);
    let home = test_home(&mut app, "template");
    let given = home.join("Desktop/poster.png");
    let written = home.join("Desktop/poster.vctemplate");
    std::fs::write(&written, b"keep me").unwrap();
    let template = json!({"command": "file.saveAsTemplate", "params": {"path": s(&given)}});
    let r = request(&mut app, &tx, guarded(), "engine.execute", template.clone());
    assert!(r["error"].as_str().unwrap().contains("never overwrite"), "{r}");
    assert_eq!(std::fs::read(&written).unwrap(), b"keep me");
    // Once that name is free the same request writes it, and the person may still write anywhere.
    std::fs::remove_file(&written).unwrap();
    let r = request(&mut app, &tx, guarded(), "engine.execute", template);
    assert_eq!(r["ok"], true, "{r}");
    assert!(written.exists());
    app.run("file.saveAsTemplate", json!({"path": s(&given)})).unwrap();
}

/// Every command of the registry that takes a file or folder parameter is either known to only
/// read there, or guarded: a path outside home is refused for it before anything runs. A command
/// upstream adds later is guarded until someone lists it as a reader.
#[test]
fn every_command_that_takes_a_path_is_guarded_or_only_reads() {
    use crate::varos::agents::{PATH_KEYS, READS, check_command, param_keys, writes_files};
    let home = std::env::temp_dir().join(format!("varos-ui-registry-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let places = varos_agents::guard::Places { home: std::fs::canonicalize(&home).unwrap(), app: None };
    let mut guarded = vec![];
    for spec in vectorcraft_engine::command_specs() {
        let keys: Vec<&str> = param_keys(spec.params, 0).into_iter().filter(|k| PATH_KEYS.contains(k)).collect();
        let Some(key) = keys.first() else { continue };
        if READS.contains(&spec.id) {
            continue;
        }
        assert!(writes_files(spec.id), "{} takes `{key}` but is not guarded", spec.id);
        let outside = if key.ends_with('s') { json!(["/etc/varos-guard-test"]) } else { json!("/etc/varos-guard-test") };
        let params = match spec.id {
            // Its own refusal comes first; its steps are checked as commands of their own.
            "action.batch" => json!({"destination": "folder", "destinationFolder": outside}),
            _ => json!({ *key: outside }),
        };
        assert!(check_command(spec.id, &params, Some(&places), None, 0).is_err(), "{} lets an agent write to {params}", spec.id);
        guarded.push(spec.id);
    }
    // The writers the review named are among them.
    for id in ["file.print", "links.unembed", "file.saveAs", "file.saveAsTemplate", "document.export", "document.exportForScreens", "file.package"] {
        assert!(guarded.contains(&id), "{id} is not guarded");
    }
    // Every listed reader is a command that takes a path: the list holds nothing stale.
    let stale: Vec<&str> = READS
        .iter()
        .copied()
        .filter(|id| vectorcraft_engine::find_command(id).is_none_or(|spec| !param_keys(spec.params, 0).iter().any(|k| PATH_KEYS.contains(k))))
        .collect();
    assert!(stale.is_empty(), "not commands that take a path: drop them from READS: {stale:?}");
}

/// A request that makes a new document and draws in it: the steps and the activity are the
/// agent's in the new document, and the canvas shows the work there.
#[test]
fn a_request_that_makes_a_document_tags_that_document() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    let first = uid(&app);
    let batch = json!({"command": "command.batch", "params": {"commands": [
        {"command": "file.new", "params": {"width": 300, "height": 300}},
        {"command": "shape.rectangle", "params": {"x": 10, "y": 10, "width": 20, "height": 20}},
    ]}});
    let r = request(&mut app, &tx, claude("s1"), "engine.execute", batch);
    assert_eq!(r["ok"], true, "{r}");
    let made = uid(&app);
    assert_ne!(made, first, "the new document is in front");
    let key = last_actor(&app).expect("the new document's step names the agent");
    assert_eq!(app.agents.registry.label(ActorKey(key)), "Claude Code");
    assert!(app.session.documents().iter().all(|d| d.history.actor.is_none()), "no document records for the agent afterwards");
    let entries: Vec<_> = app.agents.activity.entries(made).collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].actor, Some(ActorKey(key)));
    assert!(!entries[0].change.created.is_empty());
    let now = app.session.active().unwrap().revision;
    assert!(matches!(app.agents.activity.since(made, 0, now), Since::Changes(_)), "describe since 0 works for the new document");
    // The person's next step there is theirs.
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 10, "height": 10})).unwrap();
    assert_eq!(last_actor(&app), None);
    app.run("select.none", json!({})).unwrap();
    let f = presence(&mut app, std::time::Instant::now());
    assert_eq!(f.pages.len(), 1, "the new document's artboard lights up: {f:?}");
}

/// Restacking reaches the activity log, though nothing lights up for it.
#[test]
fn a_restack_is_logged() {
    let (mut app, tx) = app();
    new_doc(&mut app);
    app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 10, "height": 10})).unwrap();
    app.run("shape.rectangle", json!({"x": 30, "y": 10, "width": 10, "height": 10})).unwrap();
    let doc = uid(&app);
    let r = request(&mut app, &tx, claude("s1"), "engine.execute", json!({"command": "object.arrange.sendToBack", "params": {}}));
    assert_eq!(r["ok"], true, "{r}");
    let entry = app.agents.activity.entries(doc).last().unwrap();
    assert!(!entry.change.reordered.is_empty(), "{:?}", entry.change);
}

/// An app whose Save, Export and captures write real files (through the guards).
fn app_with_writer() -> (VectorcraftApp, Sender<ControlRequest>) {
    let services = Services { write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))), ..Services::default() };
    let (tx, rx) = std::sync::mpsc::channel();
    (VectorcraftApp::new(Session::new(), services).with_control(rx).with_agent_guards(), tx)
}

/// The document's own file may be written over only by a save of the document: a template whose
/// renamed path lands on that file is refused.
#[test]
fn only_a_save_writes_over_the_documents_own_file() {
    let (mut app, tx) = app_with_writer();
    new_doc(&mut app);
    let home = test_home(&mut app, "own");
    let own = home.join("Desktop/poster.vctemplate");
    std::fs::write(&own, b"keep me").unwrap();
    app.session.active_mut().unwrap().path = Some(s(&own));
    let template = json!({"command": "file.saveAsTemplate", "params": {"path": s(&home.join("Desktop/poster.png"))}});
    let r = request(&mut app, &tx, guarded(), "engine.execute", template);
    assert!(r["error"].as_str().unwrap_or("").contains("never overwrite"), "{r}");
    assert_eq!(std::fs::read(&own).unwrap(), b"keep me");
}

/// A window capture is written frames after its request: it is still held to that request's
/// guards, so two captures asked for at the same new path don't both write it.
#[test]
fn a_capture_written_later_keeps_its_requests_guard() {
    let (mut app, tx) = app_with_writer();
    new_doc(&mut app);
    let home = test_home(&mut app, "capture");
    let shot = home.join("Desktop/shot.png");
    let mut replies = vec![];
    for _ in 0..2 {
        let (req, reply) = ControlRequest::new("ui.screenshot", json!({"path": s(&shot), "focus": false}));
        tx.send(req.with_actor(guarded())).unwrap();
        replies.push(reply);
    }
    app.drain_control(&egui::Context::default());
    assert!(replies.iter().all(|r| r.try_recv().is_err()), "both wait for a frame");
    let first = app.screenshot_token;
    let image = std::sync::Arc::new(egui::ColorImage::new([2, 2], vec![egui::Color32::WHITE; 4]));
    let events = [first - 1, first]
        .into_iter()
        .map(|token| egui::Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::new(token), image: image.clone() })
        .collect();
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput { events, ..Default::default() });
    app.collect_screenshots(&ctx);
    ctx.end_pass().textures_delta.clear();
    let answers: Vec<Value> = replies.iter().map(|r| r.try_recv().unwrap()).collect();
    assert_eq!(answers[0]["ok"], true, "{}", answers[0]);
    assert!(answers[1]["error"].as_str().unwrap_or("").contains("never overwrite"), "{}", answers[1]);
    assert!(shot.exists());
}
