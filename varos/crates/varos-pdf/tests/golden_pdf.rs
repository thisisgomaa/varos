//! The charter round-trip law (`FOUNDATION_CHARTER.md:45`) over the frozen v1 **PDF container**
//! corpus (`varos-core/tests/fixtures/v1/README.md`): load → save → reload must equal by content.
//! Mirrors `varos-core/tests/golden.rs`'s raw-JSON version; this crate is where the PDF container
//! logic lives, so the PDF half of the corpus is proved here.

use std::path::PathBuf;

use varos_core::model::Document;

const V1_PDF_CORPUS: [&str; 8] = [
    "v1_plain_pdf.vrs",
    "v1_boardless_pdf.vrs",
    "v1_masked_pdf.vrs",
    "v1_rotated_pdf.vrs",
    "v1_translucent_pdf.vrs",
    "v1_two_artboards_pdf.vrs",
    "v1_guides_snap_pdf.vrs",
    "v1_unicode_arabic_pdf.vrs",
];

fn fixture(name: &str) -> PathBuf {
    // varos-pdf's CARGO_MANIFEST_DIR is .../varos/crates/varos-pdf → the corpus lives in the
    // sibling varos-core crate, alongside its raw-JSON twins.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v1").join(name)
}

fn load_fixture(name: &str) -> Document {
    varos_pdf::load_vrs(&fixture(name)).unwrap_or_else(|e| panic!("{name} should load through load_vrs: {e}"))
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("varos-golden-pdf-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// load → save → reload must preserve every authored-content field (`Document::content_eq`), on
/// disk, through the real PDF container path — not merely the embedded JSON blob.
#[test]
fn frozen_v1_pdf_corpus_round_trips_on_disk_by_content() {
    for name in V1_PDF_CORPUS {
        let loaded = load_fixture(name);
        let p = tmp(name);
        varos_pdf::save_vrs(&loaded, &p).unwrap_or_else(|e| panic!("{name}: save failed: {e}"));
        let reloaded = varos_pdf::load_vrs(&p).unwrap_or_else(|e| panic!("{name}: reload failed: {e}"));
        assert!(
            loaded.content_eq(&reloaded),
            "{name}: load -> save -> load through the PDF container must preserve authored content"
        );
        let _ = std::fs::remove_file(&p);
    }
}

/// The raw-JSON and PDF-container twin of each scenario must load to the SAME content — the
/// container choice is not supposed to change what the file means (ADR-0003).
#[test]
fn raw_and_pdf_twins_load_to_the_same_content() {
    let core_fixture =
        |name: &str| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v1").join(name);
    let pairs = [
        ("v1_plain.vrs", "v1_plain_pdf.vrs"),
        ("v1_boardless.vrs", "v1_boardless_pdf.vrs"),
        ("v1_masked.vrs", "v1_masked_pdf.vrs"),
        ("v1_rotated.vrs", "v1_rotated_pdf.vrs"),
        ("v1_translucent.vrs", "v1_translucent_pdf.vrs"),
        ("v1_two_artboards.vrs", "v1_two_artboards_pdf.vrs"),
        ("v1_guides_snap.vrs", "v1_guides_snap_pdf.vrs"),
        ("v1_unicode_arabic.vrs", "v1_unicode_arabic_pdf.vrs"),
    ];
    for (raw, pdf) in pairs {
        let raw_doc = varos_pdf::load_vrs(&core_fixture(raw)).unwrap_or_else(|e| panic!("{raw}: {e}"));
        let pdf_doc = varos_pdf::load_vrs(&core_fixture(pdf)).unwrap_or_else(|e| panic!("{pdf}: {e}"));
        assert!(raw_doc.content_eq(&pdf_doc), "{raw} and {pdf} must carry the same authored content");
    }
}
