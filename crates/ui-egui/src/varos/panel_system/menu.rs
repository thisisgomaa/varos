//! The Window menu as a mirror of the panel workspace: which items are checked and how the
//! workspace items are labelled while the workspace presents the editor. `menus.rs` asks here
//! first through one marked hook per function; `None` leaves the item to upstream and the shell.
use super::workspace;
use crate::VectorcraftApp;
use serde_json::Value;

/// The checked state of Window items the workspace owns: Control, Tools, each panel (by id or
/// display label) and the current workspace.
pub(crate) fn checked(app: &VectorcraftApp, id: &str, p: &Value) -> Option<bool> {
    if !super::enabled(app) {
        return None;
    }
    match id {
        "window.control" => Some(super::showing(app, "control")),
        "window.toolbar" => Some(super::showing(app, "tools")),
        "window.panel" => Some(p.get("panel").and_then(Value::as_str).is_some_and(|raw| {
            super::showing(app, raw.trim()) || crate::menus::normalize_panel(raw).is_some_and(|panel| super::showing(app, panel))
        })),
        "window.workspace" => {
            Some(p.get("name").and_then(Value::as_str).zip(workspace::current(app)).is_some_and(|(name, current)| name.eq_ignore_ascii_case(current)))
        }
        _ => None,
    }
}

/// "Reset <current workspace>" (untranslated, as the menu's dynamic label).
pub(crate) fn dynamic_label(app: &VectorcraftApp, id: &str) -> Option<String> {
    (id == "window.workspace.reset" && super::enabled(app)).then(|| format!("Reset {}", workspace::current(app).unwrap_or(workspace::DEFAULT)))
}

/// Translated labels: a workspace item shows its display name; Reset names the current one.
pub(crate) fn display_label(app: &VectorcraftApp, id: &str, label: &str) -> Option<String> {
    if !super::enabled(app) {
        return None;
    }
    match id {
        "window.workspace" => Some(workspace::label(label)),
        "window.workspace.reset" => {
            let name = workspace::label(workspace::current(app).unwrap_or(workspace::DEFAULT));
            Some(crate::i18n::fmt(tl!("Reset {name}"), &[("name", name.as_str())]))
        }
        _ => None,
    }
}
