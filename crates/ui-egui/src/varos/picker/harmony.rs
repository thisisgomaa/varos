// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The Harmony tab: a strip of rules, the wheel with the linked hues marked, the rule's six chips
//! and rows of variations around each linked hue. A click on a chip sets the focused target. The
//! rule maths are the old Varos `colour_guide` (hue offsets and the clamped Mono brightness steps),
//! as they were; no colour management.

use egui::{Pos2, Rect, Sense, Ui, vec2};
use serde::{Deserialize, Serialize};

use super::{Action, PAD, Prefs, State, TAB_GAP, TAB_H, hsv_to_rgb};
use crate::theme::Tokens;

pub(crate) const STRIP_H: f32 = 32.0;
pub(crate) const ROW_H: f32 = 28.0;
pub(crate) const SWATCH: f32 = 20.0;
pub(crate) const GUIDE_ROW_H: f32 = 24.0;
const SWATCH_GAP: f32 = 4.0;

/// A harmony rule, kept with the picker's preferences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    #[default]
    Complementary,
    Analogous,
    Split,
    Triadic,
    Tetradic,
    Square,
    Mono,
    None,
}

/// The strip's rules and their glyphs.
const RULES: [(Rule, &str); 8] = [
    (Rule::Complementary, super::icons::COMPLEMENTARY),
    (Rule::Analogous, super::icons::ANALOGOUS),
    (Rule::Split, super::icons::SPLIT),
    (Rule::Triadic, super::icons::TRIAD),
    (Rule::Tetradic, super::icons::TETRADIC),
    (Rule::Square, super::icons::SQUARE),
    (Rule::Mono, super::icons::MONO),
    (Rule::None, super::icons::NONE),
];

/// The tooltip of `rule`, translated.
fn tip(rule: Rule) -> &'static str {
    match rule {
        Rule::Complementary => tl!("Complementary"),
        Rule::Analogous => tl!("Analogous"),
        Rule::Split => tl!("Split Complementary"),
        Rule::Triadic => tl!("Triad"),
        Rule::Tetradic => tl!("Tetrad"),
        Rule::Square => tl!("Square"),
        Rule::Mono => tl!("Monochromatic"),
        Rule::None => tl!("None"),
    }
}

/// Hue offsets in degrees.
pub(crate) fn offsets(rule: Rule) -> &'static [f32] {
    match rule {
        Rule::Complementary => &[180.0],
        Rule::Analogous => &[-30.0, 30.0],
        Rule::Split => &[150.0, 210.0],
        Rule::Triadic => &[120.0, 240.0],
        Rule::Tetradic => &[60.0, 180.0, 240.0],
        Rule::Square => &[90.0, 180.0, 270.0],
        _ => &[],
    }
}

/// The linked colours (HSV 0..1), the base first.
pub(crate) fn linked(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    match rule {
        Rule::None => vec![base],
        Rule::Mono => [1.0, 0.78, 0.56, 0.36].iter().map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)]).collect(),
        _ => {
            let mut v = vec![base];
            for off in offsets(rule) {
                v.push([(base[0] + off / 360.0).rem_euclid(1.0), base[1], base[2]]);
            }
            v
        }
    }
}

/// Six chips: the linked set first, then tones from the Mono progression. None keeps only the
/// base hue; Mono continues its brightness steps without duplicate chips.
pub(crate) fn swatches(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    if rule == Rule::None {
        return std::iter::once(base)
            .chain([0.78, 0.56, 0.36, 0.22, 0.12].iter().map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)]))
            .collect();
    }
    if rule == Rule::Mono {
        return [1.0, 0.78, 0.56, 0.36, 0.22, 0.12].iter().map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)]).fold(
            Vec::new(),
            |mut out, c| {
                if !out.contains(&c) {
                    out.push(c);
                }
                out
            },
        );
    }
    let mut out = linked(rule, base);
    let seeds = out.clone();
    for k in [0.78, 0.56, 0.36] {
        for c in &seeds {
            if out.len() == 6 {
                return out;
            }
            out.push([c[0], c[1], (c[2] * k).clamp(0.06, 1.0)]);
        }
    }
    out
}

/// Rows of five around each linked hue, brightness and saturation varied.
pub(crate) fn variations(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    linked(rule, base)
        .into_iter()
        .flat_map(|c| [-0.3_f32, -0.15, 0.0, 0.15, 0.3].map(|d| [c[0], (c[1] - d).clamp(0.0, 1.0), (c[2] + d).clamp(0.0, 1.0)]))
        .collect()
}

/// The height of the Harmony tab's parts other than the wheel: the strip, the chips and the
/// variation rows.
pub(crate) fn extra_height(rule: Rule) -> f32 {
    STRIP_H + ROW_H + GUIDE_ROW_H * linked(rule, [0.0, 1.0, 1.0]).len() as f32
}

/// The rule strip at `origin` (`STRIP_H` tall).
pub(super) fn strip(ui: &mut Ui, origin: Pos2, prefs: &mut Prefs) {
    let t = Tokens::get(ui.ctx());
    for (i, (rule, icon)) in RULES.into_iter().enumerate() {
        let r = Rect::from_min_size(origin + vec2(PAD + i as f32 * (TAB_H + TAB_GAP), (STRIP_H - TAB_H) / 2.0), egui::Vec2::splat(TAB_H));
        let resp = ui.interact(r, ui.id().with(("varos-harmony-rule", i)), Sense::click());
        let on = prefs.harmony == rule;
        super::tab_button(ui, &t, r, icon, on, resp.hovered(), true);
        if resp.on_hover_text(tip(rule)).clicked() {
            prefs.harmony = rule;
        }
    }
}

/// The chips and the variation rows at `origin`; a click sets the focused target.
pub(super) fn chips(ui: &mut Ui, origin: Pos2, st: &State, prefs: &Prefs, acts: &mut Vec<Action>) {
    let base = [st.hsva[0], st.hsva[1], st.hsva[2]];
    let chip = |ui: &mut Ui, r: Rect, id: (&str, usize, usize), hsv: [f32; 3], acts: &mut Vec<Action>| {
        let [red, g, b] = hsv_to_rgb(hsv[0], hsv[1], hsv[2]);
        let c = vectorcraft_color::Color::rgb(red, g, b);
        let resp = ui.interact(r, ui.id().with(id), Sense::click());
        super::chip(ui, r, Some(&c), resp.hovered());
        if resp.on_hover_text(c.to_hex().to_uppercase()).clicked() {
            acts.push(Action::Set(c));
        }
    };
    for (i, hsv) in swatches(prefs.harmony, base).into_iter().enumerate() {
        let r = Rect::from_min_size(origin + vec2(PAD + i as f32 * (SWATCH + SWATCH_GAP), (ROW_H - SWATCH) / 2.0), egui::Vec2::splat(SWATCH));
        chip(ui, r, ("varos-harmony-chip", 0, i), hsv, acts);
    }
    for (row, colours) in variations(prefs.harmony, base).chunks(5).enumerate() {
        for (i, hsv) in colours.iter().enumerate() {
            let r = Rect::from_min_size(
                origin + vec2(PAD + i as f32 * (SWATCH + SWATCH_GAP), ROW_H + row as f32 * GUIDE_ROW_H + (GUIDE_ROW_H - SWATCH) / 2.0),
                egui::Vec2::splat(SWATCH),
            );
            chip(ui, r, ("varos-harmony-guide", row, i), *hsv, acts);
        }
    }
}
