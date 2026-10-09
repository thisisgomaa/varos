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
    /// Slice 0.6: the Export sheet's ticket for this export (`AppCommand::ExportPdf`); every
    /// [`ExportEvent`] carries it, so a sheet only ever follows its own export.
    pub ticket: u64,
    pub dest: PathBuf,
    pub doc: Arc<Document>,
    pub plan: ExportPlan,
    /// The user already agreed to replace a destination that holds an editable Varos document.
    pub replace_confirmed: bool,
    /// Slice 0.6: the Export sheet's Cancel raises it; the writer checks it before every page and
    /// again before the file is replaced, so a cancelled export writes nothing.
    pub cancel: CancelFlag,
}

/// A shared cancel flag (one per export job). Two flags are equal only when they are the SAME flag.
#[derive(Clone, Debug, Default)]
pub struct CancelFlag(Arc<AtomicBool>);
impl CancelFlag {
    /// Ask the job to stop (the Export sheet's Cancel).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    #[cfg(test)]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    /// The flag `varos_pdf::export_pdf_bytes` checks.
    pub fn flag(&self) -> &AtomicBool {
        &self.0
    }
}
impl PartialEq for CancelFlag {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CancelFlag {}

/// Slice 0.6: what the Export sheet needs to know about an export it started (`lifecycle::Effect`,
/// delivered through `host::DocUi::export_event`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportEvent {
    /// The job is on the worker; the sheet's Cancel raises this flag.
    Started { sid: SessionId, ticket: u64, cancel: CancelFlag },
    /// The PDF was written (the rename happened — the commit boundary): the sheet shows its done
    /// state (Show in Finder).
    Finished { sid: SessionId, ticket: u64, dest: PathBuf },
    /// The sheet's Cancel stopped it before the rename: nothing was written, no temp is left.
    Cancelled { sid: SessionId, ticket: u64 },
    /// Nothing will be written: the save panel was cancelled, or the export failed (already told).
    Ended { sid: SessionId, ticket: u64 },
}
impl ExportEvent {
    pub fn sid(&self) -> SessionId {
        match self {
            ExportEvent::Started { sid, .. }
            | ExportEvent::Finished { sid, .. }
            | ExportEvent::Cancelled { sid, .. }
            | ExportEvent::Ended { sid, .. } => *sid,
        }
    }
    /// The export this event is about (the sheet's ticket).
    pub fn ticket(&self) -> u64 {
        match self {
            ExportEvent::Started { ticket, .. }
            | ExportEvent::Finished { ticket, .. }
            | ExportEvent::Cancelled { ticket, .. }
            | ExportEvent::Ended { ticket, .. } => *ticket,
        }
    }
}

/// One unit of background file work.
#[derive(Clone, Debug, PartialEq)]
pub enum FileJob {
    Save(SaveJob),
    /// Slice 0.6: File ▸ Save a Copy… — the same write as `Save`, but its result only releases the
    /// tab's save slot (path, checkpoint, dirty state and Recent stay), like the Bridge's copy.
    SaveCopy(SaveJob),
    Export(ExportJob),
    Bridge(Box<BridgeFileJob>),
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
    ExportedUnconfirmed(String),
    /// The destination holds an editable Varos document and the user has not agreed to replace it
    /// yet: nothing was written.
    NeedsReplaceConfirm,
    /// Nothing usable was written; the reason is plain English.
    Failed(String),
    /// The Export sheet's Cancel stopped it before the file was replaced: nothing was written.
    Cancelled,
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
    /// A Save a Copy… landed (`FileJob::SaveCopy`).
    CopySaved(SaveDone),
    Exported(ExportDone),
    Bridge {
        ticket: u64,
        copy: bool,
        result: varos_bridge::Reply,
        done: Option<Box<FileDone>>,
    },
}

/// Bridge policy is checked on the worker before using the existing safe file writer.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeFileJob {
    pub ticket: u64,
    pub inner: FileJob,
    pub home: PathBuf,
    /// Some = CURRENT save, checked against the load/last-save fingerprint.
    pub expected: Option<(PathBuf, Option<varos_app::storage::durable::Fingerprint>)>,
}
impl FileDone {
    /// A durable save: nothing to ask or tell, so the host applies it without settling the active tab
    /// (a background save landing must not end the user's drag).
    pub fn is_quiet(&self) -> bool {
        matches!(
            self,
            FileDone::Bridge { .. }
                | FileDone::Saved(SaveDone { result: Ok(SaveOutcome::Durable), .. })
                | FileDone::CopySaved(SaveDone { result: Ok(SaveOutcome::Durable), .. })
        )
    }

    /// What the worker delivers if `job` panicked (a bug): a failure carrying the job's identity.
    pub fn panicked(job: &FileJob) -> FileDone {
        match job {
            FileJob::Bridge(j) => FileDone::Bridge {
                ticket: j.ticket,
                copy: j.expected.is_none(),
                result: varos_bridge::Reply::failure(varos_bridge::Error::new("io_error", "file worker panicked")),
                done: None,
            },
            FileJob::Save(j) => FileDone::Saved(SaveDone {
                sid: j.sid,
                ticket: j.ticket,
                dest: j.dest.clone(),
                result: Err("Varos couldn't write the document.".into()),
            }),
            FileJob::SaveCopy(j) => FileDone::CopySaved(SaveDone {
                sid: j.sid,
                ticket: j.ticket,
                dest: j.dest.clone(),
                result: Err("Varos couldn't write the copy.".into()),
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
        FileJob::Bridge(j) => execute_bridge(*j, disk),
        FileJob::Save(j) => {
            let result = disk.save(&j.doc, &j.dest);
            FileDone::Saved(SaveDone { sid: j.sid, ticket: j.ticket, dest: j.dest, result })
        }
        FileJob::SaveCopy(j) => {
            let result = disk.save(&j.doc, &j.dest);
            FileDone::CopySaved(SaveDone { sid: j.sid, ticket: j.ticket, dest: j.dest, result })
        }
        FileJob::Export(j) => {
            let result = export_to(&j, disk);
            FileDone::Exported(ExportDone { job: j, result })
        }
    }
}

fn execute_bridge(mut j: BridgeFileJob, disk: &mut dyn DocStore) -> FileDone {
    let result = (|| -> Result<FileDone, varos_bridge::Error> {
        let dest = match &mut j.inner {
            FileJob::Save(s) | FileJob::SaveCopy(s) => &mut s.dest,
            FileJob::Export(e) => &mut e.dest,
            FileJob::Bridge(_) => unreachable!(),
        };
        if let Some((path, expected)) = &j.expected {
            if expected.is_none() || disk.fingerprint(path) != *expected {
                return Err(varos_bridge::Error::new(
                    "save_conflict",
                    "backing file changed or cannot be fingerprinted",
                ));
            }
        } else {
            *dest = varos_bridge::files::destination(dest, &j.home)?;
            // Refuse existing destinations for this temporary API: existing destinations must never be replaced.
            if disk.exists(dest) {
                return Err(varos_bridge::Error::new("save_conflict", "destination exists; choose a fresh filename"));
            }
        }
        Ok(match j.inner {
            FileJob::Save(s) | FileJob::SaveCopy(s) => {
                let result = disk.save_guarded(
                    &s.doc,
                    &s.dest,
                    j.expected.as_ref().and_then(|(_, fp)| fp.as_ref()),
                    j.expected.is_none(),
                )?;
                FileDone::Saved(SaveDone { sid: s.sid, ticket: s.ticket, dest: s.dest, result: Ok(result) })
            }
            FileJob::Export(e) => {
                let result = match varos_pdf::export_pdf_bytes(&e.doc, &e.plan, &AtomicBool::new(false)) {
                    Ok(bytes) => match disk.export_guarded(&e.dest, &bytes) {
                        Ok(SaveOutcome::Durable) => ExportResult::Exported,
                        Ok(SaveOutcome::ReplacedUnconfirmed(e)) => ExportResult::ExportedUnconfirmed(e),
                        Err(e) => return Err(e),
                    },
                    Err(e) => ExportResult::Failed(e.to_string()),
                };
                FileDone::Exported(ExportDone { job: e, result })
            }
            FileJob::Bridge(_) => unreachable!(),
        })
    })();
    let (reply, done) = match result {
        Err(e) => (varos_bridge::Reply::failure(e), None),
        Ok(done) => {
            let reply = match &done {
                FileDone::Saved(SaveDone { result: Ok(SaveOutcome::Durable), .. }) => {
                    varos_bridge::Reply::success(serde_json::json!({"saved":true,"durable":true}))
                }
                FileDone::Saved(SaveDone { result: Ok(SaveOutcome::ReplacedUnconfirmed(reason)), .. }) => {
                    varos_bridge::Reply::success(serde_json::json!({"saved":true,"durable":false,"reason":reason}))
                }
                FileDone::Saved(SaveDone { result: Err(reason), .. }) => {
                    varos_bridge::Reply::failure(varos_bridge::Error::new("io_error", reason))
                }
                FileDone::Exported(ExportDone { result: ExportResult::Exported, .. }) => {
                    varos_bridge::Reply::success(serde_json::json!({"exported":true,"durable":true}))
                }
                FileDone::Exported(ExportDone { result: ExportResult::ExportedUnconfirmed(reason), .. }) => {
                    varos_bridge::Reply::success(serde_json::json!({"exported":true,"durable":false,"reason":reason}))
                }
                FileDone::Exported(ExportDone { result: ExportResult::Failed(reason), .. }) => {
                    varos_bridge::Reply::failure(varos_bridge::Error::new("io_error", reason))
                }
                FileDone::Exported(_) => {
                    varos_bridge::Reply::failure(varos_bridge::Error::new("io_error", "PDF export refused or failed"))
                }
                FileDone::Bridge { .. } | FileDone::CopySaved(_) => unreachable!(),
            };
            (reply, Some(Box::new(done)))
        }
    };
    FileDone::Bridge { ticket: j.ticket, copy: j.expected.is_none(), result: reply, done }
}

/// The export body: the destination check (a bounded byte scan, off the UI thread), the pure PDF,
/// then one durable replace. Nothing is written unless the bytes were produced.
fn export_to(j: &ExportJob, disk: &mut dyn DocStore) -> ExportResult {
    if !j.replace_confirmed && disk.read_existing(&j.dest).is_some_and(|bytes| varos_pdf::has_embedded_model(&bytes)) {
        return ExportResult::NeedsReplaceConfirm;
    }
    // Slice 0.6: the Export sheet's Cancel raises `j.cancel`; it is checked before every page and
    // once more before the destination is replaced, so a cancelled export never touches the file.
    let bytes = match varos_pdf::export_pdf_bytes(&j.doc, &j.plan, j.cancel.flag()) {
        Ok(b) => b,
        Err(varos_pdf::ExportError::Cancelled) => return ExportResult::Cancelled,
        Err(e) => return ExportResult::Failed(e.to_string()),
    };
    // …and inside the durable write up to its rename (the commit boundary: after it, the PDF is
    // there and reported as exported, whatever the flag says)
    match disk.write_export(&j.dest, &bytes, j.cancel.flag()) {
        Ok(crate::lifecycle::ExportWrite::Written) => ExportResult::Exported,
        Ok(crate::lifecycle::ExportWrite::Cancelled) => ExportResult::Cancelled,
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
    #[cfg(unix)]
    #[test]
    fn bridge_file_jobs_roundtrip_pure_pdf_and_destination_refusals() {
        let root = std::env::temp_dir().join(format!("bridge-file-jobs-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let mut ed = varos_core::editor::Editor::new();
        ed.try_execute_created(varos_core::EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Rect,
            bounds: [0.0, 0.0, 40.0, 40.0],
            parent: None,
            fill: Some([1.0; 4]),
            stroke: None,
            stroke_width: 0.0,
            opacity: 1.0,
            name: None,
        })
        .unwrap();
        let doc = Arc::new(ed.doc.clone());
        let path = root.join("saved.vrs");
        let save = |dest: PathBuf, expected| {
            FileJob::Bridge(Box::new(BridgeFileJob {
                ticket: 9,
                inner: FileJob::Save(SaveJob { sid: SessionId(1), ticket: 9, dest, doc: doc.clone() }),
                home: root.clone(),
                expected,
            }))
        };
        // No backing file exists yet; fresh Save As is independent of the live editor.
        let fresh = FileJob::Bridge(Box::new(BridgeFileJob {
            ticket: 8,
            inner: FileJob::Save(SaveJob { sid: SessionId(1), ticket: 8, dest: path.clone(), doc: doc.clone() }),
            home: root.clone(),
            expected: None,
        }));
        let done = execute(fresh, &mut crate::file_ports::DiskStore);
        assert!(matches!(done, FileDone::Bridge { result: varos_bridge::Reply { ok: true, .. }, .. }));
        let reopened = varos_pdf::load_vrs(&path).unwrap();
        assert!(reopened.content_eq(&doc));
        let fp = crate::file_ports::DiskStore.fingerprint(&path).unwrap();
        let done = execute(save(path.clone(), Some((path.clone(), Some(fp)))), &mut crate::file_ports::DiskStore);
        assert!(matches!(done, FileDone::Bridge { result: varos_bridge::Reply { ok: true, .. }, .. }));
        let pdf = root.join("logo.pdf");
        let plan = varos_pdf::plan_pdf_export(&doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
        let job = FileJob::Bridge(Box::new(BridgeFileJob {
            ticket: 10,
            inner: FileJob::Export(ExportJob {
                sid: SessionId(1),
                dest: pdf.clone(),
                doc: doc.clone(),
                plan,
                replace_confirmed: false,
                cancel: CancelFlag::default(),
                ticket: 0,
            }),
            home: root.clone(),
            expected: None,
        }));
        let done = execute(job, &mut crate::file_ports::DiskStore);
        assert!(matches!(done, FileDone::Bridge { result: varos_bridge::Reply { ok: true, .. }, .. }));
        let bytes = std::fs::read(&pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(!varos_pdf::has_embedded_model(&bytes));
        let before = std::fs::read(&path).unwrap();
        let done = execute(save(path.clone(), None), &mut crate::file_ports::DiskStore);
        assert!(matches!(done, FileDone::Bridge { result: varos_bridge::Reply { ok: false, .. }, .. }));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }
}
