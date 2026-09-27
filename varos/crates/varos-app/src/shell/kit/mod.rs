//! Minimum Start/Recovery controls. No host commands, editor access, or I/O.
//!
//! Callers own stable IDs, resolved shortcut/help text and enabled reasons. Consume
//! `activated` once. A host that handles Enter/Space itself must use `pointer_only`;
//! it can supply its model's keyboard focus with `focused`. No action runs on paint.
use egui::{Color32, Event, Id, Key, PointerButton, Response, Sense, Stroke, StrokeKind, TextStyle, Ui};

use super::tokens as t;
mod icons;
pub use icons::Icon;

#[derive(Clone, Copy, Default)]
pub enum Availability<'a> {
    #[default]
    Enabled,
    Disabled(&'a str),
    Busy(&'a str),
}

/// Presentation only. `label` also names icon-only controls for assistive technology.
pub struct Control<'a> {
    pub id: Id,
    pub label: &'a str,
    pub help: &'a str,
    pub availability: Availability<'a>,
    pub icon: Option<Icon>,
    pub selected: bool,
    pub focused: bool,
    pub pointer_only: bool,
}
impl<'a> Control<'a> {
    pub fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            help: "",
            availability: Availability::Enabled,
            icon: None,
            selected: false,
            focused: false,
            pointer_only: false,
        }
    }
}

pub struct ControlResponse {
    pub response: Response,
    /// Pointer release, first Enter/Space press, or accessibility activation; at most once per frame.
    pub activated: bool,
}

/// Neutral action button. Use `icon_only` for Home/chrome with an accessible label.
pub fn action(ui: &mut Ui, control: Control<'_>, icon_only: bool) -> ControlResponse {
    paint_control(ui, control, None, None, icon_only)
}

/// A recent-file row; `detail` is already formatted by the caller (path, date, missing state).
pub fn list_row(ui: &mut Ui, control: Control<'_>, detail: &str) -> ControlResponse {
    paint_control(ui, control, Some(detail), None, false)
}

/// Start's file row, with a separate trailing date when the caller has room.
pub fn document_row(ui: &mut Ui, control: Control<'_>, detail: &str, date: &str) -> ControlResponse {
    paint_control(ui, control, Some(detail), Some(date), false)
}

fn keyboard_visible(ui: &Ui) -> bool {
    let id = Id::new("varos-kit-keyboard-modality");
    let mut keyboard = ui.ctx().data_mut(|d| d.get_temp::<bool>(id).unwrap_or(false));
    ui.input(|i| {
        for event in &i.events {
            match event {
                Event::Key { pressed: true, .. } => keyboard = true,
                Event::PointerButton { pressed: true, .. } | Event::WindowFocused(false) => keyboard = false,
                _ => {}
            }
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, keyboard));
    keyboard
}

fn paint_control(
    ui: &mut Ui,
    c: Control<'_>,
    detail: Option<&str>,
    date: Option<&str>,
    icon_only: bool,
) -> ControlResponse {
    let icon_only = icon_only && c.icon.is_some();
    let keyboard = keyboard_visible(ui);
    let reason = match c.availability {
        Availability::Enabled => None,
        Availability::Disabled(reason) | Availability::Busy(reason) => Some(reason),
    };
    let opacity = ui.painter().opacity();
    ui.add_enabled_ui(reason.is_none(), |ui| {
        // We own disabled colours; keep the ancestor's opacity (including a ghosted parent).
        ui.set_opacity(opacity);
        let enabled = ui.is_enabled();
        let font = if date.is_some() {
            egui::FontId::proportional(t::START_FILE_SIZE)
        } else {
            TextStyle::Button.resolve(ui.style())
        };
        let label = ui.painter().layout_no_wrap(c.label.into(), font.clone(), t::TEXT);
        let icon_space = if c.icon.is_some() { t::KIT_ICON + t::KIT_GAP } else { 0.0 };
        let width = if detail.is_some() {
            ui.available_width()
        } else if icon_only {
            t::KIT_CONTROL_H
        } else {
            label.size().x + icon_space + t::KIT_PAD * 2.0
        };
        let size = egui::vec2(
            width.min(ui.available_width()).max(t::KIT_MIN_TARGET),
            if date.is_some() {
                t::START_ROW_H
            } else if detail.is_some() {
                t::KIT_ROW_H
            } else {
                t::KIT_CONTROL_H
            },
        );
        let (_, rect) = ui.allocate_space(size);
        let response = ui.interact(rect, c.id, if c.pointer_only { Sense::CLICK } else { Sense::click() });
        let hover = enabled && (response.hovered() || response.is_pointer_button_down_on());
        let fill = if !enabled {
            t::PANEL
        } else if c.selected {
            t::SURFACE
        } else if hover {
            t::HOVER
        } else if detail.is_some() {
            t::PANEL
        } else {
            t::SURFACE
        };
        let text = if enabled { t::TEXT } else { t::MUTED };
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(rect, t::r_ctrl(), fill);
        if detail.is_none() && !c.selected {
            painter.rect_stroke(rect, t::r_ctrl(), Stroke::new(t::KIT_STROKE, t::LINE2), StrokeKind::Inside);
        }
        if enabled && (c.focused || (response.has_focus() && keyboard)) {
            painter.rect_stroke(
                rect.shrink(t::KIT_STROKE),
                t::r_ctrl(),
                Stroke::new(t::KIT_FOCUS_STROKE, t::ACCENT),
                StrokeKind::Inside,
            );
        }
        let mut x = rect.left() + t::KIT_PAD;
        if let Some(icon) = c.icon {
            let center = if icon_only { rect.center() } else { egui::pos2(x + t::KIT_ICON / 2.0, rect.center().y) };
            icon.paint(&painter, center, text);
            x += icon_space;
        }
        if date.is_some() {
            painter.hline(rect.x_range(), rect.bottom() - t::KIT_STROKE, t::hairline());
        }
        if !icon_only {
            let date_width = if date.is_some_and(|d| !d.is_empty()) { t::START_DATE_W } else { 0.0 };
            if let Some(date) = date.filter(|d| !d.is_empty()) {
                let color = if hover { t::TEXT } else { t::MUTED };
                painter.text(
                    egui::pos2(rect.right() - t::KIT_PAD, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    date,
                    TextStyle::Small.resolve(ui.style()),
                    color,
                );
            }
            let width = (rect.right() - t::KIT_PAD - x - date_width).max(0.0);
            let title = elided(ui, c.label, font, text, width);
            let y = if date.is_some() {
                rect.center().y - (title.size().y + t::KIT_TEXT_GAP + TextStyle::Small.resolve(ui.style()).size) / 2.0
            } else if detail.is_some() {
                rect.top() + t::KIT_PAD
            } else {
                rect.center().y - title.size().y / 2.0
            };
            let title_height = title.size().y;
            painter.galley(egui::pos2(x, y), title, text);
            if let Some(detail) = detail {
                let color = if hover { t::TEXT } else { t::MUTED };
                let detail = elided(ui, detail, TextStyle::Small.resolve(ui.style()), color, width);
                painter.galley(egui::pos2(x, y + title_height + t::KIT_TEXT_GAP), detail, color);
            }
        }
        let accessible_label = reason.map_or_else(|| c.label.to_string(), |r| format!("{} — {r}", c.label));
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, c.selected, accessible_label.as_str())
        });
        let (activation_key, first_press) = ui.input(|i| {
            let mut any = false;
            let mut first = false;
            for event in &i.events {
                if let Event::Key { key: Key::Enter | Key::Space, pressed: true, repeat, modifiers, .. } = event {
                    any = true;
                    first |= !repeat && modifiers.is_none();
                }
            }
            (any, first)
        });
        let pointer = response.clicked_by(PointerButton::Primary);
        let keyboard_or_accessibility =
            !c.pointer_only && ((response.has_focus() && first_press) || (!activation_key && response.clicked()));
        let activated = enabled && (pointer || keyboard_or_accessibility);
        let help = reason.unwrap_or(c.help);
        let response =
            if help.is_empty() { response } else { response.on_hover_text(help).on_disabled_hover_text(help) };
        ControlResponse { response, activated }
    })
    .inner
}

fn elided(ui: &Ui, text: &str, font: egui::FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.into(), font, color);
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

pub fn section_heading(ui: &mut Ui, label: &str) -> Response {
    ui.label(egui::RichText::new(label).text_style(TextStyle::Heading).color(t::TEXT))
}

/// Static status/error/empty copy on PANEL or SURFACE (the muted text contrast contract).
/// No animation, dismiss button, or implied cancellation.
/// Caller supplies explicit copy, e.g. "Opening…" or "Couldn't open this file: …".
pub fn notice(ui: &mut Ui, message: &str) -> Response {
    ui.add(egui::Label::new(egui::RichText::new(message).color(t::MUTED)).wrap())
}
