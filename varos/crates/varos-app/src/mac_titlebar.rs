//! macOS only: the native traffic lights on the 4b band's centre line (MAC_CHROME.md §A′).
//!
//! The window keeps AppKit's own close / minimise / zoom buttons. With a full-size content view and a
//! transparent title bar AppKit lays them out in its own title-bar container (measured 32 pt tall on
//! Darwin 27, centre y 16). The 4b band is 52 pt tall with every control centred on y 26, so here the
//! container grows to the band height and each standard button's ORIGIN Y is set so its centre sits
//! on the band centre — never its x (AppKit owns the horizontal spacing, and the RTL mirror if a
//! localization ever adds it). The Electron `trafficLightPosition` / Tauri overlay pattern; no
//! toolbar (an NSToolbar would sit over the band and take tab presses for window drags — P15).
//!
//! `place_traffic_lights` READS the frames on every call and WRITES only when one is off by more
//! than `TOLERANCE` — the steady state is zero writes. AppKit re-lays its title bar out on its own
//! (window resize / zoom, becoming visible, title / document-edited changes — see MAC_CHROME.md
//! §A′ for what was measured), so the host calls it at startup, at the top of every redraw and right
//! after `set_title` / `set_document_edited`. It only ever touches the two AppKit views it has
//! verified by class name (`NSTitlebarView` inside `NSTitlebarContainerView`); anything else →
//! `Unavailable`, nothing resized. Skipped in native fullscreen (the lights live in the menu-bar
//! reveal overlay there).
//!
//! `VAROS_TITLEBAR_DEBUG=1` logs (stderr) every write with its call site and the frames before /
//! after, a calls / writes summary every 5 s of calls, and `VAROS_TITLEBAR_PROBE=1` runs a one-time
//! probe on the first redraw that reports which AppKit calls reset the placement.
use objc2::MainThreadMarker;
use objc2_app_kit::{NSView, NSWindow, NSWindowButton, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use winit::window::Window;

/// Frames within this many points are "already placed" (AppKit rounds to half points on 2×).
pub const TOLERANCE: f64 = 0.25;

/// The AppKit classes the buttons must live in: `button ⊂ NSTitlebarView ⊂ NSTitlebarContainerView`.
pub const TITLEBAR_VIEW: &str = "NSTitlebarView";
pub const TITLEBAR_CONTAINER: &str = "NSTitlebarContainerView";

/// The origin y (in its superview) that centres a `button_h`-tall button in a `band_h`-tall title
/// bar. Symmetric, so it holds whether or not the title-bar view is flipped.
pub fn centred_origin_y(band_h: f64, button_h: f64) -> f64 {
    (band_h - button_h) / 2.0
}

/// Does a frame value `current` have to be rewritten to become `want`?
pub fn needs_update(current: f64, want: f64) -> bool {
    (current - want).abs() > TOLERANCE
}

/// The title-bar container's wanted frame inside the window's frame view (`theme_h` tall): full
/// width, `band_h` tall, glued to the window's top edge (bottom-left origin unless `flipped`).
pub fn container_frame(theme_w: f64, theme_h: f64, band_h: f64, flipped: bool) -> (f64, f64, f64, f64) {
    let y = if flipped { 0.0 } else { theme_h - band_h };
    (0.0, y, theme_w, band_h)
}

/// Is this the view hierarchy we know how to adjust (the two ancestors' class names)?
pub fn known_hierarchy(titlebar_class: &str, container_class: &str) -> bool {
    titlebar_class == TITLEBAR_VIEW && container_class == TITLEBAR_CONTAINER
}

fn debug() -> bool {
    std::env::var_os("VAROS_TITLEBAR_DEBUG").is_some_and(|v| v == "1")
}

fn class_name(v: &NSView) -> String {
    v.class().name().to_string_lossy().into_owned()
}

fn ns_window(window: &Window) -> Option<objc2::rc::Retained<NSWindow>> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return None };
    // SAFETY: winit hands out its live content NSView; the caller holds a `MainThreadMarker` (the
    // event-loop thread that created the window) and only borrows the view to reach its window.
    let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
    view.window()
}

fn fmt(r: NSRect) -> String {
    format!("({:.1}, {:.1}, {:.1}×{:.1})", r.origin.x, r.origin.y, r.size.width, r.size.height)
}

const BUTTONS: [(NSWindowButton, &str); 3] = [
    (NSWindowButton::CloseButton, "close"),
    (NSWindowButton::MiniaturizeButton, "minimize"),
    (NSWindowButton::ZoomButton, "zoom"),
];

/// What one call did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// Native fullscreen: nothing touched.
    Fullscreen,
    /// Every frame was already where it belongs — nothing written.
    AlreadyPlaced,
    /// At least one frame was rewritten.
    Moved,
    /// Off the main thread, or the window / its standard buttons / the expected title-bar views
    /// could not be reached: nothing touched.
    Unavailable,
}

/// The verified views: window, title-bar container, title-bar view, theme frame.
struct Views {
    win: objc2::rc::Retained<NSWindow>,
    container: objc2::rc::Retained<NSView>,
    titlebar: objc2::rc::Retained<NSView>,
    theme: objc2::rc::Retained<NSView>,
}

fn views(window: &Window) -> Option<Views> {
    let win = ns_window(window)?;
    let close = win.standardWindowButton(NSWindowButton::CloseButton)?;
    // SAFETY (the three `superview` calls): plain AppKit view-hierarchy reads on the main thread.
    let titlebar = unsafe { close.superview() }?;
    let container = unsafe { titlebar.superview() }?;
    let theme = unsafe { container.superview() }?;
    // never resize a view we do not recognise (a future macOS may rebuild the title bar)
    if !known_hierarchy(&class_name(&titlebar), &class_name(&container)) {
        if debug() {
            eprintln!(
                "[varos titlebar] unknown hierarchy: {} in {} — not touched",
                class_name(&titlebar),
                class_name(&container)
            );
        }
        return None;
    }
    Some(Views { win, container, titlebar, theme })
}

/// Settle AppKit's own pending title-bar layout FIRST (a `setTitle` / `setDocumentEdited` / resize
/// only marks it dirty — the reset to the native height lands on the next layout pass, measured by the
/// probe), then rewrite whatever is off. Reading after the settle makes the check exact: the steady
/// state writes nothing, and a reset is fixed in the same call instead of one frame later. True when
/// something was written.
fn apply(v: &Views, band_h: f64) -> bool {
    v.theme.layoutSubtreeIfNeeded();
    let mut moved = false;
    let tb = v.theme.bounds();
    let (x, y, w, h) = container_frame(tb.size.width, tb.size.height, band_h, v.theme.isFlipped());
    let cf = v.container.frame();
    if needs_update(cf.origin.y, y) || needs_update(cf.size.height, h) || needs_update(cf.size.width, w) {
        v.container.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(w, h)));
        moved = true;
    }
    // the title-bar view normally follows its container (autoresizing); make sure it fills it
    let bar = v.titlebar.frame();
    if needs_update(bar.size.height, band_h) || needs_update(bar.origin.y, 0.0) || needs_update(bar.size.width, w) {
        v.titlebar.setFrame(NSRect::new(NSPoint::new(bar.origin.x, 0.0), NSSize::new(w, band_h)));
        moved = true;
    }
    let bar_h = v.titlebar.frame().size.height;
    for (kind, _) in BUTTONS {
        let Some(b) = v.win.standardWindowButton(kind) else { continue };
        let f = b.frame();
        let want = centred_origin_y(bar_h, f.size.height);
        if needs_update(f.origin.y, want) {
            b.setFrameOrigin(NSPoint::new(f.origin.x, want)); // never x
            moved = true;
        }
    }
    moved
}

static CALLS: AtomicU64 = AtomicU64::new(0);
static WRITES: AtomicU64 = AtomicU64::new(0);
static PROBED: AtomicBool = AtomicBool::new(false);
static WINDOW_START: Mutex<Option<(Instant, u64, u64)>> = Mutex::new(None);

/// Put the three traffic lights' centres on `band_h / 2` from the window top. Reads every call,
/// writes only when something is off. `site` names the caller in the debug log.
pub fn place_traffic_lights(window: &Window, band_h: f64, site: &'static str) -> Placement {
    let Some(_mtm) = MainThreadMarker::new() else { return Placement::Unavailable };
    let Some(v) = views(window) else { return Placement::Unavailable };
    if v.win.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return Placement::Fullscreen;
    }
    let log = debug();
    let probe_on = std::env::var_os("VAROS_TITLEBAR_PROBE").is_some_and(|p| p == "1");
    if log && site != "startup" && probe_on && !PROBED.swap(true, Ordering::Relaxed) {
        probe(&v, band_h);
    }
    let before = log.then(|| snapshot(&v));
    let moved = apply(&v, band_h);
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let writes = WRITES.fetch_add(u64::from(moved), Ordering::Relaxed) + u64::from(moved);
    if log {
        if moved {
            eprintln!(
                "[varos titlebar] write #{writes} (call {calls}, site {site}) — before: {}",
                before.unwrap_or_default()
            );
            eprintln!("[varos titlebar] write #{writes} (call {calls}, site {site}) — after:  {}", snapshot(&v));
        }
        if let Ok(mut w) = WINDOW_START.lock() {
            let (t0, c0, w0) = *w.get_or_insert((Instant::now(), calls, writes));
            if t0.elapsed().as_secs_f64() >= 5.0 {
                eprintln!(
                    "[varos titlebar] last {:.1} s: {} calls, {} writes (totals {calls} / {writes})",
                    t0.elapsed().as_secs_f64(),
                    calls - c0,
                    writes - w0
                );
                *w = Some((Instant::now(), calls, writes));
            }
        }
    }
    if moved {
        Placement::Moved
    } else {
        Placement::AlreadyPlaced
    }
}

/// `VAROS_TITLEBAR_PROBE=1`: from a placed state, perform each AppKit call the host makes (and a
/// bare layout pass), force the pending layout, and report whether the placement survived — the
/// "what resets it" measurement in MAC_CHROME.md §A′. Restores title, edited flag and frame.
fn probe(v: &Views, band_h: f64) {
    let placed = |v: &Views| !needs_update(v.container.frame().size.height, band_h);
    let settle = |v: &Views| {
        v.theme.layoutSubtreeIfNeeded();
        v.theme.displayIfNeeded();
    };
    apply(v, band_h);
    settle(v);
    eprintln!(
        "[varos titlebar probe] a bare layout pass after our own write: placement {}",
        if placed(v) { "kept" } else { "RESET" }
    );
    // each action toggles, so running it twice restores the window
    let title = v.win.title();
    let frame = v.win.frame();
    let flip = std::cell::Cell::new(false);
    let actions: [(&str, &dyn Fn()); 3] = [
        ("setTitle", &|| {
            flip.set(!flip.get());
            let t = if flip.get() { NSString::from_str("Varos probe") } else { title.clone() };
            v.win.setTitle(&t);
        }),
        ("setDocumentEdited", &|| v.win.setDocumentEdited(!v.win.isDocumentEdited())),
        ("setFrame (24 wider / back)", &|| {
            let f = v.win.frame();
            let w =
                if (f.size.width - frame.size.width).abs() < 1.0 { frame.size.width + 24.0 } else { frame.size.width };
            v.win.setFrame_display(NSRect::new(f.origin, NSSize::new(w, f.size.height)), true);
        }),
    ];
    for (what, act) in actions {
        act();
        settle(v); // force the layout pass the action queued, then look
        let reset = !placed(v);
        apply(v, band_h);
        act(); // …and the way the host does it: the check right after the call
        apply(v, band_h);
        settle(v);
        eprintln!(
            "[varos titlebar probe] {what}: the next layout pass {} · checked right after the call: placement {}",
            if reset { "RESETS the container to the native height" } else { "keeps the placement" },
            if placed(v) { "kept" } else { "LOST" },
        );
    }
    // the steady state: the real check, ~60 times a second for 5 s, nothing changing in between
    let (t0, mut calls, mut writes) = (Instant::now(), 0u32, 0u32);
    while t0.elapsed().as_secs_f64() < 5.0 {
        writes += u32::from(apply(v, band_h));
        calls += 1;
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    eprintln!("[varos titlebar probe] steady state: {calls} checks over 5 s, {writes} writes");
}

/// One log line: window / container / title-bar frames, each light's frame and its centre measured
/// from the window's top edge in points.
fn snapshot(v: &Views) -> String {
    let wf = v.win.frame();
    let mut s = format!(
        "window {} visible {} occlusion {:?} · container {} · titlebar {} (flipped {})",
        fmt(wf),
        v.win.isVisible(),
        v.win.occlusionState(),
        fmt(v.container.frame()),
        fmt(v.titlebar.frame()),
        v.titlebar.isFlipped()
    );
    for (kind, name) in BUTTONS {
        if let Some(b) = v.win.standardWindowButton(kind) {
            let in_window = b.convertRect_toView(b.bounds(), None);
            let cy = wf.size.height - (in_window.origin.y + in_window.size.height / 2.0);
            let cx = in_window.origin.x + in_window.size.width / 2.0;
            s.push_str(&format!(" · {name} {} centre ({cx:.1}, {cy:.1} from top)", fmt(b.frame())));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centred_origin_y_puts_the_button_centre_on_the_band_centre() {
        let band = f64::from(varos_app::shell::tokens::BAND_H);
        for button_h in [13.0, 14.0, 16.0, 20.0] {
            let y = centred_origin_y(band, button_h);
            assert_eq!(y + button_h / 2.0, band / 2.0, "a {button_h}-pt button");
        }
        assert_eq!(band / 2.0, 26.0, "the 4b centre line");
        // the frame view: bottom-left origin puts the container at the window top; flipped, at 0
        assert_eq!(container_frame(1512.0, 982.0, band, false), (0.0, 930.0, 1512.0, 52.0));
        assert_eq!(container_frame(1512.0, 982.0, band, true), (0.0, 0.0, 1512.0, 52.0));
    }

    #[test]
    fn needs_update_is_false_once_placed() {
        let want = centred_origin_y(52.0, 14.0);
        assert!(!needs_update(want, want));
        assert!(!needs_update(want + TOLERANCE, want), "AppKit's half-point rounding is 'placed'");
        assert!(needs_update(9.0, want), "the native 32-pt layout (y 9) must move");
        assert!(needs_update(32.0, 52.0));
    }

    /// Only the hierarchy measured on macOS is ever resized; anything else is left alone.
    #[test]
    fn only_the_known_title_bar_views_are_touched() {
        assert!(known_hierarchy("NSTitlebarView", "NSTitlebarContainerView"));
        assert!(!known_hierarchy("NSTitlebarContainerView", "NSTitlebarView"), "swapped");
        assert!(!known_hierarchy("NSView", "NSTitlebarContainerView"));
        assert!(!known_hierarchy("NSTitlebarView", "NSThemeFrame"));
    }

    /// Off the main thread the placement never touches AppKit (no window is needed to prove it:
    /// the main-thread check comes first).
    #[test]
    fn off_main_thread_is_unavailable() {
        let off = std::thread::spawn(|| MainThreadMarker::new().is_none()).join().unwrap();
        assert!(off, "a spawned thread is never the main thread");
    }
}
