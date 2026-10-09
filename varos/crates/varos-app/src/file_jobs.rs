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
    pub blobs: std::sync::Arc<varos_core::images::BlobStore>,
    pub sid: SessionId,
    /// Matches the tab's [`SaveInFlight::ticket`]; a completion with another ticket is stale.
    pub ticket: u64,
    pub dest: PathBuf,
    pub doc: Arc<Document>,
}

/// A pure-PDF export of `doc` (snapshot) with `plan`'s pages to `dest`.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportJob {
    pub blobs: std::sync::Arc<varos_core::images::BlobStore>,
    pub pdf_options: Box<varos_pdf::PdfOptions>,
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

/// One card × format, executed by the existing IO worker.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenJob {
    pub job: ExportJob,
    pub asset: varos_raster::export::Asset,
    pub options: varos_raster::export::Options,
    /// Sheet exports choose a fresh numbered name; Bridge refuses collisions.
    pub collision_names: bool,
    /// Additional pages from an all-artboards Bridge request (sheet submits one job per card).
    pub additional: Vec<varos_raster::export::Asset>,
    // ---- Lane C ----
    pub svg_options: varos_core::svg::options::Options,
    pub additional_jobs: Vec<ScreenJob>,
    pub folder_root: Option<PathBuf>,
}

/// A shared cancel flag (one per export job). Two flags are equal only when they are the SAME flag.
#[derive(Clone, Debug, Default)]
pub struct CancelFlag(Arc<AtomicBool>);
impl CancelFlag {
    pub fn from_shared(flag: Arc<AtomicBool>) -> Self {
        Self(flag)
    }
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
    /// state (Show in Finder) with the export's `report` notes (what was simplified or left out).
    Finished { sid: SessionId, ticket: u64, dest: PathBuf, report: varos_core::ExportReport },
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
    // ---- Lane H ----
    Import(crate::import_jobs::Job),
    // ---- w2-images ----
    Image(Box<crate::image_jobs::Job>),
    Template(crate::template_jobs::Job),
    Save(SaveJob),
    /// Slice 0.6: File ▸ Save a Copy… — the same write as `Save`, but its result only releases the
    /// tab's save slot (path, checkpoint, dirty state and Recent stay), like the Bridge's copy.
    SaveCopy(SaveJob),
    Export(ExportJob),
    Screen(Box<ScreenJob>),
    Bridge(Box<BridgeFileJob>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaveDone {
    pub published: Option<varos_app::storage::durable::Fingerprint>,
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
    pub report: varos_core::ExportReport,
}

/// A finished background job, applied on the UI thread through `AppCommand::FileDone`.
#[derive(Clone, Debug, PartialEq)]
pub enum FileDone {
    // ---- Lane H ----
    Import(crate::import_jobs::Done),
    // ---- w2-images ----
    Image(Box<crate::image_jobs::Done>),
    Template(crate::template_jobs::Done),
    Autosaved(Box<crate::autosave_io::Done>),
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
        if matches!(self, FileDone::Template(crate::template_jobs::Done { result: Ok(Some(_)), .. })) {
            return false; // opening a tab must settle the human gesture and refresh document UI
        }
        matches!(
            self,
            FileDone::Bridge { .. }
                | FileDone::Template(_)
                | FileDone::Autosaved(_)
                | FileDone::Saved(SaveDone { result: Ok(SaveOutcome::Durable), .. })
                | FileDone::CopySaved(SaveDone { result: Ok(SaveOutcome::Durable), .. })
        )
    }

    /// What the worker delivers if `job` panicked (a bug): a failure carrying the job's identity.
    pub fn panicked(job: &FileJob) -> FileDone {
        match job {
            FileJob::Import(j) => FileDone::Import(crate::import_jobs::Done {
                job: j.clone(),
                result: Err("Import worker panicked".into()),
            }),
            FileJob::Image(j) => FileDone::Image(Box::new(crate::image_jobs::Done {
                job: j.as_ref().clone(),
                result: Err("Image worker panicked".into()),
            })),
            FileJob::Template(j) => FileDone::Template(crate::template_jobs::Done {
                ticket: j.ticket,
                result: Err(varos_bridge::Error::new("io_error", "template worker panicked")),
            }),
            FileJob::Bridge(j) => FileDone::Bridge {
                ticket: j.ticket,
                copy: j.expected.is_none(),
                result: varos_bridge::Reply::failure(varos_bridge::Error::new("io_error", "file worker panicked")),
                done: None,
            },
            FileJob::Save(j) => FileDone::Saved(SaveDone {
                published: None,
                sid: j.sid,
                ticket: j.ticket,
                dest: j.dest.clone(),
                result: Err("Varos couldn't write the document.".into()),
            }),
            FileJob::SaveCopy(j) => FileDone::CopySaved(SaveDone {
                published: None,
                sid: j.sid,
                ticket: j.ticket,
                dest: j.dest.clone(),
                result: Err("Varos couldn't write the copy.".into()),
            }),
            FileJob::Screen(j) => FileDone::Exported(ExportDone {
                job: j.job.clone(),
                result: ExportResult::Failed("Export worker panicked.".into()),
                report: Default::default(),
            }),
            FileJob::Export(j) => FileDone::Exported(ExportDone {
                job: j.clone(),
                result: ExportResult::Failed("Varos couldn't write the PDF.".into()),
                report: varos_core::ExportReport::default(),
            }),
        }
    }
}

/// The UI thread's record of a manual save running on the worker (`DocumentSession::saving`).
#[derive(Clone, Debug)]
pub struct SaveInFlight {
    pub blobs: Arc<varos_core::images::BlobStore>,
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
        FileJob::Import(j) => FileDone::Import(crate::import_jobs::execute(j)),
        FileJob::Image(j) => FileDone::Image(Box::new(crate::image_jobs::execute(*j))),
        FileJob::Template(j) => FileDone::Template(crate::template_jobs::execute(j)),
        FileJob::Bridge(j) => execute_bridge(*j, disk),
        FileJob::Save(j) => {
            let (result, published) = match disk.save_resources_published(&j.doc, &j.blobs, &j.dest) {
                Ok((outcome, published)) => (Ok(outcome), published),
                Err(reason) => (Err(reason), None),
            };
            FileDone::Saved(SaveDone { sid: j.sid, ticket: j.ticket, dest: j.dest, result, published })
        }
        FileJob::SaveCopy(j) => {
            let (result, published) = match disk.save_resources_published(&j.doc, &j.blobs, &j.dest) {
                Ok((outcome, published)) => (Ok(outcome), published),
                Err(reason) => (Err(reason), None),
            };
            FileDone::CopySaved(SaveDone { sid: j.sid, ticket: j.ticket, dest: j.dest, result, published })
        }
        FileJob::Screen(j) => execute_screen(*j, disk, false),
        FileJob::Export(j) => {
            let (result, report) = export_to(&j, disk);
            FileDone::Exported(ExportDone { job: j, result, report })
        }
    }
}

fn execute_bridge(mut j: BridgeFileJob, disk: &mut dyn DocStore) -> FileDone {
    let result = (|| -> Result<FileDone, varos_bridge::Error> {
        // ---- Lane C: authorize the root before creating Advanced sub-folders ----
        if let FileJob::Screen(screen) = &mut j.inner {
            crate::export_folders::prepare_screen(screen, &j.home)?;
        }
        let dest = match &mut j.inner {
            FileJob::Save(s) | FileJob::SaveCopy(s) => &mut s.dest,
            FileJob::Export(e) => &mut e.dest,
            FileJob::Screen(e) => &mut e.job.dest,
            FileJob::Image(_) | FileJob::Bridge(_) | FileJob::Template(_) | FileJob::Import(_) => unreachable!(),
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
                let (result, published) = disk.save_resources_guarded(
                    &s.doc,
                    &s.blobs,
                    &s.dest,
                    j.expected.as_ref().and_then(|(_, fp)| fp.as_ref()),
                    j.expected.is_none(),
                )?;
                FileDone::Saved(SaveDone { sid: s.sid, ticket: s.ticket, dest: s.dest, result: Ok(result), published })
            }
            FileJob::Screen(e) => execute_screen(*e, disk, true),
            FileJob::Export(e) => {
                let (result, report) = match varos_pdf::images::export_with_options(
                    &e.doc,
                    &e.blobs,
                    &e.plan,
                    &e.pdf_options,
                    &AtomicBool::new(false),
                ) {
                    Ok((bytes, report)) => match disk.export_guarded(&e.dest, &bytes) {
                        Ok(SaveOutcome::Durable) => (ExportResult::Exported, report),
                        Ok(SaveOutcome::ReplacedUnconfirmed(e)) => (ExportResult::ExportedUnconfirmed(e), report),
                        Err(e) => return Err(e),
                    },
                    Err(e) => (ExportResult::Failed(e.to_string()), varos_core::ExportReport::default()),
                };
                FileDone::Exported(ExportDone { job: e, result, report })
            }
            FileJob::Image(_) | FileJob::Bridge(_) | FileJob::Template(_) | FileJob::Import(_) => unreachable!(),
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
                FileDone::Exported(ExportDone { result: ExportResult::Exported, report, .. }) => {
                    varos_bridge::Reply::success(serde_json::json!({"exported":true,"durable":true,"report":report}))
                }
                FileDone::Exported(ExportDone {
                    result: ExportResult::ExportedUnconfirmed(reason), report, ..
                }) => varos_bridge::Reply::success(
                    serde_json::json!({"exported":true,"durable":false,"reason":reason,"report":report}),
                ),
                FileDone::Exported(ExportDone { result: ExportResult::Failed(reason), .. }) => {
                    varos_bridge::Reply::failure(varos_bridge::Error::new("io_error", reason))
                }
                FileDone::Exported(_) => {
                    varos_bridge::Reply::failure(varos_bridge::Error::new("io_error", "PDF export refused or failed"))
                }
                FileDone::Autosaved(_)
                | FileDone::Bridge { .. }
                | FileDone::CopySaved(_)
                | FileDone::Template(_)
                | FileDone::Import(_)
                | FileDone::Image(_) => {
                    unreachable!()
                }
            };
            (reply, Some(Box::new(done)))
        }
    };
    FileDone::Bridge { ticket: j.ticket, copy: j.expected.is_none(), result: reply, done }
}

/// The export body: the destination check (a bounded byte scan, off the UI thread), the pure PDF,
/// then one durable replace. Nothing is written unless the bytes were produced.
fn export_to(j: &ExportJob, disk: &mut dyn DocStore) -> (ExportResult, varos_core::ExportReport) {
    if !j.replace_confirmed && disk.read_existing(&j.dest).is_some_and(|bytes| varos_pdf::has_embedded_model(&bytes)) {
        return (ExportResult::NeedsReplaceConfirm, varos_core::ExportReport::default());
    }
    // Slice 0.6: the Export sheet's Cancel raises `j.cancel`; the writer checks it inside every page
    // (`varos_pdf` write loop), so a cancelled export never produces bytes…
    let (bytes, report) =
        match varos_pdf::images::export_with_options(&j.doc, &j.blobs, &j.plan, &j.pdf_options, j.cancel.flag()) {
            Ok(b) => b,
            Err(ref e) if e == "The export was cancelled." => {
                return (ExportResult::Cancelled, varos_core::ExportReport::default())
            }
            Err(e) => return (ExportResult::Failed(e.to_string()), varos_core::ExportReport::default()),
        };
    // …and inside the durable write up to its rename (the commit boundary: after it, the PDF is
    // there and reported as exported, whatever the flag says)
    let result = match disk.write_export(&j.dest, &bytes, j.cancel.flag()) {
        Ok(crate::lifecycle::ExportWrite::Written) => ExportResult::Exported,
        Ok(crate::lifecycle::ExportWrite::Unconfirmed(reason)) => ExportResult::ExportedUnconfirmed(reason),
        Ok(crate::lifecycle::ExportWrite::Cancelled) => ExportResult::Cancelled,
        Err(reason) => ExportResult::Failed(reason),
    };
    (result, report)
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
        if s.autosave.ticket.is_some() {
            return "Autosaving…".into();
        }
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

/// Include publication warnings in the existing Done/report seam, without changing event shape.
pub fn durability_note(report: &mut varos_core::ExportReport, path: &Path, reason: &str) {
    report.notes.push(varos_core::ExportNote {
        kind: "durability".into(),
        object_id: None,
        message: format!("Exported {}, but durability could not be confirmed: {reason}", path.display()),
    });
}

/// Encode once through the shared library; honour cancellation through the durable commit boundary.
fn execute_screen(mut screen: ScreenJob, disk: &mut dyn DocStore, guarded: bool) -> FileDone {
    let extra = std::mem::take(&mut screen.additional);
    let additional_jobs = std::mem::take(&mut screen.additional_jobs);
    if extra.is_empty() && additional_jobs.is_empty() {
        return execute_screen_one(screen, disk, guarded);
    }
    let parent = screen.job.dest.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut jobs = vec![screen.clone()];
    for asset in extra {
        let mut page = screen.clone();
        page.job.dest = parent.join(varos_raster::export::file_name(&asset.name, "", page.options.format, 1));
        page.job.doc = asset.doc.clone();
        page.asset = asset;
        jobs.push(page);
    }
    jobs.extend(additional_jobs);
    let mut names = std::collections::HashSet::new();
    if jobs.iter().any(|j| !names.insert(j.job.dest.clone()) || disk.exists(&j.job.dest)) {
        return FileDone::Exported(ExportDone {
            job: screen.job,
            result: ExportResult::Failed(
                "One export destination already exists or duplicates another page name; nothing written.".into(),
            ),
            report: Default::default(),
        });
    }
    let mut report = varos_core::ExportReport::default();
    let mut warnings = Vec::new();
    for (index, job) in jobs.into_iter().enumerate() {
        let FileDone::Exported(done) = execute_screen_one(job, disk, guarded) else { unreachable!() };
        report.notes.extend(done.report.notes);
        if let ExportResult::ExportedUnconfirmed(reason) = &done.result {
            durability_note(&mut report, &done.job.dest, reason);
            warnings.push(format!("{}: {reason}", done.job.dest.display()));
        }
        if !matches!(done.result, ExportResult::Exported | ExportResult::ExportedUnconfirmed(_)) {
            return FileDone::Exported(ExportDone {
                job: screen.job,
                result: ExportResult::Failed(format!(
                    "Export stopped after {index} files: {:?}{}",
                    done.result,
                    if warnings.is_empty() {
                        String::new()
                    } else {
                        format!("; durability unconfirmed: {}", warnings.join("; "))
                    }
                )),
                report,
            });
        }
        if index > 0 {
            report.notes.push(varos_core::ExportNote {
                kind: "file".into(),
                object_id: None,
                message: format!("Exported {}", done.job.dest.display()),
            });
        }
    }
    let result = if warnings.is_empty() {
        ExportResult::Exported
    } else {
        ExportResult::ExportedUnconfirmed(warnings.join("; "))
    };
    FileDone::Exported(ExportDone { job: screen.job, result, report })
}

fn execute_screen_one(mut screen: ScreenJob, disk: &mut dyn DocStore, guarded: bool) -> FileDone {
    let (result, report) = (|| {
        if screen.job.cancel.flag().load(Ordering::Relaxed) {
            return (ExportResult::Cancelled, Default::default());
        }
        let encoded = if screen.options.format == varos_raster::export::Format::Pdf {
            varos_pdf::images::export_with_options(
                &screen.asset.doc,
                &screen.job.blobs,
                &screen.job.plan,
                &screen.job.pdf_options,
                screen.job.cancel.flag(),
            )
            .map(|(bytes, report)| varos_raster::export::Output {
                name: varos_raster::export::file_name(&screen.asset.name, "", screen.options.format, 1),
                bytes,
                report,
            })
            .map_err(|e| e.to_string())
        } else {
            varos_raster::export::encode_with_images_and_svg_options(
                &screen.asset,
                &screen.options,
                &screen.job.blobs,
                screen.job.cancel.flag(),
                &screen.svg_options,
            )
        };
        let output = match encoded {
            Ok(output) => output,
            Err(reason) => {
                return (
                    if screen.job.cancel.flag().load(Ordering::Relaxed) {
                        ExportResult::Cancelled
                    } else {
                        ExportResult::Failed(reason)
                    },
                    Default::default(),
                )
            }
        };
        if guarded {
            if let (Some(root), Some(folder)) = (&screen.folder_root, screen.job.dest.parent()) {
                if let Err(reason) = crate::export_folders::ensure(root, folder) {
                    return (ExportResult::Failed(reason), output.report);
                }
            }
        }
        if screen.collision_names {
            if let Some(folder) = screen.job.dest.parent() {
                if let Err(e) = disk.export_folder(folder) {
                    return (ExportResult::Failed(e), output.report);
                }
            }
            let parent = screen.job.dest.parent().unwrap_or(Path::new(".")).to_path_buf();
            let stem = screen.job.dest.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let mut n = 1;
            while disk.exists(&screen.job.dest) {
                n += 1;
                screen.job.dest = parent.join(varos_raster::export::file_name(&stem, "", screen.options.format, n));
                if n > 10000 {
                    return (ExportResult::Failed("Too many filename collisions.".into()), output.report);
                }
            }
        }
        let result = if guarded {
            match disk.export_guarded(&screen.job.dest, &output.bytes) {
                Ok(SaveOutcome::Durable) => ExportResult::Exported,
                Ok(SaveOutcome::ReplacedUnconfirmed(reason)) => ExportResult::ExportedUnconfirmed(reason),
                Err(e) => ExportResult::Failed(e.reason),
            }
        } else {
            match disk.export_fresh(&screen.job.dest, &output.bytes, screen.job.cancel.flag()) {
                Ok(crate::lifecycle::ExportWrite::Written) => ExportResult::Exported,
                Ok(crate::lifecycle::ExportWrite::Unconfirmed(reason)) => ExportResult::ExportedUnconfirmed(reason),
                Ok(crate::lifecycle::ExportWrite::Cancelled) => ExportResult::Cancelled,
                Err(e) => ExportResult::Failed(e),
            }
        };
        (result, output.report)
    })();
    FileDone::Exported(ExportDone { job: screen.job, result, report })
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
        ws.get_mut(id).unwrap().saving = Some(SaveInFlight {
            blobs: Default::default(),
            ticket: 1,
            dest: "/w/a.vrs".into(),
            doc,
            follow_up: false,
            started: t0,
        });
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
                inner: FileJob::Save(SaveJob {
                    blobs: Default::default(),
                    sid: SessionId(1),
                    ticket: 9,
                    dest,
                    doc: doc.clone(),
                }),
                home: root.clone(),
                expected,
            }))
        };
        // No backing file exists yet; fresh Save As is independent of the live editor.
        let fresh = FileJob::Bridge(Box::new(BridgeFileJob {
            ticket: 8,
            inner: FileJob::Save(SaveJob {
                blobs: Default::default(),
                sid: SessionId(1),
                ticket: 8,
                dest: path.clone(),
                doc: doc.clone(),
            }),
            home: root.clone(),
            expected: None,
        }));
        let done = execute(fresh, &mut crate::file_ports::DiskStore);
        assert!(matches!(done, FileDone::Bridge { result: varos_bridge::Reply { ok: true, .. }, .. }));
        let FileDone::Bridge { done: Some(done), .. } = done else { panic!("missing save completion") };
        let FileDone::Saved(saved) = *done else { panic!("missing saved snapshot") };
        assert_eq!(saved.published, crate::file_ports::DiskStore.fingerprint(&path));
        assert!(saved.published.is_some());
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
                blobs: Default::default(),
                pdf_options: Default::default(),
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

#[cfg(test)]
mod screen_durability_tests {
    use super::*;
    struct WarningStore {
        unconfirmed_at: usize,
        writes: usize,
        cancel_after_first: Option<CancelFlag>,
    }
    impl DocStore for WarningStore {
        fn load(&mut self, _: &Path) -> Result<Document, String> {
            Err("unused".into())
        }
        fn save(&mut self, _: &Document, _: &Path) -> Result<SaveOutcome, String> {
            Err("unused".into())
        }
        fn key(&self, path: &Path) -> crate::workspace::FileKey {
            crate::file_ports::file_key(path)
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
        fn write_export(
            &mut self,
            _: &Path,
            _: &[u8],
            _: &AtomicBool,
        ) -> Result<crate::lifecycle::ExportWrite, String> {
            self.writes += 1;
            if self.writes == 1 {
                if let Some(cancel) = &self.cancel_after_first {
                    cancel.cancel();
                }
            }
            Ok(if self.writes == self.unconfirmed_at {
                crate::lifecycle::ExportWrite::Unconfirmed("directory sync failed".into())
            } else {
                crate::lifecycle::ExportWrite::Written
            })
        }
    }
    #[test]
    fn cancelled_batch_reports_published_count_and_prior_durability_warning() {
        let doc = varos_pdf::load_vrs(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-cli/tests/fixtures/v3_nested_group.vrs"),
        )
        .unwrap();
        let request = varos_bridge::mcp::decode_tool("export_svg", serde_json::json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":0,"scope":"all_visible_artboards"})).unwrap();
        let varos_bridge::Request::ExportSvg(file) = request else { panic!("wrong request") };
        let screen = crate::export_ui::bridge_job(
            SessionId(1),
            7,
            &doc,
            &Default::default(),
            "/tmp/first.svg".into(),
            &file,
            "export_svg",
        )
        .unwrap();
        let mut store =
            WarningStore { unconfirmed_at: 1, writes: 0, cancel_after_first: Some(screen.job.cancel.clone()) };
        let FileDone::Exported(done) = execute_screen(screen, &mut store, true) else { panic!("wrong completion") };
        assert_eq!(store.writes, 1);
        let ExportResult::Failed(reason) = done.result else { panic!("expected partial failure") };
        assert!(reason.contains("after 1 files"));
        assert!(reason.contains("Cancelled"));
        assert!(reason.contains("directory sync failed"));
        assert!(done.report.notes.iter().any(|note| note.kind == "durability"));
    }

    #[test]
    fn bridge_multi_board_export_receipt_aggregates_warning_on_any_page() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-cli/tests/fixtures/v3_nested_group.vrs");
        let doc = varos_pdf::load_vrs(&fixture).unwrap();
        let home = std::env::temp_dir().canonicalize().unwrap();
        for unconfirmed_at in [0, 1, 2] {
            let request = varos_bridge::mcp::decode_tool("export_svg", serde_json::json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":0,"scope":"all_visible_artboards"})).unwrap();
            let varos_bridge::Request::ExportSvg(file) = request else { panic!("wrong request") };
            let screen = crate::export_ui::bridge_job(
                SessionId(1),
                7,
                &doc,
                &Default::default(),
                home.join("durability-first.svg"),
                &file,
                "export_svg",
            )
            .unwrap();
            let done = execute(
                FileJob::Bridge(Box::new(BridgeFileJob {
                    ticket: 7,
                    inner: FileJob::Screen(Box::new(screen)),
                    home: home.clone(),
                    expected: None,
                })),
                &mut WarningStore { unconfirmed_at, writes: 0, cancel_after_first: None },
            );
            let FileDone::Bridge { result, .. } = done else { panic!("wrong completion") };
            assert!(result.ok, "{result:?}");
            let receipt = result.result.unwrap();
            assert_eq!(receipt["exported"], true);
            assert_eq!(receipt["durable"], unconfirmed_at == 0);
            if unconfirmed_at > 0 {
                assert!(receipt["reason"].as_str().unwrap().contains("directory sync failed"));
                assert!(receipt["report"]["notes"].as_array().unwrap().iter().any(|n| n["kind"] == "durability"));
            }
        }
    }
}

#[cfg(all(test, unix))]
mod lane_c_folder_tests {
    use super::*;
    #[test]
    fn bridge_advanced_batch_creates_format_folder_and_refuses_repeat() {
        let dir = std::env::temp_dir().join(format!("lane-c-guarded-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let doc = varos_core::new_document::Settings { count: 2, ..Default::default() }.document().unwrap();
        let request: varos_bridge::dto::FileEffect = serde_json::from_value(serde_json::json!({
            "api":"1.2", "board":"b1", "request_id":"r1", "expected_rev":0,
            "path":dir.join("export.svg"), "scope":"all_visible_artboards",
            "options":{"screens":{"subfolders":"format","rows":[{"format":"svg"}]}}
        }))
        .unwrap();
        let job = crate::export_ui::bridge_job(
            SessionId(1),
            1,
            &doc,
            &Default::default(),
            dir.join("export.svg"),
            &request,
            "export_raster",
        )
        .unwrap();
        let result = execute(
            FileJob::Bridge(Box::new(BridgeFileJob {
                ticket: 1,
                inner: FileJob::Screen(Box::new(job.clone())),
                home: dir.clone(),
                expected: None,
            })),
            &mut crate::file_ports::DiskStore,
        );
        assert!(matches!(result, FileDone::Bridge { ref result, .. } if result.ok), "{result:?}");
        for n in 1..=2 {
            assert!(dir.join(format!("svg/Artboard {n}.svg")).is_file());
        }
        let result = execute(
            FileJob::Bridge(Box::new(BridgeFileJob {
                ticket: 2,
                inner: FileJob::Screen(Box::new(job)),
                home: dir.clone(),
                expected: None,
            })),
            &mut crate::file_ports::DiskStore,
        );
        assert!(matches!(result, FileDone::Bridge { ref result, .. } if !result.ok));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
