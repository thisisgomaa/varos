//! macOS only: the open-documents bridge (DFS S4-B). A Finder double-click, "Open With ▸ Varos",
//! `open file.vrs`, `open -a Varos file.vrs` and a drop on the Dock icon all reach the app as ONE
//! AppKit call, `-[NSApplicationDelegate application:openURLs:]`. Registering `.vrs` in the bundle
//! (tools/mac/bundle.sh) only routes the file to us; this bridge is what receives it.
//!
//! winit 0.30.13 owns `NSApp`'s delegate (`WinitApplicationDelegate`) and implements only
//! `applicationDidFinishLaunching:` / `applicationWillTerminate:`; replacing the delegate is fatal
//! (winit's `ApplicationDelegate::get` panics unless the delegate is its own class). So — the same
//! careful runtime edit `mac_caption::forbid_native_window_drag` makes to winit's view class — we
//! ADD one method to winit's delegate class, verify the IMP is ours, and log the outcome. If a
//! future winit implements the selector itself, nothing is added and the log says so.
//!
//! The IMP never calls into winit or the editor (no re-entrancy): it keeps the file URLs, pushes
//! their paths into `os_open`'s queue and wakes the loop through an `EventLoopProxy`; `main.rs`
//! drains the queue at `AboutToWait` into the one `OpenPaths` path. Pin: re-check on any winit bump.

use std::path::PathBuf;
use std::sync::Mutex;

use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::sel;
use objc2_app_kit::NSApplication;
use objc2_foundation::{MainThreadMarker, NSArray, NSURL};
use winit::event_loop::EventLoopProxy;

use crate::os_open;

/// The waker the IMP uses (set once by `install`; `None` in tests → the IMP only queues).
static WAKE: Mutex<Option<EventLoopProxy<()>>> = Mutex::new(None);

/// `- (void)application:(NSApplication *)app openURLs:(NSArray<NSURL *> *)urls`.
const OPEN_URLS_TYPES: &std::ffi::CStr = c"v@:@@";

/// What `add_open_urls` did to a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddResult {
    /// Our method was added.
    Added,
    /// Our method was already there (installed earlier) — nothing changed.
    AlreadyOurs,
    /// The class (or a superclass) already implements the selector itself — left untouched.
    Foreign,
    /// The runtime refused to add the method.
    Failed,
}

/// The paths of the `file://` URLs, in order; any other URL (https:, a custom scheme) is ignored.
pub fn file_url_paths(urls: &NSArray<NSURL>) -> Vec<PathBuf> {
    urls.iter().filter(|u| u.isFileURL()).filter_map(|u| u.to_file_path()).collect()
}

extern "C-unwind" fn application_open_urls(
    _this: *mut AnyObject,
    _cmd: Sel,
    _app: *mut AnyObject,
    urls: *mut NSArray<NSURL>,
) {
    // Never unwind into AppKit: a panic here is caught and the event dropped (logged).
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: AppKit passes a valid (possibly nil) `NSArray<NSURL *> *` for the duration of the
        // call; we only borrow it here.
        let Some(urls) = (unsafe { urls.as_ref() }) else { return };
        let paths = file_url_paths(urls);
        if os_open::enqueue(paths) > 0 {
            let wake = match WAKE.lock() {
                Ok(g) => g.clone(),
                Err(p) => p.into_inner().clone(),
            };
            if let Some(proxy) = wake {
                let _ = proxy.send_event(());
            }
        }
    }));
    if r.is_err() {
        eprintln!("[varos] open-documents: a Finder open event could not be read");
    }
}

fn our_imp() -> Imp {
    let f: extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *mut NSArray<NSURL>) = application_open_urls;
    // SAFETY: an `Imp` is a type-erased function pointer; the runtime calls it with the selector's
    // real signature (`v@:@@`: receiver, `_cmd`, two objects, void), which is exactly `f`'s ABI.
    unsafe {
        std::mem::transmute::<extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *mut NSArray<NSURL>), Imp>(f)
    }
}

/// Does `cls` answer `application:openURLs:` through OUR implementation?
pub fn handles_open_urls(cls: &AnyClass) -> bool {
    cls.instance_method(sel!(application:openURLs:))
        .is_some_and(|m| m.implementation() as *const () == our_imp() as *const ())
}

/// Add our `application:openURLs:` to `cls` unless it (or a superclass) already answers it.
pub fn add_open_urls(cls: &AnyClass) -> AddResult {
    let sel = sel!(application:openURLs:);
    if cls.instance_method(sel).is_some() {
        return if handles_open_urls(cls) { AddResult::AlreadyOurs } else { AddResult::Foreign };
    }
    // SAFETY: `cls` is a registered class; the IMP's ABI matches the encoding `v@:@@` (see
    // `our_imp`); the encoding is a static NUL-terminated C string; the runtime serialises
    // method-list edits. `class_addMethod` never replaces an existing method.
    let added = unsafe {
        objc2::ffi::class_addMethod(std::ptr::from_ref(cls).cast_mut(), sel, our_imp(), OPEN_URLS_TYPES.as_ptr())
    };
    if added.as_bool() && handles_open_urls(cls) {
        AddResult::Added
    } else {
        AddResult::Failed
    }
}

/// Install the bridge on winit's app delegate. Call after `EventLoop::build()` (the delegate
/// exists) and BEFORE `run()` (Launch Services delivers a cold-launch open during
/// `finishLaunching`, inside `run`). Logs one `[varos] open-documents bridge: …` line.
pub fn install(proxy: EventLoopProxy<()>) {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[varos] open-documents bridge: skipped (not on the main thread)");
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(delegate) = app.delegate() else {
        eprintln!("[varos] open-documents bridge: skipped (the app has no delegate yet)");
        return;
    };
    let object: &AnyObject = delegate.as_ref();
    let cls = object.class();
    match WAKE.lock() {
        Ok(mut g) => *g = Some(proxy),
        Err(p) => *p.into_inner() = Some(proxy),
    }
    let result = add_open_urls(cls);
    match result {
        AddResult::Added | AddResult::AlreadyOurs => {
            // AppKit caches which optional delegate methods exist when the delegate is SET (winit
            // set it inside `EventLoop::build`, before this method existed): re-assign the same
            // object so it re-reads them. Sound: winit keeps the strong reference
            // (`EventLoop.delegate`); the object and its class are unchanged.
            app.setDelegate(Some(&*delegate));
            eprintln!("[varos] open-documents bridge: installed on {} ({result:?})", cls.name().to_string_lossy());
        }
        AddResult::Foreign => {
            eprintln!(
                "[varos] open-documents bridge: skipped (delegate already handles openURLs — bridge not installed)"
            )
        }
        AddResult::Failed => {
            eprintln!("[varos] open-documents bridge: skipped (the runtime refused the method — Finder opens will not reach Varos)")
        }
    }
}

#[cfg(test)]
mod tests {
    //! No NSApp, window or EventLoop: throwaway `NSObject` subclasses and plain NSURL objects.
    use super::*;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{ClassBuilder, NSObject};
    use objc2_foundation::NSString;

    fn url(s: &str) -> Retained<NSURL> {
        NSURL::URLWithString(&NSString::from_str(s)).expect("a valid URL")
    }

    #[test]
    fn file_url_paths_keeps_file_urls_only() {
        let urls = NSArray::from_retained_slice(&[
            url("file:///tmp/a.vrs"),
            url("https://example.com/b.vrs"),
            NSURL::from_file_path("/tmp/ملف مع مسافة.vrs").expect("absolute path"),
        ]);
        assert_eq!(file_url_paths(&urls), [PathBuf::from("/tmp/a.vrs"), PathBuf::from("/tmp/ملف مع مسافة.vrs")]);
    }

    #[test]
    fn add_open_urls_adds_once_then_reports_already_ours() {
        let base = AnyClass::get(c"NSObject").expect("Foundation is linked");
        let cls = ClassBuilder::new(c"VarosS4ProbeDelegateA", base).expect("a fresh class name").register();
        assert!(!handles_open_urls(cls));
        assert_eq!(add_open_urls(cls), AddResult::Added);
        assert!(handles_open_urls(cls));
        assert_eq!(add_open_urls(cls), AddResult::AlreadyOurs, "idempotent");
        assert!(AnyClass::get(c"NSObject").unwrap().instance_method(sel!(application:openURLs:)).is_none());
    }

    extern "C-unwind" fn foreign_open_urls(_: *mut AnyObject, _: Sel, _: *mut AnyObject, _: *mut AnyObject) {}

    #[test]
    fn a_class_that_already_answers_open_urls_is_left_alone() {
        let base = AnyClass::get(c"NSObject").expect("Foundation is linked");
        let cls = ClassBuilder::new(c"VarosS4ProbeDelegateB", base).expect("a fresh class name").register();
        type F = extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject);
        let f: F = foreign_open_urls;
        // SAFETY: same ABI as the selector (`v@:@@`) — a stand-in for a future winit's own method.
        let ok = unsafe {
            let imp: Imp = std::mem::transmute::<F, Imp>(f);
            objc2::ffi::class_addMethod(
                std::ptr::from_ref(cls).cast_mut(),
                sel!(application:openURLs:),
                imp,
                OPEN_URLS_TYPES.as_ptr(),
            )
        };
        assert!(ok.as_bool());
        assert_eq!(add_open_urls(cls), AddResult::Foreign);
        assert!(!handles_open_urls(cls), "the existing implementation was not replaced");
    }

    #[test]
    fn add_open_urls_imp_enqueues_file_urls_only() {
        let base = AnyClass::get(c"NSObject").expect("Foundation is linked");
        let cls = ClassBuilder::new(c"VarosS4ProbeDelegateC", base).expect("a fresh class name").register();
        assert_eq!(add_open_urls(cls), AddResult::Added);
        // SAFETY: a plain instance of a registered NSObject subclass.
        let obj: Retained<NSObject> = unsafe { msg_send![cls, new] };
        let unique = format!("/tmp/varos-s4-imp-{}.vrs", std::process::id());
        let urls = NSArray::from_retained_slice(&[
            NSURL::from_file_path(&unique).expect("absolute path"),
            url("https://example.com/not-a-file.vrs"),
        ]);
        let none: *mut AnyObject = std::ptr::null_mut();
        // SAFETY: the method we just added, called with its declared argument types.
        let () = unsafe { msg_send![&*obj, application: none, openURLs: &*urls] };
        // this is the only test that touches the process-wide queue
        let batch = os_open::take_batch().expect("the file URL was queued");
        assert_eq!(
            batch.command,
            crate::app_command::AppCommand::OpenPaths(
                vec![PathBuf::from(&unique)],
                crate::app_command::OpenOrigin::OsHandoff
            )
        );
        assert_eq!(os_open::take_batch(), None, "drained once");
    }
}
