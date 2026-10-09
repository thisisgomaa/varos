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
    let job = Job::new(path.clone(), Target::Place { sid, rev });
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
    let job = Job::new(path.clone(), Target::Place { sid, rev });
    complete(execute(job), &mut ws, &mut dialog);
    let ed = &mut ws.get_mut(sid).unwrap().editor;
    assert_eq!(ed.doc.paths.len(), 1);
    ed.execute_ui(varos_core::EditCommand::Undo);
    assert_eq!(ed.doc, before);
    complete(execute(Job::new(path.clone(), Target::Open)), &mut ws, &mut dialog);
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
    let job = Job::new(path.clone(), Target::Open);
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
    let job = Job::new(path.clone(), Target::Place { sid, rev });
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

#[test]
fn clipboard_jobs_capture_without_conversion_and_guard_publication() {
    use varos_import::clipboard::{Pasteboard, Snapshot};
    struct Board(Snapshot);
    impl Pasteboard for Board {
        fn snapshot(&mut self) -> Result<Snapshot, String> {
            Ok(self.0.clone())
        }
        fn generation(&self) -> i64 {
            self.0.generation
        }
    }
    let mut ws = Workspace::new();
    let s = ws.active().unwrap();
    let sid = s.id;
    let rev = s.editor.rev;
    let before = s.editor.doc.clone();
    let mut board =
        Board(Snapshot { generation: 4, flavours: vec![("public.svg-image".into(), b"malformed svg".to_vec())] });
    let captured = crate::clipboard_in::capture(&s.editor, &mut board).unwrap().unwrap();
    let mut job = Job::new("Clipboard".into(), Target::Place { sid, rev });
    job.clipboard = Some(Box::new(captured));
    assert!(execute(job.clone()).result.is_err(), "conversion belongs to worker");
    let svg = source();
    job.clipboard.as_mut().unwrap().flavours[0].1 = std::fs::read(&svg).unwrap();
    std::fs::remove_file(svg).unwrap();
    let mut dialog = Dialog::default();
    let done = execute(job.clone());
    assert!(!crate::import_jobs::complete_with_generation(done, &mut ws, &mut dialog, 5));
    assert_eq!(ws.get(sid).unwrap().editor.doc, before);
    let done = execute(job.clone());
    job.cancel.cancel();
    assert!(!crate::import_jobs::complete_with_generation(done, &mut ws, &mut dialog, 4));
    assert!(execute(job.clone()).result.unwrap_err().contains("cancelled"));
    job.cancel = Default::default();
    let done = execute(job.clone());
    ws.get_mut(sid).unwrap().editor.begin();
    assert!(!crate::import_jobs::complete_with_generation(done, &mut ws, &mut dialog, 4));
    assert_eq!(ws.get(sid).unwrap().editor.doc, before);
    ws.get_mut(sid).unwrap().editor.commit();
    job.centre = Some([100., 100.]);
    assert!(crate::import_jobs::complete_with_generation(execute(job), &mut ws, &mut dialog, 4));
    assert_eq!(ws.get(sid).unwrap().editor.doc.paths[0].anchors[0].p, [90., 95.]);
    ws.get_mut(sid).unwrap().editor.execute_ui(varos_core::EditCommand::Undo);
    assert_eq!(ws.get(sid).unwrap().editor.doc, before);
}

#[test]
fn explicit_job_page_and_unit_choices_reach_conversion() {
    let svg = source();
    let path = svg.with_extension("dxf");
    std::fs::remove_file(svg).unwrap();
    std::fs::write(&path, "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n2\n21\n0\n0\nENDSEC\n0\nEOF\n").unwrap();
    let mut job = Job::new(path.clone(), Target::Open);
    assert!(execute(job.clone()).result.unwrap_err().contains("unit"));
    job.options.points_per_unit = Some(3.);
    let (doc, _) = execute(job).result.unwrap();
    assert_eq!(doc.paths[0].anchors[1].p[0], 6.);
    std::fs::remove_file(path).unwrap();
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 100 100] >>",
        "<< /Type /Page /Parent 2 0 R /Contents 5 0 R >>",
        "<< /Type /Page /Parent 2 0 R /Contents 5 0 R >>",
        "<< /Length 23 >>\nstream\n0 0 m 10 0 l 0 10 l h f\nendstream",
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf += &format!("{} 0 obj\n{object}\nendobj\n", i + 1);
    }
    let xref = pdf.len();
    pdf += "xref\n0 6\n0000000000 65535 f \n";
    for offset in offsets.iter().skip(1) {
        pdf += &format!("{offset:010} 00000 n \n");
    }
    pdf += &format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n");
    let svg = source();
    let path = svg.with_extension("pdf");
    std::fs::remove_file(svg).unwrap();
    std::fs::write(&path, pdf).unwrap();
    let mut job = Job::new(path.clone(), Target::Open);
    let convert = |job| {
        crate::import_jobs::execute_using(job, |bytes, format, options, _| {
            varos_import::import_file(bytes, format, options)
        })
    };
    assert!(convert(job.clone()).result.unwrap_err().contains("page"));
    job.options.page = Some(2);
    assert!(convert(job.clone()).result.is_ok());
    job.options.page = Some(3);
    assert!(convert(job).result.is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn paste_keys_are_deferred_even_when_document_actions_can_run_immediately() {
    struct Ui;
    impl crate::host::DocUi for Ui {
        fn settle(&mut self, _: &mut varos_core::Editor) -> bool {
            panic!("paste must reach host queue before touching fields");
        }
        fn document_switched(&mut self) {}
    }
    for shift in [false, true] {
        let mut editor = varos_core::Editor::new();
        let before = editor.doc.clone();
        let mut view = varos_core::geom::View::identity();
        let mut pending = crate::host::ActionQueue::default();
        crate::raise_doc(
            &mut pending,
            crate::host::DocAction::Key(winit::keyboard::KeyCode::KeyV, crate::Mods { ctrl: true, shift, alt: false }),
            &mut editor,
            &mut view,
            egui::Rect::NOTHING,
            &mut Ui,
        );
        assert!(!pending.is_empty());
        assert_eq!(editor.doc, before);
        assert_eq!(editor.rev, 0);
    }
}
