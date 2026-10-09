use std::sync::atomic::AtomicBool;
use varos_core::{format::Limits, EditCommand, Editor};
#[test]
fn text_native_reopens_editable_deliverable_is_outlines() {
    let mut ed = Editor::new();
    let t = varos_text_layout::default_text("مرحبا Varos", [20., 80.]).unwrap();
    ed.try_execute_created(EditCommand::AddText { text: t, parent: None }).unwrap();
    let native = varos_pdf::write_pdf_checked(&ed.doc, &Limits::DEFAULT).unwrap();
    let loaded = varos_pdf::load_vrs_bytes(&native, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.doc.text_boxes, ed.doc.text_boxes);
    let plan = varos_pdf::plan_pdf_export(&ed.doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
    let (pdf, report) = varos_pdf::export_pdf_bytes_with_report(&ed.doc, &plan, &AtomicBool::new(false)).unwrap();
    assert!(report.notes.iter().any(|n| n.message == "text exported as outlines"));
    assert!(!varos_pdf::has_embedded_model(&pdf));
    let out = varos_text_layout::outline_document(&ed.doc).unwrap();
    let plan2 = varos_pdf::plan_pdf_export(&out, varos_pdf::ExportScope::ArtworkBounds).unwrap();
    assert_eq!(pdf, varos_pdf::export_pdf_bytes(&out, &plan2, &AtomicBool::new(false)).unwrap());
}

#[test]
fn next_version_pdf_fixture_refuses_before_typed_decode() {
    let bytes = include_bytes!("../../varos-core/tests/fixtures/text_next/refuse_newer.pdf");
    assert!(matches!(
        varos_pdf::load_vrs_bytes(bytes, &Limits::DEFAULT),
        Err(varos_core::format::LoadError::NewerVersion { found: 7, supported: 6 })
    ));
}
