use std::process::Command;
#[test]
fn new_document_and_export_screens_headless_end_to_end() {
    let root = std::env::temp_dir().join(format!("lane-c-cli-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let settings = root.join("new.json");
    let file = root.join("new.vrs");
    let _ = std::fs::remove_file(&file);
    std::fs::write(&settings, r#"{"count":3,"bleed":3}"#).unwrap();
    let out =
        Command::new(env!("CARGO_BIN_EXE_varos-cli")).arg("new-document").arg(&settings).arg(&file).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let doc = varos_pdf::load_vrs(&file).unwrap();
    assert_eq!(doc.artboards.len(), 3);
    let screens = root.join("screens.json");
    let folder = root.join("Export");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::write(&screens,r#"{"rows":[{"format":"svg","suffix":"-web","svg":{"minify":true}},{"format":"pdf"}],"prefix":"brand-","subfolders":"format","pdf_single":true}"#).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .arg("export-screens")
        .arg(&file)
        .arg(&screens)
        .arg(&folder)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(folder.join("svg/brand-Artboard 1-web.svg").exists());
    assert!(folder.join("pdf/brand-Artboards.pdf").exists());
    let out = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .arg("export-screens")
        .arg(&file)
        .arg(&screens)
        .arg(&folder)
        .output()
        .unwrap();
    assert!(!out.status.success());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_default_svg_bytes_equal_shared_host_encoding() {
    let root = std::env::temp_dir().join(format!("lane-c-cli-default-svg-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("input.vrs");
    let settings = root.join("screens.json");
    let folder = root.join("Export");
    let _ = std::fs::remove_dir_all(&folder);
    let doc = varos_core::new_document::Settings::default().document().unwrap();
    std::fs::write(&file, varos_core::format::encode_model(&doc, &varos_core::format::Limits::DEFAULT).unwrap())
        .unwrap();
    let advanced = varos_raster::screens::Advanced {
        rows: vec![varos_raster::screens::Row { format: "svg".into(), ..Default::default() }],
        ..Default::default()
    };
    std::fs::write(&settings, serde_json::to_vec(&advanced).unwrap()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .arg("export-screens")
        .arg(&file)
        .arg(&settings)
        .arg(&folder)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let assets = varos_raster::export::plan(&doc, &varos_raster::export::Scope::AllArtboards).unwrap();
    let planned = advanced.expand(&assets, &Default::default()).unwrap().remove(0);
    let expected = varos_raster::export::encode_with_svg_options(
        &planned.asset,
        &planned.options,
        &std::sync::atomic::AtomicBool::new(false),
        &planned.svg,
    )
    .unwrap();
    assert_eq!(std::fs::read(folder.join(planned.relative)).unwrap(), expected.bytes);
    std::fs::remove_dir_all(root).unwrap();
}
