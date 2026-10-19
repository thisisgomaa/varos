//! The two secrets of Varos' agents port, and how each side proves it knows one.
//!
//! The endpoint file (`endpoint`, readable only by the user) holds two fresh 256-bit secrets per
//! launch:
//!
//! - the **token**, which the client (the MCP proxy) sends on every request line, so the app knows
//!   the line comes from a process of this user;
//! - the **server key**, which never leaves either process. Before the proxy sends anything else
//!   on a new connection it sends a `varos.hello` line with a fresh random nonce, and the app must
//!   answer with HMAC-SHA-256(server key, nonce). A process that took over the port of a Varos that
//!   crashed (another user's, say) can't read the endpoint file, so it can't answer: the proxy
//!   then drops the connection and the token and the document data never leave it.
//!
//! The token check on the app's side is cheap on purpose: the proxy puts the token last on the
//! line ([`with_token`]) and the app compares the line's tail ([`has_token`]) before it parses any
//! of the request.

use std::io::{BufRead, Read, Write};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The method of the handshake line.
pub const HELLO: &str = "varos.hello";
/// Hex digits of a secret or a nonce (32 bytes).
pub const SECRET_HEX: usize = 64;
/// Longest hello line or hello reply accepted (bytes).
pub const MAX_HELLO: usize = 512;
/// What the proof signs besides the nonce: a proof made for anything else can't be replayed here.
const CONTEXT: &[u8] = b"varos-agents-hello-v1\n";

/// `n` bytes from the operating system's random source, as hex.
#[cfg(not(target_arch = "wasm32"))]
pub fn random_hex(n: usize) -> std::io::Result<String> {
    let mut bytes = vec![0u8; n];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(format!("no random source: {e}")))?;
    Ok(hex(&bytes))
}

/// No agents port on the web.
#[cfg(target_arch = "wasm32")]
pub fn random_hex(_: usize) -> std::io::Result<String> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no agents port on the web"))
}

/// A fresh 256-bit secret (a token, a server key or a nonce).
pub fn new_secret() -> std::io::Result<String> {
    random_hex(SECRET_HEX / 2)
}

/// Is `s` shaped like a secret: exactly [`SECRET_HEX`] hex digits?
pub fn is_secret(s: &str) -> bool {
    s.len() == SECRET_HEX && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// HMAC-SHA-256 (RFC 2104).
fn hmac_sha256(key: &[u8], message: &[&[u8]]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let pad = |x: u8| k.map(|b| b ^ x);
    let mut inner = Sha256::new();
    inner.update(pad(0x36));
    for part in message {
        inner.update(part);
    }
    let mut outer = Sha256::new();
    outer.update(pad(0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// Equal, in time that doesn't depend on where they differ.
pub fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The server's proof for `nonce`: HMAC-SHA-256(server key, context ‖ nonce), as hex. `None` when
/// either isn't a secret.
pub fn proof(server_key: &str, nonce: &str) -> Option<String> {
    if !is_secret(nonce) {
        return None;
    }
    let key = unhex(server_key).filter(|k| k.len() == SECRET_HEX / 2)?;
    Some(hex(&hmac_sha256(&key, &[CONTEXT, nonce.as_bytes()])))
}

/// The hello line the client sends first on a connection.
pub fn hello_line(nonce: &str) -> String {
    json!({"id": 0, "method": HELLO, "params": {"nonce": nonce}}).to_string()
}

/// The app's answer to a line that may be a hello: `Some(reply)` when it is one (short, the hello
/// method, a well-formed nonce). Cheap: a hello is at most [`MAX_HELLO`] bytes.
pub fn answer_hello(line: &[u8], server_key: &str) -> Option<Value> {
    if line.len() > MAX_HELLO {
        return None;
    }
    let v: Value = serde_json::from_slice(line).ok()?;
    if v.get("method")?.as_str()? != HELLO {
        return None;
    }
    let proof = proof(server_key, v.get("params")?.get("nonce")?.as_str()?)?;
    Some(json!({"id": v.get("id").cloned().unwrap_or(Value::Null), "ok": true, "result": {"proof": proof}}))
}

/// Does the hello reply `line` prove the server knows `server_key` (for `nonce`)?
pub fn proves(line: &str, server_key: &str, nonce: &str) -> bool {
    let Some(want) = proof(server_key, nonce) else { return false };
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { return false };
    let got = v.get("result").and_then(|r| r.get("proof")).and_then(Value::as_str).unwrap_or("");
    v.get("ok") == Some(&Value::Bool(true)) && same(got.as_bytes(), want.as_bytes())
}

/// The client side of the handshake on a fresh connection: send a hello with a fresh nonce and
/// check the reply proves the server knows `server_key`. Nothing else may be sent before this
/// succeeds. The caller sets the read timeout.
pub fn handshake(reader: &mut impl BufRead, writer: &mut impl Write, server_key: &str) -> std::io::Result<()> {
    let nonce = new_secret()?;
    writer.write_all(hello_line(&nonce).as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    let mut reply = Vec::new();
    reader.by_ref().take(MAX_HELLO as u64 + 1).read_until(b'\n', &mut reply)?;
    let reply = String::from_utf8_lossy(&reply);
    if proves(&reply, server_key, &nonce) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the process on this port could not prove it is the Varos its endpoint file names; nothing was sent",
        ))
    }
}

/// `line` (one JSON object) with `"token": token` added as its last member, as [`has_token`]
/// expects it.
pub fn with_token(mut line: String, token: &str) -> String {
    debug_assert!(line.ends_with('}'));
    line.pop();
    if !line.ends_with('{') {
        line.push(',');
    }
    line.push_str("\"token\":\"");
    line.push_str(token);
    line.push_str("\"}");
    line
}

/// Does `line` carry `token` as its last member (see [`with_token`])? Looks at the line's tail
/// only, before anything parses it.
pub fn has_token(line: &[u8], token: &str) -> bool {
    let line = line.trim_ascii_end();
    let tail_len = "\"token\":\"".len() + token.len() + "\"}".len();
    let Some(tail) = line.len().checked_sub(tail_len).and_then(|start| line.get(start..)) else { return false };
    let before = line.len() - tail_len;
    // The member before it ends with `,`, or the token is the object's only member.
    let joined = before > 0 && matches!(line[before - 1], b',' | b'{');
    let Some(rest) = tail.strip_prefix(b"\"token\":\"").and_then(|r| r.strip_suffix(b"\"}")) else { return false };
    joined && same(rest, token.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc_4231() {
        // Test case 2.
        let mac = hmac_sha256(b"Jefe", &[b"what do ya want ", b"for nothing?"]);
        assert_eq!(hex(&mac), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        // Test case 6: a key longer than the block.
        let mac = hmac_sha256(&[0xaa; 131], &[b"Test Using Larger Than Block-Size Key - Hash Key First"]);
        assert_eq!(hex(&mac), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
    }

    #[test]
    fn secrets_are_fresh_and_well_formed() {
        let a = new_secret().unwrap();
        assert!(is_secret(&a));
        assert_ne!(a, new_secret().unwrap());
        assert!(!is_secret("ab"));
        assert!(!is_secret(&"z".repeat(64)));
    }

    #[test]
    fn only_the_key_holder_answers_the_hello() {
        let key = new_secret().unwrap();
        let other = new_secret().unwrap();
        let nonce = new_secret().unwrap();
        let reply = answer_hello(hello_line(&nonce).as_bytes(), &key).unwrap().to_string();
        assert!(proves(&reply, &key, &nonce));
        assert!(!proves(&reply, &other, &nonce), "another key's proof");
        assert!(!proves(&reply, &key, &new_secret().unwrap()), "a proof for another nonce");
        let forged = answer_hello(hello_line(&nonce).as_bytes(), &other).unwrap().to_string();
        assert!(!proves(&forged, &key, &nonce));
        assert!(!proves("{\"ok\":true}", &key, &nonce));
        // Not hellos.
        assert!(answer_hello(b"{\"method\":\"document.inspect\"}", &key).is_none());
        assert!(answer_hello(hello_line("short").as_bytes(), &key).is_none());
        assert!(answer_hello(&[b' '; MAX_HELLO + 1], &key).is_none());
    }

    #[test]
    fn the_handshake_runs_over_a_stream() {
        let key = new_secret().unwrap();
        // A server that answers the hello it is sent.
        let a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = a.local_addr().unwrap();
        let server_key = key.clone();
        let server = std::thread::spawn(move || {
            let (s, _) = a.accept().unwrap();
            let mut r = std::io::BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let mut s = s;
            writeln!(s, "{}", answer_hello(line.trim().as_bytes(), &server_key).unwrap()).unwrap();
        });
        let s = std::net::TcpStream::connect(addr).unwrap();
        let mut r = std::io::BufReader::new(s.try_clone().unwrap());
        let mut w = s;
        handshake(&mut r, &mut w, &key).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn the_token_is_checked_at_the_lines_end() {
        let token = new_secret().unwrap();
        let line = with_token(json!({"id": 1, "method": "x", "params": {"token": "decoy"}}).to_string(), &token);
        assert!(has_token(line.as_bytes(), &token));
        assert!(has_token(format!("{line}\r\n").as_bytes(), &token));
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["token"], json!(token));
        assert_eq!(v["method"], "x");
        assert!(has_token(with_token("{}".into(), &token).as_bytes(), &token));
        let wrong = new_secret().unwrap();
        assert!(!has_token(line.as_bytes(), &wrong));
        assert!(!has_token(b"{\"method\":\"x\"}", &token));
        // Only in the params: not the line's own member.
        let nested = format!("{{\"method\":\"x\",\"params\":{{\"token\":\"{token}\"}}}}");
        assert!(!has_token(nested.as_bytes(), &token));
        assert!(!has_token(b"", &token));
    }
}
