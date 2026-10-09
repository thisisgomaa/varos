use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Request, Service};
use varos_core::{
    bridge::TargetEdit,
    editor::{Editor, PaintTarget, ToolKind},
    model::{Anchor, Paint, Path},
    EditCommand,
};
struct FakeHost {
    editor: Editor,
    active: bool,
    busy: bool,
    exists: bool,
}
impl FakeHost {
    fn new() -> Self {
        let mut doc = varos_core::board::new_board();
        doc.name = "Logo".into();
        doc.ids = 24;
        for (id, x) in [(10, 20.0), (20, 140.0)] {
            let anchors = [(x, 20.), (x + 80., 20.), (x + 80., 120.), (x, 120.)]
                .into_iter()
                .enumerate()
                .map(|(i, (x, y))| Anchor { id: id + 1 + i as u32, p: [x, y], hin: None, hout: None, smooth: false })
                .collect();
            doc.paths.push(Path::new(id, anchors, true, Some([0.0, 0.5, 1.0, 1.0]), None, 1.0));
        }
        let mut editor = Editor::new();
        editor.replace_doc(doc);
        Self { editor, active: true, busy: false, exists: true }
    }
}
impl Host for FakeHost {
    fn boards(&self) -> Vec<BoardInfo> {
        if self.exists {
            vec![BoardInfo {
                board: "b1".into(),
                name: self.editor.doc.name.clone(),
                rev: self.editor.rev,
                dirty: true,
                active: self.active,
                backing_file: None,
            }]
        } else {
            vec![]
        }
    }
    fn prepare(&mut self, board: &str, mutation: bool) -> Result<(), Error> {
        if !self.exists || board != "b1" {
            return Err(Error::new("not_found", "closed"));
        }
        if mutation && !self.active {
            return Err(Error::new("board_not_active", "inactive"));
        }
        if self.busy || self.editor.transaction_open() {
            return Err(Error::new("busy", "gesture"));
        }
        Ok(())
    }
    fn access(&mut self, board: &str) -> Result<BoardAccess<'_>, Error> {
        if !self.exists || board != "b1" {
            return Err(Error::new("not_found", "closed"));
        }
        Ok(BoardAccess { editor: &mut self.editor, dirty: true })
    }
}
fn ctx() -> Context {
    Context { client: "fixture".into(), epoch: "test-epoch".into() }
}
fn req(tool: &str, arguments: Value) -> Request {
    varos_bridge::mcp::decode_tool(tool, arguments).unwrap()
}
fn handle(s: &mut Service, h: &mut FakeHost, r: Request) -> Reply {
    s.handle(h, &ctx(), r, &AtomicBool::new(false))
}
fn edit(id: &str, rev: u64) -> Request {
    req(
        "edit",
        json!({"api":"1.0","request_id":id,"board":"b1","expected_rev":rev,"ops":[{"verb":"move","ids":["path:10","path:20"],"delta":[10,0]},{"verb":"set_paint","ids":["path:10","path:20"],"fill":"#FF6600FF"}]}),
    )
}
#[test]
fn frozen_describe_and_edit_parity_and_one_undo() {
    let mut cli_host = FakeHost::new();
    let mut mcp_host = FakeHost::new();
    let mut cli_service = Service::new("test-epoch".into());
    let mut mcp_service = Service::new("test-epoch".into());
    for (tool, args, fixture) in [
        (
            "describe",
            json!({"board":"b1","rev":1,"ids":["path:10","path:20"],"fields":["bounds","paint","parent"]}),
            include_str!("fixtures/describe-1.0.json"),
        ),
        (
            "edit",
            json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"move","ids":["path:10","path:20"],"delta":[10,0]},{"verb":"set_paint","ids":["path:10","path:20"],"fill":"#FF6600FF"}]}),
            include_str!("fixtures/edit-1.0.json"),
        ),
    ] {
        let cli =
            varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":tool,"arguments":args})).unwrap()).unwrap();
        let a = handle(&mut cli_service, &mut cli_host, cli);
        let b = handle(&mut mcp_service, &mut mcp_host, req(tool, args));
        let mcp = varos_bridge::mcp::tool_result(&b);
        assert_eq!(json!(a), mcp["structuredContent"]);
        assert_eq!(varos_bridge::service::compact(&a), mcp["content"][0]["text"]);
        let frozen = serde_json::from_str::<Value>(fixture).unwrap();
        assert_eq!(serde_json::to_vec(&json!(a)).unwrap(), serde_json::to_vec(&frozen).unwrap());
    }
    assert_eq!(cli_host.editor.rev, 2);
    cli_host.editor.undo();
    assert_eq!(cli_host.editor.doc.paths[0].anchors[0].p, [20., 20.]);
    assert_eq!(cli_host.editor.doc.paths[0].fill, Paint::Solid([0., 0.5, 1., 1.]));
    assert!(!cli_host.editor.history_available(false));
    cli_host.editor.redo();
    assert_eq!(cli_host.editor.doc.paths[0].anchors[0].p, [30., 20.]);
}
#[test]
fn core_rolls_back_at_every_operation_index_including_history_and_defaults() {
    for index in 0..6 {
        let mut h = FakeHost::new();
        h.editor.objsel.insert(20);
        h.editor.selected.insert(21);
        h.editor.tool = ToolKind::Direct;
        h.editor.paint = PaintTarget::Stroke;
        h.editor.execute_ui(EditCommand::SetBoardName("human".into()));
        h.editor.undo();
        let before = h.editor.doc.clone();
        let rev = h.editor.rev;
        let fill = h.editor.cur_fill;
        let mut ops = vec![TargetEdit::Move { paths: vec![10], delta: [1., 2.] }; 6];
        ops[index] =
            TargetEdit::Paint { paths: vec![10], fill: None, stroke: None, stroke_width: None, opacity: Some(2.) };
        let error = h.editor.execute_targeted_batch(ops).unwrap_err();
        assert_eq!(error.index, index);
        assert_eq!(h.editor.doc, before);
        assert_eq!(h.editor.rev, rev);
        assert_eq!(h.editor.objsel, std::collections::HashSet::from([20]));
        assert_eq!(h.editor.selected, std::collections::HashSet::from([21]));
        assert_eq!(h.editor.cur_fill, fill);
        assert!(h.editor.history_available(true));
        assert!(!h.editor.history_available(false));
    }
}
#[test]
fn selection_defaults_preserved_and_select_is_deliberate() {
    let mut h = FakeHost::new();
    h.editor.objsel.insert(20);
    h.editor.tool = ToolKind::Direct;
    h.editor.paint = PaintTarget::Stroke;
    let fill = h.editor.cur_fill;
    let mut s = Service::new("test-epoch".into());
    assert!(handle(&mut s, &mut h, edit("r1", 1)).ok);
    assert_eq!(h.editor.objsel, std::collections::HashSet::from([20]));
    assert!(h.editor.tool == ToolKind::Direct);
    assert!(h.editor.paint == PaintTarget::Stroke);
    assert_eq!(h.editor.cur_fill, fill);
    assert!(
        handle(
            &mut s,
            &mut h,
            req("select", json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":2,"ids":["path:10"]}))
        )
        .ok
    );
    assert_eq!(h.editor.rev, 2);
    assert_eq!(h.editor.objsel, std::collections::HashSet::from([10]));
    assert!(h.editor.tool == ToolKind::Direct, "select must keep the human's tool");
}
#[test]
fn refusals_revisions_scope_and_epoch_do_not_mutate() {
    for code in ["revision_conflict", "busy", "board_not_active", "not_found"] {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        let mut c = ctx();
        let r = edit("r1", if code == "revision_conflict" { 0 } else { 1 });
        match code {
            "busy" => h.busy = true,
            "board_not_active" => h.active = false,
            "not_found" => c.epoch = "old launch".into(),
            _ => {}
        }
        let before = h.editor.doc.clone();
        let reply = s.handle(&mut h, &c, r, &AtomicBool::new(false));
        assert_eq!(reply.error.unwrap().code, code);
        assert_eq!(h.editor.doc, before);
        assert_eq!(h.editor.rev, 1);
    }
}
#[test]
fn local_service_does_not_gate_scopes() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let context = ctx();

    assert!(s.handle(&mut h, &context, edit("r1", 1), &AtomicBool::new(false)).ok);
    assert!(
        s.handle(
            &mut h,
            &context,
            req("history", json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":2,"action":"undo"})),
            &AtomicBool::new(false)
        )
        .ok
    );
}
#[test]
fn unknown_fields_and_unsupported_ops_are_indexed() {
    for bad in [
        json!({"verb":"pathfinder","ids":["path:10"]}),
        json!({"verb":"move","ids":["path:10"],"delta":[1,0],"snap":true}),
    ] {
        let error=varos_bridge::mcp::decode_tool("edit",json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"move","ids":["path:10"],"delta":[1,0]},bad]})).unwrap_err();
        assert_eq!(error.op_index, Some(1));
    }
    assert!(varos_bridge::mcp::decode_tool(
        "history",
        json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"action":"undo","confirm":true})
    )
    .is_err());
}
#[test]
fn idempotency_receipts_are_original_after_human_edits() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let r = edit("r1", 1);
    let first = handle(&mut s, &mut h, r.clone());
    h.editor.execute_ui(EditCommand::SetBoardName("human".into()));
    let again = handle(&mut s, &mut h, r);
    assert_eq!(first, again);
    assert_eq!(h.editor.rev, 3);
    assert_eq!(handle(&mut s, &mut h, edit("r1", 3)).error.unwrap().code, "invalid_argument");
    let status = handle(&mut s, &mut h, req("request_status", json!({"request_id":"r1"})));
    assert_eq!(status.result.unwrap()["receipt"], json!(first));
}
#[test]
fn history_without_grant_has_idempotent_receipt() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let before = h.editor.doc.clone();
    assert!(handle(&mut s, &mut h, edit("r1", 1)).ok);
    let undo = req("history", json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":2,"action":"undo"}));
    assert!(handle(&mut s, &mut h, undo.clone()).ok);
    assert!(h.editor.doc.content_eq(&before));
    assert!(handle(&mut s, &mut h, undo).ok);
    assert_eq!(h.editor.rev, 3);
}
#[test]
fn cancellation_at_staging_checkpoint_is_atomic() {
    let mut h = FakeHost::new();
    let before = h.editor.doc.clone();
    let calls = std::cell::Cell::new(0);
    let r = h.editor.execute_targeted_batch_cancellable(
        vec![TargetEdit::Move { paths: vec![10], delta: [10., 0.] }; 3],
        || {
            let n = calls.get();
            calls.set(n + 1);
            n >= 2
        },
    );
    assert!(r.is_err());
    assert_eq!(h.editor.doc, before);
    assert_eq!(h.editor.rev, 1);
    let mut s = Service::new("test-epoch".into());
    let flag = AtomicBool::new(true);
    let r = s.handle(&mut h, &ctx(), edit("r1", 1), &flag);
    assert_eq!(r.error.unwrap().code, "cancelled");
    flag.store(false, Ordering::Release);
}
#[test]
fn cursors_expire_and_journal_tracks_human_undo_redo() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let args = json!({"board":"b1","fields":["bounds"],"limit":1});
    let first = handle(&mut s, &mut h, req("describe", args.clone()));
    let cursor = first.result.unwrap()["cursor"].clone();
    assert!(cursor.is_string());
    assert!(handle(&mut s, &mut h, edit("r1", 1)).ok);
    h.editor.undo();
    s.observe(&mut h);
    h.editor.redo();
    s.observe(&mut h);
    let diff = handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":2})));
    assert!(diff.ok);
    assert_eq!(diff.result.unwrap()["rev"], 4);
    let mut expired = args;
    expired["cursor"] = cursor;
    assert_eq!(handle(&mut s, &mut h, req("describe", expired)).error.unwrap().code, "resync_required");
    for i in 0..130 {
        h.editor.execute_ui(EditCommand::SetBoardName(format!("human-{i}")));
        s.observe(&mut h);
    }
    assert_eq!(
        handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":1}))).error.unwrap().code,
        "resync_required"
    );
}
#[test]
fn null_paint_and_noop_are_distinct_from_missing() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let r = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"move","ids":["path:10"],"delta":[0,0]}]}),
        ),
    );
    assert!(r.ok);
    assert_eq!(r.undo_steps, 0);
    assert_eq!(h.editor.rev, 1);
    let r = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":1,"ops":[{"verb":"set_paint","ids":["path:10"],"fill":null}]}),
        ),
    );
    assert!(r.ok);
    assert_eq!(h.editor.doc.paths[0].fill, Paint::None);
    assert_eq!(h.editor.doc.paths[1].fill, Paint::Solid([0., 0.5, 1., 1.]));
}
#[test]
fn allocator_does_not_reuse_ids_after_undo_branch() {
    let mut h = FakeHost::new();
    h.editor.objsel.insert(10);
    h.editor.execute_ui(EditCommand::Copy);
    h.editor.execute_ui(EditCommand::Paste { offset: Some([1., 0.]) });
    let used = h.editor.doc.ids;
    h.editor.undo();
    h.editor.execute_ui(EditCommand::Paste { offset: Some([2., 0.]) });
    assert!(h.editor.doc.ids > used);
    assert!(h.editor.doc.paths.last().unwrap().id > used);
}
#[test]
fn transformed_partial_group_move_leaves_other_world_art_unchanged() {
    let mut h = FakeHost::new();
    h.editor.objsel.extend([10, 20]);
    h.editor.execute_ui(EditCommand::GroupSelection);
    h.editor.execute_ui(EditCommand::SetObjectRotation(45.));
    let other = h.editor.doc.outline_bbox(h.editor.doc.pidx(20).unwrap());
    let before = h.editor.doc.outline_bbox(h.editor.doc.pidx(10).unwrap());
    h.editor.execute_targeted_batch(vec![TargetEdit::Move { paths: vec![10], delta: [10., 0.] }]).unwrap();
    let after = h.editor.doc.outline_bbox(h.editor.doc.pidx(10).unwrap());
    assert!((after.0 - before.0 - 10.).abs() < 0.001);
    assert_eq!(h.editor.doc.outline_bbox(h.editor.doc.pidx(20).unwrap()), other);
}

#[cfg(unix)]
struct SocketHost {
    listener: Option<varos_bridge::ipc::Listener>,
    stop: std::sync::Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    endpoint: varos_bridge::ipc::Endpoint,
}
#[cfg(unix)]
impl SocketHost {
    fn start(delay: bool) -> Self {
        Self::start_with(delay, None)
    }
    /// Legacy endpoint plus, optionally, the ADR-0011 paired listener on the same epoch.
    fn start_with(delay: bool, paired: Option<varos_bridge::ipc::PairedConfig>) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<varos_bridge::ipc::Pending>(32);
        let listener = varos_bridge::ipc::Listener::start_with(
            tx,
            || {},
            varos_bridge::ipc::Options {
                legacy: true,
                legacy_audit: paired.as_ref().map(|p| p.paths.clone()),
                paired: paired.map(varos_bridge::ipc::PairedSource::Ready),
            },
        )
        .unwrap();
        // With the legacy listener on, endpoint_file is the legacy (token) endpoint.
        let endpoint: varos_bridge::ipc::Endpoint =
            serde_json::from_slice(&std::fs::read(&listener.endpoint_file).unwrap()).unwrap();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let epoch = endpoint.epoch.clone();
        let thread = std::thread::spawn(move || {
            let mut host = FakeHost::new();
            let mut service = Service::new(epoch);
            while !flag.load(Ordering::Acquire) {
                if let Ok(p) = rx.recv_timeout(std::time::Duration::from_millis(20)) {
                    if delay && p.request.mutation().is_some() {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    // Peer uid and identity were checked before queuing.
                    let reply = service.handle(&mut host, &p.context, p.request, &p.cancelled);
                    let _ = p.reply.send(reply);
                }
            }
        });
        Self { listener: Some(listener), stop, thread: Some(thread), endpoint }
    }
    fn client(&self) -> varos_bridge::ipc::Client {
        varos_bridge::ipc::Client::new(self.endpoint.socket.clone(), self.endpoint.token.clone()).unwrap()
    }
}
#[cfg(unix)]
impl Drop for SocketHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.thread.take().unwrap().join();
        drop(self.listener.take());
    }
}
#[cfg(unix)]
#[ignore = "requires native Unix socket bind; sandbox denies bind with EPERM"]
#[test]
fn local_socket_authentication_permissions_and_revocation() {
    use std::os::unix::fs::PermissionsExt;
    let host = SocketHost::start(false);
    let endpoint_path = host.listener.as_ref().unwrap().endpoint_file.clone();
    assert_eq!(std::fs::metadata(&endpoint_path).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::metadata(endpoint_path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    let wrong = varos_bridge::ipc::Client::new(host.endpoint.socket.clone(), "wrong".into()).unwrap();
    assert_eq!(wrong.call("bad", req("capabilities", json!({}))).error.unwrap().code, "scope_refused");
    assert!(varos_bridge::ipc::authorize_uid(123, 456).is_err());
    assert!(varos_bridge::ipc::authorize_uid(123, 123).is_ok());
    let client = host.client();
    assert!(client.call("good", req("capabilities", json!({"api":"1.0"}))).ok);
    // A peer stuck before Hello must not make desktop Quit wait for the 5-second read timeout.
    let mut idle = std::os::unix::net::UnixStream::connect(&host.endpoint.socket).unwrap();
    std::io::Write::write_all(&mut idle, b"{partial hello").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(150));
    let started = std::time::Instant::now();
    drop(host);
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    use std::io::Read;
    // Setting a timeout on a socket the host already shut down fails with EINVAL on macOS; that is fine.
    let _ = idle.set_read_timeout(Some(std::time::Duration::from_secs(1)));
    // After the host shuts the socket down, macOS may report EOF (0) or a reset instead of a clean close.
    match idle.read(&mut [0u8; 1]) {
        Ok(0) => {}
        Ok(n) => panic!("stuck peer received {n} bytes after host stop"),
        Err(e) => assert!(
            matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::BrokenPipe
            ),
            "unexpected error after host stop: {e:?}"
        ),
    }
    assert!(!endpoint_path.exists());
    assert_eq!(client.call("stale", req("list_boards", json!({}))).error.unwrap().code, "io_error");
}
#[cfg(unix)]
#[ignore = "requires native Unix socket bind; sandbox denies bind with EPERM"]
#[test]
fn real_binary_mcp_initialization_call_cancel_and_shared_cli() {
    use std::{
        io::{BufRead, Write},
        process::{Command, Stdio},
    };
    let host = SocketHost::start(true);
    let mut child = Command::new(env!("CARGO_BIN_EXE_varos-bridge"))
        .args(["mcp", "--attach"])
        .arg(&host.endpoint.socket)
        .arg("--token")
        .arg(&host.endpoint.token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let send = |w: &mut std::process::ChildStdin, v: Value| {
        writeln!(w, "{v}").unwrap();
        w.flush().unwrap();
    };
    let read = |r: &mut std::io::BufReader<std::process::ChildStdout>| {
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };
    send(
        &mut stdin,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}}),
    );
    let init = read(&mut stdout);
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    send(&mut stdin, json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    send(&mut stdin, json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    assert_eq!(read(&mut stdout)["result"]["tools"].as_array().unwrap().len(), varos_bridge::TOOLS.len());
    let args = json!({"board":"b1","ids":["path:10","path:20"],"fields":["bounds","paint","parent"]});
    send(
        &mut stdin,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"describe","arguments":args}}),
    );
    let mcp = read(&mut stdout);
    let mut cli = Command::new(env!("CARGO_BIN_EXE_varos-bridge"))
        .args(["describe", "--fields", "bounds,paint,parent", "--json", "--attach"])
        .arg(&host.endpoint.socket)
        .arg("--token")
        .arg(&host.endpoint.token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut cli_args = args.clone();
    cli_args.as_object_mut().unwrap().remove("fields");
    writeln!(cli.stdin.take().unwrap(), "{cli_args}").unwrap();
    let cli = cli.wait_with_output().unwrap();
    assert!(cli.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&cli.stdout).unwrap(), mcp["result"]["structuredContent"]);
    let edit_args = serde_json::to_value(edit("r1", 1)).unwrap()["arguments"].clone();
    send(
        &mut stdin,
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"edit","arguments":edit_args}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":4,"reason":"fixture cancellation"}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"describe","arguments":{"board":"b1"}}}),
    );
    assert_eq!(read(&mut stdout)["result"]["structuredContent"]["rev"], 1);
    drop(stdin);
    assert!(child.wait().unwrap().success());
}
#[cfg(unix)]
struct TempHome(std::path::PathBuf);
#[cfg(unix)]
impl TempHome {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vb{}", varos_bridge::conn::random_hex(3).unwrap()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(dir)
    }
}
#[cfg(unix)]
impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[cfg(unix)]
#[ignore = "requires native Unix socket bind; sandbox denies bind with EPERM"]
#[test]
fn same_uid_socket_all_scopes_no_pairing_audit_and_registry_lifecycle() {
    use std::os::unix::fs::PermissionsExt;
    use varos_bridge::conn::{
        self,
        attach::{AutoClient, Selector},
        credentials::{self, FileKeyStore},
    };
    let home = TempHome::new();
    let paths = conn::Paths::under(&home.0);
    let host_key =
        credentials::load_or_create(&FileKeyStore::new(paths.state.join("keys")), credentials::HOST_ACCOUNT).unwrap();
    let config = |key: &ed25519_dalek::SigningKey| varos_bridge::ipc::PairedConfig {
        paths: paths.clone(),
        host_key: key.clone(),
        mode: "desktop",
        app_build: "varos-app test".into(),
    };
    let host = SocketHost::start_with(true, Some(config(&host_key)));
    let registry = host.listener.as_ref().unwrap().registry_file().unwrap();
    assert_eq!(host.listener.as_ref().unwrap().paired_status(), varos_bridge::ipc::PairedStatus::Ready);
    assert_eq!(std::fs::metadata(&registry).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::metadata(registry.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    let record: conn::registry::Record = serde_json::from_slice(&std::fs::read(&registry).unwrap()).unwrap();
    assert_eq!(record.epoch, host.endpoint.epoch, "both listeners share one epoch/service");
    assert!(!std::fs::read_to_string(&registry).unwrap().contains(&host.endpoint.token));

    let store: std::sync::Arc<dyn credentials::CredentialStore> =
        std::sync::Arc::new(FileKeyStore::new(paths.state.join("keys")));
    let agent = std::sync::Arc::new(
        AutoClient::with(Ok(paths.clone()), store.clone(), Selector::Auto, None, "claude-code").unwrap(),
    );
    assert!(agent.call("c1", req("list_boards", json!({}))).ok);
    assert!(!paths.runtime.join("pairing").exists());
    assert!(!paths.state.join("trust.json").exists());

    // The legacy token Hello is never accepted on the paired socket...
    {
        use std::io::{BufRead, Write};
        let mut raw = std::os::unix::net::UnixStream::connect(&record.socket).unwrap();
        writeln!(raw, "{}", json!({"kind":"hello","api":"1.0","token":host.endpoint.token,"client":"a".repeat(64)}))
            .unwrap();
        let mut line = String::new();
        std::io::BufReader::new(raw).read_line(&mut line).unwrap();
        let reply: Reply = serde_json::from_str(&line).unwrap();
        assert_eq!(reply.error.unwrap().code, "unsupported");
    }
    // ...while the deprecated legacy socket keeps working for this transition slice.
    assert!(host.client().call("legacy", req("capabilities", json!({"api":"1.0"}))).ok);

    let caps = agent.call("c2", req("capabilities", json!({"api":"1.0"})));
    assert!(caps.ok, "{caps:?}");
    let caps = caps.result.unwrap();
    assert_eq!(caps["edit"], true);
    assert_eq!(caps["destructive_scope"], true);
    assert_eq!(caps["history_scope"], true);
    assert_eq!(caps["files_scope"], true);
    assert_eq!(caps["read"], true);
    assert_eq!(caps["trust"], "local user");
    let profile = caps["client"].as_str().unwrap().split(':').next().unwrap().to_owned();
    assert!(agent.call("c3", req("describe", json!({"board":"b1"}))).ok);
    assert!(agent.call("c4", edit("r1", 1)).ok);
    let delete = json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":2,"ops":[{"verb":"delete","ids":["path:20"]}]});
    assert!(agent.call("c5", req("edit", delete)).ok);
    assert!(
        agent
            .call(
                "c6",
                req("history", json!({"api":"1.0","request_id":"r3","board":"b1","expected_rev":3,"action":"undo"}))
            )
            .ok
    );

    // Audit: verbs/boards/revisions/results only — no payload, paint or names.
    let audit = conn::audit::tail(&paths, 50).join("\n");
    assert!(audit.contains("\"verb\":\"edit\"") && audit.contains("\"result\":\"ok\""), "{audit}");
    assert!(audit.contains(&format!("\"agent\":\"{profile}\"")));
    assert!(audit.contains("\"label\":\"claude-code\""));
    assert!(!audit.contains("pairing_requested"));
    assert!(audit.contains("\"agent\":\"legacy\""), "opt-in legacy calls are audited as agent legacy");
    assert!(!audit.contains("FF6600") && !audit.contains("Logo") && !audit.contains(&host.endpoint.token));

    // A second launch makes an unpinned agent's choice ambiguous.
    let second = SocketHost::start_with(false, Some(config(&host_key)));
    let fresh = AutoClient::with(Ok(paths.clone()), store, Selector::Auto, None, "other-agent").unwrap();
    let e = fresh.call("c9", req("capabilities", json!({"api":"1.0"}))).error.unwrap();
    assert_eq!(e.code, "ambiguous_target");
    assert_eq!(e.candidates.len(), 2);
    drop(second);
    drop(host);
    assert!(!registry.exists(), "the host removes its own registry entry on exit");
    assert!(conn::registry::scan(&paths).live.is_empty());
}
#[cfg(unix)]
#[ignore = "requires native Unix socket bind; sandbox denies bind with EPERM"]
#[test]
fn local_host_key_change_resets_session_without_pairing() {
    use varos_bridge::conn::{
        self,
        attach::{AutoClient, Selector},
        credentials::{self, FileKeyStore},
    };
    use varos_bridge::ipc::{Listener, Options, PairedSource, PairedStatus};
    let home = TempHome::new();
    let paths = conn::Paths::under(&home.0);
    let host_store = FileKeyStore::new(paths.state.join("keys"));
    let config = |key: ed25519_dalek::SigningKey| varos_bridge::ipc::PairedConfig {
        paths: paths.clone(),
        host_key: key,
        mode: "desktop",
        app_build: "varos-app test".into(),
    };
    let first_key = credentials::load_or_create(&host_store, credentials::HOST_ACCOUNT).unwrap();
    let host = SocketHost::start_with(false, Some(config(first_key)));
    let store: std::sync::Arc<dyn credentials::CredentialStore> =
        std::sync::Arc::new(FileKeyStore::new(paths.state.join("keys")));
    let agent = AutoClient::with(Ok(paths.clone()), store, Selector::Auto, None, "claude-code").unwrap();
    let caps = || req("capabilities", json!({"api":"1.0"}));
    assert!(agent.call("c2", caps()).ok);
    drop(host);
    // The host identity is replaced (e.g. the host key file was replaced) and Varos relaunches;
    // this launch loads its key on a background thread like the desktop.
    credentials::CredentialStore::delete(&host_store, credentials::HOST_ACCOUNT).unwrap();
    let second_key = credentials::load_or_create(&host_store, credentials::HOST_ACCOUNT).unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel::<varos_bridge::ipc::Pending>(32);
    let deferred = config(second_key);
    let listener = Listener::start_with(
        tx,
        || {},
        Options {
            legacy: false,
            paired: Some(PairedSource::Deferred(Box::new(move || Ok(deferred)))),
            legacy_audit: None,
        },
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while listener.paired_status() != PairedStatus::Ready && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(listener.paired_status(), PairedStatus::Ready);
    assert!(listener.registry_file().unwrap().exists());
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let epoch = listener.epoch().unwrap();
    let worker = std::thread::spawn(move || {
        let mut host = FakeHost::new();
        let mut service = Service::new(epoch);
        while !flag.load(Ordering::Acquire) {
            if let Ok(p) = rx.recv_timeout(std::time::Duration::from_millis(20)) {
                let reply = service.handle(&mut host, &p.context, p.request, &p.cancelled);
                let _ = p.reply.send(reply);
            }
        }
    });
    // Relaunch → session_reset once; the registry-matching changed key remains locally trusted.
    assert_eq!(agent.call("c3", caps()).error.unwrap().code, "session_reset");
    assert!(agent.call("c4", caps()).ok);
    assert!(agent.call("c5", caps()).ok);
    stop.store(true, Ordering::Release);
    worker.join().unwrap();
    let registry = listener.registry_file().unwrap();
    drop(listener);
    assert!(!registry.exists());
}
#[test]
fn framing_rejects_oversize_unterminated_and_schema_version() {
    assert!(varos_bridge::ipc::read_frame(&mut std::io::Cursor::new(vec![b'x'; varos_bridge::MAX_FRAME + 1])).is_err());
    assert!(varos_bridge::ipc::read_frame(&mut std::io::Cursor::new(b"{}".as_slice())).is_err());
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let r = handle(&mut s, &mut h, req("capabilities", json!({"api":"2.0"})));
    assert_eq!(r.error.unwrap().code, "unsupported");
}

#[derive(Clone)]
struct FakeTransport {
    state: std::sync::Arc<std::sync::Mutex<(Service, FakeHost)>>,
    cancellations: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<AtomicBool>>>>,
}
impl varos_bridge::mcp::Transport for FakeTransport {
    fn call(&self, id: &str, request: Request) -> Reply {
        let flag = self
            .cancellations
            .lock()
            .unwrap()
            .entry(id.into())
            .or_insert_with(|| std::sync::Arc::new(AtomicBool::new(false)))
            .clone();
        if request.mutation().is_some() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let mut state = self.state.lock().unwrap();
        let (service, host) = &mut *state;
        service.handle(host, &ctx(), request, &flag)
    }
    fn cancel(&self, id: &str) {
        self.cancellations
            .lock()
            .unwrap()
            .entry(id.into())
            .or_insert_with(|| std::sync::Arc::new(AtomicBool::new(false)))
            .store(true, Ordering::Release);
    }
}
struct ChannelRead {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    current: std::io::Cursor<Vec<u8>>,
}
impl std::io::Read for ChannelRead {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.current.read(bytes)?;
            if n > 0 {
                return Ok(n);
            }
            match self.rx.recv() {
                Ok(chunk) => self.current = std::io::Cursor::new(chunk),
                Err(_) => return Ok(0),
            }
        }
    }
}
struct ChannelWrite {
    tx: std::sync::mpsc::Sender<Vec<u8>>,
    bytes: Vec<u8>,
}
impl std::io::Write for ChannelWrite {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.tx.send(std::mem::take(&mut self.bytes)).map_err(std::io::Error::other)
    }
}
#[test]
fn mcp_fake_host_exercises_actual_binding_parity_errors_and_cancellation() {
    let transport = FakeTransport {
        state: std::sync::Arc::new(std::sync::Mutex::new((Service::new("test-epoch".into()), FakeHost::new()))),
        cancellations: Default::default(),
    };
    let state = transport.state.clone();
    let (input_tx, input) = std::sync::mpsc::channel();
    let (output, receive) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        varos_bridge::mcp::serve(
            &mut std::io::BufReader::new(ChannelRead { rx: input, current: std::io::Cursor::new(vec![]) }),
            ChannelWrite { tx: output, bytes: vec![] },
            transport,
        )
        .unwrap()
    });
    let send = |v: Value| {
        let mut data = serde_json::to_vec(&v).unwrap();
        data.push(b'\n');
        input_tx.send(data).unwrap();
    };
    let read =
        || serde_json::from_slice::<Value>(&receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap()).unwrap();
    send(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}));
    assert_eq!(read()["error"]["code"], -32002);
    send(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"fake","version":"1"}}}),
    );
    assert_eq!(read()["result"]["protocolVersion"], "2025-06-18");
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    assert_eq!(read()["result"]["tools"].as_array().unwrap().len(), varos_bridge::TOOLS.len());
    send(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"describe","arguments":{"board":"b1","ids":["path:10","path:20"],"fields":["bounds","paint","parent"]}}}),
    );
    assert_eq!(
        read()["result"]["structuredContent"],
        serde_json::from_str::<Value>(include_str!("fixtures/describe-1.0.json")).unwrap()
    );
    let args = serde_json::to_value(edit("r1", 1)).unwrap()["arguments"].clone();
    send(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"edit","arguments":args}}));
    assert_eq!(
        read()["result"]["structuredContent"],
        serde_json::from_str::<Value>(include_str!("fixtures/edit-1.0.json")).unwrap()
    );
    let args = serde_json::to_value(edit("r2", 2)).unwrap()["arguments"].clone();
    send(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"edit","arguments":args}}));
    send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":5}}));
    send(json!({"jsonrpc":"2.0","id":6,"method":"unknown"}));
    let unknown = read();
    assert_eq!(unknown["id"], 6, "cancelled request must not emit a response");
    assert_eq!(unknown["error"]["code"], -32601);
    send(json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"unknown"}}));
    assert_eq!(read()["error"]["code"], -32602);
    send(
        json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"edit","arguments":{"api":"1.0","request_id":"r3","board":"b1","expected_rev":2,"ops":[{"verb":"delete"}]}}}),
    );
    assert_eq!(read()["result"]["structuredContent"]["error"]["op_index"], 0);
    drop(input_tx);
    server.join().unwrap();
    let (service, host) = &mut *state.lock().unwrap();
    let _ = service;
    assert_eq!(host.editor.rev, 2);
    host.editor.undo();
    assert_eq!(host.editor.doc.paths[0].anchors[0].p, [20., 20.]);
    assert!(!host.editor.history_available(false));
}
#[test]
fn auth_refusal_fake_host_cannot_reach_service_or_change_revision() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    for (peer, owner, provided) in [(2, 1, "secret"), (1, 1, "wrong")] {
        let result = varos_bridge::ipc::authorize_uid(peer, owner)
            .and_then(|()| varos_bridge::ipc::authorize_token(provided, "secret"));
        let reply = match result {
            Ok(()) => handle(&mut s, &mut h, edit("r1", 1)),
            Err(_) => Reply::failure(Error::new("scope_refused", "authentication refused before host queue")),
        };
        assert_eq!(reply.error.unwrap().code, "scope_refused");
        assert_eq!(h.editor.rev, 1);
        assert!(!h.editor.history_available(false));
    }
}
#[test]
fn real_binary_stdio_initialization_has_protocol_only_stdout() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_varos-bridge"))
        .args(["mcp", "--attach", "/tmp/varos-nonexistent-test.sock", "--token", "explicit-test"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"fixture","version":"1"},"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    ] {
        writeln!(input, "{message}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines: Vec<Value> =
        String::from_utf8(output.stdout).unwrap().lines().map(|s| serde_json::from_str(s).unwrap()).collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(lines[1]["result"]["tools"].as_array().unwrap().len(), varos_bridge::TOOLS.len());
    assert!(output.stderr.is_empty());
}
#[test]
fn frozen_revision_error_parity() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let reply = handle(&mut s, &mut h, edit("r1", 0));
    let fixture = serde_json::from_str::<Value>(include_str!("fixtures/error-1.0.json")).unwrap();
    assert_eq!(json!(reply), fixture);
    assert_eq!(varos_bridge::mcp::tool_result(&reply)["structuredContent"], fixture);
    assert!(!reply.ok);
}
#[test]
fn metadata_and_artboard_queries_are_explicit_and_preferences_advance_revision() {
    let mut h = FakeHost::new();
    h.editor.doc.description = "private description".into();
    h.editor.doc.artboards.push(Default::default());
    let mut s = Service::new("test-epoch".into());
    let summary = handle(&mut s, &mut h, req("describe", json!({"board":"b1"})));
    assert!(summary.result.as_ref().unwrap().get("metadata").is_none());
    assert_eq!(summary.result.unwrap()["artboards"][0]["ref"], "a0@1");
    let detail = handle(&mut s, &mut h, req("describe", json!({"board":"b1","fields":["metadata","artboards"]})));
    assert_eq!(detail.result.unwrap()["metadata"]["description"], "private description");
    h.editor.execute_ui(EditCommand::ToggleSnapping);
    assert_eq!(h.editor.rev, 1);
    s.observe(&mut h);
    assert_eq!(h.editor.rev, 2);
    let diff = handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":1})));
    assert!(diff.result.unwrap()["board_changes"]["metadata"]["settings_digest"].is_string());
}
#[test]
fn net_diff_preserves_multiple_fields_and_cancelling_human_changes() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    s.observe(&mut h);
    h.editor.objsel.insert(10);
    h.editor.execute_ui(EditCommand::SetOpacity(0.5));
    s.observe(&mut h);
    h.editor.execute_ui(EditCommand::ApplyPaint { target: PaintTarget::Fill, color: Some([1., 0., 0., 1.]) });
    s.observe(&mut h);
    let r = handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":1})));
    let changes = r.result.unwrap()["changed"].as_array().unwrap().clone();
    let path = changes.iter().find(|c| c["id"] == "path:10").unwrap();
    assert_eq!(path["fill"], "#FF0000FF");
    assert_eq!(path["opacity"], 0.5);
    h.editor.undo();
    h.editor.undo();
    s.observe(&mut h);
    let r = handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":1})));
    assert_eq!(r.result.unwrap()["changed"], json!([]));
}
#[test]
fn frozen_compact_projection_and_open_history_receipt() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let r = handle(
        &mut s,
        &mut h,
        req("describe", json!({"board":"b1","ids":["path:10","path:20"],"fields":["bounds","paint","parent"]})),
    );
    assert_eq!(varos_bridge::service::compact(&r), include_str!("fixtures/describe-1.0.txt"));
    assert!(handle(&mut s, &mut h, edit("r1", 1)).ok);
    let r = handle(
        &mut s,
        &mut h,
        req("history", json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":2,"action":"undo"})),
    );
    let text = varos_bridge::service::compact(&r);
    assert!(r.ok, "{text}");
    assert_eq!(r.rev, Some(3));
    let bad = handle(&mut s, &mut h, req("describe", json!({"board":"b1\ninstructions"})));
    assert!(!varos_bridge::service::compact(&bad).contains('\n'));
}

#[test]
fn precommit_failures_leave_request_id_available_with_fresh_revision() {
    for code in ["busy", "revision_conflict", "cancelled"] {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        h.busy = code == "busy";
        let cancelled = AtomicBool::new(code == "cancelled");
        let rev = if code == "revision_conflict" { 0 } else { 1 };
        let failed = s.handle(&mut h, &ctx(), edit("r1", rev), &cancelled);
        assert_eq!(failed.error.unwrap().code, code);
        assert_eq!(
            handle(&mut s, &mut h, req("request_status", json!({"request_id":"r1"}))).error.unwrap().code,
            "not_found"
        );
        h.busy = false;
        h.editor.execute_ui(EditCommand::SetBoardName("human advanced revision".into()));
        let fresh = h.editor.rev;
        assert!(handle(&mut s, &mut h, edit("r1", fresh)).ok);
        assert_eq!(h.editor.rev, fresh + 1);
    }
}

#[test]
fn final_validation_failure_replays_only_to_attribute_first_invalid_op() {
    let mut h = FakeHost::new();
    let before = h.editor.doc.clone();
    let rev = h.editor.rev;
    let error = h
        .editor
        .execute_targeted_batch(vec![
            TargetEdit::Move { paths: vec![10], delta: [1., 0.] },
            TargetEdit::Move { paths: vec![10], delta: [f32::MAX, 0.] },
            TargetEdit::Move { paths: vec![10], delta: [f32::MAX, 0.] },
        ])
        .unwrap_err();
    assert_eq!(error.index, 2);
    assert_eq!(h.editor.doc, before);
    assert_eq!(h.editor.rev, rev);
    assert!(!h.editor.history_available(false));
}
#[test]
fn legacy_batch_preserves_live_tool_and_clipboard() {
    let mut h = FakeHost::new();
    h.editor.objsel.insert(20);
    h.editor.execute_ui(EditCommand::Copy);
    let clipboard = h.editor.clipboard().bounds();
    h.editor.tool = ToolKind::Direct;
    h.editor.execute_batch(vec![EditCommand::SelectPaths(vec![10]), EditCommand::Copy]).unwrap();
    assert!(h.editor.tool == ToolKind::Direct);
    assert_eq!(h.editor.clipboard().bounds(), clipboard);
    assert_eq!(h.editor.objsel, std::collections::HashSet::from([10]));
}

#[test]
fn mcp_eof_wait_is_bounded_even_when_transport_does_not_finish() {
    struct Held {
        started: std::sync::mpsc::SyncSender<()>,
        release: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    }
    impl varos_bridge::mcp::Transport for Held {
        fn call(&self, _: &str, _: Request) -> Reply {
            self.started.send(()).unwrap();
            let (lock, wake) = &*self.release;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
            Reply::success(json!({}))
        }
        fn cancel(&self, _: &str) {}
    }
    let (input_tx, input) = std::sync::mpsc::channel();
    let (output, receive) = std::sync::mpsc::channel();
    let (started_tx, started) = std::sync::mpsc::sync_channel(1);
    let release = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let held = Held { started: started_tx, release: release.clone() };
    let server = std::thread::spawn(move || {
        varos_bridge::mcp::serve(
            &mut std::io::BufReader::new(ChannelRead { rx: input, current: std::io::Cursor::new(vec![]) }),
            ChannelWrite { tx: output, bytes: vec![] },
            held,
        )
        .unwrap()
    });
    for msg in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{},"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"describe","arguments":{"board":"b1"}}}),
    ] {
        let mut bytes = serde_json::to_vec(&msg).unwrap();
        bytes.push(b'\n');
        input_tx.send(bytes).unwrap();
    }
    receive.recv_timeout(std::time::Duration::from_secs(2)).unwrap(); // initialize
    started.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    let deadline = std::time::Instant::now();
    drop(input_tx);
    server.join().unwrap();
    assert!(deadline.elapsed() < std::time::Duration::from_secs(2));
    *release.0.lock().unwrap() = true;
    release.1.notify_all();
    assert!(receive.recv_timeout(std::time::Duration::from_secs(1)).is_err(), "cancelled call must emit no response");
}

fn design_request(ops: Vec<Value>, rev: u64) -> Request {
    req("edit", json!({"api":"1.0","board":"b1","request_id":"r1","expected_rev":rev,"ops":ops}))
}
fn shape(local: &str, kind: &str, bounds: [f32; 4]) -> Value {
    json!({"verb":"add_shape","kind":kind,"bounds":bounds,"fill":"#FF6600FF","stroke":"#101010FF","stroke_width":2,"opacity":0.8,"local":local})
}
fn poster_request() -> Request {
    req("edit", serde_json::from_str(include_str!("fixtures/poster-request-1.0.json")).unwrap())
}
fn empty_host() -> FakeHost {
    let mut host = FakeHost::new();
    host.editor = Editor::new();
    host.editor.replace_doc(varos_core::board::new_board());
    host
}
#[test]
fn poster_fixture_cli_mcp_parity_local_names_atomic_history_and_png() {
    use base64::Engine;
    let arguments: Value = serde_json::from_str(include_str!("fixtures/poster-request-1.0.json")).unwrap();
    let cli =
        varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":"edit","arguments":arguments})).unwrap()).unwrap();
    let mut h = empty_host();
    let mut m = empty_host();
    h.editor.tool = ToolKind::Direct;
    m.editor.tool = ToolKind::Direct;
    h.editor.cur_fill = Some([0.2, 0.3, 0.4, 1.0]);
    m.editor.cur_fill = h.editor.cur_fill;
    let before = h.editor.doc.clone();
    let mut s = Service::new("test-epoch".into());
    let mut ms = Service::new("test-epoch".into());
    let r = handle(&mut s, &mut h, cli);
    let mr = handle(&mut ms, &mut m, poster_request());
    assert!(r.ok, "{r:?}");
    assert_eq!(json!(r), serde_json::from_str::<Value>(include_str!("fixtures/poster-result-1.0.json")).unwrap());
    assert_eq!(r, mr);
    assert_eq!(r.undo_steps, 1);
    assert_eq!(varos_bridge::mcp::tool_result(&r)["structuredContent"], json!(r));
    assert!(h.editor.tool == ToolKind::Direct);
    assert!(h.editor.objsel.is_empty());
    assert_eq!(h.editor.cur_fill, Some([0.2, 0.3, 0.4, 1.0]));
    assert_eq!(h.editor.doc.paths.len(), 5);
    let locals = &r.result.as_ref().unwrap()["locals"];
    assert_eq!(locals.as_object().unwrap().len(), 6);
    let b = locals["$b"].as_str().unwrap().strip_prefix("path:").unwrap().parse::<u32>().unwrap();
    let bbox = h.editor.doc.outline_bbox(h.editor.doc.pidx(b).unwrap());
    assert!((bbox.0 - 150.0).abs() < 0.01, "{bbox:?}");
    let group = locals["$poster"].as_str().unwrap().strip_prefix("node:").unwrap().parse::<u32>().unwrap();
    assert_eq!(h.editor.doc.node(group).unwrap().name, "Poster");
    assert_eq!(h.editor.doc.node_paths(group).len(), 5);
    let detail = handle(
        &mut s,
        &mut h,
        req("describe", json!({"board":"b1","rev":2,"ids":[locals["$a"]],"fields":["geometry"]})),
    );
    assert_eq!(detail.result.unwrap()["objects"][0]["geometry"]["anchors"].as_array().unwrap().len(), 4);
    let snapshot = handle(&mut s, &mut h, req("snapshot", json!({"board":"b1","rev":2})));
    assert!(snapshot.ok, "{snapshot:?}");
    assert_eq!(snapshot.undo_steps, 0);
    let png = base64::engine::general_purpose::STANDARD
        .decode(snapshot.result.as_ref().unwrap()["png"].as_str().unwrap())
        .unwrap();
    let decoder = png::Decoder::new(std::io::Cursor::new(&png));
    let mut reader = decoder.read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (544, 246));
    let mut pixels = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut pixels).unwrap();
    let image = varos_bridge::mcp::tool_result(&snapshot);
    assert_eq!(image["content"][1]["type"], "image");
    assert!(image["structuredContent"]["result"].get("png").is_none());
    assert!(!image["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains(snapshot.result.as_ref().unwrap()["png"].as_str().unwrap()));
    if let Some(path) = std::env::var_os("VAROS_BRIDGE_POSTER_PNG") {
        std::fs::write(&path, &png).unwrap();
    }
    let high = h.editor.doc.ids;
    h.editor.undo();
    assert!(h.editor.doc.content_eq(&before));
    assert!(!h.editor.history_available(false));
    h.editor.redo();
    assert_eq!(h.editor.doc.paths.len(), 5);
    h.editor.undo();
    let r = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.0","board":"b1","request_id":"r2","expected_rev":5,"ops":[shape("$new","rect",[0.0,0.0,10.0,10.0])]}),
        ),
    );
    assert!(r.ok, "{r:?}");
    assert!(h.editor.doc.paths[0].id > high);
}
#[test]
fn each_design_verb_success_and_indexed_not_found_rollback() {
    let operations = [
        json!({"verb":"resize","ids":["path:10"],"bounds":[30,40,160,50]}),
        json!({"verb":"rotate","ids":["path:10"],"degrees":30}),
        json!({"verb":"rename","ids":["path:10"],"name":"  New name  "}),
        json!({"verb":"align","ids":["path:10","path:20"],"mode":"left","target":"selection"}),
        json!({"verb":"distribute","ids":["path:10","path:20","$third"],"axis":"h"}),
        json!({"verb":"group","ids":["path:10","path:20"],"local":"$group"}),
        json!({"verb":"order","ids":["path:10"],"order":"front"}),
        json!({"verb":"delete","ids":["path:10"]}),
        json!({"verb":"ungroup","ids":["$group"]}),
    ];
    for op in operations {
        let mut h = FakeHost::new();
        let before = h.editor.doc.clone();
        let mut s = Service::new("test-epoch".into());
        h.editor.tool = ToolKind::Direct;
        h.editor.bridge_select(vec![20]).unwrap();
        let mut ops = vec![shape("$third", "ellipse", [350.0, 20.0, 60.0, 60.0])];
        if op["verb"] == "ungroup" {
            ops.push(json!({"verb":"group","ids":["path:10","path:20"],"local":"$group"}));
        }
        ops.push(op.clone());
        let request = design_request(ops, 1);
        let owner = ctx();

        let r = s.handle(&mut h, &owner, request.clone(), &AtomicBool::new(false));
        assert!(r.ok, "op={op} reply={r:?}");
        assert_eq!(r.undo_steps, 1);
        assert!(h.editor.tool == ToolKind::Direct);
        if op["verb"] != "delete" {
            assert!(h.editor.objsel.contains(&20));
        }
        if op["verb"] == "resize" {
            let b = h.editor.doc.outline_bbox(h.editor.doc.pidx(10).unwrap());
            for (actual, expected) in [b.0, b.1, b.2, b.3].into_iter().zip([30.0, 40.0, 190.0, 90.0]) {
                assert!((actual - expected).abs() < 0.001);
            }
        }
        if op["verb"] == "rotate" {
            assert!((h.editor.doc.unit_xform(10).rot.to_degrees() - 30.0).abs() < 0.01);
        }
        if op["verb"] == "rename" {
            assert_eq!(h.editor.doc.paths[h.editor.doc.pidx(10).unwrap()].name.as_deref(), Some("New name"));
        }
        if op["verb"] == "align" {
            assert!((h.editor.doc.outline_bbox(h.editor.doc.pidx(20).unwrap()).0 - 20.0).abs() < 0.001);
        }
        if op["verb"] == "order" {
            assert_eq!(h.editor.doc.paths.last().unwrap().id, 10);
        }
        if op["verb"] == "delete" {
            assert!(h.editor.doc.pidx(10).is_none());
        }
        if op["verb"] == "ungroup" {
            assert!(h.editor.doc.top_group_of_path(10).is_none());
        }
        h.editor.undo();
        assert!(h.editor.doc.content_eq(&before));
        assert!(!h.editor.history_available(false));
        let mut bad = op.clone();
        bad["ids"] = json!(["path:99999"]);
        let r = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":"1.0","board":"b1","request_id":"r2","expected_rev":3,"ops":[shape("$a","rect",[1.0,2.0,3.0,4.0]),bad]}),
            ),
        );
        assert_eq!(r.error.as_ref().unwrap().code, "not_found", "{op} {r:?}");
        assert_eq!(r.error.unwrap().op_index, Some(1));
        assert!(h.editor.doc.content_eq(&before));
        assert!(h.editor.history_available(true));
    }
}
#[test]
fn design_validation_failures_rollback_every_prefix_and_local_names_are_request_scoped() {
    let failures = [
        (shape("$bad", "rect", [0.0, 0.0, -1.0, 10.0]), "invalid_argument"),
        (json!({"verb":"add_shape","kind":"ellipse","bounds":[0,0,10,10],"parent":"node:999"}), "not_found"),
        (json!({"verb":"resize","ids":["path:10"],"bounds":[0,0,0.00001,1]}), "invalid_argument"),
        (json!({"verb":"rename","ids":["path:10"],"name":"  "}), "invalid_argument"),
        (json!({"verb":"align","ids":["path:10"],"mode":"center","target":"selection"}), "invalid_argument"),
        (json!({"verb":"align","ids":["path:10"],"mode":"center","target":"Auto"}), "unsupported"),
        (json!({"verb":"distribute","ids":["path:10","path:20"],"axis":"v"}), "invalid_argument"),
        (json!({"verb":"group","ids":["path:10"],"local":"$group"}), "invalid_argument"),
        (json!({"verb":"ungroup","ids":["path:10"]}), "unsupported"),
        (json!({"verb":"move","ids":["$unknown"],"delta":[10,0]}), "not_found"),
        (shape("$a", "ellipse", [0.0, 0.0, 10.0, 10.0]), "invalid_argument"),
    ];
    for (bad, code) in failures {
        for index in 1..4 {
            let mut h = FakeHost::new();
            let before = h.editor.doc.clone();
            let mut s = Service::new("test-epoch".into());
            let mut ops = vec![shape("$a", "rect", [0.0, 0.0, 10.0, 10.0])];
            for _ in 1..index {
                ops.push(json!({"verb":"move","ids":["$a"],"delta":[5,0]}));
            }
            ops.push(bad.clone());
            let r = handle(&mut s, &mut h, design_request(ops, 1));
            assert_eq!(r.error.as_ref().unwrap().code, code, "{bad} {r:?}");
            assert_eq!(r.error.unwrap().op_index, Some(index));
            assert_eq!(h.editor.doc, before);
            assert_eq!(h.editor.rev, 1);
            assert!(!h.editor.history_available(false));
        }
    }
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    assert!(handle(&mut s, &mut h, design_request(vec![shape("$a", "rect", [0.0, 0.0, 10.0, 10.0])], 1)).ok);
    let r = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.0","board":"b1","request_id":"r2","expected_rev":2,"ops":[{"verb":"move","ids":["$a"],"delta":[5,0]}]}),
        ),
    );
    assert_eq!(r.error.unwrap().code, "not_found");
}
#[test]
fn destructive_edit_without_grant_is_one_undo_and_idempotent() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let request = design_request(vec![json!({"verb":"delete","ids":["path:10"]})], 1);
    let first = handle(&mut s, &mut h, request.clone());
    assert!(first.ok, "{first:?}");
    assert_eq!(first.undo_steps, 1);
    assert_eq!(json!(handle(&mut s, &mut h, request)), json!(first));
    h.editor.undo();
    assert!(h.editor.doc.pidx(10).is_some());
}
#[test]
fn snapshots_refuse_stale_oversized_and_ungranted_reads_without_history() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let before = h.editor.doc.clone();
    for (args, code) in [
        (json!({"board":"b1","rev":0}), "revision_conflict"),
        (json!({"board":"b1","rev":1,"width":1025}), "limit_exceeded"),
        (json!({"board":"b1","rev":1,"height":0}), "limit_exceeded"),
    ] {
        let r = handle(&mut s, &mut h, req("snapshot", args));
        assert_eq!(r.error.unwrap().code, code);
    }
    let context = ctx();

    assert!(s.handle(&mut h, &context, req("snapshot", json!({"board":"b1","rev":1})), &AtomicBool::new(false)).ok);
    assert_eq!(h.editor.doc, before);
    assert!(!h.editor.history_available(false));
}

#[test]
fn shape_parent_paints_names_and_allocator_failures_are_checked() {
    for kind in ["rect", "ellipse"] {
        let mut h = empty_host();
        let parent = h.editor.doc.active_layer;
        let mut s = Service::new("test-epoch".into());
        let r = handle(
            &mut s,
            &mut h,
            design_request(
                vec![
                    json!({"verb":"add_shape","kind":kind,"bounds":[10,20,30,40],"parent":format!("node:{parent}"),"insert":"top","name":"  Shape  ","fill":null,"stroke":"#123456FF","stroke_width":3,"opacity":0.25,"local":"$shape"}),
                ],
                1,
            ),
        );
        assert!(r.ok, "{r:?}");
        let p = &h.editor.doc.paths[0];
        assert_eq!(p.name.as_deref(), Some("Shape"));
        assert_eq!(p.fill, Paint::None);
        assert_eq!(p.opacity, 0.25);
        assert_eq!(p.stroke_width, 3.0);
        assert_eq!(p.anchors.len(), 4);
        assert_eq!(h.editor.doc.node(h.editor.doc.node_of_path(p.id).unwrap()).unwrap().parent, Some(parent));
        if kind == "ellipse" {
            assert!(p.anchors.iter().all(|a| a.hin.is_some() && a.hout.is_some()));
        } else {
            assert!(p.anchors.iter().all(|a| a.hin.is_none() && a.hout.is_none()));
        }
    }
    for (property, value) in [
        ("fill", json!("#bad")),
        ("stroke_width", json!(-1)),
        ("opacity", json!(1.1)),
        ("local", json!("$bad-name")),
        ("name", json!(" ")),
    ] {
        let mut h = empty_host();
        let mut s = Service::new("test-epoch".into());
        let before = h.editor.doc.clone();
        let mut op = shape("$a", "rect", [0.0, 0.0, 10.0, 10.0]);
        op[property] = value;
        let r = handle(&mut s, &mut h, design_request(vec![op], 1));
        assert_eq!(r.error.unwrap().code, "invalid_argument");
        assert_eq!(h.editor.doc, before);
    }
    for locked in [false, true] {
        let mut h = empty_host();
        let parent = h.editor.doc.active_layer;
        let n = h.editor.doc.nodes.iter_mut().find(|n| n.id == parent).unwrap();
        n.locked = locked;
        n.hidden = !locked;
        let mut s = Service::new("test-epoch".into());
        let r = handle(&mut s, &mut h, design_request(vec![shape("$a", "rect", [0.0, 0.0, 10.0, 10.0])], 1));
        assert_eq!(r.error.unwrap().code, if locked { "locked_target" } else { "hidden_target" });
        assert!(h.editor.doc.paths.is_empty());
    }
    let mut h = empty_host();
    h.editor.doc.ids = u32::MAX - 1;
    let before = h.editor.doc.clone();
    let mut s = Service::new("test-epoch".into());
    let r = handle(&mut s, &mut h, design_request(vec![shape("$a", "rect", [0.0, 0.0, 10.0, 10.0])], 1));
    assert_eq!(r.error.unwrap().code, "invalid_argument");
    assert_eq!(h.editor.doc, before);
    // The core command itself exposes creation outcomes and validates before allocating.
    let mut ed = Editor::new();
    let id = ed
        .try_execute_created(EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Rect,
            bounds: [0.0, 0.0, 10.0, 20.0],
            parent: None,
            fill: Some([1.0; 4]),
            stroke: None,
            stroke_width: 0.0,
            opacity: 1.0,
            name: None,
        })
        .unwrap();
    assert_eq!(ed.doc.paths[0].id, id);
}
#[test]
fn stroke_only_shape_requires_positive_width() {
    for kind in ["rect", "ellipse"] {
        for width in [None, Some(0.0)] {
            let mut h = empty_host();
            let mut s = Service::new("test-epoch".into());
            let before = h.editor.doc.clone();
            let mut op = json!({"verb":"add_shape","kind":kind,"bounds":[0,0,10,20],"stroke":"#123456FF"});
            if let Some(width) = width {
                op["stroke_width"] = json!(width);
                op["fill"] = Value::Null;
            }
            let r = handle(&mut s, &mut h, design_request(vec![op], 1));
            let error = r.error.unwrap();
            assert_eq!(error.code, "invalid_argument");
            assert_eq!(error.reason, "stroke_width must be > 0 when stroke is the only paint");
            assert_eq!(r.undo_steps, 0);
            assert_eq!(h.editor.rev, 1);
            assert_eq!(h.editor.doc, before);
        }
    }
}
#[test]
fn align_all_modes_revision_bound_artboards_and_rigid_groups() {
    for (mode, want) in [
        ("left", [0.0, 20.0]),
        ("center", [210.0, 20.0]),
        ("right", [420.0, 20.0]),
        ("top", [20.0, 0.0]),
        ("middle", [20.0, 150.0]),
        ("bottom", [20.0, 300.0]),
    ] {
        let mut h = FakeHost::new();
        h.editor.doc.artboards.push(varos_core::model::Artboard {
            x: 0.0,
            y: 0.0,
            w: 500.0,
            h: 400.0,
            ..Default::default()
        });
        let mut s = Service::new("test-epoch".into());
        let r = handle(
            &mut s,
            &mut h,
            design_request(vec![json!({"verb":"align","ids":["path:10"],"mode":mode,"target":"a0@1"})], 1),
        );
        assert!(r.ok, "{mode} {r:?}");
        let b = h.editor.doc.outline_bbox(h.editor.doc.pidx(10).unwrap());
        assert!((b.0 - want[0]).abs() < 0.001 && (b.1 - want[1]).abs() < 0.001, "{mode} {b:?}");
        assert_eq!(h.editor.doc.active, 0);
    }
    for (target, code) in [("a0@0", "revision_conflict"), ("a99@1", "not_found")] {
        let mut h = FakeHost::new();
        let before = h.editor.doc.clone();
        let mut s = Service::new("test-epoch".into());
        let r = handle(
            &mut s,
            &mut h,
            design_request(
                vec![
                    shape("$a", "rect", [0.0, 0.0, 10.0, 10.0]),
                    json!({"verb":"align","ids":["$a"],"mode":"left","target":target}),
                ],
                1,
            ),
        );
        assert_eq!(r.error.as_ref().unwrap().code, code);
        assert_eq!(r.error.unwrap().op_index, Some(1));
        assert_eq!(h.editor.doc, before);
    }
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let r = handle(
        &mut s,
        &mut h,
        design_request(
            vec![
                json!({"verb":"group","ids":["path:10","path:20"],"local":"$g"}),
                shape("$a", "rect", [400.0, 20.0, 10.0, 20.0]),
                json!({"verb":"align","ids":["$g","$a"],"mode":"right","target":"selection"}),
            ],
            1,
        ),
    );
    assert!(r.ok, "{r:?}");
    let a = h.editor.doc.outline_bbox(h.editor.doc.pidx(10).unwrap());
    let b = h.editor.doc.outline_bbox(h.editor.doc.pidx(20).unwrap());
    assert!((b.0 - a.0 - 120.0).abs() < 0.001);
    assert!((b.2 - 410.0).abs() < 0.001);
}
#[test]
fn rotation_is_absolute_for_mixed_units_and_rotated_resize_uses_local_size() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let r = handle(
        &mut s,
        &mut h,
        design_request(
            vec![
                json!({"verb":"rotate","ids":["path:10"],"degrees":30}),
                json!({"verb":"rotate","ids":["path:20"],"degrees":60}),
                json!({"verb":"rotate","ids":["path:10","path:20"],"degrees":90}),
                json!({"verb":"rotate","ids":["path:10","path:20"],"degrees":90}),
                json!({"verb":"rotate","ids":["path:10","path:20"],"degrees":0}),
                json!({"verb":"rotate","ids":["path:10","path:20"],"degrees":90}),
                json!({"verb":"resize","ids":["path:10"],"bounds":[10,20,160,50]}),
            ],
            1,
        ),
    );
    assert!(r.ok, "{r:?}");
    for pid in [10, 20] {
        assert!((h.editor.doc.unit_xform(pid).rot.to_degrees() - 90.0).abs() < 0.001);
    }
    h.editor.try_execute(EditCommand::SelectPaths(vec![10])).unwrap();
    let size = h.editor.obj_local_bbox().unwrap();
    assert!((size.2 - size.0 - 160.0).abs() < 0.01 && (size.3 - size.1 - 50.0).abs() < 0.01, "{size:?}");
}
#[test]
fn whole_group_rotation_repeats_absolute_angle_and_resets_to_zero() {
    let mut h = FakeHost::new();
    h.editor.execute_ui(EditCommand::SelectPaths(vec![10, 20]));
    h.editor.execute_ui(EditCommand::GroupSelection);
    let group = h.editor.doc.top_group_of_path(10).unwrap();
    let mut s = Service::new("test-epoch".into());
    let mut at_ninety = None;
    for (i, degrees) in [90, 90, 0].into_iter().enumerate() {
        let rev = h.editor.rev;
        let request = req(
            "edit",
            json!({"api":"1.0","board":"b1","request_id":format!("r{}", i + 1),"expected_rev":rev,
                "ops":[{"verb":"rotate","ids":[format!("node:{group}")],"degrees":degrees}]}),
        );
        let reply = handle(&mut s, &mut h, request);
        assert!(reply.ok, "{reply:?}");
        assert!((h.editor.doc.node_xform(group).rot.to_degrees() - degrees as f32).abs() < 0.001);
        if i == 0 {
            at_ninety = Some(h.editor.doc.clone());
        } else if i == 1 {
            assert_eq!(Some(&h.editor.doc), at_ninety.as_ref());
            assert_eq!(reply.undo_steps, 0);
        } else {
            assert_eq!(reply.undo_steps, 1);
        }
    }
}
#[test]
fn distribution_both_axes_order_all_positions_and_partial_groups_refuse() {
    for axis in ["h", "v"] {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        let r = handle(
            &mut s,
            &mut h,
            design_request(
                vec![
                    shape("$a", "rect", [500.0, 500.0, 10.0, 10.0]),
                    json!({"verb":"distribute","ids":["path:10","path:20","$a"],"axis":axis}),
                ],
                1,
            ),
        );
        assert!(r.ok, "{r:?}");
        let centres: Vec<_> = h
            .editor
            .doc
            .paths
            .iter()
            .map(|p| {
                let b = h.editor.doc.outline_bbox(h.editor.doc.pidx(p.id).unwrap());
                if axis == "h" {
                    (b.0 + b.2) / 2.0
                } else {
                    (b.1 + b.3) / 2.0
                }
            })
            .collect();
        let mut centres = centres;
        centres.sort_by(f32::total_cmp);
        assert!((centres[1] - (centres[0] + centres[2]) / 2.0).abs() < 0.01);
    }
    for order in ["front", "forward", "backward", "back"] {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        let pid = if order == "back" || order == "backward" { 20 } else { 10 };
        let r = handle(
            &mut s,
            &mut h,
            design_request(vec![json!({"verb":"order","ids":[format!("path:{pid}")],"order":order})], 1),
        );
        assert!(r.ok, "{r:?}");
        assert_eq!(h.editor.doc.paths[0].id, 20);
        assert_eq!(r.undo_steps, 1);
    }
    for op in [
        json!({"verb":"delete","ids":["path:10"]}),
        json!({"verb":"order","ids":["path:10"],"order":"front"}),
        json!({"verb":"distribute","ids":["$g","$a"],"axis":"h"}),
    ] {
        let mut h = FakeHost::new();
        let before = h.editor.doc.clone();
        let mut s = Service::new("test-epoch".into());
        let r = handle(
            &mut s,
            &mut h,
            design_request(
                vec![
                    json!({"verb":"group","ids":["path:10","path:20"],"local":"$g"}),
                    shape("$a", "rect", [400.0, 20.0, 10.0, 20.0]),
                    op,
                ],
                1,
            ),
        );
        assert_eq!(r.error.as_ref().unwrap().code, "unsupported");
        assert_eq!(r.error.unwrap().op_index, Some(2));
        assert_eq!(h.editor.doc, before);
    }
}

#[test]
fn poster_frozen_fixture_through_actual_mcp_stdio_binding() {
    let transport = FakeTransport {
        state: std::sync::Arc::new(std::sync::Mutex::new((Service::new("test-epoch".into()), empty_host()))),
        cancellations: Default::default(),
    };
    let (input_tx, input) = std::sync::mpsc::channel();
    let (output, receive) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        varos_bridge::mcp::serve(
            &mut std::io::BufReader::new(ChannelRead { rx: input, current: std::io::Cursor::new(vec![]) }),
            ChannelWrite { tx: output, bytes: vec![] },
            transport,
        )
        .unwrap()
    });
    let send = |v: Value| {
        let mut data = serde_json::to_vec(&v).unwrap();
        data.push(b'\n');
        input_tx.send(data).unwrap();
    };
    let read =
        || serde_json::from_slice::<Value>(&receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap()).unwrap();
    send(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{},"capabilities":{}}}),
    );
    assert_eq!(read()["result"]["protocolVersion"], "2025-06-18");
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let arguments: Value = serde_json::from_str(include_str!("fixtures/poster-request-1.0.json")).unwrap();
    send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"edit","arguments":arguments}}));
    assert_eq!(
        read()["result"]["structuredContent"],
        serde_json::from_str::<Value>(include_str!("fixtures/poster-result-1.0.json")).unwrap()
    );
    send(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"snapshot","arguments":{"board":"b1","rev":2,"width":80,"height":120}}}),
    );
    let result = read()["result"].clone();
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["structuredContent"]["result"]["width"], 80);
    assert_eq!(result["structuredContent"]["result"]["height"], 120);
    assert!(result["structuredContent"]["result"].get("png").is_none());

    drop(input_tx);
    server.join().unwrap();
}
/// Slice 3: the frozen Instagram Story batch through the actual MCP stdio binding, then a page snapshot
/// of the new artboard (the story's 9:16 ratio inside the requested box).
#[test]
fn story_frozen_fixture_through_actual_mcp_stdio_binding() {
    let transport = FakeTransport {
        state: std::sync::Arc::new(std::sync::Mutex::new((Service::new("test-epoch".into()), empty_host()))),
        cancellations: Default::default(),
    };
    let (input_tx, input) = std::sync::mpsc::channel();
    let (output, receive) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        varos_bridge::mcp::serve(
            &mut std::io::BufReader::new(ChannelRead { rx: input, current: std::io::Cursor::new(vec![]) }),
            ChannelWrite { tx: output, bytes: vec![] },
            transport,
        )
        .unwrap()
    });
    let send = |v: Value| {
        let mut data = serde_json::to_vec(&v).unwrap();
        data.push(b'\n');
        input_tx.send(data).unwrap();
    };
    let read =
        || serde_json::from_slice::<Value>(&receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap()).unwrap();
    send(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{},"capabilities":{}}}),
    );
    assert_eq!(read()["result"]["protocolVersion"], "2025-06-18");
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let arguments: Value = serde_json::from_str(include_str!("fixtures/story-request-1.0.json")).unwrap();
    send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"edit","arguments":arguments}}));
    assert_eq!(
        read()["result"]["structuredContent"],
        serde_json::from_str::<Value>(include_str!("fixtures/story-result-1.0.json")).unwrap()
    );
    send(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"snapshot","arguments":{"board":"b1","rev":2,"artboard":"artboard:2","width":200,"height":200}}}),
    );
    let result = read()["result"].clone();
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["structuredContent"]["result"]["width"], 113);
    assert_eq!(result["structuredContent"]["result"]["height"], 200);
    assert_eq!(result["structuredContent"]["result"]["artboard"], "artboard:2");
    drop(input_tx);
    server.join().unwrap();
}
#[test]
fn geometry_is_explicit_bounded_paginated_and_does_not_mutate() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let before = h.editor.doc.clone();
    let plain = handle(&mut s, &mut h, req("describe", json!({"board":"b1","ids":["path:10"]})));
    assert!(plain.result.unwrap()["objects"][0].get("geometry").is_none());
    let args = json!({"board":"b1","rev":1,"ids":["path:10","path:20"],"fields":["geometry"],"limit":1});
    let first = handle(&mut s, &mut h, req("describe", args.clone()));
    let value = first.result.unwrap();
    assert_eq!(value["more"], true);
    assert_eq!(value["objects"][0]["geometry"]["closed"], true);
    let mut next = args;
    next["cursor"] = value["cursor"].clone();
    let second = handle(&mut s, &mut h, req("describe", next));
    assert_eq!(second.result.unwrap()["objects"][0]["id"], "path:20");
    assert_eq!(h.editor.doc, before);
    assert!(!h.editor.history_available(false));
    let p = &mut h.editor.doc.paths[0];
    let anchor = p.anchors[0].clone();
    for i in 0..1001 {
        let mut a = anchor.clone();
        a.id = 100 + i;
        p.anchors.push(a);
    }
    h.editor.rev += 1;
    let r = handle(&mut s, &mut h, req("describe", json!({"board":"b1","ids":["path:10"],"fields":["geometry"]})));
    assert_eq!(r.error.unwrap().code, "limit_exceeded");
}

#[test]
fn successful_create_delete_noop_still_reserves_exposed_identities() {
    let mut h = empty_host();
    let before = h.editor.doc.clone();
    let mut s = Service::new("test-epoch".into());
    let owner = ctx();

    let request =
        design_request(vec![shape("$a", "rect", [0.0, 0.0, 10.0, 10.0]), json!({"verb":"delete","ids":["$a"]})], 1);
    let r = s.handle(&mut h, &owner, request, &AtomicBool::new(false));
    assert!(r.ok, "{r:?}");
    assert_eq!(r.undo_steps, 0);
    assert_eq!(h.editor.rev, 1);
    assert!(h.editor.doc.content_eq(&before));
    assert!(!h.editor.history_available(false));
    let exposed =
        r.result.unwrap()["locals"]["$a"].as_str().unwrap().strip_prefix("path:").unwrap().parse::<u32>().unwrap();
    let r = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.0","board":"b1","request_id":"r2","expected_rev":1,"ops":[shape("$b","rect",[0.0,0.0,10.0,10.0])]}),
        ),
    );
    assert!(r.ok, "{r:?}");
    assert!(h.editor.doc.paths[0].id > exposed + 4);
    assert_eq!(r.undo_steps, 1);
}

#[test]
fn distribution_equal_centre_ties_use_stable_ids() {
    for _ in 0..8 {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        let r = handle(
            &mut s,
            &mut h,
            design_request(
                vec![
                    shape("$third", "rect", [500.0, 500.0, 10.0, 10.0]),
                    json!({"verb":"distribute","ids":["path:20","$third","path:10"],"axis":"v"}),
                ],
                1,
            ),
        );
        assert!(r.ok, "{r:?}");
        let a = h.editor.doc.outline_bbox(h.editor.doc.pidx(10).unwrap());
        let b = h.editor.doc.outline_bbox(h.editor.doc.pidx(20).unwrap());
        assert!((a.1 - 20.0).abs() < 0.001 && (b.1 - 237.5).abs() < 0.001, "{a:?} {b:?}");
    }
}

#[test]
fn explicit_group_leaves_resize_and_undo_without_sibling_changes_partial_rotate_refused() {
    for rotated in [false, true] {
        for (verb, degrees) in [("resize", 0), ("rotate", 0), ("rotate", 90)] {
            let mut h = FakeHost::new();
            h.editor.execute_ui(EditCommand::SelectPaths(vec![10, 20]));
            h.editor.execute_ui(EditCommand::GroupSelection);
            if rotated {
                h.editor.execute_ui(EditCommand::SetObjectRotation(30.0));
            }
            let group = h.editor.doc.top_group_of_path(10).unwrap();
            let before = h.editor.doc.clone();
            let sibling = h.editor.doc.paths[h.editor.doc.pidx(20).unwrap()].clone();
            let old_group_bounds =
                varos_core::bridge::describe(&before, Some(&format!("node:{group}"))).unwrap()["bounds"].clone();
            let rev = h.editor.rev;
            let op = if verb == "resize" {
                json!({"verb":verb,"ids":["path:10"],"bounds":[-100,-100,200,150]})
            } else {
                json!({"verb":verb,"ids":["path:10"],"degrees":degrees})
            };
            let mut service = Service::new("test-epoch".into());
            let reply = handle(&mut service, &mut h, design_request(vec![op], rev));
            if verb == "rotate" || rotated {
                let error = reply.error.unwrap();
                assert_eq!(error.code, "unsupported");
                if verb == "rotate" {
                    assert!(error.reason.contains("no independent absolute leaf angle"));
                    assert!(error.reason.contains(&format!("rotate the whole group node:{group}")));
                } else {
                    assert!(error.reason.contains("rotated group") && error.reason.contains("move"));
                }
                assert_eq!(reply.undo_steps, 0);
                assert_eq!(h.editor.rev, rev);
                assert_eq!(h.editor.doc, before);
                continue;
            }
            assert!(reply.ok, "{reply:?}");
            assert_eq!(reply.undo_steps, 1);
            assert_eq!(h.editor.doc.paths[h.editor.doc.pidx(20).unwrap()], sibling);
            assert_ne!(
                varos_core::bridge::describe(&h.editor.doc, Some(&format!("node:{group}"))).unwrap()["bounds"],
                old_group_bounds
            );
            if verb == "resize" {
                let actual = h.editor.doc.outline_bbox(h.editor.doc.pidx(10).unwrap());
                for (actual, expected) in
                    [actual.0, actual.1, actual.2, actual.3].into_iter().zip([-100.0, -100.0, 100.0, 50.0])
                {
                    assert!((actual - expected).abs() < 0.001);
                }
            }
            h.editor.undo();
            assert_eq!(h.editor.doc, before);
        }
    }
}

#[test]
fn order_explicit_group_id_moves_as_a_unit() {
    for order in ["front", "forward", "backward", "back"] {
        let mut h = FakeHost::new();
        let mut service = Service::new("test-epoch".into());
        let reply = handle(
            &mut service,
            &mut h,
            design_request(
                vec![
                    json!({"verb":"group","ids":["path:10","path:20"],"local":"$g"}),
                    shape("$a", "rect", [300.0, 0.0, 10.0, 10.0]),
                ],
                1,
            ),
        );
        assert!(reply.ok, "{reply:?}");
        let locals = &reply.result.unwrap()["locals"];
        let gid = locals["$g"].as_str().unwrap().strip_prefix("node:").unwrap().parse::<u32>().unwrap();
        let aid = locals["$a"].as_str().unwrap().strip_prefix("path:").unwrap().parse::<u32>().unwrap();
        if order == "back" || order == "backward" {
            let rev = h.editor.rev;
            let r = handle(
                &mut service,
                &mut h,
                req(
                    "edit",
                    json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":rev,"ops":[{"verb":"order","ids":[format!("node:{gid}")],"order":"front"}]}),
                ),
            );
            assert!(r.ok, "{r:?}");
        }
        let before = h.editor.doc.clone();
        let rev = h.editor.rev;
        let r = handle(
            &mut service,
            &mut h,
            req(
                "edit",
                json!({"api":"1.0","request_id":"r3","board":"b1","expected_rev":rev,"ops":[{"verb":"order","ids":[format!("node:{gid}")],"order":order}]}),
            ),
        );
        assert!(r.ok, "{r:?}");
        assert_eq!(r.undo_steps, 1);
        let ids: Vec<_> = h.editor.doc.paths.iter().map(|p| p.id).collect();
        assert_eq!(ids, if order == "front" || order == "forward" { vec![aid, 10, 20] } else { vec![10, 20, aid] });
        assert_eq!(h.editor.doc.node_paths(gid), vec![10, 20]);
        h.editor.undo();
        assert_eq!(h.editor.doc, before);
    }
}

#[test]
fn edit_confirm_true_is_refused_and_bare_creation_requires_paint() {
    let error=varos_bridge::mcp::decode_tool("edit",json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"confirm":true,"ops":[{"verb":"delete","ids":["path:10"]}]})).unwrap_err();
    assert_eq!(error.code, "invalid_argument");
    for extra in [json!({}), json!({"fill":null,"stroke":null})] {
        let mut h = FakeHost::new();
        let before = h.editor.doc.clone();
        let mut service = Service::new("test-epoch".into());
        let mut op = json!({"verb":"add_shape","kind":"rect","bounds":[0,0,10,10]});
        op.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let r = handle(&mut service, &mut h, design_request(vec![op], 1));
        let error = r.error.unwrap();
        assert_eq!(error.code, "invalid_argument");
        assert!(error.reason.contains("paint required"));
        assert_eq!(h.editor.doc, before);
    }
}

#[test]
fn geometry_single_object_byte_budget_and_capabilities_are_honest() {
    let mut h = FakeHost::new();
    let anchor = h.editor.doc.paths[0].anchors[0].clone();
    for i in 0..600 {
        let mut a = anchor.clone();
        a.id = 100 + i;
        h.editor.doc.paths[0].anchors.push(a);
    }
    let mut service = Service::new("test-epoch".into());
    let r = handle(
        &mut service,
        &mut h,
        req("describe", json!({"board":"b1","ids":["path:10"],"fields":["geometry"],"limit":1})),
    );
    let error = r.error.unwrap();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.reason.contains("16 KiB"));
    assert!(!error.reason.contains("reduce page limit"));
    let caps = handle(&mut service, &mut h, req("capabilities", json!({"api":"1.0"}))).result.unwrap();
    assert!(caps.get("files_roots_granted").is_none());
    assert_eq!(caps["trust"], "local user");
    assert!(caps["file_guards"].as_array().unwrap().iter().any(|v| v == "no_overwrite"));
    assert_eq!(caps["limits"]["geometry_page_bytes"], 16384);
    assert_eq!(caps["limits"]["geometry_typical_anchors_per_page"], 300);
    assert_eq!(caps["limits"]["geometry_anchor_pagination"], false);
}

#[test]
fn slice4_path_radius_validation_and_indexed_rollback() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let rev = h.editor.rev;
    let before = h.editor.doc.clone();
    let shape =
        json!({"verb":"add_shape","kind":"rect","bounds":[0,0,100,60],"radius":10,"fill":"#FF0000FF","local":"$round"});
    let path = json!({"verb":"add_path","anchors":[{"p":[0,0],"hout":[5,0]},{"p":[20,20],"hin":[15,20],"smooth":true}],"closed":false,"stroke":"#000000FF","stroke_width":2,"local":"$curve"});
    let r = handle(
        &mut s,
        &mut h,
        req("edit", json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":rev,"ops":[shape,path]})),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(r.undo_steps, 1);
    let doc = h.editor.doc.clone();
    let round = doc.paths.iter().find(|p| p.anchors.len() == 8).unwrap();
    assert_eq!(round.anchors[0].p, [10.0, 0.0]);
    assert_eq!(round.anchors[1].hout, Some([90.0 + varos_core::model::K * 10.0, 0.0]));
    let curve = doc.paths.last().unwrap();
    assert!(!curve.closed);
    assert_eq!(curve.anchors[0].hout, Some([5.0, 0.0]));
    let max = curve.anchors.iter().map(|a| a.id).max().unwrap();
    h.editor.undo();
    assert!(h.editor.doc.content_eq(&before));
    for invalid in [
        json!({"verb":"add_path","anchors":[{"p":[0,0]}],"closed":true,"fill":"#FF0000FF"}),
        json!({"verb":"add_shape","kind":"rect","bounds":[0,0,100,60],"radius":-1,"fill":"#FF0000FF"}),
        json!({"verb":"add_shape","kind":"ellipse","bounds":[0,0,100,60],"radius":10,"fill":"#FF0000FF"}),
        json!({"verb":"add_path","anchors":vec![json!({"p":[0,0]});1001],"closed":true,"fill":"#FF0000FF"}),
    ] {
        let rev = h.editor.rev;
        let r = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":rev,"ops":[{"verb":"move","ids":["path:10"],"delta":[9,0]},invalid]}),
            ),
        );
        assert!(!r.ok);
        assert_eq!(r.error.unwrap().op_index, Some(1));
        assert!(h.editor.doc.content_eq(&before));
    }
    let rev = h.editor.rev;
    let r = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":rev,"ops":[{"verb":"add_path","anchors":[{"p":[0,0]},{"p":[1,1]}],"closed":false,"fill":"#FF0000FF","local":"$new"},{"verb":"move","ids":["$new"],"delta":[3,4]}]}),
        ),
    );
    assert!(r.ok, "{r:?}");
    assert!(h.editor.doc.paths.last().unwrap().id > max);
}

#[test]
fn slice4_selection_field_and_schema_completeness() {
    let mut s = Service::new("test-epoch".into());
    let mut h = FakeHost::new();
    h.editor.bridge_select(vec![10, 20]).unwrap();
    let r = handle(&mut s, &mut h, req("describe", json!({"board":"b1","fields":["selection"],"limit":1})));
    assert!(r.ok, "{r:?}");
    let v = r.result.unwrap();
    assert_eq!(v["selection"], json!(["path:10"]));
    assert_eq!(v["more"], true);
    let r = handle(
        &mut s,
        &mut h,
        req("describe", json!({"board":"b1","fields":["selection"],"limit":1,"cursor":v["cursor"]})),
    );
    assert_eq!(r.result.unwrap()["selection"], json!(["path:20"]));
    assert!(varos_bridge::mcp::decode_tool("edit",json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":0,"ops":[{"verb":"set_artboard_color","id":"artboard:1"}]})).is_err());
    let schemas = varos_bridge::mcp::tools();
    let tools = schemas["tools"].as_array().unwrap();
    for name in varos_bridge::TOOLS {
        assert!(tools.iter().any(|t| t["name"] == *name));
    }
    let edit = tools.iter().find(|t| t["name"] == "edit").unwrap();
    let ops = edit["inputSchema"]["$defs"].as_object().unwrap().values().collect::<Vec<_>>();
    for verb in varos_bridge::EDIT_VERBS {
        assert!(ops.iter().any(|o| o["properties"]["verb"]["const"] == *verb), "{verb}");
    }
}

#[test]
fn slice4_files_scope_and_save_ticket_completion_fake_host() {
    struct Files {
        host: FakeHost,
        calls: usize,
        done: bool,
        code: Option<&'static str>,
        pending: bool,
    }
    impl Host for Files {
        fn boards(&self) -> Vec<BoardInfo> {
            self.host.boards()
        }
        fn prepare(&mut self, b: &str, m: bool) -> Result<(), Error> {
            self.host.prepare(b, m)
        }
        fn access(&mut self, b: &str) -> Result<BoardAccess<'_>, Error> {
            self.host.access(b)
        }
        fn file_effect(&mut self, _: &str, _: &varos_bridge::dto::FileEffect) -> Result<Reply, Error> {
            self.calls += 1;
            Ok(Reply::success(json!({"accepted":true,"ticket":77})))
        }
        fn file_pending(&self, _: u64) -> bool {
            self.pending
        }
        fn file_status(&mut self, t: u64) -> Option<Reply> {
            (t == 77 && self.done).then(|| {
                self.code.map_or_else(
                    || Reply::success(json!({"saved":true,"durable":true,"report":{"notes":[{"kind":"synthetic","object_id":42,"message":"worker note"}]}})),
                    |c| Reply::failure(Error::new(c, "test failure")),
                )
            })
        }
    }
    let mut h = Files { host: FakeHost::new(), calls: 0, done: false, code: None, pending: true };
    let mut s = Service::new("test-epoch".into());
    let rev = h.host.editor.rev;
    let request = || req("save", json!({"api":"1.0","board":"b1","expected_rev":rev,"request_id":"r1"}));
    let context = ctx();
    let r = s.handle(&mut h, &context, request(), &AtomicBool::new(false));
    assert!(r.ok);
    assert_eq!(h.calls, 1);
    for (tool, extras) in [
        ("save_as", json!({"path":"/granted/copy.vrs"})),
        ("export_pdf", json!({"path":"/granted/logo.pdf","scope":"artwork_bounds"})),
    ] {
        let mut args = json!({"api":"1.0","board":"b1","expected_rev":rev,"request_id":"r1"});
        args.as_object_mut().unwrap().extend(extras.as_object().unwrap().clone());
        let mut separate = Files { host: FakeHost::new(), calls: 0, done: false, code: None, pending: true };
        let mut service = Service::new("test-epoch".into());
        let granted = context.clone();

        let accepted = service.handle(&mut separate, &granted, req(tool, args), &AtomicBool::new(false));
        assert!(accepted.ok, "{accepted:?}");
        assert_eq!(accepted.undo_steps, 0);
        assert_eq!(separate.calls, 1);
    }

    for api in ["1.0", "1.1", "1.2"] {
        let mut host = Files { host: FakeHost::new(), calls: 0, done: false, code: None, pending: true };
        let mut service = Service::new("test-epoch".into());
        let result=service.handle(&mut host,&context,req("export_pdf",json!({"api":api,"board":"b1","expected_rev":rev,"request_id":"r1","path":"/granted/report.pdf","scope":"artwork_bounds"})),&AtomicBool::new(false));
        assert!(result.ok, "{result:?}");
        host.done = true;
        let completed = service.handle(
            &mut host,
            &context,
            req("request_status", json!({"request_id":"r1"})),
            &AtomicBool::new(false),
        );
        let done = completed.result.unwrap()["receipt"]["result"].clone();
        assert_eq!(done.get("report").is_some(), api == "1.2");
        let result = result.result.unwrap();
        assert!(result.get("report").is_none(), "accepted work has no completed report");
        if api == "1.2" {
            assert_eq!(done["report"]["notes"][0]["message"], "worker note");
        }
    }

    let r = s.handle(&mut h, &context, request(), &AtomicBool::new(false));
    assert!(r.ok);
    assert_eq!(r.undo_steps, 0);
    assert!(s.handle(&mut h, &context, request(), &AtomicBool::new(false)).ok);
    assert_eq!(h.calls, 1);
    let status = || req("request_status", json!({"request_id":"r1"}));
    assert_eq!(s.handle(&mut h, &context, status(), &AtomicBool::new(false)).result.unwrap()["status"], "pending");
    h.done = true;
    let r = s.handle(&mut h, &context, status(), &AtomicBool::new(false));
    assert_eq!(r.result.unwrap()["receipt"]["result"]["durable"], true);
    for code in ["save_conflict", "io_error", "scope_refused"] {
        h.code = Some(code);
        let r = s.handle(&mut h, &context, status(), &AtomicBool::new(false));
        assert!(r.ok);
        assert_eq!(r.result.unwrap()["receipt"]["error"]["code"], code);
    }
    h.done = false;
    h.pending = false;
    assert_eq!(s.handle(&mut h, &context, status(), &AtomicBool::new(false)).error.unwrap().code, "not_found");
    assert_eq!(h.host.editor.rev, rev);
}

#[test]
fn slice4_logo_frozen_fixture_adapter_parity() {
    let arguments: Value = serde_json::from_str(include_str!("fixtures/logo-request-1.0.json")).unwrap();
    let make = || {
        let mut h = FakeHost::new();
        h.editor = Editor::new();
        h
    };
    let mut cli = make();
    let mut mcp = make();
    let mut a = Service::new("test-epoch".into());
    let mut b = Service::new("test-epoch".into());
    let via_cli = serde_json::from_value::<Request>(json!({"tool":"edit","arguments":arguments})).unwrap();
    let cli_reply = handle(&mut a, &mut cli, via_cli);
    let mcp_reply = handle(&mut b, &mut mcp, req("edit", arguments));
    assert!(cli_reply.ok, "{cli_reply:?}");
    assert_eq!(cli_reply, mcp_reply);
    let value = serde_json::to_value(&cli_reply).unwrap();
    let expected: Value = serde_json::from_str(include_str!("fixtures/logo-result-1.0.json")).unwrap();
    assert_eq!(value, expected);
    assert_eq!(cli.editor.doc, mcp.editor.doc);
}

#[test]
fn slice4_logo_frozen_fixture_through_actual_mcp_stdio_binding() {
    let transport = FakeTransport {
        state: std::sync::Arc::new(std::sync::Mutex::new((Service::new("test-epoch".into()), {
            let mut h = FakeHost::new();
            h.editor = Editor::new();
            h
        }))),
        cancellations: Default::default(),
    };
    let (input_tx, input) = std::sync::mpsc::channel();
    let (output, receive) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        varos_bridge::mcp::serve(
            &mut std::io::BufReader::new(ChannelRead { rx: input, current: std::io::Cursor::new(vec![]) }),
            ChannelWrite { tx: output, bytes: vec![] },
            transport,
        )
        .unwrap()
    });
    let send = |v: Value| {
        let mut data = serde_json::to_vec(&v).unwrap();
        data.push(b'\n');
        input_tx.send(data).unwrap();
    };
    let read =
        || serde_json::from_slice::<Value>(&receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap()).unwrap();
    send(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{},"capabilities":{}}}),
    );
    assert_eq!(read()["result"]["protocolVersion"], "2025-06-18");
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let arguments: Value = serde_json::from_str(include_str!("fixtures/logo-request-1.0.json")).unwrap();
    send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"edit","arguments":arguments}}));
    assert_eq!(
        read()["result"]["structuredContent"],
        serde_json::from_str::<Value>(include_str!("fixtures/logo-result-1.0.json")).unwrap()
    );
    send(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"snapshot","arguments":{"board":"b1","rev":1,"width":80,"height":120}}}),
    );
    let result = read()["result"].clone();
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["structuredContent"]["result"]["width"], 80);
    assert_eq!(result["structuredContent"]["result"]["height"], 120);
    assert!(result["structuredContent"]["result"].get("png").is_none());

    drop(input_tx);
    server.join().unwrap();
}

#[test]
fn radius_zero_half_and_over_half_through_service() {
    for radius in [0, 10, 100] {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        let rev = h.editor.rev;
        let r = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":rev,"ops":[{"verb":"add_shape","kind":"rect","bounds":[0,0,20,20],"radius":radius,"fill":"#FFFFFFFF"}]}),
            ),
        );
        assert!(r.ok, "{r:?}");
        let path = h.editor.doc.paths.last().unwrap();
        assert_eq!(path.anchors.len(), 4);
        if radius == 0 {
            assert_eq!(path.anchors[0].p, [0.0, 0.0]);
            assert!(path.anchors.iter().all(|a| a.hin.is_none() && a.hout.is_none()));
        } else {
            assert!(path.anchors.iter().all(|a| a.hin.is_some() && a.hout.is_some()));
            assert_eq!(path.anchors[0].p, [10.0, 0.0]);
        }
    }
}

#[test]
fn noncanonical_board_refused_before_prepare() {
    let mut host = FakeHost::new();
    host.busy = true; // prepare would otherwise return busy/not_found.
    let mut service = Service::new("test-epoch".into());
    for board in ["b01", "b0", "b+1", "b1 "] {
        let reply = handle(&mut service, &mut host, req("describe", json!({"api":"1.0","board":board})));
        let error = reply.error.unwrap();
        assert_eq!(error.code, "invalid_argument");
        assert_eq!(error.reason, "board must be a canonical session handle bN");
    }
}

#[test]
fn describe_combined_frozen_transport_parity() {
    let args = json!({"board":"b1","ids":["path:10"],"fields":["metadata","artboards","selection","state","bounds","paint","parent","name","geometry"]});
    let mut h = FakeHost::new();
    h.editor.doc.artboards.push(Default::default());
    h.editor.bridge_select(vec![10, 20]).unwrap();
    let mut s = Service::new("test-epoch".into());
    let reply = handle(&mut s, &mut h, req("describe", args.clone()));
    assert!(reply.ok, "{reply:?}");
    let result = reply.result.as_ref().unwrap();
    for section in ["metadata", "artboards", "selection", "state", "objects"] {
        assert!(result.get(section).is_some(), "{section}");
    }
    assert_eq!(result["objects"].as_array().unwrap().len(), 1);
    assert_eq!(result["objects"][0]["id"], "path:10");
    assert!(result["objects"][0]["geometry"]["anchors"].is_array());
    let cli =
        varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":"describe","arguments":args})).unwrap()).unwrap();
    assert_eq!(json!(handle(&mut s, &mut h, cli)), json!(reply));
    assert_eq!(varos_bridge::mcp::tool_result(&reply)["structuredContent"], json!(reply));
    let fixture = serde_json::from_str::<Value>(include_str!("fixtures/describe-combined-1.0.json")).unwrap();
    assert_eq!(json!(reply), fixture);
    assert_eq!(
        format!("{}\n", serde_json::to_string(&reply).unwrap()),
        include_str!("fixtures/describe-combined-1.0.json")
    );
}

#[test]
fn describe_sections_do_not_page_with_objects_and_since_composes() {
    let mut h = FakeHost::new();
    h.editor.doc.artboards.extend([Default::default(), Default::default()]);
    h.editor.bridge_select(vec![10, 20]).unwrap();
    let mut s = Service::new("test-epoch".into());
    let args = json!({"board":"b1","ids":["path:10","path:20"],"fields":["artboards","selection","bounds","name","paint","parent"],"limit":1});
    let first = handle(&mut s, &mut h, req("describe", args.clone())).result.unwrap();
    assert_eq!(first["artboards"].as_array().unwrap().len(), 2);
    assert_eq!(first["selection"], json!(["path:10", "path:20"]));
    assert_eq!(first["objects"].as_array().unwrap().len(), 1);
    assert_eq!(first["more"], true);
    let mut next = args.clone();
    next["cursor"] = first["cursor"].clone();
    let second = handle(&mut s, &mut h, req("describe", next.clone())).result.unwrap();
    assert_eq!(second["objects"][0]["id"], "path:20");
    assert_eq!(second["more"], false);
    for key in ["selection", "artboards"] {
        assert_eq!(first[key], second[key]);
    }
    next["fields"] = json!(["bounds"]);
    assert_eq!(handle(&mut s, &mut h, req("describe", next)).error.unwrap().code, "resync_required");
    let board =
        handle(&mut s, &mut h, req("describe", json!({"board":"b1","fields":["artboards","selection"],"limit":1})))
            .result
            .unwrap();
    assert!(board.get("objects").is_none());
    assert_eq!(board["selection"], first["selection"]);
    assert_eq!(board["artboards"], first["artboards"]);
    assert_eq!(board["more"], false);
    let board =
        handle(&mut s, &mut h, req("describe", json!({"board":"b1","fields":["metadata","artboards"],"limit":1})))
            .result
            .unwrap();
    assert_eq!(board["artboards"].as_array().unwrap().len(), 1);
    assert_eq!(board["more"], true);
    h.editor.execute_ui(EditCommand::SetBoardName("Changed".into()));
    let mut diff_args = args;
    diff_args["since"] = json!(1);
    let diff = handle(&mut s, &mut h, req("describe", diff_args.clone())).result.unwrap();
    let plain = handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":1}))).result.unwrap();
    for key in ["from", "rev", "changed", "created", "removed", "board_changes"] {
        assert_eq!(diff[key], plain[key], "{key}");
    }
    assert_eq!(diff["objects"].as_array().unwrap().len(), 1);
    diff_args["cursor"] = diff["cursor"].clone();
    assert_eq!(handle(&mut s, &mut h, req("describe", diff_args)).result.unwrap()["objects"][0]["id"], "path:20");
    let current = handle(
        &mut s,
        &mut h,
        req("describe", json!({"board":"b1","since":2,"fields":["metadata","selection","state"]})),
    )
    .result
    .unwrap();
    assert_eq!(current["changed"], json!([]));
    assert!(current["metadata"].is_object());
    assert!(current["state"]["dirty"].as_bool().unwrap());
    assert_eq!(
        handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":0,"fields":["artboards","selection"]})))
            .error
            .unwrap()
            .code,
        "resync_required"
    );
}

#[test]
fn describe_all_field_subsets_and_unknown_fields_and_schema() {
    let fields = ["metadata", "artboards", "selection", "state", "bounds", "paint", "parent", "name", "geometry"];
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    // Observe every boundary so both current and older since requests have retained journals.
    for name in ["One", "Two", "Three"] {
        handle(&mut s, &mut h, req("describe", json!({"board":"b1"})));
        h.editor.execute_ui(EditCommand::SetBoardName(name.into()));
    }
    let rev = h.editor.rev;
    for mask in 0..(1 << fields.len()) {
        let subset: Vec<_> = fields.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, f)| *f).collect();
        let board_fields = subset.iter().any(|f| ["metadata", "artboards", "selection"].contains(f));
        let object_fields = subset.iter().any(|f| ["bounds", "paint", "parent", "name", "geometry"].contains(f));
        for ids in [None, Some(json!(["path:10"]))] {
            for since in [None, Some(rev), Some(1)] {
                let mut args = json!({"board":"b1","fields":subset});
                if let Some(ids) = &ids {
                    args["ids"] = ids.clone();
                }
                if let Some(since) = since {
                    args["since"] = json!(since);
                }
                let reply = handle(&mut s, &mut h, req("describe", args));
                assert!(reply.ok, "{subset:?}, ids={ids:?}, since={since:?}: {reply:?}");
                let result = reply.result.unwrap();
                let legacy_board = ids.is_none()
                    && ((!subset.is_empty() && subset.iter().all(|f| ["metadata", "artboards"].contains(f)))
                        || subset == ["selection"]);
                let composed = since.is_some() || (board_fields && !legacy_board);
                let objects = if composed { ids.is_some() || object_fields } else { !legacy_board };
                let mut expected_keys = vec![];
                for (key, present) in [
                    ("rev", true),
                    ("metadata", subset.contains(&"metadata")),
                    ("artboards", subset.contains(&"artboards")),
                    ("selection", since.is_some() || subset.contains(&"selection")),
                    ("selection_rev", since.is_some() || subset.contains(&"selection")),
                    ("state", composed && subset.contains(&"state")),
                    ("objects", objects),
                    ("more", since.is_none() || objects),
                    ("cursor", since.is_none() || objects),
                    ("from", since.is_some()),
                    ("changed", since.is_some()),
                    ("created", since.is_some()),
                    ("removed", since.is_some()),
                    ("board_changes", since.is_some_and(|since| since < rev)),
                    ("selection_count", since == Some(rev)),
                    ("selection_more", since == Some(rev)),
                    ("dirty", false),
                    ("active_artboard", false),
                ] {
                    if present {
                        expected_keys.push(key);
                    }
                    assert_eq!(
                        result.get(key).is_some(),
                        present,
                        "{key}, {subset:?}, ids={ids:?}, since={since:?}: {result}"
                    );
                }
                expected_keys.sort();
                let actual_keys: Vec<_> = result.as_object().unwrap().keys().map(String::as_str).collect();
                assert_eq!(actual_keys, expected_keys, "{subset:?}, ids={ids:?}, since={since:?}");
            }
        }
    }
    for args in [json!({"board":"b1","fields":["wat"]}), json!({"board":"b1","since":1,"fields":["selection","wat"]})] {
        let error = handle(&mut s, &mut h, req("describe", args)).error.unwrap();
        assert_eq!(error.code, "invalid_argument");
        assert_eq!(error.reason, "unknown describe field wat");
    }
    let schemas = varos_bridge::mcp::tools();
    let describe = schemas["tools"].as_array().unwrap().iter().find(|t| t["name"] == "describe").unwrap();
    assert!(describe["description"].as_str().unwrap().len() <= 160);
    let mut actual: Vec<_> = describe["inputSchema"]["properties"]["fields"]["items"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    let mut expected = fields.to_vec();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn describe_legacy_shapes_and_receipt_cursor_complete() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    for ids in [None, Some(json!(["path:10"]))] {
        let scoped = ids.is_some();
        let mut args = json!({"board":"b1","fields":["state"]});
        if let Some(ids) = ids {
            args["ids"] = ids;
        }
        let reply = handle(&mut s, &mut h, req("describe", args));
        assert!(reply.ok, "{reply:?}");
        let mut objects = vec![];
        if !scoped {
            objects.push(json!({"id":"node:1","kind":"layer","hidden":false,"locked":false}));
        }
        objects.push(json!({"id":"path:10","kind":"path","hidden":false,"locked":false}));
        if !scoped {
            objects.push(json!({"id":"path:20","kind":"path","hidden":false,"locked":false}));
        }
        let expected = json!({"ok":true,"board":"b1","rev":1,"undo_steps":0,"result":{"rev":1,"objects":objects,"more":false,"cursor":null}});
        assert_eq!(serde_json::to_vec(&json!(reply)).unwrap(), serde_json::to_vec(&expected).unwrap());
    }
    h.editor.doc.artboards.extend([Default::default(), Default::default()]);
    // Byte fingerprints captured from main 243f9fd; both field orders and all pages.
    let mut legacy_bytes = vec![];
    for fields in [json!(["metadata", "artboards"]), json!(["artboards", "metadata"])] {
        let mut args = json!({"board":"b1","fields":fields,"limit":1});
        loop {
            let reply = handle(&mut s, &mut h, req("describe", args.clone()));
            assert!(reply.ok, "{reply:?}");
            let result = reply.result.as_ref().unwrap();
            assert_eq!(result["artboards"].as_array().unwrap().len(), 1);
            legacy_bytes.push(json!(reply));
            if result["more"] == false {
                break;
            }
            args["cursor"] = result["cursor"].clone();
        }
    }
    assert_eq!(
        varos_bridge::service::digest(&json!(legacy_bytes)),
        "e4a4f6b145287a9b882a6654a78d3b11dc00af0537b2ea28e7dbc9e0841fb8e9"
    );

    let mut h = FakeHost::new();
    let template = h.editor.doc.paths[0].clone();
    h.editor.doc.paths.clear();
    for i in 0..200 {
        let mut path = template.clone();
        path.id = 100 + i * 10;
        for (j, anchor) in path.anchors.iter_mut().enumerate() {
            anchor.id = path.id + j as u32 + 1;
        }
        h.editor.doc.paths.push(path);
    }
    h.editor.doc.ids = 3000;
    let ids: Vec<_> = h.editor.doc.paths.iter().map(|p| format!("path:{}", p.id)).collect();
    let mut s = Service::new("test-epoch".into());
    let receipt = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.0","board":"b1","request_id":"r1","expected_rev":1,"ops":[{"verb":"move","ids":ids,"delta":[1,0]}]}),
        ),
    );
    assert!(receipt.ok, "{receipt:?}");
    let mut args = receipt.result.as_ref().unwrap()["detail_request"].clone();
    assert!(args["cursor"].is_string(), "{receipt:?}");
    assert_eq!(args["fields"], json!(["bounds", "paint", "parent", "name", "state"]));
    let mut seen = vec![];
    let mut legacy_bytes = vec![json!(receipt)];
    loop {
        let reply = handle(&mut s, &mut h, req("describe", args.clone()));
        assert!(reply.ok, "{reply:?}");
        let result = reply.result.as_ref().unwrap();
        assert!(result.get("state").is_none());
        for object in result["objects"].as_array().unwrap() {
            assert_eq!(object["hidden"], false);
            assert_eq!(object["locked"], false);
            seen.push(object["id"].as_str().unwrap().to_owned());
        }
        legacy_bytes.push(json!(reply));
        if result["more"] == false {
            break;
        }
        assert!(legacy_bytes.len() <= 201, "cursor must advance");
        args["cursor"] = result["cursor"].clone();
    }
    let mut expected = vec!["node:1".to_owned()];
    expected.extend(ids);
    assert_eq!(seen, expected);
    assert_eq!(
        varos_bridge::service::digest(&json!(legacy_bytes)),
        "a97225d48734478053ffc0d072233a4b60b558e181f3d2b205450e9a25926b2a"
    );
}

#[test]
fn describe_combined_cursor_only_tracks_requested_selection() {
    for fields in [json!(["metadata", "bounds"]), json!(["metadata", "selection", "bounds"])] {
        for since in [None, Some(1)] {
            let mut h = FakeHost::new();
            let mut s = Service::new("test-epoch".into());
            let mut args = json!({"board":"b1","ids":["path:10","path:20"],"fields":fields,"limit":1});
            if let Some(since) = since {
                args["since"] = json!(since);
            }
            let first = handle(&mut s, &mut h, req("describe", args.clone())).result.unwrap();
            args["cursor"] = first["cursor"].clone();
            h.editor.bridge_select(vec![20]).unwrap();
            let next = handle(&mut s, &mut h, req("describe", args));
            if since.is_some() || fields.as_array().unwrap().contains(&json!("selection")) {
                assert_eq!(next.error.unwrap().code, "resync_required");
            } else {
                assert!(next.ok, "{next:?}");
                assert_eq!(next.result.unwrap()["objects"][0]["id"], "path:20");
            }
        }
    }
}

#[test]
fn describe_combined_budget_errors_preserve_resync_semantics() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    handle(&mut s, &mut h, req("describe", json!({"board":"b1"})));
    h.editor.execute_ui(EditCommand::SetBoardName("Changed".into()));
    h.editor.doc.description = "x".repeat(7500);
    let diff = handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":1})));
    assert!(diff.ok, "{diff:?}");
    let combined = handle(&mut s, &mut h, req("describe", json!({"board":"b1","since":1,"fields":["metadata"]})));
    assert_eq!(
        combined.error.as_ref().map(|e| e.code.as_str()),
        Some("resync_required"),
        "diff bytes={}, combined={combined:?}",
        json!(diff).to_string().len()
    );
    let anchor = h.editor.doc.paths[0].anchors[0].clone();
    for i in 0..180 {
        let mut anchor = anchor.clone();
        anchor.id = 100 + i;
        h.editor.doc.paths[0].anchors.push(anchor);
    }
    h.editor.execute_ui(EditCommand::SetBoardName("Geometry".into()));
    for args in [json!({"board":"b1","since":1}), json!({"board":"b1","ids":["path:10"],"fields":["geometry"]})] {
        let reply = handle(&mut s, &mut h, req("describe", args));
        assert!(reply.ok, "{reply:?}");
    }
    let combined = handle(
        &mut s,
        &mut h,
        req("describe", json!({"board":"b1","since":1,"ids":["path:10"],"fields":["geometry"]})),
    );
    assert_eq!(combined.error.unwrap().code, "resync_required");
    h.editor.doc.artboards.extend((0..120).map(|_| Default::default()));
    h.editor.execute_ui(EditCommand::SetBoardName("Many pages".into()));
    let combined = handle(&mut s, &mut h, req("describe", json!({"board":"b1","fields":["artboards","selection"]})));
    let error = combined.error.unwrap();
    assert_eq!(error.code, "limit_exceeded");
    for fields in ["fields:[\"selection\"]", "fields:[\"artboards\"]"] {
        assert!(error.reason.contains(fields));
    }
}

fn economy_args(ops: Value) -> Value {
    json!({"api":"1.1","board":"b1","request_id":"r1","expected_rev":1,"ops":ops,"receipt":"ids"})
}

#[test]
fn economy_wireframe_byte_gate_exact_document_equivalence_and_receipts() {
    let original: Vec<Value> = serde_json::from_str(include_str!("fixtures/wireframe-source-1.1.json")).unwrap();
    let encoded: Vec<Value> = serde_json::from_str(include_str!("fixtures/wireframe-request-1.1.json")).unwrap();
    assert_eq!(encoded.iter().map(|r| serde_json::to_vec(r).unwrap().len()).sum::<usize>(), 8188);
    let bytes: usize = encoded
        .iter()
        .map(|r| serde_json::to_vec(&json!({"defaults":r["defaults"],"ops":r["ops"]})).unwrap().len())
        .sum();
    assert_eq!(bytes, 7960);
    assert!(bytes <= 9364);
    assert_eq!(original.iter().map(|b| b.as_array().unwrap().len()).sum::<usize>(), 201);
    let mut old = FakeHost::new();
    old.editor = Editor::new();
    let mut new = FakeHost::new();
    new.editor = Editor::new();
    let mut old_service = Service::new("test-epoch".into());
    let mut new_service = Service::new("test-epoch".into());
    let mut receipts = vec![];
    let mut states = vec![];
    let mut names = vec![];
    for (i, (ops, args)) in original.iter().zip(&encoded).enumerate() {
        let baseline = json!({"api":"1.0","board":"b1","request_id":format!("r{}",i+1),"expected_rev":i,"ops":ops});
        let before = new.editor.doc.clone();
        let a = handle(&mut old_service, &mut old, req("edit", baseline));
        assert!(a.ok, "{a:?}");
        let cli =
            varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":"edit","arguments":args})).unwrap()).unwrap();
        let b = handle(&mut new_service, &mut new, cli);
        assert!(b.ok, "{b:?}");
        assert_eq!(b.undo_steps, 1);
        let after = new.editor.doc.clone();
        let retry = handle(&mut new_service, &mut new, req("edit", args.clone()));
        assert_eq!(retry, b);
        assert_eq!(new.editor.doc, after);
        collect_creation_names(&args["ops"], &mut names);
        assert_eq!(names.len(), after.paths.len());
        let mut normalized = after.clone();
        for ((p, expected), explicit) in normalized.paths.iter_mut().zip(&old.editor.doc.paths).zip(&names) {
            if let Some(name) = explicit {
                assert_eq!(p.name.as_ref(), Some(name));
            } else {
                p.name = expected.name.clone();
            }
        }
        assert_eq!(normalized, old.editor.doc, "geometry/paint/tree/order/IDs differ in batch {i}");
        states.push((before, after));
        receipts.push(json!(b));
    }
    for (before, _) in states.iter().rev() {
        new.editor.undo();
        assert_eq!(&new.editor.doc, before);
    }
    for (_, after) in &states {
        new.editor.redo();
        assert_eq!(&new.editor.doc, after);
    }
    assert_eq!(
        json!(receipts),
        serde_json::from_str::<Value>(include_str!("fixtures/wireframe-receipt-1.1.json")).unwrap()
    );
}

#[test]
fn economy_defaults_tuples_ids_retry_and_batch_local_names() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    h.editor.objsel.insert(10);
    h.editor.cur_fill = Some([0.2, 0.3, 0.4, 1.]);
    h.editor.cur_stroke = Some([0.4, 0.3, 0.2, 1.]);
    h.editor.cur_sw = 7.;
    let human = (h.editor.cur_fill, h.editor.cur_stroke, h.editor.cur_sw, h.editor.objsel.clone());
    let mut args = economy_args(json!([
        ["rect",[1,2,20,30],{"local":"$a"}],
        ["ellipse",[30,2,20,30],null,{"stroke":"#FF0000FF","stroke_width":2}],
        ["path",[[0,0],[10,20]],false]
    ]));
    args["defaults"] = json!({"parent":"node:1","fill":"#112233FF","stroke":null,"radius":2,"opacity":0.5});
    let before = h.editor.doc.clone();
    let r = handle(&mut s, &mut h, req("edit", args.clone()));
    assert!(r.ok, "{r:?}");
    assert_eq!((h.editor.cur_fill, h.editor.cur_stroke, h.editor.cur_sw, h.editor.objsel.clone()), human);
    let result = r.result.as_ref().unwrap();
    assert!(result["created"].as_array().unwrap().iter().all(Value::is_string));
    assert!(!result.to_string().contains("bounds"));
    let paths = &h.editor.doc.paths[2..];
    assert!(paths[0].name.as_ref().unwrap().starts_with("Rect "));
    assert!(paths[1].name.as_ref().unwrap().starts_with("Ellipse "));
    assert!(paths[2].name.as_ref().unwrap().starts_with("Path "));
    assert_eq!(paths[1].fill, Paint::None);
    let after = h.editor.doc.clone();
    assert_eq!(handle(&mut s, &mut h, req("edit", args.clone())), r);
    args["ops"][0] = json!({"verb":"add_shape","kind":"rect","bounds":[1,2,20,30],"local":"$a"});
    let conflict = handle(&mut s, &mut h, req("edit", args));
    assert!(!conflict.ok);
    h.editor.undo();
    assert_eq!(h.editor.doc, before);
    h.editor.redo();
    assert_eq!(h.editor.doc, after);
    let rev = h.editor.rev;
    let fail = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.1","board":"b1","request_id":"r2","expected_rev":rev,"ops":[["rect",[0,0,10,10]]]}),
        ),
    );
    assert!(!fail.ok);
    assert_eq!(h.editor.doc, after);
}

#[test]
fn economy_rejects_bad_tuple_defaults_and_version_before_execution() {
    for ops in [
        json!([["rect",[0,0,2,2],"#FFFFFFFF",1,{"fill":"#FFFFFFFF"}]]),
        json!([["ellipse", [0, 0, 2, 2], "#FFFFFFFF", 1]]),
        json!([["rect",[0,0,2,2],{"radius":null}]]),
        json!([["path",[[0,0],[1,1]],true,{"holes":[]}]]),
        json!([["rect",[0,0,2,2],{"kind":"ellipse"}]]),
        json!([["rect", [0, 0, 2, 2], 7]]),
        json!([["rect", [0, 0, 2, 2], null, 1, {}, 0]]),
    ] {
        assert!(varos_bridge::mcp::decode_tool("edit", economy_args(ops)).is_err());
    }
    for defaults in [
        json!({"parent":null}),
        json!({"radius":null}),
        json!({"opacity":null}),
        json!({"stroke_width":null}),
        json!({"opacity":2}),
        json!({"fill":"red"}),
        json!({"name":"no"}),
    ] {
        let mut args = economy_args(json!([["rect", [0, 0, 10, 10], "#FFFFFFFF"]]));
        args["defaults"] = defaults;
        assert!(varos_bridge::mcp::decode_tool("edit", args).is_err());
    }
    for extra in [json!({"defaults":{}}), json!({"receipt":"ids"})] {
        let mut args = economy_args(json!([]));
        args["api"] = json!("1.0");
        args.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        assert!(varos_bridge::mcp::decode_tool("edit", args).is_err());
    }
    let mut args = economy_args(json!([["rect", [0, 0, 10, 10], "#FFFFFFFF"]]));
    args["api"] = json!("1.0");
    args.as_object_mut().unwrap().remove("receipt");
    assert!(varos_bridge::mcp::decode_tool("edit", args).is_err());
}

#[test]
fn economy_nested_repeat_instance_order_locals_handles_and_rollback() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let args = economy_args(
        json!([{"verb":"repeat","count":2,"dx":100,"dy":0,"ops":[{"verb":"repeat","count":2,"dx":0,"dy":50,"ops":[{"verb":"add_path","anchors":[{"p":[1,2],"hout":[3,4]},{"p":[5,6],"hin":[7,8]}],"closed":false,"fill":"#FFFFFFFF","local":"$p"}]}]}]),
    );
    let r = handle(&mut s, &mut h, req("edit", args));
    assert!(r.ok, "{r:?}");
    let locals = &r.result.as_ref().unwrap()["locals"];
    for (i, (x, y)) in [(1., 2.), (1., 52.), (101., 2.), (101., 52.)].iter().enumerate() {
        let p = &h.editor.doc.paths[2 + i];
        assert_eq!(p.anchors[0].p, [*x, *y]);
        assert_eq!(p.anchors[0].hout, Some([*x + 2., *y + 2.]));
        assert_eq!(locals[format!("$p_{}_{}", i / 2, i % 2)], format!("path:{}", p.id));
    }
    let before = h.editor.doc.clone();
    let rev = h.editor.rev;
    let mut bad = economy_args(
        json!([{"verb":"repeat","count":2,"dx":0,"dy":0,"ops":[["rect",[0,0,10,10],"#FFFFFFFF"],{"verb":"repeat","count":1,"dx":0,"dy":0,"ops":[["ellipse",[0,0,-1,10],"#FFFFFFFF"]]}]}]),
    );
    bad["request_id"] = json!("r2");
    bad["expected_rev"] = json!(rev);
    let r = handle(&mut s, &mut h, req("edit", bad));
    assert!(!r.ok);
    let e = r.error.unwrap();
    assert_eq!(e.op_index, Some(0));
    assert_eq!(e.location, vec!["instance:0", "op:1", "instance:0", "op:0"]);
    assert_eq!(h.editor.doc, before);
    assert_eq!(h.editor.rev, rev);
    // Failed allocation did not consume lifetime IDs.
    let mut good = economy_args(json!([["rect", [0, 0, 10, 10], "#FFFFFFFF"]]));
    good["request_id"] = json!("r2");
    good["expected_rev"] = json!(rev);
    let mut control = Editor::new();
    control.replace_doc(before.clone());
    let expected = control
        .try_execute_created(EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Rect,
            bounds: [0., 0., 10., 10.],
            parent: None,
            fill: Some([1.; 4]),
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    let r = handle(&mut s, &mut h, req("edit", good));
    assert!(r.ok, "{r:?}");
    assert_eq!(h.editor.doc.paths.last().unwrap().id, expected);
}

#[test]
fn economy_repeat_bounds_collisions_forward_refs_and_cancellation() {
    let leaf = json!(["rect",[0,0,10,10],"#FFFFFFFF",{"local":"$x"}]);
    let repeat = |count, ops| json!({"verb":"repeat","count":count,"dx":0,"dy":0,"ops":ops});
    for count in [0, 101] {
        assert!(varos_bridge::mcp::decode_tool("edit", economy_args(json!([repeat(count, json!([leaf]))]))).is_err());
    }
    let mut nested = leaf.clone();
    for _ in 0..5 {
        nested = repeat(1, json!([nested]));
    }
    assert!(varos_bridge::mcp::decode_tool("edit", economy_args(json!([nested]))).is_err());
    let four = repeat(1, json!([repeat(1, json!([repeat(1, json!([repeat(1, json!([leaf]))]))]))]));
    assert!(varos_bridge::mcp::decode_tool("edit", economy_args(json!([four]))).is_ok());
    let overflow = economy_args(json!([repeat(100, json!([leaf])), ["ellipse", [0, 0, 10, 10], "#FFFFFFFF"]]));
    let error = varos_bridge::mcp::decode_tool("edit", overflow).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    for ops in [
        json!([repeat(1, json!([{"verb":"move","ids":["path:10"],"delta":[1,1]}]))]),
        json!([repeat(1, json!([{"verb":"add_artboard","bounds":[0,0,10,10]}]))]),
    ] {
        assert!(varos_bridge::mcp::decode_tool("edit", economy_args(ops)).is_err());
    }
    for ops in [
        json!([repeat(1,json!([leaf])),["rect",[0,0,10,10],"#FFFFFFFF",{"local":"$x_0"}]]),
        json!([repeat(1, json!([["rect",[0,0,10,10],"#FFFFFFFF",{"local":format!("${}","x".repeat(62))}]]))]),
        json!([{"verb":"move","ids":["$x_0"],"delta":[1,1]},repeat(1,json!([leaf]))]),
        json!([repeat(1,json!([leaf])),{"verb":"move","ids":["$x"],"delta":[1,1]}]),
    ] {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        let before = h.editor.doc.clone();
        let rev = h.editor.rev;
        let r = handle(&mut s, &mut h, req("edit", economy_args(ops)));
        assert!(!r.ok, "{r:?}");
        assert_eq!(h.editor.doc, before);
        assert_eq!(h.editor.rev, rev);
    }
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let before = h.editor.doc.clone();
    let args = economy_args(json!([repeat(100, json!([leaf]))]));
    let r = s.handle(&mut h, &ctx(), req("edit", args.clone()), &AtomicBool::new(true));
    assert!(!r.ok);
    assert_eq!(h.editor.doc, before);
    let r = handle(&mut s, &mut h, req("edit", args));
    assert!(r.ok, "{r:?}");
    assert_eq!(h.editor.doc.paths.len(), 102);
    assert_eq!(r.result.unwrap()["locals"].as_object().unwrap().len(), 100);
}

#[test]
fn economy_snapshot_summary_hint_and_revision_pinned_detail() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    h.editor.doc.name = "ع".repeat(200);
    h.editor.objsel.insert(10);
    let r = handle(&mut s, &mut h, req("describe", json!({"api":"1.1","board":"b1","summary_budget":1024})));
    assert!(r.ok, "{r:?}");
    assert!(serde_json::to_vec(r.result.as_ref().unwrap()).unwrap().len() <= 1024);
    assert!(varos_bridge::service::compact(&r).len() <= 1024);
    assert!(r.result.as_ref().unwrap()["counts"].is_object());
    assert_eq!(r.result.as_ref().unwrap()["selection_count"], 1);
    let trimmed = handle(&mut s, &mut h, req("describe", json!({"api":"1.1","board":"b1","summary_budget":512})));
    assert!(trimmed.ok, "{trimmed:?}");
    assert!(trimmed.result.unwrap()["name"].as_str().unwrap().ends_with("…"));
    let mut detail = r.result.unwrap()["detail_request"].clone();
    detail["api"] = json!("1.1");
    let d = handle(&mut s, &mut h, req("describe", detail));
    assert!(d.ok, "{d:?}");
    assert_eq!(d.result.unwrap()["objects"].as_array().unwrap().len(), 3);
    let snapshot = |api, profile| json!({"api":api,"board":"b1","rev":1,"profile":profile});
    let before = h.editor.doc.clone();
    let r = handle(&mut s, &mut h, req("snapshot", snapshot("1.1", "economy")));
    assert!(r.ok, "{r:?}");
    assert_eq!(r.result.as_ref().unwrap()["width"], 512);
    assert_eq!(r.result.unwrap()["height"], 232);
    let r = handle(&mut s, &mut h, req("snapshot", snapshot("1.0", "economy")));
    assert!(!r.ok);
    let r = handle(&mut s, &mut h, req("describe", json!({"board":"b1","summary_budget":1024})));
    assert!(!r.ok);
    assert_eq!(h.editor.doc, before);
    let cap = handle(&mut s, &mut h, req("capabilities", json!({"api":"1.1"})));
    let cap = cap.result.unwrap();
    assert_eq!(cap["supported_api"], json!(["1.0", "1.1"]), "1.1 capabilities stay byte-stable");
    assert_eq!(cap["api_by_tool"]["export_pdf"], json!(["1.0", "1.1", "1.2"]));
    assert!(cap["economy_hint"].as_str().unwrap().contains("repeat"));
    assert!(!cap["economy_hint"].as_str().unwrap().contains("bars"));
    let old = handle(&mut s, &mut h, req("capabilities", json!({})));
    assert!(old.result.unwrap().get("economy_hint").is_none());
}

#[test]
fn economy_targets_preallocation_limit_and_large_ids_pagination() {
    let mut h = FakeHost::new();
    h.editor = Editor::new();
    let template = h
        .editor
        .try_execute_created(EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Rect,
            bounds: [0., 0., 10., 10.],
            parent: None,
            fill: Some([1.; 4]),
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    // Seed 1,000 validated independent paths with large identities to exercise receipt paging.
    let mut doc = h.editor.doc.clone();
    let source = doc.paths.iter().find(|p| p.id == template).unwrap().clone();
    doc.paths.clear();
    doc.nodes.retain(|n| n.kind == varos_core::model::NodeKind::Layer);
    doc.nodes[0].children.clear();
    for i in 0..1000u32 {
        let mut p = source.clone();
        p.id = 100_000_000 + i * 10;
        for (j, a) in p.anchors.iter_mut().enumerate() {
            a.id = p.id + j as u32 + 1;
        }
        doc.paths.push(p);
    }
    doc.ids = 100_010_000;
    h.editor.replace_doc(doc);
    let mut s = Service::new("test-epoch".into());
    let rev = h.editor.rev;
    let before = h.editor.doc.clone();

    let args = json!({"api":"1.1","board":"b1","request_id":"r1","expected_rev":rev,"receipt":"ids","ops":[["rect",[0,0,10,10],"#FFFFFFFF"],{"verb":"set_paint","ids":["node:1"],"fill":"#FF0000FF"}]});
    let r = handle(&mut s, &mut h, req("edit", args));
    assert!(!r.ok);
    assert_eq!(r.error.unwrap().code, "limit_exceeded");
    assert_eq!(h.editor.doc, before);
    assert_eq!(h.editor.doc.ids, before.ids);
    let ids: Vec<_> = h.editor.doc.paths.iter().map(|p| format!("path:{}", p.id)).collect();
    let args = json!({"api":"1.1","board":"b1","request_id":"r1","expected_rev":rev,"receipt":"ids","ops":[{"verb":"set_paint","ids":ids,"fill":"#FF0000FF"}]});
    let first = handle(&mut s, &mut h, req("edit", args.clone()));
    assert!(first.ok, "{first:?}");
    assert_eq!(first.result.as_ref().unwrap()["more"], true);
    assert_eq!(handle(&mut s, &mut h, req("edit", args)), first);
    let status = handle(&mut s, &mut h, req("request_status", json!({"api":"1.1","request_id":"r1"})));
    assert!(status.ok, "{status:?}");
    assert_eq!(status.result.unwrap()["receipt"], json!(first));
    let mut all = vec![];
    let mut page = first.result.clone().unwrap();
    let first_cursor = page["cursor"].clone();
    loop {
        all.extend(page["changed"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned));
        assert!(serde_json::to_vec(&page).unwrap().len() < 16_384);
        if page["more"] != true {
            break;
        }
        let r = handle(
            &mut s,
            &mut h,
            req("request_status", json!({"api":"1.1","request_id":"r1","cursor":page["cursor"]})),
        );
        assert!(r.ok, "{r:?}");
        page = r.result.unwrap()["receipt"]["result"].clone();
    }
    assert_eq!(all.len(), 1000);
    assert_eq!(all.iter().collect::<std::collections::BTreeSet<_>>().len(), 1000);
    h.editor.undo();
    let expired =
        handle(&mut s, &mut h, req("request_status", json!({"api":"1.1","request_id":"r1","cursor":first_cursor})));
    assert!(!expired.ok);
}

#[test]
fn economy_page_snapshot_fit_and_explicit_dimensions() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let r = handle(
        &mut s,
        &mut h,
        req("edit", economy_args(json!([{"verb":"add_artboard","bounds":[0,0,1440,3000],"local":"$page"}]))),
    );
    assert!(r.ok, "{r:?}");
    let page = r.result.unwrap()["locals"]["$page"].clone();
    let rev = h.editor.rev;
    for (explicit, w, height) in [(false, 246, 512), (true, 492, 1024)] {
        let mut args = json!({"api":"1.1","board":"b1","rev":rev,"artboard":page,"profile":"economy"});
        if explicit {
            args["width"] = json!(1024);
            args["height"] = json!(1024);
        }
        let r = handle(&mut s, &mut h, req("snapshot", args));
        assert!(r.ok, "{r:?}");
        let r = r.result.unwrap();
        assert_eq!(r["width"], w);
        assert_eq!(r["height"], height);
    }
}

#[test]
fn economy_actual_mcp_stdio_matches_cli_and_bounded_schema() {
    let args =
        economy_args(json!([{"verb":"repeat","count":2,"dx":20,"dy":0,"ops":[["rect",[0,0,10,10],{"local":"$a"}]]}]));
    let mut args = args;
    args["defaults"] = json!({"fill":"#FFFFFFFF"});
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let cli =
        varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":"edit","arguments":args})).unwrap()).unwrap();
    let expected = handle(&mut s, &mut h, cli);
    let transport = FakeTransport {
        state: std::sync::Arc::new(std::sync::Mutex::new((Service::new("test-epoch".into()), FakeHost::new()))),
        cancellations: Default::default(),
    };
    let (input_tx, input) = std::sync::mpsc::channel();
    let (output, receive) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        varos_bridge::mcp::serve(
            &mut std::io::BufReader::new(ChannelRead { rx: input, current: std::io::Cursor::new(vec![]) }),
            ChannelWrite { tx: output, bytes: vec![] },
            transport,
        )
        .unwrap()
    });
    for msg in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{},"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"edit","arguments":args}}),
    ] {
        let mut bytes = serde_json::to_vec(&msg).unwrap();
        bytes.push(b'\n');
        input_tx.send(bytes).unwrap();
    }
    receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
    let result: Value =
        serde_json::from_slice(&receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap()).unwrap();
    assert_eq!(result["result"], varos_bridge::mcp::tool_result(&expected));
    drop(input_tx);
    server.join().unwrap();
    let schema = varos_bridge::mcp::tools();
    let edit = schema["tools"].as_array().unwrap().iter().find(|t| t["name"] == "edit").unwrap();
    let schema = &edit["inputSchema"];
    assert_eq!(schema["properties"]["api"]["enum"], json!(["1.0", "1.1", "1.2"]));
    assert!(schema["$defs"].get("repeat4").is_none());
    assert_eq!(schema["$defs"]["repeat3"]["properties"]["ops"]["items"]["anyOf"].as_array().unwrap().len(), 1);
}

#[test]
fn economy_preserves_api_10_request_projection_and_retry_normalization() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let mut args = json!({"api":"1.0","board":"b1","request_id":"r1","expected_rev":1,"ops":[{"verb":"add_shape","kind":"rect","bounds":[0,0,10,10],"fill":"#FFFFFFFF","parent":null}]});
    let request = req("edit", args.clone());
    let projected = serde_json::to_value(&request).unwrap();
    assert!(projected["arguments"]["ops"][0].get("parent").is_none());
    assert_eq!(projected["arguments"]["ops"][0]["bounds"].to_string(), "[0.0,0.0,10.0,10.0]");
    let r = handle(&mut s, &mut h, request);
    assert!(r.ok, "{r:?}");
    args["ops"][0].as_object_mut().unwrap().remove("parent");
    assert_eq!(handle(&mut s, &mut h, req("edit", args)), r);
}

fn collect_creation_names(ops: &Value, out: &mut Vec<Option<String>>) {
    for op in ops.as_array().unwrap() {
        if op["verb"] == "repeat" {
            for _ in 0..op["count"].as_u64().unwrap() {
                collect_creation_names(&op["ops"], out);
            }
        } else if op.is_array() {
            out.push(
                op.as_array().unwrap().last().and_then(|v| v.get("name")).and_then(Value::as_str).map(str::to_owned),
            );
        } else if matches!(op["verb"].as_str(), Some("add_shape" | "add_path")) {
            out.push(op["name"].as_str().map(str::to_owned));
        }
    }
}

#[test]
fn economy_schema_size_and_flat_roots() {
    for api in ["1.0", "1.1", "1.2"] {
        let list = varos_bridge::mcp::tools_for_api(api);
        let wire = serde_json::to_vec(&list).unwrap();
        match api {
            "1.0" => assert_eq!(wire.as_slice(), include_bytes!("fixtures/mcp_tools_list_1_0.json")),
            "1.1" => assert_eq!(wire.as_slice(), include_bytes!("fixtures/mcp_tools_list_1_1.json")),
            _ => {}
        }
        let bytes = wire.len();
        println!("API {api} tools/list bytes: {bytes}");
        assert!(bytes <= 24_000, "API {api} tools/list grew to {bytes} bytes");
        for tool in list["tools"].as_array().unwrap() {
            let root = &tool["inputSchema"];
            assert_eq!(root["type"], "object");
            for key in ["oneOf", "anyOf", "allOf"] {
                assert!(root.get(key).is_none(), "{} root {key}", tool["name"]);
            }
        }
    }
    let list = varos_bridge::mcp::tools();
    let edit = list["tools"].as_array().unwrap().iter().find(|t| t["name"] == "edit").unwrap();
    for kind in ["rect", "ellipse", "path"] {
        let tuple = &edit["inputSchema"]["$defs"][format!("{kind}_tuple")];
        assert!(tuple["prefixItems"].is_array());
        assert!(tuple.get("items").is_none());
    }
    assert!(edit["description"].as_str().unwrap().contains("object operations"));
}

#[test]
fn economy_legacy_target_count_and_alias_index() {
    let shape = json!({"verb":"add_shape","kind":"rect","bounds":[0,0,10,10],"fill":"#FFFFFFFF"});
    let ids = vec!["path:1"; 1000];
    let args = json!({"api":"1.0","board":"b1","request_id":"r1","expected_rev":0,"ops":[shape,{"verb":"move","ids":ids,"delta":[1,0]}]});
    assert!(varos_bridge::mcp::decode_tool("edit", args.clone()).is_ok());
    let mut bad = args;
    bad["ops"][1]["ids"].as_array_mut().unwrap().push(json!("path:1"));
    let e = varos_bridge::mcp::decode_tool("edit", bad).unwrap_err();
    assert_eq!(e.reason, "edit exceeds 1000 explicit targets");
    assert_eq!(e.op_index, None);
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let mut args = economy_args(json!([
        {"verb":"repeat","count":3,"dx":20,"dy":0,"ops":[["rect",[0,0,10,10],"#FFFFFFFF"]]},
        {"verb":"align","ids":["path:1"],"mode":"left","target":"a0@0"},
        {"verb":"add_artboard","bounds":[0,0,100,100]}
    ]));
    args["expected_rev"] = json!(h.editor.rev);
    let r = handle(&mut s, &mut h, req("edit", args));
    assert_eq!(r.error.unwrap().op_index, Some(1));
}

#[test]
fn clipping_api_12_matches_core_commands_and_old_apis_refuse_it() {
    for api in ["1.0", "1.1"] {
        let mut h = FakeHost::new();
        let before = h.editor.doc.clone();
        let mut s = Service::new("test-epoch".into());
        let r = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":api,"request_id":"r1","board":"b1","expected_rev":1,
            "ops":[{"verb":"clip","ids":["path:10","path:20"]}]}),
            ),
        );
        assert!(!r.ok);
        assert_eq!(r.error.unwrap().code, "unsupported");
        assert_eq!(h.editor.doc, before);
    }
    let mut h = FakeHost::new();
    let mut core = h.editor.clone();
    core.try_execute(EditCommand::SelectPaths(vec![10, 20])).unwrap();
    core.try_execute(EditCommand::ClipMake).unwrap();
    let mut s = Service::new("test-epoch".into());
    let args = json!({"api":"1.2","request_id":"r1","board":"b1","expected_rev":1,
        "ops":[{"verb":"clip","ids":["path:10","path:20"]}]});
    // CLI and MCP decode the same verb before the common service boundary.
    let cli =
        varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":"edit","arguments":args})).unwrap()).unwrap();
    let result = handle(&mut s, &mut h, cli);
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(h.editor.doc, core.doc);
    assert_eq!(h.editor.rev, 2);
    let gid = h.editor.doc.top_group_of_path(10).unwrap();
    core.try_execute(EditCommand::ClipRelease).unwrap();
    let release = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","request_id":"r2","board":"b1","expected_rev":2,
        "ops":[{"verb":"release_clip","ids":[format!("node:{gid}")]}]}),
        ),
    );
    assert!(release.ok, "{:?}", release.error);
    assert_eq!(h.editor.doc, core.doc);
    assert_eq!(h.editor.rev, 3);
    h.editor.execute(EditCommand::Undo).unwrap();
    assert_eq!(h.editor.doc.node(gid).unwrap().role, varos_core::model::GroupRole::Clip);
    h.editor.execute(EditCommand::Undo).unwrap();
    assert_eq!(h.editor.doc, FakeHost::new().editor.doc);
}

#[test]
fn api_12_discovery_is_opt_in_and_old_tables_stay_identical() {
    assert_eq!(varos_bridge::mcp::tools_for_api("1.0"), varos_bridge::mcp::tools());
    assert_eq!(varos_bridge::mcp::tools_for_api("1.1"), varos_bridge::mcp::tools());
    let table = varos_bridge::mcp::tools_for_api("1.2");
    let edit = table["tools"].as_array().unwrap().iter().find(|t| t["name"] == "edit").unwrap();
    assert_eq!(edit["inputSchema"]["properties"]["api"]["enum"], json!(["1.0", "1.1", "1.2"]));
    assert!(edit["inputSchema"]["$defs"]["operation"].to_string().contains("release_clip"));
    let mut host = FakeHost::new();
    let mut service = Service::new("test-epoch".into());
    let reply = handle(&mut service, &mut host, req("capabilities", json!({"api":"1.2"})));
    assert!(reply.ok);
    let capabilities = reply.result.unwrap();
    assert_eq!(capabilities["api_by_tool"]["select"], json!(["1.0", "1.1", "1.2"]));
    let verbs = capabilities["edit_verbs"].as_array().unwrap();
    for verb in [
        "repeat",
        "clip",
        "release_clip",
        "view",
        "object",
        "distribute_mode",
        "distribute_spacing",
        "anchor_type",
        "insert_anchor",
        "delete_anchor",
    ] {
        assert_eq!(verbs.iter().filter(|v| **v == json!(verb)).count(), 1, "{verb}");
    }
}

#[test]
fn command_wave_select_modes_are_12_only_and_legacy_serialization_is_unchanged() {
    for api in ["1.0", "1.1", "1.2"] {
        let mut s = Service::new("test-epoch".into());
        let mut h = FakeHost::new();
        let reply = handle(
            &mut s,
            &mut h,
            req("select", json!({"api":api,"request_id":"r1","board":"b1","expected_rev":1,"ids":[],"mode":"all"})),
        );
        assert_eq!(reply.error.is_none(), api == "1.2", "{api}: {reply:?}");
        assert_eq!(h.editor.objsel.len(), if api == "1.2" { 2 } else { 0 });
        assert_eq!(h.editor.rev, 1);
    }
    let legacy = json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":1,"ids":["path:10"]});
    let parsed: varos_bridge::dto::Select = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), legacy);
}
#[test]
fn command_wave_legacy_edits_refuse_without_mutation() {
    for api in ["1.0", "1.1"] {
        let mut s = Service::new("test-epoch".into());
        let mut h = FakeHost::new();
        let before = h.editor.doc.clone();
        let reply = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":api,"request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"object","ids":["path:10"],"action":"lock"}]}),
            ),
        );
        assert_eq!(reply.error.unwrap().code, "unsupported");
        assert_eq!(h.editor.doc, before);
    }
}
#[test]
fn command_wave_12_lock_hide_unlock_show_are_undoable() {
    for (action, global, flag) in [("lock", "unlock_all", true), ("hide", "show_all", false)] {
        let mut s = Service::new("test-epoch".into());
        let mut h = FakeHost::new();
        let first = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":"1.2","request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"object","ids":["path:10"],"action":action}]}),
            ),
        );
        assert!(first.error.is_none(), "{first:?}");
        assert_eq!(h.editor.rev, 2);
        assert!(if flag { h.editor.doc.paths[0].locked } else { h.editor.doc.paths[0].hidden });
        let second = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":"1.2","request_id":"r2","board":"b1","expected_rev":2,"ops":[{"verb":"object","ids":[],"action":global}]}),
            ),
        );
        assert!(second.error.is_none(), "{second:?}");
        assert!(!h.editor.doc.paths[0].locked && !h.editor.doc.paths[0].hidden);
        h.editor.execute(EditCommand::Undo).unwrap();
        assert!(if flag { h.editor.doc.paths[0].locked } else { h.editor.doc.paths[0].hidden });
    }
}
#[test]
fn command_wave_12_failed_batch_rolls_back_geometry_and_flags() {
    let mut s = Service::new("test-epoch".into());
    let mut h = FakeHost::new();
    let before = h.editor.doc.clone();
    let reply = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"object","ids":["path:10"],"action":"reverse"},{"verb":"object","ids":["path:999"],"action":"lock"}]}),
        ),
    );
    assert!(reply.error.is_some());
    assert_eq!(h.editor.doc, before);
    assert_eq!(h.editor.rev, 1);
}
#[test]
fn command_wave_12_average_uses_explicit_anchor_targets() {
    let mut s = Service::new("test-epoch".into());
    let mut h = FakeHost::new();
    let reply = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"object","ids":["path:10"],"action":"average","anchors":[11,12]}]}),
        ),
    );
    assert!(reply.error.is_none(), "{reply:?}");
    assert_eq!(h.editor.doc.paths[0].anchors[0].p, [60.0, 20.0]);
    assert_eq!(h.editor.doc.paths[0].anchors[1].p, [60.0, 20.0]);
}
#[test]
fn command_wave_12_key_object_selection_and_layer_creation_are_real() {
    let mut s = Service::new("test-epoch".into());
    let mut h = FakeHost::new();
    let reply = handle(
        &mut s,
        &mut h,
        req(
            "select",
            json!({"api":"1.2","request_id":"r1","board":"b1","expected_rev":1,"ids":["path:10","path:20"],"mode":{"key_object":10}}),
        ),
    );
    assert!(reply.error.is_none(), "{reply:?}");
    assert_eq!(h.editor.key_object, Some(10));
    let key = h.editor.doc.paths[0].clone();
    let reply = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","request_id":"r2","board":"b1","expected_rev":1,"ops":[{"verb":"align","ids":["path:10","path:20"],"mode":"left","target":"key_object"}]}),
        ),
    );
    assert!(reply.error.is_none(), "{reply:?}");
    assert_eq!(h.editor.doc.paths[0], key);
    let count = h.editor.doc.roots.len();
    let rev = h.editor.rev;
    let reply = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","request_id":"r3","board":"b1","expected_rev":rev,"ops":[{"verb":"object","ids":[],"action":"new_layer"}]}),
        ),
    );
    assert!(reply.error.is_none(), "{reply:?}");
    assert_eq!(h.editor.doc.roots.len(), count + 1);
}
#[test]
fn command_wave_lasso_anchor_edits_and_paste_setting_are_opt_in() {
    for api in ["1.0", "1.1", "1.2"] {
        let mut s = Service::new("test-epoch".into());
        let mut h = FakeHost::new();
        let r = handle(
            &mut s,
            &mut h,
            req(
                "select",
                json!({"api":api,"request_id":"r1","board":"b1","expected_rev":1,"ids":[],"paste_remembers_layers":true}),
            ),
        );
        assert_eq!(r.error.is_none(), api == "1.2", "{r:?}");
        assert_eq!(h.editor.paste_remembers_layers, api == "1.2");
        let r = handle(
            &mut s,
            &mut h,
            req(
                "select",
                json!({"api":api,"request_id":"r2","board":"b1","expected_rev":1,"ids":[],"lasso":{"points":[[0,0],[150,0],[150,150],[0,150]],"objects":false,"additive":false}}),
            ),
        );
        assert_eq!(r.error.is_none(), api == "1.2", "{r:?}");
        assert_eq!(h.editor.selected.is_empty(), api != "1.2");
        let r = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":api,"request_id":"r3","board":"b1","expected_rev":1,"ops":[{"verb":"insert_anchor","ids":["path:10"],"segment":0,"t":0.5}]}),
            ),
        );
        assert_eq!(r.error.is_none(), api == "1.2", "{r:?}");
        if api == "1.2" {
            let id = h.editor.doc.paths[0].anchors[1].id;
            let revision = h.editor.rev;
            let r = handle(
                &mut s,
                &mut h,
                req(
                    "edit",
                    json!({"api":api,"request_id":"r4","board":"b1","expected_rev":revision,"ops":[{"verb":"delete_anchor","ids":["path:10"],"anchor":id}]}),
                ),
            );
            assert!(r.error.is_none(), "{r:?}");
            assert_eq!(h.editor.doc.paths[0].anchors.len(), 4);
        }
    }
}
#[test]
fn view_quick_wins_are_12_only_and_document_changes_are_real() {
    for api in ["1.0", "1.1", "1.2"] {
        let mut s = Service::new("test-epoch".into());
        let mut h = FakeHost::new();
        let r = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":api,"request_id":"r1","board":"b1","expected_rev":1,"ops":[{"verb":"view","ids":["path:10"],"action":"make_guides"}]}),
            ),
        );
        assert_eq!(r.error.is_none(), api == "1.2", "{r:?}");
        assert_eq!(h.editor.doc.guide_paths.len(), usize::from(api == "1.2"));
        let rev = h.editor.rev;
        let r = handle(
            &mut s,
            &mut h,
            req(
                "edit",
                json!({"api":api,"request_id":"r2","board":"b1","expected_rev":rev,"ops":[{"verb":"view","ids":[],"action":{"grid":{"spacing":24.0,"subdivisions":3}}},{"verb":"view","ids":[],"action":"toggle_grid"}]}),
            ),
        );
        assert_eq!(r.error.is_none(), api == "1.2", "{r:?}");
        if api == "1.2" {
            assert_eq!(h.editor.document_grid_step(), 8.0);
            assert!(!h.editor.doc.snap.show_grid);
            // The service journals persisted preferences even though core undo omits them.
            assert_eq!(h.editor.rev, rev + 1);
        }
    }
}

#[test]
fn phase_one_effects_are_opt_in_revision_pinned_and_idempotent_without_os_calls() {
    struct Effects {
        host: FakeHost,
        calls: Vec<(String, Option<Value>)>,
    }
    impl Host for Effects {
        fn prepare(&mut self, b: &str, m: bool) -> Result<(), Error> {
            self.host.prepare(b, m)
        }
        fn boards(&self) -> Vec<BoardInfo> {
            self.host.boards()
        }
        fn access(&mut self, b: &str) -> Result<BoardAccess<'_>, Error> {
            self.host.access(b)
        }
        fn file_effect(&mut self, verb: &str, r: &varos_bridge::dto::FileEffect) -> Result<Reply, Error> {
            self.calls.push((verb.into(), r.options.clone()));
            Ok(Reply::success(json!({"prepared":true})))
        }
    }
    let mut host = Effects { host: FakeHost::new(), calls: vec![] };
    let mut service = Service::new("test-epoch".into());
    let rev = host.host.editor.rev;
    for (i, verb) in ["print", "copy", "cut", "export_pdf"].into_iter().enumerate() {
        let mut args = json!({"api":"1.2","board":"b1","request_id":format!("r{}",i+1),"expected_rev":rev});
        if verb == "export_pdf" {
            args["path"] = json!("/Users/test/output.pdf");
            args["scope"] = json!("all_visible_artboards");
        }
        if ["print", "export_pdf"].contains(&verb) {
            args["options"] = json!({"preset":"press","image_ppi":150,"marks":{"crop":true}});
        }
        let cli =
            varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":verb,"arguments":args})).unwrap()).unwrap();
        let mcp = varos_bridge::mcp::decode_tool(verb, args.clone()).unwrap();
        let a = service.handle(&mut host, &ctx(), cli, &AtomicBool::new(false));
        let b = service.handle(&mut host, &ctx(), mcp, &AtomicBool::new(false));
        assert!(a.ok, "{a:?}");
        assert_eq!(a, b);
        assert_eq!(host.calls.len(), i + 1);
        let mut legacy = args;
        legacy["api"] = json!("1.1");
        legacy["request_id"] = json!("r100");
        if legacy.get("options").is_some() {
            assert!(varos_bridge::mcp::decode_tool(verb, legacy.clone()).is_err());
            assert!(varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":verb,"arguments":legacy})).unwrap())
                .is_err());
            legacy.as_object_mut().unwrap().remove("options");
        }
        if verb != "export_pdf" {
            assert!(!service.handle(&mut host, &ctx(), req(verb, legacy), &AtomicBool::new(false)).ok);
        }
    }
    let null_options = varos_bridge::mcp::decode_tool(
        "export_pdf",
        json!({"api":"1.1","request_id":"r100","board":"b1","expected_rev":rev,"path":"/Users/test/output.pdf","scope":"all_visible_artboards","options":null}),
    );
    assert!(null_options.is_err(), "explicit options, including null, require API 1.2");
    let capability =
        service.handle(&mut host, &ctx(), req("capabilities", json!({"api":"1.2"})), &AtomicBool::new(false));
    assert!(capability.ok);
    assert_eq!(capability.result.unwrap()["api_by_tool"]["cut"], json!(["1.2"]));
    let old = varos_bridge::mcp::tools();
    assert_eq!(old, varos_bridge::mcp::tools_for_api("1.1"));
    let new = varos_bridge::mcp::tools_for_api("1.2");
    assert_eq!(new["tools"].as_array().unwrap().len(), varos_bridge::TOOLS.len() + 11 + 2);
}

#[test]
fn stroke_api_12_is_atomic_versioned_and_undoable() {
    let mut s = Service::new("test-epoch".into());
    let mut h = FakeHost::new();
    s.observe(&mut h);
    let rev = h.editor.rev;
    for api in ["1.0", "1.1"] {
        let error=varos_bridge::mcp::decode_tool("edit",json!({"api":api,"board":"b1","request_id":"r1","expected_rev":rev,"ops":[{"verb":"set_stroke_style","ids":["path:10"],"stroke_style":{"cap":"Butt"}}]})).unwrap_err();
        assert_eq!(error.code, "unsupported");
        assert_eq!(h.editor.rev, rev);
    }
    let op = json!({"api":"1.2","board":"b1","request_id":"r3","expected_rev":rev,"ops":[{"op":"set_stroke_style","ids":["path:10"],"stroke_style":{"cap":"Butt","dash":[6,3]}}]});
    let r = handle(&mut s, &mut h, req("edit", op.clone()));
    assert!(r.ok, "{r:?}");
    assert_eq!(h.editor.doc.paths[0].stroke_style.cap, varos_core::stroke::StrokeCap::Butt);
    assert_eq!(r, handle(&mut s, &mut h, req("edit", op)));
    for api in ["1.0", "1.1", "1.2"] {
        let r = handle(
            &mut s,
            &mut h,
            req("describe", json!({"api":api,"board":"b1","ids":["path:10"],"fields":["paint"]})),
        );
        assert!(r.ok);
        assert_eq!(r.result.as_ref().unwrap()["objects"][0].get("stroke_style").is_some(), api == "1.2");
    }
    let r = handle(
        &mut s,
        &mut h,
        req("describe", json!({"api":"1.2","board":"b1","ids":["path:10"],"fields":["stroke_style"]})),
    );
    assert!(r.ok, "{r:?}");
    assert_eq!(r.result.unwrap()["objects"][0]["stroke_style"]["miter_limit"], 10.0);
    let before = h.editor.doc.clone();
    let rev = h.editor.rev;
    let r = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","board":"b1","request_id":"r4","expected_rev":rev,"ops":[{"verb":"set_stroke_style","ids":["path:10"],"stroke_style":{}},{"verb":"set_stroke_style","ids":["path:20"],"stroke_style":{"dash":[0,0]}}]}),
        ),
    );
    assert!(!r.ok);
    assert_eq!(h.editor.doc, before);
    h.editor.undo();
    assert!(h.editor.doc.paths[0].stroke_style.is_default());
    h.editor.redo();
    assert_eq!(h.editor.doc, before);
}

#[test]
fn stroke_api_12_schemas_capabilities_and_limit_errors() {
    let schema = varos_bridge::mcp::tools_for("1.2");
    let edit = schema["tools"].as_array().unwrap().iter().find(|t| t["name"] == "edit").unwrap();
    assert!(edit["inputSchema"]["$defs"]["set_paint"]["properties"]["stroke_style"].is_object());
    let stroke = varos_bridge::mcp::schema("edit", Some("set_stroke_style")).unwrap();
    assert_eq!(stroke["properties"]["op"]["const"], "set_stroke_style");
    assert_eq!(varos_bridge::mcp::tools_for("1.1"), varos_bridge::mcp::tools());
    let mut s = Service::new("test-epoch".into());
    let mut h = FakeHost::new();
    let reply = handle(&mut s, &mut h, req("capabilities", json!({"api":"1.2"})));
    assert!(reply.ok, "{reply:?}");
    assert_eq!(reply.result.as_ref().unwrap()["writable_vrs"], json!([5]));
    assert!(reply.result.as_ref().unwrap()["stroke_operations_schema"].is_object());
    h.editor.doc.paths[0].stroke = varos_core::model::Paint::Solid([0.0, 0.0, 0.0, 1.0]);
    let rev = h.editor.rev;
    let before = h.editor.doc.clone();
    let reply = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":rev,"ops":[{"op":"set_stroke_style","ids":["path:10"],"stroke_style":{"dash":[0.0001,0.0001]}}]}),
        ),
    );
    assert_eq!(reply.error.unwrap().code, "limit_exceeded");
    assert_eq!(h.editor.doc, before);
}

#[test]
fn stroke_scene_failure_is_a_snapshot_error_for_board_and_page() {
    use varos_core::{
        model::{Artboard, Xform},
        stroke::{StrokeCap, StrokeStyle},
    };
    let h = FakeHost::new();
    let mut doc = h.editor.doc.clone();
    let p = &mut doc.paths[0];
    p.anchors.truncate(2);
    p.anchors[0].p = [0.0, 0.0];
    p.anchors[1].p = [4000.0, 0.0];
    for a in &mut p.anchors {
        a.hin = None;
        a.hout = None;
    }
    p.closed = false;
    p.stroke = varos_core::model::Paint::Solid([0.0, 0.0, 0.0, 1.0]);
    p.stroke_width = 1.0;
    p.stroke_style = StrokeStyle { cap: StrokeCap::Butt, dash: vec![1.0, 1.0], ..Default::default() };
    let pid = p.id;
    let unit = doc.unit_of(pid).unwrap();
    doc.set_node_xform(unit, Xform { rot: 0.7, piv: [0.0, 0.0] });
    doc.artboards = vec![Artboard { id: 100, w: 6000.0, h: 6000.0, clip: false, ..Default::default() }];
    for artboard in [None, Some(100)] {
        let reply = varos_bridge::service::SnapshotJob { document: doc.clone(), rev: 1, size: [100, 100], artboard }
            .render(&AtomicBool::new(false));
        assert!(!reply.ok);
        assert!(reply.result.is_none());
        let error = reply.error.unwrap();
        assert_eq!(error.code, "limit_exceeded");
        assert!(error.reason.contains("path"));
    }
}

#[test]
fn trace_rgba_is_opt_in_atomic_and_preserves_holes() {
    let rgba: Vec<u8> = (0..8)
        .flat_map(|y| {
            (0..8)
                .flat_map(move |x| if (2..6).contains(&x) && (2..6).contains(&y) { [255u8; 4] } else { [0, 0, 0, 255] })
        })
        .collect();
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let before = serde_json::to_value(&h.editor.doc).unwrap();
    for api in ["1.0", "1.1"] {
        let rev = h.editor.rev;
        let error = varos_bridge::mcp::decode_tool("edit",json!({"api":api,"request_id":format!("trace-{api}"),"board":"b1","expected_rev":rev,"ops":[{"verb":"trace_rgba","rgba":rgba,"width":8,"height":8}]})).unwrap_err();
        assert_eq!(error.code, "unsupported");
        assert_eq!(serde_json::to_value(&h.editor.doc).unwrap(), before);
    }
    let cap = handle(&mut s, &mut h, req("capabilities", json!({"api":"1.2"}))).result.unwrap();
    assert!(cap["edit_verbs"].as_array().unwrap().contains(&json!("trace_rgba")));
    assert_eq!(cap["api_by_tool"]["edit"], json!(["1.0", "1.1", "1.2"]));
    let rev = h.editor.rev;
    let invalid = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","request_id":"r11","board":"b1","expected_rev":rev,"ops":[{"verb":"trace_rgba","rgba":rgba,"width":8,"height":8},{"verb":"trace_rgba","rgba":[],"width":8,"height":8}]}),
        ),
    );
    assert!(!invalid.ok);
    assert_eq!(invalid.error.as_ref().unwrap().op_index, Some(1));
    assert_eq!(serde_json::to_value(&h.editor.doc).unwrap(), before);
    let rev = h.editor.rev;
    let reply = handle(
        &mut s,
        &mut h,
        req(
            "edit",
            json!({"api":"1.2","request_id":"r12","board":"b1","expected_rev":rev,"ops":[{"verb":"trace_rgba","rgba":rgba,"width":8,"height":8}]}),
        ),
    );
    assert!(reply.ok, "{}", serde_json::to_value(reply).unwrap());
    assert_eq!(h.editor.doc.paths.len(), 3);
    assert_eq!(h.editor.doc.paths[2].holes.len(), 1);
    h.editor.execute(EditCommand::Undo).unwrap();
    assert_eq!(serde_json::to_value(&h.editor.doc).unwrap(), before);
}

#[test]
fn trace_api12_inherits_economy_mixed_batch_names_receipts_and_rollback() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    let before = h.editor.doc.clone();
    let rev = h.editor.rev;
    let args = json!({"api":"1.2","board":"b1","request_id":"r31","expected_rev":rev,
        "receipt":"ids","defaults":{"fill":"#112233FF","stroke":null},"ops":[
        {"verb":"trace_rgba","rgba":[0,0,0,255],"width":1,"height":1,"options":{"noise_px":0}},
        ["rect",[1,2,10,20],{"local":"$a"}],
        {"verb":"repeat","count":2,"dx":20,"dy":0,"ops":[["ellipse",[0,0,10,10],{"local":"$e"}]]},
        {"verb":"move","ids":["$e_1"],"delta":[0,5]}]});
    let reply = handle(&mut s, &mut h, req("edit", args.clone()));
    assert!(reply.ok, "{reply:?}");
    let result = reply.result.as_ref().unwrap();
    assert!(result["created"].as_array().unwrap().iter().all(Value::is_string));
    assert_eq!(h.editor.doc.paths.len(), before.paths.len() + 4);
    assert_eq!(result["created"].as_array().unwrap().len(), 4);
    assert_eq!(reply.undo_steps, 1);
    for (p, label) in h.editor.doc.paths[before.paths.len() + 1..].iter().zip(["Rect ", "Ellipse ", "Ellipse "]) {
        assert_eq!(p.fill, Paint::Solid([17. / 255., 34. / 255., 51. / 255., 1.]));
        assert!(p.name.as_ref().unwrap().starts_with(label));
    }
    assert_eq!(handle(&mut s, &mut h, req("edit", args.clone())), reply);
    h.editor.undo();
    assert_eq!(h.editor.doc, before);
    h.editor.redo();
    let after = h.editor.doc.clone();
    let mut bad = args;
    bad["request_id"] = json!("r32");
    bad["expected_rev"] = json!(h.editor.rev);
    bad["ops"].as_array_mut().unwrap().push(json!({"verb":"trace_rgba","rgba":[],"width":1,"height":1}));
    let error = handle(&mut s, &mut h, req("edit", bad)).error.unwrap();
    assert_eq!(error.op_index, Some(4));
    assert_eq!(h.editor.doc, after);
    let cap = handle(&mut s, &mut h, req("capabilities", json!({"api":"1.2"}))).result.unwrap();
    assert!(cap["edit_verbs"].as_array().unwrap().contains(&json!("repeat")));
    assert!(cap["economy_hint"].as_str().unwrap().contains("defaults"));
}

#[test]
fn trace_api12_preflights_expanded_targets_before_allocation() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    // Each leaf names one layer: below the wire limit, above the expanded path limit.
    let ops: Vec<_> = (0..99).map(|_| json!({"verb":"set_paint","ids":["node:1"],"fill":"#112233FF"})).collect();
    let template = h.editor.doc.paths[0].clone();
    for i in 0..10 {
        let mut path = template.clone();
        path.id = 1000 + i * 10;
        for (j, anchor) in path.anchors.iter_mut().enumerate() {
            anchor.id = path.id + j as u32 + 1;
        }
        h.editor.doc.paths.push(path);
    }
    h.editor.doc.ids = 1200;
    h.editor.replace_doc(h.editor.doc.clone());
    let before = h.editor.doc.clone();
    let undo = h.editor.history_preview(false).cloned();
    let args = json!({"api":"1.2","board":"b1","request_id":"r33","expected_rev":h.editor.rev,"ops":ops});
    let error = handle(&mut s, &mut h, req("edit", args)).error.unwrap();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.op_index.is_some());
    assert_eq!(h.editor.doc, before);
    assert_eq!(h.editor.history_preview(false), undo.as_ref());
}

#[test]
fn progressive_discovery_resolves_every_12_verb_without_mutation() {
    fn check_refs(value: &Value, root: &Value) {
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            assert!(root.pointer(reference.strip_prefix('#').unwrap()).is_some(), "{reference}");
        }
        match value {
            Value::Object(map) => {
                for child in map.values() {
                    check_refs(child, root);
                }
            }
            Value::Array(array) => {
                for child in array {
                    check_refs(child, root);
                }
            }
            _ => {}
        }
    }
    let mut host = FakeHost::new();
    let mut service = Service::new("test-epoch".into());
    let before = host.editor.doc.clone();
    let rev = host.editor.rev;
    let caps = handle(&mut service, &mut host, req("capabilities", json!({"api":"1.2"}))).result.unwrap();
    let index = handle(&mut service, &mut host, req("list_verbs", json!({"api":"1.2"})));
    assert!(index.ok, "{index:?}");
    let index = index.result.unwrap();
    let mut names = std::collections::BTreeSet::new();
    for group in index["groups"].as_array().unwrap() {
        for verb in group["verbs"].as_array().unwrap() {
            let name = verb["name"].as_str().unwrap();
            assert!(!verb["description"].as_str().unwrap().is_empty());
            assert!(!verb["description"].as_str().unwrap().contains('\n'));
            if group["tool"] == "edit" {
                assert!(names.insert(name.to_owned()), "duplicate {name}");
                let reply =
                    handle(&mut service, &mut host, req("schema", json!({"api":"1.2","tool":"edit","verb":name})));
                assert!(reply.ok, "{name}: {reply:?}");
                let params = reply.result.unwrap();
                check_refs(&params, &params);
                assert!(params.is_object());
            } else {
                assert!(varos_bridge::mcp::schema(name, None).is_ok(), "{name}");
            }
        }
    }
    for verb in caps["edit_verbs"].as_array().unwrap() {
        assert!(names.contains(verb.as_str().unwrap()), "missing {verb}");
    }
    assert!(names.contains("repeat"));
    assert_eq!(
        varos_bridge::mcp::schema("edit", Some("stroke_style")).unwrap(),
        varos_bridge::mcp::schema("edit", Some("set_stroke_style")).unwrap()
    );
    let table = varos_bridge::mcp::tools_for_api("1.2");
    for row in table["tools"].as_array().unwrap() {
        check_refs(&row["inputSchema"], &row["inputSchema"]);
    }
    for api in ["1.0", "1.1"] {
        for tool in ["schema", "list_verbs"] {
            let args =
                if tool == "schema" { json!({"api":api,"tool":"edit","verb":"move"}) } else { json!({"api":api}) };
            let reply = handle(&mut service, &mut host, req(tool, args));
            assert_eq!(reply.error.unwrap().code, "unsupported");
        }
    }
    for args in [
        json!({"api":"1.2","tool":"edit","verb":"unknown"}),
        json!({"api":"1.2","tool":"unknown"}),
        json!({"api":"1.2","tool":"edit"}),
        json!({"api":"1.2","tool":"select","verb":"move"}),
    ] {
        let reply = handle(&mut service, &mut host, req("schema", args));
        assert_eq!(reply.error.unwrap().code, "invalid_argument");
    }
    assert!(varos_bridge::mcp::decode_tool("schema", json!({"api":"1.2","tool":"edit","verb":"move","extra":true}))
        .is_err());
    // Exercise both discovery tools through the real MCP framing and CLI decoder.
    let transport = FakeTransport {
        state: std::sync::Arc::new(std::sync::Mutex::new((Service::new("test-epoch".into()), FakeHost::new()))),
        cancellations: Default::default(),
    };
    let (input_tx, input) = std::sync::mpsc::channel();
    let (output, receive) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        varos_bridge::mcp::serve(
            &mut std::io::BufReader::new(ChannelRead { rx: input, current: std::io::Cursor::new(vec![]) }),
            ChannelWrite { tx: output, bytes: vec![] },
            transport,
        )
        .unwrap();
    });
    for msg in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{},"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ] {
        let mut bytes = serde_json::to_vec(&msg).unwrap();
        bytes.push(b'\n');
        input_tx.send(bytes).unwrap();
    }
    receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
    for (name, args) in
        [("list_verbs", json!({"api":"1.2"})), ("schema", json!({"api":"1.2","tool":"edit","verb":"stroke_style"}))]
    {
        let envelope = json!({"tool":name,"arguments":args});
        let cli = varos_bridge::cli::decode(&serde_json::to_vec(&envelope).unwrap()).unwrap();
        let expected = handle(&mut service, &mut host, cli);
        assert!(expected.ok);
        let msg = json!({"jsonrpc":"2.0","id":name,"method":"tools/call","params":{"name":name,"arguments":args}});
        let mut bytes = serde_json::to_vec(&msg).unwrap();
        bytes.push(b'\n');
        input_tx.send(bytes).unwrap();
        let result: Value =
            serde_json::from_slice(&receive.recv_timeout(std::time::Duration::from_secs(3)).unwrap()).unwrap();
        assert_eq!(result["result"], varos_bridge::mcp::tool_result(&expected));
    }
    drop(input_tx);
    server.join().unwrap();
    assert_eq!(host.editor.doc, before);
    assert_eq!(host.editor.rev, rev);
}

#[test]
fn bridge_stroke_content_change_re_evaluates_only_once() {
    use varos_core::scene::build_scene;
    let mut s = Service::new("test-epoch".into());
    let mut h = FakeHost::new();
    for p in &mut h.editor.doc.paths {
        p.stroke = varos_core::model::Paint::Solid([0., 0., 0., 1.]);
        p.stroke_style.dash = vec![6., 3.];
    }
    s.observe(&mut h);
    build_scene(&h.editor, 1.);
    let cold = h.editor.canvas_stroke_cache.evaluations();
    assert_eq!(cold, 2);
    let op = json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":h.editor.rev,
        "ops":[{"op":"set_stroke_style","ids":["path:10"],"stroke_style":{"dash":[8,4]}}]});
    let r = handle(&mut s, &mut h, req("edit", op.clone()));
    assert!(r.ok, "{r:?}");
    build_scene(&h.editor, 1.);
    assert_eq!(h.editor.canvas_stroke_cache.evaluations(), cold + 1);
    assert_eq!(r, handle(&mut s, &mut h, req("edit", op)));
    build_scene(&h.editor, 1.);
    assert_eq!(h.editor.canvas_stroke_cache.evaluations(), cold + 1);
    h.editor.undo();
    build_scene(&h.editor, 1.);
    assert_eq!(h.editor.canvas_stroke_cache.evaluations(), cold + 2);
    build_scene(&h.editor, 1.);
    assert_eq!(h.editor.canvas_stroke_cache.evaluations(), cold + 2);
}
