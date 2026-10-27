//! Dragging in the shell, Illustrator's way: the dragged thing becomes a small ghost under the
//! pointer (a tab pill, or a 26-point icon, at 70 %), its source gets a 1-point azure outline, and
//! the one preview of the result is a thin azure line (never a ghost box). Release applies at once;
//! Escape, a lost pointer or a lost window focus cancels and nothing changes.
//!
//! What can be dragged ([`Item`]): a tab (a box's, a floating panel's or a flyout's), an icon, a
//! strip group by its grip dots, a floating strip by its title strip; a floating panel's title
//! strip only moves it. Where it can land ([`Target`]), first match wins: a tab row (insert),
//! a strip (between groups: a new group; on an icon: join its group), the Board strip's edge
//! (a floating strip docks), a box's edge band (split), else empty space in a box or over the
//! Board: a floating panel (tabs) or a lone floating icon (icons, groups). Outside the central
//! region nothing applies (the drop cancels).
//!
//! Every drop is one change of [`VarosShell`] made on a copy and kept only when it fully worked, so
//! each panel always has exactly one owner ([`VarosShell::invariants`], asserted in debug builds).

use egui::{CornerRadius, CursorIcon, Id, Key, LayerId, Order, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::floats::{self, Float, FloatStrip};
use super::persist::VarosShell;
use super::strip::{self, Group, StripRef};
use super::tree::{Axis, PanelBox, SEAM};
use super::{Hit, boxes};
use crate::VectorcraftApp;

/// The band along a box's edges (inside it) where a drop splits the box; it also reaches half a
/// seam outside.
pub(crate) const EDGE_BAND: f32 = 24.0;
/// The drop line's thickness.
const LINE: f32 = 2.0;
/// The ghost's opacity.
const GHOST_ALPHA: f32 = 0.7;
/// An icon ghost's size.
const ICON: f32 = 26.0;

/// Move, or (Alt held when the drag began) duplicate. Only a Properties editor's tab is duplicated
/// (another editor with the same section view lands); any other duplicate drag moves, and the
/// status line says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragMode {
    Move,
    Duplicate,
}

/// What is being dragged.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A panel's tab (in a box, a floating panel or a flyout).
    Tab(String),
    /// A panel's icon in a strip.
    Icon(String),
    /// A strip's group, by its grip dots (as it was when the drag began).
    Group { strip: StripRef, group: Group },
    /// A floating strip, by its title strip.
    Strip(u32),
    /// A floating panel moved by its title strip (live; no target). `from`: its rect before.
    Float { id: u32, from: [f32; 4] },
    /// A floating panel resized (live; no target) from its lower-right corner (`corner`) or its
    /// right edge. `from`: its rect before.
    Resize { id: u32, from: [f32; 4], corner: bool },
}

impl Item {
    /// A floating panel moved or resized in place: no ghost, no target.
    fn in_place(&self) -> bool {
        matches!(self, Item::Float { .. } | Item::Resize { .. })
    }
}

/// A box's edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// The split a drop on this edge makes: its axis, and whether the new box comes after.
    fn split(self) -> (Axis, bool) {
        match self {
            Side::Left => (Axis::Horizontal, false),
            Side::Right => (Axis::Horizontal, true),
            Side::Top => (Axis::Vertical, false),
            Side::Bottom => (Axis::Vertical, true),
        }
    }
}

/// Where in a strip. Groups and positions are named by a panel in them that is not dragged.
#[derive(Clone, Debug, PartialEq)]
pub enum Slot {
    /// A group of its own, before the group holding `before` (at the end: `None`).
    NewGroup { before: Option<String> },
    /// Into the group holding `group_of`, before `before` (at its end: `None`).
    Join { group_of: String, before: Option<String> },
}

/// Where a drop lands.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Box `box_id`'s tab row (a box of the tree or a floating panel), before tab `before`.
    Tabs { box_id: u32, before: Option<String> },
    /// A split of box `box_id` (the Board included) on its `side`.
    Edge { box_id: u32, side: Side },
    /// A strip.
    Strip { strip: StripRef, slot: Slot },
    /// The Board's strip, after its groups (a floating strip docks).
    Dock,
    /// A new floating panel.
    Float,
    /// A new lone floating icon (a floating strip).
    LoneIcon,
    /// A floating strip moved.
    Move,
}

/// The drag under way.
#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    pub item: Item,
    pub mode: DragMode,
    /// Where the pointer holds the ghost, from its top-left corner.
    pub grab: Vec2,
    /// The ghost's size.
    pub ghost: Vec2,
    /// The flyout open when the drag began (a drag closes it): open again when nothing lands.
    pub reopen: Option<String>,
    /// Released in a frame whose layout differs from the one the targets were found in (the
    /// window changed size): it lands at the end of that frame, on what that frame drew.
    pub released: bool,
}

/// The layer the preview (outline, line, ghost) is painted on: above the boxes, floats and flyouts.
fn preview_layer() -> LayerId {
    LayerId::new(Order::Foreground, Id::unique("varos-shell-drag"))
}

/// A drag of `item` began this frame, from `source` (its rect).
pub(crate) fn begin(app: &mut VectorcraftApp, ctx: &egui::Context, item: Item, source: Rect) {
    if app.ui.varos_shell.drag.is_some() {
        return;
    }
    let origin = ctx.input(|i| i.pointer.press_origin()).unwrap_or(source.center());
    let in_place = item.in_place();
    let mode = if !in_place && ctx.input(|i| i.modifiers.alt) { DragMode::Duplicate } else { DragMode::Move };
    if mode == DragMode::Duplicate && !duplicates(&item) {
        app.status("duplicate: later");
    }
    let ghost = match item {
        Item::Tab(_) | Item::Float { .. } => source.size(),
        Item::Resize { .. } => Vec2::ZERO,
        _ => vec2(ICON, ICON),
    };
    // A resize holds the float's lower-right corner (`source` is the float): from the pointer.
    let grab = match item {
        Item::Resize { .. } => source.max - origin,
        _ => (origin - source.min).max(Vec2::ZERO).min(ghost),
    };
    let reopen = if in_place { None } else { app.ui.varos_shell.open.clone() };
    if !in_place {
        strip::close(app);
    }
    app.ui.varos_shell.drag = Some(Drag { item, mode, grab, ghost, reopen, released: false });
    ctx.request_repaint();
}

/// Does a duplicate drag of `item` make a copy: a Properties editor's tab.
fn duplicates(item: &Item) -> bool {
    matches!(item, Item::Tab(p) if super::persist::is_properties(p))
}

/// Escape cancels a drag before anything else sees it (called first by `shortcuts::handle`): a
/// drag of the shell's, or a Properties editor's section being moved.
pub(crate) fn escape(app: &mut VectorcraftApp, ctx: &egui::Context) {
    crate::panels::properties::escape_section_drag(ctx);
    if app.ui.varos_shell.drag.is_some() && ctx.input_mut(|i| i.key_pressed(Key::Escape) && i.consume_key(i.modifiers, Key::Escape)) {
        cancel(app, ctx);
    }
}

/// Drop the drag: nothing changes (a moved or resized floating panel goes back; the flyout the
/// drag closed opens again).
pub(crate) fn cancel(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if let Some(d) = app.ui.varos_shell.drag.take() {
        if let Item::Float { id, from } | Item::Resize { id, from, .. } = d.item
            && let Some(f) = app.ui.varos_shell.float_mut(id)
        {
            f.rect = from;
        }
        reopen(app, &d);
    }
    ctx.stop_dragging();
    ctx.request_repaint();
}

/// Nothing landed: the flyout drag `d` closed opens again (its panel still in a strip).
fn reopen(app: &mut VectorcraftApp, d: &Drag) {
    if let Some(panel) = &d.reopen
        && app.ui.varos_shell.open.is_none()
        && app.ui.varos_shell.in_a_strip(panel)
    {
        strip::open(app, panel);
    }
}

/// The layout the targets are found in: the central region and every owner's arrangement. A drag
/// lands on the published hit rects only when they were drawn for this same layout.
pub(crate) fn layout_rev(shell: &VarosShell, region: Rect) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{region:?}{:?}{:?}{:?}{:?}{:?}", shell.tree, shell.strip, shell.floats, shell.float_strips, shell.maximized_box()).hash(&mut h);
    h.finish()
}

/// At the start of the shell's frame, before anything is laid out: a released drag is applied (so
/// the release frame draws the result), a lost one is cancelled, a moved floating panel follows
/// the pointer.
pub(crate) fn before(app: &mut VectorcraftApp, ctx: &egui::Context, region: Rect) {
    let Some(d) = app.ui.varos_shell.drag.clone() else { return };
    let (focused, gone, down, released, pos) = ctx.input(|i| {
        (
            i.focused,
            i.events.iter().any(|e| matches!(e, egui::Event::PointerGone)),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.pointer.latest_pos(),
        )
    });
    let Some(p) = pos.filter(|_| focused && !gone && (down || released)) else {
        cancel(app, ctx);
        return;
    };
    if d.item.in_place() {
        if let Item::Float { id, .. } | Item::Resize { id, .. } = d.item
            && let Some(f) = app.ui.varos_shell.float_mut(id)
        {
            let now = f.rect();
            let new = match d.item {
                Item::Resize { corner, .. } => floats::resized(now, p + d.grab, corner, region),
                _ => floats::clamp_into(Rect::from_min_size(p - d.grab, now.size()), region),
            };
            f.set_rect(new);
        }
        if released {
            app.ui.varos_shell.drag = None;
        }
        return;
    }
    if released {
        // The targets were found on the last frame's controls: only valid for the same layout.
        if super::hits_rev(ctx) != Some(layout_rev(&app.ui.varos_shell, region)) {
            if let Some(d) = app.ui.varos_shell.drag.as_mut() {
                d.released = true;
            }
            return;
        }
        app.ui.varos_shell.drag = None;
        let hits = super::hit_rects(ctx);
        land(app, ctx, region, &hits, &d, p);
        ctx.request_repaint();
    }
}

/// Drop `d` at `p` on the target `hits` give; with none, or one that doesn't take it, nothing
/// changes but the flyout the drag closed opens again.
fn land(app: &mut VectorcraftApp, ctx: &egui::Context, region: Rect, hits: &[(Hit, Rect)], d: &Drag, p: Pos2) {
    let landed = match target_at(&app.ui.varos_shell, ctx, region, hits, d, p) {
        Some((target, _)) => apply_drop(app, d, &target, p, hits),
        None => false,
    };
    if !landed {
        reopen(app, d);
    }
}

/// At the end of the shell's frame, once every control is drawn: the target under the pointer from
/// this frame's `hits`, and the preview: the source's outline, the line, the ghost. The line goes
/// to `hits` as [`Hit::DropLine`]; a target without a line as [`Hit::DropAt`]; the ghost as
/// [`Hit::Ghost`].
pub(crate) fn after(app: &mut VectorcraftApp, ui: &mut egui::Ui, region: Rect, hits: &mut Vec<(Hit, Rect)>) {
    let Some(d) = app.ui.varos_shell.drag.clone() else { return };
    let ctx = ui.ctx().clone();
    ctx.set_cursor_icon(match d.item {
        Item::Resize { corner: true, .. } => CursorIcon::ResizeNwSe,
        Item::Resize { corner: false, .. } => CursorIcon::ResizeHorizontal,
        _ => CursorIcon::Grabbing,
    });
    if d.item.in_place() {
        return;
    }
    let Some(p) = ctx.input(|i| i.pointer.latest_pos()) else { return };
    // Released over a layout that changed: it lands on this frame's controls.
    if d.released {
        app.ui.varos_shell.drag = None;
        land(app, &ctx, region, hits, &d, p);
        ctx.request_repaint();
        return;
    }
    let accent = crate::varos::theme::ACCENT;
    let layer = preview_layer();
    let painter = ctx.layer_painter(layer);
    if let Some(source) = source_rect(&app.ui.varos_shell, &d.item, hits) {
        painter.rect_stroke(source, CornerRadius::same(3), Stroke::new(1.0, accent), StrokeKind::Inside);
        hits.push((Hit::DragSource, source));
    }
    let ghost = Rect::from_min_size(p - d.grab, d.ghost);
    match target_at(&app.ui.varos_shell, &ctx, region, hits, &d, p) {
        Some((target, Some((line, vertical)))) => {
            painter.rect_filled(line, CornerRadius::same(1), accent);
            hits.push((Hit::DropLine { vertical, target }, line));
        }
        Some((target, None)) => hits.push((Hit::DropAt(target), ghost)),
        None => {}
    }
    paint_ghost(app, ui, layer, &d, ghost);
    hits.push((Hit::Ghost, ghost));
}

/// The ghost: a tab pill with its label, or the icon (a group's and a strip's first), at 70 %.
fn paint_ghost(app: &VectorcraftApp, ui: &mut egui::Ui, layer: LayerId, d: &Drag, ghost: Rect) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let fade = |c: egui::Color32| c.gamma_multiply(GHOST_ALPHA);
    let painter = ui.ctx().layer_painter(layer);
    let panel = item_panels(&app.ui.varos_shell, &d.item).into_iter().next().unwrap_or_default();
    if let Item::Tab(_) = d.item {
        let radius = CornerRadius::same((ghost.height() / 2.0).min(127.0) as u8);
        painter.rect_filled(ghost, radius, fade(t.button));
        let label = painter.layout_no_wrap(boxes::label(app, &panel), crate::theme::semibold(12.0), fade(t.text_strong));
        let at = pos2(ghost.left() + 10.0_f32.min(ghost.width() / 2.0), ghost.center().y - label.size().y / 2.0);
        painter.with_clip_rect(ghost.shrink2(vec2(4.0, 0.0))).galley(at, label, fade(t.text_strong));
        return;
    }
    painter.rect_filled(ghost, CornerRadius::same(3), fade(t.button));
    let mut paint_ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(ghost));
    paint_ui.set_clip_rect(ghost);
    let icon = crate::state::rail_panel(super::persist::instance_kind(&panel)).map_or("panel-left", |p| p.2);
    egui::Image::new(crate::icons::source(icon))
        .tint(fade(t.text_strong))
        .paint_at(&paint_ui, Rect::from_center_size(ghost.center(), vec2(16.0, 16.0)));
}

/// Where the dragged thing is drawn: a tab's pill, an icon, a group (grip and icons), a strip. A
/// tab dragged out of a flyout (closed by the drag) is shown by its group in the strip.
fn source_rect(shell: &VarosShell, item: &Item, hits: &[(Hit, Rect)]) -> Option<Rect> {
    let find = |want: &dyn Fn(&Hit) -> bool| hits.iter().filter(|(h, _)| want(h)).map(|(_, r)| *r).reduce(Rect::union);
    let group_of = |strip: StripRef, index: usize| {
        let group = shell.strip_groups(strip)?.get(index)?;
        let grip = strip.group_hit(index);
        let icons: Vec<Hit> = group.panels.iter().map(|p| strip.icon_hit(p)).collect();
        find(&|h| *h == grip || icons.contains(h))
    };
    match item {
        Item::Tab(panel) => find(&|h| matches!(h, Hit::Tab { panel: p, .. } if p == panel)).or_else(|| {
            let strip = match shell.float_strip_of(panel) {
                Some(id) => StripRef::Float(id),
                None => StripRef::Board,
            };
            let index = shell.strip_groups(strip)?.iter().position(|g| g.panels.contains(panel))?;
            group_of(strip, index)
        }),
        Item::Icon(panel) => find(&|h| matches!(h, Hit::StripIcon(p) | Hit::FloatStripIcon { panel: p, .. } if p == panel)),
        Item::Group { strip, group } => {
            let index = shell.strip_groups(*strip)?.iter().position(|g| g.panels == group.panels)?;
            group_of(*strip, index)
        }
        Item::Strip(id) => find(&|h| *h == Hit::FloatStrip(*id)),
        Item::Float { .. } | Item::Resize { .. } => None,
    }
}

/// The panels `item` carries.
pub(crate) fn item_panels(shell: &VarosShell, item: &Item) -> Vec<String> {
    match item {
        Item::Tab(p) | Item::Icon(p) => vec![p.clone()],
        Item::Group { group, .. } => group.panels.clone(),
        Item::Strip(id) => {
            shell.float_strips.iter().find(|s| s.id == *id).map(|s| s.groups.iter().flat_map(|g| g.panels.clone()).collect()).unwrap_or_default()
        }
        Item::Float { .. } | Item::Resize { .. } => Vec::new(),
    }
}

/// The groups `item` carries into a strip: a strip's own; a group; a panel alone, with the
/// remembered size of the group it leaves (if any).
fn item_groups(shell: &VarosShell, item: &Item) -> Vec<Group> {
    match item {
        Item::Tab(p) | Item::Icon(p) => {
            let source = shell.strip.groups.iter().chain(shell.float_strips.iter().flat_map(|s| &s.groups)).find(|g| g.panels.contains(p));
            vec![Group {
                panels: vec![p.clone()],
                temporary: !strip::Strip::is_default(p),
                width: source.map_or(floats::DEFAULT_SIZE.x, |g| g.width),
                height: source.map_or(floats::DEFAULT_SIZE.y, |g| g.height),
            }]
        }
        Item::Group { group, .. } => vec![group.clone()],
        Item::Strip(id) => shell.float_strips.iter().find(|s| s.id == *id).map(|s| s.groups.clone()).unwrap_or_default(),
        Item::Float { .. } | Item::Resize { .. } => Vec::new(),
    }
}

/// A line `LINE` thick: vertical through `x` from `y0` to `y1`, or horizontal.
fn vline(x: f32, y0: f32, y1: f32) -> Rect {
    Rect::from_min_max(pos2(x - LINE / 2.0, y0), pos2(x + LINE / 2.0, y1))
}

fn hline(y: f32, x0: f32, x1: f32) -> Rect {
    Rect::from_min_max(pos2(x0, y - LINE / 2.0), pos2(x1, y + LINE / 2.0))
}

type Found = Option<(Target, Option<(Rect, bool)>)>;

/// The target of drag `d` with the pointer at `p`, from the controls drawn (`hits`), and its line
/// (rect, vertical) when it has one.
pub(crate) fn target_at(shell: &VarosShell, ctx: &egui::Context, region: Rect, hits: &[(Hit, Rect)], d: &Drag, p: Pos2) -> Found {
    if !region.contains(p) || d.item.in_place() {
        return None;
    }
    let moving_strip = match d.item {
        Item::Strip(id) => Some(id),
        _ => None,
    };
    // A floating panel or strip on top takes the drop; another layer (a flyout, a menu) refuses it.
    if let Some(layer) = ctx.layer_id_at(p).filter(|l| l.order != Order::Background) {
        if let Some(f) = shell.floats.iter().find(|f| floats::float_layer(f.panels.id) == layer) {
            if moving_strip.is_some() {
                return Some((Target::Move, None));
            }
            let row = hits.iter().find(|(h, r)| *h == Hit::TabRow(f.panels.id) && r.contains(p));
            return match row {
                Some((_, row)) => tab_target(shell, hits, f.panels.id, *row, d, p),
                None => Some((empty_space(&d.item), None)),
            };
        }
        match shell.float_strips.iter().find(|s| floats::strip_layer(s.id) == layer) {
            Some(s) if moving_strip == Some(s.id) => {}
            Some(_) if moving_strip.is_some() => return Some((Target::Move, None)),
            Some(s) => {
                let rail = hits.iter().find(|(h, _)| *h == Hit::FloatStrip(s.id)).map(|(_, r)| *r)?;
                return strip_target(shell, hits, StripRef::Float(s.id), rail, d, p);
            }
            // Over the boxes, other windows (the canvas's task bar, a floating tool palette) let
            // the drop through; a flyout, a menu, a popup or a dialog refuses it.
            None if layer.order == Order::Middle && layer.id != Id::unique("shell-flyout") => {}
            None => return None,
        }
    }
    // The tab rows of the tree's boxes.
    if moving_strip.is_none()
        && let Some((Hit::TabRow(id), row)) =
            hits.iter().find(|(h, r)| matches!(h, Hit::TabRow(id) if shell.tree.find_box(*id).is_some()) && r.contains(p))
    {
        return tab_target(shell, hits, *id, *row, d, p);
    }
    // The Board's strip.
    let rail = hits.iter().find(|(h, _)| *h == Hit::Strip).map(|(_, r)| *r);
    if let Some(rail) = rail
        && rail.contains(p)
    {
        return strip_target(shell, hits, StripRef::Board, rail, d, p);
    }
    if moving_strip.is_some() {
        // Near the Board strip's edge, or the window's right edge: it docks there.
        if let Some(rail) = rail
            && ((p.x >= rail.left() - EDGE_BAND && p.x < rail.left() && rail.y_range().contains(p.y)) || p.x >= region.right() - EDGE_BAND)
        {
            return Some((Target::Dock, Some((vline(rail.left() - SEAM / 2.0, rail.top(), rail.bottom()), true))));
        }
        return Some((Target::Move, None));
    }
    // A box's edge band: the nearest edge.
    let board = shell.tree.boxes().into_iter().find(|b| b.is_board()).map(|b| b.id);
    let boxes = hits.iter().filter_map(|(h, r)| match h {
        Hit::Box(id) => Some((*id, *r)),
        Hit::Board => board.map(|id| (id, *r)),
        _ => None,
    });
    let mut best: Option<(f32, u32, Side, Rect)> = None;
    for (id, r) in boxes.clone() {
        // The Board's strip covers its right edge (and takes drops itself): the Board's right band
        // is the one just left of the strip, inside the Board, besides the half seam outside.
        let right = match rail {
            Some(rail) if Some(id) == board && r.contains_rect(rail) => rail.left(),
            _ => r.right(),
        };
        let inner = Rect::from_min_max(r.min, pos2(right, r.bottom()));
        if !r.expand(SEAM / 2.0).contains(p) || inner.shrink(EDGE_BAND).contains(p) {
            continue;
        }
        let to_right = if p.x > r.right() { r.right() - p.x } else { right - p.x };
        for (side, dist) in [(Side::Left, p.x - r.left()), (Side::Right, to_right), (Side::Top, p.y - r.top()), (Side::Bottom, r.bottom() - p.y)] {
            if dist < EDGE_BAND && best.is_none_or(|b| dist < b.0) {
                best = Some((dist, id, side, r));
            }
        }
    }
    if let Some((_, box_id, side, r)) = best {
        let line = match side {
            Side::Left => vline(r.left(), r.top(), r.bottom()),
            Side::Right => vline(r.right(), r.top(), r.bottom()),
            Side::Top => hline(r.top(), r.left(), r.right()),
            Side::Bottom => hline(r.bottom(), r.left(), r.right()),
        };
        return Some((Target::Edge { box_id, side }, Some((line, matches!(side, Side::Left | Side::Right)))));
    }
    // Empty space in a box or over the Board.
    boxes.clone().any(|(_, r)| r.contains(p)).then(|| (empty_space(&d.item), None))
}

/// What a drop on empty space makes of `item`: a floating panel of a tab, a lone icon of icons.
fn empty_space(item: &Item) -> Target {
    match item {
        Item::Tab(_) => Target::Float,
        Item::Strip(_) => Target::Move,
        _ => Target::LoneIcon,
    }
}

/// The insertion into box `id`'s tab row (`row`): before the first pill right of the pointer.
fn tab_target(shell: &VarosShell, hits: &[(Hit, Rect)], id: u32, row: Rect, d: &Drag, p: Pos2) -> Found {
    if matches!(d.item, Item::Strip(_)) {
        return None;
    }
    let dragged = item_panels(shell, &d.item);
    let pills: Vec<(&String, Rect)> = hits
        .iter()
        .filter_map(|(h, r)| match h {
            Hit::Tab { box_id, panel } if *box_id == id => Some((panel, *r)),
            _ => None,
        })
        .collect();
    // The box's only tabs are the dragged ones: nothing would change.
    if !pills.is_empty() && pills.iter().all(|(n, _)| dragged.contains(n)) {
        return None;
    }
    let j = pills.iter().filter(|(_, r)| r.center().x < p.x).count();
    let before = pills[j..].iter().map(|(n, _)| *n).find(|n| !dragged.contains(n)).cloned();
    let x = match (j, pills.len()) {
        (_, 0) => row.left() + 2.0,
        (0, _) => pills[0].1.left() - 1.0,
        (j, n) if j == n => pills[n - 1].1.right() + 1.0,
        (j, _) => f32::midpoint(pills[j - 1].1.right(), pills[j].1.left()),
    };
    let half = boxes::PILL_H / 2.0;
    Some((Target::Tabs { box_id: id, before }, Some((vline(x, row.center().y - half, row.center().y + half), true))))
}

/// The slot in strip `sref` (drawn in `rail`) at the pointer: over a group's grip dots (or just
/// above them), a new group before it; on an icon, its group (before it in its upper half, after it
/// in its lower half); below everything, a new group at the end. A floating strip only makes
/// new groups.
fn strip_target(shell: &VarosShell, hits: &[(Hit, Rect)], sref: StripRef, rail: Rect, d: &Drag, p: Pos2) -> Found {
    let groups = shell.strip_groups(sref)?;
    let dragged = item_panels(shell, &d.item);
    let rect_of = |hit: &Hit| hits.iter().find(|(h, _)| h == hit).map(|(_, r)| *r);
    let free = |g: &Group| g.panels.iter().find(|q| !dragged.contains(q)).cloned();
    let (x0, x1) = (rail.left() + 3.0, rail.right() - 3.0);
    // Over grip dots: a new group before that group.
    for (i, _) in groups.iter().enumerate() {
        let Some(grip) = rect_of(&sref.group_hit(i)) else { continue };
        if p.y >= grip.top() - 6.0 && p.y < grip.bottom() {
            let before = groups[i..].iter().find_map(free);
            return Some((Target::Strip { strip: sref, slot: Slot::NewGroup { before } }, Some((hline(grip.top(), x0, x1), false))));
        }
    }
    let whole_groups = matches!(d.item, Item::Strip(_));
    let mut bottom = rail.top();
    for g in groups {
        for (k, panel) in g.panels.iter().enumerate() {
            let Some(icon) = rect_of(&sref.icon_hit(panel)) else { continue };
            bottom = bottom.max(icon.bottom());
            if whole_groups || !(icon.top()..icon.bottom()).contains(&p.y) {
                continue;
            }
            let upper = p.y < icon.center().y;
            let at = if upper { k } else { k + 1 };
            // Its own group, all dragged: nothing would change.
            let group_of = free(g)?;
            let before = g.panels[at..].iter().find(|q| !dragged.contains(q)).cloned();
            let y = if upper { icon.top() } else { icon.bottom() };
            return Some((Target::Strip { strip: sref, slot: Slot::Join { group_of, before } }, Some((hline(y, x0 + 4.0, x1 - 4.0), false))));
        }
    }
    let y = (bottom + 1.0).min(rail.bottom() - 1.0);
    Some((Target::Strip { strip: sref, slot: Slot::NewGroup { before: None } }, Some((hline(y, x0, x1), false))))
}

/// Make drop `t` of drag `d` (pointer at `p`) on a copy of the shell; keep it only when it worked.
fn apply_drop(app: &mut VectorcraftApp, d: &Drag, t: &Target, p: Pos2, hits: &[(Hit, Rect)]) -> bool {
    let mut shell = app.ui.varos_shell.clone();
    let mut item = d.item.clone();
    if d.mode == DragMode::Duplicate {
        match &d.item {
            // Another Properties editor, with the same section view (not the pin), lands.
            Item::Tab(panel) if duplicates(&d.item) => {
                let id = shell.fresh_properties_id();
                let mut view = shell.views.get(panel).cloned().unwrap_or_default();
                view.pinned = None;
                shell.views.insert(id.clone(), view);
                item = Item::Tab(id);
            }
            // Any other panel moves.
            _ => app.status("duplicate: later"),
        }
    }
    let board = shell.tree.boxes().into_iter().find(|b| b.is_board()).map_or(0, |b| b.id);
    let owner = hits
        .iter()
        .find_map(|(h, r)| match h {
            Hit::Box(id) if r.contains(p) => Some(*id),
            Hit::Board if r.contains(p) => Some(board),
            _ => None,
        })
        .unwrap_or(board);
    if !apply(&mut shell, &item, t, p - d.grab, p, owner) {
        return false;
    }
    // A maximized box stays so while the boxes are the same; a new box or one gone shows the
    // whole layout again.
    let ids = |s: &VarosShell| s.tree.boxes().iter().map(|b| b.id).collect::<Vec<_>>();
    if ids(&shell) != ids(&app.ui.varos_shell) {
        shell.maximized = None;
    }
    shell.settle();
    debug_assert!(shell.invariants().is_ok(), "after a drop: {:?}", shell.invariants());
    app.ui.varos_shell = shell;
    app.ui.open_panel = app.ui.varos_shell.open.clone();
    true
}

/// Drop `item` on `t` in `shell`: true when it fully worked (else `shell` is garbage). `at`: the
/// ghost's top-left; `p`: the pointer; `owner`: the box under the pointer.
pub(crate) fn apply(shell: &mut VarosShell, item: &Item, t: &Target, at: Pos2, p: Pos2, owner: u32) -> bool {
    if *t == Target::Move {
        let Item::Strip(id) = item else { return false };
        let Some(s) = shell.float_strips.iter_mut().find(|s| s.id == *id) else { return false };
        s.pos = [at.x, at.y];
        return true;
    }
    let panels = item_panels(shell, item);
    let groups = item_groups(shell, item);
    if panels.is_empty() || groups.is_empty() {
        return false;
    }
    for panel in &panels {
        shell.take(panel);
    }
    match t {
        Target::Tabs { box_id, before } => shell.insert_tabs(*box_id, before.as_deref(), &panels),
        Target::Edge { box_id, side } => {
            let (axis, after) = side.split();
            let Some(new) = shell.tree.split_box(*box_id, axis, &panels[0], after, 0.5) else { return false };
            for (k, panel) in panels.iter().enumerate().skip(1) {
                shell.tree.insert_tab(new, k, panel);
            }
            shell.tree.set_active(new, 0);
            true
        }
        Target::Float => {
            let size = vec2(groups[0].width, groups[0].height).max(super::tree::BOX_MIN);
            let min = pos2(p.x - (p.x - at.x).min(60.0) - 6.0, p.y - floats::TITLE - boxes::HEADER / 2.0);
            let id = shell.fresh_float_id();
            let panels = PanelBox { id, tabs: panels, active: 0, collapsed: false };
            let mut f = Float { panels, rect: [0.0; 4], owner_area: owner };
            f.set_rect(Rect::from_min_size(min, size));
            shell.floats.push(f);
            true
        }
        Target::LoneIcon => {
            let id = shell.fresh_strip_id();
            let pos = p - vec2(floats::STRIP_W / 2.0, floats::TITLE + strip::GRIP + strip::ICON_ROW / 2.0);
            shell.float_strips.push(FloatStrip { id, pos: [pos.x, pos.y], groups });
            true
        }
        Target::Strip { strip, slot } => {
            let Some(target) = shell.strip_groups_mut(*strip) else { return false };
            match slot {
                Slot::NewGroup { before } => {
                    let at = before.as_ref().and_then(|b| target.iter().position(|g| g.panels.contains(b))).unwrap_or(target.len());
                    for (k, g) in groups.into_iter().enumerate() {
                        target.insert(at + k, g);
                    }
                    true
                }
                Slot::Join { group_of, before } => {
                    let Some(g) = target.iter_mut().find(|g| g.panels.contains(group_of)) else { return false };
                    let at = before.as_ref().and_then(|b| g.panels.iter().position(|q| q == b)).unwrap_or(g.panels.len());
                    for (k, panel) in panels.into_iter().enumerate() {
                        g.panels.insert(at + k, panel);
                    }
                    true
                }
            }
        }
        Target::Dock => {
            shell.strip.groups.extend(groups);
            true
        }
        Target::Move => false,
    }
}
