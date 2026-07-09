//! A7 Stage 6 — export parity: a rotated object's LIVE transform must reach the page (the drawn
//! coordinates are rotated) AND the embedded editable model must preserve the rotation across save/load.

use varos_core::model::{Anchor, Artboard, Document, Path, Xform};
use varos_pdf::{load_vrs, save_vrs, write_pdf};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
/// A 100×40 rect on a 400×400 board — non-square, so a 90° spin visibly changes the drawn coordinates.
fn rect_doc() -> Document {
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 400.0, ..Default::default() }],
        ..Default::default()
    };
    d.paths.push(Path::new(
        1,
        vec![anc(1, 100.0, 100.0), anc(2, 200.0, 100.0), anc(3, 200.0, 140.0), anc(4, 100.0, 140.0)],
        true,
        Some([0.2, 0.6, 0.9, 1.0]),
        None,
        1.0,
    ));
    d.ids = 4;
    d.sync_tree(); // materialise the leaf node so the unit transform can be attached
    d
}
fn rotate_unit_90(d: &mut Document) {
    let unit = d.unit_of(1).expect("the rect's unit");
    // 90° about the rect centre [150,120]
    d.set_node_xform(unit, Xform { rot: std::f32::consts::FRAC_PI_2, piv: [150.0, 120.0] });
}
fn page_content(bytes: &[u8]) -> Vec<u8> {
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    let (_, &pid) = pdf.get_pages().iter().next().unwrap();
    pdf.get_page_content(pid).unwrap()
}

#[test]
fn a_rotated_object_exports_rotated_coordinates() {
    let flat = rect_doc();
    let mut spun = rect_doc();
    rotate_unit_90(&mut spun);

    let s_flat = page_content(&write_pdf(&flat).unwrap());
    let s_spun = page_content(&write_pdf(&spun).unwrap());
    assert_ne!(s_flat, s_spun, "the exported page stream must reflect the live rotation (not the local geometry)");

    // concretely: the un-rotated rect draws its top-left [100,100] → page (100, 300); after a 90° spin about
    // [150,120] that corner is at world [170,70] → page (170, 330). The rotated coordinate must appear.
    let txt = String::from_utf8_lossy(&s_spun);
    assert!(txt.contains("170") && txt.contains("330"), "rotated corner coordinate present in the page stream");
}

#[test]
fn the_embedded_model_preserves_the_rotation() {
    let mut spun = rect_doc();
    rotate_unit_90(&mut spun);
    let p = std::env::temp_dir().join(format!("varos-rot-{}.vrs", std::process::id()));
    let _ = std::fs::remove_file(&p);
    save_vrs(&spun, &p).expect("save");
    let loaded = load_vrs(&p).expect("load");
    assert_eq!(loaded, spun, "the rotated model round-trips through the PDF container byte-for-byte");
    let xf = loaded.unit_xform(1);
    assert!((xf.rot - std::f32::consts::FRAC_PI_2).abs() < 1e-6, "the live rotation survives save/reload");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn a_rotated_object_is_not_culled_from_its_page() {
    // the cull box must be the WORLD (rotated) extent — a spin that pushes geometry around must not make
    // the object vanish. The rotated rect still overlaps the board, so its draw ops stay on the page.
    let mut spun = rect_doc();
    rotate_unit_90(&mut spun);
    let s = page_content(&write_pdf(&spun).unwrap());
    // a fill draw op (`f*`) proves the path survived cull and was emitted.
    assert!(String::from_utf8_lossy(&s).contains("f*"), "the rotated fill reaches the page (not culled)");
}
