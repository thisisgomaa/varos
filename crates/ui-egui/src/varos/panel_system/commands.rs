//! Layout commands. Every entry point decodes once into a typed [`Intent`]; every intent that
//! changes the layout commits through [`transact`]: its actions apply to a copy, the copy replaces
//! the layout only when every step succeeded, and transient state is invalidated in one place.
use super::{
    State, metrics,
    model::{self, Action, Edge, Layout, Mode, Source, Target},
    presenter, registry, workspace,
};
use crate::VectorcraftApp;
use serde_json::{Value, json};

/// Tab hides every visible block; Shift+Tab hides every panel except Tools and Control, one
/// panel at a time (a panel attached beside Tools hides while Tools stays). `after` is the layout
/// the hide produced: when nothing changed since, the next toggle restores `before` exactly.
#[derive(Clone, Debug)]
pub(super) struct Hidden {
    all: bool,
    before: Layout,
    after: Layout,
    hosts: Vec<u64>,
    panels: Vec<String>,
}

/// What a layout command asks for, decoded from its id and params.
enum Intent {
    /// `ui.panelReview`: turn the workspace on (with the Varos shell) or forget it.
    Enable(bool),
    Inspect,
    /// `ui.panelReview.action` and `window.panel.layout`: one atomic list of layout actions.
    Apply(Vec<Action>),
    /// `window.panel.layout` without an action: report the layout.
    Report,
    Reset,
    /// Tab (`all`) or Shift+Tab.
    ToggleBlocks {
        all: bool,
    },
    /// Show or hide the Tools (`tools`) or Control (`control`) bar.
    ToggleBar(&'static str),
    ToolsColumns {
        double: Option<bool>,
    },
    /// `window.panel*` for one panel; `raw` is the name as given, for messages.
    Panel {
        panel: String,
        raw: String,
        op: PanelOp,
    },
}

/// What happens to the block a panel command names, before the layout resolves it.
enum PanelOp {
    /// Show it; with `toggle` (the Window menu), a panel that shows closes instead.
    Show {
        toggle: bool,
    },
    Activate,
    Close,
    /// Collapse (`Some(true)`), expand (`Some(false)`) or toggle its column.
    Collapse(Option<bool>),
    Float {
        x: Option<f32>,
        y: Option<f32>,
        scope: Scope,
    },
    /// Move beside or into `anchor`'s group, or (`dock`, no anchor) to a workspace edge.
    Move {
        edge: Option<Edge>,
        anchor: Option<String>,
        before: Option<String>,
        dock: bool,
        scope: Scope,
    },
}

/// How much of the panel's block moves.
#[derive(Clone, Copy)]
enum Scope {
    Panel,
    Group,
    Column,
}

pub fn command(app: &mut VectorcraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if id != "ui.panelReview" && !super::enabled(app) {
        return None;
    }
    if id != "ui.panelReview"
        && let Some(result) = workspace::command(app, id, params)
    {
        return Some(result);
    }
    Some(decode(id, params)?.and_then(|intent| execute(app, intent)))
}

fn flag(params: &Value, key: &str) -> Option<bool> {
    params.get(key).and_then(Value::as_bool)
}

/// `None`: not a layout command. Malformed params are an error, never a partial command.
fn decode(id: &str, params: &Value) -> Option<Result<Intent, String>> {
    let actions = || -> Result<Vec<Action>, String> {
        if let Some(list) = params.get("actions") {
            return serde_json::from_value(list.clone()).map_err(|e| e.to_string());
        }
        let action = params.get("action").cloned().ok_or("Expected action")?;
        Ok(vec![serde_json::from_value(action).map_err(|e| e.to_string())?])
    };
    Some(match id {
        "ui.panelReview" => Ok(Intent::Enable(flag(params, "enabled") != Some(false))),
        "ui.panelReview.inspect" => Ok(Intent::Inspect),
        "ui.panelReview.action" => actions().map(Intent::Apply),
        "window.panel.layout" if params.get("action").is_none() && params.get("actions").is_none() => Ok(Intent::Report),
        "window.panel.layout" => actions().map(Intent::Apply),
        "ui.dock.reset" => Ok(Intent::Reset),
        "window.panels" | "window.dock" => Ok(Intent::ToggleBlocks { all: id == "window.dock" }),
        "window.toolbar" => Ok(Intent::ToggleBar("tools")),
        "window.control" => Ok(Intent::ToggleBar("control")),
        "window.toolbarColumns" => Ok(Intent::ToolsColumns { double: flag(params, "double") }),
        _ if id.starts_with("window.panel") => decode_panel(id, params),
        _ => return None,
    })
}

fn decode_panel(id: &str, params: &Value) -> Result<Intent, String> {
    let raw = params.get("panel").and_then(Value::as_str).unwrap_or_default();
    let panel = if registry::contains(raw) { raw } else { crate::menus::normalize_panel(raw).unwrap_or_default() };
    let scope = if flag(params, "column") == Some(true) {
        Scope::Column
    } else if flag(params, "group") == Some(true) {
        Scope::Group
    } else {
        Scope::Panel
    };
    let op = match id {
        "window.panel" => PanelOp::Show { toggle: flag(params, "toggle") == Some(true) },
        "window.panel.activate" => PanelOp::Activate,
        "window.panel.close" => PanelOp::Close,
        "window.panel.collapse" | "window.panel.minimize" => PanelOp::Collapse(flag(params, "collapsed").or_else(|| flag(params, "minimized"))),
        "window.panel.float" => PanelOp::Float {
            x: params.get("x").and_then(Value::as_f64).map(|v| v as f32),
            y: params.get("y").and_then(Value::as_f64).map(|v| v as f32),
            scope,
        },
        "window.panel.move" | "window.panel.dock" => {
            let edge = match params.get("zone").and_then(Value::as_str).unwrap_or("tab") {
                "left" => Some(Edge::Left),
                "right" => Some(Edge::Right),
                "top" | "above" => Some(Edge::Top),
                "bottom" | "below" => Some(Edge::Bottom),
                "tab" | "center" => None,
                _ => return Err("Unknown panel drop zone".into()),
            };
            let anchor = params.get("anchor").or_else(|| params.get("onto")).and_then(Value::as_str).map(str::to_string);
            let before = params.get("before").and_then(Value::as_str).map(str::to_string);
            PanelOp::Move { edge, anchor, before, dock: id == "window.panel.dock", scope }
        }
        _ => return Err("Unknown panel layout command".into()),
    };
    Ok(Intent::Panel { panel: panel.to_string(), raw: raw.to_string(), op })
}

fn execute(app: &mut VectorcraftApp, intent: Intent) -> Result<Value, String> {
    match intent {
        Intent::Enable(false) => {
            app.ui.varos_panel_review = None;
            return Ok(json!({"enabled":false}));
        }
        Intent::Enable(true) => {
            app.ui.varos_shell.enabled = true;
            app.ui.varos_panel_review.get_or_insert_with(State::default);
            return Ok(json!({"enabled":true,"panels":registry::BUILTINS.iter().map(|p|p.id).collect::<Vec<_>>()}));
        }
        Intent::ToolsColumns { double } => {
            let state = presenter::state_mut(&mut app.ui).ok_or("The panel workspace is not active")?;
            let actions = transact(state, |layout| Ok(vec![tools_width_toggle(layout, double.map(|double| !double))]))?;
            let double = matches!(actions.first(), Some(Action::SetToolsWidth { width }) if *width > metrics::TOOLS_MIN_WIDTH);
            app.ui.toolbar_double = double;
            app.ui.varos_shell.tools_columns = if double { 2 } else { 1 };
            return Ok(json!({"double":double}));
        }
        _ => {}
    }
    let state = presenter::state_mut(&mut app.ui).ok_or("The panel workspace is not active")?;
    match intent {
        Intent::Enable(_) | Intent::ToolsColumns { .. } => Err("Unknown panel layout command".into()),
        Intent::Inspect => Ok(
            json!({"layout":state.layout,"dragging":state.gesture.is_some(),"hits":state.hits.iter().map(|(name,r)|json!({"name":name,"rect":[r.left(),r.top(),r.width(),r.height()]})).collect::<Vec<_>>()}),
        ),
        Intent::Apply(actions) => {
            transact(state, |_| Ok(actions))?;
            Ok(json!({"layout":state.layout}))
        }
        Intent::Report => Ok(json!({"layout":state.layout})),
        Intent::Reset => {
            state.layout = Layout::new_workspace();
            state.cancel_transient();
            Ok(json!({"layout":state.layout}))
        }
        Intent::ToggleBlocks { all } => toggle_blocks(state, all).map(|shown| json!({"shown":shown})),
        Intent::ToggleBar(panel) => {
            transact(state, |layout| {
                layout.ensure_panel(panel)?;
                let shown = layout.owner(panel).filter(|h| !h.hidden).map(|h| h.id);
                Ok(vec![shown.map_or_else(|| Action::Show { panel: panel.into() }, |host| Action::Hide { host })])
            })?;
            Ok(json!({"layout":state.layout}))
        }
        Intent::Panel { panel, raw, op } => panel_command(state, &panel, &raw, op),
    }
}

/// Apply the actions `prepare` returns to a copy of the layout (after any registration it does
/// there); the copy replaces the layout only when every step succeeds. Then invalidate the
/// transient state those actions made stale: any structural change closes the compact flyout.
pub(super) fn transact(state: &mut State, prepare: impl FnOnce(&mut Layout) -> Result<Vec<Action>, String>) -> Result<Vec<Action>, String> {
    let mut next = state.layout.clone();
    let actions = prepare(&mut next)?;
    for action in &actions {
        next.apply(action)?;
    }
    state.layout = next;
    if actions.iter().any(|action| !matches!(action, Action::Activate { .. } | Action::Raise { .. } | Action::FitControlContent { .. })) {
        state.flyout = None;
    }
    Ok(actions)
}

/// Tools never become an icon column: collapsing narrows them to one column of tools.
/// `narrow: None` switches to the other width.
pub(super) fn tools_width_toggle(layout: &Layout, narrow: Option<bool>) -> Action {
    let narrow = narrow.unwrap_or_else(|| layout.tools_width().is_some_and(|width| width > metrics::TOOLS_MIN_WIDTH + 0.01));
    Action::SetToolsWidth { width: if narrow { metrics::TOOLS_MIN_WIDTH } else { metrics::TOOLS_MAX_WIDTH } }
}

/// Collapse (`Some(true)`), expand (`Some(false)`) or toggle the column holding `group`; a Tools
/// column switches its width instead. The column header and `window.panel.collapse` share this.
pub(super) fn column_toggle(layout: &Layout, group: u64, collapse: Option<bool>) -> Action {
    if layout.column_has_tools(group) {
        return tools_width_toggle(layout, collapse);
    }
    let collapse = collapse.unwrap_or(layout.column_mode(group) == Some(Mode::Full));
    Action::SetColumnMode { group, mode: if collapse { layout.collapsed_mode(group) } else { Mode::Full } }
}

fn panel_command(state: &mut State, panel: &str, raw: &str, op: PanelOp) -> Result<Value, String> {
    if matches!(op, PanelOp::Close) || matches!(op, PanelOp::Show { toggle: true }) && super::panel_showing(state, panel) {
        if !registry::contains(panel) {
            return Err(format!("unknown panel `{raw}`"));
        }
        // A compact column's open flyout closes first; its icon stays in the column.
        if let Some(group) = state.layout.group_of(panel).map(|g| g.id)
            && state.flyout == Some(group)
            && state.layout.column_mode(group).is_some_and(|mode| mode != Mode::Full)
        {
            state.flyout = None;
            return Ok(json!({"layout":state.layout,"open":null}));
        }
        let forget = crate::panels::library_panel::parse(panel).is_some();
        let mut next = state.layout.clone();
        close_panel(&mut next, panel, forget)?;
        state.layout = next;
        state.flyout = None;
        return Ok(json!({"layout":state.layout,"open":null}));
    }
    let show = matches!(op, PanelOp::Show { .. } | PanelOp::Activate);
    transact(state, |next| {
        next.ensure_panel(panel)?;
        let host = next.owner(panel).ok_or("Panel owner missing")?;
        let group = next.group_of(panel).ok_or("Panel group missing")?;
        let source = |scope| match scope {
            Scope::Column => Source::Host(host.id),
            Scope::Group => Source::Group(group.id),
            Scope::Panel => Source::Panel(panel.into()),
        };
        let action = match op {
            PanelOp::Show { .. } | PanelOp::Activate => Action::Show { panel: panel.into() },
            PanelOp::Close => return Err("Unknown panel layout command".into()),
            PanelOp::Collapse(collapse) => column_toggle(next, group.id, collapse),
            PanelOp::Float { x, y, scope } => Action::Move {
                source: source(scope),
                target: Target::Float { position: [x.unwrap_or(host.position[0]), y.unwrap_or(host.position[1])] },
            },
            PanelOp::Move { edge, anchor, before, dock, scope } => {
                let target = if let Some(anchor) = anchor {
                    let g = next.group_of(&anchor).ok_or("Unknown destination panel")?;
                    if let Some(edge) = edge {
                        Target::Split { group: g.id, edge }
                    } else {
                        let index = before.and_then(|p| g.panels.iter().position(|v| *v == p)).unwrap_or(g.panels.len());
                        Target::Merge { group: g.id, index }
                    }
                } else if dock {
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
                Action::Move { source: source(scope), target }
            }
        };
        Ok(vec![action])
    })?;
    // Showing a panel of a compact column opens its flyout.
    if show && let Some(group) = state.layout.group_of(panel).filter(|g| state.layout.column_mode(g.id) != Some(Mode::Full)) {
        state.flyout = Some(group.id);
    }
    Ok(json!({"layout":state.layout}))
}

fn toggle_blocks(state: &mut State, all: bool) -> Result<bool, String> {
    state.flyout = None;
    Ok(match state.hidden.as_ref().map(|record| record.all) {
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
                state.layout.ensure_panel(panel).and_then(|()| state.layout.apply(&Action::Show { panel: panel.into() }))?;
            }
            true
        }
        None => true,
    })
}

/// Close one panel without hiding the panels sharing its block: a panel alone hides its block
/// (Show brings it back where it was); a tab or attached group detaches into its own hidden block.
/// `forget` removes a dynamic (library) panel from the layout instead of hiding it.
pub(super) fn close_panel(layout: &mut Layout, panel: &str, forget: bool) -> Result<(), String> {
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

/// Commit one gesture's or one frame's clicked actions as a single transaction, through the same
/// command path as agents (`ui.panelReview.action`). A failure reaches the status line and the
/// layout stays as it was; `false` tells the caller to stop.
pub(super) fn commit(app: &mut VectorcraftApp, actions: Vec<Action>) -> bool {
    if actions.is_empty() {
        return true;
    }
    match app.run("ui.panelReview.action", json!({"actions": actions})) {
        Ok(_) => true,
        Err(error) => {
            app.status(error);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("ui.panelReview", json!({"enabled": true})).unwrap();
        app
    }

    fn state(app: &VectorcraftApp) -> &State {
        app.ui.varos_panel_review.as_ref().unwrap()
    }

    #[test]
    fn both_layout_entry_points_share_one_transaction_and_one_flyout_rule() {
        for id in ["ui.panelReview.action", "window.panel.layout"] {
            let mut app = app();
            let layers = state(&app).layout.owner("layers").unwrap().id;
            let group = state(&app).layout.group_of("layers").unwrap().id;
            app.ui.varos_panel_review.as_mut().unwrap().flyout = Some(group);
            app.run(id, json!({"action": Action::Activate { panel: "layers".into() }})).unwrap();
            assert_eq!(state(&app).flyout, Some(group), "{id}: activating keeps the flyout");
            app.run(id, json!({"action": Action::Hide { host: layers }})).unwrap();
            assert_eq!(state(&app).flyout, None, "{id}: a structural change closes it");
            // One failing step leaves the whole list unapplied.
            let before = state(&app).layout.clone();
            let error = app.run(id, json!({"actions": [Action::Show { panel: "layers".into() }, Action::Hide { host: u64::MAX }]}));
            assert!(error.is_err(), "{id}");
            assert_eq!(state(&app).layout, before, "{id}: nothing applied");
        }
    }

    #[test]
    fn collapsing_by_command_and_by_header_reads_the_same_column_width() {
        let mut app = app();
        let layout = &mut app.ui.varos_panel_review.as_mut().unwrap().layout;
        let tools = layout.owner("tools").unwrap().id;
        let layers = layout.owner("layers").unwrap().id;
        let group = layout.group_of("layers").unwrap().id;
        layout.apply(&Action::Move { source: Source::Host(layers), target: Target::Attach { host: tools, edge: Edge::Right } }).unwrap();
        layout.apply(&Action::SetColumnMode { group, mode: Mode::Icons }).unwrap();
        layout.apply(&Action::ResizeColumn { group, width: 100.0 }).unwrap();
        layout.apply(&Action::SetColumnMode { group, mode: Mode::Full }).unwrap();
        assert!(layout.host(tools).unwrap().compact_width < metrics::NAMES_MIN_WIDTH, "the host keeps a narrow width of its own");
        assert_eq!(column_toggle(layout, group, None), Action::SetColumnMode { group, mode: Mode::Names });
        app.run("window.panel.collapse", json!({"panel": "layers"})).unwrap();
        assert_eq!(state(&app).layout.column_mode(group), Some(Mode::Names), "the command collapses to the column's own width");
    }
}
