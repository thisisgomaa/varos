//! Varos: agents reach the running app with no setup and no prompt (docs/varos/BACKLOG.md item 8).
//!
//! **The always-on listener.** Without `--control`, every launch listens on a fresh loopback port
//! ([`start_default`]) and writes the port and a fresh 256-bit token into a file only the user can
//! read (`varos_agents::endpoint`: a 0700 folder, a 0600 file created atomically, symlinks refused;
//! removed at quit by [`remove_endpoint`]). The MCP proxy reads that file and sends the token with
//! every line; a line without it is refused and runs nothing. So any agent of this user gets in
//! (owner decision 2026-10-08: no pairing, no approval, no keychain or password prompt), and other
//! users' processes and web pages do not. Lines that came in this way are marked `guarded`: the app
//! then applies the mistake guards to the files they write (`varos::agents::guard`).
//!
//! `--control <port>` keeps upstream's behaviour exactly: no token, no guards.
//!
//! **Who sent it.** The MCP proxy also adds `"actor": {"session", "profile", "label"}` to every line;
//! the session is one per proxy process, so a reconnect keeps the agent's name. A client that names
//! no actor is one agent per connection. The actor is a claim used for attribution only (the label
//! on the canvas, the History rows), never for access.
//!
//! Each connection is served on its own thread and a line's request is made on that thread right
//! after the line is read, so the line's extras pass through a thread-local instead of widening the
//! transport's handler signature.

use std::cell::RefCell;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Map, Value, json};
use varos_agents::endpoint::{self, Endpoint};
use vectorcraft_ui_egui::ControlRequest;

/// What a line carries besides the request: who sent it and the token it shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Extras {
    actor: Option<Value>,
    token: Option<String>,
}

/// The extras of a parsed request line.
pub(super) fn extras(line: &Map<String, Value>) -> Extras {
    Extras { actor: line.get("actor").cloned(), token: line.get("token").and_then(Value::as_str).map(str::to_string) }
}

thread_local! {
    static LINE: RefCell<Extras> = RefCell::new(Extras::default());
}

/// The extras of the line just read on this thread.
pub(super) fn set_line(extras: Extras) {
    LINE.with(|l| *l.borrow_mut() = extras);
}

fn take_line() -> Extras {
    LINE.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);

/// The tokens of the always-on listeners, by local port: a connection to one of these ports must
/// show its token. Upstream's `--control` port is never in it.
static TOKENS: Mutex<Vec<(u16, String)>> = Mutex::new(Vec::new());

/// The endpoint file this launch published, removed at quit.
static ENDPOINT_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// One connection: its number, and the token it must show (`None`: upstream's `--control` port).
pub(super) struct Link {
    connection: u64,
    token: Option<String>,
}

impl Link {
    pub(super) fn new(stream: &TcpStream) -> Self {
        let port = stream.local_addr().ok().map(|a| a.port());
        let token =
            port.and_then(|port| TOKENS.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(p, _)| *p == port).map(|(_, t)| t.clone()));
        Self { connection: NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed), token }
    }

    /// The line just read, judged: `Err(reply)` refuses it (it runs nothing); `Ok(actor)` is who
    /// sent it, `guarded` when it came through the always-on listener.
    pub(super) fn admit(&self) -> Result<Value, Value> {
        let line = take_line();
        if let Some(required) = &self.token
            && !line.token.as_deref().is_some_and(|t| same(t, required))
        {
            return Err(
                json!({"ok": false, "error": "unauthorized: this Varos port needs the token from its endpoint file; use `vectorcraft-cli mcp`, which reads it. Nothing was run."}),
            );
        }
        let mut actor = line.actor.filter(Value::is_object).unwrap_or_else(|| json!({"session": format!("connection-{}", self.connection)}));
        actor["guarded"] = json!(self.token.is_some());
        Ok(actor)
    }
}

/// Compare tokens in time that doesn't depend on where they differ.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Listen on a fresh loopback port and publish it with a fresh token in `dir`. `None` (and a line
/// on stderr) when either fails: the app then runs without agents, as it would without `--control`.
pub(crate) fn start_default(dir: &Path, ctx: egui::Context) -> Option<Receiver<ControlRequest>> {
    prune_stale(dir);
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| eprintln!("varos: agents listener failed to bind: {e}")).ok()?;
    let port = listener.local_addr().ok()?.port();
    let ep = Endpoint::new(port).map_err(|e| eprintln!("varos: no random source for the agents token: {e}")).ok()?;
    TOKENS.lock().unwrap_or_else(PoisonError::into_inner).push((port, ep.token.clone()));
    let path = endpoint::publish(dir, &ep).map_err(|e| eprintln!("varos: cannot write the agents endpoint in {}: {e}", dir.display())).ok()?;
    *ENDPOINT_FILE.lock().unwrap_or_else(PoisonError::into_inner) = Some(path.clone());
    let (tx, rx) = channel::<ControlRequest>();
    std::thread::spawn(move || super::accept_loop(listener, tx, ctx));
    eprintln!("varos: agents welcome on 127.0.0.1:{port} (endpoint {})", path.display());
    Some(rx)
}

/// Remove the endpoint file this launch published (at quit).
pub(crate) fn remove_endpoint() {
    if let Some(path) = ENDPOINT_FILE.lock().unwrap_or_else(PoisonError::into_inner).take() {
        endpoint::remove(&path);
    }
}

/// Remove endpoint files whose port nobody answers on: launches that crashed.
fn prune_stale(dir: &Path) {
    for ep in endpoint::list(dir) {
        let alive = ep.addr().parse().is_ok_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok());
        if !alive {
            endpoint::remove(&endpoint::file_for(dir, ep.pid));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    fn line(v: Value) -> Extras {
        extras(v.as_object().unwrap())
    }

    fn open_link(token: Option<&str>) -> Link {
        Link { connection: 7, token: token.map(str::to_string) }
    }

    #[test]
    fn a_line_without_an_actor_is_its_connections_agent() {
        set_line(line(json!({"method": "x"})));
        assert_eq!(open_link(None).admit(), Ok(json!({"session": "connection-7", "guarded": false})));
        set_line(line(json!({"method": "x", "actor": "not an object"})));
        assert_eq!(open_link(None).admit(), Ok(json!({"session": "connection-7", "guarded": false})));
    }

    #[test]
    fn a_lines_actor_is_used_once() {
        let actor = json!({"session": "s1", "profile": "claude-code", "label": "Claude Code"});
        set_line(line(json!({"method": "x", "actor": actor})));
        let got = open_link(None).admit().unwrap();
        assert_eq!(got["label"], "Claude Code");
        assert_eq!(open_link(None).admit().unwrap()["session"], "connection-7");
    }

    #[test]
    fn the_token_port_refuses_lines_without_its_token() {
        let token = "ab".repeat(32);
        for bad in [json!({"method": "x"}), json!({"method": "x", "token": "ab"}), json!({"method": "x", "token": "cd".repeat(32)})] {
            set_line(line(bad));
            assert!(open_link(Some(&token)).admit().unwrap_err()["error"].as_str().unwrap().starts_with("unauthorized"));
        }
        // With it, the actor is guarded, whatever the client claims.
        set_line(line(json!({"method": "x", "token": token, "actor": {"session": "s", "guarded": false}})));
        assert_eq!(open_link(Some(&token)).admit().unwrap()["guarded"], true);
        // Upstream's `--control` port asks for no token and guards nothing.
        set_line(line(json!({"method": "x", "actor": {"session": "s", "guarded": true}})));
        assert_eq!(open_link(None).admit().unwrap()["guarded"], false);
    }

    #[test]
    fn tokens_compare_whole() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
    }

    /// The always-on listener end to end over TCP: the endpoint file is private, a line without the
    /// token is refused, one with it reaches the app guarded, and quitting removes the file.
    #[test]
    fn the_always_on_listener_lets_in_only_the_token() {
        let dir = std::env::temp_dir().join(format!("varos-listener-{}", std::process::id())).join("agents");
        let _ = std::fs::remove_dir_all(&dir);
        // A stale file of a crashed launch: its port answers nothing, and it goes.
        let dead = TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
        let mut stale = Endpoint::new(dead).unwrap();
        stale.pid = 1;
        endpoint::publish(&dir, &stale).unwrap();
        let rx = start_default(&dir, egui::Context::default()).expect("listening");
        let listed = endpoint::list(&dir);
        assert_eq!(listed.len(), 1, "the stale file is gone: {listed:?}");
        let (port, file) = (listed[0].port, endpoint::file_for(&dir, std::process::id()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let ask = |v: Value| -> Value {
            let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut out = s.try_clone().unwrap();
            writeln!(out, "{v}").unwrap();
            let mut reply = String::new();
            BufReader::new(s).read_line(&mut reply).unwrap();
            serde_json::from_str(&reply).unwrap()
        };
        let r = ask(json!({"id": 1, "method": "document.inspect"}));
        assert!(r["error"].as_str().unwrap().starts_with("unauthorized"), "{r}");
        assert!(rx.try_recv().is_err(), "a refused line reaches nothing");
        // The app's side: answer the next request and keep its actor.
        let app = std::thread::spawn(move || {
            let req = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let actor = req.actor.clone();
            req.reply.send(json!({"ok": true, "result": 1})).unwrap();
            actor
        });
        let r = ask(json!({"id": 2, "method": "document.inspect", "token": listed[0].token, "actor": {"session": "s1", "label": "Claude Code"}}));
        assert_eq!(r, json!({"id": 2, "ok": true, "result": 1}));
        let actor = app.join().unwrap().unwrap();
        assert_eq!(actor["guarded"], true);
        assert_eq!(actor["label"], "Claude Code");
        remove_endpoint();
        assert!(!file.exists());
    }
}
