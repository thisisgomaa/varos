//! Shared approved chrome. Rendering does not change layout or input.

use egui::{Align2, CornerRadius, FontId, Rect, Stroke, StrokeKind, Ui, Vec2, pos2};

use super::{
    metrics,
    registry::{self, PanelIcon},
};
use crate::icons;
use crate::theme::{self, Tokens};

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
        ui.painter().rect_filled(rect, CornerRadius::same(2), t.hover);
    }
    icons::paint(ui, name, Rect::from_center_size(rect.center(), Vec2::splat(CONTROL)), if hovered { t.text } else { t.text_dim });
}

/// Flat adjacent tabs; an active tab shares the body surface and has no bottom dividing line.
pub(super) fn tab(ui: &Ui, rect: Rect, label: &str, active: bool, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(
        rect,
        0.0,
        if active {
            t.panel
        } else if hovered {
            t.hover
        } else {
            t.panel_darker
        },
    );
    p.line_segment([pos2(rect.right() - 0.5, rect.top()), pos2(rect.right() - 0.5, rect.bottom())], Stroke::new(metrics::BORDER, t.border));
    if !active {
        p.line_segment([pos2(rect.left(), rect.bottom() - 0.5), pos2(rect.right(), rect.bottom() - 0.5)], Stroke::new(metrics::BORDER, t.border));
    }
    p.text(
        pos2(rect.left() + metrics::TAB_PAD, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::semibold(metrics::FONT),
        if active || hovered { t.text } else { t.text_dim },
    );
}

/// One compact panel row, with an unchanged glyph size when its name becomes visible.
pub(super) fn icon_row(ui: &Ui, rect: Rect, panel: &str, with_name: bool, active: bool, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, CornerRadius::same(3), if hovered { t.hover } else { t.panel });
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

pub(super) fn label(panel: &str) -> &'static str {
    registry::label(panel)
}
