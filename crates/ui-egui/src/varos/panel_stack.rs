//! Icon panels open as independent cards (owner, 2026-10-20: "like Illustrator, each one on its
//! own, not in a fake side list that scrolls").
//!
//! With the dock in "keep open" mode ([`PanelStack::keep_open`], on by default) a click on an icon
//! of an icon column opens its panel as a card of its own, drawn by Varos inside the main window:
//! an 8-point card with a 1-point line, no shadow, a 32-point header holding its tabs, its × and
//! its panel menu (≡). The whole header moves the card, live under the pointer; its bottom-right
//! corner and its right and bottom edges resize it, live too. A tab dragged out of a card with
//! several tabs becomes a card of its own that follows the pointer; a card let go on another
//! card's header joins it as tabs; let go on a docked group's tabs it joins that group (and leaves
//! the icon column); let go on an icon column it closes (back to its icon).
//!
//! Since the 2026-10-26 upstream merge the dock is craft-ui's shared dock area, whose floating
//! panels are OS windows. A card is not one of them: its panels stay where the layout keeps them
//! (an iconic column, so their icons stay in place) and Varos keeps the card itself
//! ([`PanelStack::live`]). craft-ui's flyout for an icon is turned into a card
//! ([`flyout`]), and `window.panel` on an icon panel opens or shows its card ([`show`]).
//!
//! Where a new card goes ([`place`]): its right edge 6 points left of the dock, its top beside its
//! icon in the column (kept in the window), then down past the cards already there; when the
//! window's bottom is reached it cascades, 24 points left and down per step. A card closed and
//! opened again comes back where it was left, at the size it was given ([`PanelStack::places`]),
//! unless half of it would lie on another card (then it takes a new place); always left of the
//! dock, also when the window or the dock changed size since. Until it is resized by hand, a card
//! is as tall as its panel's contents (capped by the window): [`frame`] fits the card to them
//! (wider too when a row of the panel doesn't fit its default width).
//!
//! Panels put together as tabs of one card stay together ([`PanelStack::groups`]): the card closed
//! by its ×, any of its panels opened again opens the whole card, with the same tabs, the same tab
//! in front, where it was. A tab closed alone (by its icon) opens again in its card.
//!
//! A shortcut or an agent's `window.panel` on a card already open only shows it (it never closes
//! it); its icon and Window › that panel (`toggle`) close it. User Interface › Auto-Collapse
//! Iconic Panels makes pop-outs transient: with it on, the column goes back to upstream's
//! one-flyout-at-a-time behaviour, as with `keep_open` off.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use craft_ui::docking::Hit;
use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::VectorcraftApp;
use crate::state::ICON_PANELS;

/// Top of a new card when the icon column hasn't been drawn yet.
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
/// The icon column's width and the main column's (`panel_docking::COLUMN_WIDTH`), for a card
/// opened before the dock has been drawn.
const ICON_COL: f32 = crate::panel_docking::ICON_WIDTH;
const DOCK_WIDTH: f32 = crate::panel_docking::COLUMN_WIDTH;
/// A card's header (its tabs, × and ≡), as the kit's card (`panel-kit.html` §5).
const HEADER: f32 = 32.0;
/// A tab's height, and the room left and right of the header's contents.
const TAB_H: f32 = 24.0;
const PAD: f32 = 4.0;
/// A header button (× and ≡).
const BUTTON: Vec2 = vec2(20.0, 24.0);
/// A card's corners.
const RADIUS: u8 = 8;
/// The least size a card is drawn at.
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
/// Frames a card Varos placed is drawn unseen: the first measures it and places it at its real size,
/// the second lets contents that settle a frame late (a panel's second layout) fit it again.
const UNSEEN: u8 = 2;
/// The cards' coordinates version: 2 since the dock became craft-ui's shared dock area (the
/// 2026-10-26 merge). Places saved before were measured against the old dock.
pub(crate) const VERSION: u32 = 2;

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

/// A card open now: its tabs, the one in front and where it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    /// The tabs, in order (never empty).
    pub panels: Vec<String>,
    /// The tab in front.
    #[serde(default)]
    pub active: usize,
    /// `[x, y, width, height]` in window points.
    pub rect: [f32; 4],
}

impl Card {
    fn first(&self) -> &str {
        self.panels.first().map_or("", String::as_str)
    }

    /// The panel shown (in front).
    fn front(&self) -> &str {
        self.panels.get(self.active).or(self.panels.first()).map_or("", String::as_str)
    }

    fn area(&self) -> Rect {
        let [x, y, w, h] = self.rect;
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    fn set_area(&mut self, r: Rect) {
        if r.is_finite() && r.width() > 0.0 && r.height() > 0.0 {
            self.rect = [r.left(), r.top(), r.width(), r.height()];
        }
    }
}

/// The icon panels opened as cards, kept with the UI state (`ui.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelStack {
    /// Preferences › User Interface › Panel cards (Varos), on by default. Off: the card layer is
    /// inert and the icon columns behave exactly as upstream's (flyouts, panels dragged out to
    /// floating windows); open cards close and the newest shows as its column's flyout.
    pub enabled: bool,
    /// Icon clicks open their panel as a card and leave the others open (Varos default). Off:
    /// upstream's one flyout at a time.
    pub keep_open: bool,
    /// The cards' coordinates version ([`VERSION`]). Files saved before the 2026-10-26 merge's
    /// dock (none, or older) have their places dropped and their open cards placed again beside
    /// their icons once.
    #[serde(default)]
    pub version: u32,
    /// The panels open as cards, in the order they were opened.
    pub cards: Vec<String>,
    /// The cards open now, each with its tabs and its place.
    pub live: Vec<Card>,
    /// Where each panel's card was last.
    pub places: BTreeMap<String, Place>,
    /// Panels put together as tabs of one card, remembered when it closes: any of them opened
    /// again opens the whole card.
    pub groups: Vec<Group>,
    /// Per card (by its first panel): the size it was fitted to, the contents' size then and how
    /// many panels the card had. Not saved.
    #[serde(skip)]
    fitted: BTreeMap<String, Fitted>,
    /// The window and the dock as drawn last. Not saved.
    #[serde(skip)]
    view: Option<View>,
    /// A card was opened or changed outside a frame's input: draw again. Not saved.
    #[serde(skip)]
    repaint: bool,
    /// The workspace generation (`UiState::docking_generation`) the cards belong to. Not saved.
    #[serde(skip)]
    generation: Option<u64>,
    /// The top of each icon in the columns, as drawn last. Not saved.
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
    /// The cards painted last frame (by first panel) and where. Not saved.
    #[serde(skip)]
    painted: Vec<(String, Rect)>,
    /// What was loaded was checked against the dock as drawn ([`migrate`], [`drop_lost_places`]).
    /// Not saved.
    #[serde(skip)]
    loaded: bool,
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
    /// follows the icon if the column is laid out anew.
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
    /// Kept for files saved before the 2026-10-26 merge (dock tabs grouped into a card); unused.
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
            enabled: true,
            keep_open: true,
            version: VERSION,
            cards: Vec::new(),
            live: Vec::new(),
            places: BTreeMap::new(),
            fitted: BTreeMap::new(),
            view: None,
            repaint: false,
            generation: None,
            groups: Vec::new(),
            icons: Vec::new(),
            gesture: None,
            hint: None,
            fresh: Vec::new(),
            signature: 0,
            painted: Vec::new(),
            loaded: false,
            #[cfg(test)]
            cost_ms: 0.0,
        }
    }
}

/// What is saved: frame-to-frame bookkeeping doesn't make the UI state differ.
impl PartialEq for PanelStack {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.keep_open == other.keep_open
            && self.version == other.version
            && self.cards == other.cards
            && self.live == other.live
            && self.places == other.places
            && self.groups == other.groups
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

/// `p` moved so that a `size` box at it lies inside `room` (its top-left corner kept on screen
/// when it is larger than the room).
pub(crate) fn clamp(p: [f32; 2], size: Vec2, room: Rect) -> Pos2 {
    let x = p[0].min(room.right() - size.x).max(room.left());
    let y = p[1].min(room.bottom() - size.y).max(room.top());
    let pos = pos2(x, y);
    if pos.is_finite() { pos } else { room.min }
}

/// `size` made to fit in `room` (a card never larger than the room left of the dock; what doesn't
/// fit scrolls), at least [`MIN_TALL`] high when the room allows.
fn bounded(size: Vec2, room: Rect) -> Vec2 {
    vec2(size.x.clamp(1.0, room.width().max(1.0)), size.y.clamp(MIN_TALL.min(room.height()), room.height().max(1.0)))
}

/// The Preferences dialog's field for Panel cards (Varos) (User Interface, under Auto-Collapse
/// Iconic Panels), and its label.
pub(crate) const PREF_KEY: &str = "__varosPanelCards";
pub(crate) const PREF_LABEL: &str = "Panel cards (Varos)";

/// Preferences › Panel cards (Varos) set to `on`. Off, the cards close at the next frame and the
/// newest shows as its icon column's flyout (upstream's own state); on again, that flyout is a
/// card again. The docking layout itself is never changed by the switch.
pub(crate) fn set_enabled(app: &mut VectorcraftApp, on: bool) {
    if app.ui.varos_panel_stack.enabled != on {
        app.ui.varos_panel_stack.enabled = on;
        app.ui.varos_panel_stack.repaint = true;
    }
}

/// Is the card mode on: keep-open, and pop-outs not made transient by Auto-Collapse.
fn active(app: &VectorcraftApp) -> bool {
    let stack = &app.ui.varos_panel_stack;
    stack.enabled && stack.keep_open && !app.session.prefs.auto_collapse_icon_panels
}

/// Panel `id` as the `'static` id of an icon panel, if it is one.
fn rail_panel(id: &str) -> Option<&'static str> {
    ICON_PANELS.iter().find(|e| e.0 == id).map(|e| e.0)
}

/// Can panel `id` open as a card now: card mode is on and it is an icon panel kept in a column
/// collapsed to icons.
fn cardable(app: &VectorcraftApp, id: &str) -> Option<&'static str> {
    let id = rail_panel(id).filter(|_| active(app))?;
    crate::panel_docking::in_iconic_column(&crate::panel_docking::layout_of(&app.ui), id).then_some(id)
}

/// The card holding panel `id`, by index.
fn card_index(app: &VectorcraftApp, id: &str) -> Option<usize> {
    app.ui.varos_panel_stack.live.iter().position(|c| c.panels.iter().any(|p| p == id))
}

/// The card led by `first`, by index.
fn led_by(app: &VectorcraftApp, first: &str) -> Option<usize> {
    app.ui.varos_panel_stack.live.iter().position(|c| c.first() == first)
}

/// Is panel `id` open in a card (its icon lit)?
pub(crate) fn in_card(app: &VectorcraftApp, id: &str) -> bool {
    active(app) && card_index(app, id).is_some()
}

/// Is panel `id` open (its icon lit): in its flyout, or in a card.
pub(crate) fn is_open(app: &VectorcraftApp, id: &str) -> bool {
    app.ui.open_panel.as_deref() == Some(id) || in_card(app, id)
}

/// `cards` follows the live cards (their panels, in the order they were opened).
fn sync_cards(app: &mut VectorcraftApp) {
    let stack = &mut app.ui.varos_panel_stack;
    let open: Vec<String> = stack.live.iter().flat_map(|c| c.panels.iter().cloned()).collect();
    stack.cards.retain(|p| open.contains(p));
    for p in open {
        if !stack.cards.contains(&p) {
            stack.cards.push(p);
        }
    }
}

/// An icon click (or Window › that panel) on panel `id`. In card mode an icon panel opens as a
/// card, or its card closes; otherwise upstream's swap (show its flyout in place of the one shown,
/// or put it away).
pub(crate) fn toggle(app: &mut VectorcraftApp, id: &str) {
    let Some(id) = cardable(app, id) else {
        app.ui.open_panel = if app.ui.open_panel.as_deref() == Some(id) { None } else { Some(id.to_string()) };
        return;
    };
    if card_index(app, id).is_some() {
        close(app, id);
    } else {
        open(app, id);
    }
}

/// `window.panel {panel}` without `toggle` (a shortcut, an agent, a Properties button): an icon
/// panel's card opens, or comes to the front of its card when open. `None` when it isn't one.
pub(crate) fn show(app: &mut VectorcraftApp, id: &str) -> Option<serde_json::Value> {
    let id = cardable(app, id)?;
    match card_index(app, id) {
        Some(k) => {
            let card = &mut app.ui.varos_panel_stack.live[k];
            card.active = card.panels.iter().position(|p| p == id).unwrap_or(card.active);
            app.ui.varos_panel_stack.repaint = true;
            app.ui.open_panel = Some(id.to_string());
        }
        None => {
            open(app, id);
        }
    }
    app.ui.dock = true;
    Some(json!({"panel": id, "card": true}))
}

/// craft-ui opened (`Some`) or closed the flyout of an icon column: in card mode an icon panel's
/// flyout becomes its card (or its card closes, the icon clicked again). → the open panel to keep
/// (`UiState::open_panel`: the newest card, or a flyout).
pub(crate) fn flyout(app: &mut VectorcraftApp, panel: Option<String>) -> Option<String> {
    match panel.as_deref().and_then(|p| cardable(app, p)) {
        Some(p) => {
            toggle(app, p);
            app.ui.open_panel.clone()
        }
        // A flyout put away by craft-ui: the newest card stays the open panel.
        None if panel.is_none() => app.ui.open_panel.clone().filter(|p| in_card(app, p)),
        None => panel,
    }
}

/// The dock area is about to open the flyout of `panel` (`UiState::open_panel`): in card mode an
/// icon panel is a card instead (opened now if it isn't one yet: a Properties button, a workspace).
/// True when the dock area opens no flyout for it.
pub(crate) fn takes_flyout(app: &mut VectorcraftApp, panel: &str) -> bool {
    let Some(p) = cardable(app, panel) else { return false };
    if card_index(app, p).is_none() {
        open(app, p);
    }
    true
}

/// craft-ui's flyout layer (its `FloatingFrame` of `area.with("flyout")`).
fn flyout_layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, crate::panel_docking::area_id().with("flyout"))
}

/// Is craft-ui's flyout open this frame for an icon panel that is a card instead? craft-ui opens
/// the flyout in the frame of the icon's click, before its event reaches Varos, and draws its
/// group's front panel (often another panel than the one clicked) beside the icon for that frame.
fn flyout_is_a_card(app: &VectorcraftApp, ctx: &egui::Context) -> bool {
    craft_ui::docking::flyout::<String>(ctx, crate::panel_docking::area_id()).is_some_and(|p| cardable(app, &p).is_some())
}

/// The dock area asks for panel bodies: none in a flyout that is a card instead (its widgets would
/// be drawn twice, once there and once in the card).
pub(crate) fn skips_body(app: &VectorcraftApp, ui: &egui::Ui) -> bool {
    ui.layer_id() == flyout_layer() && flyout_is_a_card(app, ui.ctx())
}

/// After the dock area drew: a flyout that is a card instead is wiped from this frame's picture,
/// so the card appears alone (no flash of a flyout beside the icon first).
pub(crate) fn wipe_flyout(app: &VectorcraftApp, ctx: &egui::Context) {
    if !flyout_is_a_card(app, ctx) {
        return;
    }
    ctx.graphics_mut(|g| {
        let list = g.entry(flyout_layer());
        for i in 0..list.next_idx().0 {
            list.reset_shape(egui::layers::ShapeIdx(i));
        }
    });
}

/// After cards closed: upstream's open panel follows the newest card still open.
fn follow_open_panel(app: &mut VectorcraftApp) {
    let open = app.ui.open_panel.clone();
    if open.as_deref().is_none_or(|p| rail_panel(p).is_some() && card_index(app, p).is_none()) {
        app.ui.open_panel = app.ui.varos_panel_stack.cards.last().cloned();
    }
}

/// Open panel `id` as a card: where it was left, or at the next free place beside its icon; with
/// the panels it was grouped with as tabs (in their card when it is open).
fn open(app: &mut VectorcraftApp, id: &'static str) -> bool {
    let group = app.ui.varos_panel_stack.groups.iter().find(|g| g.panels.iter().any(|p| p == id)).cloned();
    if let Some(done) = group.and_then(|g| reopen(app, id, &g)) {
        return done;
    }
    let (rect, fresh) = spot(app, id);
    let mut card = Card { panels: vec![id.to_string()], active: 0, rect: [0.0; 4] };
    card.set_area(rect);
    app.ui.varos_panel_stack.live.push(card);
    opened(app, id, id, fresh);
    true
}

/// For their first frames, cards placed beside their icon follow it if the icon column was laid
/// out anew. True when a card moved.
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
        let Some(k) = led_by(app, first) else { continue };
        let card = &mut app.ui.varos_panel_stack.live[k];
        let r = card.area();
        card.set_area(Rect::from_min_size(clamp([r.left(), r.top() + dy], r.size(), room), r.size()));
    }
    !shifts.is_empty()
}

/// The card led by `lead` was opened for `id`: its panels are cards now. `fresh`: placed by Varos
/// beside `id`'s icon, whose top was then this.
fn opened(app: &mut VectorcraftApp, lead: &str, id: &'static str, fresh: Option<Option<f32>>) {
    sync_cards(app);
    let stack = &mut app.ui.varos_panel_stack;
    stack.cards.retain(|c| c != id);
    stack.cards.push(id.to_string());
    stack.fitted.remove(lead);
    stack.fresh.retain(|f| f.first != lead);
    if let Some(icon) = fresh {
        stack.fresh.push(Fresh { first: lead.to_string(), icon: icon.map(|top| (id, top)), age: 0 });
    }
    stack.repaint = true;
    // Upstream's open panel is the newest card (code and agents that read "the open panel" see the
    // one just opened); the dock area opens no flyout for it ([`no_flyout`]).
    app.ui.open_panel = Some(id.to_string());
}

/// Open panel `id` with the panels it was grouped with (`group`): as a tab of their card when one
/// of them is open, else as their card again, its tabs in their order with the same one in front.
/// None when nothing of the group is left to open with it (it opens alone).
fn reopen(app: &mut VectorcraftApp, id: &'static str, group: &Group) -> Option<bool> {
    if let Some(k) = group.panels.iter().filter(|p| *p != id).find_map(|p| card_index(app, p)) {
        // At its place among the tabs: before the next of its group's tabs in the card. In front,
        // as it was asked for.
        let at = group.panels.iter().position(|p| p == id).unwrap_or(0);
        let card = &mut app.ui.varos_panel_stack.live[k];
        let next = group.panels[at + 1..].iter().find_map(|p| card.panels.iter().position(|q| q == p));
        let i = next.unwrap_or(card.panels.len());
        card.panels.insert(i, id.to_string());
        card.active = i;
        let lead = card.first().to_string();
        opened(app, &lead, id, None);
        return Some(true);
    }
    // Its group's panels that can open with it (not opened elsewhere since).
    let members: Vec<&'static str> = group.panels.iter().filter_map(|p| cardable(app, p)).filter(|p| card_index(app, p).is_none()).collect();
    if members.len() < 2 || !members.contains(&id) {
        return None;
    }
    let (rect, fresh) = spot(app, id);
    let active = members.iter().position(|p| *p == group.active).or_else(|| members.iter().position(|p| *p == id)).unwrap_or(0);
    let mut card = Card { panels: members.iter().map(|p| p.to_string()).collect(), active, rect: [0.0; 4] };
    card.set_area(rect);
    let lead = card.first().to_string();
    app.ui.varos_panel_stack.live.push(card);
    opened(app, &lead, id, fresh);
    Some(true)
}

/// Close panel `id`'s card (the panel only, when other panels share its card).
fn close(app: &mut VectorcraftApp, id: &str) {
    remember(app);
    sync_groups(app);
    app.ui.varos_panel_stack.signature = 0;
    if let Some(k) = card_index(app, id) {
        let stack = &mut app.ui.varos_panel_stack;
        let card = &mut stack.live[k];
        let front = card.front().to_string();
        card.panels.retain(|p| p != id);
        if card.panels.is_empty() {
            stack.live.remove(k);
        } else {
            card.active = card.panels.iter().position(|p| *p == front).unwrap_or(0).min(card.panels.len() - 1);
        }
    }
    sync_cards(app);
    follow_open_panel(app);
    app.ui.varos_panel_stack.repaint = true;
}

/// The card led by `first` closes (its ×): its panels go back to their icons. Its grouping is
/// remembered.
fn close_card(app: &mut VectorcraftApp, first: &str) {
    if led_by(app, first).is_none() {
        return;
    }
    remember(app);
    sync_groups(app);
    // Brought up to date again when its card opens again (the same tabs as now).
    app.ui.varos_panel_stack.signature = 0;
    if let Some(k) = led_by(app, first) {
        app.ui.varos_panel_stack.live.remove(k);
    }
    sync_cards(app);
    follow_open_panel(app);
    app.ui.varos_panel_stack.repaint = true;
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

/// A new card's width for panel `id`: the Color panel is the 240 pt colour picker v3 and the
/// panel's 10 pt margins; other icon panels are 256 points.
fn panel_width(id: &str) -> f32 {
    if id == crate::varos::picker::PANEL { crate::varos::picker::W + 20.0 } else { 256.0 }
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
    let size = saved.map_or(vec2(panel_width(id), START_TALL), |p| vec2(p.size[0], p.size[1]));
    let size = bounded(vec2(size.x, size.y.min(tallest)), room);
    let taken = taken(app, None);
    let back = saved.map(|p| Rect::from_min_size(clamp(p.pos, size, room), size)).filter(|r| taken.iter().all(|t| overlap(*r, *t) < 0.5));
    if let Some(back) = back {
        return (back, None);
    }
    // Its top beside its icon, kept where a card can still show in the window (its height is fitted
    // once its contents are known, and it moves up then if it must).
    let icon = stack.icons.iter().find(|e| e.0 == id).map(|e| e.1).filter(|y| y.is_finite());
    let top = icon.unwrap_or(v.top).min(room.bottom() - MARGIN - MIN_TALL).max(room.top());
    (Rect::from_min_size(place(size, &taken, room, v.right, top), size), Some(icon))
}

/// The cards' bounds (but the one led by `except`).
fn taken(app: &VectorcraftApp, except: Option<&str>) -> Vec<Rect> {
    let screen = view(app).screen;
    app.ui.varos_panel_stack.live.iter().filter(|c| Some(c.first()) != except).map(|c| shown(c.rect, screen)).collect()
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
    let step = steps.map(|k| clamp([x - CASCADE * k as f32, top + CASCADE * k as f32], size, screen)).find(|p| !near(*p));
    step.unwrap_or_else(|| clamp([x, top], size, screen))
}

/// Keep where each card is (and its size) as the place its panels open at next time.
fn remember(app: &mut VectorcraftApp) {
    let stack = &mut app.ui.varos_panel_stack;
    for c in &stack.live {
        let [x, y, w, h] = c.rect;
        if ![x, y, w, h].iter().all(|v| v.is_finite()) {
            continue;
        }
        for p in &c.panels {
            let fit = stack.places.get(p).is_none_or(|x| x.fit);
            let place = Place { pos: [x, y], size: [w, h], fit };
            if stack.places.get(p) != Some(&place) {
                stack.places.insert(p.clone(), place);
            }
        }
    }
}

/// Once a frame (after the dock area was drawn): turn other ways of showing an icon panel into
/// cards, keep the cards left of the dock, keep each card's place and fit cards to their
/// contents, and draw them.
pub(crate) fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let generation = app.ui.docking_generation;
    let first_frame = app.ui.varos_panel_stack.generation.is_none();
    let workspace = app.ui.varos_panel_stack.generation.is_some_and(|g| g != generation);
    app.ui.varos_panel_stack.generation = Some(generation);
    app.ui.varos_panel_stack.painted.clear();
    if first_frame {
        // What was loaded keeps only icon panels.
        let stack = &mut app.ui.varos_panel_stack;
        stack.places.retain(|p, _| rail_panel(p).is_some());
        for g in &mut stack.groups {
            g.panels.retain(|p| rail_panel(p).is_some());
            g.parked.clear();
        }
        stack.groups.retain(|g| g.panels.len() >= 2);
    }
    let card_mode = active(app);
    if !card_mode {
        // Panel cards off (or Auto-Collapse, or keep-open off): upstream's icon columns alone.
        leave(app);
        return;
    }
    if first_frame {
        // A UI state saved before the cards were kept (`live`): its cards open again.
        let stack = &mut app.ui.varos_panel_stack;
        if stack.live.is_empty() && !stack.cards.is_empty() {
            let cards = std::mem::take(&mut stack.cards);
            for p in cards {
                if let Some(p) = cardable(app, &p)
                    && card_index(app, p).is_none()
                {
                    open(app, p);
                }
            }
        }
    }
    if workspace {
        // A workspace was applied: it brings its own panels; cards whose panels left their icon
        // columns close.
        let stack = &mut app.ui.varos_panel_stack;
        stack.fitted.clear();
        stack.fresh.clear();
        stack.gesture = None;
        stack.hint = None;
    }
    // Cards hold icon panels kept in an icon column: a panel moved elsewhere (dragged into the
    // dock, floated) leaves its card.
    let layout = crate::panel_docking::layout_of(&app.ui);
    let stack = &mut app.ui.varos_panel_stack;
    if !stack.live.is_empty() {
        for c in &mut stack.live {
            let front = c.front().to_string();
            c.panels.retain(|p| rail_panel(p).is_some() && crate::panel_docking::in_iconic_column(&layout, p));
            c.active = c.panels.iter().position(|p| *p == front).unwrap_or(0);
        }
        stack.live.retain(|c| !c.panels.is_empty());
    }
    sync_cards(app);
    let screen = ctx.content_rect();
    if !(screen.width() > 1.0 && screen.height() > 1.0) {
        // A minimized window: nothing to follow.
        return;
    }
    let moved = measure(app, ctx);
    if !app.ui.varos_panel_stack.loaded && !app.ui.varos_panel_stack.icons.is_empty() {
        // The dock has been drawn: what was loaded is checked against it once.
        app.ui.varos_panel_stack.loaded = true;
        if app.ui.varos_panel_stack.version < VERSION {
            migrate(app);
        }
        drop_lost_places(app);
    }
    // A flyout opened another way (a Properties button, a workspace): a card.
    if let Some(p) = app.ui.open_panel.clone().as_deref().and_then(|p| cardable(app, p))
        && app.ui.dock
        && app.ui.screen_mode < 3
        && card_index(app, p).is_none()
    {
        open(app, p);
    }
    let shown = app.ui.dock && app.ui.screen_mode < 3 && !app.ui.varos_panel_stack.live.is_empty();
    if shown {
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
    if !app.ui.varos_panel_stack.live.is_empty() {
        repaint |= follow_icons(app);
        let mut moved_cards = false;
        // The window or dock changed, or a card's move or resize was let go: back into the room.
        if moved || ctx.input(|i| i.pointer.any_released()) {
            moved_cards |= keep_in_room(app);
        }
        moved_cards |= fit(app, ctx);
        repaint |= moved_cards;
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

/// A UI state saved before the cards' coordinates version [`VERSION`] (the dock before the
/// 2026-10-26 merge): its places were measured against another dock, so they are dropped, and its
/// open cards are placed again beside their icons, as new cards. Once the dock has been drawn.
fn migrate(app: &mut VectorcraftApp) {
    let stack = &mut app.ui.varos_panel_stack;
    stack.version = VERSION;
    stack.places.clear();
    stack.fitted.clear();
    stack.fresh.clear();
    let cards = std::mem::take(&mut stack.live);
    for mut card in cards {
        let Some(id) = card.panels.get(card.active).or(card.panels.first()).and_then(|p| rail_panel(p)) else { continue };
        let (rect, fresh) = spot(app, id);
        card.set_area(rect);
        let lead = card.first().to_string();
        app.ui.varos_panel_stack.live.push(card);
        let stack = &mut app.ui.varos_panel_stack;
        stack.fresh.push(Fresh { first: lead, icon: fresh.flatten().map(|top| (id, top)), age: 0 });
    }
    app.ui.varos_panel_stack.repaint = true;
}

/// Places that would show less than half of their card in the room (a window made smaller since,
/// a file from another screen) are forgotten: their panels open beside their icons again.
fn drop_lost_places(app: &mut VectorcraftApp) {
    let Some(v) = app.ui.varos_panel_stack.view else { return };
    let room = v.room();
    app.ui.varos_panel_stack.places.retain(|_, p| {
        let r = Rect::from_min_size(pos2(p.pos[0], p.pos[1]), vec2(p.size[0], p.size[1]));
        r.is_finite() && r.is_positive() && room.intersect(r).area() >= r.area() * 0.5
    });
}

/// Card mode was left (Auto-Collapse Iconic Panels on, or keep-open off): the cards close and the
/// newest shows as its icon column's flyout instead, as upstream's single transient pop-out.
fn leave(app: &mut VectorcraftApp) {
    if app.ui.varos_panel_stack.live.is_empty() {
        return;
    }
    remember(app);
    sync_groups(app);
    let newest = app.ui.varos_panel_stack.cards.last().cloned();
    let stack = &mut app.ui.varos_panel_stack;
    stack.live.clear();
    stack.cards.clear();
    stack.gesture = None;
    stack.hint = None;
    stack.fresh.clear();
    stack.fitted.clear();
    app.ui.open_panel = newest;
}

/// Keep the window and the dock as drawn this frame (from the dock area's controls). True when
/// the window or the dock changed since (or this is the first frame).
fn measure(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    let screen = ctx.content_rect();
    let shown = app.ui.dock && app.ui.screen_mode < 3;
    let layout = crate::panel_docking::layout_of(&app.ui);
    let floats = |p: &String| layout.floating.iter().any(|w| w.contains(p));
    let mut column: Option<Rect> = None;
    let mut dock_left: Option<f32> = None;
    let mut icons: Vec<(&'static str, f32)> = Vec::new();
    for (hit, r) in crate::panel_docking::hit_rects(ctx) {
        if !r.is_finite() || r.left() <= screen.left() {
            continue;
        }
        match &hit {
            Hit::Icon(p) | Hit::IconGrip(p) => {
                column = Some(column.map_or(r, |c| c.union(r)));
                if let (Hit::Icon(_), Some(id)) = (&hit, rail_panel(p)) {
                    icons.push((id, r.top()));
                }
            }
            // The docked groups' controls (floating windows' title, × and grips aren't the dock).
            // (An icon column's flyout isn't the dock either.)
            Hit::Tab(p) | Hit::DragBar(p) | Hit::Border(p) | Hit::Overflow(p) | Hit::ColumnBar(p) | Hit::Collapse(p) | Hit::StackToggle(p)
                if p != crate::panel_docking::CANVAS && !floats(p) && !crate::panel_docking::in_iconic_column(&layout, p) =>
            {
                dock_left = Some(dock_left.map_or(r.left(), |x: f32| x.min(r.left())));
            }
            _ => {}
        }
    }
    let right = match column.filter(|_| shown) {
        Some(c) => dock_left.map_or(c.left(), |x| x.min(c.left())),
        None if shown => screen.right() - ICON_COL - if crate::panel_docking::dock_collapsed(&app.ui) { 0.0 } else { DOCK_WIDTH },
        None => screen.right(),
    } - GAP;
    let top = column.filter(|_| shown).map_or(TOP, |c| c.top());
    // Cards stay in the dock area: under the app and control bars and the document tabs, above
    // the status bars (the dock area ends there).
    let mut screen = screen;
    if let Some(area) =
        craft_ui::docking::float_space::<String>(ctx, crate::panel_docking::area_id()).map(|s| s.area).filter(|a| a.is_finite() && a.is_positive())
    {
        screen = screen.intersect(area);
    }
    if let Some(tabs) = tab_strip(ctx).filter(|t| t.bottom() > screen.top() && t.bottom() < screen.bottom() - MIN_TALL) {
        screen.min.y = tabs.bottom();
    }
    let top = top.max(screen.top());
    let stack = &mut app.ui.varos_panel_stack;
    // The icons' tops, updated in place (no allocation once every icon is known).
    for (id, y) in icons {
        match stack.icons.iter_mut().find(|e| e.0 == id) {
            Some(e) => e.1 = y,
            None => stack.icons.push((id, y)),
        }
    }
    let view = View { screen, right, top };
    stack.view.replace(view) != Some(view)
}

/// Note where the document tabs were drawn this frame (`chrome::doc_tabs`): cards stay under them.
pub(crate) fn note_tab_strip(ctx: &egui::Context, strip: Rect) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("varos-card-tab-strip"), (strip, pass)));
}

/// The document tabs as drawn this frame, if they were.
fn tab_strip(ctx: &egui::Context) -> Option<Rect> {
    let (strip, pass) = ctx.data(|d| d.get_temp::<(Rect, u64)>(egui::Id::unique("varos-card-tab-strip")))?;
    (pass == ctx.cumulative_pass_nr() && strip.is_finite()).then_some(strip)
}

/// Cards reaching over the dock (or out of the window) move back into the room left of it, made
/// smaller only when larger than the room. True when one moved.
fn keep_in_room(app: &mut VectorcraftApp) -> bool {
    let Some(v) = app.ui.varos_panel_stack.view else { return false };
    // A point or two over (a dock edge that moved by a rounding) is left alone.
    let room = v.room().expand(SLACK);
    let mut moved = false;
    for c in &mut app.ui.varos_panel_stack.live {
        let [x, y, w, h] = c.rect;
        let size = bounded(vec2(w, h), room);
        let to = clamp([x, y], size, room);
        if (to - pos2(x, y)).length() > 0.5 || (size - vec2(w, h)).length() > 0.5 {
            c.set_area(Rect::from_min_size(to, size));
            moved = true;
        }
    }
    moved
}

/// Fit each card to its shown panel's contents, until it is resized by hand. A size is fitted once
/// per size of the contents, so contents that follow the card's height (a list filling it) leave
/// it as it is. True when a card changed.
fn fit(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    let Some(v) = app.ui.varos_panel_stack.view else { return false };
    let mut resized: Vec<String> = Vec::new();
    let mut fitted: Vec<(String, Fitted)> = Vec::new();
    let mut changed = false;
    // One card after the other, each fitted card in place before the next is placed (a card placed
    // under another that was just made shorter goes right under it).
    for k in 0..app.ui.varos_panel_stack.live.len() {
        let c = app.ui.varos_panel_stack.live[k].clone();
        let first = c.first().to_string();
        let shown_panel = c.front();
        let stack = &app.ui.varos_panel_stack;
        let [x, y, w, h] = c.rect;
        let before = stack.fitted.get(&first).filter(|f| f.panels == c.panels.len());
        if let Some(f) = before
            && ((h - f.size.y).abs() > 0.5 || (w - f.size.x).abs() > 0.5)
        {
            // Resized by hand: it keeps that size from now on.
            resized.push(first.clone());
            continue;
        }
        if stack.places.get(shown_panel).is_some_and(|p| !p.fit) {
            continue;
        }
        let Some((content, viewport)) = note(ctx, shown_panel) else { continue };
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
        let tall = match stack.fresh.iter().find(|f| f.first == first) {
            Some(f) => {
                let tall = wanted.min((floor - v.top.min(y).max(v.screen.top())).max(MIN_TALL));
                if before.is_none() {
                    // Its first fitting: placed again beside its icon at its real size.
                    let others = taken(app, Some(&first));
                    let room = v.room();
                    let start = f.icon.map_or(y, |(_, top)| top).min(room.bottom() - MARGIN - MIN_TALL).max(room.top());
                    at = place(vec2(wide, tall), &others, room, v.right, start);
                }
                at.y = at.y.min(floor - tall).max(v.screen.top());
                tall
            }
            None => wanted.min((floor - y.max(v.screen.top())).max(MIN_TALL)),
        };
        let moved = (tall - h).abs() > 1.0 || (wide - w).abs() > 0.5 || (at - pos2(x, y)).length() > 0.5;
        let size = if moved { vec2(wide, tall) } else { vec2(w, h) };
        fitted.push((first, Fitted { size, content, panels: c.panels.len() }));
        if moved {
            app.ui.varos_panel_stack.live[k].set_area(Rect::from_min_size(at, size));
            changed = true;
        }
    }
    let stack = &mut app.ui.varos_panel_stack;
    for first in resized {
        if let Some(c) = stack.live.iter().find(|c| c.first() == first) {
            for p in &c.panels {
                if let Some(place) = stack.places.get_mut(p) {
                    place.fit = false;
                }
            }
        }
        stack.fitted.remove(&first);
    }
    stack.fitted.extend(fitted);
    // Cards gone, or now led by another panel, are fitted afresh.
    let live: Vec<String> = stack.live.iter().map(|c| c.first().to_string()).collect();
    stack.fitted.retain(|first, _| live.contains(first));
    stack.fresh.retain(|f| live.contains(&f.first));
    changed
}

/// Note that the body of a card's panel was drawn this frame: its contents `content` big in a
/// scroll area `viewport` big.
fn note_body(ctx: &egui::Context, panel: &str, content: Vec2, viewport: Vec2) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(("varos-card-body", panel)), (content, viewport, pass)));
}

/// The note [`note_body`] left this frame for panel `id`: its contents' and scroll area's sizes.
fn note(ctx: &egui::Context, id: &str) -> Option<(Vec2, Vec2)> {
    let (content, viewport, pass) = ctx.data(|d| d.get_temp::<(Vec2, Vec2, u64)>(egui::Id::unique(("varos-card-body", id))))?;
    (pass == ctx.cumulative_pass_nr() && content.is_finite() && viewport.is_finite()).then_some((content, viewport))
}

/// The egui area of the card led by `first`. Varos' own: craft-ui's floating windows use
/// `area.with(("floating", first))`, and a card sharing that id shared its area state (fade-in,
/// size, order) with any floating window of the same first panel.
pub(crate) fn card_id(first: &str) -> egui::Id {
    egui::Id::unique("varos-card").with(first)
}

/// The room cards stay in (left of the dock, under the document tabs, above the status bars), as
/// last measured.
#[cfg(test)]
pub(crate) fn room(app: &VectorcraftApp) -> Rect {
    view(app).room()
}

/// Where the card led by `first` was painted last frame, if it was (shown, not hidden for its
/// first measuring frame).
#[cfg(test)]
pub(crate) fn painted(app: &VectorcraftApp, first: &str) -> Option<Rect> {
    app.ui.varos_panel_stack.painted.iter().find(|(p, _)| p == first).map(|(_, r)| *r)
}

/// Panel `panel`'s tab in a card's header.
fn tab_id(panel: &str) -> egui::Id {
    egui::Id::unique("varos-card-tab").with(panel)
}

/// Where a card `rect` (`[x, y, width, height]`) is drawn in `screen`: at least the least card
/// size, at most the window, inside the window.
fn shown(rect: [f32; 4], screen: Rect) -> Rect {
    let least = vec2(MIN_W.min(screen.width()), MIN_H.min(screen.height()));
    let size = vec2(rect[2], rect[3]).max(least).min(screen.size());
    Rect::from_min_size(clamp([rect[0], rect[1]], size, screen), size)
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

/// Put the card led by `first` at `rect` now, so that it is drawn there this very frame.
fn set_rect(app: &mut VectorcraftApp, first: &str, rect: Rect) {
    if let Some(k) = led_by(app, first) {
        app.ui.varos_panel_stack.live[k].set_area(rect);
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
            set_rect(app, &first, Rect::from_min_size(clamp([to.x, to.y], original.size(), room), original.size()));
            if !down {
                app.ui.varos_panel_stack.gesture = None;
                if (pos - from).length() >= TRAVEL {
                    let_go(app, ctx, &first, pos);
                }
            } else if (pos - from).length() >= TRAVEL {
                // Light up where a let-go here would put it.
                match target(app, ctx, &first, pos) {
                    Some(Target::Card(onto)) => app.ui.varos_panel_stack.hint = Some(onto),
                    Some(Target::Dock(_, zone) | Target::Icons(zone)) => preview(ctx, zone),
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
            let at = clamp([pos.x - grab.x, pos.y - grab.y], size, room);
            if let Some(k) = card_index(app, &panel) {
                let stack = &mut app.ui.varos_panel_stack;
                let card = &mut stack.live[k];
                let front = card.front().to_string();
                card.panels.retain(|p| *p != panel);
                card.active = card.panels.iter().position(|p| *p == front).unwrap_or(0);
                let mut torn = Card { panels: vec![panel.clone()], active: 0, rect: [0.0; 4] };
                torn.set_area(Rect::from_min_size(at, size));
                stack.live.push(torn);
                stack.fitted.remove(&panel);
                stack.gesture = Some(Gesture::Move { first: panel, grab, from, original: Rect::from_min_size(at, size) });
            } else {
                app.ui.varos_panel_stack.gesture = None;
            }
            sync_cards(app);
        }
        Gesture::Resize { first, right, bottom, grab, original } => {
            if cancel {
                set_rect(app, &first, original);
                app.ui.varos_panel_stack.gesture = None;
                return;
            }
            let Some(now) = led_by(app, &first).map(|k| shown(app.ui.varos_panel_stack.live[k].rect, screen)) else {
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
    let Some(k) = card_index(app, panel) else { return };
    let panels = app.ui.varos_panel_stack.live[k].panels.clone();
    let Some(from) = panels.iter().position(|p| p == panel) else { return };
    let rect = |p: &str| ctx.read_response(tab_id(p)).map(|r| r.rect);
    let Some((to, over)) =
        panels.iter().enumerate().filter(|(_, p)| *p != panel).find_map(|(i, p)| Some((i, rect(p).filter(|r| r.x_range().contains(at.x))?)))
    else {
        return;
    };
    let passed = (to < from && at.x < over.center().x) || (to > from && at.x > over.center().x);
    if !passed {
        return;
    }
    let stack = &mut app.ui.varos_panel_stack;
    let card = &mut stack.live[k];
    let lead = card.first().to_string();
    let moved = card.panels.remove(from);
    card.panels.insert(to, moved);
    card.active = to;
    // The card keeps its fitting and its placing when another tab leads it now.
    let now = card.first().to_string();
    if now != lead {
        if let Some(f) = stack.fitted.remove(&lead) {
            stack.fitted.insert(now.clone(), f);
        }
        for f in stack.fresh.iter_mut().filter(|f| f.first == lead) {
            f.first.clone_from(&now);
        }
    }
}

/// The gesture egui's drag of widget `dragged` (pressed at `press`) starts on a card, if it is one
/// of a card's.
fn begin(app: &VectorcraftApp, dragged: egui::Id, press: Pos2, screen: Rect) -> Option<Gesture> {
    for c in &app.ui.varos_panel_stack.live {
        let first = c.first().to_string();
        let rect = shown(c.rect, screen);
        let id = card_id(&first);
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
        if let Some(panel) = c.panels.iter().find(|p| dragged == tab_id(p)) {
            // A card's only tab moves the card; one of several tabs is torn out of it.
            return Some(if c.panels.len() == 1 {
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
    /// In the docked group of this panel, as tabs (let go on its tabs); the zone lit meanwhile.
    Dock(String, Rect),
    /// Back on the icon column (its panels go back to their icons); the zone lit meanwhile.
    Icons(Rect),
}

/// Where the card led by `first` lands if let go at `at`: on another card's header it joins that
/// card; on a docked group's tabs it joins that group; on an icon column it closes. Dropped
/// anywhere else, it stays where it is.
fn target(app: &VectorcraftApp, ctx: &egui::Context, first: &str, at: Pos2) -> Option<Target> {
    let screen = ctx.content_rect();
    // The topmost card under the pointer (but the moved one): on its header, the moved card joins it.
    let layers: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
    let under = app
        .ui
        .varos_panel_stack
        .live
        .iter()
        .filter(|c| c.first() != first)
        .map(|c| (c.first(), shown(c.rect, screen)))
        .filter(|(_, r)| r.contains(at))
        .max_by_key(|(p, _)| layers.iter().position(|l| l.id == card_id(p)));
    if let Some((onto, rect)) = under {
        return header(rect).contains(at).then(|| Target::Card(onto.to_string()));
    }
    for (hit, r) in crate::panel_docking::hit_rects(ctx) {
        if !r.contains(at) {
            continue;
        }
        match hit {
            Hit::Icon(_) | Hit::IconGrip(_) => return Some(Target::Icons(r)),
            Hit::Tab(p) | Hit::DragBar(p)
                if p != crate::panel_docking::CANVAS && !crate::panel_docking::floats(&app.ui, &p) && card_index(app, &p).is_none() =>
            {
                return Some(Target::Dock(p, r));
            }
            _ => {}
        }
    }
    None
}

/// Light up where a let-go would put the moved card.
fn preview(ctx: &egui::Context, zone: Rect) {
    let t = crate::theme::Tokens::get(ctx);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("varos-card-drop")));
    painter.rect(zone.shrink(1.0), CornerRadius::same(3), t.accent.gamma_multiply(0.15), Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);
}

/// The card led by `first` was let go at `at`: group it, dock it, close it, or leave it there.
fn let_go(app: &mut VectorcraftApp, ctx: &egui::Context, first: &str, at: Pos2) {
    let Some(target) = target(app, ctx, first, at) else { return };
    let Some(k) = led_by(app, first) else { return };
    match target {
        Target::Card(onto) => {
            let moved = app.ui.varos_panel_stack.live.remove(k);
            if let Some(j) = led_by(app, &onto) {
                let card = &mut app.ui.varos_panel_stack.live[j];
                let front = moved.front().to_string();
                card.panels.extend(moved.panels);
                card.active = card.panels.iter().position(|p| *p == front).unwrap_or(card.active);
                let lead = card.first().to_string();
                app.ui.varos_panel_stack.fitted.remove(&lead);
            }
        }
        Target::Dock(anchor, _) => {
            remember(app);
            let moved = app.ui.varos_panel_stack.live.remove(k);
            for p in &moved.panels {
                let action = craft_ui::docking::Action::Move {
                    panel: p.clone(),
                    anchor: anchor.clone(),
                    placement: craft_ui::docking::Placement::Tab { before: None },
                };
                let action = serde_json::to_value(action).unwrap_or_default();
                if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.layout", &json!({ "action": action })) {
                    app.ui.status = error;
                }
            }
        }
        Target::Icons(_) => close_card(app, first),
    }
    sync_cards(app);
    follow_open_panel(app);
    app.ui.varos_panel_stack.repaint = true;
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
    // The click that opened a card opened the dock area's flyout of its icon too: it closes now.
    // It drew no body and was wiped from the picture ([`skips_body`], [`wipe_flyout`]), so the
    // card is drawn this very frame.
    let area = crate::panel_docking::area_id();
    if craft_ui::docking::flyout::<String>(ctx, area).is_some_and(|p| card_index(app, &p).is_some()) {
        craft_ui::docking::close_flyout::<String>(ctx, area);
        ctx.request_repaint();
    }
    let stack = &app.ui.varos_panel_stack;
    // A card Varos just placed is drawn unseen for its first frames ([`UNSEEN`]), so that it is
    // measured, fitted and placed at its real size before it shows: it appears once, where it stays.
    let unseen = |c: &Card| stack.fresh.iter().any(|f| f.first == c.first() && f.age < UNSEEN);
    let cards: Vec<(Vec<String>, usize, Rect, bool)> =
        stack.live.iter().map(|c| (c.panels.clone(), c.active, shown(c.rect, screen), unseen(c))).collect();
    let mut asks = Vec::new();
    for (panels, active, rect, hidden) in &cards {
        let seen = card(app, ctx, &t, panels, *active, *rect, *hidden, &mut asks);
        if seen && let Some(first) = panels.first() {
            app.ui.varos_panel_stack.painted.push((first.clone(), *rect));
        }
    }
    for ask in asks {
        match ask {
            Ask::Activate(panel) => {
                if let Some(k) = card_index(app, &panel) {
                    let card = &mut app.ui.varos_panel_stack.live[k];
                    card.active = card.panels.iter().position(|p| *p == panel).unwrap_or(card.active);
                }
            }
            Ask::Close(first) => close_card(app, &first),
        }
    }
}

/// Panel `panel`'s body in a card: its contents in a scroll area, the size they took noted for
/// [`fit`].
fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, panel: &str) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let back = ui.max_rect();
    if back.is_positive() {
        ui.painter().rect_filled(back, 0.0, t.panel);
    }
    let width = ui.available_width();
    let height = (ui.available_height() - 20.0).max(0.0);
    let out = egui::ScrollArea::vertical()
        .id_salt(("shared-panel-body", panel))
        .auto_shrink([false, false])
        .show(ui, |ui| crate::dock::panel_body(app, ui, panel, width, height));
    note_body(ui.ctx(), panel, out.content_size, out.inner_rect.size());
}

/// Draw one card at `rect`: `panels` as its tabs, the `active` one's body under them.
#[allow(clippy::too_many_arguments)]
fn card(
    app: &mut VectorcraftApp,
    ctx: &egui::Context,
    t: &crate::theme::Tokens,
    panels: &[String],
    active: usize,
    rect: Rect,
    hidden: bool,
    asks: &mut Vec<Ask>,
) -> bool {
    let (Some(first), Some(front_panel)) = (panels.first(), panels.get(active).or(panels.first())) else { return false };
    let id = card_id(first);
    let hint = app.ui.varos_panel_stack.hint.as_deref() == Some(first.as_str());
    let labels: Vec<String> = panels.iter().map(|p| label(app, p)).collect();
    // No fade-in: a card is chrome, it shows at once and fully opaque (egui's default fade-in drew
    // a card reopened or re-shown half transparent for its first frames).
    egui::Area::new(id)
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .movable(false)
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| {
            // egui draws an area it has never seen once unseen too, to measure it (a sizing pass).
            let seen = !hidden && !ui.is_sizing_pass();
            if hidden {
                ui.set_invisible();
            }
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
            let inner = Rect::from_min_max(pos2(rect.left() + 1.0, head.bottom()), pos2(rect.right() - 1.0, rect.bottom() - FOOT));
            if inner.is_positive() {
                let mut child = ui.new_child(
                    egui::UiBuilder::new().id_salt(("varos-card-body", front_panel)).max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)),
                );
                child.set_clip_rect(inner.intersect(ui.clip_rect()));
                body(app, &mut child, front_panel);
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
                preview(ctx, head.shrink(1.0));
            }
            seen
        })
        .inner
}

/// Paint an icon of an icon column lit when its panel is open as a card (craft-ui lights the icon
/// of its flyout only): the kit's toggle, a dark well with a 1-point line under a bright glyph.
/// Called before the glyph is painted; → the glyph's colour (`color` when not lit).
pub(crate) fn paint_icon_well(app: &VectorcraftApp, painter: &egui::Painter, rect: Rect, id: &str, color: egui::Color32) -> egui::Color32 {
    if !in_card(app, id) {
        return color;
    }
    let t = crate::theme::Tokens::get(painter.ctx());
    painter.rect(rect.expand(5.0), CornerRadius::same(3), t.input, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    t.text_strong
}

/// Keep [`PanelStack::groups`] up to date with the cards: a card with several tabs is a group; a
/// panel that left its card (to another card) leaves its group; a panel closed keeps its place in
/// its group, so that it opens again with it. Only when the cards changed.
fn sync_groups(app: &mut VectorcraftApp) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for c in &app.ui.varos_panel_stack.live {
        c.panels.hash(&mut hasher);
        c.active.hash(&mut hasher);
    }
    let signature = hasher.finish();
    if app.ui.varos_panel_stack.signature == signature {
        return;
    }
    let current: Vec<(Vec<String>, String)> = app.ui.varos_panel_stack.live.iter().map(|c| (c.panels.clone(), c.front().to_string())).collect();
    let open: Vec<String> = current.iter().flat_map(|(p, _)| p.iter().cloned()).collect();
    let stack = &mut app.ui.varos_panel_stack;
    stack.signature = signature;
    for (panels, active) in current {
        let k = stack.groups.iter().position(|m| m.panels.iter().any(|p| panels.contains(p)));
        for (j, m) in stack.groups.iter_mut().enumerate() {
            if Some(j) != k {
                m.panels.retain(|p| !panels.contains(p));
            }
        }
        match k {
            Some(k) => {
                let m = &mut stack.groups[k];
                let mut kept = panels.clone();
                for (i, p) in m.panels.iter().enumerate() {
                    // A closed tab keeps its place; one open in another card left the group.
                    if !panels.contains(p) && !open.contains(p) {
                        kept.insert(i.min(kept.len()), p.clone());
                    }
                }
                m.panels = kept;
                m.active = active;
            }
            None if panels.len() >= 2 => stack.groups.push(Group { panels, active, parked: Vec::new() }),
            None => {}
        }
    }
    stack.groups.retain(|m| m.panels.len() >= 2);
}
