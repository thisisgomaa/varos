//! Varos: a Save its location refuses (a read-only volume) goes on as Save As in the documents
//! folder ([`super::read_only_save`]); Save As to such a path, other failures and agents' saves
//! don't.
#![cfg(unix)]

use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::fileio;

use crate::{FilePick, Services, VectorcraftApp, dialogs};

/// What the save panel answers (none: cancelled), and what it was asked.
#[derive(Clone, Default)]
struct Panel {
    answer: Rc<RefCell<Option<String>>>,
    asked: Rc<RefCell<Vec<FilePick>>>,
}

/// An app that writes files as the desktop does (atomically, the failure's kind kept), with
/// Background Save `background`.
fn app(panel: &Panel, background: bool) -> VectorcraftApp {
    let write = |p: &str, b: &[u8]| vectorcraft_format::write_atomic(Path::new(p), b).map_err(|e| e.to_string());
    let (answer, asked) = (panel.answer.clone(), panel.asked.clone());
    let services = Services {
        write: Some(Box::new(write)),
        write_shared: Some(Arc::new(write)),
        exists: Some(Box::new(|p: &str| Path::new(p).exists())),
        pick_save: Some(Box::new(move |pick: &FilePick| {
            asked.borrow_mut().push(pick.clone());
            answer.borrow().clone()
        })),
        ..Default::default()
    };
    let mut app = VectorcraftApp::new(Session::new(), services);
    app.session.prefs.background_save = background;
    app
}

/// A folder (and the file in it) as a read-only volume has them: nothing can be written there.
/// Writable again when dropped.
struct Locked {
    folder: PathBuf,
    file: PathBuf,
}

impl Locked {
    /// `None` when permissions don't hold (run as root).
    fn new(file: &Path) -> Option<Self> {
        let folder = file.parent().unwrap().to_path_buf();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o444)).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
        let locked = Locked { folder, file: file.to_path_buf() };
        let probe = locked.folder.join("probe");
        if std::fs::write(&probe, b"").is_ok() {
            let _ = std::fs::remove_file(probe);
            return None;
        }
        Some(locked)
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.folder, std::fs::Permissions::from_mode(0o755));
        let _ = std::fs::set_permissions(&self.file, std::fs::Permissions::from_mode(0o644));
    }
}

const NAME: &str = "die-cut for men 001.varos";

/// A modified document whose file is `NAME` in `root/drive`, which is then locked.
fn opened_on_a_read_only_drive(app: &mut VectorcraftApp, root: &Path) -> Option<(Locked, String)> {
    let drive = root.join("drive");
    std::fs::create_dir_all(&drive).unwrap();
    let path = drive.join(NAME).to_string_lossy().into_owned();
    let background = std::mem::replace(&mut app.session.prefs.background_save, false);
    app.run("file.new", json!({"width": 100, "height": 80})).unwrap();
    app.run("file.saveAs", json!({ "path": path })).unwrap();
    app.session.prefs.background_save = background;
    app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 30, "height": 20})).unwrap();
    let locked = Locked::new(Path::new(&path))?;
    Some((locked, path))
}

fn state(app: &VectorcraftApp) -> (bool, Option<String>) {
    let st = app.session.doc().unwrap();
    (st.is_dirty(), st.path.clone())
}

/// Owner report 2026-10-27: ⌘S on a document from a read-only drive opens the save panel in the
/// documents folder with the same name (no dead-end dialog); cancelled, the document stays
/// modified with its file; answered, it is saved there.
fn save_goes_on_in_the_save_panel(background: bool) {
    let root = vectorcraft_testkit::temp_dir(&format!("ro-save-panel-{background}"));
    let panel = Panel::default();
    let mut app = app(&panel, background);
    let Some((locked, path)) = opened_on_a_read_only_drive(&mut app, &root) else { return };

    let r = app.run("file.save", json!({}));
    if background {
        assert_eq!(r.unwrap()["background"], json!(true));
        crate::background::wait_all(&mut app);
    } else {
        assert_eq!(r.unwrap_err(), "cancelled");
        assert_eq!(app.ui.status, format!("“{NAME}” is on a read-only location: choose where to save it"));
    }
    let asked = panel.asked.borrow().clone();
    assert_eq!(asked.len(), 1, "the save panel opened once");
    assert_eq!((asked[0].name.as_str(), &asked[0].folder), (NAME, &fileio::documents_dialog_folder()));
    assert!(asked[0].folder.as_deref() != Some(&*locked.folder.to_string_lossy()));
    assert!(app.ui.dialog.is_none(), "no dead-end dialog: {:?}", app.ui.dialog.as_ref().map(|d| d.str("message")));
    assert_eq!(state(&app), (true, Some(path.clone())), "cancelled: modified, with its file");

    // Answered: written there, and the document's file now.
    let elsewhere = root.join("documents");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let chosen = elsewhere.join(NAME).to_string_lossy().into_owned();
    *panel.answer.borrow_mut() = Some(chosen.clone());
    let r = app.run("file.save", json!({})).unwrap();
    if background {
        crate::background::wait_all(&mut app);
    } else {
        assert_eq!(r["path"], json!(chosen));
    }
    assert_eq!(panel.asked.borrow().len(), 2);
    assert_eq!(state(&app), (false, Some(chosen.clone())));
    assert!(std::fs::metadata(&chosen).is_ok_and(|m| m.len() > 0));
    assert!(app.ui.dialog.is_none());

    // Save As to the read-only path the user picked: told in the dialog, no panel.
    let r = app.run("file.saveAs", json!({ "path": path }));
    if background {
        crate::background::wait_all(&mut app);
    } else {
        assert!(r.is_err());
    }
    let d = app.ui.dialog.take().unwrap_or_else(|| panic!("a dialog says so ({r:?})"));
    assert_eq!((d.kind.as_str(), d.str("message").as_str()), (dialogs::confirm::MESSAGE, format!("Can't save “{NAME}”.").as_str()));
    assert_eq!(panel.asked.borrow().len(), 2);
    drop(locked);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_save_to_a_read_only_location_opens_the_save_panel() {
    save_goes_on_in_the_save_panel(false);
}

#[test]
fn a_background_save_to_a_read_only_location_opens_the_save_panel() {
    save_goes_on_in_the_save_panel(true);
}

/// An agent's save never opens a panel: the error comes back with its code. A folder that isn't
/// there is not a read-only location: the dialog as before.
#[test]
fn agents_get_a_typed_error_and_other_failures_keep_the_dialog() {
    let root = vectorcraft_testkit::temp_dir("ro-save-agent");
    let panel = Panel::default();
    let mut app = app(&panel, false);
    let Some((locked, path)) = opened_on_a_read_only_drive(&mut app, &root) else { return };
    app.ui.answering_control = true;
    let e = app.run("file.save", json!({})).unwrap_err();
    app.ui.answering_control = false;
    // The writer's own reason after the code (`Permission denied (os error 13)` here).
    assert!(e.starts_with("read_only_location: ") && e.contains("os error"), "{e}");
    assert_eq!(varos_agents::boards::code_of(&e), Some(varos_agents::boards::READ_ONLY_LOCATION));
    assert!(app.ui.dialog.is_none() && panel.asked.borrow().is_empty());
    assert_eq!(state(&app), (true, Some(path)));
    drop(locked);

    let gone = root.join("gone").join(NAME).to_string_lossy().into_owned();
    app.session.doc_mut().unwrap().path = Some(gone.clone());
    let e = app.run("file.save", json!({})).unwrap_err();
    let d = app.ui.dialog.take().unwrap_or_else(|| panic!("a dialog says so ({e})"));
    assert_eq!((d.kind.as_str(), d.str("message").as_str()), (dialogs::confirm::MESSAGE, format!("Can't save “{NAME}”.").as_str()));
    // The system's reason, as before (`No such file or directory (os error 2)`).
    assert!(d.str("detail").contains("os error 2"), "{}", d.str("detail"));
    assert!(panel.asked.borrow().is_empty());
    assert_eq!(state(&app), (true, Some(gone)));
    let _ = std::fs::remove_dir_all(&root);
}

/// Save Changes › Save on a document from a read-only drive: the save panel asks; saved there, the
/// document closes, cancelled it stays open and modified.
#[test]
fn save_changes_on_a_read_only_location_asks_where_before_closing() {
    let root = vectorcraft_testkit::temp_dir("ro-save-close");
    let panel = Panel::default();
    let mut app = app(&panel, true);
    let Some((locked, path)) = opened_on_a_read_only_drive(&mut app, &root) else { return };
    let ask = |app: &mut VectorcraftApp| {
        let (i, uid) = (app.session.active_index().unwrap(), app.session.doc().unwrap().uid);
        app.ui.dialog = Some(crate::state::Dialog::new(crate::unsaved::KIND, json!({"index": i, "uid": uid, "name": "die-cut", "then": "close"})));
        crate::unsaved::confirm(app)
    };
    assert!(ask(&mut app).is_err());
    assert_eq!((app.session.documents().len(), state(&app)), (1, (true, Some(path))));
    let chosen = root.join(NAME).to_string_lossy().into_owned();
    *panel.answer.borrow_mut() = Some(chosen.clone());
    ask(&mut app).unwrap();
    assert!(app.session.documents().is_empty(), "saved, so closed");
    assert!(Path::new(&chosen).exists());
    assert_eq!(panel.asked.borrow().len(), 2);
    drop(locked);
    let _ = std::fs::remove_dir_all(&root);
}
