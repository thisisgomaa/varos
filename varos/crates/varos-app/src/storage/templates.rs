//! Adapted from VectorCraft engine command conventions @ a469568 (MIT OR Apache-2.0).
//! Copyright (c) 2026 ArtCraft Team and the VectorCraft contributors.
//! ADAPT: VectorCraft engine cmd/fileio/save.rs:56 and load.rs:281 (a469568).
//! Ordinary .vrs files in a conventional folder; no format flag. See NOTICE.
use std::path::{Path, PathBuf};
pub fn list(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = std::fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrs")))
        .collect::<Vec<_>>();
    paths.sort();
    paths.truncate(200);
    Ok(paths)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::testdir::TestDir;
    #[test]
    fn folder_convention() {
        let d = TestDir::new("templates");
        std::fs::write(d.join("one.vrs"), b"vrs").unwrap();
        std::fs::write(d.join("other.txt"), b"txt").unwrap();
        assert_eq!(list(&d.join("")).unwrap(), vec![d.join("one.vrs")]);
    }
}
