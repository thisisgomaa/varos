//! Lane G: API 1.2 text verbs. Source/styles are core data, never layout backend types.
use crate::dto::{Error, Operation};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use varos_core::{editor::Editor, model::NodeKind, EditCommand};
pub(crate) fn apply(
    ed: &mut Editor,
    op: &Operation,
    locals: &mut BTreeMap<String, String>,
    affected: &mut BTreeSet<String>,
) -> Result<bool, Error> {
    let fail = |s| Error::new("invalid_argument", s);
    match op {
        Operation::Typography { command } => {
            ed.try_execute(EditCommand::Typography(command.clone())).map_err(fail)?;
            affected.extend(
                ed.doc
                    .text_boxes
                    .iter()
                    .filter_map(|t| varos_core::text::node_id(&ed.doc, t.id))
                    .map(|id| format!("node:{id}")),
            );
            Ok(true)
        }
        Operation::AddText { text, parent, local } => {
            if let Some(name) = local {
                crate::design::local_name(name)?;
                if locals.contains_key(name) {
                    return Err(fail("duplicate local".into()));
                }
            }
            let parent = parent
                .as_ref()
                .map(|s| {
                    let (kind, id) = crate::design::canonical(s)?;
                    if kind != "node" {
                        return Err(fail("parent must be node:N".into()));
                    }
                    Ok(id)
                })
                .transpose()?;
            let id = ed.try_execute_created(EditCommand::AddText { text: text.clone(), parent }).map_err(fail)?;
            let node = ed
                .doc
                .nodes
                .iter()
                .find(|n| n.kind == NodeKind::Text(id))
                .ok_or_else(|| fail("missing text node".into()))?;
            let identity = format!("node:{}", node.id);
            affected.insert(identity.clone());
            if let Some(name) = local {
                locals.insert(name.clone(), identity);
            }
            Ok(true)
        }
        Operation::SetText { node, text } => {
            let value = locals.get(node).unwrap_or(node);
            let (kind, id) = crate::design::canonical(value)?;
            if kind != "node" {
                return Err(fail("text target must be node:N".into()));
            }
            let NodeKind::Text(id) = ed.doc.node(id).ok_or_else(|| fail("missing text node".into()))?.kind else {
                return Err(fail("target is not text".into()));
            };
            ed.try_execute(EditCommand::SetText { id, text: text.clone() }).map_err(fail)?;
            affected.insert(value.clone());
            Ok(true)
        }
        _ => Ok(false),
    }
}
pub(crate) fn schema() -> Value {
    let number = json!({"type":"number"});
    let pair = json!({"type":"array","items":number,"minItems":2,"maxItems":2});
    let style = json!({"type":"object","additionalProperties":false,"required":["font","size","letter_spacing","fill"],"properties":{"font":{"type":"object","additionalProperties":false,"required":["family","weight","hash"],"properties":{"family":{"type":"string"},"weight":{"type":"integer"},"hash":{"type":"string","pattern":"^[0-9a-f]{64}$"}}},"size":number,"letter_spacing":number,"fill":{"type":"array","items":number,"minItems":4,"maxItems":4}}});
    json!({"type":"object","additionalProperties":false,"required":["id","box_kind","frame","runs","para"],"properties":{"id":{"type":"integer","minimum":0},"box_kind":{"oneOf":[{"const":"Point"},{"type":"object","additionalProperties":false,"required":["Area"],"properties":{"Area":{"type":"array","items":number,"minItems":4,"maxItems":4}}}]},"frame":pair,"runs":{"type":"array","minItems":1,"maxItems":4096,"items":{"type":"object","additionalProperties":false,"required":["text","style"],"properties":{"text":{"type":"string"},"style":style}}},"para":{"type":"object","additionalProperties":false,"required":["align","kashida","direction","line_height"],"properties":{"align":{"enum":["Left","Centre","Right","Justify"]},"kashida":{"enum":["Off","Minimal","Balanced","Display"]},"direction":{"enum":["Auto","Ltr","Rtl"]},"line_height":number}}}})
}
pub(crate) fn register(schema: &mut Value) {
    crate::typography::register(schema);
    schema["$defs"]["text_box"] = self::schema();
    schema["$defs"]["add_text"] = json!({"type":"object","additionalProperties":false,"required":["verb","text"],"properties":{"verb":{"const":"add_text"},"text":{"$ref":"#/$defs/text_box"},"parent":{"type":"string"},"local":{"type":"string"}}});
    schema["$defs"]["set_text"] = json!({"type":"object","additionalProperties":false,"required":["verb","node","text"],"properties":{"verb":{"const":"set_text"},"node":{"type":"string"},"text":{"$ref":"#/$defs/text_box"}}});
    if let Some(ops) = schema["$defs"]["operation"]["anyOf"].as_array_mut() {
        for name in ["add_text", "set_text"] {
            ops.push(json!({"$ref":format!("#/$defs/{name}")}));
        }
    }
}
