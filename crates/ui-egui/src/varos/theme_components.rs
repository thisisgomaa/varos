//! Shared presentation policy. Callers retain identity, input, drag and close ownership.
use crate::theme::{self, Tokens};
use egui::{Align2, FontId, Painter, Rect, Stroke, pos2};

/// Compile-time baseline for the first migration. Runtime theme loading is a later stage.
/// Measurements and painting must consume the same definition.
pub(crate) struct Foundation {
    pub tab_font: f32,
    pub tab_height: f32,
    pub document_tab_height: f32,
    pub tab_radius: u8,
    pub tab_padding: f32,
    pub panel_body_font: f32,
    pub panel_button_font: f32,
    pub panel_heading_font: f32,
    pub control_radius: u8,
    pub box_radius: u8,
}
pub(crate) const FOUNDATION: Foundation = Foundation {
    tab_font: 12.0,
    tab_height: 24.0,
    document_tab_height: 28.0,
    tab_radius: 0,
    tab_padding: 10.0,
    panel_body_font: 12.0,
    panel_button_font: 11.0,
    panel_heading_font: 14.0,
    control_radius: 3,
    box_radius: 8,
};
impl Foundation {
    pub fn tab_font(&self) -> FontId {
        theme::semibold(self.tab_font)
    }
    pub fn panel_text(&self, ui: &mut egui::Ui) {
        let style = ui.style_mut();
        style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(self.panel_body_font));
        style.text_styles.insert(egui::TextStyle::Button, FontId::new(self.panel_button_font, egui::FontFamily::Name("ui-medium".into())));
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
    pub fn tab_label(&self, p: &Painter, rect: Rect, label: &str, t: &Tokens, active: bool, hovered: bool) {
        p.text(
            pos2(rect.left() + self.tab_padding, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            self.tab_font(),
            if active || hovered { t.text } else { t.text_dim },
        );
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
