use varos_core::format::{decode_model, encode_model, Limits, NEXT_GRADIENT_VERSION};
#[test]
fn frozen_next_paints_and_named_migration() {
    for kind in ["linear", "radial", "swatch"] {
        let bytes =
            std::fs::read(format!("{}/tests/fixtures/next_gradients/{kind}.vrs", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let loaded = decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
        assert_eq!(loaded.source_version, NEXT_GRADIENT_VERSION);
        let encoded = encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
        assert_eq!(encoded.as_bytes(), bytes);
    }
    let old = include_bytes!("fixtures/v5/cap_Butt.json");
    let loaded = decode_model(old, None, &Limits::DEFAULT).unwrap();
    assert!(loaded.migrated);
    assert_eq!(
        varos_core::format::migrate::migrate_v5_to_next_gradients(loaded.doc.clone(), &Limits::DEFAULT).unwrap(),
        loaded.doc
    );
}
#[test]
fn frozen_refusals_gate_tagged_values_before_typed_decode() {
    for name in [
        "old_gradient",
        "old_swatches",
        "future",
        "unordered",
        "singular",
        "missing_ref",
        "recursive_ref",
        "unknown_stop",
    ] {
        let bytes =
            std::fs::read(format!("{}/tests/fixtures/next_gradients/refused/{name}.vrs", env!("CARGO_MANIFEST_DIR")))
                .unwrap();
        assert!(decode_model(&bytes, None, &Limits::DEFAULT).is_err(), "{name}");
    }
}
