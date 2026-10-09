//! Lane H: existing-kit Properties > Type extension; provisional owner review pending.
use super::super::*;
use varos_app::shell::{kit::field as kf, tokens as t};
use varos_core::{
    text::TextBox,
    typography::{Action, Binding, CharacterStyle, ParagraphStyle, PathEffect, Typography},
};
pub(super) fn section(ui: &mut egui::Ui, text: &TextBox, styles: &Typography, ops: &mut Vec<Op>) {
    if text.id == 0 {
        kit::notice(ui, "Finish typing to apply named styles or path options.");
        return;
    }
    let frame = styles.frames.get(&text.id).cloned().unwrap_or_default();
    ui.label(panel_title("Styles"));
    let chars: Vec<_> = styles.characters.keys().map(String::as_str).collect();
    let current = frame.characters.first().map_or("Character style", |a| a.name.as_str());
    if let Some(i) =
        kit::text_dropdown(ui, doc_id(ui, "character-style"), current, &chars, t::TYPE_FIELD_W, "Character style")
    {
        if !text.source().is_empty() {
            ops.push(Op::Typography(Action::ApplyCharacter {
                text: text.id,
                start: 0,
                end: text.source().len(),
                name: chars[i].into(),
            }));
        }
    }
    if let Some(name) =
        name_field(ui, "Save character style", current.strip_prefix("Character style").map_or(current, |_| ""))
    {
        if let Some(run) = text.runs.first() {
            ops.push(Op::Typography(Action::DefineCharacter {
                name,
                definition: CharacterStyle { parent: None, style: Some(run.style.clone()) },
            }));
        }
    }
    if let Some(assignment) = frame.characters.first() {
        if kit::action(ui, kit::Control::new(doc_id(ui, "update-character-style"), "Update character style"), false)
            .activated
        {
            if let Some(run) = text.runs.first() {
                ops.push(Op::Typography(Action::DefineCharacter {
                    name: assignment.name.clone(),
                    definition: CharacterStyle { parent: None, style: Some(run.style.clone()) },
                }));
            }
        }
    }
    let paras: Vec<_> = styles.paragraphs.keys().map(String::as_str).collect();
    if let Some(i) = kit::text_dropdown(
        ui,
        doc_id(ui, "paragraph-style"),
        frame.paragraph.as_deref().unwrap_or("Paragraph style"),
        &paras,
        t::TYPE_FIELD_W,
        "Paragraph style",
    ) {
        ops.push(Op::Typography(Action::ApplyParagraph { text: text.id, name: paras[i].into() }));
    }
    if let Some(name) = name_field(ui, "Save paragraph style", frame.paragraph.as_deref().unwrap_or("")) {
        ops.push(Op::Typography(Action::DefineParagraph {
            name,
            definition: ParagraphStyle { parent: None, style: Some(text.para.clone()) },
        }));
    }
    if let Some(name) = &frame.paragraph {
        if kit::action(ui, kit::Control::new(doc_id(ui, "update-paragraph-style"), "Update paragraph style"), false)
            .activated
        {
            ops.push(Op::Typography(Action::DefineParagraph {
                name: name.clone(),
                definition: ParagraphStyle { parent: None, style: Some(text.para.clone()) },
            }));
        }
    }
    ui.label(panel_title("OpenType"));
    for (tag, label) in [
        ("liga", "Ligatures"),
        ("calt", "Contextual alternates"),
        ("ss01", "Stylistic set 1"),
        ("ss02", "Stylistic set 2"),
        ("lnum", "Lining numerals"),
        ("onum", "Oldstyle numerals"),
        ("tnum", "Tabular numerals"),
    ] {
        let selected = match frame.features.get(tag) {
            None => 0,
            Some(0) => 1,
            _ => 2,
        };
        let names = ["Font default", "Off", "On"];
        if let Some(i) = kit::text_dropdown(
            ui,
            doc_id(ui, ("ot", tag)),
            &format!("{label}: {}", names[selected]),
            &names,
            t::TYPE_FIELD_W,
            label,
        ) {
            let mut features = frame.features.clone();
            if i == 0 {
                features.remove(tag);
            } else {
                features.insert(tag.into(), u32::from(i == 2));
            }
            ops.push(Op::Typography(Action::Features { text: text.id, features }));
        }
    }
    if let Some(binding) = frame.binding {
        ui.label(panel_title("Text frame"));
        match binding {
            Binding::Area { path, inset } => number(ui, "Inset", inset, 0. ..=1e6, ops, |inset| Action::Bind {
                text: text.id,
                binding: Some(Binding::Area { path, inset }),
            }),
            Binding::Path { path, start, end, offset, flip, effect } => {
                for (label, value) in [("Start bracket", start), ("End bracket", end), ("Baseline offset", offset)] {
                    number(
                        ui,
                        label,
                        value,
                        if label == "Baseline offset" { -1e6..=1e6 } else { 0. ..=1e6 },
                        ops,
                        |v| Action::Bind {
                            text: text.id,
                            binding: Some(Binding::Path {
                                path,
                                start: if label == "Start bracket" { v } else { start },
                                end: if label == "End bracket" { v } else { end },
                                offset: if label == "Baseline offset" { v } else { offset },
                                flip,
                                effect,
                            }),
                        },
                    );
                }
                if let Some(i) = kit::text_dropdown(
                    ui,
                    doc_id(ui, "path-flip"),
                    if flip { "Flipped" } else { "Normal side" },
                    &["Normal side", "Flipped"],
                    t::TYPE_FIELD_W,
                    "Path side",
                ) {
                    ops.push(Op::Typography(Action::Bind {
                        text: text.id,
                        binding: Some(Binding::Path { path, start, end, offset, flip: i == 1, effect }),
                    }));
                }
                if let Some(i) = kit::text_dropdown(
                    ui,
                    doc_id(ui, "path-effect"),
                    if effect == PathEffect::Rainbow { "Rainbow" } else { "Skew" },
                    &["Rainbow", "Skew"],
                    t::TYPE_FIELD_W,
                    "Path effect",
                ) {
                    ops.push(Op::Typography(Action::Bind {
                        text: text.id,
                        binding: Some(Binding::Path {
                            path,
                            start,
                            end,
                            offset,
                            flip,
                            effect: if i == 0 { PathEffect::Rainbow } else { PathEffect::Skew },
                        }),
                    }));
                }
            }
        }
    }
    if frame.next.is_some()
        && kit::text_dropdown(
            ui,
            doc_id(ui, "unthread"),
            "Threaded frame",
            &["Unthread"],
            t::TYPE_FIELD_W,
            "Text thread",
        )
        .is_some()
    {
        ops.push(Op::Typography(Action::Thread { from: text.id, to: None }));
    }
}
fn name_field(ui: &mut egui::Ui, label: &str, value: &str) -> Option<String> {
    ui.label(micro_label(label));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(t::TYPE_FIELD_W, t::FIELD_H), egui::Sense::hover());
    kf::text_field(
        ui,
        kf::TextField {
            id: doc_id(ui, label),
            rect,
            value,
            font: t::mono(),
            framed: true,
            open: false,
            hint: "Style name",
        },
        |s| if s.trim().is_empty() || s.len() > 128 { Err("Enter a style name") } else { Ok(s.trim().to_string()) },
    )
    .commit
}
fn number(
    ui: &mut egui::Ui,
    label: &'static str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    ops: &mut Vec<Op>,
    mk: impl Fn(f32) -> Action,
) {
    ui.label(micro_label(label));
    fields::num_disabled(ui, t::TYPE_FIELD_W, Lab::Letter(""), label, value, 2, 0.1, range, false, ops, |v| {
        Op::Typography(mk(v))
    });
}
