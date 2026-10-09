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
/// Explicit MCP tools/list API 1.2 opt-in; the default 1.0/1.1 schema remains byte-identical.
fn construction_tools(api: &str) -> Value {
    let mut list = tools();
    if api != "1.2" {
        return list;
    }
    if let Some(entries) = list["tools"].as_array_mut() {
        for tool in entries {
            let name = tool["name"].as_str().unwrap_or("").to_owned();
            if name == "edit" {
                let edit = &mut tool["inputSchema"];
                let edit_ids = json!({"$ref":"#/$defs/move/properties/ids"});
                let point = json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}});
                edit["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
                edit["$defs"]["construction_points"] =
                    json!({"type":"array","minItems":1,"maxItems":1000,"items":point});
                let gesture = json!({"$ref":"#/$defs/construction_points"});
                let extra = [
                    (
                        "pathfinder",
                        object(
                            json!({"verb":{"const":"pathfinder"},"ids":edit_ids,"operation":{"enum":["unite","minus_front","intersect","exclude","divide","trim","merge","crop","outline","minus_back"]}}),
                            &["verb", "ids", "operation"],
                        ),
                    ),
                    (
                        "shape_builder",
                        object(
                            json!({"verb":{"const":"shape_builder"},"ids":edit_ids,"points":gesture,"delete":{"type":"boolean"}}),
                            &["verb", "ids", "points", "delete"],
                        ),
                    ),
                    (
                        "scissors",
                        object(
                            json!({"verb":{"const":"scissors"},"ids":edit_ids,"segment":{"type":"integer","minimum":0},"t":{"type":"number","minimum":0,"maximum":1}}),
                            &["verb", "ids", "segment", "t"],
                        ),
                    ),
                    (
                        "knife",
                        object(
                            json!({"verb":{"const":"knife"},"ids":edit_ids,"points":gesture}),
                            &["verb", "ids", "points"],
                        ),
                    ),
                    (
                        "eraser",
                        object(
                            json!({"verb":{"const":"eraser"},"ids":edit_ids,"points":gesture,"radius":{"type":"number","exclusiveMinimum":0}}),
                            &["verb", "ids", "points", "radius"],
                        ),
                    ),
                    (
                        "divide_objects_below",
                        object(json!({"verb":{"const":"divide_objects_below"},"ids":edit_ids}), &["verb", "ids"]),
                    ),
                ];
                for (verb, schema) in extra {
                    edit["$defs"][verb] = schema;
                    if let Some(ops) = edit["$defs"]["operation"]["anyOf"].as_array_mut() {
                        ops.push(json!({"$ref":format!("#/$defs/{verb}")}));
                    }
                }
            }
            if name == "capabilities" {
                tool["inputSchema"]["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
            }
        }
    }

    list
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
    crate::select_transform::schemas(&mut definitions, &mut all_ops);
    definitions.insert("operation".into(), json!({"anyOf":all_ops}));
    let edit = schemas.get_mut("edit").unwrap();
    edit["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"]});
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
    for tool in ["capabilities", "select"] {
        if let Some(schema) = schemas.get_mut(tool) {
            schema["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
        }
    }
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
            "tools/list" => rpc_result(id, tools_for_api(params.get("api").and_then(Value::as_str).unwrap_or("1.0"))),
            "tools/call" if params["name"].as_str().is_none_or(|name| !TOOLS.contains(&name)) => {
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

/// Explicit 1.2 schema projection; legacy tools() remains byte-frozen.
pub fn tools_for_api(api: &str) -> Value {
    tools_for(api)
}

pub fn tools_for(api: &str) -> Value {
    let mut out = construction_tools(api);
    if api != "1.2" {
        return out;
    }
    let style = stroke_style_schema();
    if let Some(tools) = out["tools"].as_array_mut() {
        for tool in tools {
            let name = tool["name"].as_str().unwrap_or("").to_owned();
            tool["inputSchema"]["properties"]["api"] = json!({"const":"1.2"});
            if name == "describe" {
                if let Some(fields) = tool["inputSchema"]["properties"]["fields"]["items"]["enum"].as_array_mut() {
                    fields.push(json!("stroke_style"));
                }
            }
            if name == "edit" {
                let schema = &mut tool["inputSchema"];
                schema["$defs"]["stroke_style"] = style.clone();
                schema["$defs"]["op_key"] = json!({"oneOf":[{"required":["verb"],"not":{"required":["op"]}},{"required":["op"],"not":{"required":["verb"]}}]});
                let shared_ids = schema["$defs"]["move"]["properties"]["ids"].clone();
                schema["$defs"]["ids"] = shared_ids.clone();
                schema["$defs"]["set_paint"]["properties"]["stroke_style"] = json!({"$ref":"#/$defs/stroke_style"});
                schema["$defs"]["set_stroke_style"] = object(
                    json!({"verb":{"const":"set_stroke_style"},"ids":{"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string","pattern":"^path:[1-9][0-9]*$"}},"stroke_style":{"$ref":"#/$defs/stroke_style"}}),
                    &["verb", "ids", "stroke_style"],
                );
                if let Some(ops) = schema["$defs"]["operation"]["anyOf"].as_array_mut() {
                    ops.push(json!({"$ref":"#/$defs/set_stroke_style"}));
                }
                // API 1.2 accepts the contract's `op` spelling, or the retained `verb`, but never both.
                if let Some(defs) = schema["$defs"].as_object_mut() {
                    for definition in defs.values_mut() {
                        if let Some(value) = definition["properties"]["verb"].get("const").cloned() {
                            definition["properties"]["op"] = json!({"const":value});
                            if let Some(required) = definition["required"].as_array_mut() {
                                required.retain(|v| v != "verb");
                            }
                            let mut constraints = vec![json!({"$ref":"#/$defs/op_key"})];
                            if let Some(original) = definition.as_object_mut().and_then(|d| d.remove("oneOf")) {
                                constraints.push(json!({"oneOf":original}));
                            }
                            definition["allOf"] = json!(constraints);
                            if definition["properties"]["ids"] == shared_ids {
                                definition["properties"]["ids"] = json!({"$ref":"#/$defs/ids"});
                            }
                        }
                    }
                }
            }
        }
    }
    // Keep the combined opt-in projection within the existing tools/list size budget.
    fn trim_descriptions(value: &mut Value) {
        match value {
            Value::Object(m) => {
                m.remove("description");
                for v in m.values_mut() {
                    trim_descriptions(v);
                }
            }
            Value::Array(a) => {
                for v in a {
                    trim_descriptions(v);
                }
            }
            _ => {}
        }
    }
    if let Some(tools) = out["tools"].as_array_mut() {
        for tool in tools {
            trim_descriptions(&mut tool["inputSchema"]);
        }
    }
    // Reuse identical property constraints without changing validation or accepted spellings.
    fn share(value: &mut Value, original: &Value, reference: &Value) {
        if value == original {
            *value = reference.clone();
        } else {
            match value {
                Value::Object(m) => {
                    for v in m.values_mut() {
                        share(v, original, reference);
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        share(v, original, reference);
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(tools) = out["tools"].as_array_mut() {
        if let Some(edit) = tools.iter_mut().find(|t| t["name"] == "edit") {
            let schema = &mut edit["inputSchema"];
            let d = &schema["$defs"];
            let shared = [
                ("paint", d["set_paint"]["properties"]["fill"].clone()),
                ("pt", d["move"]["properties"]["delta"].clone()),
                ("board_id", d["delete_artboard"]["properties"]["id"].clone()),
                ("local", d["add_path"]["properties"]["local"].clone()),
                ("arrow", d["stroke_style"]["properties"]["arrows"]["properties"]["start"].clone()),
                ("name", d["add_path"]["properties"]["name"].clone()),
                ("bounds", d["resize"]["properties"]["bounds"].clone()),
                ("parent", d["add_path"]["properties"]["parent"].clone()),
            ];
            for (name, original) in shared {
                share(schema, &original, &json!({"$ref":format!("#/$defs/{name}")}));
                schema["$defs"][name] = original;
            }
        }
    }
    // Compact only the combined 1.2 projection; retain standalone named definitions.
    if let Some(tools) = out["tools"].as_array_mut() {
        for tool in tools.iter_mut() {
            let description = match tool["name"].as_str() {
                Some("capabilities") => Some("Negotiate APIs, limits and locally granted scopes."),
                Some("list_boards") => Some("List authorized open boards."),
                Some("describe") => Some("Inspect scoped, paged details or revision changes."),
                Some("select") => Some("Replace selection without a document undo step."),
                Some("history") => Some("Shared undo/redo with revision and idempotency checks."),
                Some("request_status") => Some("Poll this client's retained request receipt."),
                Some("snapshot") => Some("Revision-pinned board/page PNG; maximum 1024 pixels."),
                Some("edit") => Some("Atomic explicit-target batch; one undo; retains selection. API 1.1 tuples or object operations; page verbs use persistent artboard:N IDs."),
                Some("save") => Some("Queue revision-pinned work; returns accepted/ticket, poll request_status. Fresh .vrs/.pdf only under passwd home, /Volumes/<volume>/ or iCloud/CloudStorage providers. Reject /tmp, /private/var, other ~/Library, system roots, app bundle, dot components, existing files and network volumes. Existing parents and canonical containment required. FAT32/exFAT: exclusive-rename fallback after linkat; real volumes unverified."),
                Some("save_as" | "export_pdf") => {
                    Some("Queue revision-pinned file work; poll request_status. All save safeguards apply.")
                }
                _ => None,
            };
            if let Some(description) = description {
                tool["description"] = json!(description);
            }
        }
        if let Some(edit) = tools.iter_mut().find(|t| t["name"] == "edit") {
            let schema = &mut edit["inputSchema"];
            let defs = schema["$defs"].as_object_mut().unwrap();
            // oneOf already rejects the case where both discriminator keys are present.
            defs["op_key"]["oneOf"] = json!([{"required":["verb"]},{"required":["op"]}]);
            defs["op_key"]["type"] = json!("object");
            for definition in defs.values_mut() {
                if definition["allOf"][0] == json!({"$ref":"#/$defs/op_key"}) {
                    let m = definition.as_object_mut().unwrap();
                    m.remove("type");
                    let mut constraints = m.remove("allOf").unwrap().as_array().unwrap().clone();
                    m.insert("$ref".into(), json!("#/$defs/op_key"));
                    constraints.remove(0);
                    if !constraints.is_empty() {
                        m.insert("allOf".into(), json!(constraints));
                    }
                }
            }
            // Repeat depth changes only its children, not its common fields or closure.
            let mut base = defs["repeat0"].clone();
            base["properties"]["ops"] = json!({});
            defs.insert("repeat_base".into(), base);
            for depth in 0..4 {
                let name = format!("repeat{depth}");
                let ops = defs[&name]["properties"]["ops"].clone();
                defs.insert(name, json!({"$ref":"#/$defs/repeat_base","properties":{"ops":ops}}));
            }
            // Preserve every public definition name while sharing identical constraints.
            let mut seen = std::collections::BTreeMap::<String, String>::new();
            let mut aliases = Vec::new();
            for (name, definition) in defs.iter() {
                let key = definition.to_string();
                if let Some(canonical) = seen.get(&key) {
                    aliases.push((name.clone(), canonical.clone(), definition.clone()));
                } else {
                    seen.insert(key, name.clone());
                }
            }
            for (name, canonical, original) in aliases {
                share(schema, &original, &json!({"$ref":format!("#/$defs/{canonical}")}));
                schema["$defs"][&canonical] = original;
                schema["$defs"][name] = json!({"$ref":format!("#/$defs/{canonical}")});
            }
            fn count_constraints(value: &Value, counts: &mut std::collections::BTreeMap<String, (usize, Value)>) {
                match value {
                    Value::Object(m) => {
                        if ["type", "anyOf", "oneOf", "enum", "const"].iter().any(|k| m.contains_key(*k))
                            && value["properties"]["verb"].is_null()
                            && value["properties"]["op"].is_null()
                        {
                            let entry = counts.entry(value.to_string()).or_insert((0, value.clone()));
                            entry.0 += 1;
                        }
                        for child in m.values() {
                            count_constraints(child, counts);
                        }
                    }
                    Value::Array(a) => {
                        for child in a {
                            count_constraints(child, counts);
                        }
                    }
                    _ => {}
                }
            }
            let mut counts = std::collections::BTreeMap::new();
            count_constraints(schema, &mut counts);
            let mut next = 0;
            for (key, (count, original)) in counts {
                let name = format!("merged{next}");
                let reference = json!({"$ref":format!("#/$defs/{name}")});
                if count > 1 && count * key.len() > key.len() + name.len() + 5 + count * reference.to_string().len() {
                    share(schema, &original, &reference);
                    schema["$defs"][name] = original;
                    next += 1;
                }
            }
        }
    }
    out
}

pub fn stroke_style_schema() -> Value {
    let arrows = object(
        json!({"start":{"anyOf":[{"enum":varos_core::stroke::ArrowHead::ALL},{"type":"null"}],"default":null},"end":{"anyOf":[{"enum":varos_core::stroke::ArrowHead::ALL},{"type":"null"}],"default":null},"scale_start":{"type":"number","minimum":0.01,"maximum":100,"default":1},"scale_end":{"type":"number","minimum":0.01,"maximum":100,"default":1},"align":{"enum":["Tip","Extend"],"default":"Tip"}}),
        &[],
    );
    object(
        json!({"cap":{"enum":["Butt","Round","Square"],"default":"Round"},"join":{"enum":["Miter","Round","Bevel"],"default":"Round"},"miter_limit":{"type":"number","minimum":1,"maximum":1000,"default":10},"dash":{"type":"array","items":{"anyOf":[{"const":0},{"type":"number","minimum":0.0001,"maximum":1000000}]},"anyOf":[{"minItems":0,"maxItems":0},{"minItems":2,"maxItems":2},{"minItems":4,"maxItems":4},{"minItems":6,"maxItems":6}],"default":[],"description":"0, 2, 4 or 6 entries; positive entries >= 0.0001; every dash/gap pair has positive sum"},"dash_phase":{"type":"number","minimum":-1000000,"maximum":1000000,"default":0},"align_dashes_to_corners":{"type":"boolean","default":false},"align":{"enum":["Center","Inside","Outside"],"default":"Center"},"arrows":arrows}),
        &[],
    )
}
