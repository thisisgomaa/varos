//! Preferences as Varos runs it: a modeless window with Apply (the editor stays live while it is
//! open), and the edit draft that lets Apply send only what the user changed. Upstream's
//! `prefs_dialog` keeps the fields, the widgets and the runtime preferences, and calls in here
//! through `// Varos` hooks (`open`, `confirm`, `show`).
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

use crate::VectorcraftApp;
use crate::state::Dialog;

mod window;
pub(crate) use window::show;

/// The Preferences transaction kept beside the dialog. The setting values themselves stay in
/// `Dialog::fields`, the boundary agents read and `ui.dialog.set` writes; this holds what Apply
/// needs to know about them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreferencesDraft {
    /// The setting values at opening, or at the last Apply.
    baseline: Map<String, Value>,
    /// The fields the user edited since then, whatever their value now.
    touched: BTreeSet<String>,
    /// The document whose units the draft edits.
    document_uid: Option<u64>,
}

impl PreferencesDraft {
    fn new(app: &VectorcraftApp, baseline: Map<String, Value>) -> Self {
        Self { baseline, touched: BTreeSet::new(), document_uid: app.session.active().map(|doc| doc.uid) }
    }

    /// Record a user edit: Apply sends `key` even when it is back at the value Preferences opened
    /// with (a live change made meanwhile is overridden).
    fn touch(&mut self, key: &str) {
        if key != CATEGORY {
            self.touched.insert(key.to_string());
        }
    }
}

/// Navigation, not a setting: never applied, never compared.
pub(crate) const CATEGORY: &str = "__category";

/// Set one field of the open dialog. For Preferences it also counts as the user's edit, so UI
/// edits, Reset and agents' `ui.dialog.set` all record their edits through [`PreferencesDraft::touch`].
pub(crate) fn set_field(d: &mut Dialog, draft: &mut Option<PreferencesDraft>, key: String, value: Value) {
    if d.kind == "preferences"
        && let Some(draft) = draft
    {
        draft.touch(&key);
    }
    d.fields.insert(key, value);
}

/// The open Preferences dialog as its widgets edit it. Reads go to the dialog; every edit goes
/// through [`set_field`], so a widget's change is recorded as the user's edit the moment it
/// happens (a widget reporting the value the field already holds is no edit).
pub(crate) struct Edit<'a> {
    pub(crate) dialog: &'a mut Dialog,
    pub(crate) draft: &'a mut Option<PreferencesDraft>,
}

impl Edit<'_> {
    pub(crate) fn set(&mut self, key: &str, value: Value) {
        if self.dialog.fields.get(key) != Some(&value) {
            set_field(self.dialog, self.draft, key.to_string(), value);
        }
    }
}

impl std::ops::Deref for Edit<'_> {
    type Target = Dialog;
    fn deref(&self) -> &Dialog {
        self.dialog
    }
}

/// The draft of the Preferences dialog `prefs_dialog::open` just opened: its fields, without the
/// category, are the baseline Apply compares against.
pub(crate) fn begin(app: &mut VectorcraftApp) {
    let Some(dialog) = app.ui.dialog.as_ref().filter(|d| d.kind == "preferences") else { return };
    let mut baseline = dialog.fields.clone();
    baseline.remove(CATEGORY);
    app.ui.prefs_draft = Some(PreferencesDraft::new(app, baseline));
}

/// A draft outlives no Preferences dialog (called after a dialog closes, and each frame).
pub(crate) fn settle(app: &mut VectorcraftApp) {
    if !app.ui.dialog.as_ref().is_some_and(|d| d.kind == "preferences") {
        app.ui.prefs_draft = None;
    }
}

/// The Varos preferences Apply validates and applies with upstream's UI-only fields.
const VAROS_FIELDS: [&str; 2] = [super::panel_stack::PREF_KEY, super::shell::PREF_KEY];

/// Apply validates the edited settings before changing engine or UI state. A field the user
/// edited is applied even when it is back at its opening value; untouched draft fields never
/// overwrite settings changed in the modeless editor while Preferences was open. Cancel only
/// discards edits made after the last Apply. Units of a document that is no longer active are
/// skipped (with a status message); every other edit still applies.
pub fn apply(app: &mut VectorcraftApp) -> Result<Value, String> {
    let Some(mut d) = app.ui.dialog.clone() else { return Err("no dialog open".into()) };
    if d.kind != "preferences" {
        return Err("the open dialog is not Preferences".into());
    }
    // A dialog constructed without `open` has no draft and keeps the full-apply contract.
    let draft = app.ui.prefs_draft.as_ref();
    let mut changed: Map<String, Value> = d
        .fields
        .iter()
        .filter(|(key, _)| key.as_str() != CATEGORY)
        .filter(|(key, value)| draft.is_none_or(|draft| draft.touched.contains(*key) || draft.baseline.get(*key) != Some(*value)))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut skipped_units = false;
    if changed.contains_key("unitsGeneral")
        && let Some(draft) = draft
        && draft.document_uid != app.session.active().map(|doc| doc.uid)
    {
        changed.remove("unitsGeneral");
        skipped_units = true;
    }
    // UI-only fields are validated before prefs.set so a bad draft cannot partially apply.
    for key in ["__smartGuides", "__snapToGrid", "__taskBar"].into_iter().chain(VAROS_FIELDS) {
        if changed.get(key).is_some_and(|value| !value.is_boolean()) {
            return Err(format!("{key}: expected a boolean"));
        }
    }
    let values: Map<String, Value> =
        changed.iter().filter(|(key, _)| !key.starts_with("__")).map(|(key, value)| (key.clone(), value.clone())).collect();
    let touch_ws = values.contains_key("touchWorkspace");
    if !values.is_empty() {
        app.run("prefs.set", json!({ "values": values }))?;
    }
    if touch_ws {
        crate::workspaces::sync_touch_workspace(app, app.touch_screen);
    }
    if let Some(on) = changed.get("__smartGuides").and_then(Value::as_bool) {
        app.ui.view.smart_guides = on;
    }
    if let Some(on) = changed.get("__snapToGrid").and_then(Value::as_bool) {
        app.ui.view.snap_to_grid = on;
    }
    if let Some(on) = changed.get("__taskBar").and_then(Value::as_bool) {
        app.ui.task_bar = on;
    }
    if let Some(on) = changed.get(super::panel_stack::PREF_KEY).and_then(Value::as_bool) {
        super::panel_stack::set_enabled(app, on);
    }
    if let Some(on) = changed.get(super::shell::PREF_KEY).and_then(Value::as_bool) {
        super::shell::set_enabled(app, on);
    }
    // Refresh untouched values from the live editor and establish the next Apply baseline.
    // Keep edited representations intact (for example an entered integer vs an engine float).
    let mut next = crate::prefs_dialog::current_fields(app);
    for (key, value) in &changed {
        if next.contains_key(key) {
            next.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in &next {
        d.fields.insert(key.clone(), value.clone());
    }
    // A fresh edit record against the new baseline; the draft now edits the active document's units.
    app.ui.prefs_draft = Some(PreferencesDraft::new(app, next));
    app.ui.dialog = Some(d);
    app.ui.engine_prefs = app.session.prefs.to_json();
    app.prefs_changed = true;
    if skipped_units {
        let message = tl!("The active document changed, so its units were not changed. The other settings were applied.");
        app.status(message);
        return Ok(json!({ "skipped": ["unitsGeneral"], "message": message }));
    }
    Ok(Value::Null)
}

/// OK applies the same validated draft as Apply, then closes Preferences.
pub fn confirm(app: &mut VectorcraftApp) -> Result<Value, String> {
    let result = apply(app)?;
    app.ui.dialog = None;
    app.ui.prefs_draft = None;
    Ok(result)
}

#[cfg(test)]
mod tests;
