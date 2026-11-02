//! The Varos Preferences workflow: the modeless window, Apply, and the edit draft.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::Services;
use crate::prefs_dialog::{open, show};
use vectorcraft_engine::Session;

fn app() -> VectorcraftApp {
    VectorcraftApp::new(Session::new(), Services::default())
}

/// A widget's edit is recorded through the draft setter the moment it happens: a click on a
/// checkbox marks it touched (so Apply sends it even back at its opening value); a widget
/// reporting the value the field already holds, and category navigation, are no edits.
#[test]
fn preference_widgets_record_their_edits_through_the_draft_setter() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut a = app();
    open(&mut a, Some("General"));
    let mut dialog = a.ui.dialog.take().unwrap();
    let opening = dialog.bool("showToolTips");
    let touched = |draft: &Option<PreferencesDraft>| draft.as_ref().unwrap().touched.iter().cloned().collect::<Vec<_>>();
    let mut d = Edit { dialog: &mut dialog, draft: &mut a.ui.prefs_draft };
    d.set("showToolTips", json!(opening));
    d.set(CATEGORY, json!("General"));
    assert!(touched(d.draft).is_empty(), "no value changed, and navigation is not a setting");
    a.ui.dialog = Some(dialog);
    let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).with_context_setup(crate::theme::install_fonts).build_ui_state(
        |ui, a: &mut VectorcraftApp| {
            if a.ui.dialog.is_some() {
                show(a, ui.ctx());
            }
        },
        a,
    );
    h.run_steps(3);
    for _ in 0..2 {
        h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Show Tool Tips").click();
        h.run_steps(2);
    }
    assert_eq!(h.state().ui.dialog.as_ref().unwrap().bool("showToolTips"), opening, "clicked twice: back at the opening value");
    assert_eq!(touched(&h.state().ui.prefs_draft), ["showToolTips"], "still the user's edit");
    let mut a = h.into_state();
    a.session.prefs.show_tool_tips = !opening;
    apply(&mut a).unwrap();
    assert_eq!(a.session.prefs.show_tool_tips, opening, "Apply sends the touched field over a change made meanwhile");
}

#[test]
fn apply_keeps_the_draft_and_category_then_cancel_preserves_applied_values() {
    let mut a = app();
    open(&mut a, Some("Smart Guides"));
    let d = a.ui.dialog.as_mut().unwrap();
    d.fields.insert("keyboardIncrement".into(), json!(4));
    d.fields.insert("__smartGuides".into(), json!(false));
    let draft = d.fields.clone();
    apply(&mut a).unwrap();
    let after = &a.ui.dialog.as_ref().unwrap().fields;
    for (key, value) in &draft {
        assert_eq!(after.get(key), Some(value), "draft field {key}");
    }
    assert_eq!(a.session.prefs.keyboard_increment, 4.0);
    assert!(!a.ui.view.smart_guides);
    assert_eq!(a.ui.engine_prefs["keyboardIncrement"], json!(4.0));
    assert!(a.prefs_changed);
    let d = a.ui.dialog.as_mut().unwrap();
    d.fields.insert("keyboardIncrement".into(), json!(9));
    d.fields.insert("__smartGuides".into(), json!(true));
    crate::dialogs::cancel(&mut a);
    assert!(a.ui.dialog.is_none());
    assert_eq!(a.session.prefs.keyboard_increment, 4.0);
    assert!(!a.ui.view.smart_guides);
    assert_eq!(a.ui.engine_prefs["keyboardIncrement"], json!(4.0));
}

#[test]
fn modeless_apply_merges_only_edited_fields_with_live_editor_preferences() {
    let mut a = app();
    open(&mut a, Some("General"));
    a.ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(4));
    let smart = !a.ui.view.smart_guides;
    let grid = !a.ui.view.snap_to_grid;
    a.run("prefs.set", json!({"key": "gridlineEvery", "value": 73})).unwrap();
    a.ui.view.smart_guides = smart;
    a.ui.view.snap_to_grid = grid;
    apply(&mut a).unwrap();
    assert_eq!(a.session.prefs.keyboard_increment, 4.0);
    assert_eq!(a.session.prefs.gridline_every, 73.0);
    assert_eq!(a.ui.view.smart_guides, smart);
    assert_eq!(a.ui.view.snap_to_grid, grid);
    let d = a.ui.dialog.as_ref().unwrap();
    assert_eq!(d.fields["gridlineEvery"], json!(73.0));
    assert_eq!(d.fields["__smartGuides"], json!(smart));
    assert_eq!(d.fields["__snapToGrid"], json!(grid));
    // The next Apply also leaves an externally changed, untouched field alone.
    a.run("prefs.set", json!({"key": "keyboardIncrement", "value": 7})).unwrap();
    apply(&mut a).unwrap();
    assert_eq!(a.session.prefs.keyboard_increment, 7.0);
    assert_eq!(a.ui.dialog.as_ref().unwrap().fields["keyboardIncrement"], json!(7.0));
}

#[test]
fn modeless_units_edit_for_a_document_no_longer_active_is_skipped_and_the_rest_applies() {
    let mut a = app();
    a.run("file.new", json!({"width": 200, "height": 200})).unwrap();
    open(&mut a, Some("Units"));
    let original_units = a.ui.dialog.as_ref().unwrap().fields["unitsGeneral"].clone();
    let changed_units = if original_units == "millimeters" { "points" } else { "millimeters" };
    a.ui.dialog.as_mut().unwrap().fields.insert("unitsGeneral".into(), json!(changed_units));
    a.ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(4));
    a.run("file.new", json!({"width": 300, "height": 200})).unwrap();
    let units: Vec<_> = a.session.documents().iter().map(|doc| doc.doc.units).collect();
    let active = a.session.active_index();
    let result = apply(&mut a).unwrap();
    assert_eq!(result["skipped"], json!(["unitsGeneral"]), "the units edit is reported as skipped");
    assert!(a.ui.status.contains("active document changed"), "{}", a.ui.status);
    assert_eq!(a.session.prefs.keyboard_increment, 4.0, "unrelated edits apply");
    assert_eq!(a.session.active_index(), active);
    assert_eq!(a.session.documents().iter().map(|doc| doc.doc.units).collect::<Vec<_>>(), units, "no document's units change");
    let d = a.ui.dialog.as_ref().unwrap();
    assert_eq!(d.str("__category"), "Units");
    assert_eq!(a.ui.prefs_draft.as_ref().unwrap().document_uid, a.session.active().map(|doc| doc.uid), "the draft now edits the active document");
    assert_eq!(d.fields["unitsGeneral"], json!(a.session.active().unwrap().doc.units.key()), "the units field shows the active document's");
    // A units edit made now applies to the active document.
    a.ui.dialog.as_mut().unwrap().fields.insert("unitsGeneral".into(), json!(changed_units));
    assert!(apply(&mut a).unwrap().is_null());
    assert_eq!(a.session.active().unwrap().doc.units.key(), changed_units);
}

/// A field set back to the value Preferences opened with still applies when the user touched
/// it, over a change made elsewhere meanwhile; an untouched field still keeps that change.
#[test]
fn a_touched_field_back_at_its_opening_value_overrides_a_live_change() {
    let mut a = app();
    open(&mut a, Some("General"));
    let opening = a.session.prefs.keyboard_increment;
    a.run("prefs.set", json!({"key": "keyboardIncrement", "value": opening + 6.0})).unwrap();
    a.run("prefs.set", json!({"key": "gridlineEvery", "value": 73})).unwrap();
    let d = a.ui.dialog.as_mut().unwrap();
    d.fields.insert("keyboardIncrement".into(), json!(opening + 4.0));
    d.fields.insert("keyboardIncrement".into(), json!(opening));
    set_field(d, &mut a.ui.prefs_draft, "keyboardIncrement".into(), json!(opening));
    apply(&mut a).unwrap();
    assert_eq!(a.session.prefs.keyboard_increment, opening, "the touched field wins");
    assert_eq!(a.session.prefs.gridline_every, 73.0, "the untouched field keeps the live change");
    assert!(a.ui.prefs_draft.as_ref().unwrap().touched.is_empty(), "Apply starts a fresh edit record");
}

/// Agents see only setting values and the category; the transaction stays typed beside them.
#[test]
fn the_dialog_fields_carry_no_transaction_metadata_and_agent_edits_count_as_touched() {
    let mut a = app();
    open(&mut a, Some("General"));
    let d = a.ui.dialog.as_mut().unwrap();
    let opening = d.fields["keyboardIncrement"].clone();
    set_field(d, &mut a.ui.prefs_draft, "keyboardIncrement".into(), opening);
    let fields = &a.ui.dialog.as_ref().unwrap().fields;
    for meta in ["__baseline", "__touched", "__documentUid"] {
        assert!(!fields.contains_key(meta), "{meta} stays out of the agent-visible fields");
    }
    assert!(a.ui.prefs_draft.as_ref().unwrap().touched.contains("keyboardIncrement"));
    crate::dialogs::cancel(&mut a);
    assert!(a.ui.prefs_draft.is_none(), "no draft outlives its dialog");
}

#[test]
fn invalid_apply_leaves_engine_ui_and_saved_preferences_unchanged() {
    let mut a = app();
    open(&mut a, Some("User Interface"));
    let before = a.session.prefs.clone();
    let saved = a.ui.engine_prefs.clone();
    let smart = a.ui.view.smart_guides;
    let changed = a.prefs_changed;
    let d = a.ui.dialog.as_mut().unwrap();
    d.fields.insert("keyboardIncrement".into(), json!(4));
    d.fields.insert("anchorSize".into(), json!(42));
    d.fields.insert("__smartGuides".into(), json!(!smart));
    let draft = d.fields.clone();
    assert!(apply(&mut a).is_err());
    assert_eq!(a.session.prefs, before);
    assert_eq!(a.ui.engine_prefs, saved);
    assert_eq!(a.ui.view.smart_guides, smart);
    assert_eq!(a.prefs_changed, changed);
    assert_eq!(a.ui.dialog.as_ref().unwrap().fields, draft);
}

#[test]
fn preferences_apply_button_stays_open_and_outside_clicks_are_modeless() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut a = app();
    open(&mut a, Some("General"));
    a.ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(6));
    let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).with_context_setup(crate::theme::install_fonts).build_ui_state(
        |ui, a: &mut VectorcraftApp| {
            if ui.button("Outside Preferences").clicked() {
                a.status("outside click reached editor");
            }
            if a.ui.dialog.is_some() {
                show(a, ui.ctx());
            }
        },
        a,
    );
    h.run_steps(3);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Apply").click();
    h.run_steps(3);
    assert_eq!(h.state().session.prefs.keyboard_increment, 6.0);
    assert_eq!(h.state().ui.dialog.as_ref().unwrap().str("__category"), "General");
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Outside Preferences").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.status, "outside click reached editor");
    assert!(h.state().ui.dialog.is_some());
    h.state_mut().ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(8));
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Cancel").click();
    h.run_steps(3);
    assert!(h.state().ui.dialog.is_none());
    assert_eq!(h.state().session.prefs.keyboard_increment, 6.0);
}
