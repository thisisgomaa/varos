//! Varos: the MCP server attaches to the Varos the person has open (docs/varos/BACKLOG.md item 8).
//!
//! `vectorcraft-cli mcp` with no option finds the running app through its endpoint file
//! (`varos_agents::endpoint`: the port and the per-launch token, readable only by this user) and
//! sends the token with every line. No pairing, approval or password prompt (owner decision
//! 2026-10-08). The app is looked up again on every call while none is attached, so the agent can
//! start before Varos does.
//!
//! An agent's edits must land where the person sees them, so there is no silent fallback to an
//! invisible headless document (ADR-0011): with no Varos open each call answers `host_not_running`,
//! with two open `ambiguous_target`, and nothing runs. `--headless` and `--connect` keep upstream's
//! behaviour.

use std::path::PathBuf;

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
}

impl Default for VarosAttach {
    fn default() -> Self {
        Self::new()
    }
}

impl VarosAttach {
    /// Attach through the user's agents folder ([`endpoint::dir`]).
    pub fn new() -> Self {
        Self { dir: endpoint::dir(), remote: None, client: Value::Null }
    }

    /// Attach through the endpoint files in `dir` (tests, isolated profiles).
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: Some(dir.into()), remote: None, client: Value::Null }
    }

    /// The attached app, looking it up when none is.
    fn attached(&mut self) -> Result<&mut Remote, String> {
        if self.remote.is_none() {
            self.remote = Some(self.find()?);
        }
        self.remote.as_mut().ok_or_else(|| HOST_NOT_RUNNING.to_string())
    }

    /// Connect to the one running Varos.
    fn find(&self) -> Result<Remote, String> {
        let dir = self.dir.as_ref().ok_or_else(|| HOST_NOT_RUNNING.to_string())?;
        let mut live: Vec<(endpoint::Endpoint, Remote)> =
            endpoint::list(dir).into_iter().filter_map(|e| Remote::connect(&e.addr()).ok().map(|r| (e, r))).collect();
        match live.len() {
            0 => Err(HOST_NOT_RUNNING.to_string()),
            1 => {
                let (e, remote) = live.remove(0);
                let mut remote = remote.with_token(e.token);
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

impl Backend for VarosAttach {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        match self.attached()?.call(method, params.clone()) {
            // The app quit (or restarted on a new port) since the last call: nothing was sent, so
            // look it up again and send once more.
            Err(e) if e.contains("is not reachable") => {
                self.remote = None;
                self.attached()?.call(method, params)
            }
            other => other,
        }
    }

    fn has_ui(&self) -> bool {
        true
    }

    fn describe(&self) -> String {
        match &self.remote {
            Some(r) => format!("Varos at {}", r.addr()),
            None => "Varos (attached on the first call)".into(),
        }
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
    use varos_agents::endpoint::Endpoint;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("varos-attach-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// A stand-in for Varos on a fresh port, published in `dir` as process `pid`: it answers
    /// `lines` requests and hands back what it read.
    fn fake_varos(dir: &std::path::Path, pid: u32, lines: usize) -> (Endpoint, std::thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut e = Endpoint::new(listener.local_addr().unwrap().port()).unwrap();
        e.pid = pid;
        endpoint::publish(dir, &e).unwrap();
        let app = std::thread::spawn(move || {
            let mut seen = vec![];
            // The lookup's own connection, then the real one: serve whichever sends a line.
            while seen.len() < lines {
                let Ok((stream, _)) = listener.accept() else { break };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut out = stream;
                let mut line = String::new();
                while seen.len() < lines && reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                    let v: Value = serde_json::from_str(&line).unwrap();
                    writeln!(out, "{}", json!({"id": v["id"], "ok": true, "result": {"ok": 1}})).unwrap();
                    seen.push(v);
                    line.clear();
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
        let (e, app) = fake_varos(&dir, 11, 2);
        let mut a = VarosAttach::in_dir(&dir);
        a.set_client(&json!({"name": "claude-code"}));
        assert_eq!(a.call("document.inspect", json!({})), Ok(json!({"ok": 1})));
        assert_eq!(a.call("ui.inspect", json!({})), Ok(json!({"ok": 1})));
        let seen = app.join().unwrap();
        assert_eq!(seen.len(), 2);
        for line in &seen {
            assert_eq!(line["token"], json!(e.token));
            assert_eq!(line["actor"]["label"], "Claude Code");
        }
        assert!(a.describe().starts_with("Varos at 127.0.0.1:"));
    }

    /// A Varos that accepts connections (the listen backlog) and answers nothing.
    fn idle_varos(dir: &std::path::Path, pid: u32) -> TcpListener {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut e = Endpoint::new(listener.local_addr().unwrap().port()).unwrap();
        e.pid = pid;
        endpoint::publish(dir, &e).unwrap();
        listener
    }

    #[test]
    fn two_varos_open_is_ambiguous() {
        let dir = temp("two");
        let _a = idle_varos(&dir, 21);
        let _b = idle_varos(&dir, 22);
        let err = VarosAttach::in_dir(&dir).call("document.inspect", json!({})).unwrap_err();
        assert!(err.starts_with("ambiguous_target: 2 Varos"), "{err}");
    }

    #[test]
    fn varos_opened_after_the_agent_is_found_on_the_next_call() {
        let dir = temp("later");
        let mut a = VarosAttach::in_dir(&dir);
        assert!(a.call("document.inspect", json!({})).unwrap_err().starts_with("host_not_running"));
        let (_e, app) = fake_varos(&dir, 31, 1);
        assert_eq!(a.call("document.inspect", json!({})), Ok(json!({"ok": 1})));
        assert_eq!(app.join().unwrap().len(), 1);
    }
}
