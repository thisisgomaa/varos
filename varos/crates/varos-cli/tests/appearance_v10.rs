//! Lane A: file-only appearance/mask commands, overwrite refusal and persisted v10 reload.
use std::process::Command;
#[test]
fn headless_commands_preserve_source_and_refuse_existing_destination() {
    let root = std::env::temp_dir().join(format!("varos-lane-a-cli-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("input.vrs");
    let edit = root.join("edit.json");
    let output = root.join("paint.vrs");
    let masked = root.join("mask.vrs");
    for p in [&output, &masked] {
        let _ = std::fs::remove_file(p);
    }
    let source = include_bytes!("../../varos-core/tests/fixtures/v5/plain.json");
    std::fs::write(&input, source).unwrap();
    let doc = varos_pdf::load_vrs(&input).unwrap();
    let path = doc.paths[0].id;
    std::fs::write(
        &edit,
        serde_json::to_vec(&serde_json::json!({"action":"add_fill","path":path,"paint":[0,0,1,1]})).unwrap(),
    )
    .unwrap();
    let run = |verb: &str, input: &std::path::Path, output: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_varos-cli")).arg(verb).arg(input).arg(&edit).arg(output).output().unwrap()
    };
    let result = run("appearance", &input, &output);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
    assert_eq!(std::fs::read(&input).unwrap(), source);
    let loaded = varos_pdf::load_vrs(&output).unwrap();
    assert_eq!(loaded.paths[0].stack.len(), 3);
    let bytes = std::fs::read(&output).unwrap();
    assert!(!run("appearance", &input, &output).status.success());
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    let node = loaded.node_of_path(path).unwrap();
    std::fs::write(&edit, serde_json::to_vec(&serde_json::json!({"action":"begin","node":node,"alpha":true})).unwrap())
        .unwrap();
    let result = run("mask", &output, &masked);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
    let loaded = varos_pdf::load_vrs(&masked).unwrap();
    assert!(loaded.nodes.iter().any(|n| n.role == varos_core::model::GroupRole::MaskAlpha));
    std::fs::remove_dir_all(root).unwrap();
}
