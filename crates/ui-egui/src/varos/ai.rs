//! The AI panel is navigation to working tools plus inert roadmap surfaces.
use super::{soon, theme_components::FOUNDATION as F};
use crate::{VectorcraftApp, theme::Tokens, widgets};
use egui::{Id, Ui};
use serde_json::json;

/// Called only before the host restores saved preferences. New native workspaces show AI
/// as the active tab of the existing right dock; restored states replace this default whole.
pub(crate) fn initialize_fresh(ui: &mut crate::state::UiState) {
    let shell = &mut ui.varos_shell;
    if let Some(id) = shell.tree.find_tab("properties")
        && shell.tree.add_tab(id, "ai")
    {
        shell.strip.remove("ai");
    }
}

pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let id = Id::unique("varos-ai-tab");
    let mut tab = ui.ctx().data(|d| d.get_temp::<usize>(id)).unwrap_or_default();
    if let Some(next) = soon::panel_tabs(ui, &["Ask", "Images", "History"], tab) {
        tab = next;
        ui.ctx().data_mut(|d| d.insert_temp(id, tab));
    }
    ui.add_space(F.soon_gap);
    match tab {
        0 => {
            let getting_started = note(
                ui,
                "Works today: your Codex or Claude Code in Terminal can already open and edit this board through Bridge. Its work shows in orange.",
                true,
            );
            if getting_started {
                crate::menus::invoke(app, "help.gettingStarted", json!({}));
            }
            section(ui, "Chat", &["Chat with Codex / Claude Code", "Chat with your own API key"]);
            let t = Tokens::get(ui.ctx());
            let chat_width = ui.available_width();
            egui::Frame::NONE
                .fill(t.panel_darker)
                .stroke(egui::Stroke::new(F.soon_stroke, t.border))
                .corner_radius(F.control_radius)
                .inner_margin(F.soon_chat_padding)
                .show(ui, |ui| {
                    ui.set_min_width((chat_width - f32::from(F.soon_chat_padding) * 2.0 - F.soon_stroke * 2.0).max(0.0));
                    ui.set_min_height(F.soon_chat_height - f32::from(F.soon_chat_padding) * 2.0);
                    ui.add_space(F.soon_chat_height - F.soon_row_height - f32::from(F.soon_chat_padding) * 2.0);
                    ui.horizontal(|ui| {
                        let input_width = ui.available_width() - F.control_height - ui.spacing().item_spacing.x - F.soon_stroke * 2.0;
                        soon::chat_input(ui, input_width);
                        widgets::icon_button_enabled(
                            ui,
                            "dc-arrow-up",
                            &soon::hover("Chat with Codex / Claude Code"),
                            false,
                            false,
                            F.control_height,
                        );
                    });
                });
            section(ui, "Memory", &["Project memory"]);
            section(ui, "Recipes", &["Recipes & quick scripts"]);
            section(ui, "Agents", &["Agents on your server", "Build an extension with AI"]);
        }
        1 => {
            section(ui, "Generate", &["Generate images", "Several versions", "Styles", "Find references"]);
            section(
                ui,
                "Edit",
                &["Edit with a prompt", "Expand image", "Remove object", "Remove background", "Upscale", "Adjustments", "Auto merge"],
            );
            if crop_row(app, ui) {
                crate::menus::invoke(app, "ui.cropImage", json!({}));
            }
        }
        _ => {
            note(
                ui,
                "Works today: every step an agent makes already shows with its name in Window › History, and clicking a row goes back to it.",
                false,
            );
            section(ui, "Runs", &["History per AI", "Undo this run"]);
            ui.add_space(F.tab_padding);
            ui.label(egui::RichText::new("When History per AI ships, each run becomes a row and Undo this run moves to the end of every run. Until there are runs, it sits here as a plain row.").size(F.control_font).color(Tokens::get(ui.ctx()).text_dim));
        }
    }
}
fn note(ui: &mut Ui, text: &str, help: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut clicked = false;
    egui::Frame::NONE
        .fill(t.panel_darker)
        .stroke(egui::Stroke::new(F.soon_stroke, t.border))
        .corner_radius(F.control_radius)
        .inner_margin(F.soon_note_padding)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(F.soon_icon), egui::Sense::hover());
                if help {
                    crate::icons::paint(ui, "message-circle", rect, t.text_dim);
                } else {
                    ui.painter().circle_filled(rect.center(), F.soon_status_dot * 0.5, super::theme::AGENT);
                }
                ui.vertical(|ui| {
                    let mut job = egui::text::LayoutJob::default();
                    job.append(
                        "Works today:",
                        0.0,
                        egui::TextFormat { font_id: crate::theme::semibold(F.control_font), color: t.text, ..Default::default() },
                    );
                    job.append(
                        text.strip_prefix("Works today:").unwrap_or(text),
                        0.0,
                        egui::TextFormat { font_id: egui::FontId::proportional(F.control_font), color: t.text_dim, ..Default::default() },
                    );
                    ui.label(job);
                    if help {
                        clicked = ui.add(egui::Button::new("Getting started").frame(false)).clicked();
                    }
                });
            });
        });
    clicked
}
fn section(ui: &mut Ui, label: &str, items: &[&str]) {
    ui.add_space(F.soon_gap);
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), F.soon_section_height), egui::Sense::hover());
    crate::icons::paint(
        ui,
        "chevron-down",
        egui::Rect::from_center_size(egui::pos2(rect.left() + F.soon_icon * 0.5, rect.center().y), egui::Vec2::splat(F.soon_icon)),
        t.text_dim,
    );
    ui.painter().text(
        egui::pos2(rect.left() + F.soon_icon + F.soon_gap, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        crate::theme::semibold(F.panel_body_font),
        t.text,
    );
    let detail = match label {
        "Chat" => "provider",
        "Edit" => "the selected image",
        _ => "",
    };
    ui.painter().text(rect.right_center(), egui::Align2::RIGHT_CENTER, detail, egui::FontId::proportional(F.control_font), t.text_disabled);
    ui.painter().hline(rect.x_range(), rect.bottom(), egui::Stroke::new(F.soon_stroke, t.border));
    for label in items {
        soon::row(ui, label, true, true);
    }
}

fn crop_row(app: &VectorcraftApp, ui: &mut Ui) -> bool {
    let enabled = crate::menus::enabled(app, "ui.cropImage");
    let t = Tokens::get(ui.ctx());
    let r =
        ui.add_enabled_ui(enabled, |ui| ui.allocate_exact_size(egui::vec2(ui.available_width(), F.soon_row_height), egui::Sense::click()).1).inner;
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, "Crop Image"));
    let ink = if enabled { t.text } else { t.text_disabled };
    if r.hovered() {
        ui.painter().rect_filled(r.rect, F.control_radius, t.hover);
    }
    crate::icons::paint(
        ui,
        "frame",
        egui::Rect::from_center_size(egui::pos2(r.rect.left() + F.soon_gap + F.soon_icon * 0.5, r.rect.center().y), egui::Vec2::splat(F.soon_icon)),
        ink,
    );
    ui.painter().text(
        egui::pos2(r.rect.left() + F.soon_icon + F.soon_gap * 2.0, r.rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Crop Image",
        egui::FontId::proportional(F.panel_body_font),
        ink,
    );
    ui.painter().text(
        r.rect.right_center() - egui::vec2(F.soon_gap, 0.0),
        egui::Align2::RIGHT_CENTER,
        "works today",
        egui::FontId::proportional(F.control_font),
        t.text_disabled,
    );
    r.clicked()
}

/// A navigation mirror, independent of saved panel owners. Opening it uses the existing
/// window.panel command; merely drawing it cannot migrate or rearrange a workspace.
pub(crate) fn strip(app: &mut VectorcraftApp, ui: &mut Ui, region: egui::Rect) -> egui::Rect {
    let rail = egui::Rect::from_min_max(egui::pos2(region.right() - F.soon_rail_width, region.top()), region.max);
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(rail, F.tab_radius, t.panel);
    let rect = egui::Rect::from_center_size(egui::pos2(rail.center().x, rail.top() + F.soon_rail_width * 0.5), egui::Vec2::splat(F.soon_row_height));
    navigation_button(app, ui, rect);
    egui::Rect::from_min_max(region.min, egui::pos2(rail.left(), region.bottom()))
}

pub(crate) fn legacy_strip_mirror(app: &mut VectorcraftApp, ui: &mut Ui, width: f32) {
    let (row, _) = ui.allocate_exact_size(egui::vec2(width, F.soon_rail_width), egui::Sense::hover());
    navigation_button(app, ui, egui::Rect::from_center_size(row.center(), egui::Vec2::splat(F.soon_row_height)));
}

fn navigation_button(app: &mut VectorcraftApp, ui: &mut Ui, rect: egui::Rect) {
    let t = Tokens::get(ui.ctx());
    let shown = if super::panel_system::enabled(app) { super::panel_system::showing(app, "ai") } else { super::shell::showing(app, "ai") };
    let r = ui.interact(rect, Id::unique("varos-ai-strip"), egui::Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, shown, "AI"));
    if shown || r.hovered() {
        ui.painter().rect_filled(rect, F.control_radius, t.button);
    }
    crate::icons::paint(
        ui,
        "sparkles",
        egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(F.soon_icon)),
        if shown { t.text } else { t.text_dim },
    );
    if super::shell::strip::clicked_once(ui.ctx(), r.id, r.on_hover_text("AI").clicked()) {
        crate::menus::invoke(app, "window.panel", json!({"panel":"ai","toggle":true}));
    }
}
/// New panels must not appear when reconstructing an older saved layout.
pub(crate) fn legacy_panel(id: &str, explicitly_saved: bool) -> bool {
    id != "ai" || explicitly_saved
}
