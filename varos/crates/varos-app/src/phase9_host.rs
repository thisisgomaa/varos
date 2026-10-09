//! Lane F: desktop effects, separate from the document lifecycle.
use crate::{app_command::SessionId, phase9::DesktopAction, ui::Ui, workspace::Workspace};
pub fn desktop(action: DesktopAction, ui: &mut Ui, has_document: bool) {
    match action {
        DesktopAction::Help => open("https://github.com/thisisgomaa/varos/tree/main/docs", ui),
        DesktopAction::ReportProblem => {
            if let Some(layout) = varos_app::storage::paths::AppLayout::current() {
                if let Some(folder) = layout.crash_log().parent() {
                    match std::fs::create_dir_all(folder) {
                        Ok(()) => open(&folder.to_string_lossy(), ui),
                        Err(e) => {
                            ui.phase9.open(DesktopAction::Help);
                            ui.phase9.error = Some(format!("Couldn't prepare the crash-log folder: {e}"));
                        }
                    }
                }
            } else {
                ui.phase9.open(DesktopAction::Help);
                ui.phase9.error = Some("The application data folder is unavailable".into());
            }
        }
        DesktopAction::Actions if has_document => ui.toggle_panel(varos_app::shell::PanelId::Actions),
        _ => ui.phase9.open(action),
    }
}
fn open(target: &str, ui: &mut Ui) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(target).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer.exe").arg(target).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(target).spawn();
    if let Err(e) = result {
        ui.phase9.open(DesktopAction::Help);
        ui.phase9.error = Some(format!("Couldn't open the requested Help destination: {e}"));
    }
}
pub fn record(ui: &mut Ui, ws: &mut Workspace, id: SessionId, start: bool) {
    if let Some(s) = ws.get_mut(id) {
        if !ui.commit_fields(&mut s.editor) {
            return;
        }
        if start {
            match s.editor.start_action_recording() {
                Ok(()) => ui.phase9.recording = true,
                Err(e) => ui.phase9.error = Some(e),
            }
        } else {
            match s.editor.finish_action_recording("Recorded action".into()) {
                Ok(a) => {
                    ui.phase9.actions = Some(a);
                    ui.phase9.recording = false;
                }
                Err(e) => ui.phase9.error = Some(e),
            }
        }
    }
}
pub fn load_action(ui: &mut Ui) {
    if let Some(path) = rfd::FileDialog::new().add_filter("Varos Actions", &["vrs-actions"]).pick_file() {
        use std::io::Read;
        let result = (|| {
            let mut bytes = vec![];
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(1_048_577)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            varos_core::actions::Actions::decode(&bytes)
        })();
        match result {
            Ok(a) => {
                ui.phase9.actions = Some(a);
                ui.phase9.error = None;
            }
            Err(e) => ui.phase9.error = Some(e),
        }
    }
}
pub fn save_action(ui: &mut Ui, a: &varos_core::actions::Actions) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Varos Actions", &["vrs-actions"])
        .set_file_name("Recorded action.vrs-actions")
        .save_file()
    {
        let result = (|| {
            let bytes = serde_json::to_vec_pretty(a).map_err(|e| e.to_string())?;
            match varos_app::storage::durable::write_replace(
                &varos_app::storage::durable::RealFs,
                &path,
                &bytes,
                "actions",
            )
            .map_err(|e| e.reason())?
            {
                varos_app::storage::durable::WriteOutcome::Durable => Ok(()),
                varos_app::storage::durable::WriteOutcome::ReplacedUnconfirmed(e) => {
                    Err(format!("File replaced; durability unconfirmed: {e}"))
                }
            }
        })();
        ui.phase9.error = result.err();
    }
}
pub fn shortcut(
    ui: &Ui,
    pending: &mut crate::host::ActionQueue,
    code: winit::keyboard::KeyCode,
    m: varos_core::Mods,
    active: Option<SessionId>,
    pressed: bool,
    repeat: bool,
) -> bool {
    shortcut_route(&ui.phase9.shortcuts, ui.wants_keyboard(), pending, code, m, active, pressed, repeat)
}
#[allow(clippy::too_many_arguments)]
fn shortcut_route(
    shortcuts: &crate::shortcut_editor::EditorState,
    wants_keyboard: bool,
    pending: &mut crate::host::ActionQueue,
    code: winit::keyboard::KeyCode,
    m: varos_core::Mods,
    active: Option<SessionId>,
    pressed: bool,
    repeat: bool,
) -> bool {
    if wants_keyboard {
        return false;
    }
    let handler = shortcuts.route(code, m);
    if handler.is_none() {
        return shortcuts.suppresses_default(code, m);
    }
    if !pressed || (repeat && !repeatable(handler)) {
        return true;
    }
    if let Some(handler) = handler {
        use crate::host::{DocAction as D, HostAction as H, MenuRoute as R};
        let action = match crate::host::menu_route(handler, active) {
            Some(R::App(a)) => Some(H::App(a)),
            Some(R::Key(k)) => Some(crate::host::key_action(
                k.code,
                varos_core::Mods { ctrl: k.cmd, shift: k.shift, alt: k.alt },
                active,
            )),
            Some(R::Plain(k)) => Some(H::Doc(D::Key(k, varos_core::Mods::default()))),
            Some(R::Slice4a(name)) => Some(H::Doc(D::Slice4a(name))),
            Some(R::Snap(row)) => Some(H::Doc(D::Snap(row))),
            Some(R::Selection(s)) => Some(H::Doc(D::Selection(s))),
            Some(R::Object(s)) => Some(H::Doc(D::Object(s))),
            None => None,
        };
        if let Some(action) = action {
            pending.push(action);
        }
    }
    true
}

/// Document keys retain incumbent repeat behavior; application commands activate once.
fn repeatable(handler: Option<crate::menus::MenuCmd>) -> bool {
    use crate::host::{HostAction, MenuRoute};
    match handler.and_then(|h| crate::host::menu_route(h, Some(SessionId(1)))) {
        Some(MenuRoute::Key(k)) => matches!(
            crate::host::key_action(
                k.code,
                varos_core::Mods { ctrl: k.cmd, shift: k.shift, alt: k.alt },
                Some(SessionId(1))
            ),
            HostAction::Doc(_)
        ),
        Some(MenuRoute::Plain(_)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod fix_round_tests {
    use super::*;
    #[test]
    fn reset_apply_dispatch_passes_space_release_and_arrow_repeats_to_canvas() {
        let mut shortcuts = crate::shortcut_editor::EditorState::default();
        shortcuts.effective.reset_defaults();
        // Apply publishes decoded durable overrides, including defaults written by old Reset.
        shortcuts
            .effective
            .bindings
            .insert("shortcut.temporary-hand".into(), Some(crate::shortcut_editor::Chord::parse("Space").unwrap()));
        shortcuts.effective =
            crate::shortcut_editor::Overrides::decode(&serde_json::to_vec(&shortcuts.effective).unwrap()).unwrap();
        let mut queue = crate::host::ActionQueue::default();
        for code in [winit::keyboard::KeyCode::Space, winit::keyboard::KeyCode::ArrowLeft] {
            for (pressed, repeat) in [(true, false), (true, true), (false, false)] {
                assert!(!shortcut_route(
                    &shortcuts,
                    false,
                    &mut queue,
                    code,
                    varos_core::Mods::default(),
                    None,
                    pressed,
                    repeat
                ));
            }
        }
    }
    #[test]
    fn rebound_document_keys_repeat_but_application_commands_do_not() {
        use crate::menus::{Accel, MenuCmd};
        use winit::keyboard::KeyCode;
        let doc = MenuCmd::Key(Accel { code: KeyCode::ArrowLeft, cmd: false, shift: false, alt: false });
        assert!(repeatable(Some(doc)));
        let save = MenuCmd::Key(Accel { code: KeyCode::KeyS, cmd: true, shift: false, alt: false });
        assert!(!repeatable(Some(save)));
    }
}
