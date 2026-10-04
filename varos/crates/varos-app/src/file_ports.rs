//! The REAL lifecycle ports (DFS S1 §3.4, piece B): `RfdDialogs` asks with native dialogs (rfd) in
//! the exact copy of spec §4, and `DiskStore` reads/writes `.vrs` through `varos_pdf` and says which
//! file a path names (`FileKey`). `lifecycle.rs` stays free of rfd and fs; the fake ports in its
//! tests cover the rules, this file covers the plumbing.

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
fn decision_from(r: &MessageDialogResult) -> SaveDecision {
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
fn fail_choice_from(r: &MessageDialogResult) -> SaveFailChoice {
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

// First/default button is Cancel. Escape, dismissal and unknown results never discard.
fn discard_recovery_from(result: &MessageDialogResult) -> bool {
    matches!(result, MessageDialogResult::Custom(label) if label == "Discard")
}

/// Native dialogs (rfd). Blocking: the event loop waits while one is up.
pub struct RfdDialogs;

impl Dialogs for RfdDialogs {
    fn confirm_discard_recovery(&mut self, name: &str) -> bool {
        discard_recovery_from(
            &MessageDialog::new()
                .set_level(MessageLevel::Warning)
                .set_title(format!("Discard recovery copy of “{name}”?"))
                .set_description("Unsaved changes in this copy will be lost.")
                .set_buttons(MessageButtons::OkCancelCustom(CANCEL.into(), "Discard".into()))
                .show(),
        )
    }
    fn external_change(&mut self, name: &str) -> crate::lifecycle::ExternalChoice {
        use crate::lifecycle::ExternalChoice as C;
        let result = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(format!("“{name}” was changed by another app."))
            .set_description("Saving now would replace those changes.")
            .set_buttons(MessageButtons::YesNoCancelCustom(SAVE_AS.into(), "Replace Anyway".into(), CANCEL.into()))
            .show();
        match result {
            MessageDialogResult::Yes => C::SaveAs,
            MessageDialogResult::No => C::Replace,
            MessageDialogResult::Custom(ref label) if label == SAVE_AS => C::SaveAs,
            MessageDialogResult::Custom(ref label) if label == "Replace Anyway" => C::Replace,
            _ => C::Cancel,
        }
    }
    fn pick_open(&mut self) -> Vec<PathBuf> {
        FileDialog::new()
            .set_title("Open Varos Document")
            .add_filter("Varos documents (.vrs)", &["vrs"])
            .add_filter("Varos PDF documents (.pdf)", &["pdf"])
            .pick_files()
            .unwrap_or_default()
    }

    fn pick_locate(&mut self) -> Option<PathBuf> {
        FileDialog::new().set_title("Locate Varos Document").add_filter("Varos documents", &["vrs", "pdf"]).pick_file()
    }
    fn locate_missing(&mut self, path: &Path) -> bool {
        let answer = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title("This file can't be found")
            .set_description(format!("{}\nIt may have been moved or renamed.", path.display()))
            .set_buttons(MessageButtons::OkCancelCustom("Locate…".into(), CANCEL.into()))
            .show();
        matches!(answer, MessageDialogResult::Ok)
            || matches!(answer, MessageDialogResult::Custom(ref s) if s == "Locate…")
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

    fn pick_export(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf> {
        let mut d =
            FileDialog::new().set_title("Export PDF").add_filter("PDF (.pdf)", &["pdf"]).set_file_name(suggested);
        if let Some(dir) = dir {
            d = d.set_directory(dir);
        }
        d.save_file()
    }

    fn keep_waiting_for_save(&mut self, name: &str, place: &str) -> bool {
        let (title, body) = still_saving_copy(name, place);
        let r = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(title)
            .set_description(body)
            .set_buttons(MessageButtons::OkCancelCustom(KEEP_WAITING.into(), CANCEL.into()))
            .show();
        // only an explicit Keep Waiting (or Ok on plain backends) keeps waiting; anything else cancels
        matches!(r, MessageDialogResult::Ok) || matches!(r, MessageDialogResult::Custom(ref l) if l == KEEP_WAITING)
    }

    fn confirm_export_replace(&mut self, name: &str) -> bool {
        let (title, body) = export_replace_copy(name);
        let r = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(title)
            .set_description(body)
            .set_buttons(MessageButtons::OkCancelCustom(REPLACE.into(), CANCEL.into()))
            .show();
        replace_from(&r)
    }
}

const KEEP_WAITING: &str = "Keep Waiting";

/// “Varos is still saving …” — (title, body) after a Close / Save As / Quit waited 10 s for a save.
fn still_saving_copy(name: &str, place: &str) -> (String, String) {
    let place = if place.is_empty() { "the disk".to_string() } else { place.to_string() };
    (
        format!("Varos is still saving “{name}” to {place}."),
        "Keep Waiting, or Cancel to go back to the document. It stays open with unsaved changes while the save finishes; your previous file is never left half-written.".into(),
    )
}

/// “… contains an editable Varos document.” — (title, body) of the export's second confirmation.
fn export_replace_copy(name: &str) -> (String, String) {
    (
        format!("“{name}” contains an editable Varos document."),
        "Replacing it with an export removes the editable data.".into(),
    )
}

const NEWER_VAROS: &str = "It was saved by a newer version of Varos. Update Varos to open it.";
const NOT_VAROS: &str = "It isn't a Varos document, or it is damaged.";
const NOT_WRITTEN: &str = "Varos couldn't write the document.";

/// The loader/writer's internal error text as a plain sentence for the `{reason}` in the prompts.
/// File-system failures (`read failed:` / `write failed:` / `rename failed:` + the OS text) get the
/// same wording as the storage module (`io_reason`: no “(os error N)”, capitalised, “The disk is
/// full.”…); a newer-format file says so; anything else is `fallback` (a load: not a Varos
/// document or damaged — the parser's details mean nothing to the user).
fn plain_reason(raw: &str, fallback: &str) -> String {
    use std::io;
    use varos_app::storage::durable::io_reason;
    for (prefix, reading) in [("read failed: ", true), ("write failed: ", false), ("rename failed: ", false)] {
        let Some(os_text) = raw.strip_prefix(prefix) else {
            continue;
        };
        // Rebuild the real OS error from its code when the text carries one, so the wording is exact.
        let code = os_text
            .rfind(" (os error ")
            .and_then(|i| os_text[i + " (os error ".len()..].trim_end_matches(')').parse::<i32>().ok());
        let err = match code {
            Some(c) => io::Error::from_raw_os_error(c),
            None => io::Error::other(os_text.to_string()),
        };
        if reading && err.kind() == io::ErrorKind::PermissionDenied {
            return "Varos isn't allowed to read this file.".into(); // io_reason's text speaks of writing
        }
        return io_reason(&err);
    }
    if raw.contains("newer Varos") {
        return NEWER_VAROS.into();
    }
    // `SaveRefused` is already written for the user ("This document can't be saved: … It is still open.").
    if raw.starts_with("This document can't be saved:") {
        return raw.into();
    }
    fallback.into()
}

/// The disk: `.vrs` through `varos_pdf` (atomic write), identity through `file_key`. Errors come
/// back as plain sentences (`plain_reason`).
pub struct DiskStore;

impl DocStore for DiskStore {
    fn load_with_notice(&mut self, path: &Path) -> Result<(Document, Option<&'static str>), String> {
        varos_pdf::load_vrs_with_notice(path).map_err(|e| plain_reason(&e, NOT_VAROS))
    }
    fn load(&mut self, path: &Path) -> Result<Document, String> {
        varos_pdf::load_vrs(path).map_err(|e| plain_reason(&e, NOT_VAROS))
    }
    fn save(&mut self, doc: &Document, path: &Path) -> Result<crate::lifecycle::SaveOutcome, String> {
        durable_save(&varos_app::storage::durable::RealFs, doc, path, &varos_core::format::Limits::DEFAULT)
    }
    fn fingerprint(&self, path: &Path) -> Option<varos_app::storage::durable::Fingerprint> {
        varos_app::storage::durable::fingerprint(&varos_app::storage::durable::RealFs, path)
    }
    fn key(&self, path: &Path) -> FileKey {
        file_key(path)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn write_export(&mut self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        export_write(&varos_app::storage::durable::RealFs, path, bytes)
    }
    fn read_existing(&mut self, path: &Path) -> Option<Vec<u8>> {
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() || meta.len() > varos_pdf::HAS_MODEL_SCAN_CAP as u64 {
            return None;
        }
        std::fs::read(path).ok()
    }
}

/// The export's one durable replace (the same writer as Save: temp + sync + rename). A replace whose
/// folder sync could not be confirmed still delivered the PDF, so it counts as exported.
fn export_write(fs: &dyn varos_app::storage::durable::FsPort, path: &Path, bytes: &[u8]) -> Result<(), String> {
    use varos_app::storage::{checksum::new_nonce, durable::write_replace};
    write_replace(fs, path, bytes, &new_nonce()).map(|_| ()).map_err(|e| e.reason())
}

fn durable_save(
    fs: &dyn varos_app::storage::durable::FsPort,
    doc: &Document,
    path: &Path,
    limits: &varos_core::format::Limits,
) -> Result<crate::lifecycle::SaveOutcome, String> {
    use crate::lifecycle::SaveOutcome;
    use varos_app::storage::{
        checksum::new_nonce,
        durable::{io_reason, write_replace, WriteOutcome},
    };
    // A1: decided BEFORE anything replaces the file — a save never produces a file Varos later refuses.
    // Cheap: writer-side object/token counts against the reader's own limits; the full reopen decode
    // runs only within 10 % of a limit (`varos_pdf::write_pdf_checked_report`).
    let bytes = varos_pdf::write_pdf_checked(doc, limits).map_err(|e| plain_reason(&e, NOT_WRITTEN))?;
    match write_replace(fs, path, &bytes, &new_nonce()).map_err(|e| e.reason())? {
        WriteOutcome::Durable => {
            cleanup_stale_save_temps(fs, path);
            Ok(SaveOutcome::Durable)
        }
        WriteOutcome::ReplacedUnconfirmed(e) => Ok(SaveOutcome::ReplacedUnconfirmed(io_reason(&e))),
    }
}

/// Remove only this destination's unambiguous, old temp files after confirmed Save.
/// A day of grace avoids racing another live writer; long/truncated names are deliberately skipped.
fn cleanup_stale_save_temps(fs: &dyn varos_app::storage::durable::FsPort, path: &Path) {
    let Ok(path) = fs.resolve_link(path) else {
        return;
    };
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    // A UTF-8 truncation can end up to three bytes before the limit.
    if name.len() >= varos_app::storage::durable::TEMP_NAME_MAX_BYTES - 3 {
        return;
    }
    let prefix = format!(".{name}.");
    let Ok(entries) = fs.read_dir(path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."))) else {
        return;
    };
    for entry in entries {
        let Some(nonce) = entry
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix(&prefix))
            .and_then(|n| n.strip_suffix(".varos-tmp"))
        else {
            continue;
        };
        if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        // Never follow a symlink during cleanup.
        if fs.resolve_link(&entry).is_ok_and(|target| target == entry)
            && fs.metadata(&entry).is_ok_and(|m| {
                !m.is_dir
                    && m.modified
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age >= std::time::Duration::from_secs(86400))
            })
        {
            let _ = fs.remove_file(&entry);
        }
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
        let name_id = canon.parent().zip(canon.file_name()).and_then(|(dir, name)| {
            let m = std::fs::metadata(dir).ok()?;
            Some(((m.dev(), m.ino()), volume_name(&name.to_string_lossy(), case_sensitive(dir))))
        });
        FileKey { path: canon, dev_ino, name_id }
    }
    #[cfg(not(unix))]
    {
        // Windows volumes compare case-insensitively: the whole absolute path, folded (compile-only).
        let name_id = Some(((0, 0), volume_name(&abs.to_string_lossy(), Some(false))));
        FileKey { path: abs, dev_ino: None, name_id }
    }
}

/// A file name the way a volume compares it: NFC-normalised (APFS / HFS+ ignore normalisation), and
/// case-folded unless the volume is KNOWN to be case-sensitive. Unknown counts as case-insensitive:
/// refusing a second file that differs only by case is the safe side.
pub fn volume_name(name: &str, case_sensitive: Option<bool>) -> String {
    use unicode_normalization::UnicodeNormalization;
    let nfc: String = name.nfc().collect();
    if case_sensitive == Some(true) {
        nfc
    } else {
        // Full case FOLDING, not just lowercasing: upper then lower maps the pairs lowercasing
        // alone keeps apart (Greek σ/ς, ß/SS…), so two spellings a case-insensitive volume treats
        // as one entry always get one key (over-matching is the safe side).
        nfc.to_uppercase().to_lowercase().nfc().collect()
    }
}

/// Does the volume holding `dir` compare names case-sensitively? macOS asks the volume
/// (`pathconf(_PC_CASE_SENSITIVE)`); elsewhere, or when it cannot answer: unknown.
#[cfg(target_os = "macos")]
pub fn case_sensitive(dir: &Path) -> Option<bool> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    // SAFETY: `c` is a valid NUL-terminated path for the duration of the call; pathconf only reads it.
    let r = unsafe { libc::pathconf(c.as_ptr(), libc::_PC_CASE_SENSITIVE) };
    (r >= 0).then_some(r > 0)
}
#[cfg(all(unix, not(target_os = "macos")))]
pub fn case_sensitive(_dir: &Path) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rfd::MessageDialogResult as R;
    use varos_core::editor::Editor;
    use varos_core::format::Limits;
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
            save_failed_copy("Logo.vrs", "Varos isn't allowed to write there."),
            (
                "Couldn't save “Logo.vrs”.".into(),
                "Your changes are still open. Varos isn't allowed to write there.".into()
            )
        );
        assert_eq!(
            open_failed_copy("x.vrs", NOT_VAROS),
            (
                "Couldn't open “x.vrs”.".into(),
                "It isn't a Varos document, or it is damaged. Your open documents have not changed.".into()
            )
        );
    }

    #[test]
    fn plain_reason_hides_internal_error_text() {
        // file-system failures: no prefix, no "(os error N)", a capitalised sentence (io_reason)
        let enoent = std::io::Error::from_raw_os_error(2).to_string(); // the OS's own wording + code
        let plain = enoent[..enoent.rfind(" (os error ").unwrap()].to_string();
        let expect = format!("{}{}.", plain[..1].to_uppercase(), &plain[1..]);
        assert_eq!(plain_reason(&format!("read failed: {enoent}"), NOT_VAROS), expect);
        assert_eq!(
            plain_reason("write failed: Permission denied (os error 13)", NOT_WRITTEN),
            "Varos isn't allowed to write there."
        );
        assert_eq!(
            plain_reason("read failed: Permission denied (os error 13)", NOT_VAROS),
            "Varos isn't allowed to read this file."
        );
        assert_eq!(plain_reason("rename failed: something odd", NOT_WRITTEN), "Something odd.");
        // a newer format says so; any parser detail is replaced by one plain sentence
        assert_eq!(
            plain_reason("this file was saved by a newer Varos (v3) — please update", NOT_VAROS),
            "It was saved by a newer version of Varos. Update Varos to open it."
        );
        for raw in [
            "not a valid .vrs model: expected value at line 1 column 1",
            "not a valid .vrs",
            "not a readable PDF: invalid file header",
        ] {
            assert_eq!(plain_reason(raw, NOT_VAROS), "It isn't a Varos document, or it is damaged.", "{raw}");
        }
        assert_eq!(plain_reason("serialize failed: x", NOT_WRITTEN), "Varos couldn't write the document.");
    }

    #[test]
    fn frozen_v1_broken_mask_saves_as_current_format_and_reopens_clean_without_notice() {
        use varos_app::storage::durable::RealFs;
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v2/v1_broken_mask.vrs");
        let dir = Scratch::new("v1-repair");
        let path = dir.0.join("old.vrs");
        std::fs::copy(&fixture, &path).unwrap();
        let (doc, notice) = DiskStore.load_with_notice(&path).unwrap();
        assert_eq!(notice, Some(varos_core::format::RELEASED_MASKS_NOTICE));
        durable_save(&RealFs, &doc, &path, &Limits::DEFAULT).unwrap();
        let loaded = varos_pdf::load_vrs_checked(&path, &Limits::DEFAULT).unwrap();
        assert_eq!(loaded.source_version, varos_core::format::FORMAT_VERSION);
        assert_eq!(loaded.notice(), None);
        assert_eq!(loaded.doc, doc);
    }

    #[test]
    fn disk_store_keeps_legacy_repair_notice_and_original_bytes() {
        use varos_core::model::GroupRole;
        let dir = Scratch::new("legacy-notice");
        let path = dir.0.join("old.vrs");
        let mut doc = doc_with_art();
        let pid = doc.paths[0].id;
        let mut second = doc.paths[0].clone();
        second.id = pid + 1;
        doc.paths.push(second);
        let group = doc.group(&[pid, pid + 1]).unwrap();
        let node = doc.nodes.iter_mut().find(|n| n.id == group).unwrap();
        node.role = GroupRole::Clip;
        node.mask_child = Some(99999);
        let mut v1 = serde_json::json!({"varos":1,"doc":doc});
        for key in ["name", "description", "tags"] {
            v1["doc"].as_object_mut().unwrap().remove(key); // a v1 writer never emitted the board keys
        }
        let bytes = serde_json::to_vec(&v1).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let (opened, notice) = DiskStore.load_with_notice(&path).unwrap();
        assert!(notice.unwrap().contains("broken clipping masks released"));
        assert_eq!(opened.node(group).unwrap().role, GroupRole::Normal);
        assert_eq!(opened.paths, doc.paths);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
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
        assert_eq!((&new_key.path, new_key.dev_ino), (&canon_dir.join("Logo.vrs"), None));
        assert!(new_key.name_id.is_some(), "a new file is identified by its folder + volume-compared name");
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
        // failures come back as plain sentences, never panics or raw internal text
        let missing = store.load(&dir.0.join("missing.vrs")).unwrap_err();
        assert!(!missing.contains("failed") && !missing.contains("os error") && missing.ends_with('.'), "{missing}");
        std::fs::write(dir.0.join("junk.vrs"), b"not a varos file").unwrap();
        assert_eq!(store.load(&dir.0.join("junk.vrs")).unwrap_err(), NOT_VAROS);
        let no_dir = store.save(&doc, &dir.0.join("no-such-folder").join("x.vrs")).unwrap_err();
        assert!(!no_dir.contains("failed") && !no_dir.contains("os error") && no_dir.ends_with('.'), "{no_dir}");
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
    #[test]
    fn durable_save_faults_preserve_old_bytes_and_report_unconfirmed() {
        use crate::lifecycle::SaveOutcome;
        use varos_app::storage::durable::{Fault, FaultFs, Step};
        let dir = Scratch::new("durable-faults");
        let path = dir.0.join("design.vrs");
        for step in [Step::Write { after: 7 }, Step::Sync, Step::Rename] {
            std::fs::write(&path, b"old bytes").unwrap();
            assert!(
                durable_save(&FaultFs::new(vec![Fault::at(step)]), &doc_with_art(), &path, &Limits::DEFAULT).is_err()
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"old bytes");
        }
        let result =
            durable_save(&FaultFs::new(vec![Fault::at(Step::SyncDir)]), &doc_with_art(), &path, &Limits::DEFAULT)
                .unwrap();
        assert!(matches!(result, SaveOutcome::ReplacedUnconfirmed(_)));
        assert!(varos_pdf::load_vrs(&path).is_ok());
    }

    #[test]
    fn a_save_the_reader_would_refuse_fails_before_touching_the_file() {
        use varos_app::storage::durable::RealFs;
        let dir = Scratch::new("save-budget");
        let path = dir.0.join("design.vrs");
        std::fs::write(&path, b"old bytes").unwrap();
        let tight = Limits { max_pdf_objects: 3, ..Limits::DEFAULT };
        let err = durable_save(&RealFs, &doc_with_art(), &path, &tight).unwrap_err();
        assert!(err.starts_with("This document can't be saved:") && err.contains("limit"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"old bytes", "the user's file is untouched");
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1, "no temp file left behind");
        // Under the shipped limits the same document saves and reopens.
        durable_save(&RealFs, &doc_with_art(), &path, &Limits::DEFAULT).unwrap();
        assert!(varos_pdf::load_vrs(&path).is_ok());
    }

    #[test]
    fn successful_save_cleans_only_old_exact_destination_temps() {
        use varos_app::storage::durable::{temp_path, RealFs};
        let dir = Scratch::new("stale-temps");
        let path = dir.0.join("design.vrs");
        let old = temp_path(&path, &"a".repeat(32));
        let live = temp_path(&path, &"b".repeat(32));
        let other = temp_path(&dir.0.join("other.vrs"), &"a".repeat(32));
        let unknown = dir.0.join(".design.vrs.not-a-nonce.varos-tmp");
        for p in [&old, &live, &other, &unknown] {
            std::fs::write(p, b"keep").unwrap();
        }
        let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(172800);
        for p in [&old, &other, &unknown] {
            std::fs::File::options().write(true).open(p).unwrap().set_modified(old_time).unwrap();
        }
        durable_save(&RealFs, &doc_with_art(), &path, &Limits::DEFAULT).unwrap();
        assert!(!old.exists());
        for p in [&live, &other, &unknown] {
            assert_eq!(std::fs::read(p).unwrap(), b"keep");
        }
    }
    #[test]
    fn discard_only_accepts_the_explicit_custom_button() {
        for answer in [
            MessageDialogResult::Ok,
            MessageDialogResult::Cancel,
            MessageDialogResult::Yes,
            MessageDialogResult::No,
            MessageDialogResult::Custom("Cancel".into()),
        ] {
            assert!(!discard_recovery_from(&answer));
        }
        assert!(discard_recovery_from(&MessageDialogResult::Custom("Discard".into())));
    }

    #[test]
    fn volume_name_folds_case_unless_the_volume_is_known_case_sensitive_and_always_normalises() {
        let (nfc, nfd) = ("caf\u{e9}.vrs", "cafe\u{301}.vrs");
        assert_eq!(volume_name(nfc, Some(true)), volume_name(nfd, Some(true)), "NFC = NFD, any volume");
        assert_eq!(volume_name("Logo.vrs", Some(true)), "Logo.vrs", "a case-sensitive volume keeps case");
        assert_eq!(volume_name("Logo.vrs", Some(false)), "logo.vrs");
        assert_eq!(volume_name("Logo.vrs", None), "logo.vrs", "unknown counts as case-insensitive (safe side)");
        assert_eq!(volume_name("CAF\u{c9}.vrs", None), volume_name(nfd, None));
        // full folding, not lowercasing: final sigma and sharp s fold with their pairs
        assert_eq!(volume_name("\u{3c3}\u{3c2}.vrs", None), volume_name("\u{3a3}\u{3a3}.vrs", None), "σς = ΣΣ");
        assert_eq!(volume_name("stra\u{df}e.vrs", None), volume_name("STRASSE.vrs", None), "ß = SS");
    }

    #[cfg(unix)]
    #[test]
    fn not_yet_existing_names_compare_the_way_their_volume_does() {
        let dir = Scratch::new("case-key");
        let (upper, lower) = (file_key(&dir.0.join("A.vrs")), file_key(&dir.0.join("a.vrs")));
        assert!(upper.dev_ino.is_none() && lower.dev_ino.is_none(), "neither file exists yet");
        let canon = std::fs::canonicalize(&dir.0).unwrap();
        let insensitive = case_sensitive(&canon) != Some(true);
        assert_eq!(upper.same_file(&lower), insensitive, "A.vrs vs a.vrs follows the volume");
        // NFC vs NFD spellings of one name are one file on every Mac volume
        let nfc = file_key(&dir.0.join("caf\u{e9}.vrs"));
        let nfd = file_key(&dir.0.join("cafe\u{301}.vrs"));
        assert!(nfc.same_file(&nfd));
        // the same name in another folder is another file
        std::fs::create_dir_all(dir.0.join("other")).unwrap();
        assert!(!upper.same_file(&file_key(&dir.0.join("other").join("A.vrs"))));
        assert!(!upper.same_file(&file_key(&dir.0.join("other").join("a.vrs"))));
    }
}
