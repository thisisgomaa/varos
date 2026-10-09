use std::{path::PathBuf, sync::atomic::AtomicBool};
use varos_core::{
    format::{decode_model, Limits, LoadError},
    svg::{export_svg_files, plan_svg_export, ExportScope},
};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures")
}
#[test]
fn v5_frozen_json_pdf_and_svg_goldens() {
    let root = root().join("v5");
    let index = std::fs::read_to_string(root.join("INDEX")).unwrap();
    for name in index.lines() {
        let json = std::fs::read(root.join(format!("{name}.json"))).unwrap();
        let loaded = decode_model(&json, None, &Limits::DEFAULT).unwrap();
        assert_eq!(loaded.migrated, varos_core::format::FORMAT_VERSION > 5);
        assert_eq!(
            varos_core::format::encode_model(&loaded.doc, &Limits::DEFAULT)
                .unwrap()
                .replacen(&format!("\"varos\":{}", varos_core::format::FORMAT_VERSION), "\"varos\":5", 1)
                .as_bytes(),
            json,
            "{name}: JSON"
        );
        let pdf = std::fs::read(root.join(format!("{name}.pdf"))).unwrap();
        assert_eq!(varos_pdf::load_vrs_bytes(&pdf, &Limits::DEFAULT).unwrap().doc, loaded.doc);
        let current = varos_pdf::write_pdf(&loaded.doc).unwrap();
        let mut normalized = current;
        for (needle, replacement) in [
            (
                format!("/VAROS_SchemaVersion {}", varos_core::format::FORMAT_VERSION),
                "/VAROS_SchemaVersion 5".to_owned(),
            ),
            (format!("\"varos\":{}", varos_core::format::FORMAT_VERSION), "\"varos\":5".to_owned()),
        ] {
            let at = normalized.windows(needle.len()).position(|w| w == needle.as_bytes()).expect("version stamp");
            normalized.splice(at..at + needle.len(), replacement.bytes());
        }
        assert_eq!(normalized, pdf, "{name}: PDF appearance and all non-version bytes");
        let plan = plan_svg_export(&loaded.doc, ExportScope::WholeBoard).unwrap();
        let files = export_svg_files(&loaded.doc, &plan, &AtomicBool::new(false)).unwrap();
        assert_eq!(files[0].bytes, std::fs::read(root.join(format!("{name}.svg"))).unwrap(), "{name}: SVG");
    }
}
#[test]
fn historical_future_v6_is_still_refused_as_malformed_in_both_containers() {
    for ext in ["json", "pdf"] {
        let bytes = std::fs::read(root().join(format!("refused/future_v6.{ext}"))).unwrap();
        assert!(matches!(varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT), Err(LoadError::Malformed { .. })));
    }
}

#[test]
fn v5_fixture_hashes_are_frozen() {
    use sha2::{Digest, Sha256};
    let root = root().join("v5");
    for line in std::fs::read_to_string(root.join("SHA256SUMS")).unwrap().lines() {
        let (expected, name) = line.split_once("  ").unwrap();
        let hash: String =
            Sha256::digest(std::fs::read(root.join(name)).unwrap()).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hash, expected, "{name}");
    }
}

#[test]
fn repeated_export_pages_share_one_geometry_budget() {
    let mut doc =
        decode_model(&std::fs::read(root().join("v5/cap_Butt.json")).unwrap(), None, &Limits::DEFAULT).unwrap().doc;
    doc.paths[0].stroke_style.dash = vec![1.0, 1.0];
    let mut pdf = varos_pdf::plan_pdf_export(&doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
    pdf.pages = vec![pdf.pages[0]; 1000];
    assert_eq!(
        varos_pdf::export_pdf_bytes(&doc, &pdf, &AtomicBool::new(false)),
        Err(varos_pdf::ExportError::LimitExceeded)
    );
    let mut svg = plan_svg_export(&doc, ExportScope::WholeBoard).unwrap();
    svg.pages = vec![svg.pages[0].clone(); 1000];
    assert_eq!(export_svg_files(&doc, &svg, &AtomicBool::new(false)), Err(varos_core::svg::ExportError::LimitExceeded));
}

#[test]
fn baked_translucent_stroke_stream_contains_only_coverage_not_centerline() {
    let bytes = std::fs::read(root().join("v5/align_Inside.json")).unwrap();
    let loaded = decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
    let pdf = lopdf::Document::load_mem(&varos_pdf::write_pdf(&loaded.doc).unwrap()).unwrap();
    let mut checked = 0;
    for object in pdf.objects.values() {
        if let Ok(stream) = object.as_stream() {
            let content = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
            let text = String::from_utf8_lossy(&content);
            if let Some((_, stroke)) = text.split_once("/Gk gs") {
                // One outer + one inner coverage ring. The extra original triangle caused
                // even-odd cancellation and painted the interior instead of the band.
                assert_eq!(stroke.matches(" m\n").count(), 2, "{stroke}");
                assert!(stroke.contains("f*"));
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 1);
}
