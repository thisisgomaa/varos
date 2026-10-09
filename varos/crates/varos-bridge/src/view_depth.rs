//! Lane E: API 1.2 view verb progressive-disclosure payload.
use serde_json::{json, Value};
pub fn action_schema() -> Value {
    json!({"oneOf":[{"enum":["outline","pixel_preview","snap_pixel","move_whole_pixel","trim","presentation","exit_presentation","transparency_grid"]},{"type":"object","additionalProperties":false,"minProperties":1,"maxProperties":1,"properties":{"canvas_color":{"type":"object","additionalProperties":false,"required":["rgb"],"properties":{"rgb":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"integer","minimum":0,"maximum":255}}}},"outline_node":{"type":"object","additionalProperties":false,"required":["id"],"properties":{"id":{"type":"integer","minimum":1}}},"navigator_pan":{"type":"object","additionalProperties":false,"required":["center"],"properties":{"center":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}}}},"navigator_zoom":{"type":"object","additionalProperties":false,"required":["percent"],"properties":{"percent":{"type":"number","minimum":2,"maximum":6400}}}}}]})
}

pub fn extend_schema(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.get("description").and_then(Value::as_str) == Some("API 1.2 view actions") {
                if let Some(props) = map
                    .get_mut("oneOf")
                    .and_then(Value::as_array_mut)
                    .and_then(|a| a.get_mut(1))
                    .and_then(|v| v.get_mut("properties"))
                    .and_then(Value::as_object_mut)
                {
                    props.insert("depth".into(), action_schema());
                }
                return;
            }
            for child in map.values_mut() {
                extend_schema(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                extend_schema(child);
            }
        }
        _ => {}
    }
}
