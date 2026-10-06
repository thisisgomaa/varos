//! Thin attached CLI binding; no editing implementation here.
use crate::{ipc, service::compact, Error, Reply, Request};
use std::{
    io::{self, Read, Write},
    path::PathBuf,
};
pub fn run(args: Vec<String>) -> Result<i32, String> {
    let mut it = args.into_iter();
    let verb = it.next().ok_or("expected mcp or a Bridge tool")?;
    let mut client_id = None;
    let mut endpoint = None;
    let mut token = None;
    let mut json = false;
    let mut file = None;
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--attach" => {
                if endpoint.is_some() {
                    return Err("duplicate --attach".into());
                }
                endpoint = Some(PathBuf::from(it.next().ok_or("missing --attach value")?));
            }
            "--token" => {
                if token.is_some() {
                    return Err("duplicate --token".into());
                }
                token = Some(it.next().ok_or("missing --token value")?);
            }
            "--client" => client_id = Some(it.next().ok_or("missing --client value")?),
            "--json" => json = true,
            "--request-file" => file = Some(PathBuf::from(it.next().ok_or("missing request file")?)),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    let mut client = ipc::Client::new(
        endpoint.ok_or("--attach socket is required (no automatic discovery)")?,
        token.ok_or("--token is required (default deny)")?,
    )
    .map_err(|e| e.to_string())?;
    if let Some(id) = client_id {
        client = client.with_client(id).map_err(|e| e.to_string())?;
    }
    if verb == "mcp" {
        if file.is_some() || json {
            return Err("mcp does not accept --json or --request-file".into());
        }
        crate::mcp::serve(&mut io::stdin().lock(), io::stdout(), client).map_err(|e| e.to_string())?;
        return Ok(0);
    }
    if !crate::TOOLS.contains(&verb.as_str()) {
        return Err(format!("unsupported tool {verb}"));
    }
    let mut bytes = vec![];
    match file {
        Some(path) => std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(crate::MAX_FRAME as u64 + 1)
            .read_to_end(&mut bytes),
        None => io::stdin().take(crate::MAX_FRAME as u64 + 1).read_to_end(&mut bytes),
    }
    .map_err(|e| e.to_string())?;
    let reply = if bytes.len() > crate::MAX_FRAME {
        Reply::failure(Error::new("limit_exceeded", "request exceeds 1 MiB"))
    } else {
        let args = if bytes.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
        };
        match crate::mcp::decode_tool(&verb, args) {
            Ok(request) => client.call("cli-1", request),
            Err(error) => Reply::failure(error),
        }
    };
    let output = if json { serde_json::to_string(&reply).map_err(|e| e.to_string())? } else { compact(&reply) };
    writeln!(io::stdout().lock(), "{output}").map_err(|e| e.to_string())?;
    Ok(if reply.ok { 0 } else { 1 })
}
pub fn decode(request: &[u8]) -> Result<Request, Error> {
    let v: serde_json::Value =
        serde_json::from_slice(request).map_err(|e| Error::new("invalid_argument", e.to_string()))?;
    if !v.is_object() || v.as_object().unwrap().keys().any(|k| k != "tool" && k != "arguments") {
        return Err(Error::new("invalid_argument", "expected only tool and arguments"));
    }
    let tool = v["tool"].as_str().ok_or_else(|| Error::new("invalid_argument", "tool must be a string"))?;
    crate::mcp::decode_tool(
        tool,
        v.get("arguments").cloned().ok_or_else(|| Error::new("invalid_argument", "arguments required"))?,
    )
}
