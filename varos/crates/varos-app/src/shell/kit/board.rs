//! Start v2 — Boards kit controls (lane L4): the hero button, the text button, tag / Missing pills, the
//! key chip, the filter tab, the segmented view toggle, the preset cell, the board card surface and its
//! "…" chip, the table row, and THE outside focus ring.
//!
//! Every control here is placed at an explicit rect (the page computes its layout once) and is
//! pointer-only (`Sense::CLICK`): the Start page owns the keyboard (K2 row 2) and passes `focused`
//! while its ring is visible. Text arrives as galleys laid out in `Color32::PLACEHOLDER`, so the
//! control picks the state colour. No host commands, no I/O, no animation, tokens only.
// ---- Lane F: text adapters ----
use crate::shell::kit::text::ShapedResponse as _;
// ---- end Lane F ----
use crate::shell::kit::text::ShapedPainter as _;
use std::sync::Arc;

use egui::{Color32, CornerRadius, Galley, Id, Painter, PointerButton, Rect, Sense, Stroke, StrokeKind, Ui};

use super::{Availability, ControlResponse, Icon};
use crate::shell::tokens as t;

/// K5 focus-visible: a 1 px `gap`-coloured ring hugging `rect`, then the 2 px azure ring outside it
/// (CSS `outline: 2px; outline-offset: 1px`). Painted on the layer's full clip so it is never cut.
pub fn focus_ring(painter: &Painter, rect: Rect, radius: u8, gap: Color32) {
    let gap_rect = rect.expand(t::KIT_FOCUS_GAP / 2.0);
    painter.rect_stroke(gap_rect, radius, Stroke::new(t::KIT_FOCUS_GAP, gap), StrokeKind::Middle);
    let ring = rect.expand(t::KIT_FOCUS_GAP);
    let r = radius.saturating_add(t::KIT_FOCUS_GAP as u8);
    painter.rect_stroke(ring, r, Stroke::new(t::KIT_FOCUS_STROKE, t::ACCENT), StrokeKind::Outside);
}

fn interact(ui: &mut Ui, id: Id, rect: Rect, label: &str, enabled: bool) -> ControlResponse {
    let sense = if enabled { Sense::CLICK } else { Sense::hover() };
    let response = ui.interact(rect, id, sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let activated = enabled && response.clicked_by(PointerButton::Primary);
    ControlResponse { response, activated }
}

fn hovered(r: &ControlResponse) -> bool {
    r.response.hovered() || r.response.is_pointer_button_down_on()
}

/// Paint `galley` so its first line's baseline sits on `baseline`; returns the galley's rect.
pub fn galley_at_baseline(painter: &Painter, x: f32, baseline: f32, galley: Arc<Galley>, color: Color32) -> Rect {
    let rect = Rect::from_min_size(egui::pos2(x, baseline - first_baseline(&galley)), galley.size());
    painter.shaped_galley(rect.min, galley, color);
    rect
}

/// The first row's baseline, measured from the galley's top.
pub fn first_baseline(galley: &Galley) -> f32 {
    galley.rows.first().and_then(|row| row.row.glyphs.first().map(|g| row.pos.y + g.pos.y)).unwrap_or(galley.size().y)
}

/// Paint `galley` centred vertically in a line box `[top, top + line)` (CSS half-leading).
pub fn galley_in_line(painter: &Painter, x: f32, top: f32, line: f32, galley: Arc<Galley>, color: Color32) -> Rect {
    let rect = Rect::from_min_size(egui::pos2(x, top + (line - galley.size().y) / 2.0), galley.size());
    painter.shaped_galley(rect.min, galley, color);
    rect
}

/// The hero's big buttons (src.html `.big`): 272 × 64, icon 20, title + sub-label, shortcut right.
/// `primary` = inverted (TEXT fill, BG title, ON_TEXT_MUTED sub-label); else SURFACE + LINE2 border.
pub struct BigButton {
    pub icon: Icon,
    pub title: Arc<Galley>,
    pub sub: Arc<Galley>,
    pub shortcut: Arc<Galley>,
    pub primary: bool,
    pub focused: bool,
}
pub fn big_button(ui: &mut Ui, id: Id, rect: Rect, b: BigButton, label: &str) -> ControlResponse {
    let r = interact(ui, id, rect, label, true);
    let hover = hovered(&r);
    let p = ui.painter();
    let (fill, ink, weak) = if b.primary {
        (t::TEXT, t::BG, t::ON_TEXT_MUTED)
    } else {
        (if hover { t::HOVER } else { t::SURFACE }, t::TEXT, t::MUTED)
    };
    p.rect_filled(rect, t::r_ctrl(), fill);
    if !b.primary {
        p.rect_stroke(rect, t::r_ctrl(), Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Inside);
    }
    let icon_c = egui::pos2(rect.left() + t::SB_BIG_PAD_L + t::SB_ICON_HERO / 2.0, rect.center().y);
    b.icon.paint(p, icon_c, t::SB_ICON_HERO, ink);
    let x = rect.left() + t::SB_BIG_PAD_L + t::SB_ICON_HERO + t::SB_BIG_GAP;
    let block = b.title.size().y + t::SB_BIG_SUB_GAP + b.sub.size().y;
    let top = rect.center().y - block / 2.0;
    let title_h = b.title.size().y;
    p.shaped_galley(egui::pos2(x, top), b.title, ink);
    p.shaped_galley(egui::pos2(x, top + title_h + t::SB_BIG_SUB_GAP), b.sub, weak);
    let kb =
        egui::pos2(rect.right() - t::SB_BIG_PAD_R - b.shortcut.size().x, rect.center().y - b.shortcut.size().y / 2.0);
    p.shaped_galley(kb, b.shortcut, weak);
    if b.focused {
        focus_ring(&ui.painter().with_clip_rect(ui.clip_rect()), rect, t::R, t::BG);
    }
    r
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// Bare MUTED text (Discard).
    Ghost,
    /// SURFACE fill + LINE2 border, TEXT (Recover).
    Solid,
}
/// A 28-tall text button (src.html `.btn`). Disabled / busy: DISABLED text, never activates, the reason
/// is the tooltip.
#[allow(clippy::too_many_arguments)]
pub fn text_button(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    label: Arc<Galley>,
    kind: ButtonKind,
    availability: Availability<'_>,
    focused: bool,
    name: &str,
) -> ControlResponse {
    let reason = match availability {
        Availability::Enabled => None,
        Availability::Disabled(r) | Availability::Busy(r) => Some(r),
    };
    let r = interact(ui, id, rect, name, reason.is_none());
    let hover = reason.is_none() && hovered(&r);
    let p = ui.painter();
    match kind {
        ButtonKind::Solid => {
            p.rect_filled(rect, t::r_ctrl(), if hover { t::HOVER } else { t::SURFACE });
            p.rect_stroke(rect, t::r_ctrl(), Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Inside);
        }
        ButtonKind::Ghost if hover => {
            p.rect_filled(rect, t::r_ctrl(), t::HOVER);
        }
        ButtonKind::Ghost => {}
    }
    let ink = match (reason, kind, hover) {
        (Some(_), _, _) => t::DISABLED,
        (None, ButtonKind::Ghost, false) => t::MUTED,
        _ => t::TEXT,
    };
    let pos = rect.center() - label.size() / 2.0;
    p.shaped_galley(pos, label, ink);
    if focused && reason.is_none() {
        focus_ring(&ui.painter().with_clip_rect(ui.clip_rect()), rect, t::R, t::PANEL);
    }
    match reason {
        Some(reason) => ControlResponse { response: r.response.shaped_hover_text(reason), activated: false },
        None => r,
    }
}

/// THE recovery box (src.html `.rec`): PANEL fill, 1 px LINE2 border, 8 px corners. Start's Recovered
/// band, the editor's recovery card and its Review panel are all this one box.
pub fn recovery_box(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, t::r_box(), t::PANEL);
    painter.rect_stroke(rect, t::r_box(), Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Inside);
}

/// THE Recovered band: [`recovery_box`] with the history glyph (20) at 20 in, vertically centred.
/// Returns the x where the band's text starts (the glyph + its 14 gap).
pub fn recovered_band(painter: &Painter, rect: Rect) -> f32 {
    recovery_box(painter, rect);
    recovered_glyph(painter, rect)
}

/// History glyph and text origin, for a band whose outer box is painted by its caller.
pub fn recovered_glyph(painter: &Painter, rect: Rect) -> f32 {
    let icon_x = rect.left() + t::SB_RECOV_PAD_L;
    let icon_c = egui::pos2(icon_x + t::SB_ICON_HERO / 2.0, rect.center().y);
    Icon::History.paint(painter, icon_c, t::SB_ICON_HERO, t::TEXT);
    icon_x + t::SB_ICON_HERO + t::SB_RECOV_ICON_GAP
}

/// Width of a pill holding `label` with `pad` each side.
pub fn pill_width(label: &Galley, pad: f32) -> f32 {
    label.size().x + pad * 2.0
}

/// A tag pill (src.html `.tg`): SURFACE capsule, MUTED 11/500 text. Static.
pub fn tag_pill(painter: &Painter, rect: Rect, label: Arc<Galley>) {
    painter.rect_filled(rect, t::RCAP, t::SURFACE);
    painter.shaped_galley(rect.center() - label.size() / 2.0, label, t::MUTED);
}

/// The "Missing" pill (src.html `.pill-miss`): LINE2 capsule outline, MUTED text. Static.
pub fn outline_pill(painter: &Painter, rect: Rect, label: Arc<Galley>) {
    painter.rect_stroke(rect, t::RCAP, Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Inside);
    painter.shaped_galley(rect.center() - label.size() / 2.0, label, t::MUTED);
}

/// A key chip (src.html `kbd`): LINE2 outline, 3 px corners, TEXT mono label. Static.
pub fn kbd(painter: &Painter, rect: Rect, label: Arc<Galley>) {
    painter.rect_stroke(rect, t::r_ctrl(), Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Inside);
    painter.shaped_galley(rect.center() - label.size() / 2.0, label, t::TEXT);
}

/// One tag-filter tab (src.html `.flt`): label + mono count, centred in the row; the selected one is
/// TEXT with the 2 px azure bar under it (inset by the padding), the rest MUTED. Hover = TEXT label.
#[allow(clippy::too_many_arguments)]
pub fn filter_tab(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    label: Arc<Galley>,
    count: Arc<Galley>,
    selected: bool,
    focused: bool,
    name: &str,
) -> ControlResponse {
    let r = interact(ui, id, rect, name, true);
    let hover = hovered(&r);
    let p = ui.painter();
    let ink = if selected || hover { t::TEXT } else { t::MUTED };
    let x = rect.left() + t::SB_FILTER_PAD;
    let lw = label.size().x;
    galley_in_line(p, x, rect.top(), rect.height(), label, ink);
    galley_in_line(p, x + lw + t::SB_FILTER_INNER, rect.top(), rect.height(), count, t::MUTED);
    if selected {
        let bar = Rect::from_min_max(
            egui::pos2(rect.left() + t::SB_FILTER_PAD, rect.bottom() - t::SB_FILTER_BAR),
            egui::pos2(rect.right() - t::SB_FILTER_PAD, rect.bottom()),
        );
        p.rect_filled(bar, CornerRadius::same(t::SB_FILTER_BAR_R), t::ACCENT);
    }
    if focused {
        focus_ring(&ui.painter().with_clip_rect(ui.clip_rect()), rect, t::R, t::BG);
    }
    r
}

/// The segmented view toggle (src.html `.seg`): a LINE outline holding icon segments; the selected
/// segment has a TOGGLE_WELL fill and TEXT glyph, the others MUTED. Returns the activated segment.
pub struct SegmentedFrame {
    pub chosen: Option<usize>,
    pub rects: Vec<Rect>,
    pub hot: Vec<bool>,
}

/// The outside size of one `.seg` track. The stroke and one-point pad surround the segments; the
/// same one-point token separates adjacent segments.
pub fn segmented_size(count: usize, segment: egui::Vec2) -> egui::Vec2 {
    let gaps = count.saturating_sub(1) as f32 * t::SB_SEG_PAD;
    egui::vec2(
        count as f32 * segment.x + gaps + (t::KIT_STROKE + t::SB_SEG_PAD) * 2.0,
        segment.y + (t::KIT_STROKE + t::SB_SEG_PAD) * 2.0,
    )
}

/// Paint and interact with the one shared `.seg` track. Hosts paint their icon/text content into the
/// returned segment rects. `help` is reserved for icon-only segments or useful explanations; visible
/// text labels must not be repeated as tooltips.
pub fn segmented_frame(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    names: &[&str],
    selected: usize,
    segment: egui::Vec2,
    help: Option<&[&str]>,
) -> SegmentedFrame {
    ui.painter().rect_stroke(rect, CornerRadius::same(t::R), Stroke::new(t::KIT_STROKE, t::LINE), StrokeKind::Inside);
    let mut chosen = None;
    let mut rects = Vec::with_capacity(names.len());
    let mut hot = Vec::with_capacity(names.len());
    let inner = rect.shrink(t::KIT_STROKE + t::SB_SEG_PAD);
    for (i, name) in names.iter().enumerate() {
        let gap = if names.len() > 1 {
            (inner.width() - names.len() as f32 * segment.x) / (names.len() - 1) as f32
        } else {
            0.0
        };
        let x = inner.left() + i as f32 * (segment.x + gap);
        let seg = Rect::from_min_size(egui::pos2(x, inner.top()), segment);
        let lift = ((t::KIT_MIN_TARGET - seg.height()) / 2.0).max(0.0);
        let r = interact(ui, id.with(i), seg.expand2(egui::vec2(0.0, lift)), name, true);
        let is_hot = hovered(&r);
        if i == selected || is_hot {
            ui.painter().rect_filled(
                seg,
                CornerRadius::same(t::SB_SEG_R),
                if i == selected { t::TOGGLE_WELL } else { t::HOVER },
            );
        }
        let response = help.map_or(r.response.clone(), |tips| r.response.shaped_hover_text(tips[i]));
        if response.clicked_by(PointerButton::Primary) {
            chosen = Some(i);
        }
        rects.push(seg);
        hot.push(is_hot);
    }
    SegmentedFrame { chosen, rects, hot }
}

pub fn segmented(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    segments: &[(Icon, &str)],
    selected: usize,
    focused: Option<usize>,
) -> (Option<usize>, Vec<Rect>) {
    segmented_sized(
        ui,
        id,
        rect,
        segments,
        selected,
        focused,
        egui::vec2(t::SB_SEG_BTN_W, t::SB_SEG_BTN_H),
        t::SB_ICON_SMALL,
    )
}

/// Panel hosts reuse the shared segment interaction and paint with their own token geometry.
#[allow(clippy::too_many_arguments)]
pub fn segmented_sized(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    segments: &[(Icon, &str)],
    selected: usize,
    focused: Option<usize>,
    segment: egui::Vec2,
    glyph: f32,
) -> (Option<usize>, Vec<Rect>) {
    debug_assert!(segments.iter().all(|(_, name)| !name.trim().is_empty()), "icon segments require tooltips");
    let names = segments.iter().map(|(_, name)| *name).collect::<Vec<_>>();
    let frame = segmented_frame(ui, id, rect, &names, selected, segment, Some(&names));
    for (i, (icon, _)) in segments.iter().enumerate() {
        let seg = frame.rects[i];
        let on = i == selected;
        let hover = frame.hot[i];
        let p = ui.painter();
        icon.paint(p, seg.center(), glyph, if on || hover { t::TEXT } else { t::MUTED });
        if focused == Some(i) {
            focus_ring(&ui.painter().with_clip_rect(ui.clip_rect()), seg, t::SB_SEG_R, t::BG);
        }
    }
    (frame.chosen, frame.rects)
}

/// One preset cell (src.html `.pr`): the artboard's outline (`outline` = its size at the shared
/// scale) on a shared baseline, name 13/500, mono size. Hover = a ROW_HOVER wash; the focus ring is
/// drawn just inside the cell so it never crosses the panel edge.
#[allow(clippy::too_many_arguments)]
pub fn preset_cell(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    outline: egui::Vec2,
    name_line: (Arc<Galley>, f32),
    size_line: (Arc<Galley>, f32),
    focused: bool,
    label: &str,
) -> ControlResponse {
    let r = interact(ui, id, rect, label, true);
    let hover = hovered(&r);
    let p = ui.painter();
    if hover {
        p.rect_filled(rect, CornerRadius::ZERO, t::ROW_HOVER);
    }
    let base = rect.top() + t::SB_PRESET_TOP + t::SB_PRESET_PV;
    let ab = Rect::from_min_max(
        egui::pos2(rect.center().x - outline.x / 2.0, base - outline.y),
        egui::pos2(rect.center().x + outline.x / 2.0, base),
    );
    p.rect_filled(ab, CornerRadius::ZERO, t::BG);
    p.rect_stroke(ab, CornerRadius::ZERO, Stroke::new(t::KIT_STROKE, t::MUTED), StrokeKind::Inside);
    let (name, name_top) = name_line;
    let (size, size_top) = size_line;
    p.shaped_galley(egui::pos2(rect.center().x - name.size().x / 2.0, name_top), name, t::TEXT);
    p.shaped_galley(egui::pos2(rect.center().x - size.size().x / 2.0, size_top), size, t::MUTED);
    if focused {
        let inset = t::KIT_FOCUS_GAP + t::KIT_FOCUS_STROKE + t::KIT_STROKE;
        focus_ring(&ui.painter().with_clip_rect(ui.clip_rect()), rect.shrink(inset), t::R, t::PANEL);
    }
    r
}

/// A 1 px dashed rectangle (CSS `border: 1px dashed`).
pub fn dashed_rect(painter: &Painter, rect: Rect, color: Color32) {
    let pts = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(t::KIT_STROKE, color), t::SB_DASH, t::SB_DASH_GAP));
}

/// What a board card shows around its content.
#[derive(Clone, Copy, Debug, Default)]
pub struct CardState {
    pub focused: bool,
    pub menu_open: bool,
}
/// The board card surface (src.html `.bc`): PANEL with an invisible border at rest; hover / open
/// menu = ROW_HOVER + LINE2 (keyboard focus is the ring, drawn by [`card_border`]). "Hover" is the pointer
/// anywhere over the card, including over its own "…" chip. Paints the frame and the well background (BG, top corners 7, LINE
/// under it); the page paints the content. Returns the card response and whether the "…" chip shows.
pub fn card(ui: &mut Ui, id: Id, rect: Rect, well: Rect, state: CardState, label: &str) -> (ControlResponse, bool) {
    let r = interact(ui, id, rect, label, true);
    let lit = r.response.contains_pointer() || r.response.is_pointer_button_down_on() || state.menu_open;
    let p = ui.painter();
    p.rect_filled(rect, t::r_box(), if lit { t::ROW_HOVER } else { t::PANEL });
    let wr = CornerRadius { nw: t::SB_WELL_R, ne: t::SB_WELL_R, sw: 0, se: 0 };
    p.rect_filled(well, wr, t::BG);
    p.hline(well.x_range(), well.bottom() - t::KIT_STROKE / 2.0, t::hairline());
    (r, lit)
}
/// Paint the card's border last (over the well) and its ring.
pub fn card_border(ui: &Ui, rect: Rect, lit: bool, focused: bool) {
    let p = ui.painter();
    p.rect_stroke(
        rect,
        t::r_box(),
        Stroke::new(t::KIT_STROKE, if lit { t::LINE2 } else { t::PANEL }),
        StrokeKind::Inside,
    );
    if focused {
        focus_ring(&ui.painter().with_clip_rect(ui.clip_rect()), rect, t::RBOX, t::BG);
    }
}

/// The card's "…" chip (src.html `.more`): 24 × 24, SURFACE + LINE2, HOVER while its menu is open.
pub fn more_chip(ui: &mut Ui, id: Id, rect: Rect, open: bool) -> ControlResponse {
    let r = interact(ui, id, rect, "Board actions", true);
    let p = ui.painter();
    p.rect_filled(rect, t::r_ctrl(), if open || hovered(&r) { t::HOVER } else { t::SURFACE });
    p.rect_stroke(rect, t::r_ctrl(), Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Inside);
    Icon::More.paint(p, rect.center(), t::ICON_MD, t::TEXT);
    let response = r.response.shaped_hover_text("Locate or remove from Recent");
    ControlResponse { response, activated: r.activated }
}

/// One list-view table row (src.html `.tr.row`): LINE hairline under it, ROW_HOVER on hover/focus.
pub fn table_row(ui: &mut Ui, id: Id, rect: Rect, focused: bool, label: &str) -> ControlResponse {
    let r = interact(ui, id, rect, label, true);
    let p = ui.painter();
    if hovered(&r) || focused {
        p.rect_filled(rect, CornerRadius::ZERO, t::ROW_HOVER);
    }
    p.hline(rect.x_range(), rect.bottom() - t::KIT_STROKE / 2.0, t::hairline());
    if focused {
        focus_ring(&ui.painter().with_clip_rect(ui.clip_rect()), rect, t::R, t::BG);
    }
    r
}

/// Width of a removable tag chip holding `label` (the tag field in the Board section).
pub fn tag_chip_width(label: &Galley) -> f32 {
    t::SB_PILL_PAD + label.size().x + t::SB_CHIP_GAP + t::SB_CHIP_X + t::SB_CHIP_PAD_R
}

/// A removable tag chip (the Board section's tag field): the tag pill's look (SURFACE capsule, MUTED
/// 11/500) with an × at its end. `activated` = the × was clicked (remove this tag). The × glyph is a
/// registry icon; its hit target is a `KIT_MIN_TARGET` square around it.
pub fn tag_chip(ui: &mut Ui, id: Id, rect: Rect, label: Arc<Galley>, name: &str) -> ControlResponse {
    let x_center = egui::pos2(rect.right() - t::SB_CHIP_PAD_R - t::SB_CHIP_X / 2.0, rect.center().y);
    // the × is drawn small; its hit target is the kit minimum square around it
    let r = interact(ui, id, Rect::from_center_size(x_center, egui::Vec2::splat(t::KIT_MIN_TARGET)), name, true);
    let hover = hovered(&r);
    let p = ui.painter();
    p.rect_filled(rect, t::RCAP, t::SURFACE);
    let text_pos = egui::pos2(rect.left() + t::SB_PILL_PAD, rect.center().y - label.size().y / 2.0);
    p.shaped_galley(text_pos, label, t::MUTED);
    Icon::Remove.paint(p, x_center, t::SB_CHIP_X, if hover { t::TEXT } else { t::MUTED });
    let response = r.response.shaped_hover_text(crate::i18n::message(ui.ctx(), "Remove {name}", &[("name", name)]));
    ControlResponse { response, activated: r.activated }
}
