mod effects_support;
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
        assert!(loaded.migrated);
        assert_eq!(loaded.source_version, 5);
        // The next writer changes only the envelope; authored v5 content stays byte-identical.
        let expected = String::from_utf8(json.clone()).unwrap().replacen(
            "\"varos\":5",
            &format!("\"varos\":{}", varos_core::format::FORMAT_VERSION),
            1,
        );
        assert_eq!(varos_core::format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap(), expected, "{name}: JSON");
        let pdf = std::fs::read(root.join(format!("{name}.pdf"))).unwrap();
        assert_eq!(varos_pdf::load_vrs_bytes(&pdf, &Limits::DEFAULT).unwrap().doc, loaded.doc);
        let current = varos_pdf::write_pdf(&loaded.doc).unwrap();
        assert_eq!(varos_pdf::load_vrs_bytes(&current, &Limits::DEFAULT).unwrap().doc, loaded.doc);
        let old_pdf = lopdf::Document::load_mem(&pdf).unwrap();
        let new_pdf = lopdf::Document::load_mem(&current).unwrap();
        for (old, new) in old_pdf.get_pages().values().zip(new_pdf.get_pages().values()) {
            assert_eq!(
                old_pdf.get_page_content(*old).unwrap(),
                new_pdf.get_page_content(*new).unwrap(),
                "{name}: painted PDF content"
            );
        }
        // Only the container and model version stamps change; all legacy appearance bytes stay frozen.
        assert_eq!(effects_support::normalized(&current), effects_support::normalized(&pdf), "{name}: PDF");
        let plan = plan_svg_export(&loaded.doc, ExportScope::WholeBoard).unwrap();
        let files = export_svg_files(&loaded.doc, &plan, &AtomicBool::new(false)).unwrap();
        assert_eq!(files[0].bytes, std::fs::read(root.join(format!("{name}.svg"))).unwrap(), "{name}: SVG");
    }
}
#[test]
fn frozen_v6_refusals_stay_invalid_with_current_version() {
    for ext in ["json", "pdf"] {
        let bytes = std::fs::read(root().join(format!("refused/future_v6.{ext}"))).unwrap();
        assert!(matches!(varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT), Err(LoadError::Malformed { .. })));
    }
}
#[test]
fn next_future_refusal_precedes_typed_decode_in_both_containers() {
    use lopdf::Object;
    let version = varos_core::format::FORMAT_VERSION + 1;
    let json = format!("{{\"varos\":{version},\"doc\":42}}");
    let mut pdf = lopdf::Document::load_mem(&std::fs::read(root().join("refused/future_v6.pdf")).unwrap()).unwrap();
    for obj in pdf.objects.values_mut() {
        match obj {
            Object::Dictionary(d) if d.has(b"VAROS_SchemaVersion") => {
                d.set("VAROS_SchemaVersion", version as i64);
            }
            Object::Stream(stream) => {
                stream.dict.remove(b"Filter");
                stream.set_content(json.as_bytes().to_vec());
            }
            _ => {}
        }
    }
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).unwrap();
    for bytes in [json.as_bytes(), bytes.as_slice()] {
        assert_eq!(
            varos_pdf::load_vrs_bytes(bytes, &Limits::DEFAULT).unwrap_err(),
            LoadError::NewerVersion { found: version, supported: varos_core::format::FORMAT_VERSION }
        );
    }
}
#[test]
fn quicklook_future_refusal_precedes_typed_decode_in_both_containers() {
    for ext in ["json", "pdf"] {
        let bytes = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/quicklook/future-v10.{ext}")),
        )
        .unwrap();
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
