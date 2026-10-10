use std::path::Path;

fn unchecked_calls(source: &str) -> usize {
    // Normalize whitespace so chained calls split across lines count too.
    source.chars().filter(|c| !c.is_whitespace()).collect::<String>().matches(".unwrap()").count()
}

#[test]
fn bridge_no_panic_ratchet() {
    fn visit(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .expect("read Bridge source directory")
            .map(|entry| {
                let path = entry.expect("read Bridge source entry").path();
                if path.is_dir() {
                    visit(&path)
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let source = std::fs::read_to_string(path).expect("read Bridge source");
                    // Inline test modules are at the end of these production files.
                    let source = source.split("\n#[cfg(test)]\nmod ").next().unwrap_or("");
                    let source = source.split("\n#[cfg(all(test, unix))]\nmod ").next().unwrap_or("");
                    unchecked_calls(source)
                } else {
                    0
                }
            })
            .sum()
    }
    let count = visit(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    // Existing unchecked calls may only decrease; includes MCP schema generation.
    assert!(count <= 93, "Bridge production unwrap count grew to {count}");
}

#[test]
fn bridge_ratchet_counts_schema_compaction_and_expansion_calls() {
    let former_calls = r#"
        schema["$defs"].as_object_mut().unwrap();
        definition.as_object_mut().unwrap();
        m.remove("allOf").unwrap().as_array().unwrap().clone();
        value.as_object().unwrap().clone();
        expanded.as_object_mut()
            .unwrap()
            .entry("allOf");
        constraints.as_array_mut().unwrap().push(target);
    "#;
    assert_eq!(unchecked_calls(former_calls), 7);
}

// Frozen from the verbatim main-now (73f3221) builder, using serde_json::to_vec.
// These are independent checked-in bytes; never regenerate from the live builder.
fn assert_legacy_fixture(api: &str, fixture: &[u8]) {
    let bytes = serde_json::to_vec(&varos_bridge::mcp::tools_for_api(api)).expect("serialize tools/list");
    println!("API {api} tools/list: {} B", bytes.len());
    assert_eq!(bytes.as_slice(), fixture, "legacy tools/list bytes changed for API {api}");
    assert_eq!(serde_json::to_vec(&varos_bridge::mcp::tools()).expect("serialize default tools/list"), fixture);
}

#[test]
fn tools_list_1_0_matches_frozen_main_fixture() {
    assert_legacy_fixture("1.0", include_bytes!("fixtures/mcp_tools_list_1_0.json"));
}

#[test]
fn tools_list_1_1_matches_frozen_main_fixture() {
    assert_legacy_fixture("1.1", include_bytes!("fixtures/mcp_tools_list_1_1.json"));
}

#[test]
fn tools_list_size_ratchet_all_api_versions() {
    let bytes = serde_json::to_vec(&varos_bridge::mcp::tools_for_api("1.2")).expect("serialize tools/list").len();
    println!("API 1.2 tools/list: {bytes} B");
    assert!(bytes <= 24_000, "API 1.2 tools/list grew to {bytes} B");
}

#[test]
fn align_key_object_is_advertised_in_every_api() {
    for api in ["1.0", "1.1"] {
        let list = varos_bridge::mcp::tools_for_api(api);
        let edit =
            list["tools"].as_array().expect("tools array").iter().find(|row| row["name"] == "edit").expect("edit tool");
        let pattern =
            edit["inputSchema"]["$defs"]["align"]["properties"]["target"]["pattern"].as_str().expect("target pattern");
        assert!(pattern.contains("|key_object|"), "API {api}: {pattern}");
    }
    let align = varos_bridge::mcp::schema("edit", Some("align")).expect("API 1.2 align schema");
    let pattern = align["properties"]["target"]["pattern"].as_str().expect("target pattern");
    assert!(pattern.contains("|key_object|"), "API 1.2: {pattern}");
}

/// Integration w3: the combined wave-3 verbs must still fit ONE list_verbs reply page (16 KiB text
/// budget, `MAX_TEXT`); edit rows inherit `enabled`/`disabled_reason` from their group.
#[test]
fn list_verbs_fits_one_reply_page() {
    let bytes = serde_json::to_vec(&varos_bridge::mcp::list_verbs()).expect("serialize list_verbs").len();
    println!("API 1.2 list_verbs: {bytes} B");
    assert!(bytes + 512 <= varos_bridge::MAX_TEXT, "list_verbs grew to {bytes} B (16 KiB page incl. envelope)");
}
