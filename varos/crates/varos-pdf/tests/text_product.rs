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
    let bytes = include_bytes!("../../varos-core/tests/fixtures/text_next/refuse_newer_v11.pdf");
    assert!(matches!(
        varos_pdf::load_vrs_bytes(bytes, &Limits::DEFAULT),
        Err(varos_core::format::LoadError::NewerVersion { found: 11, supported: varos_core::format::FORMAT_VERSION })
    ));
}

// Frozen adaptation of b3d39ee core/format/mod.rs::peek_version's newer-version gate.
// Deserialize only the envelope header: this is the v5 logic, not an old binary run.
fn frozen_v5_gate(bytes: &[u8]) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Head {
        varos: u32,
    }
    let head: Head = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if head.varos > 5 {
        return Err(format!("This file needs a newer Varos. It uses file format {}; this build supports up to 5. Update Varos to open it. The file has not been changed.", head.varos));
    }
    Ok(())
}
#[test]
fn frozen_v5_reader_refuses_text_json_and_native_pdf_before_document_decode() {
    let json = include_bytes!("../../varos-core/tests/fixtures/text_next/mixed.json");
    assert!(frozen_v5_gate(json).unwrap_err().contains("newer Varos"));
    // Even an invalid document must take the version-refusal path first.
    assert!(frozen_v5_gate(br#"{"varos":6,"doc":"not a document"}"#).unwrap_err().contains("newer Varos"));
    let doc = varos_core::format::decode_model(json, None, &Limits::DEFAULT).unwrap().doc;
    let bytes = varos_pdf::write_pdf_checked(&doc, &Limits::DEFAULT).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let model = pdf.catalog().unwrap().get(b"VAROS_Model").unwrap();
    let (_, model) = pdf.dereference(model).unwrap();
    assert!(frozen_v5_gate(&model.as_stream().unwrap().content).unwrap_err().contains("newer Varos"));
}

#[test]
fn object_clipboard_keeps_source_and_exports_only_selected_text() {
    let mut ed = Editor::new();
    let id = ed
        .try_execute_created(EditCommand::AddText {
            text: varos_text_layout::default_text("ABC", [20., 80.]).unwrap(),
            parent: None,
        })
        .unwrap();
    ed.try_execute_created(EditCommand::AddText {
        text: varos_text_layout::default_text("UNSELECTED", [2000., 80.]).unwrap(),
        parent: None,
    })
    .unwrap();
    ed.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
    let clipboard = ed.capture_selection_clipboard(false);
    let vectors = varos_pdf::clipboard_vectors(&ed.doc, &clipboard).unwrap();
    assert!(String::from_utf8(vectors.internal).unwrap().contains("ABC"));
    assert!(vectors.rect[2] < 500.);
    assert!(!vectors.document.paths.is_empty());
    assert!(vectors.document.text_boxes.is_empty());
    assert!(vectors.pdf.starts_with(b"%PDF"));
    assert!(String::from_utf8(vectors.svg).unwrap().contains("<path"));
}
