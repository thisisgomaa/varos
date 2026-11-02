//! Registry-backed panel workspace with one ownership and interaction model.
//! The ordinary shell remains untouched when this review is not explicitly enabled.
mod content;
mod geometry;
pub(crate) mod metrics;
pub mod model;
mod paint;
mod registry;
pub(crate) mod workspace;

use crate::{VectorcraftApp, theme::Tokens};
use egui::{CursorIcon, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use geometry::{GroupRect, HostRect};
use model::{Action, Edge, Host, Layout, Mode, Source, Target};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub layout: Layout,
    #[serde(default)]
    pub workspaces: workspace::Store,
    /// What the last Tab / Shift+Tab hid, so the next one shows exactly that again.
    #[serde(skip)]
    hidden: Option<Hidden>,
    #[serde(skip)]
    compact_offsets: std::collections::BTreeMap<u64, f32>,
    #[serde(skip)]
    gesture: Option<Gesture>,
    #[serde(skip)]
    flyout: Option<u64>,
    #[serde(skip)]
    hits: Vec<(String, Rect)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            layout: Layout::new_slice(),
            workspaces: Default::default(),
            hidden: None,
            compact_offsets: Default::default(),
            gesture: None,
            flyout: None,
            hits: Vec::new(),
        }
    }
}

/// Malformed/unknown layouts never reach the renderer; legacy preferences remain independent.
pub(crate) fn load<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<State>, D::Error> {
    let raw = Value::deserialize(deserializer)?;
    let Some(mut state) = serde_json::from_value::<Option<State>>(raw).unwrap_or_default() else { return Ok(None) };
    if state.layout.normalize_tools().is_err() {
        return Ok(None);
    }
    state.workspaces = state.workspaces.sanitized();
    Ok(Some(state))
}

/// Tab hides every visible block; Shift+Tab hides every panel except Tools and Control, one
/// panel at a time (a panel attached beside Tools hides while Tools stays). `after` is the layout
/// the hide produced: when nothing changed since, the next toggle restores `before` exactly.
#[derive(Clone, Debug)]
struct Hidden {
    all: bool,
    before: Layout,
    after: Layout,
    hosts: Vec<u64>,
    panels: Vec<String>,
}

#[derive(Clone, Debug)]
struct Gesture {
    before: Layout,
    kind: GestureKind,
    grab: Vec2,
    origin: Rect,
    geometry: Vec<HostRect>,
    from_flyout: bool,
}
#[derive(Clone, Debug)]
enum GestureKind {
    Move(Source),
    Resize { host: u64, left: bool },
    Height { host: u64, top: bool },
    ColumnWidth { host: u64, group: u64, left: bool, width: f32 },
    Divider { split: u64, axis: model::Axis, min: f32, max: f32 },
}

/// The panel workspace presents the editor only while the Varos shell switch is on; turning that
/// switch off in Preferences returns to the classic dock without discarding the saved layout.
pub fn enabled(app: &VectorcraftApp) -> bool {
    active_layout(&app.ui).is_some()
}

/// The presented layout, or `None` while another presenter owns the editor chrome.
pub(crate) fn active_layout(ui: &crate::state::UiState) -> Option<&Layout> {
    if !ui.varos_shell.enabled {
        return None;
    }
    ui.varos_panel_review.as_ref().map(|state| &state.layout)
}

/// Whether library panel `panel` shows in the presented layout; `None` while the workspace is off.
pub(crate) fn library_open(ui: &crate::state::UiState, panel: &str) -> Option<bool> {
    active_layout(ui).map(|layout| layout.owner(panel).is_some_and(|host| !host.hidden))
}

/// The library panels shown in the presented layout; `None` while the workspace is off.
pub(crate) fn open_library_panels(ui: &crate::state::UiState) -> Option<Vec<String>> {
    let layout = active_layout(ui)?;
    Some(
        layout
            .hosts
            .iter()
            .filter(|host| !host.hidden)
            .flat_map(|host| model::groups(&host.root))
            .flat_map(|group| group.panels.iter())
            .filter(|panel| panel.starts_with(crate::panels::library_panel::PREFIX))
            .cloned()
            .collect(),
    )
}

/// A library panel showing another library keeps its place in the presented layout.
pub(crate) fn rename_panel(ui: &mut crate::state::UiState, from: &str, to: &str) -> Result<(), String> {
    if active_layout(ui).is_none() {
        return Ok(());
    }
    let Some(state) = ui.varos_panel_review.as_mut() else { return Ok(()) };
    if state.layout.owner(from).is_none() || state.layout.owner(to).is_some() {
        return Ok(());
    }
    let mut next = state.layout.clone();
    for host in &mut next.hosts {
        model::rename_panel(&mut host.root, from, to);
    }
    next.validate()?;
    state.layout = next;
    Ok(())
}

/// Forget every library panel outside `kept` (startup restores only persistent libraries).
pub(crate) fn retain_library_panels(ui: &mut crate::state::UiState, kept: &[String]) {
    if active_layout(ui).is_none() {
        return;
    }
    let Some(state) = ui.varos_panel_review.as_mut() else { return };
    let stale: Vec<String> = state
        .layout
        .hosts
        .iter()
        .flat_map(|host| model::groups(&host.root))
        .flat_map(|group| group.panels.iter())
        .filter(|panel| panel.starts_with(crate::panels::library_panel::PREFIX) && !kept.contains(panel))
        .cloned()
        .collect();
    for panel in stale {
        // A panel that cannot be detached stays where it is; the layout itself remains valid.
        let _ = close_panel(&mut state.layout, &panel, true);
    }
}

/// Close one panel without hiding the panels sharing its block: a panel alone hides its block
/// (Show brings it back where it was); a tab or attached group detaches into its own hidden block.
/// `forget` removes a dynamic (library) panel from the layout instead of hiding it.
fn close_panel(layout: &mut Layout, panel: &str, forget: bool) -> Result<(), String> {
    let Some(host) = layout.owner(panel) else { return Ok(()) };
    let alone = model::groups(&host.root).iter().map(|g| g.panels.len()).sum::<usize>() == 1;
    let position = host.position;
    let mut next = layout.clone();
    if !alone {
        next.apply(&Action::Move { source: Source::Panel(panel.into()), target: Target::Float { position } })?;
    }
    let owner = next.owner(panel).ok_or("Panel owner missing")?.id;
    if forget {
        next.hosts.retain(|h| h.id != owner);
        next.validate()?;
    } else {
        next.apply(&Action::Hide { host: owner })?;
    }
    *layout = next;
    Ok(())
}

fn is_special(panel: &str) -> bool {
    registry::get(panel).is_some_and(|spec| spec.kind != registry::PanelKind::Standard)
}

/// Hide what Tab (`all`) or Shift+Tab hides, recording it; `false` when nothing was visible.
fn hide_for_tab(state: &mut State, all: bool) -> bool {
    let before = state.layout.clone();
    let mut next = before.clone();
    let mut hosts = Vec::new();
    let mut panels = Vec::new();
    for host in before.hosts.iter().filter(|h| !h.hidden) {
        let contents: Vec<&String> = model::groups(&host.root).into_iter().flat_map(|g| g.panels.iter()).collect();
        if all || !contents.iter().any(|p| is_special(p)) {
            hosts.push(host.id);
        } else {
            panels.extend(contents.into_iter().filter(|p| !is_special(p)).cloned());
        }
    }
    for host in &mut next.hosts {
        if hosts.contains(&host.id) {
            host.hidden = true;
        }
    }
    panels.retain(|panel| close_panel(&mut next, panel, false).is_ok());
    if hosts.is_empty() && panels.is_empty() {
        return false;
    }
    state.layout = next;
    state.hidden = Some(Hidden { all, before, after: state.layout.clone(), hosts, panels });
    true
}

/// Show again what the last Tab / Shift+Tab hid; `false` when there is no record.
fn restore_hidden(state: &mut State) -> bool {
    let Some(record) = state.hidden.take() else { return false };
    if state.layout == record.after {
        state.layout = record.before;
        return true;
    }
    // The layout changed in between (a workspace switch, a dragged block): show what still exists.
    for host in &mut state.layout.hosts {
        if record.hosts.contains(&host.id) {
            host.hidden = false;
        }
    }
    for panel in &record.panels {
        // A panel removed meanwhile has nothing left to show.
        let _ = state.layout.apply(&Action::Show { panel: panel.clone() });
    }
    true
}

/// Is `panel` shown: its block is visible and, in a compact column, its flyout is open.
fn panel_showing(state: &State, panel: &str) -> bool {
    if !state.layout.owner(panel).is_some_and(|h| !h.hidden) {
        return false;
    }
    let Some(group) = state.layout.group_of(panel) else { return false };
    state.layout.column_mode(group.id).is_none_or(|mode| mode == Mode::Full) || state.flyout == Some(group.id)
}

/// A panel requested through `open_panel` (Control bar links, Properties "more options", older
/// callers) shows in the presented layout, as the other shells follow that request.
fn follow_open_panel(app: &mut VectorcraftApp) -> bool {
    let Some(request) = app.ui.open_panel.take() else { return false };
    let panel = if registry::contains(&request) {
        request
    } else if let Some(panel) = crate::menus::normalize_panel(&request) {
        panel.to_string()
    } else {
        app.status(format!("unknown panel `{request}`"));
        return false;
    };
    if let Err(error) = app.run("window.panel", json!({"panel": panel})) {
        app.status(error);
    }
    true
}

pub fn command(app: &mut VectorcraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if id != "ui.panelReview" && !enabled(app) {
        return None;
    }
    if id == "ui.panelReview" {
        if params.get("enabled").and_then(Value::as_bool) == Some(false) {
            app.ui.varos_panel_review = None;
            return Some(Ok(json!({"enabled":false})));
        }
        app.ui.varos_shell.enabled = true;
        app.ui.varos_panel_review.get_or_insert_with(|| State { layout: Layout::new_workspace(), ..Default::default() });
        return Some(Ok(json!({"enabled":true,"panels":registry::BUILTINS.iter().map(|p|p.id).collect::<Vec<_>>()})));
    }
    if let Some(result) = workspace::command(app, id, params) {
        return Some(result);
    }
    let state = app.ui.varos_panel_review.as_mut()?;
    if id == "ui.panelReview.inspect" {
        return Some(Ok(
            json!({"layout":state.layout,"dragging":state.gesture.is_some(),"hits":state.hits.iter().map(|(name,r)|json!({"name":name,"rect":[r.left(),r.top(),r.width(),r.height()]})).collect::<Vec<_>>()}),
        ));
    }
    if id == "ui.panelReview.action" {
        let action = params
            .get("action")
            .cloned()
            .ok_or_else(|| "Expected action".to_string())
            .and_then(|v| serde_json::from_value::<Action>(v).map_err(|e| e.to_string()));
        return Some(action.and_then(|action| {
            state.layout.apply(&action)?;
            if !matches!(action, Action::Activate { .. } | Action::Raise { .. } | Action::FitControlContent { .. }) {
                state.flyout = None;
            }
            Ok(json!({"layout":state.layout}))
        }));
    }
    if id == "window.panel.layout" {
        if let Some(action) = params.get("action") {
            return Some(
                serde_json::from_value::<Action>(action.clone())
                    .map_err(|e| e.to_string())
                    .and_then(|a| state.layout.apply(&a))
                    .map(|()| json!({"layout":state.layout})),
            );
        }
        return Some(Ok(json!({"layout":state.layout})));
    }
    if id == "ui.dock.reset" {
        state.layout = Layout::new_workspace();
        state.gesture = None;
        state.flyout = None;
        return Some(Ok(json!({"layout":state.layout})));
    }
    if matches!(id, "window.panels" | "window.dock") {
        state.flyout = None;
        let all = id == "window.dock";
        let shown = match state.hidden.as_ref().map(|record| record.all) {
            // Tab after Shift+Tab hides the bars too; Tab after Tab shows everything again.
            Some(false) if all => {
                restore_hidden(state);
                !hide_for_tab(state, true)
            }
            Some(record_all) => {
                restore_hidden(state);
                if record_all && !all {
                    // Shift+Tab after Tab shows the panels and leaves Tools and Control hidden.
                    for host in &mut state.layout.hosts {
                        if model::groups(&host.root).iter().all(|g| g.panels.iter().all(|p| is_special(p))) {
                            host.hidden = true;
                        }
                    }
                }
                true
            }
            None if hide_for_tab(state, all) => false,
            None if all => {
                // Nothing showed and nothing was hidden by Tab: show Tools and Control.
                for panel in ["tools", "control"] {
                    if let Err(e) = state.layout.ensure_panel(panel).and_then(|()| state.layout.apply(&Action::Show { panel: panel.into() })) {
                        return Some(Err(e));
                    }
                }
                true
            }
            None => true,
        };
        return Some(Ok(json!({"shown":shown})));
    }
    if matches!(id, "window.toolbar" | "window.control") {
        let panel = if id == "window.toolbar" { "tools" } else { "control" };
        let action =
            state.layout.owner(panel).filter(|h| !h.hidden).map_or_else(|| Action::Show { panel: panel.into() }, |h| Action::Hide { host: h.id });
        return Some(state.layout.ensure_panel(panel).and_then(|()| state.layout.apply(&action)).map(|()| json!({"layout":state.layout})));
    }
    if id == "window.toolbarColumns" {
        let double =
            params.get("double").and_then(Value::as_bool).unwrap_or(state.layout.tools_width().is_none_or(|w| w <= metrics::TOOLS_MIN_WIDTH));
        let width = if double { metrics::TOOLS_MAX_WIDTH } else { metrics::TOOLS_MIN_WIDTH };
        if let Err(e) = state.layout.apply(&Action::SetToolsWidth { width }) {
            return Some(Err(e));
        }
        app.ui.toolbar_double = double;
        app.ui.varos_shell.tools_columns = if double { 2 } else { 1 };
        return Some(Ok(json!({"double":double})));
    }
    if id.starts_with("window.panel") {
        let raw = params.get("panel").and_then(Value::as_str).unwrap_or_default();
        let panel = if registry::contains(raw) { raw } else { crate::menus::normalize_panel(raw).unwrap_or_default() };
        let toggle = id == "window.panel" && params.get("toggle").and_then(Value::as_bool) == Some(true);
        if (toggle && panel_showing(state, panel)) || id == "window.panel.close" {
            if !registry::contains(panel) {
                return Some(Err(format!("unknown panel `{raw}`")));
            }
            // A compact column's open flyout closes first; its icon stays in the column.
            if let Some(group) = state.layout.group_of(panel).map(|g| g.id)
                && state.flyout == Some(group)
                && state.layout.column_mode(group).is_some_and(|mode| mode != Mode::Full)
            {
                state.flyout = None;
                return Some(Ok(json!({"layout":state.layout,"open":null})));
            }
            let forget = crate::panels::library_panel::parse(panel).is_some();
            let mut next = state.layout.clone();
            return Some(close_panel(&mut next, panel, forget).map(|()| {
                state.layout = next;
                state.flyout = None;
                json!({"layout":state.layout,"open":null})
            }));
        }
        let mut next = state.layout.clone();
        let result = (|| -> Result<Action, String> {
            next.ensure_panel(panel)?;
            let host = next.owner(panel).ok_or("Panel owner missing")?;
            let group = next.group_of(panel).ok_or("Panel group missing")?;
            let source = if params.get("column").and_then(Value::as_bool) == Some(true) {
                Source::Host(host.id)
            } else if params.get("group").and_then(Value::as_bool) == Some(true) {
                Source::Group(group.id)
            } else {
                Source::Panel(panel.into())
            };
            let action = match id {
                "window.panel" | "window.panel.activate" => Action::Show { panel: panel.into() },
                "window.panel.collapse" | "window.panel.minimize" if panel == "tools" => {
                    // Tools never become an icon column: collapsing narrows them to one column.
                    let narrow = params
                        .get("collapsed")
                        .or_else(|| params.get("minimized"))
                        .and_then(Value::as_bool)
                        .unwrap_or(next.tools_width().is_some_and(|w| w > metrics::TOOLS_MIN_WIDTH));
                    Action::SetToolsWidth { width: if narrow { metrics::TOOLS_MIN_WIDTH } else { metrics::TOOLS_MAX_WIDTH } }
                }
                "window.panel.collapse" | "window.panel.minimize" => {
                    let collapsed = params
                        .get("collapsed")
                        .or_else(|| params.get("minimized"))
                        .and_then(Value::as_bool)
                        .unwrap_or(next.column_mode(group.id) == Some(Mode::Full));
                    Action::SetColumnMode {
                        group: group.id,
                        mode: if collapsed {
                            if host.dock.is_some() && host.compact_width >= 72.0 { Mode::Names } else { Mode::Icons }
                        } else {
                            Mode::Full
                        },
                    }
                }
                "window.panel.float" => Action::Move {
                    source,
                    target: Target::Float {
                        position: [
                            params.get("x").and_then(Value::as_f64).map_or(host.position[0], |v| v as f32),
                            params.get("y").and_then(Value::as_f64).map_or(host.position[1], |v| v as f32),
                        ],
                    },
                },
                "window.panel.move" | "window.panel.dock" => {
                    let zone = params.get("zone").and_then(Value::as_str).unwrap_or("tab");
                    let edge = match zone {
                        "left" => Some(Edge::Left),
                        "right" => Some(Edge::Right),
                        "top" | "above" => Some(Edge::Top),
                        "bottom" | "below" => Some(Edge::Bottom),
                        "tab" | "center" => None,
                        _ => return Err("Unknown panel drop zone".into()),
                    };
                    let anchor = params.get("anchor").or_else(|| params.get("onto")).and_then(Value::as_str);
                    let target = if let Some(anchor) = anchor {
                        let g = next.group_of(anchor).ok_or("Unknown destination panel")?;
                        if let Some(edge) = edge {
                            Target::Split { group: g.id, edge }
                        } else {
                            let index = params
                                .get("before")
                                .and_then(Value::as_str)
                                .and_then(|p| g.panels.iter().position(|v| v == p))
                                .unwrap_or(g.panels.len());
                            Target::Merge { group: g.id, index }
                        }
                    } else if id == "window.panel.dock" {
                        Target::Dock {
                            edge: edge.unwrap_or(match registry::get(panel).map(|s| s.kind) {
                                Some(registry::PanelKind::Tools) => Edge::Left,
                                Some(registry::PanelKind::Control) => Edge::Top,
                                _ => Edge::Right,
                            }),
                        }
                    } else {
                        return Err("Expected destination panel".into());
                    };
                    Action::Move { source, target }
                }
                _ => return Err("Unknown panel layout command".into()),
            };
            next.apply(&action)?;
            Ok(action)
        })();
        return Some(result.map(|action| {
            state.layout = next;
            state.flyout = if matches!(action, Action::Show { .. })
                && state.layout.group_of(panel).is_some_and(|g| state.layout.column_mode(g.id) != Some(Mode::Full))
            {
                state.layout.group_of(panel).map(|g| g.id)
            } else {
                None
            };
            json!({"layout":state.layout})
        }));
    }
    None
}

/// A layout gesture owns keyboard input until it commits or is cancelled.
pub fn gesture_keys(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    let Some(state) = app.ui.varos_panel_review.as_mut() else { return false };
    if state.gesture.is_none() {
        return false;
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        state.gesture = None;
        state.flyout = None;
        ctx.stop_dragging();
        ctx.request_repaint();
    }
    true
}

pub fn showing(app: &VectorcraftApp, panel: &str) -> bool {
    enabled(app) && app.ui.varos_panel_review.as_ref().is_some_and(|state| panel_showing(state, panel))
}

fn run(app: &mut VectorcraftApp, action: Action) {
    if let Err(error) = app.run("ui.panelReview.action", json!({"action":action})) {
        app.status(error);
    }
}

/// Home suspends editor interactions without changing the saved workspace.
pub(crate) fn suspend_for_home(app: &mut VectorcraftApp) {
    if let Some(state) = app.ui.varos_panel_review.as_mut() {
        state.gesture = None;
        state.flyout = None;
        state.hits.clear();
    }
}

pub fn frame(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    follow_open_panel(app);
    if let Some(width) = app.ui.varos_panel_review.as_ref().and_then(|s| s.layout.tools_width()) {
        app.ui.toolbar_double = width >= 2.0 * metrics::TOOL_BUTTON + model::HOST_CHROME[0];
        app.ui.varos_shell.tools_columns = if app.ui.toolbar_double { 2 } else { 1 };
    }
    let t = Tokens::get(ui.ctx());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.panel_darker)).show(ui, |ui| {
        let region = ui.max_rect();
        if app.ui.screen_mode >= 3 {
            // Presentation Mode shows the artwork alone: no Tools, Control or panels, so no panel
            // body can edit the document, and no block reserves canvas space.
            suspend_for_home(app);
            super::shell::boxes::board(app, ui, region, false);
            return;
        }
        draw(app, ui, region);
        content::popovers(app, ui.ctx(), region);
        // A panel requested while this frame drew (a Control bar link) shows on the next frame.
        if follow_open_panel(app) {
            ui.ctx().request_repaint();
        }
    });
}

fn draw(app: &mut VectorcraftApp, ui: &mut egui::Ui, region: Rect) {
    let ctx = ui.ctx().clone();
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    if ctx.input(|i| i.pointer.primary_clicked())
        && app.ui.varos_panel_review.as_ref().is_some_and(|s| s.gesture.is_none())
        && let Some(p) = pointer
    {
        let front =
            app.ui
                .varos_panel_review
                .as_ref()
                .and_then(|s| {
                    s.layout.hosts.iter().rev().find(|h| {
                        !h.hidden && ctx.layer_id_at(p) == Some(egui::LayerId::new(Order::Middle, Id::unique("panel-review-host").with(h.id)))
                    })
                })
                .map(|h| h.id);
        if let Some(host) = front {
            run(app, Action::Raise { host });
        }
    }
    let Some(mut state) = app.ui.varos_panel_review.clone() else { return };
    let cancelled = ctx.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused || i.events.iter().any(|e| matches!(e, egui::Event::PointerGone)));
    let released = ctx.input(|i| i.pointer.primary_released());
    if cancelled {
        state.gesture = None;
        state.flyout = None;
    }
    let mut final_action = None;
    let mut final_column_position = None;
    let mut expand_after_drop = None;
    let mut shown = state.layout.clone();
    let mut drop_line = None;
    if let Some(gesture) = state.gesture.clone() {
        if gesture.before != state.layout {
            state.gesture = None;
        } else if let Some(p) = pointer {
            match &gesture.kind {
                GestureKind::Move(source) => {
                    let position = [(p.x - gesture.grab.x - region.left()).max(0.0), (p.y - gesture.grab.y - region.top()).max(0.0)];
                    if shown.apply(&Action::Move { source: source.clone(), target: Target::Float { position } }).is_ok() {
                        let panel = source_panel(&gesture.before, source);
                        if let Some(owner) = panel.as_deref().and_then(|p| shown.owner(p)).map(|h| h.id)
                            && let Some(index) = shown.hosts.iter().position(|h| h.id == owner)
                        {
                            if gesture.from_flyout && shown.apply(&Action::SetMode { host: owner, mode: Mode::Full }).is_err() {
                                state.gesture = None;
                            }
                            let h = shown.hosts.remove(index);
                            shown.hosts.push(h);
                        }
                    }
                    // Commit the visible, constrained origin so collapsing at a workspace edge
                    // cannot uncover an off-screen stored position and make the block jump.
                    let floating_position = source_panel(&gesture.before, source)
                        .as_deref()
                        .and_then(|panel| shown.owner(panel))
                        .map(|host| {
                            let r = geometry::measure_host(host, region).rect;
                            [r.left() - region.left(), r.top() - region.top()]
                        })
                        .unwrap_or(position);
                    let target = target_at(&gesture.before, &gesture.geometry, source, p, region).map(|(target, line)| {
                        (if matches!(target, Target::Float { .. }) { Target::Float { position: floating_position } } else { target }, line)
                    });
                    drop_line = target.as_ref().and_then(|(_, line)| *line);
                    if released {
                        if gesture.from_flyout && target.as_ref().is_some_and(|(t, _)| matches!(t, Target::Float { .. })) {
                            expand_after_drop = source_panel(&gesture.before, source);
                        }
                        final_action = target.map(|(target, _)| Action::Move { source: source.clone(), target });
                    }
                }
                GestureKind::Height { host, top } => {
                    let r = gesture.origin;
                    let trim = state.layout.host(*host).map_or(0.0, geometry::docked_bar_trim);
                    let minimum = state.layout.host(*host).map_or(180.0, |h| {
                        let minimum = geometry::minimum_height(&h.root);
                        if standalone_kind(h) == Some(registry::PanelKind::Tools) { minimum.max(h.expanded[0]) } else { minimum }
                    }) - trim;
                    let bottom = if *top {
                        r.bottom()
                    } else {
                        (p.y - gesture.grab.y).clamp(r.top() + minimum.min(region.bottom() - r.top()), region.bottom())
                    };
                    let mut y = if *top { (p.y - gesture.grab.y).clamp(region.top(), (r.bottom() - minimum).max(region.top())) } else { r.top() };
                    let width = state.layout.host(*host).filter(|h| h.dock.is_some()).map_or(r.width(), |h| h.expanded[0]);
                    let mut height = (bottom - y).max(1.0);
                    if state.layout.host(*host).is_some_and(|h| standalone_kind(h) == Some(registry::PanelKind::Control)) {
                        height = height.min((width - trim).max(1.0));
                        if *top {
                            y = bottom - height;
                        }
                    }
                    let action = Action::Resize { host: *host, position: [r.left() - region.left(), y - region.top()], width, height: height + trim };
                    if shown.apply(&action).is_err() {
                        state.gesture = None;
                    }
                    if released {
                        final_action = Some(action);
                    }
                }
                GestureKind::Divider { split, axis, min, max } => {
                    let r = gesture.origin;
                    let ratio = if *axis == model::Axis::Horizontal {
                        (p.x - r.left()) / (r.width() - model::SPLIT_GAP).max(1.0)
                    } else {
                        (p.y - r.top()) / (r.height() - model::SPLIT_GAP).max(1.0)
                    };
                    let action = Action::SplitRatio { split: *split, ratio: ratio.clamp(*min, *max) };
                    if shown.apply(&action).is_err() {
                        state.gesture = None;
                    }
                    if released {
                        final_action = Some(action);
                    }
                }
                GestureKind::ColumnWidth { host, group, left, width } => {
                    let delta = p.x - gesture.grab.x - if *left { gesture.origin.left() } else { gesture.origin.right() };
                    let width = (*width + if *left { -delta } else { delta }).clamp(1.0, region.width().max(1.0));
                    let action = Action::ResizeColumn { group: *group, width };
                    if shown.apply(&action).is_err() {
                        state.gesture = None;
                    } else if *left && state.layout.host(*host).is_some_and(|h| h.dock.is_none()) {
                        // Floating left-edge resizing keeps the block's opposite edge fixed.
                        if let (Some(original), Some(after)) =
                            (state.layout.host(*host), shown.host(*host).map(|h| geometry::measure_host(h, region).rect))
                        {
                            let before = geometry::measure_host(original, region).rect;
                            let x = (before.right() - after.width() - region.left()).max(0.0);
                            if let Some(h) = shown.hosts.iter_mut().find(|h| h.id == *host) {
                                h.position[0] = x;
                            }
                        }
                    }
                    if released {
                        final_action = Some(action);
                        if *left && let Some(h) = shown.host(*host).filter(|h| h.dock.is_none()) {
                            final_column_position = Some((*host, h.position));
                        }
                    }
                }
                GestureKind::Resize { host, left } => {
                    let r = gesture.origin;
                    let full = state.layout.host(*host).is_some_and(|h| h.mode == Mode::Full);
                    let minimum = if full {
                        state.layout.host(*host).map_or(240.0, |h| {
                            let minimum = geometry::minimum_width(&h.root);
                            if standalone_kind(h) == Some(registry::PanelKind::Control) { minimum.max(h.expanded[1]) } else { minimum }
                        })
                    } else {
                        paint::COMPACT
                    };
                    let right = if *left {
                        r.right()
                    } else {
                        (p.x - gesture.grab.x).clamp(r.left() + minimum.min(region.right() - r.left()), region.right())
                    };
                    let left_edge =
                        if *left { (p.x - gesture.grab.x).clamp(region.left(), (r.right() - minimum).max(region.left())) } else { r.left() };
                    let mut width = (right - left_edge).min(if full { region.width() } else { 320.0 }).max(1.0);
                    if full
                        && let Some(h) = state.layout.host(*host)
                        && standalone_kind(h) == Some(registry::PanelKind::Tools)
                    {
                        width = width.clamp(metrics::TOOLS_MIN_WIDTH, metrics::TOOLS_MAX_WIDTH).min(h.expanded[1]);
                    }
                    let x = if *left { right - width } else { left_edge };
                    let mut action = Action::Resize {
                        host: *host,
                        position: [x - region.left(), r.top() - region.top()],
                        width,
                        height: state.layout.host(*host).filter(|h| h.dock.is_some()).map_or(r.height(), |h| h.expanded[1]),
                    };
                    if shown.apply(&action).is_err() {
                        state.gesture = None;
                    } else if *left && state.layout.host(*host).is_some_and(|h| h.dock.is_none() && model::has_tools(&h.root)) {
                        // Bounding a Tools leaf can reduce the resulting combined width.
                        // Anchor using that final width, then reapply once from the original tree.
                        if let Some(measured) = geometry::measure(&shown, region).iter().find(|h| h.id == *host) {
                            let anchored_x = (r.right() - measured.rect.width() - region.left()).max(0.0);
                            if let Action::Resize { position, .. } = &mut action {
                                position[0] = anchored_x;
                            }
                            shown = state.layout.clone();
                            if shown.apply(&action).is_err() {
                                state.gesture = None;
                            }
                        }
                    }
                    if released {
                        final_action = Some(action);
                    }
                }
            }
        }
        if released {
            state.gesture = None;
        }
    }
    app.ui.varos_panel_review = Some(state.clone());
    if let Some(action) = final_action {
        run(app, action);
        if let Some((host, position)) = final_column_position {
            run(app, Action::Move { source: Source::Host(host), target: Target::Float { position } });
        }
        if let Some(host) = expand_after_drop.as_deref().and_then(|p| app.ui.varos_panel_review.as_ref()?.layout.owner(p)).map(|h| h.id) {
            run(app, Action::SetMode { host, mode: Mode::Full });
        }
        if let Some(s) = &app.ui.varos_panel_review {
            state = s.clone();
            shown = s.layout.clone();
        }
        drop_line = None;
    } else if released && state.gesture.is_none() {
        // An invalid/outside drop ends the gesture without committing. Discard its
        // floating preview now, so this release frame already matches saved docks.
        shown = state.layout.clone();
        drop_line = None;
    }
    if released || cancelled {
        ctx.request_repaint();
    }
    // The canvas and panel chrome must use the same preview layout. Keeping the
    // committed dock reservation while its panel is visibly floating leaves an
    // empty strip behind until release (and during a dock-width resize).
    let (mut measured, board) = geometry::arrange(&shown, region);
    // Floating Areas are separate egui layers, so canvas gestures cannot start through them.
    super::shell::boxes::board(app, ui, board, false);
    if state.gesture.is_none()
        && let Some(p) = pointer
        && let Some(host) = measured.iter().rev().find(|h| h.rect.contains(p))
        && let Some(column) = host.columns.iter().find(|c| c.rect.contains(p) && c.mode != Mode::Full)
    {
        let delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if delta != 0.0 {
            let offset = state.compact_offsets.entry(column.key).or_default();
            *offset = (*offset - delta).clamp(0.0, geometry::compact_column_scroll_max(host, column.key));
        }
    }
    geometry::scroll_compact(&mut measured, &shown, &state.compact_offsets);
    let mut hits = Vec::new();
    let mut pending = None;
    let mut actions = Vec::new();
    let mut flyout = if state.gesture.is_some() { None } else { state.flyout };
    // Register bounds before input, then raise only the actual front host. egui queues
    // raises as a set: raising every host preserves their old order, so a formerly
    // docked Control could stay behind an ordinary floating panel after detaching.
    for item in &measured {
        let layer = egui::LayerId::new(Order::Middle, Id::unique("panel-review-host").with(item.id));
        ctx.memory_mut(|m| {
            m.areas_mut().set_state(layer, egui::AreaState { pivot_pos: Some(item.rect.min), size: Some(item.rect.size()), ..Default::default() })
        });
    }
    if let Some(front) = measured.last() {
        ctx.move_to_top(egui::LayerId::new(Order::Middle, Id::unique("panel-review-host").with(front.id)));
    }
    for item in &measured {
        let Some(host) = shown.host(item.id) else { continue };
        render_host(app, &ctx, host, item, region, &state, &mut hits, &mut pending, &mut actions, &mut flyout);
    }
    if let Some(group) = flyout {
        render_flyout(app, &ctx, &shown, &measured, group, region, &mut hits, &mut pending, &mut actions, &mut flyout);
    }
    if let Some(line) = drop_line {
        ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::unique("panel-review-drop"))).rect_filled(line, 0, Tokens::get(&ctx).accent);
    }
    if let Some(s) = &mut app.ui.varos_panel_review {
        s.hits = hits;
        s.flyout = flyout;
        s.compact_offsets = state.compact_offsets.clone();
        if let Some((kind, rect, from_flyout)) = pending {
            let press = ctx.input(|i| i.pointer.press_origin()).or(pointer).unwrap_or(rect.min);
            let grab = match &kind {
                GestureKind::Resize { left, .. } | GestureKind::ColumnWidth { left, .. } => {
                    vec2(press.x - if *left { rect.left() } else { rect.right() }, 0.0)
                }
                GestureKind::Height { top, .. } => vec2(0.0, press.y - if *top { rect.top() } else { rect.bottom() }),
                _ => press - rect.min,
            };
            let mut geometry = geometry::measure(&s.layout, region);
            geometry::scroll_compact(&mut geometry, &s.layout, &s.compact_offsets);
            s.gesture = Some(Gesture { before: s.layout.clone(), kind, grab, origin: rect, geometry, from_flyout });
            s.flyout = None;
            ctx.request_repaint();
        }
    }
    for action in actions {
        run(app, action);
        ctx.request_repaint();
    }
    // Both floating and docked Control follow their actual wrapped rows.
    // Defer fitting during pointer gestures so their committed snapshot remains stable.
    let fitted: Vec<_> = app.ui.varos_panel_review.as_ref().filter(|s| s.gesture.is_none()).map_or_else(Vec::new, |s| {
        s.layout
            .hosts
            .iter()
            .filter_map(|host| {
                if host.hidden || host.mode != Mode::Full || standalone_kind(host) != Some(registry::PanelKind::Control) {
                    return None;
                }
                let panel = model::groups(&host.root).first()?.panels.first()?.clone();
                let height = content::control_height(&ctx, &panel)?.ceil();
                (height.is_finite() && height > 0.0 && host.dock_content_height.is_none_or(|old| (old - height).abs() > 0.5))
                    .then_some(Action::FitControlContent { host: host.id, height })
            })
            .collect()
    });
    for action in fitted {
        run(app, action);
        ctx.request_repaint();
    }
    if state.gesture.is_some() {
        ctx.set_cursor_icon(match state.gesture.as_ref().map(|g| &g.kind) {
            Some(GestureKind::Resize { .. } | GestureKind::ColumnWidth { .. }) => CursorIcon::ResizeHorizontal,
            Some(GestureKind::Height { .. }) => CursorIcon::ResizeVertical,
            Some(GestureKind::Divider { axis: model::Axis::Horizontal, .. }) => CursorIcon::ResizeHorizontal,
            Some(GestureKind::Divider { .. }) => CursorIcon::ResizeVertical,
            _ => CursorIcon::Default,
        });
        ctx.request_repaint();
    }
}

fn source_panel(layout: &Layout, source: &Source) -> Option<String> {
    match source {
        Source::Panel(p) => Some(p.clone()),
        Source::Group(id) => layout.group(*id)?.panels.first().cloned(),
        Source::Host(id) => model::groups(&layout.host(*id)?.root).first()?.panels.first().cloned(),
    }
}

fn standalone_kind(host: &Host) -> Option<registry::PanelKind> {
    let model::Node::Group(group) = &host.root else { return None };
    if group.panels.len() != 1 {
        return None;
    }
    registry::get(&group.active).map(|s| s.kind)
}

fn valid_target(layout: &Layout, source: &Source, target: Target, line: Rect) -> Option<(Target, Option<Rect>)> {
    let mut next = layout.clone();
    next.apply(&Action::Move { source: source.clone(), target: target.clone() }).ok()?;
    Some((target, Some(line)))
}

fn target_at(layout: &Layout, measured: &[HostRect], source: &Source, p: Pos2, region: Rect) -> Option<(Target, Option<Rect>)> {
    if !region.contains(p) {
        return None;
    }
    let floating = || Some((Target::Float { position: [p.x - region.left(), p.y - region.top()] }, None));
    let edges = [
        (Edge::Top, p.y - region.top()),
        (Edge::Bottom, region.bottom() - p.y),
        (Edge::Left, p.x - region.left()),
        (Edge::Right, region.right() - p.x),
    ];
    for (edge, distance) in edges {
        if distance < 10.0
            && let Some(target) = valid_target(layout, source, Target::Dock { edge }, geometry::edge_line(region, edge))
        {
            return Some(target);
        }
    }
    if edges.iter().any(|(_, distance)| *distance < 10.0) {
        return floating();
    }
    let source_host = layout.source_host(source)?;
    let whole = matches!(source, Source::Host(_))
        || match &source_host.root {
            model::Node::Group(g) => matches!(source,Source::Group(id) if *id==g.id) || matches!(source,Source::Panel(_) if g.panels.len()==1),
            _ => false,
        };
    for host in measured.iter().rev() {
        if whole && host.id == source_host.id {
            continue;
        }
        if !host.rect.expand(12.0).contains(p) {
            continue;
        }
        let outer = if (p.y - host.rect.bottom()).abs() <= 12.0 {
            Some(Edge::Bottom)
        } else if (p.y - host.rect.top()).abs() <= 8.0 {
            Some(Edge::Top)
        } else if layout.host(host.id).is_some_and(|h| h.mode == Mode::Full) && (p.x - host.rect.left()).abs() <= 8.0 {
            Some(Edge::Left)
        } else if layout.host(host.id).is_some_and(|h| h.mode == Mode::Full) && (p.x - host.rect.right()).abs() <= 8.0 {
            Some(Edge::Right)
        } else {
            None
        };
        if let Some(edge) = outer {
            return valid_target(layout, source, Target::Attach { host: host.id, edge }, geometry::edge_line(host.rect, edge)).or_else(floating);
        }
        for divider in &host.dividers {
            if divider.rect.expand(3.0).contains(p) {
                let second = host.groups.iter().find(|g| {
                    if divider.axis == model::Axis::Horizontal {
                        g.rect.left() >= divider.rect.right() - 0.5
                    } else {
                        g.rect.top() >= divider.rect.bottom() - 0.5
                    }
                });
                if let Some(g) = second {
                    let edge = if divider.axis == model::Axis::Horizontal { Edge::Left } else { Edge::Top };
                    if let Some(target) = valid_target(layout, source, Target::Split { group: g.id, edge }, geometry::edge_line(g.rect, edge)) {
                        return Some(target);
                    }
                }
            }
        }
        for g in &host.groups {
            if !g.rect.contains(p) || matches!(source,Source::Group(id) if *id==g.id) {
                continue;
            }
            let compact = g.mode != Mode::Full;
            let over_tabs = compact || p.y <= g.rect.top() + paint::TABS;
            let side_zone = 28.0_f32.min(g.rect.width() * 0.25);
            let edge = if p.y - g.rect.top() < 8.0 {
                Some(Edge::Top)
            } else if g.rect.bottom() - p.y < 8.0 {
                Some(Edge::Bottom)
            } else if !over_tabs && p.x - g.rect.left() < side_zone {
                Some(Edge::Left)
            } else if !over_tabs && g.rect.right() - p.x < side_zone {
                Some(Edge::Right)
            } else {
                None
            };
            if let Some(edge) = edge {
                return valid_target(layout, source, Target::Split { group: g.id, edge }, geometry::edge_line(g.rect, edge)).or_else(floating);
            }
            if g.tabs.is_empty() {
                // Standalone bars have their own contents, not a hidden tab-merge zone.
                return floating();
            }
            let index = if over_tabs {
                g.tabs.iter().position(|(_, r)| if compact { p.y < r.center().y } else { p.x < r.center().x }).unwrap_or(g.tabs.len())
            } else {
                g.tabs.len()
            };
            let r = g.tabs.get(index).map(|(_, r)| *r).or_else(|| g.tabs.last().map(|(_, r)| *r))?;
            let line = if compact {
                Rect::from_min_size(pos2(r.left(), if index == g.tabs.len() { r.bottom() } else { r.top() }), vec2(r.width(), 2.0))
            } else {
                Rect::from_min_size(pos2(if index == g.tabs.len() { r.right() } else { r.left() }, r.top()), vec2(2.0, r.height()))
            };
            return valid_target(layout, source, Target::Merge { group: g.id, index }, line).or_else(floating);
        }
    }
    // Preserve the exact grab offset on release, including a detached tab's new host.
    floating()
}

#[allow(clippy::too_many_arguments)]
fn render_host(
    app: &mut VectorcraftApp,
    ctx: &egui::Context,
    host: &Host,
    geo: &HostRect,
    region: Rect,
    state: &State,
    hits: &mut Vec<(String, Rect)>,
    pending: &mut Option<(GestureKind, Rect, bool)>,
    actions: &mut Vec<Action>,
    flyout: &mut Option<u64>,
) {
    let id = Id::unique("panel-review-host").with(host.id);
    let rect = geo.rect;
    egui::Area::new(id).order(Order::Middle).fixed_pos(rect.min).constrain(false).fade_in(false).movable(false).show(ctx, |ui| {
        ui.set_min_size(rect.size());
        ui.set_clip_rect(region.intersect(rect));
        let docked = host.dock.is_some();
        let footer = geometry::footer(host);
        let control = geometry::is_control(host);
        paint::frame(ui, rect, docked, geometry::title_height(host));
        let title = Rect::from_min_size(rect.min, vec2(rect.width(), geometry::title_height(host)));
        hits.push((format!("host:{}", host.id), rect));
        if control {
            let grip = geometry::control_grip(rect);
            let t = Tokens::get(ctx);
            ui.painter().rect_filled(grip, 0, t.panel_darker);
            let center = grip.center();
            for offset in [-4.0, 0.0, 4.0] {
                ui.painter().circle_filled(center + vec2(0.0, offset), 0.8, t.text_dim);
            }
            let response = ui.interact(grip, id.with("drag"), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Default);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Move Control bar"));
            hits.extend([(format!("control-grip:{}", host.id), grip), (format!("host-grip:{}", host.id), grip)]);
            if response.drag_started() && state.gesture.is_none() {
                *pending = Some((GestureKind::Move(Source::Host(host.id)), rect, false));
            }
        } else {
            let close = Rect::from_min_size(title.min, vec2(20.0, paint::TITLE));
            // Each column arrow owns its entire hit region; title dragging must not overlap it.
            for (index, column) in geo.columns.iter().enumerate() {
                let left = if index == 0 { if docked { title.left() } else { close.right() } } else { column.rect.left() };
                let grip = Rect::from_min_max(pos2(left, title.top()), pos2(column.rect.right() - 19.0, title.bottom()));
                if grip.is_positive() {
                    let response = ui.interact(grip, id.with(("drag", column.key)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Default);
                    if index == 0 {
                        hits.push((format!("host-grip:{}", host.id), grip));
                    }
                    if response.drag_started() && state.gesture.is_none() {
                        *pending = Some((GestureKind::Move(Source::Host(host.id)), rect, false));
                    }
                }
            }
            if !docked {
                let response = ui.interact(close, id.with("close"), Sense::click());
                paint::control(ui, close, "x", response.hovered());
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Hide panel block"));
                hits.push((format!("close:{}", host.id), close));
                if response.clicked() && state.gesture.is_none() {
                    actions.push(Action::Hide { host: host.id });
                }
            }
            for (index, column) in geo.columns.iter().enumerate() {
                let toggle = Rect::from_min_size(pos2(column.rect.right() - 19.0, title.top()), vec2(20.0, paint::TITLE));
                let tools = geo.groups.iter().any(|g| {
                    g.column == column.key
                        && model::groups(&host.root).iter().any(|source| source.id == g.id && source.panels.iter().any(|p| p == "tools"))
                });
                let narrow_tools = state.layout.tools_width().is_some_and(|w| w <= metrics::TOOLS_MIN_WIDTH + 0.01);
                let action = if tools {
                    Action::SetToolsWidth { width: if narrow_tools { metrics::TOOLS_MAX_WIDTH } else { metrics::TOOLS_MIN_WIDTH } }
                } else {
                    Action::SetColumnMode {
                        group: column.key,
                        mode: if column.mode == Mode::Full {
                            if docked && column.compact_width >= 72.0 { Mode::Names } else { Mode::Icons }
                        } else {
                            Mode::Full
                        },
                    }
                };
                let response = ui.interact(toggle, id.with(("column-mode", column.key)), Sense::click());
                paint::control(
                    ui,
                    toggle,
                    if if tools { !narrow_tools } else { (column.mode == Mode::Full) != (host.dock == Some(Edge::Right)) } {
                        "shell-chevrons-left"
                    } else {
                        "shell-chevrons-right"
                    },
                    response.hovered(),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        if tools { "Switch toolbar width" } else { "Collapse or expand panel column" },
                    )
                });
                hits.push((format!("column-mode:{}", column.key), toggle));
                if index + 1 == geo.columns.len() {
                    hits.push((format!("mode:{}", host.id), toggle));
                }
                if response.clicked() && state.gesture.is_none() {
                    if !docked && !tools && column.mode == Mode::Full {
                        // Collapse at the visible origin, including an oversized saved position
                        // constrained by the current workspace. Never reveal that stale position.
                        actions.push(Action::Move {
                            source: Source::Host(host.id),
                            target: Target::Float { position: [rect.left() - region.left(), rect.top() - region.top()] },
                        });
                    }
                    actions.push(action);
                }
                if index > 0 {
                    ui.painter().vline(
                        column.rect.left() - model::SPLIT_GAP * 0.5,
                        title.y_range(),
                        Stroke::new(metrics::BORDER, Tokens::get(ctx).border),
                    );
                }
            }
        }
        let clip = ui.clip_rect();
        ui.set_clip_rect(clip.intersect(Rect::from_min_max(pos2(rect.left(), title.bottom()), pos2(rect.right(), rect.bottom() - footer))));
        for g in &geo.groups {
            render_group(app, ui, host, rect, g, state.gesture.is_some(), false, hits, pending, actions, flyout);
        }
        ui.set_clip_rect(clip);
        for divider in &geo.dividers {
            let cursor = if divider.axis == model::Axis::Horizontal { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical };
            let response = ui.interact(divider.rect, id.with(("divider", divider.id)), Sense::drag()).on_hover_cursor(cursor);
            let t = Tokens::get(ctx);
            ui.painter().rect_filled(divider.rect, 0, t.panel_darker);
            let color = if response.hovered() || response.dragged() { t.accent } else { t.border };
            let points = if divider.axis == model::Axis::Horizontal {
                [pos2(divider.rect.center().x, divider.rect.top()), pos2(divider.rect.center().x, divider.rect.bottom())]
            } else {
                [pos2(divider.rect.left(), divider.rect.center().y), pos2(divider.rect.right(), divider.rect.center().y)]
            };
            ui.painter().line_segment(points, egui::Stroke::new(metrics::BORDER, color));
            hits.push((format!("divider:{}", divider.id), divider.rect));
            if response.drag_started() && state.gesture.is_none() {
                let compact = if divider.axis == model::Axis::Horizontal {
                    geo.columns.iter().find_map(|column| {
                        if column.mode == Mode::Full || column.rect.bottom() <= divider.rect.top() || column.rect.top() >= divider.rect.bottom() {
                            return None;
                        }
                        if (column.rect.right() - divider.rect.left()).abs() < 1.0 {
                            Some((column, false))
                        } else if (column.rect.left() - divider.rect.right()).abs() < 1.0 {
                            Some((column, true))
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };
                if let Some((column, left)) = compact {
                    let width = state.layout.column_width(column.key).unwrap_or(column.rect.width() + 2.0);
                    *pending = Some((GestureKind::ColumnWidth { host: host.id, group: column.key, left, width }, column.rect, false));
                } else {
                    *pending = Some((
                        GestureKind::Divider { split: divider.id, axis: divider.axis, min: divider.min_ratio, max: divider.max_ratio },
                        divider.parent,
                        false,
                    ));
                }
            }
        }
        if geo.groups.iter().any(|g| g.mode == Mode::Full) && !matches!(host.dock, Some(Edge::Left | Edge::Right)) && !control {
            for top in [false, true] {
                if matches!(host.dock, Some(Edge::Top)) && top || matches!(host.dock, Some(Edge::Bottom)) && !top {
                    continue;
                }
                let y = if top { rect.top() } else { rect.bottom() - 4.0 };
                let edge = Rect::from_min_size(pos2(rect.left() + 22.0, y), vec2((rect.width() - 44.0).max(1.0), 4.0));
                let response =
                    ui.interact(edge, id.with(if top { "top" } else { "bottom" }), Sense::drag()).on_hover_cursor(CursorIcon::ResizeVertical);
                hits.push((format!("resize-{}:{}", if top { "top" } else { "bottom" }, host.id), edge));
                if response.drag_started() && state.gesture.is_none() {
                    *pending = Some((GestureKind::Height { host: host.id, top }, rect, false));
                }
            }
        }
        for column in &geo.columns {
            let tools = geo.groups.iter().any(|g| {
                g.column == column.key
                    && model::groups(&host.root).iter().any(|source| source.id == g.id && source.panels.iter().any(|p| p == "tools"))
            });
            if control || (tools && geo.columns.len() == 1) {
                continue;
            }
            for left in [true, false] {
                let outer = if left { (column.rect.left() - rect.left()).abs() <= 2.0 } else { (rect.right() - column.rect.right()).abs() <= 2.0 };
                if !outer && column.mode == Mode::Full {
                    continue;
                }
                if outer
                    && (matches!(host.dock, Some(Edge::Left)) && left
                        || matches!(host.dock, Some(Edge::Right)) && !left
                        || matches!(host.dock, Some(Edge::Top | Edge::Bottom)))
                {
                    continue;
                }
                let x = if left { column.rect.left() - 1.0 } else { column.rect.right() - 3.0 };
                let edge = Rect::from_min_max(pos2(x, title.bottom() + 2.0), pos2(x + 4.0, column.rect.bottom()));
                let response =
                    ui.interact(edge, id.with(("column-resize", column.key, left)), Sense::drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);
                hits.push((format!("column-resize-{}:{}", if left { "left" } else { "right" }, column.key), edge));
                if outer {
                    hits.push((format!("resize-{}:{}", if left { "left" } else { "right" }, host.id), edge));
                }
                if response.drag_started() && state.gesture.is_none() {
                    let width = state.layout.column_width(column.key).unwrap_or(column.rect.width() + 2.0);
                    *pending = Some((GestureKind::ColumnWidth { host: host.id, group: column.key, left, width }, column.rect, false));
                }
            }
        }
        for left in [true, false] {
            if !control && standalone_kind(host) != Some(registry::PanelKind::Tools) {
                continue;
            }
            if matches!(host.dock, Some(Edge::Top | Edge::Bottom)) {
                continue;
            }
            if matches!(host.dock, Some(Edge::Left)) && left || matches!(host.dock, Some(Edge::Right)) && !left {
                continue;
            }
            let handle_width = if control { 2.0 } else { 5.0 };
            let x = if left { rect.left() } else { rect.right() - handle_width };
            let edge = Rect::from_min_max(pos2(x, title.bottom() + 2.0), pos2(x + handle_width, rect.bottom() - footer));
            let response =
                ui.interact(edge, id.with(if left { "left" } else { "right" }), Sense::drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);
            hits.push((format!("resize-{}:{}", if left { "left" } else { "right" }, host.id), edge));
            if response.drag_started() && state.gesture.is_none() {
                *pending = Some((GestureKind::Resize { host: host.id, left }, rect, false));
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn render_group(
    app: &mut VectorcraftApp,
    ui: &mut egui::Ui,
    host: &Host,
    host_rect: Rect,
    geo: &GroupRect,
    dragging: bool,
    flyout_view: bool,
    hits: &mut Vec<(String, Rect)>,
    pending: &mut Option<(GestureKind, Rect, bool)>,
    actions: &mut Vec<Action>,
    flyout: &mut Option<u64>,
) {
    let Some(group) = model::groups(&host.root).into_iter().find(|g| g.id == geo.id) else { return };
    let full = geo.mode == Mode::Full;
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(geo.rect, 0, t.panel);
    if full && geo.tabs.is_empty() {
        let grip = Rect::from_min_max(geo.rect.min, pos2(geo.rect.right(), geo.body.top()));
        if grip.is_positive() {
            ui.painter().rect_filled(grip, 0, t.panel_darker);
            let response =
                ui.interact(grip, Id::unique(("panel-review-bar-grip", group.id)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Default);
            hits.push((format!("group-grip:{}", group.id), grip));
            if response.drag_started() && !dragging {
                *pending = Some((GestureKind::Move(Source::Group(group.id)), geo.rect, false));
            }
        }
        body(app, ui, &group.active, geo.body);
        return;
    }
    let head = Rect::from_min_size(geo.rect.min, vec2(geo.rect.width(), if full { paint::TABS } else { paint::GRIP }));
    if full {
        ui.painter().rect_filled(head, 0, t.panel_darker);
    }
    let grip = if full {
        Rect::from_min_max(
            pos2(geo.tabs.last().map_or(head.left(), |(_, r)| r.right()), head.top()),
            pos2(head.right() - if geo.overflow.is_empty() { 24.0 } else { 48.0 } - if flyout_view { 24.0 } else { 0.0 }, head.bottom()),
        )
    } else {
        head
    };

    let response =
        ui.interact(grip, Id::unique("panel-review-group-grip").with((group.id, full)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Default);
    hits.push((format!("group-grip:{}", group.id), grip));
    if response.drag_started() && !dragging {
        *pending = Some((GestureKind::Move(Source::Group(group.id)), if model::groups(&host.root).len() == 1 { host_rect } else { geo.rect }, false));
    }
    for (panel, rect) in &geo.tabs {
        if !rect.intersects(ui.clip_rect()) {
            continue;
        }
        let response =
            ui.interact(*rect, Id::unique("panel-review-tab").with((panel, full)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Default);
        if full {
            paint::tab(ui, *rect, paint::label(panel), group.active == *panel, response.hovered());
        } else {
            paint::icon_row(ui, *rect, panel, geo.mode == Mode::Names, group.active == *panel, response.hovered());
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, group.active == *panel, paint::label(panel)));
        hits.push((format!("tab:{panel}"), *rect));
        if response.drag_started() && !dragging {
            *pending = Some((
                GestureKind::Move(Source::Panel(panel.clone())),
                if model::groups(&host.root).len() == 1 { host_rect } else { geo.rect },
                false,
            ));
        }
        if response.clicked() && !dragging {
            actions.push(Action::Activate { panel: panel.clone() });
            if !full {
                *flyout = if *flyout == Some(group.id) && group.active == *panel { None } else { Some(group.id) };
            }
        }
    }
    if full && !geo.overflow.is_empty() {
        let r = Rect::from_min_size(pos2(head.right() - if flyout_view { 72.0 } else { 48.0 }, head.top()), vec2(24.0, head.height()));
        ui.scope_builder(egui::UiBuilder::new().id_salt(("tabs-overflow", group.id)).max_rect(r), |ui| {
            ui.menu_button("…", |ui| {
                for panel in &geo.overflow {
                    if ui.selectable_label(&group.active == panel, paint::label(panel)).clicked() {
                        actions.push(Action::Activate { panel: panel.clone() });
                        ui.close();
                    }
                }
            });
        });
        hits.push((format!("tabs-overflow:{}", group.id), r));
    }
    if full {
        let r = Rect::from_min_size(pos2(head.right() - 24.0, head.top()), vec2(24.0, head.height()));
        let response = ui.interact(r, Id::unique(("panel-options", group.id)), Sense::click()).on_hover_cursor(CursorIcon::Default);
        paint::control(ui, r, "menu", response.hovered());
        hits.push((format!("panel-menu:{}", group.id), r));
        egui::Popup::menu(&response).show(|ui| {
            if crate::panels::panel_menu_items(app, ui, super::shell::persist::instance_kind(&group.active)) {
                ui.separator();
            }
            if group.active == "tools" {
                let narrow = geo.rect.width() + model::HOST_CHROME[0] <= metrics::TOOLS_MIN_WIDTH + 0.01;
                if ui.button(if narrow { "Two tool columns" } else { "One tool column" }).clicked() {
                    actions.push(Action::SetToolsWidth { width: if narrow { metrics::TOOLS_MAX_WIDTH } else { metrics::TOOLS_MIN_WIDTH } });
                    ui.close();
                }
            } else if ui.button("Collapse to icons").clicked() {
                actions.push(Action::SetColumnMode { group: group.id, mode: Mode::Icons });
                ui.close();
            }
            if ui.button("Hide panel block").clicked() {
                actions.push(Action::Hide { host: host.id });
                ui.close();
            }
        });
    }
    if full && geo.body.is_positive() {
        body(app, ui, &group.active, geo.body);
    }
    ui.painter().hline(geo.rect.x_range(), geo.rect.bottom() - 0.5, Stroke::new(1.0, t.divider));
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, panel: &str, rect: Rect) {
    ui.scope_builder(
        egui::UiBuilder::new().scope_id(Id::unique(("panel-review-content", panel))).max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(rect.intersect(ui.clip_rect()));
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            if content::show(app, ui, panel, rect) {
                return;
            }
            if matches!(panel, "layers" | "properties" | "libraries") || panel.starts_with("properties@") {
                super::shell::boxes::panel_body(app, ui, panel, rect.width(), (rect.height() - 18.0).max(0.0));
            } else {
                crate::widgets::panel_scroll(
                    ui,
                    egui::ScrollArea::vertical().id_salt(("panel-review-scroll", panel)).max_height(rect.height()).auto_shrink([false, false]),
                    |ui| super::shell::boxes::panel_body(app, ui, panel, rect.width() - 4.0, (rect.height() - 18.0).max(0.0)),
                );
            }
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn render_flyout(
    app: &mut VectorcraftApp,
    ctx: &egui::Context,
    layout: &Layout,
    measured: &[HostRect],
    group: u64,
    region: Rect,
    hits: &mut Vec<(String, Rect)>,
    pending: &mut Option<(GestureKind, Rect, bool)>,
    actions: &mut Vec<Action>,
    flyout: &mut Option<u64>,
) {
    let Some(host) = layout
        .hosts
        .iter()
        .find(|h| !h.hidden && layout.column_mode(group) != Some(Mode::Full) && model::groups(&h.root).iter().any(|g| g.id == group))
    else {
        return;
    };
    let Some(anchor) = measured.iter().find(|h| h.id == host.id).and_then(|h| h.groups.iter().find(|g| g.id == group)) else { return };
    let Some(g) = layout.group(group) else { return };
    let remembered = layout.expanded_group_size(group).unwrap_or([300.0, 380.0]);
    let width = remembered[0].max(240.0).min(region.width());
    // The side Color panel fits its slider/spectrum contents, independently of
    // both the toolbar's wheel preferences and an older remembered wheel size.
    let preferred_height = if g.active == "color" {
        crate::panels::color::content_height(ctx).unwrap_or(300.0) + paint::TITLE + paint::TABS + paint::FOOT
    } else {
        remembered[1].max(180.0)
    };
    let height = preferred_height.min(region.height());
    let prefer_left = host.dock == Some(Edge::Right);
    let x = if !prefer_left && anchor.rect.right() + width + 4.0 <= region.right() {
        anchor.rect.right() + 4.0
    } else {
        (anchor.rect.left() - width - 4.0).max(region.left())
    };
    let y = anchor.rect.top().min(region.bottom() - height).max(region.top());
    let mut expanded_group = g.clone();
    expanded_group.presentation = None;
    let temporary = Host {
        root: model::Node::Group(expanded_group),
        dock: None,
        position: [x - region.left(), y - region.top()],
        expanded: [width, height],
        mode: Mode::Full,
        ..host.clone()
    };
    let mut geo = geometry::measure_host(&temporary, region);
    for g in &mut geo.groups {
        let tab_end = g.rect.right() - 72.0;
        let mut overflow = g.overflow.clone();
        g.tabs.retain_mut(|(panel, rect)| {
            if rect.right() > tab_end {
                if !overflow.contains(panel) {
                    overflow.push(panel.clone());
                }
                if rect.left() >= tab_end {
                    return false;
                }
                rect.max.x = tab_end;
            }
            true
        });
        g.overflow = overflow;
    }
    let drag_rect = geo.rect;
    geo.rect.min.y += paint::TITLE;
    egui::Area::new(Id::unique("panel-review-flyout"))
        .order(Order::Foreground)
        .fixed_pos(geo.rect.min)
        .constrain(false)
        .fade_in(false)
        .movable(false)
        .show(ctx, |ui| {
            ui.set_min_size(geo.rect.size());
            ui.set_clip_rect(region.intersect(geo.rect));
            ui.painter().rect_filled(geo.rect, 3, Tokens::get(ctx).panel);
            for g in &geo.groups {
                let mut request = None;
                render_group(app, ui, &temporary, drag_rect, g, false, true, hits, &mut request, actions, flyout);
                if let Some((kind, rect, _)) = request {
                    *pending = Some((kind, rect, true));
                }
            }
            let close = Rect::from_min_size(pos2(geo.rect.right() - 48.0, geo.rect.top()), vec2(24.0, paint::TABS));
            let response = ui.interact(close, Id::unique(("panel-flyout-close", group)), Sense::click());
            paint::control(ui, close, "shell-chevrons-left", response.hovered());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Close panel flyout"));
            hits.push((format!("flyout-close:{group}"), close));
            if response.clicked() {
                *flyout = None;
            }
            ui.painter().rect_stroke(geo.rect, 3, Stroke::new(1.0, Tokens::get(ctx).border), StrokeKind::Inside);
        });
}

#[cfg(test)]
mod ui_tests;

#[cfg(test)]
mod workspace_ui_tests;

#[cfg(test)]
mod column_ui_tests;

#[cfg(test)]
mod floating_ui_tests;

#[cfg(test)]
mod drag_paint_tests;

#[cfg(test)]
mod color_flyout_tests;

#[cfg(test)]
mod review_fix_tests;
