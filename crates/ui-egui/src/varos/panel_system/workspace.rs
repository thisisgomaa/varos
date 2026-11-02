//! Saved panel arrangements, separate from the legacy shell and document state.

use super::model::Layout;
use crate::{VectorcraftApp, state::Dialog};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const DEFAULT: &str = "Design";
pub const STUDIO: &str = "Studio";
const LIMIT: usize = 64;
const NAME_LIMIT: usize = 128;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub name: String,
    pub layout: Layout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub current: String,
    pub saved: Vec<Saved>,
    pub changes: Vec<Saved>,
}

impl Default for Store {
    fn default() -> Self {
        Self { current: DEFAULT.into(), saved: Vec::new(), changes: Vec::new() }
    }
}

pub fn is_builtin(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case(DEFAULT) || name.trim().eq_ignore_ascii_case(STUDIO)
}

pub fn label(name: &str) -> String {
    if name.trim().eq_ignore_ascii_case(DEFAULT) {
        tl!("Design").to_string()
    } else if name.trim().eq_ignore_ascii_case(STUDIO) {
        tl!("Studio").to_string()
    } else {
        name.to_string()
    }
}

pub fn current(app: &VectorcraftApp) -> Option<&str> {
    super::presenter::state(&app.ui).map(|s| s.workspaces.current.as_str())
}

pub fn names(app: &VectorcraftApp) -> Option<Vec<String>> {
    super::presenter::state(&app.ui).map(|s| s.workspaces.names())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().count() <= NAME_LIMIT && !name.chars().any(char::is_control)
}

impl Store {
    pub fn names(&self) -> Vec<String> {
        [DEFAULT.to_string(), STUDIO.to_string()].into_iter().chain(self.saved.iter().map(|s| s.name.clone())).collect()
    }

    fn baseline(&self, name: &str) -> Result<Saved, String> {
        if name.trim().eq_ignore_ascii_case(DEFAULT) {
            Ok(Saved { name: DEFAULT.into(), layout: Layout::new_workspace() })
        } else if name.trim().eq_ignore_ascii_case(STUDIO) {
            Ok(Saved { name: STUDIO.into(), layout: Layout::studio_workspace()? })
        } else {
            self.saved.iter().find(|s| s.name.eq_ignore_ascii_case(name.trim())).cloned().ok_or_else(|| format!("Unknown workspace `{name}`"))
        }
    }

    fn remember(&mut self, layout: &Layout) -> Result<(), String> {
        let baseline = self.baseline(&self.current)?;
        self.changes.retain(|s| !s.name.eq_ignore_ascii_case(&baseline.name));
        if layout != &baseline.layout {
            self.changes.push(Saved { name: baseline.name, layout: layout.clone() });
        }
        let excess = self.changes.len().saturating_sub(LIMIT);
        self.changes.drain(..excess);
        Ok(())
    }

    fn switch(&mut self, layout: &mut Layout, name: &str) -> Result<(), String> {
        let baseline = self.baseline(name)?;
        if baseline.name.eq_ignore_ascii_case(&self.current) {
            return layout.validate();
        }
        let next = self.changes.iter().find(|s| s.name.eq_ignore_ascii_case(&baseline.name)).unwrap_or(&baseline).clone();
        next.layout.validate()?;
        layout.validate()?;
        self.remember(layout)?;
        self.current = baseline.name;
        *layout = next.layout;
        Ok(())
    }

    fn reset(&mut self, layout: &mut Layout) -> Result<(), String> {
        let saved = self.baseline(&self.current)?;
        saved.layout.validate()?;
        self.changes.retain(|s| !s.name.eq_ignore_ascii_case(&saved.name));
        self.current = saved.name;
        *layout = saved.layout;
        Ok(())
    }

    fn save(&mut self, layout: &Layout, name: &str) -> Result<(), String> {
        let name = name.trim();
        if !valid_name(name) || is_builtin(name) {
            return Err("Choose a nonempty workspace name of at most 128 characters, other than Design or Studio".into());
        }
        layout.validate()?;
        let existing = self.saved.iter().position(|s| s.name.eq_ignore_ascii_case(name));
        if existing.is_none() && self.saved.len() >= LIMIT {
            return Err("At most 64 saved workspaces are supported".into());
        }
        if !self.current.eq_ignore_ascii_case(name) {
            self.remember(layout)?;
        }
        let saved = Saved { name: name.into(), layout: layout.clone() };
        if let Some(existing) = existing.and_then(|i| self.saved.get_mut(i)) {
            *existing = saved;
        } else {
            self.saved.push(saved);
        }
        self.changes.retain(|s| !s.name.eq_ignore_ascii_case(name));
        self.current = name.into();
        Ok(())
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<(), String> {
        let from = from.trim();
        let to = to.trim();
        if !valid_name(to) || is_builtin(to) || self.saved.iter().any(|s| s.name.eq_ignore_ascii_case(to) && !s.name.eq_ignore_ascii_case(from)) {
            return Err(format!("`{to}` is not an available workspace name"));
        }
        let saved = self.saved.iter_mut().find(|s| s.name.eq_ignore_ascii_case(from)).ok_or_else(|| format!("No user workspace `{from}`"))?;
        saved.name = to.into();
        for changed in self.changes.iter_mut().filter(|s| s.name.eq_ignore_ascii_case(from)) {
            changed.name = to.into();
        }
        if self.current.eq_ignore_ascii_case(from) {
            self.current = to.into();
        }
        Ok(())
    }

    fn delete(&mut self, layout: &mut Layout, name: &str) -> Result<(), String> {
        let name = name.trim();
        if !self.saved.iter().any(|s| s.name.eq_ignore_ascii_case(name)) {
            return Err(format!("No user workspace `{name}`"));
        }
        // Deleting the active workspace restores Design as it was left, without recording
        // the deleted layout under Design's name.
        let fallback = if self.current.eq_ignore_ascii_case(name) {
            let next = self.changes.iter().find(|s| s.name.eq_ignore_ascii_case(DEFAULT)).map_or_else(Layout::new_workspace, |s| s.layout.clone());
            next.validate()?;
            Some(next)
        } else {
            None
        };
        self.saved.retain(|s| !s.name.eq_ignore_ascii_case(name));
        self.changes.retain(|s| !s.name.eq_ignore_ascii_case(name));
        if let Some(next) = fallback {
            self.current = DEFAULT.into();
            *layout = next;
        }
        Ok(())
    }

    /// A bad saved arrangement never invalidates unrelated preferences or valid workspaces.
    pub fn sanitized(mut self) -> Self {
        let mut seen = std::collections::BTreeSet::new();
        self.saved.retain_mut(|saved| {
            saved.name = saved.name.trim().to_string();
            saved.layout.normalize_tools().is_ok()
                && valid_name(&saved.name)
                && !is_builtin(&saved.name)
                && saved.layout.validate().is_ok()
                && seen.insert(saved.name.to_lowercase())
        });
        self.saved.truncate(LIMIT);
        let names = self.names();
        let mut seen = std::collections::BTreeSet::new();
        self.changes.retain_mut(|changed| {
            let Some(name) = names.iter().find(|name| name.eq_ignore_ascii_case(changed.name.trim())) else { return false };
            changed.name = name.clone();
            changed.layout.normalize_tools().is_ok() && changed.layout.validate().is_ok() && seen.insert(name.to_lowercase())
        });
        self.changes.truncate(LIMIT);
        self.current = names.into_iter().find(|name| name.eq_ignore_ascii_case(self.current.trim())).unwrap_or_else(|| DEFAULT.into());
        self
    }
}

/// Existing Window commands use this store while the new panel system is enabled.
pub fn command(app: &mut VectorcraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    let state = super::presenter::state(&app.ui)?;
    if !id.starts_with("window.workspace") {
        return None;
    }
    if id == "window.workspace.list" {
        return Some(Ok(
            json!({"current":state.workspaces.current,"workspaces":state.workspaces.names(),"custom":state.workspaces.saved.iter().map(|s|&s.name).collect::<Vec<_>>()}),
        ));
    }
    if id == "window.workspace.manage" {
        let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
        app.ui.dialog = Some(Dialog::new("manageWorkspaces", json!({"selected":name,"name":name})));
        return Some(Ok(Value::Null));
    }
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    if id == "window.workspace.new" && params.get("name").is_none_or(Value::is_null) {
        let name = (1..=LIMIT + 1).map(|i| format!("Workspace {i}")).find(|n| state.workspaces.baseline(n).is_err()).unwrap_or_default();
        app.ui.dialog = Some(Dialog::new("newWorkspace", json!({"name":name})));
        return Some(Ok(Value::Null));
    }
    let mut store = state.workspaces.clone();
    let state = super::presenter::state_mut(&mut app.ui)?;
    // The store changes on its own copy and the layout through the one layout transaction; both
    // are kept only when the command succeeds.
    let result = super::commands::transact(state, |layout| {
        match id {
            "window.workspace" => store.switch(layout, name),
            "window.workspace.reset" => store.reset(layout),
            "window.workspace.new" => store.save(layout, name),
            "window.workspace.rename" => store.rename(name, params.get("to").and_then(Value::as_str).unwrap_or_default()),
            "window.workspace.delete" => store.delete(layout, name),
            _ => Err(format!("Unknown workspace command `{id}`")),
        }?;
        Ok(((), super::commands::Stale::Everything))
    });
    if let Err(error) = result {
        return Some(Err(error));
    }
    let current = store.current.clone();
    state.workspaces = store;
    crate::workspaces::sync(&app.ui);
    Some(Ok(json!({"workspace":current})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::varos::panel_system::model::{Action, Mode};

    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("ui.panelReview", json!({"enabled":true})).unwrap();
        app.ui.varos_panel_review.as_mut().unwrap().layout = Layout::new_workspace();
        app
    }

    fn change(layout: &mut Layout, panel: &str) {
        let host = layout.owner(panel).unwrap().id;
        let action = if panel == "tools" {
            Action::SetToolsWidth { width: super::super::metrics::TOOLS_MIN_WIDTH }
        } else if panel == "control" {
            Action::Hide { host }
        } else {
            Action::SetMode { host, mode: Mode::Names }
        };
        layout.apply(&action).unwrap();
        layout.apply(&Action::Hide { host }).unwrap();
    }

    #[test]
    fn save_switch_reset_and_serialization_keep_independent_arrangements() {
        let mut store = Store::default();
        let mut layout = Layout::new_workspace();
        change(&mut layout, "layers");
        let saved = layout.clone();
        store.save(&layout, "Mine").unwrap();
        change(&mut layout, "tools");
        let changed = layout.clone();
        store.switch(&mut layout, "Design").unwrap();
        assert_eq!(layout, saved, "Design retains the arrangement left when Mine was saved");
        store = serde_json::from_value(serde_json::to_value(&store).unwrap()).unwrap();
        store.switch(&mut layout, "mine").unwrap();
        assert_eq!(layout, changed);
        store.reset(&mut layout).unwrap();
        assert_eq!(layout, saved);
        assert!(!store.changes.iter().any(|s| s.name == "Mine"));
    }

    #[test]
    fn invalid_names_and_unknown_switches_are_non_mutating() {
        let mut store = Store::default();
        let mut layout = Layout::new_workspace();
        store.save(&layout, "Mine").unwrap();
        let before = (store.clone(), layout.clone());
        assert!(store.switch(&mut layout, "Missing").is_err());
        assert!(store.save(&layout, "Design").is_err());
        assert!(store.save(&layout, " ").is_err());
        assert!(store.rename("Mine", "Design").is_err());
        assert!(store.delete(&mut layout, "Design").is_err());
        assert_eq!((store, layout), before);
    }

    #[test]
    fn rename_delete_and_sanitize_preserve_valid_workspaces() {
        let mut store = Store::default();
        let mut layout = Layout::new_workspace();
        change(&mut layout, "layers");
        let design = layout.clone();
        store.save(&layout, "Mine").unwrap();
        store.rename("mine", "Ours").unwrap();
        assert_eq!(store.current, "Ours");
        change(&mut layout, "tools");
        store.delete(&mut layout, "ours").unwrap();
        assert_eq!(store.current, DEFAULT);
        assert_eq!(layout, design);
        store.save(&layout, "Kept").unwrap();
        store.saved.push(Saved { name: "kept".into(), layout: layout.clone() });
        let mut invalid = layout.clone();
        invalid.version = u32::MAX;
        store.saved.push(Saved { name: "Bad".into(), layout: invalid });
        store.current = "Missing".into();
        let store = store.sanitized();
        assert_eq!(store.names(), ["Design", "Studio", "Kept"]);
        assert_eq!(store.current, DEFAULT);
    }

    #[test]
    fn native_new_workspace_dialog_and_window_labels_use_the_new_store() {
        let mut app = app();
        app.run("file.new", json!({"width":300,"height":300})).unwrap();
        let old_shell = serde_json::to_value(&app.ui.varos_shell).unwrap();
        let document = app.session.active().unwrap().doc.clone();
        app.run("window.workspace.new", json!({})).unwrap();
        app.ui.dialog.as_mut().unwrap().fields.insert("name".into(), json!("Review"));
        crate::workspaces::confirm(&mut app).unwrap();
        assert_eq!(current(&app), Some("Review"));
        assert_eq!(crate::menus::checked(&app, "window.workspace", &json!({"name":"Review"})), Some(true));
        assert_eq!(crate::menus::dynamic_label(&app, "window.workspace.reset", "Reset Essentials"), "Reset Review");
        assert_eq!(app.run("window.workspace.list", json!({})).unwrap()["custom"], json!(["Review"]));
        app.run("window.workspace.reset", json!({})).unwrap();
        assert!(std::sync::Arc::ptr_eq(&app.session.active().unwrap().doc, &document));
        assert_eq!(serde_json::to_value(&app.ui.varos_shell).unwrap(), old_shell);
        assert!(app.ui.dialog.is_none());
        assert!(crate::workspaces::menu_items().iter().any(|item| matches!(item, crate::menus::Item::Cmd("Review", "window.workspace", _))));
    }

    #[test]
    fn ui_preferences_keep_layout_and_workspaces_but_drop_transient_interaction() {
        let mut app = app();
        let state = app.ui.varos_panel_review.as_mut().unwrap();
        change(&mut state.layout, "tools");
        state.workspaces.save(&state.layout, "Saved").unwrap();
        change(&mut state.layout, "control");
        state.flyout = state.layout.group_of("layers").map(|group| group.id);
        state.hits.push(("stale".into(), egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(10.0, 10.0))));
        let expected_layout = state.layout.clone();
        let expected_store = state.workspaces.clone();
        let saved = serde_json::to_value(&app.ui).unwrap();
        let panel_state = &saved["varos_panel_review"];
        assert!(panel_state.get("gesture").is_none());
        assert!(panel_state.get("flyout").is_none());
        assert!(panel_state.get("hits").is_none());
        let restored = crate::UiState::read_preferences(&serde_json::to_vec(&saved).unwrap()).unwrap().ui.sanitized();
        let state = restored.varos_panel_review.unwrap();
        assert_eq!(state.layout, expected_layout);
        assert_eq!(state.workspaces, expected_store);
        assert!(state.gesture.is_none() && state.flyout.is_none() && state.hits.is_empty());
        assert_eq!(restored.varos_shell, app.ui.varos_shell);
    }

    #[test]
    fn studio_starter_has_four_visible_blocks_and_one_owner_for_every_panel() {
        use super::super::{
            geometry,
            model::{Edge, groups},
            registry,
        };
        let design = Layout::new_workspace();
        let studio = Layout::studio_workspace().unwrap();
        studio.validate().unwrap();
        assert_eq!(design, Layout::new_workspace(), "Studio never changes the Design baseline");
        let mut owners: Vec<_> = studio.hosts.iter().flat_map(|h| groups(&h.root)).flat_map(|g| g.panels.iter().map(String::as_str)).collect();
        let mut expected: Vec<_> = registry::BUILTINS.iter().map(|p| p.id).collect();
        owners.sort_unstable();
        expected.sort_unstable();
        assert_eq!(owners, expected);
        assert_eq!(studio.hosts.iter().filter(|h| !h.hidden).count(), 4);
        assert_eq!(studio.owner("tools").unwrap().dock, Some(Edge::Left));
        assert_eq!(studio.tools_width(), Some(super::super::metrics::TOOLS_MAX_WIDTH));
        assert_eq!(studio.owner("control").unwrap().dock, Some(Edge::Top));
        let full = studio.owner("properties").unwrap();
        assert_eq!(full.id, studio.owner("layers").unwrap().id);
        assert_eq!(groups(&full.root).iter().map(|g| g.active.as_str()).collect::<Vec<_>>(), ["properties", "layers"]);
        assert_ne!(studio.group_of("properties").unwrap().id, studio.group_of("layers").unwrap().id);
        assert_eq!(full.mode, Mode::Full);
        let icons = studio.owner("color").unwrap();
        assert_eq!(icons.mode, Mode::Icons);
        assert_eq!(
            groups(&icons.root).iter().map(|g| g.active.as_str()).collect::<Vec<_>>(),
            ["color", "swatches", "gradient", "stroke", "appearance"]
        );
        let measured = geometry::measure(&studio, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0)));
        let full_rect = measured.iter().find(|h| h.id == full.id).unwrap().rect;
        let icon_rect = measured.iter().find(|h| h.id == icons.id).unwrap().rect;
        assert!((full_rect.width() - 320.0).abs() < 0.01);
        assert!((icon_rect.width() - 44.0).abs() < 0.01);
        assert_eq!(icon_rect.right(), full_rect.left(), "compact strip sits immediately on the canvas side of the full dock");
        assert_eq!(full_rect.right(), 1600.0);
    }

    #[test]
    fn studio_switch_reset_and_round_trip_preserve_customized_design() {
        let mut store = Store::default();
        let mut layout = Layout::new_workspace();
        change(&mut layout, "layers");
        let design = layout.clone();
        store.switch(&mut layout, " studio ").unwrap();
        assert_eq!(store.current, STUDIO);
        assert_eq!(layout, Layout::studio_workspace().unwrap());
        change(&mut layout, "tools");
        let changed_studio = layout.clone();
        store.switch(&mut layout, DEFAULT).unwrap();
        assert_eq!(layout, design);
        store = serde_json::from_value(serde_json::to_value(&store).unwrap()).unwrap();
        store = store.sanitized();
        store.switch(&mut layout, STUDIO).unwrap();
        assert_eq!(layout, changed_studio);
        store.reset(&mut layout).unwrap();
        assert_eq!(layout, Layout::studio_workspace().unwrap());
        assert!(!store.changes.iter().any(|saved| saved.name == STUDIO));
        store.switch(&mut layout, DEFAULT).unwrap();
        assert_eq!(layout, design);
        assert!(store.save(&layout, STUDIO).is_err());
        assert!(store.delete(&mut layout, STUDIO).is_err());
    }

    #[test]
    fn deleting_a_custom_workspace_restores_design_even_when_studio_was_changed_first() {
        let mut store = Store::default();
        let mut layout = Layout::new_workspace();
        let design = layout.clone();
        store.switch(&mut layout, STUDIO).unwrap();
        change(&mut layout, "tools");
        store.save(&layout, "Custom").unwrap();
        store.delete(&mut layout, "Custom").unwrap();
        assert_eq!(store.current, DEFAULT);
        assert_eq!(layout, design);
    }

    #[test]
    fn tools_and_control_menu_checks_follow_host_visibility() {
        let mut app = app();
        for (panel, command) in [("tools", "window.toolbar"), ("control", "window.control")] {
            assert_eq!(crate::menus::checked(&app, command, &json!({})), Some(true));
            let state = app.ui.varos_panel_review.as_mut().unwrap();
            let host = state.layout.owner(panel).unwrap().id;
            state.layout.apply(&Action::Hide { host }).unwrap();
            assert_eq!(crate::menus::checked(&app, command, &json!({})), Some(false));
            app.ui.varos_panel_review.as_mut().unwrap().layout.apply(&Action::Show { panel: panel.into() }).unwrap();
            assert_eq!(crate::menus::checked(&app, command, &json!({})), Some(true));
        }
    }
}
