//! Lane A frozen PDF/SVG appearance goldens, alpha groups and format refusals.
use std::sync::atomic::AtomicBool;
use varos_core::format::{self, Limits};
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v10")
}
#[test]
fn frozen_appearance_pdf_and_svg_goldens() {
    for name in ["plain", "multiple", "alpha", "isolated_alpha"] {
        let bytes = std::fs::read(root().join(format!("{name}.json"))).unwrap();
        let loaded = format::decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
        assert_eq!(format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap().as_bytes(), bytes);
        let pdf = varos_pdf::write_pdf(&loaded.doc).unwrap();
        assert_eq!(pdf, std::fs::read(root().join(format!("{name}.pdf"))).unwrap());
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert!(!parsed.get_pages().is_empty());
        let plan = varos_core::svg::plan_svg_export(&loaded.doc, varos_core::svg::ExportScope::WholeBoard).unwrap();
        let files = varos_core::svg::export_svg_files(&loaded.doc, &plan, &AtomicBool::new(false)).unwrap();
        assert_eq!(files[0].bytes, std::fs::read(root().join(format!("{name}.svg"))).unwrap());
        if name.contains("alpha") {
            assert!(pdf.windows(6).any(|s| s == b"/SMask"));
            assert!(pdf.windows(9).any(|s| s == b"/S /Alpha"));
            let svg = std::str::from_utf8(&files[0].bytes).unwrap();
            assert!(svg.contains("<mask "));
            assert!(svg.contains("mask-type:alpha"));
        }
    }
}
#[test]
fn frozen_refusals_and_old_reader_header_gate() {
    for name in ["refused_v9_stack", "refused_future", "refused_missing_base", "refused_unknown_blend"] {
        let bytes = std::fs::read(root().join(format!("{name}.json"))).unwrap();
        assert!(format::decode_model(&bytes, None, &Limits::DEFAULT).is_err());
    }
    let bytes = std::fs::read(root().join("multiple.json")).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v["varos"].as_u64().unwrap() > 9, "v9 reader refuses before decoding stack");
}
#[test]
fn frozen_corpus_hashes() {
    use sha2::{Digest, Sha256};
    for line in std::fs::read_to_string(root().join("SHA256SUMS")).unwrap().lines() {
        let (hash, name) = line.split_once("  ").unwrap();
        let bytes = std::fs::read(root().join(name)).unwrap();
        assert_eq!(Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect::<String>(), hash, "{name}");
    }
}
