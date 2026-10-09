//! The native macOS menu bar as a TABLE (split out of `chrome.rs`, slice 0.6 — a pure move): one
//! file per menu (`file`, `edit`, `object`, `view`, `window`); this module holds the shared types and
//! assembles them, left → right. Every item is a MIRROR of a path that already exists — a ⌘-shortcut
//! keystroke, a lifecycle command, or a toggle an egui menu already offers. The AppKit glue that
//! turns this table into an NSMenu lives in `mac_menu.rs` (macOS only).
#![cfg_attr(not(target_os = "macos"), allow(dead_code))] // the menu table is only built on macOS

use varos_app::shell::PanelId;
use winit::keyboard::KeyCode;

mod edit;
mod file;
mod object;
#[cfg(test)]
mod tests;
mod view;
mod window;

/// A menu key equivalent: ⌘ + optional ⇧ / ⌥ + key — or, for a function key only, the bare key
/// (`cmd: false`: File ▸ Revert = F12, as in Illustrator).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Accel {
    pub code: KeyCode,
    pub shift: bool,
    pub alt: bool,
    /// ⌘ is part of the shortcut. `false` only for a function key (tested: `menus::tests`).
    pub cmd: bool,
}
const fn cmd(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: false, alt: false, cmd: true })
}
const fn cmd_shift(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: true, alt: false, cmd: true })
}
const fn cmd_alt(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: false, alt: true, cmd: true })
}
/// A bare function key (no modifier): File ▸ Revert's F12.
const fn fkey(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: false, alt: false, cmd: false })
}

/// A native File-menu row's (and Varos ▸ Quit's) lifecycle identity (DFS S1 §3.6 / review F5). These
/// dispatch through `MenuCmd::File`, never through `MenuCmd::Key`'s synthetic-keystroke path (spec
/// §4: "Menus and physical keys dispatch once through command IDs, not synthetic key events") — a
/// focused text field must not swallow ⌘S. S1-D's `to_app_command` is the one place that turns a
/// `FileCmd` into an `AppCommand`; S6-C added `Export` (File ▸ Export ▸ PDF…, no shortcut — the
/// spec lists none, work order R6). Slice 0.6 added Close All ⌥⌘W, Save a Copy ⌥⌘S, Revert F12 and
/// Export Selection… (no key) — Illustrator's keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCmd {
    New,
    Open,
    CloseTab,
    CloseAll,
    Save,
    SaveAs,
    SaveCopy,
    Revert,
    Export,
    ExportSelection,
    Print,
    Quit,
}

/// What the File rows' enabled state reads, every frame (`mac_menu::MacMenu::sync_file_rows`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocMenuState {
    /// A document tab is showing (not Home).
    pub active: bool,
    /// The active tab has a file AND unsaved changes (`lifecycle::can_revert`).
    pub can_revert: bool,
    /// The active tab has a selection (`lifecycle::has_selection`).
    pub has_selection: bool,
}

/// Is File row `f` enabled in state `s`? New / Open / Quit always; Revert only with a file and
/// unsaved changes; Export Selection only with a selection; every other row needs a document.
pub fn file_row_enabled(f: FileCmd, s: DocMenuState) -> bool {
    match f {
        FileCmd::New | FileCmd::Open | FileCmd::Quit => true,
        FileCmd::Print => s.active && cfg!(target_os = "macos"),
        FileCmd::Revert => s.active && s.can_revert,
        FileCmd::ExportSelection => s.active && s.has_selection,
        FileCmd::CloseTab
        | FileCmd::CloseAll
        | FileCmd::Save
        | FileCmd::SaveAs
        | FileCmd::SaveCopy
        | FileCmd::Export => s.active,
    }
}

/// What a clicked item does — each one an EXISTING path in the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCmd {
    /// The ⌘ + key shortcut, fed to the same dispatch the keyboard uses (`main.rs`).
    Key(Accel),
    /// A PLAIN key (no modifier) fed to that same dispatch — for a click-only row that shows NO key
    /// equivalent, so AppKit never takes the key from a focused text field (e.g. Edit ▸ Delete runs
    /// the Delete/Backspace path, while Backspace keeps deleting text in a field). The host runs it
    /// only when no text field wants the keyboard.
    Plain(KeyCode),
    /// A File-menu row (or Varos ▸ Quit) — see `FileCmd`.
    File(FileCmd),
    /// The bar's Window menu rows.
    ToggleRail,
    ToggleDock,
    TogglePicker,
    TogglePanel(PanelId),
    ResetLayout,
    /// A snapping row (View): flips one `SnapConfig` flag. Alignment / Geometric Guides lived only in
    /// the magnet quick-menu before 4b removed it from the band.
    Snap(SnapRow),
}

/// The snapping rows of the View menu — each one `SnapConfig` flag (`main.rs` `menu_snap_toggle`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapRow {
    Grid,
    Point,
    AlignGuides,
    GeomGuides,
}

/// A check mark, read back from the real state every frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Rulers,
    Guides,
    GuidesLocked,
    SmartGuides,
    SnapGrid,
    SnapPoint,
    AlignGuides,
    GeomGuides,
    Rail,
    Picker,
    Dock,
    Panel(PanelId),
}

/// Standard macOS items that AppKit itself performs (no Varos behaviour behind them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    Zoom,
    Fullscreen,
    BringAllToFront,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Item { id: String, label: &'static str, accel: Option<Accel>, cmd: MenuCmd, check: Option<Check> },
    Sub { label: &'static str, items: Vec<Entry> },
    Native(Native),
    Sep,
}

fn item(id: &str, label: &'static str, accel: Option<Accel>, cmd: MenuCmd) -> Entry {
    Entry::Item { id: id.into(), label, accel, cmd, check: None }
}
/// A shortcut item: the accelerator shown IS the keystroke it sends.
fn key(id: &str, label: &'static str, a: Option<Accel>) -> Entry {
    let acc = a.expect("a shortcut item has a key");
    Entry::Item { id: id.into(), label, accel: a, cmd: MenuCmd::Key(acc), check: None }
}
fn key_check(id: &str, label: &'static str, a: Option<Accel>, check: Check) -> Entry {
    let acc = a.expect("a shortcut item has a key");
    Entry::Item { id: id.into(), label, accel: a, cmd: MenuCmd::Key(acc), check: Some(check) }
}
/// A File-menu row (and Varos ▸ Quit): the accelerator shown IS the keystroke `FileCmd` runs — never
/// `MenuCmd::Key`'s text-field-forwarding path (review F5).
fn file_key(id: &str, label: &'static str, a: Option<Accel>, cmd: FileCmd) -> Entry {
    let accel = a.expect("a shortcut item has a key");
    Entry::Item { id: id.into(), label, accel: Some(accel), cmd: MenuCmd::File(cmd), check: None }
}
/// A File-menu row with NO shortcut (File ▸ Export ▸ PDF…): still a `MenuCmd::File` command row.
fn file_row(id: &str, label: &'static str, cmd: FileCmd) -> Entry {
    Entry::Item { id: id.into(), label, accel: None, cmd: MenuCmd::File(cmd), check: None }
}
fn toggle(id: &str, label: &'static str, cmd: MenuCmd, check: Check) -> Entry {
    Entry::Item { id: id.into(), label, accel: None, cmd, check: Some(check) }
}

/// The whole menu bar, left → right. The first menu is the application menu (macOS titles it with
/// the app's name whatever label it gets).
pub fn menus() -> Vec<(&'static str, Vec<Entry>)> {
    vec![
        ("Varos", app_rows()),
        ("File", file::rows()),
        ("Edit", edit::rows()),
        ("Object", object::rows()),
        ("View", view::rows()),
        ("Window", window::rows()),
    ]
}

/// The application menu (Varos ▸ About … Quit).
fn app_rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        Entry::Native(Native::About),
        Entry::Sep,
        Entry::Native(Native::Services),
        Entry::Sep,
        Entry::Native(Native::Hide),
        Entry::Native(Native::HideOthers),
        Entry::Native(Native::ShowAll),
        Entry::Sep,
        file_key("app.quit", "Quit Varos", cmd(K::KeyQ), FileCmd::Quit),
    ]
}

/// Every clickable item in the bar (depth-first) — the table checks in the tests walk it.
/// Native Recent is a capped mirror, never a separately maintained list.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn recent_menu(recents: &varos_app::storage::recents::Recents) -> Vec<(String, std::path::PathBuf)> {
    recents
        .entries()
        .iter()
        .take(10)
        .map(|e| {
            (
                format!("{} — {}", e.name, e.path.parent().map_or_else(String::new, |p| p.display().to_string())),
                e.path.clone(),
            )
        })
        .collect()
}

#[cfg(test)]
pub fn flat_items(menus: &[(&'static str, Vec<Entry>)]) -> Vec<Entry> {
    fn walk(v: &[Entry], out: &mut Vec<Entry>) {
        for e in v {
            match e {
                Entry::Item { .. } => out.push(e.clone()),
                Entry::Sub { items, .. } => walk(items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for (_, v) in menus {
        walk(v, &mut out);
    }
    out
}

/// The egui key for a shortcut key — used to hand a menu keystroke to a focused text field, exactly
/// as the keyboard would have. Covers every key the menu table uses (tested).
pub fn egui_key(code: KeyCode) -> Option<egui::Key> {
    use egui::Key as E;
    use KeyCode as K;
    Some(match code {
        K::KeyA => E::A,
        K::KeyC => E::C,
        K::KeyD => E::D,
        K::KeyG => E::G,
        K::KeyN => E::N,
        K::KeyO => E::O,
        K::KeyP => E::P,
        K::KeyQ => E::Q,
        K::KeyR => E::R,
        K::KeyS => E::S,
        K::KeyU => E::U,
        K::KeyV => E::V,
        K::KeyW => E::W,
        K::KeyX => E::X,
        K::KeyZ => E::Z,
        K::F12 => E::F12,
        K::Digit0 => E::Num0,
        K::Digit1 => E::Num1,
        K::Equal => E::Equals,
        K::Minus => E::Minus,
        K::Semicolon => E::Semicolon,
        K::BracketLeft => E::OpenBracket,
        K::BracketRight => E::CloseBracket,
        _ => return None,
    })
}
