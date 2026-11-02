//! Varos keyboard owners that run inside upstream's `shortcuts::handle`, each behind one marked
//! hook, before the shortcut table sees the frame's keys.
use crate::VectorcraftApp;
use egui::{Key, Modifiers};

/// A document-tab reorder or a panel-layout gesture owns the keyboard until it ends (Escape
/// cancels it). `true`: no shortcut or tool sees this frame's keys.
pub(crate) fn gestures(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    if super::tab_reorder::tab_reorder_keys(ctx) {
        crate::ime::controls_without_editor(app, ctx);
        return true;
    }
    super::panel_system::gesture_keys(app, ctx)
}

/// Preferences is modeless: while one of its fields has the keyboard, no shortcut sees the keys,
/// and Escape leaves the field and keeps the draft (it does not close the window). `true`: the
/// frame's keys are taken.
pub(crate) fn preferences_field(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    if crate::dialogs::blocks_workspace(app)
        || app.ui.palette_open
        || !app.ui.dialog.as_ref().is_some_and(|d| d.kind == "preferences")
        || !ctx.egui_wants_keyboard_input()
    {
        return false;
    }
    crate::ime::controls_without_editor(app, ctx);
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
    }
    true
}
