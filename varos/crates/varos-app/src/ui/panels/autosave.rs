//! Slice 1.10 provisional controls: owner design review pending.
use super::super::*;
use varos_app::shell::{
    kit::field::{self, TextField},
    tokens as t,
};
pub(super) fn settings(
    ui: &mut egui::Ui,
    w: f32,
    r: &crate::recovery_host::RecoveryUi,
    commands: &mut Vec<AppCommand>,
) {
    if toggle_row(ui, w, "Autosave to file", r.autosave_enabled) {
        commands.push(AppCommand::SetAutosave(!r.autosave_enabled, r.autosave_interval_seconds));
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("After inactivity (seconds)").font(t::small()).color(t::MUTED));
        if r.autosave_enabled {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(t::DOC_UNITS_W, t::FIELD_H), egui::Sense::hover());
            let value = r.autosave_interval_seconds.to_string();
            let edit = field::text_field(
                ui,
                TextField {
                    id: ui.id().with("autosave-interval"),
                    rect,
                    value: &value,
                    font: t::small(),
                    framed: true,
                    open: false,
                    hint: "120",
                },
                |text| {
                    text.parse::<u64>()
                        .ok()
                        .filter(|n| varos_app::storage::settings::valid_autosave_interval(*n))
                        .ok_or("Use 30–1800 seconds")
                },
            );
            if let Some(seconds) = edit.commit {
                commands.push(AppCommand::SetAutosave(true, seconds));
            }
        } else {
            ui.label(RichText::new(r.autosave_interval_seconds.to_string()).font(t::small()).color(t::MUTED));
        }
    });
    ui.label(
        RichText::new(if r.enabled {
            "Saves changes to the open file. Recovery copies stay on."
        } else {
            "Saves changes to the open file. Recovery copies are off."
        })
        .font(t::small())
        .color(t::MUTED),
    );
}
