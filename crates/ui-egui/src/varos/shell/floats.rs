//! Floating panels and floating icon strips (a lone floating icon is a strip of one icon): shell-
//! owned Areas above the boxes, below menus and dialogs. Flat like the boxes: an 8-point rounded
//! fill and a 1-point hairline, no shadow, no motion.
//!
//! A floating panel is a box ([`PanelBox`]) with a thin title strip on top: `×` (its panels go back
//! to the Board's strip) at the left, `<<` (it becomes a lone icon) at the right; it is moved by
//! that strip, resized from its lower-right corner and its right edge, and collapsed to its title
//! and tab row by a double-click on a tab. A floating strip has the same title strip with `×` and
//! `>>` (each of its groups becomes a floating panel); its icons open flyouts beside it.

use egui::{CornerRadius, Id, LayerId, Order, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};

use super::Hit;
use super::boxes;
use super::drag::{self, Item};
use super::strip::{self, Group, Mode, StripRef};
use super::tree::{BOX_MIN, PanelBox, ROW};
use crate::VectorcraftApp;
use crate::theme::Tokens;

/// The title strip's height (floating panels and floating strips).
pub(crate) const TITLE: f32 = 16.0;
/// A floating panel's size when nothing remembers one.
pub(crate) const DEFAULT_SIZE: Vec2 = vec2(260.0, 360.0);
/// A floating strip's width (an Icons-mode strip's).
pub(crate) const STRIP_W: f32 = 34.0;
/// The title strip's `×`, `<<` and `>>`.
const BUTTON: Vec2 = vec2(12.0, 14.0);
/// The lower-right resize corner and the right resize edge.
const CORNER: f32 = 12.0;
const EDGE: f32 = 5.0;
/// Room below a floating strip's last icon.
const STRIP_FOOT: f32 = 4.0;

/// A floating panel: panels as tabs, above the boxes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Float {
    /// Its panels; the id is unique among the tree's boxes and the floats.
    #[serde(rename = "box")]
    pub panels: PanelBox,
    /// Left, top, width, height in window points: its whole size (collapsed, only the title strip
    /// and the tab row show).
    pub rect: [f32; 4],
    /// The box it was dropped over when it was made (the Board's id included). A float lies above
    /// all boxes; this only records where it came from.
    #[serde(default)]
    pub owner_area: u32,
}

impl Float {
    pub(crate) fn rect(&self) -> Rect {
        let [x, y, w, h] = self.rect;
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    pub(crate) fn set_rect(&mut self, r: Rect) {
        self.rect = [r.left(), r.top(), r.width(), r.height()];
    }

    /// The rect it shows in `region`: kept inside it, its title and tab row only while collapsed.
    pub(crate) fn shown(&self, region: Rect) -> Rect {
        let full = clamp_into(self.rect(), region);
        if self.panels.collapsed { Rect::from_min_size(full.min, vec2(full.width(), (TITLE + ROW).min(full.height()))) } else { full }
    }
}

/// A floating icon strip: groups of icons, like the Board's strip, anywhere above the boxes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatStrip {
    /// Unique among the floating strips.
    pub id: u32,
    /// Its top-left corner in window points.
    pub pos: [f32; 2],
    pub groups: Vec<Group>,
}

impl FloatStrip {
    pub(crate) fn size(&self) -> Vec2 {
        let icons: usize = self.groups.iter().map(|g| g.panels.len()).sum();
        vec2(STRIP_W, TITLE + strip::GRIP * self.groups.len() as f32 + strip::ICON_ROW * icons as f32 + STRIP_FOOT)
    }

    pub(crate) fn rect(&self, region: Rect) -> Rect {
        clamp_into(Rect::from_min_size(pos2(self.pos[0], self.pos[1]), self.size()), region)
    }
}

/// Floating panel `rect` resized with the pointer at `p` (already offset by where it holds the
/// corner): from its lower-right corner (`corner`) or its right edge; at least a box's minimum, at
/// most to `region`'s edges.
pub(crate) fn resized(rect: Rect, p: egui::Pos2, corner: bool, region: Rect) -> Rect {
    let max = region.max - rect.min;
    let w = (p.x - rect.left()).clamp(BOX_MIN.x.min(max.x), max.x.max(1.0));
    let h = if corner { (p.y - rect.top()).clamp(BOX_MIN.y.min(max.y), max.y.max(1.0)) } else { rect.height() };
    Rect::from_min_size(rect.min, vec2(w, h))
}

/// `rect` moved (and shrunk only when bigger) to lie inside `region`.
pub(crate) fn clamp_into(rect: Rect, region: Rect) -> Rect {
    let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
    let size =
        vec2(finite(rect.width(), DEFAULT_SIZE.x), finite(rect.height(), DEFAULT_SIZE.y)).max(vec2(1.0, 1.0)).min(region.size().max(vec2(1.0, 1.0)));
    let x = finite(rect.left(), region.left()).clamp(region.left(), (region.right() - size.x).max(region.left()));
    let y = finite(rect.top(), region.top()).clamp(region.top(), (region.bottom() - size.y).max(region.top()));
    Rect::from_min_size(pos2(x, y), size)
}

fn float_area(id: u32) -> Id {
    Id::unique("varos-shell-float").with(id)
}

fn strip_area(id: u32) -> Id {
    Id::unique("varos-shell-float-strip").with(id)
}

/// The layer floating panel `id` is drawn on.
pub(crate) fn float_layer(id: u32) -> LayerId {
    LayerId::new(Order::Middle, float_area(id))
}

/// The layer floating strip `id` is drawn on.
pub(crate) fn strip_layer(id: u32) -> LayerId {
    LayerId::new(Order::Middle, strip_area(id))
}

/// What a floating panel or strip asks for after it is drawn.
enum Ask {
    /// `×`: its panels go back to the Board's strip.
    Close,
    /// A floating panel's `<<`: it becomes a lone icon.
    ToIcon,
    /// A floating strip's `>>`: each group becomes a floating panel.
    Expand,
    /// The ≡ menu's Close Panel, or a tab double-clicked.
    Box(boxes::Ask),
}

/// Draw every floating panel, then every floating strip (and its flyout), inside `region`.
pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut egui::Ui, region: Rect, hits: &mut Vec<(Hit, Rect)>) {
    let floats = app.ui.varos_shell.floats.clone();
    for f in &floats {
        float(app, ui, region, f, hits);
    }
    let strips = app.ui.varos_shell.float_strips.clone();
    for s in &strips {
        float_strip(app, ui, region, s, hits);
    }
}

/// A title strip button (`×`, `<<`, `>>`): true when clicked.
fn button(ui: &mut egui::Ui, rect: Rect, id: Id, glyph: &str, label: &str, t: &Tokens) -> bool {
    let response = ui.interact(rect, id, Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(3), t.hover);
    }
    let color = if response.hovered() { t.text_strong } else { t.text_dim };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(10.0), color);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    response.on_hover_text(label).clicked()
}

fn float(app: &mut VectorcraftApp, ui: &mut egui::Ui, region: Rect, f: &Float, hits: &mut Vec<(Hit, Rect)>) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let id = f.panels.id;
    let area = float_area(id);
    let layer = float_layer(id);
    let rect = f.shown(region);
    ctx.memory_mut(|m| m.areas_mut().set_state(layer, egui::AreaState { pivot_pos: Some(rect.min), size: Some(rect.size()), ..Default::default() }));
    let title = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE));
    let close = Rect::from_min_size(title.min + vec2(3.0, 1.0), BUTTON);
    let to_icon = Rect::from_min_size(pos2(title.right() - 3.0 - BUTTON.x, title.top() + 1.0), BUTTON);
    let corner = Rect::from_min_max(rect.max - vec2(CORNER, CORNER), rect.max);
    let edge = Rect::from_min_max(pos2(rect.right() - EDGE, title.bottom() + ROW), pos2(rect.right(), rect.bottom() - CORNER));
    let mut ask = None;
    egui::Area::new(area).order(Order::Middle).fixed_pos(rect.min).constrain(false).fade_in(false).movable(false).show(&ctx, |ui| {
        ui.set_min_size(rect.size());
        ui.set_clip_rect(region);
        let painter = ui.painter().clone();
        painter.rect_filled(rect, CornerRadius::same(boxes::RADIUS), t.panel);
        // The title strip moves it (under its buttons, which take the clicks).
        let grab = ui.interact(title, area.with("title"), Sense::drag());
        if grab.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
        if grab.drag_started() {
            drag::begin(app, &ctx, Item::Float { id, from: f.rect }, title);
        }
        if button(ui, close, area.with("close"), "×", "Close", &t) {
            ask = Some(Ask::Close);
        }
        if button(ui, to_icon, area.with("icon"), "<<", "Collapse to icon", &t) {
            ask = Some(Ask::ToIcon);
        }
        let body = Rect::from_min_max(title.left_bottom(), rect.max);
        if let Some(a) = boxes::show(app, ui, &f.panels, body, true, hits) {
            ask = Some(Ask::Box(a));
        }
        if !f.panels.collapsed {
            // Above the body: the resize handles take the pointer there.
            let corner_response = ui.interact(corner, area.with("corner"), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeNwSe);
            let edge_response = ui.interact(edge, area.with("edge"), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            for offset in [vec2(0.0, 3.0), vec2(3.0, 3.0), vec2(3.0, 0.0)] {
                painter.circle_filled(rect.max - vec2(4.0, 4.0) - offset, 0.7, t.text_dim);
            }
            // A resize is a drag ([`drag::Item::Resize`]): applied before the float is drawn, and
            // Escape or a lost focus or pointer puts the old size back.
            if corner_response.drag_started() {
                drag::begin(app, &ctx, Item::Resize { id, from: f.rect, corner: true }, rect);
            } else if edge_response.drag_started() {
                drag::begin(app, &ctx, Item::Resize { id, from: f.rect, corner: false }, rect);
            }
        }
        painter.hline((rect.left() + 1.0)..=(rect.right() - 1.0), title.bottom() - 0.5, Stroke::new(1.0, t.border));
        painter.rect_stroke(rect, CornerRadius::same(boxes::RADIUS), Stroke::new(1.0, t.border), StrokeKind::Inside);
    });
    hits.push((Hit::Float(id), rect));
    hits.push((Hit::FloatTitle(id), title));
    hits.push((Hit::FloatClose(id), close));
    hits.push((Hit::FloatToIcon(id), to_icon));
    if !f.panels.collapsed {
        hits.push((Hit::FloatResize(id), corner));
        hits.push((Hit::FloatResizeRight(id), edge));
    }
    let shell = &mut app.ui.varos_shell;
    match ask {
        Some(Ask::Close) => {
            shell.close_float(id);
            ctx.request_repaint();
        }
        Some(Ask::ToIcon) => {
            shell.float_to_icon(id);
            ctx.request_repaint();
        }
        Some(Ask::Box(boxes::Ask::Close(panel))) => {
            shell.close_tab(&panel);
            ctx.request_repaint();
        }
        Some(Ask::Box(boxes::Ask::Collapse(b))) => {
            shell.toggle_collapsed(b);
            ctx.request_repaint();
        }
        Some(Ask::Box(boxes::Ask::Duplicate(panel))) => {
            super::duplicate_properties(app, &panel);
            ctx.request_repaint();
        }
        // A floating panel is never maximized (it lies above the boxes).
        Some(Ask::Box(boxes::Ask::Maximize(_)) | Ask::Expand) | None => {}
    }
}

fn float_strip(app: &mut VectorcraftApp, ui: &mut egui::Ui, region: Rect, s: &FloatStrip, hits: &mut Vec<(Hit, Rect)>) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let id = s.id;
    let sref = StripRef::Float(id);
    let area = strip_area(id);
    let layer = strip_layer(id);
    let rect = s.rect(region);
    ctx.memory_mut(|m| m.areas_mut().set_state(layer, egui::AreaState { pivot_pos: Some(rect.min), size: Some(rect.size()), ..Default::default() }));
    let title = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE));
    let close = Rect::from_min_size(title.min + vec2(3.0, 1.0), BUTTON);
    let expand = Rect::from_min_size(pos2(title.right() - 3.0 - BUTTON.x, title.top() + 1.0), BUTTON);
    let mut ask = None;
    let mut tops = Vec::new();
    let mut icons = Vec::new();
    egui::Area::new(area).order(Order::Middle).fixed_pos(rect.min).constrain(false).fade_in(false).movable(false).show(&ctx, |ui| {
        ui.set_min_size(rect.size());
        ui.set_clip_rect(region);
        let painter = ui.painter().clone();
        painter.rect_filled(rect, CornerRadius::same(boxes::RADIUS), t.panel);
        // The title strip drags the whole strip (to the Board's edge, or elsewhere).
        let grab = ui.interact(title, area.with("title"), Sense::drag());
        if grab.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
        if grab.drag_started() {
            drag::begin(app, &ctx, Item::Strip(id), title);
        }
        if button(ui, close, area.with("close"), "×", "Close", &t) {
            ask = Some(Ask::Close);
        }
        if button(ui, expand, area.with("expand"), ">>", "Expand panels", &t) {
            ask = Some(Ask::Expand);
        }
        let mut column = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("varos-shell-float-strip-column", id))
                .max_rect(Rect::from_min_max(title.left_bottom(), rect.max))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        column.spacing_mut().item_spacing.y = 0.0;
        strip::column(app, &mut column, sref, &s.groups, rect.width(), rect, hits, &mut tops, &mut icons);
        painter.hline((rect.left() + 1.0)..=(rect.right() - 1.0), title.bottom() - 0.5, Stroke::new(1.0, t.border));
        painter.rect_stroke(rect, CornerRadius::same(boxes::RADIUS), Stroke::new(1.0, t.border), StrokeKind::Inside);
    });
    hits.push((Hit::FloatStrip(id), rect));
    hits.push((Hit::FloatStripTitle(id), title));
    hits.push((Hit::FloatStripClose(id), close));
    hits.push((Hit::FloatStripExpand(id), expand));
    strip::flyout(app, ui, region, rect, &tops, sref, hits);
    // Painted after its flyout's input: the open panel's icon is lit on the click frame.
    let mut paint_ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(rect));
    paint_ui.set_clip_rect(rect.intersect(region));
    strip::paint_icons(app, &paint_ui, icons, Mode::Icons, sref, hits);
    let shell = &mut app.ui.varos_shell;
    match ask {
        Some(Ask::Close) => {
            shell.close_float_strip(id);
            ctx.request_repaint();
        }
        Some(Ask::Expand) => {
            shell.expand_float_strip(id);
            ctx.request_repaint();
        }
        _ => {}
    }
}
