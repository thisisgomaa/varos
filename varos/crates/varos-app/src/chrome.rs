//! Platform window chrome — pure data + logic, GPU-free and tested (docs/foundation/MAC_CHROME.md).
//!
//! * `TOPBAR`: how our own top bar sits in the window on this platform (left inset for the macOS
//!   traffic lights, whether we paint our own ─ ☐ ✕ caps).
//! * `caption_hit`: is a physical-px point on the EMPTY part of the bar (the drag band)?
//! * `menus()`: the native macOS menu bar as a table. Every item is a MIRROR of a path that already
//!   exists — a ⌘-shortcut keystroke, the ✕ close path, or a toggle an egui menu already offers.
//!   The AppKit glue that turns this table into an NSMenu lives in `mac_menu.rs` (macOS only).
#![cfg_attr(not(target_os = "macos"), allow(dead_code))] // the menu table is only built on macOS

use varos_app::shell::PanelId;
use winit::keyboard::KeyCode;

/// How the top bar fits the window chrome on one platform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopbarChrome {
    /// Logical height: match the native 28 pt title area on macOS.
    pub height: f32,
    /// Logical px before the burger cell (macOS: room for the native traffic lights).
    pub lead: f32,
    /// Logical px between the right-most bar control and the window edge when there are no caps.
    pub right_inset: f32,
    /// Do we paint our own minimize / maximize / close caps (Windows' stripped caption)?
    pub window_caps: bool,
}

/// The table: macOS keeps the native traffic lights (so no caps of ours, and a left inset that clears
/// them — they end ≈ 70 pt from the left); every other platform keeps the original Windows numbers.
pub const fn topbar_chrome(macos: bool) -> TopbarChrome {
    if macos {
        TopbarChrome { height: 28.0, lead: 78.0, right_inset: 6.0, window_caps: false }
    } else {
        TopbarChrome { height: 46.0, lead: 4.0, right_inset: 0.0, window_caps: true }
    }
}

/// This build's top-bar chrome.
pub const TOPBAR: TopbarChrome = topbar_chrome(cfg!(target_os = "macos"));

/// The actual rectangles painted / hit-tested by the top bar. Text widths come from egui's
/// font measurement; all padding, vertical alignment and tab fitting live here.
pub struct TopbarLayout {
    pub caps: Option<[egui::Rect; 3]>,
    pub menu: egui::Rect,
    pub magnet: egui::Rect,
    pub window: egui::Rect,
    pub share: egui::Rect,
    pub export: egui::Rect,
    pub search: egui::Rect,
    pub tabs: Vec<egui::Rect>,
    pub plus: Option<egui::Rect>,
}

pub fn topbar_layout(
    bar: egui::Rect,
    chrome: TopbarChrome,
    button_text_widths: [f32; 3],
    search_width: f32,
    tab_text_widths: impl IntoIterator<Item = f32>,
) -> TopbarLayout {
    use egui::{pos2, vec2, Rect};
    let cy = bar.center().y;
    let caps = chrome.window_caps.then(|| {
        [3.0, 2.0, 1.0].map(|i| {
            Rect::from_min_max(
                pos2(bar.right() - i * 42.0, bar.top()),
                pos2(bar.right() - (i - 1.0) * 42.0, bar.bottom()),
            )
        })
    });
    let caps_left = caps.map_or(bar.right() - chrome.right_inset, |r| r[0].left());
    let magnet = Rect::from_center_size(pos2(caps_left - 6.0 - 14.0, cy), vec2(28.0, 28.0));
    let button = |right: f32, text_width: f32| {
        Rect::from_min_max(pos2(right - text_width - 24.0, cy - 13.0), pos2(right, cy + 13.0))
    };
    let window = button(magnet.left() - 8.0, button_text_widths[0]);
    let share = button(window.left() - 8.0, button_text_widths[1]);
    let export = button(share.left() - 8.0, button_text_widths[2]);
    let search_right = export.left() - 8.0;
    let search = Rect::from_min_max(pos2(search_right - search_width, cy - 12.0), pos2(search_right, cy + 12.0));
    let menu = Rect::from_min_size(pos2(bar.left() + chrome.lead, bar.top()), vec2(36.0, bar.height()));
    let tabs_right = search.left() - 12.0;
    let mut tx = menu.right() + 8.0;
    let mut tabs = Vec::new();
    for text_width in tab_text_widths {
        let tw = (12.0 + text_width + 8.0 + 18.0 + 4.0).clamp(76.0, 220.0);
        if tx + tw > tabs_right {
            break;
        }
        tabs.push(Rect::from_min_size(pos2(tx, cy - 14.0), vec2(tw, 28.0)));
        tx += tw + 4.0;
    }
    let plus = (tx + 32.0 <= tabs_right).then(|| Rect::from_center_size(pos2(tx + 16.0, cy), vec2(32.0, 28.0)));
    TopbarLayout { caps, menu, magnet, window, share, export, search, tabs, plus }
}

pub fn tab_close_rect(tab: egui::Rect) -> egui::Rect {
    egui::Rect::from_center_size(egui::pos2(tab.right() - 13.0, tab.center().y), egui::vec2(18.0, 18.0))
}

/// Is the window opaque from the first frame? macOS: yes — a transparent NSWindow let the title strip
/// show the desktop through (Ahmed 2026-09-23), so the splash card sits on the dark window instead of
/// floating over the desktop. Windows keeps its transparent floating splash.
pub const OPAQUE_WINDOW: bool = cfg!(target_os = "macos");

/// Is physical-px point (x, y) inside the caption band of height `h` and NOT on one of the bar's
/// interactive rects (`[l, t, r, b]`, physical px, as `cursors::set_caption` receives them)?
pub fn caption_hit(h: i32, excl: &[[i32; 4]], x: i32, y: i32) -> bool {
    y >= 0 && y < h && x >= 0 && !excl.iter().any(|r| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
}

/// A menu key equivalent: ⌘ + optional ⇧ / ⌥ + key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Accel {
    pub code: KeyCode,
    pub shift: bool,
    pub alt: bool,
}
const fn cmd(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: false, alt: false })
}
const fn cmd_shift(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: true, alt: false })
}
const fn cmd_alt(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: false, alt: true })
}

/// What a clicked item does — each one an EXISTING path in the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCmd {
    /// The ⌘ + key shortcut, fed to the same dispatch the keyboard uses (`main.rs`).
    Key(Accel),
    /// The ✕ caption button's path (`WinAction::Close`: save window state, exit).
    Close,
    /// The bar's Window menu rows.
    ToggleRail,
    ToggleDock,
    TogglePanel(PanelId),
    /// The magnet (Snapping) quick-menu rows.
    SnapGrid,
    SnapPoint,
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
    Rail,
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
fn toggle(id: &str, label: &'static str, cmd: MenuCmd, check: Check) -> Entry {
    Entry::Item { id: id.into(), label, accel: None, cmd, check: Some(check) }
}

/// The whole menu bar, left → right. The first menu is the application menu (macOS titles it with
/// the app's name whatever label it gets).
pub fn menus() -> Vec<(&'static str, Vec<Entry>)> {
    use KeyCode as K;
    let mut window = vec![
        Entry::Native(Native::Minimize),
        Entry::Native(Native::Zoom),
        Entry::Sep,
        toggle("win.rail", "Tool rail", MenuCmd::ToggleRail, Check::Rail),
        toggle("win.dock", "Control bar", MenuCmd::ToggleDock, Check::Dock),
        Entry::Sep,
    ];
    for p in PanelId::DOCKABLE {
        window.push(Entry::Item {
            id: format!("win.panel.{}", p.title()),
            label: p.title(),
            accel: None,
            cmd: MenuCmd::TogglePanel(p),
            check: Some(Check::Panel(p)),
        });
    }
    window.extend([Entry::Sep, Entry::Native(Native::BringAllToFront)]);
    vec![
        (
            "Varos",
            vec![
                Entry::Native(Native::About),
                Entry::Sep,
                Entry::Native(Native::Services),
                Entry::Sep,
                Entry::Native(Native::Hide),
                Entry::Native(Native::HideOthers),
                Entry::Native(Native::ShowAll),
                Entry::Sep,
                item("app.quit", "Quit Varos", cmd(K::KeyQ), MenuCmd::Close),
            ],
        ),
        (
            "File",
            vec![
                key("file.open", "Open\u{2026}", cmd(K::KeyO)),
                Entry::Sep,
                key("file.save", "Save", cmd(K::KeyS)),
                key("file.saveas", "Save As\u{2026}", cmd_shift(K::KeyS)),
                Entry::Sep,
                item("file.close", "Close Window", cmd(K::KeyW), MenuCmd::Close),
            ],
        ),
        ("Edit", vec![key("edit.undo", "Undo", cmd(K::KeyZ)), key("edit.redo", "Redo", cmd_shift(K::KeyZ))]),
        (
            "Object",
            vec![
                key("obj.again", "Transform Again", cmd(K::KeyD)),
                Entry::Sep,
                Entry::Sub {
                    label: "Arrange",
                    items: vec![
                        key("obj.front", "Bring to Front", cmd_shift(K::BracketRight)),
                        key("obj.forward", "Bring Forward", cmd(K::BracketRight)),
                        key("obj.backward", "Send Backward", cmd(K::BracketLeft)),
                        key("obj.back", "Send to Back", cmd_shift(K::BracketLeft)),
                    ],
                },
                Entry::Sep,
                key("obj.group", "Group", cmd(K::KeyG)),
                key("obj.ungroup", "Ungroup", cmd_shift(K::KeyG)),
            ],
        ),
        (
            "View",
            vec![
                key("view.fit", "Fit in Window", cmd(K::Digit0)),
                key("view.actual", "Actual Size", cmd(K::Digit1)),
                key("view.zoomin", "Zoom In", cmd(K::Equal)),
                key("view.zoomout", "Zoom Out", cmd(K::Minus)),
                Entry::Sep,
                key_check("view.rulers", "Rulers", cmd(K::KeyR), Check::Rulers),
                key_check("view.guides", "Guides", cmd(K::Semicolon), Check::Guides),
                key_check("view.lockguides", "Lock Guides", cmd_alt(K::Semicolon), Check::GuidesLocked),
                key_check("view.smart", "Smart Guides", cmd(K::KeyU), Check::SmartGuides),
                Entry::Sep,
                toggle("view.snapgrid", "Snap to Grid", MenuCmd::SnapGrid, Check::SnapGrid),
                toggle("view.snappoint", "Snap to Point", MenuCmd::SnapPoint, Check::SnapPoint),
                Entry::Sep,
                Entry::Native(Native::Fullscreen),
            ],
        ),
        ("Window", window),
    ]
}

/// Every clickable item in the bar (depth-first) — the table checks in the tests walk it.
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
        K::KeyD => E::D,
        K::KeyG => E::G,
        K::KeyO => E::O,
        K::KeyQ => E::Q,
        K::KeyR => E::R,
        K::KeyS => E::S,
        K::KeyU => E::U,
        K::KeyW => E::W,
        K::KeyZ => E::Z,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_topbar_numbers_are_unchanged_and_mac_clears_the_traffic_lights() {
        let win = topbar_chrome(false);
        assert_eq!(win, TopbarChrome { height: 46.0, lead: 4.0, right_inset: 0.0, window_caps: true });
        let mac = topbar_chrome(true);
        assert!(!mac.window_caps, "macOS uses the native traffic lights, never our ─ ☐ ✕");
        assert!(mac.lead >= 72.0, "the three traffic lights end ≈ 70 pt from the left edge");
        assert!(mac.right_inset > 0.0);
        assert_eq!(TOPBAR, topbar_chrome(cfg!(target_os = "macos")));
    }

    #[test]
    fn mac_topbar_controls_share_the_native_traffic_light_centre() {
        let chrome = topbar_chrome(true);
        // Include a translated bar, minimum window width, overflow tabs and a wide window.
        for origin in [egui::pos2(0.0, 0.0), egui::pos2(31.0, 47.0)] {
            for width in [800.0, 1280.0, 1920.0] {
                let bar = egui::Rect::from_min_size(origin, egui::vec2(width, chrome.height));
                let layout = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, [65.0, 180.0, 300.0]);
                assert!(layout.caps.is_none());
                assert!(!layout.tabs.is_empty());
                if width >= 1280.0 {
                    assert!(layout.plus.is_some());
                }
                let controls = [layout.menu, layout.magnet, layout.window, layout.share, layout.export, layout.search]
                    .into_iter()
                    .chain(layout.tabs.iter().copied())
                    .chain(layout.tabs.iter().copied().map(tab_close_rect))
                    .chain(layout.plus);
                let traffic_light_centre = bar.top() + 14.0;
                for rect in controls {
                    assert!(bar.contains_rect(rect), "control {rect:?} escapes bar {bar:?}");
                    assert!(
                        (rect.center().y - traffic_light_centre).abs() <= 1.0,
                        "control {rect:?} is not centred on traffic lights at {traffic_light_centre}"
                    );
                }
            }
        }
    }

    #[test]
    fn caption_hit_is_the_empty_band_only() {
        let excl = [[100, 0, 200, 92]];
        assert!(caption_hit(92, &excl, 50, 10));
        assert!(caption_hit(92, &excl, 250, 91));
        assert!(!caption_hit(92, &excl, 150, 10), "an interactive rect is not a drag spot");
        assert!(!caption_hit(92, &excl, 50, 92), "below the band");
        assert!(!caption_hit(0, &[], 50, 0), "no band published yet (splash) → never drag");
    }

    #[test]
    fn every_shortcut_item_shows_exactly_the_keystroke_it_sends() {
        for e in flat_items(&menus()) {
            if let Entry::Item { id, accel, cmd: MenuCmd::Key(k), .. } = e {
                assert_eq!(accel, Some(k), "{id}: shown shortcut ≠ sent keystroke");
            }
        }
    }

    #[test]
    fn ids_and_accelerators_are_unique() {
        let items = flat_items(&menus());
        let mut ids = std::collections::HashSet::new();
        let mut accels = std::collections::HashSet::new();
        for e in &items {
            if let Entry::Item { id, accel, .. } = e {
                assert!(ids.insert(id.clone()), "duplicate id {id}");
                if let Some(a) = accel {
                    assert!(accels.insert(*a), "{id}: accelerator {a:?} used twice");
                }
            }
        }
    }

    #[test]
    fn every_menu_key_can_be_handed_to_a_text_field() {
        for e in flat_items(&menus()) {
            if let Entry::Item { id, accel: Some(a), .. } = e {
                assert!(egui_key(a.code).is_some(), "{id}: no egui key for {:?}", a.code);
            }
        }
    }

    #[test]
    fn the_bar_has_the_standard_mac_menus_and_mirrors_every_dockable_panel() {
        let m = menus();
        let titles: Vec<&str> = m.iter().map(|(t, _)| *t).collect();
        assert_eq!(titles, ["Varos", "File", "Edit", "Object", "View", "Window"]);
        let items = flat_items(&m);
        let has = |c: MenuCmd| items.iter().any(|e| matches!(e, Entry::Item { cmd, .. } if *cmd == c));
        for p in PanelId::DOCKABLE {
            assert!(has(MenuCmd::TogglePanel(p)), "Window menu misses {}", p.title());
        }
        assert!(has(MenuCmd::ToggleRail) && has(MenuCmd::ToggleDock));
        // ⌘Q and ⌘W both take the ✕ path
        let close: Vec<_> = items
            .iter()
            .filter_map(|e| match e {
                Entry::Item { cmd: MenuCmd::Close, accel, .. } => *accel,
                _ => None,
            })
            .collect();
        assert_eq!(close, [cmd(KeyCode::KeyQ).unwrap(), cmd(KeyCode::KeyW).unwrap()]);
        // mirrors only: no item may claim a New / Export / clipboard key that has no path yet
        for missing in [KeyCode::KeyN, KeyCode::KeyC, KeyCode::KeyV, KeyCode::KeyX, KeyCode::KeyA] {
            assert!(
                !items.iter().any(|e| matches!(e, Entry::Item { accel: Some(a), .. } if a.code == missing)),
                "{missing:?} has no existing shortcut path — must not be in the menu"
            );
        }
    }
}
