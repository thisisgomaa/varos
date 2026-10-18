//! Warm dark Varos tokens, fonts and egui visuals. No shadows or decorative azure.
use std::sync::Arc;

use crate::theme::{Brightness, FONT_MONO, FONT_UI, FONT_UI_SEMIBOLD, Tokens};
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const BG: Color32 = rgb(0x141313);
pub const PANEL: Color32 = rgb(0x1b1919);
pub const SURFACE: Color32 = rgb(0x242121);
pub const HOVER: Color32 = rgb(0x2b2828);
pub const LINE: Color32 = rgb(0x2c2929);
pub const TEXT: Color32 = rgb(0xe9e6e3);
pub const MUTED: Color32 = rgb(0x8f8a86);
pub const DISABLED: Color32 = rgb(0x6e6a66);
pub const ROW_HOVER: Color32 = rgb(0x262323);
pub const INPUT_WELL: Color32 = rgb(0x171515);
pub const ACCENT: Color32 = rgb(0x0c8ce9);
pub const ACCENT_HOVER: Color32 = rgb(0x2b9df4);
pub const SEAM: Color32 = Color32::BLACK;
pub const ERROR: Color32 = rgb(0xe05c5c);

fn active_fill() -> Color32 {
    // Opaque equivalent of Varos's subtle azure wash over SURFACE.
    Color32::from_rgb(33, 47, 60)
}

/// Map custom-painted chrome as well as egui widgets to the same palette.
pub fn tokens() -> Tokens {
    Tokens {
        dark: true,
        app_bar: BG,
        panel: PANEL,
        panel_darker: BG,
        input: INPUT_WELL,
        input_border: LINE,
        border: LINE,
        divider: LINE,
        text: TEXT,
        text_dim: MUTED,
        text_disabled: DISABLED,
        icon: MUTED,
        hover: HOVER,
        tool_active: active_fill(),
        accent: ACCENT,
        accent_strong: ACCENT_HOVER,
        row_selected: active_fill(),
        pasteboard: BG,
        ruler: PANEL,
        ruler_tick: MUTED,
        measure_bg: PANEL,
        button: SURFACE,
        radius: 3,
        text_strong: TEXT,
        tab_strip: BG,
        // Keep the upstream slider rail visible against the warm panel.
        button_border: rgb(0x3b3636),
        selection: ACCENT,
        anchor_selected: ACCENT,
        flyout_active: ACCENT,
        caption_close: ERROR,
        caption_close_text: TEXT,
        error: ERROR,
        // Guides, bleed and artwork warnings keep their content semantics.
        ..Tokens::for_brightness(Brightness::Dark)
    }
}

/// Floating boxes that override egui's window radius use the Varos box radius too.
pub fn box_radius(t: &Tokens, upstream: u8) -> CornerRadius {
    CornerRadius::same(if t.panel == PANEL && t.accent == ACCENT { 8 } else { upstream })
}

/// Three upstream floating widgets specify their own shadows instead of using Visuals.
pub fn chrome_shadow(t: &Tokens, offset: [i8; 2], blur: u8, alpha: u8) -> egui::epaint::Shadow {
    if t.panel == PANEL && t.accent == ACCENT {
        egui::epaint::Shadow::NONE
    } else {
        egui::epaint::Shadow { offset, blur, spread: 0, color: Color32::from_black_alpha(alpha) }
    }
}

/// Varos is a proper name in every interface language.
pub fn extend_catalog(plain: &mut std::collections::HashMap<String, String>) {
    plain.insert("Varos".into(), "Varos".into());
}

/// Build Varos fonts independently, leaving upstream font-only consumers unchanged.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = crate::theme::font_definitions();
    register_fonts(&mut fonts);
    ctx.set_fonts(fonts);
}

/// Insert after upstream font registration, retaining craft-fonts and system fallbacks.
pub fn register_fonts(fonts: &mut FontDefinitions) {
    for (name, bytes) in [
        ("Varos Inter", include_bytes!("../../assets/varos/fonts/Inter-Regular.ttf").as_slice()),
        ("Varos Inter Medium", include_bytes!("../../assets/varos/fonts/Inter-Medium.ttf").as_slice()),
        ("Varos Inter Semibold", include_bytes!("../../assets/varos/fonts/Inter-SemiBold.ttf").as_slice()),
        ("Varos Arabic", include_bytes!("../../assets/varos/fonts/IBMPlexSansArabic-Regular.ttf").as_slice()),
        ("Varos Mono", include_bytes!("../../assets/varos/fonts/JetBrainsMono-Regular.ttf").as_slice()),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    for (family, face) in [
        (FontFamily::Proportional, "Varos Inter"),
        (FontFamily::Name(FONT_UI.into()), "Varos Inter"),
        (FontFamily::Name("ui-medium".into()), "Varos Inter Medium"),
        (FontFamily::Name(FONT_UI_SEMIBOLD.into()), "Varos Inter Semibold"),
        (FontFamily::Monospace, "Varos Mono"),
        (FontFamily::Name(FONT_MONO.into()), "Varos Mono"),
    ] {
        let stack = fonts.families.entry(family).or_default();
        stack.splice(0..0, [face.to_owned(), "Varos Arabic".to_owned()]);
    }
}

/// Apply the selectable dark-only Varos appearance after upstream setup.
/// Fonts are registered once per context, after upstream setup.
pub fn apply(ctx: &egui::Context) {
    let id = egui::Id::new("varos-fonts-installed");
    let installed = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if !installed {
        install_fonts(ctx);
        ctx.data_mut(|d| d.insert_temp(id, true));
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, tokens()));
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = INPUT_WELL;
    v.faint_bg_color = ROW_HOVER;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.override_text_color = Some(TEXT);
    v.weak_text_color = Some(MUTED);
    v.hyperlink_color = TEXT;
    v.error_fg_color = ERROR;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, TEXT);
    for (widget, fill) in [
        (&mut v.widgets.noninteractive, PANEL),
        (&mut v.widgets.inactive, SURFACE),
        (&mut v.widgets.hovered, HOVER),
        (&mut v.widgets.active, active_fill()),
        (&mut v.widgets.open, active_fill()),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, LINE);
        widget.fg_stroke = Stroke::new(1.0, TEXT);
        widget.corner_radius = CornerRadius::same(3);
        widget.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    ctx.set_visuals(v);
    // Keep upstream density and named typography roles; change only the visual system.
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size = egui::vec2(24.0, 24.0);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.spacing.slider_width = 120.0;
        s.text_styles = [
            (egui::TextStyle::Small, egui::FontId::proportional(11.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(13.0)),
            (egui::TextStyle::Button, egui::FontId::new(13.0, FontFamily::Name("ui-medium".into()))),
            (egui::TextStyle::Heading, crate::theme::semibold(15.0)),
            (egui::TextStyle::Monospace, crate::theme::mono(11.5)),
        ]
        .into();
        s.animation_time = 0.08;
        s.visuals.disabled_alpha = 0.48;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varos_appearance_survives_scheme_and_accent_selection() {
        use crate::theme::{Accent, Scheme};
        for scheme in Scheme::ALL {
            for accent in Accent::ALL {
                let t = Tokens::for_scheme(scheme, Brightness::Varos, accent);
                assert_eq!(t.panel, PANEL);
                assert_eq!(t.accent, ACCENT);
                let ctx = egui::Context::default();
                crate::theme::apply_with(&ctx, Brightness::Varos, accent, scheme);
                assert_eq!(Tokens::get(&ctx).accent, ACCENT);
            }
        }
    }

    #[test]
    fn varos_palette_and_fonts_survive_a_headless_frame() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        apply(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            let style = ui.style();
            assert_eq!(Tokens::get(ctx).pasteboard, BG);
            assert_eq!(style.visuals.window_shadow, egui::epaint::Shadow::NONE);
            assert_eq!(style.visuals.popup_shadow, egui::epaint::Shadow::NONE);
            assert_eq!(style.visuals.widgets.inactive.bg_fill, SURFACE);
            assert_eq!(style.visuals.widgets.hovered.bg_fill, HOVER);
            assert_eq!(style.visuals.selection.bg_fill, ACCENT);
            assert_eq!(style.visuals.window_corner_radius, CornerRadius::same(8));
            ctx.fonts_mut(|fonts| {
                let definitions = fonts.definitions();
                assert_eq!(definitions.families[&FontFamily::Proportional][0], "Varos Inter");
                assert_eq!(definitions.families[&FontFamily::Monospace][0], "Varos Mono");
                assert!(fonts.has_glyph(&egui::FontId::proportional(13.0), 'ع'));
            });
        });
        output.textures_delta.clear();
        crate::theme::apply(&ctx, Brightness::Light);
        assert!(!Tokens::get(&ctx).dark);
        crate::theme::apply(&ctx, Brightness::Varos);
        assert_eq!(Tokens::get(&ctx).panel, PANEL);
    }
}
