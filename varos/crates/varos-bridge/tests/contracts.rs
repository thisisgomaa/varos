use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use varos_bridge::{dto::HistoryAction, BoardAccess, BoardInfo, Context, Error, Host, Reply, Request, Service};
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
    Context {
        client: "fixture".into(),
        epoch: "test-epoch".into(),
        read: true,
        edit: true,
        allow_history: false,
        allow_destructive: false,
    }
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
        assert_eq!(json!(a), serde_json::from_str::<Value>(fixture).unwrap());
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
        h.editor.execute(EditCommand::SetBoardName("human".into()));
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
    for code in ["revision_conflict", "busy", "board_not_active", "scope_refused", "not_found"] {
        let mut h = FakeHost::new();
        let mut s = Service::new("test-epoch".into());
        let mut c = ctx();
        let r = edit("r1", if code == "revision_conflict" { 0 } else { 1 });
        match code {
            "busy" => h.busy = true,
            "board_not_active" => h.active = false,
            "scope_refused" => c.edit = false,
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
    h.editor.execute(EditCommand::SetBoardName("human".into()));
    let again = handle(&mut s, &mut h, r);
    assert_eq!(first, again);
    assert_eq!(h.editor.rev, 3);
    assert_eq!(handle(&mut s, &mut h, edit("r1", 3)).error.unwrap().code, "invalid_argument");
    let status = handle(&mut s, &mut h, req("request_status", json!({"request_id":"r1"})));
    assert_eq!(status.result.unwrap()["receipt"], json!(first));
}
#[test]
fn history_challenge_and_owner_grant_are_bound_single_use() {
    let mut h = FakeHost::new();
    let mut s = Service::new("test-epoch".into());
    assert!(handle(&mut s, &mut h, edit("r1", 1)).ok);
    let args = json!({"api":"1.0","request_id":"r2","board":"b1","expected_rev":2,"action":"undo"});
    let denied = handle(&mut s, &mut h, req("history", args.clone()));
    let hash = denied.error.unwrap().digest.clone().unwrap();
    let mut exact = args.clone();
    exact["digest"] = json!(hash);
    assert_eq!(handle(&mut s, &mut h, req("history", exact)).error.unwrap().code, "confirmation_required");
    assert_eq!(h.editor.rev, 2);
    let mut c = ctx();
    c.allow_history = true;
    let challenge = s.handle(&mut h, &c, req("history", args.clone()), &AtomicBool::new(false));
    let mut exact = args;
    exact["digest"] = json!(challenge.error.unwrap().digest.clone().unwrap());
    let r = req("history", exact);
    assert!(s.handle(&mut h, &c, r.clone(), &AtomicBool::new(false)).ok);
    assert_eq!(h.editor.rev, 3);
    assert_eq!(h.editor.doc.paths[0].anchors[0].p, [20., 20.]);
    assert!(s.handle(&mut h, &c, r, &AtomicBool::new(false)).ok);
    assert_eq!(h.editor.rev, 3, "receipt replay cannot undo twice");
    let _ = HistoryAction::Redo;
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
        h.editor.execute(EditCommand::SetBoardName(format!("human-{i}")));
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
    h.editor.execute(EditCommand::Copy);
    h.editor.execute(EditCommand::Paste { offset: Some([1., 0.]) });
    let used = h.editor.doc.ids;
    h.editor.undo();
    h.editor.execute(EditCommand::Paste { offset: Some([2., 0.]) });
    assert!(h.editor.doc.ids > used);
    assert!(h.editor.doc.paths.last().unwrap().id > used);
}
#[test]
fn transformed_partial_group_move_leaves_other_world_art_unchanged() {
    let mut h = FakeHost::new();
    h.editor.objsel.extend([10, 20]);
    h.editor.execute(EditCommand::GroupSelection);
    h.editor.execute(EditCommand::SetObjectRotation(45.));
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
        let (tx, rx) = std::sync::mpsc::sync_channel::<varos_bridge::ipc::Pending>(32);
        let listener = varos_bridge::ipc::Listener::start(tx, || {}).unwrap();
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
        .args(["describe", "--json", "--attach"])
        .arg(&host.endpoint.socket)
        .arg("--token")
        .arg(&host.endpoint.token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(cli.stdin.take().unwrap(), "{args}").unwrap();
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
    h.editor.execute(EditCommand::ToggleSnapping);
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
    h.editor.execute(EditCommand::SetOpacity(0.5));
    s.observe(&mut h);
    h.editor.execute(EditCommand::ApplyPaint { target: PaintTarget::Fill, color: Some([1., 0., 0., 1.]) });
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
fn frozen_compact_projection_and_confirmation_digest() {
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
    assert!(text.contains(r.error.unwrap().digest.as_ref().unwrap()));
    assert!(text.contains("expected_rev=2"));
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
        h.editor.execute(EditCommand::SetBoardName("human advanced revision".into()));
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
    h.editor.execute(EditCommand::Copy);
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
        let mut request = design_request(ops, 1);
        let mut owner = ctx();
        owner.allow_destructive = true;
        let mut r = s.handle(&mut h, &owner, request.clone(), &AtomicBool::new(false));
        if ["delete", "ungroup"].contains(&op["verb"].as_str().unwrap()) {
            assert_eq!(r.error.as_ref().unwrap().code, "confirmation_required");
            assert!(h.editor.doc.content_eq(&before));
            if let Request::Edit(ref mut edit) = request {
                edit.digest = r.error.unwrap().digest.clone();
            }
            r = s.handle(&mut h, &owner, request, &AtomicBool::new(false));
        }
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
fn destructive_grants_default_deny_exact_payload_and_receipt_retry() {
    let mut h = FakeHost::new();
    let before = h.editor.doc.clone();
    let mut s = Service::new("test-epoch".into());
    let request = design_request(vec![json!({"verb":"delete","ids":["path:10"]})], 1);
    let challenge = handle(&mut s, &mut h, request.clone());
    assert_eq!(challenge.error.as_ref().unwrap().code, "confirmation_required");
    let mut confirmed = request.clone();
    if let Request::Edit(ref mut e) = confirmed {
        e.digest = challenge.error.unwrap().digest.clone();
    }
    assert_eq!(handle(&mut s, &mut h, confirmed.clone()).error.unwrap().code, "confirmation_required");
    assert_eq!(h.editor.doc, before);
    let mut owner = ctx();
    owner.allow_destructive = true;
    let r = s.handle(&mut h, &owner, request, &AtomicBool::new(false));
    if let Request::Edit(ref mut e) = confirmed {
        e.digest = r.error.unwrap().digest.clone();
    }
    let mut changed = confirmed.clone();
    if let Request::Edit(ref mut e) = changed {
        e.ops.push(varos_bridge::dto::Operation::Move { ids: vec!["path:20".into()], delta: [2.0, 0.0] });
    }
    assert_eq!(s.handle(&mut h, &ctx(), changed, &AtomicBool::new(false)).error.unwrap().code, "confirmation_required");
    let success = s.handle(&mut h, &owner, confirmed.clone(), &AtomicBool::new(false));
    assert!(success.ok, "{success:?}");
    assert_eq!(s.handle(&mut h, &owner, confirmed, &AtomicBool::new(false)), success);
    assert!(h.editor.doc.pidx(10).is_none());
    h.editor.undo();
    assert!(h.editor.doc.content_eq(&before));
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
    let mut context = ctx();
    context.read = false;
    assert_eq!(
        s.handle(&mut h, &context, req("snapshot", json!({"board":"b1","rev":1})), &AtomicBool::new(false))
            .error
            .unwrap()
            .code,
        "scope_refused"
    );
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
        json!({"verb":"resize","ids":["path:10"],"bounds":[0,0,10,10]}),
        json!({"verb":"rotate","ids":["path:10"],"degrees":45}),
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
    let mut owner = ctx();
    owner.allow_destructive = true;
    let mut request =
        design_request(vec![shape("$a", "rect", [0.0, 0.0, 10.0, 10.0]), json!({"verb":"delete","ids":["$a"]})], 1);
    let challenge = s.handle(&mut h, &owner, request.clone(), &AtomicBool::new(false));
    assert_eq!(h.editor.doc, before);
    if let Request::Edit(ref mut e) = request {
        e.digest = challenge.error.unwrap().digest.clone();
    }
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
