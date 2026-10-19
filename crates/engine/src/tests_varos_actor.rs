//! Varos: undo steps carry the agent that made them (`History::actor`, `Session::set_actor`), in
//! whichever document the request edits, and an open undo group never takes in another actor's
//! edits (the person's scrub, an agent's atomic batch).

use serde_json::json;

use crate::Session;

fn actors(s: &Session) -> Vec<Option<u64>> {
    s.doc().unwrap().history.undo.iter().map(|e| e.actor).collect()
}

fn labels(s: &Session) -> Vec<String> {
    s.doc().unwrap().history.undo.iter().map(|e| e.label.clone()).collect()
}

fn rect(s: &mut Session, x: i32) {
    s.execute("shape.rectangle", &json!({"x": x, "y": 10, "width": 20, "height": 20})).unwrap();
}

/// The person scrubs a numeric field (an undo group) while an agent's request edits: the agent's
/// edit is a step of its own, and the person's scrub goes on in a step after it.
#[test]
fn the_persons_undo_group_never_takes_in_an_agents_edit() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    rect(&mut s, 10);
    let base = s.doc().unwrap().history.undo.len();
    s.begin_undo_group();
    s.execute("object.setBounds", &json!({"width": 30})).unwrap();
    s.execute("object.setBounds", &json!({"width": 31})).unwrap();
    s.set_actor(Some(7));
    rect(&mut s, 100);
    s.set_actor(None);
    s.execute("object.setBounds", &json!({"width": 32})).unwrap();
    s.end_undo_group(false);
    assert_eq!(actors(&s)[base..], [None, Some(7), None]);
    // Undo takes back the person's scrub after the agent's step, then the agent's step alone.
    let count = |s: &Session| s.doc().unwrap().doc.layers[0].children().map_or(0, Vec::len);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(count(&s), 2);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(count(&s), 1, "the agent's rectangle is its own step");
}

/// An agent's atomic batch (`edit.undoGroup.begin` … `end`) spans several requests; the person's
/// edit in between is the person's step, never part of the agent's.
#[test]
fn an_agents_undo_group_never_takes_in_the_persons_edit() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    let base = s.doc().unwrap().history.undo.len();
    s.set_actor(Some(3));
    s.execute("edit.undoGroup.begin", &json!({"label": "Poster"})).unwrap();
    rect(&mut s, 10);
    rect(&mut s, 40);
    s.set_actor(None);
    rect(&mut s, 70);
    s.set_actor(Some(3));
    rect(&mut s, 100);
    s.execute("edit.undoGroup.end", &json!({})).unwrap();
    s.set_actor(None);
    assert_eq!(actors(&s)[base..], [Some(3), None, Some(3)]);
    assert_eq!(labels(&s)[base..], ["Poster", "Rectangle", "Poster"]);
    // Without anyone else's edit in between, the group stays one step.
    s.set_actor(Some(3));
    s.execute("edit.undoGroup.begin", &json!({"label": "Logo"})).unwrap();
    rect(&mut s, 130);
    rect(&mut s, 160);
    s.execute("edit.undoGroup.end", &json!({})).unwrap();
    s.set_actor(None);
    assert_eq!(actors(&s)[base..], [Some(3), None, Some(3), Some(3)]);
    assert_eq!(labels(&s).last().map(String::as_str), Some("Logo"));
}

/// A request that makes a new document and edits it tags the new document's steps too, and the
/// person's steps after it are the person's, in every document.
#[test]
fn the_actor_follows_the_document_a_request_makes() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    let first = s.active_index().unwrap();
    s.set_actor(Some(5));
    s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
    assert_ne!(s.active_index(), Some(first), "the new document is active");
    rect(&mut s, 10);
    assert_eq!(actors(&s).last(), Some(&Some(5)));
    s.set_actor(None);
    assert!(s.documents().iter().all(|d| d.history.actor.is_none()));
    rect(&mut s, 40);
    assert_eq!(actors(&s).last(), Some(&None));
}
