//! API 1.2-only bytes-free image discovery and descriptions. Legacy tools/list remains frozen.
use serde_json::{json, Value};
pub fn append_tools(table: &mut Value) {
    if let Some(rows) = table["tools"].as_array_mut() {
        let base = json!({"api":{"const":"1.2"},"board":{"type":"string"},"request_id":{"type":"string"},"expected_rev":{"type":"integer"}});
        let mut props = base.clone();
        props["path"] = json!({"type":"string","minLength":1,"maxLength":4096});
        props["options"] = json!({"type":"object","additionalProperties":false,"properties":{"mode":{"enum":["Embed","Link"]},"at":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}},"bounds":{"type":"array","minItems":4,"maxItems":4,"items":{"type":"number"}},"ppi":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","exclusiveMinimum":0}},"xform":{"type":"object","additionalProperties":false,"properties":{"a":{"type":"number"},"b":{"type":"number"},"c":{"type":"number"},"d":{"type":"number"},"e":{"type":"number"},"f":{"type":"number"}},"required":["a","b","c","d","e","f"]}}});
        rows.push(json!({"name":"add_image","description":"Place local image; embed default; poll file ticket.","inputSchema":{"type":"object","additionalProperties":false,"properties":props,"required":["api","board","request_id","expected_rev","path"]}}));
        let mut props = base;
        props["options"] = json!({"type":"object","properties":{"action":{"enum":["transform","crop","relink","update","embed","unembed","go_to","trace","rasterize","effects_ppi","package"]},"id":{"type":"integer"},"path":{"type":"string"},"bounds":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4},"xform":{"type":"object"},"opacity":{"type":"number"},"ppi":{"type":"number"},"background":{"type":["array","null"]},"options":{"type":"object"}},"required":["action"],"additionalProperties":false});
        props["options"] = json!({"oneOf":ACTIONS.iter().filter_map(|name|operation_schema(name)).collect::<Vec<_>>()});
        rows.push(json!({"name":"image_action","description":"Checked crop/transform/links/trace/rasterize/effects PPI; package copies to a new folder. Image id is an integer from describe scope images.","inputSchema":{"type":"object","additionalProperties":false,"properties":props,"required":["api","board","request_id","expected_rev","options"]}}));
    }
}
pub fn describe(ed: &varos_core::Editor) -> Value {
    json!({"rev":ed.rev,"images":ed.doc.images.iter().map(|i|json!({"id":format!("image:{}",i.id),"key":i.blob,"dimensions":[i.px_w,i.px_h],"source_ppi":i.ppi,"effective_ppi":i.effective_ppi(),"transform":i.xform,"opacity":i.opacity,"placement":i.placement,"link":i.link,"link_status":format!("{:?}",varos_core::images::links::status(i,&ed.blobs)),"proxy_available":ed.blobs.get(&i.blob).is_some(),"full_available":ed.blobs.get(&i.blob).is_some_and(|b|b.original.is_some())})).collect::<Vec<_>>()})
}

/// Explicit image operations share the checked command layer; all payloads remain bytes-free.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Transform { id: u32, xform: varos_core::images::ImageAffine, opacity: f32 },
    Crop { id: u32, bounds: [f32; 4] },
    Relink { id: u32, path: String },
    Update { id: u32 },
    Embed { id: u32 },
    Unembed { id: u32, path: String },
    GoTo { id: u32 },
    Trace { id: u32, options: varos_core::trace::TraceOptions },
    Rasterize { id: u32, ppi: f32, background: Option<[f32; 4]> },
    EffectsPpi { ppi: f32 },
}
pub fn run(ed: &mut varos_core::Editor, op: Operation) -> Result<Value, String> {
    use varos_core::{
        images::{links, ImageEdit, PlacementMode},
        EditCommand,
    };
    let home = crate::files::account_home().ok();
    match op {
        Operation::Transform { id, xform, opacity } => {
            ed.try_execute(EditCommand::Image(ImageEdit::Transform { id, xform, opacity }))?
        }
        Operation::Crop { id, bounds } => ed.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds }))?,
        Operation::Embed { id } => {
            ed.try_execute(EditCommand::Image(ImageEdit::Mode { id, mode: PlacementMode::Embed, link: None }))?
        }
        Operation::Relink { id, path } => {
            links::relink(ed, id, std::path::Path::new(&path), home.as_deref())?;
        }
        Operation::Update { id } => links::update(ed, &[id], home.as_deref())?,
        Operation::Unembed { id, path } => links::unembed(ed, id, std::path::Path::new(&path), home.as_deref())?,
        Operation::GoTo { id } => {
            if !ed.doc.images.iter().any(|i| i.id == id) {
                return Err("Image not found".into());
            }
            ed.objsel.clear();
            ed.objsel.insert(id);
        }
        Operation::Trace { id, options } => {
            varos_core::images::trace::expand(ed, id, &options)?;
        }
        Operation::Rasterize { id, ppi, background } => {
            varos_raster::images::rasterize_object(ed, id, ppi, background)?
        }
        Operation::EffectsPpi { ppi } => ed.try_execute(EditCommand::Image(ImageEdit::EffectsPpi(ppi)))?,
    }
    Ok(describe(ed))
}

pub const ACTIONS: &[&str] = &[
    "transform",
    "crop",
    "relink",
    "update",
    "embed",
    "unembed",
    "go_to",
    "trace",
    "rasterize",
    "effects_ppi",
    "package",
];
pub fn operation_schema(name: &str) -> Option<Value> {
    if !ACTIONS.contains(&name) {
        return None;
    }
    let mut properties = json!({"action":{"const":name}});
    let mut required = vec!["action"];
    if !matches!(name, "effects_ppi" | "package") {
        properties["id"] = json!({"type":"integer","minimum":1,"maximum":u32::MAX});
        required.push("id");
    }
    if matches!(name, "package" | "relink" | "unembed") {
        properties["path"] = json!({"type":"string","minLength":1,"maxLength":4096});
        required.push("path");
    }
    if name == "transform" {
        properties["xform"] = json!({"type":"object","additionalProperties":false,"properties":{"a":{"type":"number"},"b":{"type":"number"},"c":{"type":"number"},"d":{"type":"number"},"e":{"type":"number"},"f":{"type":"number"}},"required":["a","b","c","d","e","f"]});
        properties["opacity"] = json!({"type":"number","minimum":0,"maximum":1});
        required.extend(["xform", "opacity"]);
    }
    if name == "crop" {
        properties["bounds"] = json!({"type":"array","minItems":4,"maxItems":4,"items":{"type":"number"}});
        required.push("bounds");
    }
    if matches!(name, "rasterize" | "effects_ppi") {
        properties["ppi"] = json!({"type":"number","minimum":1,"maximum":2400});
        required.push("ppi");
    }
    if name == "rasterize" {
        properties["background"] = json!({"type":["array","null"],"minItems":4,"maxItems":4,"items":{"type":"number","minimum":0,"maximum":1}});
        required.push("background");
    }
    if name == "trace" {
        properties["options"] = json!({"type":"object","additionalProperties":false,"properties":{"mode":{"oneOf":[{"enum":["BlackWhite","Grayscale"]},{"type":"object","additionalProperties":false,"properties":{"Color":{"type":"object","additionalProperties":false,"properties":{"colors":{"type":"integer","minimum":1,"maximum":255}},"required":["colors"]}},"required":["Color"]}]},"threshold":{"type":"integer","minimum":0,"maximum":255},"paths_fidelity":{"type":"number","minimum":0,"maximum":100},"corners":{"type":"number","minimum":0,"maximum":100},"noise_px":{"type":"integer","minimum":0,"maximum":u32::MAX},"ignore_white":{"type":"boolean"}}});
        required.push("options");
    }
    Some(json!({"type":"object","additionalProperties":false,"properties":properties,"required":required}))
}
