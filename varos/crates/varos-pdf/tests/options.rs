use std::sync::atomic::AtomicBool;
use varos_core::model::{Artboard, Document};
use varos_pdf::*;
fn fixture() -> (Document, ExportPlan) {
    let mut doc = Document::default();
    doc.artboards.push(Artboard { w: 200., h: 100., bleed: 3., ..Default::default() });
    let plan = plan_pdf_export(&doc, ExportScope::AllVisibleArtboards).unwrap();
    (doc, plan)
}
#[test]
fn default_options_are_byte_identical_to_the_historic_writer() {
    let (doc, plan) = fixture();
    let cancel = AtomicBool::new(false);
    let (bytes, report) = export_pdf_with_options(&doc, &plan, &PdfOptions::default(), &cancel).unwrap();
    assert_eq!(bytes, export_pdf_bytes(&doc, &plan, &cancel).unwrap());
    assert_eq!(report.notes[0].kind, "no_raster_content");
}
#[test]
fn press_boxes_marks_and_compression_have_golden_geometry() {
    let (doc, plan) = fixture();
    let mut options = PdfOptions::preset(PdfPreset::Press);
    options.marks = PdfMarks { crop: true, registration: true, page_info: true, ..Default::default() };
    let (bytes, _) = export_pdf_with_options(&doc, &plan, &options, &AtomicBool::new(false)).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let page = pdf.get_object(pdf.get_pages()[&1]).unwrap().as_dict().unwrap();
    let mut golden = String::new();
    for key in ["MediaBox", "BleedBox", "TrimBox"] {
        let values = page
            .get(key.as_bytes())
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap().to_string())
            .collect::<Vec<_>>();
        golden.push_str(&format!("{key}: {}\n", values.join(" ")));
    }
    assert_eq!(golden, include_str!("fixtures/options/boxes.txt"));
    let content = String::from_utf8(pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    assert!(content.contains("0 0 0 1 K 0.25 w"));
    assert!(content.contains("27 3 m 27 21 l S"));
    let start = content.find("q 0 0 0 1 K").expect("marks stream");
    assert_eq!(content[start..].trim_end(), include_str!("fixtures/options/marks.txt").trim_end());
    assert!(pdf.objects.values().any(|v| v.as_stream().is_ok_and(|s| s.dict.has(b"Filter"))));
}
#[test]
fn preset_only_input_matches_cli_preset_and_invalid_options_refuse() {
    let press: PdfOptions = serde_json::from_str(r#"{"preset":"press"}"#).unwrap();
    assert_eq!(press, PdfOptions::preset(PdfPreset::Press));
    for bad in [
        r#"{"image_ppi":0}"#,
        r#"{"marks":{"colour_bars":true}}"#,
        r#"{"boxes":{"bleed_override":-1}}"#,
        r#"{"unknown":true}"#,
    ] {
        assert!(serde_json::from_str::<PdfOptions>(bad).is_err());
    }
    let (doc, plan) = fixture();
    assert!(export_pdf_with_options(&doc, &plan, &press, &AtomicBool::new(true)).is_err());
}

#[test]
fn coincident_boards_keep_their_own_bleed_for_all_and_active_scopes() {
    let (mut doc, _) = fixture();
    doc.artboards.push(Artboard { bleed: 9.0, ..doc.artboards[0].clone() });
    doc.active = 1;
    let options = PdfOptions::preset(PdfPreset::Press);
    for (scope, expected) in
        [(ExportScope::AllVisibleArtboards, vec![3.0, 9.0]), (ExportScope::ActiveArtboard, vec![9.0])]
    {
        let plan = plan_pdf_export(&doc, scope).unwrap();
        assert_eq!(plan.pages.iter().map(|p| p.bleed).collect::<Vec<_>>(), expected);
        let (bytes, _) = export_pdf_with_options(&doc, &plan, &options, &AtomicBool::new(false)).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        for ((_, id), bleed) in pdf.get_pages().into_iter().zip(expected) {
            let page = pdf.get_object(id).unwrap().as_dict().unwrap();
            let values = |key: &[u8]| {
                page.get(key).unwrap().as_array().unwrap().iter().map(|v| v.as_float().unwrap()).collect::<Vec<_>>()
            };
            assert_eq!(values(b"TrimBox"), [bleed, bleed, 200.0 + bleed, 100.0 + bleed]);
            assert_eq!(values(b"BleedBox"), [0.0, 0.0, 200.0 + 2.0 * bleed, 100.0 + 2.0 * bleed]);
        }
    }
}
#[test]
fn options_preserve_writer_diagnostics_and_append_ppi_note() {
    let (doc, plan) = fixture();
    let cancel = AtomicBool::new(false);
    for options in [PdfOptions::default(), PdfOptions::preset(PdfPreset::Print)] {
        let (_, base) = export_pdf_bytes_with_report(&doc, &plan, &cancel).unwrap();
        let (_, report) = export_pdf_with_options(&doc, &plan, &options, &cancel).unwrap();
        assert_eq!(&report.notes[..base.notes.len()], base.notes.as_slice());
        assert_eq!(report.notes.len(), base.notes.len() + 1);
        assert_eq!(report.notes.last().unwrap().kind, "no_raster_content");
    }
}

#[test]
fn document_setup_asymmetric_bleed_drives_pdf_boxes_and_crop_marks() {
    let (mut doc, _) = fixture();
    doc.artboards[0].bleed_edges = Some([1.0, 2.0, 3.0, 4.0]);
    let plan = plan_pdf_export(&doc, ExportScope::AllVisibleArtboards).unwrap();
    let mut options = PdfOptions::preset(PdfPreset::Press);
    options.marks.crop = true;
    let (bytes, _) = export_pdf_with_options(&doc, &plan, &options, &AtomicBool::new(false)).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let id = pdf.get_pages()[&1];
    let page = pdf.get_object(id).unwrap().as_dict().unwrap();
    for (key, expected) in [
        (b"MediaBox".as_slice(), [0., 0., 254., 152.]),
        (b"BleedBox".as_slice(), [24., 24., 230., 128.]),
        (b"TrimBox".as_slice(), [28., 27., 228., 127.]),
    ] {
        let actual =
            page.get(key).unwrap().as_array().unwrap().iter().map(|v| v.as_float().unwrap()).collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
    let content = String::from_utf8(pdf.get_page_content(id).unwrap()).unwrap();
    assert!(content.contains("28 3 m 28 21 l S"));
    assert!(content.contains("3 27 m 21 27 l S"));
}
