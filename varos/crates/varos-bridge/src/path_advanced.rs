//! Lane C opt-in verbs; legacy tools/list remains frozen.
use crate::{
    dto::{Error, Operation},
    service::resolve,
};
use std::collections::{BTreeMap, BTreeSet};
use varos_core::{path_advanced::Action, EditCommand, Editor};
pub(crate) fn apply(
    ed: &mut Editor,
    op: &Operation,
    locals: &BTreeMap<String, String>,
    affected: &mut BTreeSet<String>,
    expanded: &mut usize,
) -> Result<Option<u32>, Error> {
    let fail = |s| Error::new("invalid_argument", s);
    let ids = op
        .ids()
        .iter()
        .map(|id| {
            if id.starts_with('$') {
                locals.get(id).cloned().ok_or_else(|| fail("Unknown local id".into()))
            } else {
                Ok(id.clone())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let paths = resolve(
        &ed.doc,
        &ids,
        matches!(op, Operation::WidthTool { .. } | Operation::ScaleStrokes { .. } | Operation::NewDocument { .. }),
    )?;
    *expanded += paths.len();
    if *expanded > 1000 {
        return Err(fail("Expanded targets exceed 1000".into()));
    }
    if !paths.is_empty() {
        ed.try_execute(EditCommand::SelectPaths(paths.clone())).map_err(fail)?;
    }
    let command = match op {
        // ---- Lane B w3-effects ----
        Operation::WidthTool { .. } => EditCommand::LiveEffects(varos_core::effects::Action::Tool),
        Operation::LiveEffects { effects, .. } => {
            EditCommand::LiveEffects(varos_core::effects::Action::Set { ids: paths.clone(), effects: effects.clone() })
        }
        Operation::WidthProfile { profile, .. } => EditCommand::LiveEffects(varos_core::effects::Action::Width {
            ids: paths.clone(),
            profile: profile.clone(),
        }),
        Operation::ExpandLive { .. } => {
            EditCommand::LiveEffects(varos_core::effects::Action::Expand { ids: paths.clone() })
        }
        // ---- end Lane B w3-effects ----
        Operation::OutlineStroke { .. } => EditCommand::PathAdvanced(Action::Outline),
        Operation::OffsetPath { delta, join, miter, .. } => {
            EditCommand::PathAdvanced(Action::Offset { delta: *delta, join: *join, miter: *miter })
        }
        Operation::Expand { .. } => EditCommand::PathAdvanced(Action::Expand),
        Operation::ScaleStrokes { enabled } => EditCommand::SetScaleStrokes(*enabled),
        Operation::NewDocument { settings } => EditCommand::NewDocument(settings.clone()),
        Operation::LiveCorners { corners, .. } => {
            if paths.is_empty() {
                return Err(fail("Select corner paths".into()));
            }
            for id in &paths {
                ed.try_execute(EditCommand::SetCorners { path: *id, corners: corners.clone() }).map_err(fail)?;
            }
            affected.extend(paths.iter().map(|id| format!("path:{id}")));
            return Ok(None);
        }
        _ => return Err(fail("Unknown path operation".into())),
    };
    ed.try_execute(command).map_err(fail)?;
    affected.extend(paths.iter().map(|id| format!("path:{id}")));
    Ok(None)
}
pub(crate) fn schemas(defs: &mut serde_json::Map<String, serde_json::Value>, ops: &mut Vec<serde_json::Value>) {
    use serde_json::json;
    let ids = json!({"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string","pattern":"^(path|node):[1-9][0-9]*$"}});
    for (verb, fields, required) in [
        ("outline_stroke", json!({"ids":ids}), vec!["ids"]),
        (
            "offset_path",
            json!({"ids":ids,"delta":{"type":"number","minimum":-7200,"maximum":7200},"join":{"enum":["Round","Miter","Bevel"]},"miter":{"type":"number","minimum":1,"maximum":1000}}),
            vec!["ids", "delta", "join", "miter"],
        ),
        ("expand", json!({"ids":ids}), vec!["ids"]),
        (
            "live_corners",
            json!({"ids":ids,"corners":{"type":"array","maxItems":1000,"items":{"type":"object","additionalProperties":false,"required":["radius","kind"],"properties":{"radius":{"type":"number","minimum":0,"maximum":1000000},"kind":{"enum":["round","inverted","chamfer"]}}}}}),
            vec!["ids", "corners"],
        ),
        ("scale_strokes", json!({"enabled":{"type":"boolean"}}), vec!["enabled"]),
        (
            "new_document",
            json!({"settings":{"type":"object","additionalProperties":false,"properties":{"colour_mode":{"enum":["Rgb","Cmyk"]},"width":{"type":"number","exclusiveMinimum":0},"height":{"type":"number","exclusiveMinimum":0},"units":{"enum":["Px","Pt","Pica","Mm","Cm","In"]},"count":{"type":"integer","minimum":1,"maximum":100},"columns":{"type":"integer","minimum":1,"maximum":100},"spacing":{"type":"number","minimum":0},"layout":{"enum":["grid","row","column"]},"bleed":{"type":"number","minimum":0},"ppi":{"type":"number","minimum":1,"maximum":9600}}}}),
            vec!["settings"],
        ),
    ] {
        let mut properties = fields.as_object().cloned().unwrap_or_default();
        properties.insert("verb".into(), json!({"const":verb}));
        let mut required = required;
        required.push("verb");
        defs.insert(
            verb.into(),
            json!({"type":"object","additionalProperties":false,"properties":properties,"required":required}),
        );
        ops.push(json!({"$ref":format!("#/$defs/{verb}")}));
    }
}
pub(crate) fn export_schema(name: &str) -> serde_json::Value {
    use serde_json::json;
    let svg = json!({"type":"object","additionalProperties":false,"properties":{"styling":{"enum":["attributes","inline"]},"decimals":{"type":"integer","minimum":0,"maximum":8},"ids":{"type":"boolean"},"minify":{"type":"boolean"}}});
    let screens = json!({"type":"object","additionalProperties":false,"properties":{"rows":{"type":"array","minItems":1,"maxItems":32,"items":{"type":"object","additionalProperties":false,"properties":{"scale":{"type":"number","exclusiveMinimum":0,"maximum":64},"suffix":{"type":"string"},"format":{"enum":["pdf","svg","png","jpg","webp","tiff"]},"svg":svg}},},"prefix":{"type":"string"},"open_folder":{"type":"boolean"},"subfolders":{"enum":["none","scale","format"]},"pdf_single":{"type":"boolean"},"include_bleed":{"type":"boolean"},"include_colour":{"type":"boolean"},"whole_board":{"type":"boolean"},"range":{"type":"string"}}});
    if name == "export_svg" {
        json!({"type":"object","additionalProperties":false,"properties":{"svg":svg}})
    } else {
        json!({"type":"object","additionalProperties":false,"properties":{"screens":screens}})
    }
}
