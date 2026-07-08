//! Ruler-guide logic — pure math, no UI (allowed per the math-test rule). Locks the wiring that can
//! silently break: objects snapping onto guides, grabbing a guide, and a dragged-out guide snapping.

use varos_core::editor::Editor;
use varos_core::model::{Anchor, Artboard, Guide, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn rect(id: u32, x: f32, y: f32, w: f32, h: f32) -> Path {
    Path::new(
        id,
        vec![anc(1, x, y), anc(2, x + w, y), anc(3, x + w, y + h), anc(4, x, y + h)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

#[test]
fn object_snaps_to_a_vertical_guide() {
    let mut ed = Editor::new();
    ed.doc.paths.push(rect(10, 0.0, 0.0, 50.0, 50.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.doc.guides.push(Guide { vertical: true, pos: 200.0 });
    ed.objsel.insert(10); // moving object excludes its own edges from targets
                          // push the left edge to x=197 — 3 from the guide (200) → snaps onto it
    let (nd, _g, _) = ed.snap_move((0.0, 0.0, 50.0, 50.0), [197.0, 0.0]);
    assert!((nd[0] - 200.0).abs() < 0.01, "left edge should snap onto the guide at x=200, got dx={}", nd[0]);
}

#[test]
fn guide_hit_test() {
    let mut ed = Editor::new();
    ed.ppu = 1.0;
    ed.doc.guides.push(Guide { vertical: true, pos: 100.0 });
    assert_eq!(ed.guide_at([103.0, 9999.0]), Some(0), "within grab tolerance on the X axis (guide is infinite in Y)");
    assert_eq!(ed.guide_at([150.0, 50.0]), None, "too far");
    // hidden / locked → not grabbable
    ed.guides_hidden = true;
    assert_eq!(ed.guide_at([100.0, 50.0]), None);
}

#[test]
fn dragged_out_guide_snaps_to_the_page_edge() {
    let mut ed = Editor::new();
    ed.doc.artboards = vec![Artboard::default()]; // a 1080² page at the origin (left edge x=0) to snap to
    ed.ppu = 1.0;
    ed.set_guide_preview(true, [5.0, 0.0]); // vertical guide pulled to x=5, 5 from the page edge → snaps to 0
    assert!((ed.guide_preview.unwrap().pos - 0.0).abs() < 0.01, "a guide near the page edge should snap to it");
}
