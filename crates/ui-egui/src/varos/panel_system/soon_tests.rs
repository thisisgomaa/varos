//! Integration through the production workspace presenter and saved UiState loader.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::test_support::*;

#[test]
fn ai_strip_mirrors_window_panel_and_fresh_studio_keeps_the_right_dock() {
    let mut h = Harness::workspace();
    h.app.run("window.workspace", json!({"name":"Studio"})).unwrap();
    h.app.ui.varos_panel_review.as_mut().unwrap().initialize_ai();
    let out = h.settle();
    let response = h.ctx.read_response(egui::Id::unique("varos-ai-strip")).unwrap();
    assert!(response.enabled());
    let visible = texts(&out);
    assert!(visible.iter().any(|(text, _, _)| text == "Ask"));
    assert!(visible.iter().any(|(text, _, _)| text == "Agents on your server"));
    assert_eq!(h.layout().owner("ai").unwrap().dock, Some(Edge::Right));
    let before = vectorcraft_format::save(&h.app.session.active().unwrap().doc, false);
    h.click(response.rect.center());
    assert!(!super::showing(&h.app, "ai"));
    h.app.run("window.panel", json!({"panel":"ai","toggle":true})).unwrap();
    h.settle();
    assert!(super::showing(&h.app, "ai"));
    assert_eq!(vectorcraft_format::save(&h.app.session.active().unwrap().doc, false), before);
}

#[test]
fn loading_and_presenting_old_or_custom_ai_workspaces_never_changes_saved_owners() {
    for contains_ai in [false, true] {
        let mut h = Harness::workspace();
        let mut saved = serde_json::to_value(&h.app.ui).unwrap();
        let hosts = saved["varos_panel_review"]["layout"]["hosts"].as_array_mut().unwrap();
        if contains_ai {
            let host = hosts.iter_mut().find(|host| host["root"]["panels"] == json!(["ai"])).unwrap();
            host["hidden"] = json!(true);
            host["dock"] = serde_json::Value::Null;
            host["position"] = json!([250.0, 190.0]);
        } else {
            hosts.retain(|host| host["root"]["panels"] != json!(["ai"]));
        }
        h.app.ui = crate::state::UiState::read_preferences(&serde_json::to_vec(&saved).unwrap()).unwrap().ui;
        let before = h.layout().clone();
        h.settle();
        assert_eq!(h.layout(), &before);
        assert_eq!(h.layout().owner("ai").is_some(), contains_ai);
        // The mirror exists even when the old saved layout has no AI owner.
        assert!(h.ctx.read_response(egui::Id::unique("varos-ai-strip")).is_some());
    }
}

#[test]
fn historical_preference_reconstruction_never_backfills_ai() {
    use crate::state::UiState;
    let empty = UiState::read_preferences(b"{}").unwrap();
    assert!(empty.ui.varos_shell.strip.group("ai").is_none());
    for shell in [json!({"version":1}), json!({"version":7}), json!({"version":7,"strip":null})] {
        let raw = json!({"varos_shell":shell});
        let read = UiState::read_preferences(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert!(read.ui.varos_shell.strip.group("ai").is_none());
    }
    let raw = include_bytes!("../../../tests/fixtures/ui-before-canvas-layout.json");
    let read = UiState::read_preferences(raw).unwrap();
    assert!(read.intact && read.converted);
    assert!(read.ui.varos_shell.strip.group("ai").is_none());
    assert!(!read.ui.docking.as_ref().unwrap().contains(&"ai".to_owned()));
    for workspace in read.ui.custom_workspaces.iter().chain(&read.ui.workspace_changes) {
        assert!(!workspace.docking.as_ref().unwrap().contains(&"ai".to_owned()));
    }
    let explicit = json!({"docking_icons":["ai"]});
    let read = UiState::read_preferences(&serde_json::to_vec(&explicit).unwrap()).unwrap();
    assert!(read.ui.docking.as_ref().unwrap().contains(&"ai".to_owned()));
}

#[test]
fn fresh_legacy_shell_docks_ai_but_restoring_preferences_replaces_that_default() {
    let mut fresh = crate::state::UiState::default();
    crate::varos::ai::initialize_fresh(&mut fresh);
    let shell = &fresh.varos_shell;
    let id = shell.tree.find_tab("properties").unwrap();
    assert_eq!(shell.tree.find_tab("ai"), Some(id));
    assert_eq!(shell.tree.find_box(id).unwrap().front(), Some("ai"));
    assert!(shell.strip.group("ai").is_none());
    let saved = crate::state::UiState::read_preferences(b"{}").unwrap().ui;
    fresh = saved.sanitized();
    assert!(fresh.varos_shell.tree.find_tab("ai").is_none());
    assert!(fresh.varos_shell.strip.group("ai").is_none());
}
