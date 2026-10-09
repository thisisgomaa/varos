//! Lane C: API 1.2 progressive-disclosure colour-management verb.
use crate::{
    dto::{Error, Operation},
    service::resolve,
};
use std::collections::BTreeSet;
use varos_core::{colour_management_commands::Command, EditCommand, Editor};
pub(crate) fn apply(
    ed: &mut Editor,
    op: &Operation,
    command: &Command,
    affected: &mut BTreeSet<String>,
) -> Result<Option<u32>, Error> {
    let fail = |s| Error::new("invalid_argument", s);
    let ids = if op.ids().is_empty() { vec![] } else { resolve(&ed.doc, op.ids(), false)? };
    if matches!(command, Command::Paint { .. }) {
        if ids.is_empty() {
            return Err(fail("colour paint requires targets".into()));
        }
        ed.try_execute(EditCommand::SelectPaths(ids.clone())).map_err(fail)?;
    } else if !ids.is_empty() {
        return Err(fail("document colour settings omit targets".into()));
    }
    ed.try_execute(EditCommand::ColourManagement(command.clone())).map_err(fail)?;
    affected.extend(ids.iter().map(|id| format!("path:{id}")));
    Ok(None)
}
pub(crate) fn schemas(defs: &mut serde_json::Map<String, serde_json::Value>, ops: &mut Vec<serde_json::Value>) {
    use serde_json::json;
    let unit = json!({"type":"number","minimum":0,"maximum":1});
    let obj = |properties: serde_json::Value, required: &[&str]| json!({"type":"object","additionalProperties":false,"properties":properties,"required":required});
    let alt = obj(json!({"c":unit,"m":unit,"y":unit,"k":unit}), &["c", "m", "y", "k"]);
    let colour = json!({"oneOf":[obj(json!({"model":{"const":"rgb"},"r":unit,"g":unit,"b":unit}),&["model","r","g","b"]),obj(json!({"model":{"const":"cmyk"},"c":unit,"m":unit,"y":unit,"k":unit}),&["model","c","m","y","k"]),obj(json!({"model":{"const":"gray"},"value":unit}),&["model","value"]),obj(json!({"model":{"const":"spot"},"name":{"type":"string","minLength":1,"maxLength":256},"tint":unit,"alt":alt}),&["model","name","tint","alt"])]});
    let profile = json!({"anyOf":[{"type":"null"},obj(json!({"name":{"type":"string","minLength":1,"maxLength":256},"data":{"type":"string","minLength":256,"maxLength":8388608,"pattern":"^([0-9a-fA-F]{2})+$"}}),&["name","data"])]});
    let command = json!({"oneOf":[obj(json!({"action":{"enum":["proof","overprint"]},"enabled":{"type":"boolean"}}),&["action","enabled"]),obj(json!({"action":{"const":"mode"},"mode":{"enum":["Rgb","Cmyk"]}}),&["action","mode"]),obj(json!({"action":{"const":"profile"},"profile":profile}),&["action","profile"]),obj(json!({"action":{"const":"paint"},"target":{"enum":["Fill","Stroke"]},"colour":obj(json!({"colour":colour,"alpha":unit}),&["colour","alpha"])}),&["action","target","colour"])]});
    let managed = obj(
        json!({"type":{"const":"managed"},"value":obj(json!({"colour":colour,"alpha":unit}),&["colour","alpha"])}),
        &["type", "value"],
    );
    fn extend(value: &mut serde_json::Value, managed: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(paint) = map.get_mut("paint") {
                    if let Some(any) = paint.get_mut("anyOf").and_then(serde_json::Value::as_array_mut) {
                        any.push(managed.clone());
                    }
                }
                for child in map.values_mut() {
                    extend(child, managed);
                }
            }
            serde_json::Value::Array(a) => {
                for child in a {
                    extend(child, managed);
                }
            }
            _ => {}
        }
    }
    if let Some(s) = defs.get_mut("colour") {
        extend(s, &managed);
    }
    defs.insert("colour_management".into(),obj(json!({"verb":{"const":"colour_management"},"ids":{"type":"array","maxItems":1000,"items":{"type":"string","pattern":"^(path|node):[1-9][0-9]*$"}},"command":command}),&["verb","ids","command"]));
    ops.push(json!({"$ref":"#/$defs/colour_management"}));
}
