//! Avoid building/scanning command shortcuts when none can be consumed. Keep all surrounding
//! tool, clipboard, text and IME handling in the upstream dispatcher, in its original order.

use egui::{Context, Event, KeyboardShortcut};
use serde_json::Value;

/// egui 0.36's `count_and_consume_key` consumes only pressed `Event::Key` entries in `events`
/// (including repeats). With no such entry every command-chord check would return false.
/// This is deliberately NOT a shortcut cache: key frames always see current user overrides.
pub(crate) fn for_frame(ctx: &Context) -> Vec<(KeyboardShortcut, &'static str, Value)> {
    if ctx.input(|i| i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, .. }))) { crate::shortcuts::all_shortcuts() } else { Vec::new() }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use egui::{Key, Modifiers, RawInput};

    #[test]
    fn pointer_text_clipboard_and_key_releases_need_no_command_table() {
        for events in [
            vec![],
            vec![Event::PointerMoved(egui::pos2(10.0, 10.0))],
            vec![Event::Text("a".into())],
            vec![Event::Copy, Event::Cut, Event::Paste("text".into())],
            vec![Event::Key { key: Key::Z, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::COMMAND }],
        ] {
            let ctx = Context::default();
            let mut output = ctx.run_ui(RawInput { events, ..Default::default() }, |ui| {
                let before = ui.input(|i| i.events.len());
                assert!(for_frame(ui.ctx()).is_empty());
                assert_eq!(ui.input(|i| i.events.len()), before, "the gate consumes no input");
            });
            output.textures_delta.clear();
        }
    }

    #[test]
    fn presses_repeats_and_changed_overrides_keep_the_original_table() {
        let mut state = crate::state::UiState::default();
        for (repeat, chord) in [(false, "Cmd+Alt+U"), (true, "Cmd+Alt+Y")] {
            state.shortcut_overrides.insert("edit.undo".into(), chord.into());
            crate::shortcut_editor::sync(&state);
            let wanted = crate::shortcuts::parse(chord).unwrap();
            let event = Event::Key { key: wanted.logical_key, physical_key: None, pressed: true, repeat, modifiers: wanted.modifiers };
            let ctx = Context::default();
            let mut output = ctx.run_ui(RawInput { events: vec![event], ..Default::default() }, |ui| {
                let before = ui.input(|i| i.events.len());
                let rows = for_frame(ui.ctx());
                assert_eq!(rows, crate::shortcuts::all_shortcuts(), "order and params are preserved");
                assert!(rows.iter().any(|(sc, id, _)| *id == "edit.undo" && *sc == wanted));
                assert_eq!(ui.input(|i| i.events.len()), before, "the dispatcher still owns consumption");
            });
            output.textures_delta.clear();
        }
    }
}
