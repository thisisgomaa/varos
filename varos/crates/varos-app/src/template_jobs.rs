//! Phase 1 template I/O: owned revision snapshot, existing file-worker queue, cancellable publication.
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use varos_core::model::Document;
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub ticket: u64,
    pub path: PathBuf,
    pub document: Option<Arc<Document>>,
    pub cancel: crate::file_jobs::CancelFlag,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    pub ticket: u64,
    pub result: Result<Option<Box<Document>>, varos_bridge::Error>,
}
fn cancelled(flag: &AtomicBool) -> Result<(), varos_bridge::Error> {
    if flag.load(Ordering::Acquire) {
        Err(varos_bridge::Error::new("cancelled", "template job cancelled"))
    } else {
        Ok(())
    }
}
pub fn execute(job: Job) -> Done {
    let result = (|| {
        cancelled(job.cancel.flag())?;
        if let Some(doc) = job.document {
            varos_bridge::templates::save_new_cancellable(&doc, &job.path, job.cancel.flag())?;
            Ok(None)
        } else {
            let doc =
                varos_pdf::load_vrs(&job.path).map_err(|e| varos_bridge::Error::new("io_error", e.to_string()))?;
            cancelled(job.cancel.flag())?;
            Ok(Some(Box::new(doc)))
        }
    })();
    Done { ticket: job.ticket, result }
}
pub fn complete(done: Done, ws: &mut crate::workspace::Workspace) {
    let reply = match done.result {
        Err(e) => varos_bridge::Reply::failure(e),
        Ok(None) => varos_bridge::Reply::success(serde_json::json!({"written":true})),
        Ok(Some(doc)) => {
            let id = ws.add_template(*doc);
            varos_bridge::Reply::success(
                serde_json::json!({"board":format!("b{}",id.0),"name":"Untitled","dirty":true,"backing_file":null}),
            )
        }
    };
    crate::bridge_host::file_completed(done.ticket, reply);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_pins_snapshot_cancels_and_opens_dirty_untitled() {
        let folder = std::env::temp_dir().join(format!("template-job-{}", crate::file_jobs::next_ticket()));
        let path = folder.join("test.vrs");
        let mut ed = varos_core::Editor::new();
        ed.execute(varos_core::EditCommand::SetPpi(300.0)).unwrap();
        let job =
            Job { ticket: 1, path: path.clone(), document: Some(Arc::new(ed.doc.clone())), cancel: Default::default() };
        ed.execute(varos_core::EditCommand::SetPpi(72.0)).unwrap();
        assert_eq!(execute(job.clone()).result, Ok(None));
        assert_eq!(varos_pdf::load_vrs(&path).unwrap().units.ppi, 300.0);
        assert_eq!(execute(job.clone()).result.unwrap_err().code, "save_conflict");
        let mut cancelled_job = job.clone();
        cancelled_job.path = folder.join("cancel.vrs");
        cancelled_job.cancel.cancel();
        assert_eq!(execute(cancelled_job.clone()).result.unwrap_err().code, "cancelled");
        assert!(!cancelled_job.path.exists());
        let load = Job { document: None, cancel: Default::default(), ..job };
        let done = execute(load.clone());
        assert!(!crate::file_jobs::FileDone::Template(done.clone()).is_quiet());
        assert!(crate::file_jobs::FileDone::Template(Done { ticket: 2, result: Ok(None) }).is_quiet());
        let mut ws = crate::workspace::Workspace::new();
        complete(done, &mut ws);
        let s = ws.sessions().last().unwrap();
        assert!(s.path.is_none());
        assert!(s.is_dirty());
        assert_eq!(s.editor.doc.units.ppi, 300.0);
        load.cancel.cancel();
        assert_eq!(execute(load).result.unwrap_err().code, "cancelled");
        std::fs::remove_dir_all(folder).unwrap();
    }
}
