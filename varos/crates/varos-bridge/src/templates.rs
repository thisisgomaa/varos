//! Template folder convention and exclusive atomic publication, shared by CLI and desktop Bridge.
use crate::Error;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub fn path(name: &str) -> Result<PathBuf, Error> {
    let leaf = Path::new(name);
    if leaf.components().count() != 1
        || name.starts_with('.')
        || name.contains(['/', '\\'])
        || leaf.extension().is_none_or(|e| e != "vrs")
    {
        return Err(Error::new("invalid_argument", "template must be a plain NAME.vrs"));
    }
    Ok(crate::storage_paths::data_root()
        .ok_or_else(|| Error::new("io_error", "app data unavailable"))?
        .join("Templates")
        .join(leaf))
}
pub fn save_new(doc: &varos_core::model::Document, dest: &Path) -> Result<(), Error> {
    save_new_cancellable(doc, dest, &std::sync::atomic::AtomicBool::new(false))
}
pub fn save_new_cancellable(
    doc: &varos_core::model::Document,
    dest: &Path,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), Error> {
    let check = || {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            Err(Error::new("cancelled", "template job cancelled"))
        } else {
            Ok(())
        }
    };
    check()?;
    let bytes = varos_core::file::doc_to_blob(doc).map_err(|e| Error::new("invalid_argument", e))?;
    check()?;
    let folder = dest.parent().ok_or_else(|| Error::new("invalid_argument", "template folder missing"))?;
    std::fs::create_dir_all(folder).map_err(|e| Error::new("io_error", e.to_string()))?;
    let temp = folder.join(format!(".template-{}-{}.tmp", std::process::id(), crate::conn::random_hex(16)?));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
        file.write_all(bytes.as_bytes())?;
        file.sync_all()?;
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "template job cancelled"));
        }
        // Publication refuses both an existing file and a symlink, including a race.
        std::fs::hard_link(&temp, dest)
    })();
    let _ = std::fs::remove_file(&temp);
    result.map_err(|e| {
        Error::new(
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "save_conflict"
            } else if e.kind() == std::io::ErrorKind::Interrupted {
                "cancelled"
            } else {
                "io_error"
            },
            e.to_string(),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_is_normal_vrs_and_never_overwrites() {
        let folder = std::env::temp_dir().join(format!("varos-template-test-{}", crate::conn::random_hex(8).unwrap()));
        let dest = folder.join("test.vrs");
        let doc = varos_core::model::Document::default();
        save_new(&doc, &dest).unwrap();
        assert_eq!(varos_core::file::load_vrs(&dest).unwrap(), doc);
        assert_eq!(save_new(&doc, &dest).unwrap_err().code, "save_conflict");
        std::fs::remove_dir_all(folder).unwrap();
        for name in ["../escape.vrs", ".hidden.vrs", "a/b.vrs", "a\\b.vrs", "wrong.pdf"] {
            assert!(path(name).is_err());
        }
    }
}
