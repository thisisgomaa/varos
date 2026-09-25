//! DFS S1 §3.5 — the host's command path, as pure pieces the event loop in `main.rs` calls.
//!
//! Every lifecycle / window command becomes an [`AppCommand`] here: shortcut keys ([`lifecycle_key`]
//! → [`to_app_command`], [`tab_key`]), native File / Window rows ([`menu_route`]), the custom
//! caption's window controls ([`win_action_command`]), the OS close request (`AppCommand::Quit`), the
//! tab strip / burger (`Ui::take_app_commands`), and files handed in from outside
//! ([`open_paths_command`]). They wait in ONE FIFO queue of [`HostAction`]s that `main.rs` drains at
//! `AboutToWait` through its `dispatch`: `Window(_)` effects on the window, everything else through
//! [`run_lifecycle`]. The document actions keys and menu rows raise ([`DocAction`]: a shortcut key,
//! Edit ▸ Delete, a snap row) share that order but not always that path: one runs at once when
//! nothing is queued — no command, no pointer button whose chrome command is still to come
//! ([`ActionQueue::doc_runs_now`]) — else it queues behind and goes through `dispatch` too.
//! Pointer input and panel edits act on the editor directly, outside the queue; a pointer button only
//! marks the queue, so what is raised after a click waits for the click's own chrome command.
//!
//! No window, no GPU, no dialogs here: everything is testable headless (the dialogs and the disk are
//! the lifecycle's ports).

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use varos_core::editor::{Editor, Mods};
use winit::keyboard::KeyCode;

use crate::app_command::{AppCommand, OpenOrigin, SessionId, WindowCmd};
use crate::chrome::{Accel, FileCmd, MenuCmd};
use crate::lifecycle::{Dialogs, DocStore, Lifecycle};
use crate::ui::WinAction;
use crate::workspace::Workspace;

/// The File meaning of a shortcut key, if any: ⌘N New · ⌘O Open… · ⌘S Save · ⇧⌘S Save As… ·
/// ⌘W Close Tab · ⌘Q Quit — the same `FileCmd`s the native File rows carry. `ctrl` is Ctrl or ⌘ (as
/// `Editor::mods`). Without it the letters stay tool keys (S = Scale…), and ⌥ never means a file
/// command. Pure.
pub fn lifecycle_key(code: KeyCode, ctrl: bool, shift: bool, alt: bool) -> Option<FileCmd> {
    if !ctrl || alt {
        return None;
    }
    Some(match (code, shift) {
        (KeyCode::KeyN, false) => FileCmd::New,
        (KeyCode::KeyO, false) => FileCmd::Open,
        (KeyCode::KeyS, false) => FileCmd::Save,
        (KeyCode::KeyS, true) => FileCmd::SaveAs,
        (KeyCode::KeyW, false) => FileCmd::CloseTab,
        (KeyCode::KeyQ, false) => FileCmd::Quit,
        _ => return None,
    })
}

/// Ctrl+Tab / Ctrl+⇧Tab: the next / previous tab (wrapping). Keyboard-only in S1 — there is no menu
/// row, so it is not a `FileCmd`. Pure.
pub fn tab_key(code: KeyCode, ctrl: bool, shift: bool, alt: bool) -> Option<AppCommand> {
    (ctrl && !alt && code == KeyCode::Tab).then_some(if shift {
        AppCommand::ActivatePrevious
    } else {
        AppCommand::ActivateNext
    })
}

/// The ONE mapper from a [`FileCmd`] (a native File row, a lifecycle key) to its [`AppCommand`].
/// Save / Save As / Close Tab act on the active tab; with no active tab (S2's empty workspace) they
/// mean nothing (`None`).
pub fn to_app_command(cmd: FileCmd, active: Option<SessionId>) -> Option<AppCommand> {
    Some(match cmd {
        FileCmd::New => AppCommand::NewDocument,
        FileCmd::Open => AppCommand::OpenDialog,
        FileCmd::Save => AppCommand::Save(active?),
        FileCmd::SaveAs => AppCommand::SaveAs(active?),
        FileCmd::CloseTab => AppCommand::CloseDocument(active?),
        FileCmd::Quit => AppCommand::Quit,
    })
}

/// A shortcut key's lifecycle command, if it has one: [`lifecycle_key`] then [`to_app_command`], or
/// [`tab_key`].
pub fn key_command(code: KeyCode, m: Mods, active: Option<SessionId>) -> Option<AppCommand> {
    match lifecycle_key(code, m.ctrl, m.shift, m.alt) {
        Some(f) => to_app_command(f, active),
        None => tab_key(code, m.ctrl, m.shift, m.alt),
    }
}

/// The keyboard as the window holds it — the host's ONE truth for the held keys, never a document
/// tab's: the modifiers (Ctrl or ⌘, ⇧, ⌥) as winit's `ModifiersChanged` last reported them, Space (the
/// pan / A9 reposition key), and which keys went down AS A COMMAND. winit reports a modifier only when
/// it CHANGES, so a tab switch while Control is still held must not forget the Control (owner
/// hand-test 2026-09-25: held-Ctrl Tab · Tab cycled only once, because the key path read the incoming
/// tab's reset copy). A tab's `Editor::mods` / `Editor::space` are only mirrors for the gestures that
/// read them ([`Self::mirror`]): rewritten on every change, by [`run_lifecycle`] after every command,
/// and cleared with everything else when the window loses focus ([`Self::focus_lost`]).
#[derive(Clone, Default)]
pub struct Keyboard {
    held: Mods,
    space: bool,
    /// The keys whose press was a command (Ctrl+Tab, ⌘S …), until their release: every later event of
    /// such a key goes where its press went, whatever the modifiers did in between (Codex review P2).
    commands_down: Vec<KeyCode>,
}

/// Where one key event goes ([`Keyboard::key`]).
#[derive(Debug, PartialEq)]
pub enum KeyRoute {
    /// A command key's fresh press: queue this command. The key goes nowhere else.
    Queue(AppCommand),
    /// The rest of a command key — its repeats, its release: nowhere (not egui, not the document).
    Swallow,
    /// Not a command key, from press to release: egui / the document, as usual.
    Pass,
}

impl Keyboard {
    /// winit's `ModifiersChanged`: the new truth, mirrored into the active tab's editor.
    pub fn modifiers_changed(&mut self, m: Mods, active: Option<&mut Editor>) {
        self.held = m;
        self.mirror_into(active);
    }

    /// Space went down (`true`) or up, as the canvas's key: mirrored into the active tab's editor.
    pub fn space_changed(&mut self, down: bool, active: Option<&mut Editor>) {
        self.space = down;
        self.mirror_into(active);
    }

    /// The window lost key focus: the key-ups now go to whatever took it. winit releases the modifiers
    /// itself (a synthetic `ModifiersChanged`); Space and the command keys are let go here. The active
    /// tab's mirror follows.
    pub fn focus_lost(&mut self, active: Option<&mut Editor>) {
        *self = Keyboard::default();
        self.mirror_into(active);
    }

    /// The modifiers held right now.
    pub fn held(&self) -> Mods {
        self.held
    }

    /// Space is held right now.
    pub fn space(&self) -> bool {
        self.space
    }

    /// Copy the held keys into a tab's editor (the one it shows now).
    pub fn mirror(&self, ed: &mut Editor) {
        ed.mods = self.held;
        ed.space = self.space;
    }

    fn mirror_into(&self, active: Option<&mut Editor>) {
        if let Some(ed) = active {
            self.mirror(ed);
        }
    }

    /// The file / tab command a key is with the held modifiers, if any ([`key_command`]).
    pub fn command(&self, code: KeyCode, active: Option<SessionId>) -> Option<AppCommand> {
        key_command(code, self.held, active)
    }

    /// Route one key event. A FRESH press is classified on the held modifiers ([`Self::command`]);
    /// its repeats and its release then go where the press went, even when a modifier went up or down
    /// in between — so Ctrl+Tab's key-up never leaks to egui, and a plain Tab's key-up always reaches
    /// it (egui's held-key state stays true).
    pub fn key(&mut self, code: KeyCode, pressed: bool, repeat: bool, active: Option<SessionId>) -> KeyRoute {
        let was_command = self.commands_down.contains(&code);
        if !pressed || repeat {
            if !pressed {
                self.commands_down.retain(|&c| c != code);
            }
            return if was_command { KeyRoute::Swallow } else { KeyRoute::Pass };
        }
        match self.command(code, active) {
            Some(cmd) => {
                if !was_command {
                    self.commands_down.push(code);
                }
                KeyRoute::Queue(cmd)
            }
            None => {
                self.commands_down.retain(|&c| c != code);
                KeyRoute::Pass
            }
        }
    }
}

/// One entry of the host's action queue (DFS S1 review P1): a lifecycle / window command, or a
/// document action raised by a key or a menu row. Keys, native menu rows, the burger and the tab
/// strip all feed ONE FIFO queue, so a ⌘Z pressed after a ⌘S in the same event batch runs after the
/// Save, never before it.
#[derive(Clone)]
pub enum HostAction {
    /// A command for `main.rs`'s `dispatch` (always waits for the queue's drain at `AboutToWait`).
    App(AppCommand),
    /// A document action on whichever tab is active when it runs.
    Doc(DocAction),
}

/// A document action a key or a menu row raises (not a lifecycle command).
#[derive(Clone, Copy)]
pub enum DocAction {
    /// A document shortcut key (`main.rs`'s `doc_key`) with the modifiers held when it was pressed.
    Key(KeyCode, Mods),
    /// A magnet quick-menu row (grid = Snap to Grid, else Snap to Point).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // raised only by the macOS menu bar
    Snap { grid: bool },
}

/// A shortcut key meant for the document (not typed into a field): its lifecycle command, else the
/// document shortcut itself. Pure.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // the menu bar's ⌘-rows; the keyboard splits earlier
pub fn key_action(code: KeyCode, m: Mods, active: Option<SessionId>) -> HostAction {
    match key_command(code, m, active) {
        Some(c) => HostAction::App(c),
        None => HostAction::Doc(DocAction::Key(code, m)),
    }
}

/// The host's ONE FIFO action queue (DFS S1 review P1). Commands wait here for the drain at
/// `AboutToWait`; a document action runs at once only when nothing is waiting ([`Self::doc_runs_now`]).
///
/// A pointer button fed to egui (a tab chip, a burger row) becomes a command only at the next Ui
/// frame, so every press and every release leaves a MARK in the queue, in event order. The Ui frame
/// inserts each command at the mark of the event that produced it — for a click, its RELEASE
/// ([`Self::chrome_frame`]) — and removes every mark. So click tab B then ⌘Z undoes on B, while
/// press B, ⌘Z, release B undoes before B is activated.
#[derive(Default)]
pub struct ActionQueue {
    slots: Vec<Slot>,
    next_mark: u64,
}

/// One place in the queue: an action, or the mark a pointer event left.
enum Slot {
    Action(HostAction),
    Mark { id: u64, release: bool },
}

impl ActionQueue {
    pub fn push(&mut self, a: HostAction) {
        self.slots.push(Slot::Action(a));
    }

    pub fn extend(&mut self, it: impl IntoIterator<Item = HostAction>) {
        self.slots.extend(it.into_iter().map(Slot::Action));
    }

    /// A pointer button was pressed (`release` false) or released: leave its mark. Returns its id.
    pub fn pointer_button(&mut self, release: bool) -> u64 {
        let id = self.next_mark;
        self.next_mark += 1;
        self.slots.push(Slot::Mark { id, release });
        id
    }

    /// The latest release mark still waiting: where the click egui reports at the next Ui frame came
    /// from (egui reports at most one click per frame, decided at a release).
    pub fn last_release(&self) -> Option<u64> {
        self.slots.iter().rev().find_map(|s| match *s {
            Slot::Mark { id, release: true } => Some(id),
            _ => None,
        })
    }

    /// The Ui frame ran: each command goes in at the mark of the pointer event that produced it (in
    /// the given order; at the tail when it has none, or its mark is gone), then every mark is removed.
    pub fn chrome_frame(&mut self, produced: impl IntoIterator<Item = (Option<u64>, HostAction)>) {
        for (mark, a) in produced {
            let at = mark
                .and_then(|m| self.slots.iter().position(|s| matches!(*s, Slot::Mark { id, .. } if id == m)))
                .unwrap_or(self.slots.len());
            self.slots.insert(at, Slot::Action(a)); // before the mark: several keep their order
        }
        self.slots.retain(|s| matches!(s, Slot::Action(_)));
    }

    /// Nothing is waiting: no queued action and no pointer mark.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// May a freshly raised document action run at once? Only when nothing raised earlier is still
    /// waiting — no queued action, no pointer mark ([`Self::is_empty`]); running it then IS running it
    /// in event order, and a shortcut with nothing ahead of it answers in the same frame as before.
    pub fn doc_runs_now(&self) -> bool {
        self.is_empty()
    }

    /// The drain: every action ahead of the first mark, in order. What is behind a mark waits for
    /// the Ui frame that turns the pointer events into their commands.
    pub fn take_ready(&mut self) -> Vec<HostAction> {
        let n = self.slots.iter().position(|s| matches!(s, Slot::Mark { .. })).unwrap_or(self.slots.len());
        self.slots
            .drain(..n)
            .map(|s| match s {
                Slot::Action(a) => a,
                Slot::Mark { .. } => unreachable!("the drain stops at the first mark"),
            })
            .collect()
    }
}

/// The custom caption's window controls. The ✕ is the Quit transaction (S1 has one window, so
/// Close Window = Quit — work order §6 Q1).
pub fn win_action_command(a: WinAction) -> AppCommand {
    match a {
        WinAction::Minimize => AppCommand::Window(WindowCmd::Minimize),
        WinAction::ToggleMaximize => AppCommand::Window(WindowCmd::ToggleMaximize),
        WinAction::Close => AppCommand::Quit,
    }
}

/// Files that came from outside the app: the startup argument (`CommandLine`, the first instance's
/// own file) or a second instance's hand-off (`OsHandoff`). Nothing to open → no command.
pub fn open_paths_command(paths: Vec<PathBuf>, origin: OpenOrigin) -> Option<AppCommand> {
    if paths.is_empty() {
        None
    } else {
        Some(AppCommand::OpenPaths(paths, origin))
    }
}

/// Where a native menu row goes.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // the native menu bar is macOS-only
#[derive(Clone, Debug, PartialEq)]
pub enum MenuRoute {
    /// A command for the one dispatch (File rows, Quit, the Window rows).
    App(AppCommand),
    /// A ⌘-row: the keyboard's own shortcut path (handed to a focused text field instead).
    Key(Accel),
    /// A click-only plain-key row (Edit ▸ Delete): the key path, never while a text field is focused.
    Plain(KeyCode),
    /// A magnet quick-menu row (grid = Snap to Grid, else Snap to Point).
    Snap { grid: bool },
}

/// Route one native menu row. `None` = nothing to do (a document row with no document).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // the native menu bar is macOS-only
pub fn menu_route(cmd: MenuCmd, active: Option<SessionId>) -> Option<MenuRoute> {
    Some(match cmd {
        MenuCmd::File(f) => MenuRoute::App(to_app_command(f, active)?),
        MenuCmd::Key(k) => MenuRoute::Key(k),
        MenuCmd::Plain(code) => MenuRoute::Plain(code),
        MenuCmd::ToggleRail => MenuRoute::App(AppCommand::Window(WindowCmd::ToggleRail)),
        MenuCmd::ToggleDock => MenuRoute::App(AppCommand::Window(WindowCmd::ToggleDock)),
        MenuCmd::TogglePanel(p) => MenuRoute::App(AppCommand::Window(WindowCmd::TogglePanel(p))),
        MenuCmd::SnapGrid => MenuRoute::Snap { grid: true },
        MenuCmd::SnapPoint => MenuRoute::Snap { grid: false },
    })
}

/// The window title: the active document's name, `*` when it has unsaved changes (spec naming; the
/// tool name is no longer in it — work order §6.7).
pub fn window_title(name: &str, dirty: bool) -> String {
    format!("{name}{} — Varos", if dirty { "*" } else { "" })
}

/// The scene-cache key: the scene signature mixed with WHICH document it is. Two tabs with the same
/// `rev`, view and selection have equal `scene_signature`s (it hashes only the live paths' content),
/// so without the id a tab switch could redraw the other tab's cached art (work order F12).
pub fn scene_key(id: SessionId, signature: u64) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (id, signature).hash(&mut h);
    h.finish()
}

/// Which way a left-button release goes (UI audit 04, A2/B1: the surface that got the press gets
/// its release — a release whose press went to chrome must never end, i.e. commit, an Editor
/// transaction such as the colour picker's open session).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftRelease {
    /// A Space / middle-button pan ends.
    EndPan,
    /// The press went to the chrome (egui): so does its release. The Editor is not told.
    Chrome,
    /// The press started on the canvas: the gesture ends (over a panel a dragged guide is deleted).
    Canvas { over_panel: bool },
}

/// Route a left-button release. `pressed_on_canvas` = the press was a canvas press (it began a
/// canvas gesture that has not been cut off, e.g. by a tab switch). Pure.
pub fn route_left_release(pressed_on_canvas: bool, panning: bool, over_panel: bool) -> LeftRelease {
    if panning {
        LeftRelease::EndPan
    } else if !pressed_on_canvas {
        LeftRelease::Chrome
    } else {
        LeftRelease::Canvas { over_panel }
    }
}

/// The Ui side of a lifecycle command (the real `ui::Ui`; a recorder in tests).
pub trait DocUi {
    /// Close every Ui-side edit still open on the outgoing document (colour picker, rename buffers).
    fn settle(&mut self, ed: &mut Editor);
    /// Drop the Ui's per-document caches (layer rows, drag, collapse, search).
    fn document_switched(&mut self);
}

impl DocUi for crate::ui::Ui {
    fn settle(&mut self, ed: &mut Editor) {
        crate::ui::Ui::settle(self, ed)
    }
    fn document_switched(&mut self) {
        crate::ui::Ui::document_switched(self)
    }
}

/// What one lifecycle command leaves for the event loop to do.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Ran {
    /// Every tab was resolved by a Quit: save the window state and exit.
    pub exit: bool,
    /// The lifecycle ran: the per-document caches (scene cache) must be rebuilt.
    pub ran: bool,
    /// The active tab changed: an in-flight canvas gesture / pan belongs to the old one.
    pub switched: bool,
}

/// Run ONE lifecycle command (any `AppCommand` but `Window`) the way work order §3.5 says:
/// 1. finish every open edit on the active tab — the Ui side first (an open colour picker is
///    cancelled, not committed), then the pointer gesture (`DocumentSession::settle`);
/// 2. run the lifecycle rules over the workspace and the ports;
/// 3. mirror the keyboard's held keys ([`Keyboard::mirror`]: modifiers + Space) into the (maybe
///    new) active editor — never a blanket reset: Control still held after a Ctrl+Tab must still
///    read as held, or the next Tab is a plain Tab; Space still held must still reposition (A9). A
///    key-up a native dialog swallowed is not lost either: the window lost focus, and
///    [`Keyboard::focus_lost`] / winit's `ModifiersChanged` rewrite the mirror. Then ALWAYS reset
///    the Ui's per-document caches (an Open can replace a pristine tab in place).
///
/// A click on the chip that is already active is not a lifecycle change: nothing is settled or reset.
pub fn run_lifecycle(
    cmd: AppCommand,
    ws: &mut Workspace,
    ui: &mut dyn DocUi,
    dialogs: &mut dyn Dialogs,
    store: &mut dyn DocStore,
    keys: &Keyboard,
) -> Ran {
    debug_assert!(!matches!(cmd, AppCommand::Window(_)), "window commands are the host's");
    if matches!(cmd, AppCommand::ActivateDocument(id) if ws.active_id() == Some(id)) {
        return Ran::default();
    }
    if let Some(s) = ws.active_mut() {
        ui.settle(&mut s.editor);
        s.settle();
    }
    let before = ws.active_id();
    let effect = Lifecycle { ws: &mut *ws, dialogs, store }.run(cmd);
    if let Some(s) = ws.active_mut() {
        keys.mirror(&mut s.editor);
    }
    ui.document_switched();
    Ran { exit: effect.exit, ran: true, switched: ws.active_id() != before }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::{flat_items, menus, Entry};
    use crate::lifecycle::{SaveDecision, SaveFailChoice};
    use std::path::Path;
    use varos_core::editor::{Drag, PaintTarget, ToolKind};
    use varos_core::geom::View;
    use varos_core::model::{Anchor, Document, Path as VPath};
    use varos_core::scene::scene_signature;
    use varos_core::EditCommand;

    const ID: SessionId = SessionId(7);

    fn m(ctrl: bool, shift: bool, alt: bool) -> Mods {
        Mods { ctrl, shift, alt }
    }

    #[test]
    fn lifecycle_key_maps_file_and_tab_keys() {
        use KeyCode as K;
        for (code, shift, want) in [
            (K::KeyN, false, FileCmd::New),
            (K::KeyO, false, FileCmd::Open),
            (K::KeyS, false, FileCmd::Save),
            (K::KeyS, true, FileCmd::SaveAs),
            (K::KeyW, false, FileCmd::CloseTab),
            (K::KeyQ, false, FileCmd::Quit),
        ] {
            assert_eq!(lifecycle_key(code, true, shift, false), Some(want), "{code:?} shift={shift}");
            assert_eq!(lifecycle_key(code, true, shift, true), None, "⌥ never makes {code:?} a file command");
        }
        // Ctrl+Tab / Ctrl+⇧Tab switch tabs (keyboard-only, so not a FileCmd)
        assert_eq!(tab_key(K::Tab, true, false, false), Some(AppCommand::ActivateNext));
        assert_eq!(tab_key(K::Tab, true, true, false), Some(AppCommand::ActivatePrevious));
        assert_eq!(tab_key(K::Tab, false, false, false), None, "plain Tab is not a tab switch");
        assert_eq!(tab_key(K::Tab, true, false, true), None);
        assert_eq!(tab_key(K::KeyN, true, false, false), None);
        assert_eq!(key_command(K::Tab, m(true, true, false), Some(ID)), Some(AppCommand::ActivatePrevious));
        // plain N / O / S / W (and Tab) stay tool / UI keys: S = Scale, ⇧O = Artboard …
        for code in [K::KeyN, K::KeyO, K::KeyS, K::KeyW, K::KeyQ, K::Tab] {
            for shift in [false, true] {
                assert_eq!(lifecycle_key(code, false, shift, false), None, "{code:?} without ⌘");
            }
        }
        // the document shortcuts are not lifecycle keys
        for code in [K::KeyZ, K::KeyC, K::KeyV, K::KeyX, K::KeyA, K::Digit0, K::Digit1, K::Equal] {
            assert_eq!(lifecycle_key(code, true, false, false), None, "{code:?}");
        }
        assert_eq!(lifecycle_key(K::KeyN, true, true, false), None, "⇧⌘N is not New");
    }

    #[test]
    fn to_app_command_targets_the_active_tab() {
        let a = Some(ID);
        assert_eq!(to_app_command(FileCmd::New, a), Some(AppCommand::NewDocument));
        assert_eq!(to_app_command(FileCmd::Open, a), Some(AppCommand::OpenDialog));
        assert_eq!(to_app_command(FileCmd::Save, a), Some(AppCommand::Save(ID)));
        assert_eq!(to_app_command(FileCmd::SaveAs, a), Some(AppCommand::SaveAs(ID)));
        assert_eq!(to_app_command(FileCmd::CloseTab, a), Some(AppCommand::CloseDocument(ID)));
        assert_eq!(to_app_command(FileCmd::Quit, a), Some(AppCommand::Quit));
        // no document: the document rows mean nothing; New / Open / Quit still work (S2's empty workspace)
        for f in [FileCmd::Save, FileCmd::SaveAs, FileCmd::CloseTab] {
            assert_eq!(to_app_command(f, None), None, "{f:?}");
        }
        for f in [FileCmd::New, FileCmd::Open, FileCmd::Quit] {
            assert!(to_app_command(f, None).is_some(), "{f:?}");
        }
        // the keyboard path is lifecycle_key → to_app_command
        assert_eq!(key_command(KeyCode::KeyS, m(true, true, false), a), Some(AppCommand::SaveAs(ID)));
        assert_eq!(key_command(KeyCode::KeyS, m(false, false, false), a), None, "S = Scale tool");
    }

    #[test]
    fn every_file_menu_row_reaches_its_lifecycle_command() {
        let items = flat_items(&menus());
        let mut file_rows = Vec::new();
        for e in &items {
            let Entry::Item { id, accel, cmd, .. } = e else { continue };
            match *cmd {
                MenuCmd::File(f) => {
                    let want = to_app_command(f, Some(ID)).expect("a document is open");
                    assert_eq!(menu_route(*cmd, Some(ID)), Some(MenuRoute::App(want)), "{id}");
                    // the key the row SHOWS is the key the keyboard maps to the same command
                    if let Some(k) = accel {
                        assert_eq!(lifecycle_key(k.code, true, k.shift, k.alt), Some(f), "{id}: shown key ≠ key path");
                    }
                    file_rows.push(f);
                }
                // a ⌘-row is a document shortcut, never a synthetic file key (spec §4: command ids)
                MenuCmd::Key(k) => {
                    let m = Mods { ctrl: true, shift: k.shift, alt: k.alt };
                    assert_eq!(key_command(k.code, m, Some(ID)), None, "{id} hides a file command")
                }
                _ => {}
            }
        }
        for f in [FileCmd::New, FileCmd::Open, FileCmd::CloseTab, FileCmd::Save, FileCmd::SaveAs, FileCmd::Quit] {
            assert!(file_rows.contains(&f), "the menu bar has a {f:?} row");
        }
    }

    #[test]
    fn every_command_source_maps_to_its_app_command() {
        // the custom caption's controls
        assert_eq!(win_action_command(WinAction::Minimize), AppCommand::Window(WindowCmd::Minimize));
        assert_eq!(win_action_command(WinAction::ToggleMaximize), AppCommand::Window(WindowCmd::ToggleMaximize));
        assert_eq!(win_action_command(WinAction::Close), AppCommand::Quit, "✕ = the Quit transaction");
        // the Window menu rows are window commands; the other rows keep their own paths
        let p = varos_app::shell::PanelId::DOCKABLE[0];
        for (cmd, want) in [
            (MenuCmd::ToggleRail, MenuRoute::App(AppCommand::Window(WindowCmd::ToggleRail))),
            (MenuCmd::ToggleDock, MenuRoute::App(AppCommand::Window(WindowCmd::ToggleDock))),
            (MenuCmd::TogglePanel(p), MenuRoute::App(AppCommand::Window(WindowCmd::TogglePanel(p)))),
            (MenuCmd::Plain(KeyCode::Backspace), MenuRoute::Plain(KeyCode::Backspace)),
            (MenuCmd::SnapGrid, MenuRoute::Snap { grid: true }),
            (MenuCmd::SnapPoint, MenuRoute::Snap { grid: false }),
        ] {
            assert_eq!(menu_route(cmd, Some(ID)), Some(want), "{cmd:?}");
        }
        assert_eq!(menu_route(MenuCmd::File(FileCmd::Save), None), None, "Save with no document");
        // files from outside: the first instance's own argument, a second instance's hand-off
        let f = PathBuf::from("/work/logo.vrs");
        assert_eq!(
            open_paths_command(vec![f.clone()], OpenOrigin::CommandLine),
            Some(AppCommand::OpenPaths(vec![f.clone()], OpenOrigin::CommandLine))
        );
        assert_eq!(
            open_paths_command(vec![f.clone()], OpenOrigin::OsHandoff),
            Some(AppCommand::OpenPaths(vec![f], OpenOrigin::OsHandoff))
        );
        assert_eq!(open_paths_command(vec![], OpenOrigin::OsHandoff), None, "nothing handed over → no command");
    }

    #[test]
    fn window_title_names_active_document_and_dirty() {
        assert_eq!(window_title("Untitled-1", false), "Untitled-1 — Varos");
        assert_eq!(window_title("Logo.vrs", true), "Logo.vrs* — Varos");
        let mut ws = Workspace::new();
        ws.new_untitled();
        let s = ws.active_mut().unwrap();
        s.editor.execute(EditCommand::AddArtboard);
        assert_eq!(window_title(&s.display_name(), s.is_dirty()), "Untitled-2* — Varos");
    }

    fn line(id: u32) -> VPath {
        let a = |i: u32, x: f32| Anchor { id: i, p: [x, 0.0], hin: None, hout: None, smooth: false };
        VPath::new(id, vec![a(id + 1, 0.0), a(id + 2, 50.0)], false, None, Some([1.0, 0.0, 0.0, 1.0]), 2.0)
    }

    #[test]
    fn scene_key_differs_between_sessions_with_equal_signature() {
        // tab A: blank · tab B: a (non-selected) line — same rev, view, tool and selection
        let a = Editor::new();
        let mut b = Editor::new();
        b.doc.paths.push(line(1));
        b.doc.sync_tree();
        assert_eq!(a.rev, b.rev);
        let (sa, sb) =
            (scene_signature(&a, View::identity(), [800, 600]), scene_signature(&b, View::identity(), [800, 600]));
        assert_eq!(sa, sb, "premise: the signature alone cannot tell these two documents apart");
        assert_ne!(scene_key(SessionId(1), sa), scene_key(SessionId(2), sb), "the tab id must split them");
        assert_eq!(scene_key(SessionId(1), sa), scene_key(SessionId(1), sa), "same tab, same scene = a cache hit");
    }

    #[test]
    fn a_release_goes_where_its_press_went() {
        assert_eq!(route_left_release(true, true, false), LeftRelease::EndPan);
        assert_eq!(route_left_release(false, true, true), LeftRelease::EndPan);
        // pressed on chrome, released anywhere → the chrome's
        assert_eq!(route_left_release(false, false, false), LeftRelease::Chrome);
        assert_eq!(route_left_release(false, false, true), LeftRelease::Chrome);
        // pressed on the canvas → the canvas gesture ends, wherever it is released
        assert_eq!(route_left_release(true, false, false), LeftRelease::Canvas { over_panel: false });
        assert_eq!(route_left_release(true, false, true), LeftRelease::Canvas { over_panel: true });
    }

    /// UI audit 04 B1: a click inside the colour picker (a chrome press + release) must leave its
    /// session open, so Cancel still reverts. The old host sent every release to `pointer_up`, which
    /// ended the picker's transaction.
    #[test]
    fn picker_cancel_still_reverts_after_a_click_on_the_chrome() {
        let red = [1.0, 0.0, 0.0, 1.0];
        let setup = || {
            let mut ed = Editor::new();
            ed.doc.paths.push(line(1));
            ed.doc.sync_tree();
            ed.objsel.insert(1);
            ed.execute(EditCommand::PickerBegin);
            ed.execute(EditCommand::PickerLivePaint { target: PaintTarget::Stroke, color: [0.0, 0.0, 1.0, 1.0] });
            ed
        };
        let stroke = |ed: &Editor| ed.doc.paths[0].stroke.solid();
        // the new routing: the release of a chrome press never reaches the editor
        let mut ed = setup();
        if let LeftRelease::Canvas { .. } = route_left_release(false, false, true) {
            ed.pointer_up();
        }
        ed.execute(EditCommand::PickerCancel);
        assert_eq!(stroke(&ed), Some(red), "Cancel reverts the live colour");
        // the old routing (every release → pointer_up) is what broke it
        let mut ed = setup();
        ed.pointer_up();
        ed.execute(EditCommand::PickerCancel);
        assert_ne!(stroke(&ed), Some(red), "premise: a stray pointer_up ends the picker session");
    }

    /// Records what the host asked of the Ui, and in which state the editor was then.
    #[derive(Default)]
    struct FakeUi {
        log: Vec<&'static str>,
        gesture_open_at_settle: Option<bool>,
    }
    impl DocUi for FakeUi {
        fn settle(&mut self, ed: &mut Editor) {
            self.log.push("settle");
            self.gesture_open_at_settle = Some(ed.transaction_open());
        }
        fn document_switched(&mut self) {
            self.log.push("switched");
        }
    }
    /// The commands used here never ask anything or touch a file.
    struct NoDialogs;
    impl Dialogs for NoDialogs {
        fn pick_open(&mut self) -> Vec<PathBuf> {
            unreachable!()
        }
        fn pick_save(&mut self, _: &str, _: Option<&Path>) -> Option<PathBuf> {
            unreachable!()
        }
        fn ask_save_changes(&mut self, _: &str, _: Option<(usize, usize)>) -> SaveDecision {
            unreachable!()
        }
        fn save_failed(&mut self, _: &str, _: &str) -> SaveFailChoice {
            unreachable!()
        }
        fn open_failed(&mut self, _: &str, _: &str) {
            unreachable!()
        }
        fn confirm_replace(&mut self, _: &str) -> bool {
            unreachable!()
        }
        fn notice(&mut self, _: &str, _: &str) {
            unreachable!()
        }
    }
    struct NoStore;
    impl DocStore for NoStore {
        fn load(&mut self, _: &Path) -> Result<Document, String> {
            unreachable!()
        }
        fn save(&mut self, _: &Document, _: &Path) -> Result<(), String> {
            unreachable!()
        }
        fn key(&self, p: &Path) -> crate::workspace::FileKey {
            crate::workspace::FileKey { path: p.to_path_buf(), dev_ino: None }
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
    }

    /// A command with no modifier held (the mouse / a menu row / a key whose modifiers are up by now).
    fn run(ws: &mut Workspace, ui: &mut FakeUi, cmd: AppCommand) -> Ran {
        run_held(ws, ui, cmd, Mods::default())
    }

    fn run_held(ws: &mut Workspace, ui: &mut FakeUi, cmd: AppCommand, held: Mods) -> Ran {
        let mut keys = Keyboard::default();
        keys.modifiers_changed(held, None);
        run_keys(ws, ui, cmd, &keys)
    }

    fn run_keys(ws: &mut Workspace, ui: &mut FakeUi, cmd: AppCommand, keys: &Keyboard) -> Ran {
        run_lifecycle(cmd, ws, ui, &mut NoDialogs, &mut NoStore, keys)
    }

    #[test]
    fn a_lifecycle_command_settles_the_ui_then_the_gesture_then_resets() {
        let mut ws = Workspace::new();
        let first = ws.active_id().unwrap();
        {
            // a rectangle drag still in flight on tab 1 (the mouse is down)
            let ed = &mut ws.active_mut().unwrap().editor;
            ed.set_tool(ToolKind::Rect);
            ed.pointer_down([0.0, 0.0]);
            ed.pointer_move([80.0, 60.0]);
            assert!(ed.transaction_open() && !matches!(ed.drag, Drag::None));
        }
        let mut ui = FakeUi::default();
        let ran = run(&mut ws, &mut ui, AppCommand::NewDocument);
        assert_eq!(ran, Ran { exit: false, ran: true, switched: true });
        assert_eq!(ui.log, ["settle", "switched"], "Ui settle first, caches reset after");
        assert_eq!(ui.gesture_open_at_settle, Some(true), "the Ui settles BEFORE the gesture is finished");
        let old = &ws.get(first).unwrap().editor;
        assert!(!old.transaction_open() && matches!(old.drag, Drag::None), "the drag was finished…");
        assert_eq!(old.doc.paths.len(), 1, "…into the tab it was drawn on");
        assert!(ws.active().unwrap().editor.doc.paths.is_empty(), "the new tab is clean and boardless");
    }

    #[test]
    fn the_active_editor_mirrors_the_keyboard_after_every_command_and_no_op_activation_does_nothing() {
        let mut ws = Workspace::new();
        let id = ws.active_id().unwrap();
        ws.active_mut().unwrap().editor.mods = m(true, true, false);
        let mut ui = FakeUi::default();
        // clicking the chip that is already active: not a lifecycle change at all
        assert_eq!(run(&mut ws, &mut ui, AppCommand::ActivateDocument(id)), Ran::default());
        assert!(ui.log.is_empty());
        assert!(ws.active().unwrap().editor.mods.ctrl, "untouched");
        // Ctrl+Tab with one tab: no switch, but the command ran — the editor now reads what the keyboard holds
        let ran = run_held(&mut ws, &mut ui, AppCommand::ActivateNext, m(true, false, false));
        assert_eq!(ran, Ran { exit: false, ran: true, switched: false });
        let mods = ws.active().unwrap().editor.mods;
        assert!(mods.ctrl && !mods.shift, "Control is still held; the stale ⇧ is gone");
        // a command after the keys were released (e.g. under a native dialog): nothing reads as held
        run(&mut ws, &mut ui, AppCommand::ActivateNext);
        let mods = ws.active().unwrap().editor.mods;
        assert!(!mods.ctrl && !mods.shift, "no modifier is held");
        // a clean workspace quits at once
        assert!(run(&mut ws, &mut ui, AppCommand::Quit).exit);
    }

    #[test]
    fn the_keyboard_outlives_a_tab_switch() {
        let mut ws = Workspace::new();
        let a = ws.active_id().unwrap();
        let b = ws.new_untitled();
        assert!(ws.activate(a));
        let mut kb = Keyboard::default();
        kb.modifiers_changed(m(true, false, false), ws.active_mut().map(|s| &mut s.editor));
        assert!(ws.active().unwrap().editor.mods.ctrl, "the active editor mirrors the keyboard");
        assert_eq!(kb.command(KeyCode::Tab, Some(a)), Some(AppCommand::ActivateNext));
        let mut ui = FakeUi::default();
        assert!(run_keys(&mut ws, &mut ui, AppCommand::ActivateNext, &kb).switched);
        assert_eq!(ws.active_id(), Some(b));
        // winit sends nothing more while Control stays down: the next Tab is still Ctrl+Tab
        assert_eq!(kb.command(KeyCode::Tab, Some(b)), Some(AppCommand::ActivateNext));
        assert!(ws.active().unwrap().editor.mods.ctrl, "the incoming tab's gestures see Control held too");
        // Control released: the keyboard and the active mirror both let go
        kb.modifiers_changed(Mods::default(), ws.active_mut().map(|s| &mut s.editor));
        assert_eq!(kb.command(KeyCode::Tab, Some(b)), None, "plain Tab is not a tab switch");
        assert!(!ws.active().unwrap().editor.mods.ctrl);
    }

    /// Codex review P3: Space is a held key like the modifiers — ONE truth in the host, not a per-tab
    /// flag a switch forgets (A9: a live placement drag repositions while Space is down).
    #[test]
    fn held_space_outlives_a_tab_switch_and_focus_loss_lets_it_go() {
        let mut ws = Workspace::new();
        let a = ws.active_id().unwrap();
        let b = ws.new_untitled();
        assert!(ws.activate(a));
        let mut kb = Keyboard::default();
        kb.space_changed(true, ws.active_mut().map(|s| &mut s.editor));
        assert!(ws.active().unwrap().editor.space, "the active editor mirrors Space");
        let mut ui = FakeUi::default();
        assert!(run_keys(&mut ws, &mut ui, AppCommand::ActivateNext, &kb).switched);
        assert_eq!(ws.active_id(), Some(b));
        assert!(kb.space(), "still held");
        assert!(ws.active().unwrap().editor.space, "the incoming tab's editor sees Space held");
        // the window loses focus (a dialog, ⌘Tab): Space's key-up will never come — let it go now
        kb.modifiers_changed(m(true, false, false), ws.active_mut().map(|s| &mut s.editor));
        kb.focus_lost(ws.active_mut().map(|s| &mut s.editor));
        assert!(!kb.space() && !kb.held().ctrl, "nothing reads as held");
        let ed = &ws.active().unwrap().editor;
        assert!(!ed.space && !ed.mods.ctrl, "and neither does the active mirror");
    }

    // ---- the action queue (review re-check: pointer input orders document keys) ----

    fn names(v: &[HostAction]) -> Vec<String> {
        v.iter()
            .map(|a| match a {
                HostAction::App(c) => format!("{c:?}"),
                HostAction::Doc(DocAction::Key(code, _)) => format!("Key({code:?})"),
                HostAction::Doc(DocAction::Snap { grid }) => format!("Snap({grid})"),
            })
            .collect()
    }

    const UNDO: HostAction =
        HostAction::Doc(DocAction::Key(KeyCode::KeyZ, Mods { ctrl: true, shift: false, alt: false }));

    fn activate(n: u64) -> HostAction {
        HostAction::App(AppCommand::ActivateDocument(SessionId(n)))
    }

    #[test]
    fn a_key_alone_runs_now() {
        let q = ActionQueue::default();
        assert!(q.doc_runs_now());
    }

    #[test]
    fn a_key_after_a_whole_click_waits_behind_the_click() {
        let mut q = ActionQueue::default();
        q.pointer_button(false);
        q.pointer_button(true); // the release on tab B's chip — egui makes it Activate(B) at the frame
        assert!(!q.doc_runs_now(), "the click's command is still to come");
        q.push(UNDO);
        let at = q.last_release();
        q.chrome_frame([(at, activate(2))]);
        assert_eq!(names(&q.take_ready()), ["ActivateDocument(SessionId(2))", "Key(KeyZ)"]);
        assert!(q.is_empty());
    }

    #[test]
    fn a_key_between_press_and_release_runs_before_the_click() {
        // (a) press B → ⌘Z → release B → frame: the click completes AFTER the key
        let mut q = ActionQueue::default();
        q.pointer_button(false);
        assert!(!q.doc_runs_now(), "an unfilled mark: the key waits");
        q.push(UNDO);
        q.pointer_button(true);
        let at = q.last_release();
        q.chrome_frame([(at, activate(2))]);
        assert_eq!(names(&q.take_ready()), ["Key(KeyZ)", "ActivateDocument(SessionId(2))"]);
    }

    #[test]
    fn two_clicks_around_a_key_keep_event_order() {
        // (b) click A → ⌘Z → click B → frame: Activate(A), ⌘Z, Activate(B)
        let mut q = ActionQueue::default();
        q.pointer_button(false);
        let release_a = q.pointer_button(true);
        q.push(UNDO);
        q.pointer_button(false);
        let release_b = q.pointer_button(true);
        assert_eq!(q.last_release(), Some(release_b));
        q.chrome_frame([(Some(release_a), activate(1)), (Some(release_b), activate(2))]);
        assert_eq!(
            names(&q.take_ready()),
            ["ActivateDocument(SessionId(1))", "Key(KeyZ)", "ActivateDocument(SessionId(2))"]
        );
    }

    #[test]
    fn unfilled_marks_go_after_the_frame_and_keys_stop_waiting() {
        // (c) a canvas click raises no chrome command: its marks must not outlive the frame
        let mut q = ActionQueue::default();
        q.pointer_button(false);
        q.pointer_button(true);
        assert!(!q.doc_runs_now());
        q.chrome_frame([]);
        assert!(q.is_empty() && q.doc_runs_now(), "no marks left: the next key runs at once");
    }

    #[test]
    fn a_key_before_a_click_ran_first() {
        let mut q = ActionQueue::default();
        assert!(q.doc_runs_now(), "the key runs at once, ahead of the later click");
        q.pointer_button(false);
        q.pointer_button(true);
        let at = q.last_release();
        q.chrome_frame([(at, activate(2))]);
        assert_eq!(names(&q.take_ready()), ["ActivateDocument(SessionId(2))"]);
    }

    #[test]
    fn a_drain_before_the_ui_frame_keeps_the_click_and_what_follows_it() {
        let mut q = ActionQueue::default();
        q.push(HostAction::App(AppCommand::Save(SessionId(1))));
        q.pointer_button(false);
        q.pointer_button(true);
        q.push(UNDO);
        q.push(HostAction::App(AppCommand::Save(SessionId(1))));
        assert_eq!(names(&q.take_ready()), ["Save(SessionId(1))"], "only what came before the click");
        assert!(!q.is_empty() && !q.doc_runs_now());
        let at = q.last_release();
        q.chrome_frame([(at, activate(2))]);
        assert_eq!(names(&q.take_ready()), ["ActivateDocument(SessionId(2))", "Key(KeyZ)", "Save(SessionId(1))"]);
        // a command with no pointer event behind it goes to the tail
        q.chrome_frame([(None, HostAction::App(AppCommand::NewDocument))]);
        assert_eq!(names(&q.take_ready()), ["NewDocument"]);
        assert!(q.is_empty());
    }
}
