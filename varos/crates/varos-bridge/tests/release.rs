//! Lane G: API isolation and actual MCP transport allowlist, headless.
use serde_json::{json, Value};
use std::{
    io::{self, Write},
    sync::{atomic::AtomicBool, Arc, Mutex},
};
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Request, Service};
struct Empty;
impl Host for Empty {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![]
    }
    fn prepare(&mut self, _: &str, _: bool) -> Result<(), Error> {
        Err(Error::new("not_found", "no documents"))
    }
    fn access(&mut self, _: &str) -> Result<BoardAccess<'_>, Error> {
        Err(Error::new("not_found", "no documents"))
    }
}
#[test]
fn release_refuses_legacy_apis_and_reports_unsupported_host() {
    let mut service = Service::new("test".into());
    for (api, code) in [("1.0", "unsupported"), ("1.1", "unsupported"), ("1.2", "unsupported_host")] {
        let request = varos_bridge::mcp::decode_tool("release", json!({"api":api,"action":{"kind":"check"}})).unwrap();
        let reply = service.handle(
            &mut Empty,
            &Context { client: "test".into(), epoch: "test".into() },
            request,
            &AtomicBool::new(false),
        );
        assert!(!reply.ok);
        assert_eq!(reply.error.unwrap().code, code);
    }
}
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct Transport;
impl varos_bridge::mcp::Transport for Transport {
    fn call(&self, _: &str, request: Request) -> Reply {
        assert!(matches!(request, Request::Release(_)));
        Reply::success(json!({"accepted":true}))
    }
    fn cancel(&self, _: &str) {}
}
#[test]
fn mcp_transport_routes_progressively_disclosed_release_tool() {
    let frames = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{},"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"release","arguments":{"api":"1.2","action":{"kind":"crash_report"}}}}),
    ];
    let mut input = Vec::new();
    for frame in frames {
        varos_bridge::ipc::write_frame(&mut input, &frame).unwrap();
    }
    let capture = Capture::default();
    varos_bridge::mcp::serve(&mut io::Cursor::new(input), capture.clone(), Transport).unwrap();
    let bytes = capture.0.lock().unwrap().clone();
    let mut read = io::Cursor::new(bytes);
    let mut replies = vec![];
    while let Some(frame) = varos_bridge::ipc::read_frame(&mut read).unwrap() {
        replies.push(serde_json::from_slice::<Value>(&frame).unwrap());
    }
    let reply = replies.iter().find(|r| r["id"] == 2).unwrap();
    assert_eq!(reply["result"]["structuredContent"]["result"]["accepted"], true);
}
#[test]
fn release_discovery_does_not_consume_inline_budget() {
    let size = serde_json::to_vec(&varos_bridge::mcp::tools_for("1.2")).unwrap().len();
    println!("API 1.2 tools/list: {size} B");
    assert!(size <= 24_000);
    let list = varos_bridge::mcp::list_verbs();
    println!("list_verbs: {} B", list.to_string().len());
    let row = list["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["verbs"].as_array().unwrap())
        .find(|r| r["name"] == "release")
        .unwrap();
    assert_eq!(row["actions"].as_array().unwrap().len(), 6);
    let schema = varos_bridge::mcp::schema("release", None).unwrap();
    assert_ne!(schema, Value::Null);
}
