//! Lane F: desktop effects, separate from the document lifecycle.
use crate::{app_command::SessionId, phase9::DesktopAction, ui::Ui, workspace::Workspace};
pub fn desktop(action: DesktopAction, ui: &mut Ui) {
    match action {
        DesktopAction::Help => open("https://github.com/thisisgomaa/varos/tree/main/docs", ui),
        DesktopAction::ReportProblem => {
            if let Some(layout) = varos_app::storage::paths::AppLayout::current() {
                if let Some(folder) = layout.crash_log().parent() {
                    open(&folder.to_string_lossy(), ui);
                }
            }
        }
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
        ui.phase9.error = Some(e.to_string());
    }
}
pub fn record(ui: &mut Ui, ws: &mut Workspace, id: SessionId, start: bool) {
    if let Some(s) = ws.get_mut(id) {
        if !ui.commit_fields(&mut s.editor) {
            return;
        }
        if start {
            if let Err(e) = s.editor.start_action_recording() {
                ui.phase9.error = Some(e);
            }
        } else {
            match s.editor.finish_action_recording("Recorded action".into()) {
                Ok(a) => ui.phase9.actions = Some(a),
                Err(e) => ui.phase9.error = Some(e),
            }
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
    if ui.wants_keyboard() {
        return false;
    }
    let handler = ui.phase9.shortcuts.route(code, m);
    if handler.is_none() {
        return ui.phase9.shortcuts.suppresses_default(code, m);
    }
    if !pressed || repeat {
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
