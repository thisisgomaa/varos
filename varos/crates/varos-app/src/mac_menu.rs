//! macOS only: the native menu bar (muda) built from `chrome::menus()`, and the opaque window
//! background. See docs/foundation/MAC_CHROME.md. Nothing here decides behaviour — a click becomes
//! a `chrome::MenuCmd` that `main.rs` runs through the SAME paths the keyboard / buttons use.

use crate::chrome::{self, Check, Entry, MenuCmd, Native};
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use winit::event_loop::EventLoopProxy;
use winit::keyboard::KeyCode;
use winit::window::Window;

/// muda key for a menu key (every key `chrome::menus()` uses — tested).
pub fn muda_code(code: KeyCode) -> Option<Code> {
    use KeyCode as K;
    Some(match code {
        K::KeyC => Code::KeyC,
        K::KeyD => Code::KeyD,
        K::KeyG => Code::KeyG,
        K::KeyO => Code::KeyO,
        K::KeyQ => Code::KeyQ,
        K::KeyR => Code::KeyR,
        K::KeyS => Code::KeyS,
        K::KeyU => Code::KeyU,
        K::KeyV => Code::KeyV,
        K::KeyW => Code::KeyW,
        K::KeyX => Code::KeyX,
        K::KeyZ => Code::KeyZ,
        K::Digit0 => Code::Digit0,
        K::Digit1 => Code::Digit1,
        K::Equal => Code::Equal,
        K::Minus => Code::Minus,
        K::Semicolon => Code::Semicolon,
        K::BracketLeft => Code::BracketLeft,
        K::BracketRight => Code::BracketRight,
        _ => return None,
    })
}

fn accelerator(a: chrome::Accel) -> Option<Accelerator> {
    let mut m = Modifiers::SUPER; // ⌘
    if a.shift {
        m |= Modifiers::SHIFT;
    }
    if a.alt {
        m |= Modifiers::ALT;
    }
    muda_code(a.code).map(|c| Accelerator::new(m, c))
}

/// The live menu bar. Keep it alive for the whole run (it owns the NSMenu items).
pub struct MacMenu {
    menu: Menu,
    window_menu: Submenu,
    cmds: HashMap<MenuId, MenuCmd>,
    checks: Vec<(Check, CheckMenuItem)>,
    rx: Receiver<MenuId>,
}

impl MacMenu {
    /// Build the bar from the table and route clicks to `drain` (the proxy wakes the idle loop).
    pub fn build(proxy: EventLoopProxy<()>) -> Result<Self, muda::Error> {
        let (tx, rx) = channel::<MenuId>();
        let wake = Mutex::new((tx, proxy));
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Ok(g) = wake.lock() {
                let _ = g.0.send(e.id);
                let _ = g.1.send_event(());
            }
        }));
        let menu = Menu::new();
        let mut cmds = HashMap::new();
        let mut checks = Vec::new();
        let mut window_menu = None;
        for (title, entries) in chrome::menus() {
            let sub = Submenu::new(title, true);
            fill(&sub, &entries, &mut cmds, &mut checks)?;
            menu.append(&sub)?;
            if title == "Window" {
                window_menu = Some(sub);
            }
        }
        let window_menu = window_menu.expect("the table has a Window menu (tested)");
        Ok(Self { menu, window_menu, cmds, checks, rx })
    }

    /// Make it the app's menu bar. Call once the app has finished launching (first `NewEvents`).
    pub fn install(&self) {
        self.menu.init_for_nsapp();
        self.window_menu.set_as_windows_menu_for_nsapp(); // AppKit lists the open window there
    }

    /// The commands clicked (or ⌘-keyed) since the last call.
    pub fn drain(&self) -> Vec<MenuCmd> {
        self.rx.try_iter().filter_map(|id| self.cmds.get(&id).copied()).collect()
    }

    /// Write each check mark from the real state (only the ones that differ).
    pub fn sync(&self, is_on: impl Fn(Check) -> bool) {
        for (c, item) in &self.checks {
            let on = is_on(*c);
            if item.is_checked() != on {
                item.set_checked(on);
            }
        }
    }
}

fn fill(
    sub: &Submenu,
    entries: &[Entry],
    cmds: &mut HashMap<MenuId, MenuCmd>,
    checks: &mut Vec<(Check, CheckMenuItem)>,
) -> Result<(), muda::Error> {
    for e in entries {
        match e {
            Entry::Sep => sub.append(&PredefinedMenuItem::separator())?,
            Entry::Native(n) => sub.append(&native(*n))?,
            Entry::Sub { label, items } => {
                let s = Submenu::new(*label, true);
                fill(&s, items, cmds, checks)?;
                sub.append(&s)?;
            }
            Entry::Item { id, label, accel, cmd, check } => {
                let acc = accel.and_then(accelerator);
                let mid = MenuId::new(id);
                cmds.insert(mid.clone(), *cmd);
                let it: Box<dyn IsMenuItem> = match check {
                    Some(c) => {
                        let ci = CheckMenuItem::with_id(mid, *label, true, false, acc);
                        checks.push((*c, ci.clone()));
                        Box::new(ci)
                    }
                    None => Box::new(MenuItem::with_id(mid, *label, true, acc)),
                };
                sub.append(it.as_ref())?;
            }
        }
    }
    Ok(())
}

fn native(n: Native) -> PredefinedMenuItem {
    match n {
        Native::About => PredefinedMenuItem::about(
            Some("About Varos"),
            Some(AboutMetadata {
                name: Some("Varos".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some("Arabic-first vector design \u{b7} pre-alpha".into()),
                ..Default::default()
            }),
        ),
        Native::Services => PredefinedMenuItem::services(None),
        Native::Hide => PredefinedMenuItem::hide(Some("Hide Varos")),
        Native::HideOthers => PredefinedMenuItem::hide_others(None),
        Native::ShowAll => PredefinedMenuItem::show_all(None),
        Native::Minimize => PredefinedMenuItem::minimize(None),
        Native::Zoom => PredefinedMenuItem::maximize(Some("Zoom")),
        Native::Fullscreen => PredefinedMenuItem::fullscreen(None),
        Native::BringAllToFront => PredefinedMenuItem::bring_all_to_front(None),
    }
}

/// Paint the NSWindow's own background `rgb` (sRGB), so nothing behind the window ever shows through
/// — not the title strip, not the frame before the GPU's first present, not a live-resize edge.
pub fn set_window_background(window: &Window, rgb: [u8; 3]) {
    use objc2_app_kit::{NSColor, NSView};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };
    // SAFETY: winit hands out its live content NSView; we are on the main thread (the event-loop
    // thread that created the window) and only borrow the view for these two AppKit calls.
    let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
    if let Some(ns_window) = view.window() {
        let [r, g, b] = rgb.map(|c| c as f64 / 255.0);
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
        ns_window.setBackgroundColor(Some(&color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_key_has_a_native_key_equivalent() {
        for e in chrome::flat_items(&chrome::menus()) {
            if let Entry::Item { id, accel: Some(a), .. } = e {
                let acc = accelerator(a).unwrap_or_else(|| panic!("{id}: no muda key for {:?}", a.code));
                assert!(acc.modifiers().contains(Modifiers::SUPER), "{id}: a Mac menu shortcut uses ⌘");
                assert_eq!(acc.modifiers().contains(Modifiers::SHIFT), a.shift, "{id}");
                assert_eq!(acc.modifiers().contains(Modifiers::ALT), a.alt, "{id}");
            }
        }
    }
}
