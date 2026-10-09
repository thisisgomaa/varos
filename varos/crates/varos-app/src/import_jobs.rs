//! Foreign file parsing on the existing IO worker; explicit revision-bound publication.
use crate::{app_command::SessionId, file_jobs::CancelFlag, lifecycle::Dialogs, workspace::Workspace};
use std::{path::PathBuf, sync::atomic::Ordering};
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Open,
    Place { sid: SessionId, rev: u64 },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub path: PathBuf,
    pub target: Target,
    pub cancel: CancelFlag,
    pub options: varos_import::ImportOptions,
    pub clipboard: Option<Box<varos_import::clipboard::Snapshot>>,
    pub centre: Option<varos_core::geom::Pt>,
    pub ticket: Option<u64>,
}
impl Job {
    pub fn new(path: PathBuf, target: Target) -> Self {
        Self {
            path,
            target,
            cancel: Default::default(),
            options: varos_import::ImportOptions {
                loss_policy: varos_import::LossPolicy::AllowReported,
                ..Default::default()
            },
            clipboard: None,
            centre: None,
            ticket: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    pub job: Job,
    pub result: Result<(Box<varos_core::model::Document>, varos_import::ImportReport), String>,
}
pub fn execute(job: Job) -> Done {
    execute_using(job, varos_import::worker::isolated_import)
}
pub fn execute_using(
    job: Job,
    convert: impl FnOnce(
        &[u8],
        varos_import::Format,
        varos_import::ImportOptions,
        &std::sync::atomic::AtomicBool,
    ) -> Result<(varos_core::model::Document, varos_import::ImportReport), String>,
) -> Done {
    let result = (|| {
        if job.cancel.flag().load(Ordering::Acquire) {
            return Err("Import cancelled".into());
        }
        if let Some(snapshot) = &job.clipboard {
            let (doc, report, _) =
                varos_import::clipboard::stage_cancellable(snapshot, job.options, job.cancel.flag())?;
            return Ok((Box::new(doc), report));
        }
        let bytes = if job.ticket.is_some() {
            let ext = job.path.extension().and_then(|e| e.to_str()).ok_or("Import source extension required")?;
            varos_bridge::files::read_source(&job.path, ext, varos_import::MAX_BYTES).map_err(|e| e.reason)?
        } else {
            crate::foreign_import::bytes(&job.path)?
        };
        let ext = job.path.extension().and_then(|e| e.to_str()).ok_or("Import source extension required")?;

        let (doc, report) =
            convert(&bytes, varos_import::Format::from_extension(ext)?, job.options, job.cancel.flag())?;
        Ok((Box::new(doc), report))
    })();
    Done { job, result }
}
pub fn complete(done: Done, ws: &mut Workspace, dialogs: &mut dyn Dialogs) -> bool {
    use varos_import::clipboard::Pasteboard;
    complete_using_generation(done, ws, dialogs, &|| crate::clipboard_in::SystemPasteboard.generation())
}
#[cfg(test)]
pub fn complete_with_generation(done: Done, ws: &mut Workspace, dialogs: &mut dyn Dialogs, generation: i64) -> bool {
    complete_using_generation(done, ws, dialogs, &|| generation)
}
fn complete_using_generation(
    done: Done,
    ws: &mut Workspace,
    dialogs: &mut dyn Dialogs,
    generation: &dyn Fn() -> i64,
) -> bool {
    let result = (|| {
        if done.job.cancel.flag().load(Ordering::Acquire) {
            return Err("Import cancelled".to_string());
        }
        let (mut doc, report) = done.result?;
        if let Some(snapshot) = &done.job.clipboard {
            if snapshot.generation != generation() {
                return Err("Clipboard changed before publication".into());
            }
        }
        if done.job.ticket.is_none()
            && !report.loss_notes.is_empty()
            && !dialogs.accept_import_losses(&report.loss_notes)
        {
            return Err("Import declined".into());
        }
        if done.job.cancel.flag().load(Ordering::Acquire) {
            return Err("Import cancelled".into());
        }
        if done.job.clipboard.as_ref().is_some_and(|s| s.generation != generation()) {
            return Err("Clipboard changed before publication".into());
        }
        if let Some(target) = done.job.centre {
            crate::clipboard_in::centre(&mut doc, target);
        }
        let rev = match done.job.target {
            Target::Open => {
                ws.new_imported(*doc);
                ws.active().map_or(0, |s| s.editor.rev)
            }
            Target::Place { sid, rev } => {
                if done.job.ticket.is_some() && ws.document_target() != Some(sid) {
                    return Err("Board is no longer active; import again".into());
                }
                let s = ws.get_mut(sid).ok_or("Board closed during import")?;
                if s.editor.rev != rev {
                    return Err("Document changed during import; place again".into());
                }
                if crate::autosave_host::editor_busy(&s.editor) {
                    return Err("Document is busy; finish the active gesture and import again".into());
                }
                varos_core::placement::check(&s.editor, &doc)?;
                s.editor.try_execute(varos_core::EditCommand::PlaceArtwork(doc)).map_err(|e| e.to_string())?;
                s.editor.rev
            }
        };
        let mut reply = varos_bridge::Reply::success(serde_json::json!({"rev":rev,"report":report,"undo_steps":1}));
        reply.rev = Some(rev);
        reply.undo_steps = 1;
        Ok(reply)
    })();
    let published = result.is_ok();
    if let Some(ticket) = done.job.ticket {
        let reply = result.unwrap_or_else(|e| {
            varos_bridge::Reply::failure(varos_bridge::Error::new(
                if e.contains("cancelled") {
                    "cancelled"
                } else if e.contains("Clipboard changed") {
                    "stale_clipboard"
                } else if e.contains("changed during") {
                    "revision_conflict"
                } else if e.contains("busy") {
                    "busy"
                } else {
                    "invalid_argument"
                },
                e,
            ))
        });
        crate::bridge_host::file_completed(ticket, reply);
    } else if let Err(e) = result {
        if e != "Import cancelled" && e != "Import declined" {
            dialogs.open_failed(&done.job.path.file_name().unwrap_or_default().to_string_lossy(), &e);
        }
    }
    published
}

/// Import results never settle a human gesture as a side effect of cancellation or refusal.
pub fn complete_on_host(
    mut done: Done,
    ws: &mut Workspace,
    ui: &mut dyn crate::host::DocUi,
    dialogs: &mut dyn Dialogs,
    keys: &crate::host::Keyboard,
) -> crate::host::Ran {
    if done.result.is_ok()
        && !done.job.cancel.flag().load(Ordering::Acquire)
        && (ui.field_has_focus()
            || ui.bridge_preview_active()
            || ws.sessions().iter().any(|s| crate::autosave_host::editor_busy(&s.editor)))
    {
        done.result = Err("Document is busy; finish the active gesture or field and import again".into());
    }
    let before = ws.active_id();
    let published = complete(done, ws, dialogs);
    let switched = before != ws.active_id();
    if published {
        if let Some(s) = ws.active_mut() {
            keys.mirror(&mut s.editor);
        }
        ui.document_switched();
    }
    crate::host::Ran { ran: published, switched, ..Default::default() }
}
