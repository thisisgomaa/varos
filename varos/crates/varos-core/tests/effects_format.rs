use varos_core::format::{self, Invalid, Limits, LoadError};
const V11: &[u8] = include_bytes!("fixtures/w3-effects/v11-effects.json");
#[test]
fn frozen_v11_roundtrip_keeps_typed_effects_width_and_v9_migration() {
    let loaded = format::decode_model(V11, None, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.source_version, 11);
    assert_eq!(loaded.migrated, format::FORMAT_VERSION > 11);
    assert_eq!(loaded.doc.paths[0].effects.len(), 4);
    let encoded = format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
    let back = format::decode_model(encoded.as_bytes(), None, &Limits::DEFAULT).unwrap();
    assert_eq!(back.doc.paths, loaded.doc.paths);
    let old =
        format::decode_model(include_bytes!("fixtures/w3-effects/v9-plain.json"), None, &Limits::DEFAULT).unwrap();
    assert!(old.migrated);
    assert!(old.doc.paths[0].effects.is_empty());
    assert!(old.doc.paths[0].stroke_style.width_profile.is_none());
}
#[test]
fn frozen_v11_refusals_are_typed_and_predecode() {
    for fixture in [
        include_bytes!("fixtures/w3-effects/refused-effects-v10.json").as_slice(),
        include_bytes!("fixtures/w3-effects/refused-width-v10.json").as_slice(),
    ] {
        assert_eq!(
            format::decode_model(fixture, None, &Limits::DEFAULT).unwrap_err(),
            LoadError::Invalid(Invalid::FieldNotInFormat { field: "effects/width_profile", version: 10 })
        );
    }
    assert!(matches!(
        format::decode_model(include_bytes!("fixtures/w3-effects/refused-invalid-effect.json"), None, &Limits::DEFAULT),
        Err(LoadError::Invalid(Invalid::Stroke { .. }))
    ));
    assert_eq!(
        format::decode_model(include_bytes!("fixtures/w3-effects/refused-future.json"), None, &Limits::DEFAULT)
            .unwrap_err(),
        LoadError::NewerVersion { found: 15, supported: format::FORMAT_VERSION }
    );
}
#[test]
fn named_v10_to_v11_is_pure_identity_even_with_optional_sibling_data() {
    let doc = format::decode_model(V11, None, &Limits::DEFAULT).unwrap().doc;
    assert_eq!(format::migrate_v10_to_v11(doc.clone(), &Limits::DEFAULT).unwrap(), doc);
}
