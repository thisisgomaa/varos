//! Design tokens — transcribed VERBATIM from `docs/UI_VISION_MOCKUP.html` `:root` (the visual law).
//! This is the ONLY place raw colour / radius / spacing numbers live in the shell.
//! (BOX_SYSTEM_PLAN §3 + ruling 1 "tokens from the mockup" + ruling 4 "azure is a scalpel".)
use egui::{Color32, CornerRadius, Stroke};

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
pub const GUIDE: Color32 = rgb(0xff54a8); // smart guides
pub const SEAM: Color32 = rgb(0x0e0d0d); // the VOID — seams, app bar, status (darker than BG)

// ── secondary palette (content / samples, NOT chrome) ──
pub const NAVY: Color32 = rgb(0x12263a);
pub const AMBER: Color32 = rgb(0xf0b429);
pub const RULER_BG: Color32 = rgb(0x181616);
pub const CLOSE_RED: Color32 = rgb(0xc42b1c);
pub const NONE_RED: Color32 = rgb(0xe05c5c);
pub const DOT_GRID: Color32 = Color32::from_rgba_premultiplied(11, 11, 11, 11); // rgba(255,255,255,.045)
pub const VOID_HOVER: Color32 = Color32::from_rgba_premultiplied(10, 10, 10, 10); // rgba(255,255,255,.04)
pub const GRIP: Color32 = rgb(0x9d9893); // the move-handle pill — soft light grey (Ahmed's reference)

// ── radii & rhythm ──
pub const R: u8 = 3; // controls: fields, chips, buttons, tabs
pub const RBOX: u8 = 8; // boxes / panels (rounder / fancier — Ahmed 07-04)
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

#[cfg(test)]
mod tests {
    use egui::Color32;

    /// The premultiplied azure consts are bit-equal to the `from_rgba_unmultiplied` calls they
    /// replace (that constructor is not const, so the outputs are baked in — this proves them).
    #[test]
    fn premultiplied_exact() {
        assert_eq!(super::ACCENT_SEL, Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, 60));
        assert_eq!(super::ACCENT_TINT, Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, 34));
    }
}
