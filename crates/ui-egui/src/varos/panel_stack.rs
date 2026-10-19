//! Several icon panels open at once (owner, 2026-10-19: "opening Paragraph closes Character").
//!
//! Upstream pops ONE panel out of the icon column at a time (`UiState::open_panel`). With the dock
//! in "keep open" mode ([`PanelStack::keep_open`], on by default) an icon click toggles its own
//! panel only: the open panels stack top to bottom beside the column, oldest first, each card with
//! its own title, menu and » that puts it away, and the stack scrolls when it is taller than the
//! window. Every open panel's icon is lit.
//!
//! `open_panel` stays the newest open panel, so the rest of the app (Window menu, workspaces,
//! shortcuts, docking, floating) keeps working on it unchanged: [`reconcile`] follows what upstream
//! code did to it since the last frame (a panel opened is added to the stack, a panel put away is
//! taken out). With one panel open, upstream's own flyout draws it; from two on, [`show`] draws the
//! stack. User Interface › Auto-Collapse Iconic Panels makes flyouts transient, so with it on the
//! column goes back to upstream's one-at-a-time behaviour.

use egui::{CornerRadius, Rect, Sense, vec2};
use serde::{Deserialize, Serialize};

use crate::state::DockTab;
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, floating, icons, panels};

/// Top of the stack: where upstream's flyout begins (`dock::FLYOUT_TOP`).
const TOP: f32 = 110.0;
/// The icon column's width (`dock::ICON_COL`).
const ICON_COL: f32 = 38.0;
/// The tabbed group's default width (`dock::DOCK_WIDTH`).
const DOCK_WIDTH: f32 = 300.0;
/// Height of a card's title strip, as upstream's flyout.
const STRIP: f32 = 26.0;
/// Room between two cards.
const GAP: f32 = 6.0;
/// How tall Properties or Layers is in a stack of several (alone, it takes the window's height).
const TALL_IN_STACK: f32 = 320.0;

/// The panels popped out of the icon column, kept with the UI state (`ui.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelStack {
    /// Icon clicks toggle their own panel and leave the others open (Varos default). Off: upstream's
    /// one flyout at a time.
    pub keep_open: bool,
    /// The open panels, oldest first; the newest is `UiState::open_panel`.
    pub open: Vec<String>,
    /// `open_panel` as [`reconcile`] last saw it. Not saved.
    #[serde(skip)]
    seen: Option<String>,
    /// The workspace generation (`UiState::docking_generation`) the stack belongs to: a workspace
    /// applied since starts it again from its own panel. Not saved.
    #[serde(skip)]
    generation: Option<u64>,
}

impl Default for PanelStack {
    fn default() -> Self {
        Self { keep_open: true, open: Vec::new(), seen: None, generation: None }
    }
}

/// Is the stack in charge: keep-open mode, and flyouts not made transient by Auto-Collapse.
fn active(app: &VectorcraftApp) -> bool {
    app.ui.varos_panel_stack.keep_open && !app.session.prefs.auto_collapse_icon_panels
}

/// Panel `id` as the `'static` id the flyouts use, if it is a panel at all.
fn intern(id: &str) -> Option<&'static str> {
    crate::state::all_panels().map(|(q, _)| q).find(|q| *q == id).or_else(|| crate::panels::library_panel::intern(id))
}

/// Would `id` pop out beside the column (as upstream's `dock::floating_panel` decides): a panel,
/// not one of the tabbed group's while the dock is expanded, not floating. With a shared docking
/// layout, not one of its floating groups' panels, nor one its expanded dock shows.
fn pops_out(app: &VectorcraftApp, id: &str) -> bool {
    let docked = app.ui.docking.as_ref().is_some_and(|l| {
        let floats = l.floating.iter().any(|g| g.panels.iter().any(|p| p == id));
        floats || (!app.ui.dock_collapsed && l.contains(&id.to_string()))
    });
    intern(id).is_some() && !(DockTab::from_id(id).is_some() && !app.ui.dock_collapsed) && floating::group_of(&app.ui, id).is_none() && !docked
}

/// Follow what happened to `open_panel` since the last call: opened → the newest in the stack;
/// put away → out of the stack. Panels that no longer pop out (floated, back in the expanded dock)
/// leave it; with `open_panel` put away, the newest left takes its place.
pub(crate) fn reconcile(app: &mut VectorcraftApp) {
    let generation = app.ui.docking_generation;
    if !active(app) {
        // One flyout at a time: the stack is just upstream's open panel.
        let stack = &mut app.ui.varos_panel_stack;
        stack.open = app.ui.open_panel.iter().cloned().collect();
        stack.seen = app.ui.open_panel.clone();
        stack.generation = Some(generation);
        return;
    }
    let ui = &mut app.ui;
    let stack = &mut ui.varos_panel_stack;
    if stack.generation.is_some_and(|g| g != generation) {
        // A workspace was applied: it brings its own panel.
        stack.open.clear();
        stack.seen = None;
    }
    stack.generation = Some(generation);
    if ui.open_panel != stack.seen {
        match (&ui.open_panel, &stack.seen) {
            (Some(now), _) => {
                stack.open.retain(|p| p != now);
                stack.open.push(now.clone());
            }
            (None, Some(gone)) => stack.open.retain(|p| p != gone),
            (None, None) => {}
        }
    }
    let mut open = std::mem::take(&mut app.ui.varos_panel_stack.open);
    open.retain(|p| pops_out(app, p));
    open.dedup();
    if app.ui.open_panel.is_none() {
        app.ui.open_panel = open.last().cloned();
    }
    let stack = &mut app.ui.varos_panel_stack;
    stack.open = open;
    stack.seen = app.ui.open_panel.clone();
}

/// Is panel `id` popped out (its icon lit)?
pub(crate) fn is_open(app: &VectorcraftApp, id: &str) -> bool {
    app.ui.open_panel.as_deref() == Some(id) || (active(app) && app.ui.varos_panel_stack.open.iter().any(|p| p == id))
}

/// An icon click (or Window › that panel) on panel `id`: in keep-open mode it opens or puts away
/// that panel only; otherwise upstream's swap (open it in place of the one open, or put it away).
pub(crate) fn toggle(app: &mut VectorcraftApp, id: &str) {
    if !active(app) {
        app.ui.open_panel = if app.ui.open_panel.as_deref() == Some(id) { None } else { Some(id.to_string()) };
        return;
    }
    reconcile(app);
    let stack = &mut app.ui.varos_panel_stack;
    if stack.open.iter().any(|p| p == id) {
        stack.open.retain(|p| p != id);
        if app.ui.open_panel.as_deref() == Some(id) {
            app.ui.open_panel = stack.open.last().cloned();
        }
    } else {
        stack.open.push(id.to_string());
        app.ui.open_panel = Some(id.to_string());
    }
    app.ui.varos_panel_stack.seen = app.ui.open_panel.clone();
}

/// The panels the stack shows now, oldest first.
pub(crate) fn shown(app: &VectorcraftApp) -> Vec<&'static str> {
    if !active(app) {
        return Vec::new();
    }
    app.ui.varos_panel_stack.open.iter().filter(|p| pops_out(app, p)).filter_map(|p| intern(p)).collect()
}

/// Draw the stack beside the icon column when two or more panels are open; true when it did
/// (upstream's single flyout then draws nothing). Called at the top of `dock::floating_panel`.
pub(crate) fn show(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    reconcile(app);
    let ids = shown(app);
    if ids.len() < 2 {
        return false;
    }
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    // Right edge: as upstream's flyout, 6 points left of the dock (its icon column, or the docked
    // columns left of it); without a dock on screen, the window's right edge.
    let docked = app.ui.dock && app.ui.screen_mode < 3;
    let column = if docked {
        ctx.data(|d| d.get_temp::<f32>(egui::Id::new("dock-icon-column-left")))
            .filter(|x| x.is_finite() && *x > screen.left() && *x <= screen.right())
            .unwrap_or_else(|| screen.right() - ICON_COL - if app.ui.dock_collapsed { 0.0 } else { DOCK_WIDTH })
    } else {
        screen.right()
    };
    let left = ctx
        .data(|d| d.get_temp::<f32>(crate::dock::dock_left_id()))
        .filter(|x| x.is_finite() && *x > screen.left() && *x <= column)
        .unwrap_or(column);
    let right = if docked { left } else { column } - 6.0;
    let width = ids.iter().map(|id| crate::dock::panel_width(id)).fold(0.0, f32::max);
    let max_height = (screen.bottom() - TOP - 30.0).max(160.0);
    let mut close = None;
    let mut torn = None;
    let area = egui::Area::new(egui::Id::new("varos-panel-stack")).order(egui::Order::Foreground).pivot(egui::Align2::RIGHT_TOP);
    area.fixed_pos(egui::pos2(right, TOP)).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt("varos-panel-stack-scroll").max_height(max_height).auto_shrink([true, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = GAP;
            for &id in &ids {
                ui.push_id(id, |ui| {
                    if let Some(c) = card(app, ui, &t, id, width) {
                        match c {
                            Card::Close => close = Some(id),
                            Card::Torn(left) => torn = Some((id, left)),
                        }
                    }
                });
            }
        });
    });
    if let Some((id, left)) = torn {
        floating::tear(app, ctx, &[id], id, floating::strip_grab(ctx, left));
    } else if let Some(id) = close {
        toggle(app, id);
    }
    true
}

/// What happened to a card this frame.
enum Card {
    /// Its » was clicked.
    Close,
    /// Its title was dragged out: it floats, grabbed this far right of its left edge.
    Torn(f32),
}

/// One panel of the stack: upstream's flyout look (title tab on a darker strip, the panel menu,
/// the » that puts it away), then the panel itself.
fn card(app: &mut VectorcraftApp, ui: &mut egui::Ui, t: &Tokens, id: &'static str, width: f32) -> Option<Card> {
    let label = crate::panels::library_panel::tab_label(app, id)
        .unwrap_or_else(|| crate::state::all_panels().find(|(q, _)| *q == id).map_or(id, |(_, label)| tl!(label)).to_string());
    let radius = crate::varos::theme::box_radius(t, 4);
    let mut out = None;
    egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(radius).inner_margin(egui::Margin::ZERO).show(ui, |ui| {
        ui.set_width(width);
        let (strip, _) = ui.allocate_exact_size(vec2(width, STRIP), Sense::hover());
        let r = radius.nw;
        ui.painter().rect_filled(strip, CornerRadius { nw: r, ne: r, sw: 0, se: 0 }, t.panel_darker);
        let title =
            Rect::from_min_size(strip.min, vec2(ui.painter().layout_no_wrap(label.clone(), theme::semibold(12.0), t.text).size().x + 24.0, STRIP));
        ui.painter().rect_filled(title, CornerRadius { nw: r, ne: 0, sw: 0, se: 0 }, t.panel);
        ui.painter().text(title.left_center() + vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, &label, theme::semibold(12.0), t.text);
        let close = Rect::from_center_size(strip.right_center() - vec2(13.0, 0.0), vec2(14.0, 14.0));
        let cr = ui.interact(close, ui.id().with("close-panel"), Sense::click());
        icons::paint(ui, "chevrons-right", close, if cr.hovered() { t.text } else { t.text_dim });
        if cr.clicked() {
            out = Some(Card::Close);
        }
        let menu = Rect::from_center_size(strip.right_center() - vec2(34.0, 0.0), vec2(16.0, 16.0));
        panels::panel_menu(app, ui, id, menu);
        let drag = ui.interact(title, ui.id().with("title"), Sense::drag()).on_hover_cursor(egui::CursorIcon::Grab);
        if drag.dragged() && ui.input(|i| i.pointer.is_decidedly_dragging()) {
            out = Some(Card::Torn(title.left()));
        }
        // Floated from its menu: its group shows it from this frame on.
        if matches!(out, Some(Card::Torn(_))) || floating::group_of(&app.ui, id).is_some() {
            return;
        }
        crate::dock::panel_body(app, ui, id, width, TALL_IN_STACK);
    });
    out
}
