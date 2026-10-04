//! The document-lifecycle coordinator (DFS S1 §3.4): New / Open / Save / Save As / Close / Quit and
//! tab switching, run over the `Workspace` through two PORTS — `Dialogs` (every question the user is
//! asked) and `DocStore` (every file read/write). The host plugs in the real rfd + disk ports
//! (`file_ports.rs`); tests plug in scripted fakes. No rfd, fs or egui in this file.
//!
//! API frozen by S1-A; the rule bodies of `Lifecycle::run` are S1-B's.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use varos_app::storage::recents::BoardSummary;
use varos_core::model::Document;

use crate::app_command::{AppCommand, SessionId};
use crate::file_jobs::{self, ExportDone, ExportJob, ExportResult, FileDone, FileJob, SaveDone, SaveInFlight, SaveJob};
use crate::workspace::{FileKey, Workspace};

/// The answer to “Save changes to “name”?”. Escape / closing the prompt = `Cancel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveDecision {
    Save,
    DontSave,
    Cancel,
}

/// The answer to “Couldn't save “name”.”.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFailChoice {
    TryAgain,
    SaveAs,
    Cancel,
}

/// Every question the lifecycle asks the user. Blocking; the host implements it with native
/// dialogs, tests with a scripted queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Durable,
    ReplacedUnconfirmed(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalChoice {
    SaveAs,
    Replace,
    Cancel,
}

pub trait Dialogs {
    fn confirm_discard_recovery(&mut self, _name: &str) -> bool {
        false
    }
    fn external_change(&mut self, _name: &str) -> ExternalChoice {
        ExternalChoice::Cancel
    }
    /// The Open dialog (multi-select). Empty = cancelled.
    fn pick_open(&mut self) -> Vec<PathBuf>;
    fn pick_locate(&mut self) -> Option<PathBuf> {
        self.pick_open().into_iter().next()
    }
    fn locate_missing(&mut self, _path: &Path) -> bool {
        false
    }
    /// The Save dialog, pre-filled with `suggested` in `dir`. `None` = cancelled.
    fn pick_save(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf>;
    /// “Save changes to “name”?” — `progress` = `Some((i, n))` during Quit (“Document i of n”).
    fn ask_save_changes(&mut self, name: &str, progress: Option<(usize, usize)>) -> SaveDecision;
    /// “Couldn't save “name”.” with the reason.
    fn save_failed(&mut self, name: &str, reason: &str) -> SaveFailChoice;
    /// “Couldn't open “name”.” with the reason.
    fn open_failed(&mut self, name: &str, reason: &str);
    /// “Replace “name”?” — asked only when WE changed the chosen name (appended `.vrs`) and that
    /// file exists; the Save dialog already asked about the name the user typed.
    fn confirm_replace(&mut self, name: &str) -> bool;
    /// A plain notice (e.g. “… is open in another tab…”).
    fn notice(&mut self, title: &str, body: &str);
    /// The Export PDF save panel, pre-filled with `suggested` in `dir`. `None` = cancelled.
    fn pick_export(&mut self, _suggested: &str, _dir: Option<&Path>) -> Option<PathBuf> {
        None
    }
    /// “… contains an editable Varos document. Replacing it with an export removes the editable
    /// data.” — `true` only for an explicit Replace.
    fn confirm_export_replace(&mut self, _name: &str) -> bool {
        false
    }
    /// “Varos is still saving “name” to place.” — Keep Waiting (`true`) or Cancel. Asked when a
    /// Close / Save As / Quit has waited [`crate::host::SAVE_WAIT_ASK`] for an in-flight save.
    fn keep_waiting_for_save(&mut self, _name: &str, _place: &str) -> bool {
        true
    }
}

/// Every file operation the lifecycle performs.
pub trait DocStore {
    fn load(&mut self, path: &Path) -> Result<Document, String>;
    /// Additive notice seam; existing stores need not produce migration notices.
    fn load_with_notice(&mut self, path: &Path) -> Result<(Document, Option<&'static str>), String> {
        self.load(path).map(|doc| (doc, None))
    }
    fn save(&mut self, doc: &Document, path: &Path) -> Result<SaveOutcome, String>;
    /// The file's identity: absolute + canonical path (the parent canonicalised for a file that does
    /// not exist yet), plus device/inode on unix.
    fn key(&self, path: &Path) -> FileKey;
    fn exists(&self, path: &Path) -> bool;
    fn fingerprint(&self, _path: &Path) -> Option<varos_app::storage::durable::Fingerprint> {
        None
    }
    /// Only successful lifecycle outcomes reach Recent. Default preserves small fake stores.
    /// `board` = the summary of the document as it now is ON DISK (just loaded or just written), which
    /// Recent caches so Home never parses files; `None` = nothing new was read or written (an open that
    /// only focused an already-open tab), so the cached summary stays.
    fn remember(&mut self, _path: &Path, _relocated_from: Option<&Path>, _board: Option<&BoardSummary>) {}
    fn remove_recent(&mut self, _path: &Path) {}
    fn clear_recent(&mut self) {}
    /// Replace `path` with the exported PDF `bytes`, durably. An export is never a Recent entry.
    fn write_export(&mut self, _path: &Path, _bytes: &[u8]) -> Result<(), String> {
        Err("Varos couldn't write the PDF.".into())
    }
    /// An existing export destination's bytes, for the embedded-model check — only for a readable
    /// file of at most `varos_pdf::HAS_MODEL_SCAN_CAP` bytes. Runs on the worker.
    fn read_existing(&mut self, _path: &Path) -> Option<Vec<u8>> {
        None
    }
}

/// What the host must do after a command.
#[derive(Debug, Default, PartialEq)]
pub struct Effect {
    /// Every tab was resolved by a Quit — save the window state and exit.
    pub exit: bool,
    /// Tabs whose coalesced second ⌘S is due now (their first background save landed): the host
    /// queues `Save(id)` for each, behind whatever is already waiting.
    pub follow_up_saves: Vec<SessionId>,
}

/// One lifecycle command run over the workspace and the ports.
///
/// The host settles first (`Ui::settle`, then the active `DocumentSession::settle`), so no command
/// runs mid-gesture or with a colour-picker preview open. The lifecycle does not settle by itself: it
/// cannot reach the Ui, and settling a session before the picker is cancelled would COMMIT the
/// preview.
pub struct Lifecycle<'a> {
    pub ws: &'a mut Workspace,
    pub dialogs: &'a mut dyn Dialogs,
    pub store: &'a mut dyn DocStore,
    /// `Some` = background mode (the app): ⌘S / Save As / Export queue a [`FileJob`] here for the
    /// host's worker, and the result returns as `AppCommand::FileDone`. `None` = everything runs
    /// inline on this thread (the S1 behaviour; Close/Quit's own "Save" always does).
    pub jobs: Option<&'a mut Vec<FileJob>>,
}

impl Lifecycle<'_> {
    /// Run one command. `AppCommand::Window(_)` is ignored here (host-owned).
    pub fn run(&mut self, cmd: AppCommand) -> Effect {
        match cmd {
            AppCommand::SetRecoveryEnabled(_)
            | AppCommand::RetryRecovery(_)
            | AppCommand::Recover(_)
            | AppCommand::DiscardRecovery(_)
            | AppCommand::DeferRecovery => {} // host-owned
            AppCommand::InstallRecovered(copy) => {
                self.ws.add_recovered(*copy);
            }
            AppCommand::Home | AppCommand::ReviewRecovery => self.ws.show_home(),
            AppCommand::OpenRecent(path) => {
                if self.store.exists(&path) {
                    self.open_one(path, None);
                } else if self.dialogs.locate_missing(&path) {
                    self.locate(path);
                }
            }
            AppCommand::LocateRecent(path) => self.locate(path),
            AppCommand::RemoveRecent(path) => self.store.remove_recent(&path),
            AppCommand::ClearRecent => self.store.clear_recent(),
            AppCommand::NewBoard => {
                // `Editor::new()` holds `board::new_board()`: a free canvas with zero artboards
                self.ws.new_untitled();
            }
            AppCommand::NewWithPreset(preset) => {
                let size = self.ws.custom_board_size();
                self.ws.new_untitled_with(varos_core::board::new_board_with_preset(preset, size));
            }
            AppCommand::OpenDialog => {
                let picked = self.dialogs.pick_open();
                self.open_paths(picked);
            }
            AppCommand::OpenPaths(paths, _origin) => self.open_paths(paths),
            AppCommand::Save(id) if self.jobs.is_some() => self.save_in_background(id, false),
            AppCommand::SaveAs(id) if self.jobs.is_some() => self.save_in_background(id, true),
            AppCommand::Save(id) => {
                self.save(id, false);
            }
            AppCommand::SaveAs(id) => {
                self.save(id, true);
            }
            AppCommand::ShowExport(_) => {} // host-owned: it opens the Export sheet
            AppCommand::ExportPdf(id, scope) => return self.export(id, scope),
            AppCommand::FileDone(done) => return self.file_done(*done),
            AppCommand::CloseDocument(id) => self.close(id),
            AppCommand::Quit => return Effect { exit: self.quit(), ..Effect::default() },
            AppCommand::ActivateDocument(id) => {
                self.ws.activate(id);
            }
            AppCommand::ActivateNext => {
                self.ws.activate_relative(1);
            }
            AppCommand::ActivatePrevious => {
                self.ws.activate_relative(-1);
            }
            AppCommand::ReorderDocument(id, slot) => {
                self.ws.reorder(id, slot);
            }
            AppCommand::Window(_) => {}
        }
        Effect::default()
    }

    /// Open: each path gets a FRESH key. A file that is already open (same path, or same
    /// device/inode = an alias) focuses its tab and is never reloaded, even when that tab is dirty.
    /// Otherwise the file is read into a candidate: success → `add_loaded` (which reuses a pristine
    /// active `Untitled`); failure → “Couldn't open …”, and no tab, path, selection or history changes.
    fn open_paths(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            self.open_one(path, None);
        }
    }
    fn locate(&mut self, old: PathBuf) {
        if let Some(path) = self.dialogs.pick_locate() {
            self.open_one(path, Some(&old));
        }
    }
    fn open_one(&mut self, path: PathBuf, old: Option<&Path>) {
        let key = self.store.key(&path);
        if let Some(id) = self.open_tab_of(&key, None) {
            self.ws.activate(id);
            self.store.remember(&key.path, old, None);
            return;
        }
        match self.store.load_with_notice(&path) {
            Ok((doc, notice)) => {
                let board = BoardSummary::of(&doc);
                let at = key.path.clone();
                let id = self.ws.add_loaded(doc, at.clone(), key);
                if let Some(s) = self.ws.get_mut(id) {
                    s.source_fingerprint = self.store.fingerprint(&at);
                    // A4: the released-mask repair changed the content → the tab opens dirty.
                    s.repaired_on_open = notice == Some(varos_core::format::RELEASED_MASKS_NOTICE);
                }
                self.store.remember(&at, old, Some(&board));
                if let Some(message) = notice {
                    self.dialogs.notice(&format!("Opened “{}”", file_name(&path)), message);
                }
            }
            Err(reason) => self.dialogs.open_failed(&file_name(&path), &reason),
        }
    }

    /// The open tab (other than `except`) holding the file `key` names. Each tab's key is computed
    /// FRESH from its path (one stat per tab): a stored key's inode goes stale when another app
    /// rewrites the file atomically, and an alias opened after that would otherwise slip past.
    ///
    /// A file a tab is SAVING to counts as that tab's from the moment its background job is created
    /// until the result lands or fails (`DocumentSession::saving`): the one claimed-paths rule every
    /// destination check (Save As, Export) and every open (⌘O, Recent, the OS hand-off) consults, so
    /// two tabs can never both write one new file, an open focuses the tab still saving it, and an
    /// export never overwrites it.
    fn open_tab_of(&self, key: &FileKey, except: Option<SessionId>) -> Option<SessionId> {
        let store = &*self.store;
        let names =
            |s: &crate::workspace::DocumentSession, p: &Path| store.key(p).same_file(key) && Some(s.id) != except;
        self.ws
            .sessions()
            .iter()
            .find(|s| s.path.as_deref().is_some_and(|p| names(s, p)))
            .or_else(|| self.ws.sessions().iter().find(|s| s.saving.as_ref().is_some_and(|f| names(s, &f.dest))))
            .map(|s| s.id)
    }

    /// Save tab `id` (with `save_as`, or when it has no `.vrs` path yet: Save As). `true` only when
    /// the document is on disk and the tab took the new checkpoint. A cancelled dialog, or a failure
    /// the user did not resolve, leaves the path, the name and the checkpoint exactly as they were.
    fn save(&mut self, id: SessionId, save_as: bool) -> bool {
        let Some(s) = self.ws.get(id) else {
            return false;
        };
        let mut target = save_target(s, save_as);
        loop {
            let Some(dest) = self.prepare_dest(id, target.take()) else {
                return false; // Save As / the external-change prompt cancelled
            };
            match self.write(id, &dest) {
                Ok(SaveOutcome::Durable) => return true,
                Ok(SaveOutcome::ReplacedUnconfirmed(reason)) => {
                    self.dialogs.notice("Save needs confirmation", &format!("Saved, but Varos couldn't confirm the disk finished writing. Your document stays open with unsaved changes. Existing recovery copies are kept.\n{reason}"));
                    return false;
                }
                Err(reason) => {
                    let name = self.name_of(id);
                    match self.dialogs.save_failed(&name, &reason) {
                        SaveFailChoice::TryAgain => target = Some(dest),
                        SaveFailChoice::SaveAs => {} // `target` stays None → the Save dialog again
                        SaveFailChoice::Cancel => return false,
                    }
                }
            }
        }
    }

    /// Where a save of tab `id` goes: `target` (else the Save As dialog), after the external-change
    /// prompt when that file was changed by another app since Varos last read or wrote it. `None` =
    /// the user cancelled. Shared by the inline and the background save.
    fn prepare_dest(&mut self, id: SessionId, mut target: Option<PathBuf>) -> Option<PathBuf> {
        loop {
            let dest = match target.take() {
                Some(p) => p,
                None => self.choose_save_path(id)?,
            };
            let changed = self.ws.get(id).is_some_and(|s| {
                (s.path.as_deref() == Some(dest.as_path())
                    || s.key.as_ref().is_some_and(|key| key.same_file(&self.store.key(&dest))))
                    && s.source_fingerprint != self.store.fingerprint(&dest)
            });
            if changed {
                match self.dialogs.external_change(&self.name_of(id)) {
                    ExternalChoice::Cancel => return None,
                    ExternalChoice::SaveAs => continue,
                    // The external-change prompt's "Replace Anyway" is the one confirmation.
                    ExternalChoice::Replace => {}
                }
            }
            return Some(dest);
        }
    }

    /// ⌘S / Save As in background mode. The questions (Save As dialog, external-change prompt) are
    /// asked here, on the UI thread; then the document is SNAPSHOTTED and the encode + durable write
    /// go to the worker. A ⌘S while this tab's save is still in flight coalesces into ONE follow-up
    /// save (run as a normal ⌘S after the first lands). The host makes Save As wait for an in-flight
    /// save first (`host::save_barrier`), so it never meets one here.
    fn save_in_background(&mut self, id: SessionId, save_as: bool) {
        let Some(s) = self.ws.get_mut(id) else {
            return;
        };
        if let Some(flight) = s.saving.as_mut() {
            if !save_as {
                flight.follow_up = true;
            }
            return;
        }
        let target = save_target(s, save_as);
        self.start_save(id, target);
    }

    /// Ask what a save needs, snapshot, and queue the job. Nothing changes when the user cancels.
    fn start_save(&mut self, id: SessionId, target: Option<PathBuf>) {
        let Some(dest) = self.prepare_dest(id, target) else {
            return;
        };
        let Some(s) = self.ws.get_mut(id) else {
            return;
        };
        let doc = Arc::new(s.editor.doc.clone());
        let ticket = file_jobs::next_ticket();
        s.saving = Some(SaveInFlight {
            ticket,
            dest: dest.clone(),
            doc: doc.clone(),
            follow_up: false,
            started: std::time::Instant::now(),
        });
        self.queue(FileJob::Save(SaveJob { sid: id, ticket, dest, doc }));
    }

    /// Hand `job` to the host's worker. Inline mode (no worker) runs it here and applies its result
    /// at once; a follow-up that produces is dropped there (inline saves never coalesce).
    fn queue(&mut self, job: FileJob) {
        if let Some(jobs) = self.jobs.as_deref_mut() {
            jobs.push(job);
            return;
        }
        let done = file_jobs::execute(job, &mut *self.store);
        let _ = self.file_done(done);
    }

    /// A background job finished: apply it to its tab (a closed tab is ignored, a stale ticket too).
    fn file_done(&mut self, done: FileDone) -> Effect {
        match done {
            FileDone::Saved(done) => self.save_done(done),
            FileDone::Exported(done) => {
                self.export_done(done);
                Effect::default()
            }
        }
    }

    /// A background save landed. Durable: the tab takes the path and its checkpoint becomes the
    /// snapshot that was written (later edits keep it dirty), and the coalesced follow-up is due if
    /// there is something left to save. Unconfirmed: as the inline save. Failed: “Couldn't save” —
    /// the tab stays dirty, the old file is intact; Try Again / Save As start a fresh save.
    fn save_done(&mut self, done: SaveDone) -> Effect {
        let mut effect = Effect::default();
        let Some(flight) = self.ws.get_mut(done.sid).and_then(|s| s.saving.take_if(|f| f.ticket == done.ticket)) else {
            return effect;
        };
        let (id, dest) = (done.sid, done.dest);
        match done.result {
            Ok(SaveOutcome::Durable) => {
                let key = self.store.key(&dest);
                let fingerprint = self.store.fingerprint(&dest);
                let board = BoardSummary::of(&flight.doc); // the snapshot that was written
                if let Some(s) = self.ws.get_mut(id) {
                    s.mark_saved_snapshot(dest.clone(), key, Arc::unwrap_or_clone(flight.doc));
                    s.source_fingerprint = fingerprint;
                    if flight.follow_up && s.is_dirty_exact() {
                        effect.follow_up_saves.push(id);
                    }
                }
                self.store.remember(&dest, None, Some(&board));
            }
            Ok(SaveOutcome::ReplacedUnconfirmed(reason)) => {
                let key = self.store.key(&dest);
                let fingerprint = self.store.fingerprint(&dest);
                if let Some(s) = self.ws.get_mut(id) {
                    s.path = Some(dest.clone());
                    s.key = Some(key);
                    s.untitled = None;
                    s.save_unconfirmed = true;
                    s.recovered = None;
                    s.source_fingerprint = fingerprint;
                }
                self.dialogs.notice("Save needs confirmation", &format!("Saved, but Varos couldn't confirm the disk finished writing. Your document stays open with unsaved changes. Existing recovery copies are kept.\n{reason}"));
            }
            Err(reason) => {
                let name = self.name_of(id);
                match self.dialogs.save_failed(&name, &reason) {
                    SaveFailChoice::TryAgain => self.start_save(id, Some(dest)),
                    SaveFailChoice::SaveAs => self.start_save(id, None),
                    SaveFailChoice::Cancel => {}
                }
            }
        }
        effect
    }

    /// Export PDF (the sheet's Export…): plan the pages on a snapshot, ask for the destination with
    /// the Export save panel (`.pdf` forced), refuse an open document's own file, then queue the
    /// pure-PDF job. Nothing about the tab changes — path, checkpoint, dirty state, Recent.
    fn export(&mut self, id: SessionId, scope: varos_pdf::ExportScope) -> Effect {
        let Some(s) = self.ws.get(id) else {
            return Effect::default();
        };
        let doc = Arc::new(s.editor.doc.clone());
        let plan = match varos_pdf::plan_pdf_export(&doc, scope) {
            Ok(plan) => plan,
            Err(why) => {
                self.dialogs
                    .notice("Couldn't export PDF.", &format!("{} Your document has not changed.", why.reason()));
                return Effect::default();
            }
        };
        let suggested = file_jobs::default_export_name(s);
        let dir = s
            .path
            .as_deref()
            .or_else(|| s.recovered.as_ref().and_then(|r| r.original_path.as_deref()))
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let dest = loop {
            let Some(picked) = self.dialogs.pick_export(&suggested, dir.as_deref()) else {
                return Effect::default(); // cancelled: nothing happens
            };
            let (dest, appended) = file_jobs::with_pdf(picked);
            // An open document's own file (a `.pdf`-opened tab) is never overwritten by an export.
            if self.open_tab_of(&self.store.key(&dest), None).is_some() {
                self.dialogs
                    .notice("Choose another name.", &format!("“{}” is an open Varos document.", file_name(&dest)));
                continue;
            }
            // The panel asked about the name the user typed; only a name WE changed needs its own ask.
            if appended && self.store.exists(&dest) && !self.dialogs.confirm_replace(&file_name(&dest)) {
                continue;
            }
            break dest;
        };
        if let Some(s) = self.ws.get_mut(id) {
            s.exports.push(std::time::Instant::now());
        }
        self.queue(FileJob::Export(ExportJob { sid: id, dest, doc, plan, replace_confirmed: false }));
        Effect::default()
    }

    /// An export finished: say so (“Exported name.pdf”), or why not; a destination holding an editable
    /// Varos document is replaced only after one more explicit Replace.
    ///
    /// The tab may have closed meanwhile: then the result is inert — a written PDF is reported once,
    /// neutrally; a failure or a pending "replace the editable document?" question about a document
    /// that is no longer open is dropped (nothing is asked, nothing is queued).
    fn export_done(&mut self, done: ExportDone) {
        let name = file_name(&done.job.dest);
        let Some(s) = self.ws.get_mut(done.job.sid) else {
            if done.result == ExportResult::Exported {
                self.dialogs.notice(&format!("Exported {name}"), "");
            }
            return;
        };
        if !s.exports.is_empty() {
            s.exports.remove(0);
        }
        match done.result {
            ExportResult::Exported => {
                self.dialogs.notice(&format!("Exported {name}"), "Your document has not changed.")
            }
            ExportResult::Failed(reason) => {
                let reason = reason.trim().trim_end_matches('.');
                self.dialogs.notice("Couldn't export PDF.", &format!("{reason}. Your document has not changed."));
            }
            ExportResult::NeedsReplaceConfirm => {
                if self.dialogs.confirm_export_replace(&name) {
                    if let Some(s) = self.ws.get_mut(done.job.sid) {
                        s.exports.push(std::time::Instant::now());
                    }
                    self.queue(FileJob::Export(ExportJob { replace_confirmed: true, ..done.job }));
                }
            }
        }
    }

    /// The Save As dialog for tab `id`. It suggests `<name>.vrs` in the file's folder. When the
    /// chosen name lacks `.vrs` it is appended (no part of the name is dropped), and replacing an
    /// existing file under that appended name is confirmed first. A file another open tab holds is
    /// refused (two writers for one file). A refusal returns to the dialog; `None` = cancelled.
    fn choose_save_path(&mut self, id: SessionId) -> Option<PathBuf> {
        let s = self.ws.get(id)?;
        let (suggested, dir) = if let Some(source) = &s.recovered {
            (
                format!("{}-recovered.vrs", stem_of(Some(Path::new(&source.name)), &source.name)),
                source.original_path.as_deref().and_then(Path::parent).map(Path::to_path_buf),
            )
        } else {
            (
                format!("{}.vrs", stem_of(s.path.as_deref(), &s.display_name())),
                s.path.as_deref().and_then(Path::parent).map(Path::to_path_buf),
            )
        };
        loop {
            let picked = self.dialogs.pick_save(&suggested, dir.as_deref())?;
            let (dest, appended) = if is_vrs(&picked) { (picked, false) } else { (with_vrs(picked), true) };
            // Another tab's file is refused FIRST, so the user is never asked “Replace?” about a file
            // that cannot be replaced anyway.
            let key = self.store.key(&dest);
            if let Some(other) = self.open_tab_of(&key, Some(id)) {
                let other = self.name_of(other);
                self.dialogs.notice(
                    &format!("“{other}” is open in another tab."),
                    "Saving here would replace that document. Choose another name, or close that tab first.",
                );
                continue;
            }
            // The Save dialog already asked about the name the user typed; only a name WE changed
            // needs its own “Replace?”.
            if appended && self.store.exists(&dest) && !self.dialogs.confirm_replace(&file_name(&dest)) {
                continue;
            }
            return Some(dest);
        }
    }

    /// Write tab `id` to `dest`. Only after the store succeeded does the tab take the path, the name
    /// and the new checkpoint, with a key recomputed AFTER the write (an atomic save replaces the
    /// file's inode).
    fn write(&mut self, id: SessionId, dest: &Path) -> Result<SaveOutcome, String> {
        let s = self.ws.get(id).ok_or_else(|| "The document is no longer open.".to_string())?;
        let outcome = self.store.save(&s.editor.doc, dest)?;
        let board = BoardSummary::of(&s.editor.doc);
        let key = self.store.key(dest);
        if let Some(s) = self.ws.get_mut(id) {
            if outcome == SaveOutcome::Durable {
                s.mark_saved(dest.to_path_buf(), key);
            } else {
                s.path = Some(dest.to_path_buf());
                s.key = Some(key);
                s.untitled = None;
                s.save_unconfirmed = true;
                s.recovered = None; // explicit write adopted this path; uncertainty still forces dirty
            }
            s.source_fingerprint = self.store.fingerprint(dest);
        }
        if outcome == SaveOutcome::Durable {
            self.store.remember(dest, None, Some(&board));
        }
        Ok(outcome)
    }

    /// Close tab `id`. Clean → closed at once. Dirty → ask about THAT tab by name without activating
    /// it: Save (possibly through Save As) closes only when the write completed, Don't Save closes,
    /// Cancel keeps it.
    fn close(&mut self, id: SessionId) {
        let Some(s) = self.ws.get(id) else {
            return;
        };
        if s.is_dirty_exact() && !self.resolve(id, None) {
            return;
        }
        self.ws.remove(id);
    }

    /// The quit transaction over the dirty tabs, in tab order, each activated behind its prompt
    /// (“Document i of n”). Save must complete or the quit aborts; Don't Save counts only for this
    /// run; Cancel aborts and keeps every tab (earlier saves stay saved). `true` = every tab was
    /// resolved and the app may exit. Nothing is removed either way: the host exits.
    fn quit(&mut self) -> bool {
        let dirty: Vec<SessionId> = self.ws.sessions().iter().filter(|s| s.is_dirty_exact()).map(|s| s.id).collect();
        let n = dirty.len();
        for (i, id) in dirty.into_iter().enumerate() {
            self.ws.activate(id);
            if !self.resolve(id, Some((i + 1, n))) {
                return false;
            }
        }
        true
    }

    /// Ask “Save changes to …?” about tab `id` and carry the answer out. `true` = resolved (saved,
    /// or Don't Save); `false` = Cancel, or a Save that did not complete.
    fn resolve(&mut self, id: SessionId, progress: Option<(usize, usize)>) -> bool {
        let name = self.name_of(id);
        match self.dialogs.ask_save_changes(&name, progress) {
            SaveDecision::Save => self.save(id, false),
            SaveDecision::DontSave => true,
            SaveDecision::Cancel => false,
        }
    }

    fn name_of(&self, id: SessionId) -> String {
        self.ws.get(id).map(|s| s.display_name()).unwrap_or_default()
    }
}

/// Save writes native `.vrs` only: a `.pdf`-opened or never-saved document (and Save As) goes
/// through the Save As dialog (`None`).
fn save_target(s: &crate::workspace::DocumentSession, save_as: bool) -> Option<PathBuf> {
    match (&s.path, save_as) {
        (Some(p), false) if is_vrs(p) => Some(p.clone()),
        _ => None,
    }
}

/// `true` when the path ends in `.vrs` (any case).
fn is_vrs(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrs"))
}

/// The chosen name + `.vrs`, keeping every part of it (`logo.pdf` → `logo.pdf.vrs`).
fn with_vrs(p: PathBuf) -> PathBuf {
    let mut s = p.into_os_string();
    s.push(".vrs");
    PathBuf::from(s)
}

/// The Save As suggestion's stem: the file's stem (`Logo.pdf` → `Logo`), else the tab name.
fn stem_of(path: Option<&Path>, display_name: &str) -> String {
    path.and_then(Path::file_stem).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| file_safe(display_name))
}

/// A board name used as a suggested file name: path separators and `:` become `-` (a board can be
/// named "Logo / v2" but a file cannot); the Save dialog still lets the user change it.
fn file_safe(name: &str) -> String {
    name.chars().map(|c| if matches!(c, '/' | '\\' | ':') { '-' } else { c }).collect()
}

/// The name a prompt shows for a path (its file name, else the whole path).
fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

#[cfg(test)]
mod tests {
    //! Headless lifecycle tests: a scripted `FakeDialogs` (an answer queue that records every
    //! prompt) and an in-memory `FakeStore` (per-path load/save failure, aliases, an inode that
    //! changes on every save). No rfd, no file system, no GPU, no EventLoop.
    use super::*;
    use crate::app_command::{OpenOrigin, WindowCmd};
    use crate::workspace::DocumentSession;
    use std::collections::{HashMap, HashSet, VecDeque};
    use varos_core::editor::{Editor, ToolKind};
    use varos_core::geom::Rgba;
    use varos_core::model::{Anchor, Artboard, Path as VPath};
    use varos_core::EditCommand;

    const RED: Rgba = [1.0, 0.0, 0.0, 1.0];
    const BLUE: Rgba = [0.0, 0.0, 1.0, 1.0];

    // ───────────── fake ports ─────────────

    /// One scripted answer, consumed in order by the prompt of the same kind.
    #[derive(Debug)]
    enum Ans {
        Open(Vec<PathBuf>),
        Pick(Option<PathBuf>),
        Decide(SaveDecision),
        Fail(SaveFailChoice),
        Replace(bool),
        Locate(bool),
        External(ExternalChoice),
    }

    #[derive(Default)]
    struct FakeDialogs {
        answers: VecDeque<Ans>,
        /// Every prompt shown, in order (`open`, `save-as <suggested> in <dir>`, `ask <name> [i/n]`,
        /// `failed <name>: <reason>`, `open-failed <name>: <reason>`, `replace <name>`, `notice <title>`).
        log: Vec<String>,
    }
    impl FakeDialogs {
        fn next(&mut self, prompt: String) -> Ans {
            self.log.push(prompt.clone());
            self.answers.pop_front().unwrap_or_else(|| panic!("unscripted prompt `{prompt}`; log: {:?}", self.log))
        }
    }
    impl Dialogs for FakeDialogs {
        fn external_change(&mut self, name: &str) -> ExternalChoice {
            match self.next(format!("external {name}")) {
                Ans::External(choice) => choice,
                a => panic!("unexpected {a:?}"),
            }
        }
        fn locate_missing(&mut self, _: &Path) -> bool {
            match self.next("missing".into()) {
                Ans::Locate(answer) => answer,
                a => panic!("unexpected {a:?}"),
            }
        }
        fn pick_open(&mut self) -> Vec<PathBuf> {
            match self.next("open".into()) {
                Ans::Open(v) => v,
                a => panic!("the Open dialog got {a:?}"),
            }
        }
        fn pick_save(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf> {
            let dir = dir.map_or("-".into(), |d| d.display().to_string());
            match self.next(format!("save-as {suggested} in {dir}")) {
                Ans::Pick(p) => p,
                a => panic!("the Save dialog got {a:?}"),
            }
        }
        fn ask_save_changes(&mut self, name: &str, progress: Option<(usize, usize)>) -> SaveDecision {
            let prompt = match progress {
                Some((i, n)) => format!("ask {name} {i}/{n}"),
                None => format!("ask {name}"),
            };
            match self.next(prompt) {
                Ans::Decide(d) => d,
                a => panic!("“Save changes?” got {a:?}"),
            }
        }
        fn save_failed(&mut self, name: &str, reason: &str) -> SaveFailChoice {
            match self.next(format!("failed {name}: {reason}")) {
                Ans::Fail(c) => c,
                a => panic!("“Couldn't save” got {a:?}"),
            }
        }
        fn open_failed(&mut self, name: &str, reason: &str) {
            self.log.push(format!("open-failed {name}: {reason}"));
        }
        fn confirm_replace(&mut self, name: &str) -> bool {
            match self.next(format!("replace {name}")) {
                Ans::Replace(b) => b,
                a => panic!("“Replace?” got {a:?}"),
            }
        }
        fn notice(&mut self, title: &str, _body: &str) {
            self.log.push(format!("notice {title}"));
        }
        fn pick_export(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf> {
            let dir = dir.map_or("-".into(), |d| d.display().to_string());
            match self.next(format!("export {suggested} in {dir}")) {
                Ans::Pick(p) => p,
                a => panic!("the Export panel got {a:?}"),
            }
        }
        fn confirm_export_replace(&mut self, name: &str) -> bool {
            match self.next(format!("replace-editable {name}")) {
                Ans::Replace(b) => b,
                a => panic!("“contains an editable Varos document” got {a:?}"),
            }
        }
    }

    /// An in-memory disk. Keys are `(path, (7, inode))`; every save gives the file a NEW inode
    /// (like `write_atomic`'s temp + rename). An alias keeps its own path but shares the target's
    /// inode (a case variant / hard link: only the inode can tell).
    #[derive(Default)]
    struct FakeStore {
        files: HashMap<PathBuf, Document>,
        inodes: HashMap<PathBuf, u64>,
        next_ino: u64,
        fingerprints: HashMap<PathBuf, varos_app::storage::durable::Fingerprint>,
        unconfirmed: bool,
        aliases: HashMap<PathBuf, PathBuf>,
        fail_load: HashSet<PathBuf>,
        /// Save failures still to come per path (`u32::MAX` = always).
        fail_save: HashMap<PathBuf, u32>,
        loads: Vec<PathBuf>,
        saves: Vec<PathBuf>,
        notices: HashMap<PathBuf, &'static str>,
        recent: varos_app::storage::recents::Recents,
        /// Exported PDFs by destination (the real export bytes), and other raw files on the disk.
        exported: HashMap<PathBuf, Vec<u8>>,
        raw: HashMap<PathBuf, Vec<u8>>,
        fail_export: bool,
    }
    impl FakeStore {
        fn target(&self, p: &Path) -> PathBuf {
            self.aliases.get(p).cloned().unwrap_or_else(|| p.to_path_buf())
        }
        fn put(&mut self, p: &str, doc: Document) {
            self.files.insert(PathBuf::from(p), doc);
            self.rewrite(p);
        }
        /// Another app rewrote the file atomically: same content, a new inode.
        fn rewrite(&mut self, p: &str) {
            self.next_ino += 1;
            self.inodes.insert(PathBuf::from(p), self.next_ino);
        }
        fn doc(&self, p: &str) -> &Document {
            self.files.get(Path::new(p)).unwrap_or_else(|| panic!("no file {p}"))
        }
    }
    impl DocStore for FakeStore {
        fn fingerprint(&self, path: &Path) -> Option<varos_app::storage::durable::Fingerprint> {
            self.fingerprints.get(path).copied()
        }
        fn remember(&mut self, path: &Path, old: Option<&Path>, board: Option<&BoardSummary>) {
            if let Some(old) = old {
                self.recent.relocate(old, path, None, 50);
            } else {
                self.recent.record(path, None, 50);
            }
            if let Some(board) = board {
                self.recent.set_board(path, board.clone(), 50);
            }
        }
        fn load_with_notice(&mut self, path: &Path) -> Result<(Document, Option<&'static str>), String> {
            let doc = self.load(path)?;
            Ok((doc, self.notices.get(&self.target(path)).copied()))
        }
        fn load(&mut self, path: &Path) -> Result<Document, String> {
            let t = self.target(path);
            self.loads.push(t.clone());
            if self.fail_load.contains(&t) {
                return Err("The file is damaged".into());
            }
            self.files.get(&t).cloned().ok_or_else(|| "No such file".into())
        }
        fn save(&mut self, doc: &Document, path: &Path) -> Result<SaveOutcome, String> {
            let t = self.target(path);
            if let Some(left) = self.fail_save.get_mut(&t) {
                if *left > 0 {
                    if *left != u32::MAX {
                        *left -= 1;
                    }
                    return Err("The disk is not writable".into());
                }
            }
            self.saves.push(t.clone());
            self.files.insert(t.clone(), doc.clone());
            self.next_ino += 1;
            self.inodes.insert(t, self.next_ino);
            Ok(if self.unconfirmed {
                SaveOutcome::ReplacedUnconfirmed("disk sync failed".into())
            } else {
                SaveOutcome::Durable
            })
        }
        fn key(&self, path: &Path) -> FileKey {
            let t = self.target(path);
            FileKey { path: path.to_path_buf(), dev_ino: self.inodes.get(&t).map(|i| (7, *i)), name_id: None }
        }
        fn exists(&self, path: &Path) -> bool {
            let t = self.target(path);
            self.files.contains_key(&t) || self.raw.contains_key(&t) || self.exported.contains_key(&t)
        }
        fn write_export(&mut self, path: &Path, bytes: &[u8]) -> Result<(), String> {
            if self.fail_export {
                return Err("The disk is full.".into());
            }
            self.exported.insert(self.target(path), bytes.to_vec());
            Ok(())
        }
        fn read_existing(&mut self, path: &Path) -> Option<Vec<u8>> {
            let t = self.target(path);
            self.raw.get(&t).or_else(|| self.exported.get(&t)).cloned()
        }
    }

    // ───────────── rig + helpers ─────────────

    struct Rig {
        ws: Workspace,
        d: FakeDialogs,
        s: FakeStore,
    }
    impl Rig {
        fn new() -> Self {
            Rig { ws: Workspace::new(), d: FakeDialogs::default(), s: FakeStore::default() }
        }
        fn run(&mut self, cmd: AppCommand) -> Effect {
            Lifecycle { ws: &mut self.ws, dialogs: &mut self.d, store: &mut self.s, jobs: None }.run(cmd)
        }
        /// Background mode (the app): the command's jobs come back instead of running.
        fn bg(&mut self, cmd: AppCommand) -> (Effect, Vec<FileJob>) {
            let mut jobs = Vec::new();
            let effect =
                Lifecycle { ws: &mut self.ws, dialogs: &mut self.d, store: &mut self.s, jobs: Some(&mut jobs) }
                    .run(cmd);
            (effect, jobs)
        }
        /// The worker runs `job` on the fake disk; its result is applied as the host applies it.
        fn land(&mut self, job: FileJob) -> (Effect, Vec<FileJob>) {
            let done = file_jobs::execute(job, &mut self.s);
            self.bg(AppCommand::FileDone(Box::new(done)))
        }
        fn script(&mut self, answers: impl IntoIterator<Item = Ans>) {
            self.d.answers.extend(answers);
        }
        /// Every scripted answer was used; returns (and clears) the prompt log.
        fn prompts(&mut self) -> Vec<String> {
            assert!(self.d.answers.is_empty(), "unused answers: {:?}", self.d.answers);
            std::mem::take(&mut self.d.log)
        }
        fn open(&mut self, p: &str) -> SessionId {
            self.run(AppCommand::OpenPaths(vec![PathBuf::from(p)], OpenOrigin::CommandLine));
            self.ws.active_id().unwrap()
        }
        fn get(&self, id: SessionId) -> &DocumentSession {
            self.ws.get(id).expect("the tab is open")
        }
        fn ed(&mut self, id: SessionId) -> &mut Editor {
            &mut self.ws.get_mut(id).expect("the tab is open").editor
        }
        fn active(&self) -> SessionId {
            self.ws.active_id().unwrap()
        }
        fn ids(&self) -> Vec<SessionId> {
            self.ws.sessions().iter().map(|s| s.id).collect()
        }
        fn names(&self) -> Vec<String> {
            self.ws.sessions().iter().map(|s| s.display_name()).collect()
        }
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    /// A saved-looking file: one 100×100 board and one square of `color`.
    fn art(color: Rgba) -> Document {
        let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
        let sq = VPath::new(
            10,
            vec![a(100, [20.0, 20.0]), a(101, [50.0, 20.0]), a(102, [50.0, 50.0]), a(103, [20.0, 50.0])],
            true,
            Some(color),
            None,
            1.0,
        );
        Document {
            artboards: vec![Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, name: "A".into(), ..Artboard::default() }],
            paths: vec![sq],
            ids: 200,
            ..Document::default()
        }
    }

    /// Draw a square in `color` with the Rect tool (a real, committed edit).
    fn draw(ed: &mut Editor, color: Rgba) {
        let prev = ed.tool;
        ed.cur_fill = Some(color);
        ed.set_tool(ToolKind::Rect);
        ed.pointer_down([200.0, 200.0]);
        ed.pointer_move([240.0, 240.0]);
        ed.pointer_up();
        ed.set_tool(prev);
    }

    fn fills(ed: &Editor) -> Vec<Option<Rgba>> {
        ed.doc.paths.iter().map(|p| p.fill.solid()).collect()
    }

    /// The fills of the art stored in the fake file at `path`.
    fn file_fills(s: &FakeStore, path: &str) -> Vec<Option<Rgba>> {
        s.doc(path).paths.iter().map(|p| p.fill.solid()).collect()
    }

    // ───────────── New / Open ─────────────

    #[test]
    fn new_board_is_a_clean_free_canvas_and_presets_open_with_their_artboard() {
        use varos_core::board::PresetId;
        let mut r = Rig::new();
        r.run(AppCommand::NewBoard);
        let board = r.active();
        assert!(r.get(board).editor.doc.artboards.is_empty(), "New board = zero artboards");
        assert!(!r.get(board).is_dirty_exact());
        for (preset, w, h) in [
            (PresetId::Square, 1080.0, 1080.0),
            (PresetId::Portrait, 1080.0, 1350.0),
            (PresetId::Story, 1080.0, 1920.0),
            (PresetId::A4, 595.0, 842.0),
            (PresetId::Custom, 1080.0, 1080.0), // no custom size remembered yet → the table fallback
        ] {
            r.run(AppCommand::NewWithPreset(preset));
            let id = r.active();
            let s = r.get(id);
            assert_eq!(s.editor.doc.artboards.len(), 1, "{preset:?}");
            let ab = &s.editor.doc.artboards[0];
            assert_eq!((ab.w, ab.h), (w, h), "{preset:?}");
            assert!(!s.is_dirty_exact() && s.path.is_none(), "{preset:?}: a clean Untitled board");
            r.ed(id).execute(EditCommand::Undo);
            assert_eq!(r.get(id).editor.doc.artboards.len(), 1, "{preset:?}: undo cannot remove the preset page");
        }
        r.ws.set_custom_board_size((300.0, 250.0));
        r.run(AppCommand::NewWithPreset(PresetId::Custom));
        let ab = &r.get(r.active()).editor.doc.artboards[0];
        assert_eq!((ab.w, ab.h), (300.0, 250.0), "Custom = the last-used custom size");
        let names = r.names();
        assert_eq!(names.len(), 8, "each New is its own tab: {names:?}");
        assert!(names.iter().all(|n| n.starts_with("Untitled-")), "{names:?}");
    }

    #[test]
    fn the_board_name_is_the_tab_name_and_the_suggested_file_name() {
        let mut r = Rig::new();
        let id = r.active();
        assert_eq!(r.names(), ["Untitled-1"]);
        r.ed(id).execute(EditCommand::SetBoardName("شعار / v2".into()));
        assert_eq!(r.names(), ["شعار / v2"], "a named, unsaved board shows its name at once");
        assert_eq!(crate::host::window_title(&r.get(id).display_name(), r.get(id).is_dirty()), "شعار / v2* — Varos");
        r.script([Ans::Pick(None)]);
        r.run(AppCommand::Save(id));
        assert_eq!(r.prompts(), ["save-as شعار - v2.vrs in -"], "the name suggests a safe file name");
        r.script([Ans::Pick(Some(p("/d/logo.vrs")))]);
        r.run(AppCommand::Save(id));
        assert_eq!(r.prompts(), ["save-as شعار - v2.vrs in -"]);
        assert_eq!(r.names(), ["شعار / v2"], "the board name still wins over the file stem");
        r.ed(id).execute(EditCommand::SetBoardName(String::new()));
        assert_eq!(r.names(), ["logo"], "no board name → the file stem");
    }

    #[test]
    fn recent_caches_the_board_summary_only_on_successful_open_and_save() {
        use varos_app::storage::recents::BoardSummary;
        let mut r = Rig::new();
        let mut doc = art(RED);
        doc.name = "Logo".into();
        doc.description = "Round two".into();
        doc.tags = vec!["client".into(), "عربي".into()];
        r.s.put("/d/a.vrs", doc);
        let a = r.open("/d/a.vrs");
        let cached = |r: &Rig| r.s.recent.entries()[0].board.clone();
        let want = BoardSummary {
            name: "Logo".into(),
            description: "Round two".into(),
            tags: vec!["client".into(), "عربي".into()],
            artboards: 1,
        };
        assert_eq!(cached(&r), Some(want.clone()), "open caches what is on disk");
        assert_eq!(r.s.recent.entries()[0].name, "Logo");

        // an unsaved edit never reaches Recent, not even when the open tab is focused again
        r.ed(a).execute(EditCommand::SetBoardTags(vec!["draft".into()]));
        r.ed(a).execute(EditCommand::AddArtboard);
        r.open("/d/a.vrs");
        assert_eq!(cached(&r), Some(want.clone()), "focusing an open tab keeps the on-disk summary");

        // a failed save leaves the cache alone; the successful retry updates it
        r.s.fail_save.insert(p("/d/a.vrs"), 1);
        r.script([Ans::Fail(SaveFailChoice::TryAgain)]);
        r.run(AppCommand::Save(a));
        r.prompts();
        let saved = BoardSummary { tags: vec!["draft".into()], artboards: 2, ..want };
        assert_eq!(cached(&r), Some(saved), "only the landed save is cached");

        // a failed open never reaches Recent at all
        r.s.put("/d/bad.vrs", art(BLUE));
        r.s.fail_load.insert(p("/d/bad.vrs"));
        r.open("/d/bad.vrs");
        assert!(r.s.recent.entries().iter().all(|e| e.path != p("/d/bad.vrs")));
    }

    #[test]
    fn a_background_save_caches_the_snapshot_it_wrote_not_later_edits() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(RED));
        let a = r.open("/d/a.vrs");
        r.ed(a).execute(EditCommand::SetBoardName("Written".into()));
        let (_, jobs) = r.bg(AppCommand::Save(a));
        r.ed(a).execute(EditCommand::SetBoardName("Typed after".into())); // while the save is on the worker
        for job in jobs {
            r.land(job);
        }
        let e = &r.s.recent.entries()[0];
        assert_eq!(e.board.as_ref().map(|b| b.name.as_str()), Some("Written"));
        assert_eq!(e.name, "Written");
        assert!(r.get(a).is_dirty_exact(), "the later rename is still unsaved");
    }

    #[test]
    fn new_command_adds_clean_boardless_untitled() {
        let mut r = Rig::new();
        let first = r.active();
        draw(r.ed(first), RED);
        assert_eq!(r.run(AppCommand::NewBoard), Effect::default());
        let b = r.active();
        assert_ne!(b, first);
        assert_eq!(r.names(), ["Untitled-1", "Untitled-2"]);
        let s = r.get(b);
        assert!(s.is_pristine() && !s.is_dirty_exact(), "a new tab is clean");
        assert!(s.editor.doc.artboards.is_empty() && s.editor.doc.paths.is_empty(), "…and boardless");
        assert_eq!(r.get(first).editor.doc.paths.len(), 1, "New never clears the other tab");
        // close it and ⌘N again: the number is never reused
        r.run(AppCommand::CloseDocument(b));
        r.run(AppCommand::NewBoard);
        assert_eq!(r.names(), ["Untitled-1", "Untitled-3"]);
        assert!(r.prompts().is_empty(), "New and closing a clean tab ask nothing");
    }

    #[test]
    fn open_lands_in_a_new_tab_and_dirty_tab_survives() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let first = r.active();
        draw(r.ed(first), RED);
        r.script([Ans::Open(vec![p("/d/a.vrs")])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(r.prompts(), ["open"], "no discard prompt: nothing is replaced");
        assert_eq!(r.names(), ["Untitled-1", "a"]);
        let a = r.active();
        assert_ne!(a, first);
        assert_eq!(fills(&r.get(a).editor), [Some(BLUE)]);
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert!(r.get(first).is_dirty_exact(), "the edited Untitled survives, still dirty");
        assert_eq!(fills(&r.get(first).editor), [Some(RED)]);
        // a cancelled Open dialog does nothing
        r.script([Ans::Open(vec![])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(r.prompts(), ["open"]);
        assert_eq!(r.ids(), [first, a]);
    }

    #[test]
    fn migration_notice_only_after_successful_first_open() {
        let mut r = Rig::new();
        r.s.put("old.vrs", Document::default());
        r.s.notices.insert(PathBuf::from("old.vrs"), "Broken clipping mask released.");
        r.open("old.vrs");
        assert_eq!(r.prompts(), vec!["notice Opened “old.vrs”"]);
        r.open("old.vrs");
        assert!(r.prompts().is_empty(), "already-open files are not reloaded or re-notified");
        r.s.put("bad.vrs", Document::default());
        r.s.notices.insert(PathBuf::from("bad.vrs"), "must not be shown");
        r.s.fail_load.insert(PathBuf::from("bad.vrs"));
        r.open("bad.vrs");
        let prompts = r.prompts();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].starts_with("open-failed"));
    }

    #[test]
    fn released_mask_repair_opens_dirty_and_save_makes_it_clean() {
        let mut r = Rig::new();
        r.s.put("old.vrs", art(RED));
        r.s.put("migrated.vrs", art(BLUE));
        r.s.notices.insert(PathBuf::from("old.vrs"), varos_core::format::RELEASED_MASKS_NOTICE);
        r.s.notices.insert(PathBuf::from("migrated.vrs"), varos_core::format::MIGRATION_NOTICE);
        let plain = r.open("migrated.vrs");
        assert!(!r.get(plain).is_dirty_exact(), "a plain format migration is not a content change");
        let id = r.open("old.vrs");
        let _ = r.prompts();
        assert!(r.get(id).is_dirty() && r.get(id).is_dirty_exact(), "the repair is a content change");
        r.run(AppCommand::Save(id));
        assert_eq!(r.s.saves, vec![p("old.vrs")]);
        assert!(!r.get(id).is_dirty_exact(), "Save wrote the repair");
    }

    #[test]
    fn open_reuses_pristine_untitled() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        assert_eq!(r.names(), ["a"], "the untouched Untitled-1 was replaced, not kept");
        assert_eq!(r.ids(), [a]);
        assert!(!r.get(a).is_dirty_exact());
        assert!(r.prompts().is_empty());
    }

    #[test]
    fn open_already_open_file_focuses_without_reload() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED); // dirty
        let rev = r.get(a).editor.rev;
        let other = {
            r.run(AppCommand::NewBoard);
            r.active()
        };
        assert_eq!(r.s.loads.len(), 1);
        // the same path again → focus, no reload, the unsaved edit is kept
        r.open("/d/a.vrs");
        assert_eq!(r.active(), a);
        assert_eq!(r.s.loads.len(), 1, "never reloaded");
        assert_eq!(r.get(a).editor.rev, rev);
        assert!(r.get(a).is_dirty_exact(), "a dirty tab is focused, not replaced");
        // an alias (another spelling, same inode) → the same tab
        r.s.aliases.insert(p("/Link/A.VRS"), p("/d/a.vrs"));
        r.run(AppCommand::ActivateDocument(other));
        r.open("/Link/A.VRS");
        assert_eq!(r.active(), a, "an alias focuses the open tab");
        // another app rewrote the file (new inode): the alias still finds the tab, because each tab's
        // key is computed fresh on Open (not the stale stored key)
        r.s.rewrite("/d/a.vrs");
        r.run(AppCommand::ActivateDocument(other));
        r.open("/Link/A.VRS");
        assert_eq!(r.active(), a, "…even after an external atomic rewrite");
        assert_eq!(r.s.loads.len(), 1);
        assert_eq!(r.ids(), [a, other], "no second tab for the same file");
        assert!(r.prompts().is_empty());
    }

    #[test]
    fn open_failure_changes_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/broken.vrs", art(RED));
        r.s.fail_load.insert(p("/d/broken.vrs"));
        // a pristine Untitled survives a failed open under the same id
        let first = r.active();
        r.open("/d/broken.vrs");
        assert_eq!(r.ids(), [first]);
        assert!(r.get(first).is_pristine());
        assert_eq!(r.prompts(), ["open-failed broken.vrs: The file is damaged"]);
        // an edited file with a selection: nothing about it changes
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        {
            let ed = r.ed(a);
            ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
        }
        let (doc, rev, sel) = {
            let s = r.get(a);
            (s.editor.doc.clone(), s.editor.rev, s.editor.objsel.clone())
        };
        r.script([Ans::Open(vec![p("/d/broken.vrs"), p("/d/missing.vrs")])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(
            r.prompts(),
            ["open", "open-failed broken.vrs: The file is damaged", "open-failed missing.vrs: No such file"]
        );
        assert_eq!(r.ids(), [a]);
        assert_eq!(r.active(), a);
        let s = r.get(a);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert!(s.editor.doc == doc && s.editor.rev == rev && s.editor.objsel == sel);
        assert!(s.is_dirty_exact());
        r.ed(a).execute(EditCommand::Undo);
        assert!(!r.get(a).is_dirty_exact(), "the history is intact: undo still returns to the saved state");
    }

    #[test]
    fn open_multiple_paths_mixed_success() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.pdf", art(RED));
        r.s.put("/d/bad.vrs", art(RED));
        r.s.fail_load.insert(p("/d/bad.vrs"));
        r.script([Ans::Open(vec![p("/d/a.vrs"), p("/d/bad.vrs"), p("/d/b.pdf"), p("/d/a.vrs")])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(r.prompts(), ["open", "open-failed bad.vrs: The file is damaged"]);
        assert_eq!(r.names(), ["a", "b"], "the pristine Untitled-1 took the first file");
        assert_eq!(r.s.loads.len(), 3, "the repeated a.vrs was focused, not loaded twice");
        assert_eq!(r.active(), r.ids()[0], "…and ends up active");
        assert!(r.ws.sessions().iter().all(|s| !s.is_dirty_exact()));
    }

    // ───────────── Save / Save As ─────────────

    #[test]
    fn save_untitled_goes_through_save_as() {
        let mut r = Rig::new();
        let id = r.active();
        draw(r.ed(id), RED);
        r.script([Ans::Pick(Some(p("/d/Logo.vrs")))]);
        r.run(AppCommand::Save(id));
        assert_eq!(r.prompts(), ["save-as Untitled-1.vrs in -"]);
        let s = r.get(id);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/Logo.vrs")));
        assert_eq!((s.display_name().as_str(), s.untitled), ("Logo", None));
        assert!(!s.is_dirty_exact());
        assert!(r.s.doc("/d/Logo.vrs").content_eq(&s.editor.doc));
        assert_eq!(s.key, Some(r.s.key(Path::new("/d/Logo.vrs"))), "the key is taken after the write");
        // the next Save writes in place without asking
        draw(r.ed(id), BLUE);
        r.run(AppCommand::Save(id));
        assert!(r.prompts().is_empty());
        assert_eq!(r.s.saves, [p("/d/Logo.vrs"), p("/d/Logo.vrs")]);
        assert_eq!(file_fills(&r.s, "/d/Logo.vrs"), [Some(RED), Some(BLUE)]);
    }

    #[test]
    fn save_as_cancel_changes_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.script([Ans::Pick(None)]);
        r.run(AppCommand::SaveAs(a));
        assert_eq!(r.prompts(), ["save-as a.vrs in /d"]);
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert!(r.get(a).is_dirty_exact());
        assert!(r.s.saves.is_empty());
        // an Untitled whose first Save is cancelled stays Untitled and dirty
        r.run(AppCommand::NewBoard);
        let u = r.active();
        draw(r.ed(u), RED);
        r.script([Ans::Pick(None)]);
        r.run(AppCommand::Save(u));
        assert_eq!(r.prompts(), ["save-as Untitled-2.vrs in -"]);
        let s = r.get(u);
        assert!(s.path.is_none() && s.untitled == Some(2) && s.is_dirty_exact());
        assert!(r.s.saves.is_empty());
    }

    #[test]
    fn save_failure_keeps_path_and_dirty_then_try_again_succeeds() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.s.fail_save.insert(p("/d/a.vrs"), 2);
        r.script([Ans::Fail(SaveFailChoice::Cancel)]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["failed a: The disk is not writable"]);
        assert!(r.get(a).is_dirty_exact(), "a failed save keeps the tab dirty");
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)], "the old file is intact");
        // fails once more, then Try Again succeeds
        r.script([Ans::Fail(SaveFailChoice::TryAgain)]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["failed a: The disk is not writable"]);
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE), Some(RED)]);
    }

    #[test]
    fn save_failure_save_as_route() {
        let mut r = Rig::new();
        r.s.put("/ro/a.vrs", art(BLUE));
        r.s.fail_save.insert(p("/ro/a.vrs"), u32::MAX);
        let a = r.open("/ro/a.vrs");
        draw(r.ed(a), RED);
        r.script([Ans::Fail(SaveFailChoice::SaveAs), Ans::Pick(Some(p("/d/copy.vrs")))]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["failed a: The disk is not writable", "save-as a.vrs in /ro"]);
        let s = r.get(a);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/copy.vrs")));
        assert!(!s.is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/ro/a.vrs"), [Some(BLUE)], "the read-only original is untouched");
        assert_eq!(r.s.saves, [p("/d/copy.vrs")]);
    }

    #[test]
    fn save_as_moves_path_only_after_success() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let old_key = r.get(a).key.clone();
        draw(r.ed(a), RED);
        r.s.fail_save.insert(p("/d/b.vrs"), 1);
        r.script([Ans::Pick(Some(p("/d/b.vrs"))), Ans::Fail(SaveFailChoice::Cancel)]);
        r.run(AppCommand::SaveAs(a));
        assert_eq!(r.prompts(), ["save-as a.vrs in /d", "failed a: The disk is not writable"]);
        let s = r.get(a);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/a.vrs")), "a failed Save As keeps the old path");
        assert_eq!(s.key, old_key);
        assert!(s.is_dirty_exact());
        r.script([Ans::Pick(Some(p("/d/b.vrs")))]);
        r.run(AppCommand::SaveAs(a));
        assert_eq!(r.prompts(), ["save-as a.vrs in /d"]);
        let s = r.get(a);
        assert_eq!((s.path.as_deref(), s.display_name().as_str()), (Some(Path::new("/d/b.vrs")), "b"));
        assert!(!s.is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)], "the old file is left intact");
        assert_eq!(file_fills(&r.s, "/d/b.vrs"), [Some(BLUE), Some(RED)]);
        // the old file is no longer "open": opening it adds a tab
        r.open("/d/a.vrs");
        assert_eq!(r.names(), ["b", "a"]);
    }

    #[test]
    fn save_as_onto_other_open_tab_is_refused() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(RED));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(b), BLUE);
        // onto A's file (also through an alias of it) → refused, back to the dialog, then cancelled
        r.s.aliases.insert(p("/Link/A.vrs"), p("/d/a.vrs"));
        // …and "/d/a" (we append .vrs → A's file) is refused WITHOUT a pointless “Replace?” first
        r.script([
            Ans::Pick(Some(p("/d/a.vrs"))),
            Ans::Pick(Some(p("/Link/A.vrs"))),
            Ans::Pick(Some(p("/d/a"))),
            Ans::Pick(None),
        ]);
        r.run(AppCommand::SaveAs(b));
        assert_eq!(
            r.prompts(),
            [
                "save-as b.vrs in /d",
                "notice “a” is open in another tab.",
                "save-as b.vrs in /d",
                "notice “a” is open in another tab.",
                "save-as b.vrs in /d",
                "notice “a” is open in another tab.",
                "save-as b.vrs in /d",
            ]
        );
        assert!(r.s.saves.is_empty(), "A's file was never overwritten");
        assert_eq!(r.get(b).path.as_deref(), Some(Path::new("/d/b.vrs")));
        assert!(r.get(b).is_dirty_exact());
        assert_eq!(r.active(), b, "the refusal does not switch tabs");
        // onto its OWN file is fine
        r.script([Ans::Pick(Some(p("/d/b.vrs")))]);
        r.run(AppCommand::SaveAs(b));
        assert_eq!(r.prompts(), ["save-as b.vrs in /d"]);
        assert!(!r.get(b).is_dirty_exact());
        assert!(!r.get(a).is_dirty_exact());
    }

    #[test]
    fn save_on_pdf_path_offers_vrs() {
        let mut r = Rig::new();
        r.s.put("/d/Old.pdf", art(BLUE));
        let a = r.open("/d/Old.pdf");
        draw(r.ed(a), RED);
        r.script([Ans::Pick(Some(p("/d/Old.vrs")))]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["save-as Old.vrs in /d"], "Save on a .pdf goes through Save As, suggesting .vrs");
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/Old.vrs")));
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(r.s.saves, [p("/d/Old.vrs")], "the .pdf was never written");
    }

    #[test]
    fn appended_extension_asks_before_replacing() {
        let mut r = Rig::new();
        r.s.put("/d/logo.vrs", art(BLUE));
        r.s.put("/d/mark.VRS", art(BLUE));
        let u = r.active();
        draw(r.ed(u), RED);
        // "logo" → logo.vrs exists → asked; No → back to the dialog; "logo.pdf" → logo.pdf.vrs (new)
        r.script([Ans::Pick(Some(p("/d/logo"))), Ans::Replace(false), Ans::Pick(Some(p("/d/logo.pdf")))]);
        r.run(AppCommand::SaveAs(u));
        assert_eq!(r.prompts(), ["save-as Untitled-1.vrs in -", "replace logo.vrs", "save-as Untitled-1.vrs in -"]);
        assert_eq!(r.get(u).path.as_deref(), Some(Path::new("/d/logo.pdf.vrs")), "no part of the name is dropped");
        assert_eq!(file_fills(&r.s, "/d/logo.vrs"), [Some(BLUE)], "logo.vrs was not replaced");
        // "logo" again, Replace → written
        draw(r.ed(u), RED);
        r.script([Ans::Pick(Some(p("/d/logo"))), Ans::Replace(true)]);
        r.run(AppCommand::SaveAs(u));
        assert_eq!(r.prompts(), ["save-as logo.pdf.vrs in /d", "replace logo.vrs"]);
        assert_eq!(r.get(u).path.as_deref(), Some(Path::new("/d/logo.vrs")));
        // a name the user typed WITH .vrs (any case) was already confirmed by the Save dialog
        r.script([Ans::Pick(Some(p("/d/mark.VRS")))]);
        r.run(AppCommand::SaveAs(u));
        assert_eq!(r.prompts(), ["save-as logo.vrs in /d"]);
        assert_eq!(r.get(u).path.as_deref(), Some(Path::new("/d/mark.VRS")));
    }

    #[test]
    fn open_after_save_still_focuses_the_same_tab() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.aliases.insert(p("/Link/A.VRS"), p("/d/a.vrs"));
        let a = r.open("/d/a.vrs");
        let before = r.get(a).key.clone().unwrap();
        draw(r.ed(a), RED);
        r.run(AppCommand::Save(a));
        let after = r.get(a).key.clone().unwrap();
        assert_ne!(before.dev_ino, after.dev_ino, "the save replaced the inode");
        assert_eq!(after, r.s.key(Path::new("/d/a.vrs")), "the tab's key was refreshed after the save");
        r.run(AppCommand::NewBoard);
        r.open("/d/a.vrs");
        assert_eq!(r.active(), a, "the same path after a save is the same tab");
        r.run(AppCommand::ActivateNext);
        r.open("/Link/A.VRS");
        assert_eq!(r.active(), a, "…and so is an alias (the refreshed inode)");
        assert_eq!(r.s.loads.len(), 1);
        assert_eq!(r.ws.sessions().len(), 2);
        assert!(r.prompts().is_empty());
    }

    // ───────────── dirty + history across tabs ─────────────

    #[test]
    fn undo_back_to_saved_is_clean_end_to_end() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.run(AppCommand::Save(a));
        draw(r.ed(a), BLUE);
        assert!(r.get(a).is_dirty() && r.ws.tabs()[0].dirty);
        r.ed(a).execute(EditCommand::Undo);
        assert!(!r.get(a).is_dirty() && !r.ws.tabs()[0].dirty, "undo back to the saved content: no dot");
        // …so Close and Quit ask nothing
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true, ..Effect::default() });
        r.run(AppCommand::CloseDocument(a));
        assert!(r.prompts().is_empty());
        assert_eq!(
            r.names(),
            ["Untitled-2"],
            "closed without a prompt (Untitled-1 was the pristine tab the open replaced)"
        );
        assert_eq!(r.s.saves.len(), 1);
    }

    #[test]
    fn two_tabs_have_independent_history() {
        let mut r = Rig::new();
        let a = r.active();
        draw(r.ed(a), RED);
        r.run(AppCommand::NewBoard);
        let b = r.active();
        draw(r.ed(b), BLUE);
        let rev_b = r.get(b).editor.rev;
        r.run(AppCommand::ActivateDocument(a));
        assert_eq!(r.active(), a);
        assert_eq!(fills(&r.get(a).editor), [Some(RED)], "each tab shows only its own art");
        assert_eq!(fills(&r.get(b).editor), [Some(BLUE)]);
        r.ed(a).execute(EditCommand::Undo);
        assert!(r.get(a).editor.doc.paths.is_empty(), "⌘Z in A undoes A's square");
        assert_eq!(fills(&r.get(b).editor), [Some(BLUE)], "…and only A's");
        assert_eq!(r.get(b).editor.rev, rev_b);
        assert!(!r.get(a).is_dirty_exact() && r.get(b).is_dirty_exact());
        r.run(AppCommand::ActivateDocument(b));
        r.ed(b).execute(EditCommand::Undo);
        assert!(r.get(b).editor.doc.paths.is_empty());
        r.ed(a).execute(EditCommand::Redo);
        assert_eq!(fills(&r.get(a).editor), [Some(RED)], "A's redo stack survived the switches");
    }

    // ───────────── Close ─────────────

    #[test]
    fn close_clean_tab_no_prompt() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(RED));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        r.run(AppCommand::CloseDocument(a));
        assert!(r.prompts().is_empty());
        assert_eq!(r.ids(), [b]);
        assert_eq!(r.active(), b);
        r.run(AppCommand::CloseDocument(b));
        assert_eq!(r.names(), ["Untitled-2"], "the last tab closes to a fresh Untitled");
        r.run(AppCommand::CloseDocument(SessionId(999)));
        assert!(r.prompts().is_empty() && r.s.saves.is_empty());
    }

    #[test]
    fn close_dirty_tab_save_dont_save_cancel() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(a), RED);
        draw(r.ed(b), RED);
        // Cancel → kept, dirty
        r.script([Ans::Decide(SaveDecision::Cancel)]);
        r.run(AppCommand::CloseDocument(b));
        assert_eq!(r.prompts(), ["ask b"]);
        assert_eq!(r.ids(), [a, b]);
        assert!(r.get(b).is_dirty_exact());
        // Save → written, then closed
        r.script([Ans::Decide(SaveDecision::Save)]);
        r.run(AppCommand::CloseDocument(b));
        assert_eq!(r.prompts(), ["ask b"]);
        assert_eq!(r.ids(), [a]);
        assert_eq!(file_fills(&r.s, "/d/b.vrs"), [Some(BLUE), Some(RED)]);
        // Don't Save → closed, nothing written
        r.script([Ans::Decide(SaveDecision::DontSave)]);
        r.run(AppCommand::CloseDocument(a));
        assert_eq!(r.prompts(), ["ask a"]);
        assert_eq!(r.names(), ["Untitled-2"]);
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)]);
        assert_eq!(r.s.saves, [p("/d/b.vrs")]);
    }

    #[test]
    fn close_inactive_dirty_tab_saves_that_tab_only() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(a), RED);
        draw(r.ed(b), RED);
        assert_eq!(r.active(), b);
        r.script([Ans::Decide(SaveDecision::Save)]);
        r.run(AppCommand::CloseDocument(a));
        assert_eq!(r.prompts(), ["ask a"], "the prompt names the tab being closed");
        assert_eq!(r.s.saves, [p("/d/a.vrs")], "that tab is the one saved");
        assert_eq!(r.ids(), [b]);
        assert_eq!(r.active(), b, "the active tab never changed");
        assert!(r.get(b).is_dirty_exact(), "the active tab was not saved");
    }

    #[test]
    fn close_save_as_cancel_keeps_tab() {
        let mut r = Rig::new();
        let u = r.active();
        draw(r.ed(u), RED);
        r.run(AppCommand::NewBoard);
        r.run(AppCommand::ActivateDocument(u));
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(None)]);
        r.run(AppCommand::CloseDocument(u));
        assert_eq!(r.prompts(), ["ask Untitled-1", "save-as Untitled-1.vrs in -"]);
        assert_eq!(r.names(), ["Untitled-1", "Untitled-2"], "a cancelled Save As keeps the tab");
        assert_eq!(r.active(), u, "…and the focus");
        assert!(r.get(u).is_dirty_exact());
        // a save that fails and is cancelled keeps it too
        r.s.fail_save.insert(p("/ro/x.vrs"), u32::MAX);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(Some(p("/ro/x.vrs"))), Ans::Fail(SaveFailChoice::Cancel)]);
        r.run(AppCommand::CloseDocument(u));
        assert_eq!(
            r.prompts(),
            ["ask Untitled-1", "save-as Untitled-1.vrs in -", "failed Untitled-1: The disk is not writable"]
        );
        assert_eq!(r.ws.sessions().len(), 2);
        assert!(r.get(u).is_dirty_exact() && r.get(u).path.is_none());
    }

    #[test]
    fn close_inactive_untitled_suggests_its_own_name() {
        let mut r = Rig::new();
        let one = r.active();
        draw(r.ed(one), RED);
        r.run(AppCommand::NewBoard);
        let two = r.active();
        draw(r.ed(two), BLUE);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(Some(p("/d/one.vrs")))]);
        r.run(AppCommand::CloseDocument(one));
        assert_eq!(r.prompts(), ["ask Untitled-1", "save-as Untitled-1.vrs in -"]);
        assert_eq!(file_fills(&r.s, "/d/one.vrs"), [Some(RED)], "Untitled-1's art was saved");
        assert_eq!(r.ids(), [two]);
        assert!(r.get(two).is_dirty_exact() && r.get(two).path.is_none(), "Untitled-2 untouched");
    }

    // ───────────── Quit ─────────────

    #[test]
    fn quit_clean_asks_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.open("/d/a.vrs");
        r.run(AppCommand::NewBoard);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true, ..Effect::default() });
        assert!(r.prompts().is_empty() && r.s.saves.is_empty());
    }

    #[test]
    fn quit_cancel_on_second_keeps_everything_and_first_stays_saved() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        r.s.put("/d/c.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        let c = r.open("/d/c.vrs");
        draw(r.ed(b), RED);
        draw(r.ed(a), RED);
        assert_eq!(r.active(), c, "the clean tab is active when ⌘Q is pressed");
        r.script([Ans::Decide(SaveDecision::Save), Ans::Decide(SaveDecision::Cancel)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: false, ..Effect::default() });
        assert_eq!(r.prompts(), ["ask a 1/2", "ask b 2/2"], "tab order, Document i of n");
        assert_eq!(r.ids(), [a, b, c], "Cancel keeps every tab");
        assert!(!r.get(a).is_dirty_exact(), "the first document stays saved");
        assert!(r.get(b).is_dirty_exact(), "the second is still dirty");
        assert_eq!(r.s.saves, [p("/d/a.vrs")]);
        assert_eq!(r.active(), b, "the document behind the last prompt is active");
    }

    #[test]
    fn quit_dont_save_all_exits_and_writes_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let u = r.active();
        draw(r.ed(u), RED);
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.script([Ans::Decide(SaveDecision::DontSave), Ans::Decide(SaveDecision::DontSave)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true, ..Effect::default() });
        assert_eq!(r.prompts(), ["ask Untitled-1 1/2", "ask a 2/2"]);
        assert!(r.s.saves.is_empty(), "Don't Save writes nothing");
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)]);
        assert_eq!(r.ids(), [u, a], "no tab is destroyed by the lifecycle: the host exits");
    }

    #[test]
    fn quit_save_failure_aborts() {
        let mut r = Rig::new();
        r.s.put("/ro/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        r.s.fail_save.insert(p("/ro/a.vrs"), u32::MAX);
        let a = r.open("/ro/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(a), RED);
        draw(r.ed(b), RED);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Fail(SaveFailChoice::Cancel)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: false, ..Effect::default() });
        assert_eq!(r.prompts(), ["ask a 1/2", "failed a: The disk is not writable"], "b is never asked");
        assert!(r.get(a).is_dirty_exact() && r.get(b).is_dirty_exact());
        assert!(r.s.saves.is_empty());
        // Try Again that keeps failing, then Save As elsewhere, lets the quit continue
        r.script([
            Ans::Decide(SaveDecision::Save),
            Ans::Fail(SaveFailChoice::TryAgain),
            Ans::Fail(SaveFailChoice::SaveAs),
            Ans::Pick(Some(p("/d/a.vrs"))),
            Ans::Decide(SaveDecision::DontSave),
        ]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true, ..Effect::default() });
        assert_eq!(
            r.prompts(),
            [
                "ask a 1/2",
                "failed a: The disk is not writable",
                "failed a: The disk is not writable",
                "save-as a.vrs in /ro",
                "ask b 2/2",
            ]
        );
        assert_eq!(r.s.saves, [p("/d/a.vrs")]);
    }

    #[test]
    fn quit_untitled_save_as_cancel_aborts() {
        let mut r = Rig::new();
        let u = r.active();
        draw(r.ed(u), RED);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(None)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: false, ..Effect::default() });
        assert_eq!(r.prompts(), ["ask Untitled-1 1/1", "save-as Untitled-1.vrs in -"]);
        assert!(r.get(u).is_dirty_exact() && r.get(u).path.is_none());
    }

    // ───────────── app-wide state + routing ─────────────

    #[test]
    fn clipboard_survives_close_of_active_tab() {
        let mut r = Rig::new();
        let a = r.active();
        draw(r.ed(a), RED);
        {
            let ed = r.ed(a);
            ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
            ed.execute(EditCommand::Copy);
        }
        r.run(AppCommand::NewBoard);
        let b = r.active();
        assert_eq!(r.get(b).editor.clipboard().len(), 1, "the clipboard follows a tab switch");
        r.run(AppCommand::ActivateDocument(a));
        r.script([Ans::Decide(SaveDecision::DontSave)]);
        r.run(AppCommand::CloseDocument(a));
        assert_eq!(r.prompts(), ["ask Untitled-1"]);
        assert_eq!(r.active(), b);
        assert_eq!(r.get(b).editor.clipboard().len(), 1, "…and survives closing the tab that held it");
        r.ed(b).execute(EditCommand::Paste { offset: None });
        assert_eq!(fills(&r.get(b).editor), [Some(RED)], "pasting into the other tab works");
    }

    #[test]
    fn switch_reorder_and_window_commands_route_to_the_workspace() {
        let mut r = Rig::new();
        let a = r.active();
        r.run(AppCommand::NewBoard);
        let b = r.active();
        r.run(AppCommand::NewBoard);
        let c = r.active();
        r.run(AppCommand::ActivateNext);
        assert_eq!(r.active(), a, "Ctrl+Tab wraps around");
        r.run(AppCommand::ActivatePrevious);
        assert_eq!(r.active(), c);
        r.run(AppCommand::ReorderDocument(c, 0));
        assert_eq!(r.ids(), [c, a, b]);
        assert_eq!(r.run(AppCommand::Window(WindowCmd::Minimize)), Effect::default());
        assert_eq!(r.ids(), [c, a, b]);
        assert_eq!(r.active(), c);
        assert!(r.prompts().is_empty());
    }
    #[test]
    fn launch_and_new_from_start_preserve_the_internal_session_without_a_phantom_tab() {
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        assert!(r.ws.on_home());
        assert!(r.ws.visible_tabs().is_empty());
        assert_eq!(r.ws.sessions().len(), 1);
        r.run(AppCommand::NewBoard);
        assert!(!r.ws.on_home());
        assert_eq!(r.ws.visible_tabs().len(), 1);
        assert_eq!(r.ws.active().unwrap().display_name(), "Untitled-1");
        assert!(r.ws.active().unwrap().editor.doc.artboards.is_empty());
        assert!(!r.ws.active().unwrap().is_dirty_exact());
        r.run(AppCommand::CloseDocument(r.active()));
        assert!(r.ws.on_home());
        assert!(r.ws.visible_tabs().is_empty());
    }
    #[test]
    fn file_intent_opens_canvas_and_successful_recent_open_records_once() {
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(RED));
        let a = r.open("/d/a.vrs");
        assert!(!r.ws.on_home());
        assert_eq!(r.ws.visible_tabs().len(), 1);
        r.open("/d/b.vrs");
        r.run(AppCommand::Home);
        r.run(AppCommand::OpenRecent(p("/d/a.vrs")));
        assert_eq!(r.active(), a);
        assert!(!r.ws.on_home());
        assert_eq!(
            r.s.recent.entries().iter().map(|e| e.path.clone()).collect::<Vec<_>>(),
            [p("/d/a.vrs"), p("/d/b.vrs")]
        );
        assert_eq!(r.s.loads.len(), 2, "already-open recent is focused, never reloaded");
    }
    #[test]
    fn home_and_return_keep_documents_selection_view_and_dirty_state() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        r.ws.active_mut().unwrap().view = varos_core::geom::View { zoom: 3.0, pan: [31.0, 42.0] };
        let document = r.get(a).editor.doc.clone();
        let selection = r.get(a).editor.selected.clone();
        r.run(AppCommand::NewBoard);
        r.run(AppCommand::ActivateDocument(a));
        r.run(AppCommand::Home);
        assert_eq!(r.ws.document_target(), None);
        assert_eq!(r.ws.visible_tabs().len(), 2);
        r.run(AppCommand::ActivateDocument(a));
        assert_eq!(r.ws.document_target(), Some(a));
        assert!(r.get(a).editor.doc.content_eq(&document));
        assert_eq!(r.get(a).editor.selected, selection);
        assert_eq!(r.get(a).view.zoom, 3.0);
        assert_eq!(r.get(a).view.pan, [31.0, 42.0]);
        assert!(!r.get(a).is_dirty_exact());
    }
    #[test]
    fn failed_open_and_cancelled_missing_leave_start_recents_and_sessions_unchanged() {
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        r.s.recent.record(Path::new("/gone.vrs"), None, 1);
        let recent = r.s.recent.clone();
        let ids = r.ids();
        r.script([Ans::Locate(false)]);
        r.run(AppCommand::OpenRecent(p("/gone.vrs")));
        assert_eq!(r.s.recent, recent);
        assert_eq!(r.ids(), ids);
        assert!(r.ws.on_home());
        r.s.put("/bad.vrs", art(BLUE));
        r.s.fail_load.insert(p("/bad.vrs"));
        r.run(AppCommand::OpenRecent(p("/bad.vrs")));
        assert_eq!(r.s.recent, recent);
        assert_eq!(r.ids(), ids);
        assert!(r.ws.on_home());
    }
    #[test]
    fn locate_validates_before_relocating_and_cancel_changes_nothing() {
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        r.s.recent.record(Path::new("/gone.vrs"), None, 1);
        let original = r.s.recent.clone();
        r.script([Ans::Open(vec![])]);
        r.run(AppCommand::LocateRecent(p("/gone.vrs")));
        assert_eq!(r.s.recent, original);
        r.script([Ans::Open(vec![p("/bad.vrs")])]);
        r.run(AppCommand::LocateRecent(p("/gone.vrs")));
        assert_eq!(r.s.recent, original);
        assert!(r.ws.on_home());
        r.s.put("/found.vrs", art(BLUE));
        r.script([Ans::Open(vec![p("/found.vrs")])]);
        r.run(AppCommand::LocateRecent(p("/gone.vrs")));
        assert_eq!(r.s.recent.entries().len(), 1);
        assert_eq!(r.s.recent.entries()[0].path, p("/found.vrs"));
        assert!(!r.ws.on_home());
    }
    #[test]
    fn only_a_successful_save_records_a_recent() {
        let mut r = Rig::new();
        let id = r.active();
        r.script([Ans::Pick(None)]);
        r.run(AppCommand::Save(id));
        assert!(r.s.recent.entries().is_empty());
        r.s.fail_save.insert(p("/fail.vrs"), u32::MAX);
        r.script([Ans::Pick(Some(p("/fail.vrs"))), Ans::Fail(SaveFailChoice::Cancel)]);
        r.run(AppCommand::Save(id));
        assert!(r.s.recent.entries().is_empty());
        r.script([Ans::Pick(Some(p("/ok.vrs")))]);
        r.run(AppCommand::Save(id));
        assert_eq!(r.s.recent.entries()[0].path, p("/ok.vrs"));
    }
    #[test]
    fn external_change_cancel_save_as_and_explicit_replace() {
        use varos_app::storage::durable::Fingerprint;
        for choice in [ExternalChoice::Cancel, ExternalChoice::SaveAs, ExternalChoice::Replace] {
            let mut r = Rig::new();
            r.s.put("a.vrs", art(RED));
            r.s.fingerprints.insert(p("a.vrs"), Fingerprint { len: 10, modified: None });
            let id = r.open("a.vrs");
            draw(r.ed(id), BLUE);
            // Includes external deletion: an existing fingerprint becomes unavailable.
            r.s.fingerprints.remove(&p("a.vrs"));
            r.script([Ans::External(choice)]);
            match choice {
                ExternalChoice::SaveAs => r.script([Ans::Pick(Some(p("copy.vrs")))]),
                // "Replace Anyway" IS the confirmation: no second "Replace?" dialog follows.
                ExternalChoice::Replace | ExternalChoice::Cancel => {}
            }
            r.run(AppCommand::Save(id));
            assert!(r.prompts().iter().any(|s| s.starts_with("external")));
            assert!(!r.prompts().iter().any(|s| s.starts_with("replace")), "one confirmation only");
            match choice {
                ExternalChoice::Cancel => {
                    assert!(r.s.saves.is_empty());
                    assert!(r.get(id).is_dirty_exact());
                }
                ExternalChoice::SaveAs => {
                    assert_eq!(r.s.saves, vec![p("copy.vrs")]);
                    assert_eq!(r.s.doc("a.vrs"), &art(RED));
                }
                ExternalChoice::Replace => assert_eq!(r.s.saves, vec![p("a.vrs")]),
            }
        }
    }

    #[test]
    fn unconfirmed_save_adopts_path_keeps_dirty_and_cancels_close() {
        let mut r = Rig::new();
        let id = r.active();
        draw(r.ed(id), RED);
        r.s.unconfirmed = true;
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(Some(p("new.vrs")))]);
        r.run(AppCommand::CloseDocument(id));
        let s = r.get(id);
        assert_eq!(s.path, Some(p("new.vrs")));
        assert!(s.is_dirty_exact() && s.is_dirty());
        assert!(s.save_unconfirmed);
        assert!(r.s.recent.entries().is_empty());
        assert!(r.prompts().iter().any(|s| s.contains("Save needs confirmation")));
        r.s.unconfirmed = false;
        r.run(AppCommand::Save(id));
        assert!(!r.get(id).is_dirty_exact());
    }

    // ───────────── DFS S6 / F1 follow-up: background Save and Export ─────────────

    fn one(jobs: Vec<FileJob>) -> FileJob {
        assert_eq!(jobs.len(), 1, "{jobs:?}");
        jobs.into_iter().next().unwrap()
    }

    #[test]
    fn background_save_writes_the_snapshot_taken_at_cmd_s_and_a_later_edit_stays_dirty() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        let (_, jobs) = r.bg(AppCommand::Save(a));
        let job = one(jobs);
        assert!(r.s.saves.is_empty(), "nothing is written on the UI thread");
        assert!(r.get(a).saving.is_some());
        // ⌘S then ⌘Z (S1 FIFO rule): the file gets the pre-undo state, the tab stays dirty
        r.ed(a).undo();
        assert_eq!(fills(&r.get(a).editor), [Some(BLUE)]);
        r.land(job);
        assert_eq!(r.prompts(), Vec::<String>::new());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE), Some(RED)], "the snapshot taken at ⌘S");
        assert!(r.get(a).saving.is_none());
        assert!(r.get(a).is_dirty_exact(), "the undo after ⌘S is not in the file");
        // redo back to what was saved: clean (the checkpoint is the written snapshot)
        r.ed(a).redo();
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(r.s.recent.entries().len(), 1, "a landed save is recorded in Recent once");
    }

    #[test]
    fn background_save_of_an_untitled_tab_asks_first_and_adopts_the_path_only_when_it_lands() {
        let mut r = Rig::new();
        let a = r.active();
        draw(r.ed(a), RED);
        r.script([Ans::Pick(Some(p("/d/new.vrs")))]);
        let (_, jobs) = r.bg(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["save-as Untitled-1.vrs in -"]);
        let job = one(jobs);
        assert_eq!(r.get(a).path, None, "still Untitled while the save is on the worker");
        assert!(!r.get(a).is_pristine(), "a tab being saved is never replaced by an Open");
        r.land(job);
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/new.vrs")));
        assert!(!r.get(a).is_dirty_exact());
    }

    #[test]
    fn second_cmd_s_while_one_is_in_flight_coalesces_into_one_follow_up() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        let job = one(r.bg(AppCommand::Save(a)).1);
        draw(r.ed(a), BLUE);
        for _ in 0..3 {
            let (effect, jobs) = r.bg(AppCommand::Save(a));
            assert!(jobs.is_empty() && effect == Effect::default(), "no second writer while one is in flight");
        }
        let (effect, jobs) = r.land(job);
        assert!(jobs.is_empty());
        assert_eq!(effect.follow_up_saves, [a], "ONE follow-up, due now");
        assert!(r.get(a).is_dirty_exact());
        // the host queues it as a normal ⌘S: a fresh snapshot with the later edit
        let job = one(r.bg(AppCommand::Save(a)).1);
        let (effect, _) = r.land(job);
        assert!(effect.follow_up_saves.is_empty());
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE), Some(RED), Some(BLUE)]);
        assert_eq!(r.s.saves.len(), 2, "two writes for four ⌘S presses");
        // a repeated ⌘S with nothing new after it needs no follow-up
        draw(r.ed(a), RED);
        let job = one(r.bg(AppCommand::Save(a)).1);
        r.bg(AppCommand::Save(a));
        assert!(r.land(job).0.follow_up_saves.is_empty(), "nothing left to save");
    }

    #[test]
    fn background_save_failure_keeps_dirty_and_the_old_file_then_try_again_saves() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.s.fail_save.insert(p("/d/a.vrs"), 1);
        let job = one(r.bg(AppCommand::Save(a)).1);
        r.script([Ans::Fail(SaveFailChoice::TryAgain)]);
        let (_, jobs) = r.land(job);
        assert_eq!(r.prompts(), ["failed a: The disk is not writable"]);
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)], "the old file is intact");
        assert!(r.get(a).is_dirty_exact(), "a failed save keeps the tab dirty");
        let retry = one(jobs);
        assert!(r.get(a).saving.is_some(), "Try Again is a fresh background save");
        r.land(retry);
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE), Some(RED)]);
        // Cancel: nothing else happens
        draw(r.ed(a), BLUE);
        r.s.fail_save.insert(p("/d/a.vrs"), 1);
        let job = one(r.bg(AppCommand::Save(a)).1);
        r.script([Ans::Fail(SaveFailChoice::Cancel)]);
        assert!(r.land(job).1.is_empty());
        assert!(r.get(a).is_dirty_exact() && r.get(a).saving.is_none());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE), Some(RED)]);
    }

    #[test]
    fn background_save_still_asks_about_an_external_change_before_writing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.s.fingerprints.insert(p("/d/a.vrs"), varos_app::storage::durable::Fingerprint { len: 1, modified: None });
        r.script([Ans::External(ExternalChoice::Cancel)]);
        let (_, jobs) = r.bg(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["external a"]);
        assert!(jobs.is_empty() && r.get(a).saving.is_none(), "cancelled before anything was queued");
    }

    /// A two-board document opened from `path` (dirty after one more square).
    fn two_boards(r: &mut Rig, path: &str) -> SessionId {
        let mut doc = art(BLUE);
        doc.artboards.push(Artboard { x: 200.0, y: 0.0, w: 50.0, h: 80.0, name: "B".into(), ..Artboard::default() });
        r.s.put(path, doc);
        let a = r.open(path);
        draw(r.ed(a), RED);
        a
    }

    #[test]
    fn export_writes_a_pure_pdf_and_leaves_path_dirty_recents_and_the_vrs_untouched() {
        use varos_pdf::ExportScope;
        let mut r = Rig::new();
        let a = two_boards(&mut r, "/d/a.vrs");
        let recents_before = r.s.recent.entries().len();
        let (rev, saves) = (r.get(a).editor.rev, r.s.saves.len());
        r.script([Ans::Pick(Some(p("/out/a.pdf")))]);
        let (_, jobs) = r.bg(AppCommand::ExportPdf(a, ExportScope::AllVisibleArtboards));
        assert_eq!(r.prompts(), ["export a.pdf in /d"]);
        let job = one(jobs);
        r.land(job);
        assert_eq!(r.prompts(), ["notice Exported a.pdf"]);
        let s = r.get(a);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/a.vrs")), "the path never changes");
        assert!(s.is_dirty_exact(), "the dot stays: an export is not a save");
        assert_eq!(s.editor.rev, rev);
        assert!(s.exports.is_empty());
        assert_eq!(r.s.saves.len(), saves, "the .vrs is never written");
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)]);
        assert_eq!(r.s.recent.entries().len(), recents_before, "an export is never a Recent entry");
        // the PDF itself: one page per visible board, no editable model inside
        let bytes = &r.s.exported[&p("/out/a.pdf")];
        let pdf = lopdf::Document::load_mem(bytes).expect("a real PDF");
        assert_eq!(pdf.get_pages().len(), 2, "page count = artboards");
        assert!(!varos_pdf::has_embedded_model(bytes), "no embedded model");
        assert!(
            varos_pdf::load_vrs_bytes(bytes, &varos_core::format::Limits::DEFAULT).is_err(),
            "Varos refuses to open its own export as a document"
        );
    }

    #[test]
    fn export_of_a_pdf_document_suggests_name_export_and_refuses_its_own_file() {
        use varos_pdf::ExportScope;
        let mut r = Rig::new();
        let a = two_boards(&mut r, "/d/Poster.pdf");
        // the owner decision: `<name> export.pdf`; picking the open document's own file is refused
        r.script([Ans::Pick(Some(p("/d/Poster.pdf"))), Ans::Pick(None)]);
        let (_, jobs) = r.bg(AppCommand::ExportPdf(a, ExportScope::ActiveArtboard));
        assert_eq!(
            r.prompts(),
            ["export Poster export.pdf in /d", "notice Choose another name.", "export Poster export.pdf in /d"]
        );
        assert!(jobs.is_empty(), "cancelled: nothing queued");
        assert!(r.s.exported.is_empty() && r.get(a).exports.is_empty());
        // a typed name without .pdf gets it (and is asked about when that file exists)
        r.s.raw.insert(p("/out/flyer.pdf"), b"%PDF-1.7 someone else's".to_vec());
        r.script([Ans::Pick(Some(p("/out/flyer"))), Ans::Replace(true)]);
        let job = one(r.bg(AppCommand::ExportPdf(a, ExportScope::ActiveArtboard)).1);
        assert_eq!(r.prompts(), ["export Poster export.pdf in /d", "replace flyer.pdf"]);
        r.land(job);
        let pdf = lopdf::Document::load_mem(&r.s.exported[&p("/out/flyer.pdf")]).unwrap();
        assert_eq!(pdf.get_pages().len(), 1, "the active artboard only");
    }

    #[test]
    fn export_cancel_and_failure_write_nothing_and_say_so() {
        use varos_pdf::ExportScope;
        let mut r = Rig::new();
        let a = two_boards(&mut r, "/d/a.vrs");
        r.script([Ans::Pick(None)]);
        assert!(r.bg(AppCommand::ExportPdf(a, ExportScope::AllVisibleArtboards)).1.is_empty());
        assert_eq!(r.prompts(), ["export a.pdf in /d"]);
        // a failing disk: a plain notice, nothing written, the document unchanged
        r.s.fail_export = true;
        r.script([Ans::Pick(Some(p("/out/a.pdf")))]);
        let job = one(r.bg(AppCommand::ExportPdf(a, ExportScope::AllVisibleArtboards)).1);
        r.land(job);
        assert_eq!(r.prompts(), ["export a.pdf in /d", "notice Couldn't export PDF."]);
        assert!(r.s.exported.is_empty());
        assert!(r.get(a).is_dirty_exact() && r.get(a).exports.is_empty());
        // a scope that cannot export says why before any panel
        let (_, jobs) = r.bg(AppCommand::ExportPdf(a, ExportScope::ArtworkBounds));
        assert!(jobs.is_empty());
        assert_eq!(r.prompts(), ["notice Couldn't export PDF."]);
    }

    #[test]
    fn export_over_an_editable_varos_pdf_asks_once_more_before_replacing_it() {
        use varos_pdf::ExportScope;
        let mut r = Rig::new();
        let a = two_boards(&mut r, "/d/a.vrs");
        let native = varos_pdf::write_pdf(&art(BLUE)).unwrap();
        r.s.raw.insert(p("/out/old.pdf"), native.clone());
        r.script([Ans::Pick(Some(p("/out/old.pdf"))), Ans::Replace(false)]);
        let job = one(r.bg(AppCommand::ExportPdf(a, ExportScope::AllVisibleArtboards)).1);
        let (_, jobs) = r.land(job);
        assert_eq!(r.prompts(), ["export a.pdf in /d", "replace-editable old.pdf"]);
        assert!(jobs.is_empty() && r.s.exported.is_empty(), "Cancel keeps the editable file");
        r.script([Ans::Pick(Some(p("/out/old.pdf"))), Ans::Replace(true)]);
        let job = one(r.bg(AppCommand::ExportPdf(a, ExportScope::AllVisibleArtboards)).1);
        let confirmed = one(r.land(job).1);
        r.land(confirmed);
        assert_eq!(r.prompts(), ["export a.pdf in /d", "replace-editable old.pdf", "notice Exported old.pdf"]);
        assert!(!varos_pdf::has_embedded_model(&r.s.exported[&p("/out/old.pdf")]));
    }

    // ───────────── S4: files the OS hands us (Finder / Dock / `open`) ─────────────
    //
    // The host's path, minus AppKit: paths land in `os_open`'s queue, the `AboutToWait` drain turns
    // them into ONE `OpenPaths` (`os_open::route`), and the lifecycle runs it like any other open.

    /// What the host does at `AboutToWait`: drain → route → run. `false` = nothing was waiting.
    fn drain_os_opens(r: &mut Rig, q: &mut crate::os_open::OsOpenQueue) -> bool {
        match crate::os_open::route(q) {
            Some(batch) => {
                assert!(batch.raise_window, "an OS open always brings the window forward");
                r.run(batch.command);
                true
            }
            None => false,
        }
    }

    #[test]
    fn s4_cold_os_open_replaces_start_with_the_file_and_no_untitled() {
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        r.s.put("/d/b.vrs", art(BLUE));
        let mut q = crate::os_open::OsOpenQueue::default();
        q.push(p("/d/b.vrs"));
        assert!(drain_os_opens(&mut r, &mut q));
        assert!(!r.ws.on_home(), "the file shows instead of Start");
        assert_eq!(r.names(), ["b"], "no empty Untitled tab is created");
        assert_eq!(r.ws.visible_tabs().len(), 1);
        assert_eq!(fills(&r.get(r.active()).editor), [Some(BLUE)]);
        assert_eq!(r.s.recent.entries().len(), 1, "recorded in Recent like any open");
        assert!(r.prompts().is_empty());
    }

    #[test]
    fn s4_paths_arriving_before_the_host_is_ready_are_buffered_and_drained_once() {
        // the open event comes first (inside `run()`, before the first frame) …
        let mut q = crate::os_open::OsOpenQueue::default();
        q.push(p("/d/b.vrs"));
        // … then the host exists
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        r.s.put("/d/b.vrs", art(BLUE));
        assert!(drain_os_opens(&mut r, &mut q));
        assert!(!drain_os_opens(&mut r, &mut q), "the next drain finds nothing");
        assert_eq!(r.names(), ["b"]);
        assert_eq!(r.s.loads, [p("/d/b.vrs")], "read exactly once");
    }

    #[test]
    fn s4_warm_os_open_adds_a_tab_and_the_dirty_tab_survives() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(RED));
        r.s.put("/d/b.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), BLUE);
        let a_rev = r.get(a).editor.rev;
        let mut q = crate::os_open::OsOpenQueue::default();
        q.push(p("/d/b.vrs"));
        drain_os_opens(&mut r, &mut q);
        assert_eq!(r.names(), ["a", "b"]);
        let b = r.active();
        assert_ne!(b, a, "the new file is the active tab");
        assert!(r.get(a).is_dirty_exact(), "A keeps its unsaved changes");
        assert_eq!(r.get(a).editor.rev, a_rev, "A is not touched");
        assert!(!r.get(b).is_dirty_exact());
        assert!(r.prompts().is_empty(), "nothing is replaced, so nothing is asked");
    }

    #[test]
    fn s4_already_open_file_focuses_its_tab_without_reloading() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(RED));
        r.s.put("/d/b.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), BLUE); // dirty: a reload would lose this
        r.open("/d/b.vrs");
        let (ids, loads, a_rev) = (r.ids(), r.s.loads.len(), r.get(a).editor.rev);
        let mut q = crate::os_open::OsOpenQueue::default();
        q.push(p("/d/a.vrs"));
        drain_os_opens(&mut r, &mut q);
        assert_eq!(r.active(), a, "focused");
        assert_eq!(r.ids(), ids, "no new tab");
        assert_eq!(r.s.loads.len(), loads, "never reloaded");
        assert_eq!(r.get(a).editor.rev, a_rev);
        assert!(r.get(a).is_dirty_exact());
        assert!(r.prompts().is_empty());
    }

    #[test]
    fn s4_several_files_open_as_tabs_in_order() {
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        for (f, c) in [("/d/c.vrs", RED), ("/d/a.vrs", BLUE), ("/d/b.vrs", RED)] {
            r.s.put(f, art(c));
        }
        let mut q = crate::os_open::OsOpenQueue::default();
        for f in ["/d/c.vrs", "/d/a.vrs", "/d/c.vrs", "/d/b.vrs"] {
            q.push(p(f));
        }
        drain_os_opens(&mut r, &mut q);
        assert_eq!(r.names(), ["c", "a", "b"], "Finder's order, each file once, no Untitled");
        assert_eq!(r.ws.get(r.active()).unwrap().display_name(), "b", "the last one is in front");
        assert_eq!(r.s.loads.len(), 3);
        assert!(r.prompts().is_empty());
    }

    #[test]
    fn s4_refused_file_shows_the_notice_and_start_stays_start() {
        let mut r = Rig::new();
        r.ws = Workspace::start_page();
        r.s.put("/d/bad.vrs", art(BLUE));
        r.s.fail_load.insert(p("/d/bad.vrs"));
        let ids = r.ids();
        let mut q = crate::os_open::OsOpenQueue::default();
        q.push(p("/d/bad.vrs"));
        drain_os_opens(&mut r, &mut q);
        assert_eq!(r.prompts(), ["open-failed bad.vrs: The file is damaged"]);
        assert!(r.ws.on_home(), "Start stays Start");
        assert!(r.ws.visible_tabs().is_empty(), "no blank tab");
        assert_eq!(r.ids(), ids);
        assert!(r.s.recent.entries().is_empty(), "a refused file is not recorded in Recent");
    }

    #[test]
    fn s4_bad_file_among_good_ones_opens_the_rest_and_leaves_the_dirty_tab_alone() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(RED));
        r.s.put("/d/bad.vrs", art(RED));
        r.s.fail_load.insert(p("/d/bad.vrs"));
        r.s.put("/d/ملف عربي مع مسافات.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), BLUE);
        let mut q = crate::os_open::OsOpenQueue::default();
        q.push(p("/d/bad.vrs"));
        q.push(p("/d/ملف عربي مع مسافات.vrs"));
        drain_os_opens(&mut r, &mut q);
        assert_eq!(r.prompts(), ["open-failed bad.vrs: The file is damaged"]);
        assert_eq!(r.names(), ["a", "ملف عربي مع مسافات"]);
        assert!(r.get(a).is_dirty_exact());
    }

    // ───────────── review fixes: claimed paths (P0) and inert results for closed tabs (P2) ─────────────

    #[test]
    fn two_untitled_tabs_cannot_both_save_to_one_new_file() {
        let mut r = Rig::new();
        let a = r.active();
        draw(r.ed(a), RED);
        r.script([Ans::Pick(Some(p("/d/same.vrs")))]);
        let job = one(r.bg(AppCommand::Save(a)).1);
        r.prompts();
        r.run(AppCommand::NewBoard);
        let b = r.active();
        draw(r.ed(b), BLUE);
        // the file Untitled-1 is still writing is claimed: the existing "open in another tab" refusal
        r.script([Ans::Pick(Some(p("/d/same.vrs"))), Ans::Pick(None)]);
        assert!(r.bg(AppCommand::Save(b)).1.is_empty());
        assert_eq!(
            r.prompts(),
            [
                "save-as Untitled-2.vrs in -",
                "notice “Untitled-1” is open in another tab.",
                "save-as Untitled-2.vrs in -"
            ]
        );
        // opening that file (⌘O / Recent / Finder) focuses the tab still saving it; nothing loads
        r.run(AppCommand::OpenPaths(vec![p("/d/same.vrs")], OpenOrigin::OsHandoff));
        assert_eq!(r.active(), a);
        assert!(r.s.loads.is_empty());
        // the save fails: the claim is released, so tab B may use the name now
        r.s.fail_save.insert(p("/d/same.vrs"), 1);
        r.script([Ans::Fail(SaveFailChoice::Cancel)]);
        r.land(job);
        r.prompts();
        assert!(r.get(a).saving.is_none() && r.get(a).path.is_none());
        r.script([Ans::Pick(Some(p("/d/same.vrs")))]);
        assert_eq!(r.bg(AppCommand::Save(b)).1.len(), 1, "free again after the failure");
    }

    #[test]
    fn export_onto_a_file_a_tab_is_still_saving_is_refused() {
        use varos_pdf::ExportScope;
        let mut r = Rig::new();
        let a = two_boards(&mut r, "/d/a.vrs");
        r.run(AppCommand::NewBoard);
        let b = r.active();
        let doc = Arc::new(r.get(b).editor.doc.clone());
        r.ws.get_mut(b).unwrap().saving = Some(SaveInFlight {
            ticket: 1,
            dest: p("/out/claimed.pdf"),
            doc,
            follow_up: false,
            started: std::time::Instant::now(),
        });
        r.script([Ans::Pick(Some(p("/out/claimed.pdf"))), Ans::Pick(None)]);
        assert!(r.bg(AppCommand::ExportPdf(a, ExportScope::AllVisibleArtboards)).1.is_empty());
        assert_eq!(r.prompts(), ["export a.pdf in /d", "notice Choose another name.", "export a.pdf in /d"]);
    }

    #[test]
    fn a_closed_tabs_export_result_is_inert_but_a_written_pdf_is_reported_once() {
        use varos_pdf::ExportScope;
        let mut r = Rig::new();
        // pending "replace the editable document?" for a closed tab: nothing asked, nothing queued
        let a = two_boards(&mut r, "/d/a.vrs");
        r.s.raw.insert(p("/out/old.pdf"), varos_pdf::write_pdf(&art(BLUE)).unwrap());
        r.script([Ans::Pick(Some(p("/out/old.pdf")))]);
        let job = one(r.bg(AppCommand::ExportPdf(a, ExportScope::AllVisibleArtboards)).1);
        r.script([Ans::Decide(SaveDecision::DontSave)]);
        r.run(AppCommand::CloseDocument(a));
        r.prompts();
        let (_, jobs) = r.land(job);
        assert!(jobs.is_empty() && r.prompts().is_empty() && r.s.exported.is_empty());
        // a failure for a closed tab: silent
        let b = two_boards(&mut r, "/d/b.vrs");
        r.s.fail_export = true;
        r.script([Ans::Pick(Some(p("/out/b.pdf")))]);
        let job = one(r.bg(AppCommand::ExportPdf(b, ExportScope::AllVisibleArtboards)).1);
        r.script([Ans::Decide(SaveDecision::DontSave)]);
        r.run(AppCommand::CloseDocument(b));
        r.prompts();
        r.land(job);
        assert!(r.prompts().is_empty());
        // a PDF that WAS written: one neutral notice
        r.s.fail_export = false;
        let c = two_boards(&mut r, "/d/c.vrs");
        r.script([Ans::Pick(Some(p("/out/c.pdf")))]);
        let job = one(r.bg(AppCommand::ExportPdf(c, ExportScope::AllVisibleArtboards)).1);
        r.script([Ans::Decide(SaveDecision::DontSave)]);
        r.run(AppCommand::CloseDocument(c));
        r.prompts();
        r.land(job);
        assert_eq!(r.prompts(), ["notice Exported c.pdf"]);
        assert!(r.s.exported.contains_key(&p("/out/c.pdf")));
    }

    /// Round 2: the claim uses the ONE file key, so on the real disk a destination that differs only
    /// by case (on a case-insensitive volume) is the same claimed file; another folder is not.
    #[test]
    fn a_claimed_new_file_refuses_its_case_variant_on_the_real_disk() {
        use crate::file_ports::DiskStore;
        let dir = std::env::temp_dir().join(format!("varos-claim-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::create_dir_all(dir.join("other")).unwrap();
        let (upper, lower, elsewhere) = (dir.join("A.vrs"), dir.join("a.vrs"), dir.join("other").join("a.vrs"));
        #[cfg(unix)]
        let canon = std::fs::canonicalize(&dir).unwrap();
        // the expectation comes from the volume itself (pathconf), not from the key under test
        #[cfg(unix)]
        let same_on_this_volume = crate::file_ports::case_sensitive(&canon) != Some(true);
        #[cfg(not(unix))]
        let same_on_this_volume = true;
        let mut ws = Workspace::new();
        let a = ws.active_id().unwrap();
        draw(&mut ws.get_mut(a).unwrap().editor, RED);
        let b = ws.new_untitled();
        draw(&mut ws.get_mut(b).unwrap().editor, BLUE);
        let mut d = FakeDialogs::default();
        let mut jobs = Vec::new();
        let run = |ws: &mut Workspace, d: &mut FakeDialogs, jobs: &mut Vec<FileJob>, cmd| {
            Lifecycle { ws, dialogs: d, store: &mut DiskStore, jobs: Some(jobs) }.run(cmd);
        };
        d.answers.push_back(Ans::Pick(Some(upper.clone())));
        run(&mut ws, &mut d, &mut jobs, AppCommand::Save(a));
        assert_eq!(jobs.len(), 1, "A.vrs claimed; nothing written yet");
        d.answers.extend([Ans::Pick(Some(lower.clone())), Ans::Pick(None)]);
        run(&mut ws, &mut d, &mut jobs, AppCommand::Save(b));
        let refused = d.log.iter().any(|l| l == "notice “Untitled-1” is open in another tab.");
        assert_eq!(refused, same_on_this_volume, "{:?}", d.log);
        if same_on_this_volume {
            assert_eq!(jobs.len(), 1, "the case variant is refused: one writer");
        }
        d.log.clear();
        d.answers.clear();
        d.answers.push_back(Ans::Pick(Some(elsewhere)));
        if ws.get(b).unwrap().saving.is_none() {
            run(&mut ws, &mut d, &mut jobs, AppCommand::Save(b));
            assert!(ws.get(b).unwrap().saving.is_some(), "another folder is another file: allowed");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
