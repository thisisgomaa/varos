//! Real CLI roundtrip, including the API guard, without a desktop host.
use std::process::Command;
use varos_core::{
    live,
    model::{Anchor, NodeKind, Path},
    Editor,
};
#[test]
fn apply_live_make_options_expand_and_legacy_refusal() {
    let root = std::env::temp_dir().join(format!("lane-e-live-cli-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut ed = Editor::new();
    let pid = ed.doc.nid();
    let anchors = [[0., 0.], [10., 0.], [10., 10.], [0., 10.]]
        .into_iter()
        .map(|p| Anchor { id: ed.doc.nid(), p, hin: None, hout: None, smooth: false })
        .collect();
    ed.doc.paths.push(Path::new(pid, anchors, true, Some([1., 0., 0., 1.]), None, 0.));
    ed.doc.sync_tree();
    let input = root.join("input.vrs");
    let batch = root.join("batch.json");
    let output = root.join("live.vrs");
    std::fs::write(&input, varos_core::format::encode_model(&ed.doc, &Default::default()).unwrap()).unwrap();
    let action = serde_json::json!({"Live":{"action":"make","paths":[pid],"kind":{"effect":"repeat","repeat":{"mode":"grid","rows":2,"cols":3,"gap":[5,5]}}}});
    let run = |out: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_varos-cli"))
            .arg("apply")
            .arg(&input)
            .arg("--batch")
            .arg(&batch)
            .arg("--out")
            .arg(out)
            .output()
            .unwrap()
    };
    std::fs::write(&batch, serde_json::json!({"api":"1.1","commands":[action]}).to_string()).unwrap();
    assert!(!run(&output).status.success());
    assert!(!output.exists());
    std::fs::write(&batch, serde_json::json!({"api":"1.2","commands":[action]}).to_string()).unwrap();
    let result = run(&output);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
    let doc = varos_pdf::load_vrs(&output).unwrap();
    assert!(live::has_live(&doc));
    let node = doc.nodes.iter().find(|n| matches!(n.kind, NodeKind::Live(_))).unwrap().id;
    assert_eq!(live::evaluated_document(&doc).unwrap().unwrap().paths.len(), 6);
    std::fs::write(&input, std::fs::read(&output).unwrap()).unwrap();
    std::fs::write(
        &batch,
        serde_json::json!({"api":"1.2","commands":[
 {"Live":{"action":"options","node":node,"kind":{"effect":"repeat","repeat":{"mode":"mirror","axis":"vertical"}}}},
 {"Live":{"action":"expand","node":node}}]})
        .to_string(),
    )
    .unwrap();
    let result = run(&root.join("expanded.vrs"));
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
    let baked = varos_pdf::load_vrs(&root.join("expanded.vrs")).unwrap();
    assert!(!live::has_live(&baked));
    assert_eq!(baked.paths.len(), 2);
    std::fs::remove_dir_all(root).unwrap();
}
