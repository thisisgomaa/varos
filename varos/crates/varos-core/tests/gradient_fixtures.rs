use varos_core::format::{decode_model, encode_model, Limits, FORMAT_VERSION, GRADIENT_VERSION};
#[test]
fn frozen_next_paints_and_named_migration() {
    for kind in ["linear", "radial", "swatch"] {
        let bytes =
            std::fs::read(format!("{}/tests/fixtures/next_gradients/{kind}.vrs", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let loaded = decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
        assert_eq!(loaded.source_version, GRADIENT_VERSION);
        assert_eq!(loaded.migrated, FORMAT_VERSION > GRADIENT_VERSION);
        // Frozen v7 bytes re-save identically apart from the writer stamp (integration w2).
        let encoded = encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
        assert_eq!(
            encoded,
            std::str::from_utf8(&bytes).unwrap().replacen("\"varos\":7", &format!("\"varos\":{FORMAT_VERSION}"), 1)
        );
    }
    let old = include_bytes!("fixtures/v5/cap_Butt.json");
    let loaded = decode_model(old, None, &Limits::DEFAULT).unwrap();
    assert!(loaded.migrated);
    assert_eq!(
        varos_core::format::migrate::migrate_v6_to_v7(loaded.doc.clone(), &Limits::DEFAULT).unwrap(),
        loaded.doc
    );
}
#[test]
fn frozen_refusals_gate_tagged_values_before_typed_decode() {
    for name in [
        "old_gradient",
        "old_swatches",
        "v6_gradient",
        "v6_swatches",
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
        let bytes = if name == "future" {
            std::fs::read(format!(
                "{}/tests/fixtures/next_gradients/refused/future_v11.vrs",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap()
        } else {
            bytes
        };
        let error = decode_model(&bytes, None, &Limits::DEFAULT).unwrap_err();
        use varos_core::format::{Invalid, LoadError};
        match name {
            "old_gradient" | "old_swatches" => assert_eq!(
                error,
                LoadError::Invalid(Invalid::FieldNotInFormat { field: "gradient paints / swatches", version: 5 })
            ),
            // integration w2: a format-6 (images) file cannot carry format-7 paints either
            "v6_gradient" | "v6_swatches" => assert_eq!(
                error,
                LoadError::Invalid(Invalid::FieldNotInFormat { field: "gradient paints / swatches", version: 6 })
            ),
            "future" => assert_eq!(error, LoadError::NewerVersion { found: 11, supported: FORMAT_VERSION }),
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
fn gradient_format_number_is_pinned() {
    assert_eq!(GRADIENT_VERSION, 7);
}

#[test]
fn identity_migration_performs_no_validation_or_repair() {
    let mut doc = varos_core::model::Document::default();
    doc.units.ppi = -1.;
    assert_eq!(varos_core::format::migrate::migrate_v6_to_v7(doc.clone(), &Limits::DEFAULT).unwrap(), doc);
}
