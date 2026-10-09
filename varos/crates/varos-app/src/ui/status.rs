//! Shared muted status hint, including file autosave. No independent repaint timer.
use varos_app::shell::tokens as t;
pub(super) fn hint(ui: &mut egui::Ui, rect: egui::Rect, text: &str) {
    ui.painter().with_clip_rect(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, text, t::small(), t::MUTED);
    ui.interact(rect, ui.id().with("file-status"), egui::Sense::hover()).on_hover_text(text);
}
