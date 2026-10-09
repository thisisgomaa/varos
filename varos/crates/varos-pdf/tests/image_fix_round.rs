//! Reviewer reproductions; every test remains headless.
use std::sync::{atomic::AtomicBool, Arc};
use varos_core::{
    format::Limits,
    images::{self, Pixels, PlacementMode},
    Editor,
};
fn image(ed: &mut Editor, size: u32, color: [u8; 4]) {
    let bytes = images::codec::encode_png(&Pixels {
        budget: None,
        width: size,
        height: size,
        rgba: Arc::from(color.repeat(size as usize * size as usize)),
    })
    .unwrap();
    images::links::place_bytes(ed, &bytes, [0.; 2], None, PlacementMode::Embed, None).unwrap();
}
#[test]
fn pinned_large_images_save_and_budget_limited_open_keep_originals() {
    let mut ed = Editor::new();
    image(&mut ed, 4500, [255, 0, 0, 255]);
    image(&mut ed, 4500, [0, 255, 0, 255]);
    let resident = images::budget::charged_bytes();
    let bytes = varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT).unwrap();
    assert_eq!(images::budget::charged_bytes(), resident, "save must not acquire decode leases");
    assert!(bytes.len() < 2 * 1024 * 1024, "Flate appearance prevents raw 162MB save");
    let loaded = varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap();
    assert!(loaded.notice().unwrap().contains("proxy"));
    assert_eq!(loaded.doc, ed.doc);
    for meta in &loaded.doc.assets {
        assert!(loaded.blobs.get(&meta.key).unwrap().original.is_some());
    }
    assert!(varos_pdf::images::write_vrs(&loaded.doc, &loaded.blobs, &Limits::DEFAULT).is_ok());
}
#[test]
fn invisible_image_retains_assets_but_never_requests_appearance() {
    let mut ed = Editor::new();
    image(&mut ed, 2, [255; 4]);
    ed.doc.images[0].opacity = 0.;
    let bytes = varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT).unwrap();
    assert!(varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT)
        .unwrap()
        .blobs
        .get(&ed.doc.assets[0].key)
        .unwrap()
        .original
        .is_some());
    let plan = varos_pdf::plan_pdf_export(&ed.doc, varos_pdf::ExportScope::ArtworkBounds);
    // Invisible artwork has no export bounds; a requested explicit page still exports successfully.
    assert!(plan.is_err());
    let pages =
        vec![varos_pdf::PageSpec { rect: [0., 0., 72., 72.], background: None, bleed: 0., bleed_edges: [0.; 4] }];
    assert!(varos_pdf::images::export_pdf(&ed.doc, &ed.blobs, &pages, 300., false, &AtomicBool::new(false)).is_ok());
}
#[test]
fn image_addition_keeps_exact_vector_cubics_and_knockout_alpha() {
    let fixture = include_bytes!("../../varos-core/tests/fixtures/v5/align_Inside.json");
    let mut ed = Editor::new();
    ed.replace_doc(varos_core::format::decode_model(fixture, None, &Limits::DEFAULT).unwrap().doc);
    image(&mut ed, 2, [255; 4]);
    let bytes = varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert!(pdf.objects.values().any(|o| o.as_stream().is_ok_and(|s| s
        .dict
        .get(b"Group")
        .is_ok_and(|g| g.as_dict().is_ok_and(|g| g.get(b"K").unwrap().as_bool().is_ok_and(|v| v))))));
    assert!(pdf.objects.values().any(|o| o
        .as_dict()
        .is_ok_and(|d| d.get(b"ca").is_ok_and(|a| a.as_float().is_ok_and(|v| (v - 0.8).abs() < 1e-6)))));
    let streams: Vec<_> = pdf.objects.values().filter_map(|o| o.as_stream().ok()).collect();
    assert!(
        streams.iter().any(|s| String::from_utf8_lossy(&s.content).contains(" c\n")),
        "native cubics survive images"
    );
    std::fs::write(std::env::temp_dir().join("w2-image-vector-fix.pdf"), bytes).unwrap();
}
#[test]
fn upright_rgb_jpeg_uses_original_dct_stream() {
    let bytes = include_bytes!("fixtures/passthrough.jpg");
    let mut ed = Editor::new();
    images::links::place_bytes(&mut ed, bytes, [0.; 2], None, PlacementMode::Embed, None).unwrap();
    let pdf = lopdf::Document::load_mem(&varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT).unwrap())
        .unwrap();
    let stream = pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .find(|s| s.dict.get(b"Filter").is_ok_and(|f| f.as_name().is_ok_and(|n| n == b"DCTDecode")))
        .unwrap();
    assert_eq!(stream.content, bytes);
}
#[test]
fn cmyk_jpeg_uses_decoded_rgb_flate_not_mislabeled_dct() {
    let bytes = include_bytes!("fixtures/cmyk-no-passthrough.jpg");
    assert!(!images::codec::jpeg_rgb_original(bytes));
    let mut ed = Editor::new();
    images::links::place_bytes(&mut ed, bytes, [0.; 2], None, PlacementMode::Embed, None).unwrap();
    let pdf = lopdf::Document::load_mem(&varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT).unwrap())
        .unwrap();
    assert!(!pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .any(|s| s.dict.get(b"Filter").is_ok_and(|f| f.as_name().is_ok_and(|n| n == b"DCTDecode"))));
}
