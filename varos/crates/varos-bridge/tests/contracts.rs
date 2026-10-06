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
    Context { client: "fixture".into(), epoch: "test-epoch".into(), read: true, edit: true, allow_history: false }
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
    for bad in
        [json!({"verb":"delete","ids":["path:10"]}), json!({"verb":"move","ids":["path:10"],"delta":[1,0],"snap":true})]
    {
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
    assert_eq!(read(&mut stdout)["result"]["tools"].as_array().unwrap().len(), 7);
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
    assert_eq!(read()["result"]["tools"].as_array().unwrap().len(), 7);
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
    assert_eq!(lines[1]["result"]["tools"].as_array().unwrap().len(), 7);
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
