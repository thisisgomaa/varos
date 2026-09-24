//! The REAL lifecycle ports (DFS S1 §3.4, piece B): `RfdDialogs` asks with native dialogs (rfd) in
//! the exact copy of spec §4, and `DiskStore` reads/writes `.vrs` through `varos_pdf` and says which
//! file a path names (`FileKey`). `lifecycle.rs` stays free of rfd and fs; the fake ports in its
//! tests cover the rules, this file covers the plumbing.
// S1-B: nothing calls this module until S1-D wires the host; S1-D removes this allow (as in the
// three S1-A modules).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use varos_core::model::Document;

use crate::lifecycle::{Dialogs, DocStore, SaveDecision, SaveFailChoice};
use crate::workspace::FileKey;

// Button labels (spec §4). Custom labels come back from rfd as `Custom(label)`.
const SAVE: &str = "Save";
const DONT_SAVE: &str = "Don't Save";
const CANCEL: &str = "Cancel";
const TRY_AGAIN: &str = "Try Again";
const SAVE_AS: &str = "Save As…";
const REPLACE: &str = "Replace";

/// Map the “Save changes?” result onto a decision (batch 1's `quit_answer`, moved here). Backends
/// that only know Yes/No/Cancel map the same way. Anything unknown (dismissed, Escape) is Cancel —
/// the safe answer, never a discard.
pub fn decision_from(r: &MessageDialogResult) -> SaveDecision {
    use MessageDialogResult as R;
    match r {
        R::Yes => SaveDecision::Save,
        R::No => SaveDecision::DontSave,
        R::Custom(l) if l == SAVE => SaveDecision::Save,
        R::Custom(l) if l == DONT_SAVE => SaveDecision::DontSave,
        _ => SaveDecision::Cancel,
    }
}

/// Map the “Couldn't save” result onto a choice; anything unknown is Cancel.
pub fn fail_choice_from(r: &MessageDialogResult) -> SaveFailChoice {
    use MessageDialogResult as R;
    match r {
        R::Yes => SaveFailChoice::TryAgain,
        R::No => SaveFailChoice::SaveAs,
        R::Custom(l) if l == TRY_AGAIN => SaveFailChoice::TryAgain,
        R::Custom(l) if l == SAVE_AS => SaveFailChoice::SaveAs,
        _ => SaveFailChoice::Cancel,
    }
}

/// Map the “Replace?” result; only an explicit Replace (or Ok/Yes on plain backends) replaces.
fn replace_from(r: &MessageDialogResult) -> bool {
    use MessageDialogResult as R;
    matches!(r, R::Ok | R::Yes) || matches!(r, R::Custom(l) if l == REPLACE)
}

/// “Save changes to “name”?” — (title, body). During Quit the body ends with “Document i of n”.
fn save_changes_copy(name: &str, progress: Option<(usize, usize)>) -> (String, String) {
    let mut body = String::from("Your changes will be lost if you don't save them.");
    if let Some((i, n)) = progress {
        body.push_str(&format!("\n\nDocument {i} of {n}"));
    }
    (format!("Save changes to “{name}”?"), body)
}

/// “Couldn't save “name”.” — (title, body).
fn save_failed_copy(name: &str, reason: &str) -> (String, String) {
    (format!("Couldn't save “{name}”."), format!("Your changes are still open. {}", sentence(reason)))
}

/// “Couldn't open “name”.” — (title, body).
fn open_failed_copy(name: &str, reason: &str) -> (String, String) {
    (format!("Couldn't open “{name}”."), format!("{} Your open documents have not changed.", sentence(reason)))
}

/// A reason as one sentence: trimmed, ending in exactly one full stop.
fn sentence(reason: &str) -> String {
    let r = reason.trim().trim_end_matches('.');
    if r.is_empty() {
        "Something went wrong.".into()
    } else {
        format!("{r}.")
    }
}

/// Native dialogs (rfd). Blocking: the event loop waits while one is up.
pub struct RfdDialogs;

impl Dialogs for RfdDialogs {
    fn pick_open(&mut self) -> Vec<PathBuf> {
        FileDialog::new()
            .set_title("Open Varos Document")
            .add_filter("Varos documents (.vrs)", &["vrs"])
            .add_filter("Varos PDF documents (.pdf)", &["pdf"])
            .pick_files()
            .unwrap_or_default()
    }

    fn pick_save(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf> {
        let mut d = FileDialog::new()
            .set_title("Save Varos Document")
            .add_filter("Varos document (.vrs)", &["vrs"])
            .set_file_name(suggested);
        if let Some(dir) = dir {
            d = d.set_directory(dir);
        }
        d.save_file()
    }

    fn ask_save_changes(&mut self, name: &str, progress: Option<(usize, usize)>) -> SaveDecision {
        let (title, body) = save_changes_copy(name, progress);
        let r = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(title)
            .set_description(body)
            .set_buttons(MessageButtons::YesNoCancelCustom(SAVE.into(), DONT_SAVE.into(), CANCEL.into()))
            .show();
        decision_from(&r)
    }

    fn save_failed(&mut self, name: &str, reason: &str) -> SaveFailChoice {
        let (title, body) = save_failed_copy(name, reason);
        let r = MessageDialog::new()
            .set_level(MessageLevel::Error)
            .set_title(title)
            .set_description(body)
            .set_buttons(MessageButtons::YesNoCancelCustom(TRY_AGAIN.into(), SAVE_AS.into(), CANCEL.into()))
            .show();
        fail_choice_from(&r)
    }

    fn open_failed(&mut self, name: &str, reason: &str) {
        let (title, body) = open_failed_copy(name, reason);
        MessageDialog::new()
            .set_level(MessageLevel::Error)
            .set_title(title)
            .set_description(body)
            .set_buttons(MessageButtons::Ok)
            .show();
    }

    fn confirm_replace(&mut self, name: &str) -> bool {
        let r = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(format!("“{name}” already exists. Do you want to replace it?"))
            .set_description("Replacing it will overwrite its current contents.")
            .set_buttons(MessageButtons::OkCancelCustom(REPLACE.into(), CANCEL.into()))
            .show();
        replace_from(&r)
    }

    fn notice(&mut self, title: &str, body: &str) {
        MessageDialog::new()
            .set_level(MessageLevel::Info)
            .set_title(title)
            .set_description(body)
            .set_buttons(MessageButtons::Ok)
            .show();
    }
}

/// The disk: `.vrs` through `varos_pdf` (atomic write), identity through `file_key`.
pub struct DiskStore;

impl DocStore for DiskStore {
    fn load(&mut self, path: &Path) -> Result<Document, String> {
        varos_pdf::load_vrs(path)
    }
    fn save(&mut self, doc: &Document, path: &Path) -> Result<(), String> {
        varos_pdf::save_vrs(doc, path)
    }
    fn key(&self, path: &Path) -> FileKey {
        file_key(path)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

/// Which file `path` names (DFS S1 F1 / spec §2 identity): the absolute path with symlinks resolved
/// (for a file that does not exist yet: its folder resolved + its name), plus device/inode on unix so
/// an alias the path cannot see (a case variant on APFS, a hard link) still matches a live file.
/// Compare keys with `FileKey::same_file` (path first: every save replaces the inode).
///
/// Windows: the absolute path only, not canonicalised (`canonicalize` returns `\\?\` verbatim paths
/// that would leak into tab tooltips) and no file id — until S4. Windows is a compile-only target.
pub fn file_key(path: &Path) -> FileKey {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let canon = std::fs::canonicalize(&abs).unwrap_or_else(|_| match (abs.parent(), abs.file_name()) {
            (Some(dir), Some(name)) => std::fs::canonicalize(dir).map(|d| d.join(name)).unwrap_or_else(|_| abs.clone()),
            _ => abs.clone(),
        });
        let dev_ino = std::fs::metadata(&canon).ok().map(|m| (m.dev(), m.ino()));
        FileKey { path: canon, dev_ino }
    }
    #[cfg(not(unix))]
    {
        FileKey { path: abs, dev_ino: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rfd::MessageDialogResult as R;
    use varos_core::editor::Editor;
    use varos_core::model::{Anchor, Artboard, Path as VPath};

    /// A scratch directory under the system temp dir, removed on drop.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("varos-s1b-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("create scratch dir");
            Scratch(p)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn doc_with_art() -> Document {
        let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
        let sq = VPath::new(
            10,
            vec![a(100, [20.0, 20.0]), a(101, [50.0, 20.0]), a(102, [50.0, 50.0]), a(103, [20.0, 50.0])],
            true,
            Some([1.0, 0.0, 0.0, 1.0]),
            None,
            1.0,
        );
        let doc = Document {
            artboards: vec![Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, name: "A".into(), ..Artboard::default() }],
            paths: vec![sq],
            ids: 200,
            ..Document::default()
        };
        let mut ed = Editor::new();
        ed.replace_doc(doc); // what a session holds (post `sync_tree`)
        ed.doc
    }

    #[test]
    fn decision_from_dialog_results_and_unknown_is_cancel() {
        assert_eq!(decision_from(&R::Custom(SAVE.into())), SaveDecision::Save);
        assert_eq!(decision_from(&R::Custom(DONT_SAVE.into())), SaveDecision::DontSave);
        assert_eq!(decision_from(&R::Custom(CANCEL.into())), SaveDecision::Cancel);
        assert_eq!(decision_from(&R::Yes), SaveDecision::Save);
        assert_eq!(decision_from(&R::No), SaveDecision::DontSave);
        assert_eq!(decision_from(&R::Cancel), SaveDecision::Cancel);
        assert_eq!(decision_from(&R::Ok), SaveDecision::Cancel);
        assert_eq!(decision_from(&R::Custom("Something else".into())), SaveDecision::Cancel);
        // “Couldn't save”
        assert_eq!(fail_choice_from(&R::Custom(TRY_AGAIN.into())), SaveFailChoice::TryAgain);
        assert_eq!(fail_choice_from(&R::Custom(SAVE_AS.into())), SaveFailChoice::SaveAs);
        assert_eq!(fail_choice_from(&R::Custom(CANCEL.into())), SaveFailChoice::Cancel);
        assert_eq!(fail_choice_from(&R::Yes), SaveFailChoice::TryAgain);
        assert_eq!(fail_choice_from(&R::No), SaveFailChoice::SaveAs);
        assert_eq!(fail_choice_from(&R::Cancel), SaveFailChoice::Cancel);
        assert_eq!(fail_choice_from(&R::Custom("Something else".into())), SaveFailChoice::Cancel);
        // “Replace?”
        assert!(replace_from(&R::Custom(REPLACE.into())) && replace_from(&R::Ok));
        assert!(!replace_from(&R::Custom(CANCEL.into())) && !replace_from(&R::Cancel));
    }

    #[test]
    fn prompt_copy_follows_the_spec() {
        assert_eq!(
            save_changes_copy("Logo.vrs", None),
            ("Save changes to “Logo.vrs”?".into(), "Your changes will be lost if you don't save them.".into())
        );
        assert_eq!(
            save_changes_copy("Untitled-2", Some((1, 2))).1,
            "Your changes will be lost if you don't save them.\n\nDocument 1 of 2"
        );
        assert_eq!(
            save_failed_copy("Logo.vrs", "write failed: permission denied"),
            (
                "Couldn't save “Logo.vrs”.".into(),
                "Your changes are still open. write failed: permission denied.".into()
            )
        );
        assert_eq!(
            open_failed_copy("x.vrs", "not a valid .vrs."),
            ("Couldn't open “x.vrs”.".into(), "not a valid .vrs. Your open documents have not changed.".into())
        );
    }

    #[test]
    fn disk_store_round_trips_through_varos_pdf() {
        let dir = Scratch::new("roundtrip");
        let path = dir.0.join("Logo.vrs");
        let doc = doc_with_art();
        let mut store = DiskStore;
        assert!(!store.exists(&path));
        // a file that does not exist yet: its folder is resolved, no inode
        let new_key = store.key(&path);
        #[cfg(unix)]
        let canon_dir = std::fs::canonicalize(&dir.0).unwrap();
        #[cfg(not(unix))]
        let canon_dir = std::path::absolute(&dir.0).unwrap(); // Windows: the absolute path only (S4)
        assert_eq!(new_key, FileKey { path: canon_dir.join("Logo.vrs"), dev_ino: None });
        store.save(&doc, &path).expect("save");
        assert!(store.exists(&path));
        let back = store.load(&path).expect("load");
        let mut ed = Editor::new();
        ed.replace_doc(back);
        assert!(ed.doc.content_eq(&doc), "what was saved is what loads");
        let key = store.key(&path);
        assert!(key.same_file(&new_key), "the key before the first save names the same file");
        assert_eq!(key.path, canon_dir.join("Logo.vrs"));
        #[cfg(unix)]
        assert!(key.dev_ino.is_some(), "an existing file carries its device/inode");
        // a relative spelling of the same file (via `..`) is the same file
        let dotted = dir.0.join("sub").join("..").join("Logo.vrs");
        std::fs::create_dir_all(dir.0.join("sub")).unwrap();
        assert!(store.key(&dotted).same_file(&key));
        // failures come back as errors, never panics
        assert!(store.load(&dir.0.join("missing.vrs")).is_err());
        std::fs::write(dir.0.join("junk.vrs"), b"not a varos file").unwrap();
        assert!(store.load(&dir.0.join("junk.vrs")).is_err());
        assert!(store.save(&doc, &dir.0.join("no-such-folder").join("x.vrs")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn disk_store_key_sees_symlink_alias() {
        let dir = Scratch::new("alias");
        let real = dir.0.join("real.vrs");
        let mut store = DiskStore;
        store.save(&doc_with_art(), &real).expect("save");
        let link = dir.0.join("alias.vrs");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let (k_real, k_link) = (store.key(&real), store.key(&link));
        assert!(k_link.same_file(&k_real), "a symlink names the same file");
        assert_eq!(k_link.path, k_real.path, "…and resolves to the real path (a Save replaces the file, not the link)");
        // a save through the real path swaps the inode; the alias still matches a FRESH key
        store.save(&doc_with_art(), &real).expect("save again");
        let k_after = store.key(&real);
        assert_ne!(k_after.dev_ino, k_real.dev_ino, "the atomic save replaced the inode");
        assert!(store.key(&link).same_file(&k_after));
        assert!(k_real.same_file(&k_after), "the old key still names the file by its path");
        // a hard link: another path, only the inode can tell
        let hard = dir.0.join("hard.vrs");
        std::fs::hard_link(&real, &hard).unwrap();
        let k_hard = store.key(&hard);
        assert_ne!(k_hard.path, k_after.path);
        assert!(k_hard.same_file(&k_after), "a hard link is the same file by device/inode");
        assert!(!store.key(&dir.0.join("other.vrs")).same_file(&k_after));
    }
}
