//! The shell's saved state: `varos_shell` in `ui.json` (version 2), apart from craft-ui's
//! `docking` layout, which it never touches.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::tree::Node;

/// The saved layout's version.
pub(crate) const VERSION: u32 = 2;

/// Preferences › Varos shell (new) and the shell's box layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VarosShell {
    /// The window's central region is the Varos shell (off: craft-ui's dock, as upstream).
    pub enabled: bool,
    /// The layout's version ([`VERSION`]).
    pub version: u32,
    /// The boxes.
    pub tree: Node,
    /// The sole Board owns this strip; it is independent of ordinary panel boxes.
    pub strip: super::strip::Strip,
    #[serde(skip)]
    pub open: Option<String>,
}

impl Default for VarosShell {
    fn default() -> Self {
        Self {
            enabled: false,
            version: VERSION,
            strip: super::strip::Strip::default_for(&Node::default_layout()),
            tree: Node::default_layout(),
            open: None,
        }
    }
}

/// A panel id the app knows (an icon panel, a dock tab or a library panel).
pub(crate) fn known(id: &str) -> bool {
    crate::menus::normalize_panel(id) == Some(id)
}

/// `varos_shell` read from what `ui.json` holds: the switch kept whatever the layout; a layout of
/// another version, or one that doesn't read, is the default; unknown panel ids are dropped.
pub(crate) fn from_value(v: &Value) -> VarosShell {
    let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let tree = matches!(v.get("version").and_then(Value::as_u64), Some(1 | 2))
        .then(|| v.get("tree").and_then(|t| Node::deserialize(t).ok()))
        .flatten()
        .map_or_else(Node::default_layout, |t| t.sanitized(&known));
    let mut strip = v.get("strip").and_then(|s| super::strip::Strip::deserialize(s).ok()).unwrap_or_else(|| super::strip::Strip::default_for(&tree));
    strip.sanitize(&tree);
    VarosShell { enabled, version: VERSION, tree, strip, open: None }
}

/// Serde's reader for `UiState::varos_shell`: never fails the whole UI state.
pub(crate) fn lenient<'de, D: Deserializer<'de>>(d: D) -> Result<VarosShell, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(from_value(&v))
}

impl VarosShell {
    /// Atomic ownership transfer from strip to box, ready for Stage 3.
    #[allow(dead_code)] // Stage 3 will drive this ownership transfer through dragging.
    pub fn add_tab(&mut self, target: u32, panel: &str) -> bool {
        if !self.tree.add_tab(target, panel) {
            return false;
        }
        self.strip.remove(panel);
        if self.open.as_deref() == Some(panel) {
            self.open = None;
        }
        true
    }
    pub fn close_tab(&mut self, panel: &str) -> bool {
        if !self.tree.remove_tab(panel) {
            return false;
        }
        self.strip.append(panel);
        true
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_switch_and_the_layout_round_trip_through_ui_json() {
        let mut ui = crate::state::UiState::default();
        assert!(!ui.varos_shell.enabled, "off by default");
        ui.varos_shell.enabled = true;
        ui.varos_shell.add_tab(3, "stroke");
        ui.varos_shell.tree.set_ratio(&[], 0.61);
        let saved = serde_json::to_value(&ui).unwrap();
        assert_eq!(saved["varos_shell"]["enabled"], json!(true));
        assert_eq!(saved["varos_shell"]["version"], json!(VERSION));
        let back: crate::state::UiState = serde_json::from_value(saved).unwrap();
        assert_eq!(back.varos_shell, ui.varos_shell);
        assert!(back.docking.is_none(), "craft-ui's layout is its own");
    }

    #[test]
    fn a_bad_or_foreign_layout_never_fails_the_ui_state() {
        for raw in [json!(null), json!(3), json!({"enabled": true, "tree": "x", "version": 1}), json!({"enabled": true, "version": 99, "tree": {}})] {
            let ui: crate::state::UiState = serde_json::from_value(json!({"varos_shell": raw})).unwrap();
            assert_eq!(ui.varos_shell.tree, Node::default_layout(), "{raw}");
        }
        let ui: crate::state::UiState = serde_json::from_value(json!({"varos_shell": {"enabled": true}})).unwrap();
        assert!(ui.varos_shell.enabled, "the switch is kept without a layout");
        let ui: crate::state::UiState = serde_json::from_value(json!({})).unwrap();
        assert_eq!(ui.varos_shell, VarosShell::default());
    }

    #[test]
    fn unknown_panels_are_dropped_on_load() {
        let mut shell = VarosShell::default();
        shell.tree.add_tab(2, "stroke");
        let mut saved = serde_json::to_value(&shell).unwrap();
        let tabs = saved["tree"]["split"]["b"]["split"]["a"]["leaf"]["tabs"].as_array_mut().unwrap();
        tabs.push(json!("noSuchPanel"));
        let back = from_value(&saved);
        assert_eq!(back.tree, shell.tree);
        assert_eq!(back.tree.find_tab("noSuchPanel"), None);
    }
}
