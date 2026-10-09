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
}
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    pub job: Job,
    pub result: Result<(Box<varos_core::model::Document>, varos_import::ImportReport), String>,
}
pub fn execute(job: Job) -> Done {
    let result = (|| {
        if job.cancel.flag().load(Ordering::Acquire) {
            return Err("Import cancelled".into());
        }
        let bytes = crate::foreign_import::bytes(&job.path)?;
        let ext = job.path.extension().and_then(|e| e.to_str()).ok_or("Import source extension required")?;
        let options =
            varos_import::ImportOptions { loss_policy: varos_import::LossPolicy::AllowReported, ..Default::default() };
        let (doc, report) = varos_import::worker::isolated_import(
            &bytes,
            varos_import::Format::from_extension(ext)?,
            options,
            job.cancel.flag(),
        )?;
        Ok((Box::new(doc), report))
    })();
    Done { job, result }
}
pub fn complete(done: Done, ws: &mut Workspace, dialogs: &mut dyn Dialogs) -> bool {
    if done.job.cancel.flag().load(Ordering::Acquire) {
        return false;
    }
    let name = done.job.path.file_name().unwrap_or_default().to_string_lossy();
    let (doc, report) = match done.result {
        Ok(v) => v,
        Err(e) => {
            dialogs.open_failed(&name, &e);
            return false;
        }
    };
    if !report.loss_notes.is_empty() && !dialogs.accept_import_losses(&report.loss_notes) {
        return false;
    }
    if done.job.cancel.flag().load(Ordering::Acquire) {
        return false;
    }
    match done.job.target {
        Target::Open => {
            ws.new_imported(*doc);
            true
        }
        Target::Place { sid, rev } => {
            if let Some(s) = ws.get_mut(sid) {
                if s.editor.rev != rev {
                    dialogs.open_failed(&name, "Document changed during import; place again");
                    return false;
                }
                if let Err(e) = varos_core::placement::check(&s.editor, &doc) {
                    dialogs.open_failed(&name, &e);
                    return false;
                }
                if let Err(e) = s.editor.try_execute(varos_core::EditCommand::PlaceArtwork(doc)) {
                    dialogs.open_failed(&name, &e.to_string());
                    return false;
                }
                true
            } else {
                false
            }
        }
    }
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
