//! Phase 10 full schemas are published only by API 1.2 schema/list_verbs.
use serde_json::{json, Map, Value};
pub(crate) fn schemas(defs: &mut Map<String, Value>, ops: &mut Vec<Value>) {
    let number = |lo: f64, hi: f64| json!({"type":"number","minimum":lo,"maximum":hi});
    let pair = |lo: f64, hi: f64| json!({"type":"array","minItems":2,"maxItems":2,"items":number(lo,hi)});
    let obj = |kind: &str, props: Value, required: Vec<&str>| {
        let mut p = props.as_object().cloned().unwrap_or_default();
        p.insert("type".into(), json!({"const":kind}));
        let mut r = required;
        r.push("type");
        json!({"type":"object","additionalProperties":false,"properties":p,"required":r})
    };
    let effect = json!({"oneOf":[
    obj("offset",json!({"delta":number(-7200.,7200.),"join":{"enum":["Round","Miter","Bevel"]},"miter":number(1.,1000.)}),vec!["delta","join","miter"]),
    obj("zig_zag",json!({"size":number(-10000.,10000.),"ridges":{"type":"integer","minimum":0,"maximum":100},"smooth":{"type":"boolean"}}),vec!["size","ridges","smooth"]),
    obj("transform",json!({"copies":{"type":"integer","minimum":0,"maximum":1000},"move":pair(-1e6,1e6),"scale":pair(-10.,10.),"rotate":number(-36000.,36000.),"reflect":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"boolean"}}}),vec!["copies","move","scale","rotate","reflect"]),
    obj("warp",json!({"style":{"enum":["arc","arc_lower","arc_upper","arch","bulge","shell_lower","shell_upper","flag","wave","fish","rise","fisheye","inflate","squeeze","twist"]},"bend":number(-100.,100.),"h":number(-100.,100.),"v":number(-100.,100.)}),vec!["style","bend","h","v"])
    ]});
    let ids = json!({"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string","pattern":"^(path|node):[1-9][0-9]*$"}});
    let point = json!({"type":"array","minItems":3,"maxItems":3,"prefixItems":[number(0.,1.),number(0.,100.),number(0.,100.)],"items":false});
    let profile = json!({"oneOf":[{"type":"null"},{"type":"object","additionalProperties":false,"required":["points"],"properties":{"points":{"type":"array","minItems":2,"maxItems":256,"items":point}}}]});
    for (verb, fields, required) in [
        ("width_tool", json!({}), vec!["verb"]),
        (
            "live_effects",
            json!({"ids":ids,"effects":{"type":"array","maxItems":16,"items":effect}}),
            vec!["verb", "ids", "effects"],
        ),
        ("width_profile", json!({"ids":ids,"profile":profile}), vec!["verb", "ids", "profile"]),
        ("expand_live", json!({"ids":ids}), vec!["verb", "ids"]),
    ] {
        let mut properties = fields.as_object().cloned().unwrap_or_default();
        properties.insert("verb".into(), json!({"const":verb}));
        defs.insert(
            verb.into(),
            json!({"type":"object","additionalProperties":false,"properties":properties,"required":required}),
        );
        ops.push(json!({"$ref":format!("#/$defs/{verb}")}));
    }
}
