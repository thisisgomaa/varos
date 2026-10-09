//! Lane C: provisional controls from the existing kit; owner design review pending.
use super::*;
use varos_app::shell::{kit::field as kf, tokens as t};
use varos_core::colour_management::{Cmyk, Colour, ColourMode, ManagedColour};
use varos_core::colour_management_commands::Command;
pub(crate) fn mode(ui: &mut egui::Ui, value: &mut ColourMode) -> bool {
    if let Some(i) = kit::text_dropdown(
        ui,
        ui.id().with("colour-mode"),
        if *value == ColourMode::Rgb { "RGB" } else { "CMYK" },
        &["RGB", "CMYK"],
        t::DOC_UNITS_W,
        "Document colour mode",
    ) {
        *value = if i == 0 { ColourMode::Rgb } else { ColourMode::Cmyk };
        return true;
    }
    false
}
pub(crate) fn setup(ui: &mut egui::Ui, ed: &Editor, ops: &mut Vec<Op>) {
    let mut value = ed.doc.colour_mode;
    if mode(ui, &mut value) {
        ops.push(Op::DocumentSetup(EditCommand::ColourManagement(Command::Mode { mode: value })));
    }
    kit::notice(ui, ed.doc.output_profile.as_ref().map_or("Unprofiled CMYK approximation", |p| p.name.as_str()));
}
#[derive(Clone)]
struct State {
    values: [f32; 4],
    spot: bool,
    name: String,
    tint: f32,
    alpha: f32,
}
fn number(ui: &mut egui::Ui, name: &str, value: &mut f32) {
    let e = kf::number_field(
        ui,
        kf::NumberField {
            id: ui.id().with(name),
            width: t::GRADIENT_NUMBER_W,
            label: kf::Label::Letter(name),
            tip: name,
            value: *value,
            decimals: 1,
            speed: 0.5,
            range: 0.0..=100.0,
            disabled: false,
        },
    );
    if let Some(v) = e.commit.or(e.live) {
        *value = v;
    }
}
pub(crate) fn panel(ui: &mut egui::Ui, ed: &Editor, ops: &mut Vec<Op>) {
    for (label, enabled, command) in [
        ("Proof Colours", ed.colour_preview.proof, Command::Proof { enabled: !ed.colour_preview.proof }),
        (
            "Overprint Preview (approx.)",
            ed.colour_preview.overprint,
            Command::Overprint { enabled: !ed.colour_preview.overprint },
        ),
    ] {
        let mut control = kit::Control::new(ui.id().with(label), label);
        control.selected = enabled;
        if kit::action(ui, control, false).activated {
            ops.push(Op::DocumentSetup(EditCommand::ColourManagement(command)));
        }
    }
    if ed.colour_preview.overprint {
        kit::notice(ui, "All vector paints multiply; ink separations are not simulated.");
    }
    kit::text(ui, "CMYK / Spot", t::body(), t::TEXT);
    let key = ui.id().with("managed-colour");
    let mut state = ui.ctx().data(|d| d.get_temp::<State>(key)).unwrap_or_else(|| {
        let c = ed
            .doc
            .paths
            .iter()
            .find(|p| ed.selected_pids().contains(&p.id))
            .and_then(|p| {
                if ed.paint == PaintTarget::Fill {
                    p.fill.resolved(&ed.doc).representative()
                } else {
                    p.stroke.resolved(&ed.doc).representative()
                }
            })
            .unwrap_or([0., 0., 0., 1.]);
        State {
            values: Cmyk::from_rgb(c).channels().map(|v| v * 100.),
            spot: false,
            name: "Spot 1".into(),
            tint: 100.,
            alpha: c[3],
        }
    });
    if let Some(i) = kit::text_dropdown(
        ui,
        key.with("kind"),
        if state.spot { "Spot" } else { "CMYK" },
        &["CMYK", "Spot"],
        t::DOC_UNITS_W,
        "Colour model",
    ) {
        state.spot = i == 1;
    }
    ui.horizontal(|ui| {
        for (name, v) in ["C", "M", "Y", "K"].into_iter().zip(&mut state.values) {
            number(ui, name, v);
        }
    });
    if state.spot {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(t::GRADIENT_BAR_W, t::FIELD_H), egui::Sense::hover());
        if let Some(name) = kf::text_field(
            ui,
            kf::TextField {
                id: key.with("name"),
                rect,
                value: &state.name,
                font: t::body(),
                framed: true,
                open: false,
                hint: "Spot name",
            },
            |s| if !s.trim().is_empty() && s.len() <= 256 { Ok(s.to_owned()) } else { Err("Enter a spot name") },
        )
        .commit
        {
            state.name = name;
        }
        number(ui, "Tint", &mut state.tint);
    }
    if kit::action(ui, kit::Control::new(key.with("apply"), "Apply colour"), false).activated {
        let [c, m, y, k] = state.values.map(|v| v / 100.);
        let colour = if state.spot {
            Colour::Spot { name: state.name.clone(), tint: state.tint / 100., alt: Cmyk { c, m, y, k } }
        } else {
            Colour::Cmyk { c, m, y, k }
        };
        ops.push(Op::DocumentSetup(EditCommand::ColourManagement(Command::Paint {
            target: ed.paint,
            colour: ManagedColour { colour, alpha: state.alpha },
        })));
    }
    kit::text(ui, "Unprofiled screen approximation", t::micro(), t::MUTED);
    ui.ctx().data_mut(|d| d.insert_temp(key, state));
}
