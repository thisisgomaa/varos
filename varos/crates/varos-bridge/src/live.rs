//! API 1.2 live verbs. Full schemas are progressive-disclosure only.
use crate::dto::{Error, Operation};
use std::collections::{BTreeMap, BTreeSet};
use varos_core::{live::Action, EditCommand, Editor};
pub(crate) const VERBS: &[&str] =
    &["live_make", "live_options", "live_release", "live_expand", "live_isolate", "live_spine"];
fn id(value: &str, prefix: &str, locals: &BTreeMap<String, String>) -> Result<u32, Error> {
    let value = if value.starts_with('$') {
        locals.get(value).map(String::as_str).ok_or_else(|| Error::new("invalid_argument", "Unknown local id"))?
    } else {
        value
    };
    value
        .strip_prefix(prefix)
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| Error::new("invalid_argument", format!("Expected {prefix}N")))
}
pub(crate) fn apply(
    ed: &mut Editor,
    op: &Operation,
    locals: &BTreeMap<String, String>,
    affected: &mut BTreeSet<String>,
    expanded: &mut usize,
) -> Result<Option<u32>, Error> {
    let node = |s: &str| id(s, "node:", locals);
    let path = |s: &str| id(s, "path:", locals);
    let action = match op {
        Operation::LiveMake { ids, kind } => {
            let paths = ids.iter().map(|s| path(s)).collect::<Result<Vec<_>, _>>()?;
            *expanded += paths.len();
            Action::Make { paths, kind: *kind }
        }
        Operation::LiveOptions { node: n, kind } => Action::Options { node: node(n)?, kind: *kind },
        Operation::LiveRelease { node: n } => Action::Release { node: node(n)? },
        Operation::LiveExpand { node: n } => Action::Expand { node: node(n)? },
        Operation::LiveIsolate { node: n } => Action::Isolate { node: n.as_deref().map(node).transpose()? },
        Operation::LiveSpine { node: n, path: p } => Action::Spine { node: node(n)?, path: path(p)? },
        _ => return Err(Error::new("invalid_argument", "Unknown live operation")),
    };
    if *expanded > 1000 {
        return Err(Error::new("limit_exceeded", "Expanded targets exceed 1000"));
    }
    match op {
        Operation::LiveOptions { node, .. }
        | Operation::LiveRelease { node }
        | Operation::LiveExpand { node }
        | Operation::LiveSpine { node, .. } => {
            affected.insert(node.clone());
        }
        _ => {}
    }
    let before: BTreeSet<_> = ed.doc.nodes.iter().map(|n| n.id).collect();
    ed.try_execute(EditCommand::Live(action)).map_err(|e| Error::new("invalid_argument", e))?;
    affected.extend(ed.doc.nodes.iter().filter(|n| !before.contains(&n.id)).map(|n| format!("node:{}", n.id)));
    if let Operation::LiveMake { .. } = op {
        return Ok(ed
            .doc
            .nodes
            .iter()
            .find(|n| !before.contains(&n.id) && matches!(n.kind, varos_core::model::NodeKind::Live(_)))
            .map(|n| n.id));
    }
    Ok(None)
}
pub(crate) fn schemas(defs: &mut serde_json::Map<String, serde_json::Value>, ops: &mut Vec<serde_json::Value>) {
    use serde_json::json;
    let point = json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":-1000000,"maximum":1000000}});
    let count = json!({"type":"integer","minimum":1,"maximum":1000});
    let obj = |fields: serde_json::Value, required: Vec<&str>| json!({"type":"object","additionalProperties":false,"properties":fields,"required":required});
    let repeat = json!({"oneOf":[obj(json!({"mode":{"const":"radial"},"count":count,"radius":{"type":"number","minimum":0,"maximum":1000000}}),vec!["mode","count","radius"]),obj(json!({"mode":{"const":"grid"},"rows":count,"cols":count,"gap":point}),vec!["mode","rows","cols","gap"]),obj(json!({"mode":{"const":"mirror"},"axis":{"enum":["horizontal","vertical"]}}),vec!["mode","axis"])]});
    let envelope = json!({"oneOf":[obj(json!({"mode":{"const":"warp"},"preset":{"enum":["arc","flag","bulge"]},"bend":{"type":"number","minimum":-1,"maximum":1}}),vec!["mode","preset","bend"]),obj(json!({"mode":{"const":"mesh"},"points":{"type":"array","minItems":4,"maxItems":4,"items":point}}),vec!["mode","points"])]});
    let kind = json!({"oneOf":[obj(json!({"effect":{"const":"blend"},"spine":{"type":["integer","null"],"minimum":1},"steps":{"type":"integer","minimum":0,"maximum":1000},"orientation":{"enum":["page","path"]}}),vec!["effect","spine","steps","orientation"]),obj(json!({"effect":{"const":"repeat"},"repeat":repeat}),vec!["effect","repeat"]),obj(json!({"effect":{"const":"envelope"},"envelope":envelope}),vec!["effect","envelope"])]});
    let node = json!({"type":"string","pattern":"^node:[1-9][0-9]*$"});
    for (verb, fields, required) in [
        (
            "live_make",
            json!({"ids":{"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string","pattern":"^path:[1-9][0-9]*$"}},"kind":kind}),
            vec!["ids", "kind"],
        ),
        ("live_options", json!({"node":node,"kind":kind}), vec!["node", "kind"]),
        ("live_release", json!({"node":node}), vec!["node"]),
        ("live_expand", json!({"node":node}), vec!["node"]),
        ("live_isolate", json!({"node":{"type":["string","null"],"pattern":"^node:[1-9][0-9]*$"}}), vec!["node"]),
        (
            "live_spine",
            json!({"node":node,"path":{"type":"string","pattern":"^path:[1-9][0-9]*$"}}),
            vec!["node", "path"],
        ),
    ] {
        let mut fields = fields.as_object().cloned().unwrap_or_default();
        fields.insert("verb".into(), json!({"const":verb}));
        let mut required = required;
        required.push("verb");
        defs.insert(verb.into(), obj(json!(fields), required));
        ops.push(json!({"$ref":format!("#/$defs/{verb}")}));
    }
}
