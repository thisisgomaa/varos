//! FB1 — an OPEN filled shape (a shape a deleted anchor opened, A32) must FILL in the exported PDF,
//! exactly as it fills on the canvas (`scene::fill_prims`). WYSIWYG: the .ai-style PDF and the screen
//! must agree — the old `p.closed` fill guard silently dropped an opened shape's fill on export.
//!
//! Run with:  cargo test -p varos-pdf --test open_fill

use varos_core::model::{Anchor, Artboard, Document, Path};
use varos_pdf::write_pdf;

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn one_path(p: Path) -> Document {
    // page_color None → the page emits NO background fill, so the only fill op on the stream can come
    // from the artwork itself. That makes "does the open shape fill?" a decisive check.
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, page_color: None, ..Default::default() }],
        ..Default::default()
    };
    d.ids = 100;
    d.paths.push(p);
    d.sync_tree();
    d
}
/// The first page's decoded content-stream operators.
fn page_ops(bytes: &[u8]) -> Vec<String> {
    let pdf = lopdf::Document::load_mem(bytes).expect("valid pdf");
    let (_, &pid) = pdf.get_pages().iter().next().unwrap();
    let content = pdf.get_page_content(pid).unwrap();
    lopdf::content::Content::decode(&content).unwrap().operations.into_iter().map(|op| op.operator).collect()
}
fn fills(ops: &[String]) -> bool {
    ops.iter().any(|o| o == "f" || o == "f*")
}

#[test]
fn an_open_filled_shape_fills_in_the_pdf() {
    // a triangle, OPEN (as if a corner was deleted), fill only — must still fill the page
    let tri = Path::new(
        1,
        vec![anc(1, 40.0, 40.0), anc(2, 200.0, 60.0), anc(3, 120.0, 220.0)],
        false,
        Some([0.2, 0.7, 0.3, 1.0]),
        None,
        1.0,
    );
    assert!(fills(&page_ops(&write_pdf(&one_path(tri)).unwrap())), "the open shape's fill must reach the page (FB1)");
}

#[test]
fn a_fill_less_open_line_still_never_fills() {
    // guard against over-filling: a bare open line (no fill colour) emits a stroke, never a fill —
    // this also proves the page background contributes no fill, so the test above is decisive.
    let line = Path::new(
        1,
        vec![anc(1, 40.0, 40.0), anc(2, 200.0, 60.0), anc(3, 120.0, 220.0)],
        false,
        None,
        Some([0.9, 0.2, 0.2, 1.0]),
        3.0,
    );
    let ops = page_ops(&write_pdf(&one_path(line)).unwrap());
    assert!(!fills(&ops), "a fill-less open path must not fill, got {ops:?}");
    assert!(ops.iter().any(|o| o == "S"), "…but it must still stroke, got {ops:?}");
}
