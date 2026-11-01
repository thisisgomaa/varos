//! The new Varos shell, stages 1–4 (`docs/varos/NEW_SHELL_PLAN.md`): Preferences › Varos shell (new)
//! draws the window's central region as our own split tree of boxes instead of craft-ui's dock.
//!
//! A row of workspace tabs runs along the top ([`workspace`]); a box can be maximized to fill the
//! region (a double-click on its tab row's empty space, or its ≡ menu) and restored the same way;
//! the Properties panel is an editor of sections, and may have more than one instance
//! (`properties@2`, …), each following the selection with its own section view.
//!
//! The Board (the canvas, with its document tabs) is one box; every other box holds panels as
//! tabs ([`boxes`]); 12-point seams between them are the dividers ([`divider`]). The layout is
//! [`tree::Node`], saved as `varos_shell` in `ui.json` ([`persist`]). Tabs, icons, strip groups and
//! floating strips are dragged ([`drag`]); panels also float ([`floats`]). Off, the window is exactly
//! upstream's dock and cards; both layouts persist on their own, and the switch takes effect on
//! the next frame.
//!
//! Everything the shell needs from the editor is the stable host API: `canvas_area` (the canvas),
//! `dock::panel_body` (a panel's body) and `panels::panel_menu_items` (its ≡ menu).

pub(crate) mod boxes;
pub(crate) mod divider;
pub(crate) mod drag;
pub(crate) mod floats;
pub(crate) mod motion;
pub(crate) mod persist;
mod resize;
pub(crate) mod strip;
pub(crate) mod tokens;
pub(crate) mod tools;
pub(crate) mod tree;
pub(crate) mod workspace;

use egui::Rect;
use serde_json::{Value, json};

use crate::VectorcraftApp;

/// The Preferences dialog's field for Varos shell (new) (User Interface, under Panel cards
/// (Varos)), and its label.
pub(crate) const PREF_KEY: &str = "__varosShell";
pub(crate) const PREF_LABEL: &str = "Varos shell (new)";

/// The void around the boxes, inside the central region.
pub(crate) const OUTER: f32 = 0.0;

/// Is the window's central region the Varos shell.
pub(crate) fn enabled(app: &VectorcraftApp) -> bool {
    app.ui.varos_shell.enabled
}

/// Preferences › Varos shell (new) set to `on`; the next frame draws the chosen shell.
pub(crate) fn set_enabled(app: &mut VectorcraftApp, on: bool) {
    if app.ui.varos_shell.enabled != on {
        strip::close(app);
        app.ui.varos_shell.drag = None;
    }
    app.ui.varos_shell.enabled = on;
    // An explicit choice, on or off, so a later load does not treat `false` as the old default.
    app.ui.varos_shell.enabled_chosen = true;
}

/// A control of the shell as drawn last frame: what drags aim at, and what tests and agents read.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // Varos: some fields are read by the tests only.
pub(crate) enum Hit {
    /// The Board (the canvas region, its document tabs included).
    Board,
    Tools,
    ToolsGrip,
    Strip,
    StripToggle,
    StripIcon(String),
    StripGroup(usize),
    Flyout,
    FlyoutTab(String),
    FlyoutClose,
    FlyoutResize,
    /// The canvas inside the Board (inside the rulers and scroll bars): where canvas gestures go.
    BoardBody,
    /// An ordinary box, whole.
    Box(u32),
    /// A box's tab row (where its pills sit).
    TabRow(u32),
    /// A panel's tab in a box.
    Tab {
        box_id: u32,
        panel: String,
    },
    /// A box's ≡ button.
    Menu(u32),
    BoxToIcon(u32),
    /// The divider of the split at this path.
    Divider(tree::Path),
    /// A floating panel, whole (its tab row, tabs and ≡ are [`Hit::TabRow`], [`Hit::Tab`] and
    /// [`Hit::Menu`] of its box id).
    Float(u32),
    /// A floating panel's title strip (it moves the panel), its `×`, its `<<`, its lower-right
    /// resize corner and its right resize edge.
    FloatTitle(u32),
    FloatClose(u32),
    FloatToIcon(u32),
    FloatResize(u32),
    FloatResizeRight(u32),
    /// A floating strip, whole; its title strip (it drags the strip), its `×` and its `>>`.
    FloatStrip(u32),
    FloatStripTitle(u32),
    FloatStripClose(u32),
    FloatStripExpand(u32),
    /// A floating strip's group (its grip dots) and icon.
    FloatStripGroup {
        strip: u32,
        index: usize,
    },
    FloatStripIcon {
        strip: u32,
        panel: String,
    },
    /// During a drag: the azure line previewing where it lands.
    DropLine {
        vertical: bool,
        target: drag::Target,
    },
    /// During a drag: a landing with no line (a new floating panel, a lone icon, a moved strip),
    /// at the ghost.
    DropAt(drag::Target),
    /// During a drag: the ghost under the pointer.
    Ghost,
    /// During a drag: the dragged thing's place, outlined (a flyout's tab: its group in the strip).
    DragSource,
    /// The workspace row, a workspace's tab (as much of it as shows), `‹` (false) or `›` (true)
    /// where tabs are out of sight, and `+`.
    Workspaces,
    WorkspaceTab(String),
    WorkspaceOverflow(bool),
    WorkspaceNew,
    /// The Board's ≡ (Maximize Board / Restore Layout), at the bottom of its strip, and the empty
    /// space of its document tabs' row (a double-click maximizes or restores).
    BoardMenu,
    BoardHeader,
    /// The azure line shown while a divider is dragged past a column's minimum (release collapses
    /// that column into the strip).
    AzureHint,
    /// The canvas-side edge of the Board's strip, while a column is collapsed: dragging it outward
    /// past ~120 pt opens the column again.
    StripEdge,
    /// The canvas-side edge of the tool strip: dragging it snaps between 1, 2 and 3 columns.
    ToolsEdge,
}

fn hits_id() -> egui::Id {
    egui::Id::unique("varos-shell-hits")
}

/// The shell's controls as drawn by the last pass (or by this pass, once it drew them), with their
/// rects in window points; none when the last pass didn't draw the shell (it is off).
#[allow(dead_code)] // Varos: read by the tests now and by stage 3's tab drags next.
pub(crate) fn hit_rects(ctx: &egui::Context) -> Vec<(Hit, Rect)> {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<Hits>(hits_id())).filter(|h| h.pass + 1 >= now).map(|h| (*h.hits).clone()).unwrap_or_default()
}

/// The layout revision ([`drag::layout_rev`]) the last pass's hit rects were drawn for, while a
/// drag is under way (none otherwise, or when they are stale).
pub(crate) fn hits_rev(ctx: &egui::Context) -> Option<u64> {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<Hits>(hits_id())).filter(|h| h.pass + 1 >= now).and_then(|h| h.rev)
}

/// The hit rects, the pass (`cumulative_pass_nr` while it drew) they belong to and, during a
/// drag, the layout they were drawn for.
#[derive(Clone, Default)]
struct Hits {
    pass: u64,
    hits: std::sync::Arc<Vec<(Hit, Rect)>>,
    rev: Option<u64>,
}

/// Draw the central region: the boxes and their dividers, or the Board alone on Home and in the
/// full screen modes.
pub(crate) fn frame(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.panel)).show(ui, |ui| crate::varos::profile::measure("shell", || draw(app, ui)));
}

fn draw(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    ui.spacing_mut().scroll = egui::style::ScrollStyle { bar_width: 4.0, floating_width: 2.0, ..egui::style::ScrollStyle::floating() };
    let frame = ui.ctx().cumulative_frame_nr();
    let follow = ui.ctx().data_mut(|d| {
        let id = egui::Id::unique("shell-follow-frame");
        let fresh = d.get_temp::<u64>(id) != Some(frame);
        d.insert_temp(id, frame);
        fresh
    });
    if follow {
        follow_open_panel(app);
    }
    let area = ui.max_rect();
    let mut hits = Vec::new();
    let alone = crate::varos::home::is_home(app) || app.ui.screen_mode >= 1;
    if alone && app.ui.varos_shell.drag.is_some() {
        drag::cancel(app, ui.ctx());
    }
    // Boxes, floats and drags live below the workspace row (the whole region when alone).
    let region = if alone { area } else { drag_region(area) };
    // A released drag lands before anything is laid out: this frame draws the result.
    drag::before(app, ui.ctx(), region);
    if alone {
        strip::close(app);
        hits.push((Hit::Board, area));
        let canvas = if crate::varos::home::is_home(app) { area } else { tools::show(app, ui, area, region, &mut hits) };
        board(app, ui, canvas, false, &mut hits);
        tools::finish_edge(app, ui, &mut hits);
    } else {
        // The workspace row on top; the boxes, floats and drags below it.

        let inner = Rect::from_min_max(egui::pos2(region.left() + OUTER, region.top()), region.max - egui::vec2(OUTER, OUTER));
        let inner = strip::show(app, ui, inner, &mut hits);
        let inner = tools::show(app, ui, inner, region, &mut hits);
        let lay_out = |shell: &persist::VarosShell| match shell.maximized_box() {
            Some(id) => tree::Layout { boxes: vec![(id, inner)], dividers: Vec::new() },
            None => shell.tree.layout(inner),
        };
        let mut layout = lay_out(&app.ui.varos_shell);
        if divider::interact(app, ui, &layout) {
            // The new ratio is drawn this very frame.
            layout = lay_out(&app.ui.varos_shell);
        }
        hits.extend(layout.dividers.iter().map(|d| (Hit::Divider(d.path.clone()), d.rect)));
        let mut asks = Vec::new();
        for (id, rect) in &layout.boxes {
            let Some(b) = app.ui.varos_shell.tree.find_box(*id).cloned() else { continue };
            if b.is_board() {
                hits.push((Hit::Board, *rect));
                board(app, ui, *rect, false, &mut hits);
            } else if let Some(ask) = boxes::show(app, ui, &b, motion::settle(ui.ctx(), &format!("box-{}", b.id), *rect, region), false, &mut hits) {
                asks.push(ask);
            }
        }
        divider::paint(ui, &layout, &mut hits);
        for ask in asks {
            match ask {
                boxes::Ask::Close(panel) => {
                    app.ui.varos_shell.close_tab(&panel);
                }
                boxes::Ask::Collapse(id) => {
                    if !app.ui.varos_shell.toggle_collapsed(id) {
                        app.status("Collapse: no box above or below takes its room");
                    }
                }
                boxes::Ask::Maximize(id) => {
                    strip::close(app);
                    app.ui.varos_shell.toggle_maximized(id);
                }
                boxes::Ask::Duplicate(panel) => {
                    duplicate_properties(app, &panel);
                }
                boxes::Ask::ToIcon(id) => {
                    app.ui.varos_shell.collapse_box_to_strip(id);
                }
            }
            ui.ctx().request_repaint();
        }
        floats::show(app, ui, region, &mut hits);
        // A flyout belongs to a strip's panel (one that left its strip takes its flyout along).
        if let Some(open) = app.ui.varos_shell.open.clone()
            && !app.ui.varos_shell.in_a_strip(&open)
        {
            strip::close(app);
        }
        drag::after(app, ui, region, &mut hits);
        strip::finish_edge(app, ui, &mut hits);
        tools::finish_edge(app, ui, &mut hits);
    }
    if !crate::varos::home::is_home(app) && app.ui.toolbar && app.ui.screen_mode < 3 {
        crate::toolbar::shell_popovers(app, ui.ctx(), region);
    }
    let pass = ui.ctx().cumulative_pass_nr();
    // Only a drag needs to know the layout its targets were found in (worth a hash per frame).
    let rev = app.ui.varos_shell.drag.as_ref().map(|_| drag::layout_rev(&app.ui.varos_shell, region));
    ui.ctx().data_mut(|d| d.insert_temp(hits_id(), Hits { pass, hits: std::sync::Arc::new(hits), rev }));
    // A panel asked for by code that sets `open_panel` while this frame drew shows on the next.
    if app.ui.open_panel != app.ui.varos_shell.open {
        ui.ctx().request_repaint();
    }
}

/// A rect remembered for this frame only. egui's temp map can drop a `Default` value, and
/// [`egui::Rect`] is not one, so the frame number says which pass stored it.
#[derive(Clone, Copy, Default)]
struct StampedRect {
    frame: u64,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

pub(crate) fn stamp_rect(ctx: &egui::Context, id: egui::Id, rect: egui::Rect) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| {
        d.insert_temp(id, StampedRect { frame, min_x: rect.min.x, min_y: rect.min.y, max_x: rect.max.x, max_y: rect.max.y });
    });
}

pub(crate) fn stamped_rect(ctx: &egui::Context, id: egui::Id) -> Option<egui::Rect> {
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<StampedRect>(id))
        .filter(|s| s.frame == frame)
        .map(|s| egui::Rect::from_min_max(egui::pos2(s.min_x, s.min_y), egui::pos2(s.max_x, s.max_y)))
}

/// Where boxes, floats and drags live in the central region `area`: below the workspace row.
pub(crate) fn drag_region(area: Rect) -> Rect {
    area
}

/// Properties editor `panel`'s ≡ › Duplicate: another Properties editor (`properties@<n>`) with the
/// same section view, in a box of its own below (a floating panel's: a tab beside it). Both edit
/// the same document.
pub(crate) fn duplicate_properties(app: &mut VectorcraftApp, panel: &str) -> Option<String> {
    let shell = &mut app.ui.varos_shell;
    if !persist::is_properties(panel) && persist::instance_kind(panel) != "control" {
        return None;
    }
    let id = shell.fresh_instance_id(persist::instance_kind(panel));
    let mut view = shell.views.get(panel).cloned().unwrap_or_default();
    view.pinned = None;
    let placed = if let Some(b) = shell.tree.find_tab(panel) {
        shell.tree.split_box(b, tree::Axis::Vertical, &id, true, 0.5).is_some()
    } else if let Some(f) = shell.float_of(panel) {
        shell.insert_tabs(f, None, std::slice::from_ref(&id))
    } else {
        false
    };
    if !placed {
        return None;
    }
    shell.maximized = shell.maximized_box();
    shell.views.insert(id.clone(), view);
    shell.settle();
    Some(id)
}

/// The Board in `rect`, and the canvas inside it when a document shows ([`Hit::BoardBody`]).
fn board(app: &mut VectorcraftApp, ui: &mut egui::Ui, rect: Rect, framed: bool, hits: &mut Vec<(Hit, Rect)>) {
    boxes::board(app, ui, rect, framed);
    // The canvas sets its rect on every frame it draws a document (not Home).
    if !crate::varos::home::is_home(app)
        && app.session.active().is_some()
        && let Some(body) = app.canvas_rect.filter(|r| rect.contains_rect(*r))
    {
        hits.push((Hit::BoardBody, body));
    }
}

/// Compatibility shim: external writes request a strip flyout or activate an existing box tab.
/// The shell mirrors its transient flyout into `open_panel`; box activation clears it. An
/// unchanged mirrored value is not a new request. Panel bodies may request another panel for
/// the next frame, so never overwrite their writes at the end of drawing.
fn follow_open_panel(app: &mut VectorcraftApp) {
    if app.ui.open_panel == app.ui.varos_shell.open {
        return;
    }
    let request = app.ui.open_panel.take();
    strip::close(app);
    if let Some(panel) = request.as_deref().and_then(crate::menus::normalize_panel) {
        show_panel(app, panel, false);
    }
}

/// Is panel `id` shown: a tab of a box (not one hidden by a maximized box) or of a floating panel,
/// or the open flyout.
pub(crate) fn showing(app: &VectorcraftApp, id: &str) -> bool {
    let shell = &app.ui.varos_shell;
    shell.tree.find_tab(id).is_some_and(|b| shell.maximized_box().is_none_or(|m| m == b))
        || shell.float_of(id).is_some()
        || shell.open.as_deref() == Some(id)
}

/// The window commands while the shell is on; `None` for every other command (and for all of them
/// while it is off). Strip panels (the Board's strip or a floating one) open as flyouts; tabs of
/// boxes and floating panels activate. Toggle closes either, returning a closed tab to the strip.
/// The workspace commands are the shell's own ([`workspace::command`]); the dock commands are not
/// the shell's (nothing changes; the status line says so).
pub(crate) fn command(app: &mut VectorcraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if !enabled(app) {
        return None;
    }
    let raw = params.get("panel").and_then(Value::as_str).unwrap_or_default();
    let panel = || resolve_panel(app, raw).ok_or_else(|| format!("unknown panel `{raw}`"));
    match id {
        "window.control" => Some(Ok(show_panel(app, "control", true))),
        "window.panel" | "window.panel.activate" => Some(panel().map(|panel| {
            let toggle = id == "window.panel" && params.get("toggle").and_then(Value::as_bool) == Some(true);
            show_panel(app, &panel, toggle)
        })),
        "window.panel.close" => Some(panel().map(|panel| {
            let closed = app.ui.varos_shell.close_tab(&panel) || app.ui.varos_shell.open.as_deref() == Some(panel.as_str());
            if app.ui.varos_shell.open.as_deref() == Some(panel.as_str()) {
                strip::close_panel(app, &panel);
            }
            json!({"panel": panel, "closed": closed})
        })),
        "window.dock" | "window.panels" | "window.collapseDock" | "ui.dock" | "ui.dock.reset" => Some(Ok(not_yet(app, id))),
        _ if id.starts_with("window.workspace") => Some(workspace::command(app, id, params).unwrap_or_else(|| Ok(not_yet(app, id)))),
        _ if id.starts_with("window.panel.") => Some(Ok(not_yet(app, id))),
        _ => None,
    }
}

/// The panel `raw` names for the shell's window commands: another Properties editor the shell
/// has (`properties@2`), else a panel the app knows (by id or label, whatever the case).
pub(crate) fn resolve_panel(app: &VectorcraftApp, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("control") {
        return Some("control".into());
    }
    if persist::is_extra_instance(raw) {
        return app.ui.varos_shell.owns(raw).then(|| raw.to_string());
    }
    crate::menus::normalize_panel(raw).map(str::to_string)
}

fn show_panel(app: &mut VectorcraftApp, panel: &str, toggle: bool) -> Value {
    if crate::varos::home::is_home(app) || app.ui.screen_mode >= 1 {
        strip::close(app);
        app.status("Panels are hidden on Home and in full screen modes");
        return json!({"panel": panel, "shown": false});
    }
    let shell = &mut app.ui.varos_shell;
    if let Some(id) = shell.tree.find_tab(panel) {
        strip::close(app);
        // A tab hidden by a maximized box: the layout comes back, and the tab with it.
        let hidden = app.ui.varos_shell.maximized_box().is_some_and(|m| m != id);
        if hidden {
            app.ui.varos_shell.maximized = None;
        }
        if toggle && !hidden {
            app.ui.varos_shell.close_tab(panel);
        } else {
            app.ui.varos_shell.tree.activate(panel);
            app.ui.varos_shell.tree.expand(id);
        }
    } else if shell.float_of(panel).is_some() {
        strip::close(app);
        if toggle {
            app.ui.varos_shell.close_tab(panel);
        } else {
            app.ui.varos_shell.activate_float_tab(panel);
        }
    } else if shell.float_strip_of(panel).is_some() {
        if toggle && shell.open.as_deref() == Some(panel) {
            strip::close(app);
        } else {
            strip::open(app, panel);
        }
    } else {
        if app.ui.varos_shell.strip.group(panel).is_none() {
            app.ui.varos_shell.strip.append(panel);
        }
        if toggle && app.ui.varos_shell.open.as_deref() == Some(panel) {
            strip::close(app);
        } else {
            strip::open(app, panel);
        }
    }
    let shell = &app.ui.varos_shell;
    json!({"panel": panel, "shown": showing(app, panel), "box": shell.tree.find_tab(panel).or_else(|| shell.float_of(panel))})
}

/// A command the shell doesn't have yet: nothing changes; the status line says so.
fn not_yet(app: &mut VectorcraftApp, id: &str) -> Value {
    app.status(format!("{id}: not in the Varos shell yet"));
    json!({"ignored": id})
}
