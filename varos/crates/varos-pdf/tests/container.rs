//! The .vrs PDF container, headless: the file IS a PDF (any viewer's sniff), the embedded model
//! round-trips byte-perfect, one page per artboard, and legacy raw-JSON .vrs files still open.

use varos_core::model::{Anchor, Artboard, Document, Path};
use varos_pdf::{load_vrs, save_vrs, write_pdf};

fn anc(id: u32, x: f32, y: f32) -> Anchor { Anchor { id, p: [x, y], hin: None, hout: None, smooth: false } }
fn demo_doc() -> Document {
    let mut d = Document::default();
    d.artboards = vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, ..Default::default() }];
    d.paths.push(Path::new(1, vec![anc(1, 40.0, 40.0), anc(2, 200.0, 60.0), anc(3, 120.0, 220.0)],
                           true, Some([0.2, 0.7, 0.3, 1.0]), Some([0.0, 0.0, 0.0, 0.5]), 6.0)); // knockout case
    d.paths.push(Path::new(2, vec![anc(4, 250.0, 50.0), anc(5, 350.0, 250.0)],
                           false, None, Some([0.9, 0.2, 0.2, 1.0]), 3.0));                      // open stroke
    d.paths[0].opacity = 0.8;
    d.ids = 5;
    d
}
fn tmp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("varos-pdf-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn the_file_is_a_real_pdf_with_the_model_inside() {
    let bytes = write_pdf(&demo_doc()).expect("writes");
    assert!(bytes.starts_with(b"%PDF-"), "any viewer must sniff this as a PDF");
    let s = String::from_utf8_lossy(&bytes);
    assert!(s.contains("VAROS_Model"), "private catalog key present");
    assert!(s.contains("EmbeddedFiles"), "name-tree home present");
    assert!(s.contains("AF"), "associated-files array present");
}

#[test]
fn container_round_trips_the_document_identically() {
    let doc = demo_doc();
    let p = tmp("roundtrip.vrs");
    save_vrs(&doc, &p).expect("save");
    let loaded = load_vrs(&p).expect("load");
    assert_eq!(loaded, doc, "the model recovered from inside the PDF equals the saved one");
    // and the very same file is a PDF on disk
    let raw = std::fs::read(&p).unwrap();
    assert!(raw.starts_with(b"%PDF-"));
    let _ = std::fs::remove_file(&p);
}

#[test]
fn one_page_per_artboard() {
    let mut doc = demo_doc();
    doc.artboards.push(Artboard { x: 500.0, y: 0.0, w: 200.0, h: 200.0, ..Default::default() });
    doc.artboards.push(Artboard { x: 0.0, y: 400.0, w: 300.0, h: 100.0, ..Default::default() });
    let bytes = write_pdf(&doc).expect("writes");
    let pdf = lopdf::Document::load_mem(&bytes).expect("lopdf parses our output");
    assert_eq!(pdf.get_pages().len(), 3, "3 artboards → 3 pages");
}

#[test]
fn groups_survive_the_container_round_trip() {
    // Ahmed's report: after reopening a file, a group moves disintegrated. Groups live in
    // Document.groups + group_of (HashMap<u32,u32> — integer keys through JSON are the suspect).
    let mut d = demo_doc();
    d.paths.push(Path::new(3, vec![anc(6, 10.0, 10.0), anc(7, 20.0, 10.0), anc(8, 15.0, 20.0)],
                           true, Some([0.5, 0.5, 0.5, 1.0]), None, 1.0));
    d.ids = 8;
    d.groups.push(varos_core::model::Group { id: 100, name: "G".into(), parent: None });
    d.group_of.insert(1, 100);
    d.group_of.insert(3, 100);
    let p = tmp("groups.vrs");
    save_vrs(&d, &p).expect("save");
    let loaded = load_vrs(&p).expect("load");
    assert_eq!(loaded.group_of, d.group_of, "group membership must survive save/reopen");
    assert_eq!(loaded.groups.len(), 1, "the group registry must survive save/reopen");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn legacy_raw_json_vrs_still_opens() {
    let doc = demo_doc();
    let p = tmp("legacy.vrs");
    varos_core::file::save_vrs(&doc, &p).expect("legacy JSON writer");   // the first slice's format
    let loaded = load_vrs(&p).expect("the PDF loader sniffs and falls back to JSON");
    assert_eq!(loaded, doc);
    let _ = std::fs::remove_file(&p);
}
