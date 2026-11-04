//! Shared presentation policy. Callers retain identity, input, drag and close ownership.
use crate::theme::{self, Tokens};
use egui::{Align2, FontId, Painter, Rect, Stroke, pos2};

/// Compile-time baseline for the first migration. Runtime theme loading is a later stage.
/// Measurements and painting must consume the same definition.
pub(crate) struct Foundation {
    pub soon_tag_size: egui::Vec2,
    pub soon_gap: f32,
    pub soon_icon: f32,
    pub soon_row_height: f32,
    pub soon_stroke: f32,
    pub soon_chat_height: f32,
    pub soon_rail_width: f32,
    pub soon_note_padding: egui::Vec2,
    pub soon_chat_padding: i8,
    pub soon_section_height: f32,
    pub soon_status_dot: f32,
    pub soon_submenu_min_width: f32,
    pub tab_font: f32,
    pub tab_height: f32,
    pub document_tab_height: f32,
    pub tab_radius: u8,
    pub tab_padding: f32,
    pub compact_tab_padding: f32,
    pub panel_body_font: f32,
    pub panel_heading_font: f32,
    pub control_radius: u8,
    pub control_height: f32,
    pub dialog_button_height: f32,
    pub compact_field_height: f32,
    pub control_font: f32,
    pub legacy_control_font: f32,
    pub button_font: f32,
    pub code_field_font: f32,
    pub button_padding: egui::Vec2,
    pub field_margin: i8,
    pub box_radius: u8,
    pub ruler_font: f32,
    pub welcome_title_font: f32,
    pub about_title_font: f32,
    pub chip_radius: u8,
    pub preview_radius: u8,
    pub checkbox_radius: u8,
    pub card_radius: u8,
    pub inset_radius: u8,
    pub home_title_font: f32,
    pub badge_font: f32,
    pub avatar_font: f32,
    pub body_font: f32,
    pub heading_font: f32,
    pub dialog_title_font: f32,
    pub metadata_font: f32,
    pub status_font: f32,
    pub brand_font: f32,
    pub dialog_tab_height: f32,
    pub tab_underline: f32,
    pub floating_header_height: f32,
    pub floating_title_height: f32,
    pub panel_title_height: f32,
}
pub(crate) const FOUNDATION: Foundation = Foundation {
    soon_tag_size: egui::vec2(30.0, 16.0),
    soon_gap: 6.0,
    soon_icon: 16.0,
    soon_row_height: 28.0,
    soon_stroke: 1.0,
    soon_chat_height: 118.0,
    soon_rail_width: 36.0,
    soon_note_padding: egui::vec2(9.0, 8.0),
    soon_chat_padding: 8,
    soon_section_height: 34.0,
    soon_status_dot: 7.0,
    soon_submenu_min_width: 200.0,
    tab_font: 12.0,
    tab_height: 24.0,
    document_tab_height: 28.0,
    tab_radius: 0,
    tab_padding: 10.0,
    compact_tab_padding: 6.0,
    panel_body_font: 12.0,
    panel_heading_font: 14.0,
    control_radius: 3,
    control_height: 24.0,
    dialog_button_height: 28.0,
    compact_field_height: 20.0,
    control_font: 11.0,
    legacy_control_font: 12.5,
    button_font: 13.0,
    code_field_font: 12.0,
    button_padding: egui::vec2(6.0, 2.0),
    field_margin: 4,
    box_radius: 8,
    ruler_font: 9.5,
    welcome_title_font: 26.0,
    about_title_font: 22.0,
    chip_radius: 11,
    preview_radius: 2,
    checkbox_radius: 2,
    card_radius: 10,
    inset_radius: 4,
    home_title_font: 20.0,
    badge_font: 9.0,
    avatar_font: 10.0,
    body_font: 13.0,
    heading_font: 15.0,
    dialog_title_font: 16.0,
    metadata_font: 11.5,
    status_font: 10.5,
    brand_font: 13.5,
    dialog_tab_height: 30.0,
    tab_underline: 2.0,
    floating_header_height: 32.0,
    floating_title_height: 18.0,
    panel_title_height: 20.0,
};
impl Foundation {
    /// Standard egui controls and custom painters share these defaults. Compact/editor roles
    /// keep their explicit dimensions; widget IDs and event handling belong to callers.
    pub fn control_spacing(&self, spacing: &mut egui::style::Spacing) {
        spacing.button_padding = self.button_padding;
        spacing.interact_size = egui::Vec2::splat(self.control_height);
    }

    pub fn tab_font(&self) -> FontId {
        theme::semibold(self.tab_font)
    }
    pub fn panel_text(&self, ui: &mut egui::Ui) {
        let style = ui.style_mut();
        style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(self.panel_body_font));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::new(self.control_font, egui::FontFamily::Name(crate::theme::FONT_UI_MEDIUM.into())));
    }
    /// Visuals only: no interaction allocation or keyboard/focus side effects.
    pub fn tab_surface(&self, p: &Painter, rect: Rect, t: &Tokens, active: bool, hovered: bool) {
        p.rect_filled(
            rect,
            egui::CornerRadius::same(self.tab_radius),
            if active {
                t.panel
            } else if hovered {
                t.hover
            } else {
                t.panel_darker
            },
        );
        p.line_segment([pos2(rect.right() - 0.5, rect.top()), pos2(rect.right() - 0.5, rect.bottom())], Stroke::new(1.0, t.border));
        if !active {
            p.line_segment([pos2(rect.left(), rect.bottom() - 0.5), pos2(rect.right(), rect.bottom() - 0.5)], Stroke::new(1.0, t.border));
        }
    }
    pub fn tab_ink(&self, t: &Tokens, active: bool, hovered: bool) -> egui::Color32 {
        if active || hovered { t.text } else { t.text_dim }
    }
    pub fn tab_label(&self, p: &Painter, rect: Rect, label: &str, t: &Tokens, active: bool, hovered: bool) {
        p.text(pos2(rect.left() + self.tab_padding, rect.center().y), Align2::LEFT_CENTER, label, self.tab_font(), self.tab_ink(t, active, hovered));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arabic_and_latin_tab_measurements_follow_the_shared_font() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            for label in ["Layers", "الطبقات", "Artwork •"] {
                let measure = |font| ctx.fonts_mut(|f| f.layout_no_wrap(label.into(), font, egui::Color32::WHITE).size());
                assert_eq!(FOUNDATION.tab_font().family, crate::theme::semibold(12.0).family);
                let before = measure(FOUNDATION.tab_font());
                let larger = Foundation { tab_font: 20.0, ..FOUNDATION };
                let after = measure(larger.tab_font());
                assert!(after.x > before.x && after.y > before.y);
            }
        });
        out.textures_delta.clear();
    }
}
