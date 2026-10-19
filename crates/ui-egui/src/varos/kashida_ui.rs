//! Varos: the Paragraph panel's kashida rows, in its Middle Eastern group
//! ([`super::middle_eastern_ui::paragraph_group`]), when any paragraph the panel acts on runs right
//! to left (the only kind kashida stretches). After the owner's panel mockup
//! (`docs/varos/mockups/panels.html`, Paragraph variants A and C).
//!
//! * **Kashida**: three icon + label buttons, Off · Auto · Force, their tatweel icons growing
//!   longer. The icons are new to people, so the words stay beside them.
//! * **Strength**: its icon, a rail (0–100, 50 by default) and a number field that takes any value
//!   from 0 up (owner 2026-10-19): justified lines read more than 100 as 100, and Force elongates
//!   lines that aren't justified by the strength in percent. Greyed while Kashida is Off.
//!
//! Both act through `text.setFormat` (`kashida`, `kashidaStrength`) on the paragraphs the panel
//! acts on, as the panel's other controls do: a click is one undo step, a rail drag previews live
//! and is one undo step when released. When the paragraphs differ nothing lights, the modes they
//! have are tinted, and the rail drops its thumb and the field reads "–"; a click sets them all.
//! Labels and tooltips are English sources translated through the upstream catalogs (`i18n/*.tsv`,
//! "Varos: kashida controls").

use egui::{Rect, Ui, vec2};
use serde_json::json;
use vectorcraft_doc::ParaStyle;
use vectorcraft_doc::text::{KashidaMode, KashidaOptions};

use super::me_icons;
use super::middle_eastern_ui::{Choice, Drawn, for_each_text, number_field, rail, row_icon, segments, targeted as aimed};
use crate::VectorcraftApp;

/// The choices: mode, its `kashida` value, its icon and the drawn record's key.
const MODES: [(KashidaMode, &str, &str, &str); 3] = [
    (KashidaMode::Off, "off", me_icons::KASHIDA_OFF, "kashida-off"),
    (KashidaMode::Auto, "auto", me_icons::KASHIDA_AUTO, "kashida-auto"),
    (KashidaMode::Force, "force", me_icons::KASHIDA_FORCE, "kashida-force"),
];

/// A choice's label and tooltip (English sources, translated through the upstream catalogs).
fn mode_text(mode: KashidaMode) -> (&'static str, &'static str) {
    match mode {
        KashidaMode::Off => (tl!("Off"), tl!("No kashida: word spacing alone justifies the lines")),
        KashidaMode::Auto => (tl!("Auto"), tl!("Justified lines take tatweels in their Arabic words before word spacing")),
        KashidaMode::Force => (
            tl!("Force"),
            tl!(
                "Deeper: more tatweels per word and more of each line's slack; the last line of a paragraph justified with its last line aligned takes kashida too"
            ),
        ),
    }
}

/// A paragraph the Paragraph panel acts on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Para {
    pub kashida: KashidaOptions,
    /// It runs right to left (resolved).
    pub rtl: bool,
    /// Its direction setting (`None`: Auto).
    pub direction: Option<vectorcraft_doc::ParaDirection>,
}

/// Each paragraph the Paragraph panel acts on: those the Type tool's selection touches while it
/// edits text, else every paragraph of the selected text objects (in groups too).
pub(crate) fn targeted(app: &VectorcraftApp) -> Vec<Para> {
    let mut out = vec![];
    for_each_text(app, |t, range| {
        let plain = t.plain_text();
        let paras: Vec<&str> = plain.split('\n').collect();
        let indices = range.map_or(0..paras.len(), |(a, b)| t.paragraphs_in(a, b));
        for i in indices {
            let p = t.para_at(i);
            let rtl = vectorcraft_text::paragraph_is_rtl(paras.get(i).copied().unwrap_or_default(), p.direction);
            out.push(Para { kashida: p.kashida, rtl, direction: p.direction });
        }
    });
    out
}

/// Are the rows shown: the first paragraph runs right to left (`first_rtl`), or any other the
/// panel acts on does.
pub(crate) fn shown(app: &VectorcraftApp, first_rtl: bool) -> bool {
    first_rtl || targeted(app).iter().any(|p| p.rtl)
}

/// The step name of a change (the command's own label, already translated).
const STEP: &str = "Character / Paragraph";

/// Where the rows were drawn and what they showed (read by the tests; the panel ignores it).
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct KashidaRects {
    pub segments: [Rect; 3],
    /// The strength rail and its number field.
    pub slider: Rect,
    pub field: Rect,
    /// The paragraphs differ in mode, and in strength.
    pub mixed_mode: bool,
    pub mixed_strength: bool,
    pub controls: Vec<Drawn>,
}

/// The kashida rows of the paragraphs the panel acts on (`para`: the first of them).
pub(crate) fn controls(app: &mut VectorcraftApp, ui: &mut Ui, para: &ParaStyle) -> KashidaRects {
    let k: KashidaOptions = para.kashida;
    let all = targeted(app);
    let first = all.first().map_or(k, |p| p.kashida);
    let mixed_mode = all.iter().any(|p| p.kashida.mode != first.mode);
    let mixed_strength = all.iter().any(|p| p.kashida.strength != first.strength);
    let any_on = if all.is_empty() { k.mode != KashidaMode::Off } else { all.iter().any(|p| p.kashida.mode != KashidaMode::Off) };
    let mut rects =
        KashidaRects { segments: [Rect::NOTHING; 3], slider: Rect::NOTHING, field: Rect::NOTHING, mixed_mode, mixed_strength, controls: vec![] };
    ui.add_space(2.0);
    let labels = MODES.map(|m| mode_text(m.0));
    let choices: Vec<Choice<'_>> = MODES
        .iter()
        .zip(&labels)
        .map(|((mode, _, icon, key), (label, tip))| Choice {
            key,
            icon,
            icon_size: vec2(24.0, 16.0),
            label,
            tip: (*tip).into(),
            lit: !mixed_mode && first.mode == *mode,
            tinted: mixed_mode && all.iter().any(|p| p.kashida.mode == *mode),
            ghost: false,
        })
        .collect();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let w = ui.available_width();
        if let Some(i) = segments(ui, &choices, w, &mut rects.controls) {
            // Every paragraph the panel acts on takes the mode, whatever the first one has.
            let p = aimed(app, json!({ "kashida": MODES[i].1 }));
            app.run("text.setFormat", p).ok();
        }
    });
    for (i, key) in ["kashida-off", "kashida-auto", "kashida-force"].iter().enumerate() {
        rects.segments[i] = rects.controls.iter().find(|c| c.key == *key).map_or(Rect::NOTHING, |c| c.rect);
    }
    ui.add_enabled_ui(any_on, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let tip = tl!("How much of a line's slack kashida fills and how many tatweels a word takes (50: balanced)");
            row_icon(ui, "strength-icon", me_icons::KASHIDA_STRENGTH, tip, &mut rects.controls);
            let value = (!mixed_strength).then_some(f64::from(first.strength));
            let field_w = 40.0;
            let w = (ui.available_width() - field_w - 6.0).max(40.0);
            // The rail spans 0–100; the field takes more (an elongation in percent).
            let (r, hit) = rail(ui, "varos-kashida-strength-rail", tl!("Kashida strength"), tip, value, 100.0, w);
            rects.slider = r;
            rects.controls.push(Drawn { key: "strength", rect: r, tip: tip.into(), ..Default::default() });
            if let Some((v, phase)) = hit {
                // A drag previews from its first change and commits once, when released.
                let p = aimed(app, json!({ "kashidaStrength": v.max(0.0) }));
                crate::panels::live_run(app, STEP, "text.setFormat", p, phase);
            }
            let (f, typed) = number_field(ui, "varos-kashida-strength-field", tl!("Kashida strength"), value, field_w);
            rects.field = f;
            rects.controls.push(Drawn { key: "strength-field", rect: f, tip: tl!("Kashida strength").into(), ..Default::default() });
            if let Some(v) = typed {
                let p = aimed(app, json!({ "kashidaStrength": v.round().max(0.0) }));
                app.run("text.setFormat", p).ok();
            }
        });
    });
    rects
}

#[cfg(test)]
#[path = "tests_kashida_ui.rs"]
mod tests;
