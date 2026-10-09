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
    let mut list = legacy_tools_list();
    if api != "1.2" {
        return list;
    }
    // Transform schemas and compaction belong exclusively to the opt-in projection.
    if let Some(entries) = list["tools"].as_array_mut() {
        if let Some(edit) = entries.iter_mut().find(|tool| tool["name"] == "edit") {
            let mut extra = serde_json::Map::new();
            let mut ops = Vec::new();
            // ---- Lane E ----
            crate::view_depth::extend_schema(&mut edit["inputSchema"]);
            crate::select_transform::schemas(&mut extra, &mut ops);
            // ---- Lane D: schemas feed both list_verbs and progressive schema ----
            crate::drawing::schemas(&mut extra, &mut ops);
            if let Some(defs) = edit["inputSchema"]["$defs"].as_object_mut() {
                defs.extend(extra);
            }
            if let Some(all) = edit["inputSchema"]["$defs"]["operation"]["anyOf"].as_array_mut() {
                all.extend(ops);
            }
        }
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
/// Default discovery is the frozen main-now legacy projection.
pub fn tools() -> Value {
    legacy_tools_list()
}

// Copied verbatim from main-now (73f3221), except for the function name.
// Keep API 1.2 mutations outside this builder; fixtures lock its serialized bytes.
fn legacy_tools_list() -> Value {
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
            json!({"verb":{"const":"align"},"ids":edit_ids,"mode":{"enum":["left","center","right","top","middle","bottom"]},"target":{"type":"string","pattern":"^(selection|key_object|artboard:[1-9][0-9]*|\\$[A-Za-z][A-Za-z0-9_]{0,62}|a[0-9]+@[0-9]+)$","description":"selection, artboard:N, a request-local bound to an artboard, or the deprecated revision-bound aN@rev"}}),
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
    for name in ["select", "edit"] {
        if let Some(schema) = schemas.get_mut(name) {
            schema["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
        }
    }
    if let Some(schema) = schemas.get_mut("select") {
        schema["properties"]["paste_remembers_layers"] =
            json!({"type":"boolean","description":"API 1.2 app paste preference; ids must be empty"});
        schema["properties"]["lasso"] = json!({"type":"object","additionalProperties":false,"required":["points","objects","additive"],"properties":{"points":{"type":"array","minItems":3,"maxItems":4096,"items":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}}},"objects":{"type":"boolean"},"additive":{"type":"boolean"}}});
        schema["properties"]["mode"] = json!({"description":"API 1.2 only; omitted retains explicit ids selection", "oneOf":[{"enum":["all","deselect","reselect","inverse","above","below","artboard"]},{"type":"object","additionalProperties":false,"required":["same"],"properties":{"same":{"enum":["fill","fill_stroke","stroke","stroke_weight","opacity","appearance"]}}},{"type":"object","additionalProperties":false,"required":["key_object"],"properties":{"key_object":{"type":"integer","minimum":1}}},{"type":"object","additionalProperties":false,"required":["group"],"properties":{"group":{"type":"integer","minimum":1}}}]});
    }
    if let Some(schema) = schemas.get_mut("edit") {
        let wave = [
            object(
                json!({"verb":{"const":"view"},"ids":ids,"action":{"description":"API 1.2 view actions","oneOf":[{"enum":["make_guides","release_guides","clear_guides","toggle_grid","convert_artboards"]},{"type":"object","minProperties":1,"maxProperties":1,"additionalProperties":false,"properties":{"grid":object(json!({"spacing":{"type":"number","minimum":0.01,"maximum":1000000},"subdivisions":{"type":"integer","minimum":1,"maximum":100}}), &["spacing","subdivisions"]),"guide_position":object(json!({"index":{"type":"integer","minimum":0},"position":{"type":"number"}}), &["index","position"]),"fit_artboard":object(json!({"id":{"type":"integer","minimum":1},"selected":{"type":"boolean"}}), &["id","selected"]),"reorder_artboard":object(json!({"id":{"type":"integer","minimum":1},"position":{"type":"integer","minimum":0}}), &["id","position"])}}]}}),
                &["verb", "ids", "action"],
            ),
            object(
                json!({"verb":{"const":"anchor_type"},"ids":ids,"anchor":{"type":"integer","minimum":1},"smooth":{"type":"boolean"}}),
                &["verb", "ids", "anchor", "smooth"],
            ),
            object(
                json!({"verb":{"const":"object"},"ids":{"type":"array","maxItems":1000,"items":{"type":"string"}},"action":{"enum":["lock","unlock_all","hide","show_all","expand_transform","reverse","average","add_anchors","clean_up","join","compound_make","compound_release","new_layer","new_sublayer","send_to_current_layer"]},"anchors":{"type":"array","maxItems":1000,"items":{"type":"integer","minimum":1}}}),
                &["verb", "ids", "action"],
            ),
            object(
                json!({"verb":{"const":"insert_anchor"},"ids":ids,"segment":{"type":"integer","minimum":0},"t":{"type":"number","exclusiveMinimum":0,"exclusiveMaximum":1}}),
                &["verb", "ids", "segment", "t"],
            ),
            object(
                json!({"verb":{"const":"delete_anchor"},"ids":ids,"anchor":{"type":"integer","minimum":1}}),
                &["verb", "ids", "anchor"],
            ),
            object(
                json!({"verb":{"const":"distribute_mode"},"ids":ids,"mode":{"enum":["left","center","right","top","middle","bottom"]}}),
                &["verb", "ids", "mode"],
            ),
            object(
                json!({"verb":{"const":"distribute_spacing"},"ids":ids,"axis":{"enum":["h","v"]},"gap":{"type":"number"}}),
                &["verb", "ids", "axis", "gap"],
            ),
        ];
        if let Some(ops) = schema["$defs"]["operation"]["anyOf"].as_array_mut() {
            ops.extend(wave);
        }
    }
    // Intern repeated edit schemas rather than widening the tools/list byte ratchet.
    if let Some(schema) = schemas.get_mut("edit") {
        fn intern(value: &mut Value, patterns: &[(&str, Value)]) {
            if let Some((name, _)) = patterns.iter().find(|(_, pattern)| value == pattern) {
                *value = json!({"$ref":format!("#/$defs/{name}")});
                return;
            }
            match value {
                Value::Object(map) => {
                    for v in map.values_mut() {
                        intern(v, patterns);
                    }
                }
                Value::Array(array) => {
                    for v in array {
                        intern(v, patterns);
                    }
                }
                _ => {}
            }
        }
        let patterns = [
            ("paint", paint.clone()),
            ("point", point.clone()),
            ("page_bounds", page_bounds.clone()),
            ("artboard", artboard.clone()),
            ("object_ids", edit_ids.clone()),
            ("plain_ids", ids.clone()),
            ("bounds", bounds.clone()),
            ("local", local.clone()),
            ("name", name.clone()),
        ];
        intern(schema, &patterns);
        if let Some(defs) = schema["$defs"].as_object_mut() {
            for (name, value) in patterns {
                defs.insert(name.into(), value);
            }
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
            "tools/call"
                if params["name"].as_str().is_none_or(|name| {
                    !TOOLS.contains(&name)
                        && name != "import_svg"
                        && !["schema", "list_verbs"].contains(&name)
                        && !crate::TOOLS_12.contains(&name)
                        && !["export_svg", "export_raster", "save_template", "new_from_template", "window_memory"]
                            .contains(&name)
                }) =>
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

/// Explicit 1.2 schema projection; legacy tools() remains byte-frozen.
pub fn tools_for_api(api: &str) -> Value {
    tools_for(api)
}

pub(crate) fn full_tools_for(api: &str) -> Value {
    let mut out = construction_tools(api);
    if api != "1.2" {
        return out;
    }
    if let Some(rows) = out["tools"].as_array_mut() {
        rows.push(json!({"name":"import_svg","description":"SVG/SVGZ; files scope; undo; losses.","inputSchema":object(json!({"api":{"const":"1.2"},"board":{"type":"string"},"request_id":{"type":"string"},"expected_rev":{"type":"integer"},"path":{"type":"string"}}), &["api","board","request_id","expected_rev","path"])}));
    }
    append_export_tools(&mut out);
    append_document_tools(&mut out);
    if let Some(list) = out["tools"].as_array_mut() {
        if let Some(export) = list.iter_mut().find(|v| v["name"] == "export_pdf") {
            export["inputSchema"]["properties"]["options"] =
                json!({"type":"object","description":"PDF preset, image_ppi, compress_streams, boxes and marks"});
        }
    }
    if let Some(list) = out["tools"].as_array_mut() {
        for name in crate::TOOLS_12 {
            list.push(json!({"name":name,"description":"API 1.2 desktop host effect. Print opens a prepared PDF in Preview; Copy/Cut publish the current selection to the OS clipboard.","inputSchema": {"type":"object","additionalProperties":false,"properties":{"api":{"const":"1.2"},"board":{"type":"string"},"request_id":{"type":"string"},"expected_rev":{"type":"integer"},"scope":{"enum":["active_artboard","all_visible_artboards","artwork_bounds"]},"options":{"type":"object"}},"required":["api","board","request_id","expected_rev"]}}));
        }
    }
    if let Some(rows) = out["tools"].as_array_mut() {
        if let Some(edit) = rows.iter_mut().find(|r| r["name"] == "edit") {
            if let Some(ops) = edit["inputSchema"]["$defs"]["operation"]["anyOf"].as_array_mut() {
                ops.extend(
                ["clip", "release_clip"].map(|verb| object(json!({"verb":{"const":verb},"ids":{"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string"}}}), &["verb","ids"])));
            }
        }
    }
    let style = stroke_style_schema();
    if let Some(tools) = out["tools"].as_array_mut() {
        for tool in tools {
            let name = tool["name"].as_str().unwrap_or("").to_owned();
            if ![
                "import_svg",
                "export_svg",
                "export_raster",
                "save_template",
                "new_from_template",
                "window_memory",
                "print",
                "copy",
                "cut",
            ]
            .contains(&name.as_str())
            {
                tool["inputSchema"]["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
            }
            if name == "describe" {
                if let Some(fields) = tool["inputSchema"]["properties"]["fields"]["items"]["enum"].as_array_mut() {
                    fields.push(json!("stroke_style"));
                }
            }
            if name == "edit" {
                let schema = &mut tool["inputSchema"];
                schema["$defs"]["trace_rgba"] = object(
                    json!({"verb":{"const":"trace_rgba"},"rgba":{"type":"array","items":{"type":"integer","minimum":0,"maximum":255}},"width":{"type":"integer","minimum":1},"height":{"type":"integer","minimum":1},"options":{"type":"object","description":"API 1.2 only: TraceOptions; mode BlackWhite, Grayscale, or {Color:{colors:1..255}}; fidelity/corners 0..100, threshold 0..255, noise_px, ignore_white"}}),
                    &["verb", "rgba", "width", "height"],
                );
                if let Some(ops) = schema["$defs"]["operation"]["anyOf"].as_array_mut() {
                    ops.push(json!({"$ref":"#/$defs/trace_rgba"}));
                }
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
                if original.is_null() || original.get("$ref").is_some() {
                    continue;
                }
                share(schema, &original, &json!({"$ref":format!("#/$defs/{name}")}));
                schema["$defs"][name] = original;
            }
        }
    }
    // Compact only the combined 1.2 projection; retain standalone named definitions.
    if let Some(tools) = out["tools"].as_array_mut() {
        for tool in tools.iter_mut() {
            let description = match tool["name"].as_str() {
                Some("capabilities") => Some("APIs/limits/scopes."),
                Some("list_boards") => Some("Open boards."),
                Some("describe") => Some("Paged detail/diff."),
                Some("select") => Some("Select; no undo."),
                Some("history") => Some("Checked undo/redo."),
                Some("request_status") => Some("Poll receipt."),
                Some("snapshot") => Some("Pinned PNG; <=1024px."),
                Some("edit") => Some("Atomic targets; one undo; selection retained. Tuples/objects, repeat, defaults, IDs receipts. Trace: pixels. Pages: artboard:N."),
                Some("save") => Some("Pinned file job; poll request_status. Fresh .vrs/.pdf in home/Volumes/iCloud/CloudStorage; local volumes, existing canonical parents. Protected/dot/symlink/hardlink/overwrite guards; see capabilities."),
                Some("save_as" | "export_pdf") => {
                    Some("Pinned file job; save guards; poll request_status.")
                }
                Some("export_svg" | "export_raster") => Some("Pinned output; file guards; poll receipt."),
                Some("save_template" | "new_from_template") => Some("NAME.vrs; poll receipt; opening is dirty Untitled."),
                Some("window_memory") => Some("Persisted window geometry."),
                Some("print") => Some("Open prepared PDF in Preview."),
                Some("copy" | "cut") => Some("Publish selection; cut deletes with undo."),
                _ => None,
            };
            if let Some(description) = description {
                tool["description"] = json!(description);
            }
        }
        if let Some(edit) = tools.iter_mut().find(|t| t["name"] == "edit") {
            let schema = &mut edit["inputSchema"];
            let Some(defs) = schema["$defs"].as_object_mut() else {
                // Compaction is optional; retain the original projection if definitions are absent.
                return out;
            };
            // oneOf already rejects the case where both discriminator keys are present.
            defs["op_key"]["oneOf"] = json!([{"required":["verb"]},{"required":["op"]}]);
            defs["op_key"]["type"] = json!("object");
            for definition in defs.values_mut() {
                if definition["allOf"][0] == json!({"$ref":"#/$defs/op_key"}) {
                    let Some(m) = definition.as_object_mut() else {
                        // Leave non-object definitions unchanged rather than compacting them.
                        continue;
                    };
                    let Some(Value::Array(mut constraints)) = m.get("allOf").cloned() else {
                        // Preserve the original definition if its constraints cannot be compacted.
                        continue;
                    };
                    m.remove("type");
                    m.remove("allOf");
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
                let name = format!("{next:x}");
                let reference = json!({"$ref":format!("#/$defs/{name}")});
                if count > 1 && count * key.len() > key.len() + name.len() + 5 + count * reference.to_string().len() {
                    share(schema, &original, &reference);
                    schema["$defs"][name] = original;
                    next += 1;
                }
            }
        }
    }
    if let Some(rows) = out["tools"].as_array_mut() {
        rows.push(json!({"name":"schema","description":"Full params schema on demand; no document required.","inputSchema":object(json!({"api":{"const":"1.2"},"tool":{"type":"string"},"verb":{"type":"string"}}), &["api","tool"])}));
        rows.push(json!({"name":"list_verbs","description":"Grouped names and one-line descriptions; no document required.","inputSchema":object(json!({"api":{"const":"1.2"}}), &["api"])}));
    }
    out
}

/// API 1.2 publishes core schemas and an index of extended edit operations.
/// Discovery never changes the typed decoder or execution path.
pub fn tools_for(api: &str) -> Value {
    if api != "1.2" {
        return legacy_tools_list();
    }
    let mut out = full_tools_for(api);
    if let Some(rows) = out["tools"].as_array_mut() {
        if let Some(edit) = rows.iter_mut().find(|row| row["name"] == "edit") {
            let root = edit["inputSchema"].clone();
            let mut alternatives = Vec::new();
            let mut extended = Vec::new();
            if let Some(ops) = root["$defs"]["operation"]["anyOf"].as_array() {
                for op in ops {
                    let expanded = expand_schema(op, &root);
                    let verb = schema_verb(&expanded);
                    if verb.is_none_or(core_verb) {
                        alternatives.push(op.clone());
                    } else if let Some(verb) = verb {
                        extended.push(verb.to_owned());
                    }
                }
            }
            let descriptions = extended
                .iter()
                .map(|verb| format!("{verb}: {}", verb_description(verb)))
                .collect::<Vec<_>>()
                .join("; ");
            alternatives.push(json!({"type":"object","properties":{"verb":{"enum":extended},"op":{"enum":extended}},"oneOf":[{"required":["verb"],"not":{"required":["op"]}},{"required":["op"],"not":{"required":["verb"]}}],"description":descriptions}));
            edit["inputSchema"]["$defs"]["operation"]["anyOf"] = json!(alternatives);
            prune_definitions(&mut edit["inputSchema"]);
            edit["description"] = json!("Atomic typed edits; core params inline. Extended verbs: call schema api 1.2 tool edit verb NAME before use; list_verbs groups all verbs. Decoder validates all params.");
        }
    }
    out
}

fn schema_verb(schema: &Value) -> Option<&str> {
    schema["properties"]["verb"]["const"]
        .as_str()
        .or_else(|| schema.get("allOf").and_then(Value::as_array).and_then(|parts| parts.iter().find_map(schema_verb)))
}

fn core_verb(verb: &str) -> bool {
    matches!(
        verb,
        "move"
            | "set_paint"
            | "add_shape"
            | "add_path"
            | "resize"
            | "rotate"
            | "rename"
            | "align"
            | "group"
            | "order"
            | "delete"
            | "ungroup"
            | "repeat"
    )
}

fn verb_description(verb: &str) -> String {
    match verb {
        "view" => "Outline, pixel preview/snap, Navigator, screen modes and canvas preferences".into(),
        "clip" => "Create a clipping group".into(),
        "release_clip" => "Release a clipping group".into(),
        "pathfinder" => "Combine paths with a Boolean operation".into(),
        "trace_rgba" => "Trace RGBA pixels into vector paths".into(),
        "set_stroke_style" | "stroke_style" => "Set caps, joins, dashes and arrows".into(),
        "document_setup" => "Set units, PPI, bleed or transparency grid".into(),
        "repeat" => "Repeat creation operations with an offset".into(),
        _ => format!("Apply {}", verb.replace('_', " ")),
    }
}

/// Expand local references while preserving sibling validation constraints.
fn expand_schema(value: &Value, root: &Value) -> Value {
    if let Some(pointer) = value.get("$ref").and_then(Value::as_str).and_then(|s| s.strip_prefix('#')) {
        if let Some(target) = root.pointer(pointer) {
            let target = expand_schema(target, root);
            if let Some(map) = value.as_object() {
                let siblings: serde_json::Map<String, Value> = map
                    .iter()
                    .filter(|(key, _)| key.as_str() != "$ref")
                    .map(|(key, value)| (key.clone(), expand_schema(value, root)))
                    .collect();
                if siblings.is_empty() {
                    return target;
                }
                let mut siblings = Value::Object(siblings);
                if let Some(map) = siblings.as_object_mut() {
                    let constraints = map.entry("allOf").or_insert_with(|| json!([]));
                    if let Some(constraints) = constraints.as_array_mut() {
                        constraints.push(target);
                        return siblings;
                    }
                }
            }
        }
    }
    match value {
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), expand_schema(v, root))).collect()),
        Value::Array(array) => Value::Array(array.iter().map(|v| expand_schema(v, root)).collect()),
        _ => value.clone(),
    }
}

/// Retain only definitions reachable from the public schema, including transitive refs.
fn prune_definitions(schema: &mut Value) {
    fn refs(value: &Value, found: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                if let Some(name) = map.get("$ref").and_then(Value::as_str).and_then(|s| s.strip_prefix("#/$defs/")) {
                    found.insert(name.split('/').next().unwrap_or(name).into());
                }
                for (key, child) in map {
                    if key != "$defs" {
                        refs(child, found);
                    }
                }
            }
            Value::Array(array) => {
                for child in array {
                    refs(child, found);
                }
            }
            _ => {}
        }
    }
    let mut needed = std::collections::BTreeSet::new();
    refs(schema, &mut needed);
    loop {
        let previous = needed.clone();
        for name in &previous {
            refs(&schema["$defs"][name], &mut needed);
        }
        if needed == previous {
            break;
        }
    }
    if let Some(defs) = schema["$defs"].as_object_mut() {
        defs.retain(|name, _| needed.contains(name));
    }
}

pub fn schema(tool: &str, verb: Option<&str>) -> Result<Value, Error> {
    let table = full_tools_for("1.2");
    let root = table["tools"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["name"] == tool))
        .map(|row| &row["inputSchema"])
        .ok_or_else(|| Error::new("invalid_argument", "unknown schema tool"))?;
    let Some(verb) = verb else {
        if tool == "edit" {
            return Err(Error::new("invalid_argument", "edit schema requires a verb"));
        }
        return Ok(root.clone());
    };
    if tool != "edit" {
        return Err(Error::new("invalid_argument", "verb discovery requires tool edit"));
    }
    let name = if verb == "stroke_style" { "set_stroke_style" } else { verb };
    if let Some(ops) = root["$defs"]["operation"]["anyOf"].as_array() {
        for op in ops {
            let expanded = expand_schema(op, root);
            if schema_verb(&expanded) == Some(name) {
                let mut params = op
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|reference| reference.strip_prefix('#'))
                    .and_then(|pointer| root.pointer(pointer))
                    .unwrap_or(op)
                    .clone();
                params["$defs"] = root["$defs"].clone();
                prune_definitions(&mut params);
                return Ok(params);
            }
        }
    }
    Err(Error::new("invalid_argument", "unknown edit verb"))
}

pub fn list_verbs() -> Value {
    let table = full_tools_for("1.2");
    let mut core = Vec::new();
    let mut extended = Vec::new();
    let mut tools = Vec::new();
    if let Some(rows) = table["tools"].as_array() {
        for row in rows {
            let name = row["name"].as_str().unwrap_or_default();
            if name == "edit" {
                let root = &row["inputSchema"];
                if let Some(ops) = root["$defs"]["operation"]["anyOf"].as_array() {
                    for op in ops {
                        let expanded = expand_schema(op, root);
                        if let Some(verb) = schema_verb(&expanded) {
                            let entry = json!({"name":verb,"description":verb_description(verb)});
                            if core_verb(verb) {
                                core.push(entry);
                            } else {
                                extended.push(entry);
                            }
                        }
                    }
                }
            } else {
                tools.push(json!({"name":name,"description":row["description"]}));
            }
        }
    }
    json!({"api":"1.2","groups":[{"tool":"edit","group":"core","verbs":core},{"tool":"edit","group":"extended","verbs":extended},{"group":"tools","verbs":tools}]})
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

fn append_export_tools(result: &mut Value) {
    let Some(list) = result.get_mut("tools").and_then(Value::as_array_mut) else { return };
    for name in ["export_svg", "export_raster"] {
        let mut properties = json!({"api":{"const":"1.2"},"board":{"type":"string"},"request_id":{"type":"string"},"expected_rev":{"type":"integer","minimum":0},"path":{"type":"string"},"scope":{"type":"string","description":"all_visible_artboards, whole_board, artwork_bounds, selection or artboard:N"}});
        if name == "export_raster" {
            properties["format"] = json!({"enum":["png","jpeg","webp","tiff"],"default":"png"});
            properties["scale"] = json!({"type":"number","exclusiveMinimum":0,"maximum":64,"default":1});
            properties["ppi"] = json!({"type":"number","exclusiveMinimum":0,"maximum":4608});
            properties["quality"] = json!({"type":"integer","minimum":0,"maximum":100,"default":90});
            properties["transparent"] = json!({"type":"boolean","default":true});
        }
        list.push(json!({"name":name,"description":"Queue revision-pinned deliverables with ExportReport; fresh destination only. Additional artboards are written beside path under their artboard names.","inputSchema":object(properties,&["api","board","request_id","expected_rev","path","scope"])}));
    }
}

/// Additive opt-in schema projection. Legacy tools/list remains byte-identical.
fn append_document_tools(out: &mut Value) {
    if let Some(list) = out["tools"].as_array_mut() {
        for name in ["save_template", "new_from_template"] {
            list.push(json!({"name":name,"description":"API 1.2 folder-convention template; path is a plain NAME.vrs. Returns accepted/ticket; poll receipt for completion. Opening creates Untitled and dirty.","inputSchema":object(json!({"api":{"const":"1.2"},"board":{"type":"string"},"request_id":{"type":"string"},"expected_rev":{"type":"integer"},"path":{"type":"string"}}),&["api","board","request_id","expected_rev","path"])}));
        }
        list.push(json!({"name":"window_memory","description":"API 1.2 persisted window geometry query.","inputSchema":object(json!({"api":{"const":"1.2"}}),&["api"])}));
        for tool in list.iter_mut() {
            if ["edit", "describe", "capabilities"].contains(&tool["name"].as_str().unwrap_or_default()) {
                tool["inputSchema"]["properties"]["api"] = json!({"enum":["1.0","1.1","1.2"],"default":"1.0"});
            }
            if tool["name"] == "edit" {
                tool["inputSchema"]["$defs"]["document_setup"] = object(
                    json!({"verb":{"const":"document_setup"},"field":{"enum":["units","ppi","bleed","transparency_grid"]},"value":{},"artboard":{"type":"string","pattern":"^artboard:[1-9][0-9]*$"}}),
                    &["verb", "field", "value"],
                );
                if let Some(ops) = tool["inputSchema"]["$defs"]["operation"]["anyOf"].as_array_mut() {
                    ops.push(json!({"$ref":"#/$defs/document_setup"}));
                }
                tool["description"]=json!("API 1.2 adds ops {verb:document_setup, field:units|ppi|bleed|transparency_grid, value, artboard?:artboard:N}; bleed is top/right/bottom/left in pt.");
            }
            if tool["name"] == "describe" {
                if let Some(fields) = tool["inputSchema"]["properties"]["fields"]["items"]["enum"].as_array_mut() {
                    fields.push(json!("document_info"));
                }
            }
        }
    }
}

#[cfg(test)]
fn compact_api_12_schema(schema: &mut Value) {
    fn visit(value: &mut Value, action: &mut impl FnMut(&mut Value)) {
        action(value);
        match value {
            Value::Object(map) => {
                for child in map.values_mut() {
                    visit(child, action);
                }
            }
            Value::Array(array) => {
                for child in array {
                    visit(child, action);
                }
            }
            _ => {}
        }
    }
    visit(schema, &mut |value| {
        if let Some(map) = value.as_object_mut() {
            map.remove("description");
        }
    });
    let Some(defs) = schema["$defs"].as_object() else { return };
    let names: std::collections::BTreeMap<_, _> = defs
        .keys()
        .filter(|name| !matches!(name.as_str(), "operation" | "document_setup"))
        .enumerate()
        .map(|(index, name)| (name.clone(), index.to_string()))
        .collect();
    visit(schema, &mut |value| {
        if let Some(reference) = value.get_mut("$ref") {
            if let Some(name) = reference.as_str().and_then(|s| s.strip_prefix("#/$defs/")) {
                if let Some(short) = names.get(name) {
                    *reference = json!(format!("#/$defs/{short}"));
                }
            }
        }
    });
    let Some(old_defs) = schema["$defs"].as_object_mut() else { return };
    let defs = std::mem::take(old_defs);
    for (name, value) in defs {
        old_defs.insert(names.get(&name).cloned().unwrap_or(name), value);
    }
    // Repeated numeric/parent constraints are cheaper as shared definitions.
    for (index, pattern) in [
        json!({"type":"number","minimum":0,"maximum":1}),
        json!({"type":"string","pattern":"^node:[1-9][0-9]*$"}),
        json!({"type":"number","minimum":0}),
        json!({"type":"integer","minimum":1,"maximum":100}),
        json!({"type":"integer","minimum":1}),
        json!({"type":"integer","minimum":0}),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("c{index}");
        let mut count = 0;
        visit(schema, &mut |value| {
            if *value == pattern {
                *value = json!({"$ref":format!("#/$defs/{name}")});
                count += 1;
            }
        });
        if count > 0 {
            schema["$defs"][name] = pattern;
        }
    }
}

/// Compatibility entry point for API 1.2 discovery.
pub fn tools_12() -> Value {
    tools_for_api("1.2")
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn api_12_schema_compaction_preserves_validation_constraints() {
        fn expand(value: &Value, root: &Value, depth: usize) -> Value {
            assert!(depth < 64, "cyclic schema reference");
            if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
                let pointer = reference.strip_prefix('#').expect("local schema reference");
                return expand(root.pointer(pointer).expect("resolved schema reference"), root, depth + 1);
            }
            match value {
                Value::Object(map) => Value::Object(
                    map.iter()
                        .filter(|(key, _)| !matches!(key.as_str(), "$defs" | "description"))
                        .map(|(key, value)| (key.clone(), expand(value, root, depth + 1)))
                        .collect(),
                ),
                Value::Array(array) => Value::Array(array.iter().map(|v| expand(v, root, depth + 1)).collect()),
                _ => value.clone(),
            }
        }
        let mut table = tools();
        append_export_tools(&mut table);
        append_document_tools(&mut table);
        // Includes nested repeat/tuple references and all additive document schemas.
        for row in table["tools"].as_array().unwrap() {
            let original = &row["inputSchema"];
            let mut compact = original.clone();
            compact_api_12_schema(&mut compact);
            assert_eq!(expand(original, original, 0), expand(&compact, &compact, 0), "{}", row["name"]);
        }
    }

    #[test]
    fn api_12_discovery_unifies_export_and_command_lanes() {
        for api in ["1.0", "1.1"] {
            assert_eq!(tools_for_api(api), tools());
            assert!(!tools_for_api(api)["tools"].as_array().unwrap().iter().any(|row| {
                matches!(
                    row["name"].as_str(),
                    Some("export_svg" | "export_raster" | "save_template" | "new_from_template" | "window_memory")
                )
            }));
        }
        let table = tools_for_api("1.2");
        assert_eq!(tools_12(), table);
        let rows = table["tools"].as_array().unwrap();
        let mut names = std::collections::HashSet::new();
        for row in rows {
            assert!(names.insert(row["name"].as_str().unwrap()), "duplicate tool: {row}");
        }
        for name in [
            "export_pdf",
            "export_svg",
            "export_raster",
            "capabilities",
            "select",
            "edit",
            "save_template",
            "new_from_template",
            "window_memory",
            "print",
            "copy",
            "cut",
        ] {
            assert!(names.contains(name), "missing {name}");
        }
        for name in ["export_svg", "export_raster", "save_template", "new_from_template", "window_memory"] {
            let row = rows.iter().find(|row| row["name"] == name).unwrap();
            assert_eq!(row["inputSchema"]["properties"]["api"]["const"], "1.2");
        }
        for verb in ["clip", "release_clip", "view", "object", "anchor_type", "distribute_mode", "distribute_spacing"] {
            assert_eq!(schema_verb(&schema("edit", Some(verb)).unwrap()), Some(verb));
        }
    }
}
