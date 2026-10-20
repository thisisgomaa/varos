//! Icon panels open as independent cards (owner, 2026-10-20: "like Illustrator, each one on its
//! own, not in a fake side list that scrolls").
//!
//! With the dock in "keep open" mode ([`PanelStack::keep_open`], on by default) a click on an icon
//! of the icon column floats its panel as a card of its own: upstream's floating panel group
//! (`window.panel.float`), opened at a computed place beside the column and kept open. A card has
//! its own place and size, moves by its tab or grip, resizes by its corner, docks or groups like
//! any floating group (dropped on another card's tabs, on the left edge of the dock, on the column)
//! and closes on its own (its ×, its icon again, Window › that panel). Its icon stays in the column
//! and is lit while the card is open. Tab and Shift+Tab hide and bring back every card with the
//! rest of the panels (upstream hides floating groups with the dock).
//!
//! Where a new card goes ([`place`]): its right edge 6 points left of the dock, top aligned with
//! the icon column, then down past the cards already there; when the window's bottom is reached it
//! cascades from the column's top, 24 points left and down per step. A card closed and opened
//! again comes back where it was left, at the size it was given ([`PanelStack::places`]), unless
//! half of it would lie on another card (then it takes a new place); always left of the dock, also
//! when the window or the dock changed size since (cards restored from `ui.json` too). Until it
//! is resized by hand, a card is as tall as its panel's contents (capped by the window): the body
//! hook ([`note_body`]) reports how big the contents are and [`frame`] fits the card to them (wider
//! too when a row of the panel doesn't fit its default width).
//!
//! Panels opened another way (a Properties button, a workspace, a shortcut) become cards too. A
//! shortcut or an agent's `window.panel` on a card already open only shows it (it never closes
//! it); its icon and Window › that panel (`toggle`) close it. Two
//! upstream pop-outs stay as they were: a panel dragged out of the column (it leaves the column,
//! as in Illustrator) and put back pops out of its icon once; the tabbed group's panels in a
//! collapsed dock pop out one at a time. `UiState::open_panel` names the newest card, so upstream
//! code and agents that read "the open panel" see the one just opened. User Interface ›
//! Auto-Collapse Iconic Panels makes pop-outs transient: with it on the column goes back to
//! upstream's one-at-a-time behaviour, as with `keep_open` off.

use std::collections::BTreeMap;

use egui::{Pos2, Rect, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::ICON_PANELS;
use crate::{VectorcraftApp, floating};

/// Top of a new card when the icon column hasn't been drawn yet (upstream's `dock::FLYOUT_TOP`).
const TOP: f32 = 110.0;
/// Room between the dock and a card, and between two cards stacked down the column.
const GAP: f32 = 6.0;
/// Each step of the cascade once the column is full.
const CASCADE: f32 = 24.0;
/// Room kept free under a card at the window's bottom.
const MARGIN: f32 = 8.0;
/// A new card's height before its contents have been measured.
const START_TALL: f32 = 360.0;
/// How far over the room left of the dock a card may reach before it is moved back.
const SLACK: f32 = 2.0;
/// The least height a card is fitted to.
const MIN_TALL: f32 = 120.0;
/// The icon column's width (`dock::ICON_COL`) and the tabbed group's (`dock::DOCK_WIDTH`), for a
/// card opened before the dock has been drawn.
const ICON_COL: f32 = 38.0;
const DOCK_WIDTH: f32 = 310.0;

/// Where a panel's card was last, kept with the UI state (`ui.json`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// Top-left corner in screen points.
    pub pos: [f32; 2],
    /// Width and height.
    pub size: [f32; 2],
    /// Fitted to its contents (until resized by hand).
    #[serde(default = "yes")]
    pub fit: bool,
}

fn yes() -> bool {
    true
}

/// The icon panels opened as cards, kept with the UI state (`ui.json`). The cards themselves are
/// upstream floating groups (`UiState::docking`, `UiState::floating_panels`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelStack {
    /// Icon clicks open their panel as a card and leave the others open (Varos default). Off:
    /// upstream's one pop-out at a time.
    pub keep_open: bool,
    /// The panels open as cards, in the order they were opened.
    pub cards: Vec<String>,
    /// Where each panel's card was last.
    pub places: BTreeMap<String, Place>,
    /// The icon panels floating in the last frame. Not saved.
    #[serde(skip)]
    seen: Vec<&'static str>,
    /// A pop-out left as upstream made it (a dragged-out panel put back). Not saved.
    #[serde(skip)]
    flyout: Option<String>,
    /// Per card (by its first panel): the size it was fitted to, the contents' size then and how
    /// many panels the card had. Not saved.
    #[serde(skip)]
    fitted: BTreeMap<String, Fitted>,
    /// Native sets (drawn by `floating`) as drawn last, by first panel. Not saved.
    #[serde(skip)]
    drawn: BTreeMap<String, Rect>,
    /// The window and the dock as drawn last. Not saved.
    #[serde(skip)]
    view: Option<View>,
    /// A card was opened or changed outside a frame's input: draw again. Not saved.
    #[serde(skip)]
    repaint: bool,
    /// The workspace generation (`UiState::docking_generation`) the cards belong to. Not saved.
    #[serde(skip)]
    generation: Option<u64>,
}

impl Default for PanelStack {
    fn default() -> Self {
        Self {
            keep_open: true,
            cards: Vec::new(),
            places: BTreeMap::new(),
            seen: Vec::new(),
            flyout: None,
            fitted: BTreeMap::new(),
            drawn: BTreeMap::new(),
            view: None,
            repaint: false,
            generation: None,
        }
    }
}

/// What is saved: frame-to-frame bookkeeping doesn't make the UI state differ.
impl PartialEq for PanelStack {
    fn eq(&self, other: &Self) -> bool {
        self.keep_open == other.keep_open && self.cards == other.cards && self.places == other.places
    }
}

/// A card's fitting: the size it was given, its contents' size then, how many panels it had.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fitted {
    size: Vec2,
    content: Vec2,
    panels: usize,
}

/// The window and where cards open in it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct View {
    screen: Rect,
    /// A new card's right edge (left of the icon column and of the dock).
    right: f32,
    /// A new card's top.
    top: f32,
}

impl View {
    /// Where cards stay: the window left of the dock.
    fn room(&self) -> Rect {
        Rect::from_min_max(self.screen.min, pos2(self.right.max(self.screen.left() + 1.0), self.screen.bottom()))
    }
}

/// Is the card mode on: keep-open, and pop-outs not made transient by Auto-Collapse.
fn active(app: &VectorcraftApp) -> bool {
    app.ui.varos_panel_stack.keep_open && !app.session.prefs.auto_collapse_icon_panels
}

/// Panel `id` as the `'static` id of an icon-column panel, if it is one.
fn rail_panel(id: &str) -> Option<&'static str> {
    ICON_PANELS.iter().find(|e| e.0 == id).map(|e| e.0)
}

/// Is panel `id` in a floating group (a card, or a group docked as a dock column)?
fn floating(app: &VectorcraftApp, id: &str) -> bool {
    match &app.ui.docking {
        Some(layout) => layout.floating.iter().any(|g| g.panels.iter().any(|p| p == id)),
        None => floating::group_of(&app.ui, id).is_some(),
    }
}

/// Does panel `id` keep its icon in the column though it floats: it is open as a card.
pub(crate) fn keeps_icon(app: &VectorcraftApp, id: &str) -> bool {
    active(app) && app.ui.varos_panel_stack.cards.iter().any(|p| p == id) && floating(app, id)
}

/// Is panel `id` open (its icon lit): popped out, or open as a card.
pub(crate) fn is_open(app: &VectorcraftApp, id: &str) -> bool {
    app.ui.open_panel.as_deref() == Some(id) || keeps_icon(app, id)
}

/// An icon click (or Window › that panel) on panel `id`. In card mode an icon panel opens as a
/// card, or its card closes; otherwise upstream's swap (pop it out in place of the one popped
/// out, or put it away).
pub(crate) fn toggle(app: &mut VectorcraftApp, id: &str) {
    let Some(id) = rail_panel(id).filter(|_| active(app)) else {
        app.ui.open_panel = if app.ui.open_panel.as_deref() == Some(id) { None } else { Some(id.to_string()) };
        return;
    };
    if floating(app, id) {
        close(app, id);
    } else if app.ui.open_panel.as_deref() == Some(id) {
        // Popped out of its icon (a dragged-out panel put back): its icon puts it away.
        app.ui.open_panel = None;
    } else {
        open(app, id);
    }
}

/// Open panel `id` as a card: where it was left, or at the next free place beside the dock.
/// False when upstream refused (its status says why).
fn open(app: &mut VectorcraftApp, id: &'static str) -> bool {
    let rect = spot(app, id);
    let params = json!({"panel": id, "x": rect.left(), "y": rect.top(), "width": rect.width(), "height": rect.height()});
    if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.float", &params) {
        app.ui.status = error;
        return false;
    }
    let stack = &mut app.ui.varos_panel_stack;
    stack.cards.retain(|p| p != id);
    stack.cards.push(id.to_string());
    stack.fitted.remove(id);
    stack.repaint = true;
    // Upstream's open panel is the newest card (its float put the pop-out away).
    app.ui.open_panel = Some(id.to_string());
    true
}

/// Close panel `id`'s card (the panel only, when other panels share its card).
fn close(app: &mut VectorcraftApp, id: &str) {
    remember(app);
    if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.close", &json!({ "panel": id })) {
        app.ui.status = error;
    }
    app.ui.varos_panel_stack.cards.retain(|p| p != id);
    if app.ui.open_panel.as_deref() == Some(id) {
        app.ui.open_panel = None;
    }
}

/// The window and dock as last drawn, or a guess before the first frame.
fn view(app: &VectorcraftApp) -> View {
    app.ui.varos_panel_stack.view.unwrap_or_else(|| {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
        let dock = ICON_COL + if app.ui.dock_collapsed { 0.0 } else { DOCK_WIDTH };
        View { screen, right: screen.right() - dock - GAP, top: TOP }
    })
}

/// How much of the smaller of `a` and `b` they share (0 apart, 1 one inside the other).
fn overlap(a: Rect, b: Rect) -> f32 {
    let both = a.intersect(b);
    if !both.is_positive() {
        return 0.0;
    }
    both.area() / a.area().min(b.area()).max(1.0)
}

/// Panel `id`'s card: where it was left (moved into the room left of the dock), unless half of it
/// or more would lie on another card; else a new place.
fn spot(app: &VectorcraftApp, id: &str) -> Rect {
    let v = view(app);
    let room = v.room();
    let stack = &app.ui.varos_panel_stack;
    let saved = stack.places.get(id).filter(|p| p.pos.iter().chain(&p.size).all(|v| v.is_finite()) && p.size.iter().all(|v| *v > 0.0));
    let tallest = (v.screen.bottom() - v.top - MARGIN).max(MIN_TALL);
    let size = saved.map_or(vec2(crate::dock::panel_width(id), START_TALL), |p| vec2(p.size[0], p.size[1]));
    let size = vec2(size.x.max(1.0), size.y.clamp(MIN_TALL, tallest));
    let taken = taken(app);
    let back = saved.map(|p| Rect::from_min_size(floating::clamp(p.pos, size, room), size)).filter(|r| taken.iter().all(|t| overlap(*r, *t) < 0.5));
    back.unwrap_or_else(|| Rect::from_min_size(place(size, &taken, room, v.right, v.top), size))
}

/// The floating groups' bounds (not those docked as dock columns): a shared-layout window's own
/// rect (moved into the window as it is drawn), a native set's bounds as drawn last, else a guess.
fn taken(app: &VectorcraftApp) -> Vec<Rect> {
    let screen = view(app).screen;
    let stack = &app.ui.varos_panel_stack;
    app.ui
        .floating_panels
        .iter()
        .filter(|g| g.docked.is_none())
        .filter_map(|g| {
            let first = g.panels.first()?;
            let shared = app.ui.docking.as_ref().filter(|_| !crate::panel_docking::decorated(g));
            if let Some(r) = shared.and_then(|l| l.floating.iter().find(|f| f.panels.first() == Some(first))).map(|f| f.rect) {
                let size = vec2(r[2], r[3]);
                return Some(Rect::from_min_size(floating::clamp([r[0], r[1]], size, screen), size));
            }
            if let Some(r) = stack.drawn.get(first) {
                return Some(*r);
            }
            let size = vec2(g.width.unwrap_or_else(|| crate::dock::panel_width(first)), START_TALL);
            Some(Rect::from_min_size(pos2(g.pos[0], g.pos[1]), size))
        })
        .collect()
}

/// Where a new card `size` big goes among the cards `taken` in `screen`: right edge at `right`,
/// from `top` down past the cards in its way; when the window's bottom is reached, a cascade from
/// the top of that column, [`CASCADE`] points left and down per step, at the first step no card's
/// corner is near.
pub(crate) fn place(size: Vec2, taken: &[Rect], screen: Rect, right: f32, top: f32) -> Pos2 {
    let x = (right - size.x).max(screen.left());
    let top = top.max(screen.top());
    let bottom = screen.bottom() - MARGIN;
    let mut y = top;
    loop {
        let r = Rect::from_min_size(pos2(x, y), size);
        let past = taken.iter().filter(|t| t.shrink(0.5).intersects(r)).map(|t| t.bottom()).reduce(f32::max);
        match past {
            // Free, and on screen (or the column's first place: a card taller than the window
            // still opens there, fitted into it).
            None if y <= top || r.bottom() <= bottom => return pos2(x, y),
            None => break,
            Some(b) => {
                y = b + GAP;
                if y + size.y > bottom {
                    break;
                }
            }
        }
    }
    let near = |p: Pos2| taken.iter().any(|t| (t.min - p).length() < CASCADE / 2.0);
    let step = (1..=taken.len() + 1).map(|k| pos2(x - CASCADE * k as f32, top + CASCADE * k as f32)).find(|p| !near(*p));
    floating::clamp(step.map_or([x, top], |p| [p.x, p.y]), size, screen)
}

/// Keep where each card is (and its size) as the place it opens at next time.
fn remember(app: &mut VectorcraftApp) {
    let ui = &mut app.ui;
    let stack = &mut ui.varos_panel_stack;
    for g in ui.floating_panels.iter().filter(|g| g.docked.is_none()) {
        let Some(first) = g.panels.first() else { continue };
        let kept = ui.docking.as_ref().and_then(|l| l.floating.iter().find(|f| f.panels.first() == Some(first))).map(|f| [f.rect[2], f.rect[3]]);
        let drawn = stack.drawn.get(first).map(|r| [r.width(), r.height()]);
        let Some(size) = kept.or(drawn) else { continue };
        if !g.pos.iter().chain(&size).all(|v| v.is_finite()) {
            continue;
        }
        for p in g.panels.iter().filter(|p| stack.cards.contains(p)) {
            let fit = stack.places.get(p).is_none_or(|x| x.fit);
            let place = Place { pos: g.pos, size, fit };
            if stack.places.get(p) != Some(&place) {
                stack.places.insert(p.clone(), place);
            }
        }
    }
}

/// The body of a card's panel was drawn this frame: its contents `content` big in a scroll area
/// `viewport` big. Called from upstream's floating-panel body (`panel_docking`).
pub(crate) fn note_body(ctx: &egui::Context, panel: &str, content: Vec2, viewport: Vec2) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("varos-card-body", panel)), (content, viewport, pass)));
}

/// Once a frame (from `dock::floating_panel`, after the dock and its floating groups were drawn):
/// follow what upstream did to the cards, turn other pop-outs of icon panels into cards, keep the
/// cards left of the dock, keep each card's place and fit cards to their contents.
pub(crate) fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let generation = app.ui.docking_generation;
    let stack = &mut app.ui.varos_panel_stack;
    match stack.generation {
        // First frame: what was loaded keeps only icon panels.
        None => {
            stack.cards.retain(|p| rail_panel(p).is_some());
            stack.places.retain(|p, _| rail_panel(p).is_some());
        }
        // A workspace was applied: it brings its own panels.
        Some(g) if g != generation => {
            stack.cards.clear();
            stack.seen.clear();
            stack.fitted.clear();
            stack.flyout = None;
        }
        Some(_) => {}
    }
    stack.generation = Some(generation);
    let screen = ctx.content_rect();
    if !active(app) || !(screen.width() > 1.0 && screen.height() > 1.0) {
        // Not in card mode (or a minimized window): nothing to follow.
        app.ui.varos_panel_stack.seen.clear();
        return;
    }
    let moved = measure(app, ctx);
    if let Some(p) = app.ui.open_panel.as_deref().and_then(rail_panel)
        && !floating(app, p)
        && app.ui.dock
        && app.ui.screen_mode < 3
    {
        let stack = &mut app.ui.varos_panel_stack;
        let docked = app.ui.docking.as_ref().is_some_and(|l| l.contains(&p.to_string()));
        if stack.seen.contains(&p) {
            if stack.cards.iter().any(|c| c == p) {
                // Its card was put back in the dock (its ×, dropped on the column): it closes,
                // rather than popping out of its icon.
                app.ui.open_panel = None;
            } else {
                // A dragged-out panel put back pops out of its icon, as upstream.
                stack.flyout = Some(p.to_string());
            }
        } else if stack.flyout.as_deref() != Some(p) && !docked && !open(app, p) {
            // Opened another way (a Properties button, a workspace): a card. Refused (a broken
            // saved layout): it isn't asked again each frame.
            app.ui.open_panel = None;
        }
    }
    let ui = &mut app.ui;
    if ui.varos_panel_stack.flyout.is_some() && ui.varos_panel_stack.flyout != ui.open_panel {
        ui.varos_panel_stack.flyout = None;
    }
    // The icon panels floating now, for the next frame.
    let mut seen = std::mem::take(&mut ui.varos_panel_stack.seen);
    seen.clear();
    match &ui.docking {
        Some(l) => seen.extend(l.floating.iter().flat_map(|g| g.panels.iter()).filter_map(|p| rail_panel(p))),
        None => seen.extend(ui.floating_panels.iter().flat_map(|g| g.panels.iter()).filter_map(|p| rail_panel(p))),
    }
    ui.varos_panel_stack.seen = seen;
    let mut repaint = std::mem::take(&mut app.ui.varos_panel_stack.repaint);
    if !app.ui.varos_panel_stack.cards.is_empty() {
        let mut cards = std::mem::take(&mut app.ui.varos_panel_stack.cards);
        cards.retain(|p| floating(app, p));
        // Upstream's open panel follows the newest card still open when nothing else pops out
        // (its layout changes put the pop-out away).
        if app.ui.open_panel.is_none() && app.ui.dock {
            app.ui.open_panel = cards.last().cloned();
        }
        app.ui.varos_panel_stack.cards = cards;
        let mut moves = Vec::new();
        if moved {
            keep_in_room(app, &mut moves);
        }
        fit(app, ctx, &mut moves);
        repaint |= !moves.is_empty();
        apply(app, moves);
        // Places are kept once a move or resize is let go.
        if !ctx.input(|i| i.pointer.any_down()) {
            remember(app);
        }
    }
    if repaint {
        ctx.request_repaint();
    }
}

/// Keep the window, the dock and the native sets as drawn this frame. True when the window or
/// the dock changed since (or this is the first frame).
fn measure(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    let screen = ctx.content_rect();
    let shown = app.ui.dock && app.ui.screen_mode < 3;
    let column = ctx.data(|d| d.get_temp::<Rect>(floating::icons_rect_id())).filter(|r| r.is_finite() && r.left() > screen.left());
    let right = match column.filter(|_| shown) {
        Some(c) => {
            // Left of the icon column, of the docked columns and of the dock (the shared layout's
            // tabbed group included).
            let left = |id: egui::Id| ctx.data(|d| d.get_temp::<f32>(id)).filter(|x| x.is_finite() && *x > screen.left());
            let dock = ctx.data(|d| d.get_temp::<Rect>(floating::dock_rect_id())).map(|r| r.left()).filter(|x| x.is_finite() && *x > screen.left());
            [left(crate::dock::dock_left_id()), dock].into_iter().flatten().fold(c.left(), f32::min)
        }
        None if shown => screen.right() - ICON_COL - if app.ui.dock_collapsed { 0.0 } else { DOCK_WIDTH },
        None => screen.right(),
    } - GAP;
    let top = column.filter(|_| shown).map_or(TOP, |c| c.top());
    let ui = &mut app.ui;
    let stack = &mut ui.varos_panel_stack;
    stack.drawn.clear();
    for g in ui.floating_panels.iter().filter(|g| crate::panel_docking::decorated(g)) {
        if let Some(first) = g.panels.first()
            && let Some(r) = ctx.data(|d| d.get_temp::<Rect>(floating::group_rect_id(first))).filter(|r| r.is_finite())
        {
            stack.drawn.insert(first.clone(), r);
        }
    }
    let view = View { screen, right, top };
    stack.view.replace(view) != Some(view)
}

/// The cards of the shared docking layout (not native sets) and their rects, with the panel shown.
fn shared_cards<'a>(
    app: &'a VectorcraftApp,
    layout: &'a craft_ui::docking::Layout<String>,
) -> impl Iterator<Item = (&'a craft_ui::docking::Floating<String>, &'a String)> + 'a {
    layout.floating.iter().filter_map(move |g| {
        let first = g.panels.first()?;
        if app.ui.floating_panels.iter().any(|n| n.panels.first() == Some(first) && crate::panel_docking::decorated(n)) {
            return None;
        }
        let shown = g.panels.get(g.active).or(g.panels.first())?;
        app.ui.varos_panel_stack.cards.contains(shown).then_some((g, shown))
    })
}

/// The window or the dock changed: cards reaching over the dock (or out of the window) move back
/// into the room left of it, keeping their size.
fn keep_in_room(app: &VectorcraftApp, moves: &mut Vec<craft_ui::docking::Action<String>>) {
    let (Some(layout), Some(v)) = (&app.ui.docking, app.ui.varos_panel_stack.view) else { return };
    let room = v.room();
    for (g, _) in shared_cards(app, layout) {
        let [x, y, w, h] = g.rect;
        // A point or two over (a dock edge that moved by a rounding) is left alone.
        let to = floating::clamp([x, y], vec2(w, h), room.expand(SLACK));
        if (to - pos2(x, y)).length() > 0.5
            && let Some(first) = g.panels.first()
        {
            moves.push(craft_ui::docking::Action::MoveFloating { panel: first.clone(), rect: [to.x, to.y, w, h] });
        }
    }
}

/// Fit each card drawn by the shared docking layout to its shown panel's contents, until it is
/// resized by hand. A size is fitted once per size of the contents, so contents that follow the
/// card's height (a list filling it) leave it as it is.
fn fit(app: &mut VectorcraftApp, ctx: &egui::Context, moves: &mut Vec<craft_ui::docking::Action<String>>) {
    let Some(layout) = &app.ui.docking else { return };
    let Some(v) = app.ui.varos_panel_stack.view else { return };
    let mut resized: Vec<String> = Vec::new();
    let mut fitted: Vec<(String, Fitted)> = Vec::new();
    for (g, shown) in shared_cards(app, layout) {
        let Some(first) = g.panels.first() else { continue };
        let stack = &app.ui.varos_panel_stack;
        let [x, y, w, h] = g.rect;
        let before = stack.fitted.get(first).filter(|f| f.panels == g.panels.len());
        if let Some(f) = before
            && ((h - f.size.y).abs() > 0.5 || (w - f.size.x).abs() > 0.5)
            && !moves.iter().any(|m| matches!(m, craft_ui::docking::Action::MoveFloating { panel, .. } if panel == first))
        {
            // Resized by hand: it keeps that size from now on.
            resized.push(first.clone());
            continue;
        }
        if stack.places.get(shown).is_some_and(|p| !p.fit) {
            continue;
        }
        let Some((content, viewport)) = note(ctx, shown) else { continue };
        if before.is_some_and(|f| (f.content - content).abs().max_elem() <= 1.0) {
            continue;
        }
        // As tall as the contents (within the window); wider only when a row doesn't fit.
        let tallest = (v.screen.bottom() - y.max(v.screen.top()) - MARGIN).max(MIN_TALL);
        let tall = (h + content.y - viewport.y).clamp(MIN_TALL, tallest).round();
        let wide = if content.x > viewport.x + 0.5 { (w + content.x - viewport.x).min(v.screen.width() - 16.0).max(w).ceil() } else { w };
        let changed = (tall - h).abs() > 1.0 || (wide - w).abs() > 0.5;
        let size = if changed { vec2(wide, tall) } else { vec2(w, h) };
        fitted.push((first.clone(), Fitted { size, content, panels: g.panels.len() }));
        if changed {
            // Widened leftwards: its right edge stays (beside the dock).
            let x = (x - (size.x - w)).max(v.screen.left());
            moves.retain(|m| !matches!(m, craft_ui::docking::Action::MoveFloating { panel, .. } if panel == first));
            moves.push(craft_ui::docking::Action::MoveFloating { panel: first.clone(), rect: [x, y, size.x, size.y] });
        }
    }
    let ui = &mut app.ui;
    let stack = &mut ui.varos_panel_stack;
    for first in resized {
        if let Some(g) = ui.docking.as_ref().and_then(|l| l.floating.iter().find(|g| g.panels.first() == Some(&first))) {
            for p in &g.panels {
                if let Some(place) = stack.places.get_mut(p) {
                    place.fit = false;
                }
            }
        }
        stack.fitted.remove(&first);
    }
    stack.fitted.extend(fitted);
    // Cards gone, or now led by another panel, are fitted afresh.
    if let Some(layout) = &ui.docking {
        stack.fitted.retain(|first, _| layout.floating.iter().any(|g| g.panels.first() == Some(first)));
    }
}

/// Apply the moves of [`frame`] through upstream's layout command.
fn apply(app: &mut VectorcraftApp, moves: Vec<craft_ui::docking::Action<String>>) {
    if moves.is_empty() {
        return;
    }
    // Fitting a card is no layout change of upstream's: its pop-out (the newest card) stays.
    let open = app.ui.open_panel.clone();
    for action in moves {
        let Ok(action) = serde_json::to_value(action) else { continue };
        if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.layout", &json!({ "action": action })) {
            app.ui.status = error;
        }
    }
    app.ui.open_panel = open;
}

/// The note [`note_body`] left this frame for panel `id`: its contents' and scroll area's sizes.
fn note(ctx: &egui::Context, id: &str) -> Option<(Vec2, Vec2)> {
    let (content, viewport, pass) = ctx.data(|d| d.get_temp::<(Vec2, Vec2, u64)>(egui::Id::new(("varos-card-body", id))))?;
    (pass == ctx.cumulative_pass_nr() && content.is_finite() && viewport.is_finite()).then_some((content, viewport))
}
