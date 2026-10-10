//! Lane H: full schema through progressive disclosure only; no inline summary.
use serde_json::{json, Value};
pub(crate) fn register(schema: &mut Value) {
    let integer = json!({"type":"integer","minimum":0});
    let number = json!({"type":"number"});
    let binding = json!({"oneOf":[{"type":"null"},{"type":"object","additionalProperties":false,"required":["Area"],"properties":{"Area":{"type":"object","additionalProperties":false,"required":["path","inset"],"properties":{"path":integer,"inset":number}}}},{"type":"object","additionalProperties":false,"required":["Path"],"properties":{"Path":{"type":"object","additionalProperties":false,"required":["path","start","end","offset","flip","effect"],"properties":{"path":integer,"start":number,"end":number,"offset":number,"flip":{"type":"boolean"},"effect":{"enum":["Rainbow","Skew"]}}}}}]});
    let text_box = crate::text::schema();
    let variants = [
        ("bind", json!({"text":integer,"binding":binding}), vec!["text", "binding"]),
        ("thread", json!({"from":integer,"to":{"type":["integer","null"]}}), vec!["from", "to"]),
        (
            "define_character",
            json!({"name":{"type":"string"},"definition":{"type":"object","additionalProperties":false,"required":["parent","style"],"properties":{"parent":{"type":["string","null"]},"style":{"oneOf":[{"type":"null"},text_box["properties"]["runs"]["items"]["properties"]["style"].clone()]}}}}),
            vec!["name", "definition"],
        ),
        (
            "define_paragraph",
            json!({"name":{"type":"string"},"definition":{"type":"object","additionalProperties":false,"required":["parent","style"],"properties":{"parent":{"type":["string","null"]},"style":{"oneOf":[{"type":"null"},text_box["properties"]["para"].clone()]}}}}),
            vec!["name", "definition"],
        ),
        (
            "apply_character",
            json!({"text":integer,"start":integer,"end":integer,"name":{"type":"string"}}),
            vec!["text", "start", "end", "name"],
        ),
        ("apply_paragraph", json!({"text":integer,"name":{"type":"string"}}), vec!["text", "name"]),
        (
            "features",
            json!({"text":integer,"features":{"type":"object","maxProperties":64,"propertyNames":{"pattern":"^[A-Za-z0-9]{4}$"},"additionalProperties":{"type":"integer","minimum":0,"maximum":65535}}}),
            vec!["text", "features"],
        ),
    ];
    let mut actions = Vec::new();
    for (name, mut props, mut required) in variants {
        props["action"] = json!({"const":name});
        required.push("action");
        actions.push(json!({"type":"object","additionalProperties":false,"properties":props,"required":required}));
    }
    schema["$defs"]["typography"] = json!({"type":"object","additionalProperties":false,"required":["verb","command"],"properties":{"verb":{"const":"typography"},"command":{"oneOf":actions}}});
    if let Some(ops) = schema["$defs"]["operation"]["anyOf"].as_array_mut() {
        ops.push(json!({"$ref":"#/$defs/typography"}));
    }
}
