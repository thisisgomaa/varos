//! Integration w2 (2026-10-09): the final wave-2 format numbers — images 6, gradients 7, text 8,
//! Live Corners + embedded preview 9 — and the contiguous named migration chain v1 → v9.
use varos_core::format::{
    decode_model, encode_model, Limits, LoadError, CORNERS_VERSION, FORMAT_VERSION, GRADIENT_VERSION, IMAGE_VERSION,
    TEXT_FORMAT_VERSION,
};

#[test]
fn writer_and_lane_numbers_are_pinned_literally() {
    assert_eq!((IMAGE_VERSION, GRADIENT_VERSION, TEXT_FORMAT_VERSION, CORNERS_VERSION), (6, 7, 8, 9));
    assert_eq!(FORMAT_VERSION, 14);
    let blob = encode_model(&varos_core::model::Document::default(), &Limits::DEFAULT).unwrap();
    assert!(blob.starts_with("{\"varos\":14,\"doc\":{"));
}

#[test]
fn the_chain_is_contiguous_and_the_next_number_is_newer() {
    // Lane H isolated v9 base: the integrator adds v10-v13 migration steps.
    assert_eq!(varos_core::format::readable_versions(), (1..=9).chain([14]).collect::<Vec<_>>());
    let v5 = std::str::from_utf8(include_bytes!("fixtures/v5/plain.json")).unwrap();
    let next = FORMAT_VERSION + 1;
    let json = v5.replacen("{\"varos\":5,", &format!("{{\"varos\":{next},"), 1);
    assert_eq!(
        decode_model(json.as_bytes(), None, &Limits::DEFAULT).unwrap_err(),
        LoadError::NewerVersion { found: next, supported: FORMAT_VERSION }
    );
}

#[test]
fn every_v5_fixture_round_trips_byte_identical_apart_from_the_stamp() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v5");
    let mut checked = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(original) = std::str::from_utf8(&bytes) else { continue };
        if !original.starts_with("{\"varos\":5,") {
            continue;
        }
        let loaded = decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
        assert!(loaded.migrated && loaded.source_version == 5, "{}", path.display());
        let saved = encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
        assert_eq!(
            saved.trim_end(),
            original.replacen("{\"varos\":5,", &format!("{{\"varos\":{FORMAT_VERSION},"), 1).trim_end(),
            "{}",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 0);
}
