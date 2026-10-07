//! Owning-thread adapter. Socket workers never see Workspace, Editor or egui.
use crate::{app_command::SessionId, host::DocUi, workspace::Workspace};
use std::cell::RefCell;
use varos_bridge::{BoardAccess, BoardInfo, Error, Host, Service};
use varos_core::editor::{AbDrag, Drag, ToolKind};
thread_local! { static SERVICE: RefCell<Option<Service>> = const { RefCell::new(None) }; }
pub fn initialize(epoch: String) {
    SERVICE.with(|s| *s.borrow_mut() = Some(Service::new(epoch)));
}
struct Desktop<'a> {
    ws: &'a mut Workspace,
    ui: Option<&'a mut dyn DocUi>,
    snapshot: Option<varos_bridge::service::SnapshotJob>,
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
    fn snapshot(
        &mut self,
        job: varos_bridge::service::SnapshotJob,
        _: &std::sync::atomic::AtomicBool,
    ) -> varos_bridge::Reply {
        self.snapshot = Some(job);
        varos_bridge::Reply::success(serde_json::json!({"pending":true}))
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
pub fn observe(ws: &mut Workspace) {
    SERVICE.with(|s| {
        if let Some(service) = s.borrow_mut().as_mut() {
            service.observe(&mut Desktop { ws, ui: None, snapshot: None });
        }
    });
}
pub fn run(request: varos_bridge::ipc::Pending, ws: &mut Workspace, ui: &mut dyn DocUi) -> crate::host::Ran {
    let mut desktop = Desktop { ws, ui: Some(ui), snapshot: None };
    let reply = SERVICE.with(|s| match s.borrow_mut().as_mut() {
        Some(service) => service.handle(&mut desktop, &request.context, request.request, &request.cancelled),
        None => varos_bridge::Reply::failure(Error::new("unsupported", "attachment listener unavailable")),
    });
    let changed = reply.ok && reply.request_id.is_some();
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
    crate::host::Ran { ran: changed, ..Default::default() }
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
                ed.execute(varos_core::EditCommand::SetBoardName("human field".into()));
                self.commit = false;
            }
            self.valid
        }
        fn document_switched(&mut self) {}
    }
    fn request(ws: &mut Workspace, fields: &mut Fields) -> varos_bridge::Reply {
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
    fn dispatch(ws: &mut Workspace, fields: &mut Fields, req: varos_bridge::Request) -> varos_bridge::Reply {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        run(
            varos_bridge::ipc::Pending {
                context: varos_bridge::Context {
                    client: "test".into(),
                    epoch: "epoch".into(),
                    read: true,
                    edit: true,
                    allow_history: false,
                    allow_destructive: false,
                },
                request: req,
                cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                reply: tx,
            },
            ws,
            fields,
        );
        rx.recv().unwrap()
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
                context: varos_bridge::Context {
                    client: "test".into(),
                    epoch: "epoch".into(),
                    read: true,
                    edit: true,
                    allow_history: false,
                    allow_destructive: false,
                },
                request: req,
                cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                reply: tx,
            },
            &mut ws,
            &mut fields,
        );
        assert!(!ran.ran);
        assert!(fields.commit);
        assert_eq!(ws.active().unwrap().editor.doc, before);
        assert!(!ws.active().unwrap().editor.history_available(false));
        ws.active_mut().unwrap().editor.execute(varos_core::EditCommand::SetBoardName("later human edit".into()));
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
