//! Lane A: opt-in typed edits; full parameters live behind schema/list_verbs.
use crate::dto::{Error, Operation};
use serde_json::{json, Value};
use varos_core::{EditCommand, Editor};
pub fn apply(ed: &mut Editor, op: &Operation) -> Result<(), Error> {
    let command = match op {
        Operation::Appearance { edit } => EditCommand::Appearance(edit.clone()),
        Operation::Mask { edit } => EditCommand::Mask(edit.clone()),
        _ => return Err(Error::new("invalid_argument", "unknown appearance verb")),
    };
    ed.try_execute(command).map_err(|e| Error::new("invalid_argument", e))
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}
pub fn register(schema: &mut Value) {
    let id = json!({"type":"integer","minimum":1,"maximum":4294967295u64});
    let index = json!({"type":"integer","minimum":0,"maximum":63});
    let unit = json!({"type":"number","minimum":0,"maximum":1});
    let paint = json!({"oneOf":[{"type":"null"},{"type":"array","minItems":4,"maxItems":4,"items":unit},{"type":"object","additionalProperties":false,"required":["type","value"],"properties":{"type":{"enum":["gradient","swatch_ref"]},"value":{"type":"object"}}}]});
    let opts = object(json!({"opacity":unit,"visible":{"type":"boolean"},"blend":{"const":"Normal"}}), &[]);
    let look = object(json!({"opacity":unit,"isolate":{"type":"boolean"}}), &[]);
    let width = json!({"type":"number","minimum":0});
    let fill = object(json!({"paint":paint,"opts":opts}), &["paint", "opts"]);
    let stroke = object(
        json!({"paint":paint,"opts":opts,"width":width,"style":{"$ref":"#/$defs/stroke_style"}}),
        &["paint", "opts", "width", "style"],
    );
    let base =
        json!({"type":"array","minItems":2,"maxItems":2,"prefixItems":[{"enum":["Fill","Stroke"]},opts],"items":false});
    let stack_item = json!({"oneOf":[object(json!({"Base":base}),&["Base"]),object(json!({"Fill":fill}),&["Fill"]),object(json!({"Stroke":stroke}),&["Stroke"])]});
    let specs = [
        (
            "appearance",
            vec![
                object(
                    json!({"action":{"const":"set_stack"},"path":id,"stack":{"type":"array","maxItems":64,"items":stack_item}}),
                    &["action", "path", "stack"],
                ),
                object(json!({"action":{"const":"add_fill"},"path":id,"paint":paint}), &["action", "path", "paint"]),
                object(
                    json!({"action":{"const":"add_stroke"},"path":id,"paint":paint,"width":width}),
                    &["action", "path", "paint", "width"],
                ),
                object(
                    json!({"action":{"const":"set_entry"},"path":id,"index":index,"paint":paint,"opacity":unit,"visible":{"type":"boolean"}}),
                    &["action", "path", "index", "paint", "opacity", "visible"],
                ),
                object(
                    json!({"action":{"const":"reorder"},"path":id,"from":index,"to":index}),
                    &["action", "path", "from", "to"],
                ),
                object(json!({"action":{"const":"delete"},"path":id,"index":index}), &["action", "path", "index"]),
                object(json!({"action":{"const":"expand"},"path":id}), &["action", "path"]),
                object(
                    json!({"action":{"const":"set_look"},"node":id,"look":{"oneOf":[look,{"type":"null"}]}}),
                    &["action", "node", "look"],
                ),
            ],
        ),
        (
            "mask",
            vec![
                object(
                    json!({"action":{"const":"add"},"node":id,"mask":id,"alpha":{"type":"boolean"}}),
                    &["action", "node", "mask", "alpha"],
                ),
                object(
                    json!({"action":{"const":"begin"},"node":id,"alpha":{"type":"boolean"}}),
                    &["action", "node", "alpha"],
                ),
                object(
                    json!({"action":{"const":"mode"},"node":id,"alpha":{"type":"boolean"}}),
                    &["action", "node", "alpha"],
                ),
                object(json!({"action":{"const":"release"},"node":id}), &["action", "node"]),
            ],
        ),
    ];
    for (verb, edits) in specs {
        schema["$defs"][verb] = object(json!({"verb":{"const":verb},"edit":{"oneOf":edits}}), &["verb", "edit"]);
        if let Some(ops) = schema["$defs"]["operation"]["anyOf"].as_array_mut() {
            ops.push(json!({"$ref":format!("#/$defs/{verb}")}));
        }
    }
}
pub fn affected(op: &Operation) -> Vec<String> {
    use varos_core::appearance_edits::{AppearanceEdit as A, MaskEdit as M};
    match op {
        Operation::Appearance { edit: A::SetLook { node, .. } } => vec![format!("node:{node}")],
        Operation::Appearance { edit } => {
            let path = match edit {
                A::SetStack { path, .. }
                | A::AddFill { path, .. }
                | A::AddStroke { path, .. }
                | A::SetEntry { path, .. }
                | A::Reorder { path, .. }
                | A::Delete { path, .. }
                | A::Expand { path, .. } => *path,
                A::SetLook { .. } => return vec![],
            };
            vec![format!("path:{path}")]
        }
        Operation::Mask { edit: M::Add { node, mask, .. } } => vec![format!("node:{node}"), format!("node:{mask}")],
        Operation::Mask { edit: M::Begin { node, .. } | M::Mode { node, .. } | M::Release { node } } => {
            vec![format!("node:{node}")]
        }
        _ => vec![],
    }
}
pub fn describe(ed: &Editor, ids: Option<&[String]>, limit: usize) -> Result<Value, Error> {
    if let Some(ids) = ids {
        for value in ids {
            let (kind, id) = crate::design::canonical(value)?;
            if (kind == "path" && ed.doc.pidx(id).is_none()) || (kind == "node" && ed.doc.node(id).is_none()) {
                return Err(Error::new("not_found", format!("unknown appearance target {value}")));
            }
        }
    }
    let paths=ed.doc.paths.iter().filter(|p|ids.is_none_or(|ids|ids.contains(&format!("path:{}",p.id)))).map(|p|json!({"id":format!("path:{}",p.id),"stack":p.appearance().stack(),"fill":p.fill,"stroke":p.stroke,"opacity":p.opacity}));
    let nodes=ed.doc.nodes.iter().filter(|n|matches!(n.kind,varos_core::model::NodeKind::Group|varos_core::model::NodeKind::Layer) && ids.is_none_or(|ids|ids.contains(&format!("node:{}",n.id)))).map(|n|json!({"id":format!("node:{}",n.id),"look":n.look,"role":n.role,"mask_child":n.mask_child.map(|id|format!("node:{id}"))}));
    let items = paths.chain(nodes).take(limit + 1).collect::<Vec<_>>();
    let more = items.len() > limit;
    let out = json!({"rev":ed.rev,"items":items.into_iter().take(limit).collect::<Vec<_>>(),"more":more});
    if out.to_string().len() > crate::MAX_FRAME - 4096 {
        return Err(Error::new("limit_exceeded", "appearance detail exceeds transport; use fewer ids"));
    }
    Ok(out)
}
