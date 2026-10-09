//! API 1.2 tools; old DTOs and capability replies remain frozen.
use crate::{
    design::canonical,
    dto::{Error, Operation},
    service::resolve,
};
use std::collections::BTreeSet;
use varos_core::{EditCommand, Editor};
pub(crate) fn apply(ed: &mut Editor, op: &Operation, affected: &mut BTreeSet<String>) -> Result<Option<u32>, Error> {
    let fail = |s| Error::new("invalid_argument", s);
    let ids = op.ids();
    match op {
        // ---- w2-gradients ----
        Operation::Colour { command, .. } => return crate::colour::apply(ed, op, command, affected),
        Operation::ToolOptions { wand, eyedropper } => {
            if let Some(o) = wand {
                ed.try_execute(EditCommand::SetWandOptions(*o)).map_err(fail)?;
            }
            if let Some(o) = eyedropper {
                ed.try_execute(EditCommand::SetEyedropperOptions(*o)).map_err(fail)?;
            }
        }
        Operation::Isolation { exit, .. } => {
            let node = if *exit {
                None
            } else {
                if ids.len() != 1 {
                    return Err(fail("isolation requires one group node".into()));
                }
                let (kind, n) = canonical(&ids[0])?;
                if kind != "node" {
                    return Err(fail("isolation requires a node id".into()));
                }
                Some(n)
            };
            ed.try_execute(EditCommand::Isolate(node)).map_err(fail)?;
        }
        Operation::Layers { action, .. } => {
            let nodes = ids
                .iter()
                .map(|id| {
                    let (kind, n) = canonical(id)?;
                    if kind != "node" {
                        return Err(fail("layers requires node ids".into()));
                    }
                    Ok(n)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            ed.try_execute(EditCommand::LayerFamily { action: *action, nodes }).map_err(fail)?;
        }
        _ => {
            let paths = resolve(&ed.doc, ids, false)?;
            if !matches!(op, Operation::MagicWand { .. }) {
                ed.try_execute(EditCommand::SelectPaths(paths.clone())).map_err(fail)?;
            }
            let command = match op {
                Operation::Transform { spec, .. } => EditCommand::Transform(*spec),
                Operation::MagicWand { options, mode, .. } => {
                    if paths.len() != 1 {
                        return Err(fail("wand requires one source path".into()));
                    }
                    EditCommand::MagicWand { source: paths[0], options: *options, mode: *mode }
                }
                Operation::Eyedropper { source, options, colour_only, .. } => {
                    let source = resolve(&ed.doc, std::slice::from_ref(source), false)?;
                    if source.len() != 1 {
                        return Err(fail("eyedropper requires one source path".into()));
                    }
                    EditCommand::Eyedropper { source: source[0], options: *options, colour_only: *colour_only }
                }
                _ => return Err(fail("unknown slice 4A operation".into())),
            };
            ed.try_execute(command).map_err(fail)?;
            affected.extend(paths.iter().map(|p| format!("path:{p}")));
        }
    }
    Ok(None)
}

pub(crate) fn schemas(
    definitions: &mut serde_json::Map<String, serde_json::Value>,
    all_ops: &mut Vec<serde_json::Value>,
) {
    use serde_json::json;
    let ids = json!({"type":"array","maxItems":1000,"items":{"type":"string","pattern":"^(path|node):[1-9][0-9]*$"}});
    let pick = json!({"type":"object","additionalProperties":false,"properties":{"fill":{"type":"boolean"},"stroke":{"type":"boolean"},"weight":{"type":"boolean"},"opacity":{"type":"boolean"}}});
    let spec = json!({"type":"object","additionalProperties":false,"properties":{"scale":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}},"movement":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}},"angle":{"type":"number"},"reflect":{"type":["number","null"]},"shear":{"type":"number","exclusiveMinimum":-89.9,"exclusiveMaximum":89.9},"shear_axis":{"type":"number"},"origin":{"type":["array","null"],"minItems":2,"maxItems":2,"items":{"type":"number"}},"each":{"type":"boolean"},"random":{"type":"boolean"},"seed":{"type":"integer","minimum":0,"maximum":4294967295u64},"copy":{"type":"boolean"}}});
    for (verb, fields, required) in [
        (
            "tool_options",
            json!({"wand":{"type":"object","additionalProperties":false,"properties":{"pick":pick,"colour":{"type":"number","minimum":0},"weight":{"type":"number","minimum":0},"opacity":{"type":"number","minimum":0}}},"eyedropper":pick}),
            vec![],
        ),
        ("transform", json!({"ids":ids,"spec":spec}), vec!["ids", "spec"]),
        (
            "magic_wand",
            json!({"ids":ids,"options":{"type":"object","additionalProperties":false,"properties":{"pick":pick,"colour":{"type":"number","minimum":0},"weight":{"type":"number","minimum":0},"opacity":{"type":"number","minimum":0}}},"mode":{"enum":["set","add","subtract"]}}),
            vec!["ids", "options", "mode"],
        ),
        (
            "eyedropper",
            json!({"ids":ids,"source":{"type":"string","pattern":"^path:[1-9][0-9]*$"},"options":pick,"colour_only":{"type":"boolean"}}),
            vec!["ids", "source", "options", "colour_only"],
        ),
        ("isolation", json!({"ids":ids,"exit":{"type":"boolean"}}), vec!["ids", "exit"]),
        (
            "layers",
            json!({"ids":ids,"action":{"enum":["release_sequence","release_build","collect","merge","flatten","locate","hide_others","lock_others"]}}),
            vec!["ids", "action"],
        ),
    ] {
        let mut properties = fields;
        properties["verb"] = json!({"const":verb});
        let mut required = required;
        required.push("verb");
        definitions.insert(verb.into(),json!({"type":"object","additionalProperties":false,"properties":properties,"required":required,"description":"Requires API 1.2"}));
        all_ops.push(json!({"$ref":format!("#/$defs/{verb}")}));
    }
    compact_schemas(definitions);
}

// Intern repeated property schemas within the edit root; validation stays identical.
fn compact_schemas(definitions: &mut serde_json::Map<String, serde_json::Value>) {
    use serde_json::{json, Value};
    fn count(v: &Value, counts: &mut std::collections::BTreeMap<String, (usize, Value)>) {
        if let Value::Object(o) = v {
            if o.contains_key("type")
                && !o.get("properties").is_some_and(|p| p.get("verb").is_some())
                && !o.contains_key("prefixItems")
            {
                let key = v.to_string();
                if key.len() > 55 {
                    let entry = counts.entry(key).or_insert((0, v.clone()));
                    entry.0 += 1;
                }
            }
            for child in o.values() {
                count(child, counts);
            }
        } else if let Value::Array(a) = v {
            for child in a {
                count(child, counts);
            }
        }
    }
    fn replace(v: &mut Value, refs: &std::collections::BTreeMap<String, String>) {
        if let Some(name) = refs.get(&v.to_string()) {
            *v = json!({"$ref":format!("#/$defs/{name}")});
            return;
        }
        match v {
            Value::Object(o) => {
                for child in o.values_mut() {
                    replace(child, refs);
                }
            }
            Value::Array(a) => {
                for child in a {
                    replace(child, refs);
                }
            }
            _ => {}
        }
    }
    let mut counts = std::collections::BTreeMap::new();
    for v in definitions.values() {
        count(v, &mut counts);
    }
    let mut refs = std::collections::BTreeMap::new();
    let mut shared = Vec::new();
    for (key, (count, value)) in counts {
        if count > 1 {
            let name = format!("field{}", shared.len());
            refs.insert(key, name.clone());
            shared.push((name, value));
        }
    }
    for v in definitions.values_mut() {
        replace(v, &refs);
    }
    definitions.extend(shared);
}
