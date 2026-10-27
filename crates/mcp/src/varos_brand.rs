//! Varos: the product name in what this MCP server tells agents (FORK.md rule 4: the product is
//! Varos; crate names, tool ids, resource URIs and persisted identifiers stay upstream's).
//!
//! The server's text is upstream's, written with "VectorCraft"; it is branded here, in one place,
//! where it leaves the server: the `initialize` instructions and title, `tools/list`,
//! `prompts/list` and the prompts' text. Tool ids and `vectorcraft://` URIs are lowercase and so
//! never touched.

use serde_json::Value;

/// The product name agents see.
pub(crate) const PRODUCT: &str = "Varos";

/// `s` with the product name.
pub(crate) fn text(s: &str) -> String {
    s.replace("VectorCraft", PRODUCT)
}

/// Every string of `v` with the product name (descriptions and titles of a list result).
pub(crate) fn value(v: Value) -> Value {
    match v {
        Value::String(s) if s.contains("VectorCraft") => Value::String(text(&s)),
        Value::Array(items) => Value::Array(items.into_iter().map(value).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, value(v))).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::server::stateless_request;

    /// What agents read names Varos: the instructions, the title, tool and prompt descriptions,
    /// and the prompts' text; tool ids and resource URIs keep upstream's names.
    #[test]
    fn agents_read_the_brand_name() {
        let tools = stateless_request("tools/list", &json!({})).unwrap().unwrap();
        let prompts = stateless_request("prompts/list", &json!({})).unwrap().unwrap();
        for v in [&tools, &prompts] {
            let s = v.to_string();
            assert!(!s.contains("VectorCraft"), "{s}");
        }
        assert!(tools.to_string().contains("Varos"));
        assert!(tools["tools"].as_array().unwrap().iter().any(|t| t["name"] == "command_run"), "tool ids unchanged");
        let name = prompts["prompts"][0]["name"].as_str().unwrap();
        let mut args = serde_json::Map::new();
        for a in prompts["prompts"][0]["arguments"].as_array().unwrap() {
            args.insert(a["name"].as_str().unwrap().into(), json!("x"));
        }
        let got = stateless_request("prompts/get", &json!({"name": name, "arguments": args})).unwrap().unwrap();
        assert!(!got.to_string().contains("VectorCraft"), "{got}");
        // `initialize`: the title and the instructions.
        let mut s = crate::Server::new(Box::new(crate::Headless::with_document()));
        let line = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}).to_string();
        let reply: serde_json::Value = serde_json::from_str(&s.handle_line(&line).unwrap()).unwrap();
        let r = &reply["result"];
        assert_eq!(r["serverInfo"]["title"], vectorcraft_format::VAROS_IDENTITY);
        assert_eq!(r["serverInfo"]["version"], vectorcraft_format::VAROS_IDENTITY);
        assert_eq!(r["serverInfo"]["name"], "varos", "the server identifies the fork");
        let instructions = r["instructions"].as_str().unwrap();
        assert!(instructions.starts_with("Varos is a professional vector illustration app."), "{instructions}");
        assert!(!instructions.contains("VectorCraft") && instructions.contains("vectorcraft://object/{id}"), "{instructions}");
    }
}
