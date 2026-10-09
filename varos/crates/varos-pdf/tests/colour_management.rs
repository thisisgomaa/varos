use std::sync::atomic::AtomicBool;
use varos_core::{
    colour_management::*,
    format::{decode_model, Limits},
    model::Paint,
};
fn export(doc: &varos_core::model::Document) -> (Vec<u8>, varos_core::ExportReport) {
    let plan = varos_pdf::plan_pdf_export(doc, varos_pdf::default_scope(doc)).unwrap();
    varos_pdf::export_pdf_bytes_with_report(doc, &plan, &AtomicBool::new(false)).unwrap()
}
#[test]
fn process_gray_and_separation_golden_operators() {
    let mut doc =
        decode_model(include_bytes!("../../varos-core/tests/fixtures/v12/colours.json"), None, &Limits::DEFAULT)
            .unwrap()
            .doc;
    for (colour, space, operator) in [
        (Colour::Cmyk { c: 0.2, m: 0.3, y: 0.4, k: 0.1 }, "DeviceCMYK", include_str!("fixtures/v12/process.txt")),
        (Colour::Gray { value: 0.4 }, "DeviceGray", include_str!("fixtures/v12/gray.txt")),
        (
            Colour::Spot { name: "Brand ink".into(), tint: 0.7, alt: Cmyk { c: 0.1, m: 0.9, y: 0., k: 0.1 } },
            "Separation",
            include_str!("fixtures/v12/spot.txt"),
        ),
    ] {
        doc.paths[0].fill = Paint::Managed(ManagedColour { colour, alpha: 1. });
        let (bytes, report) = export(&doc);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains(space), "{space}");
        assert!(text.contains(operator), "{operator}");
        assert!(!report.notes.is_empty());
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        for id in pdf.get_pages().values() {
            pdf.get_page_content(*id).unwrap();
        }
    }
}
#[test]
fn iccbased_output_intent_and_rgb_byte_identity() {
    let old = include_bytes!("../../varos-core/tests/fixtures/v5/plain.json");
    let mut doc = decode_model(old, None, &Limits::DEFAULT).unwrap().doc;
    let rgb = varos_pdf::write_pdf(&doc).unwrap();
    let old = lopdf::Document::load_mem(include_bytes!("../../varos-core/tests/fixtures/v5/plain.pdf")).unwrap();
    let new = lopdf::Document::load_mem(&rgb).unwrap();
    for (a, b) in old.get_pages().values().zip(new.get_pages().values()) {
        assert_eq!(old.get_page_content(*a).unwrap(), new.get_page_content(*b).unwrap());
    }
    doc.output_profile =
        Some(IccProfile::new("sRGB".into(), &moxcms::ColorProfile::new_srgb().encode().unwrap()).unwrap());
    doc.paths[0].fill = Paint::Managed(ManagedColour { colour: Colour::Rgb { r: 0.2, g: 0.3, b: 0.4 }, alpha: 1. });
    let (bytes, _) = export(&doc);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/ICCBased") && text.contains("/OutputIntents") && text.contains("/ICC3 cs"));
}
#[test]
fn pdfx4_requires_printer_profile_and_emits_metadata_and_trimbox() {
    let mut doc = decode_model(include_bytes!("../../varos-core/tests/fixtures/v5/plain.json"), None, &Limits::DEFAULT)
        .unwrap()
        .doc;
    let plan = varos_pdf::plan_pdf_export(&doc, varos_pdf::default_scope(&doc)).unwrap();
    let options = varos_pdf::PdfOptions::preset(varos_pdf::PdfPreset::PdfX4);
    assert!(varos_pdf::export_pdf_with_options(&doc, &plan, &options, &AtomicBool::new(false)).is_err());
    let mut bytes = moxcms::ColorProfile::new_srgb().encode().unwrap();
    // Synthetic matrix printer profile: no third-party profile assets.
    bytes[12..16].copy_from_slice(b"prtr");
    doc.output_profile = Some(IccProfile::new("Test printer RGB".into(), &bytes).unwrap());
    let (bytes, report) = varos_pdf::export_pdf_with_options(&doc, &plan, &options, &AtomicBool::new(false)).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.version, "1.6");
    assert!(pdf.trailer.has(b"ID"));
    let info = pdf.trailer.get(b"Info").unwrap().as_reference().unwrap();
    assert!(pdf.get_object(info).unwrap().as_dict().unwrap().has(b"CreationDate"));
    assert!(pdf.catalog().unwrap().has(b"Metadata") && pdf.catalog().unwrap().has(b"OutputIntents"));
    for id in pdf.get_pages().values() {
        assert!(pdf.get_object(*id).unwrap().as_dict().unwrap().has(b"TrimBox"));
    }
    assert!(report.notes.iter().any(|n| n.kind == "pdfx4"));
}
