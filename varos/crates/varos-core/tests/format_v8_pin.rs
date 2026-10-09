//! Integration 2026-10-09: text pinned to its final format number 8 before images (6) and
//! gradients (7) land. Stage 2 replaces the temporary v5 → v8 identity with the real chain.
use varos_core::format::{decode_model, encode_model, Limits, LoadError, FORMAT_VERSION, TEXT_FORMAT_VERSION};

#[test]
fn writer_is_pinned_to_literal_8() {
    assert_eq!(FORMAT_VERSION, 8);
    assert_eq!(TEXT_FORMAT_VERSION, 8);
    let blob = encode_model(&varos_core::model::Document::default(), &Limits::DEFAULT).unwrap();
    assert!(blob.starts_with("{\"varos\":8,\"doc\":{"));
}

#[test]
fn reserved_formats_6_and_7_are_refused_by_this_build() {
    let v5 = std::str::from_utf8(include_bytes!("fixtures/v5/plain.json")).unwrap();
    for v in [6u32, 7] {
        let json = v5.replacen("{\"varos\":5,", &format!("{{\"varos\":{v},"), 1);
        assert!(
            matches!(
                decode_model(json.as_bytes(), None, &Limits::DEFAULT),
                Err(LoadError::MigrationFailed { from, .. }) if from == v
            ),
            "format {v}"
        );
    }
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
            original.replacen("{\"varos\":5,", "{\"varos\":8,", 1).trim_end(),
            "{}",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 0);
}
