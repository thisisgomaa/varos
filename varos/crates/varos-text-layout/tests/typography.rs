use varos_core::{
    editor::Editor,
    text::{Alignment, Direction, Kashida},
    typography::{Action, Binding, CharacterStyle, ParagraphStyle, PathEffect},
    EditCommand,
};
use varos_text_layout::{
    default_text,
    flow::{band, intervals, ArcPath},
    TextLayout,
};
fn scene(source: &str) -> (Editor, u32, u32) {
    let mut ed = Editor::new();
    let shape = ed
        .try_execute_created(EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Ellipse,
            bounds: [0., 0., 260., 260.],
            parent: None,
            fill: None,
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    let mut t = default_text(source, [0., 40.]).unwrap();
    t.para.direction = Direction::Rtl;
    t.para.align = Alignment::Justify;
    t.para.kashida = Kashida::Balanced;
    let id = ed.try_execute_created(EditCommand::AddText { text: t, parent: None }).unwrap();
    (ed, id, shape)
}
#[test]
fn arabic_circle_justifies_without_losing_logical_source() {
    let (mut ed, id, path) = scene(&"سلام عليكم ورحمة الله مرحبا بالعالم ".repeat(10));
    ed.try_execute(EditCommand::Typography(Action::Bind {
        text: id,
        binding: Some(Binding::Area { path, inset: 8. }),
    }))
    .unwrap();
    let mut engine = TextLayout::bundled().unwrap();
    let t = &ed.doc.text_boxes[0];
    let a = engine.compose_document(&ed.doc, t, 1.).unwrap();
    assert!(a.overset);
    assert!(a.layout.issues.iter().any(|issue| matches!(issue, varos_text::Issue::KashidaInserted { .. })));
    assert!(!a.paths.is_empty());
    assert!(a.layout.lines.len() > 2);
    assert_eq!(a.layout.source, t.source());
    for p in &a.paths {
        for a in &p.anchors {
            let dx = (a.p[0] - 130.) / 130.;
            let dy = (a.p[1] - 130.) / 130.;
            assert!(dx * dx + dy * dy < 1.01);
        }
    }
    let b = engine.compose_document(&ed.doc, t, 1.).unwrap();
    assert_eq!(a.paths, b.paths);
    ed.undo();
    assert!(ed.doc.typography.is_empty());
}
#[test]
fn intervals_holes_empty_and_band_are_conservative() {
    let rings = vec![
        vec![[0., 0.], [100., 0.], [100., 100.], [0., 100.]],
        vec![[30., 30.], [70., 30.], [70., 70.], [30., 70.]],
    ];
    assert_eq!(intervals(&rings, 50.), vec![[0., 30.], [70., 100.]]);
    assert!(intervals(&rings, 101.).is_empty());
    assert_eq!(band(&rings, 20., 80., 2.), vec![[2., 28.], [72., 98.]]);
}
#[test]
fn path_rtl_clusters_remain_logical_and_geometry_finite() {
    let (mut ed, id, path) = scene("سَلَام 123 Varos");
    ed.try_execute(EditCommand::Typography(Action::Bind {
        text: id,
        binding: Some(Binding::Path { path, start: 0., end: 0., offset: 5., flip: true, effect: PathEffect::Rainbow }),
    }))
    .unwrap();
    let mut engine = TextLayout::bundled().unwrap();
    let result = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    assert_eq!(result.layout.source, "سَلَام 123 Varos");
    assert!(!result.paths.is_empty());
    assert!(result.paths.iter().flat_map(|p| &p.anchors).flat_map(|a| a.p).all(f32::is_finite));
    for g in result.layout.lines.iter().flat_map(|l| &l.glyphs) {
        assert!(result.layout.source.get(g.cluster.clone()).is_some());
    }
    assert!(ArcPath::new(vec![[0., 0.], [0., 0.]], false).unwrap().sample(0.).is_none());
    assert_eq!(ArcPath::new(vec![[10., 0.], [0., 0.]], false).unwrap().sample(5.).unwrap().1, [-1., 0.]);
}
#[test]
fn named_styles_update_inherit_and_reject_cycles_atomically() {
    let (mut ed, id, _) = scene("سلام Varos");
    let mut style = ed.doc.text_boxes[0].runs[0].style.clone();
    style.size = 42.;
    ed.try_execute(EditCommand::Typography(Action::DefineCharacter {
        name: "Headline".into(),
        definition: CharacterStyle { parent: None, style: Some(style.clone()) },
    }))
    .unwrap();
    ed.try_execute(EditCommand::Typography(Action::ApplyCharacter {
        text: id,
        start: 0,
        end: "سلام".len(),
        name: "Headline".into(),
    }))
    .unwrap();
    let t = ed.doc.typography.resolved(&ed.doc.text_boxes[0]).unwrap();
    assert_eq!(t.runs[0].style.size, 42.);
    assert_eq!(t.source(), "سلام Varos");
    assert_eq!(t.runs.len(), 2);
    style.size = 48.;
    ed.try_execute(EditCommand::Typography(Action::DefineCharacter {
        name: "Headline".into(),
        definition: CharacterStyle { parent: None, style: Some(style) },
    }))
    .unwrap();
    assert_eq!(ed.doc.typography.resolved(&ed.doc.text_boxes[0]).unwrap().runs[0].style.size, 48.);
    let before = ed.doc.clone();
    assert!(ed
        .try_execute(EditCommand::Typography(Action::DefineCharacter {
            name: "Headline".into(),
            definition: CharacterStyle { parent: Some("Headline".into()), style: None }
        }))
        .is_err());
    assert_eq!(ed.doc, before);
    ed.try_execute(EditCommand::Typography(Action::DefineParagraph {
        name: "Body".into(),
        definition: ParagraphStyle { parent: None, style: Some(Default::default()) },
    }))
    .unwrap();
    ed.try_execute(EditCommand::Typography(Action::ApplyParagraph { text: id, name: "Body".into() })).unwrap();
    assert_eq!(ed.doc.typography.resolved(&ed.doc.text_boxes[0]).unwrap().para, Default::default());
    assert!(ed
        .try_execute(EditCommand::Typography(Action::Features { text: id, features: [("rlig".into(), 0)].into() }))
        .is_err());
}
#[test]
fn thread_preserves_one_source_and_rejects_cycles() {
    let (mut ed, id, path) = scene(&"مرحبا بالعالم ".repeat(20));
    ed.try_execute(EditCommand::Typography(Action::Bind {
        text: id,
        binding: Some(Binding::Area { path, inset: 20. }),
    }))
    .unwrap();
    let mut empty = default_text("", [300., 0.]).unwrap();
    empty.box_kind = varos_core::text::TextBoxKind::Area([300., 0., 200., 300.]);
    let next = ed.try_execute_created(EditCommand::AddText { text: empty, parent: None }).unwrap();
    ed.try_execute(EditCommand::Typography(Action::Thread { from: id, to: Some(next) })).unwrap();
    let mut engine = TextLayout::bundled().unwrap();
    let a = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    let b = engine.compose_document(&ed.doc, &ed.doc.text_boxes[1], 1.).unwrap();
    assert!(!b.paths.is_empty());
    assert!(b.layout.lines[0].range.start >= a.layout.lines.last().unwrap().range.end);
    assert!(ed.doc.text_boxes[1].source().is_empty());
    assert!(ed.try_execute(EditCommand::Typography(Action::Thread { from: next, to: Some(id) })).is_err());
}

#[test]
fn features_change_real_glyphs_and_cache_tracks_geometry() {
    let (mut ed, id, path) = scene("AV AV");
    let fonts = varos_text_layout::bundled_fonts().unwrap();
    let face = &fonts.faces()[0];
    ed.doc.text_boxes[0].runs[0].style.font = varos_core::text::FontRef {
        family: face.family.into(),
        weight: 400,
        hash: varos_text_layout::font_hash(face.content_hash),
    };
    ed.try_execute(EditCommand::Typography(Action::Features { text: id, features: [("kern".into(), 1)].into() }))
        .unwrap();
    let mut engine = TextLayout::bundled().unwrap();
    let a = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    let count = engine.layouts;
    assert_eq!(engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap().paths, a.paths);
    assert_eq!(engine.layouts, count);
    ed.try_execute(EditCommand::Typography(Action::Features { text: id, features: [("kern".into(), 0)].into() }))
        .unwrap();
    let b = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    assert_ne!(a.layout.lines[0].glyphs, b.layout.lines[0].glyphs);
    ed.try_execute(EditCommand::Typography(Action::Bind {
        text: id,
        binding: Some(Binding::Area { path, inset: 0. }),
    }))
    .unwrap();
    let a = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    for anchor in &mut ed.doc.paths[0].anchors {
        anchor.p[0] += 50.;
        anchor.hin = anchor.hin.map(|[x, y]| [x + 50., y]);
        anchor.hout = anchor.hout.map(|[x, y]| [x + 50., y]);
    }
    let b = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    assert_ne!(a.paths, b.paths);
}

#[test]
fn path_editing_map_roundtrips_rtl_carets_and_keeps_marks_on_cluster_tangent() {
    let (mut ed, id, path) = scene("سَلَام 123 Varos");
    for flip in [false, true] {
        ed.try_execute(EditCommand::Typography(Action::Bind {
            text: id,
            binding: Some(Binding::Path { path, start: 12., end: 10., offset: 0., flip, effect: PathEffect::Rainbow }),
        }))
        .unwrap();
        let mut engine = TextLayout::bundled().unwrap();
        let composed = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
        let map = varos_text_layout::path_mapping::PathMap::from_document(&ed.doc, id, &composed).unwrap().unwrap();
        let baseline = composed.layout.lines[0].baseline;
        for caret in &composed.layout.carets {
            if let Some(point) = map.point([caret.x, baseline]) {
                let back = map.unmap(point);
                assert!((back[0] - caret.x).abs() < 0.02);
                assert_eq!(back[1], baseline);
                assert!(map.at_anchor([caret.x, baseline - 12.], caret.x).unwrap().iter().all(|x| x.is_finite()));
            }
        }
    }
}

#[test]
fn threaded_allocator_keeps_two_widow_lines_and_preserves_source() {
    let mut ed = Editor::new();
    let mut text = default_text(&"one two three four five six ".repeat(4), [0., 0.]).unwrap();
    text.para.direction = Direction::Ltr;
    text.para.align = Alignment::Left;
    text.box_kind = varos_core::text::TextBoxKind::Area([0., 0., 170., 2000.]);
    let id = ed.try_execute_created(EditCommand::AddText { text, parent: None }).unwrap();
    let mut engine = TextLayout::bundled().unwrap();
    // Activate the shape-frame allocator before measuring the unconstrained capacity.
    ed.try_execute(EditCommand::Typography(Action::Features { text: id, features: Default::default() })).unwrap();
    let full = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    let count = full.layout.lines.len();
    assert!(count >= 4);
    let last_visible = &full.layout.lines[count - 2];
    let mut text = ed.doc.text_boxes[0].clone();
    text.box_kind =
        varos_core::text::TextBoxKind::Area([0., 0., 170., last_visible.baseline + last_visible.descent + 0.1]);
    ed.try_execute(EditCommand::SetText { id, text }).unwrap();
    let mut next = default_text("", [200., 0.]).unwrap();
    next.box_kind = varos_core::text::TextBoxKind::Area([200., 0., 170., 2000.]);
    let target = ed.try_execute_created(EditCommand::AddText { text: next, parent: None }).unwrap();
    ed.try_execute(EditCommand::Typography(Action::Thread { from: id, to: Some(target) })).unwrap();
    let first = engine.compose_document(&ed.doc, &ed.doc.text_boxes[0], 1.).unwrap();
    let second = engine.compose_document(&ed.doc, &ed.doc.text_boxes[1], 1.).unwrap();
    assert_eq!(first.layout.lines.len(), count - 2);
    assert_eq!(second.layout.lines.len(), 2);
    assert_eq!(second.layout.source, ed.doc.text_boxes[0].source());
    assert!(!second.overset);
}

#[test]
fn source_edits_preserve_named_character_style_and_later_updates() {
    let (mut ed, id, _) = scene("سلام");
    let mut style = ed.doc.text_boxes[0].runs[0].style.clone();
    style.size = 42.;
    ed.try_execute(EditCommand::Typography(Action::DefineCharacter {
        name: "Arabic".into(),
        definition: CharacterStyle { parent: None, style: Some(style.clone()) },
    }))
    .unwrap();
    ed.try_execute(EditCommand::Typography(Action::ApplyCharacter {
        text: id,
        start: 0,
        end: "سلام".len(),
        name: "Arabic".into(),
    }))
    .unwrap();
    let mut text = ed.doc.text_boxes[0].clone();
    text.runs[0].text.push_str(" عليكم");
    ed.try_execute(EditCommand::SetText { id, text }).unwrap();
    style.size = 48.;
    ed.try_execute(EditCommand::Typography(Action::DefineCharacter {
        name: "Arabic".into(),
        definition: CharacterStyle { parent: None, style: Some(style) },
    }))
    .unwrap();
    let resolved = ed.doc.typography.resolved(&ed.doc.text_boxes[0]).unwrap();
    assert!(resolved.runs.iter().all(|r| r.style.size == 48.));
    assert_eq!(resolved.source(), "سلام عليكم");
    assert_eq!(ed.doc.typography.frames[&id].characters[0].end, resolved.source().len());
}
