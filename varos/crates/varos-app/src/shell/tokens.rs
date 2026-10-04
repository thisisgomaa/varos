//! Runtime design tokens — the warm palette and current UI text roles.
//! Historical mockups are visual references, not a second machine-synchronized token source.
//! This is the ONLY place raw colour / radius / spacing numbers live in the shell.
//! (BOX_SYSTEM_PLAN §3 + ruling 1 "tokens from the mockup" + ruling 4 "azure is a scalpel".)
use egui::{Color32, CornerRadius, Stroke};

/// Presentation only; shortcut dispatch continues to accept the same physical modifiers.
pub const fn primary_mod_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl"
    }
}

/// Compact shortcut hint. Keep the existing Windows spelling byte-for-byte.
pub fn shortcut_label(key: &str) -> String {
    let separator = if cfg!(target_os = "macos") { "" } else { "+" };
    format!("{}{separator}{key}", primary_mod_label())
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// ── rule 7: the warm-black ramp (R ≥ G ≥ B). Chrome uses ONLY these. ──
pub const BG: Color32 = rgb(0x141313); // board / base — the signature warm black
pub const PANEL: Color32 = rgb(0x1b1919); // box / panel fill
pub const SURFACE: Color32 = rgb(0x242121); // inset field / control fill
pub const HOVER: Color32 = rgb(0x2b2828); // hover-state fill
pub const LINE: Color32 = rgb(0x2c2929); // 1px hairline
pub const LINE2: Color32 = rgb(0x3b3735); // stronger hairline
pub const TEXT: Color32 = rgb(0xe9e6e3); // primary text
pub const MUTED: Color32 = rgb(0x8f8a86); // secondary text / icons
pub const FAINT: Color32 = rgb(0x6e6a66); // tertiary / micro-labels
pub const ROW_HOVER: Color32 = rgb(0x262323); // list-row hover — calmer than HOVER (between SURFACE and HOVER)
pub const INPUT_WELL: Color32 = rgb(0x171515); // darker inset well behind a focused text edit
pub const ACCENT: Color32 = rgb(0x0c8ce9); // azure scalpel — active / selection / focus ONLY
pub const ACCENT_HOVER: Color32 = rgb(0x2b9df4); // hovered accent button — one step lighter azure
                                                 // `from_rgba_unmultiplied` is not const — these are its EXACT outputs for the azure at α 60 / 34
                                                 // (proven bit-equal by `premultiplied_exact` below).
pub const ACCENT_SEL: Color32 = Color32::from_rgba_premultiplied(3, 33, 55, 60); // text-selection fill
pub const ACCENT_TINT: Color32 = Color32::from_rgba_premultiplied(2, 19, 31, 34); // faint azure wash (selected row)
pub const GUIDE: Color32 = rgb(0xff54a8); // reserved for smart guides (not yet wired — keep, Ahmed 07-08)
pub const SEAM: Color32 = rgb(0x0e0d0d); // the VOID — seams, app bar, status (darker than BG)

// ── secondary palette (content / samples, NOT chrome) ──
pub const NAVY: Color32 = rgb(0x12263a);
pub const AMBER: Color32 = rgb(0xf0b429);
pub const RULER_BG: Color32 = rgb(0x181616);
pub const CLOSE_RED: Color32 = rgb(0xc42b1c);
pub const NONE_RED: Color32 = rgb(0xe05c5c);
pub const DOT_GRID: Color32 = Color32::from_rgba_premultiplied(11, 11, 11, 11); // rgba(255,255,255,.045)
pub const VOID_HOVER: Color32 = Color32::from_rgba_premultiplied(10, 10, 10, 10); // rgba(255,255,255,.04)

// ── radii & rhythm ──
pub const R: u8 = 3; // controls: fields, chips, buttons, tabs
pub const RBOX: u8 = 8; // boxes / panels (rounder / fancier — Ahmed 07-04)
pub const RCAP: u8 = 11; // pill / capsule radius — tab pills + scroll chevrons, one "Claude bubble" look (Ahmed 07-08)
pub const SEAM_GAP: f32 = 12.0; // equal void between all boxes (wider +20% so boxes breathe — Ahmed 07-04)

pub fn r_ctrl() -> CornerRadius {
    CornerRadius::same(R)
}
pub fn r_box() -> CornerRadius {
    CornerRadius::same(RBOX)
}
pub fn hairline() -> Stroke {
    Stroke::new(1.0, LINE)
}

/// Existing dense desktop text ramp, centralized without changing sizes in U0-A.
/// Names use Proportional; numeric fields use Monospace. All bundled faces are Regular (400).
pub fn text_styles() -> std::collections::BTreeMap<egui::TextStyle, egui::FontId> {
    use egui::{FontId, TextStyle};
    [
        (TextStyle::Heading, FontId::proportional(13.5)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(12.5)),
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Monospace, FontId::monospace(12.5)),
    ]
    .into()
}

/// Apply the constitution's base look to a context: warm-dark visuals + INSTANT (no animation).
/// Idempotent — safe to call every frame.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.animation_time = 0.0; // a WORK tool: menus/popups appear INSTANTLY, never a fade (memory: no-animations)
    style.interaction.resize_grab_radius_side = 4.0; // tight seam-grab zone — precise, "on the mouse" (Ahmed 07-04)
                                                     // thin OVERLAY scrollbars — the default solid bar was a fat grey slab (Ahmed 07-04 "ضخم جدا").
                                                     // floating = invisible until you hover the body, then a slim handle; never steals layout width.
    let mut scroll = egui::style::ScrollStyle::floating();
    scroll.bar_width = 8.0;
    scroll.floating_width = 6.0;
    scroll.handle_min_length = 24.0;
    style.spacing.scroll = scroll;
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = r_box();
    v.popup_shadow = egui::epaint::Shadow::NONE; // rule 2: not one shadow in the whole app
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.override_text_color = Some(TEXT);
    v.extreme_bg_color = SURFACE;
    v.faint_bg_color = SURFACE;
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.inactive.bg_fill = SURFACE;
    v.widgets.inactive.weak_bg_fill = SURFACE;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.active.bg_fill = HOVER;
    v.widgets.active.weak_bg_fill = HOVER;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.selection.bg_fill = ACCENT_SEL;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r_ctrl();
        w.bg_stroke = Stroke::new(1.0, LINE);
    }
    style.visuals = v;
    // apply to BOTH theme slots → the same dark look regardless of the OS/active theme.
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

pub const START_PAD: f32 = 32.0;
pub const START_GAP: f32 = 24.0;
pub const START_WIDTH: f32 = 1040.0;
pub const START_SIDEBAR: f32 = 208.0;
pub const START_WIDE: f32 = 760.0;
pub const START_TITLE_SIZE: f32 = 26.0;
pub const START_SECTION_SIZE: f32 = 20.0;
pub const START_FILE_SIZE: f32 = 14.0;
pub const START_ROW_H: f32 = 72.0;
pub const START_DATE_W: f32 = 112.0;
pub const START_DATE_BREAK: f32 = 480.0;
pub const START_EMPTY_PAD: f32 = 48.0;

// Minimum Start/Recovery kit, in logical points. No runtime colour/size literals in controls.
pub const KIT_MIN_TARGET: f32 = 24.0;
pub const KIT_CONTROL_H: f32 = 32.0;
pub const KIT_ROW_H: f32 = 56.0;
pub const KIT_ICON: f32 = ICON_MD;
pub const KIT_PAD: f32 = 8.0;
pub const KIT_GAP: f32 = 8.0;
pub const KIT_TEXT_GAP: f32 = 4.0;
pub const KIT_STROKE: f32 = 1.0;
pub const KIT_FOCUS_STROKE: f32 = 2.0;
/// The focus ring sits OUTSIDE an icon button, separated from it by this gap painted in the panel
/// colour — so the ring stays visible on an azure block too (UI_SYSTEM K5: focus is an overlay).
pub const KIT_FOCUS_GAP: f32 = 1.0;
/// Lucide icons rasterize at 32 px, close to their 13–18 pt draw size: egui-wgpu textures carry no
/// mipmaps, so a large raster shown small eats thin strokes (Ahmed 2026-07-11). Toolbar and kit share it.
pub const ICON_RASTER: u32 = 32;

// ── Icon display sizes (icon stage 1, 2026-10-04). The study found six ad-hoc sizes (11/13/14/15/16/17,
// ICON_LIBRARY_STUDY §2.3); they collapse to these three. 13 and 16 are the study's §6.5 inline/default
// sizes; 18 is the owner-accepted QW6 icon-button size (UI_SYSTEM "micro 10.5 / icon 18"). No widget
// types a raw icon size again — a source-scan test holds the files that draw icons to these tokens. ──
/// Inline glyphs: search fields, field labels, the Layers eye/lock column, the status-bar Fit.
pub const ICON_SM: f32 = 13.0;
/// Default glyphs: the tool rail, the shape slot, kit rows and menus.
pub const ICON_MD: f32 = 16.0;
/// Icon buttons (`kit::icon_button`, the Align/Pathfinder chips): 18 inside the 26 × 24 target (QW6).
pub const ICON_LG: f32 = 18.0;
/// The icon button's hit target — never below `KIT_MIN_TARGET` in either direction.
pub const ICON_BTN_W: f32 = 26.0;
pub const ICON_BTN_H: f32 = KIT_MIN_TARGET;
/// A toggle's "on" mark (owner decision: toggle = a small azure bar, no fill; tool = an azure block).
pub const ICON_BAR_W: f32 = 10.0;
pub const ICON_BAR_H: f32 = 2.0;
/// Kit menus: minimum popup width and the gap between the anchor and the popup.
pub const KIT_MENU_MIN_W: f32 = 176.0;
pub const KIT_MENU_GAP: f32 = 4.0;

#[cfg(test)]
mod tests {
    use egui::Color32;

    #[test]
    fn shortcut_labels_use_the_platform_primary_modifier() {
        let (primary, save, search) =
            if cfg!(target_os = "macos") { ("⌘", "⌘S", "⌘ K") } else { ("Ctrl", "Ctrl+S", "Ctrl K") };
        assert_eq!(super::primary_mod_label(), primary);
        assert_eq!(super::shortcut_label("S"), save);
        assert_eq!(format!("{} K", super::primary_mod_label()), search);
    }

    /// The premultiplied azure consts are bit-equal to the `from_rgba_unmultiplied` calls they
    /// replace (that constructor is not const, so the outputs are baked in — this proves them).
    #[test]
    fn premultiplied_exact() {
        assert_eq!(super::ACCENT_SEL, Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, 60));
        assert_eq!(super::ACCENT_TINT, Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, 34));
    }
}
