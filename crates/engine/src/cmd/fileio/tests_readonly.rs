//! Varos: a write to a location that takes no file (a read-only volume, a folder without
//! permission) is [`EngineError::ReadOnlyLocation`], with the message any failed write has; other
//! failures stay [`EngineError::Other`].
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;

use super::{write_file, write_file_with};
use crate::EngineError;

#[test]
fn a_write_under_a_read_only_folder_is_a_read_only_location() {
    let d = vectorcraft_testkit::temp_dir("engine-readonly-write");
    let ro = d.join("locked");
    std::fs::create_dir_all(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    let path = ro.join("die-cut.pdf").to_string_lossy().into_owned();
    let plain = write_file(&path, b"%PDF");
    let streamed = write_file_with(&path, &mut |w: &mut dyn std::io::Write| w.write_all(b"%PDF"));
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
    if plain.is_ok() {
        // Run as root: permissions don't hold, nothing to check.
        let _ = std::fs::remove_dir_all(&d);
        return;
    }
    for r in [plain, streamed] {
        match r {
            Err(EngineError::ReadOnlyLocation(m)) => {
                assert!(m.starts_with(&format!("{path}: ")), "{m}");
                // The message reads as any failed write's: the path, then the system's reason.
                assert_eq!(EngineError::ReadOnlyLocation(m.clone()).to_string(), m);
            }
            r => panic!("not a read-only location: {r:?}"),
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_write_into_a_missing_folder_keeps_the_old_error() {
    let d = vectorcraft_testkit::temp_dir("engine-missing-write");
    let path = d.join("gone").join("Poster.varos").to_string_lossy().into_owned();
    match write_file(&path, b"art") {
        Err(EngineError::Other(m)) => {
            assert!(m.starts_with(&format!("{path}: ")), "{m}");
            // The system's own wording, as before (`No such file or directory (os error 2)`).
            assert!(m.contains("os error 2"), "{m}");
        }
        r => panic!("{r:?}"),
    }
    let _ = std::fs::remove_dir_all(&d);
}
