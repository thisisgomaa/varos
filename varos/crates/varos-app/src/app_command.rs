//! The app-level command vocabulary (DFS S1 §3.2) — the ONE path every document-lifecycle and
//! window request travels, whichever source raised it: keys, the native macOS menu, the burger
//! menu, the tab strip, the window controls, the OS file hand-off and the startup file argument.
//!
//! `AppCommand` is app plumbing (ADR-0002), the sibling of `varos_core::EditCommand`: document
//! EDITS stay `EditCommand`s inside one `Editor`; everything that creates, opens, saves, closes,
//! switches or reorders whole documents is an `AppCommand`, run by `lifecycle::Lifecycle` (or, for
//! `Window(_)`, by the host). It is not a plugin or scripting protocol.
//!
//! API frozen by S1-A — S1-B (lifecycle), S1-C (tab strip) and S1-D (host) build on it as is.

use std::path::PathBuf;

use varos_app::shell::PanelId;

/// A document tab's identity for the life of the app run. Never reused, never an index: tabs can
/// be reordered and closed, and a stale id simply finds nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

/// Where an `OpenPaths` request came from (the lifecycle rules are the same; the host and the
/// messages may differ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenOrigin {
    /// File ▸ Open… / ⌘O / the burger row (paths picked in the Open dialog).
    // Frozen S1-A API: in S1 the Open dialog's paths are opened inside `Lifecycle::run(OpenDialog)`,
    // so no source builds `OpenPaths(.., Dialog)` yet (S2's Start / Recent rows are its callers).
    #[allow(dead_code)]
    Dialog,
    /// The file argument the app was started with (once, after the first framed frame).
    CommandLine,
    /// A path forwarded by the OS / a second instance (the single-instance hand-off).
    OsHandoff,
}

/// Window / panel effects (F4.2). The lifecycle ignores these; the host performs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCmd {
    Minimize,
    ToggleMaximize,
    ToggleRail,
    ToggleDock,
    TogglePanel(PanelId),
}

/// Every document-lifecycle request. There is deliberately no Export command until S6: Export is
/// shown disabled. In S1, Close Window = `Quit` (one window).
#[derive(Clone, Debug, PartialEq)]
pub enum AppCommand {
    /// ⌘N / `+` / File ▸ New — a fresh, clean, boardless `Untitled-N` tab.
    NewDocument,
    /// ⌘O / File ▸ Open… — show the Open dialog, then open what was picked.
    OpenDialog,
    /// Open these files (an already-open file is focused, never reloaded).
    OpenPaths(Vec<PathBuf>, OpenOrigin),
    /// ⌘S — save this tab (goes through Save As when it has no `.vrs` path yet).
    Save(SessionId),
    /// ⇧⌘S — save this tab under a new name.
    SaveAs(SessionId),
    /// ⌘W / the chip's × / middle-click — close this tab (asks first when it has unsaved changes).
    CloseDocument(SessionId),
    /// ⌘Q / the red traffic light / the ✕ caption button — the quit transaction over all tabs.
    Quit,
    /// A click on a tab chip.
    ActivateDocument(SessionId),
    /// Ctrl+Tab (wraps around).
    ActivateNext,
    /// Ctrl+⇧Tab (wraps around).
    ActivatePrevious,
    /// A chip dropped at an insertion SLOT of the FULL tab order: `0` = before the first tab,
    /// `n` = after the last (the drawn slot mapped by `chrome::tab_full_slot`). See `Workspace::reorder`.
    ReorderDocument(SessionId, usize),
    /// F4.2 window/panel effects, performed by the host.
    Window(WindowCmd),
}

/// What the tab strip draws for one tab — a per-frame snapshot, rebuilt by `Workspace::tabs`.
#[derive(Clone, Debug, PartialEq)]
pub struct TabView {
    pub id: SessionId,
    /// `Untitled-3`, or the file name WITH its extension (`Logo.vrs`); equal file names get
    /// ` — <parent folder>` appended.
    pub label: String,
    /// Unsaved changes (the neutral dot before the name).
    pub dirty: bool,
    /// The full path, or `Not saved yet`.
    pub tooltip: String,
}
