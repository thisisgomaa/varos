// ---- Lane F: shaped chrome ----
// ---- Lane F: text adapters ----
use varos_app::shell::kit::text::ShapedResponse as _;
// ---- end Lane F ----
use varos_app::shell::kit::text::ShapedPainter as _;
// ---- end Lane F ----
use super::*;

pub(crate) fn menu_open(ui: &egui::Ui, id: egui::Id) -> bool {
    ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false))
}
pub(crate) fn menu_set(ui: &egui::Ui, id: egui::Id, open: bool) {
    ui.data_mut(|d| d.insert_temp(id, open));
}
pub(crate) fn menu_toggle(ui: &egui::Ui, id: egui::Id) {
    let v = !menu_open(ui, id);
    menu_set(ui, id, v);
}

/// The dropdown body anchored below `anchor` (only when open). `flush = Some(bar_bottom)` renders
/// it as an EXTENSION of the app bar (Ahmed 07-07): seam fill, square top corners hanging straight
/// off the bar's bottom edge, and the shared edge erased — bar and menu read as ONE dark surface.
pub(crate) fn menu_below(
    ui: &egui::Ui,
    id: egui::Id,
    anchor: &egui::Response,
    flush: Option<f32>,
    add: impl FnOnce(&mut egui::Ui),
) {
    if !menu_open(ui, id) {
        return;
    }
    let ctx = ui.ctx().clone();
    // Illustrator-crisp chrome (Ahmed 07-07): sharp MENU_R corners, vertical padding only — the
    // rows run edge-to-edge so their hover strips touch the border. Flush menus keep the seam
    // fill + square top so they read as extensions of the app bar.
    let frame = egui::Frame {
        fill: if flush.is_some() { SEAM } else { SOLID_PANEL },
        stroke: Stroke::new(1.0, BORDER),
        corner_radius: if flush.is_some() {
            CornerRadius { nw: 0, ne: 0, sw: MENU_R, se: MENU_R }
        } else {
            CornerRadius::same(MENU_R)
        },
        inner_margin: Margin::symmetric(0, MENU_PAD_V),
        ..Default::default()
    };
    let pos = match flush {
        Some(y) => egui::pos2(anchor.rect.left(), y),
        None => anchor.rect.left_bottom() + egui::vec2(0.0, 4.0),
    };
    let out = egui::Area::new(id.with("menu")).order(egui::Order::Foreground).fixed_pos(pos).constrain(true).show(
        &ctx,
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0; // contiguous rows — separators bring their own air
            let r = frame.show(ui, |ui| add(ui)).response.rect;
            if flush.is_some() {
                // erase the top border segment — the menu melts into the bar, 100% one colour
                ui.painter().hline(r.left() + 1.0..=r.right() - 1.0, r.top() + 0.5, Stroke::new(1.5, SEAM));
            }
            r
        },
    );
    let rect = out.inner;
    let close = ctx.input(|i| i.key_pressed(egui::Key::Escape))
        || ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer.interact_pos().is_some_and(|p| !rect.expand(4.0).contains(p) && !anchor.rect.contains(p))
        });
    if close {
        ctx.data_mut(|d| d.insert_temp(id, false));
    }
}

// ── menu metrics (Ahmed 07-07: "مساحات محسوبة بالمللي") — ONE place, Illustrator-crisp ──
// Rows are contiguous (no inter-row gap), the hover strip bleeds edge-to-edge and is SQUARE, text
// always starts after a fixed ✓-gutter so every menu lines up, shortcuts hang right in mono.
pub(crate) const MENU_ROW_H: f32 = 26.0; // row height
pub(crate) const MENU_GUTTER: f32 = 28.0; // left column reserved for ✓ marks — text aligns after it, always
pub(crate) const MENU_PAD_V: i8 = 5; // frame's vertical padding (rows themselves are full-bleed)
pub(crate) const MENU_R: u8 = 4; // outer radius — sharp, a work tool (was 10: "ناعمة ومايعة")

/// One menu row: label left (after the gutter), shortcut right. Returns true on click.
pub(crate) fn menu_row(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let translated = varos_app::i18n::translate(ui.ctx(), label);
    let label = translated.as_ref();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), MENU_ROW_H), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, HOVER); // full-bleed, square — AI-crisp
    }
    ui.painter().shaped_text(
        egui::pos2(rect.left() + MENU_GUTTER, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        TEXT,
    );
    if !shortcut.is_empty() {
        ui.painter().shaped_text(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::monospace(10.5),
            MUTED,
        );
    }
    resp.clicked()
}

/// A menu row that ISN'T available yet (burger ▸ Export…, DFS S1 §3.6): DISABLED text, no hover fill,
/// `Sense::hover` only — it can never be clicked, so it cannot become an "enabled dead button" (spec
/// §2 forbids those). A tooltip carries the reason.
pub(crate) fn menu_row_disabled(ui: &mut egui::Ui, label: &str, tip: &str) {
    let translated = varos_app::i18n::translate(ui.ctx(), label);
    let label = translated.as_ref();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), MENU_ROW_H), egui::Sense::hover());
    ui.painter().shaped_text(
        egui::pos2(rect.left() + MENU_GUTTER, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        DISABLED,
    );
    resp.shaped_hover_text(tip);
}

/// A toggle row: same skeleton as `menu_row`, with a hand-drawn ✓ in the gutter when on —
/// Illustrator's Window-menu look (NOT a checkbox; Ahmed 07-07).
pub(crate) fn check_row(ui: &mut egui::Ui, label: &str, checked: bool) -> bool {
    let translated = varos_app::i18n::translate(ui.ctx(), label);
    let label = translated.as_ref();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), MENU_ROW_H), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, HOVER);
    }
    if checked {
        let c = egui::pos2(rect.left() + 14.0, rect.center().y);
        let knee = c + egui::vec2(-1.2, 2.8);
        ui.painter().line_segment([c + egui::vec2(-4.0, -0.2), knee], Stroke::new(1.6, TEXT));
        ui.painter().line_segment([knee, c + egui::vec2(4.2, -3.4)], Stroke::new(1.6, TEXT));
    }
    ui.painter().shaped_text(
        egui::pos2(rect.left() + MENU_GUTTER, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        TEXT,
    );
    resp.clicked()
}

/// Full-width menu separator with even breath above/below.
pub(crate) fn menu_sep(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(r.left()..=r.right(), r.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(4.0);
}

/// The egui event a ⌘-clipboard key means to a focused text field — the same mapping egui-winit
/// applies to real key presses (`is_copy_command` & co.). `None` = not a clipboard key (forward the
/// key itself); `Some(None)` = ⌘V with nothing pasteable (egui-winit then sends nothing either).
/// `clipboard` is only read for ⌘V.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // the caller is the macOS menu hand-off
pub(crate) fn text_clipboard_event(
    key: egui::Key,
    clipboard: impl FnOnce() -> Option<String>,
) -> Option<Option<egui::Event>> {
    match key {
        egui::Key::C => Some(Some(egui::Event::Copy)),
        egui::Key::X => Some(Some(egui::Event::Cut)),
        egui::Key::V => {
            Some(clipboard().map(|t| t.replace("\r\n", "\n")).filter(|t| !t.is_empty()).map(egui::Event::Paste))
        }
        _ => None,
    }
}
