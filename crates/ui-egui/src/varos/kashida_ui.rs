//! Varos: the Paragraph panel's kashida controls (provisional UI, awaiting the owner's review).
//!
//! Under the alignment buttons, for a paragraph that runs right to left (the only kind kashida
//! stretches): a segmented **Kashida: Off · Auto · Force** control and a **Strength** slider
//! (0–100, 50 by default; off while Kashida is Off). Both act through `text.setFormat` (`kashida`,
//! `kashidaStrength`) on the paragraphs the panel acts on, as the panel's other controls do: a
//! click is one undo step, a slider drag previews live and is one undo step when released.
//!
//! Design: Varos tokens only. Azure marks the active segment and the slider's fill (the active
//! state), 3 px corners on the segmented control's ends, a 1 px border, no shadows.

use egui::{Color32, CornerRadius, Rect, Sense, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::ParaStyle;
use vectorcraft_doc::text::{KashidaMode, KashidaOptions};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, Live};

/// The segments: mode, label, its `kashida` value and its tooltip. The labels are provisional
/// English, not yet in the upstream catalogs.
const MODES: [(KashidaMode, &str, &str, &str); 3] = [
    (KashidaMode::Off, "Off", "off", "No kashida: word spacing alone justifies the lines"),
    (KashidaMode::Auto, "Auto", "auto", "Justified lines take tatweels in their Arabic words before word spacing"),
    (
        KashidaMode::Force,
        "Force",
        "force",
        "Deeper: more tatweels per word and more of each line's slack; the last line of a paragraph justified with its last line aligned takes kashida too",
    ),
];

/// The step name of a change (the command's own label, already translated).
const STEP: &str = "Character / Paragraph";

/// Panel state: a strength drag is previewing.
const DRAGGING: &str = "varos-kashida-dragging";

/// Where the controls were drawn (for tests).
#[derive(Clone, Copy, Debug)]
pub(crate) struct KashidaRects {
    pub segments: [Rect; 3],
    pub slider: Rect,
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

/// The kashida controls of `para`'s paragraphs.
pub(crate) fn controls(app: &mut VectorcraftApp, ui: &mut Ui, para: &ParaStyle) -> KashidaRects {
    let t = Tokens::get(ui.ctx());
    let k: KashidaOptions = para.kashida;
    let mut rects = KashidaRects { segments: [Rect::NOTHING; 3], slider: Rect::NOTHING };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui.add_sized(vec2(58.0, 22.0), egui::Label::new(egui::RichText::new("Kashida").size(11.5).color(t.text_dim)));
        label.on_hover_text("Tatweels in the Arabic words of justified right-to-left lines");
        let w = ((ui.available_width() - 4.0) / 3.0).clamp(40.0, 72.0);
        for (i, (mode, name, key, tip)) in MODES.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
            rects.segments[i] = rect;
            let on = k.mode == *mode;
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
            p.rect(rect, radius, fill, egui::Stroke::new(1.0, if on { t.accent } else { t.button_border }), egui::StrokeKind::Inside);
            let text = if on { Color32::WHITE } else { t.text };
            p.text(rect.center(), egui::Align2::CENTER_CENTER, *name, egui::FontId::proportional(11.5), text);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, *name));
            if resp.on_hover_text(*tip).clicked() && !on {
                let p = on_paragraphs(app, json!({ "kashida": key }));
                app.run("text.setFormat", p).ok();
            }
        }
    });
    ui.horizontal(|ui| {
        let label = ui.add_sized(vec2(58.0, 20.0), egui::Label::new(egui::RichText::new("Strength").size(11.5).color(t.text_dim)));
        label.on_hover_text("How much of a line's slack kashida fills and how many tatweels a word takes (50: balanced)");
        ui.add_enabled_ui(k.mode != KashidaMode::Off, |ui| {
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
