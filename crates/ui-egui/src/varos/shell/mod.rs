//! The new Varos shell, stages 1–3 (`docs/varos/NEW_SHELL_PLAN.md`): Preferences › Varos shell (new)
//! draws the window's central region as our own split tree of boxes instead of craft-ui's dock.
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
        app.ui.varos_shell.drag = None;
    }
    app.ui.varos_shell.enabled = on;
}

/// A control of the shell as drawn last frame: what drags aim at, and what tests and agents read.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // Varos: some fields are read by the tests only.
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
    let alone = crate::varos::home::is_home(app) || app.ui.screen_mode >= 1;
    if alone && app.ui.varos_shell.drag.is_some() {
        drag::cancel(app, ui.ctx());
    }
    // A released drag lands before anything is laid out: this frame draws the result.
    drag::before(app, ui.ctx(), area);
    if alone {
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
            } else if let Some(ask) = boxes::show(app, ui, &b, *rect, false, &mut hits) {
                asks.push(ask);
            }
        }
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
            }
            ui.ctx().request_repaint();
        }
        floats::show(app, ui, area, &mut hits);
        // A flyout belongs to a strip's panel (one that left its strip takes its flyout along).
        if let Some(open) = app.ui.varos_shell.open.clone()
            && !app.ui.varos_shell.in_a_strip(&open)
        {
            strip::close(app);
        }
        drag::after(app, ui, area, &mut hits);
    }
    let pass = ui.ctx().cumulative_pass_nr();
    // Only a drag needs to know the layout its targets were found in (worth a hash per frame).
    let rev = app.ui.varos_shell.drag.as_ref().map(|_| drag::layout_rev(&app.ui.varos_shell, area));
    ui.ctx().data_mut(|d| d.insert_temp(hits_id(), Hits { pass, hits: std::sync::Arc::new(hits), rev }));
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

/// Is panel `id` shown: a tab of a box or of a floating panel, or the open flyout.
pub(crate) fn showing(app: &VectorcraftApp, id: &str) -> bool {
    let shell = &app.ui.varos_shell;
    shell.tree.find_tab(id).is_some() || shell.float_of(id).is_some() || shell.open.as_deref() == Some(id)
}

/// The window commands while the shell is on; `None` for every other command (and for all of them
/// while it is off). Strip panels (the Board's strip or a floating one) open as flyouts; tabs of
/// boxes and floating panels activate. Toggle closes either, returning a closed tab to the strip.
/// Dock/workspace commands remain for a later stage.
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
    let shell = &mut app.ui.varos_shell;
    if let Some(id) = shell.tree.find_tab(panel) {
        strip::close(app);
        if toggle {
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
