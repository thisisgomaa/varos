//! Integration w2 (2026-10-09): the frozen mixed format-9 document (placed image, gradient fill +
//! global swatch, Live Corners, editable text) pins the combined `Document` key order, round-trips
//! byte-for-byte, exports image + gradient through PDF/SVG/CPU, and is refused by every frozen older
//! reader gate (v5/v6/v7/v8) before any typed decode, in raw JSON and in the PDF container.
use std::sync::atomic::AtomicBool;
use varos_core::format::{decode_model, encode_model, Limits, LoadError, FORMAT_VERSION};

const JSON: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v9/mixed.json");
const VRS: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v9/mixed.vrs");

fn embedded_model_json(pdf_bytes: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(pdf_bytes).unwrap();
    let catalog = doc.catalog().unwrap();
    let (_, o) = doc.dereference(catalog.get(b"VAROS_Model").unwrap()).unwrap();
    String::from_utf8(o.as_stream().unwrap().content.clone()).unwrap()
}

/// Frozen header gate shared by the v5–v8 readers (`peek_version` with that era's constant).
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
fn mixed_v9_round_trips_and_pins_document_key_order() {
    assert_eq!(FORMAT_VERSION, 12);
    let loaded = decode_model(JSON, None, &Limits::DEFAULT).unwrap();
    assert_eq!((loaded.source_version, loaded.migrated), (9, true));
    assert_eq!(
        encode_model(&loaded.doc, &Limits::DEFAULT).unwrap(),
        std::str::from_utf8(JSON).unwrap().replacen("\"varos\":9", "\"varos\":12", 1)
    );
    let value: serde_json::Value = serde_json::from_slice(JSON).unwrap();
    let keys: Vec<&str> = value["doc"].as_object().unwrap().keys().map(String::as_str).collect();
    // serde_json's Map keeps insertion order only with preserve_order; compare the raw byte order.
    let text = std::str::from_utf8(JSON).unwrap();
    let mut at = 0;
    for key in ["\"images\":", "\"assets\":", "\"swatches\":", "\"text_boxes\":", "\"paths\":", "\"nodes\":"] {
        let next = text[at..].find(key).unwrap_or_else(|| panic!("{key} missing or out of order")) + at;
        at = next;
    }
    assert!(keys.contains(&"swatches") && keys.contains(&"text_boxes"));
    let doc = &loaded.doc;
    assert_eq!((doc.images.len(), doc.swatches.len(), doc.text_boxes.len()), (1, 1, 1));
    assert!(doc.paths.iter().any(|p| !p.corners.is_empty() && matches!(p.fill, varos_core::model::Paint::Gradient(_))));
}

#[test]
fn mixed_v9_container_reopens_with_resources_and_rewrites_identically() {
    let loaded = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.doc, decode_model(JSON, None, &Limits::DEFAULT).unwrap().doc);
    assert!(loaded.blobs.get(&loaded.doc.images[0].blob).is_some());
    let rewritten = varos_pdf::images::write_vrs(&loaded.doc, &loaded.blobs, &Limits::DEFAULT).unwrap();
    assert_eq!(embedded_model_json(&rewritten), embedded_model_json(VRS).replacen("\"varos\":9", "\"varos\":12", 1));
    let old = lopdf::Document::load_mem(VRS).unwrap();
    let new = lopdf::Document::load_mem(&rewritten).unwrap();
    for (a, b) in old.get_pages().values().zip(new.get_pages().values()) {
        assert_eq!(old.get_page_content(*a).unwrap(), new.get_page_content(*b).unwrap());
    }
}

#[test]
fn mixed_v9_exports_image_gradient_and_text_in_pdf_svg_and_cpu() {
    let loaded = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    let doc = &loaded.doc;
    // PDF appearance: the image XObject and the gradient shading are both written.
    let pdf = String::from_utf8_lossy(VRS);
    assert!(pdf.contains("/Subtype /Image") || pdf.contains("/Subtype/Image"), "image XObject");
    assert!(pdf.contains("/ShadingType") || pdf.contains("/Shading"), "gradient shading");
    // SVG via the image-aware writer: image element and a gradient definition (never a solid placeholder).
    let outlined = varos_text_layout::outline_document(doc).unwrap();
    let plan = varos_core::svg::plan_svg_export(&outlined, varos_core::svg::ExportScope::WholeBoard).unwrap();
    let (files, _) =
        varos_core::images::svg::export(&outlined, &loaded.blobs, &plan, false, &AtomicBool::new(false)).unwrap();
    let svg = std::str::from_utf8(&files[0].bytes).unwrap();
    assert!(svg.contains("<image "), "image element");
    assert!(svg.contains("linearGradient"), "gradient definition");
    // Live Corners are resolved before painting (review P1): the corner path's SVG geometry is curved.
    let corner = doc.paths.iter().find(|p| !p.corners.is_empty()).unwrap();
    let marker = format!("id=\"path-{}", corner.id);
    let at = svg.find(&marker).expect("corner path element");
    let element = &svg[at..at + svg[at..].find("</g>").unwrap()];
    assert!(element.contains(" d=\"M") && element.contains('C'), "rounded corners export as curves: {element}");
    // CPU scene: image + gradient paint kinds, and the gradient ring follows the rounded outline.
    let mut ed = varos_core::Editor::new();
    ed.replace_doc(outlined.clone());
    ed.blobs = loaded.blobs.clone();
    let scene = varos_core::scene::build_artwork_scene(&ed, 1.);
    assert!(scene.errors.is_empty(), "{:?}", scene.errors);
    let prims: Vec<_> = scene.content.iter().flat_map(|g| g.prims()).collect();
    assert!(prims.iter().any(|p| matches!(p, varos_core::scene::Prim::Image { .. })));
    let ring = prims
        .iter()
        .find_map(|p| match p {
            varos_core::scene::Prim::GradientFill { rings, stroke: false, .. } => Some(rings[0].clone()),
            _ => None,
        })
        .expect("gradient fill");
    assert!(ring.len() > 8, "a rounded rectangle flattens to more than its four corners");
    // No ring point sits on the sharp corner (40,40): radius 8 cuts it off.
    assert!(ring.iter().all(|q| (q[0] - 40.).hypot(q[1] - 40.) > 2.), "corner (40,40) is rounded");
}

#[test]
fn frozen_v5_to_v8_gates_refuse_v9_raw_json_and_container_before_decode() {
    let model = embedded_model_json(VRS);
    let catalog_version = lopdf::Document::load_mem(VRS)
        .unwrap()
        .catalog()
        .unwrap()
        .get(b"VAROS_SchemaVersion")
        .unwrap()
        .as_i64()
        .unwrap() as u32;
    for supported in [5, 6, 7, 8] {
        let expected = Err(LoadError::NewerVersion { found: 9, supported });
        assert_eq!(frozen_gate(JSON, supported), expected, "v{supported} raw JSON");
        assert_eq!(frozen_gate(model.as_bytes(), supported), expected, "v{supported} embedded model");
        assert!(catalog_version > supported, "v{supported} catalog stamp");
    }
}

#[test]
fn each_frozen_gate_refuses_the_next_wave_two_writer() {
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures");
    // v5 gate ↔ v6 images, v6 gate ↔ v7 gradients, v7 gate ↔ v8 text, v8 gate ↔ v9 corners.
    for (supported, file, found) in [
        (5, "v6-images/embedded-crop.json", 6),
        (6, "next_gradients/linear.vrs", 7),
        (7, "text_next/mixed.json", 8),
        (8, "lane_c/next_corners.json", 9),
    ] {
        let bytes = std::fs::read(core.join(file)).unwrap();
        assert_eq!(frozen_gate(&bytes, supported), Err(LoadError::NewerVersion { found, supported }), "{file}");
        // and the current reader accepts each era (migrating the older ones)
        assert!(decode_model(&bytes, None, &Limits::DEFAULT).is_ok(), "{file}");
    }
    // Container side: the v6-images native file is refused by the v5 gate on its catalog and model.
    let vrs = std::fs::read(core.join("v6-images/embedded-crop.vrs")).unwrap();
    assert_eq!(
        frozen_gate(embedded_model_json(&vrs).as_bytes(), 5),
        Err(LoadError::NewerVersion { found: 6, supported: 5 })
    );
}
