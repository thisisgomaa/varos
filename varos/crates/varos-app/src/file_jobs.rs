//! DFS S6-B/C and the F1 performance follow-up: the BACKGROUND file jobs — manual Save (⌘S, Save As)
//! and Export PDF — as plain data. The lifecycle decides everything that needs the user (which file,
//! the external-change prompt, the Export save panel) on the UI thread and then hands the host a
//! [`FileJob`] holding an owned snapshot of the document. The host runs it on the one generic I/O
//! worker (`storage::io_worker`, owned by `recovery_host`) through [`execute`], and the result comes
//! back as `AppCommand::FileDone`, applied on the UI thread by the lifecycle.
//!
//! Rules this module carries (work order `DFS_S4_S6_ASSOCIATION_EXPORT.md` §3.3 + the 2026-09-27 F1
//! follow-up):
//! - **A save writes the snapshot taken at ⌘S.** The checkpoint moves to THAT snapshot when it lands,
//!   so an edit (or an undo) made while it was on the worker keeps the tab dirty — S1's FIFO rule
//!   "⌘S then ⌘Z saves the pre-undo state" holds off-thread too.
//! - A second ⌘S while one is in flight never queues a second writer; it sets ONE follow-up
//!   ([`SaveInFlight::follow_up`]) that runs as a normal ⌘S after the first lands.
//! - Close / Close Tab / Quit / Save As wait for an in-flight save first (`host::save_barrier`).
//! - **Export mutates nothing**: it never touches the tab's path, checkpoint, dirty state, Recent or
//!   the `.vrs`; it writes only the PDF it was asked to write.
//! - No "saving…" flicker: the status text appears only once a job has run longer than
//!   [`STATUS_DELAY`] (plain text, no animation).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use varos_core::model::Document;
use varos_pdf::ExportPlan;

use crate::app_command::SessionId;
use crate::lifecycle::{DocStore, SaveOutcome};
use crate::workspace::{DocumentSession, Workspace};

/// How long a background save / export runs before the status bar says so (no flicker for small
/// documents).
pub const STATUS_DELAY: Duration = Duration::from_millis(300);

/// A manual save of `doc` (the snapshot taken when the user asked) to `dest`.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveJob {
    pub sid: SessionId,
    /// Matches the tab's [`SaveInFlight::ticket`]; a completion with another ticket is stale.
    pub ticket: u64,
    pub dest: PathBuf,
    pub doc: Arc<Document>,
}

/// A pure-PDF export of `doc` (snapshot) with `plan`'s pages to `dest`.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportJob {
    pub sid: SessionId,
    pub dest: PathBuf,
    pub doc: Arc<Document>,
    pub plan: ExportPlan,
    /// The user already agreed to replace a destination that holds an editable Varos document.
    pub replace_confirmed: bool,
}

/// One unit of background file work.
#[derive(Clone, Debug, PartialEq)]
pub enum FileJob {
    Save(SaveJob),
    Export(ExportJob),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaveDone {
    pub sid: SessionId,
    pub ticket: u64,
    pub dest: PathBuf,
    /// The store's answer (`Err` = a plain-English reason; the old file is intact).
    pub result: Result<SaveOutcome, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExportResult {
    Exported,
    /// The destination holds an editable Varos document and the user has not agreed to replace it
    /// yet: nothing was written.
    NeedsReplaceConfirm,
    /// Nothing usable was written; the reason is plain English.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportDone {
    /// The job as submitted (a confirmed retry resubmits it).
    pub job: ExportJob,
    pub result: ExportResult,
}

/// A finished background job, applied on the UI thread through `AppCommand::FileDone`.
#[derive(Clone, Debug, PartialEq)]
pub enum FileDone {
    Saved(SaveDone),
    Exported(ExportDone),
}

impl FileDone {
    /// A durable save: nothing to ask or tell, so the host applies it without settling the active tab
    /// (a background save landing must not end the user's drag).
    pub fn is_quiet(&self) -> bool {
        matches!(self, FileDone::Saved(SaveDone { result: Ok(SaveOutcome::Durable), .. }))
    }

    /// What the worker delivers if `job` panicked (a bug): a failure carrying the job's identity.
    pub fn panicked(job: &FileJob) -> FileDone {
        match job {
            FileJob::Save(j) => FileDone::Saved(SaveDone {
                sid: j.sid,
                ticket: j.ticket,
                dest: j.dest.clone(),
                result: Err("Varos couldn't write the document.".into()),
            }),
            FileJob::Export(j) => FileDone::Exported(ExportDone {
                job: j.clone(),
                result: ExportResult::Failed("Varos couldn't write the PDF.".into()),
            }),
        }
    }
}

/// The UI thread's record of a manual save running on the worker (`DocumentSession::saving`).
#[derive(Clone, Debug)]
pub struct SaveInFlight {
    pub ticket: u64,
    pub dest: PathBuf,
    /// The snapshot being written: it becomes the checkpoint when the save lands.
    pub doc: Arc<Document>,
    /// ⌘S was pressed again while this one was in flight: save once more after it lands.
    pub follow_up: bool,
    pub started: Instant,
}

/// A fresh, never-reused save ticket.
pub fn next_ticket() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Run one job (on the worker; inline only when the worker is unavailable). Only `save`,
/// `read_existing` and `write_export` of `disk` are used — never `remember`: an export is never a
/// Recent entry, and a save's Recent entry is recorded by the lifecycle when its result is applied.
pub fn execute(job: FileJob, disk: &mut dyn DocStore) -> FileDone {
    match job {
        FileJob::Save(j) => {
            let result = disk.save(&j.doc, &j.dest);
            FileDone::Saved(SaveDone { sid: j.sid, ticket: j.ticket, dest: j.dest, result })
        }
        FileJob::Export(j) => {
            let result = export_to(&j, disk);
            FileDone::Exported(ExportDone { job: j, result })
        }
    }
}

/// The export body: the destination check (a bounded byte scan, off the UI thread), the pure PDF,
/// then one durable replace. Nothing is written unless the bytes were produced.
fn export_to(j: &ExportJob, disk: &mut dyn DocStore) -> ExportResult {
    if !j.replace_confirmed && disk.read_existing(&j.dest).is_some_and(|bytes| varos_pdf::has_embedded_model(&bytes)) {
        return ExportResult::NeedsReplaceConfirm;
    }
    // No Cancel control exists yet (the S6 cancel contract needs a real mechanism, not a decoration),
    // so the flag is never raised.
    let bytes = match varos_pdf::export_pdf_bytes(&j.doc, &j.plan, &AtomicBool::new(false)) {
        Ok(b) => b,
        Err(e) => return ExportResult::Failed(e.to_string()),
    };
    match disk.write_export(&j.dest, &bytes) {
        Ok(()) => ExportResult::Exported,
        Err(reason) => ExportResult::Failed(reason),
    }
}

/// The Export save panel's suggested name: `<name>.pdf`, except for a document that is itself a
/// `.pdf` (owner decision 2026-09-27): `<name> export.pdf`, so the suggestion never names the open
/// document's own file. A recovered copy uses its original name; an untitled tab its tab name.
pub fn default_export_name(s: &DocumentSession) -> String {
    match (&s.path, &s.recovered) {
        (Some(p), _) => export_name_for(p),
        (None, Some(source)) => export_name_for(Path::new(&source.name)),
        (None, None) => format!("{}.pdf", s.display_name()),
    }
}

/// [`default_export_name`] for a file name / path.
pub fn export_name_for(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled".into());
    if is_pdf(path) {
        format!("{stem} export.pdf")
    } else {
        format!("{stem}.pdf")
    }
}

/// `true` when the path ends in `.pdf` (any case).
pub fn is_pdf(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// The chosen export name with `.pdf` forced, keeping every part of it (`logo.png` → `logo.png.pdf`);
/// the flag says whether `.pdf` was appended (then the panel never asked about that name).
pub fn with_pdf(p: PathBuf) -> (PathBuf, bool) {
    if is_pdf(&p) {
        return (p, false);
    }
    let mut s = p.into_os_string();
    s.push(".pdf");
    (PathBuf::from(s), true)
}

/// The status-bar text for background file work that has run longer than [`STATUS_DELAY`]: the
/// active tab first, then the others. Empty when there is nothing to say.
pub fn status_text(ws: &Workspace, now: Instant) -> String {
    let due = |t: Instant| now.saturating_duration_since(t) >= STATUS_DELAY;
    let active = ws.active_id();
    let mut order: Vec<&DocumentSession> = ws.sessions().iter().filter(|s| Some(s.id) == active).collect();
    order.extend(ws.sessions().iter().filter(|s| Some(s.id) != active));
    for s in order {
        if s.saving.as_ref().is_some_and(|f| due(f.started)) {
            return format!("Saving “{}”…", s.display_name());
        }
        if s.exports.first().is_some_and(|&t| due(t)) {
            return "Exporting PDF…".into();
        }
    }
    String::new()
}

/// When [`status_text`] will next change on its own (a job crossing [`STATUS_DELAY`]), if ever.
pub fn next_status_wake(ws: &Workspace, now: Instant) -> Option<Instant> {
    ws.sessions()
        .iter()
        .flat_map(|s| s.saving.as_ref().map(|f| f.started).into_iter().chain(s.exports.first().copied()))
        .map(|t| t + STATUS_DELAY)
        .filter(|&at| at > now)
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_export_name_follows_the_document_and_avoids_its_own_pdf() {
        assert_eq!(export_name_for(Path::new("/w/Logo.vrs")), "Logo.pdf");
        // owner decision: a document that IS a .pdf never gets its own file name suggested
        assert_eq!(export_name_for(Path::new("/w/Poster.pdf")), "Poster export.pdf");
        assert_eq!(export_name_for(Path::new("/w/Poster.PDF")), "Poster export.pdf");
        assert_eq!(export_name_for(Path::new("/w/شعار المشروع.vrs")), "شعار المشروع.pdf");
        let mut ws = Workspace::new();
        let id = ws.new_untitled();
        let s = ws.get(id).unwrap();
        assert_eq!(default_export_name(s), format!("{}.pdf", s.display_name()));
    }

    #[test]
    fn with_pdf_forces_the_extension_and_keeps_the_name() {
        assert_eq!(with_pdf(PathBuf::from("/w/a.pdf")), (PathBuf::from("/w/a.pdf"), false));
        assert_eq!(with_pdf(PathBuf::from("/w/a.PDF")), (PathBuf::from("/w/a.PDF"), false));
        assert_eq!(with_pdf(PathBuf::from("/w/a")), (PathBuf::from("/w/a.pdf"), true));
        assert_eq!(with_pdf(PathBuf::from("/w/a.vrs")), (PathBuf::from("/w/a.vrs.pdf"), true));
    }

    #[test]
    fn status_appears_only_after_the_delay_and_wakes_once_for_it() {
        let mut ws = Workspace::new();
        let id = ws.new_untitled();
        let t0 = Instant::now();
        assert_eq!(status_text(&ws, t0), "");
        assert_eq!(next_status_wake(&ws, t0), None);
        let doc = Arc::new(ws.get(id).unwrap().editor.doc.clone());
        ws.get_mut(id).unwrap().saving =
            Some(SaveInFlight { ticket: 1, dest: "/w/a.vrs".into(), doc, follow_up: false, started: t0 });
        // a quick save never shows a "saving" state
        assert_eq!(status_text(&ws, t0 + Duration::from_millis(299)), "");
        assert_eq!(next_status_wake(&ws, t0), Some(t0 + STATUS_DELAY));
        let name = ws.get(id).unwrap().display_name();
        assert_eq!(status_text(&ws, t0 + STATUS_DELAY), format!("Saving “{name}”…"));
        assert_eq!(next_status_wake(&ws, t0 + STATUS_DELAY), None, "no wake once the text is up");
        ws.get_mut(id).unwrap().saving = None;
        ws.get_mut(id).unwrap().exports.push(t0);
        assert_eq!(status_text(&ws, t0 + Duration::from_millis(100)), "");
        assert_eq!(status_text(&ws, t0 + Duration::from_secs(1)), "Exporting PDF…");
    }
}
