//! Workspaces in the Varos shell (stage 4): one row of tabs at the top of the central region —
//! the presets Design, Typography and Print, the user's own workspaces and `+`. A workspace is the
//! shell's whole arrangement ([`Arrangement`]: the tree, the Board's strip, floating panels and
//! strips, the Properties editors' section views, the maximized box); switching keeps the open
//! documents. As with the old workspaces (`crate::workspaces`), a workspace remembers the changes
//! made in it until Reset brings it back as saved.
//!
//! The row is one line: too many tabs scroll sideways with the wheel or trackpad and with a
//! middle-button drag; `‹` / `›` at its ends show there is more. The front workspace is lit.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::floats::{Float, FloatStrip};
use super::persist::{CollapsedColumn, VarosShell};
use super::strip::Strip;
use super::tree::Node;
use crate::VectorcraftApp;
use crate::panels::properties::PropertiesView;

/// The default workspace (the default layout).
pub(crate) const DESIGN: &str = "Design";
/// The presets, in the row's order.
pub(crate) const PRESETS: [&str; 3] = [DESIGN, "Typography", "Print"];
/// No more remembered layouts than this (the oldest go).
const CHANGES_CAP: usize = 64;

/// A workspace's arrangement of the shell, saved under its name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    pub name: String,
    pub tree: Node,
    #[serde(default)]
    pub tools_pos: Option<[f32; 2]>,
    #[serde(default = "super::persist::default_columns")]
    pub tools_columns: u8,
    pub strip: Strip,
    #[serde(default)]
    pub floats: Vec<Float>,
    #[serde(default)]
    pub float_strips: Vec<FloatStrip>,
    #[serde(default)]
    pub views: BTreeMap<String, PropertiesView>,
    #[serde(default)]
    pub maximized: Option<u32>,
    /// Columns collapsed into the strip; restored with the workspace.
    #[serde(default)]
    pub collapsed_columns: Vec<CollapsedColumn>,
}

impl Arrangement {
    /// Made sound after loading, as the shell's own layout is: known panels, one owner each, a
    /// Board (else the default layout).
    pub(crate) fn sanitized(self) -> Arrangement {
        let mut shell = VarosShell {
            tree: self.tree.sanitized(&super::persist::known),
            tools_columns: self.tools_columns.clamp(1, 3),
            tools_pos: self.tools_pos.filter(|p| p.iter().all(|v| v.is_finite())),
            strip: self.strip,
            floats: self.floats,
            float_strips: self.float_strips,
            views: self.views,
            maximized: self.maximized,
            collapsed_columns: self.collapsed_columns,
            ..VarosShell::default()
        };
        shell.sanitize(false);
        shell.capture(&self.name)
    }
}

/// Is `name` a preset (whatever its case).
pub(crate) fn is_preset(name: &str) -> bool {
    PRESETS.iter().any(|p| p.eq_ignore_ascii_case(name))
}

/// Preset `name` (whatever its case) as it comes before any change.
pub(crate) fn preset(name: &str) -> Option<Arrangement> {
    PRESETS.iter().find(|p| p.eq_ignore_ascii_case(name)).map(|p| preset_named(p))
}

/// The Design preset: the default layout.
fn design() -> Arrangement {
    preset_named(DESIGN)
}

/// Preset `name` (one of [`PRESETS`]; another name is Design's layout under it).
fn preset_named(name: &str) -> Arrangement {
    let tree = match name {
        "Typography" => Node::board_and_column(&["character", "paragraph", "openType"], &["properties", "layers", "glyphs"]),
        "Print" => Node::board_and_column(&["layers", "artboards"], &["properties", "separations", "docInfo"]),
        _ => Node::default_layout(),
    };
    let strip = Strip::default_for(&tree);
    Arrangement {
        name: name.to_string(),
        tree,
        tools_pos: None,
        tools_columns: 1,
        strip,
        floats: Vec::new(),
        float_strips: Vec::new(),
        views: BTreeMap::new(),
        maximized: None,
        collapsed_columns: Vec::new(),
    }
}

impl VarosShell {
    /// The arrangement shown now, called `name` (pins are not kept: they belong to the session).
    pub(crate) fn capture(&self, name: &str) -> Arrangement {
        let mut views = self.views.clone();
        for v in views.values_mut() {
            v.pinned = None;
        }
        Arrangement {
            name: name.to_string(),
            tree: self.tree.clone(),
            tools_pos: self.tools_pos,
            tools_columns: self.tools_columns,
            strip: self.strip.clone(),
            floats: self.floats.clone(),
            float_strips: self.float_strips.clone(),
            views,
            maximized: self.maximized_box(),
            collapsed_columns: self.collapsed_columns.clone(),
        }
    }

    /// Show arrangement `a` as workspace `a.name` (the flyout closes, a drag ends).
    pub(crate) fn apply(&mut self, a: &Arrangement) {
        self.tree = a.tree.clone();
        self.tools_pos = a.tools_pos;
        self.tools_columns = a.tools_columns;
        self.strip = a.strip.clone();
        self.floats = a.floats.clone();
        self.float_strips = a.float_strips.clone();
        self.views = a.views.clone();
        self.maximized = a.maximized;
        self.collapsed_columns = a.collapsed_columns.clone();
        self.workspace = a.name.clone();
        self.open = None;
        self.drag = None;
        self.maximized = self.maximized_box();
        self.settle();
    }

    /// Every workspace's name: the presets, then the user's.
    pub(crate) fn workspace_names(&self) -> Vec<String> {
        PRESETS.iter().map(|p| (*p).to_string()).chain(self.workspaces.iter().map(|w| w.name.clone())).collect()
    }

    /// Workspace `name` (whatever its case) as saved: a user workspace, else a preset.
    pub(crate) fn find_saved(&self, name: &str) -> Option<Arrangement> {
        self.workspaces.iter().find(|w| w.name.eq_ignore_ascii_case(name)).cloned().or_else(|| preset(name))
    }

    /// Keep the arrangement as it is now as the current workspace's changes.
    fn remember(&mut self) {
        let Some(saved) = self.find_saved(&self.workspace) else { return };
        let now = self.capture(&saved.name);
        self.workspace_changes.retain(|w| !w.name.eq_ignore_ascii_case(&saved.name));
        // Unchanged since it was saved: nothing to remember.
        if now != saved {
            self.workspace_changes.push(now);
        }
        let over = self.workspace_changes.len().saturating_sub(CHANGES_CAP);
        self.workspace_changes.drain(..over);
    }

    /// Switch to workspace `name` as it was left, else as saved; the one left keeps its changes.
    pub(crate) fn switch_workspace(&mut self, name: &str) -> Result<String, String> {
        let saved = self.find_saved(name).ok_or_else(|| format!("unknown workspace (one of {})", self.workspace_names().join(", ")))?;
        self.remember();
        let a = self.workspace_changes.iter().find(|w| w.name.eq_ignore_ascii_case(&saved.name)).cloned().unwrap_or(saved);
        self.apply(&a);
        Ok(a.name)
    }

    /// The current workspace as it was saved (a preset: the shell's default arrangement for it);
    /// its changes go. The documents are not touched.
    pub(crate) fn reset_workspace(&mut self) -> String {
        let a = self.find_saved(&self.workspace).unwrap_or_else(design);
        self.workspace_changes.retain(|w| !w.name.eq_ignore_ascii_case(&a.name));
        self.apply(&a);
        a.name
    }

    /// Save the arrangement shown as workspace `name` (replacing a user workspace of that name,
    /// whatever its case), which becomes the current one, with no changes; the workspace left
    /// remembers the arrangement as its changes.
    pub(crate) fn save_workspace(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("workspace name can't be empty".into());
        }
        if is_preset(name) {
            return Err(format!("`{name}` is a built-in workspace; choose another name"));
        }
        // The workspace left keeps the layout it shows as its changes (switching back to it
        // brings them back), unless it is the one saved over.
        if !self.workspace.eq_ignore_ascii_case(name) {
            self.remember();
        }
        let a = self.capture(name);
        match self.workspaces.iter_mut().find(|w| w.name.eq_ignore_ascii_case(name)) {
            Some(w) => *w = a,
            None => self.workspaces.push(a),
        }
        self.workspace_changes.retain(|w| !w.name.eq_ignore_ascii_case(name));
        self.workspace = name.to_string();
        Ok(())
    }

    /// Rename user workspace `from` to `to` (its changes follow it).
    pub(crate) fn rename_workspace(&mut self, from: &str, to: &str) -> Result<(), String> {
        let to = to.trim();
        let taken = self.workspaces.iter().any(|w| w.name.eq_ignore_ascii_case(to) && !w.name.eq_ignore_ascii_case(from));
        if to.is_empty() || is_preset(to) || taken {
            return Err(format!("`{to}` is not an available name"));
        }
        let w = self.workspaces.iter_mut().find(|w| w.name == from).ok_or_else(|| format!("no user workspace `{from}`"))?;
        w.name = to.to_string();
        for w in self.workspace_changes.iter_mut().filter(|w| w.name == from) {
            w.name = to.to_string();
        }
        if self.workspace == from {
            self.workspace = to.to_string();
        }
        Ok(())
    }

    /// Delete user workspace `name`; the current one deleted, Design (as it was left) shows.
    pub(crate) fn delete_workspace(&mut self, name: &str) -> Result<(), String> {
        let before = self.workspaces.len();
        self.workspaces.retain(|w| w.name != name);
        if self.workspaces.len() == before {
            return Err(if is_preset(name) { format!("`{name}` is built in and can't be deleted") } else { format!("no workspace `{name}`") });
        }
        self.workspace_changes.retain(|w| w.name != name);
        if self.workspace == name {
            let left = self.workspace_changes.iter().find(|w| w.name == DESIGN).cloned().unwrap_or_else(design);
            self.apply(&left);
        }
        Ok(())
    }

    /// After loading: user workspaces with unique names that are not presets' (the first kept);
    /// remembered changes only for workspaces that exist, under their saved spelling, once each;
    /// the current workspace one that exists (else Design, the arrangement shown kept as it is).
    pub(crate) fn sanitize_workspaces(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        self.workspaces.retain(|w| {
            let name = w.name.trim();
            let fresh = !name.is_empty() && !is_preset(name) && !seen.iter().any(|s| s.eq_ignore_ascii_case(name));
            if fresh {
                seen.push(name.to_string());
            }
            fresh
        });
        let names = self.workspace_names();
        let mut kept: Vec<String> = Vec::new();
        self.workspace_changes.retain_mut(|w| {
            let Some(saved) = names.iter().find(|n| n.eq_ignore_ascii_case(&w.name)) else { return false };
            if kept.contains(saved) {
                return false;
            }
            w.name = saved.clone();
            kept.push(saved.clone());
            true
        });
        self.workspace = names.iter().find(|n| n.eq_ignore_ascii_case(&self.workspace)).cloned().unwrap_or_else(|| DESIGN.to_string());
    }
}

/// The label of workspace `name` in the row and menus: a preset's translated, the user's as typed.
pub(crate) fn label(name: &str) -> String {
    if is_preset(name) { tl!(name).to_string() } else { name.to_string() }
}

/// `window.workspace*` while the shell is on.
pub(crate) fn command(app: &mut VectorcraftApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    let s = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_string);
    let r = match id {
        "window.workspace" => {
            let name = s("name").unwrap_or_default();
            leave(app);
            app.ui.varos_shell.switch_workspace(&name).map(|name| json!({"workspace": name}))
        }
        "window.workspace.reset" => {
            leave(app);
            Ok(json!({"workspace": app.ui.varos_shell.reset_workspace()}))
        }
        "window.workspace.new" => match s("name") {
            Some(n) => app.ui.varos_shell.save_workspace(&n).map(|()| json!({"workspace": n.trim()})),
            None => {
                let shell = &app.ui.varos_shell;
                let n = (1..).map(|i| format!("Workspace {i}")).find(|n| shell.find_saved(n).is_none()).unwrap_or_default();
                app.ui.dialog = Some(crate::state::Dialog::new("newWorkspace", json!({"name": n})));
                Ok(Value::Null)
            }
        },
        "window.workspace.manage" => {
            let name = s("name").unwrap_or_default();
            app.ui.dialog = Some(crate::state::Dialog::new("manageWorkspaces", json!({"selected": name, "name": name})));
            Ok(Value::Null)
        }
        "window.workspace.delete" => {
            let name = s("name").unwrap_or_default();
            if app.ui.varos_shell.workspace == name {
                leave(app);
            }
            app.ui.varos_shell.delete_workspace(&name).map(|()| Value::Null)
        }
        "window.workspace.rename" => {
            app.ui.varos_shell.rename_workspace(&s("name").unwrap_or_default(), &s("to").unwrap_or_default()).map(|()| Value::Null)
        }
        "window.workspace.list" => {
            let shell = &app.ui.varos_shell;
            let custom: Vec<&str> = shell.workspaces.iter().map(|w| w.name.as_str()).collect();
            Ok(json!({"current": shell.workspace, "workspaces": shell.workspace_names(), "custom": custom}))
        }
        _ => return None,
    };
    Some(r)
}

/// Before the arrangement changes: the flyout closes and a drag ends.
fn leave(app: &mut VectorcraftApp) {
    super::strip::close(app);
    app.ui.varos_shell.drag = None;
}

/// Compact top-bar switcher; all changes use the existing workspace commands/dialogs.
pub(crate) fn switcher(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    let current = crate::varos::panel_system::workspace::current(app).map(str::to_owned).unwrap_or_else(|| app.ui.varos_shell.workspace.clone());
    let t = crate::theme::Tokens::get(ui.ctx());
    let mut job = egui::text::LayoutJob::simple(label(&current), egui::FontId::proportional(11.0), t.text, 128.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(galley.size().x + 32.0, 24.0), egui::Sense::click());
    #[cfg(test)]
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("simple-workspace-rect"), rect));
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, t.hover);
    }
    ui.painter().galley(rect.left_center() + egui::vec2(8.0, -galley.size().y / 2.0), galley, t.text);
    crate::icons::paint(
        ui,
        "chevron-down",
        egui::Rect::from_center_size(rect.right_center() - egui::vec2(10.0, 0.0), egui::vec2(12.0, 12.0)),
        t.text_dim,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label(&current)));

    egui::Popup::menu(&response).show(|ui| {
        for name in crate::varos::panel_system::workspace::names(app).unwrap_or_else(|| app.ui.varos_shell.workspace_names()) {
            if ui.selectable_label(name == current, label(&name)).clicked() {
                if let Err(e) = app.run("window.workspace", json!({"name": name})) {
                    app.status(e);
                }
                ui.close();
            }
        }
        ui.separator();
        for (label, command) in [
            (tl!("New…"), "window.workspace.new"),
            (tl!("Rename…"), "window.workspace.manage"),
            (tl!("Delete"), "window.workspace.delete"),
            (tl!("Reset"), "window.workspace.reset"),
        ] {
            let enabled = !matches!(command, "window.workspace.manage" | "window.workspace.delete") || !is_preset(&current);
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                if let Err(e) = app.run(command, if command == "window.workspace.new" { json!({}) } else { json!({"name": current}) }) {
                    app.status(e);
                }
                ui.close();
            }
        }
    });
}
