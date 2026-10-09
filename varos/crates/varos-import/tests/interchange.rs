use std::sync::{atomic::AtomicBool, Arc};
use varos_core::{
    format::{self, Limits},
    model::{GroupRole, NodeKind},
    EditCommand, Editor,
};
use varos_import::{import_file, Format, ImportOptions, LossPolicy};
fn options() -> ImportOptions {
    ImportOptions { loss_policy: LossPolicy::AllowReported, ..Default::default() }
}
// Hand-written ASCII PDF objects/xref; no writer-generated or binary fixtures.
fn pdf(content: &str, extra_catalog: &str) -> Vec<u8> {
    let objects=[format!("<< /Type /Catalog /Pages 2 0 R {extra_catalog} >>"),"<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [10 20 210 120] >>".into(),"<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /XObject << /Im1 5 0 R >> >> >>".into(),format!("<< /Length {} >>\nstream\n{content}\nendstream",content.len()),"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 3 >>\nstream\nRGB\nendstream".into()];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = vec![0];
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out += &format!("{} 0 obj\n{obj}\nendobj\n", i + 1);
    }
    let xref = out.len();
    out += &format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len());
    for offset in offsets.iter().skip(1) {
        out += &format!("{offset:010} 00000 n \n");
    }
    out += &format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len());
    out.into_bytes()
}
fn dxf(entities: &str) -> Vec<u8> {
    format!(
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    )
    .into_bytes()
}
#[test]
fn pdf_paths_paints_page_transform_and_stroke_state() {
    let input = pdf("q 1 0 0 1 5 0 cm 1 0 0 rg 0 0 1 RG 2 w 2 J 2 j 10 20 20 30 re B Q", "");
    let (doc, report) = import_file(&input, Format::Pdf, ImportOptions::default()).unwrap();
    assert!(report.loss_notes.is_empty());
    assert_eq!((doc.artboards[0].w, doc.artboards[0].h), (200., 100.));
    let p = &doc.paths[1];
    assert_eq!(p.anchors[0].p, [5., 100.]);
    assert_eq!(p.fill.solid(), Some([1., 0., 0., 1.]));
    assert_eq!(p.stroke.solid(), Some([0., 0., 1., 1.]));
    assert_eq!(p.stroke_width, 2.);
    assert_eq!(p.stroke_style.cap, varos_core::stroke::StrokeCap::Square);
    assert_eq!(p.stroke_style.join, varos_core::stroke::StrokeJoin::Bevel);
    assert!(doc.nodes.iter().any(|n| n.role == GroupRole::Clip));
    let encoded = format::encode_model(&doc, &Limits::DEFAULT).unwrap();
    let loaded = format::decode_model(encoded.as_bytes(), None, &Limits::DEFAULT).unwrap();
    assert_eq!(doc, loaded.doc);
}
#[test]
fn pdf_clip_cubic_dash_and_loss_notes() {
    let input =
        pdf("q 10 20 30 40 re W n [2 3] 1 d 10 20 m 20 30 40 50 60 70 c S Q BT /F1 10 Tf (hello) Tj ET /Im1 Do", "");
    assert!(import_file(&input, Format::Pdf, ImportOptions::default()).unwrap_err().contains("explicit acceptance"));
    let (doc, report) = import_file(&input, Format::Pdf, options()).unwrap();
    assert_eq!(report.loss_notes.len(), 2);
    assert!(report.loss_notes.iter().any(|n| n.contains("text omitted")));
    assert!(report.loss_notes.iter().any(|n| n.contains("images omitted")));
    assert!(doc.paths.last().unwrap().anchors[0].hout.is_some());
    assert_eq!(doc.paths.last().unwrap().stroke_style.dash, vec![2., 3.]);
    assert_eq!(doc.nodes.iter().filter(|n| n.role == GroupRole::Clip).count(), 2);
}
#[test]
fn pdf_ai_native_firewall_and_unsupported_appearance() {
    let basic = pdf("10 20 30 40 re f", "");
    let (_, report) = import_file(&basic, Format::Ai, options()).unwrap();
    assert!(report.loss_notes[0].contains("AI private"));
    assert!(import_file(b"%!PS-Adobe", Format::Ai, options()).is_err());
    let native = pdf("10 20 30 40 re f", "/VAROS_SchemaVersion 99");
    assert!(import_file(&native, Format::Pdf, options()).unwrap_err().contains("native reader"));
    for content in [
        "0 1 0 0 k 10 20 30 40 re f",
        "/G1 gs 10 20 30 40 re f",
        "Q",
        "10 20 30 40 re f /Unknown Do",
        "10 20 30 40 re f 1 Tr",
    ] {
        assert!(import_file(&pdf(content, ""), Format::Pdf, options()).is_err(), "{content}");
    }
    assert!(Format::from_extension("vrs").is_err());
    assert!(import_file(b"", Format::Dwg, options()).unwrap_err().contains("DWG"));
}
#[test]
fn dxf_units_layers_bulges_arcs_and_legacy_polylines() {
    let line = "0\nLINE\n8\nInk\n62\n1\n10\n0\n20\n0\n11\n25.4\n21\n25.4\n";
    let arc = "0\nARC\n8\nCurves\n10\n0\n20\n0\n40\n10\n50\n0\n51\n90\n";
    let poly = "0\nLWPOLYLINE\n70\n1\n90\n3\n10\n0\n20\n0\n42\n1\n10\n10\n20\n0\n10\n5\n20\n10\n";
    let old = "0\nPOLYLINE\n70\n0\n0\nVERTEX\n10\n0\n20\n0\n0\nVERTEX\n10\n1\n20\n1\n0\nSEQEND\n";
    let bytes = dxf(&format!("{line}{arc}{poly}{old}"));
    assert!(import_file(&bytes, Format::Dxf, ImportOptions::default()).is_err());
    let (doc, report) = import_file(&bytes, Format::Dxf, options()).unwrap();
    assert_eq!(report.paths, 4);
    assert_eq!(report.loss_notes.len(), 2);
    let line = doc.paths.iter().find(|p| p.stroke.solid() == Some([1., 0., 0., 1.])).unwrap();
    assert!((line.anchors[1].p[0] - 72.).abs() < 0.001);
    assert!((line.anchors[1].p[1] + 72.).abs() < 0.001);
    assert!(doc.nodes.iter().any(|n| n.kind == NodeKind::Layer && n.name == "Curves"));
    assert!(doc.paths.iter().any(|p| p.closed && p.anchors.iter().any(|a| a.hout.is_some())));
    let mut ed = Editor::new();
    ed.execute_ui(EditCommand::PlaceArtwork(Box::new(doc.clone())));
    assert_eq!(ed.doc.paths.len(), 4);
    assert!(ed.doc.nodes.iter().any(|n| n.kind == NodeKind::Group && n.name == "Curves"));
    ed.execute_ui(EditCommand::Undo);
    assert!(ed.doc.paths.is_empty());
    ed.execute_ui(EditCommand::Redo);
    assert_eq!(ed.doc.paths.len(), 4);
}
#[test]
fn dxf_cubic_spline_exact_controls_and_explicit_units() {
    let spline="0\nSPLINE\n71\n3\n40\n0\n40\n0\n40\n0\n40\n0\n40\n1\n40\n1\n40\n1\n40\n1\n10\n0\n20\n0\n10\n1\n20\n2\n10\n2\n20\n2\n10\n3\n20\n0\n";
    let (doc, report) = import_file(&dxf(spline), Format::Dxf, options()).unwrap();
    assert!(report.loss_notes[0].contains("spline programme"));
    assert_eq!(doc.paths[0].anchors.len(), 2);
    let unitless = String::from_utf8(dxf(spline)).unwrap().replace("$INSUNITS\n70\n4", "$INSUNITS\n70\n0");
    assert!(import_file(unitless.as_bytes(), Format::Dxf, options()).unwrap_err().contains("explicit points_per_unit"));
    let (doc, _) =
        import_file(unitless.as_bytes(), Format::Dxf, ImportOptions { points_per_unit: Some(1.), ..options() })
            .unwrap();
    assert_eq!(doc.paths[0].anchors[0].hout, Some([1., -2.]));
    for entity in ["0\nTEXT\n1\nhi\n", "0\nLINE\n10\nNaN\n20\n0\n11\n1\n21\n1\n", "0\nINSERT\n2\nblock\n"] {
        assert!(import_file(&dxf(entity), Format::Dxf, options()).is_err());
    }
}
#[test]
fn cancellation_limits_and_no_preferred_flavour_fallback() {
    let bytes = pdf("10 20 30 40 re f", "");
    assert!(varos_import::import_cancellable(&bytes, Format::Pdf, options(), Arc::new(AtomicBool::new(true)))
        .unwrap_err()
        .contains("cancelled"));
    let snapshot = varos_import::clipboard::Snapshot {
        generation: 1,
        flavours: vec![("public.svg-image".into(), b"bad SVG".to_vec()), ("com.adobe.pdf".into(), bytes)],
    };
    assert!(varos_import::clipboard::stage(&snapshot, options()).is_err());
    let png = varos_import::clipboard::Snapshot { generation: 1, flavours: vec![("public.png".into(), vec![1])] };
    assert!(varos_import::clipboard::stage(&png, options()).unwrap_err().contains("image/blob"));
    assert!(import_file(&vec![0; varos_import::MAX_BYTES + 1], Format::Pdf, options()).is_err());
}
