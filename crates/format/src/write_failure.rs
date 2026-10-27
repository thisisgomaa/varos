//! Varos: why the last atomic file write on this thread failed, by its kind
//! ([`std::io::ErrorKind`]), so the app can act on it: a Save to a location that can't be written
//! (a read-only volume, a folder without permission) goes on as Save As in a folder that takes the
//! file. The app's file services hand errors on as text, which says nothing reliable in any
//! language; this keeps the kind beside it. Every write on a thread replaces what the last one
//! left, and [`take_write_failure`] clears it.

use std::cell::Cell;
use std::io;

thread_local! {
    /// The kind of the last failed [`crate::write_atomic_with`] on this thread, until taken.
    static LAST: Cell<Option<io::ErrorKind>> = const { Cell::new(None) };
}

/// Remember why a write on this thread failed (the atomic writer, on its error).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) fn note(e: &io::Error) {
    LAST.with(|l| l.set(Some(e.kind())));
}

/// Why the last atomic write on this thread failed since this was last asked, and forget it.
/// `None` when none failed (or the failure didn't come from the atomic writer: a refusal of the
/// automation roots, a browser download).
pub fn take_write_failure() -> Option<io::ErrorKind> {
    LAST.with(Cell::take)
}

/// Does a write that failed with `kind` mean the location takes no file at all: a read-only volume
/// (`EROFS`) or a folder or file the user may not write (`EACCES`, `EPERM`)? A full disk, a missing
/// folder or anything else doesn't.
pub fn refuses_writes(kind: io::ErrorKind) -> bool {
    matches!(kind, io::ErrorKind::ReadOnlyFilesystem | io::ErrorKind::PermissionDenied)
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    /// A folder nobody may add files to (as a read-only volume refuses them): the failed write
    /// leaves its kind, which [`refuses_writes`] reads as a location that can't be written. A folder
    /// that isn't there is not one.
    #[test]
    fn a_write_into_a_read_only_folder_leaves_a_refusal() {
        let d = vectorcraft_testkit::temp_dir("write-failure-ro");
        let ro = d.join("locked");
        std::fs::create_dir_all(&ro).unwrap();
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
        let path = ro.join("Poster.varos");
        let _ = take_write_failure();
        let r = crate::write_atomic(&path, b"art");
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        if r.is_ok() {
            // Run as root: permissions don't hold, nothing to check.
            return;
        }
        let kind = take_write_failure().expect("the failure is kept");
        assert!(refuses_writes(kind), "{kind:?}");
        assert_eq!(take_write_failure(), None, "taken once");
        assert!(!path.exists());

        let gone = d.join("missing").join("Poster.varos");
        assert!(crate::write_atomic(&gone, b"art").is_err());
        let kind = take_write_failure().expect("the failure is kept");
        assert_eq!(kind, io::ErrorKind::NotFound);
        assert!(!refuses_writes(kind));

        // A write that works leaves nothing.
        assert!(crate::write_atomic(&d.join("ok.varos"), b"art").is_ok());
        assert_eq!(take_write_failure(), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_read_only_and_permission_kinds_refuse_writes() {
        for (kind, refuses) in [
            (io::ErrorKind::ReadOnlyFilesystem, true),
            (io::ErrorKind::PermissionDenied, true),
            (io::Error::from_raw_os_error(30).kind(), true),
            (io::Error::from_raw_os_error(13).kind(), true),
            (io::Error::from_raw_os_error(1).kind(), true),
            (io::ErrorKind::NotFound, false),
            (io::ErrorKind::StorageFull, false),
            (io::ErrorKind::Other, false),
        ] {
            assert_eq!(refuses_writes(kind), refuses, "{kind:?}");
        }
    }
}
