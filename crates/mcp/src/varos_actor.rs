//! Varos: the agent behind this MCP server, as the desktop app shows it (docs/varos/BACKLOG.md
//! item 8).
//!
//! Every control line this server sends to the app carries `"actor": {session, profile, label}`:
//! the session is one per server process (one per agent conversation), the profile and label come
//! from the client's `initialize` `clientInfo` ("claude-code" → "Claude Code"). The app uses it to
//! name the agent on the canvas and in the History panel. The model sends nothing for this, so it
//! costs no tokens. It is attribution only, never access control.

use std::sync::OnceLock;

use serde_json::{Value, json};

/// The actor for a client that introduced itself with `client_info` (`initialize` params'
/// `clientInfo`, `{name, title?, version}`).
pub(crate) fn actor(client_info: &Value) -> Value {
    let name = client_info.get("name").and_then(Value::as_str).unwrap_or("");
    let title = client_info.get("title").and_then(Value::as_str).filter(|t| !t.trim().is_empty());
    let label = title.map_or_else(|| label_for(name), str::to_string);
    json!({"session": session(), "profile": name, "label": label})
}

/// One id per server process: a reconnect to the app keeps it.
fn session() -> &'static str {
    static SESSION: OnceLock<String> = OnceLock::new();
    SESSION.get_or_init(|| {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        format!("mcp-{}-{nanos:x}", std::process::id())
    })
}

/// A readable label for a client name: the agents people use by their names, anything else with
/// its words capitalised ("my-agent" → "My Agent").
fn label_for(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let known = [("claude", "Claude Code"), ("codex", "Codex"), ("cursor", "Cursor"), ("gemini", "Gemini"), ("grok", "Grok")];
    if let Some((_, label)) = known.iter().find(|(k, _)| lower.starts_with(k)) {
        return (*label).to_string();
    }
    name.split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_clients_get_their_names() {
        assert_eq!(actor(&json!({"name": "claude-code", "version": "2.1"}))["label"], "Claude Code");
        assert_eq!(actor(&json!({"name": "codex-mcp-client"}))["label"], "Codex");
        assert_eq!(actor(&json!({"name": "my-agent_tool"}))["label"], "My Agent Tool");
        assert_eq!(actor(&json!({"name": "x", "title": "Design Bot"}))["label"], "Design Bot");
        assert_eq!(actor(&json!({}))["label"], "");
    }

    #[test]
    fn every_line_to_the_app_names_the_agent() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        // A stand-in for the desktop app: read one request line, answer it, hand the line back.
        let app = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut out = stream;
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let v: Value = serde_json::from_str(&line).unwrap();
            writeln!(out, "{}", json!({"id": v["id"], "ok": true, "result": {}})).unwrap();
            v
        });
        let mut s = crate::Server::new(Box::new(crate::Remote::connect(&addr).unwrap()));
        let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "claude-code", "version": "2"}}});
        s.handle_line(&init.to_string());
        s.handle_line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string());
        s.handle_line(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "doc_inspect", "arguments": {}}}).to_string());
        let sent = app.join().unwrap();
        assert_eq!(sent["actor"]["label"], "Claude Code", "{sent}");
        assert_eq!(sent["actor"]["profile"], "claude-code");
        assert!(sent["actor"]["session"].as_str().unwrap().starts_with("mcp-"));
    }

    #[test]
    fn the_session_is_one_per_process() {
        let a = actor(&json!({"name": "claude-code"}));
        let b = actor(&json!({"name": "codex"}));
        assert_eq!(a["session"], b["session"]);
        assert_eq!(a["profile"], "claude-code");
    }
}
