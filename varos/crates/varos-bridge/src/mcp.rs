//! Newline JSON-RPC binding. Editing stays in Service.
use crate::{dto::*, ipc, service::compact, MCP_VERSION, TOOLS};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{self, BufRead, Write},
    sync::{Arc, Mutex},
};
pub fn tool_result(reply: &Reply) -> Value {
    let mut projection = reply.clone();
    let png = projection.result.as_mut().and_then(|r| r.as_object_mut()).and_then(|r| r.remove("png"));
    let mut content = vec![json!({"type":"text","text":compact(&projection)})];
    if let Some(png) = png {
        content.push(json!({"type":"image","mimeType":"image/png","data":png}));
    }
    json!({"content":content,"structuredContent":projection,"isError":!reply.ok})
}
pub fn decode_tool(name: &str, args: Value) -> Result<Request, Error> {
    if name == "edit" {
        let edit: Edit =
            serde_json::from_value(args.clone()).map_err(|e| Error::new("invalid_argument", e.to_string()))?;
        crate::economy::expand(&edit)?;
    }
    serde_json::from_value(json!({"tool":name,"arguments":args}))
        .map_err(|e| Error::new("invalid_argument", e.to_string()))
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub fn tools() -> Value {
    let api = json!({"type":"string","const":"1.0","default":"1.0"});
    let ids = json!({"type":"array","items":{"type":"string","pattern":"^(path|node):[0-9]+$"},"maxItems":1000});
    let rev = json!({"type":"integer","minimum":0});
    let board = json!({"type":"string","pattern":"^b[0-9]+$"});
    let request_id = json!({"type":"string","pattern":"^r[1-9][0-9]*$"});
    let page = json!({"type":"integer","minimum":1,"maximum":100,"default":20});
    let cursor = json!({"type":"string"});
    let paint = json!({"anyOf":[{"type":"string","pattern":"^#[0-9A-Fa-f]{8}$"},{"type":"null"}]});
    let mut schemas = HashMap::new();
    schemas.insert("capabilities", object(json!({"api":api}), &[]));
    schemas.insert("list_boards", object(json!({"api":api,"limit":page,"cursor":cursor}), &[]));
    schemas.insert("describe",object(json!({"api":api,"board":board,"rev":rev,"ids":ids,"fields":{"type":"array","items":{"enum":["bounds","paint","parent","name","state","metadata","artboards","geometry","selection"]}},"since":rev,"limit":page,"cursor":cursor}),&["board"]));
    schemas.insert(
        "select",
        object(
            json!({"api":api,"board":board,"request_id":request_id,"expected_rev":rev,"ids":ids}),
            &["api", "board", "request_id", "expected_rev", "ids"],
        ),
    );
    let move_schema = object(
        json!({"verb":{"const":"move"},"ids":ids,"delta":{"type":"array","items":{"type":"number"},"minItems":2,"maxItems":2}}),
        &["verb", "ids", "delta"],
    );
    let paint_schema = object(
        json!({"verb":{"const":"set_paint"},"ids":ids,"fill":paint,"stroke":paint,"stroke_width":{"type":"number","minimum":0},"opacity":{"type":"number","minimum":0,"maximum":1}}),
        &["verb", "ids"],
    );
    let edit_ids = json!({"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string","pattern":"^((path|node):[1-9][0-9]*|\\$[A-Za-z][A-Za-z0-9_]{0,62})$"}});
    let bounds = json!({"type":"array","minItems":4,"maxItems":4,"items":{"type":"number"},"description":"[x,y,width,height], positive dimensions"});
    let local = json!({"type":"string","pattern":"^\\$[A-Za-z][A-Za-z0-9_]{0,62}$"});
    let name = json!({"type":"string","minLength":1,"maxLength":256});
    let mut move_schema = move_schema;
    move_schema["properties"]["ids"] = edit_ids.clone();
    let mut paint_schema = paint_schema;
    paint_schema["properties"]["ids"] = edit_ids.clone();
    let mut operation_schemas = vec![
        move_schema,
        paint_schema,
        object(
            json!({"verb":{"const":"add_shape"},"kind":{"enum":["rect","ellipse"]},"bounds":bounds,"radius":{"type":"number","minimum":0,"description":"rect only; at most half the shorter side; creates cubic path geometry"},"parent":{"type":"string","pattern":"^node:[1-9][0-9]*$"},"insert":{"enum":["top"]},"local":local,"name":name,"fill":paint,"stroke":paint,"stroke_width":{"type":"number","minimum":0},"opacity":{"type":"number","minimum":0,"maximum":1}}),
            &["verb", "kind", "bounds"],
        ),
        object(json!({"verb":{"const":"resize"},"ids":edit_ids,"bounds":bounds}), &["verb", "ids", "bounds"]),
        object(
            json!({"verb":{"const":"rotate"},"ids":edit_ids,"degrees":{"type":"number"}}),
            &["verb", "ids", "degrees"],
        ),
        object(json!({"verb":{"const":"rename"},"ids":edit_ids,"name":name}), &["verb", "ids", "name"]),
        object(
            json!({"verb":{"const":"align"},"ids":edit_ids,"mode":{"enum":["left","center","right","top","middle","bottom"]},"target":{"type":"string","pattern":"^(selection|artboard:[1-9][0-9]*|\\$[A-Za-z][A-Za-z0-9_]{0,62}|a[0-9]+@[0-9]+)$","description":"selection, artboard:N, a request-local bound to an artboard, or the deprecated revision-bound aN@rev"}}),
            &["verb", "ids", "mode", "target"],
        ),
        object(
            json!({"verb":{"const":"distribute"},"ids":edit_ids,"axis":{"enum":["h","v"]}}),
            &["verb", "ids", "axis"],
        ),
        object(json!({"verb":{"const":"group"},"ids":edit_ids,"local":local}), &["verb", "ids"]),
        object(
            json!({"verb":{"const":"order"},"ids":edit_ids,"order":{"enum":["front","forward","backward","back"]}}),
            &["verb", "ids", "order"],
        ),
    ];
    let point = json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}});
    operation_schemas.push(object(json!({"verb":{"const":"add_path"},"anchors":{"type":"array","minItems":2,"maxItems":1000,"items":object(json!({"p":point,"hin":{"anyOf":[point,{"type":"null"}]},"hout":{"anyOf":[point,{"type":"null"}]},"smooth":{"type":"boolean","default":false}}),&["p"])},"closed":{"type":"boolean"},"parent":{"type":"string","pattern":"^node:[1-9][0-9]*$"},"local":local,"name":name,"fill":paint,"stroke":paint,"stroke_width":{"type":"number","minimum":0},"opacity":{"type":"number","minimum":0,"maximum":1}}),&["verb","anchors","closed"]));
    for verb in ["delete", "ungroup"] {
        operation_schemas.push(object(json!({"verb":{"const":verb},"ids":edit_ids}), &["verb", "ids"]));
    }
    // slice 3: page verbs, addressed by the persistent artboard:N (or a request-local bound to one)
    let artboard = json!({"type":"string","pattern":"^(artboard:[1-9][0-9]*|\\$[A-Za-z][A-Za-z0-9_]{0,62})$"});
    let page_bounds = json!({"type":"array","minItems":4,"maxItems":4,"items":{"type":"number"},"description":"[x,y,width,height] in points; width and height at least 1"});
    let add_artboard = json!({"verb":{"const":"add_artboard"},"bounds":page_bounds,"preset":{"enum":["square","portrait","story","a4"],"description":"square 1080x1080, portrait 1080x1350, story 1080x1920 (px = pt at 72 ppi), a4 595x842 pt"},"origin":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"},"description":"top-left for a preset page; default: right of the right-most page"},"name":name,"local":local});
    operation_schemas.push(json!({"type":"object","properties":add_artboard,"required":["verb"],"additionalProperties":false,"oneOf":[{"required":["bounds"],"not":{"anyOf":[{"required":["preset"]},{"required":["origin"]}]}},{"required":["preset"],"not":{"required":["bounds"]}}]}));
    operation_schemas.push(object(
        json!({"verb":{"const":"resize_artboard"},"id":artboard,"bounds":page_bounds}),
        &["verb", "id", "bounds"],
    ));
    operation_schemas
        .push(object(json!({"verb":{"const":"rename_artboard"},"id":artboard,"name":name}), &["verb", "id", "name"]));
    for verb in ["delete_artboard", "set_active_artboard"] {
        operation_schemas.push(object(json!({"verb":{"const":verb},"id":artboard}), &["verb", "id"]));
    }
    operation_schemas.push(object(
        json!({"verb":{"const":"reorder_artboard"},"id":artboard,"position":{"type":"integer","minimum":0}}),
        &["verb", "id", "position"],
    ));
    operation_schemas.push(object(json!({"verb":{"const":"duplicate_artboard"},"id":artboard,"with_art":{"type":"boolean"},"offset":{"anyOf":[point,{"type":"null"}]},"local":local}),&["verb","id","with_art"]));
    operation_schemas.push(object(
        json!({"verb":{"const":"set_artboard_color"},"id":artboard,"color":paint}),
        &["verb", "id", "color"],
    ));
    operation_schemas.push(object(
        json!({"verb":{"const":"set_artboard_clip"},"id":artboard,"clip":{"type":"boolean"}}),
        &["verb", "id", "clip"],
    ));
    schemas.insert(
        "save",
        object(
            json!({"api":api,"board":board,"request_id":request_id,"expected_rev":rev}),
            &["api", "board", "request_id", "expected_rev"],
        ),
    );
    schemas.insert("save_as",object(json!({"api":api,"board":board,"request_id":request_id,"expected_rev":rev,"path":{"type":"string","minLength":1}}),&["api","board","request_id","expected_rev","path"]));
    schemas.insert("export_pdf",object(json!({"api":api,"board":board,"request_id":request_id,"expected_rev":rev,"path":{"type":"string","minLength":1},"scope":{"type":"string","pattern":"^(all_visible_artboards|artwork_bounds|artboard:[1-9][0-9]*)$"}}),&["api","board","request_id","expected_rev","path","scope"]));
    schemas.insert("snapshot",object(json!({"api":api,"board":board,"rev":rev,"artboard":{"type":"string","pattern":"^artboard:[1-9][0-9]*$","description":"render only this page, at its aspect ratio inside width x height (default 1024 x 1024)"},"width":{"type":"integer","minimum":1,"maximum":1024,"description":"default 544 (board preview) or 1024 (page)"},"height":{"type":"integer","minimum":1,"maximum":1024,"description":"default 246 (board preview) or 1024 (page)"}}),&["board","rev"]));
    schemas.insert("edit",object(json!({"api":api,"board":board,"request_id":request_id,"expected_rev":rev,"digest":{"type":"string","pattern":"^[0-9a-f]{64}$"},"ops":{"type":"array","minItems":1,"maxItems":100,"items":{"oneOf":operation_schemas}}}),&["api","board","request_id","expected_rev","ops"]));
    schemas.insert("history",object(json!({"api":api,"board":board,"request_id":request_id,"expected_rev":rev,"action":{"enum":["undo","redo"]},"digest":{"type":"string","pattern":"^[0-9a-f]{64}$"}}),&["api","board","request_id","expected_rev","action"]));
    schemas.insert("request_status", object(json!({"api":api,"request_id":request_id}), &["request_id"]));
    // Declare each operation once; version-specific legality stays in the decoder.
    let mut definitions = serde_json::Map::new();
    let mut all_ops = Vec::new();
    let mut creations = Vec::new();
    for schema in &operation_schemas {
        let verb = schema["properties"]["verb"]["const"].as_str().unwrap();
        definitions.insert(verb.into(), schema.clone());
        let reference = json!({"$ref":format!("#/$defs/{verb}")});
        all_ops.push(reference.clone());
        if matches!(verb, "add_shape" | "add_path") {
            creations.push(reference);
        }
    }
    for kind in ["rect", "ellipse", "path"] {
        let source = definitions[if kind == "path" { "add_path" } else { "add_shape" }].clone();
        let mut options = source["properties"].clone();
        for key in ["verb", "kind", "bounds", "anchors", "closed"] {
            options.as_object_mut().unwrap().remove(key);
        }
        if kind != "rect" {
            options.as_object_mut().unwrap().remove("radius");
        }
        let opts = format!("{kind}_options");
        definitions.insert(opts.clone(), object(options, &[]));
        let opt = json!({"$ref":format!("#/$defs/{opts}")});
        let mut prefix = if kind == "path" {
            vec![
                json!({"const":kind}),
                json!({"type":"array","minItems":2,"maxItems":1000,"items":point}),
                json!({"type":"boolean"}),
            ]
        } else {
            vec![json!({"const":kind}), bounds.clone()]
        };
        let minimum = prefix.len();
        prefix.push(json!({"anyOf":[paint,opt]}));
        if kind == "rect" {
            prefix.push(json!({"anyOf":[{"type":"number","minimum":0},opt]}));
        }
        prefix.push(opt);
        let tuple = format!("{kind}_tuple");
        definitions.insert(tuple.clone(), json!({"type":"array","prefixItems":prefix,"minItems":minimum,"maxItems":prefix.len(),"description":"Optional positional fill, rect radius, then options. Decoder rejects duplicate options and misplaced slots."}));
        let reference = json!({"$ref":format!("#/$defs/{tuple}")});
        all_ops.push(reference.clone());
        creations.push(reference);
    }
    definitions.insert("creation".into(), json!({"anyOf":creations}));
    for depth in (0..4).rev() {
        let mut alternatives = vec![json!({"$ref":"#/$defs/creation"})];
        if depth < 3 {
            alternatives.push(json!({"$ref":format!("#/$defs/repeat{}",depth+1)}));
        }
        definitions.insert(format!("repeat{depth}"), object(json!({"verb":{"const":"repeat"},"ops":{"type":"array","minItems":1,"maxItems":100,"items":{"anyOf":alternatives}},"count":{"type":"integer","minimum":1,"maximum":100},"dx":{"type":"number"},"dy":{"type":"number"}}), &["verb","ops","count","dx","dy"]));
    }
    all_ops.push(json!({"$ref":"#/$defs/repeat0"}));
    definitions.insert("operation".into(), json!({"anyOf":all_ops}));
    let edit = schemas.get_mut("edit").unwrap();
    edit["properties"]["api"] = json!({"enum":["1.0","1.1"]});
    edit["properties"]["ops"]["items"] = json!({"$ref":"#/$defs/operation"});
    edit["properties"]["defaults"] = object(
        json!({"parent":{"type":"string","pattern":"^node:[1-9][0-9]*$"},"fill":paint,"stroke":paint,"stroke_width":{"type":"number","minimum":0},"radius":{"type":"number","minimum":0},"opacity":{"type":"number","minimum":0,"maximum":1}}),
        &[],
    );
    edit["properties"]["receipt"] = json!({"const":"ids"});
    edit["$defs"] = json!(definitions);
    for (tool, field, schema) in [
        ("request_status", "cursor", json!({"type":"string"})),
        ("describe", "summary_budget", json!({"type":"integer","minimum":256,"maximum":1024})),
        ("snapshot", "profile", json!({"const":"economy"})),
    ] {
        let root = schemas.get_mut(tool).unwrap();
        root["properties"]["api"] = json!({"enum":["1.0","1.1"],"default":"1.0"});
        root["properties"][field] = schema;
    }
    for tool in ["capabilities", "list_boards", "select", "history", "save", "save_as", "export_pdf"] {
        schemas.get_mut(tool).unwrap()["properties"]["api"] = json!({"enum":["1.0","1.1"],"default":"1.0"});
    }
    schemas.get_mut("export_pdf").unwrap()["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
    let tools:Vec<_>=TOOLS.iter().map(|name|json!({"name":name,"description":match *name {
        "capabilities"=>"Negotiate Bridge API 1.0/1.1; export_pdf additionally supports 1.2 reports; local user trust grants every scope. Inspect limits and file mistake-guards.",
        "list_boards"=>"List authorized open boards, never files or Recent entries.",
        "describe"=>"Summary first. fields compose board/object detail; ids scope objects; limit/cursor page objects; since adds net changes or resync_required.",
        "select"=>"Deliberately replace human selection with explicit targets; no document undo step.",
        "edit"=>"Atomic design batch with explicit targets; one human undo step. Retains human selection. API 1.1 supports creation tuples; use object operations if your client does not support prefixItems. Page verbs use persistent artboard:N ids.",
        "snapshot"=>"Explicit revision-pinned CPU PNG preview of the board, or of one artboard:N page. Returns an MCP image; max 1024 pixels per dimension.",
        "save"|"save_as"|"export_pdf"=>"Queue revision-pinned file work. Returns accepted and ticket; poll request_status. Allowed: fresh .vrs/.pdf names under passwd home, /Volumes/<volume>/, ~/Library/Mobile Documents (iCloud Drive), or ~/Library/CloudStorage/<provider>/ (Dropbox/Google Drive/OneDrive). Refused: /tmp, /private/var, other ~/Library, system roots, running app bundle, dot components and existing files. Network volumes unsupported. Parents must exist and canonical containment is rechecked. FAT32/exFAT use macOS exclusive-rename fallback after linkat; real volumes unverified.",
        "history"=>"One shared undo/redo entry. Local agents need no approval; revision and idempotency checks still apply.",
        _=>"Get a retained receipt by monotonic request_id for this proxy client.",
    },"inputSchema":schemas[*name]})).collect();
    json!({"tools":tools})
}
/// API 1.2 discovery is opt-in, leaving the historic tool-list bytes untouched.
pub fn tools_for_api(api: Option<&str>) -> Value {
    let mut result = tools();
    if api == Some("1.2") {
        if let Some(list) = result["tools"].as_array_mut() {
            if let Some(capabilities) = list.iter_mut().find(|v| v["name"] == "capabilities") {
                capabilities["inputSchema"]["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
            }
            if let Some(export) = list.iter_mut().find(|v| v["name"] == "export_pdf") {
                export["inputSchema"]["properties"]["options"] =
                    json!({"type":"object","description":"PDF preset, image_ppi, compress_streams, boxes and marks"});
            }
        }
        if let Some(list) = result["tools"].as_array_mut() {
            for name in crate::TOOLS_12 {
                list.push(json!({"name":name,"description":"API 1.2 desktop host effect. Print opens a prepared PDF in Preview; Copy/Cut publish the current selection to the OS clipboard.","inputSchema": {"type":"object","additionalProperties":false,"properties":{"api":{"const":"1.2"},"board":{"type":"string"},"request_id":{"type":"string"},"expected_rev":{"type":"integer"},"scope":{"enum":["active_artboard","all_visible_artboards","artwork_bounds"]},"options":{"type":"object"}},"required":["api","board","request_id","expected_rev"]}}));
            }
        }
    }
    result
}
fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
/// Keep reading cancellations while a tool waits for the UI owning thread.
pub trait Transport: Send + Sync + 'static {
    fn call(&self, call_id: &str, request: Request) -> Reply;
    fn cancel(&self, call_id: &str);
    /// The MCP client's self-declared name: a display label / profile-mapping hint, never identity.
    fn client_info(&self, _name: &str) {}
}
impl Transport for crate::conn::attach::AutoClient {
    fn call(&self, call_id: &str, request: Request) -> Reply {
        crate::conn::attach::AutoClient::call(self, call_id, request)
    }
    fn cancel(&self, call_id: &str) {
        crate::conn::attach::AutoClient::cancel(self, call_id)
    }
    fn client_info(&self, name: &str) {
        self.set_label(name)
    }
}
impl Transport for ipc::Client {
    fn call(&self, call_id: &str, request: Request) -> Reply {
        ipc::Client::call(self, call_id, request)
    }
    fn cancel(&self, call_id: &str) {
        ipc::Client::cancel(self, call_id)
    }
}
pub fn serve<T: Transport>(
    reader: &mut impl BufRead,
    writer: impl Write + Send + 'static,
    client: T,
) -> io::Result<()> {
    let writer = Arc::new(Mutex::new(writer));
    let client = Arc::new(client);
    type Active = HashMap<String, (String, Arc<std::sync::atomic::AtomicBool>)>;
    let active = Arc::new(Mutex::new(Active::new()));
    let (tx, rx) = std::sync::mpsc::sync_channel::<(Value, String, Request, Arc<std::sync::atomic::AtomicBool>)>(6);
    let (worker_writer, worker_client, worker_active) = (writer.clone(), client.clone(), active.clone());
    let worker = std::thread::spawn(move || {
        while let Ok((id, call, req, flag)) = rx.recv() {
            let reply = if flag.load(std::sync::atomic::Ordering::Acquire) {
                Reply::failure(Error::new("cancelled", "cancelled before attachment dispatch"))
            } else {
                worker_client.call(&call, req)
            };
            if !flag.load(std::sync::atomic::Ordering::Acquire) {
                let _ =
                    ipc::write_frame(&mut *worker_writer.lock().unwrap(), &rpc_result(id.clone(), tool_result(&reply)));
            }
            worker_active.lock().unwrap().remove(&id.to_string());
        }
    });
    let mut initialized = false;
    let mut ready = false;
    let mut serial = 0u64;
    while let Some(bytes) = ipc::read_frame(reader)? {
        let msg: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => {
                ipc::write_frame(&mut *writer.lock().unwrap(), &rpc_error(Value::Null, -32700, "parse error"))?;
                continue;
            }
        };
        let id = msg.get("id").cloned();
        let valid_id = id.as_ref().is_none_or(|v| v.is_string() || v.is_number());
        if msg["jsonrpc"] != "2.0" || !msg["method"].is_string() || !valid_id || !msg.is_object() {
            ipc::write_frame(
                &mut *writer.lock().unwrap(),
                &rpc_error(Value::Null, -32600, "invalid JSON-RPC request"),
            )?;
            continue;
        }
        let method = msg["method"].as_str().unwrap();
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        if method == "notifications/cancelled" {
            if let Some(request) = params.get("requestId") {
                if let Some((call, flag)) = active.lock().unwrap().get(&request.to_string()).cloned() {
                    if !flag.swap(true, std::sync::atomic::Ordering::AcqRel) {
                        let cancel_client = client.clone();
                        std::thread::spawn(move || cancel_client.cancel(&call));
                    }
                }
            }
            continue;
        }
        if method == "notifications/initialized" && initialized {
            ready = true;
            continue;
        }
        let Some(id) = id else {
            continue;
        };
        let result = match method {
            "initialize" if !initialized => {
                if params["protocolVersion"].as_str().is_none()
                    || !params["clientInfo"].is_object()
                    || !params["capabilities"].is_object()
                {
                    rpc_error(id, -32602, "invalid initialize parameters")
                } else {
                    initialized = true;
                    if let Some(name) = params["clientInfo"]["name"].as_str() {
                        client.client_info(name);
                    }
                    rpc_result(
                        id,
                        json!({"protocolVersion":MCP_VERSION,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"varos-bridge","version":env!("CARGO_PKG_VERSION")},"instructions":"Call capabilities api 1.0 first. Summary then ids/fields. Explicit board/revision/targets; consume either text or structured content. Connection errors are typed: host_not_running (ask the owner to open Varos), ambiguous_target (ask the owner which Varos), session_reset (list_boards again)."}),
                    )
                }
            }
            "ping" => rpc_result(id, json!({})),
            _ if !ready => rpc_error(id, -32002, "initialize and notifications/initialized required"),
            "tools/list" => rpc_result(id, tools_for_api(params["api"].as_str())),
            "tools/call"
                if params["name"]
                    .as_str()
                    .is_none_or(|name| !TOOLS.contains(&name) && !crate::TOOLS_12.contains(&name)) =>
            {
                rpc_error(id, -32602, "unknown or missing tool name")
            }
            "tools/call" => {
                let decoded = params["name"]
                    .as_str()
                    .ok_or_else(|| Error::new("invalid_argument", "tool name required"))
                    .and_then(|name| decode_tool(name, params.get("arguments").cloned().unwrap_or_else(|| json!({}))));
                match decoded {
                    Err(e) => rpc_result(id, tool_result(&Reply::failure(e))),
                    Ok(req) => {
                        if active.lock().unwrap().len() >= 6 {
                            rpc_result(
                                id,
                                tool_result(&Reply::failure(Error::new("busy", "too many pending MCP calls"))),
                            )
                        } else {
                            serial += 1;
                            let call = format!("mcp-{serial}");
                            let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
                            let inserted = match active.lock().unwrap().entry(id.to_string()) {
                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    entry.insert((call.clone(), flag.clone()));
                                    true
                                }
                                std::collections::hash_map::Entry::Occupied(_) => false,
                            };
                            if !inserted {
                                ipc::write_frame(
                                    &mut *writer.lock().unwrap(),
                                    &rpc_error(id, -32600, "duplicate pending JSON-RPC id"),
                                )?;
                            } else if tx.try_send((id.clone(), call, req, flag)).is_err() {
                                active.lock().unwrap().remove(&id.to_string());
                                ipc::write_frame(
                                    &mut *writer.lock().unwrap(),
                                    &rpc_result(
                                        id,
                                        tool_result(&Reply::failure(Error::new("busy", "MCP queue is full"))),
                                    ),
                                )?;
                            }
                            continue;
                        }
                    }
                }
            }
            _ => rpc_error(id, -32601, "method not found"),
        };
        ipc::write_frame(&mut *writer.lock().unwrap(), &result)?;
    }
    let calls: Vec<_> = active.lock().unwrap().values().cloned().collect();
    for (call, flag) in calls {
        flag.store(true, std::sync::atomic::Ordering::Release);
        let cancel_client = client.clone();
        std::thread::spawn(move || cancel_client.cancel(&call));
    }
    drop(tx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while !worker.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    if worker.is_finished() {
        let _ = worker.join();
    }
    Ok(())
}
