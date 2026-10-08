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
/// Toggle/segment on well (owner 2026-10-08): below PANEL, no azure bar.
pub const TOGGLE_WELL: Color32 = rgb(0x000000);
pub const PANEL: Color32 = rgb(0x1b1919); // box / panel fill
pub const SURFACE: Color32 = rgb(0x242121); // inset field / control fill
pub const HOVER: Color32 = rgb(0x2b2828); // hover-state fill
pub const LINE: Color32 = rgb(0x2c2929); // 1px hairline
pub const LINE2: Color32 = rgb(0x3b3735); // stronger hairline
pub const TEXT: Color32 = rgb(0xe9e6e3); // primary text
pub const MUTED: Color32 = rgb(0x8f8a86); // secondary text / icons
pub const DISABLED: Color32 = rgb(0x6e6a66); // disabled-state text only
pub const ROW_HOVER: Color32 = rgb(0x262323); // list-row hover — calmer than HOVER (between SURFACE and HOVER)
pub const INPUT_WELL: Color32 = rgb(0x171515); // darker inset well behind a focused text edit
pub const ACCENT: Color32 = rgb(0x0c8ce9); // azure scalpel — active / selection / focus ONLY
/// Owner 2026-10-08: azure = human selection/focus; AGENT = anything an agent is doing.
/// Burnt orange: 3.81:1 on white and 4.58:1 on the dark PANEL (full-strength UI outlines).
pub const AGENT: Color32 = rgb(0xc76a20);
pub const AGENT_PAGE_STROKE: f32 = 1.5;
pub const AGENT_OBJECT_STROKE: f32 = 1.0;
pub const AGENT_LABEL_MIN_PAGE_W: f32 = 160.0;
pub const AGENT_LABEL_H: f32 = 18.0;
pub const AGENT_LABEL_GAP: f32 = 4.0;
pub const AGENT_LABEL_PAD: f32 = 4.0;
/// Existing artboard settings dots occupy the last 26 pt of the title row.
pub const AGENT_LABEL_TITLE_TRAIL: f32 = 30.0;
pub const ACCENT_HOVER: Color32 = rgb(0x2b9df4); // hovered accent button — one step lighter azure
                                                 // `from_rgba_unmultiplied` is not const — these are its EXACT outputs for the azure at α 60 / 34
                                                 // (proven bit-equal by `premultiplied_exact` below).
pub const ACCENT_SEL: Color32 = Color32::from_rgba_premultiplied(3, 33, 55, 60); // text-selection fill
pub const ACCENT_TINT: Color32 = Color32::from_rgba_premultiplied(2, 19, 31, 34); // faint azure wash (selected row)
pub const GUIDE: Color32 = rgb(0xff54a8); // reserved for smart guides (not yet wired — keep, Ahmed 07-08)
pub const SEAM: Color32 = rgb(0x000000); // the VOID — band, seams, status (one flat backdrop, 4b 2026-10-05)

// ── secondary palette (content / samples, NOT chrome) ──
pub const NAVY: Color32 = rgb(0x12263a);
pub const AMBER: Color32 = rgb(0xf0b429);
pub const RULER_BG: Color32 = rgb(0x181616);
pub const CLOSE_RED: Color32 = rgb(0xc42b1c);
pub const NONE_RED: Color32 = rgb(0xe05c5c);
/// A field's refused-commit reason and failure notices (UI_SYSTEM K5; same value as `NONE_RED`):
/// 4.88:1 on PANEL, so the reason sits on PANEL.
pub const ERROR: Color32 = rgb(0xe05c5c);
pub const DOT_GRID: Color32 = Color32::from_rgba_premultiplied(11, 11, 11, 11); // rgba(255,255,255,.045)
pub const VOID_HOVER: Color32 = Color32::from_rgba_premultiplied(10, 10, 10, 10); // rgba(255,255,255,.04)

// ── radii & rhythm ──
pub const R: u8 = 3; // controls: fields, chips, buttons, tabs
pub const RBOX: u8 = 8; // boxes / panels (rounder / fancier — Ahmed 07-04)
pub const RCAP: u8 = 11; // pill / capsule radius — tab pills + scroll chevrons, one "Claude bubble" look (Ahmed 07-08)
pub const SEAM_GAP: f32 = 12.0; // equal void between all boxes (wider +20% so boxes breathe — Ahmed 07-04)

// ── the 4b top band (owner-approved 2026-10-05, docs/foundation/MAC_CHROME.md §"4b band"): one 52-pt
// band = 12 + 28 + 12, every control 28 tall on the one centre line y 26 (the traffic lights' too).
// Boxes start at the band's bottom; the 12 above/below a chip is the same 12 as the seams. ──
/// The band's height (macOS); boxes start right under it.
pub const BAND_H: f32 = 52.0;
/// Every control in the band: Home, tabs, `+`, `+N`, the V mark.
pub const BAND_CHIP_H: f32 = 28.0;
/// The void above (and below) a chip — the seam rhythm.
pub const BAND_PAD_Y: f32 = SEAM_GAP;
/// The right zone (empty band, the V mark at its right end — no Search since 2026-10-06) when no
/// panel column is docked, and on Home: this wide, ending `SEAM_GAP` before the window's right edge.
pub const BAND_RIGHT_ZONE_W: f32 = 288.0;
/// The V mark's square button and the mark painted inside it.
pub const BAND_BRAND: f32 = 28.0;
pub const BAND_BRAND_MARK: f32 = 18.0;
/// Home ↔ the first tab; the tabs never come closer than this to the V mark either.
pub const BAND_GAP: f32 = 8.0;
/// The "+N ⌄" button (hidden tabs) and its gap to its neighbours.
pub const BAND_OVERFLOW_W: f32 = 40.0;
pub const BAND_OVERFLOW_GAP: f32 = 4.0;
/// "+N" sits this far in; the chevron's left edge this far in.
pub const BAND_OVERFLOW_TEXT_X: f32 = 8.0;
pub const BAND_OVERFLOW_CHEV_X: f32 = 24.0;
/// The count's size (Inter 11.5) and the chevron glyph.
pub const BAND_OVERFLOW_TEXT: f32 = 11.5;
pub const BAND_OVERFLOW_CHEV: f32 = 12.0;
/// The tabs leave at least this much void before the right zone (the drag handle).
pub const BAND_TABS_END_GAP: f32 = SEAM_GAP;
/// The dashed landing-slot outline while a tab is dragged: dash and gap lengths.
pub const BAND_DASH: f32 = 3.0;
pub const BAND_DASH_GAP: f32 = 2.0;
/// A tab chip: width = clamp(TAB_PAD_L + name width (Inter 500 12) + TAB_TRAIL, TAB_W_MIN, TAB_W_MAX).
pub const TAB_W_MIN: f32 = 88.0;
pub const TAB_W_MAX: f32 = 176.0;
pub const TAB_PAD_L: f32 = 12.0;
pub const TAB_TRAIL: f32 = 30.0;
/// The name's box (from `TAB_PAD_L`) is this much narrower than the chip (room for the dot / ×).
pub const TAB_NAME_CLIP: f32 = 42.0;
/// The dirty dot (diameter) and the dot / × centre's distance from the chip's right edge.
pub const TAB_DOT: f32 = 6.0;
pub const TAB_MARK_INSET: f32 = 16.0;
/// The × hit square, and the × glyph inside it.
pub const TAB_CLOSE_HIT: f32 = 18.0;
pub const TAB_CLOSE_ICON: f32 = 12.0;

pub fn r_ctrl() -> CornerRadius {
    CornerRadius::same(R)
}
pub fn r_box() -> CornerRadius {
    CornerRadius::same(RBOX)
}
pub fn hairline() -> Stroke {
    Stroke::new(1.0, LINE)
}

/// Compatibility mapping for existing call sites. New UI uses the semantic constructors below.
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

/// Start v2 type scale (points / static weight / family).
///
/// | Constructor | Size | Weight | Family |
/// |---|---:|---:|---|
/// | `h1` | 30 | 600 | Inter |
/// | `h2` | 18 | 600 | Inter |
/// | `button` | 15 | 500 | Inter |
/// | `name` | 14 | 600 | Inter |
/// | `body` | 13 | 400 | Inter |
/// | `small` | 12 | 400 | Inter |
/// | `tag` | 11 | 500 | Inter |
/// | `mono` | 11 | 400 | JetBrains Mono |
fn weighted(size: f32, family: &'static str) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(family.into()))
}
pub fn h1() -> egui::FontId {
    weighted(30.0, super::fonts::UI_600)
}
pub fn h2() -> egui::FontId {
    weighted(18.0, super::fonts::UI_600)
}
pub fn button() -> egui::FontId {
    weighted(15.0, super::fonts::UI_500)
}
pub fn name() -> egui::FontId {
    weighted(14.0, super::fonts::UI_600)
}
pub fn body() -> egui::FontId {
    weighted(13.0, super::fonts::UI_400)
}
pub fn small() -> egui::FontId {
    weighted(12.0, super::fonts::UI_400)
}
pub fn tag() -> egui::FontId {
    weighted(11.0, super::fonts::UI_500)
}
pub fn mono() -> egui::FontId {
    weighted(11.0, super::fonts::MONO_400)
}
// ── Start v2 extra roles (lane L4): the mockup's other size/weight pairs, built on the same families.
// | `button_strong` 15/600 (primary hero title) · `lede` 15/400 (first-launch lede) · `body_medium` 13/500
// | (preset name, Recovered name, menu) · `small_medium` 12/500 (panel heading, text buttons) · `micro`
// | 11/400 (status line, table header).
pub fn button_strong() -> egui::FontId {
    weighted(15.0, super::fonts::UI_600)
}
pub fn lede() -> egui::FontId {
    weighted(15.0, super::fonts::UI_400)
}
pub fn body_medium() -> egui::FontId {
    weighted(13.0, super::fonts::UI_500)
}
pub fn small_medium() -> egui::FontId {
    weighted(12.0, super::fonts::UI_500)
}
pub fn micro() -> egui::FontId {
    weighted(11.0, super::fonts::UI_400)
}

/// Upper-case panel section label: Inter 500, 10.5 pt, tracked and secondary.
pub fn micro_label(text: impl AsRef<str>) -> egui::RichText {
    egui::RichText::new(text.as_ref().to_uppercase())
        .font(weighted(T_MICRO, super::fonts::UI_500))
        .extra_letter_spacing(MICRO_TRACKING)
        .color(MUTED)
}

/// A panel's real title: Inter 600, 13 pt, primary text.
pub fn panel_title(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).font(panel_title_font()).color(TEXT)
}
/// The font of [`panel_title`] (Inter 600, 13 pt), for hand-painted titles (the Review panel).
pub fn panel_title_font() -> egui::FontId {
    weighted(PANEL_TITLE_TEXT, super::fonts::UI_600)
}

/// Tabular numeric values/readouts at the caller's size; labels remain proportional Inter.
pub fn numeric_value(size: f32) -> egui::FontId {
    egui::FontId::monospace(size)
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

// ── Start v2 — Boards (lane L4, 2026-10-04). `SB_` = Start · Boards. Every number below is read off the
// design of record, design-reference/mockups/start-v2/src.html (its CSS is the spec; NOTES.md there
// explains the grid). CSS border-box paddings that sit inside a 1 px border are written border-inclusive
// (e.g. the box's "padding: 31px 39px" + 1 px border = 32 / 40). Text sizes live in
// `start_page::style` until lane L1's type tokens land. ──
/// The primary (inverted, TEXT-filled) button's secondary text and shortcut (NOTES "Tokens").
pub const ON_TEXT_MUTED: Color32 = rgb(0x5c5753);
/// The card well's dot grid: white α .07 (src.html `.dots`), premultiplied — proven in tests/start_page.rs.
pub const WELL_DOT: Color32 = Color32::from_rgba_premultiplied(18, 18, 18, 18);
// page frame: the BG box sits SEAM_GAP inside the area under the bar; the status line is the void below it
pub const SB_STATUS_H: f32 = 32.0;
pub const SB_STATUS_INSET: f32 = 24.0;
pub const SB_STATUS_ICON_GAP: f32 = 8.0;
/// The box's top padding: the mockup's 32 (box at y 40 under a 28 bar) minus the 12 the 4b band now
/// holds above the box (box at y 52) — so the approved content stays at window y 72 and the two card
/// rows still fit at 1512 × 982 (review 2026-10-05).
pub const SB_PAD_TOP: f32 = 32.0 - SEAM_GAP;
pub const SB_PAD_X: f32 = 40.0;
pub const SB_PAD_BOTTOM: f32 = 28.0;
// the column grid (responsive rule: start_page::grid_columns)
pub const SB_COL: f32 = 272.0;
pub const SB_COL_MAX: f32 = 320.0;
pub const SB_GUTTER: f32 = SEAM_GAP;
pub const SB_COLS_MIN: usize = 3;
pub const SB_COLS_MAX: usize = 6;
/// The content column never grows past six maximum columns; beyond it, it is centred in the box.
pub const SB_CONTENT_MAX: f32 = SB_COL_MAX * SB_COLS_MAX as f32 + SB_GUTTER * (SB_COLS_MAX - 1) as f32;
// hero: actions (cols 1–2) + the preset panel (cols 3–5); stacks when the panel would drop under its minimum
pub const SB_HERO_H: f32 = 152.0;
pub const SB_BIG_W: f32 = SB_COL;
pub const SB_BIG_H: f32 = 64.0;
pub const SB_ACTIONS_W: f32 = SB_BIG_W * 2.0 + SB_GUTTER;
pub const SB_PRESETS_W: f32 = 840.0;
pub const SB_PRESETS_MIN_W: f32 = 560.0;
/// Content width under which the hero stacks (actions above the preset panel): 556 + 12 + 560.
pub const SB_HERO_STACK_W: f32 = SB_ACTIONS_W + SB_GUTTER + SB_PRESETS_MIN_W;
pub const SB_BIG_PAD_L: f32 = 18.0;
pub const SB_BIG_PAD_R: f32 = 16.0;
pub const SB_BIG_GAP: f32 = 14.0;
pub const SB_BIG_SUB_GAP: f32 = 3.0;
/// The hero buttons' and the Recovered band's glyph (src.html `svg.i.lg`).
pub const SB_ICON_HERO: f32 = 20.0;
/// Small glyphs: view toggle, status shield (src.html `svg.i.sm`).
pub const SB_ICON_SMALL: f32 = 14.0;
pub const SB_LEDE_GAP: f32 = 18.0;
pub const SB_LEDE_W: f32 = 500.0;
pub const SB_KEYS_GAP: f32 = 16.0;
pub const SB_KBD_GAP: f32 = 6.0;
pub const SB_KBD_H: f32 = 18.0;
pub const SB_KBD_PAD: f32 = 4.0;
// the preset panel ("…or start with an artboard — true proportions")
pub const SB_PANEL_HEAD_H: f32 = 32.0;
pub const SB_PANEL_PAD_X: f32 = 16.0;
pub const SB_PANEL_HEAD_GAP: f32 = 8.0;
pub const SB_PRESET_TOP: f32 = 12.0;
pub const SB_PRESET_PV: f32 = 56.0;
pub const SB_PRESET_NAME_GAP: f32 = 10.0;
pub const SB_PRESET_SIZE_GAP: f32 = 3.0;
/// One shared scale for every preset outline: 1920 document units = 56 px, on one baseline.
pub const SB_PRESET_SCALE: f32 = SB_PRESET_PV / 1920.0;
pub const SB_DASH: f32 = 3.0;
pub const SB_DASH_GAP: f32 = 2.0;
// vertical rhythm under the hero
pub const SB_SECTION_GAP: f32 = 28.0;
pub const SB_RECOV_H: f32 = 52.0;
pub const SB_RECOV_PAD_L: f32 = 20.0;
pub const SB_RECOV_PAD_R: f32 = 12.0;
pub const SB_RECOV_ICON_GAP: f32 = 14.0;
pub const SB_RECOV_TEXT_GAP: f32 = 12.0;
pub const SB_BTN_H: f32 = 28.0;
pub const SB_BTN_PAD: f32 = 14.0;
// ── The editor's recovery card + Review panel (owner decision 2026-10-06, direction B; mockups
// recovery-B-canvas-card-*.png / review-panel.png). The card IS Start's Recovered band (`SB_RECOV_*`,
// `SB_BTN_*`) floating in the Board box; these are only the numbers the band does not have. ──
/// The card's / panel's bottom edge sits this far above the Board box's bottom edge (also the least
/// gap kept to the box's other edges when the box is small).
pub const RC_GAP: f32 = 24.0;
/// From the end of the sentence to Later.
pub const RC_TEXT_BTN_GAP: f32 = 20.0;
/// Between Later and Review (and Discard and Restore).
pub const RC_BTN_GAP: f32 = 8.0;
/// Under this Board-box width the card drops its second phrase ("from your last session").
pub const RC_NARROW_W: f32 = 600.0;
// Compact floating recovery layouts.
/// Minimum sentence width before the card stacks its text above the buttons.
pub const RC_MIN_TEXT_W: f32 = 80.0;
/// Card height when its text and buttons are stacked.
pub const RC_STACK_CARD_H: f32 = 88.0;
/// Review row height when its text and buttons are stacked.
pub const RC_STACK_ROW_H: f32 = 96.0;
/// Below this inner row width, Review rows stack their text above the buttons.
pub const RC_COMPACT_ROW_W: f32 = 420.0;

/// The Review panel: width, header / row / footer heights, side padding.
pub const RC_PANEL_W: f32 = 620.0;
pub const RC_HEAD_H: f32 = 44.0;
pub const RC_ROW_H: f32 = KIT_ROW_H;
pub const RC_FOOT_H: f32 = 36.0;
pub const RC_PAD_X: f32 = 16.0;
/// Header glyph → title, title → count, footer shield → text.
pub const RC_HEAD_ICON_GAP: f32 = 10.0;
pub const RC_COUNT_GAP: f32 = 8.0;
pub const RC_FOOT_ICON_GAP: f32 = 8.0;
/// A row's name line and detail line: their tops inside the 56 row, and their line boxes.
pub const RC_NAME_TOP: f32 = 11.0;
pub const RC_DETAIL_TOP: f32 = 31.0;
pub const RC_NAME_LINE: f32 = 16.0;
pub const RC_DETAIL_LINE: f32 = 14.0;
/// The gap between the detail text and the folder path on a row's second line.
pub const RC_PATH_GAP: f32 = 12.0;
pub const SB_HEAD_H: f32 = 28.0;
pub const SB_HEAD_GAP: f32 = 16.0;
pub const SB_COUNT_GAP: f32 = 8.0;
pub const SB_FILTERS_GAP: f32 = 32.0;
pub const SB_FILTER_PAD: f32 = 8.0;
pub const SB_FILTER_INNER: f32 = 6.0;
pub const SB_FILTER_SPACING: f32 = 2.0;
pub const SB_FILTER_BAR: f32 = 2.0;
pub const SB_FILTER_BAR_R: u8 = 1;
pub const SB_SEG_BTN_W: f32 = 28.0;
pub const SB_SEG_BTN_H: f32 = 22.0;
pub const SB_SEG_PAD: f32 = 1.0;
pub const SB_SEG_R: u8 = 2;
// board cards
pub const SB_CARD_H: f32 = 266.0;
pub const SB_WELL_H: f32 = 123.0;
pub const SB_WELL_R: u8 = RBOX - 1;
pub const SB_CARD_PAD_X: f32 = 14.0;
pub const SB_CARD_PAD_TOP: f32 = 11.0;
pub const SB_CARD_PAD_BOTTOM: f32 = 12.0;
pub const SB_DATE_GAP: f32 = 10.0;
pub const SB_DESC_GAP: f32 = 5.0;
pub const SB_TAGS_GAP: f32 = 10.0;
pub const SB_PILL_H: f32 = 20.0;
pub const SB_PILL_PAD: f32 = 8.0;
pub const SB_PILL_GAP: f32 = 4.0;
/// The Board section's removable tag chip: the gap before its ×, the × glyph, the padding after it.
pub const SB_CHIP_GAP: f32 = 4.0;
pub const SB_CHIP_X: f32 = 10.0;
pub const SB_CHIP_PAD_R: f32 = 6.0;
/// The Board section (Properties, nothing selected): a label line, the description box's rows, the
/// tag field's line height and inner padding, the gap after each field.
pub const BOARD_LABEL_H: f32 = 18.0;
pub const BOARD_DESC_ROWS: usize = 3;
pub const BOARD_TAG_LINE: f32 = 24.0;
pub const BOARD_TAG_PAD: f32 = 4.0;
pub const BOARD_TAG_INPUT_MIN: f32 = 72.0;
pub const BOARD_GAP: f32 = 8.0;
/// The least gap before "2 artboards" (it sits in the tags row: `.tags { gap: 4px }`, `margin-left: auto`).
pub const SB_FACTS_GAP: f32 = SB_PILL_GAP;
pub const SB_THUMB_W: f32 = 244.0;
pub const SB_THUMB_H: f32 = 99.0;
/// A thumbnail whose aspect is within this of the well's is a whole-well image (lane L3's 544×246).
pub const SB_THUMB_WELL_TOLERANCE: f32 = 0.08;
pub const SB_DOT_STEP: f32 = 12.0;
pub const SB_DOT_R: f32 = 0.75;
pub const SB_CHIP: f32 = 24.0;
pub const SB_CHIP_INSET: f32 = 9.0;
pub const SB_MENU_W: f32 = 184.0;
pub const SB_MENU_ROW_H: f32 = 28.0;
pub const SB_MENU_PAD: f32 = 10.0;
pub const SB_MISS_X: f32 = 14.0;
pub const SB_MISS_Y: f32 = 12.0;
pub const SB_MISS_GAP: f32 = 8.0;
pub const SB_MISS_PILL_H: f32 = 18.0;
pub const SB_MISS_PILL_PAD: f32 = 7.0;
/// The typographic placeholder (no thumbnail yet): an outline per artboard (≤ 3, offset) or a dashed
/// "free" outline, with the board's initials.
pub const SB_PH_W: f32 = 88.0;
pub const SB_PH_H: f32 = 56.0;
pub const SB_PH_STACK: f32 = 6.0;
pub const SB_PH_MAX: usize = 3;
// the list view (numbered table: # · Name + description · Tags · Folder · Modified)
pub const SB_TH_H: f32 = 32.0;
pub const SB_ROW_H: f32 = 50.0;
pub const SB_COL_NUM: f32 = 40.0;
pub const SB_COL_WIDE: f32 = 300.0;
pub const SB_COL_WIDE_MIN: f32 = 160.0;
/// The Tags and Folder columns each take this share of what the fixed columns leave (300 at 1408).
pub const SB_COL_WIDE_SHARE: f32 = 0.26;
pub const SB_COL_DATE: f32 = 132.0;
pub const SB_COL_GAP: f32 = 16.0;
pub const SB_NUM_PAD: f32 = 4.0;
pub const SB_LIST_DESC_GAP: f32 = 1.0;
// empty / first launch (centred block)
pub const SB_FIRST_LEDE_GAP: f32 = 12.0;
pub const SB_FIRST_LEDE_W: f32 = 520.0;
pub const SB_FIRST_BTNS_GAP: f32 = 32.0;
pub const SB_FIRST_KEYS_GAP: f32 = 20.0;
pub const SB_FIRST_PRESETS_GAP: f32 = 56.0;
pub const SB_FIRST_PAD_BOTTOM: f32 = 24.0;

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
/// Panel icon segments: 28×20 paint, lifted to a 24 pt interaction target.
pub const PANEL_SEG_W: f32 = 28.0;
pub const PANEL_SEG_H: f32 = 20.0;
pub const PANEL_SEG_GLYPH: f32 = ICON_MD;
pub const HARMONY_TRACK_W: f32 = 232.0;
pub const PANEL_STRIP_GAP: f32 = 2.0;
pub const PANEL_DIVIDER_H: f32 = 16.0;
pub const DOC_UNITS_W: f32 = 52.0;
pub const SWATCH_RING_INSET: f32 = 5.0;
/// Panel typography and section rhythm (polish pass, 2026-10-05).
pub const T_MICRO: f32 = 10.5;
pub const MICRO_TRACKING: f32 = 0.6;
pub const PANEL_TITLE_TEXT: f32 = 13.0;
pub const SECTION_GAP_HALF: f32 = 7.0;
/// Align's two section breaks compensate for its taller micro-labels and 24-point segmented track.
pub const ALIGN_SECTION_GAP: f32 = 5.0;
pub const LABEL_GAP: f32 = 6.0;
/// Shared grey segmented control geometry.
pub const SEG_W: f32 = 60.0;
pub const SEG_BTN_H: f32 = 20.0;
pub const SEG_H: f32 = SEG_BTN_H + (KIT_STROKE + SB_SEG_PAD) * 2.0;
pub const SEG_TEXT: f32 = 11.0;
/// The fixed leading name slot keeps each control-bar mode stable as the selection changes.
pub const CONTROL_BAR_NAME_W: f32 = 64.0;
pub const CONTROL_BAR_NAME_H: f32 = 26.0;
pub const CONTROL_BAR_NAME_TEXT: f32 = 11.5;
/// One Pathfinder glyph geometry in either its dock or control-bar home.
pub const PF_BAR_W: f32 = ICON_BTN_W;
pub const PF_BAR_H: f32 = ICON_BTN_H;
pub const PF_INK: f32 = 16.0;
pub const PF_SQUARE: f32 = 10.0;
pub const PF_OFFSET: f32 = 8.0;
pub const PF_STROKE: f32 = 1.5;
pub const PF_RADIUS: u8 = 2; // egui's integer radius: nearest representable value to the 1.5 pt target
/// Fields (K3, piece P2 — `kit::field`): the number-field row and its label column, the text-field
/// box, the value text size and the label letter size, and the text inset inside a box.
pub const FIELD_H: f32 = 25.0;
pub const TEXT_FIELD_H: f32 = 26.0;
pub const FIELD_LABEL_W: f32 = 18.0;
pub const FIELD_TEXT: f32 = 13.0;
pub const FIELD_LABEL_TEXT: f32 = 11.5;
pub const FIELD_INSET_X: f32 = 8.0;
pub const FIELD_INSET_Y: f32 = 3.0;
/// Number-only typography and inset; shared text fields deliberately retain `FIELD_TEXT`/`FIELD_INSET_X`.
pub const NUM_TEXT: f32 = 12.0;
pub const NUM_INSET_X: f32 = 3.0;
pub const NUM_LABEL_RIGHT_INSET: f32 = 4.0;
pub const NUM_ICON_CENTER_X: f32 = 11.0;
pub const FIELD_LABEL_BOX_GAP: f32 = 2.0;
/// Transform geometry shared with paint-row alignment.
pub const PANEL_ITEM_GAP_X: f32 = 6.0;
pub const TRANSFORM_REFPOINT_SIZE: f32 = 38.0;
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
