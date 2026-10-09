//! CLI bytes are the same shared deliverable bytes used by app IO jobs and Bridge.
use std::sync::atomic::AtomicBool;
use varos_raster::export::{self, Format, Options, Scope};
#[test]
fn svg_and_png_match_shared_bytes_and_refuse_overwrites() {
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v3_nested_group.vrs");
    let doc = varos_pdf::load_vrs(&input).unwrap();
    let id = doc.artboards[0].id;
    let asset = export::plan(&doc, &Scope::Artboard(id)).unwrap().remove(0);
    let dir = std::env::temp_dir().join(format!("varos-cli-deliverables-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for format in [Format::Svg, Format::Png] {
        let out = dir.join(format!("out.{}", format.extension()));
        let _ = std::fs::remove_file(&out);
        let verb = if format == Format::Svg { "export-svg" } else { "export-raster" };
        let args = [verb, input.to_str().unwrap(), "--artboard", &id.to_string(), "--out", out.to_str().unwrap()];
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_varos-cli")).args(args).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
        let expected =
            export::encode(&asset, &Options { format, ..Default::default() }, &AtomicBool::new(false)).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), expected.bytes);
        assert!(!std::process::Command::new(env!("CARGO_BIN_EXE_varos-cli"))
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(std::fs::read(&out).unwrap(), expected.bytes);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
