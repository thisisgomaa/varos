//! Varos: agents reach the running app with no setup and no prompt (docs/varos/BACKLOG.md item 8).
//!
//! **The always-on listener.** Without `--control`, every launch listens on a fresh loopback port
//! ([`start_default`]) and writes the port and two fresh 256-bit secrets into a file only the user
//! can read (`varos_agents::endpoint`: a 0700 folder, a 0600 file created atomically, symlinks
//! refused; removed at quit by [`remove_endpoint`]). The MCP proxy reads that file. On every new
//! connection it first sends a `varos.hello` with a random nonce, and this side answers with the
//! proof that it holds the file's server key (`varos_agents::auth`); only then does the proxy send
//! requests, each with the token as its last member. A line is let through only when its tail
//! carries the token, checked before anything parses it ([`Link::screen`]); any other line (but a
//! hello) is refused and the connection closed. So any agent of this user gets in (owner decision
//! 2026-10-08: no pairing, no approval, no keychain or password prompt), and other users'
//! processes and web pages do not; and a process of another user that takes over a crashed
//! launch's port can't pass for Varos, so the proxy never sends it the token or a document. Lines
//! that came in this way are marked `guarded`: the app then applies the mistake guards to the files
//! they write (`varos::agents::guard`).
//!
//! Connections that haven't shown the token yet have their own small pool ([`MAX_PENDING`]) and a
//! short deadline ([`AUTH_WINDOW`]); the token moves a connection into the main pool, so peers that
//! never authenticate can't hold the slots of the user's agents.
//!
//! `--control <port>` keeps upstream's behaviour exactly: no secrets, no guards.
//!
//! **Who sent it.** The MCP proxy also adds `"actor": {"session", "profile", "label"}` to every line;
//! the session is one per proxy process, so a reconnect keeps the agent's name. A client that names
//! no actor is one agent per connection. The actor is a claim used for attribution only (the label
//! on the canvas, the History rows), never for access.
//!
//! Each connection is served on its own thread and a line's request is made on that thread right
//! after the line is read, so the line's extras pass through a thread-local instead of widening the
//! transport's handler signature.

use std::cell::{Cell, RefCell};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use varos_agents::auth;
use varos_agents::endpoint::{self, Endpoint};
use vectorcraft_ui_egui::ControlRequest;

use super::{ConnectionSlot, ConnectionTimeouts, MAX_CONNECTIONS, acquire_slot_of};

/// Connections of the agents port that haven't shown the token yet, at once.
pub(super) const MAX_PENDING: usize = 8;
/// How long such a connection has to send its hello and its first request (each line).
pub(super) const AUTH_WINDOW: Duration = Duration::from_secs(2);

const UNAUTHORIZED: &str = "unauthorized: this Varos port needs the token from its endpoint file; use `vectorcraft-cli mcp`, which reads it. Nothing was run; closing the connection.";

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

/// The two secrets of an always-on listener.
#[derive(Clone)]
struct Secrets {
    token: String,
    server_key: String,
}

/// The secrets of the always-on listeners, by local port: a connection to one of these ports must
/// show its token. Upstream's `--control` port is never in it.
static SECRETS: Mutex<Vec<(u16, Secrets)>> = Mutex::new(Vec::new());

/// The endpoint file this launch published, removed at quit.
static ENDPOINT_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// What [`Link::screen`] makes of a line before it is parsed.
pub(super) enum Screen {
    /// A request to parse and run.
    Pass,
    /// Answered here, nothing run (a hello); the connection stays open.
    Answer(Value),
    /// Refused, nothing run; the connection closes.
    Refuse(Value),
}

/// One connection: its number, the secrets it must know (`None`: upstream's `--control` port), and
/// the connection slot it holds.
pub(super) struct Link {
    connection: u64,
    secrets: Option<Secrets>,
    /// When it connected: [`AUTH_WINDOW`] counts from here, for all its lines together.
    opened: Instant,
    authenticated: Cell<bool>,
    slot: RefCell<Option<ConnectionSlot>>,
    /// The pool an agents-port connection moves into once it shows the token (`None`: it keeps
    /// the slot it has).
    main: Option<Arc<AtomicUsize>>,
}

impl Link {
    /// The link of `stream`, holding `slot` while it lives.
    pub(super) fn new(stream: &TcpStream, slot: ConnectionSlot) -> Self {
        let port = stream.local_addr().ok().map(|a| a.port());
        let secrets =
            port.and_then(|port| SECRETS.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(p, _)| *p == port).map(|(_, s)| s.clone()));
        let authenticated = Cell::new(secrets.is_none());
        Self {
            connection: NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed),
            secrets,
            opened: Instant::now(),
            authenticated,
            slot: RefCell::new(Some(slot)),
            main: None,
        }
    }

    /// Move into `main` (a slot of at most [`MAX_CONNECTIONS`]) once the token is shown.
    fn moving_into(mut self, main: Arc<AtomicUsize>) -> Self {
        self.main = Some(main);
        self
    }

    /// The idle and line budgets for the next line: until the token is shown, what is left of
    /// [`AUTH_WINDOW`] since the connection opened (hellos and blank lines don't restart it; none
    /// left closes the connection).
    pub(super) fn budgets(&self, idle: Duration, line: Duration) -> (Duration, Duration) {
        if self.authenticated.get() {
            return (idle, line);
        }
        let left = AUTH_WINDOW.saturating_sub(self.opened.elapsed());
        (left.min(idle), left.min(line))
    }

    /// A line just read, before anything parses it. On the agents port: a line whose tail carries
    /// the token passes (the first one moves the connection into the main pool); a hello is
    /// answered with the server's proof; anything else is refused and closes the connection.
    pub(super) fn screen(&self, line: &[u8]) -> Screen {
        let Some(secrets) = &self.secrets else { return Screen::Pass };
        if auth::has_token(line, &secrets.token) {
            if !self.authenticated.get() {
                if let Some(main) = &self.main {
                    let Some(slot) = acquire_slot_of(main, MAX_CONNECTIONS) else {
                        return Screen::Refuse(json!({"ok": false, "error": "too many control connections"}));
                    };
                    // The pending slot goes back to its pool here.
                    *self.slot.borrow_mut() = Some(slot);
                }
                self.authenticated.set(true);
            }
            return Screen::Pass;
        }
        if line.trim_ascii().is_empty() {
            return Screen::Pass;
        }
        match auth::answer_hello(line.trim_ascii(), &secrets.server_key) {
            Some(proof) => Screen::Answer(proof),
            None => Screen::Refuse(json!({"ok": false, "error": UNAUTHORIZED})),
        }
    }

    /// The line just read, judged: `Err(reply)` refuses it (it runs nothing); `Ok(actor)` is who
    /// sent it, `guarded` when it came through the always-on listener.
    pub(super) fn admit(&self) -> Result<Value, Value> {
        let line = take_line();
        if let Some(secrets) = &self.secrets
            && !line.token.as_deref().is_some_and(|t| auth::same(t.as_bytes(), secrets.token.as_bytes()))
        {
            // [`Link::screen`] let only lines with the token through; this is the second look.
            return Err(json!({"ok": false, "error": UNAUTHORIZED}));
        }
        let mut actor = line.actor.filter(Value::is_object).unwrap_or_else(|| json!({"session": format!("connection-{}", self.connection)}));
        actor["guarded"] = json!(self.secrets.is_some());
        Ok(actor)
    }
}

/// Listen on a fresh loopback port and publish it with fresh secrets in `dir`. `None` (and a line
/// on stderr) when either fails: the app then runs without agents, as it would without `--control`.
pub(crate) fn start_default(dir: &Path, ctx: egui::Context) -> Option<Receiver<ControlRequest>> {
    prune_stale(dir);
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| eprintln!("varos: agents listener failed to bind: {e}")).ok()?;
    let port = listener.local_addr().ok()?.port();
    let ep = Endpoint::new(port).map_err(|e| eprintln!("varos: no random source for the agents secrets: {e}")).ok()?;
    let path = endpoint::publish(dir, &ep).map_err(|e| eprintln!("varos: cannot write the agents endpoint in {}: {e}", dir.display())).ok()?;
    SECRETS.lock().unwrap_or_else(PoisonError::into_inner).push((port, Secrets { token: ep.token.clone(), server_key: ep.server_key.clone() }));
    *ENDPOINT_FILE.lock().unwrap_or_else(PoisonError::into_inner) = Some(path.clone());
    let (tx, rx) = channel::<ControlRequest>();
    std::thread::spawn(move || accept_loop(listener, tx, ctx));
    eprintln!("varos: agents welcome on 127.0.0.1:{port} (endpoint {})", path.display());
    Some(rx)
}

/// The agents port's accept loop: a new connection takes a slot of the small pending pool, and
/// moves into the main pool when it shows the token ([`Link::screen`]).
fn accept_loop(listener: TcpListener, tx: Sender<ControlRequest>, ctx: egui::Context) {
    let main = Arc::new(AtomicUsize::new(0));
    let pending = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming().flatten() {
        let Some(slot) = acquire_slot_of(&pending, MAX_PENDING) else {
            let mut out = super::DeadlineWriter::new(stream, AUTH_WINDOW);
            // Best effort: the connection is dropped either way.
            if out.begin_reply().is_ok() {
                let _ = super::write_line(&mut out, &json!({"ok": false, "error": "too many control connections"}));
            }
            continue;
        };
        let link = Link::new(&stream, slot).moving_into(Arc::clone(&main));
        if let Err(e) = super::spawn_link(stream, tx.clone(), ctx.clone(), link, ConnectionTimeouts::default()) {
            eprintln!("varos: agents connection worker failed to start: {e}");
        }
    }
}

/// Remove the endpoint file this launch published (at quit).
pub(crate) fn remove_endpoint() {
    if let Some(path) = ENDPOINT_FILE.lock().unwrap_or_else(PoisonError::into_inner).take() {
        endpoint::remove(&path);
    }
}

/// What a published endpoint's port says when asked for the proof.
#[derive(Debug, PartialEq)]
enum Probe {
    /// The Varos the file names: it proved it holds the file's server key.
    Varos,
    /// Nothing listens there, it closed without an answer, or it answered without the proof: a
    /// launch that crashed (and maybe another program on its port now).
    Stale,
    /// No answer in time, or too busy to take one more connection: maybe a Varos that is stopped
    /// or loaded. Kept.
    Unknown,
}

fn probe(ep: &Endpoint) -> Probe {
    let Ok(addr) = ep.addr().parse() else { return Probe::Stale };
    let Ok(stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(200)) else { return Probe::Stale };
    let Ok(nonce) = auth::new_secret() else { return Probe::Unknown };
    if stream.set_read_timeout(Some(Duration::from_millis(500))).is_err() || stream.set_write_timeout(Some(Duration::from_millis(500))).is_err() {
        return Probe::Unknown;
    }
    let mut out = &stream;
    if out.write_all(format!("{}\n", auth::hello_line(&nonce)).as_bytes()).is_err() {
        return Probe::Stale;
    }
    let mut reply = Vec::new();
    match BufReader::new(&stream).take(auth::MAX_HELLO as u64 + 1).read_until(b'\n', &mut reply) {
        Ok(0) => Probe::Stale,
        Ok(_) if auth::proves(&String::from_utf8_lossy(&reply), &ep.server_key, &nonce) => Probe::Varos,
        Ok(_) if String::from_utf8_lossy(&reply).contains("too many control connections") => Probe::Unknown,
        Ok(_) => Probe::Stale,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => Probe::Unknown,
        Err(_) => Probe::Stale,
    }
}

/// Remove endpoint files of launches that crashed: their port answers nothing, or something there
/// answers without the proof that it is that Varos ([`probe`]). A port that is slow to answer is
/// left alone.
fn prune_stale(dir: &Path) {
    for ep in endpoint::list(dir) {
        if probe(&ep) == Probe::Stale {
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

    fn slot() -> ConnectionSlot {
        acquire_slot_of(&Arc::new(AtomicUsize::new(0)), 1).unwrap()
    }

    /// A link to the `--control` port (`None`), or to an agents port with these secrets.
    fn open_link(secrets: Option<(&str, &str)>) -> Link {
        Link {
            connection: 7,
            secrets: secrets.map(|(token, server_key)| Secrets { token: token.into(), server_key: server_key.into() }),
            opened: Instant::now(),
            authenticated: Cell::new(secrets.is_none()),
            slot: RefCell::new(Some(slot())),
            main: None,
        }
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
        let (token, key) = ("ab".repeat(32), "cd".repeat(32));
        let link = || open_link(Some((&token, &key)));
        for bad in [json!({"method": "x"}), json!({"method": "x", "token": "ab"}), json!({"method": "x", "token": "cd".repeat(32)})] {
            assert!(matches!(link().screen(bad.to_string().as_bytes()), Screen::Refuse(_)), "{bad}");
            set_line(line(bad));
            assert!(link().admit().unwrap_err()["error"].as_str().unwrap().starts_with("unauthorized"));
        }
        // The token only inside the params is not the line's.
        let nested = format!("{{\"method\":\"x\",\"params\":{{\"token\":\"{token}\"}}}}");
        assert!(matches!(link().screen(nested.as_bytes()), Screen::Refuse(_)));
        // With it last on the line, it passes, and the actor is guarded whatever the client claims.
        let good = auth::with_token(json!({"method": "x", "actor": {"session": "s", "guarded": false}}).to_string(), &token);
        let l = link();
        assert!(matches!(l.screen(good.as_bytes()), Screen::Pass));
        set_line(line(serde_json::from_str(&good).unwrap()));
        assert_eq!(l.admit().unwrap()["guarded"], true);
        // Upstream's `--control` port asks for nothing and guards nothing.
        assert!(matches!(open_link(None).screen(b"{\"method\":\"x\"}"), Screen::Pass));
        set_line(line(json!({"method": "x", "actor": {"session": "s", "guarded": true}})));
        assert_eq!(open_link(None).admit().unwrap()["guarded"], false);
    }

    #[test]
    fn the_agents_port_answers_a_hello_with_its_proof() {
        let (token, key) = (auth::new_secret().unwrap(), auth::new_secret().unwrap());
        let nonce = auth::new_secret().unwrap();
        let Screen::Answer(reply) = open_link(Some((&token, &key))).screen(auth::hello_line(&nonce).as_bytes()) else {
            panic!("a hello is answered")
        };
        assert!(auth::proves(&reply.to_string(), &key, &nonce));
        assert!(!auth::proves(&reply.to_string(), &token, &nonce), "the token is not the server key");
    }

    /// Until the token is shown, a connection has [`AUTH_WINDOW`] in all, whatever it sends.
    #[test]
    fn the_unauthenticated_window_is_one_window() {
        let (token, key) = ("ab".repeat(32), "cd".repeat(32));
        let mut l = open_link(Some((&token, &key)));
        let long = Duration::from_secs(300);
        assert!(l.budgets(long, long).0 <= AUTH_WINDOW);
        l.opened = Instant::now() - AUTH_WINDOW;
        assert_eq!(l.budgets(long, long), (Duration::ZERO, Duration::ZERO), "used up: the next read fails at once");
        assert!(matches!(l.screen(auth::with_token("{}".into(), &token).as_bytes()), Screen::Pass));
        assert_eq!(l.budgets(long, long), (long, long), "the token lifts it");
    }

    /// An agents port on a fresh port with fresh secrets, as [`start_default`] makes one (without
    /// an endpoint file).
    fn agents_port() -> (Endpoint, Receiver<ControlRequest>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let ep = Endpoint::new(listener.local_addr().unwrap().port()).unwrap();
        SECRETS.lock().unwrap().push((ep.port, Secrets { token: ep.token.clone(), server_key: ep.server_key.clone() }));
        let (tx, rx) = channel();
        std::thread::spawn(move || accept_loop(listener, tx, egui::Context::default()));
        (ep, rx)
    }

    fn connect(port: u16) -> (TcpStream, BufReader<TcpStream>) {
        let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let r = BufReader::new(s.try_clone().unwrap());
        (s, r)
    }

    fn read_reply(r: &mut BufReader<TcpStream>) -> String {
        let mut reply = String::new();
        r.read_line(&mut reply).unwrap();
        reply
    }

    /// Peers that never show the token have a small pool of their own and two seconds: they can't
    /// hold the agents' slots, and they are closed when their window ends.
    #[test]
    fn unauthenticated_peers_get_a_small_pool_and_a_short_window() {
        let (ep, rx) = agents_port();
        let idle: Vec<_> = (0..MAX_PENDING).map(|_| connect(ep.port)).collect();
        let (_extra, mut r) = connect(ep.port);
        assert!(read_reply(&mut r).contains("too many control connections"));
        // A held peer that keeps sending hellos is still closed when its window ends.
        let (mut chatty, mut chatty_r) = idle.into_iter().next().unwrap();
        let started = std::time::Instant::now();
        let mut closed = false;
        while started.elapsed() < AUTH_WINDOW * 3 {
            if writeln!(chatty, "{}", auth::hello_line(&auth::new_secret().unwrap())).is_err() {
                closed = true;
                break;
            }
            let mut reply = String::new();
            if chatty_r.read_line(&mut reply).map_or(true, |n| n == 0) {
                closed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        assert!(closed, "the window was restarted by its lines");
        assert!(started.elapsed() < AUTH_WINDOW + Duration::from_secs(2));
        // The pool is free again: the user's agent gets in.
        let (mut s, mut r) = connect(ep.port);
        let app = std::thread::spawn(move || {
            let req = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            req.reply.send(json!({"ok": true, "result": 1})).unwrap();
        });
        writeln!(s, "{}", auth::with_token(json!({"id": 3, "method": "document.inspect"}).to_string(), &ep.token)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&read_reply(&mut r)).unwrap(), json!({"id": 3, "ok": true, "result": 1}));
        app.join().unwrap();
    }

    /// A stand-in on `port` that answers a hello with a proof made from the wrong key.
    fn impostor() -> TcpListener {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let l = listener.try_clone().unwrap();
        std::thread::spawn(move || {
            let wrong = auth::new_secret().unwrap();
            for stream in l.incoming().flatten() {
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if r.read_line(&mut line).is_ok()
                    && let Some(reply) = auth::answer_hello(line.trim().as_bytes(), &wrong)
                {
                    let _ = writeln!(&stream, "{reply}");
                }
            }
        });
        listener
    }

    /// The always-on listener end to end over TCP: stale files go (nothing on the port, or something
    /// that can't prove it is that Varos), a slow one stays; the endpoint file is private; a line
    /// without the token is refused; the hello is answered with the proof; a line with the token
    /// reaches the app guarded; and quitting removes the file.
    #[test]
    fn the_always_on_listener_lets_in_only_the_token() {
        let dir = std::env::temp_dir().join(format!("varos-listener-{}", std::process::id())).join("agents");
        let _ = std::fs::remove_dir_all(&dir);
        let publish_as = |pid: u32, port: u16| {
            let mut e = Endpoint::new(port).unwrap();
            e.pid = pid;
            endpoint::publish(&dir, &e).unwrap();
        };
        let dead = TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
        publish_as(1, dead);
        let other = impostor();
        publish_as(2, other.local_addr().unwrap().port());
        // Accepts (the listen backlog) and never answers: maybe a stopped Varos. Kept.
        let silent = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        publish_as(3, silent.local_addr().unwrap().port());
        let rx = start_default(&dir, egui::Context::default()).expect("listening");
        let listed = endpoint::list(&dir);
        let pids: Vec<u32> = listed.iter().map(|e| e.pid).collect();
        assert!(!pids.contains(&1) && !pids.contains(&2), "stale files are gone: {pids:?}");
        assert!(pids.contains(&3), "a port that is only slow is left alone: {pids:?}");
        let me = listed.iter().find(|e| e.pid == std::process::id()).unwrap().clone();
        let file = endpoint::file_for(&dir, std::process::id());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let ask = |line: String| -> Value {
            let (mut s, mut r) = connect(me.port);
            writeln!(s, "{line}").unwrap();
            serde_json::from_str(&read_reply(&mut r)).unwrap()
        };
        let r = ask(json!({"id": 1, "method": "document.inspect"}).to_string());
        assert!(r["error"].as_str().unwrap().starts_with("unauthorized"), "{r}");
        assert!(rx.try_recv().is_err(), "a refused line reaches nothing");
        let nonce = auth::new_secret().unwrap();
        assert!(auth::proves(&ask(auth::hello_line(&nonce)).to_string(), &me.server_key, &nonce));
        // The app's side: answer the next request and keep its actor.
        let app = std::thread::spawn(move || {
            let req = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let actor = req.actor.clone();
            req.reply.send(json!({"ok": true, "result": 1})).unwrap();
            actor
        });
        let line = json!({"id": 2, "method": "document.inspect", "actor": {"session": "s1", "label": "Claude Code"}}).to_string();
        let r = ask(auth::with_token(line, &me.token));
        assert_eq!(r, json!({"id": 2, "ok": true, "result": 1}));
        let actor = app.join().unwrap().unwrap();
        assert_eq!(actor["guarded"], true);
        assert_eq!(actor["label"], "Claude Code");
        remove_endpoint();
        assert!(!file.exists());
        drop(silent);
    }

    /// The real MCP proxy against the real listener: it finds this launch through the endpoint
    /// file, the listener proves it is this Varos, and the request arrives with its agent, guarded.
    #[test]
    fn the_mcp_proxy_attaches_to_the_listener() {
        use vectorcraft_mcp::Backend;
        let dir = std::env::temp_dir().join(format!("varos-proxy-e2e-{}", std::process::id())).join("agents");
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let ep = Endpoint::new(listener.local_addr().unwrap().port()).unwrap();
        endpoint::publish(&dir, &ep).unwrap();
        SECRETS.lock().unwrap().push((ep.port, Secrets { token: ep.token.clone(), server_key: ep.server_key.clone() }));
        let (tx, rx) = channel();
        std::thread::spawn(move || accept_loop(listener, tx, egui::Context::default()));
        let app = std::thread::spawn(move || {
            let mut actors = vec![];
            for _ in 0..2 {
                let req = rx.recv_timeout(Duration::from_secs(10)).unwrap();
                actors.push(req.actor.clone());
                req.reply.send(json!({"ok": true, "result": {"method": req.method}})).unwrap();
            }
            actors
        });
        let mut proxy = vectorcraft_mcp::VarosAttach::in_dir(&dir);
        proxy.set_client(&json!({"name": "claude-code"}));
        assert_eq!(proxy.call("document.inspect", json!({})), Ok(json!({"method": "document.inspect"})));
        assert_eq!(proxy.call("ui.inspect", json!({})), Ok(json!({"method": "ui.inspect"})));
        for actor in app.join().unwrap() {
            let actor = actor.unwrap();
            assert_eq!(actor["guarded"], true);
            assert_eq!(actor["label"], "Claude Code");
        }
    }
}
