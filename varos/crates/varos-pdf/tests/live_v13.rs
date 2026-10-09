//! Frozen v13 authored containers and evaluated-only deliverables.
#[path = "support/native_era.rs"]
mod native_era;
use std::sync::atomic::AtomicBool;
use varos_core::{
    format::{self, Invalid, Limits, LoadError},
    live, Editor,
};
const JSON: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v13/live.json");
const VRS: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v13/live.vrs");
#[test]
fn frozen_live_json_and_pdf_reopen_editably_and_rewrite_identically() {
    let loaded = format::decode_model(JSON, None, &Limits::DEFAULT).unwrap();
    assert_eq!((loaded.source_version, loaded.migrated), (13, format::FORMAT_VERSION > 13));
    assert!(live::has_live(&loaded.doc));
    // integration w3: the frozen v13 body re-saves byte-identical apart from the writer stamp
    let current = format!("\"varos\":{}", format::FORMAT_VERSION);
    assert_eq!(
        format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap().replacen(&current, "\"varos\":13", 1).as_bytes(),
        JSON
    );
    let native = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    assert_eq!(native.doc, loaded.doc);
    assert_eq!(
        native_era::restamp(
            &varos_pdf::write_pdf_checked(&native.doc, &Limits::DEFAULT).unwrap(),
            format::FORMAT_VERSION,
            13
        ),
        VRS
    );
}
#[test]
fn evaluated_exports_match_expand_and_keep_authored_model() {
    let loaded = format::decode_model(JSON, None, &Limits::DEFAULT).unwrap();
    let doc = &loaded.doc;
    let expanded = live::evaluated_document(doc).unwrap().unwrap();
    let cancel = AtomicBool::new(false);
    let plan = varos_pdf::plan_pdf_export(doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
    let pdf = varos_pdf::export_pdf_bytes(doc, &plan, &cancel).unwrap();
    assert!(!varos_pdf::has_embedded_model(&pdf));
    assert_eq!(pdf, varos_pdf::export_pdf_bytes(&expanded, &plan, &cancel).unwrap());
    let mut ed = Editor::new();
    ed.doc = doc.clone();
    let scene = varos_core::scene::build_artwork_scene(&ed, 1.);
    ed.doc = expanded.clone();
    assert_eq!(scene.content, varos_core::scene::build_artwork_scene(&ed, 1.).content);
    let splan = varos_core::svg::plan_svg_export(doc, varos_core::svg::ExportScope::WholeBoard).unwrap();
    assert_eq!(
        varos_core::svg::export_svg_files(doc, &splan, &cancel).unwrap(),
        varos_core::svg::export_svg_files(&expanded, &splan, &cancel).unwrap()
    );
    assert!(live::has_live(doc));
}
#[test]
fn v13_refusals_are_specific_and_migration_is_pure() {
    assert_eq!(
        format::decode_model(
            include_bytes!("../../varos-core/tests/fixtures/v13/refused-v12-live.json"),
            None,
            &Limits::DEFAULT
        )
        .unwrap_err(),
        LoadError::Invalid(Invalid::FieldNotInFormat { field: "nodes[].kind.Live", version: 12 })
    );
    assert_eq!(
        format::decode_model(
            include_bytes!("../../varos-core/tests/fixtures/v13/refused-newer.json"),
            None,
            &Limits::DEFAULT
        )
        .unwrap_err(),
        // integration w3: every refused-future fixture is format 15 (writer 14)
        LoadError::NewerVersion { found: 15, supported: format::FORMAT_VERSION }
    );
    assert_eq!(
        format::decode_model(
            include_bytes!("../../varos-core/tests/fixtures/v13/refused-grid-zero.json"),
            None,
            &Limits::DEFAULT
        )
        .unwrap_err(),
        LoadError::Invalid(Invalid::NonFinite { what: "Invalid grid rows/cols/gap".into() })
    );
    let doc = varos_core::model::Document { active: 999, ..Default::default() };
    let migrated = live::migrate_v12_to_v13(doc.clone(), &Limits::DEFAULT).unwrap();
    assert_eq!(migrated, doc);
    // Frozen adaptation of era-12's header gate, before inspecting the new enum.
    #[derive(serde::Deserialize)]
    struct Head {
        varos: u32,
    }
    fn gate(json: &[u8]) -> Result<(), LoadError> {
        let h: Head = serde_json::from_slice(json).unwrap();
        if h.varos > 12 {
            Err(LoadError::NewerVersion { found: h.varos, supported: 12 })
        } else {
            Ok(())
        }
    }
    assert_eq!(gate(JSON), Err(LoadError::NewerVersion { found: 13, supported: 12 }));
    assert_eq!(gate(br#"{"varos":13,"doc":42}"#), Err(LoadError::NewerVersion { found: 13, supported: 12 }));
    let pdf = lopdf::Document::load_mem(VRS).unwrap();
    let catalog = pdf.catalog().unwrap();
    assert_eq!(catalog.get(b"VAROS_SchemaVersion").unwrap().as_i64().unwrap(), 13);
    let (_, obj) = pdf.dereference(catalog.get(b"VAROS_Model").unwrap()).unwrap();
    assert_eq!(gate(&obj.as_stream().unwrap().content), Err(LoadError::NewerVersion { found: 13, supported: 12 }));
}
#[test]
fn frozen_live_hashes() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v13");
    for line in std::fs::read_to_string(root.join("SHA256SUMS")).unwrap().lines() {
        let (hash, name) = line.split_once("  ").unwrap();
        let actual: String =
            Sha256::digest(std::fs::read(root.join(name)).unwrap()).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(actual, hash, "{name}");
    }
}
