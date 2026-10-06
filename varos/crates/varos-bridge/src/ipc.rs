//! Local attachment only. Unix peer identity plus a per-launch capability; no TCP fallback.
#[cfg(unix)]
use crate::API;
use crate::{Context, Error, Reply, Request, MAX_FRAME};
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
enum Frame {
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
        use std::os::unix::{fs::MetadataExt, net::UnixStream};
        let meta = std::fs::symlink_metadata(&self.endpoint)?;
        use std::os::unix::fs::FileTypeExt;
        if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "endpoint must be an owner-only socket"));
        }
        let mut stream = UnixStream::connect(&self.endpoint)?;
        check_peer(&stream)?;
        stream.set_read_timeout(Some(Duration::from_secs(35)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
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
#[cfg(unix)]
fn check_peer(stream: &std::os::unix::net::UnixStream) -> io::Result<()> {
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
/// Kept for the lifetime of the app event loop. Dropping removes endpoint and revokes workers.
pub struct Listener {
    pub endpoint_file: PathBuf,
    stop: Arc<AtomicBool>,
    attached: Arc<std::sync::atomic::AtomicUsize>,
    #[cfg(unix)]
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Listener {
    #[cfg(unix)]
    pub fn start(queue: mpsc::SyncSender<Pending>, wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        use std::os::unix::{
            fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
            net::UnixListener,
        };
        let token = random_id()?;
        let epoch = random_id()?;
        let dir = std::env::temp_dir().join(format!("varos-bridge-{}-{}", std::process::id(), &epoch[..12]));
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        struct Cleanup(PathBuf, bool);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if !self.1 {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
        }
        let mut cleanup = Cleanup(dir.clone(), false);
        let socket = dir.join("bridge.sock");
        let listener = UnixListener::bind(&socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let endpoint_file = dir.join("endpoint.json");
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&endpoint_file)?;
        write_frame(&mut f, &Endpoint { api: API.into(), socket, token: token.clone(), epoch: epoch.clone() })?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let attached = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let connections = attached.clone();
        let allow_destructive = std::env::var("VAROS_BRIDGE_ALLOW_DESTRUCTIVE").as_deref() == Ok("1");
        let allow_history = std::env::var("VAROS_BRIDGE_ALLOW_HISTORY").as_deref() == Ok("1");
        let wake = Arc::new(wake);
        type CancelMap = std::collections::HashMap<(String, String), (Arc<AtomicBool>, std::time::Instant)>;
        let cancellations = Arc::new(std::sync::Mutex::new(CancelMap::new()));
        let thread = std::thread::spawn(move || {
            let mut workers = vec![];
            while !stopped.load(Ordering::Acquire) {
                workers
                    .retain(|(h, _): &(std::thread::JoinHandle<()>, std::os::unix::net::UnixStream)| !h.is_finished());
                cancellations.lock().unwrap().retain(|_, (_, time)| time.elapsed() < Duration::from_secs(60));
                match listener.accept() {
                    Ok((stream, _)) if workers.len() < 8 => {
                        let (token, epoch, queue, wake, stopped, cancellations) = (
                            token.clone(),
                            epoch.clone(),
                            queue.clone(),
                            wake.clone(),
                            stopped.clone(),
                            cancellations.clone(),
                        );
                        let Ok(shutdown) = stream.try_clone() else { continue };
                        let connections = connections.clone();
                        let worker = std::thread::spawn(move || {
                            // macOS/BSD: an accepted stream inherits the listener's O_NONBLOCK; make it
                            // blocking so the framed reads below wait instead of failing with WouldBlock.
                            let _ = stream.set_nonblocking(false);
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                            let run = (|| -> io::Result<()> {
                                check_peer(&stream)?;
                                let mut reader = std::io::BufReader::new(stream.try_clone()?);
                                let mut writer = stream;
                                let hello: Frame = serde_json::from_slice(
                                    &read_frame(&mut reader)?.ok_or_else(|| io::Error::other("missing hello"))?,
                                )?;
                                let Frame::Hello { api, token: provided, client } = hello else {
                                    return Err(io::Error::other("hello required"));
                                };
                                if authorize_token(&provided, &token).is_err()
                                    || client.len() != 64
                                    || !client.bytes().all(|b| b.is_ascii_hexdigit())
                                {
                                    return write_frame(
                                        &mut writer,
                                        &Reply::failure(Error::new(
                                            "scope_refused",
                                            "invalid capability, client or API",
                                        )),
                                    );
                                }
                                if api != API {
                                    return write_frame(
                                        &mut writer,
                                        &Reply::failure(Error::new("unsupported", "Bridge API must be 1.0")),
                                    );
                                }
                                connections.fetch_add(1, Ordering::AcqRel);
                                struct Attached(Arc<std::sync::atomic::AtomicUsize>);
                                impl Drop for Attached {
                                    fn drop(&mut self) {
                                        self.0.fetch_sub(1, Ordering::AcqRel);
                                    }
                                }
                                let _attached = Attached(connections);
                                write_frame(
                                    &mut writer,
                                    &Reply::success(serde_json::json!({"api":API,"epoch":epoch})),
                                )?;
                                let frame: Frame = serde_json::from_slice(
                                    &read_frame(&mut reader)?.ok_or_else(|| io::Error::other("missing call"))?,
                                )?;
                                match frame {
                                    Frame::Cancel { call_id } => {
                                        if call_id.len() > 128 {
                                            return Err(io::Error::new(
                                                io::ErrorKind::InvalidInput,
                                                "call id too long",
                                            ));
                                        }
                                        let mut map = cancellations.lock().unwrap();
                                        if map.len() < 64 {
                                            map.entry((client, call_id))
                                                .or_insert_with(|| {
                                                    (Arc::new(AtomicBool::new(false)), std::time::Instant::now())
                                                })
                                                .0
                                                .store(true, Ordering::Release);
                                        }
                                        drop(map);
                                        write_frame(
                                            &mut writer,
                                            &Reply::success(serde_json::json!({"cancellation":"requested"})),
                                        )
                                    }
                                    Frame::Call { call_id, request } => {
                                        if call_id.len() > 128 {
                                            return Err(io::Error::other("call id too long"));
                                        }
                                        let key = (client.clone(), call_id);
                                        let flag = {
                                            let mut map = cancellations.lock().unwrap();
                                            if map.len() >= 64 && !map.contains_key(&key) {
                                                return write_frame(
                                                    &mut writer,
                                                    &Reply::failure(Error::new("busy", "cancellation registry full")),
                                                );
                                            }
                                            map.entry(key.clone())
                                                .or_insert_with(|| {
                                                    (Arc::new(AtomicBool::new(false)), std::time::Instant::now())
                                                })
                                                .0
                                                .clone()
                                        };
                                        let (reply, rx) = mpsc::sync_channel(1);
                                        let pending = Pending {
                                            context: Context {
                                                client,
                                                epoch,
                                                read: true,
                                                edit: true,
                                                allow_history,
                                                allow_destructive,
                                            },
                                            request,
                                            cancelled: flag.clone(),
                                            reply,
                                        };
                                        let result = if queue.try_send(pending).is_err() {
                                            Reply::failure(Error::new("busy", "attachment queue is full"))
                                        } else {
                                            wake();
                                            let started = std::time::Instant::now();
                                            loop {
                                                match rx.recv_timeout(Duration::from_millis(100)) {
                                                    Ok(r) => break r,
                                                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                                                        break Reply::failure(Error::new("io_error", "host stopped"))
                                                    }
                                                    Err(mpsc::RecvTimeoutError::Timeout) => {
                                                        if stopped.load(Ordering::Acquire)
                                                            || started.elapsed() > Duration::from_secs(30)
                                                        {
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
                                        cancellations.lock().unwrap().remove(&key);
                                        write_frame(&mut writer, &result)
                                    }
                                    _ => Err(io::Error::other("unexpected hello")),
                                }
                            })();
                            if let Err(e) = run {
                                eprintln!("[varos-bridge] connection refused ({:?})", e.kind());
                            }
                        });
                        workers.push((worker, shutdown));
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                    Err(_) => break,
                }
            }
            for (flag, _) in cancellations.lock().unwrap().values() {
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
        });
        cleanup.1 = true;
        Ok(Self { endpoint_file, stop, attached, thread: Some(thread) })
    }
    #[cfg(not(unix))]
    pub fn start(_queue: mpsc::SyncSender<Pending>, _wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "native attachment unsupported"))
    }
    pub fn has_clients(&self) -> bool {
        self.attached.load(Ordering::Acquire) > 0
    }
    pub fn epoch(&self) -> io::Result<String> {
        Ok(serde_json::from_slice::<Endpoint>(&std::fs::read(&self.endpoint_file)?)?.epoch)
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        #[cfg(unix)]
        if let Some(h) = self.thread.take() {
            let deadline = std::time::Instant::now() + Duration::from_secs(1);
            while !h.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if h.is_finished() {
                let _ = h.join();
            }
        }
        if let Some(dir) = self.endpoint_file.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
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
