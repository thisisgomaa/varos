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
pub fn complete(done: Done, ws: &mut Workspace, dialogs: &mut dyn Dialogs) {
    if done.job.cancel.flag().load(Ordering::Acquire) {
        return;
    }
    let name = done.job.path.file_name().unwrap_or_default().to_string_lossy();
    let (doc, report) = match done.result {
        Ok(v) => v,
        Err(e) => {
            dialogs.open_failed(&name, &e);
            return;
        }
    };
    if !report.loss_notes.is_empty() && !dialogs.accept_import_losses(&report.loss_notes) {
        return;
    }
    if done.job.cancel.flag().load(Ordering::Acquire) {
        return;
    }
    match done.job.target {
        Target::Open => {
            ws.new_imported(*doc);
        }
        Target::Place { sid, rev } => {
            if let Some(s) = ws.get_mut(sid) {
                if s.editor.rev != rev {
                    dialogs.open_failed(&name, "Document changed during import; place again");
                    return;
                }
                if let Err(e) = varos_core::placement::check(&s.editor, &doc) {
                    dialogs.open_failed(&name, &e);
                    return;
                }
                s.editor.execute_ui(varos_core::EditCommand::PlaceArtwork(doc));
            }
        }
    }
}
