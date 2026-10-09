//! Lane G: provisional kit sheets; owner design review pending.
use crate::app_command::AppCommand;
use std::{io::Read, sync::mpsc};
use varos_app::{
    shell::{
        kit::{self, Control},
        tokens as t,
    },
    storage::paths::AppLayout,
};
use varos_bridge::release::{self, Action, Sheet};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopAction {
    Check,
    Download,
    CrashLogs,
    CrashReport,
    Close,
}
impl DesktopAction {
    #[cfg(test)]
    pub fn action(self) -> Action {
        match self {
            Self::Check => Action::Check {},
            Self::Download => Action::Download {},
            Self::CrashLogs => Action::CrashLogs {},
            Self::CrashReport => Action::CrashReport {},
            Self::Close => Action::Close {},
        }
    }
    pub fn from_action(action: &Action) -> Option<Self> {
        Some(match action {
            Action::Check {} => Self::Check,
            Action::Download {} => Self::Download,
            Action::CrashLogs {} => Self::CrashLogs,
            Action::CrashReport {} => Self::CrashReport,
            Action::Close {} => Self::Close,
            _ => return None,
        })
    }
}
#[derive(Default)]
pub struct State {
    pub sheet: Option<Sheet>,
    pending: Option<mpsc::Receiver<Result<Sheet, String>>>,
}
fn limited(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("File exceeds display limit".into());
    }
    Ok(bytes)
}
/// Additive update settings in settings.json: endpoint and independently trusted Ed25519 public key.
fn configuration(layout: &AppLayout) -> Result<(String, String), String> {
    let bytes = limited(&layout.settings(), varos_app::storage::settings_codec::MAX_BYTES)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let url = value["update_manifest_url"].as_str().ok_or("No update manifest URL configured in settings")?.to_owned();
    release::https(&url)?;
    let key =
        value["update_public_key"].as_str().ok_or("No trusted update signing key configured in settings")?.to_owned();
    Ok((url, key))
}
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    release::https(url)?;
    // No redirect following: a changed origin must be configured explicitly. Bounded time and size.
    let mut child = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--max-filesize",
            "262144",
            "--url",
            url,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Couldn't start HTTPS transport: {e}"))?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or("HTTPS transport has no output")?
        .take(release::MAX_MANIFEST as u64 + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > release::MAX_MANIFEST {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Manifest exceeds size limit or transport failed".into());
    }
    if !child.wait().map_err(|e| e.to_string())?.success() {
        return Err("Couldn't fetch the signed update manifest".into());
    }
    Ok(bytes)
}
fn open(target: &std::ffi::OsStr) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = std::process::Command::new("xdg-open");
    command.arg(target).spawn().map(|_| ()).map_err(|e| format!("Couldn't open destination: {e}"))
}
impl State {
    pub fn handle(&mut self, action: DesktopAction, ctx: &egui::Context) {
        let result = (|| -> Result<(), String> {
            match action {
                DesktopAction::Close => self.sheet = None,
                DesktopAction::Download => {
                    let Some(Sheet::Available(manifest)) = &self.sheet else {
                        return Err("Check for an available signed update first".into());
                    };
                    release::https(&manifest.download_url)?;
                    open(std::ffi::OsStr::new(&manifest.download_url))?;
                }
                DesktopAction::Check => {
                    if self.pending.is_some() {
                        return Ok(());
                    }
                    let layout = AppLayout::current().ok_or("Application data folder unavailable")?;
                    let (url, key) = configuration(&layout)?;
                    let (send, receive) = mpsc::channel();
                    self.pending = Some(receive);
                    self.sheet = Some(Sheet::Checking);
                    let wake = ctx.clone();
                    std::thread::Builder::new()
                        .name("varos-update-check".into())
                        .spawn(move || {
                            let result =
                                fetch(&url).and_then(|bytes| release::verify(&bytes, &key, env!("CARGO_PKG_VERSION")));
                            let _ = send.send(result);
                            wake.request_repaint();
                        })
                        .map_err(|e| {
                            self.pending = None;
                            e.to_string()
                        })?;
                }
                DesktopAction::CrashLogs => {
                    let layout = AppLayout::current().ok_or("Application data folder unavailable")?;
                    let folder = layout.root.join("Logs");
                    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                    open(folder.as_os_str())?;
                }
                DesktopAction::CrashReport => {
                    let layout = AppLayout::current().ok_or("Application data folder unavailable")?;
                    let text = match limited(&layout.crash_log(), 65_536) {
                        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                        Err(e) if !layout.crash_log().exists() => format!("No crash report saved. {e}"),
                        Err(e) => return Err(e),
                    };
                    self.sheet = Some(Sheet::Crash(text));
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.sheet = Some(Sheet::Failed(e));
        }
        ctx.request_repaint();
    }
    pub fn draw(&mut self, ctx: &egui::Context, commands: &mut Vec<AppCommand>) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    if matches!(self.sheet, Some(Sheet::Checking)) {
                        self.sheet = Some(result.unwrap_or_else(Sheet::Failed));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.sheet = Some(Sheet::Failed("Update worker stopped".into()));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let Some(sheet) = &self.sheet else { return };
        let title = if matches!(sheet, Sheet::Crash(_)) { "Crash Report" } else { "Check for Updates" };
        egui::Area::new(egui::Id::new("lane-g-sheet"))
            .order(egui::Order::Foreground)
            .fixed_pos(ctx.content_rect().center() - egui::vec2(t::DOC_SHEET_W / 2., t::DOC_SHEET_TOP))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t::PANEL)
                    .stroke(egui::Stroke::new(t::KIT_STROKE, t::LINE2))
                    .corner_radius(t::r_box())
                    .inner_margin(egui::Margin::same(t::KIT_PAD as i8))
                    .show(ui, |ui| {
                        ui.set_width(t::DOC_SHEET_W);
                        kit::section_heading(ui, title);
                        egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.5).show(ui, |ui| {
                            match sheet {
                                Sheet::Checking => {
                                    kit::notice(ui, "Fetching and verifying the signed manifest…");
                                }
                                Sheet::Failed(e) => {
                                    kit::notice(ui, e);
                                }
                                Sheet::Crash(text) => {
                                    kit::notice(ui, text);
                                }
                                Sheet::Current(m) | Sheet::Available(m) => {
                                    kit::notice(
                                        ui,
                                        &format!("{} · installed {}", m.version, env!("CARGO_PKG_VERSION")),
                                    );
                                    if matches!(sheet, Sheet::Current(_)) {
                                        kit::notice(ui, "No newer release available.");
                                    }
                                    kit::notice(ui, &m.notes);
                                }
                            }
                        });
                        if matches!(sheet, Sheet::Available(_))
                            && kit::action(ui, Control::new(ui.id().with("download"), "Download"), false).activated
                        {
                            commands.push(AppCommand::Release(DesktopAction::Download));
                        }
                        if matches!(sheet, Sheet::Crash(_))
                            && kit::action(ui, Control::new(ui.id().with("folder"), "Show Crash Logs"), false).activated
                        {
                            commands.push(AppCommand::Release(DesktopAction::CrashLogs));
                        }
                        if kit::action(ui, Control::new(ui.id().with("close"), "Close"), false).activated
                            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            commands.push(AppCommand::Release(DesktopAction::Close));
                        }
                    });
            });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_requires_verified_available_sheet() {
        let mut state = State::default();
        state.handle(DesktopAction::Download, &egui::Context::default());
        assert!(matches!(state.sheet, Some(Sheet::Failed(_))));
        state.handle(DesktopAction::Close, &egui::Context::default());
        assert!(state.sheet.is_none());
    }
    #[test]
    fn all_desktop_actions_round_trip() {
        for a in [
            DesktopAction::Check,
            DesktopAction::Download,
            DesktopAction::CrashLogs,
            DesktopAction::CrashReport,
            DesktopAction::Close,
        ] {
            assert_eq!(DesktopAction::from_action(&a.action()), Some(a));
        }
    }
    #[test]
    fn sheet_controls_are_scoped_to_verified_results() {
        let manifest = release::Manifest {
            version: "1.0.0".into(),
            notes: "Changes".into(),
            download_url: "https://example.org/download".into(),
        };
        for sheet in [
            Sheet::Checking,
            Sheet::Current(manifest.clone()),
            Sheet::Available(manifest),
            Sheet::Failed("Offline".into()),
            Sheet::Crash("Report".into()),
        ] {
            let available = matches!(sheet, Sheet::Available(_));
            let crash = matches!(sheet, Sheet::Crash(_));
            let mut state = State { sheet: Some(sheet), pending: None };
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            varos_app::shell::fonts::install(&ctx);
            let mut commands = vec![];
            let output = ctx.run_ui(egui::RawInput::default(), |_| state.draw(&ctx, &mut commands));
            let names = output
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .into_iter()
                .filter_map(|(_, n)| n.label().map(str::to_owned))
                .collect::<Vec<_>>();
            assert_eq!(names.iter().any(|n| n == "Download"), available);
            assert_eq!(names.iter().any(|n| n == "Show Crash Logs"), crash);
            assert!(commands.is_empty(), "paint performs no effect");
        }
    }
    #[test]
    fn menus_dispatch_without_a_document() {
        let menus = crate::menus::flat_items(&crate::menus::menus());
        for id in ["app.updates", "help.crash_logs", "help.crash_report"] {
            let cmd = menus
                .iter()
                .find_map(|e| match e {
                    crate::menus::Entry::Item { id: found, cmd, .. } if found == id => Some(*cmd),
                    _ => None,
                })
                .unwrap();
            assert!(matches!(
                crate::host::menu_route(cmd, None),
                Some(crate::host::MenuRoute::App(AppCommand::Release(_)))
            ));
        }
    }
    #[test]
    fn crash_read_is_bounded_and_preserves_bytes() {
        let path = std::env::temp_dir().join(format!("varos-lane-g-crash-{}", std::process::id()));
        std::fs::write(&path, b"crash report").unwrap();
        assert_eq!(limited(&path, 32).unwrap(), b"crash report");
        assert!(limited(&path, 2).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
