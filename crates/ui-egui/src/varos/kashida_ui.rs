//! Varos: the Paragraph panel's kashida controls (provisional UI, awaiting the owner's review).
//!
//! Under the alignment buttons, when any paragraph the panel acts on runs right to left (the only
//! kind kashida stretches): a segmented **Kashida: Off · Auto · Force** control and a **Strength** slider
//! (0–100, 50 by default; off while Kashida is Off). Both act through `text.setFormat` (`kashida`,
//! `kashidaStrength`) on the paragraphs the panel acts on, as the panel's other controls do: a
//! click is one undo step, a slider drag previews live and is one undo step when released. When
//! the paragraphs differ, the controls show it (no lit segment, a dash after the label) and a click
//! sets them all. Labels and tooltips are English sources translated through the upstream
//! catalogs (`i18n/*.tsv`, "Varos: kashida controls").
//!
//! Design: Varos tokens only. Azure marks the active segment and the slider's fill (the active
//! state), 3 px corners on the segmented control's ends, a 1 px border, no shadows.

use egui::{Color32, CornerRadius, Rect, Sense, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::text::{KashidaMode, KashidaOptions};
use vectorcraft_doc::{NodeKind, ParaStyle};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, Live};

/// The segments: mode and its `kashida` value.
const MODES: [(KashidaMode, &str); 3] = [(KashidaMode::Off, "off"), (KashidaMode::Auto, "auto"), (KashidaMode::Force, "force")];

/// A segment's label and tooltip (English sources, translated through the upstream catalogs).
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

/// The kashida setting and direction (right to left?) of each paragraph the Paragraph panel acts
/// on: those the Type tool's selection touches while it edits text, else every paragraph of the
/// selected text objects (in groups too).
pub(crate) fn targeted(app: &VectorcraftApp) -> Vec<(KashidaOptions, bool)> {
    let Some(st) = app.session.active() else { return vec![] };
    let mut out = vec![];
    let mut add = |t: &vectorcraft_doc::TextObject, range: Option<(usize, usize)>| {
        let plain = t.plain_text();
        let paras: Vec<&str> = plain.split('\n').collect();
        let indices = range.map_or(0..paras.len(), |(a, b)| t.paragraphs_in(a, b));
        for i in indices {
            let p = t.para_at(i);
            out.push((p.kashida, vectorcraft_text::paragraph_is_rtl(paras.get(i).copied().unwrap_or_default(), p.direction)));
        }
    };
    if let Some((id, a, b)) = crate::panels::character::text_editing(app) {
        if let Some(NodeKind::Text(t)) = st.doc.node(id).map(|n| &n.kind) {
            add(t, Some((a, b)));
        }
        return out;
    }
    fn walk(node: &vectorcraft_doc::Node, add: &mut AddText<'_>) {
        if let NodeKind::Text(t) = &node.kind {
            add(t, None);
        }
        for child in node.children().into_iter().flatten() {
            walk(child, add);
        }
    }
    for node in st.selection.objects.iter().filter_map(|id| st.doc.node(*id)) {
        walk(node, &mut add);
    }
    out
}

/// What [`targeted`] does with each text object: its paragraphs in a byte range (`None`: all).
type AddText<'a> = dyn FnMut(&vectorcraft_doc::TextObject, Option<(usize, usize)>) + 'a;

/// Are the controls shown: the first paragraph runs right to left (`first_rtl`), or any other the
/// panel acts on does.
pub(crate) fn shown(app: &VectorcraftApp, first_rtl: bool) -> bool {
    first_rtl || targeted(app).iter().any(|(_, rtl)| *rtl)
}

/// The step name of a change (the command's own label, already translated).
const STEP: &str = "Character / Paragraph";

/// Panel state: a strength drag is previewing.
const DRAGGING: &str = "varos-kashida-dragging";

/// Where the controls were drawn and what they showed (read by the tests; the panel ignores it).
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct KashidaRects {
    pub segments: [Rect; 3],
    pub slider: Rect,
    /// The paragraphs differ in mode, and in strength.
    pub mixed_mode: bool,
    pub mixed_strength: bool,
}

/// `p` aimed at the paragraphs the Paragraph panel acts on: the Type tool's selection (ending its
/// typing session) or the selected text objects (as the panel's own `run_on_paragraphs`).
fn on_paragraphs(app: &mut VectorcraftApp, mut p: Value) -> Value {
    if let Some((id, a, b)) = crate::panels::character::text_editing(app) {
        crate::panels::character::end_typing(app);
        p["ids"] = json!([id.0]);
        p["start"] = json!(a);
        p["end"] = json!(b);
    }
    p
}

/// The kashida controls of the paragraphs the panel acts on (`para`: the first of them). When they
/// differ, no segment is lit and the label shows a dash (mixed); the slider shows the first
/// paragraph's strength, its label dashed too. A click on a segment sets every paragraph.
pub(crate) fn controls(app: &mut VectorcraftApp, ui: &mut Ui, para: &ParaStyle) -> KashidaRects {
    let t = Tokens::get(ui.ctx());
    let k: KashidaOptions = para.kashida;
    let all = targeted(app);
    let mixed_mode = all.iter().any(|(o, _)| o.mode != all[0].0.mode);
    let mixed_strength = all.iter().any(|(o, _)| o.strength() != all[0].0.strength());
    let any_on = all.is_empty() && k.mode != KashidaMode::Off || all.iter().any(|(o, _)| o.mode != KashidaMode::Off);
    let mut rects = KashidaRects { segments: [Rect::NOTHING; 3], slider: Rect::NOTHING, mixed_mode, mixed_strength };
    let mixed_tip = tl!("Mixed: the selected paragraphs differ");
    let label_text = |s: &str, mixed: bool| if mixed { format!("{s} –") } else { s.to_string() };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui
            .add_sized(vec2(58.0, 22.0), egui::Label::new(egui::RichText::new(label_text(tl!("Kashida"), mixed_mode)).size(11.5).color(t.text_dim)));
        label.on_hover_text(if mixed_mode { mixed_tip } else { tl!("Tatweels in the Arabic words of justified right-to-left lines") });
        let w = ((ui.available_width() - 4.0) / 3.0).clamp(40.0, 72.0);
        for (i, (mode, key)) in MODES.iter().enumerate() {
            let (name, tip) = mode_text(*mode);
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
            rects.segments[i] = rect;
            let on = !mixed_mode && k.mode == *mode;
            // Mixed: the modes the paragraphs have are outlined (indeterminate), none is lit.
            let present = mixed_mode && all.iter().any(|(o, _)| o.mode == *mode);
            let r = 3;
            let radius = match i {
                0 => CornerRadius { nw: r, sw: r, ne: 0, se: 0 },
                2 => CornerRadius { nw: 0, sw: 0, ne: r, se: r },
                _ => CornerRadius::ZERO,
            };
            let fill = if on {
                t.accent
            } else if resp.hovered() {
                t.hover
            } else {
                t.button
            };
            let p = ui.painter();
            let border = if on {
                t.accent
            } else if present {
                t.text_dim
            } else {
                t.button_border
            };
            p.rect(rect, radius, fill, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
            let text = if on { Color32::WHITE } else { t.text };
            p.text(rect.center(), egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(11.5), text);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, name));
            // Every paragraph the panel acts on takes the mode, whatever the first one has.
            if resp.on_hover_text(tip).clicked() {
                let p = on_paragraphs(app, json!({ "kashida": key }));
                app.run("text.setFormat", p).ok();
            }
        }
    });
    ui.horizontal(|ui| {
        let label = ui.add_sized(
            vec2(58.0, 20.0),
            egui::Label::new(egui::RichText::new(label_text(tl!("Strength"), mixed_strength)).size(11.5).color(t.text_dim)),
        );
        label.on_hover_text(if mixed_strength {
            mixed_tip
        } else {
            tl!("How much of a line's slack kashida fills and how many tatweels a word takes (50: balanced)")
        });
        ui.add_enabled_ui(any_on, |ui| {
            let mut v = f64::from(k.strength());
            ui.spacing_mut().slider_width = (ui.available_width() - 44.0).max(60.0);
            let resp = widgets::slider(ui, egui::Slider::new(&mut v, 0.0..=100.0).step_by(1.0).max_decimals(0));
            rects.slider = resp.rect;
            // A drag previews from its first change and commits once, when released.
            let dragging: bool = crate::panels::pstate(ui.ctx(), DRAGGING);
            let phase = if resp.drag_stopped() && dragging {
                Live::Released
            } else if resp.dragged() && resp.changed() {
                Live::Dragging
            } else if resp.changed() && !resp.dragged() {
                Live::Released
            } else {
                Live::Idle
            };
            match phase {
                Live::Dragging => crate::panels::set_pstate(ui.ctx(), DRAGGING, true),
                Live::Released => crate::panels::set_pstate(ui.ctx(), DRAGGING, false),
                Live::Idle => {}
            }
            if phase != Live::Idle {
                let p = on_paragraphs(app, json!({ "kashidaStrength": v.round() }));
                crate::panels::live_run(app, STEP, "text.setFormat", p, phase);
            }
        });
    });
    rects
}

#[cfg(test)]
#[path = "tests_kashida_ui.rs"]
mod tests;
