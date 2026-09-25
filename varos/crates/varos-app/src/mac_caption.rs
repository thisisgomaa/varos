//! macOS caption decisions; no window, event loop, or GPU needed.
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::sel;
use std::time::{Duration, Instant};

/// Our answer to AppKit's `-[NSView mouseDownCanMoveWindow]`: always NO.
///
/// P15 (owner 2026-09-25, "dragging a tab moves the whole window"): the window has a full-size
/// content view under a transparent title bar (MAC_CHROME.md §A), so the top 28 pt of winit's
/// content view sit in the title-bar area. A plain NSView is not opaque and therefore answers YES
/// here (measured 2026-09-25 on the dev Mac, Darwin 27: `isOpaque=false
/// mouseDownCanMoveWindow=true`), and winit's view does not override it — so AppKit made the WHOLE top strip a native window-drag region, tabs and
/// buttons included. A drag that started on a tab moved the window (in the window server, so egui
/// never saw the motion and the tab never reordered) while plain clicks still reached egui.
/// Answering NO leaves exactly one drag path: our own `caption_drag_position` → `drag_window()` on
/// EMPTY bar space, the same `chrome::caption_hit` predicate Windows' `WM_NCHITTEST` uses.
extern "C-unwind" fn mouse_down_cannot_move_window(_this: *const AnyObject, _cmd: Sel) -> Bool {
    Bool::NO
}

/// Does `cls` already answer NO to `mouseDownCanMoveWindow` through our implementation?
pub fn native_window_drag_forbidden(cls: &AnyClass) -> bool {
    cls.instance_method(sel!(mouseDownCanMoveWindow))
        .is_some_and(|m| m.implementation() as *const () == mouse_down_cannot_move_window as *const ())
}

/// Make every instance of the view class `cls` answer NO to `mouseDownCanMoveWindow`. Only `cls`
/// changes (the method is added to it — or replaced if it defines one); its superclass (`NSView`)
/// keeps its own answer. Returns whether `cls` now answers NO.
pub fn forbid_native_window_drag(cls: &AnyClass) -> bool {
    let sel = sel!(mouseDownCanMoveWindow);
    // Reuse the inherited method's type encoding (`B16@0:8` on arm64, `c16@0:8` on x86_64).
    let Some(inherited) = cls.instance_method(sel) else { return false };
    let imp_fn: extern "C-unwind" fn(*const AnyObject, Sel) -> Bool = mouse_down_cannot_move_window;
    // SAFETY: the selector's signature is `- (BOOL)mouseDownCanMoveWindow`: receiver, `_cmd`, BOOL
    // return — exactly `imp_fn`'s ABI; `class_replaceMethod` stores the pointer type-erased and the
    // encoding is copied from the method it overrides. `cls` is a registered class; the runtime
    // serialises method-list edits.
    unsafe {
        let imp: Imp = std::mem::transmute::<extern "C-unwind" fn(*const AnyObject, Sel) -> Bool, Imp>(imp_fn);
        let types = objc2::ffi::method_getTypeEncoding(inherited);
        objc2::ffi::class_replaceMethod(std::ptr::from_ref(cls).cast_mut(), sel, imp, types);
    }
    native_window_drag_forbidden(cls)
}

/// Background bar geometry permits dragging only when egui has no interaction there.
pub fn caption_drag_allowed(in_caption: bool, egui_hit: bool, dragging: bool) -> bool {
    in_caption && !egui_hit && !dragging
}

/// Positions are egui logical pixels, independent of the display's scale factor.
pub fn caption_double_click(
    previous: Option<(Instant, [f32; 2])>,
    position: [f32; 2],
    now: Instant,
    dragged: bool,
) -> bool {
    !dragged
        && previous.is_some_and(|(at, first)| {
            now.checked_duration_since(at).is_some_and(|dt| dt <= Duration::from_millis(350))
                && (position[0] - first[0]).powi(2) + (position[1] - first[1]).powi(2) <= 4.0_f32.powi(2)
        })
}

#[derive(Default)]
pub struct CaptionClicks {
    previous: Option<(Instant, [f32; 2])>,
    dragged: bool,
}

impl CaptionClicks {
    pub fn press(&mut self, position: [f32; 2], now: Instant) -> bool {
        let zoom = caption_double_click(self.previous, position, now, self.dragged);
        self.previous = if zoom { None } else { Some((now, position)) };
        self.dragged = false;
        zoom
    }

    pub fn reset_after_drag(&mut self) {
        self.previous = None;
        self.dragged = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_drag_requires_empty_bar_without_egui_hit_or_drag() {
        for in_caption in [false, true] {
            for egui_hit in [false, true] {
                for dragging in [false, true] {
                    assert_eq!(
                        caption_drag_allowed(in_caption, egui_hit, dragging),
                        (in_caption, egui_hit, dragging) == (true, false, false)
                    );
                }
            }
        }
    }

    /// P15 root cause, GPU- and window-free: a view class that inherits `NSView`'s answer says YES to
    /// `mouseDownCanMoveWindow` (winit's content view is exactly that), so AppKit drags the window
    /// from ANY press in the title-bar strip. After the fix the class answers NO through our
    /// implementation, and `NSView` itself is left alone.
    #[test]
    fn content_view_class_never_lets_appkit_drag_the_window() {
        let ns_view = AnyClass::get(c"NSView").expect("AppKit is linked");
        let cls =
            objc2::runtime::ClassBuilder::new(c"VarosP15ProbeView", ns_view).expect("a fresh class name").register();
        assert!(!native_window_drag_forbidden(cls), "setup: the inherited NSView answer is in effect");
        assert!(forbid_native_window_drag(cls), "the content view class must answer NO");
        assert!(native_window_drag_forbidden(cls));
        assert!(!native_window_drag_forbidden(ns_view), "NSView itself must keep its own answer");
        // Idempotent: the host may install it again (a second window, a re-created view).
        assert!(forbid_native_window_drag(cls));
        // The installed IMP really answers NO (it never reads `self`, so no instance is needed).
        let imp = cls.instance_method(sel!(mouseDownCanMoveWindow)).expect("method").implementation();
        // SAFETY: `imp` is `mouse_down_cannot_move_window` (checked above) — same signature.
        let f: extern "C-unwind" fn(*const AnyObject, Sel) -> Bool = unsafe { std::mem::transmute(imp) };
        assert!(f(std::ptr::null(), sel!(mouseDownCanMoveWindow)).is_false());
    }

    #[test]
    fn caption_double_click_requires_nearby_timely_presses_without_drag() {
        let at = Instant::now();
        let first = Some((at, [200.0, 20.0]));
        let soon = at + Duration::from_millis(100);
        assert!(!caption_double_click(None, [200.0, 20.0], soon, false));
        assert!(caption_double_click(first, [204.0, 20.0], at + Duration::from_millis(350), false));
        assert!(!caption_double_click(first, [200.0, 20.0], at + Duration::from_millis(351), false));
        assert!(!caption_double_click(first, [204.1, 20.0], soon, false));
        assert!(!caption_double_click(first, [200.0, 24.1], soon, false));
        assert!(!caption_double_click(first, [203.0, 23.0], soon, false));
        assert!(!caption_double_click(first, [800.0, 20.0], soon, false));
        assert!(!caption_double_click(first, [200.0, 20.0], soon, true));
        assert!(!caption_double_click(Some((soon, [200.0, 20.0])), [200.0, 20.0], at, false));
    }

    #[test]
    fn caption_click_sequence_resets_after_drag_and_zoom() {
        let at = Instant::now();
        let mut clicks = CaptionClicks::default();
        let pos = [200.0, 20.0];
        assert!(!clicks.press(pos, at));
        clicks.reset_after_drag();
        assert!(clicks.previous.is_none());
        assert!(!clicks.press(pos, at + Duration::from_millis(100)));
        assert!(clicks.press(pos, at + Duration::from_millis(200)));
        assert!(clicks.previous.is_none());
        assert!(!clicks.press(pos, at + Duration::from_millis(300)));
    }
}
