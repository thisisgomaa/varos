//! Lane C: preserve process channels or a named ink from the existing CMYK slider tab.
use super::*;
use varos_app::shell::kit::field::{self as kf, Label, NumberField, TextField};
use varos_core::colour_management::{Cmyk, Colour, ManagedColour};
use varos_core::colour_management_commands::Command;
pub(super) struct State {
    spot: bool,
    name: String,
    tint: f32,
}
impl Default for State {
    fn default() -> Self {
        Self { spot: false, name: "Spot 1".into(), tint: 100. }
    }
}
pub(super) fn height() -> f32 {
    t::PICKER_SLIDER_ROW_H * 3.
}
pub(super) fn show(ui: &mut egui::Ui, m: &mut ColorPanel, ops: &mut Vec<Op>) {
    if m.mode != modes::Mode::Cmyk {
        return;
    }
    let MTarget::Paint(target) = m.target else { return };
    if let Some(i) = kit::text_dropdown(
        ui,
        ui.id().with("ink-kind"),
        if m.managed.spot { "Spot" } else { "Process CMYK" },
        &["Process CMYK", "Spot"],
        t::PICKER_MODE_W,
        "Preserve source colour model",
    ) {
        m.managed.spot = i == 1;
    }
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(t::PICKER_MODE_W, t::FIELD_H), egui::Sense::hover());
        if let Some(name) = kf::text_field(
            ui,
            TextField {
                id: ui.id().with("ink-name"),
                rect,
                value: &m.managed.name,
                font: t::body(),
                framed: true,
                open: false,
                hint: "Spot name",
            },
            |s| if s.len() <= 256 && !s.trim().is_empty() { Ok(s.to_owned()) } else { Err("Enter a spot name") },
        )
        .commit
        {
            m.managed.name = name;
        }
        let edit = kf::number_field(
            ui,
            NumberField {
                id: ui.id().with("ink-tint"),
                width: t::GRADIENT_NUMBER_W,
                label: Label::Letter("Tint"),
                tip: "Spot tint %",
                value: m.managed.tint,
                decimals: 1,
                speed: 0.5,
                range: 0.0..=100.0,
                disabled: !m.managed.spot,
            },
        );
        if let Some(v) = edit.commit.or(edit.live) {
            m.managed.tint = v;
        }
    });
    if kit::action(ui, kit::Control::new(ui.id().with("keep-ink"), "Apply source colour"), false).activated {
        m.finish(ops);
        let rgba = m.color();
        let values = m
            .channel_state
            .filter(|(mode, _, _)| *mode == modes::Mode::Cmyk)
            .map_or_else(|| Cmyk::from_rgb(rgba).channels().map(|v| v * 100.), |(_, _, v)| v);
        let [c, mm, y, k] = values.map(|v| v / 100.);
        let colour = if m.managed.spot {
            Colour::Spot { name: m.managed.name.clone(), tint: m.managed.tint / 100., alt: Cmyk { c, m: mm, y, k } }
        } else {
            Colour::Cmyk { c, m: mm, y, k }
        };
        ops.push(Op::DocumentSetup(EditCommand::ColourManagement(Command::Paint {
            target,
            colour: ManagedColour { colour, alpha: rgba[3] },
        })));
    }
}
