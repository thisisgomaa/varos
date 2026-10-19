//! Four cool Varos brightness palettes, Inter fonts and shadowless egui visuals.
use crate::theme::{Brightness, FONT_MONO, FONT_UI, FONT_UI_SEMIBOLD, Tokens};
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};
use std::sync::Arc;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const ACCENT: Color32 = rgb(0x0c8ce9);
/// Owner 2026-10-08: azure is the person's selection and focus; AGENT (burnt orange) is anything an
/// agent is doing: the artboard it works on, the objects it adds, its rows in the History panel.
pub const AGENT: Color32 = rgb(0xc76a20);
/// Width of the outline around the artboard an agent works on.
pub const AGENT_PAGE_STROKE: f32 = 1.5;
/// Width of the outline of an object an agent created or changed (it fades out).
pub const AGENT_OBJECT_STROKE: f32 = 1.0;
/// An artboard narrower than this on screen gets its outline but no name chips.
pub const AGENT_LABEL_MIN_PAGE_W: f32 = 160.0;
/// Height of an agent's name chip above its artboard, the gap around it and its inner padding.
pub const AGENT_LABEL_H: f32 = 18.0;
pub const AGENT_LABEL_GAP: f32 = 4.0;
pub const AGENT_LABEL_PAD: f32 = 4.0;
/// Size of the agent name text (chips and History rows).
pub const AGENT_LABEL_TEXT: f32 = 11.0;

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub hover: Color32,
    pub line: Color32,
    pub row_hover: Color32,
    pub input: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub seam: Color32,
    pub rail: Color32,
}

pub(crate) fn palette(b: Brightness) -> Palette {
    let colors = match b {
        Brightness::Dark => [0x121418, 0x1a1d23, 0x22262e, 0x2a2f38, 0x30363f, 0x252a32, 0x0f1115, 0xeceef2, 0xa2a8b3, 0x6f7580, 0x0b0c0f, 0x424a56],
        Brightness::MediumDark => {
            [0x1c2027, 0x242931, 0x2c323b, 0x353c46, 0x3b424d, 0x303640, 0x171b21, 0xeef0f3, 0xaab0ba, 0x7b828e, 0x12151a, 0x4c5563]
        }
        Brightness::MediumLight => {
            [0xd9dde3, 0xe6e9ee, 0xf1f3f6, 0xe0e4ea, 0xc3c9d2, 0xdce1e8, 0xffffff, 0x16191e, 0x5a616c, 0x8b929d, 0xb5bdc8, 0xa1aab7]
        }
        Brightness::Light => [0xeef0f4, 0xf7f8fa, 0xffffff, 0xe8ebf0, 0xd2d7df, 0xe6eaf0, 0xffffff, 0x111418, 0x5f6672, 0x969da8, 0xc4cbd5, 0xaab3c0],
    };
    let [bg, panel, surface, hover, line, row_hover, input, text, muted, disabled, seam, rail] = colors.map(rgb);
    Palette { bg, panel, surface, hover, line, row_hover, input, text, muted, disabled, seam, rail }
}

/// Fresh sessions use the owner's base; persisted preferences are restored by the host.
pub(crate) fn default_brightness(session: &mut vectorcraft_engine::Session) {
    if session.prefs.ui_brightness == "mediumDark" {
        session.prefs.ui_brightness = "dark".into();
    }
}

/// Keep geometry/semantic colours upstream; replace the full neutral ramp at one hook.
/// Modern's Blue default becomes azure, while explicit accent and Classic choices survive.
pub fn tokens_for(b: Brightness, base: Tokens, default_accent: bool) -> Tokens {
    let p = palette(b);
    let accent = if default_accent { ACCENT } else { base.accent };
    let mut t = Tokens {
        dark: matches!(b, Brightness::Dark | Brightness::MediumDark),
        app_bar: p.bg,
        panel: p.panel,
        panel_darker: p.bg,
        input: p.input,
        input_border: p.line,
        border: p.line,
        divider: p.seam,
        text: p.text,
        text_dim: p.muted,
        text_disabled: p.disabled,
        icon: p.muted,
        hover: p.hover,
        tool_active: p.hover,
        row_selected: p.row_hover,
        accent,
        accent_strong: base.accent_strong,
        pasteboard: p.bg,
        ruler: p.panel,
        ruler_tick: p.muted,
        measure_bg: p.panel,
        button: p.surface,
        radius: 3,
        text_strong: p.text,
        tab_strip: p.bg,
        button_border: p.rail,
        selection: accent,
        anchor_selected: accent,
        flyout_active: accent,
        ..base
    };
    // Tint only selected/active controls; neutral surfaces never carry decorative azure.
    let tint = |bg: Color32| {
        Color32::from_rgb(
            ((u16::from(bg.r()) * 7 + u16::from(accent.r())) / 8) as u8,
            ((u16::from(bg.g()) * 7 + u16::from(accent.g())) / 8) as u8,
            ((u16::from(bg.b()) * 7 + u16::from(accent.b())) / 8) as u8,
        )
    };
    t.tool_active = tint(p.surface);
    t.row_selected = tint(p.row_hover);
    t
}

/// All four appearances share box geometry, independent of accent selection.
pub fn box_radius(_t: &Tokens, _upstream: u8) -> CornerRadius {
    CornerRadius::same(8)
}
pub fn chrome_shadow(_t: &Tokens, _offset: [i8; 2], _blur: u8, _alpha: u8) -> egui::epaint::Shadow {
    egui::epaint::Shadow::NONE
}

pub fn extend_catalog(plain: &mut std::collections::HashMap<String, String>) {
    plain.insert("Varos".into(), "Varos".into());
}

/// Build Varos fonts independently, leaving upstream font-only consumers unchanged.
fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(crate::theme::font_definitions());
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
        // Match upstream's Inter optical size so dense controls still fit at 800 pt.
        let mut data = FontData::from_static(bytes);
        if name.starts_with("Varos Inter") {
            data = data.tweak(egui::epaint::text::FontTweak { scale: 0.92, ..Default::default() });
        }
        fonts.font_data.insert(name.into(), Arc::new(data));
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

/// Apply fonts and flat widget visuals to every built-in brightness.
pub fn apply(ctx: &egui::Context, b: Brightness) {
    let id = egui::Id::new("varos-fonts-installed");
    let installed = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if !installed {
        install_fonts(ctx);
        ctx.data_mut(|d| d.insert_temp(id, true));
    }
    let t = Tokens::get(ctx);
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, t));
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.panel;
    v.window_fill = t.panel;
    v.extreme_bg_color = t.input;
    v.faint_bg_color = palette(b).row_hover;
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.override_text_color = Some(t.text);
    v.weak_text_color = Some(t.text_dim);
    v.hyperlink_color = t.text;
    v.error_fg_color = t.error;
    v.selection.bg_fill = t.accent;
    v.selection.stroke = Stroke::new(1.0, t.text);
    for (widget, fill) in [
        (&mut v.widgets.noninteractive, t.panel),
        (&mut v.widgets.inactive, t.button),
        (&mut v.widgets.hovered, t.hover),
        (&mut v.widgets.active, t.tool_active),
        (&mut v.widgets.open, t.tool_active),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, t.border);
        widget.fg_stroke = Stroke::new(1.0, t.text);
        widget.corner_radius = CornerRadius::same(3);
        widget.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text_dim);
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.accent);
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
    use crate::theme::{Accent, Scheme};
    use serde_json::json;

    fn contrast(a: Color32, b: Color32) -> f64 {
        let luminance = |c: Color32| {
            let linear = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn four_palettes_keep_contrast_fonts_and_shadowless_geometry() {
        for b in Brightness::ALL {
            for scheme in Scheme::ALL {
                for accent in Accent::ALL {
                    let ctx = egui::Context::default();
                    crate::theme::apply_with(&ctx, b, accent, scheme);
                    let t = Tokens::get(&ctx);
                    let p = palette(b);
                    assert_eq!(t.panel, p.panel);
                    assert_eq!(t.pasteboard, p.bg);
                    for bg in [t.app_bar, t.panel, t.button, t.hover, t.input, t.row_selected] {
                        assert!(contrast(t.text, bg) >= 4.5, "{b:?}: body text on {bg:?}");
                    }
                    for bg in [t.panel, t.pasteboard, t.input] {
                        assert!(contrast(t.text_dim, bg) >= 4.5, "{b:?}: muted text on {bg:?}");
                    }
                    assert_eq!(t.radius, 3);
                    assert_eq!(box_radius(&t, 4), CornerRadius::same(8));
                    assert_eq!(chrome_shadow(&t, [0, 3], 10, 90), egui::epaint::Shadow::NONE);
                    if scheme == Scheme::Modern {
                        let want = if accent == Accent::default() { ACCENT } else { t.with_accent(accent).accent };
                        assert_eq!(t.accent, want);
                    }
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                        assert_eq!(ui.visuals().window_shadow, egui::epaint::Shadow::NONE);
                        assert_eq!(ui.visuals().popup_shadow, egui::epaint::Shadow::NONE);
                        assert_eq!(ui.visuals().window_corner_radius, CornerRadius::same(8));
                        assert_eq!(ui.visuals().widgets.inactive.corner_radius, CornerRadius::same(3));
                        assert_eq!(ui.visuals().faint_bg_color, p.row_hover);
                        ui.ctx().fonts_mut(|fonts| {
                            let definitions = fonts.definitions();
                            assert_eq!(definitions.families[&FontFamily::Proportional][0], "Varos Inter");
                            assert_eq!(definitions.families[&FontFamily::Monospace][0], "Varos Mono");
                            assert!(fonts.has_glyph(&egui::FontId::proportional(13.0), 'ع'));
                        });
                    });
                    out.textures_delta.clear();
                }
            }
        }
    }

    #[test]
    fn retired_brightness_preferences_migrate_silently() {
        for id in ["varos", "varosWarm"] {
            let mut app = crate::VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
            app.ui.engine_prefs = json!({"uiBrightness": id});
            crate::prefs_dialog::restore(&mut app);
            assert_eq!(app.session.prefs.ui_brightness, "dark");
            assert_eq!(Brightness::parse(id), None);
            assert_eq!(app.ui.status, "");
        }
        for id in ["Varos", "VarosWarm", "varos", "varosWarm"] {
            assert_eq!(serde_json::from_value::<Brightness>(json!(id)).unwrap(), Brightness::Dark);
        }
    }

    #[test]
    fn brightness_dropdown_shows_four_labels_and_roundtrips() {
        use vectorcraft_engine::cmd::prefscmds::{PREF_SPECS, PrefKind};
        let sp = PREF_SPECS.iter().find(|sp| sp.key == "uiBrightness").unwrap();
        let PrefKind::Choice(opts) = sp.kind else { panic!("brightness must be a choice") };
        assert_eq!(opts, &[("dark", "Dark"), ("mediumDark", "Medium Dark"), ("mediumLight", "Medium Light"), ("light", "Light")]);
        for b in Brightness::ALL {
            let mut app = crate::VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
            app.run("window.brightness", json!({"brightness": b.id()})).unwrap();
            crate::prefs_dialog::open(&mut app, Some("User Interface"));
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx, b);
            let mut shown = vec![];
            fn texts(s: &egui::Shape, out: &mut Vec<String>) {
                match s {
                    egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                    _ => {}
                }
            }
            for _ in 0..2 {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 1000.0))),
                        ..Default::default()
                    },
                    |ui| crate::prefs_dialog::show(&mut app, ui.ctx()),
                );
                shown.clear();
                out.shapes.iter().for_each(|s| texts(&s.shape, &mut shown));
                out.textures_delta.clear();
            }
            assert!(shown.iter().any(|s| s == b.label()), "{b:?}: {shown:?}");
            assert!(!shown.iter().any(|s| s == b.id()), "raw id shown: {shown:?}");
            crate::prefs_dialog::confirm(&mut app).unwrap();
            crate::prefs_dialog::snapshot(&mut app);
            let saved = serde_json::to_value(&app.ui).unwrap();
            app.ui = serde_json::from_value(saved).unwrap();
            crate::prefs_dialog::restore(&mut app);
            assert_eq!(app.session.prefs.ui_brightness, b.id());
        }
    }
}
