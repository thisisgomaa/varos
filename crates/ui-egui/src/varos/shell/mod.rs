//! The new Varos shell, stages 1–2 (`docs/varos/NEW_SHELL_PLAN.md`): Preferences › Varos shell (new)
//! draws the window's central region as our own split tree of boxes instead of craft-ui's dock.
//!
//! The Board (the canvas, with its document tabs) is one box; every other box holds panels as
//! tabs ([`boxes`]); 12-point seams between them are the dividers ([`divider`]). The layout is
//! [`tree::Node`], saved as `varos_shell` in `ui.json` ([`persist`]). Off, the window is exactly
//! upstream's dock and cards; both layouts persist on their own, and the switch takes effect on
//! the next frame.
//!
//! Everything the shell needs from the editor is the stable host API: `canvas_area` (the canvas),
//! `dock::panel_body` (a panel's body) and `panels::panel_menu_items` (its ≡ menu).

pub(crate) mod boxes;
pub(crate) mod divider;
pub(crate) mod persist;
pub(crate) mod strip;
pub(crate) mod tree;

use egui::Rect;
use serde_json::{Value, json};

use crate::VectorcraftApp;

/// The Preferences dialog's field for Varos shell (new) (User Interface, under Panel cards
/// (Varos)), and its label.
pub(crate) const PREF_KEY: &str = "__varosShell";
pub(crate) const PREF_LABEL: &str = "Varos shell (new)";

/// The void around the boxes, inside the central region.
pub(crate) const OUTER: f32 = tree::SEAM;

/// Is the window's central region the Varos shell.
pub(crate) fn enabled(app: &VectorcraftApp) -> bool {
    app.ui.varos_shell.enabled
}

/// Preferences › Varos shell (new) set to `on`; the next frame draws the chosen shell.
pub(crate) fn set_enabled(app: &mut VectorcraftApp, on: bool) {
    if app.ui.varos_shell.enabled != on {
        strip::close(app);
    }
    app.ui.varos_shell.enabled = on;
}

/// A control of the shell as drawn last frame, for tests, agents and the next stages' drags.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // Varos: the fields are read by the tests now and by stage 3's tab drags next.
pub(crate) enum Hit {
    /// The Board (the canvas region, its document tabs included).
    Board,
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
    /// The divider of the split at this path.
    Divider(tree::Path),
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

/// The hit rects and the pass (`cumulative_pass_nr` while it drew) they belong to.
#[derive(Clone, Default)]
struct Hits {
    pass: u64,
    hits: std::sync::Arc<Vec<(Hit, Rect)>>,
}

/// Draw the central region: the boxes and their dividers, or the Board alone on Home and in the
/// full screen modes.
pub(crate) fn frame(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.divider)).show(ui, |ui| crate::varos::profile::measure("shell", || draw(app, ui)));
}

fn draw(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
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
    if crate::varos::home::is_home(app) || app.ui.screen_mode >= 1 {
        strip::close(app);
        hits.push((Hit::Board, area));
        board(app, ui, area, false, &mut hits);
    } else {
        let inner = area.shrink(OUTER);
        let mut layout = app.ui.varos_shell.tree.layout(inner);
        if divider::interact(app, ui, &layout) {
            // The new ratio is drawn this very frame.
            layout = app.ui.varos_shell.tree.layout(inner);
        }
        hits.extend(layout.dividers.iter().map(|d| (Hit::Divider(d.path.clone()), d.rect)));
        let mut asks = Vec::new();
        for (id, rect) in &layout.boxes {
            let Some(b) = app.ui.varos_shell.tree.find_box(*id).cloned() else { continue };
            if b.is_board() {
                hits.push((Hit::Board, *rect));
                let canvas = strip::show(app, ui, *rect, &mut hits);
                board(app, ui, canvas, true, &mut hits);
            } else if let Some(ask) = boxes::show(app, ui, &b, *rect, &mut hits) {
                asks.push(ask);
            }
        }
        for ask in asks {
            match ask {
                boxes::Ask::Close(panel) => {
                    app.ui.varos_shell.close_tab(&panel);
                    ui.ctx().request_repaint();
                }
            }
        }
    }
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| d.insert_temp(hits_id(), Hits { pass, hits: std::sync::Arc::new(hits) }));
    // A panel asked for by code that sets `open_panel` while this frame drew shows on the next.
    if app.ui.open_panel != app.ui.varos_shell.open {
        ui.ctx().request_repaint();
    }
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

pub(crate) fn showing(app: &VectorcraftApp, id: &str) -> bool {
    app.ui.varos_shell.tree.find_tab(id).is_some() || app.ui.varos_shell.open.as_deref() == Some(id)
}

/// The window commands while the shell is on; `None` for every other command (and for all of them
/// while it is off). Strip panels open as flyouts; box tabs activate. Toggle closes either,
/// returning a closed tab to the strip. Dock/workspace commands remain for a later stage.
pub(crate) fn command(app: &mut VectorcraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if !enabled(app) {
        return None;
    }
    let panel = || {
        let raw = params.get("panel").and_then(Value::as_str).unwrap_or_default();
        crate::menus::normalize_panel(raw).ok_or_else(|| format!("unknown panel `{raw}`"))
    };
    match id {
        "window.panel" | "window.panel.activate" => Some(panel().map(|panel| {
            let toggle = id == "window.panel" && params.get("toggle").and_then(Value::as_bool) == Some(true);
            show_panel(app, panel, toggle)
        })),
        "window.panel.close" => Some(panel().map(|panel| {
            let closed = app.ui.varos_shell.close_tab(panel) || app.ui.varos_shell.open.as_deref() == Some(panel);
            if app.ui.varos_shell.open.as_deref() == Some(panel) {
                strip::close_panel(app, panel);
            }
            json!({"panel": panel, "closed": closed})
        })),
        "window.dock" | "window.panels" | "window.collapseDock" | "ui.dock" | "ui.dock.reset" => Some(Ok(not_yet(app, id))),
        _ if id.starts_with("window.panel.") || id.starts_with("window.workspace") => Some(Ok(not_yet(app, id))),
        _ => None,
    }
}

fn show_panel(app: &mut VectorcraftApp, panel: &str, toggle: bool) -> Value {
    if crate::varos::home::is_home(app) || app.ui.screen_mode >= 1 {
        strip::close(app);
        app.status("Panels are hidden on Home and in full screen modes");
        return json!({"panel": panel, "shown": false});
    }
    if app.ui.varos_shell.tree.find_tab(panel).is_some() {
        strip::close(app);
        if toggle {
            app.ui.varos_shell.close_tab(panel);
        } else {
            app.ui.varos_shell.tree.activate(panel);
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
    json!({"panel": panel, "shown": showing(app, panel), "box": app.ui.varos_shell.tree.find_tab(panel)})
}

/// A command the shell doesn't have yet: nothing changes; the status line says so.
fn not_yet(app: &mut VectorcraftApp, id: &str) -> Value {
    app.status(format!("{id}: not in the Varos shell yet"));
    json!({"ignored": id})
}
