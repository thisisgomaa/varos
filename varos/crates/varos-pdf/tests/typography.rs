use std::sync::atomic::AtomicBool;
use varos_core::{format::Limits, EditCommand, Editor};
#[test]
fn arabic_ligatures_marks_and_latin_extract_from_subset() {
    let source = "سَلَام مرحبا 123 Varos";
    let mut ed = Editor::new();
    let mut t = varos_text_layout::default_text(source, [100., 100.]).unwrap();
    t.para.align = varos_core::text::Alignment::Left;
    ed.try_execute_created(EditCommand::AddText { text: t, parent: None }).unwrap();
    let native = varos_pdf::write_pdf_checked(&ed.doc, &Limits::DEFAULT).unwrap();
    let pdf = lopdf::Document::load_mem(&native).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert_eq!(extracted.trim(), source, "{extracted:?}");
    assert!(pdf.objects.values().any(|o| o.as_dict().ok().is_some_and(|d| d
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        == Some(b"CIDFontType2"))));
    let loaded = varos_pdf::load_vrs_bytes(&native, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.doc.text_boxes, ed.doc.text_boxes);
    let plan = varos_pdf::plan_pdf_export(&ed.doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
    let (out, report) = varos_pdf::export_pdf_bytes_with_report(&ed.doc, &plan, &AtomicBool::new(false)).unwrap();
    assert!(report.notes.iter().any(|n| n.kind == "text_embedded"));
    assert!(!report.notes.iter().any(|n| n.kind == "text_outlines"));
    assert_eq!(lopdf::Document::load_mem(&out).unwrap().extract_text(&[1]).unwrap().trim(), source);
    assert!(!varos_pdf::has_embedded_model(&out));
}
#[test]
fn package_fonts_have_hashes_and_licences() {
    let mut ed = Editor::new();
    ed.try_execute_created(EditCommand::AddText {
        text: varos_text_layout::default_text("سلام", [0., 60.]).unwrap(),
        parent: None,
    })
    .unwrap();
    let root = std::env::temp_dir().join(format!("lane-h-package-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    varos_pdf::package::package(&ed.doc, &Default::default(), &root).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("Fonts/manifest.json")).unwrap()).unwrap();
    assert!(!manifest.as_array().unwrap().is_empty());
    for item in manifest.as_array().unwrap() {
        let bytes = std::fs::read(root.join(item["file"].as_str().unwrap())).unwrap();
        let font = varos_text_layout::font_export::FontFace::new("Package", 400, bytes.into()).unwrap();
        assert_eq!(varos_text_layout::font_hash(font.content_hash), item["sha256"].as_str().unwrap());
    }
    let reopened = varos_pdf::load_vrs(&root.join("Document.vrs")).unwrap();
    assert_eq!(reopened.text_boxes, ed.doc.text_boxes);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_v14_pdf_reopens_and_v9_header_reader_refuses() {
    let bytes = include_bytes!("../../varos-core/tests/fixtures/v14/styles.vrs");
    let loaded = varos_pdf::load_vrs_bytes(bytes, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.source_version, 14);
    assert!(!loaded.doc.typography.is_empty());
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    assert_eq!(pdf.catalog().unwrap().get(b"VAROS_SchemaVersion").unwrap().as_i64().unwrap(), 14);
    let (_, model) = pdf.dereference(pdf.catalog().unwrap().get(b"VAROS_Model").unwrap()).unwrap();
    #[derive(serde::Deserialize)]
    struct FrozenHeader {
        varos: u32,
    }
    let header: FrozenHeader = serde_json::from_slice(&model.as_stream().unwrap().content).unwrap();
    assert!(header.varos > 9);
    assert!(matches!(
        varos_pdf::load_vrs_bytes(
            include_bytes!("../../varos-core/tests/fixtures/v14/refuse-v15.pdf"),
            &Limits::DEFAULT
        ),
        Err(varos_core::format::LoadError::NewerVersion { found: 15, supported: 14 })
    ));
}

#[test]
fn overset_source_is_not_extracted_from_deliverable_pdf() {
    let source = "Visible first line\nSecond line\nSECRET_LAST";
    let mut text = varos_text_layout::default_text(source, [0., 0.]).unwrap();
    text.box_kind = varos_core::text::TextBoxKind::Area([0., 150., 300., 48.]);
    text.para.align = varos_core::text::Alignment::Left;
    let mut ed = Editor::new();
    ed.try_execute_created(EditCommand::AddText { text, parent: None }).unwrap();
    let plan = varos_pdf::plan_pdf_export(&ed.doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
    let (bytes, report) = varos_pdf::export_pdf_bytes_with_report(&ed.doc, &plan, &AtomicBool::new(false)).unwrap();
    let extracted = lopdf::Document::load_mem(&bytes).unwrap().extract_text(&[1]).unwrap();
    assert!(extracted.contains("Visible"));
    assert!(!extracted.contains("SECRET_LAST"));
    assert!(report.notes.iter().any(|n| n.kind == "text_overset"));
    assert_eq!(ed.doc.text_boxes[0].source(), source);
}

#[test]
fn off_page_and_clipped_lines_are_not_extractable() {
    for clipped in [false, true] {
        let mut ed = Editor::new();
        ed.doc.artboards.push(varos_core::model::Artboard {
            x: 0.,
            y: 0.,
            w: 400.,
            h: if clipped { 400. } else { 80. },
            ..Default::default()
        });
        ed.doc.assign_artboard_ids();
        let mut text = varos_text_layout::default_text("VISIBLE\n\n\nOFF_PAGE_SECRET", [10., 30.]).unwrap();
        text.para.align = varos_core::text::Alignment::Left;
        let id = ed.try_execute_created(EditCommand::AddText { text, parent: None }).unwrap();
        if clipped {
            let mask = ed
                .try_execute_created(EditCommand::AddShape {
                    kind: varos_core::model::ShapeKind::Rect,
                    bounds: [0., 0., 400., 80.],
                    parent: None,
                    fill: Some([0., 0., 0., 1.]),
                    stroke: None,
                    stroke_width: 0.,
                    opacity: 1.,
                    name: None,
                })
                .unwrap();
            // Construct the valid persisted clip tree directly: the current selection
            // command still restricts masks to path/image selection units.
            let text_node = varos_core::text::node_id(&ed.doc, id).unwrap();
            let mask_node = ed.doc.node_of_path(mask).unwrap();
            let mut group = ed.doc.node(mask_node).unwrap().clone();
            let parent = group.parent.unwrap();
            group.id = ed.doc.nid();
            let gid = group.id;
            group.kind = varos_core::model::NodeKind::Group;
            group.role = varos_core::model::GroupRole::Clip;
            group.mask_child = Some(mask_node);
            group.children = vec![mask_node, text_node];
            for n in &mut ed.doc.nodes {
                if [text_node, mask_node].contains(&n.id) {
                    n.parent = Some(gid);
                }
                if n.id == parent {
                    n.children.retain(|id| ![text_node, mask_node].contains(id));
                    n.children.insert(0, gid);
                }
            }
            ed.doc.nodes.push(group);
        }
        let plan = varos_pdf::plan_pdf_export(&ed.doc, varos_pdf::ExportScope::ActiveArtboard).unwrap();
        let bytes = varos_pdf::export_pdf_bytes(&ed.doc, &plan, &AtomicBool::new(false)).unwrap();
        let extracted = lopdf::Document::load_mem(&bytes).unwrap().extract_text(&[1]).unwrap();
        assert!(extracted.contains("VISIBLE"), "{extracted:?}");
        assert!(!extracted.contains("OFF_PAGE_SECRET"), "{extracted:?}");
    }
}
