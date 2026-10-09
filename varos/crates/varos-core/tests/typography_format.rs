use varos_core::format::{self, Limits};
const FIXTURES: &[(&str, &[u8])] = &[
    ("styles", include_bytes!("fixtures/v14/styles.json")),
    ("shape", include_bytes!("fixtures/v14/shape.json")),
    ("path", include_bytes!("fixtures/v14/path.json")),
];
#[test]
fn v14_frozen_roundtrip_and_old_reader_refusal() {
    for (name, bytes) in FIXTURES {
        let loaded = format::decode_model(bytes, None, &Limits::DEFAULT).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(loaded.source_version, 14);
        let encoded = format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
        let reopened = format::decode_model(encoded.as_bytes(), None, &Limits::DEFAULT).unwrap();
        assert_eq!(loaded.doc, reopened.doc);
        let header: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert!(header["varos"].as_u64().unwrap() > 9, "frozen v9 header gate must refuse before decoding");
    }
    for bytes in [
        include_bytes!("fixtures/v14/refuse-v15.json").as_slice(),
        include_bytes!("fixtures/v14/refuse-v9-typography.json"),
        include_bytes!("fixtures/v14/refuse-style-cycle.json"),
    ] {
        assert!(format::decode_model(bytes, None, &Limits::DEFAULT).is_err());
    }
}
#[test]
fn migration_preserves_v9_content_and_is_pure() {
    let bytes = include_bytes!("fixtures/text_next/mixed.json");
    let mut legacy: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    legacy["varos"] = 9.into();
    let encoded = serde_json::to_vec(&legacy).unwrap();
    let loaded = format::decode_model(&encoded, None, &Limits::DEFAULT).unwrap();
    assert!(loaded.migrated);
    assert!(loaded.doc.typography.is_empty());
    let before = loaded.doc.clone();
    let migrated = varos_core::typography_format::migrate_v13_to_v14(before.clone(), &Limits::DEFAULT).unwrap();
    assert_eq!(migrated, before);
}
#[test]
fn fixture_hashes_are_frozen() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v14");
    for line in include_str!("fixtures/v14/SHA256SUMS").lines() {
        let (digest, name) = line.split_once("  ").unwrap();
        let actual = Sha256::digest(std::fs::read(root.join(name)).unwrap());
        assert_eq!(actual.iter().map(|b| format!("{b:02x}")).collect::<String>(), digest);
    }
}
