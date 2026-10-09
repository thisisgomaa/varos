//! Colour Stage-1 polish helpers — pure logic (allowed per the math-test rule): the recent-colours MRU
//! (newest first, deduped, capped) and the derived document-colours scan (first appearance, deduped).

use varos_core::editor::{Editor, PaintTarget, ToolKind};
use varos_core::geom::Rgba;
use varos_core::model::{Anchor, Paint, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn tri(id: u32, base: u32, fill: Option<Rgba>, stroke: Option<Rgba>) -> Path {
    Path::new(
        id,
        vec![anc(base, 0.0, 0.0), anc(base + 1, 10.0, 0.0), anc(base + 2, 0.0, 10.0)],
        true,
        fill,
        stroke,
        1.0,
    )
}

const RED: Rgba = [1.0, 0.0, 0.0, 1.0];
const GREEN: Rgba = [0.0, 1.0, 0.0, 1.0];
const BLUE: Rgba = [0.0, 0.0, 1.0, 1.0];

#[test]
fn recent_is_mru_deduped_and_capped() {
    let mut ed = Editor::new();
    ed.push_recent(RED);
    ed.push_recent(GREEN);
    ed.push_recent(RED); // re-push → moves to front, no duplicate
    assert_eq!(ed.recent_colors.len(), 2);
    assert_eq!(ed.recent_colors[0], RED, "re-pushed colour moves to the front");
    assert_eq!(ed.recent_colors[1], GREEN);
    for i in 0..20 {
        ed.push_recent([i as f32 / 20.0, 0.5, 0.5, 1.0]);
    }
    assert_eq!(ed.recent_colors.len(), 12, "MRU is capped at 12");
}

#[test]
fn direct_path_level_selection_receives_paint() {
    // The bug: with the Direct-Selection tool a PATH-LEVEL selection (dsel_path set, no individual anchor)
    // was ignored by apply_paint → changing colour / removing stroke silently did nothing.
    let mut ed = Editor::new();
    ed.doc.paths.push(tri(1, 1, Some(RED), Some(BLUE)));
    ed.doc.ids = 3;
    ed.set_tool(ToolKind::Direct);
    ed.dsel_path = Some(1); // clicking a path with the Direct tool = path-level select
    ed.paint = PaintTarget::Fill;
    ed.apply_paint(Some(GREEN));
    assert_eq!(ed.doc.paths[0].fill, Paint::Solid(GREEN), "Direct path-level select must receive the fill change");
    ed.paint = PaintTarget::Stroke;
    ed.apply_paint(None);
    assert_eq!(ed.doc.paths[0].stroke, Paint::None, "removing the stroke must work in Direct path-level mode");
}

#[test]
fn inspector_reads_the_direct_selection_colour() {
    // repr_path drives the inspector swatches; it must see the Direct path-level selection (not objsel only).
    let mut ed = Editor::new();
    ed.doc.paths.push(tri(7, 1, Some(RED), Some(BLUE)));
    ed.dsel_path = Some(7);
    let pi = ed.repr_path().expect("a Direct path-level selection is a selection");
    assert_eq!(ed.doc.paths[pi].fill, Paint::Solid(RED), "the inspector reads the selected path's real colour");
}

#[test]
fn document_colors_scan_dedupes_in_first_appearance_order() {
    let mut ed = Editor::new();
    ed.doc.paths.push(tri(1, 1, Some(RED), Some(BLUE)));
    ed.doc.paths.push(tri(2, 10, Some(RED), None)); // duplicate fill → no new entry
    ed.doc.paths.push(tri(3, 20, Some(GREEN), Some(BLUE))); // duplicate stroke → only GREEN is new
    let dc = ed.document_colors();
    assert_eq!(dc, vec![RED, BLUE, GREEN], "unique colours in first-appearance order, got {:?}", dc);
}

#[test]
fn board_document_and_recent_sources_are_distinct_first_use_and_capped_36() {
    use varos_core::model::Artboard;
    let mut ed = Editor::new();
    ed.doc.artboards = vec![
        Artboard { id: 100, x: -5., y: -5., w: 30., h: 30., ..Default::default() },
        Artboard { id: 101, x: 95., y: -5., w: 30., h: 30., ..Default::default() },
    ];
    ed.doc.paths.push(tri(1, 1, Some(RED), Some(BLUE)));
    let mut off = tri(2, 10, Some(GREEN), Some(BLUE));
    for a in &mut off.anchors {
        a.p[0] += 100.;
    }
    ed.doc.paths.push(off);
    ed.doc.paths.push(tri(3, 20, Some(RED), None));
    ed.push_recent([0.5, 0.5, 0.5, 1.]);
    assert_eq!(ed.board_colors(), vec![RED, BLUE]);
    ed.doc.active = 1;
    assert_eq!(ed.board_colors(), vec![GREEN, BLUE]);
    assert_eq!(ed.document_colors(), vec![RED, BLUE, GREEN]);
    assert_eq!(ed.recent_colors, vec![[0.5, 0.5, 0.5, 1.]]);
    ed.doc.active = 0;
    for i in 0..40 {
        ed.doc.paths.push(tri(10 + i, 100 + i * 3, Some([i as f32 / 40., 0.5, 0.5, 1.]), None));
    }
    assert_eq!(ed.board_colors().len(), 36);
    assert_eq!(ed.document_colors().len(), 36);
    assert_eq!(&ed.document_colors()[..3], &[RED, BLUE, GREEN]);
    ed.doc.artboards.clear();
    assert!(ed.board_colors().is_empty());
    assert_eq!(ed.document_colors().len(), 36);
}
