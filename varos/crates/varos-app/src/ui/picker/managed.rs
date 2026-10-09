//! Lane C: preserve process channels or a named ink from the existing CMYK slider tab.
use super::*;
use varos_app::shell::kit::field::{self as kf, Label, NumberField, TextField};
use varos_core::colour_management::{Cmyk, ManagedColour};
use varos_core::colour_management_commands::Command;
pub(super) struct State {
    spot: bool,
    name: String,
    tint: f32,
    source: Option<varos_core::model::Paint>,
    original: Option<ManagedColour>,
}
impl Default for State {
    fn default() -> Self {
        Self { spot: false, name: "Spot 1".into(), tint: 100., source: None, original: None }
    }
}
pub(super) fn seed(m: &mut ColorPanel, ed: &Editor) {
    if m.gesture_active() {
        return;
    }
    let MTarget::Paint(target) = m.target else { return };
    let paint = super::super::colour_management::source_paint(ed, target);
    if m.managed.source.as_ref() == Some(&paint) {
        return;
    }
    let state = super::super::colour_management::State::from_paint(&paint);
    // Live RGB edits already retain their authored channel_state; foreign edits re-seed in follow_selection.
    if matches!(paint, varos_core::model::Paint::Managed(_)) {
        m.managed.original = state.original;
        m.managed.spot = state.spot;
        m.managed.name = state.name;
        m.managed.tint = state.tint;
        m.channel_state = Some((modes::Mode::Cmyk, m.color(), state.values));
    }
    m.managed.source = Some(paint);
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
        apply_source(m, target, ops);
    }
}
fn apply_source(m: &mut ColorPanel, target: PaintTarget, ops: &mut Vec<Op>) {
    m.finish(ops);
    let rgba = m.color();
    let values = m
        .channel_state
        .filter(|(mode, _, _)| *mode == modes::Mode::Cmyk)
        .map_or_else(|| Cmyk::from_rgb(rgba).channels().map(|v| v * 100.), |(_, _, v)| v);
    ops.push(Op::DocumentSetup(EditCommand::ColourManagement(Command::Paint {
        target,
        colour: super::super::colour_management::source_colour(
            values,
            m.managed.spot,
            &m.managed.name,
            m.managed.tint,
            rgba[3],
            m.managed.original.as_ref(),
        ),
    })));
}
#[cfg(test)]
mod tests {
    use super::*;
    use varos_core::colour_management::Colour;
    use varos_core::model::{Paint, Path};
    #[test]
    fn reopening_and_applying_preserves_cmyk_and_resolved_spot() {
        for colour in [
            Colour::Cmyk { c: 0.2, m: 0.3, y: 0.4, k: 0.1 },
            Colour::Spot { name: "Brand ink".into(), tint: 0.65, alt: Cmyk { c: 0.2, m: 0.3, y: 0.4, k: 0.1 } },
        ] {
            let mut ed = Editor::new();
            let authored = Paint::Managed(ManagedColour { colour, alpha: 0.7 });
            ed.doc.swatches.push(varos_core::swatches::Swatch {
                id: 1,
                name: "Global".into(),
                paint: authored.clone(),
                global: true,
                group: String::new(),
            });
            let mut p = Path::new(1, vec![], true, None, None, 1.);
            p.fill = Paint::SwatchRef { id: 1 };
            ed.doc.paths.push(p);
            ed.doc.sync_tree();
            ed.objsel.insert(1);
            let mut panel = None;
            open_picker(&mut panel, MTarget::Paint(PaintTarget::Fill), &mut ed);
            let m = panel.as_mut().unwrap();
            m.mode = modes::Mode::Cmyk;
            assert_eq!(m.channel_values(), [0.2, 0.3, 0.4, 0.1].map(|v| v * 100.));
            let mut ops = Vec::new();
            apply_source(m, PaintTarget::Fill, &mut ops);
            apply_ops(&mut ed, ops);
            assert_eq!(ed.doc.paths[0].fill, authored);
            // External source edits with identical display RGB still update the ink controls.
            let replacement = Paint::Managed(ManagedColour {
                colour: Colour::Cmyk { c: 0., m: 0.125, y: 0.25, k: 0.28 },
                alpha: 0.7,
            });
            ed.doc.paths[0].fill = replacement;
            follow_selection(m, &mut ed);
            assert_eq!(m.channel_values(), [0., 12.5, 25., 28.]);
            assert!(!m.managed.spot);
        }
    }
    #[test]
    fn seed_does_not_overwrite_channels_during_active_gesture() {
        let ed = Editor::new();
        let mut m = ColorPanel::new(MTarget::Paint(PaintTarget::Fill), Some([0.5; 4]), false);
        m.mode = modes::Mode::Cmyk;
        m.adopt_channel(0, 32.);
        m.gesture = Some(Gesture::Slider);
        seed(&mut m, &ed);
        assert_eq!(m.channel_values()[0], 32.);
    }
}
