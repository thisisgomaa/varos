//! ADR-0011 C1 contracts without sockets, GPU or event loops: registry, handshake, pairing,
//! trust, audit, `--attach auto` state machine and MCP startup without a host.
#![cfg(unix)]
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};
use std::{os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc};
use varos_bridge::conn::{
    attach::{self, AutoClient, Selection, Selector},
    credentials,
    handshake::{AgentSide, Expect, Frame, HostSide},
    manage,
    registry::{self, Entry, Record, VersionRange},
    trust::Scopes,
    Paths,
};

/// A short owner-only temp root (socket paths must stay under the sun_path limit).
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("vb{}", varos_bridge::conn::random_hex(3).unwrap()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(dir)
    }
    fn paths(&self) -> Paths {
        Paths::under(&self.0)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn key() -> SigningKey {
    let b: [u8; 32] =
        varos_bridge::conn::unhex(&varos_bridge::conn::random_hex(32).unwrap()).unwrap().try_into().unwrap();
    SigningKey::from_bytes(&b)
}
fn record(paths: &Paths, instance: &str, host: &SigningKey) -> Record {
    let pid = std::process::id();
    Record {
        discovery_version: 1,
        instance_id: instance.into(),
        epoch: varos_bridge::conn::random_hex(32).unwrap(),
        pid,
        start: registry::process_start(pid).unwrap(),
        mode: "desktop".into(),
        socket: paths.hosts().join(instance).join(registry::SOCKET_FILE),
        connection: VersionRange { min: "1.0".into(), max: "1.0".into() },
        api: "1.0".into(),
        app_build: "varos-app test".into(),
        host_fingerprint: credentials::fingerprint(&host.verifying_key()),
    }
}
fn mode(path: &std::path::Path) -> u32 {
    std::fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn registry_round_trip_permissions_stale_cleanup_and_forgery() {
    let t = Temp::new();
    let paths = t.paths();
    let host = key();
    let socket = Entry::reserve(&paths, "00112233aabbccdd").unwrap();
    assert!(socket.as_os_str().len() <= registry::MAX_SOCKET_PATH);
    let rec = record(&paths, "00112233aabbccdd", &host);
    let entry = Entry::publish(&paths, rec.clone()).unwrap();
    assert_eq!(mode(&paths.runtime), 0o700);
    assert_eq!(mode(&paths.hosts()), 0o700);
    assert_eq!(mode(&entry.dir), 0o700);
    let file = entry.dir.join(registry::RECORD_FILE);
    assert_eq!(mode(&file), 0o600);
    // No credential, board name, path or token in the record.
    let text = std::fs::read_to_string(&file).unwrap();
    for forbidden in ["token", "board", ".vrs", "secret"] {
        assert!(!text.contains(forbidden), "{text}");
    }
    let scan = registry::scan(&paths);
    assert_eq!(scan.live, vec![rec.clone()]);

    // Stale: same pid but a different start identity (pid reuse) is not live.
    let stale_dir = paths.hosts().join("ffffffffffffffff");
    Entry::reserve(&paths, "ffffffffffffffff").unwrap();
    let mut stale = record(&paths, "ffffffffffffffff", &host);
    stale.start = "1.000000".into();
    Entry::publish(&paths, stale).unwrap();
    // Forged: socket outside its own entry directory.
    let forged_dir = paths.hosts().join("1111111111111111");
    Entry::reserve(&paths, "1111111111111111").unwrap();
    let mut forged = record(&paths, "1111111111111111", &host);
    forged.socket = PathBuf::from("/tmp/elsewhere.sock");
    let bytes = serde_json::to_vec(&forged).unwrap();
    varos_bridge::conn::fsutil::write_atomic(&forged_dir.join(registry::RECORD_FILE), &bytes).unwrap();
    // A symlinked entry directory is ignored.
    std::os::unix::fs::symlink(&entry.dir, paths.hosts().join("2222222222222222")).unwrap();
    let scan = registry::scan(&paths);
    assert_eq!(scan.live, vec![rec.clone()], "{:?}", scan.diagnostics);
    assert_eq!(scan.stale, vec![stale_dir.clone()]);
    assert!(scan.diagnostics.iter().any(|d| d.contains("own entry directory")));
    assert!(scan.diagnostics.iter().any(|d| d.contains("2222222222222222")));
    assert_eq!(registry::cleanup_stale(&scan, 16), 1);
    assert!(!stale_dir.exists());
    assert!(entry.dir.exists(), "cleanup never touches a live entry or follows links");

    // Owner-only enforcement: a group-readable record is ignored.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(registry::scan(&paths).live.is_empty());
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    // Identity-checked removal.
    entry.remove();
    assert!(!entry.dir.exists());
    assert!(registry::scan(&paths).live.is_empty());
}

#[test]
fn socket_path_limit_is_validated() {
    let long = PathBuf::from(format!("/{}", "x".repeat(registry::MAX_SOCKET_PATH)));
    assert_eq!(registry::check_socket_path(&long).unwrap_err().code, "unsupported");
}

#[test]
fn deterministic_selection_never_guesses() {
    let t = Temp::new();
    let paths = t.paths();
    let (h1, h2) = (key(), key());
    let a = record(&paths, "aaaaaaaaaaaaaaaa", &h1);
    let b = record(&paths, "bbbbbbbbbbbbbbbb", &h1);
    let c = record(&paths, "cccccccccccccccc", &h2);
    assert_eq!(attach::select(&[], &Selector::Auto), Selection::NotRunning);
    // A sole candidate can attach.
    assert_eq!(attach::select(std::slice::from_ref(&a), &Selector::Auto), Selection::Attach(a.clone()));
    // Several candidates → owner must choose.
    assert!(
        matches!(attach::select(&[a.clone(), c.clone()], &Selector::Auto), Selection::Ambiguous(v) if v.len() == 2)
    );
    // Reversing candidates never selects a launch implicitly.
    assert!(
        matches!(attach::select(&[c.clone(), a.clone()], &Selector::Auto), Selection::Ambiguous(v) if v.len() == 2)
    );
    // Two launches of the same host remain ambiguous.
    let sel = attach::select(&[a.clone(), b.clone()], &Selector::Auto);
    assert!(matches!(&sel, Selection::Ambiguous(v) if v.len() == 2));
    // Explicit selectors narrow.
    let by_id = attach::select(&[a.clone(), b.clone()], &Selector::Instance(b.instance_id.clone()));
    assert_eq!(by_id, Selection::Attach(b.clone()));
    assert_eq!(attach::select(std::slice::from_ref(&a), &Selector::Pid(1)), Selection::NotRunning);
    assert_eq!(attach::select(std::slice::from_ref(&a), &Selector::Pid(a.pid)), Selection::Attach(a.clone()));
    // Candidate list is bounded and carries no epoch, socket or names.
    let many: Vec<Record> = (0..20).map(|i| record(&paths, &format!("{i:016x}"), &h1)).collect();
    let Selection::Ambiguous(v) = attach::select(&many, &Selector::Auto) else { panic!() };
    let e = attach::ambiguous(&v);
    assert_eq!(e.code, "ambiguous_target");
    assert_eq!(e.candidates.len(), attach::MAX_CANDIDATES);
    let text = serde_json::to_string(&e).unwrap();
    assert!(!text.contains("epoch") && !text.contains("sock"), "{text}");
}

struct Pair {
    host: SigningKey,
    agent: SigningKey,
    expect: Expect,
}
fn pair() -> Pair {
    let host = key();
    Pair {
        expect: Expect {
            instance_id: "0123456789abcdef".into(),
            epoch: "e".repeat(64),
            host_fingerprint: credentials::fingerprint(&host.verifying_key()),
        },
        host,
        agent: key(),
    }
}
const PROFILE: &str = "abcdefabcdef0123";
fn session() -> String {
    "5".repeat(64)
}

#[test]
fn handshake_happy_path_binds_identity_epoch_and_versions() {
    let p = pair();
    let (agent, hello) = AgentSide::hello().unwrap();
    let (mut host, challenge) = HostSide::challenge(&p.host, &p.expect.instance_id, &p.expect.epoch, &hello).unwrap();
    let proof = agent.respond(&challenge, &p.expect, &p.agent, PROFILE, &session(), "Claude Code").unwrap();
    let verified = host.verify(&proof).unwrap();
    assert_eq!(verified.profile_id, PROFILE);
    assert_eq!(verified.fingerprint, credentials::fingerprint(&p.agent.verifying_key()));
    assert_eq!(verified.label, "Claude Code");
    // Frames carry public material only.
    for f in [&hello, &challenge, &proof] {
        let text = serde_json::to_string(f).unwrap();
        assert!(!text.contains("token"));
        assert!(!text.contains(&varos_bridge::conn::hex(p.agent.as_bytes())));
        assert!(!text.contains(&varos_bridge::conn::hex(p.host.as_bytes())));
    }
}

#[test]
fn handshake_rejects_wrong_keys_replay_epoch_and_downgrade() {
    let p = pair();
    // Wrong host key: a different key than the registry fingerprint.
    let (agent, hello) = AgentSide::hello().unwrap();
    let impostor = key();
    let (_, challenge) = HostSide::challenge(&impostor, &p.expect.instance_id, &p.expect.epoch, &hello).unwrap();
    let e = agent.respond(&challenge, &p.expect, &p.agent, PROFILE, &session(), "x").unwrap_err();
    assert_eq!(e.code, "host_identity_mismatch");
    // Epoch mismatch: the host relaunched after the record was read.
    let (_, relaunched) = HostSide::challenge(&p.host, &p.expect.instance_id, &"f".repeat(64), &hello).unwrap();
    assert_eq!(
        agent.respond(&relaunched, &p.expect, &p.agent, PROFILE, &session(), "x").unwrap_err().code,
        "session_reset"
    );
    // Tampered epoch inside a genuine challenge breaks the signature.
    let (_, mut tampered) = HostSide::challenge(&p.host, &p.expect.instance_id, &p.expect.epoch, &hello).unwrap();
    if let Frame::HostChallenge { epoch, .. } = &mut tampered {
        *epoch = "f".repeat(64);
    }
    assert_eq!(
        agent.respond(&tampered, &p.expect, &p.agent, PROFILE, &session(), "x").unwrap_err().code,
        "host_identity_mismatch"
    );
    // A replayed (old) challenge does not match this agent's fresh nonce.
    let (other_agent, _) = AgentSide::hello().unwrap();
    let (_, old) = HostSide::challenge(&p.host, &p.expect.instance_id, &p.expect.epoch, &hello).unwrap();
    assert_eq!(
        other_agent.respond(&old, &p.expect, &p.agent, PROFILE, &session(), "x").unwrap_err().code,
        "host_identity_mismatch"
    );

    // Wrong agent key: proof claims key A but is signed by key B.
    let (agent, hello) = AgentSide::hello().unwrap();
    let (mut host, challenge) = HostSide::challenge(&p.host, &p.expect.instance_id, &p.expect.epoch, &hello).unwrap();
    let mut proof = agent.respond(&challenge, &p.expect, &key(), PROFILE, &session(), "x").unwrap();
    if let Frame::ClientProof { agent_key, .. } = &mut proof {
        *agent_key = credentials::public_hex(&p.agent.verifying_key());
    }
    assert_eq!(host.verify(&proof).unwrap_err().code, "identity_invalid");
    // Replayed nonce: a valid proof cannot be used on another connection or twice.
    let (agent, hello) = AgentSide::hello().unwrap();
    let (mut host, challenge) = HostSide::challenge(&p.host, &p.expect.instance_id, &p.expect.epoch, &hello).unwrap();
    let proof = agent.respond(&challenge, &p.expect, &p.agent, PROFILE, &session(), "x").unwrap();
    let (mut other_host, _) = HostSide::challenge(&p.host, &p.expect.instance_id, &p.expect.epoch, &hello).unwrap();
    assert_eq!(other_host.verify(&proof).unwrap_err().code, "identity_invalid");
    assert!(host.verify(&proof).is_ok());
    assert_eq!(host.verify(&proof).unwrap_err().code, "identity_invalid");
    // Downgrade / unknown versions fail closed; legacy token Hello is refused.
    let bad = Frame::ClientHello { connection: vec!["0.9".into()], api: "1.0".into(), nonce: "a".repeat(64) };
    assert_eq!(
        HostSide::challenge(&p.host, "0123456789abcdef", &p.expect.epoch, &bad).unwrap_err().code,
        "unsupported"
    );
    assert!(serde_json::from_value::<Frame>(json!({"kind":"hello","api":"1.0","token":"t","client":"c"})).is_err());
    assert!(serde_json::from_value::<Frame>(
        json!({"kind":"client_hello","connection":["1.0"],"api":"1.0","nonce":"a","scopes":["history"]})
    )
    .is_err());
}

#[test]
fn concurrent_first_use_creates_one_profile_without_trust_state() {
    let t = Temp::new();
    let paths = t.paths();
    let store: Arc<dyn credentials::CredentialStore> = Arc::new(credentials::MemoryStore::new());
    // Two MCP processes of the same client starting together (threads share the flock semantics).
    let ids: Vec<String> = (0..4)
        .map(|_| {
            let (paths, store) = (paths.clone(), store.clone());
            std::thread::spawn(move || attach::resolve_profile(&paths, store.as_ref(), "claude-code", None).unwrap().0)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    assert!(ids.iter().all(|id| id == &ids[0]), "{ids:?}");
    // A second, newer profile for the same label exists (e.g. created elsewhere); the approved
    // one wins, the newest unapproved one does not.
    let newer = "fedcba9876543210".to_string();
    let newer_key = credentials::load_or_create(store.as_ref(), &credentials::agent_account(&newer)).unwrap();
    let mut profiles = attach::LocalProfiles::load(&paths).unwrap();
    profiles.profiles.push(attach::LocalProfile {
        profile_id: newer.clone(),
        label: "claude-code".into(),
        public_key: credentials::public_hex(&newer_key.verifying_key()),
        created: u64::MAX,
    });
    varos_bridge::conn::fsutil::write_atomic(
        &paths.state.join(attach::PROFILES_FILE),
        &serde_json::to_vec(&profiles).unwrap(),
    )
    .unwrap();
    assert_eq!(
        attach::resolve_profile(&paths, store.as_ref(), "claude-code", None).unwrap().0,
        newer,
        "no approval: newest"
    );
}

#[test]
fn deferred_host_key_never_blocks_start_and_failure_is_reported_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use varos_bridge::ipc::{Listener, Options, PairedSource, PairedStatus};
    let (tx, _rx) = std::sync::mpsc::sync_channel(1);
    let woke = Arc::new(AtomicUsize::new(0));
    let w = woke.clone();
    let started = std::time::Instant::now();
    let load = || -> Result<varos_bridge::ipc::PairedConfig, varos_bridge::Error> {
        std::thread::sleep(std::time::Duration::from_millis(300));
        Err(credentials::unavailable("key file permission denied"))
    };
    let listener = Listener::start_with(
        tx,
        move || {
            w.fetch_add(1, Ordering::SeqCst);
        },
        Options { legacy: false, paired: Some(PairedSource::Deferred(Box::new(load))), legacy_audit: None },
    )
    .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_millis(200), "start must not wait for the key");
    assert_eq!(listener.paired_status(), PairedStatus::Starting);
    assert!(listener.take_unavailable_notice().is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while listener.paired_status() == PairedStatus::Starting && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        matches!(listener.paired_status(), PairedStatus::Unavailable(r) if r.starts_with("credential_unavailable"))
    );
    assert!(woke.load(Ordering::SeqCst) >= 1, "the host is woken to show its notice");
    assert!(listener.take_unavailable_notice().unwrap().contains("denied"));
    assert!(listener.take_unavailable_notice().is_none(), "told once per launch");
    assert!(listener.registry_file().is_none());
    // Dropping while a slow load is still running returns promptly.
    let (tx, _rx) = std::sync::mpsc::sync_channel(1);
    let slow = || -> Result<varos_bridge::ipc::PairedConfig, varos_bridge::Error> {
        std::thread::sleep(std::time::Duration::from_secs(3));
        Err(credentials::unavailable("late"))
    };
    let listener = Listener::start_with(
        tx,
        || {},
        Options { legacy: false, paired: Some(PairedSource::Deferred(Box::new(slow))), legacy_audit: None },
    )
    .unwrap();
    let dropping = std::time::Instant::now();
    drop(listener);
    assert!(dropping.elapsed() < std::time::Duration::from_secs(2));
}

#[cfg(target_os = "macos")]
#[test]
fn private_reads_never_block_on_a_planted_fifo() {
    let t = Temp::new();
    let fifo = t.0.join("endpoint.json");
    let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: valid NUL-terminated path.
    assert_eq!(unsafe { libc_mkfifo(c.as_ptr(), 0o600) }, 0);
    let started = std::time::Instant::now();
    assert!(varos_bridge::conn::fsutil::read_private(&fifo).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}
#[cfg(target_os = "macos")]
extern "C" {
    #[link_name = "mkfifo"]
    fn libc_mkfifo(path: *const std::os::raw::c_char, mode: u16) -> i32;
}

#[test]
fn unavailable_credential_store_is_typed_and_does_not_publish_profile() {
    let t = Temp::new();
    let paths = t.paths();
    let store = credentials::MemoryStore::new();
    store.set_locked(true);
    let e = attach::resolve_profile(&paths, &store, "claude-code", None).unwrap_err();
    assert_eq!(e.code, "credential_unavailable");
    assert!(!paths.state.join(attach::PROFILES_FILE).exists());
}

#[test]
fn profiles_are_remembered_per_client_label() {
    let t = Temp::new();
    let paths = t.paths();
    let store = credentials::FileKeyStore::new(paths.state.join("keys"));
    let (a, ka) = attach::resolve_profile(&paths, &store, "claude-code", None).unwrap();
    let (a2, _) = attach::resolve_profile(&paths, &store, "claude-code", None).unwrap();
    assert_eq!(a, a2, "a fresh session of the same client reuses its profile");
    let (b, _) = attach::resolve_profile(&paths, &store, "codex-mcp-client", None).unwrap();
    assert_ne!(a, b, "a different client gets its own profile");
    let profiles = std::fs::read_to_string(paths.state.join(attach::PROFILES_FILE)).unwrap();
    assert!(!profiles.contains(&varos_bridge::conn::hex(ka.as_bytes())), "no secret in profile references");
    // --identity is a public reference; a missing key is refused.
    assert_eq!(attach::resolve_profile(&paths, &store, "x", Some(&a)).unwrap().0, a);
    assert_eq!(
        attach::resolve_profile(&paths, &store, "x", Some("0000000000000000")).unwrap_err().code,
        "identity_invalid"
    );
}

#[test]
fn auto_attach_state_machine_with_fake_registry() {
    let t = Temp::new();
    let paths = t.paths();
    let host = key();
    let a = record(&paths, "aaaaaaaaaaaaaaaa", &host);
    let b = record(&paths, "bbbbbbbbbbbbbbbb", &host);
    let live = Arc::new(std::sync::Mutex::new(Vec::<Record>::new()));
    let feed = live.clone();
    let store: Arc<dyn credentials::CredentialStore> = Arc::new(credentials::MemoryStore::new());
    let client = AutoClient::with(Ok(paths.clone()), store.clone(), Selector::Auto, None, "claude-code")
        .unwrap()
        .with_scanner(move |_| registry::Scan { live: feed.lock().unwrap().clone(), ..Default::default() });
    let caps = || varos_bridge::mcp::decode_tool("capabilities", json!({"api":"1.0"})).unwrap();
    let code = |r: varos_bridge::Reply| r.error.unwrap().code;
    // Absent host: typed, no credential created.
    assert_eq!(code(client.call("c1", caps())), "host_not_running");
    assert!(!paths.state.join(attach::PROFILES_FILE).exists());
    // Two launches: ambiguous, bounded candidates.
    *live.lock().unwrap() = vec![a.clone(), b.clone()];
    let reply = client.call("c2", caps());
    let error = reply.error.unwrap();
    assert_eq!(error.code, "ambiguous_target");
    assert_eq!(error.candidates.len(), 2);
    assert!(client.pinned().is_none());
    // One launch: attach (its socket is gone here, so the transport reports not running) and pin.
    *live.lock().unwrap() = vec![a.clone()];
    assert_eq!(code(client.call("c3", caps())), "host_not_running");
    assert_eq!(client.pinned(), Some((a.instance_id.clone(), a.epoch.clone())));
    // A second launch appears: the live pin is kept, never silently switched.
    *live.lock().unwrap() = vec![b.clone(), a.clone()];
    client.call("c4", caps());
    assert_eq!(client.pinned(), Some((a.instance_id.clone(), a.epoch.clone())));
    // The pinned launch restarts (new epoch): session_reset once, then re-pinned.
    let mut relaunched = a.clone();
    relaunched.epoch = "9".repeat(64);
    *live.lock().unwrap() = vec![relaunched.clone()];
    assert_eq!(code(client.call("c5", caps())), "session_reset");
    assert_eq!(client.pinned(), Some((relaunched.instance_id.clone(), relaunched.epoch.clone())));
    assert_eq!(code(client.call("c6", caps())), "host_not_running");
    // Explicit selector that matches nothing.
    let pinned_pid = AutoClient::with(Ok(paths.clone()), store, Selector::Pid(1), None, "x")
        .unwrap()
        .with_scanner(move |_| registry::Scan { live: vec![a.clone()], ..Default::default() });
    let e = pinned_pid.call("c7", caps()).error.unwrap();
    assert_eq!(e.code, "host_not_running");
    assert!(e.reason.contains("--pid 1"));
}

#[test]
fn real_binary_mcp_initializes_and_reports_host_not_running_without_varos() {
    use std::io::{BufRead, Write};
    let t = Temp::new();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_varos-bridge"))
        .arg("mcp")
        .env(varos_bridge::conn::HOME_OVERRIDE, &t.0)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut send = move |v: Value| {
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
    };
    let started = std::time::Instant::now();
    send(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"claude-code","version":"1"}}}),
    );
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let init: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "initialize must not wait for a host");
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    line.clear();
    stdout.read_line(&mut line).unwrap();
    let tools: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), varos_bridge::TOOLS.len());
    send(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"capabilities","arguments":{"api":"1.0"}}}),
    );
    line.clear();
    stdout.read_line(&mut line).unwrap();
    let call: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(call["result"]["isError"], true);
    assert_eq!(call["result"]["structuredContent"]["error"]["code"], "host_not_running");
    drop(send);
    assert!(child.wait().unwrap().success());
    // Nothing secret or persistent was created for an absent host.
    assert!(!t.0.join("state").exists());
}

#[test]
fn old_public_profile_without_file_key_requires_new_identity() {
    let t = Temp::new();
    let paths = t.paths();
    // Simulates the old backend without ever accessing a real Keychain.
    let old_store = credentials::MemoryStore::new();
    let (old_id, _) = attach::resolve_profile(&paths, &old_store, "claude-code", None).unwrap();
    let files = credentials::FileKeyStore::new(paths.state.join("keys"));
    let e = attach::resolve_profile(&paths, &files, "claude-code", Some(&old_id)).unwrap_err();
    assert_eq!(e.code, "identity_invalid");
    assert!(e.reason.contains("retry"));
    let (new_id, key) = attach::resolve_profile(&paths, &files, "claude-code", None).unwrap();
    assert_ne!(new_id, old_id);
    assert!(!paths.state.join("trust.json").exists());
    assert_eq!(
        credentials::load_existing(&files, &credentials::agent_account(&new_id)).unwrap().unwrap().verifying_key(),
        key.verifying_key()
    );
}

#[test]
fn pairing_and_revocation_cli_are_noops_without_accessing_state() {
    for args in [
        vec!["pair"],
        vec!["pair", "--approve", "anything"],
        vec!["pair", "--deny"],
        vec!["agents", "revoke", "anything"],
    ] {
        assert_eq!(manage::run(args.into_iter().map(str::to_owned).collect()).unwrap(), 0);
    }
    assert_eq!(Scopes::ALL.names(), "read,edit,destructive,history,files");
    assert!(varos_bridge::ipc::authorize_uid(501, 501).is_ok());
    assert_eq!(varos_bridge::ipc::authorize_uid(502, 501).unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
}
