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

#[test]
fn tools_list_size_ratchet_all_api_versions() {
    for api in ["1.0", "1.1", "1.2"] {
        let bytes = serde_json::to_vec(&varos_bridge::mcp::tools_for_api(api)).expect("serialize tools/list").len();
        println!("API {api} tools/list: {bytes} B");
        assert!(bytes <= 24_000, "API {api} tools/list grew to {bytes} B");
    }
}
