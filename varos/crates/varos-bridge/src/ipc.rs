//! Local attachment only; no TCP fallback. Two listeners share one epoch and one Service:
//! - paired (ADR-0011 C1): registry entry + signed handshake + open local trust and agent profile ids;
//! - legacy (ADR-0009 slice 1, deprecated): per-launch endpoint file + bearer token, opt-in only
//!   (`VAROS_BRIDGE_LEGACY=1`) for one transition slice, on its own socket and audited as agent
//!   `legacy`. Legacy auth is never accepted on the paired socket.
use crate::{conn, Context, Error, Reply, Request, MAX_FRAME};
#[cfg(unix)]
use crate::{conn::trust::Scopes, API};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::time::Duration;
use std::{
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};

pub fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = vec![];
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, "unterminated frame"))
            };
        }
        let n = buf.iter().position(|b| *b == b'\n').map(|n| n + 1).unwrap_or(buf.len());
        if bytes.len() + n > MAX_FRAME {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame exceeds 1 MiB"));
        }
        bytes.extend_from_slice(&buf[..n]);
        reader.consume(n);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            return Ok(Some(bytes));
        }
    }
}
pub fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()
}
pub fn random_id() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub api: String,
    pub socket: PathBuf,
    pub token: String,
    pub epoch: String,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Frame {
    Hello { api: String, token: String, client: String },
    Call { call_id: String, request: Request },
    Cancel { call_id: String },
}
/// Authentication happens before DTOs enter the app queue. The host owns the only service.
#[derive(Clone, Debug)]
pub struct Pending {
    pub context: Context,
    pub request: Request,
    pub cancelled: Arc<AtomicBool>,
    pub reply: mpsc::SyncSender<Reply>,
    pub file_audit: Option<(conn::Paths, conn::audit::Entry)>,
}
impl PartialEq for Pending {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}
#[derive(Clone)]
pub struct Client {
    endpoint: PathBuf,
    token: String,
    client: String,
}
impl Client {
    pub fn new(endpoint: PathBuf, token: String) -> io::Result<Self> {
        Ok(Self { endpoint, token, client: random_id()? })
    }
    pub fn with_client(mut self, client: String) -> io::Result<Self> {
        if client.len() != 64 || !client.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "client id must be 64 hex digits"));
        }
        self.client = client;
        Ok(self)
    }
    pub fn call(&self, call_id: &str, request: Request) -> Reply {
        self.exchange(Frame::Call { call_id: call_id.into(), request }).unwrap_or_else(|e| {
            Reply::failure(Error::new(
                "io_error",
                format!("attachment: {e}; query request_status before retrying an edit"),
            ))
        })
    }
    pub fn cancel(&self, call_id: &str) {
        let _ = self.exchange(Frame::Cancel { call_id: call_id.into() });
    }
    #[cfg(unix)]
    fn exchange(&self, frame: Frame) -> io::Result<Reply> {
        let mut stream = connect_checked(&self.endpoint)?;
        write_frame(
            &mut stream,
            &Frame::Hello { api: API.into(), token: self.token.clone(), client: self.client.clone() },
        )?;
        let mut read = std::io::BufReader::new(stream.try_clone()?);
        let hello: Reply =
            serde_json::from_slice(&read_frame(&mut read)?.ok_or_else(|| io::Error::other("host disconnected"))?)?;
        if !hello.ok {
            return Ok(hello);
        }
        write_frame(&mut stream, &frame)?;
        serde_json::from_slice(&read_frame(&mut read)?.ok_or_else(|| io::Error::other("host disconnected"))?)
            .map_err(io::Error::other)
    }
    #[cfg(not(unix))]
    fn exchange(&self, _frame: Frame) -> io::Result<Reply> {
        let _ = (&self.endpoint, &self.token, &self.client);
        Ok(Reply::failure(Error::new("unsupported", "native attachment is Unix-only; Windows is compile-only")))
    }
}
/// Connect to an owner-only socket and verify the peer uid (agent side).
#[cfg(unix)]
pub(crate) fn connect_checked(path: &Path) -> io::Result<std::os::unix::net::UnixStream> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "endpoint must be an owner-only socket"));
    }
    let stream = std::os::unix::net::UnixStream::connect(path)?;
    check_peer(&stream)?;
    stream.set_read_timeout(Some(Duration::from_secs(35)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}
#[cfg(unix)]
pub(crate) fn check_peer(stream: &std::os::unix::net::UnixStream) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    #[cfg(target_os = "macos")]
    let uid = {
        let mut uid = 0;
        let mut gid = 0;
        // SAFETY: getpeereid writes exactly two uid/gid values for this live Unix stream.
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        uid
    };
    #[cfg(target_os = "linux")]
    let uid = {
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: the correctly sized ucred and socklen buffers live for this call.
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut cred as *mut libc::ucred).cast(),
                &mut len,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        cred.uid
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err(io::Error::new(io::ErrorKind::Unsupported, "peer credentials unavailable"));
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    authorize_uid(uid, unsafe { libc::geteuid() })
}
pub fn authorize_uid(peer: u32, owner: u32) -> io::Result<()> {
    if peer == owner {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "peer uid differs from owner"))
    }
}
pub fn authorize_token(actual: &str, expected: &str) -> io::Result<()> {
    let mut diff = actual.len() ^ expected.len();
    for i in 0..expected.len() {
        diff |= usize::from(actual.as_bytes().get(i).copied().unwrap_or(0) ^ expected.as_bytes()[i]);
    }
    if diff == 0 {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "invalid attachment capability"))
    }
}
/// Host-side paired-listener configuration (ADR-0011 C1).
pub struct PairedConfig {
    pub paths: conn::Paths,
    /// The per-user host identity, loaded from the CredentialStore by the caller.
    pub host_key: ed25519_dalek::SigningKey,
    /// `desktop` or `headless`.
    pub mode: &'static str,
    pub app_build: String,
}
/// Loads the host identity. `Deferred` runs on a background thread so a slow or unavailable
/// file store never blocks the app window.
#[allow(clippy::large_enum_variant)] // built once per launch
pub enum PairedSource {
    Ready(PairedConfig),
    Deferred(Box<dyn FnOnce() -> Result<PairedConfig, Error> + Send>),
}
pub struct Options {
    /// Deprecated slice-1 token endpoint (one transition slice; opt-in).
    pub legacy: bool,
    pub paired: Option<PairedSource>,
    /// Where legacy (token) calls are audited as agent `legacy`.
    pub legacy_audit: Option<conn::Paths>,
}
/// The deprecated token listener runs on the desktop only when this is `1` (default off).
pub const LEGACY_ENV: &str = "VAROS_BRIDGE_LEGACY";

/// The paired listener's state, observable by the host (for its notice path).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairedStatus {
    Off,
    Starting,
    Ready,
    /// Not listening; the reason is owner-readable (e.g. `credential_unavailable: …`).
    Unavailable(String),
}
#[derive(Default)]
struct PairedState {
    status: Option<PairedStatus>,
    entry: Option<conn::registry::Entry>,
    /// The paired accept thread and the socket it blocks on (`Drop` connects once to wake it).
    #[cfg(unix)]
    thread: Option<(std::thread::JoinHandle<()>, PathBuf)>,
    closed: bool,
    notice_taken: bool,
}

#[cfg(unix)]
struct PairedHost {
    key: ed25519_dalek::SigningKey,
    instance_id: String,
    paths: conn::Paths,
}
#[cfg(unix)]
enum Auth {
    Legacy { token: String },
    Paired(Arc<PairedHost>),
}
/// What a successful authentication admits for the one call on this connection.
#[cfg(unix)]
struct Admitted {
    client: String,
    /// Audited calls: (paths, agent, session prefix). Legacy calls use agent `legacy`.
    audit: Option<(conn::Paths, String, String, String)>,
}
#[cfg(unix)]
#[derive(Clone)]
struct Shared {
    queue: mpsc::SyncSender<Pending>,
    wake: Arc<dyn Fn() + Send + Sync>,
    stopped: Arc<AtomicBool>,
    attached: Arc<std::sync::atomic::AtomicUsize>,
    epoch: String,
    cancellations: Arc<std::sync::Mutex<CancelMap>>,
    legacy_audit: Option<conn::Paths>,
}
#[cfg(unix)]
type CancelMap = std::collections::HashMap<(String, String), (Arc<AtomicBool>, std::time::Instant)>;
/// Accepted file jobs retain the request flag after the socket reply, so Cancel still reaches them.
#[cfg(unix)]
fn release_call_cancellation(map: &mut CancelMap, key: &(String, String), flag: &Arc<AtomicBool>) {
    // The map and this call own two references; an extra owner is queued/running work.
    if Arc::strong_count(flag) <= 2 {
        map.remove(key);
    }
}

/// Kept for the lifetime of the app event loop. Dropping removes endpoints and revokes workers.
pub struct Listener {
    /// The legacy endpoint file when the legacy listener runs (else empty).
    pub endpoint_file: PathBuf,
    epoch: String,
    stop: Arc<AtomicBool>,
    attached: Arc<std::sync::atomic::AtomicUsize>,
    /// Accept threads (each with the socket it blocks on, woken once by `Drop`) and the
    /// deferred host-key loader (no socket; it ends when the load returns).
    #[cfg(unix)]
    threads: Vec<(std::thread::JoinHandle<()>, Option<PathBuf>)>,
    legacy_dir: Option<PathBuf>,
    paired: Arc<std::sync::Mutex<PairedState>>,
}
impl Listener {
    /// Legacy-only listener (slice-1 contract; tests and the deprecated transition path).
    pub fn start(queue: mpsc::SyncSender<Pending>, wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        Self::start_with(queue, wake, Options { legacy: true, paired: None, legacy_audit: None })
    }
    /// Desktop host: the paired listener, whose host key is loaded from the platform
    /// CredentialStore on a background thread (the window never waits for file IO), plus
    /// the deprecated token listener only when `VAROS_BRIDGE_LEGACY=1`. A failed key load
    /// (`credential_unavailable`, e.g. unsafe key file permissions) leaves Varos running without the
    /// paired listener and is reported through [`Listener::take_unavailable_notice`].
    pub fn start_desktop(
        queue: mpsc::SyncSender<Pending>,
        wake: impl Fn() + Send + Sync + 'static,
        app_build: &str,
    ) -> io::Result<Self> {
        let legacy = std::env::var(LEGACY_ENV).as_deref() == Ok("1");
        let app_build = app_build.to_owned();
        let deferred = move || -> Result<PairedConfig, Error> {
            let paths = conn::Paths::resolve()?;
            let store = conn::credentials::platform_store();
            let host_key = conn::credentials::load_or_create(store.as_ref(), conn::credentials::HOST_ACCOUNT)?;
            Ok(PairedConfig { paths, host_key, mode: "desktop", app_build })
        };
        let legacy_audit = if legacy { conn::Paths::resolve().ok() } else { None };
        Self::start_with(
            queue,
            wake,
            Options { legacy, paired: Some(PairedSource::Deferred(Box::new(deferred))), legacy_audit },
        )
    }
    #[cfg(unix)]
    pub fn start_with(
        queue: mpsc::SyncSender<Pending>,
        wake: impl Fn() + Send + Sync + 'static,
        options: Options,
    ) -> io::Result<Self> {
        use std::os::unix::{
            fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
            net::UnixListener,
        };
        let epoch = random_id()?;
        let shared = Shared {
            queue,
            wake: Arc::new(wake),
            stopped: Arc::new(AtomicBool::new(false)),
            attached: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            epoch: epoch.clone(),
            cancellations: Arc::new(std::sync::Mutex::new(CancelMap::new())),
            legacy_audit: options.legacy_audit,
        };
        let mut this = Self {
            endpoint_file: PathBuf::new(),
            epoch: epoch.clone(),
            stop: shared.stopped.clone(),
            attached: shared.attached.clone(),
            threads: vec![],
            legacy_dir: None,
            paired: Arc::new(std::sync::Mutex::new(PairedState {
                status: Some(PairedStatus::Off),
                ..Default::default()
            })),
        };
        // Any early return drops `this`, which stops threads and removes what was published.
        match options.paired {
            None => {}
            Some(PairedSource::Ready(config)) => {
                bring_up_paired(config, &shared, &this.paired).map_err(|e| io::Error::other(e.reason))?;
            }
            Some(PairedSource::Deferred(load)) => {
                this.paired.lock().unwrap().status = Some(PairedStatus::Starting);
                let (shared, state) = (shared.clone(), this.paired.clone());
                let loader = std::thread::spawn(move || {
                    let outcome = load().and_then(|config| bring_up_paired(config, &shared, &state));
                    if let Err(e) = outcome {
                        eprintln!("[varos-bridge] paired attachment unavailable: {}: {}", e.code, e.reason);
                        state.lock().unwrap().status =
                            Some(PairedStatus::Unavailable(format!("{}: {}", e.code, e.reason)));
                    }
                    (shared.wake)();
                });
                this.threads.push((loader, None));
            }
        }
        if options.legacy {
            let token = random_id()?;
            let dir = std::env::temp_dir().join(format!("varos-bridge-{}-{}", std::process::id(), &epoch[..12]));
            std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
            this.legacy_dir = Some(dir.clone());
            let socket = dir.join("bridge.sock");
            let listener = UnixListener::bind(&socket)?;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
            // BLOCKING accept (no timed poll at rest); `Drop` wakes it through this socket.
            let wake_socket = socket.clone();
            let endpoint_file = dir.join("endpoint.json");
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&endpoint_file)?;
            write_frame(&mut f, &Endpoint { api: API.into(), socket, token: token.clone(), epoch: epoch.clone() })?;
            this.endpoint_file = endpoint_file;
            let accept = spawn_accept(listener, Arc::new(Auth::Legacy { token }), shared.clone());
            this.threads.push((accept, Some(wake_socket)));
        }
        Ok(this)
    }
    #[cfg(not(unix))]
    pub fn start_with(
        _queue: mpsc::SyncSender<Pending>,
        _wake: impl Fn() + Send + Sync + 'static,
        _options: Options,
    ) -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "native attachment unsupported"))
    }
    pub fn has_clients(&self) -> bool {
        self.attached.load(Ordering::Acquire) > 0
    }
    pub fn epoch(&self) -> io::Result<String> {
        Ok(self.epoch.clone())
    }
    pub fn paired_status(&self) -> PairedStatus {
        self.paired.lock().unwrap().status.clone().unwrap_or(PairedStatus::Off)
    }
    /// This host's discovery registry record, once the paired listener is up.
    pub fn registry_file(&self) -> Option<PathBuf> {
        self.paired.lock().unwrap().entry.as_ref().map(|e| e.dir.join(conn::registry::RECORD_FILE))
    }
    /// Once per launch: the owner-readable reason agents cannot connect (for the host's notice).
    pub fn take_unavailable_notice(&self) -> Option<String> {
        let mut state = self.paired.lock().unwrap();
        match (&state.status, state.notice_taken) {
            (Some(PairedStatus::Unavailable(reason)), false) => {
                let reason = reason.clone();
                state.notice_taken = true;
                Some(reason)
            }
            _ => None,
        }
    }
    /// One-line diagnostic for the desktop's stderr (paths only, never tokens or keys).
    pub fn diagnostic(&self) -> String {
        let mut parts = vec![match (self.paired_status(), self.registry_file()) {
            (PairedStatus::Ready, Some(r)) => format!("registry {}", r.display()),
            (PairedStatus::Starting, _) => "paired listener starting (loading the host key)".into(),
            (PairedStatus::Unavailable(reason), _) => format!("paired listener unavailable: {reason}"),
            _ => "paired listener off".into(),
        }];
        if let Some(d) = &self.legacy_dir {
            parts.push(format!("legacy token endpoint (deprecated, opt-in) {}", d.join("endpoint.json").display()));
        }
        parts.join("; ")
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // `closed` under the lock: a deferred bring-up that has not published yet never will.
        let (entry, paired_thread) = {
            let mut state = self.paired.lock().unwrap();
            state.closed = true;
            #[cfg(unix)]
            let thread = state.thread.take();
            #[cfg(not(unix))]
            let thread: Option<()> = None;
            (state.entry.take(), thread)
        };
        #[cfg(unix)]
        {
            let threads: Vec<_> =
                self.threads.drain(..).chain(paired_thread.map(|(h, socket)| (h, Some(socket)))).collect();
            // Wake every accept thread first (one connection to its own socket each; a failed
            // connect means it is gone already), then join each with a 1-second bound.
            for (_, socket) in &threads {
                if let Some(socket) = socket {
                    let _ = std::os::unix::net::UnixStream::connect(socket);
                }
            }
            for (h, _) in threads {
                let deadline = std::time::Instant::now() + Duration::from_secs(1);
                while !h.is_finished() && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(5));
                }
                if h.is_finished() {
                    let _ = h.join();
                }
            }
        }
        #[cfg(not(unix))]
        let _ = paired_thread;
        // Sockets/records are removed only after the threads were woken through them.
        if let Some(entry) = entry {
            entry.remove();
        }
        if let Some(dir) = self.legacy_dir.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Reserve, bind, publish the registry record, then start the blocking accept thread. A client
/// that connects in between waits in the listen backlog; a publish failure leaves no thread.
#[cfg(unix)]
fn bring_up_paired(
    config: PairedConfig,
    shared: &Shared,
    state: &Arc<std::sync::Mutex<PairedState>>,
) -> Result<(), Error> {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    let ioe = |e: io::Error| Error::new("unsupported", format!("paired listener: {e}"));
    let paths = config.paths.clone();
    let cleaned = conn::registry::cleanup_stale(&conn::registry::scan(&paths), 16);
    if cleaned > 0 {
        eprintln!("[varos-bridge] removed {cleaned} stale registry entr(ies)");
    }
    let instance_id = conn::random_hex(8)?;
    let socket = conn::registry::Entry::reserve(&paths, &instance_id)?;
    // Until the record is published, a failure removes the reserved socket and folder.
    struct Reserved(PathBuf, bool);
    impl Drop for Reserved {
        fn drop(&mut self) {
            if !self.1 {
                let _ = conn::fsutil::remove_owned(&self.0);
                if let Some(dir) = self.0.parent() {
                    let _ = std::fs::remove_dir(dir);
                }
            }
        }
    }
    let mut reserved = Reserved(socket.clone(), false);
    let listener = UnixListener::bind(&socket).map_err(ioe)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).map_err(ioe)?;
    let pid = std::process::id();
    let fingerprint = conn::credentials::fingerprint(&config.host_key.verifying_key());
    let record = conn::registry::Record {
        discovery_version: conn::DISCOVERY_VERSION,
        instance_id: instance_id.clone(),
        epoch: shared.epoch.clone(),
        pid,
        start: conn::registry::process_start(pid)
            .ok_or_else(|| Error::new("unsupported", "cannot read this process's start identity"))?,
        mode: config.mode.into(),
        socket,
        connection: conn::registry::VersionRange { min: conn::CONNECTION.into(), max: conn::CONNECTION.into() },
        api: API.into(),
        app_build: config.app_build.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(128).collect(),
        host_fingerprint: fingerprint.clone(),
    };
    let host = Arc::new(PairedHost { key: config.host_key, instance_id, paths: paths.clone() });
    let wake_socket = record.socket.clone();
    let entry = conn::registry::Entry::publish(&paths, record)?;
    reserved.1 = true;
    // Spawn under the state lock so `Drop` either sees `closed` here or finds the thread to wake.
    let mut s = state.lock().unwrap();
    if s.closed {
        // The host shut down while the key was loading: never leave a record or thread behind.
        entry.remove();
        return Ok(());
    }
    let thread = spawn_accept(listener, Arc::new(Auth::Paired(host)), shared.clone());
    s.thread = Some((thread, wake_socket));
    eprintln!("[varos-bridge] registry {}", entry.dir.join(conn::registry::RECORD_FILE).display());
    s.entry = Some(entry);
    s.status = Some(PairedStatus::Ready);
    Ok(())
}

#[cfg(unix)]
fn spawn_accept(
    listener: std::os::unix::net::UnixListener,
    auth: Arc<Auth>,
    shared: Shared,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut workers = vec![];
        while !shared.stopped.load(Ordering::Acquire) {
            // BLOCKING accept: the thread sleeps in the kernel until a client (or `Drop`'s wake
            // connection) arrives — no timed poll at rest.
            let accepted = listener.accept();
            if shared.stopped.load(Ordering::Acquire) {
                break; // the wake connection from `Drop` (or anything racing it): never served
            }
            // Prune AFTER the (possibly hours-long) accept, so the worker cap sees the live set.
            workers.retain(|(h, _): &(std::thread::JoinHandle<()>, std::os::unix::net::UnixStream)| !h.is_finished());
            shared
                .cancellations
                .lock()
                .unwrap()
                .retain(|_, (flag, time)| Arc::strong_count(flag) > 1 || time.elapsed() < Duration::from_secs(60));
            match accepted {
                Ok((stream, _)) if workers.len() < 8 => {
                    let Ok(shutdown) = stream.try_clone() else { continue };
                    let (auth, shared) = (auth.clone(), shared.clone());
                    let worker = std::thread::spawn(move || {
                        // The listener blocks; stay explicit so the framed reads below never see WouldBlock.
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                        if let Err(e) = serve_connection(stream, &auth, &shared) {
                            eprintln!("[varos-bridge] connection refused ({:?})", e.kind());
                        }
                    });
                    workers.push((worker, shutdown));
                }
                Ok(_) => {}
                Err(e) if matches!(e.kind(), io::ErrorKind::Interrupted | io::ErrorKind::ConnectionAborted) => {}
                Err(_) => break,
            }
        }
        for (flag, _) in shared.cancellations.lock().unwrap().values() {
            flag.store(true, Ordering::Release);
        }
        for (_, stream) in &workers {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        while workers.iter().any(|(h, _)| !h.is_finished()) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        for (h, _) in workers {
            if h.is_finished() {
                let _ = h.join();
            }
        }
    })
}

/// Slice-1 Hello: bearer token + caller-chosen client id. Deprecated; its own socket only.
#[cfg(unix)]
fn legacy_hello(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    token: &str,
    epoch: &str,
    audit: &Option<conn::Paths>,
) -> io::Result<Option<Admitted>> {
    let hello: Frame = serde_json::from_slice(&read_frame(reader)?.ok_or_else(|| io::Error::other("missing hello"))?)?;
    let Frame::Hello { api, token: provided, client } = hello else {
        return Err(io::Error::other("hello required"));
    };
    if authorize_token(&provided, token).is_err()
        || client.len() != 64
        || !client.bytes().all(|b| b.is_ascii_hexdigit())
    {
        write_frame(writer, &Reply::failure(Error::new("scope_refused", "invalid capability, client or API")))?;
        return Ok(None);
    }
    if api != API {
        write_frame(writer, &Reply::failure(Error::new("unsupported", "Bridge API must be 1.0")))?;
        return Ok(None);
    }
    write_frame(writer, &Reply::success(serde_json::json!({"api":API,"epoch":epoch})))?;
    let audit =
        audit.clone().map(|paths| (paths, "legacy".to_owned(), client[..8].to_ascii_lowercase(), "legacy".into()));
    Ok(Some(Admitted { client, audit }))
}

/// Connection 1.0: host proof, agent proof, then every scope for the already checked local uid.
#[cfg(unix)]
fn paired_handshake(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    host: &Arc<PairedHost>,
    epoch: &str,
) -> io::Result<Option<Admitted>> {
    use conn::handshake;
    let refuse = |writer: &mut dyn Write, e: Error| -> io::Result<Option<Admitted>> {
        let mut w = writer;
        write_frame(&mut w, &Reply::failure(e))?;
        Ok(None)
    };
    let first = read_frame(reader)?.ok_or_else(|| io::Error::other("missing client_hello"))?;
    let hello: handshake::Frame = match serde_json::from_slice(&first) {
        Ok(f) => f,
        Err(_) => {
            return refuse(
                writer,
                Error::new(
                    "unsupported",
                    "connection 1.0 client_hello required; legacy token Hello is not accepted here",
                ),
            )
        }
    };
    let (mut state, challenge) = match handshake::HostSide::challenge(&host.key, &host.instance_id, epoch, &hello) {
        Ok(v) => v,
        Err(e) => return refuse(writer, e),
    };
    write_frame(writer, &challenge)?;
    let proof: handshake::Frame = match read_frame(reader)?.map(|b| serde_json::from_slice(&b)) {
        Some(Ok(f)) => f,
        _ => return refuse(writer, Error::new("identity_invalid", "client_proof required")),
    };
    let agent = match state.verify(&proof) {
        Ok(a) => a,
        Err(e) => return refuse(writer, e),
    };
    // check_peer has already established the same uid. Identity only attributes audit/history.
    let scopes = Scopes::ALL;
    write_frame(
        writer,
        &Reply::success(serde_json::json!({
            "connection": conn::CONNECTION, "api": API, "epoch": epoch, "instance": host.instance_id,
            "agent": agent.profile_id, "scopes": scopes, "trust": "local user",
        })),
    )?;
    Ok(Some(Admitted {
        client: format!("{}:{}", agent.profile_id, agent.session),
        audit: Some((host.paths.clone(), agent.profile_id, agent.session[..8].into(), agent.label)),
    }))
}

#[cfg(unix)]
fn serve_connection(stream: std::os::unix::net::UnixStream, auth: &Auth, shared: &Shared) -> io::Result<()> {
    check_peer(&stream)?;
    let mut reader = std::io::BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let admitted = match auth {
        Auth::Legacy { token } => legacy_hello(&mut reader, &mut writer, token, &shared.epoch, &shared.legacy_audit)?,
        Auth::Paired(host) => paired_handshake(&mut reader, &mut writer, host, &shared.epoch)?,
    };
    let Some(admitted) = admitted else { return Ok(()) };
    shared.attached.fetch_add(1, Ordering::AcqRel);
    struct Attached(Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Attached {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::AcqRel);
        }
    }
    let _attached = Attached(shared.attached.clone());
    let frame: Frame =
        serde_json::from_slice(&read_frame(&mut reader)?.ok_or_else(|| io::Error::other("missing call"))?)?;
    let client = admitted.client;
    match frame {
        Frame::Cancel { call_id } => {
            if call_id.len() > 128 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "call id too long"));
            }
            let mut map = shared.cancellations.lock().unwrap();
            if map.len() < 64 {
                map.entry((client, call_id))
                    .or_insert_with(|| (Arc::new(AtomicBool::new(false)), std::time::Instant::now()))
                    .0
                    .store(true, Ordering::Release);
            }
            drop(map);
            write_frame(&mut writer, &Reply::success(serde_json::json!({"cancellation":"requested"})))
        }
        Frame::Call { call_id, request } => {
            if call_id.len() > 128 {
                return Err(io::Error::other("call id too long"));
            }
            let key = (client.clone(), call_id);
            let flag = {
                let mut map = shared.cancellations.lock().unwrap();
                if map.len() >= 64 && !map.contains_key(&key) {
                    return write_frame(&mut writer, &Reply::failure(Error::new("busy", "cancellation registry full")));
                }
                map.entry(key.clone())
                    .or_insert_with(|| (Arc::new(AtomicBool::new(false)), std::time::Instant::now()))
                    .0
                    .clone()
            };
            let verb = request.tool();
            let board = request.board().map(str::to_owned);
            let mutation = request.mutation().map(|(id, rev)| (id.to_owned(), rev));
            let audit_entry = |result: &str, to_rev: Option<u64>| {
                admitted.audit.as_ref().map(|(paths, agent, session, label)| {
                    (
                        paths.clone(),
                        conn::audit::Entry {
                            ticket: None,
                            t: conn::now_secs(),
                            event: "call".into(),
                            agent: agent.clone(),
                            label: label.clone(),
                            session: session.clone(),
                            request_id: mutation.as_ref().map(|m| m.0.clone()),
                            verb: Some(verb.into()),
                            board: board.clone(),
                            from_rev: mutation.as_ref().map(|m| m.1),
                            to_rev,
                            result: result.into(),
                        },
                    )
                })
            };
            // Reserve audit capacity before a mutation: a failing audit denies the mutation.
            if mutation.is_some() {
                if let Some((paths, entry)) = audit_entry("admitted", None) {
                    if conn::audit::append(&paths, &entry).is_err() {
                        shared.cancellations.lock().unwrap().remove(&key);
                        return write_frame(
                            &mut writer,
                            &Reply::failure(Error::new(
                                "audit_unavailable",
                                "audit log unwritable; mutations are refused",
                            )),
                        );
                    }
                }
            }
            let (reply, rx) = mpsc::sync_channel(1);
            let pending = Pending {
                context: Context { client, epoch: shared.epoch.clone() },
                request,
                cancelled: flag.clone(),
                reply,
                file_audit: audit_entry("pending", None),
            };
            let result = if shared.queue.try_send(pending).is_err() {
                Reply::failure(Error::new("busy", "attachment queue is full"))
            } else {
                (shared.wake)();
                let started = std::time::Instant::now();
                loop {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(r) => break r,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            break Reply::failure(Error::new("io_error", "host stopped"))
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if shared.stopped.load(Ordering::Acquire) || started.elapsed() > Duration::from_secs(30) {
                                flag.store(true, Ordering::Release);
                                break Reply::failure(Error::new(
                                    "busy",
                                    "host did not drain before deadline; query receipt",
                                ));
                            }
                        }
                    }
                }
            };
            release_call_cancellation(
                &mut shared.cancellations.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                &key,
                &flag,
            );
            let code = result.error.as_ref().map_or("ok", |e| e.code.as_str()).to_owned();
            if let Some((paths, entry)) = audit_entry(&code, result.rev) {
                let _ = conn::audit::append(&paths, &entry);
            }
            write_frame(&mut writer, &result)
        }
        _ => Err(io::Error::other("unexpected hello")),
    }
}
/// Endpoint discovery prints owner-readable paths only; it never reads tokens implicitly.
#[cfg(unix)]
pub fn endpoint_paths() -> io::Result<Vec<PathBuf>> {
    let mut paths = vec![];
    for e in std::fs::read_dir(std::env::temp_dir())? {
        let e = e?;
        let name = e.file_name();
        if name.to_string_lossy().starts_with("varos-bridge-") {
            let path = e.path().join("endpoint.json");
            if secure_endpoint_file(&path).is_ok() {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}
#[cfg(not(unix))]
pub fn endpoint_paths() -> io::Result<Vec<PathBuf>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "native attachment unsupported"))
}
pub fn secure_endpoint_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(path)?;
        if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "endpoint file must be owner-only"));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(io::Error::new(io::ErrorKind::Unsupported, "native attachment unsupported"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Exercise the actual host handshake without a socket or any persistent trust state.
    #[cfg(unix)]
    #[test]
    fn local_handshake_grants_all_scopes_without_pairing_state() {
        use conn::{credentials, handshake};
        struct Output(Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for Output {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        struct Agent {
            input: io::Cursor<Vec<u8>>,
            output: Arc<std::sync::Mutex<Vec<u8>>>,
            state: handshake::AgentSide,
            expect: handshake::Expect,
            key: ed25519_dalek::SigningKey,
            proof_sent: bool,
        }
        impl io::Read for Agent {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                let available = self.fill_buf()?;
                let n = available.len().min(bytes.len());
                bytes[..n].copy_from_slice(&available[..n]);
                self.consume(n);
                Ok(n)
            }
        }
        impl BufRead for Agent {
            fn fill_buf(&mut self) -> io::Result<&[u8]> {
                if self.input.position() as usize == self.input.get_ref().len() && !self.proof_sent {
                    let challenge = serde_json::from_slice(&self.output.lock().unwrap()).unwrap();
                    let proof = self
                        .state
                        .respond(&challenge, &self.expect, &self.key, "abcdefabcdef0123", &"5".repeat(64), "Test Agent")
                        .unwrap();
                    let mut bytes = serde_json::to_vec(&proof).unwrap();
                    bytes.push(b'\n');
                    self.input = io::Cursor::new(bytes);
                    self.proof_sent = true;
                }
                self.input.fill_buf()
            }
            fn consume(&mut self, n: usize) {
                self.input.consume(n);
            }
        }
        let store = credentials::MemoryStore::new();
        let host_key = credentials::load_or_create(&store, credentials::HOST_ACCOUNT).unwrap();
        let agent_key = credentials::load_or_create(&store, "agent-test").unwrap();
        let expect = handshake::Expect {
            instance_id: "0123456789abcdef".into(),
            epoch: "e".repeat(64),
            host_fingerprint: credentials::fingerprint(&host_key.verifying_key()),
        };
        let paths =
            conn::Paths::under(std::env::temp_dir().join(format!("bridge-no-trust-{}", conn::random_hex(8).unwrap())));
        let host =
            Arc::new(PairedHost { key: host_key, instance_id: expect.instance_id.clone(), paths: paths.clone() });
        let (state, hello) = handshake::AgentSide::hello().unwrap();
        let mut bytes = serde_json::to_vec(&hello).unwrap();
        bytes.push(b'\n');
        let output = Arc::new(std::sync::Mutex::new(vec![]));
        let mut reader = Agent {
            input: io::Cursor::new(bytes),
            output: output.clone(),
            state,
            expect: expect.clone(),
            key: agent_key,
            proof_sent: false,
        };
        authorize_uid(501, 501).unwrap();
        assert!(authorize_uid(502, 501).is_err());
        let admitted =
            paired_handshake(&mut reader, &mut Output(output.clone()), &host, &expect.epoch).unwrap().unwrap();
        let (_, profile, _, label) = admitted.audit.unwrap();
        assert_eq!(profile, "abcdefabcdef0123");
        assert_eq!(label, "Test Agent");
        assert!(!paths.state.exists());
        assert!(!paths.runtime.exists());
        let output = output.lock().unwrap();
        let welcome: Reply = serde_json::from_slice(output.split(|b| *b == b'\n').nth(1).unwrap()).unwrap();
        assert_eq!(welcome.result.unwrap()["scopes"], serde_json::json!(Scopes::ALL));
    }

    /// The accept thread blocks in the kernel (no timed poll) and still stops promptly: `Drop`
    /// wakes it with one connection, so it never waits out the 1-second join deadline.
    #[cfg(unix)]
    #[ignore = "requires native Unix socket bind; sandbox denies bind with EPERM"]
    #[test]
    fn blocking_accept_thread_stops_promptly_on_drop() {
        let (tx, _rx) = mpsc::sync_channel(1);
        let listener = Listener::start(tx, || {}).unwrap();
        let dir = listener.endpoint_file.parent().unwrap().to_path_buf();
        std::thread::sleep(Duration::from_millis(200)); // the thread is parked in accept()
        let started = std::time::Instant::now();
        drop(listener);
        assert!(started.elapsed() < Duration::from_millis(500), "drop took {:?}", started.elapsed());
        assert!(!dir.exists(), "endpoint removed");
    }
    /// Both listeners (paired + legacy) block in accept and are each woken through their own
    /// socket: dropping the pair is still prompt and leaves no registry record or endpoint.
    #[cfg(unix)]
    #[ignore = "requires native Unix socket bind; sandbox denies bind with EPERM"]
    #[test]
    fn both_blocking_accept_threads_stop_promptly_on_drop() {
        use std::os::unix::fs::PermissionsExt;
        let home = std::env::temp_dir().join(format!("vb{}", conn::random_hex(3).unwrap()));
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        let host_key =
            conn::credentials::load_or_create(&conn::credentials::MemoryStore::new(), conn::credentials::HOST_ACCOUNT)
                .unwrap();
        let config =
            PairedConfig { paths: conn::Paths::under(&home), host_key, mode: "desktop", app_build: "test".into() };
        let (tx, _rx) = mpsc::sync_channel(1);
        let listener = Listener::start_with(
            tx,
            || {},
            Options { legacy: true, paired: Some(PairedSource::Ready(config)), legacy_audit: None },
        )
        .unwrap();
        let registry = listener.registry_file().unwrap();
        let legacy = listener.endpoint_file.parent().unwrap().to_path_buf();
        std::thread::sleep(Duration::from_millis(200)); // both threads parked in accept()
        let started = std::time::Instant::now();
        drop(listener);
        assert!(started.elapsed() < Duration::from_millis(500), "drop took {:?}", started.elapsed());
        assert!(!registry.exists() && !legacy.exists());
        let _ = std::fs::remove_dir_all(home);
    }
    #[test]
    fn hello_cannot_grant_history_or_destructive() {
        let hello = serde_json::json!({"kind":"hello","api":"1.0","token":"token","client":"client"});
        assert!(serde_json::from_value::<Frame>(hello.clone()).is_ok());
        for field in ["allow_history", "allow_destructive"] {
            let mut widened = hello.clone();
            widened[field] = serde_json::json!(true);
            assert!(serde_json::from_value::<Frame>(widened).is_err());
        }
    }
}

#[cfg(all(test, unix))]
mod queued_cancellation_tests {
    use super::*;
    #[test]
    fn accepted_worker_flag_remains_addressable_until_work_finishes() {
        let key = ("client".into(), "call".into());
        let flag = Arc::new(AtomicBool::new(false));
        let mut map = CancelMap::new();
        map.insert(key.clone(), (flag.clone(), std::time::Instant::now()));
        let worker = flag.clone();
        release_call_cancellation(&mut map, &key, &flag);
        map[&key].0.store(true, Ordering::Release);
        assert!(worker.load(Ordering::Acquire));
        drop(worker);
        release_call_cancellation(&mut map, &key, &flag);
        assert!(!map.contains_key(&key));
    }
}
