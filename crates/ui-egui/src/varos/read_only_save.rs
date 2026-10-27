//! Varos: a Save to a location that takes no file goes on as Save As (owner report 2026-10-27: a
//! PDF opened from an NTFS drive, which macOS mounts read-only, met ⌘S with «Can't save … Read-only
//! file system (os error 30)» and only a Close button).
//!
//! When File › Save (the document's own file) fails because its location can't be written, by the
//! error's kind ([`vectorcraft_format::refuses_writes`]: a read-only volume, a folder or file the
//! user may not write; the atomic writer keeps the kind, [`vectorcraft_format::take_write_failure`]),
//! the status bar says so in one line and the save panel opens at once with the same file name,
//! format and options, in the user's documents folder. Cancelling it leaves the document modified,
//! with the file it had, as after any failed save. Every other failure (a full disk, a missing
//! folder…), and Save As, Save a Copy and exports to a path picked in a panel, keep the dialog
//! that names the reason ([`crate::io::report_write_error`]). An agent's request never opens a
//! panel: it gets the error with the code [`READ_ONLY_LOCATION`] instead.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Map, Value, json};
use varos_agents::boards::{READ_ONLY_LOCATION, error};
use vectorcraft_engine::cmd::fileio::{self, SaveMode, SavePlan};

use crate::VectorcraftApp;
use crate::picks::{self, Entry};

/// What a planned save needs to go on elsewhere when its location refuses the file.
pub(crate) struct Retry {
    /// Set by the work ([`Watch::run`]) when the write failed because the location takes no file.
    refused: Arc<AtomicBool>,
    /// Started for an agent's request (`answering_control` when it was asked for).
    by_agent: bool,
    /// For a Save (the document's own file): its format and options, which Save As keeps.
    save: Option<(&'static str, Map<String, Value>)>,
}

/// The work's side of [`Retry`]: sent with it to the thread that writes.
pub(crate) struct Watch(Arc<AtomicBool>);

impl Retry {
    /// For `plan`, about to be written.
    pub(crate) fn of(app: &VectorcraftApp, plan: &SavePlan) -> Self {
        Retry {
            refused: Arc::default(),
            by_agent: app.ui.answering_control,
            save: (plan.mode == SaveMode::Save).then(|| (plan.format.id, plan.options.clone())),
        }
    }

    /// The watch the work runs its writes under.
    pub(crate) fn watch(&self) -> Watch {
        Watch(self.refused.clone())
    }

    /// The result of a save that failed with `e` writing `path` (document `uid`), when it isn't the
    /// usual dialog: `None` unless the location refused the file. An agent gets `e` with the code
    /// [`READ_ONLY_LOCATION`]; a person's Save goes on in the save panel ([`save_elsewhere`]).
    pub(crate) fn instead(self, app: &mut VectorcraftApp, uid: u64, path: &str, e: &str) -> Option<Result<Value, String>> {
        if !self.refused.load(Ordering::Relaxed) {
            return None;
        }
        if self.by_agent || app.ui.answering_control {
            return Some(Err(error(READ_ONLY_LOCATION, e)));
        }
        let (format, options) = self.save.filter(|_| !crate::io::is_web(app))?;
        save_elsewhere(app, uid, path, format, options)
    }
}

impl Watch {
    /// Run the writes of `work` and note whether one failed because its location takes no file.
    pub(crate) fn run(&self, work: impl FnOnce() -> Result<Value, String>) -> Result<Value, String> {
        // What an earlier write on this thread left is not this work's: forgotten first.
        let _ = vectorcraft_format::take_write_failure();
        let r = work();
        if r.is_err() && vectorcraft_format::take_write_failure().is_some_and(vectorcraft_format::refuses_writes) {
            self.0.store(true, Ordering::Relaxed);
        }
        r
    }
}

/// The status bar's line while the save panel asks where the refused file goes.
fn refused_status(path: &str) -> String {
    let name = fileio::file_name(path);
    crate::i18n::fmt(tl!("“{name}” is on a read-only location: choose where to save it"), &[("name", &name)])
}

/// Save As for document `uid` after its Save to `path` was refused: the save panel suggests the
/// same name in the documents folder, and the file is written in `format` with `options` (nothing
/// more is asked). It runs as its own entry, so a panel shown off the UI thread (a sheet) runs
/// this again when it answers. `None` when no panel can ask now (the host shows none, another file
/// dialog is open) or the document is gone: the usual dialog then. → what was written, `{pending}`
/// while the panel is open, or the error (`cancelled`).
fn save_elsewhere(
    app: &mut VectorcraftApp,
    uid: u64,
    path: &str,
    format: &'static str,
    options: Map<String, Value>,
) -> Option<Result<Value, String>> {
    if app.services.pick_save.is_none() || app.picks.is_waiting() {
        return None;
    }
    let i = app.session.documents().iter().position(|d| d.uid == uid)?;
    if app.session.active_index() != Some(i) && app.run("document.activate", json!({ "index": i })).is_err() {
        return None;
    }
    let status = refused_status(path);
    app.status(status.clone());
    let params = json!({ "format": format, "options": options });
    let folder = fileio::documents_dialog_folder();
    let save = Rc::new(move |app: &mut VectorcraftApp| crate::io::save_suggesting(app, SaveMode::SaveAs, &params, false, folder.clone()));
    let again = save.clone();
    let rerun: Rc<dyn Fn(&mut VectorcraftApp)> = Rc::new(move |app: &mut VectorcraftApp| {
        if let Err(e) = again(app) {
            app.status(e);
        }
    });
    let r = picks::as_own_entry(app, Entry::Call(rerun), |app| save(app));
    Some(match r {
        // The panel is open (a sheet): its answer writes the file.
        Err(e) if e == "cancelled" && app.picks.is_waiting() => {
            app.status(status);
            Ok(json!({ "pending": "fileDialog" }))
        }
        r => r,
    })
}
