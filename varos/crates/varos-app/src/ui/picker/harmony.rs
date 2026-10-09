use super::*;
use varos_app::storage::layout::HarmonyRule as Rule;
const RULES: [(Rule, Icon, &str); 8] = [
    (Rule::Complementary, Icon::HarmonyComplementary, "Complementary"),
    (Rule::Analogous, Icon::HarmonyAnalogous, "Analogous"),
    (Rule::Split, Icon::HarmonySplit, "Split complementary"),
    (Rule::Triadic, Icon::HarmonyTriad, "Triad"),
    (Rule::Tetradic, Icon::HarmonyTetradic, "Tetradic"),
    (Rule::Square, Icon::HarmonySquare, "Square"),
    (Rule::Mono, Icon::HarmonyMono, "Monochromatic"),
    (Rule::Shades, Icon::HarmonyNone, "Shades"),
];
pub(super) fn show(ui: &mut egui::Ui, m: &mut ColorPanel, s: &Snap, layout: &mut PickerLayout, ops: &mut Vec<Op>) {
    let (strip, _) = ui.allocate_exact_size(egui::vec2(t::PICKER_W, t::PICKER_HARMONY_STRIP_H), egui::Sense::hover());
    let mut row = ui.new_child(egui::UiBuilder::new().id_salt("harmony-rules").max_rect(strip.shrink(t::PICKER_PAD)));
    row.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t::PICKER_TAB_GAP;
        for (rule, icon, tip) in RULES {
            if kit::icon_button_sized(
                ui,
                ui.id().with(tip),
                icon,
                tip,
                kit::IconState::Toggle(m.harmony == rule),
                egui::vec2(t::PICKER_TAB_H, t::PICKER_TAB_H),
                t::PICKER_GLYPH,
            )
            .activated
                && !kit::field::blocked(ui.ctx())
            {
                m.harmony = rule;
                layout.harmony = rule;
            }
        }
    });
    wheel::show(ui, m, s, ops);
    let (row, _) = ui.allocate_exact_size(egui::vec2(t::PICKER_W, t::PICKER_HARMONY_ROW_H), egui::Sense::hover());
    for (i, [h, s, v]) in harmony_rules::swatches(m.harmony, [m.hsva[0], m.hsva[1], m.hsva[2]]).into_iter().enumerate()
    {
        let rgb = hsv_to_rgb(h, s, v);
        let color = [rgb[0], rgb[1], rgb[2], m.hsva[3]];
        let r = egui::Rect::from_min_size(
            row.min
                + egui::vec2(
                    t::PICKER_PAD + i as f32 * (t::PICKER_HARMONY_SWATCH + t::PICKER_SWATCH_GAP),
                    (t::PICKER_HARMONY_ROW_H - t::PICKER_HARMONY_SWATCH) / 2.0,
                ),
            egui::Vec2::splat(t::PICKER_HARMONY_SWATCH),
        );
        #[cfg(test)]
        super::super::fields::tests::probe(&format!("harmony swatch {i}"), r);
        if color[3] < 1.0 {
            checker(&ui.painter_at(r), r, t::PICKER_CHECKER);
        }
        ui.painter().rect(r, t::r_ctrl(), rgba_c32a(color), t::hairline(), StrokeKind::Inside);
        if ui.interact(r, ui.id().with(("harmony", i)), egui::Sense::click()).on_hover_text(hex_of(color)).clicked()
            && !kit::field::blocked(ui.ctx())
        {
            ops.push(Op::PickerSet(m.target, color));
        }
    }
}
