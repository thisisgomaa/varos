use varos_core::{text::*, EditCommand, Editor};
use varos_text_layout::{default_text, edit::EditSession, TextLayout};
#[test]
fn arabic_joins_mixed_source_and_rtl_carets() {
    let mut e = TextLayout::bundled().unwrap();
    let t = default_text("سلام", [0., 0.]).unwrap();
    let c = e.compose(&t, 1.).unwrap().clone();
    assert!(c.layout.lines[0].rtl);
    assert!(!c.paths.is_empty());
    let mut s = EditSession::new(t);
    s.select_all();
    assert_eq!(s.copy(), "سلام");
    s.insert("مرحبا Varos ١٢٣").unwrap();
    let mixed = e.compose(&s.draft, 1.).unwrap();
    assert_eq!(mixed.layout.source, "مرحبا Varos ١٢٣");
    assert!(mixed.layout.lines[0].glyphs.iter().any(|g| g.level % 2 == 0));
    assert!(mixed.layout.lines[0].glyphs.iter().any(|g| g.level % 2 == 1));
    let mut before = default_text("س", [0., 0.]).unwrap();
    let isolated = e.compose(&before, 1.).unwrap().layout.lines[0].glyphs[0].id;
    before.runs[0].text = "سس".into();
    assert_ne!(e.compose(&before, 1.).unwrap().layout.lines[0].glyphs[0].id, isolated);
    let start = c.layout.carets.iter().find(|c| c.byte == 0).unwrap();
    let end = c.layout.carets.iter().find(|c| c.byte == "سلام".len()).unwrap();
    assert!(start.x > end.x);
}
#[test]
fn cache_zoom_and_missing_snapshot() {
    let mut e = TextLayout::bundled().unwrap();
    let mut t = default_text("نص", [10., 50.]).unwrap();
    e.compose(&t, 1.).unwrap();
    let count = e.layouts;
    e.compose(&t, 1.).unwrap();
    assert_eq!(e.layouts, count);
    e.compose(&t, 2.).unwrap();
    assert_eq!(e.layouts, count + 1);
    t.runs[0].style.font.hash = "0".repeat(64);
    assert!(e.compose(&t, 1.).unwrap_err().contains("missing font snapshot"));
}
#[test]
fn justification_at_several_widths_preserves_source() {
    let mut e = TextLayout::bundled().unwrap();
    let mut t = default_text("اللغة العربية جميلة ومتنوعة في كل مكان والكتابة العربية فن جميل", [0., 0.]).unwrap();
    t.para.align = Alignment::Justify;
    t.para.kashida = Kashida::Balanced;
    for width in [160., 240., 360.] {
        t.box_kind = TextBoxKind::Area([0., 0., width, 500.]);
        let c = e.compose(&t, 1.).unwrap();
        assert_eq!(c.layout.source, t.source());
        assert!(c.layout.lines.len() > 1);
        assert!(c.layout.carets.iter().all(|caret| t.source().is_char_boundary(caret.byte)));
        assert!(c.layout.lines.iter().all(|l| l.width.is_finite()));
    }
}
#[test]
fn editing_preserves_runs_clusters_and_undo_batch() {
    let mut e = TextLayout::bundled().unwrap();
    let t = default_text("سَلَام", [0., 0.]).unwrap();
    let mut s = EditSession::new(t);
    let layout = e.compose(&s.draft, 1.).unwrap().layout.clone();
    s.delete(&layout, true).unwrap();
    assert!(!s.draft.source().ends_with('م'));
    s.select_all();
    s.insert("مرحبا").unwrap();
    s.insert(" world").unwrap();
    assert_eq!(s.draft.source(), "مرحبا world");
    let mut ed = Editor::new();
    ed.try_execute_created(EditCommand::AddText { text: s.draft, parent: None }).unwrap();
    assert_eq!(ed.rev, 1);
    ed.undo();
    assert!(ed.doc.text_boxes.is_empty());
}
#[test]
fn outlines_replace_leaf_in_paint_order_keep_source_and_holes() {
    let mut ed = Editor::new();
    let mut t = default_text("O", [10., 60.]).unwrap();
    let mut e = TextLayout::bundled().unwrap();
    let f = &e.fonts().faces()[0];
    t.runs[0].style.font = FontRef {
        family: f.family.into(),
        weight: u32::from(f.weight),
        hash: varos_text_layout::font_hash(f.content_hash),
    };
    ed.try_execute_created(EditCommand::AddText { text: t, parent: None }).unwrap();
    let before = ed.doc.clone();
    let out = e.outlined(&ed.doc, 1.).unwrap();
    assert_eq!(ed.doc, before);
    assert!(out.text_boxes.is_empty());
    assert!(out.paths.iter().any(|p| !p.holes.is_empty()));
    assert!(out.paint_list().count() > 0);
}
#[test]
fn empty_point_has_real_caret() {
    let mut e = TextLayout::bundled().unwrap();
    let t = default_text("", [0., 0.]).unwrap();
    assert!(!e.compose(&t, 1.).unwrap().layout.carets.is_empty());
}

#[test]
fn layout_resource_refusal_is_explicit() {
    let mut e = TextLayout::bundled().unwrap();
    let t = default_text(&"a".repeat(65_537), [0., 0.]).unwrap();
    assert!(e.compose(&t, 1.).unwrap_err().contains("limit exceeded"));
}

#[test]
fn area_overflow_is_retained_as_source_and_reported() {
    let mut t = default_text("مرحبا\nمرحبا\nمرحبا", [0., 0.]).unwrap();
    t.box_kind = TextBoxKind::Area([0., 0., 240., 50.]);
    let mut e = TextLayout::bundled().unwrap();
    assert!(e.compose(&t, 1.).unwrap().overset);
    let mut ed = Editor::new();
    ed.try_execute_created(EditCommand::AddText { text: t, parent: None }).unwrap();
    assert!(varos_text_layout::export_notes(&ed.doc).unwrap().iter().any(|n| n.kind == "text_overset"));
    assert_eq!(ed.doc.text_boxes[0].source().lines().count(), 3);
}

#[test]
fn latin_tracking_moves_end_caret_and_preserves_clusters() {
    let mut e = TextLayout::bundled().unwrap();
    let mut t = default_text("ABC", [0., 0.]).unwrap();
    t.para.direction = Direction::Ltr;
    t.para.align = Alignment::Left;
    let before = e.compose(&t, 1.).unwrap().layout.clone();
    t.runs[0].style.letter_spacing = 3.;
    let after = &e.compose(&t, 1.).unwrap().layout;
    assert!((after.lines[0].width - before.lines[0].width - 6.).abs() < 0.01);
    let end = |l: &varos_text::Layout| l.carets.iter().filter(|c| c.byte == 3).map(|c| c.x).fold(0f32, f32::max);
    assert!((end(after) - end(&before) - 6.).abs() < 0.01);
}
