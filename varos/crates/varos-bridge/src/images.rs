//! API 1.2-only bytes-free image discovery and descriptions. Legacy tools/list remains frozen.
use serde_json::{json,Value};
pub fn append_tools(table:&mut Value) {
    if let Some(rows)=table["tools"].as_array_mut(){rows.push(json!({"name":"add_image","description":"Place a bounded local image asynchronously; embed by default. Poll the returned file ticket.","inputSchema":{"type":"object","additionalProperties":false,"properties":{"api":{"const":"1.2"},"board":{"type":"string"},"request_id":{"type":"string"},"expected_rev":{"type":"integer"},"path":{"type":"string","minLength":1,"maxLength":4096},"options":{"type":"object","additionalProperties":false,"properties":{"mode":{"enum":["Embed","Link"],"default":"Embed"},"at":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}},"bounds":{"type":"array","minItems":4,"maxItems":4,"items":{"type":"number"}}}}},"required":["api","board","request_id","expected_rev","path"]}}));}
}
pub fn describe(ed:&varos_core::Editor)->Value {
    json!({"rev":ed.rev,"images":ed.doc.images.iter().map(|i|json!({"id":format!("image:{}",i.id),"key":i.blob,"dimensions":[i.px_w,i.px_h],"source_ppi":i.ppi,"effective_ppi":i.effective_ppi(),"transform":i.xform,"opacity":i.opacity,"placement":i.placement,"link":i.link,"proxy_available":ed.blobs.get(&i.blob).is_some(),"full_available":ed.blobs.get(&i.blob).is_some_and(|b|b.original.is_some())})).collect::<Vec<_>>()})
}
