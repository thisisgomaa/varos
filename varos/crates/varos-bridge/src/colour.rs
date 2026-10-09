//! Lane B: opt-in typed paint/table/recolor operations, discoverable through schema/list_verbs.
use crate::{
    dto::{Error, Operation},
    service::resolve,
};
use std::collections::BTreeSet;
use varos_core::{colour_commands::ColourCommand as C, EditCommand, Editor};
pub(crate) fn apply(
    ed: &mut Editor,
    op: &Operation,
    command: &C,
    affected: &mut BTreeSet<String>,
) -> Result<Option<u32>, Error> {
    if matches!(command, C::Begin | C::Live { .. } | C::Commit | C::Cancel) {
        return Err(Error::new("unsupported", "Bridge colour edits are atomic; use paint for the final gesture"));
    }
    let ids = if op.ids().is_empty() { vec![] } else { resolve(&ed.doc, op.ids(), false)? };
    if matches!(command, C::Paint { .. } | C::Recolor { .. } | C::Reduce { .. }) {
        if ids.is_empty() {
            return Err(Error::new("invalid_argument", "colour paint requires targets"));
        }
        ed.try_execute(EditCommand::SelectPaths(ids.clone())).map_err(|e| Error::new("invalid_argument", e))?;
    } else if !ids.is_empty() {
        return Err(Error::new("invalid_argument", "swatch table edits omit targets"));
    }
    ed.try_execute(EditCommand::Colour(command.clone())).map_err(|e| Error::new("invalid_argument", e))?;
    affected.extend(ids.iter().map(|p| format!("path:{p}")));
    Ok(None)
}
pub(crate) fn schemas(defs: &mut serde_json::Map<String, serde_json::Value>, ops: &mut Vec<serde_json::Value>) {
    use serde_json::json;
    let unit = json!({"type":"number","minimum":0,"maximum":1});
    let rgba = json!({"type":"array","minItems":4,"maxItems":4,"items":unit});
    let grad = json!({"type":"object","additionalProperties":false,"required":["kind","stops","spread","placement","focal"],"properties":{"kind":{"enum":["linear","radial"]},"stops":{"type":"array","minItems":2,"maxItems":256,"items":{"type":"object","additionalProperties":false,"required":["offset","colour","opacity","midpoint"],"properties":{"offset":unit,"colour":rgba,"opacity":unit,"midpoint":{"type":"number","minimum":0.01,"maximum":0.99}}}},"spread":{"enum":["pad","reflect","repeat"]},"placement":{"type":"array","minItems":6,"maxItems":6,"items":{"type":"number"}},"focal":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}}}});
    let paint = json!({"anyOf":[{"type":"null"},rgba,{"type":"object","additionalProperties":false,"required":["type","value"],"properties":{"type":{"const":"gradient"},"value":grad}},{"type":"object","additionalProperties":false,"required":["type","value"],"properties":{"type":{"const":"swatch_ref"},"value":{"type":"object","additionalProperties":false,"required":["id"],"properties":{"id":{"type":"integer","minimum":1}}}}}]});
    let swatch = json!({"type":"object","additionalProperties":false,"required":["id","name","paint"],"properties":{"id":{"type":"integer","minimum":1},"name":{"type":"string","minLength":1,"maxLength":256},"paint":paint,"global":{"type":"boolean"},"group":{"type":"string","maxLength":256}}});
    let command = json!({"oneOf":[obj(json!({"action":{"const":"reduce"},"count":{"type":"integer","minimum":1,"maximum":256}}),&["action","count"]),obj(json!({"action":{"const":"import_palette"},"format":{"enum":["gpl","ase","native"]},"data":{"type":"array","maxItems":65536,"items":{"type":"integer","minimum":0,"maximum":255}}}),&["action","format","data"]),obj(json!({"action":{"const":"tool"}}),&["action"]),obj(json!({"action":{"const":"paint"},"target":{"enum":["Fill","Stroke"]},"paint":paint}),&["action","target","paint"]),obj(json!({"action":{"const":"upsert_swatch"},"swatch":swatch}),&["action","swatch"]),obj(json!({"action":{"const":"delete_swatch"},"id":{"type":"integer","minimum":1}}),&["action","id"]),obj(json!({"action":{"const":"import_swatches"},"swatches":{"type":"array","maxItems":4096,"items":swatch}}),&["action","swatches"]),obj(json!({"action":{"const":"recolor"},"palette":{"type":"array","minItems":1,"maxItems":256,"items":rgba}}),&["action","palette"])]});
    defs.insert("colour".into(),obj(json!({"verb":{"const":"colour"},"ids":{"type":"array","maxItems":1000,"items":{"type":"string","pattern":"^(path|node):[1-9][0-9]*$"}},"command":command}),&["verb","ids","command"]));
    ops.push(json!({"$ref":"#/$defs/colour"}));
}
fn obj(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    serde_json::json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}
