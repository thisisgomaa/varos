//! Icon panels open as independent cards (owner, 2026-10-20: "like Illustrator, each one on its
//! own, not in a fake side list that scrolls").
//!
//! With the dock in "keep open" mode ([`PanelStack::keep_open`], on by default) a click on an icon
//! of the icon column floats its panel as a card of its own. A card is an upstream floating panel
//! group (`window.panel.float`) for the model, persistence and docking, but Varos draws it and
//! handles its pointer (lane-panels3, owner: "they feel stuck"): an 8-point card with a 1-point
//! line, no shadow, a 32-point header holding its tabs, its × and its panel menu (≡). The whole
//! header moves the card, live under the pointer (no outline that jumps on release); its
//! bottom-right corner and its right and bottom edges resize it, live too. A tab dragged out of a
//! card with several tabs becomes a card of its own that follows the pointer; a card let go on
//! another card's header joins it as tabs; let go on the dock or the icon column it docks (goes
//! back to its icon), as upstream's drop targets say ([`crate::floating::drop_target`]).
//!
//! Where a new card goes ([`place`]): its right edge 6 points left of the dock, its top beside its
//! icon in the column (kept in the window), then down past the cards already there; when the
//! window's bottom is reached it cascades, 24 points left and down per step. A card closed and
//! opened again comes back where it was left, at the size it was given ([`PanelStack::places`]),
//! unless half of it would lie on another card (then it takes a new place); always left of the
//! dock, also when the window or the dock changed size since (cards restored from `ui.json` too).
//! Until it is resized by hand, a card is as tall as its panel's contents (capped by the window):
//! the body hook ([`note_body`]) reports how big the contents are and [`frame`] fits the card to
//! them (wider too when a row of the panel doesn't fit its default width).
//!
//! Panels put together as tabs of one card stay together ([`PanelStack::groups`]): the card closed
//! by its ×, any of its panels opened again opens the whole card, with the same tabs, the same tab
//! in front, where it was. A tab closed alone (by its icon) opens again in its card.
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
use std::hash::{Hash, Hasher};

use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};
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
/// A card's header (its tabs, × and ≡), as the kit's card (`panel-kit.html` §5).
const HEADER: f32 = 32.0;
/// A tab's height, and the room left and right of the header's contents.
const TAB_H: f32 = 24.0;
const PAD: f32 = 4.0;
/// A header button (× and ≡).
const BUTTON: Vec2 = vec2(20.0, 24.0);
/// A card's corners.
const RADIUS: u8 = 8;
/// The least size a card is drawn at (upstream's floating panel limits).
const MIN_W: f32 = 220.0;
const MIN_H: f32 = HEADER + 48.0;
/// The resize grips: the bottom-right corner, and the strips along the right and bottom edges.
const CORNER: f32 = 12.0;
const EDGE: f32 = 4.0;
/// Room under a card's contents, inside its rounded corners.
const FOOT: f32 = 6.0;
/// Points the pointer travels from its press before a let-go drops the card on anything (a click
/// on a header never groups or docks a card).
const TRAVEL: f32 = 4.0;
/// The tab label's size.
const LABEL: f32 = 11.0;

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
    /// Panels put together as tabs of one card, remembered when it closes: any of them opened
    /// again opens the whole card.
    pub groups: Vec<Group>,
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
    /// The top of each icon in the column, as drawn last. Not saved.
    #[serde(skip)]
    icons: Vec<(&'static str, f32)>,
    /// A card being moved or resized, or a tab being dragged. Not saved.
    #[serde(skip)]
    gesture: Option<Gesture>,
    /// The card whose header the moved card would join if let go now (by first panel). Not saved.
    #[serde(skip)]
    hint: Option<String>,
    /// Cards placed by Varos and not moved by hand since: fitting may move them up to keep them in
    /// the window. Not saved.
    #[serde(skip)]
    fresh: Vec<Fresh>,
    /// The cards' panels when [`PanelStack::groups`] was last brought up to date. Not saved.
    #[serde(skip)]
    signature: u64,
    /// How long the last frame's card work took (moving and drawing them), in ms. Tests only.
    #[cfg(test)]
    #[serde(skip)]
    pub(crate) cost_ms: f64,
}

/// A card placed by Varos (beside its icon) and not moved by hand since.
#[derive(Clone, Debug)]
struct Fresh {
    /// Its first panel.
    first: String,
    /// The icon it was placed beside and that icon's top then: for its first frames the card
    /// follows the icon if the column is laid out anew (the dock switching to its shared layout
    /// when the first card opens).
    icon: Option<(&'static str, f32)>,
    /// Frames since it opened.
    age: u8,
}

/// Panels kept together as the tabs of one card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// The tabs, in order.
    pub panels: Vec<String>,
    /// The tab in front.
    pub active: String,
    /// Tabs that aren't icon panels (a dock tab grouped in), put back in the dock when the card
    /// closed: they come back into the card with it, unless moved elsewhere since.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parked: Vec<String>,
}

/// What the pointer is doing to a card.
#[derive(Clone, Debug)]
enum Gesture {
    /// The card led by `first` follows the pointer, held `grab` from its top-left corner; `from` is
    /// where the press began, `original` where the card was (Escape puts it back).
    Move { first: String, grab: Vec2, from: Pos2, original: Rect },
    /// A tab of a card with several tabs is dragged; once out of its header (`strip`) it leaves
    /// as a card of its own (`size` big) that follows the pointer.
    Tab { panel: String, from: Pos2, strip: Rect, size: Vec2 },
    /// The card led by `first` is resized by its right and/or bottom edge, the pointer held `grab`
    /// from its bottom-right corner.
    Resize { first: String, right: bool, bottom: bool, grab: Vec2, original: Rect },
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
            groups: Vec::new(),
            icons: Vec::new(),
            gesture: None,
            hint: None,
            fresh: Vec::new(),
            signature: 0,
            #[cfg(test)]
            cost_ms: 0.0,
        }
    }
}

/// What is saved: frame-to-frame bookkeeping doesn't make the UI state differ.
impl PartialEq for PanelStack {
    fn eq(&self, other: &Self) -> bool {
        self.keep_open == other.keep_open && self.cards == other.cards && self.places == other.places && self.groups == other.groups
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
    /// The window down to the dock's bottom (above the status bars).
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

/// `size` made to fit in `room` (a card never larger than the room left of the dock; what doesn't
/// fit scrolls), at least [`MIN_TALL`] high when the room allows.
fn bounded(size: Vec2, room: Rect) -> Vec2 {
    vec2(size.x.clamp(1.0, room.width().max(1.0)), size.y.clamp(MIN_TALL.min(room.height()), room.height().max(1.0)))
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

/// Is this floating group of the shared layout a card (drawn by Varos, not by upstream's floating
/// window): card mode is on, it holds a panel opened as a card and it isn't a native set.
pub(crate) fn owns(app: &VectorcraftApp, group: &craft_ui::docking::Floating<String>) -> bool {
    active(app)
        && group.panels.iter().any(|p| app.ui.varos_panel_stack.cards.contains(p))
        && !group.panels.first().is_some_and(|first| native(app, first))
}

/// Is the floating group led by `first` a native set (a dock column, a set, collapsed), drawn by
/// `floating`?
fn native(app: &VectorcraftApp, first: &str) -> bool {
    app.ui.floating_panels.iter().any(|n| n.panels.first().is_some_and(|p| p == first) && crate::panel_docking::decorated(n))
}

/// The cards of the shared layout.
fn owned(app: &VectorcraftApp) -> impl Iterator<Item = &craft_ui::docking::Floating<String>> {
    app.ui.docking.iter().flat_map(|l| l.floating.iter()).filter(|g| owns(app, g))
}

/// The card holding panel `id`, if it is in one.
fn card_of<'a>(app: &'a VectorcraftApp, id: &str) -> Option<&'a craft_ui::docking::Floating<String>> {
    owned(app).find(|g| g.panels.iter().any(|p| p == id))
}

/// The icon of panel `id` was drawn in the column with its top at `top` (a new card opens beside
/// it). Kept in place: no allocation once every icon is known.
pub(crate) fn note_icon(app: &mut VectorcraftApp, id: &'static str, top: f32) {
    let icons = &mut app.ui.varos_panel_stack.icons;
    match icons.iter_mut().find(|e| e.0 == id) {
        Some(e) => e.1 = top,
        None => icons.push((id, top)),
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

/// Open panel `id` as a card: where it was left, or at the next free place beside its icon; with
/// the panels it was grouped with as tabs (in their card when it is open). False when upstream
/// refused (its status says why).
fn open(app: &mut VectorcraftApp, id: &'static str) -> bool {
    let group = app.ui.varos_panel_stack.groups.iter().find(|g| g.panels.iter().any(|p| p == id)).cloned();
    if let Some(done) = group.and_then(|g| reopen(app, id, &g)) {
        return done;
    }
    let (rect, fresh) = spot(app, id);
    let params = json!({"panel": id, "x": rect.left(), "y": rect.top(), "width": rect.width(), "height": rect.height()});
    if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.float", &params) {
        app.ui.status = error;
        return false;
    }
    opened(app, &[id], id, fresh);
    true
}

/// For their first frames, cards placed beside their icon follow it if the icon column was laid
/// out anew (the first card switches the dock to its shared layout, which draws the column a bit
/// higher or lower). True when a card moved.
fn follow_icons(app: &mut VectorcraftApp) -> bool {
    let stack = &mut app.ui.varos_panel_stack;
    if stack.fresh.is_empty() {
        return false;
    }
    let mut shifts: Vec<(String, f32)> = Vec::new();
    for f in stack.fresh.iter_mut().filter(|f| f.age < 6) {
        f.age += 1;
        if let Some((icon, top)) = &mut f.icon
            && let Some(now) = stack.icons.iter().find(|e| e.0 == *icon).map(|e| e.1).filter(|y| y.is_finite())
            && (now - *top).abs() > 0.5
        {
            shifts.push((f.first.clone(), now - *top));
            *top = now;
        }
    }
    let room = view(app).room();
    for (first, dy) in &shifts {
        let Some(r) = app.ui.docking.as_ref().and_then(|l| l.floating.iter().find(|g| g.panels.first() == Some(first))).map(|g| g.rect) else {
            continue;
        };
        let size = vec2(r[2], r[3]);
        set_rect(app, first, Rect::from_min_size(floating::clamp([r[0], r[1] + dy], size, room), size));
    }
    !shifts.is_empty()
}

/// `panels` (led by the first) were opened as one card, `id` asked for: they are cards now. `fresh`:
/// placed by Varos beside `id`'s icon, whose top was then this.
fn opened(app: &mut VectorcraftApp, panels: &[&'static str], id: &'static str, fresh: Option<Option<f32>>) {
    let stack = &mut app.ui.varos_panel_stack;
    for p in panels.iter().filter(|p| **p != id).chain([&id]).filter(|p| rail_panel(p).is_some()) {
        stack.cards.retain(|c| c != p);
        stack.cards.push(p.to_string());
    }
    if let Some(lead) = panels.first() {
        stack.fitted.remove(*lead);
        stack.fresh.retain(|f| f.first != *lead);
        if let Some(icon) = fresh {
            stack.fresh.push(Fresh { first: lead.to_string(), icon: icon.map(|top| (id, top)), age: 0 });
        }
    }
    stack.repaint = true;
    // Upstream's open panel is the newest card (its float put the pop-out away).
    app.ui.open_panel = Some(id.to_string());
}

/// Open panel `id` with the panels it was grouped with (`group`): as a tab of their card when one
/// of them is open, else as their card again, its tabs in their order with the same one in front.
/// None when nothing of the group is left to open with it (it opens alone).
fn reopen(app: &mut VectorcraftApp, id: &'static str, group: &Group) -> Option<bool> {
    let fail = |app: &mut VectorcraftApp, result: Option<Result<serde_json::Value, String>>| match result {
        Some(Err(error)) => {
            app.ui.status = error;
            true
        }
        _ => false,
    };
    if let Some(host) = group.panels.iter().filter(|p| *p != id).find(|p| card_of(app, p).is_some()).cloned() {
        let result = crate::panel_docking::command(app, "window.panel.float", &json!({"panel": id, "onto": host}));
        if fail(app, result) {
            return Some(false);
        }
        // At its place among the tabs (it was appended): before the next of its group's tabs in
        // the card. In front, as it was asked for.
        let card: Vec<String> = card_of(app, &host).map(|g| g.panels.clone()).unwrap_or_default();
        let at = group.panels.iter().position(|p| p == id).unwrap_or(0);
        if let Some(next) = group.panels[at + 1..].iter().find(|p| card.contains(p)).cloned() {
            let action = craft_ui::docking::Action::Move {
                panel: id.to_string(),
                anchor: next.clone(),
                placement: craft_ui::docking::Placement::Tab { before: Some(next) },
            };
            let action = serde_json::to_value(action).unwrap_or_default();
            let result = crate::panel_docking::command(app, "window.panel.layout", &json!({ "action": action }));
            if fail(app, result) {
                return Some(false);
            }
        }
        opened(app, &[id], id, None);
        return Some(true);
    }
    // Its group's panels closed with it (or parked in the dock by its ×), not those opened
    // elsewhere since.
    let shown = |p: &String| app.ui.docking.as_ref().is_some_and(|l| l.contains(p));
    let members: Vec<&'static str> = group
        .panels
        .iter()
        .filter(|p| *p == id || !shown(p) || group.parked.contains(p))
        .filter_map(|p| crate::menus::normalize_panel(p).filter(|n| n == p))
        .collect();
    let (lead, rest) = members.split_first().filter(|(_, rest)| !rest.is_empty())?;
    let (rect, fresh) = spot(app, id);
    let params = json!({"panel": lead, "x": rect.left(), "y": rect.top(), "width": rect.width(), "height": rect.height()});
    let result = crate::panel_docking::command(app, "window.panel.float", &params);
    if fail(app, result) {
        return Some(false);
    }
    for p in rest {
        let result = crate::panel_docking::command(app, "window.panel.float", &json!({"panel": p, "onto": lead}));
        if fail(app, result) {
            return Some(false);
        }
    }
    let front = members.iter().find(|p| **p == group.active).unwrap_or(&id);
    let action = serde_json::to_value(craft_ui::docking::Action::Activate { panel: front.to_string() }).unwrap_or_default();
    let result = crate::panel_docking::command(app, "window.panel.layout", &json!({ "action": action }));
    if fail(app, result) {
        return Some(false);
    }
    opened(app, &members, id, fresh);
    Some(true)
}

/// Close panel `id`'s card (the panel only, when other panels share its card).
fn close(app: &mut VectorcraftApp, id: &str) {
    remember(app);
    sync_groups(app);
    app.ui.varos_panel_stack.signature = 0;
    if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.close", &json!({ "panel": id })) {
        app.ui.status = error;
    }
    app.ui.varos_panel_stack.cards.retain(|p| p != id);
    if app.ui.open_panel.as_deref() == Some(id) {
        app.ui.open_panel = None;
    }
}

/// The card led by `first` closes (its ×): its icon panels go back to their icons, other panels
/// (a dock tab grouped with them) back to the dock. Its grouping is remembered.
fn close_card(app: &mut VectorcraftApp, first: &str) {
    let Some(panels) = card_of(app, first).map(|g| g.panels.clone()) else { return };
    remember(app);
    sync_groups(app);
    // Its other panels go back to the dock, as parked tabs of its group.
    if let Some(g) = app.ui.varos_panel_stack.groups.iter_mut().find(|g| g.panels.iter().any(|p| panels.contains(p))) {
        g.parked = panels.iter().filter(|p| rail_panel(p).is_none()).cloned().collect();
    }
    // Brought up to date again when its card opens again (the same tabs as now).
    app.ui.varos_panel_stack.signature = 0;
    for p in &panels {
        let id = if rail_panel(p).is_some() { "window.panel.close" } else { "window.panel.dock" };
        if let Some(Err(error)) = crate::panel_docking::command(app, id, &json!({ "panel": p })) {
            app.ui.status = error;
        }
    }
    app.ui.varos_panel_stack.cards.retain(|p| !panels.contains(p));
    if app.ui.open_panel.as_ref().is_some_and(|p| panels.contains(p)) {
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
/// or more would lie on another card; else a new place beside its icon (then also the icon's top it
/// was placed by, if known).
fn spot(app: &VectorcraftApp, id: &str) -> (Rect, Option<Option<f32>>) {
    let v = view(app);
    let room = v.room();
    let stack = &app.ui.varos_panel_stack;
    let saved = stack.places.get(id).filter(|p| p.pos.iter().chain(&p.size).all(|v| v.is_finite()) && p.size.iter().all(|v| *v > 0.0));
    let tallest = (v.screen.bottom() - v.top - MARGIN).max(MIN_TALL);
    let size = saved.map_or(vec2(crate::dock::panel_width(id), START_TALL), |p| vec2(p.size[0], p.size[1]));
    let size = bounded(vec2(size.x, size.y.min(tallest)), room);
    let taken = taken(app, None);
    let back = saved.map(|p| Rect::from_min_size(floating::clamp(p.pos, size, room), size)).filter(|r| taken.iter().all(|t| overlap(*r, *t) < 0.5));
    if let Some(back) = back {
        return (back, None);
    }
    // Its top beside its icon, kept where a card can still show in the window (its height is fitted
    // once its contents are known, and it moves up then if it must).
    let icon = stack.icons.iter().find(|e| e.0 == id).map(|e| e.1).filter(|y| y.is_finite());
    let top = icon.unwrap_or(v.top).min(room.bottom() - MARGIN - MIN_TALL).max(room.top());
    (Rect::from_min_size(place(size, &taken, room, v.right, top), size), Some(icon))
}

/// The floating groups' bounds (not those docked as dock columns, nor the one led by `except`): a
/// shared-layout window's own rect (moved into the window as it is drawn), a native set's bounds as
/// drawn last, else a guess.
fn taken(app: &VectorcraftApp, except: Option<&str>) -> Vec<Rect> {
    let screen = view(app).screen;
    let stack = &app.ui.varos_panel_stack;
    app.ui
        .floating_panels
        .iter()
        .filter(|g| g.docked.is_none())
        .filter_map(|g| {
            let first = g.panels.first().filter(|f| Some(f.as_str()) != except)?;
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
/// from `top` (beside its icon) down past the cards in its way; when the window's bottom is
/// reached, up past them from `top` instead; when the window's top is reached too, a cascade from
/// `top`, [`CASCADE`] points left and down per step, at the first step no card's corner is near.
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
    // Up from `top`, right above the cards in its way.
    let mut y = top;
    loop {
        let r = Rect::from_min_size(pos2(x, y), size);
        match taken.iter().filter(|t| t.shrink(0.5).intersects(r)).map(|t| t.top()).reduce(f32::min) {
            None if y >= screen.top() && y < top => return pos2(x, y),
            None => break,
            Some(t) => {
                y = t - GAP - size.y;
                if y < screen.top() {
                    break;
                }
            }
        }
    }
    // Each step as it would land (moved into the room), so two steps clamped onto the same corner
    // don't both count as free.
    let near = |p: Pos2| taken.iter().any(|t| (t.min - p).length() < CASCADE / 2.0);
    let steps = 1..=(taken.len() + 1).max(2) * 2;
    let step = steps.map(|k| floating::clamp([x - CASCADE * k as f32, top + CASCADE * k as f32], size, screen)).find(|p| !near(*p));
    step.unwrap_or_else(|| floating::clamp([x, top], size, screen))
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
            for g in &mut stack.groups {
                g.panels.retain(|p| crate::menus::normalize_panel(p) == Some(p.as_str()));
            }
            stack.groups.retain(|g| g.panels.len() >= 2);
        }
        // A workspace was applied: it brings its own panels. Its floating icon panels are cards
        // (workspaces keep the floating groups, not which of them were cards).
        Some(g) if g != generation => {
            stack.cards.clear();
            stack.seen.clear();
            stack.fitted.clear();
            stack.fresh.clear();
            stack.gesture = None;
            stack.hint = None;
            stack.flyout = None;
            let ui = &mut app.ui;
            let groups: Vec<&String> = match &ui.docking {
                Some(l) => l.floating.iter().flat_map(|g| g.panels.iter()).collect(),
                None => ui.floating_panels.iter().flat_map(|g| g.panels.iter()).collect(),
            };
            ui.varos_panel_stack.cards = groups.into_iter().filter(|p| rail_panel(p).is_some()).cloned().collect();
        }
        Some(_) => {}
    }
    app.ui.varos_panel_stack.generation = Some(generation);
    let screen = ctx.content_rect();
    if !active(app) {
        leave(app);
        return;
    }
    if !(screen.width() > 1.0 && screen.height() > 1.0) {
        // A minimized window: nothing to follow.
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
    // The cards are drawn by Varos where upstream draws its floating groups: with the panels shown.
    if !app.ui.varos_panel_stack.cards.is_empty() && app.ui.dock && app.ui.screen_mode < 3 && app.ui.docking.is_some() {
        #[cfg(test)]
        let start = std::time::Instant::now();
        track(app, ctx);
        draw(app, ctx);
        #[cfg(test)]
        {
            app.ui.varos_panel_stack.cost_ms = start.elapsed().as_secs_f64() * 1000.0;
        }
    } else {
        app.ui.varos_panel_stack.gesture = None;
        app.ui.varos_panel_stack.hint = None;
    }
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
        repaint |= follow_icons(app);
        let mut moves = Vec::new();
        // The window or dock changed, or a card's move or resize was let go: back into the room.
        if moved || ctx.input(|i| i.pointer.any_released()) {
            keep_in_room(app, &mut moves);
        }
        fit(app, ctx, &mut moves);
        repaint |= !moves.is_empty();
        apply(app, moves);
        // Places and groups are kept once a move or resize is let go.
        if !ctx.input(|i| i.pointer.any_down()) {
            remember(app);
            sync_groups(app);
        }
    }
    if repaint {
        ctx.request_repaint();
    }
}

/// Card mode was left (Auto-Collapse Iconic Panels on, or keep-open off): the cards close and the
/// newest pops out of its icon instead, as upstream's single transient pop-out.
fn leave(app: &mut VectorcraftApp) {
    app.ui.varos_panel_stack.seen.clear();
    if app.ui.varos_panel_stack.cards.is_empty() {
        return;
    }
    remember(app);
    let mut cards = std::mem::take(&mut app.ui.varos_panel_stack.cards);
    cards.retain(|p| floating(app, p));
    for p in &cards {
        if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.close", &json!({ "panel": p })) {
            app.ui.status = error;
        }
    }
    app.ui.open_panel = cards.last().cloned();
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
    // Native sets' bounds, updated in place (no allocation once a set is known).
    if !stack.drawn.is_empty() {
        stack.drawn.retain(|first, _| ui.floating_panels.iter().any(|g| crate::panel_docking::decorated(g) && g.panels.first() == Some(first)));
    }
    for g in ui.floating_panels.iter().filter(|g| crate::panel_docking::decorated(g)) {
        if let Some(first) = g.panels.first()
            && let Some(r) = ctx.data(|d| d.get_temp::<Rect>(floating::group_rect_id(first))).filter(|r| r.is_finite())
        {
            match stack.drawn.get_mut(first) {
                Some(kept) => *kept = r,
                None => {
                    stack.drawn.insert(first.clone(), r);
                }
            }
        }
    }
    // Cards keep above the status bars: down to the icon column's bottom, as the dock.
    let mut screen = screen;
    if let Some(c) = column.filter(|c| shown && c.bottom() > top + MIN_TALL && c.bottom() < screen.bottom()) {
        screen.max.y = c.bottom();
    }
    let view = View { screen, right, top };
    stack.view.replace(view) != Some(view)
}

/// The cards of the shared docking layout (not native sets) and their rects, with the panel shown.
fn shared_cards<'a>(
    app: &'a VectorcraftApp,
    layout: &'a craft_ui::docking::Layout<String>,
) -> impl Iterator<Item = (&'a craft_ui::docking::Floating<String>, &'a String)> + 'a {
    layout.floating.iter().filter(|g| owns(app, g)).filter_map(|g| Some((g, g.panels.get(g.active).or(g.panels.first())?)))
}

/// Cards reaching over the dock (or out of the window) move back into the room left of it, made
/// smaller only when larger than the room.
fn keep_in_room(app: &VectorcraftApp, moves: &mut Vec<craft_ui::docking::Action<String>>) {
    let (Some(layout), Some(v)) = (&app.ui.docking, app.ui.varos_panel_stack.view) else { return };
    let room = v.room();
    for (g, _) in shared_cards(app, layout) {
        let [x, y, w, h] = g.rect;
        // A point or two over (a dock edge that moved by a rounding) is left alone.
        let room = room.expand(SLACK);
        let size = bounded(vec2(w, h), room);
        let to = floating::clamp([x, y], size, room);
        if ((to - pos2(x, y)).length() > 0.5 || (size - vec2(w, h)).length() > 0.5)
            && let Some(first) = g.panels.first()
        {
            moves.push(craft_ui::docking::Action::MoveFloating { panel: first.clone(), rect: [to.x, to.y, size.x, size.y] });
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
        // As tall as the contents (within the window); wider only when a row doesn't fit. A card
        // Varos placed (beside its icon) and nobody moved since goes up rather than get shorter
        // when it would reach under the window's bottom (not above the icon column's top).
        let wanted = (h + content.y - viewport.y).max(MIN_TALL).round();
        let floor = v.screen.bottom() - MARGIN;
        let widest = v.room().width().max(1.0);
        let wide = if content.x > viewport.x + 0.5 { (w + content.x - viewport.x).ceil().max(w).min(widest) } else { w.min(widest) };
        // Widened leftwards: its right edge stays (beside the dock).
        let mut at = pos2((x - (wide - w)).max(v.screen.left()).min(v.room().right() - wide).max(v.screen.left()), y);
        let tall = match stack.fresh.iter().find(|f| f.first == *first) {
            Some(f) => {
                let tall = wanted.min((floor - v.top.min(y).max(v.screen.top())).max(MIN_TALL));
                if before.is_none() {
                    // Its first fitting (it hasn't been seen yet: a new egui area is drawn once
                    // invisibly to measure it): placed again beside its icon at its real size.
                    let others = taken(app, Some(first));
                    let room = v.room();
                    let start = f.icon.map_or(y, |(_, top)| top).min(room.bottom() - MARGIN - MIN_TALL).max(room.top());
                    at = place(vec2(wide, tall), &others, room, v.right, start);
                }
                at.y = at.y.min(floor - tall).max(v.screen.top());
                tall
            }
            None => wanted.min((floor - y.max(v.screen.top())).max(MIN_TALL)),
        };
        let changed = (tall - h).abs() > 1.0 || (wide - w).abs() > 0.5 || (at - pos2(x, y)).length() > 0.5;
        let size = if changed { vec2(wide, tall) } else { vec2(w, h) };
        fitted.push((first.clone(), Fitted { size, content, panels: g.panels.len() }));
        if changed {
            moves.retain(|m| !matches!(m, craft_ui::docking::Action::MoveFloating { panel, .. } if panel == first));
            moves.push(craft_ui::docking::Action::MoveFloating { panel: first.clone(), rect: [at.x, at.y, size.x, size.y] });
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
        if !stack.fresh.is_empty() {
            stack.fresh.retain(|f| layout.floating.iter().any(|g| g.panels.first() == Some(&f.first)));
        }
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
        // A size set here is no hand resize.
        if let craft_ui::docking::Action::MoveFloating { panel, rect } = &action
            && let Some(f) = app.ui.varos_panel_stack.fitted.get_mut(panel)
        {
            f.size = vec2(rect[2], rect[3]);
        }
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

/// The egui area of the card led by `first` (the id upstream's floating window had, so the card
/// keeps its place among the layers and `floating::drop_target` finds it).
fn card_id(first: &str) -> egui::Id {
    egui::Id::new("vectorcraft-panel-docking").with(("floating", first))
}

/// Panel `panel`'s tab in a card's header.
fn tab_id(panel: &str) -> egui::Id {
    egui::Id::new("varos-card-tab").with(panel)
}

/// Where a card `rect` (`[x, y, width, height]`, the layout's) is drawn in `screen`: at least the
/// least card size, at most the window, inside the window.
fn shown(rect: [f32; 4], screen: Rect) -> Rect {
    let least = vec2(MIN_W.min(screen.width()), MIN_H.min(screen.height()));
    let size = vec2(rect[2], rect[3]).max(least).min(screen.size());
    Rect::from_min_size(floating::clamp([rect[0], rect[1]], size, screen), size)
}

/// The header of a card drawn at `rect`.
fn header(rect: Rect) -> Rect {
    Rect::from_min_max(rect.min, pos2(rect.right(), (rect.top() + HEADER).min(rect.bottom())))
}

/// Panel `id`'s tab label: its label, or a library panel's library name.
fn label(app: &VectorcraftApp, id: &str) -> String {
    crate::panels::library_panel::tab_label(app, id)
        .or_else(|| crate::state::all_panels().find(|p| p.0 == id).map(|p| tl!(p.1).to_string()))
        .unwrap_or_else(|| id.to_string())
}

/// Put the card led by `first` at `rect` now (the layout and its legacy mirror), so that it is
/// drawn there this very frame.
fn set_rect(app: &mut VectorcraftApp, first: &str, rect: Rect) {
    if !(rect.is_finite() && rect.width() > 0.0 && rect.height() > 0.0) {
        return;
    }
    if let Some(g) = app.ui.docking.as_mut().and_then(|l| l.floating.iter_mut().find(|g| g.panels.first().is_some_and(|p| p == first))) {
        g.rect = [rect.left(), rect.top(), rect.width(), rect.height()];
    }
    if let Some(g) = app.ui.floating_panels.iter_mut().find(|g| g.panels.first().is_some_and(|p| p == first)) {
        g.pos = [rect.left(), rect.top()];
        g.width = Some(rect.width());
    }
}

/// Carry the pointer's gesture on a card (before the cards are drawn, so they are drawn where the
/// pointer has them this frame): start one when egui starts dragging a card's header, tab or
/// resize grip; move or resize the card with the pointer; let go, drop it.
fn track(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let screen = ctx.content_rect();
    let room = view(app).room();
    // Where the pointer was last seen in the window this frame: a move followed by the pointer
    // leaving the window (`PointerGone`) in the same frame still counts.
    let (pos, down, cancel) = ctx.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down(), i.key_pressed(egui::Key::Escape) || !i.focused));
    if app.ui.varos_panel_stack.gesture.is_none()
        && down
        && let Some(dragged) = ctx.dragged_id()
    {
        let press = ctx.input(|i| i.pointer.press_origin()).or(pos);
        app.ui.varos_panel_stack.gesture = press.and_then(|press| begin(app, dragged, press, screen));
        if let Some(Gesture::Move { first, .. } | Gesture::Resize { first, .. }) = &app.ui.varos_panel_stack.gesture {
            // Placed by hand from now on.
            let first = first.clone();
            app.ui.varos_panel_stack.fresh.retain(|f| f.first != first);
        }
    }
    app.ui.varos_panel_stack.hint = None;
    let Some(gesture) = app.ui.varos_panel_stack.gesture.clone() else { return };
    let Some(pos) = pos.filter(|p| p.is_finite()) else {
        // The pointer left the window (a frame ago): the card waits where it was last held; a
        // let-go out there drops nothing.
        if !down {
            app.ui.varos_panel_stack.gesture = None;
        }
        return;
    };
    match gesture {
        Gesture::Move { first, grab, from, original } => {
            if cancel {
                set_rect(app, &first, original);
                app.ui.varos_panel_stack.gesture = None;
                return;
            }
            // The card stays in the room left of the dock and above the status bars while held;
            // the pointer itself (for the drop targets) goes anywhere.
            let to = pos - grab;
            set_rect(app, &first, Rect::from_min_size(floating::clamp([to.x, to.y], original.size(), room), original.size()));
            if !down {
                app.ui.varos_panel_stack.gesture = None;
                if (pos - from).length() >= TRAVEL {
                    let_go(app, ctx, &first, pos);
                }
            } else if (pos - from).length() >= TRAVEL {
                // Light up where a let-go here would put it.
                match target(app, ctx, &first, pos) {
                    Some(Target::Card(onto)) => app.ui.varos_panel_stack.hint = Some(onto),
                    Some(Target::Upstream(drop, zone)) => floating::preview(ctx, drop, zone),
                    None => {}
                }
            }
        }
        Gesture::Tab { panel, from, strip, size } => {
            if cancel || !down {
                app.ui.varos_panel_stack.gesture = None;
                return;
            }
            if strip.expand2(vec2(8.0, 10.0)).contains(pos) {
                // Along its own header: the tab takes the place of the tab it passes, live.
                reorder(app, ctx, &panel, pos);
                return;
            }
            // Out of its card's header: it leaves as a card of its own, held by its tab.
            let grab = vec2(PAD + 12.0, HEADER / 2.0);
            let at = floating::clamp([pos.x - grab.x, pos.y - grab.y], size, room);
            let open = app.ui.open_panel.clone();
            let params = json!({"panel": panel, "x": at.x, "y": at.y, "width": size.x, "height": size.y});
            let result = crate::panel_docking::command(app, "window.panel.float", &params);
            app.ui.open_panel = open;
            app.ui.varos_panel_stack.gesture = match result {
                Some(Err(error)) => {
                    app.ui.status = error;
                    None
                }
                _ => {
                    app.ui.varos_panel_stack.fitted.remove(&panel);
                    Some(Gesture::Move { first: panel, grab, from, original: Rect::from_min_size(at, size) })
                }
            };
        }
        Gesture::Resize { first, right, bottom, grab, original } => {
            if cancel {
                set_rect(app, &first, original);
                app.ui.varos_panel_stack.gesture = None;
                return;
            }
            let Some(now) =
                app.ui.docking.as_ref().and_then(|l| l.floating.iter().find(|g| g.panels.first() == Some(&first))).map(|g| shown(g.rect, screen))
            else {
                app.ui.varos_panel_stack.gesture = None;
                return;
            };
            let corner = pos - grab;
            let max = pos2(if right { corner.x } else { now.right() }, if bottom { corner.y } else { now.bottom() });
            let size = (max - now.min).max(vec2(MIN_W, MIN_H)).min(room.max - now.min).max(vec2(1.0, 1.0));
            set_rect(app, &first, Rect::from_min_size(now.min, size));
            if !down {
                app.ui.varos_panel_stack.gesture = None;
            }
        }
    }
}

/// Tab `panel`, dragged along its card's header to `at`, passes the middle of another tab of its
/// card: it moves to that tab's other side (in front, as a dragged tab is).
fn reorder(app: &mut VectorcraftApp, ctx: &egui::Context, panel: &str, at: Pos2) {
    let Some(panels) = card_of(app, panel).map(|g| g.panels.clone()) else { return };
    let Some(from) = panels.iter().position(|p| p == panel) else { return };
    let rect = |p: &str| ctx.read_response(tab_id(p)).map(|r| r.rect);
    let Some((to, over)) =
        panels.iter().enumerate().filter(|(_, p)| *p != panel).find_map(|(i, p)| Some((i, rect(p).filter(|r| r.x_range().contains(at.x))?)))
    else {
        return;
    };
    let before = if to < from && at.x < over.center().x {
        Some(panels[to].clone())
    } else if to > from && at.x > over.center().x {
        panels.get(to + 1).cloned()
    } else {
        return;
    };
    let anchor = panels[to].clone();
    let open = app.ui.open_panel.clone();
    let action = craft_ui::docking::Action::Move { panel: panel.to_string(), anchor, placement: craft_ui::docking::Placement::Tab { before } };
    let action = serde_json::to_value(action).unwrap_or_default();
    if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.layout", &json!({ "action": action })) {
        app.ui.status = error;
    }
    app.ui.open_panel = open;
}

/// The gesture egui's drag of widget `dragged` (pressed at `press`) starts on a card, if it is one
/// of a card's.
fn begin(app: &VectorcraftApp, dragged: egui::Id, press: Pos2, screen: Rect) -> Option<Gesture> {
    for g in owned(app) {
        let Some(first) = g.panels.first() else { continue };
        let rect = shown(g.rect, screen);
        let id = card_id(first);
        let mover = || Gesture::Move { first: first.clone(), grab: press - rect.min, from: press, original: rect };
        let resize = |right, bottom| Gesture::Resize { first: first.clone(), right, bottom, grab: press - rect.max, original: rect };
        if dragged == id.with("header") {
            return Some(mover());
        }
        if dragged == id.with("resize") {
            return Some(resize(true, true));
        }
        if dragged == id.with("resize-x") {
            return Some(resize(true, false));
        }
        if dragged == id.with("resize-y") {
            return Some(resize(false, true));
        }
        if let Some(panel) = g.panels.iter().find(|p| dragged == tab_id(p)) {
            // A card's only tab moves the card; one of several tabs is torn out of it.
            return Some(if g.panels.len() == 1 {
                mover()
            } else {
                Gesture::Tab { panel: panel.clone(), from: press, strip: header(rect), size: rect.size() }
            });
        }
    }
    None
}

/// Where a moved card would land.
enum Target {
    /// In the card led by this panel, as tabs (let go on its header).
    Card(String),
    /// Upstream's drop target (the dock, a dock column, another floating group's tabs).
    Upstream(floating::Drop, Rect),
}

/// Where the card led by `first` lands if let go at `at`: on another card's header it joins that
/// card; else where upstream's drop targets say (the dock and the icon column dock it, a dock
/// column's edge docks it as a column, a group that isn't a card takes it as tabs). Dropped
/// anywhere else, it stays where it is.
fn target(app: &VectorcraftApp, ctx: &egui::Context, first: &str, at: Pos2) -> Option<Target> {
    let screen = ctx.content_rect();
    // The topmost card under the pointer (but the moved one): on its header, the moved card joins it.
    let layers: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
    let under = owned(app)
        .filter(|g| g.panels.first().is_some_and(|p| p != first))
        .filter_map(|g| Some((g.panels.first()?, shown(g.rect, screen))))
        .filter(|(_, r)| r.contains(at))
        .max_by_key(|(p, _)| layers.iter().position(|l| l.id == card_id(p)));
    if let Some((onto, rect)) = under {
        return header(rect).contains(at).then(|| Target::Card(onto.clone()));
    }
    let panel = crate::menus::normalize_panel(first)?;
    match floating::drop_target(app, ctx, floating::Moving::Panel(panel), at)? {
        (drop @ (floating::Drop::Dock | floating::Drop::Column(_)), zone) => Some(Target::Upstream(drop, zone)),
        (drop @ floating::Drop::Stack(onto), zone) if card_of(app, onto).is_none() => Some(Target::Upstream(drop, zone)),
        _ => None,
    }
}

/// The card led by `first` was let go at `at`: group it, dock it, or leave it there.
fn let_go(app: &mut VectorcraftApp, ctx: &egui::Context, first: &str, at: Pos2) {
    let Some(target) = target(app, ctx, first, at) else { return };
    let panels: Vec<String> = card_of(app, first).map(|g| g.panels.clone()).unwrap_or_default();
    let open = app.ui.open_panel.clone();
    let (id, params) = match target {
        Target::Card(onto) => ("window.panel.float", json!({"panel": first, "group": true, "onto": onto})),
        Target::Upstream(floating::Drop::Stack(onto), _) => ("window.panel.float", json!({"panel": first, "group": true, "onto": onto})),
        Target::Upstream(floating::Drop::Column(slot), _) => ("window.panel.dock", json!({"panel": first, "group": true, "column": slot})),
        Target::Upstream(_, _) => ("window.panel.dock", json!({"panel": first, "group": true})),
    };
    if let Err(error) = app.run(id, params) {
        app.ui.status = error;
    }
    // A card put back in the dock closes (its icon panels go back to their icons): it doesn't pop
    // out of its icon instead.
    app.ui.open_panel = match app.ui.open_panel.take() {
        Some(p) if panels.contains(&p) && !floating(app, &p) => None,
        Some(p) if id == "window.panel.float" => open.or(Some(p)),
        other => other,
    };
}

/// What a card's header asked for this frame.
enum Ask {
    /// Bring this tab to the front of its card.
    Activate(String),
    /// Close the card led by this panel.
    Close(String),
}

/// Draw every card: its frame, header (tabs, ×, ≡), its panel's body and its resize grips.
fn draw(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let screen = ctx.content_rect();
    let t = crate::theme::Tokens::get(ctx);
    let cards: Vec<(Vec<String>, usize, Rect)> = owned(app).map(|g| (g.panels.clone(), g.active, shown(g.rect, screen))).collect();
    let mut asks = Vec::new();
    for (panels, active, rect) in &cards {
        card(app, ctx, &t, panels, *active, *rect, &mut asks);
    }
    let open = app.ui.open_panel.clone();
    for ask in asks {
        match ask {
            Ask::Activate(panel) => {
                let action = serde_json::to_value(craft_ui::docking::Action::Activate { panel }).unwrap_or_default();
                if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.layout", &json!({ "action": action })) {
                    app.ui.status = error;
                }
                app.ui.open_panel.clone_from(&open);
            }
            Ask::Close(first) => close_card(app, &first),
        }
    }
}

/// Draw one card at `rect`: `panels` as its tabs, the `active` one's body under them.
fn card(app: &mut VectorcraftApp, ctx: &egui::Context, t: &crate::theme::Tokens, panels: &[String], active: usize, rect: Rect, asks: &mut Vec<Ask>) {
    let (Some(first), Some(front_panel)) = (panels.first(), panels.get(active).or(panels.first())) else { return };
    let id = card_id(first);
    let hint = app.ui.varos_panel_stack.hint.as_deref() == Some(first.as_str());
    let labels: Vec<String> = panels.iter().map(|p| label(app, p)).collect();
    egui::Area::new(id).order(egui::Order::Middle).fixed_pos(rect.min).movable(false).constrain(false).show(ctx, |ui| {
        ui.set_min_size(rect.size());
        let rect = Rect::from_min_size(ui.min_rect().min, rect.size());
        ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter().clone();
        let radius = CornerRadius::same(RADIUS);
        painter.rect_filled(rect, radius, t.panel);
        let head = header(rect);
        let menu = Rect::from_min_size(pos2(head.right() - PAD - BUTTON.x, head.center().y - BUTTON.y / 2.0), BUTTON);
        let close = menu.translate(vec2(-BUTTON.x - 2.0, 0.0));
        // The header moves the card, all of it but its × and ≡ (egui gives a press's click and its
        // drag to different widgets: a drag area under a button would move the card from it). Its
        // tabs lie on top and have their own drag (a tab moves the card or leaves it).
        let _ = ui.interact(Rect::from_min_max(head.min, pos2(close.left() - 1.0, head.bottom())), id.with("header"), Sense::drag());
        // Tabs from the left, as wide as their labels, narrower when they don't all fit.
        let font = egui::FontId::proportional(LABEL);
        let galleys: Vec<_> = labels.iter().map(|l| painter.layout_no_wrap(l.clone(), font.clone(), t.text)).collect();
        let room = (close.left() - 2.0 - head.left() - PAD).max(0.0);
        let wanted: f32 = galleys.iter().map(|g| g.size().x + 12.0).sum();
        let scale = if wanted > room { room / wanted.max(1.0) } else { 1.0 };
        let mut x = head.left() + PAD;
        for (i, (panel, galley)) in panels.iter().zip(&galleys).enumerate() {
            let tab = Rect::from_min_size(pos2(x, head.center().y - TAB_H / 2.0), vec2(((galley.size().x + 12.0) * scale).floor(), TAB_H));
            x = tab.right() + 2.0;
            let response = ui.interact(tab, tab_id(panel), Sense::click_and_drag());
            let front = i == active;
            // The tab in front is lit as the kit's toggles: a dark well, bright text, no azure.
            let text = if front {
                painter.rect(tab, CornerRadius::same(3), t.input, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                t.text_strong
            } else if response.hovered() {
                painter.rect_filled(tab, CornerRadius::same(3), t.hover);
                t.text
            } else {
                t.text_dim
            };
            let at = pos2(tab.left() + 6.0, tab.center().y - galley.size().y / 2.0);
            painter.with_clip_rect(tab.shrink2(vec2(3.0, 0.0)).intersect(painter.clip_rect())).galley(at, galley.clone(), text);
            if response.clicked() && !front {
                asks.push(Ask::Activate(panel.clone()));
            }
        }
        // × closes the card; ≡ is upstream's panel menu.
        let response = ui.interact(close, id.with("close"), Sense::click());
        if response.hovered() {
            painter.rect_filled(close, CornerRadius::same(3), t.hover);
        }
        crate::icons::paint(
            ui,
            "x",
            Rect::from_center_size(close.center(), vec2(16.0, 16.0)),
            if response.hovered() { t.text_strong } else { t.text_dim },
        );
        if response.on_hover_text(tl!("Close")).clicked() {
            asks.push(Ask::Close(first.clone()));
        }
        crate::panels::panel_menu(app, ui, front_panel, Rect::from_center_size(menu.center(), vec2(18.0, 18.0)));
        painter.hline((rect.left() + 1.0)..=(rect.right() - 1.0), head.bottom() - 0.5, Stroke::new(1.0, t.border));
        // The shown panel's body, inside the rounded corners.
        let body = Rect::from_min_max(pos2(rect.left() + 1.0, head.bottom()), pos2(rect.right() - 1.0, rect.bottom() - FOOT));
        if body.is_positive() {
            let mut child = ui.new_child(
                egui::UiBuilder::new().id_salt(("varos-card-body", front_panel)).max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(body.intersect(ui.clip_rect()));
            crate::panel_docking::panel_body(app, &mut child, front_panel);
        }
        // Resize grips, over the body's edges.
        let corner = Rect::from_min_max(rect.max - vec2(CORNER, CORNER), rect.max);
        let right = Rect::from_min_max(pos2(rect.right() - EDGE, head.bottom()), pos2(rect.right(), corner.top()));
        let bottom = Rect::from_min_max(pos2(rect.left() + RADIUS as f32, rect.bottom() - EDGE), pos2(corner.left(), rect.bottom()));
        for (zone, key, cursor) in [
            (right, "resize-x", egui::CursorIcon::ResizeHorizontal),
            (bottom, "resize-y", egui::CursorIcon::ResizeVertical),
            (corner, "resize", egui::CursorIcon::ResizeNwSe),
        ] {
            let grip = ui.interact(zone, id.with(key), Sense::drag());
            if grip.hovered() || grip.dragged() {
                ctx.set_cursor_icon(cursor);
            }
        }
        // Its 1-point line, over everything inside it.
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        if hint {
            // A card let go here joins this one.
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("varos-card-drop")));
            painter.rect(
                head.shrink(2.0),
                CornerRadius::same(3),
                t.accent.gamma_multiply(0.15),
                Stroke::new(1.5, t.accent),
                egui::StrokeKind::Inside,
            );
        }
        ctx.data_mut(|d| d.insert_temp(floating::group_rect_id(first), rect));
    });
}

/// Keep [`PanelStack::groups`] up to date with the cards: a card with several tabs is a group; a
/// panel that left its card (to another card, the dock) leaves its group; a panel closed keeps its
/// place in its group, so that it opens again with it. Only when the cards changed.
fn sync_groups(app: &mut VectorcraftApp) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for g in owned(app) {
        g.panels.hash(&mut hasher);
        g.active.hash(&mut hasher);
    }
    let signature = hasher.finish();
    if app.ui.varos_panel_stack.signature == signature {
        return;
    }
    let current: Vec<(Vec<String>, String)> =
        owned(app).filter_map(|g| Some((g.panels.clone(), g.panels.get(g.active).or(g.panels.first())?.clone()))).collect();
    let ui = &mut app.ui;
    let Some(layout) = ui.docking.as_ref() else { return };
    let stack = &mut ui.varos_panel_stack;
    stack.signature = signature;
    for (panels, active) in current {
        let k = stack.groups.iter().position(|m| m.panels.iter().any(|p| panels.contains(p)));
        for (j, m) in stack.groups.iter_mut().enumerate() {
            if Some(j) != k {
                m.panels.retain(|p| !panels.contains(p));
                m.parked.retain(|p| !panels.contains(p));
            }
        }
        match k {
            Some(k) => {
                let m = &mut stack.groups[k];
                let mut kept = panels.clone();
                for (i, p) in m.panels.iter().enumerate() {
                    if !panels.contains(p) && (!layout.contains(p) || m.parked.contains(p)) {
                        kept.insert(i.min(kept.len()), p.clone());
                    }
                }
                m.panels = kept;
                m.active = active;
                m.parked.retain(|p| !panels.contains(p));
            }
            None if panels.len() >= 2 => stack.groups.push(Group { panels, active, parked: Vec::new() }),
            None => {}
        }
    }
    // A parked tab floating elsewhere since left its group.
    for m in &mut stack.groups {
        let moved: Vec<String> = m.parked.iter().filter(|p| layout.floating.iter().any(|g| g.panels.contains(p))).cloned().collect();
        m.panels.retain(|p| !moved.contains(p));
        m.parked.retain(|p| !moved.contains(p) && m.panels.contains(p));
    }
    stack.groups.retain(|m| m.panels.len() >= 2);
}
