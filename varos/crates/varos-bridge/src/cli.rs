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
    let mut output_path = None;
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
            "--output" => {
                if output_path.is_some() {
                    return Err("duplicate --output".into());
                }
                output_path = Some(PathBuf::from(it.next().ok_or("missing --output path")?));
            }
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
        if file.is_some() || json || output_path.is_some() {
            return Err("mcp does not accept --json or --request-file".into());
        }
        crate::mcp::serve(&mut io::stdin().lock(), io::stdout(), client).map_err(|e| e.to_string())?;
        return Ok(0);
    }
    if !crate::TOOLS.contains(&verb.as_str()) {
        return Err(format!("unsupported tool {verb}"));
    }
    if (verb == "snapshot") != output_path.is_some() {
        return Err("snapshot requires --output <explicit-new-path>; --output is only for snapshot".into());
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
    let mut reply = if bytes.len() > crate::MAX_FRAME {
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
    if reply.ok {
        if let Some(path) = output_path {
            write_snapshot(&mut reply, &path)?;
        }
    }
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

fn write_snapshot(reply: &mut Reply, path: &std::path::Path) -> Result<(), String> {
    use base64::Engine;
    let result = reply.result.as_mut().ok_or("missing snapshot result")?;
    let data = result.get("png").and_then(serde_json::Value::as_str).ok_or("missing snapshot bytes")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    result.as_object_mut().ok_or("bad snapshot result")?.remove("png");
    result["path"] = serde_json::json!(path);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_writes_only_an_explicit_new_file_and_strips_bytes() {
        use base64::Engine;
        let png = varos_raster::rasterize(std::sync::Arc::new(varos_core::model::Document::default()), [32, 16])
            .encode_png()
            .unwrap();
        let mut reply = Reply::success(
            serde_json::json!({"png":base64::engine::general_purpose::STANDARD.encode(&png),"width":32,"height":16}),
        );
        let path = std::env::temp_dir().join(format!(
            "varos-snapshot-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let original = reply.clone();
        write_snapshot(&mut reply, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), png);
        assert!(reply.result.as_ref().unwrap().get("png").is_none());
        let mut repeat = original;
        assert!(write_snapshot(&mut repeat, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), png);
        std::fs::remove_file(path).unwrap();
    }
}
