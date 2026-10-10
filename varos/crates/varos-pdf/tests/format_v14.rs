//! Integration w3 (2026-10-10): the frozen mixed format-14 document carries every wave-2 and wave-3
//! model addition (image, gradient + Live Corners, appearance stack, live effect, global CMYK swatch,
//! live Repeat node, styled area text bound to a shape). It pins the combined `Document` key order,
//! round-trips byte-for-byte in raw JSON and in the native container, writes the expected PDF
//! resources, and is refused by every frozen older reader gate (v9–v13) before any typed decode.
use varos_core::format::{decode_model, encode_model, Limits, LoadError, FORMAT_VERSION};

const JSON: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v14-mixed/mixed.json");
const VRS: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v14-mixed/mixed.vrs");

fn embedded_model_json(pdf_bytes: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(pdf_bytes).unwrap();
    let catalog = doc.catalog().unwrap();
    let (_, o) = doc.dereference(catalog.get(b"VAROS_Model").unwrap()).unwrap();
    String::from_utf8(o.as_stream().unwrap().content.clone()).unwrap()
}

/// Frozen header gate shared by every reader since v5 (`peek_version` with that era's constant).
/// This adapts the gate; it does not execute an old binary.
fn frozen_gate(body: &[u8], supported: u32) -> Result<u32, LoadError> {
    #[derive(serde::Deserialize)]
    struct Head {
        varos: u32,
    }
    let head: Head = serde_json::from_slice(body).expect("a version header");
    if head.varos > supported {
        Err(LoadError::NewerVersion { found: head.varos, supported })
    } else {
        Ok(head.varos)
    }
}

#[test]
fn mixed_v14_round_trips_byte_for_byte_and_pins_document_key_order() {
    assert_eq!(FORMAT_VERSION, 14);
    let loaded = decode_model(JSON, None, &Limits::DEFAULT).unwrap();
    assert_eq!((loaded.source_version, loaded.migrated), (14, false));
    assert_eq!(encode_model(&loaded.doc, &Limits::DEFAULT).unwrap().as_bytes(), JSON);
    let text = std::str::from_utf8(JSON).unwrap();
    let mut at = 0;
    for key in [
        "\"images\":",
        "\"assets\":",
        "\"swatches\":",
        "\"text_boxes\":",
        "\"typography\":",
        "\"paths\":",
        "\"nodes\":",
    ] {
        at += text[at..].find(key).unwrap_or_else(|| panic!("{key} missing or out of order"));
    }
    // the appearance stack and the live effects lead each path object (v10 then v11)
    let first = &text[text.find("\"paths\":[{").unwrap()..];
    assert!(first.starts_with("\"paths\":[{\"stack\":["), "stack is the first path key");
    assert!(first.find("\"effects\":").unwrap() < first.find("\"id\":").unwrap(), "effects precede id");
    let doc = &loaded.doc;
    use varos_core::model::{NodeKind, Paint};
    assert_eq!((doc.images.len(), doc.swatches.len(), doc.text_boxes.len()), (1, 1, 1));
    let styled = doc.paths.iter().find(|p| !p.stack.is_empty()).expect("appearance stack");
    assert!(!styled.effects.is_empty() && !styled.corners.is_empty() && matches!(styled.fill, Paint::Gradient(_)));
    assert!(matches!(doc.swatches[0].paint, Paint::Managed(_)));
    assert!(doc.paths.iter().any(|p| matches!(p.fill, Paint::SwatchRef { .. })));
    assert!(doc.nodes.iter().any(|n| matches!(n.kind, NodeKind::Live(_))));
    let frame = doc.typography.frames.values().next().expect("typography frame");
    assert!(matches!(frame.binding, Some(varos_core::typography::Binding::Area { .. })));
    assert!(!doc.typography.characters.is_empty() && !doc.typography.paragraphs.is_empty());
}

#[test]
fn mixed_v14_container_reopens_and_rewrites_identically_with_every_resource() {
    let loaded = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.doc, decode_model(JSON, None, &Limits::DEFAULT).unwrap().doc);
    assert!(loaded.blobs.get(&loaded.doc.images[0].blob).is_some());
    assert_eq!(embedded_model_json(VRS).as_bytes(), JSON);
    let rewritten = varos_pdf::images::write_vrs(&loaded.doc, &loaded.blobs, &Limits::DEFAULT).unwrap();
    assert_eq!(rewritten, VRS, "the native writer is deterministic for the mixed document");
    let pdf = String::from_utf8_lossy(VRS);
    for (needle, what) in [
        ("/Subtype /Image", "image XObject"),
        ("ShadingType", "gradient shading"),
        ("/Type /Font", "embedded selectable text"),
        ("DeviceCMYK", "managed CMYK swatch colour space"),
    ] {
        assert!(pdf.contains(needle), "{what}");
    }
}

#[test]
fn frozen_v9_to_v13_gates_refuse_v14_raw_json_and_container_before_decode() {
    let model = embedded_model_json(VRS);
    let catalog = lopdf::Document::load_mem(VRS)
        .unwrap()
        .catalog()
        .unwrap()
        .get(b"VAROS_SchemaVersion")
        .unwrap()
        .as_i64()
        .unwrap() as u32;
    assert_eq!(catalog, 14);
    for supported in [9, 10, 11, 12, 13] {
        let expected = Err(LoadError::NewerVersion { found: 14, supported });
        assert_eq!(frozen_gate(JSON, supported), expected, "v{supported} raw JSON");
        assert_eq!(frozen_gate(model.as_bytes(), supported), expected, "v{supported} embedded model");
        assert!(catalog > supported, "v{supported} catalog stamp");
    }
}

#[test]
fn each_frozen_wave_three_gate_refuses_the_next_writer() {
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures");
    // v9 gate ↔ v10 appearance, v10 ↔ v11 effects, v11 ↔ v12 colour, v12 ↔ v13 live, v13 ↔ v14 typography.
    for (supported, file, found) in [
        (9, "v10/multiple.json", 10),
        (10, "w3-effects/v11-effects.json", 11),
        (11, "v12/colours.json", 12),
        (12, "v13/live.json", 13),
        (13, "v14/styles.json", 14),
    ] {
        let bytes = std::fs::read(core.join(file)).unwrap();
        assert_eq!(frozen_gate(&bytes, supported), Err(LoadError::NewerVersion { found, supported }), "{file}");
        // and the current reader accepts each era (migrating the older ones)
        assert!(decode_model(&bytes, None, &Limits::DEFAULT).is_ok(), "{file}");
    }
    // Container side: the frozen v13 native file is refused by the v12 gate on its catalog and model.
    let vrs = std::fs::read(core.join("v13/live.vrs")).unwrap();
    assert_eq!(
        frozen_gate(embedded_model_json(&vrs).as_bytes(), 12),
        Err(LoadError::NewerVersion { found: 13, supported: 12 })
    );
}
