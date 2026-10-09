//! Thin attached CLI binding; no editing implementation here.
use crate::{ipc, service::compact, Error, Reply, Request};
use std::{
    io::{self, Read, Write},
    path::PathBuf,
};
/// Either the ADR-0011 `--attach auto` transport or the deprecated slice-1 token attachment.
enum Attachment {
    Auto(Box<crate::conn::attach::AutoClient>),
    Legacy(ipc::Client),
}
impl Attachment {
    fn call(&self, call_id: &str, request: Request) -> Reply {
        match self {
            Self::Auto(c) => c.call(call_id, request),
            Self::Legacy(c) => c.call(call_id, request),
        }
    }
}
pub fn run(args: Vec<String>) -> Result<i32, String> {
    use crate::conn::attach::{AutoClient, Selector};
    let mut it = args.into_iter();
    let verb = it.next().ok_or("expected mcp or a Bridge tool")?;
    let mut client_id = None;
    let mut endpoint = None;
    let mut token = None;
    let mut json = false;
    let mut file = None;
    let mut output_path = None;
    let mut selector = None;
    let mut identity = None;
    let mut fields = None;
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--attach" => {
                if endpoint.is_some() {
                    return Err("duplicate --attach".into());
                }
                endpoint = Some(it.next().ok_or("missing --attach value")?);
            }
            "--token" => {
                if token.is_some() {
                    return Err("duplicate --token".into());
                }
                token = Some(it.next().ok_or("missing --token value")?);
            }
            "--instance" | "--pid" => {
                if selector.is_some() {
                    return Err("use at most one of --instance / --pid".into());
                }
                let value = it.next().ok_or(format!("missing {flag} value"))?;
                selector = Some(if flag == "--pid" {
                    Selector::Pid(value.parse().map_err(|_| "--pid must be a number")?)
                } else if crate::conn::is_hex(&value, 16) {
                    Selector::Instance(value)
                } else {
                    return Err("--instance must be a 16-hex instance id (see varos-cli bridge hosts)".into());
                });
            }
            "--identity" => {
                if identity.is_some() {
                    return Err("duplicate --identity".into());
                }
                identity = Some(it.next().ok_or("missing --identity value")?);
            }
            "--client" => client_id = Some(it.next().ok_or("missing --client value")?),
            "--output" => {
                if output_path.is_some() {
                    return Err("duplicate --output".into());
                }
                output_path = Some(PathBuf::from(it.next().ok_or("missing --output path")?));
            }
            "--fields" => {
                if verb != "describe" || fields.is_some() {
                    return Err("--fields is only for describe and may be given once".into());
                }
                fields = Some(it.next().ok_or("missing --fields value")?);
            }
            "--json" => json = true,
            "--request-file" => file = Some(PathBuf::from(it.next().ok_or("missing request file")?)),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    let legacy = endpoint.as_deref().is_some_and(|e| e != "auto");
    let client = if legacy {
        // Deprecated slice-1 path (ADR-0011 §5 transition): explicit socket + per-launch token.
        if selector.is_some() || identity.is_some() {
            return Err("--instance/--pid/--identity apply only to --attach auto".into());
        }
        let mut client = ipc::Client::new(
            PathBuf::from(endpoint.unwrap()),
            token.ok_or("--token is required with an explicit --attach socket (deprecated slice-1 mode)")?,
        )
        .map_err(|e| e.to_string())?;
        if let Some(id) = client_id {
            client = client.with_client(id).map_err(|e| e.to_string())?;
        }
        Attachment::Legacy(client)
    } else {
        if token.is_some() {
            return Err(
                "--token belongs only to the deprecated explicit-socket mode; --attach auto uses file identity keys"
                    .into(),
            );
        }
        let label = if verb == "mcp" { "mcp-client" } else { "varos-bridge-cli" };
        let mut auto = AutoClient::new(selector.unwrap_or(Selector::Auto), identity, label)
            .map_err(|e| format!("{}: {}", e.code, e.reason))?;
        if let Some(id) = client_id {
            auto = auto.with_session(id.to_ascii_lowercase()).map_err(|e| e.reason.clone())?;
        }
        Attachment::Auto(Box::new(auto))
    };
    if verb == "mcp" {
        if file.is_some() || json || output_path.is_some() {
            return Err("mcp does not accept --json or --request-file".into());
        }
        match client {
            Attachment::Auto(c) => crate::mcp::serve(&mut io::stdin().lock(), io::stdout(), *c),
            Attachment::Legacy(c) => crate::mcp::serve(&mut io::stdin().lock(), io::stdout(), c),
        }
        .map_err(|e| e.to_string())?;
        return Ok(0);
    }
    if !crate::TOOLS.contains(&verb.as_str())
        && verb != "import_file"
        && verb != "import_clipboard"
        && ![
            "help",
            "import_svg",
            "preferences",
            "history_list",
            "history_jump",
            "actions",
            "shortcuts",
            "command_index",
        ]
        .contains(&verb.as_str())
        && !crate::TOOLS_12.contains(&verb.as_str())
        && !["export_svg", "export_raster", "save_template", "new_from_template", "window_memory"]
            .contains(&verb.as_str())
    {
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
        let mut args = if bytes.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
        };
        if let Some(fields) = fields {
            apply_fields(&mut args, &fields)?;
        }
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
fn apply_fields(args: &mut serde_json::Value, fields: &str) -> Result<(), String> {
    let args = args.as_object_mut().ok_or("describe arguments must be an object")?;
    if args.contains_key("fields") {
        return Err("fields supplied both in request and --fields".into());
    }
    let fields: Vec<String> = if fields.trim_start().starts_with('[') {
        serde_json::from_str(fields).map_err(|e| format!("invalid --fields: {e}"))?
    } else {
        fields.split(',').map(|f| f.trim().to_owned()).collect()
    };
    args.insert("fields".into(), serde_json::json!(fields));
    Ok(())
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
    fn describe_fields_flag_matches_json_arguments() {
        for fields in ["artboards, selection,bounds", r#"["artboards","selection","bounds"]"#] {
            let mut args = serde_json::json!({"board":"b1"});
            apply_fields(&mut args, fields).unwrap();
            assert_eq!(args["fields"], serde_json::json!(["artboards", "selection", "bounds"]));
            assert!(crate::mcp::decode_tool("describe", args.clone()).is_ok());
            assert!(apply_fields(&mut args, fields).is_err());
        }
    }
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
