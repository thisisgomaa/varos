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
        let error = decode_model(&bytes, None, &Limits::DEFAULT).unwrap_err();
        use varos_core::format::{Invalid, LoadError};
        match name {
            "old_gradient" | "old_swatches" => assert_eq!(
                error,
                LoadError::Invalid(Invalid::FieldNotInFormat { field: "gradient paints / swatches", version: 5 })
            ),
            "future" => assert_eq!(error, LoadError::NewerVersion { found: 7, supported: 6 }),
            "unknown_stop" => assert!(
                matches!(error, LoadError::Malformed { detail, .. } if detail.contains("unknown field `future`"))
            ),
            _ => {
                let what = match name {
                    "unordered" => "gradient stops must be ordered",
                    "singular" => "singular gradient placement",
                    "missing_ref" => "unresolved or recursive swatch reference",
                    "recursive_ref" => "invalid swatch table",
                    _ => unreachable!(),
                };
                assert_eq!(error, LoadError::Invalid(Invalid::NonFinite { what: what.into() }));
            }
        }
    }
}

#[test]
fn provisional_version_is_pinned_until_integrator_renumbers() {
    assert_eq!(NEXT_GRADIENT_VERSION, 6);
    assert_eq!(varos_core::format::FORMAT_VERSION, 6);
}

#[test]
fn identity_migration_performs_no_validation_or_repair() {
    let mut doc = varos_core::model::Document::default();
    doc.units.ppi = -1.;
    assert_eq!(varos_core::format::migrate::migrate_v5_to_next_gradients(doc.clone(), &Limits::DEFAULT).unwrap(), doc);
}
