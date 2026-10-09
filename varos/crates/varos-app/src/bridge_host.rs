//! Owning-thread adapter. Socket workers never see Workspace, Editor or egui.
use crate::{app_command::SessionId, host::DocUi, workspace::Workspace};
use std::cell::RefCell;
use varos_bridge::{BoardAccess, BoardInfo, Error, Host, Service};
use varos_core::editor::{AbDrag, Drag, ToolKind};
thread_local! { static SERVICE: RefCell<Option<Service>> = const { RefCell::new(None) }; }
pub fn initialize(epoch: String) {
    crate::agent_presence::clear();
    SERVICE.with(|s| *s.borrow_mut() = Some(Service::new(epoch)));
    FILE_RESULTS.with(|r| r.borrow_mut().clear());
    FILE_PENDING.with(|r| r.borrow_mut().clear());
    FILE_AUDIT.with(|r| r.borrow_mut().clear());
    FILE_EVICTED.with(|r| r.borrow_mut().clear());
}
struct Desktop<'a> {
    ws: &'a mut Workspace,
    ui: Option<&'a mut dyn DocUi>,
    snapshot: Option<varos_bridge::service::SnapshotJob>,
    files: Option<&'a mut dyn crate::host::FileJobs>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    audit: Option<(varos_bridge::conn::Paths, varos_bridge::conn::audit::Entry)>,
}
thread_local! {
    static FILE_AUDIT: RefCell<std::collections::HashMap<u64, (varos_bridge::conn::Paths, varos_bridge::conn::audit::Entry)>> = RefCell::new(std::collections::HashMap::new());
    static FILE_EVICTED: RefCell<std::collections::VecDeque<u64>> = const { RefCell::new(std::collections::VecDeque::new()) };
    static FILE_PENDING: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
    static FILE_RESULTS: RefCell<std::collections::VecDeque<(u64,varos_bridge::Reply)>> = const { RefCell::new(std::collections::VecDeque::new()) };
}
pub fn file_completed(ticket: u64, reply: varos_bridge::Reply) {
    FILE_AUDIT.with(|r| {
        if let Some((paths, mut entry)) = r.borrow_mut().remove(&ticket) {
            entry.t = varos_bridge::conn::now_secs();
            entry.event = "file_completed".into();
            entry.ticket = Some(ticket);
            entry.result = reply.error.as_ref().map_or("ok", |e| e.code.as_str()).into();
            let _ = varos_bridge::conn::audit::append(&paths, &entry);
        }
    });
    FILE_PENDING.with(|r| r.borrow_mut().remove(&ticket));
    FILE_RESULTS.with(|r| {
        let mut r = r.borrow_mut();
        r.push_back((ticket, reply));
        while r.len() > 8192 {
            if let Some((ticket, _)) = r.pop_front() {
                FILE_EVICTED.with(|e| {
                    let mut e = e.borrow_mut();
                    if !e.contains(&ticket) {
                        e.push_back(ticket);
                    }
                    if e.len() > 8192 {
                        e.pop_front();
                    }
                });
            }
        }
    });
}
fn session(board: &str) -> Result<SessionId, Error> {
    board
        .strip_prefix('b')
        .and_then(|n| n.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .map(SessionId)
        .ok_or_else(|| Error::new("invalid_argument", "board must be a session handle bN"))
}
impl Host for Desktop<'_> {
    fn set_paste_remembers_layers(&mut self, board: &str, enabled: bool) -> Result<(), Error> {
        if !self
            .ui
            .as_deref_mut()
            .is_some_and(|ui| ui.queue_app_command(crate::app_command::AppCommand::SetPasteRemembersLayers(enabled)))
        {
            return Err(Error::new("unsupported", "desktop settings queue unavailable"));
        }
        self.access(board)?
            .editor
            .execute(varos_core::EditCommand::SetPasteRemembersLayers(enabled))
            .map_err(|reason| Error::new("internal", reason.to_string()))?;
        Ok(())
    }
    fn window_memory(&mut self) -> Result<varos_bridge::Reply, Error> {
        let path = varos_app::storage::paths::AppLayout::current().map(|l| l.window_json());
        let (_, geometry) = varos_app::storage::window::WindowStore::load(&varos_app::storage::durable::RealFs, path);
        Ok(varos_bridge::Reply::success(serde_json::json!({"geometry":geometry,"source":"window.json"})))
    }

    fn snapshot(
        &mut self,
        job: varos_bridge::service::SnapshotJob,
        _: &std::sync::atomic::AtomicBool,
    ) -> varos_bridge::Reply {
        self.snapshot = Some(job);
        varos_bridge::Reply::success(serde_json::json!({"pending":true}))
    }
    fn file_pending(&self, ticket: u64) -> bool {
        !FILE_EVICTED.with(|r| r.borrow().contains(&ticket)) && FILE_PENDING.with(|r| r.borrow().contains(&ticket))
    }
    fn file_status(&mut self, ticket: u64) -> Option<varos_bridge::Reply> {
        FILE_RESULTS.with(|r| r.borrow().iter().find(|(t, _)| *t == ticket).map(|(_, r)| r.clone()))
    }
    // ---- Lane H: service cancellation reaches the isolated parser and publication guard ----
    fn import_effect(
        &mut self,
        verb: &str,
        request: &varos_bridge::dto::FileEffect,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<varos_bridge::Reply, Error> {
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Error::new("cancelled", "Import cancelled"));
        }
        if FILE_PENDING.with(|r| r.borrow().len() >= 8) {
            return Err(Error::new("busy", "eight file jobs are already pending"));
        }
        let Some(mut job) = crate::bridge_import::stage(self.ws, verb, request)? else {
            let s = self.ws.get_mut(session(&request.board)?).ok_or_else(|| Error::new("not_found", "board closed"))?;
            if s.editor.clipboard().is_empty() {
                return Err(Error::new("not_found", "Clipboard is empty"));
            }
            s.editor
                .try_execute(varos_core::EditCommand::Paste { offset: None })
                .map_err(|e| Error::new("invalid_argument", e))?;
            return Ok(varos_bridge::Reply::success(
                serde_json::json!({"rev":s.editor.rev,"report":varos_import::ImportReport::default()}),
            ));
        };
        let ticket = crate::file_jobs::next_ticket();
        job.ticket = Some(ticket);
        job.cancel = crate::file_jobs::CancelFlag::from_shared(self.cancel.clone());
        self.files
            .as_deref_mut()
            .ok_or_else(|| Error::new("busy", "file worker unavailable"))?
            .submit(crate::file_jobs::FileJob::Import(job))
            .map_err(|_| Error::new("busy", "file worker unavailable"))?;
        FILE_PENDING.with(|r| r.borrow_mut().insert(ticket));
        if let Some(audit) = self.audit.clone() {
            FILE_AUDIT.with(|r| r.borrow_mut().insert(ticket, audit));
        }
        Ok(varos_bridge::Reply::success(serde_json::json!({"accepted":true,"ticket":ticket})))
    }
    // ---- End Lane H ----
    fn file_effect(
        &mut self,
        verb: &str,
        request: &varos_bridge::dto::FileEffect,
    ) -> Result<varos_bridge::Reply, Error> {
        // ---- w2-images ----
        if verb == "image_action" {
            let sid = session(&request.board)?;
            let s = self.ws.get_mut(sid).ok_or_else(|| Error::new("not_found", "board closed"))?;
            if s.editor.rev != request.expected_rev
                || s.editor.transaction_open()
                || !matches!(s.editor.drag, varos_core::editor::Drag::None)
            {
                return Err(Error::new("busy", "image target changed or gesture active"));
            }
            let options =
                request.options.clone().ok_or_else(|| Error::new("invalid_argument", "image operation required"))?;
            if matches!(options["action"].as_str(), Some("relink" | "update")) {
                let id = options["id"]
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or_else(|| Error::new("invalid_argument", "image id required"))?;
                let image = s
                    .editor
                    .doc
                    .images
                    .iter()
                    .find(|i| i.id == id)
                    .ok_or_else(|| Error::new("not_found", "image unavailable"))?;
                let path = if options["action"] == "relink" {
                    std::path::PathBuf::from(
                        options["path"]
                            .as_str()
                            .ok_or_else(|| Error::new("invalid_argument", "image source required"))?,
                    )
                } else {
                    varos_core::images::links::update_path(image, &s.editor.blobs)
                        .map_err(|e| Error::new("invalid_argument", e))?
                };
                if FILE_PENDING.with(|r| r.borrow().len() >= 8) {
                    return Err(Error::new("busy", "eight file jobs are already pending"));
                }
                let ticket = crate::file_jobs::next_ticket();
                let job = crate::image_jobs::Job {
                    sid,
                    ticket,
                    expected_rev: request.expected_rev,
                    path,
                    replace: Some(id),
                    bytes: None,
                    options: crate::image_jobs::Options { mode: image.placement, ..Default::default() },
                    bridge: true,
                    cancel: crate::file_jobs::CancelFlag::from_shared(self.cancel.clone()),
                };
                self.files
                    .as_deref_mut()
                    .ok_or_else(|| Error::new("busy", "file worker unavailable"))?
                    .submit(crate::file_jobs::FileJob::Image(Box::new(job)))
                    .map_err(|_| Error::new("busy", "file worker unavailable"))?;
                FILE_PENDING.with(|r| r.borrow_mut().insert(ticket));
                return Ok(varos_bridge::Reply::success(serde_json::json!({"accepted":true,"ticket":ticket})));
            }
            if options["action"] == "package" {
                let path =
                    options["path"].as_str().ok_or_else(|| Error::new("invalid_argument", "package path required"))?;
                varos_pdf::package::package(&s.editor.doc, &s.editor.blobs, std::path::Path::new(path))
                    .map_err(|e| Error::new("file_error", e))?;
                return Ok(varos_bridge::Reply::success(serde_json::json!({"packaged":true})));
            }
            let op = serde_json::from_value(options).map_err(|e| Error::new("invalid_argument", e.to_string()))?;
            let result = varos_bridge::images::run(&mut s.editor, op).map_err(|e| Error::new("invalid_argument", e))?;
            return Ok(varos_bridge::Reply::success(result));
        }
        if verb == "add_image" {
            let sid = session(&request.board)?;
            let s = self.ws.get(sid).ok_or_else(|| Error::new("not_found", "board closed"))?;
            if s.editor.rev != request.expected_rev {
                return Err(Error::new("stale_revision", "document changed"));
            }
            if FILE_PENDING.with(|r| r.borrow().len() >= 8) {
                return Err(Error::new("busy", "eight file jobs are already pending"));
            }
            let options = serde_json::from_value(request.options.clone().unwrap_or_else(|| serde_json::json!({})))
                .map_err(|e| Error::new("invalid_argument", e.to_string()))?;
            let ticket = crate::file_jobs::next_ticket();
            let job = crate::image_jobs::Job {
                sid,
                ticket,
                expected_rev: request.expected_rev,
                path: std::path::PathBuf::from(
                    request.path.as_deref().ok_or_else(|| Error::new("invalid_argument", "image path required"))?,
                ),
                options,
                replace: None,
                bytes: None,
                bridge: true,
                cancel: crate::file_jobs::CancelFlag::from_shared(self.cancel.clone()),
            };
            self.files
                .as_deref_mut()
                .ok_or_else(|| Error::new("busy", "file worker unavailable"))?
                .submit(crate::file_jobs::FileJob::Image(Box::new(job)))
                .map_err(|_| Error::new("busy", "file worker unavailable"))?;
            FILE_PENDING.with(|r| r.borrow_mut().insert(ticket));
            return Ok(varos_bridge::Reply::success(serde_json::json!({"accepted":true,"ticket":ticket})));
        }
        if ["save_template", "new_from_template"].contains(&verb) {
            let name =
                request.path.as_deref().ok_or_else(|| Error::new("invalid_argument", "template name required"))?;
            let path = varos_bridge::templates::path(name)?;
            if FILE_PENDING.with(|r| r.borrow().len() >= 8) {
                return Err(Error::new("busy", "eight file jobs are already pending"));
            }
            let s = self.ws.get(session(&request.board)?).ok_or_else(|| Error::new("not_found", "board closed"))?;
            let document = (verb == "save_template").then(|| std::sync::Arc::new(s.editor.doc.clone()));
            let ticket = crate::file_jobs::next_ticket();
            let job = crate::template_jobs::Job {
                ticket,
                path,
                document,
                cancel: crate::file_jobs::CancelFlag::from_shared(self.cancel.clone()),
            };
            self.files
                .as_deref_mut()
                .ok_or_else(|| Error::new("busy", "file worker unavailable"))?
                .submit(crate::file_jobs::FileJob::Template(job))
                .map_err(|_| Error::new("busy", "file worker unavailable"))?;
            FILE_PENDING.with(|r| r.borrow_mut().insert(ticket));
            if let Some(audit) = self.audit.clone() {
                FILE_AUDIT.with(|r| r.borrow_mut().insert(ticket, audit));
            }
            return Ok(varos_bridge::Reply::success(serde_json::json!({"accepted":true,"ticket":ticket})));
        }
        use crate::file_jobs::{BridgeFileJob, ExportJob, FileJob, SaveInFlight, SaveJob};
        if ["print", "copy", "cut"].contains(&verb) {
            let id = session(&request.board)?;
            let session = self.ws.get_mut(id).ok_or_else(|| Error::new("not_found", "board closed"))?;
            if request.path.is_some() {
                return Err(Error::new("invalid_argument", "host effect does not take a destination"));
            }
            if verb == "print" {
                let options = request
                    .options
                    .clone()
                    .map(serde_json::from_value::<varos_pdf::PdfOptions>)
                    .transpose()
                    .map_err(|e| Error::new("invalid_argument", e.to_string()))?
                    .unwrap_or_default();
                let scope = match request.scope.as_deref() {
                    None => varos_pdf::default_scope(&session.editor.doc),
                    Some("active_artboard") => varos_pdf::ExportScope::ActiveArtboard,
                    Some("all_visible_artboards") => varos_pdf::ExportScope::AllVisibleArtboards,
                    Some("artwork_bounds") => varos_pdf::ExportScope::ArtworkBounds,
                    _ => return Err(Error::new("invalid_argument", "invalid print scope")),
                };
                let job = crate::print_job::build_with_images(
                    &session.editor.doc,
                    &session.editor.blobs,
                    scope,
                    &options,
                    &std::env::temp_dir(),
                    crate::file_jobs::next_ticket(),
                )
                .map_err(|e| Error::new("invalid_argument", e))?;
                crate::print_job::hand_off(job).map_err(|e| Error::new("io_error", e))?;
                return Ok(varos_bridge::Reply::success(
                    serde_json::json!({"opened_preview":true,"instruction":"Choose File > Print in Preview"}),
                ));
            }
            if request.scope.is_some() || request.options.is_some() {
                return Err(Error::new("invalid_argument", "clipboard effects use current selection and no options"));
            }
            let report = crate::os_clipboard::perform(
                &mut session.editor,
                verb == "cut",
                &mut crate::os_clipboard::SystemPasteboard,
            )
            .map_err(|e| Error::new("io_error", e))?;
            return Ok(varos_bridge::Reply::success(serde_json::json!(report)));
        }

        if FILE_PENDING.with(|r| r.borrow().len() >= 8) {
            return Err(Error::new("busy", "eight file jobs are already pending"));
        }
        let id = session(&request.board)?;
        let s = self.ws.get(id).ok_or_else(|| Error::new("not_found", "board closed"))?;
        if !verb.starts_with("export_") && s.saving.is_some() {
            return Err(Error::new("busy", "save in progress"));
        }
        let mut snapshot = s.editor.doc.clone();
        let ticket = crate::file_jobs::next_ticket();
        let home = varos_bridge::conn::fsutil::user_home_dir()
            .map_err(|e| Error::new("io_error", format!("user home unavailable: {e}")))?;
        let extension = match verb {
            "export_svg" => "svg",
            "export_raster" => varos_raster::export::Format::parse(request.format.as_deref().unwrap_or("png"))
                .map_err(|e| Error::new("invalid_argument", e))?
                .extension(),
            "export_pdf" => "pdf",
            _ => "vrs",
        };
        let expected =
            if verb == "save" {
                Some((
                    s.key.as_ref().map(|k| k.path.clone()).or_else(|| s.path.clone()).ok_or_else(|| {
                        Error::new("invalid_argument", "save needs a CURRENT backing file; use save_as")
                    })?,
                    s.source_fingerprint,
                ))
            } else {
                None
            };
        let dest = if let Some((p, _)) = &expected {
            p.clone()
        } else {
            let path = std::path::PathBuf::from(
                request.path.as_ref().ok_or_else(|| Error::new("invalid_argument", "path required"))?,
            );
            varos_bridge::files::validate_path(&path, extension)?;
            path
        };
        varos_bridge::files::validate_path(&dest, extension)?;
        let mut inner = if matches!(verb, "export_svg" | "export_raster") {
            FileJob::Screen(Box::new(crate::export_ui::bridge_job(
                id,
                ticket,
                &snapshot,
                &s.editor.selected_pids(),
                dest.clone(),
                request,
                verb,
            )?))
        } else if verb == "export_pdf" {
            let scope = match request.scope.as_deref() {
                Some("all_visible_artboards") => varos_pdf::ExportScope::AllVisibleArtboards,
                Some("artwork_bounds") => varos_pdf::ExportScope::ArtworkBounds,
                Some(id) if id.starts_with("artboard:") => {
                    let n = id
                        .strip_prefix("artboard:")
                        .and_then(|n| n.parse::<u32>().ok())
                        .filter(|n| *n > 0 && format!("artboard:{n}") == id)
                        .ok_or_else(|| Error::new("invalid_argument", "invalid artboard id"))?;
                    let i = snapshot.artboard_index(n).ok_or_else(|| Error::new("not_found", "unknown page"))?;
                    if snapshot.artboards[i].hidden {
                        return Err(Error::new("hidden_target", "page is hidden"));
                    }
                    snapshot.active = i;
                    varos_pdf::ExportScope::ActiveArtboard
                }
                _ => {
                    return Err(Error::new(
                        "invalid_argument",
                        "scope must be all_visible_artboards, artwork_bounds, or artboard:N",
                    ))
                }
            };
            let plan =
                varos_pdf::plan_pdf_export(&snapshot, scope).map_err(|e| Error::new("invalid_argument", e.reason()))?;
            FileJob::Export(ExportJob {
                blobs: std::sync::Arc::new(s.editor.blobs.clone()),
                pdf_options: request
                    .options
                    .clone()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|e| Error::new("invalid_argument", e.to_string()))?
                    .unwrap_or_default(),
                sid: id,
                dest: dest.clone(),
                doc: std::sync::Arc::new(snapshot),
                plan,
                replace_confirmed: false,
                cancel: Default::default(),
                ticket: 0, // a Bridge export has no sheet
            })
        } else {
            FileJob::Save(SaveJob {
                blobs: std::sync::Arc::new(s.editor.blobs.clone()),
                sid: id,
                ticket,
                dest: dest.clone(),
                doc: std::sync::Arc::new(snapshot),
            })
        };
        match &mut inner {
            FileJob::Export(e) => e.blobs = std::sync::Arc::new(s.editor.blobs.clone()),
            FileJob::Screen(e) => e.job.blobs = std::sync::Arc::new(s.editor.blobs.clone()),
            _ => {}
        }
        let save = if let FileJob::Save(j) = &inner { Some(j.doc.clone()) } else { None };
        let worker = self.files.as_deref_mut().ok_or_else(|| Error::new("busy", "file worker unavailable"))?;
        worker
            .submit(FileJob::Bridge(Box::new(BridgeFileJob { ticket, inner, home, expected })))
            .map_err(|_| Error::new("busy", "file worker unavailable"))?;
        FILE_PENDING.with(|r| r.borrow_mut().insert(ticket));
        if let Some(audit) = self.audit.clone() {
            FILE_AUDIT.with(|r| r.borrow_mut().insert(ticket, audit));
        }
        if let Some(doc) = save {
            self.ws.get_mut(id).expect("session").saving = Some(SaveInFlight {
                blobs: std::sync::Arc::new(self.ws.get(id).map(|s| s.editor.blobs.clone()).unwrap_or_default()),
                ticket,
                dest,
                doc,
                follow_up: false,
                started: std::time::Instant::now(),
            });
        }
        Ok(varos_bridge::Reply::success(serde_json::json!({"accepted":true,"ticket":ticket})))
    }
    fn build(&self) -> &str {
        concat!("varos-app ", env!("CARGO_PKG_VERSION"))
    }
    fn boards(&self) -> Vec<BoardInfo> {
        self.ws
            .visible_tabs()
            .iter()
            .filter_map(|t| self.ws.get(t.id))
            .map(|s| BoardInfo {
                board: format!("b{}", s.id.0),
                name: s.editor.doc.name.clone(),
                rev: s.editor.rev,
                dirty: s.is_dirty(),
                active: self.ws.document_target() == Some(s.id),
                backing_file: s.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
            })
            .collect()
    }
    fn observation_ids(&self) -> Vec<String> {
        self.ws.visible_tabs().iter().map(|t| format!("b{}", t.id.0)).collect()
    }
    fn observation_access(&mut self, board: &str) -> Result<BoardAccess<'_>, Error> {
        let s = self.ws.get_mut(session(board)?).ok_or_else(|| Error::new("not_found", "board closed"))?;
        Ok(BoardAccess { editor: &mut s.editor, dirty: false })
    }
    fn prepare(&mut self, board: &str, mutation: bool) -> Result<(), Error> {
        let id = session(board)?;
        if !self.ws.visible_tabs().iter().any(|t| t.id == id) {
            return Err(Error::new("not_found", "board closed or not exposed"));
        }
        let active = self.ws.document_target() == Some(id);
        if mutation && !active {
            return Err(Error::new(
                "board_not_active",
                "activate this board in Varos first; no implicit tab switching",
            ));
        }
        let s = self.ws.get_mut(id).ok_or_else(|| Error::new("not_found", "board closed"))?;
        if mutation && active {
            if let Some(ui) = self.ui.as_deref_mut() {
                ui.cancel_picker_sample(&mut s.editor);
            }
        }
        if s.editor.transaction_open()
            || (s.editor.tool == ToolKind::Pen && s.editor.active.is_some())
            || !matches!(s.editor.drag, Drag::None)
            || !matches!(s.editor.ab_drag, AbDrag::None)
            || s.editor.origin_preview.is_some()
            || s.editor.guide_preview.is_some()
        {
            return Err(Error::new("busy", "human gesture is active"));
        }
        if active {
            if let Some(ui) = self.ui.as_deref_mut() {
                if ui.bridge_preview_active() {
                    return Err(Error::new("busy", "human picker or panel gesture is active"));
                }
                if mutation && !ui.settle_fields(&mut s.editor) {
                    return Err(Error::new("busy", "field_invalid: finish or correct the human field first"));
                }
            }
        }
        Ok(())
    }
    fn access(&mut self, board: &str) -> Result<BoardAccess<'_>, Error> {
        let id = session(board)?;
        if !self.ws.visible_tabs().iter().any(|t| t.id == id) {
            return Err(Error::new("not_found", "board closed or not authorized"));
        }
        let s = self.ws.get_mut(id).ok_or_else(|| Error::new("not_found", "board closed"))?;
        let dirty = s.is_dirty();
        Ok(BoardAccess { editor: &mut s.editor, dirty })
    }
}
/// Owner-readable body for the one-time "agents can't connect" notice.
pub fn unavailable_text(reason: &str) -> String {
    format!(
        "Varos works normally, but AI agents (Claude Code, Codex…) can't connect to it in this session.\n\nVaros couldn't use its connection key file ({reason}).\n\nFix the key file or directory as described above, then quit and reopen Varos."
    )
}
pub fn observe(ws: &mut Workspace) {
    SERVICE.with(|s| {
        if let Some(service) = s.borrow_mut().as_mut() {
            service.observe(&mut Desktop {
                ws,
                ui: None,
                snapshot: None,
                files: None,
                cancel: Default::default(),
                audit: None,
            });
        }
    });
}
pub fn run_with_files<'a>(
    request: varos_bridge::ipc::Pending,
    ws: &'a mut Workspace,
    ui: &'a mut dyn DocUi,
    files: Option<&'a mut dyn crate::host::FileJobs>,
) -> crate::host::Ran {
    let before = ws.document_target();
    let accepted = crate::agent_presence::accept(&request, ws, std::time::Instant::now());
    let mut desktop = Desktop {
        ws,
        ui: Some(ui),
        snapshot: None,
        files,
        cancel: request.cancelled.clone(),
        audit: request.file_audit.clone(),
    };
    let reply = SERVICE.with(|s| match s.borrow_mut().as_mut() {
        Some(service) => service.handle_borrowed(&mut desktop, &request.context, &request.request, &request.cancelled),
        None => varos_bridge::Reply::failure(Error::new("unsupported", "attachment listener unavailable")),
    });
    crate::agent_presence::complete(accepted, &reply, &request.request, desktop.ws, std::time::Instant::now());
    let changed = reply.ok && reply.request_id.is_some();
    let switched = before != desktop.ws.document_target();
    if switched {
        if let Some(ui) = desktop.ui.as_mut() {
            ui.document_switched();
        }
    }
    if let Some(job) = desktop.snapshot.take().filter(|_| reply.ok) {
        // No Workspace/Editor/UI reference crosses this boundary. The existing reply channel
        // delivers the pinned image; cancellation is checked before raster, encode and reply.
        let failure_channel = request.reply.clone();
        let failure_board = reply.board.clone();
        let failure_rev = reply.rev;
        if let Err(error) = std::thread::Builder::new().name("bridge-snapshot".into()).spawn(move || {
            let mut rendered = job.render(&request.cancelled);
            rendered.board = reply.board;
            rendered.rev = reply.rev;
            let _ = request.reply.send(rendered);
        }) {
            let mut failure =
                varos_bridge::Reply::failure(Error::new("busy", format!("snapshot worker unavailable: {error}")));
            failure.board = failure_board;
            failure.rev = failure_rev;
            let _ = failure_channel.send(failure);
        }
    } else {
        let _ = request.reply.send(reply);
    }
    crate::host::Ran { ran: changed, switched, ..Default::default() }
}
#[cfg(test)]
pub fn run(request: varos_bridge::ipc::Pending, ws: &mut Workspace, ui: &mut dyn DocUi) -> crate::host::Ran {
    run_with_files(request, ws, ui, None)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fields {
        valid: bool,
        commit: bool,
    }
    impl DocUi for Fields {
        fn settle(&mut self, ed: &mut varos_core::editor::Editor) -> bool {
            if self.commit {
                ed.execute_ui(varos_core::EditCommand::SetBoardName("human field".into()));
                self.commit = false;
            }
            self.valid
        }
        fn document_switched(&mut self) {}
    }
    fn request(ws: &mut Workspace, fields: &mut dyn DocUi) -> varos_bridge::Reply {
        initialize("epoch".into());
        let rev = ws.active().unwrap().editor.rev;
        let board = format!("b{}", ws.active_id().unwrap().0);
        let req = varos_bridge::mcp::decode_tool(
            "select",
            serde_json::json!({"api":"1.0","request_id":"r1","board":board,"expected_rev":rev,"ids":[]}),
        )
        .unwrap();
        dispatch(ws, fields, req)
    }
    fn dispatch(ws: &mut Workspace, fields: &mut dyn DocUi, req: varos_bridge::Request) -> varos_bridge::Reply {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        run(
            varos_bridge::ipc::Pending {
                context: varos_bridge::Context { client: "test".into(), epoch: "epoch".into() },
                request: req,
                cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                reply: tx,

                file_audit: None,
            },
            ws,
            fields,
        );
        rx.recv().unwrap()
    }
    #[test]
    fn export_can_queue_while_save_is_in_flight() {
        #[derive(Default)]
        struct Jobs {
            wait: crate::host::SaveWait,
            queued: Vec<crate::file_jobs::FileJob>,
        }
        impl crate::host::FileJobs for Jobs {
            fn submit(&mut self, job: crate::file_jobs::FileJob) -> Result<(), crate::file_jobs::FileJob> {
                self.queued.push(job);
                Ok(())
            }
            fn save_wait(&mut self) -> &mut crate::host::SaveWait {
                &mut self.wait
            }
        }
        initialize("epoch".into());
        let mut ws = Workspace::new();
        let id = ws.active_id().unwrap();
        let s = ws.get_mut(id).unwrap();
        s.editor.doc.artboards.push(varos_core::model::Artboard { id: 100, w: 100.0, h: 100.0, ..Default::default() });
        s.saving = Some(crate::file_jobs::SaveInFlight {
            blobs: Default::default(),
            ticket: 999,
            dest: "/tmp/original.vrs".into(),
            doc: std::sync::Arc::new(s.editor.doc.clone()),
            follow_up: false,
            started: std::time::Instant::now(),
        });
        let request: varos_bridge::dto::FileEffect = serde_json::from_value(serde_json::json!({"request_id":"r1","board":format!("b{}",id.0),"expected_rev":s.editor.rev,"path":varos_bridge::conn::fsutil::user_home_dir().unwrap().join("copy.PDF"),"scope":"all_visible_artboards"})).unwrap();
        assert_eq!(request.api, "1.0");
        let mut jobs = Jobs::default();
        let mut host = Desktop {
            ws: &mut ws,
            ui: None,
            snapshot: None,
            files: Some(&mut jobs),
            cancel: Default::default(),
            audit: None,
        };
        assert_eq!(host.file_effect("save_as", &request).unwrap_err().code, "busy");
        assert!(host.file_effect("export_pdf", &request).unwrap().ok);
        assert_eq!(host.ws.get(id).unwrap().saving.as_ref().unwrap().ticket, 999);
        assert_eq!(jobs.queued.len(), 1);
    }
    #[test]
    fn completion_audit_records_outcome_without_paths() {
        initialize("epoch".into());
        let root =
            std::env::temp_dir().join(format!("bridge-completion-audit-{}", varos_app::storage::checksum::new_nonce()));
        let paths = varos_bridge::conn::Paths::under(&root);
        let mut entry = varos_bridge::conn::audit::Entry::event("call", "fake-agent", "accepted");
        entry.verb = Some("save_as".into());
        entry.board = Some("b1".into());
        FILE_AUDIT.with(|r| r.borrow_mut().insert(42, (paths.clone(), entry)));
        file_completed(
            42,
            varos_bridge::Reply::failure(Error::new("save_conflict", "private path must not be audited")),
        );
        let lines = varos_bridge::conn::audit::tail(&paths, 10);
        assert_eq!(lines.len(), 1);
        let entry: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(entry["event"], "file_completed");
        assert_eq!(entry["verb"], "save_as");
        assert_eq!(entry["board"], "b1");
        assert_eq!(entry["ticket"], 42);
        assert_eq!(entry["result"], "save_conflict");
        assert!(!lines[0].contains("private path"));
        assert!(entry.get("path").is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn file_host_guards_and_open_trust_capability() {
        initialize("epoch".into());
        let mut ws = Workspace::new();
        let board = format!("b{}", ws.active_id().unwrap().0);
        let mut host =
            Desktop { ws: &mut ws, ui: None, snapshot: None, files: None, cancel: Default::default(), audit: None };
        for (verb, path, code) in [
            ("save_as", "/tmp/copy.vrs", "scope_refused"),
            ("export_pdf", "/tmp/export.pdf", "scope_refused"),
            ("save_as", "/private/var/copy.vrs", "scope_refused"),
            ("save_as", "relative.vrs", "invalid_argument"),
            ("export_pdf", "/tmp/wrong.vrs", "invalid_argument"),
            ("save_as", "/tmp/wrong.pdf", "invalid_argument"),
            ("save_as", "/tmp/", "invalid_argument"),
            ("save_as", "/tmp/.hidden/copy.vrs", "scope_refused"),
            ("export_pdf", "/System/export.pdf", "scope_refused"),
            ("export_pdf", "/Applications/export.pdf", "scope_refused"),
            ("export_pdf", "/Library/export.pdf", "scope_refused"),
        ] {
            let request = varos_bridge::dto::FileEffect {
                format: None,
                scale: None,
                ppi: None,
                quality: None,
                transparent: None,
                options: None,
                api: "1.0".into(),
                request_id: "r1".into(),
                board: board.clone(),
                expected_rev: 0,
                path: Some(path.into()),
                scope: Some("artwork_bounds".into()),
            };
            assert_eq!(host.file_effect(verb, &request).unwrap_err().code, code, "{path}");
        }
        let mut service = Service::new("epoch".into());
        let reply = service.handle(
            &mut host,
            &varos_bridge::Context { client: "c".into(), epoch: "epoch".into() },
            varos_bridge::mcp::decode_tool("capabilities", serde_json::json!({})).unwrap(),
            &std::sync::atomic::AtomicBool::new(false),
        );
        let result = reply.result.unwrap();
        assert!(result.get("files_roots_granted").is_none());
        assert_eq!(result["trust"], "local user");
        assert!(result["file_guards"].as_array().unwrap().len() >= 7);
        assert!(result.get("files_roots").is_none());
    }
    #[test]
    fn completion_eviction_and_restart_are_not_pending() {
        initialize("epoch".into());
        for ticket in 1..=8193 {
            file_completed(ticket, varos_bridge::Reply::success(serde_json::json!({})));
        }
        assert!(FILE_EVICTED.with(|r| r.borrow().contains(&1)));
        let mut ws = Workspace::new();
        let host =
            Desktop { ws: &mut ws, ui: None, snapshot: None, files: None, cancel: Default::default(), audit: None };
        assert!(!host.file_pending(1));
        FILE_PENDING.with(|r| r.borrow_mut().insert(9000));
        assert!(host.file_pending(9000));
        initialize("new epoch".into());
        assert!(!host.file_pending(9000));
    }
    #[test]
    fn snapshot_worker_replies_with_captured_revision_after_human_edit() {
        snapshot_worker_pins_revision(false);
    }

    #[test]
    fn page_snapshot_worker_replies_with_captured_revision_after_human_edit() {
        snapshot_worker_pins_revision(true);
    }

    fn snapshot_worker_pins_revision(page: bool) {
        initialize("epoch".into());
        let mut ws = Workspace::new();
        let artboard = if page {
            let mut doc = ws.active().unwrap().editor.doc.clone();
            let id = doc.nid();
            doc.artboards.push(varos_core::model::Artboard { id, w: 1080.0, h: 1920.0, ..Default::default() });
            ws.active_mut().unwrap().editor.replace_doc(doc);
            Some(format!("artboard:{id}"))
        } else {
            None
        };
        let board = format!("b{}", ws.active_id().unwrap().0);
        let rev = ws.active().unwrap().editor.rev;
        let before = ws.active().unwrap().editor.doc.clone();
        let mut args = serde_json::json!({"board":board,"rev":rev,"width":80,"height":40});
        if let Some(id) = &artboard {
            args["artboard"] = serde_json::json!(id);
        }
        let req = varos_bridge::mcp::decode_tool("snapshot", args).unwrap();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let mut fields = Fields { valid: true, commit: true };
        let ran = run(
            varos_bridge::ipc::Pending {
                context: varos_bridge::Context { client: "test".into(), epoch: "epoch".into() },
                request: req,
                cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                reply: tx,

                file_audit: None,
            },
            &mut ws,
            &mut fields,
        );
        assert!(!ran.ran);
        assert!(fields.commit);
        assert_eq!(ws.active().unwrap().editor.doc, before);
        assert!(!ws.active().unwrap().editor.history_available(false));
        ws.active_mut().unwrap().editor.execute_ui(varos_core::EditCommand::SetBoardName("later human edit".into()));
        let reply = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(reply.ok, "{reply:?}");
        assert_eq!(reply.board, Some(board));
        assert_eq!(reply.rev, Some(rev));
        let result = reply.result.unwrap();
        assert_eq!(result["rev"], rev);
        if let Some(id) = artboard {
            assert_eq!(result["artboard"], id);
            assert_eq!(result["preview"], "CPU page preview");
            assert_eq!(result["width"], 23);
            assert_eq!(result["height"], 40);
        } else {
            assert_eq!(result["width"], 80);
            assert_eq!(result["height"], 40);
        }
        assert!(ws.active().unwrap().editor.rev > rev);
    }

    #[test]
    fn describe_keeps_pending_field_open_without_an_undo_step() {
        initialize("epoch".into());
        let mut ws = Workspace::new();
        let board = format!("b{}", ws.active_id().unwrap().0);
        let rev = ws.active().unwrap().editor.rev;
        let before = ws.active().unwrap().editor.doc.clone();
        let mut fields = Fields { valid: true, commit: true };
        let describe = varos_bridge::mcp::decode_tool("describe", serde_json::json!({"board":board})).unwrap();
        assert!(dispatch(&mut ws, &mut fields, describe).ok);
        assert!(fields.commit, "pending human text is still open and uncommitted");
        assert_eq!(ws.active().unwrap().editor.doc, before);
        assert_eq!(ws.active().unwrap().editor.rev, rev);
        assert!(!ws.active().unwrap().editor.history_available(false));
    }
    #[test]
    fn invalid_field_refuses_without_dismissal() {
        let mut ws = Workspace::new();
        let mut f = Fields { valid: false, commit: false };
        assert_eq!(request(&mut ws, &mut f).error.unwrap().code, "busy");
        assert!(!f.valid);
    }
    #[test]
    fn field_commits_human_step_then_rechecks_revision() {
        let mut ws = Workspace::new();
        let mut f = Fields { valid: true, commit: true };
        assert_eq!(request(&mut ws, &mut f).error.unwrap().code, "revision_conflict");
        ws.active_mut().unwrap().editor.undo();
        assert_ne!(ws.active().unwrap().editor.doc.name, "human field");
    }
    #[test]
    fn home_refuses_and_does_not_switch() {
        let mut ws = Workspace::new();
        ws.show_home();
        assert_eq!(
            request(&mut ws, &mut Fields { valid: true, commit: false }).error.unwrap().code,
            "board_not_active"
        );
        assert!(ws.on_home());
    }
    #[test]
    fn bridge_mutation_cancels_unaccepted_sample_before_busy_check() {
        struct Sample;
        impl DocUi for Sample {
            fn cancel_picker_sample(&mut self, ed: &mut varos_core::Editor) {
                ed.execute_ui(varos_core::EditCommand::PickerCancel);
            }
            fn settle(&mut self, _: &mut varos_core::Editor) -> bool {
                true
            }
            fn document_switched(&mut self) {}
        }
        let mut ws = Workspace::new();
        let ed = &mut ws.active_mut().unwrap().editor;
        ed.doc.paths.push(varos_core::model::Path::new(1, vec![], true, Some([1., 0., 0., 1.]), None, 1.));
        ed.doc.sync_tree();
        ed.objsel.insert(1);
        let before = ed.doc.clone();
        let rev = ed.rev;
        let defaults = (ed.cur_fill, ed.cur_stroke);
        ed.picker_begin();
        ed.paint_live(varos_core::editor::PaintTarget::Fill, Some([0., 1., 0., 1.]));
        assert!(request(&mut ws, &mut Sample).ok);
        let ed = &ws.active().unwrap().editor;
        assert_eq!(ed.doc, before);
        assert_eq!(ed.rev, rev);
        assert_eq!((ed.cur_fill, ed.cur_stroke), defaults);
        assert!(ed.recent_colors.is_empty());
        assert!(!ed.transaction_open());
    }
    #[test]
    fn gesture_refuses() {
        let mut ws = Workspace::new();
        ws.active_mut().unwrap().editor.begin();
        assert_eq!(request(&mut ws, &mut Fields { valid: true, commit: false }).error.unwrap().code, "busy");
        assert!(ws.active().unwrap().editor.transaction_open());
    }
    #[test]
    fn pen_path_between_clicks_refuses() {
        let mut ws = Workspace::new();
        let editor = &mut ws.active_mut().unwrap().editor;
        editor.tool = ToolKind::Pen;
        editor.active = Some(1);
        assert!(!editor.transaction_open());
        assert_eq!(request(&mut ws, &mut Fields { valid: true, commit: false }).error.unwrap().code, "busy");
        assert_eq!(ws.active().unwrap().editor.active, Some(1));
    }
}

#[cfg(test)]
mod template_queue_tests {
    use super::*;
    #[derive(Default)]
    struct Jobs {
        wait: crate::host::SaveWait,
        queued: Vec<crate::file_jobs::FileJob>,
    }
    impl crate::host::FileJobs for Jobs {
        fn submit(&mut self, job: crate::file_jobs::FileJob) -> Result<(), crate::file_jobs::FileJob> {
            self.queued.push(job);
            Ok(())
        }
        fn save_wait(&mut self) -> &mut crate::host::SaveWait {
            &mut self.wait
        }
    }
    #[test]
    fn template_host_only_queues_revision_snapshot_and_cancel_flag() {
        initialize("epoch".into());
        let mut ws = Workspace::new();
        let id = ws.active_id().unwrap();
        ws.get_mut(id).unwrap().editor.execute(varos_core::EditCommand::SetPpi(300.0)).unwrap();
        let mut jobs = Jobs::default();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let request: varos_bridge::dto::FileEffect = serde_json::from_value(serde_json::json!({"api":"1.2","request_id":"r1","board":format!("b{}",id.0),"expected_rev":ws.get(id).unwrap().editor.rev,"path":"queue-only-no-io.vrs"})).unwrap();
        let mut host = Desktop {
            ws: &mut ws,
            ui: None,
            snapshot: None,
            files: Some(&mut jobs),
            cancel: cancel.clone(),
            audit: None,
        };
        let reply = host.file_effect("save_template", &request).unwrap();
        assert_eq!(reply.result.as_ref().unwrap()["accepted"], true);
        let ticket = reply.result.unwrap()["ticket"].as_u64().unwrap();
        assert!(host.file_pending(ticket));
        assert!(host.ws.get(id).unwrap().saving.is_none());
        host.ws.get_mut(id).unwrap().editor.execute(varos_core::EditCommand::SetPpi(72.0)).unwrap();
        assert!(host.file_effect("new_from_template", &request).unwrap().ok);
        let crate::file_jobs::FileJob::Template(load) = jobs.queued.pop().unwrap() else {
            panic!("template read queue")
        };
        assert!(load.document.is_none());
        let crate::file_jobs::FileJob::Template(job) = jobs.queued.pop().unwrap() else { panic!("template queue") };
        assert_eq!(job.document.as_ref().unwrap().units.ppi, 300.0);
        cancel.store(true, std::sync::atomic::Ordering::Release);
        assert_eq!(crate::template_jobs::execute(job).result.unwrap_err().code, "cancelled");
    }
    #[test]
    fn import_host_queues_without_reading_and_returns_cancellable_ticket() {
        initialize("epoch".into());
        let mut ws = Workspace::new();
        let id = ws.active_id().unwrap();
        let before = ws.get(id).unwrap().editor.doc.clone();
        let mut jobs = Jobs::default();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let request: varos_bridge::dto::FileEffect = serde_json::from_value(serde_json::json!({"api":"1.2","request_id":"import","board":format!("b{}",id.0),"expected_rev":0,"path":"/no-such-import.pdf","options":{"page":2}})).unwrap();
        let mut host = Desktop {
            ws: &mut ws,
            ui: None,
            snapshot: None,
            files: Some(&mut jobs),
            cancel: cancel.clone(),
            audit: None,
        };
        let reply = host.import_effect("import_file", &request, &std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert_eq!(reply.result.as_ref().unwrap()["accepted"], true);
        let ticket = reply.result.unwrap()["ticket"].as_u64().unwrap();
        assert!(host.file_pending(ticket));
        assert_eq!(host.ws.get(id).unwrap().editor.doc, before);
        let crate::file_jobs::FileJob::Import(job) = jobs.queued.pop().unwrap() else { panic!("import job") };
        assert_eq!(job.options.page, Some(2));
        assert_eq!(job.target, crate::import_jobs::Target::Place { sid: id, rev: 0 });
        cancel.store(true, std::sync::atomic::Ordering::Release);
        let done = crate::import_jobs::execute(job);
        assert!(done.result.as_ref().unwrap_err().contains("cancelled"));
        let mut dialogs = crate::file_ports::RfdDialogs;
        assert!(!crate::import_jobs::complete(done, &mut ws, &mut dialogs));
        assert_eq!(
            FILE_RESULTS.with(|r| r.borrow().back().unwrap().1.error.as_ref().unwrap().code.clone()),
            "cancelled"
        );
        assert!(!FILE_PENDING.with(|r| r.borrow().contains(&ticket)));
    }
}
