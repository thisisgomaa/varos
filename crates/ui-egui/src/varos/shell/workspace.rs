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

use egui::{CornerRadius, Id, Rect, Sense, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::Hit;
use super::floats::{Float, FloatStrip};
use super::persist::VarosShell;
use super::strip::Strip;
use super::tree::Node;
use crate::VectorcraftApp;
use crate::panels::properties::PropertiesView;
use crate::theme::Tokens;

/// The default workspace (the default layout).
pub(crate) const DESIGN: &str = "Design";
/// The presets, in the row's order.
pub(crate) const PRESETS: [&str; 3] = [DESIGN, "Typography", "Print"];
/// The row's height.
pub(crate) const ROW_H: f32 = 28.0;
/// A tab pill's height.
const PILL_H: f32 = 22.0;
/// Room left and right of a tab's label inside its pill, and between two pills.
const PILL_PAD: f32 = 12.0;
const PILL_GAP: f32 = 2.0;
/// The `+` button, and the `‹` / `›` overflow marks.
const ADD_W: f32 = 24.0;
const MORE_W: f32 = 16.0;
/// How far a click on `‹` / `›` scrolls, and a wheel line.
const PAGE: f32 = 160.0;
const LINE: f32 = 40.0;
/// No more remembered layouts than this (the oldest go).
const CHANGES_CAP: usize = 64;

/// A workspace's arrangement of the shell, saved under its name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    pub name: String,
    pub tree: Node,
    pub strip: Strip,
    #[serde(default)]
    pub floats: Vec<Float>,
    #[serde(default)]
    pub float_strips: Vec<FloatStrip>,
    #[serde(default)]
    pub views: BTreeMap<String, PropertiesView>,
    #[serde(default)]
    pub maximized: Option<u32>,
}

impl Arrangement {
    /// Made sound after loading, as the shell's own layout is: known panels, one owner each, a
    /// Board (else the default layout).
    pub(crate) fn sanitized(self) -> Arrangement {
        let mut shell = VarosShell {
            tree: self.tree.sanitized(&super::persist::known),
            strip: self.strip,
            floats: self.floats,
            float_strips: self.float_strips,
            views: self.views,
            maximized: self.maximized,
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
    Arrangement { name: name.to_string(), tree, strip, floats: Vec::new(), float_strips: Vec::new(), views: BTreeMap::new(), maximized: None }
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
            strip: self.strip.clone(),
            floats: self.floats.clone(),
            float_strips: self.float_strips.clone(),
            views,
            maximized: self.maximized_box(),
        }
    }

    /// Show arrangement `a` as workspace `a.name` (the flyout closes, a drag ends).
    pub(crate) fn apply(&mut self, a: &Arrangement) {
        self.tree = a.tree.clone();
        self.strip = a.strip.clone();
        self.floats = a.floats.clone();
        self.float_strips = a.float_strips.clone();
        self.views = a.views.clone();
        self.maximized = a.maximized;
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

fn scroll_id() -> Id {
    Id::unique("varos-shell-workspace-scroll")
}

/// The row's scroll offset (how far its tabs are moved left).
pub(crate) fn scroll(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(scroll_id())).unwrap_or(0.0)
}

/// The scroll a frame's wheel and trackpad events over the row ask for (content moving right, as
/// egui reports it, scrolls back), in points.
fn wheel(ctx: &egui::Context, width: f32) -> f32 {
    ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                    egui::MouseWheelUnit::Point => *delta,
                    egui::MouseWheelUnit::Line => *delta * LINE,
                    egui::MouseWheelUnit::Page => *delta * width,
                }),
                _ => None,
            })
            .map(|d| -(d.x + d.y))
            .sum()
    })
}

/// What a click in the row asks for, done once the row is drawn.
enum Ask {
    Switch(String),
    New,
    Reset(String),
    Rename(String),
    Delete(String),
    Scroll(f32),
}

/// Draw the workspace row in `row`: the tabs (the front one lit), `‹` / `›` when some are out of
/// sight, and `+`. Its controls go to `hits`.
pub(crate) fn row(app: &mut VectorcraftApp, ui: &mut egui::Ui, row: Rect, hits: &mut Vec<(Hit, Rect)>) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let painter = ui.painter().clone();
    hits.push((Hit::Workspaces, row));
    let names = app.ui.varos_shell.workspace_names();
    let current = app.ui.varos_shell.workspace.clone();
    let add = Rect::from_min_size(pos2(row.right() - ADD_W, row.center().y - PILL_H / 2.0), vec2(ADD_W, PILL_H));
    let lane = Rect::from_min_max(row.min, pos2(add.left() - 4.0, row.bottom()));
    let galleys: Vec<_> = names
        .iter()
        .map(|n| {
            let font = if *n == current { crate::theme::semibold(12.0) } else { egui::FontId::proportional(12.0) };
            painter.layout_no_wrap(label(n), font, t.text)
        })
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| (g.size().x + 2.0 * PILL_PAD).ceil()).collect();
    let content = widths.iter().map(|w| w + PILL_GAP).sum::<f32>();
    let max_scroll = (content - lane.width()).max(0.0);
    // The wheel or trackpad over the row, and a middle-button drag begun on it, scroll it.
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let mut offset = scroll(&ctx);
    if pointer.is_some_and(|p| row.contains(p)) {
        offset += wheel(&ctx, lane.width());
    }
    let middle = scroll_id().with("middle");
    let (pressed, down, delta, origin) = ctx.input(|i| {
        (
            i.pointer.button_pressed(egui::PointerButton::Middle),
            i.pointer.button_down(egui::PointerButton::Middle),
            i.pointer.delta(),
            i.pointer.press_origin(),
        )
    });
    if pressed && origin.or(pointer).is_some_and(|p| row.contains(p)) {
        ctx.data_mut(|d| d.insert_temp(middle, true));
    }
    let panning = down && ctx.data(|d| d.get_temp::<bool>(middle)).unwrap_or(false);
    if panning {
        offset -= delta.x;
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    } else {
        ctx.data_mut(|d| d.remove_temp::<bool>(middle));
    }
    // The front workspace just switched to comes into sight.
    let shown = scroll_id().with("front");
    if ctx.data(|d| d.get_temp::<String>(shown)).as_deref() != Some(current.as_str()) {
        if let Some(k) = names.iter().position(|n| *n == current) {
            let left: f32 = widths[..k].iter().map(|w| w + PILL_GAP).sum();
            let right = left + widths[k];
            if left < offset {
                offset = left;
            } else if right > offset + lane.width() {
                offset = right - lane.width();
            }
        }
        ctx.data_mut(|d| d.insert_temp(shown, current.clone()));
    }
    offset = offset.clamp(0.0, max_scroll);
    let mut ask = None;
    let clip = lane.intersect(painter.clip_rect());
    let mut x = lane.left() - offset;
    for ((name, galley), w) in names.iter().zip(galleys).zip(&widths) {
        let pill = Rect::from_min_size(pos2(x, row.center().y - PILL_H / 2.0), vec2(*w, PILL_H));
        x = pill.right() + PILL_GAP;
        let seen = pill.intersect(lane);
        if !seen.is_positive() {
            continue;
        }
        let response = ui.interact(seen, Id::unique("varos-shell-workspace").with(name), Sense::click());
        let front = *name == current;
        let text = if front {
            painter.with_clip_rect(clip).rect_filled(pill, CornerRadius::same((PILL_H / 2.0) as u8), t.button);
            t.text_strong
        } else if response.hovered() {
            painter.with_clip_rect(clip).rect_filled(pill, CornerRadius::same((PILL_H / 2.0) as u8), t.hover);
            t.text
        } else {
            t.text_dim
        };
        painter.with_clip_rect(clip).galley(pos2(pill.left() + PILL_PAD, pill.center().y - galley.size().y / 2.0), galley.clone(), text);
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, front, galley.text()));
        if response.clicked() && !front {
            ask = Some(Ask::Switch(name.clone()));
        }
        let preset = is_preset(name);
        response.context_menu(|ui| {
            ui.set_min_width(170.0);
            if crate::widgets::menu_item(ui, "Reset", true, false) {
                ask = Some(Ask::Reset(name.clone()));
            }
            if crate::widgets::menu_item(ui, "Rename…", !preset, false) {
                ask = Some(Ask::Rename(name.clone()));
            }
            if crate::widgets::menu_item(ui, "Delete", !preset, false) {
                ask = Some(Ask::Delete(name.clone()));
            }
        });
        hits.push((Hit::WorkspaceTab(name.clone()), seen));
    }
    // The ends that hide tabs show it: `‹` at the left, `›` at the right (a click scrolls).
    for (right, hidden) in [(false, offset > 0.5), (true, offset < max_scroll - 0.5)] {
        if !hidden {
            continue;
        }
        let mark = if right {
            Rect::from_min_max(pos2(lane.right() - MORE_W, row.top()), pos2(lane.right(), row.bottom()))
        } else {
            Rect::from_min_max(row.min, pos2(lane.left() + MORE_W, row.bottom()))
        };
        painter.rect_filled(mark, CornerRadius::ZERO, t.divider);
        let response = ui.interact(mark, scroll_id().with(("more", right)), Sense::click());
        let icon = if right { "chevron-right" } else { "chevron-left" };
        crate::icons::paint(
            ui,
            icon,
            Rect::from_center_size(mark.center(), vec2(12.0, 12.0)),
            if response.hovered() { t.text_strong } else { t.text_dim },
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, if right { "More workspaces" } else { "Earlier workspaces" }));
        if response.clicked() {
            ask = Some(Ask::Scroll(if right { PAGE } else { -PAGE }));
        }
        hits.push((Hit::WorkspaceOverflow(right), mark));
    }
    let response = ui.interact(add, scroll_id().with("new"), Sense::click());
    if response.hovered() {
        painter.rect_filled(add, CornerRadius::same(3), t.hover);
    }
    crate::icons::paint(
        ui,
        "plus",
        Rect::from_center_size(add.center(), vec2(14.0, 14.0)),
        if response.hovered() { t.text_strong } else { t.text_dim },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("New Workspace…")));
    if response.on_hover_text(tl!("Save this layout as a new workspace")).clicked() {
        ask = Some(Ask::New);
    }
    hits.push((Hit::WorkspaceNew, add));
    if let Some(Ask::Scroll(by)) = ask {
        offset = (offset + by).clamp(0.0, max_scroll);
    }
    ctx.data_mut(|d| d.insert_temp(scroll_id(), offset));
    if ask.is_some() {
        ctx.request_repaint();
    }
    let run = |app: &mut VectorcraftApp, id: &str, p: Value| {
        if let Err(e) = app.run(id, p) {
            app.status(e);
        }
    };
    match ask {
        Some(Ask::Switch(name)) => run(app, "window.workspace", json!({"name": name})),
        Some(Ask::New) => run(app, "window.workspace.new", json!({})),
        Some(Ask::Reset(name)) => {
            if name != current {
                run(app, "window.workspace", json!({"name": name}));
            }
            run(app, "window.workspace.reset", json!({}));
        }
        Some(Ask::Rename(name)) => run(app, "window.workspace.manage", json!({"name": name})),
        Some(Ask::Delete(name)) => run(app, "window.workspace.delete", json!({"name": name})),
        Some(Ask::Scroll(_)) | None => {}
    }
}
