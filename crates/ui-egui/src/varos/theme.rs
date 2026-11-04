//! The Varos palettes (Figma UI3 neutrals + Varos azure, owner 2026-10-28), Inter fonts and
//! shadowless egui visuals.
use crate::theme::{Brightness, FONT_MONO, FONT_UI, FONT_UI_SEMIBOLD, Tokens};
use egui::class::HasClasses as _;
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};
use std::sync::Arc;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The one accent: selection, focus, active and the drop preview only, at every brightness.
pub const ACCENT: Color32 = rgb(0x0c8ce9);
/// Text and glyphs on an [`ACCENT`] fill.
pub const ACCENT_ON: Color32 = Color32::WHITE;
/// Owner 2026-10-08: azure is the person's selection and focus; AGENT (orange, #F29A3E since the
/// 2026-10-28 colour round) is anything an agent is doing: the artboard it works on, the objects it
/// adds, its rows in the History panel.
pub const AGENT: Color32 = rgb(0xf29a3e);
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
pub const AGENT_LABEL_TEXT: f32 = crate::varos::theme_components::FOUNDATION.control_font;

/// One brightness' colour roles, named as in the colour-round brief
/// (`docs/varos/lanes/brief-colours.md`). [`tokens_for`] maps them onto [`Tokens`]; roles without a
/// `Tokens` field (menus, secondary icons, child rows, accent hover) wait here for their consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Panels, the bg of every docked group and floating card.
    pub panel: Color32,
    /// The canvas around the artboards and the darkest well.
    pub canvas: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub icon: Color32,
    pub icon_secondary: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub field: Color32,
    pub field_hover_border: Color32,
    pub tab_bar: Color32,
    pub tab_active: Color32,
    /// Menus stay dark in both modes (Figma).
    pub menu: Color32,
    pub menu_text: Color32,
    pub menu_dim: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub guide: Color32,
    pub accent_hover: Color32,
    pub selected_row: Color32,
    pub selected_row_child: Color32,
}

/// Figma UI3 dark, exactly as the owner chose it (2026-10-28).
pub const DARK: Palette = Palette {
    panel: rgb(0x2c2c2c),
    canvas: rgb(0x1e1e1e),
    border: rgb(0x444444),
    border_strong: rgb(0x555555),
    text: rgb(0xffffff),
    text_secondary: rgb(0xb3b3b3),
    text_tertiary: rgb(0x888888),
    icon: rgb(0xf5f5f5),
    icon_secondary: rgb(0xb3b3b3),
    hover: rgb(0x383838),
    pressed: rgb(0x444444),
    field: rgb(0x383838),
    field_hover_border: rgb(0x4d4d4d),
    tab_bar: rgb(0x1e1e1e),
    tab_active: rgb(0x2c2c2c),
    menu: rgb(0x1e1e1e),
    menu_text: rgb(0xffffff),
    menu_dim: rgb(0x9a9a9a),
    danger: rgb(0xf24822),
    warning: rgb(0xffcd29),
    guide: rgb(0xf24822),
    accent_hover: rgb(0x2e9eee),
    selected_row: rgb(0x1e3a52),
    selected_row_child: rgb(0x243445),
};

/// Figma UI3 light. The brief names no light danger / warning / guide: they keep the dark values
/// (signal colours, not neutrals).
pub const LIGHT: Palette = Palette {
    panel: rgb(0xffffff),
    canvas: rgb(0xf5f5f5),
    border: rgb(0xe6e6e6),
    border_strong: rgb(0xd9d9d9),
    text: rgb(0x1e1e1e),
    text_secondary: rgb(0x6e6e6e),
    text_tertiary: rgb(0x8a8a8a),
    icon: rgb(0x1e1e1e),
    icon_secondary: rgb(0x757575),
    hover: rgb(0xf5f5f5),
    pressed: rgb(0xe6e6e6),
    field: rgb(0xf5f5f5),
    field_hover_border: rgb(0xe6e6e6),
    tab_bar: rgb(0xe8e8e8),
    tab_active: rgb(0xffffff),
    menu: rgb(0x1e1e1e),
    menu_text: rgb(0xffffff),
    menu_dim: rgb(0x9a9a9a),
    danger: rgb(0xf24822),
    warning: rgb(0xffcd29),
    guide: rgb(0xf24822),
    accent_hover: rgb(0x0a7bce),
    selected_row: rgb(0xe3f1fc),
    selected_row_child: rgb(0xf0f7fd),
};

/// How far Medium Dark and Medium Light move their surfaces from their own endpoint towards the
/// other one. 0.08 is the largest step that keeps the azure focus ring at 3:1 on their panels.
pub const MEDIUM_MIX: f32 = 0.08;

impl Palette {
    /// These roles with every surface, line and selected row moved `k` of the way to `other`'s;
    /// text, icons, menus and signal colours stay this endpoint's (they are what reads on them).
    pub fn toward(self, other: Palette, k: f32) -> Palette {
        let mix = |a: Color32, b: Color32| {
            let ch = |p: u8, q: u8| (f32::from(p) + (f32::from(q) - f32::from(p)) * k).round() as u8;
            Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
        };
        Palette {
            panel: mix(self.panel, other.panel),
            canvas: mix(self.canvas, other.canvas),
            border: mix(self.border, other.border),
            border_strong: mix(self.border_strong, other.border_strong),
            hover: mix(self.hover, other.hover),
            pressed: mix(self.pressed, other.pressed),
            field: mix(self.field, other.field),
            field_hover_border: mix(self.field_hover_border, other.field_hover_border),
            tab_bar: mix(self.tab_bar, other.tab_bar),
            tab_active: mix(self.tab_active, other.tab_active),
            selected_row: mix(self.selected_row, other.selected_row),
            selected_row_child: mix(self.selected_row_child, other.selected_row_child),
            ..self
        }
    }

    /// Derived (no brief role): halfway between panel and hover, egui's faint striped-row fill.
    pub fn row_stripe(&self) -> Color32 {
        let mid = |p: u8, q: u8| ((u16::from(p) + u16::from(q)) / 2) as u8;
        Color32::from_rgb(mid(self.panel.r(), self.hover.r()), mid(self.panel.g(), self.hover.g()), mid(self.panel.b(), self.hover.b()))
    }
}

/// The roles at brightness `b`: Dark and Light are the Figma endpoints, the medium levels sit
/// [`MEDIUM_MIX`] inside them.
pub fn palette(b: Brightness) -> Palette {
    match b {
        Brightness::Dark => DARK,
        Brightness::MediumDark => DARK.toward(LIGHT, MEDIUM_MIX),
        Brightness::MediumLight => LIGHT.toward(DARK, MEDIUM_MIX),
        Brightness::Light => LIGHT,
    }
}

/// Fresh sessions use the owner's base; persisted preferences are restored by the host.
pub(crate) fn default_brightness(session: &mut vectorcraft_engine::Session) {
    if session.prefs.ui_brightness == "mediumDark" {
        session.prefs.ui_brightness = "dark".into();
    }
}

/// Keep geometry/semantic colours upstream; replace every neutral at one hook. Modern's Blue
/// default becomes azure, while explicit accent and Classic choices survive (their selected rows
/// are the panel tinted towards their accent, as upstream does).
///
/// Field → role (see `docs/varos/SHELL_SIMPLE_DIRECTION.md`, "Colour round"):
/// - `app_bar`, `tab_strip`, `divider` → tab_bar (the top band, the seams between docked groups and
///   the 1 px dividers inside panels are one tone, as the old warm-black seam was);
/// - `panel`, `ruler`, `measure_bg` → panel; `panel_darker`, `pasteboard` → canvas;
/// - `input` → field; `input_border` → field_hover_border; `border` → border;
///   `button_border` → border_strong;
/// - `text`, `text_strong` → text; `text_dim` → text_secondary; `text_disabled` → text_tertiary;
/// - `icon` → icon; `ruler_tick` → icon_secondary;
/// - `hover` → hover; `tool_active` and `button` → pressed (`button` paints the front tab pill,
///   lit toggles and filled cards beside `hover`, which equals field in Figma's ramp, so it takes
///   the next step to stay distinct);
/// - `accent`, `accent_strong`, `selection`, `anchor_selected`, `flyout_active` → accent (#0C8CE9:
///   Figma fills primary buttons and checks with the accent itself; accent_hover has no field);
/// - `row_selected` → selected_row; `error` → danger; `warning` → warning;
///   `guide`, `smart_guide` → guide.
pub fn tokens_for(b: Brightness, base: Tokens, default_accent: bool) -> Tokens {
    let p = palette(b);
    let accent = if default_accent { ACCENT } else { base.accent };
    let dark = matches!(b, Brightness::Dark | Brightness::MediumDark);
    let row_selected = if default_accent {
        p.selected_row
    } else {
        // Other accents: the panel tinted towards them, at upstream's strengths.
        let k = if dark { 0.24 } else { 0.2 };
        let ch = |from: u8, to: u8| (f32::from(from) + (f32::from(to) - f32::from(from)) * k).round() as u8;
        Color32::from_rgb(ch(p.panel.r(), accent.r()), ch(p.panel.g(), accent.g()), ch(p.panel.b(), accent.b()))
    };
    Tokens {
        dark,
        app_bar: p.tab_bar,
        panel: p.panel,
        panel_darker: p.canvas,
        input: p.field,
        input_border: p.field_hover_border,
        border: p.border,
        divider: p.tab_bar,
        text: p.text,
        text_dim: p.text_secondary,
        text_disabled: p.text_tertiary,
        icon: p.icon,
        hover: p.hover,
        tool_active: p.pressed,
        row_selected,
        accent,
        accent_strong: if default_accent { ACCENT } else { base.accent_strong },
        pasteboard: p.canvas,
        ruler: p.panel,
        ruler_tick: p.icon_secondary,
        smart_guide: p.guide,
        guide: p.guide,
        measure_bg: p.panel,
        button: p.pressed,
        radius: crate::varos::theme_components::FOUNDATION.control_radius,
        text_strong: p.text,
        tab_strip: p.tab_bar,
        button_border: p.border_strong,
        selection: accent,
        anchor_selected: accent,
        flyout_active: accent,
        error: p.danger,
        warning: p.warning,
        ..base
    }
}

/// All four appearances share box geometry, independent of accent selection.
pub fn box_radius(_t: &Tokens, _upstream: u8) -> CornerRadius {
    CornerRadius::same(crate::varos::theme_components::FOUNDATION.box_radius)
}
pub fn chrome_shadow(_t: &Tokens, _offset: [i8; 2], _blur: u8, _alpha: u8) -> egui::epaint::Shadow {
    egui::epaint::Shadow::NONE
}

/// Build Varos fonts independently, leaving upstream font-only consumers unchanged.
fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(crate::theme::interface_fonts());
}

/// One embedded face shared by egui and the shaped Arabic renderer.
pub(crate) const ARABIC_FONT: &[u8] = include_bytes!("../../assets/varos/fonts/IBMPlexSansArabic-Regular.ttf");

pub(crate) fn arabic_font_db() -> &'static vectorcraft_text::FontDb {
    static INSTALLED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let db = vectorcraft_text::FontDb::global();
    INSTALLED.get_or_init(|| {
        db.add_font(ARABIC_FONT.to_vec());
    });
    db
}

/// Insert after upstream font registration, retaining craft-fonts and system fallbacks.
pub fn register_fonts(fonts: &mut FontDefinitions) {
    for (name, bytes) in [
        ("Varos Inter", include_bytes!("../../assets/varos/fonts/Inter-Regular.ttf").as_slice()),
        ("Varos Inter Medium", include_bytes!("../../assets/varos/fonts/Inter-Medium.ttf").as_slice()),
        ("Varos Inter Semibold", include_bytes!("../../assets/varos/fonts/Inter-SemiBold.ttf").as_slice()),
        ("Varos Arabic", ARABIC_FONT),
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

/// Fork hook: own one complete style application, retaining upstream's initializer as fallback.
/// A future upstream-only consumer may decline this hook without coupling to Varos style internals.
pub(crate) fn apply_owned(ctx: &egui::Context, brightness: Brightness, tokens: Tokens) -> bool {
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, tokens));
    apply(ctx, brightness);
    true
}

/// Apply fonts and flat widget visuals to every built-in brightness.
pub fn apply(ctx: &egui::Context, b: Brightness) {
    let id = egui::Id::unique("varos-fonts-installed");
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
    v.faint_bg_color = palette(b).row_stripe();
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = CornerRadius::same(crate::varos::theme_components::FOUNDATION.box_radius);
    v.menu_corner_radius = CornerRadius::same(crate::varos::theme_components::FOUNDATION.box_radius);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.override_text_color = Some(t.text);
    v.weak_text_color = Some(t.text_dim);
    v.hyperlink_color = t.text;
    v.error_fg_color = t.error;
    v.selection.bg_fill = t.accent;
    v.selection.stroke = Stroke::new(1.0, t.text);
    // egui's own widgets follow Figma's controls: field tone at rest, one step up (pressed) on
    // hover and press (Figma's hover tone equals its field tone, so it cannot mark a field's hover).
    for (widget, fill) in [
        (&mut v.widgets.noninteractive, t.panel),
        (&mut v.widgets.inactive, t.input),
        (&mut v.widgets.hovered, t.tool_active),
        (&mut v.widgets.active, t.tool_active),
        (&mut v.widgets.open, t.tool_active),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, t.border);
        widget.fg_stroke = Stroke::new(1.0, t.text);
        widget.corner_radius = CornerRadius::same(crate::varos::theme_components::FOUNDATION.control_radius);
        widget.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text_dim);
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.accent);
    ctx.set_visuals(v);
    // Keep upstream density and named typography roles; change only the visual system.
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        crate::varos::theme_components::FOUNDATION.control_spacing(&mut s.spacing);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.spacing.slider_width = 120.0;
        s.text_styles = [
            (egui::TextStyle::Small, egui::FontId::proportional(crate::varos::theme_components::FOUNDATION.control_font)),
            (egui::TextStyle::Body, egui::FontId::proportional(crate::varos::theme_components::FOUNDATION.body_font)),
            (
                egui::TextStyle::Button,
                egui::FontId::new(crate::varos::theme_components::FOUNDATION.button_font, FontFamily::Name("ui-medium".into())),
            ),
            (egui::TextStyle::Heading, crate::theme::semibold(crate::varos::theme_components::FOUNDATION.heading_font)),
            (egui::TextStyle::Monospace, crate::theme::mono(crate::varos::theme_components::FOUNDATION.metadata_font)),
        ]
        .into();
        s.animation_time = 0.08;
        s.visuals.disabled_alpha = 0.48;
    });
    ctx.replace_widget_theme::<egui::widget_style::TextEditStyle>(TextFields);
    ctx.replace_widget_theme::<egui::widget_style::CheckboxStyle>(Checkboxes);
    ctx.replace_widget_theme::<egui::widget_style::ButtonStyle>(Buttons);
}

/// Text fields as egui 0.36 drew them. egui 0.37's default text field style gives every text edit
/// at least the interact height (24 points) with its text at the top: a field framed by Varos
/// (4-point margins, or a 26-point box) came out taller than the controls beside it, its text
/// riding high. Without that floor a single-line field is as tall as its text again, centred by
/// its box (owner report on v11, 2026-10-26).
struct TextFields;

impl egui::theme::StyleProvider<egui::widget_style::TextEditStyle> for TextFields {
    fn style(&mut self, args: &egui::widget_style::StyleArgs<'_>) -> egui::widget_style::TextEditStyle {
        let mut style =
            <egui::theme::DefaultStyle as egui::theme::StyleProvider<egui::widget_style::TextEditStyle>>::style(&mut egui::theme::DefaultStyle, args);
        style.atom_layout.min_size.y = 0.0;
        if !args.classes.has_class(&egui::TextEdit::CLASS_READ_ONLY) {
            let t = Tokens::get(args.ctx);
            style.atom_layout.frame.stroke.color = if args.state == egui::widget_style::WidgetState::Active { t.accent } else { t.input_border };
        }
        style
    }
}

/// Native and custom checkboxes share the compact checkbox role, independently of buttons.
struct Checkboxes;
impl egui::theme::StyleProvider<egui::widget_style::CheckboxStyle> for Checkboxes {
    fn style(&mut self, args: &egui::widget_style::StyleArgs<'_>) -> egui::widget_style::CheckboxStyle {
        let mut style =
            <egui::theme::DefaultStyle as egui::theme::StyleProvider<egui::widget_style::CheckboxStyle>>::style(&mut egui::theme::DefaultStyle, args);
        style.checkbox_frame.corner_radius = CornerRadius::same(crate::varos::theme_components::FOUNDATION.checkbox_radius);
        style
    }
}

/// Ordinary filled actions share the button color roles; text fields keep their own surface.
/// Native selection, frameless/combo variants and explicit widget overrides remain native.
struct Buttons;
impl egui::theme::StyleProvider<egui::widget_style::ButtonStyle> for Buttons {
    fn style(&mut self, args: &egui::widget_style::StyleArgs<'_>) -> egui::widget_style::ButtonStyle {
        let mut style =
            <egui::theme::DefaultStyle as egui::theme::StyleProvider<egui::widget_style::ButtonStyle>>::style(&mut egui::theme::DefaultStyle, args);
        let special = args.classes.has_class(&egui::Button::CLASS_SELECTED)
            || args.classes.has_class(&egui::Button::CLASS_NO_FRAME)
            || args.classes.has_class(&egui::ComboBox::CLASS)
            || args.classes.has_class(&egui::ComboBox::CLASS_OPEN)
            || args.classes.has_class(&egui::DragValue::CLASS);
        if !special && style.atom_layout.frame.fill != Color32::TRANSPARENT {
            let t = Tokens::get(args.ctx);
            use egui::widget_style::WidgetState;
            style.atom_layout.frame.fill = match args.state {
                WidgetState::Inactive => t.button,
                WidgetState::Hovered => t.hover,
                WidgetState::Active => t.tool_active,
                WidgetState::Noninteractive => style.atom_layout.frame.fill,
            };
        }
        style
    }
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

    /// The brief's two endpoints, value by value: the owner wants this exact palette.
    #[test]
    fn figma_endpoints_are_the_brief_values() {
        let hex = |c: Color32| format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
        let roles = |p: Palette| {
            [
                p.panel,
                p.canvas,
                p.border,
                p.border_strong,
                p.text,
                p.text_secondary,
                p.text_tertiary,
                p.icon,
                p.icon_secondary,
                p.hover,
                p.pressed,
                p.field,
                p.field_hover_border,
                p.tab_bar,
                p.tab_active,
                p.menu,
                p.menu_text,
                p.menu_dim,
                p.danger,
                p.warning,
                p.guide,
                p.accent_hover,
                p.selected_row,
                p.selected_row_child,
            ]
            .map(hex)
        };
        assert_eq!(
            roles(DARK),
            [
                "#2C2C2C", "#1E1E1E", "#444444", "#555555", "#FFFFFF", "#B3B3B3", "#888888", "#F5F5F5", "#B3B3B3", "#383838", "#444444", "#383838",
                "#4D4D4D", "#1E1E1E", "#2C2C2C", "#1E1E1E", "#FFFFFF", "#9A9A9A", "#F24822", "#FFCD29", "#F24822", "#2E9EEE", "#1E3A52", "#243445",
            ]
        );
        assert_eq!(
            roles(LIGHT),
            [
                "#FFFFFF", "#F5F5F5", "#E6E6E6", "#D9D9D9", "#1E1E1E", "#6E6E6E", "#8A8A8A", "#1E1E1E", "#757575", "#F5F5F5", "#E6E6E6", "#F5F5F5",
                "#E6E6E6", "#E8E8E8", "#FFFFFF", "#1E1E1E", "#FFFFFF", "#9A9A9A", "#F24822", "#FFCD29", "#F24822", "#0A7BCE", "#E3F1FC", "#F0F7FD",
            ]
        );
        assert_eq!(hex(ACCENT), "#0C8CE9");
        assert_eq!(hex(ACCENT_ON), "#FFFFFF");
        assert_eq!(hex(AGENT), "#F29A3E");
    }

    /// Every `Tokens` field the palette owns, at both endpoints (the mapping in [`tokens_for`]).
    #[test]
    fn tokens_map_the_roles() {
        for (b, p) in [(Brightness::Dark, DARK), (Brightness::Light, LIGHT)] {
            let t = Tokens::for_scheme(Scheme::Modern, b, Accent::default());
            assert_eq!(t.dark, b == Brightness::Dark);
            for (field, want) in [
                (t.app_bar, p.tab_bar),
                (t.tab_strip, p.tab_bar),
                (t.divider, p.tab_bar),
                (t.panel, p.panel),
                (t.ruler, p.panel),
                (t.measure_bg, p.panel),
                (t.panel_darker, p.canvas),
                (t.pasteboard, p.canvas),
                (t.input, p.field),
                (t.input_border, p.field_hover_border),
                (t.border, p.border),
                (t.button_border, p.border_strong),
                (t.text, p.text),
                (t.text_strong, p.text),
                (t.text_dim, p.text_secondary),
                (t.text_disabled, p.text_tertiary),
                (t.icon, p.icon),
                (t.ruler_tick, p.icon_secondary),
                (t.hover, p.hover),
                (t.tool_active, p.pressed),
                (t.button, p.pressed),
                (t.row_selected, p.selected_row),
                (t.error, p.danger),
                (t.warning, p.warning),
                (t.guide, p.guide),
                (t.smart_guide, p.guide),
                (t.accent, ACCENT),
                (t.accent_strong, ACCENT),
                (t.selection, ACCENT),
                (t.anchor_selected, ACCENT),
                (t.flyout_active, ACCENT),
            ] {
                assert_eq!(field, want, "{b:?}");
            }
        }
    }

    /// The medium levels are each endpoint moved [`MEDIUM_MIX`] towards the other, surface by
    /// surface, and keep their endpoint's text; every surface sits between the two endpoints.
    #[test]
    fn medium_levels_interpolate_between_the_endpoints() {
        let between = |m: Color32, a: Color32, b: Color32| {
            [(m.r(), a.r(), b.r()), (m.g(), a.g(), b.g()), (m.b(), a.b(), b.b())].iter().all(|&(m, a, b)| a.min(b) <= m && m <= a.max(b))
        };
        for (b, from, to) in [(Brightness::MediumDark, DARK, LIGHT), (Brightness::MediumLight, LIGHT, DARK)] {
            let p = palette(b);
            assert_eq!(p, from.toward(to, MEDIUM_MIX));
            assert_eq!((p.text, p.text_secondary, p.icon, p.menu), (from.text, from.text_secondary, from.icon, from.menu));
            for (m, a, z) in [
                (p.panel, from.panel, to.panel),
                (p.canvas, from.canvas, to.canvas),
                (p.hover, from.hover, to.hover),
                (p.field, from.field, to.field),
            ] {
                assert!(between(m, a, z) && m != a, "{b:?}: {m:?} between {a:?} and {z:?}");
            }
        }
        assert_eq!(palette(Brightness::MediumDark).panel, rgb(0x3d3d3d));
        assert_eq!(palette(Brightness::MediumLight).panel, rgb(0xeeeeee));
    }

    /// Body text ≥ 4.5:1 and secondary text ≥ 3:1 on every surface they sit on, in every
    /// brightness, scheme and accent; the azure focus ring ≥ 3:1 on the panel at every brightness
    /// and on canvas, field and hover at the two Figma endpoints. The one documented exception
    /// (owner 2026-10-28, kept on purpose): azure on the pressed tone — #444444 dark 2.76:1,
    /// #E6E6E6 light 2.83:1, the light tab bar #E8E8E8 (pressed-toned) 2.88:1; the medium levels,
    /// mid greys, drop it to 2.25:1 (Medium Dark) and 2.50:1 (Medium Light).
    #[test]
    fn palettes_keep_contrast_with_one_documented_exception() {
        for b in Brightness::ALL {
            for scheme in Scheme::ALL {
                for accent in Accent::ALL {
                    let t = Tokens::for_scheme(scheme, b, accent);
                    let surfaces = [
                        t.app_bar,
                        t.tab_strip,
                        t.panel,
                        t.panel_darker,
                        t.pasteboard,
                        t.input,
                        t.button,
                        t.hover,
                        t.tool_active,
                        t.row_selected,
                        t.ruler,
                        t.measure_bg,
                    ];
                    for bg in surfaces {
                        assert!(contrast(t.text, bg) >= 4.5, "{b:?} {scheme:?} {accent:?}: body text on {bg:?}");
                        assert!(contrast(t.text_dim, bg) >= 3.0, "{b:?} {scheme:?} {accent:?}: secondary text on {bg:?}");
                    }
                }
            }
            let p = palette(b);
            assert!(contrast(ACCENT, p.panel) >= 3.0, "{b:?}: focus on the panel");
            assert!(contrast(p.menu_text, p.menu) >= 4.5 && contrast(p.menu_dim, p.menu) >= 3.0, "{b:?}: menus");
            assert!(contrast(p.text, p.selected_row_child) >= 4.5, "{b:?}: child rows");
            // The exception: recorded and bounded, not fixed.
            let pressed = contrast(ACCENT, p.pressed);
            assert!((2.2..3.0).contains(&pressed), "{b:?}: azure on pressed {pressed:.2}");
        }
        assert!((2.75..2.77).contains(&contrast(ACCENT, DARK.pressed)) && (2.82..2.84).contains(&contrast(ACCENT, LIGHT.pressed)));
        for p in [DARK, LIGHT] {
            for bg in [p.canvas, p.field, p.hover] {
                assert!(contrast(ACCENT, bg) >= 3.0, "focus on {bg:?}");
            }
        }
        assert!((2.8..3.0).contains(&contrast(ACCENT, LIGHT.tab_bar)), "the light tab bar is pressed-toned");
    }

    #[test]
    fn four_palettes_apply_fonts_and_shadowless_geometry() {
        for b in Brightness::ALL {
            for scheme in Scheme::ALL {
                for accent in Accent::ALL {
                    let ctx = egui::Context::default();
                    crate::theme::apply_with(&ctx, b, accent, scheme);
                    let t = Tokens::get(&ctx);
                    let p = palette(b);
                    assert_eq!(t.panel, p.panel);
                    assert_eq!(t.pasteboard, p.canvas);
                    assert_eq!(t.radius, 3);
                    assert_eq!(box_radius(&t, 4), CornerRadius::same(crate::varos::theme_components::FOUNDATION.box_radius));
                    assert_eq!(chrome_shadow(&t, [0, 3], 10, 90), egui::epaint::Shadow::NONE);
                    if scheme == Scheme::Modern {
                        let want = if accent == Accent::default() { ACCENT } else { t.with_accent(accent).accent };
                        assert_eq!(t.accent, want);
                    }
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                        assert_eq!(ui.visuals().window_shadow, egui::epaint::Shadow::NONE);
                        assert_eq!(ui.visuals().popup_shadow, egui::epaint::Shadow::NONE);
                        assert_eq!(ui.visuals().window_corner_radius, CornerRadius::same(crate::varos::theme_components::FOUNDATION.box_radius));
                        assert_eq!(ui.visuals().widgets.inactive.corner_radius, CornerRadius::same(3));
                        assert_eq!(ui.visuals().widgets.inactive.bg_fill, p.field);
                        assert_eq!(ui.visuals().widgets.hovered.bg_fill, p.pressed);
                        assert_eq!(ui.visuals().faint_bg_color, p.row_stripe());
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
