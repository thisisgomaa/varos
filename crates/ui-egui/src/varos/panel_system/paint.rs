//! Shared approved chrome. Rendering does not change layout or input.

use crate::varos::theme_components::FOUNDATION as LOOK;

use egui::{Align2, CornerRadius, FontId, Rect, Stroke, StrokeKind, Ui, Vec2, pos2};

use super::{
    geometry::Measurements,
    metrics,
    registry::{self, PanelIcon},
};
use crate::icons;
use crate::theme::Tokens;

// Keep existing geometry call sites stable while sharing one density source.
pub(super) use super::metrics::{COMPACT, CONTROL, FOOT, GRIP, ICON, ROW, TABS, TITLE};

/// Opaque shared frame with a dark drag band and one outer hairline.
pub(super) fn frame(ui: &Ui, rect: Rect, docked: bool, title_height: f32) {
    let t = Tokens::get(ui.ctx());
    let p = ui.painter();
    let radius = if docked { 0 } else { metrics::FRAME_RADIUS };
    p.rect_filled(rect, CornerRadius::same(radius), t.panel);
    if title_height > 0.0 {
        let header = Rect::from_min_max(rect.min, pos2(rect.right(), (rect.top() + title_height).min(rect.bottom())));
        p.rect_filled(header, CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, t.panel_darker);
        p.line_segment([pos2(rect.left(), header.bottom()), pos2(rect.right(), header.bottom())], Stroke::new(metrics::BORDER, t.border));
    }
    p.rect_stroke(rect, CornerRadius::same(radius), Stroke::new(metrics::BORDER, t.border), StrokeKind::Inside);
}

/// The hit rectangle belongs to the caller; the glyph keeps its approved logical size.
pub(super) fn control(ui: &Ui, rect: Rect, name: &str, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    if hovered {
        ui.painter().rect_filled(rect, CornerRadius::same(LOOK.control_radius), t.hover);
    }
    icons::paint(ui, name, Rect::from_center_size(rect.center(), Vec2::splat(CONTROL)), if hovered { t.text } else { t.text_dim });
}

/// Flat adjacent tabs; an active tab shares the body surface and has no bottom dividing line.
pub(super) fn tab(ui: &Ui, rect: Rect, label: &str, active: bool, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    LOOK.tab_surface(&p, rect, &t, active, hovered);
    LOOK.tab_label(&p, rect, label, &t, active, hovered);
}

/// One compact panel row, with an unchanged glyph size when its name becomes visible.
pub(super) fn icon_row(ui: &Ui, rect: Rect, panel: &str, with_name: bool, active: bool, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, CornerRadius::same(LOOK.control_radius), if hovered { t.hover } else { t.panel });
    let center = if with_name { pos2(rect.left() + metrics::ROW_PAD + ICON * 0.5, rect.center().y) } else { rect.center() };
    let tint = if active || hovered { t.text } else { t.text_dim };
    match registry::get(panel).map(|spec| spec.icon) {
        Some(PanelIcon::Named(icon)) => icons::paint(ui, icon, Rect::from_center_size(center, Vec2::splat(ICON)), tint),
        Some(PanelIcon::SwatchesGrid) => {
            // Original geometry retained exactly from the approved panel identity.
            let grid = Rect::from_center_size(center, Vec2::splat(ICON * 0.75));
            let stroke = Stroke::new(ICON / 15.0, tint);
            p.rect_stroke(grid, 2, stroke, StrokeKind::Middle);
            p.hline(grid.x_range(), center.y, stroke);
            p.vline(center.x, grid.y_range(), stroke);
        }
        None => {}
    }
    if with_name {
        p.text(
            pos2(rect.left() + metrics::ROW_PAD + ICON + metrics::ICON_LABEL_GAP, rect.center().y),
            Align2::LEFT_CENTER,
            label(panel),
            FontId::proportional(metrics::FONT),
            if active || hovered { t.text } else { t.text_dim },
        );
    }
}

/// A panel's name as its tab, flyout row, menus and screen readers show it: localized.
pub(super) fn label(panel: &str) -> &'static str {
    tl!(registry::label(panel))
}

/// The font a tab's label is painted (and measured) in.
fn tab_font() -> FontId {
    LOOK.tab_font()
}

/// Every tab width `layout` needs: each panel's localized label measured in the tab font, plus
/// its padding on both sides, so the model lays the tabs out from real text in any language.
pub(super) fn measurements(ctx: &egui::Context, layout: &super::model::Layout) -> Measurements {
    let panels = layout.hosts.iter().flat_map(|host| super::model::groups(&host.root)).flat_map(|group| group.panels.iter());
    let widths = panels
        .map(|panel| {
            let text = ctx.fonts_mut(|fonts| fonts.layout_no_wrap(label(panel).to_owned(), tab_font(), egui::Color32::PLACEHOLDER).size().x);
            (panel.clone(), (text.ceil() + 2.0 * metrics::TAB_PAD + 2.0 * metrics::BORDER).clamp(metrics::TAB_MIN_WIDTH, metrics::TAB_MAX_WIDTH))
        })
        .collect();
    Measurements { tabs: widths, bodies: super::sizing::snapshot(ctx) }
}
