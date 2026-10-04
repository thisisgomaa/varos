//! Minimum Start/Recovery controls, plus THE icon button (`icon_button`, icon stage 1) and the icon
//! registry (`Icon`). No host commands, editor access, or I/O.
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
            icon.paint(&painter, center, t::KIT_ICON, text);
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
        let activated = enabled && activation(ui, &response, c.pointer_only);
        let help = reason.unwrap_or(c.help);
        let response =
            if help.is_empty() { response } else { response.on_hover_text(help).on_disabled_hover_text(help) };
        ControlResponse { response, activated }
    })
    .inner
}

/// One activation per press: the primary pointer click, the first (non-repeat, unmodified) Enter/Space
/// while focused, or an accessibility click. `pointer_only` hosts own Enter/Space themselves.
fn activation(ui: &Ui, response: &Response, pointer_only: bool) -> bool {
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
        !pointer_only && ((response.has_focus() && first_press) || (!activation_key && response.clicked()));
    pointer || keyboard_or_accessibility
}

/// How an icon button shows its state. Owner decision (UI_SYSTEM "on states"): a tool is an azure
/// block, a toggle is a small azure bar with no fill; a plain action has neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconState<'a> {
    /// A one-shot action (Add, Duplicate, Flip…).
    Action,
    /// A tool: an azure block while it is the active one.
    Tool(bool),
    /// An on/off toggle: a small azure bar under the glyph while on.
    Toggle(bool),
    /// Not available now; the reason is added to the tooltip, the button never activates.
    Disabled(&'a str),
}

/// The hover text an icon button shows: its label (the old text, plus its shortcut), and for a disabled
/// button the reason too. Labels move to tooltips — they never disappear.
pub fn icon_tooltip(tooltip: &str, state: IconState<'_>) -> String {
    match state {
        IconState::Disabled(reason) if !reason.is_empty() => format!("{tooltip} — {reason}"),
        _ => tooltip.to_string(),
    }
}

/// THE icon button: a registry glyph at `t::ICON_LG` in a `t::ICON_BTN_W` × `t::ICON_BTN_H` target
/// (≥ 24 pt), 3 px corners, HOVER fill on hover, the state marks of [`IconState`], the azure focus
/// ring for keyboard focus only, and a tooltip that is never empty. Activation is the same single
/// event as [`action`]: pointer release, the first Enter/Space while focused, or an accessibility click.
pub fn icon_button(ui: &mut Ui, id: Id, icon: Icon, tooltip: &str, state: IconState<'_>) -> ControlResponse {
    debug_assert!(!tooltip.is_empty(), "an icon button carries its old text label as a tooltip");
    let keyboard = keyboard_visible(ui);
    let disabled = matches!(state, IconState::Disabled(_));
    let help = icon_tooltip(tooltip, state);
    let opacity = ui.painter().opacity();
    ui.add_enabled_ui(!disabled, |ui| {
        ui.set_opacity(opacity); // we own the disabled colours; keep any ghosted ancestor's opacity
        let enabled = ui.is_enabled();
        let (_, rect) = ui.allocate_space(egui::vec2(t::ICON_BTN_W, t::ICON_BTN_H));
        let response = ui.interact(rect, id, Sense::click());
        let hover = enabled && (response.hovered() || response.is_pointer_button_down_on());
        let (block, bar) = match state {
            IconState::Tool(on) => (on, false),
            IconState::Toggle(on) => (false, on),
            IconState::Action | IconState::Disabled(_) => (false, false),
        };
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        if block && enabled {
            painter.rect_filled(rect, t::r_ctrl(), t::ACCENT);
        } else if hover {
            painter.rect_filled(rect, t::r_ctrl(), t::HOVER);
        }
        let ink = if !enabled {
            t::FAINT
        } else if block || bar || hover {
            t::TEXT
        } else {
            t::MUTED
        };
        icon.paint(&painter, rect.center(), t::ICON_LG, ink);
        if bar && enabled {
            let mark = egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.bottom() - t::ICON_BAR_H / 2.0),
                egui::vec2(t::ICON_BAR_W, t::ICON_BAR_H),
            );
            painter.rect_filled(mark, egui::CornerRadius::ZERO, t::ACCENT);
        }
        if enabled && response.has_focus() && keyboard {
            // Over an azure block the ring would vanish: it switches to TEXT there (K5 focus overlay).
            let ring = if block { t::TEXT } else { t::ACCENT };
            painter.rect_stroke(rect, t::r_ctrl(), Stroke::new(t::KIT_FOCUS_STROKE, ring), StrokeKind::Inside);
        }
        response
            .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, block || bar, help.as_str()));
        let activated = enabled && activation(ui, &response, false);
        let response = response.on_hover_text(help.as_str()).on_disabled_hover_text(help.as_str());
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

/// A kit menu entry: a hand-painted row or a hairline separator.
#[derive(Clone, Copy, Debug)]
pub enum MenuEntry<'a> {
    Item(&'a str),
    Separator,
}

/// The single open kit menu (one at a time). `focus` is an ordinal among the Items.
#[derive(Clone, Copy, Debug)]
struct MenuState {
    owner: Id,
    pos: egui::Pos2,
    anchor: Option<egui::Rect>,
    rect: Option<egui::Rect>,
    focus: Option<usize>,
    keyboard: bool,
}
fn menu_key() -> Id {
    Id::new("varos-kit-menu")
}
fn menu_state(ctx: &egui::Context) -> Option<MenuState> {
    ctx.data(|d| d.get_temp::<MenuState>(menu_key()))
}
fn store_menu(ctx: &egui::Context, state: MenuState) {
    ctx.data_mut(|d| d.insert_temp(menu_key(), state));
}

/// Open the menu owned by `owner` at `pos` (top-left). `anchor` is the control that toggles it:
/// a press on the anchor does not count as "outside" (so the anchor can close it again).
pub fn open_menu(ctx: &egui::Context, owner: Id, pos: egui::Pos2, anchor: Option<egui::Rect>) {
    store_menu(ctx, MenuState { owner, pos, anchor, rect: None, focus: None, keyboard: false });
}
/// The anchor's toggle: open under `anchor` (right-aligned to it) or close when already open.
pub fn toggle_menu_below(ctx: &egui::Context, owner: Id, anchor: egui::Rect) {
    if is_menu_open(ctx, owner) {
        close_menu(ctx);
    } else {
        let pos = egui::pos2(anchor.right() - t::KIT_MENU_MIN_W, anchor.bottom() + t::KIT_MENU_GAP);
        open_menu(ctx, owner, pos, Some(anchor));
    }
}
pub fn close_menu(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<MenuState>(menu_key()));
}
/// Any kit menu is open: hosts stop their own keyboard traversal while it is.
pub fn menu_open(ctx: &egui::Context) -> bool {
    menu_state(ctx).is_some()
}
pub fn is_menu_open(ctx: &egui::Context, owner: Id) -> bool {
    menu_state(ctx).is_some_and(|s| s.owner == owner)
}

/// Draw `owner`'s menu if it is the open one; returns the activated entry index (into `entries`).
/// Keyboard while open: ↑/↓ move between items (stop at the ends), Enter/Space activate, Esc closes.
/// The menu owns those keys (they are consumed). A press outside closes it. No shadow, no animation.
pub fn menu(ctx: &egui::Context, owner: Id, entries: &[MenuEntry<'_>]) -> Option<usize> {
    let mut state = menu_state(ctx).filter(|s| s.owner == owner)?;
    let items: Vec<usize> =
        entries.iter().enumerate().filter(|(_, e)| matches!(e, MenuEntry::Item(_))).map(|(i, _)| i).collect();
    if items.is_empty() {
        close_menu(ctx);
        return None;
    }
    let last = items.len() - 1;
    let mut chosen = None;
    let mut close = false;
    ctx.input_mut(|i| {
        for event in &i.events {
            match event {
                Event::Key { key, pressed: true, repeat, .. } => match key {
                    Key::ArrowDown => {
                        state.focus = Some(state.focus.map_or(0, |f| (f + 1).min(last)));
                        state.keyboard = true;
                    }
                    Key::ArrowUp => {
                        state.focus = Some(state.focus.map_or(last, |f| f.saturating_sub(1)));
                        state.keyboard = true;
                    }
                    Key::Enter | Key::Space if !repeat => chosen = state.focus.map(|f| items[f]),
                    Key::Escape => close = true,
                    _ => {}
                },
                Event::PointerButton { pressed: true, pos, .. } => {
                    state.keyboard = false;
                    let inside =
                        state.rect.is_none_or(|r| r.contains(*pos)) || state.anchor.is_some_and(|a| a.contains(*pos));
                    close |= !inside;
                }
                _ => {}
            }
        }
        i.events.retain(|e| {
            !matches!(
                e,
                Event::Key {
                    key: Key::ArrowDown | Key::ArrowUp | Key::Enter | Key::Space | Key::Escape | Key::Tab,
                    ..
                }
            )
        });
    });
    if close {
        close_menu(ctx);
        return None;
    }
    let font = TextStyle::Button.resolve(&ctx.global_style());
    let widest = entries
        .iter()
        .filter_map(|e| if let MenuEntry::Item(label) = e { Some(*label) } else { None })
        .map(|label| ctx.fonts_mut(|f| f.layout_no_wrap(label.into(), font.clone(), t::TEXT).size().x))
        .fold(0.0, f32::max);
    let width = (widest + t::KIT_PAD * 2.0).max(t::KIT_MENU_MIN_W);
    let area = egui::Area::new(menu_key()).order(egui::Order::Foreground).fixed_pos(state.pos).show(ctx, |ui| {
        egui::Frame::new()
            .fill(t::SURFACE)
            .stroke(Stroke::new(t::KIT_STROKE, t::LINE2))
            .corner_radius(t::r_box())
            .inner_margin(t::KIT_TEXT_GAP)
            .show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let mut ordinal = 0;
                for (index, entry) in entries.iter().enumerate() {
                    match entry {
                        MenuEntry::Separator => {
                            separator(ui);
                        }
                        MenuEntry::Item(label) => {
                            let mut c = Control::new(owner.with(("menu-item", index)), label);
                            c.pointer_only = true;
                            c.focused = state.keyboard && state.focus == Some(ordinal);
                            let r = menu_row(ui, c);
                            if r.response.hovered() && !state.keyboard {
                                state.focus = Some(ordinal);
                            }
                            if r.activated {
                                chosen = Some(index);
                            }
                            ordinal += 1;
                        }
                    }
                }
            });
    });
    state.rect = Some(area.response.rect);
    if chosen.is_some() {
        close_menu(ctx);
    } else {
        store_menu(ctx, state);
    }
    chosen
}

/// One menu row: full width, control height, 3 px corners, HOVER fill on hover, the azure ring only
/// for keyboard focus. Activation is the pointer (or a focused accessibility click); hosts that own
/// Enter pass `pointer_only` and `focused`.
pub fn menu_row(ui: &mut Ui, c: Control<'_>) -> ControlResponse {
    let reason = match c.availability {
        Availability::Enabled => None,
        Availability::Disabled(reason) | Availability::Busy(reason) => Some(reason),
    };
    let enabled = reason.is_none() && ui.is_enabled();
    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width().max(t::KIT_MIN_TARGET), t::KIT_CONTROL_H));
    let response = ui.interact(rect, c.id, if c.pointer_only { Sense::CLICK } else { Sense::click() });
    let hover = enabled && (response.hovered() || response.is_pointer_button_down_on());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if hover {
        painter.rect_filled(rect, t::r_ctrl(), t::HOVER);
    }
    if enabled && c.focused {
        painter.rect_stroke(rect, t::r_ctrl(), Stroke::new(t::KIT_FOCUS_STROKE, t::ACCENT), StrokeKind::Inside);
    }
    let text = if enabled { t::TEXT } else { t::MUTED };
    let mut x = rect.left() + t::KIT_PAD;
    if let Some(icon) = c.icon {
        icon.paint(&painter, egui::pos2(x + t::KIT_ICON / 2.0, rect.center().y), t::KIT_ICON, text);
        x += t::KIT_ICON + t::KIT_GAP;
    }
    let width = (rect.right() - t::KIT_PAD - x).max(0.0);
    let galley = elided(ui, c.label, TextStyle::Button.resolve(ui.style()), text, width);
    painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, text);
    let label = reason.map_or_else(|| c.label.to_string(), |r| format!("{} — {r}", c.label));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label.as_str()));
    let activated = enabled && (response.clicked_by(PointerButton::Primary) || (!c.pointer_only && response.clicked()));
    ControlResponse { response, activated }
}

/// A kit hairline separator (LINE, 1 px) with KIT_GAP of vertical room; never egui's `ui.separator()`.
pub fn separator(ui: &mut Ui) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), t::KIT_GAP), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, t::hairline());
    response
}
