//! Lane H worker/host publication tests: no UI, no GPU, no EventLoop.
use crate::{
    import_jobs::{complete, execute, Job, Target},
    lifecycle::{Dialogs, SaveDecision, SaveFailChoice},
    workspace::Workspace,
};
use std::path::{Path, PathBuf};
#[derive(Default)]
struct Dialog {
    accept: bool,
    errors: Vec<String>,
}
impl Dialogs for Dialog {
    fn accept_import_losses(&mut self, _: &[String]) -> bool {
        self.accept
    }
    fn pick_open(&mut self) -> Vec<PathBuf> {
        vec![]
    }
    fn pick_save(&mut self, _: &str, _: Option<&Path>) -> Option<PathBuf> {
        None
    }
    fn ask_save_changes(&mut self, _: &str, _: Option<(usize, usize)>) -> SaveDecision {
        SaveDecision::Cancel
    }
    fn save_failed(&mut self, _: &str, _: &str) -> SaveFailChoice {
        SaveFailChoice::Cancel
    }
    fn open_failed(&mut self, _: &str, e: &str) {
        self.errors.push(e.into());
    }
    fn confirm_replace(&mut self, _: &str) -> bool {
        false
    }
    fn notice(&mut self, _: &str, _: &str) {}
}
fn source() -> PathBuf {
    let path = std::env::temp_dir().join(format!("lane-h-worker-{}.svg", crate::file_jobs::next_ticket()));
    std::fs::write(&path,br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0L20 0L0 10Z" fill="red"/></svg>"#).unwrap();
    path
}
#[test]
fn worker_cancel_stale_revision_place_and_dirty_pathless_open() {
    let path = source();
    let mut ws = Workspace::new();
    ws.new_untitled();
    let sid = ws.active_id().unwrap();
    let rev = ws.get(sid).unwrap().editor.rev;
    let before = ws.get(sid).unwrap().editor.doc.clone();
    let mut dialog = Dialog::default();
    let job = Job { path: path.clone(), target: Target::Place { sid, rev }, cancel: Default::default() };
    let mut stale = job.clone();
    stale.target = Target::Place { sid, rev: rev + 1 };
    complete(execute(stale), &mut ws, &mut dialog);
    assert_eq!(ws.get(sid).unwrap().editor.doc, before);
    assert!(dialog.errors[0].contains("changed"));
    let done = execute(job.clone());
    job.cancel.cancel();
    complete(done, &mut ws, &mut dialog);
    assert_eq!(ws.get(sid).unwrap().editor.doc, before);
    let cancelled = execute(job);
    assert!(cancelled.result.unwrap_err().contains("cancelled"));
    let job = Job { path: path.clone(), target: Target::Place { sid, rev }, cancel: Default::default() };
    complete(execute(job), &mut ws, &mut dialog);
    let ed = &mut ws.get_mut(sid).unwrap().editor;
    assert_eq!(ed.doc.paths.len(), 1);
    ed.execute_ui(varos_core::EditCommand::Undo);
    assert_eq!(ed.doc, before);
    complete(
        execute(Job { path: path.clone(), target: Target::Open, cancel: Default::default() }),
        &mut ws,
        &mut dialog,
    );
    let imported = ws.active().unwrap();
    assert!(imported.path.is_none());
    assert!(imported.is_dirty());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn losses_are_reviewed_before_publication_and_native_routing_has_no_fallback() {
    let path = source();
    std::fs::write(&path,br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><text>hello</text><path d="M0 0L20 0L0 10Z"/></svg>"#).unwrap();
    let mut ws = Workspace::new();
    ws.new_untitled();
    let count = ws.sessions().len();
    let mut dialog = Dialog::default();
    let job = Job { path: path.clone(), target: Target::Open, cancel: Default::default() };
    let done = execute(job.clone());
    assert!(!done.result.as_ref().unwrap().1.loss_notes.is_empty());
    complete(done, &mut ws, &mut dialog);
    assert_eq!(ws.sessions().len(), count);
    dialog.accept = true;
    complete(execute(job), &mut ws, &mut dialog);
    assert_eq!(ws.sessions().len(), count + 1);
    assert!(!crate::foreign_import::is_foreign(Path::new("renamed.vrs")));
    let pdf = path.with_extension("pdf");
    std::fs::write(&pdf, b"%PDF-1.4 /VAROS_Model broken").unwrap();
    assert!(!crate::foreign_import::is_foreign(&pdf));
    std::fs::write(&pdf, b"%PDF-1.4 model.varos.json broken").unwrap();
    assert!(!crate::foreign_import::is_foreign(&pdf));
    std::fs::remove_file(pdf).unwrap();
    std::fs::remove_file(path).unwrap();
}
#[test]
fn host_completion_refusal_never_settles_or_resets_a_human_transaction() {
    struct Ui {
        resets: usize,
    }
    impl crate::host::DocUi for Ui {
        fn settle(&mut self, _: &mut varos_core::Editor) -> bool {
            panic!("must not settle an import result")
        }
        fn document_switched(&mut self) {
            self.resets += 1;
        }
    }
    let path = source();
    let mut ws = Workspace::new();
    ws.new_untitled();
    let sid = ws.active_id().unwrap();
    let rev = ws.get(sid).unwrap().editor.rev;
    let mut ui = Ui { resets: 0 };
    let mut dialog = Dialog::default();
    let keys = crate::host::Keyboard::default();
    let job = Job { path: path.clone(), target: Target::Place { sid, rev }, cancel: Default::default() };
    let done = execute(job.clone());
    ws.get_mut(sid).unwrap().editor.begin();
    let before = ws.get(sid).unwrap().editor.doc.clone();
    let ran = crate::import_jobs::complete_on_host(done, &mut ws, &mut ui, &mut dialog, &keys);
    assert!(!ran.ran);
    assert!(dialog.errors.last().unwrap().contains("busy"));
    assert!(ws.get(sid).unwrap().editor.transaction_open());
    assert_eq!(ws.get(sid).unwrap().editor.doc, before);
    assert_eq!(ui.resets, 0);
    let done = execute(job.clone());
    job.cancel.cancel();
    assert!(!crate::import_jobs::complete_on_host(done, &mut ws, &mut ui, &mut dialog, &keys).ran);
    assert!(ws.get(sid).unwrap().editor.transaction_open());
    assert_eq!(ui.resets, 0);
    std::fs::remove_file(path).unwrap();
}
