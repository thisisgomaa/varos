//! Varos: the MCP server attaches to the Varos the person has open (docs/varos/BACKLOG.md item 8).
//!
//! `vectorcraft-cli mcp` with no option finds the running app through its endpoint file
//! (`varos_agents::endpoint`: the port and the per-launch secrets, readable only by this user). A
//! port is not taken on trust: on every new connection the app must first answer a hello with the
//! proof that it holds the file's server key ([`prove_server`]); a process that took over the port
//! of a Varos that crashed can't, and gets nothing but the hello's random nonce. Then the token
//! goes with every line. No pairing, approval or password prompt (owner decision 2026-10-08). The
//! app is looked up again on every call while none is attached, so the agent can start before
//! Varos does.
//!
//! An agent's edits must land where the person sees them, so there is no silent fallback to an
//! invisible headless document (ADR-0011): with no Varos open each call answers `host_not_running`,
//! with two open `ambiguous_target`, and nothing runs. `--headless` and `--connect` keep upstream's
//! behaviour.

use std::io::BufRead;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;
use varos_agents::endpoint;

use crate::backend::{Backend, Remote};

/// What the agent reads when Varos isn't open.
pub const HOST_NOT_RUNNING: &str =
    "host_not_running: Varos is not open on this computer. Ask the person to open Varos, then call again. Nothing was run.";

/// The running Varos, found through its endpoint file.
pub struct VarosAttach {
    dir: Option<PathBuf>,
    remote: Option<Remote>,
    client: Value,
    /// The board the calls act on (`varos_boards`): `Some` while a tool call names one (or
    /// explicitly none: `Some(Null)`); `None` outside tool calls (resources, completions), which
    /// then act on the board the agent last named, never on the tab in front.
    target: Option<Value>,
    /// The board the agent last named, without its revision.
    named: Option<Value>,
}

impl Default for VarosAttach {
    fn default() -> Self {
        Self::new()
    }
}

impl VarosAttach {
    /// Attach through the user's agents folder ([`endpoint::dir`]).
    pub fn new() -> Self {
        Self { dir: endpoint::dir(), remote: None, client: Value::Null, target: None, named: None }
    }

    /// Attach through the endpoint files in `dir` (tests, isolated profiles).
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: Some(dir.into()), remote: None, client: Value::Null, target: None, named: None }
    }

    /// The attached app, looking it up when none is.
    fn attached(&mut self) -> Result<&mut Remote, String> {
        if self.remote.is_none() {
            self.remote = Some(self.find()?);
        }
        self.remote.as_mut().ok_or_else(|| HOST_NOT_RUNNING.to_string())
    }

    /// Connect to the one running Varos: the endpoints whose port proves it is the Varos the file
    /// names.
    fn find(&self) -> Result<Remote, String> {
        let dir = self.dir.as_ref().ok_or_else(|| HOST_NOT_RUNNING.to_string())?;
        let mut live: Vec<(endpoint::Endpoint, Remote)> = endpoint::list(dir)
            .into_iter()
            .filter_map(|e| Remote::connect_varos(&e.addr(), e.token.clone(), e.server_key.clone()).ok().map(|r| (e, r)))
            .collect();
        match live.len() {
            0 => Err(HOST_NOT_RUNNING.to_string()),
            1 => {
                let (_, mut remote) = live.remove(0);
                remote.set_client(&self.client);
                Ok(remote)
            }
            n => {
                let pids: Vec<String> = live.iter().map(|(e, _)| e.pid.to_string()).collect();
                Err(format!(
                    "ambiguous_target: {n} Varos windows are open (processes {}). Ask the person to close the extra one, then call again. Nothing was run.",
                    pids.join(", ")
                ))
            }
        }
    }
}

/// How long the app has to answer the hello.
const PROOF_WAIT: Duration = Duration::from_secs(3);

/// The client side of the handshake on a fresh connection to Varos' agents port: the app must
/// prove it holds `server_key` before anything else is sent (`varos_agents::auth::handshake`).
pub(crate) fn prove_server(reader: &mut impl BufRead, stream: &TcpStream, server_key: &str) -> std::io::Result<()> {
    let kept = stream.read_timeout()?;
    stream.set_read_timeout(Some(PROOF_WAIT))?;
    let mut writer = stream;
    let proved = varos_agents::auth::handshake(reader, &mut writer, server_key);
    stream.set_read_timeout(kept)?;
    proved
}

impl VarosAttach {
    /// The target each line carries: the tool call's, else the board last named.
    fn line_target(&self) -> Option<Value> {
        match &self.target {
            Some(t) => Some(t.clone()).filter(|t| !t.is_null()),
            None => self.named.clone(),
        }
    }
}

impl Backend for VarosAttach {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let target = self.line_target();
        let remote = self.attached()?;
        remote.set_target(target.clone())?;
        match remote.call(method, params.clone()) {
            // The app quit (or restarted on a new port) since the last call: nothing was sent, so
            // look it up again and send once more.
            Err(e) if e.contains("is not reachable") => {
                self.remote = None;
                let remote = self.attached()?;
                remote.set_target(target)?;
                remote.call(method, params)
            }
            other => other,
        }
    }

    fn set_target(&mut self, target: Option<Value>) -> Result<(), String> {
        if let Some(board) = target.as_ref().and_then(|t| t.get("board")).filter(|b| !b.is_null()) {
            self.named = Some(serde_json::json!({ "board": board }));
        }
        self.target = target;
        Ok(())
    }

    fn last_board(&self) -> Value {
        self.remote.as_ref().map_or(Value::Null, Backend::last_board)
    }

    fn board_scoped(&self) -> bool {
        true
    }

    fn has_ui(&self) -> bool {
        true
    }

    fn describe(&self) -> String {
        let at = match &self.remote {
            Some(r) => format!("Varos at {}", r.addr()),
            None => "Varos (attached on the first call)".into(),
        };
        // The board rule, where every agent reads it (`initialize` instructions).
        format!(
            "{at}. Every call is board-scoped: call capabilities, then list_boards, and pass `board` (plus `revision` to save or export) with each call"
        )
    }

    fn set_client(&mut self, client_info: &Value) {
        self.client = client_info.clone();
        if let Some(r) = &mut self.remote {
            r.set_client(client_info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use varos_agents::auth;
    use varos_agents::endpoint::Endpoint;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("varos-attach-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// A stand-in for Varos on a fresh port, published in `dir` as process `pid`: it answers each
    /// connection's hello with its proof, then up to `lines` requests in all, and hands back what
    /// it read. `knows_key: false` makes it an impostor that took the port over: it answers hellos
    /// with another key's proof.
    fn fake_varos(dir: &std::path::Path, pid: u32, lines: usize, knows_key: bool) -> (Endpoint, std::thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut e = Endpoint::new(listener.local_addr().unwrap().port()).unwrap();
        e.pid = pid;
        endpoint::publish(dir, &e).unwrap();
        let key = if knows_key { e.server_key.clone() } else { auth::new_secret().unwrap() };
        listener.set_nonblocking(true).unwrap();
        let app = std::thread::spawn(move || {
            let mut seen = vec![];
            let start = std::time::Instant::now();
            // The lookup's own connection, then the real one: serve whichever sends lines.
            while seen.iter().filter(|v: &&Value| v["method"] != auth::HELLO).count() < lines && start.elapsed() < Duration::from_secs(3) {
                let Ok((stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut out = stream;
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                    let v: Value = serde_json::from_str(&line).unwrap();
                    if let Some(proof) = auth::answer_hello(line.trim().as_bytes(), &key) {
                        writeln!(out, "{proof}").unwrap();
                    } else {
                        writeln!(out, "{}", json!({"id": v["id"], "ok": true, "result": {"ok": 1}})).unwrap();
                    }
                    seen.push(v);
                    line.clear();
                    if seen.iter().filter(|v| v["method"] != auth::HELLO).count() >= lines {
                        break;
                    }
                }
            }
            seen
        });
        (e, app)
    }

    #[test]
    fn with_no_varos_open_every_call_says_so() {
        let mut a = VarosAttach::in_dir(temp("none"));
        let err = a.call("document.inspect", json!({})).unwrap_err();
        assert!(err.starts_with("host_not_running"), "{err}");
        // A file left by a crash (its port answers nothing) changes nothing.
        let dir = temp("stale");
        let dead = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        endpoint::publish(&dir, &Endpoint::new(dead).unwrap()).unwrap();
        let err = VarosAttach::in_dir(&dir).call("document.inspect", json!({})).unwrap_err();
        assert!(err.starts_with("host_not_running"), "{err}");
    }

    #[test]
    fn every_line_carries_the_token_and_the_agent() {
        let dir = temp("one");
        let (e, app) = fake_varos(&dir, 11, 2, true);
        let mut a = VarosAttach::in_dir(&dir);
        a.set_client(&json!({"name": "claude-code"}));
        assert_eq!(a.call("document.inspect", json!({})), Ok(json!({"ok": 1})));
        assert_eq!(a.call("ui.inspect", json!({})), Ok(json!({"ok": 1})));
        let seen = app.join().unwrap();
        let requests: Vec<&Value> = seen.iter().filter(|v| v["method"] != auth::HELLO).collect();
        assert_eq!(requests.len(), 2);
        for line in requests {
            assert_eq!(line["token"], json!(e.token));
            assert_eq!(line["actor"]["label"], "Claude Code");
        }
        // The hello carries a nonce and nothing else of ours.
        let hello = seen.iter().find(|v| v["method"] == auth::HELLO).unwrap();
        assert!(hello.get("token").is_none() && hello.get("actor").is_none());
        assert!(a.describe().starts_with("Varos at 127.0.0.1:"));
    }

    /// Another user's process took over the port of a Varos that crashed. It can't read the
    /// endpoint file, so it can't prove it is that Varos: the proxy sends it the hello's random
    /// nonce and nothing else (no token, no request, no params), and tells the agent no Varos is open.
    #[test]
    fn a_process_that_cannot_prove_it_is_varos_gets_nothing() {
        let dir = temp("impostor");
        let (_e, app) = fake_varos(&dir, 41, 1, false);
        let mut a = VarosAttach::in_dir(&dir);
        a.set_client(&json!({"name": "claude-code"}));
        let err = a.call("document.inspect", json!({"secret": "the person's document"})).unwrap_err();
        assert!(err.starts_with("host_not_running"), "{err}");
        let seen = app.join().unwrap();
        assert!(!seen.is_empty(), "it was asked for the proof");
        for line in &seen {
            assert_eq!(line["method"], auth::HELLO, "only hellos reach it: {line}");
            assert!(line.get("token").is_none() && line.get("actor").is_none(), "{line}");
            assert!(!line.to_string().contains("person's document"));
        }
    }

    #[test]
    fn two_varos_open_is_ambiguous() {
        let dir = temp("two");
        let (_a, _ta) = fake_varos(&dir, 21, 1, true);
        let (_b, _tb) = fake_varos(&dir, 22, 1, true);
        let err = VarosAttach::in_dir(&dir).call("document.inspect", json!({})).unwrap_err();
        assert!(err.starts_with("ambiguous_target: 2 Varos"), "{err}");
    }

    #[test]
    fn varos_opened_after_the_agent_is_found_on_the_next_call() {
        let dir = temp("later");
        let mut a = VarosAttach::in_dir(&dir);
        assert!(a.call("document.inspect", json!({})).unwrap_err().starts_with("host_not_running"));
        let (_e, app) = fake_varos(&dir, 31, 1, true);
        assert_eq!(a.call("document.inspect", json!({})), Ok(json!({"ok": 1})));
        assert_eq!(app.join().unwrap().iter().filter(|v| v["method"] != auth::HELLO).count(), 1);
    }
}
