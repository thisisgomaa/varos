//! Burger mirror of Object > Clipping Mask; uses the existing painted menu kit.
use super::*;
pub(super) fn seed(ctx: &egui::Context, ed: &Editor) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("clip-enabled"), (ed.clip_make_enabled(), ed.clip_release_enabled())));
}
pub(super) fn rows(ui: &mut egui::Ui, active: Option<SessionId>, cmds: &mut Vec<AppCommand>) -> bool {
    let state = ui.ctx().data(|d| d.get_temp::<(bool, bool)>(egui::Id::new("clip-enabled"))).unwrap_or_default();
    let mut hit = false;
    for (release, enabled, label, shortcut) in [
        (false, state.0, "Clipping Mask > Make", shortcut_label("7")),
        (
            true,
            state.1,
            "Clipping Mask > Release",
            if cfg!(target_os = "macos") { "⌥⌘7".into() } else { "Alt+Ctrl+7".into() },
        ),
    ] {
        if let Some(id) = active.filter(|_| enabled) {
            if menu_row(ui, label, &shortcut) {
                cmds.push(AppCommand::Clip(id, release));
                hit = true;
            }
        } else {
            menu_row_disabled(ui, label, "Select artwork for this clipping command.");
        }
    }
    hit
}
