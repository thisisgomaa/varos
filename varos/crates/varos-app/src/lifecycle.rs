//! The document-lifecycle coordinator (DFS S1 §3.4): New / Open / Save / Save As / Close / Quit and
//! tab switching, run over the `Workspace` through two PORTS — `Dialogs` (every question the user is
//! asked) and `DocStore` (every file read/write). The host plugs in the real rfd + disk ports; tests
//! plug in scripted fakes. No rfd, fs or egui in this file.
//!
//! API frozen by S1-A; the rule bodies of `Lifecycle::run` are S1-B's.
// S1-A: nothing calls this module until S1-D wires the host; S1-D removes this allow. (Plain `allow`,
// not `cfg_attr(not(test), ..)`: test builds would otherwise fail `clippy --all-targets -D warnings`.)
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use varos_core::model::Document;

use crate::app_command::AppCommand;
use crate::workspace::{FileKey, Workspace};

/// The answer to “Save changes to “name”?”. Escape / closing the prompt = `Cancel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveDecision {
    Save,
    DontSave,
    Cancel,
}

/// The answer to “Couldn't save “name”.”.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFailChoice {
    TryAgain,
    SaveAs,
    Cancel,
}

/// Every question the lifecycle asks the user. Blocking; the host implements it with native
/// dialogs, tests with a scripted queue.
pub trait Dialogs {
    /// The Open dialog (multi-select). Empty = cancelled.
    fn pick_open(&mut self) -> Vec<PathBuf>;
    /// The Save dialog, pre-filled with `suggested` in `dir`. `None` = cancelled.
    fn pick_save(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf>;
    /// “Save changes to “name”?” — `progress` = `Some((i, n))` during Quit (“Document i of n”).
    fn ask_save_changes(&mut self, name: &str, progress: Option<(usize, usize)>) -> SaveDecision;
    /// “Couldn't save “name”.” with the reason.
    fn save_failed(&mut self, name: &str, reason: &str) -> SaveFailChoice;
    /// “Couldn't open “name”.” with the reason.
    fn open_failed(&mut self, name: &str, reason: &str);
    /// “Replace “name”?” — asked only when WE changed the chosen name (appended `.vrs`) and that
    /// file exists; the Save dialog already asked about the name the user typed.
    fn confirm_replace(&mut self, name: &str) -> bool;
    /// A plain notice (e.g. “… is open in another tab…”).
    fn notice(&mut self, title: &str, body: &str);
}

/// Every file operation the lifecycle performs.
pub trait DocStore {
    fn load(&mut self, path: &Path) -> Result<Document, String>;
    fn save(&mut self, doc: &Document, path: &Path) -> Result<(), String>;
    /// The file's identity: absolute + canonical path (the parent canonicalised for a file that does
    /// not exist yet), plus device/inode on unix.
    fn key(&self, path: &Path) -> FileKey;
    fn exists(&self, path: &Path) -> bool;
}

/// What the host must do after a command.
#[derive(Debug, Default, PartialEq)]
pub struct Effect {
    /// Every tab was resolved by a Quit — save the window state and exit.
    pub exit: bool,
}

/// One lifecycle command run over the workspace and the ports.
pub struct Lifecycle<'a> {
    pub ws: &'a mut Workspace,
    pub dialogs: &'a mut dyn Dialogs,
    pub store: &'a mut dyn DocStore,
}

impl Lifecycle<'_> {
    /// Run one command. `AppCommand::Window(_)` is ignored here (host-owned).
    pub fn run(&mut self, cmd: AppCommand) -> Effect {
        // S1-B replaces: the stub quits only when nothing is unsaved and does nothing else.
        match cmd {
            AppCommand::Quit => Effect { exit: !self.ws.sessions().iter().any(|s| s.is_dirty_exact()) },
            _ => Effect::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_command::{OpenOrigin, WindowCmd};
    use varos_core::EditCommand;

    /// Answers nothing — the stub never asks.
    struct NoDialogs;
    impl Dialogs for NoDialogs {
        fn pick_open(&mut self) -> Vec<PathBuf> {
            unreachable!("the stub asks nothing")
        }
        fn pick_save(&mut self, _: &str, _: Option<&Path>) -> Option<PathBuf> {
            unreachable!("the stub asks nothing")
        }
        fn ask_save_changes(&mut self, _: &str, _: Option<(usize, usize)>) -> SaveDecision {
            unreachable!("the stub asks nothing")
        }
        fn save_failed(&mut self, _: &str, _: &str) -> SaveFailChoice {
            unreachable!("the stub asks nothing")
        }
        fn open_failed(&mut self, _: &str, _: &str) {
            unreachable!("the stub asks nothing")
        }
        fn confirm_replace(&mut self, _: &str) -> bool {
            unreachable!("the stub asks nothing")
        }
        fn notice(&mut self, _: &str, _: &str) {
            unreachable!("the stub asks nothing")
        }
    }
    struct NoStore;
    impl DocStore for NoStore {
        fn load(&mut self, _: &Path) -> Result<Document, String> {
            unreachable!("the stub touches no file")
        }
        fn save(&mut self, _: &Document, _: &Path) -> Result<(), String> {
            unreachable!("the stub touches no file")
        }
        fn key(&self, p: &Path) -> FileKey {
            FileKey { path: p.to_path_buf(), dev_ino: None }
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
    }

    #[test]
    fn stub_quits_only_when_nothing_is_unsaved() {
        let mut ws = Workspace::new();
        let (mut d, mut s) = (NoDialogs, NoStore);
        let mut lc = Lifecycle { ws: &mut ws, dialogs: &mut d, store: &mut s };
        assert_eq!(lc.run(AppCommand::Quit), Effect { exit: true });
        assert_eq!(lc.run(AppCommand::Window(WindowCmd::Minimize)), Effect::default());
        assert_eq!(lc.run(AppCommand::OpenPaths(vec![], OpenOrigin::Dialog)), Effect::default());
        lc.ws.active_mut().expect("S1 always has an active tab").editor.execute(EditCommand::AddArtboard);
        assert_eq!(lc.run(AppCommand::Quit), Effect { exit: false }, "a dirty tab blocks the stub quit");
        assert_eq!(lc.ws.sessions().len(), 1);
    }
}
