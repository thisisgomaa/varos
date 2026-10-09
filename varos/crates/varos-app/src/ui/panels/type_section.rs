//! Lane G: Properties > Type, implemented (provisional UI, owner design review pending).
use super::super::*;
use varos_app::shell::tokens::TYPE_FIELD_W;
use varos_core::text::{Alignment, Direction, Kashida, TextBox};
pub(crate) fn type_section(ui: &mut egui::Ui, text: &TextBox, ops: &mut Vec<Op>) {
    ui.label(panel_title("Type"));
    let mut next = text.clone();
    let Some(first) = text.runs.first() else {
        return;
    };
    let Ok(fonts) = varos_text_layout::host_fonts::snapshot() else {
        kit::notice(ui, "Font discovery failed");
        return;
    };
    let labels: Vec<_> = fonts.faces().iter().map(|f| format!("{} {}", f.family, f.weight)).collect();
    let names: Vec<_> = labels.iter().map(String::as_str).collect();
    let selected = fonts
        .faces()
        .iter()
        .position(|f| varos_text_layout::font_hash(f.content_hash) == first.style.font.hash)
        .map(|i| names[i]);
    let missing = format!("Missing: {}", first.style.font.family);
    if let Some(index) = kit::text_dropdown(
        ui,
        doc_id(ui, "type-family"),
        selected.unwrap_or(&missing),
        &names,
        TYPE_FIELD_W,
        "Font family",
    ) {
        {
            if let Some(font) = fonts.faces().get(index) {
                for run in &mut next.runs {
                    run.style.font = varos_core::text::FontRef {
                        family: font.family.into(),
                        weight: u32::from(font.weight),
                        hash: varos_text_layout::font_hash(font.content_hash),
                    };
                }
            }
        }
    }
    number(ui, "Font size", first.style.size, 0.1..=4096., false, ops, |value| {
        let mut next = text.clone();
        for run in &mut next.runs {
            run.style.size = value;
        }
        Op::Text(next)
    });
    number(ui, "Line height", text.para.line_height, 1.3..=20., false, ops, |value| {
        let mut next = text.clone();
        next.para.line_height = value;
        Op::Text(next)
    });
    let arabic = varos_core::text::contains_arabic(&text.source());
    number(
        ui,
        if arabic { "Letter spacing (Arabic: 0)" } else { "Letter spacing" },
        first.style.letter_spacing,
        -first.style.size..=first.style.size,
        arabic,
        ops,
        |value| {
            let mut next = text.clone();
            for run in &mut next.runs {
                run.style.letter_spacing = value;
            }
            Op::Text(next)
        },
    );
    let align = [Alignment::Left, Alignment::Centre, Alignment::Right, Alignment::Justify];
    if let Some(i) = choice(
        ui,
        "Alignment",
        &["Left", "Centre", "Right", "Justify"],
        align.iter().position(|v| *v == text.para.align).unwrap_or(2),
    ) {
        next.para.align = align[i];
    }
    let direction = [Direction::Auto, Direction::Ltr, Direction::Rtl];
    if let Some(i) = choice(
        ui,
        "Direction",
        &["Auto", "Left to right", "Right to left"],
        direction.iter().position(|v| *v == text.para.direction).unwrap_or(0),
    ) {
        next.para.direction = direction[i];
    }
    let kashida = [Kashida::Off, Kashida::Minimal, Kashida::Balanced, Kashida::Display];
    if let Some(i) = choice(
        ui,
        "Kashida",
        &["Off", "Minimal", "Balanced", "Display"],
        kashida.iter().position(|v| *v == text.para.kashida).unwrap_or(0),
    ) {
        next.para.kashida = kashida[i];
    }
    if next != *text {
        ops.push(Op::Text(next));
    }
}
fn choice(ui: &mut egui::Ui, label: &str, names: &[&str], selected: usize) -> Option<usize> {
    kit::text_dropdown(
        ui,
        doc_id(ui, ("type", label)),
        &format!("{label}: {}", names[selected]),
        names,
        TYPE_FIELD_W,
        label,
    )
}
#[allow(clippy::too_many_arguments)]
fn number(
    ui: &mut egui::Ui,
    label: &'static str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    disabled: bool,
    ops: &mut Vec<Op>,
    mk: impl Fn(f32) -> Op,
) {
    ui.label(micro_label(label));
    fields::num_disabled(ui, TYPE_FIELD_W, Lab::Letter(""), label, value, 2, 0.1, range, disabled, ops, mk);
}
