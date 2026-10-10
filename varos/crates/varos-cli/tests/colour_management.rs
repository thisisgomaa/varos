use std::process::Command;
#[test]
fn targeted_paint_and_document_mode_use_checked_cli_boundary() {
    let root = std::env::temp_dir().join(format!("varos-colour-cli-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v5/plain.json");
    let doc = varos_pdf::load_vrs(&source).unwrap();
    let id = doc.paths[0].id;
    for (index,command) in [serde_json::json!({"action":"mode","mode":"Cmyk"}),serde_json::json!({"ids":[id],"command":{"action":"paint","target":"Fill","colour":{"colour":{"model":"gray","value":0.3},"alpha":1}}})].into_iter().enumerate() {
        let json=root.join(format!("{index}.json"));let out=root.join(format!("{index}.vrs"));let _=std::fs::remove_file(&out);std::fs::write(&json,command.to_string()).unwrap();
        let output=Command::new(env!("CARGO_BIN_EXE_varos-cli")).arg("colour-management").arg(&source).arg(&json).arg(&out).output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stdout));let saved=varos_pdf::load_vrs(&out).unwrap();
        if index==0 {assert_eq!(saved.colour_mode,varos_core::colour_management::ColourMode::Cmyk);}else{assert!(matches!(saved.paths[0].fill,varos_core::model::Paint::Managed(_)));}
    }
    std::fs::remove_dir_all(root).unwrap();
}
