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
    assert_eq!(e.layouts, count);
    t.runs[0].style.font.hash = "0".repeat(64);
    assert!(e.compose(&t, 1.).unwrap_err().contains("missing font snapshot"));
}
#[test]
fn justification_at_several_widths_preserves_source() {
    let mut e = TextLayout::bundled().unwrap();
    let mut t = default_text("اللغة العربية جميلة ومتنوعة في كل مكان والكتابة العربية فن جميل", [0., 0.]).unwrap();
    t.para.align = Alignment::Justify;
    t.para.kashida = Kashida::Balanced;
    let mut insertions = 0;
    for width in [160., 240., 360.] {
        t.box_kind = TextBoxKind::Area([0., 0., width, 500.]);
        let c = e.compose(&t, 1.).unwrap();
        assert_eq!(c.layout.source, t.source());
        assert!(c.layout.lines.len() > 1);
        assert!(c.layout.carets.iter().all(|caret| t.source().is_char_boundary(caret.byte)));
        assert!(c.layout.lines.iter().all(|l| l.width.is_finite()));
        insertions += c.layout.issues.iter().filter(|i| matches!(i, varos_text::Issue::KashidaInserted { .. })).count();
        for (index, line) in c.layout.lines.iter().enumerate().take(c.layout.lines.len() - 1) {
            assert!(
                (line.width - width).abs() <= 0.5
                    || c.layout
                        .issues
                        .iter()
                        .any(|i| matches!(i, varos_text::Issue::JustificationResidual { line, .. } if *line == index))
            );
        }
    }
    assert!(insertions > 0, "Balanced policy must perform real safe kashida insertion");
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

#[test]
fn visual_arrows_move_on_every_press_and_end_caret_has_affinity_fallback() {
    let mut e = TextLayout::bundled().unwrap();
    for source in ["abc", "سلام", "مرحبا Varos"] {
        let mut session = EditSession::new(default_text(source, [0., 0.]).unwrap());
        let layout = e.compose(&session.draft, 1.).unwrap().layout.clone();
        assert!(session.current_caret(&layout).is_some());
        let left = layout.carets.first().unwrap();
        session.caret = left.byte;
        session.affinity = left.affinity;
        let mut changed = 0;
        for _ in 0..layout.carets.len() {
            let previous = session.caret;
            session.arrow(&layout, true, true);
            if session.caret == previous {
                break;
            }
            changed += 1;
            assert!(source.is_char_boundary(session.caret));
        }
        assert!(changed >= 2, "{source}");
        assert_eq!(session.current_caret(&layout).unwrap().x, layout.carets.last().unwrap().x);
    }
}

#[test]
fn multiline_visual_navigation_and_selection_remain_source_aware() {
    let mut e = TextLayout::bundled().unwrap();
    let mut s = EditSession::new(
        default_text(
            "سلام
Varos",
            [0., 0.],
        )
        .unwrap(),
    );
    let layout = e.compose(&s.draft, 1.).unwrap().layout.clone();
    assert_eq!(s.current_caret(&layout).unwrap().line, 1);
    s.vertical(&layout, false, true);
    assert_eq!(s.current_caret(&layout).unwrap().line, 0);
    assert!(!layout.selection_rects(s.range()).is_empty());
    assert!(s.draft.source().is_char_boundary(s.caret));
    s.line_edge(&layout, true, false);
    assert!(s.range().is_empty());
    assert_eq!(
        s.current_caret(&layout).unwrap().x,
        layout.carets.iter().filter(|c| c.line == 0).map(|c| c.x).fold(f32::NEG_INFINITY, f32::max)
    );
}

#[test]
fn point_alignment_anchors_each_line_without_changing_source_or_wrapping() {
    let mut e = TextLayout::bundled().unwrap();
    let mut t = default_text(
        "سلام
مرحبا بالعالم",
        [100., 80.],
    )
    .unwrap();
    for (align, fraction) in [(Alignment::Left, 0.), (Alignment::Centre, 0.5), (Alignment::Right, 1.)] {
        t.para.align = align;
        let c = e.compose(&t, 1.).unwrap();
        assert_eq!(c.layout.lines.len(), 2);
        assert_eq!(c.layout.source, t.source());
        for (index, line) in c.layout.lines.iter().enumerate() {
            let left = c.layout.carets.iter().filter(|c| c.line == index).map(|c| c.x).fold(f32::INFINITY, f32::min);
            assert!((left + line.width * fraction).abs() < 0.01);
        }
    }
}

#[test]
fn cache_pressure_and_zoom_do_not_repeat_shaping() {
    let mut engine = TextLayout::bundled().unwrap();
    let mut text = default_text("A", [0., 40.]).unwrap();
    for id in 1..=300 {
        text.id = id;
        engine.compose(&text, 1.).unwrap();
    }
    assert_eq!(engine.layouts, 300);
    for zoom in [1., 1., 2.] {
        for id in 1..=300 {
            text.id = id;
            engine.compose(&text, zoom).unwrap();
        }
        assert_eq!(engine.layouts, 300);
    }
    text.runs[0].text = "B".into();
    engine.compose(&text, 2.).unwrap();
    assert_eq!(engine.layouts, 301);
}
