//! Files the operating system asks us to open (DFS S4): a Finder double-click, Finder "Open With",
//! `open file.vrs` / `open -a Varos file.vrs`, or a drop on the Dock icon. Pure std — no AppKit,
//! no winit, no egui — so the rules are unit-tested headless on every platform.
//!
//! The macOS bridge (`mac_open.rs`) pushes paths here from inside AppKit's open-documents callback
//! and wakes the event loop; `main.rs` drains the queue at `AboutToWait` into the ONE open path
//! (`AppCommand::OpenPaths`, run by `lifecycle::Lifecycle` through the FIFO `ActionQueue`), so
//! validation-before-replace, Recent, the open notices and "an open file focuses its tab" all apply
//! exactly as they do for ⌘O and Recent. Nothing here opens a file.
//!
//! Cold launch: Launch Services delivers the open event inside `run()` (during `finishLaunching`,
//! before `applicationDidFinishLaunching:`), long before anything could consume it. The queue holds
//! the paths until the first drain; a drain takes everything at once, so each path opens once.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::app_command::{AppCommand, OpenOrigin};

/// Paths waiting for the host's next drain, in arrival order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct OsOpenQueue {
    pending: Vec<PathBuf>,
}

impl OsOpenQueue {
    /// Queue one path. Refused (`false`): an empty or relative path (the OS always hands absolute
    /// file paths; anything else is not a file we can name), or a path already waiting (Finder can
    /// repeat an event; one batch never opens the same file twice).
    pub fn push(&mut self, path: PathBuf) -> bool {
        if path.as_os_str().is_empty() || !path.is_absolute() || self.pending.contains(&path) {
            return false;
        }
        self.pending.push(path);
        true
    }

    /// Everything waiting, in arrival order; the queue is empty afterwards.
    pub fn drain(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.pending)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// What the host does with one drained batch: the ONE open command, and whether to bring the
/// window to the front (an OS open always does — the user just asked for this file from outside).
#[derive(Clone, Debug, PartialEq)]
pub struct OsOpenBatch {
    pub command: AppCommand,
    pub raise_window: bool,
}

/// The routing decision for whatever is waiting. `None` = nothing arrived (no command, no raise).
/// Cold vs warm needs no branch here: the lifecycle's `OpenPaths` already turns Start's private
/// placeholder into the first file's tab (no Untitled), appends a tab next to existing work, and
/// focuses a file that is already open without reloading it.
pub fn route(queue: &mut OsOpenQueue) -> Option<OsOpenBatch> {
    let paths = queue.drain();
    if paths.is_empty() {
        return None;
    }
    Some(OsOpenBatch { command: AppCommand::OpenPaths(paths, OpenOrigin::OsHandoff), raise_window: true })
}

/// The process-wide queue the AppKit callback fills (it cannot reach the host's state).
static QUEUE: Mutex<OsOpenQueue> = Mutex::new(OsOpenQueue { pending: Vec::new() });

fn with_queue<R>(f: impl FnOnce(&mut OsOpenQueue) -> R) -> R {
    // poison-tolerant: a panic elsewhere must not lose (or re-panic on) queued files
    match QUEUE.lock() {
        Ok(mut q) => f(&mut q),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

/// Queue paths from the OS. Returns how many were accepted.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS bridge produces OS events today
pub fn enqueue(paths: impl IntoIterator<Item = PathBuf>) -> usize {
    with_queue(|q| paths.into_iter().filter(|p| q.push(p.clone())).count())
}

/// The host's drain (once per `AboutToWait`).
pub fn take_batch() -> Option<OsOpenBatch> {
    with_queue(route)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(name: &str) -> PathBuf {
        // absolute on every platform the tests run on
        std::env::temp_dir().join(name)
    }

    #[test]
    fn early_events_wait_until_drained_once() {
        // events before the host exists: nothing consumes them until the first drain
        let mut q = OsOpenQueue::default();
        assert!(q.push(abs("a.vrs")));
        assert!(q.push(abs("b.vrs")));
        assert!(!q.is_empty());
        let first = route(&mut q).expect("the waiting files form one batch");
        assert_eq!(first.command, AppCommand::OpenPaths(vec![abs("a.vrs"), abs("b.vrs")], OpenOrigin::OsHandoff));
        assert!(first.raise_window);
        assert!(q.is_empty());
        assert_eq!(route(&mut q), None, "a second drain finds nothing: each file opens once");
    }

    #[test]
    fn several_files_keep_their_order_and_duplicates_coalesce() {
        let mut q = OsOpenQueue::default();
        for name in ["c.vrs", "a.vrs", "c.vrs", "b.vrs", "a.vrs"] {
            q.push(abs(name));
        }
        assert_eq!(q.drain(), [abs("c.vrs"), abs("a.vrs"), abs("b.vrs")]);
        // after a drain the same file may arrive again (the lifecycle then focuses its tab)
        assert!(q.push(abs("a.vrs")));
    }

    #[test]
    fn relative_and_empty_paths_are_refused() {
        let mut q = OsOpenQueue::default();
        assert!(!q.push(PathBuf::new()));
        assert!(!q.push(PathBuf::from("relative/x.vrs")));
        assert!(q.is_empty());
        assert_eq!(route(&mut q), None, "nothing usable arrived → no command, no window raise");
    }

    /// The bundle routes `.vrs` to Varos (and nothing else): the Info.plist that tools/mac/bundle.sh
    /// writes declares the document type (Editor, Owner) and exports its UTI; no `.pdf` claim.
    #[test]
    fn bundle_plist_declares_the_vrs_document_type_and_uti() {
        let script = include_str!("../../../../tools/mac/bundle.sh");
        let squash = |s: &str| s.split_whitespace().collect::<String>();
        let plist = squash(script);
        assert!(script.contains("DOC_UTI=\"com.varos.editor.document\""));
        for needle in [
            "<key>CFBundleDocumentTypes</key>",
            "<key>CFBundleTypeRole</key><string>Editor</string>",
            "<key>LSHandlerRank</key><string>Owner</string>",
            "<key>CFBundleTypeExtensions</key><array><string>vrs</string></array>",
            "<key>LSItemContentTypes</key><array><string>${DOC_UTI}</string></array>",
            "<key>UTExportedTypeDeclarations</key>",
            "<key>UTTypeIdentifier</key><string>${DOC_UTI}</string>",
            "<key>UTTypeConformsTo</key><array><string>public.data</string>",
            "<key>public.filename-extension</key><array><string>vrs</string></array>",
        ] {
            assert!(plist.contains(&squash(needle)), "bundle.sh's Info.plist lacks {needle}");
        }
        assert!(
            !plist.contains("<string>pdf</string>") && !plist.contains("com.adobe.pdf"),
            "Varos must not claim .pdf"
        );
        assert!(script.contains("NO_INSTALL"), "the bundle can be built without installing over /Applications");
    }

    #[test]
    fn unicode_and_space_paths_pass_through_unchanged() {
        let mut q = OsOpenQueue::default();
        let p = abs("ملف تجربة — v2.vrs");
        assert!(q.push(p.clone()));
        assert_eq!(q.drain(), [p]);
    }
}
