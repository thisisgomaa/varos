mod common;
use common::*;
use unicode_segmentation::UnicodeSegmentation;
use varos_text::outlines::*;
use varos_text::*;

fn matrix(row: usize) {
    let mut engine = engine();
    for text in CORPUS[row].texts {
        for size in SIZES_PT {
            for width in [None, Some(120.), Some(600.)] {
                let mut req = Request::new(text, size, width);
                if row == 7 {
                    req.direction = if text.starts_with("شعار") { Direction::Ltr } else { Direction::Rtl };
                }
                let layout = engine.layout(&req).unwrap();
                assert_eq!(layout, engine.layout(&req).unwrap(), "determinism {}", CORPUS[row].id);
                assert_eq!(layout.copy(0..text.len()), Some(*text));
                let boundaries: Vec<_> = text.grapheme_indices(true).map(|(i, _)| i).chain([text.len()]).collect();
                for (line_i, line) in layout.lines.iter().enumerate() {
                    assert!(line.baseline.is_finite() && line.ascent.is_finite() && line.descent.is_finite());
                    for g in &line.glyphs {
                        assert!(g.cluster.start < g.cluster.end && text.get(g.cluster.clone()).is_some(), "{g:?}");
                        assert!([g.x, g.y, g.advance, g.offset[0], g.offset[1], g.size].iter().all(|v| v.is_finite()));
                        // Every grapheme overlapping a glyph is confined to the same line.
                        for (start, grapheme) in text.grapheme_indices(true) {
                            let end = start + grapheme.len();
                            if g.cluster.start < end && g.cluster.end > start {
                                for (other_i, other) in layout.lines.iter().enumerate() {
                                    if other_i != line_i {
                                        assert!(
                                            !other
                                                .glyphs
                                                .iter()
                                                .any(|h| h.cluster.start < end && h.cluster.end > start),
                                            "split grapheme {grapheme:?}"
                                        );
                                    }
                                }
                            }
                        }
                        if g.id == 0 {
                            assert!(layout.issues.iter().any(|i|matches!(i,Issue::UnsupportedCluster(r) if r.start<g.cluster.end && r.end>g.cluster.start)));
                        }
                    }
                }
                assert!(!layout.carets.is_empty(), "{text:?}");
                for boundary in &boundaries {
                    assert!(
                        layout.carets.iter().any(|c| c.byte == *boundary),
                        "unmapped boundary {boundary} in {text:?} width {width:?}"
                    );
                }
                for (i, c) in text.char_indices() {
                    if !c.is_control()
                        && !c.is_whitespace()
                        && !matches!(c, '\u{200c}' | '\u{200d}' | '\u{2066}'..='\u{2069}')
                    {
                        assert!(
                            layout.lines.iter().flat_map(|l| &l.glyphs).any(|g| g.cluster.contains(&i))
                                || layout
                                    .issues
                                    .iter()
                                    .any(|issue| matches!(issue,Issue::UnsupportedCluster(r) if r.contains(&i))),
                            "dropped {c:?}"
                        );
                    }
                }
                assert!(layout.carets.len() <= 4 * (text.chars().count() + layout.lines.len() + 1));
                for caret in &layout.carets {
                    assert!(boundaries.contains(&caret.byte), "caret inside grapheme: {text:?} {caret:?}");
                    assert!(caret.x.is_finite());
                    assert!(layout.hit(caret.line, caret.x).contains(&caret));
                    assert!(layout.caret(caret.byte, caret.affinity).contains(&caret));
                }
                // An explicit finite visual walk and reverse walk preserve every identity.
                let walk: Vec<_> = layout.carets.iter().collect();
                let mut reverse: Vec<_> = layout.carets.iter().rev().collect();
                reverse.reverse();
                assert_eq!(walk, reverse);
                let mut styled = StyledText::new(text);
                for (i, p) in styled.paint_per_grapheme.iter_mut().enumerate() {
                    *p = i as u32;
                }
                let original = styled.clone();
                let undo = styled.replace(0..text.len(), "X", 7).unwrap();
                assert_eq!(styled.text, "X");
                styled = undo;
                assert_eq!(styled, original);
                let outlines = layout_outlines(&font_set(true), &layout).unwrap();
                for outline in &outlines {
                    if let Some(bounds) = outline.ink_bounds() {
                        assert!(bounds.iter().all(|v| v.is_finite()));
                    }
                }
            }
        }
    }
}
#[test]
fn joins() {
    matrix(0);
    let mut e = engine();
    let full = e.layout(&Request::new("بب", 48., None)).unwrap();
    let single = e.layout(&Request::new("ب", 48., None)).unwrap();
    assert!(full.lines[0].glyphs.iter().any(|g| g.id != single.lines[0].glyphs[0].id));
}
#[test]
fn ligatures() {
    matrix(1);
    let l = engine().layout(&Request::new("لا", 48., None)).unwrap();
    // Evidence specifically for pinned Plex, not a universal Arabic glyph count.
    assert!(l.lines[0].glyphs.iter().any(|g| g.cluster == (0..4)));
    assert!(l.carets.iter().any(|c| c.byte == 2));
}
#[test]
fn marks() {
    matrix(2);
    let mut e = engine();
    for text in CORPUS[2].texts {
        let layout = e.layout(&Request::new(text, 200., Some(120.))).unwrap();
        assert!(layout.lines.iter().flat_map(|l| &l.glyphs).any(|g| g.advance == 0. && g.offset != [0., 0.]));
        let all = layout_outlines(&font_set(true), &layout).unwrap();
        assert_eq!(layout.ink_bounds, ink_bounds(&all));
        let mut styled = StyledText::new(text);
        for (start, g) in text.grapheme_indices(true) {
            let before = styled.clone();
            let undo = styled.replace(start..start + g.len(), "X", 7).unwrap();
            assert_eq!(undo, before);
            styled = undo;
        }
    }
}
#[test]
fn mixed_bidi() {
    matrix(3);
    bidi_visual_order_oracle();
    let l = engine().layout(&Request::new("Logo شعار v2", 48., None)).unwrap();
    assert!(l.levels.iter().any(|l| l % 2 == 1) && l.levels.iter().any(|l| l % 2 == 0));
    assert!(l
        .carets
        .iter()
        .any(|a| l.carets.iter().any(|b| a.byte == b.byte && a.affinity != b.affinity && (a.x - b.x).abs() > 1.)));
}
#[test]
fn wrapping() {
    matrix(4);
    let mut e = engine();
    let text = CORPUS[4].texts[0];
    let a = e.layout(&Request::new(text, 48., Some(120.))).unwrap();
    let b = e.layout(&Request::new(text, 48., Some(600.))).unwrap();
    assert!(a.lines.len() > b.lines.len());
    let mut req = Request::new("سلام", 48., Some(600.));
    req.direction = Direction::Rtl;
    let start = e.layout(&req).unwrap();
    req.end_align = true;
    let end = e.layout(&req).unwrap();
    assert!(start.lines[0].glyphs[0].x > end.lines[0].glyphs[0].x);
    // Legal word boundaries don't alter joins: wrapped words match their isolated shapes.
    for line in &a.lines {
        let t = &text[line.range.clone()];
        let mut request = Request::new(t, 48., None);
        request.direction = Direction::Rtl;
        let isolated = e.layout(&request).unwrap();
        let mut ids: Vec<_> = line
            .glyphs
            .iter()
            .filter(|g| !text[g.cluster.clone()].trim().is_empty())
            .map(|g| (g.cluster.start - line.range.start, g.id))
            .collect();
        let mut other: Vec<_> = isolated
            .lines
            .iter()
            .flat_map(|l| &l.glyphs)
            .filter(|g| !t[g.cluster.clone()].trim().is_empty())
            .map(|g| (g.cluster.start, g.id))
            .collect();
        ids.sort();
        other.sort();
        assert_eq!(ids, other);
    }
}
#[test]
fn joining_controls() {
    matrix(5);
    let mut e = engine();
    let ids = |l: Layout| l.lines.into_iter().flat_map(|l| l.glyphs).map(|g| g.id).collect::<Vec<_>>();
    assert_ne!(
        ids(e.layout(&Request::new("ب\u{200d}ب", 48., None)).unwrap()),
        ids(e.layout(&Request::new("ب\u{200c}ب", 48., None)).unwrap())
    );
    assert_ne!(
        ids(e.layout(&Request::new("سلام", 48., None)).unwrap()),
        ids(e.layout(&Request::new("سـلام", 48., None)).unwrap())
    );
}
#[test]
fn style_context() {
    matrix(6);
    let mut e = engine();
    let mut req = Request::new("شعار", 48., None);
    req.face = ARABIC;
    let base = e.layout(&req).unwrap();
    req.styles = vec![Style { range: 0..4, face: ARABIC, size: 48., baseline_shift: 0., paint: 0xff0000 }];
    assert_eq!(base, e.layout(&req).unwrap(), "paint never enters shaping attrs");
    req.styles[0].face = LATIN;
    let changed = e.layout(&req).unwrap();
    assert_eq!(changed, e.layout(&req).unwrap());
    assert!(changed.issues.iter().any(|i| matches!(i, Issue::Substituted { .. })));
    // A real supported font boundary uses both faces; Arabic Inter request falls back.
    req = Request::new("Logo", 48., None);
    req.styles = vec![Style { range: 0..2, face: ARABIC, size: 48., baseline_shift: 0., paint: 0 }];
    let fonts = e.layout(&req).unwrap();
    assert!(fonts.lines[0].glyphs.iter().any(|g| g.face == ARABIC));
    assert!(fonts.lines[0].glyphs.iter().any(|g| g.face == LATIN));
}
#[test]
fn explicit_direction() {
    matrix(7);
    let mut e = engine();
    for (text, direction, rtl) in
        [("Logo شعار", Direction::Rtl, true), ("شعار Logo", Direction::Ltr, false), ("", Direction::Rtl, true)]
    {
        let mut req = Request::new(text, 48., Some(600.));
        req.direction = direction;
        let l = e.layout(&req).unwrap();
        assert_eq!(l.source, text);
        assert!(l.lines.iter().all(|l| l.rtl == rtl));
        assert!(!l.lines.is_empty());
    }
}
#[test]
fn isolates_newlines() {
    matrix(8);
    assert_eq!(paste_normalize("A\r\nشعار"), "A\nشعار");
    let l = engine().layout(&Request::new("A\r\nشعار", 48., None)).unwrap();
    assert_eq!(l.source, "A\r\nشعار");
    assert_eq!(l.lines[1].range.start, 3);
    assert!(l.carets.iter().any(|c| c.byte == 3));
}
#[test]
fn fallback() {
    matrix(9);
    let mut e = engine();
    let l = e.layout(&Request::new("شَعار", 48., None)).unwrap();
    assert!(l.lines[0].glyphs.iter().all(|g| g.face == ARABIC));
    assert!(l.issues.iter().any(|i| matches!(i, Issue::Substituted { .. })));
    let missing = Engine::new(font_set(false)).unwrap().layout(&Request::new("شعار", 48., None)).unwrap();
    assert!(missing.issues.iter().any(|i| matches!(i, Issue::UnsupportedCluster(_))));
    let emoji = e.layout(&Request::new("👩\u{200d}💻", 48., None)).unwrap();
    assert!(emoji.issues.contains(&Issue::UnsupportedCluster(0..11)));
}
#[test]
fn mixed_metrics_rotation_limits_and_language() {
    let mut e = engine();
    let mut r = Request::new("Logo سلام", 48., None);
    r.styles = vec![Style { range: 5..13, face: ARABIC, size: 200., baseline_shift: 40., paint: 0 }];
    let l = e.layout(&r).unwrap();
    assert!(l.lines[0].ascent > 100.);
    let bounds = ink_bounds(&layout_outlines(&font_set(true), &l).unwrap()).unwrap();
    assert!(bounds[3] > bounds[1]);
    r.language = "ar";
    assert!(!e.layout(&r).unwrap().issues.contains(&Issue::UnsupportedLanguage("ar".into())));
    r.size = f32::NAN;
    assert!(e.layout(&r).is_err());
    let huge = "x".repeat(1_048_577);
    assert_eq!(e.layout(&Request::new(&huge, 12., None)), Err("resource limit"));
    assert!(Engine::validate_font(b"malformed").is_err());
    assert!(Engine::validate_font(INTER).is_ok());
    r = Request::new("AV", 48., None);
    let on = e.layout(&r).unwrap();
    r.features.push(Feature { tag: *b"kern", value: 0 });
    let off = e.layout(&r).unwrap();
    assert_ne!(on.lines[0].glyphs, off.lines[0].glyphs);
}

fn bidi_visual_order_oracle() {
    let mut engine = engine();
    for text in CORPUS[3].texts.iter().chain(CORPUS[8].texts.iter().take(1)) {
        for direction in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
            for width in [None, Some(120.), Some(600.)] {
                let mut req = Request::new(text, 48., width);
                req.direction = direction;
                let layout = engine.layout(&req).unwrap();
                let base = match direction {
                    Direction::Auto => None,
                    Direction::Ltr => Some(unicode_bidi::Level::ltr()),
                    Direction::Rtl => Some(unicode_bidi::Level::rtl()),
                };
                let bidi = unicode_bidi::BidiInfo::new(text, base);
                for line in &layout.lines {
                    if line.range.is_empty() {
                        continue;
                    }
                    let levels = bidi.reordered_levels(&bidi.paragraphs[0], line.range.clone());
                    let mut logical: Vec<_> = line.glyphs.iter().map(|g| g.cluster.start).collect();
                    logical.sort();
                    logical.dedup();
                    let expected: Vec<_> =
                        unicode_bidi::BidiInfo::reorder_visual(&logical.iter().map(|i| levels[*i]).collect::<Vec<_>>())
                            .into_iter()
                            .map(|i| logical[i])
                            .collect();
                    let mut visual: Vec<_> = line.glyphs.iter().filter(|g| g.advance > 0.).collect();
                    visual.sort_by(|a, b| a.x.total_cmp(&b.x));
                    let actual: Vec<_> = visual.iter().map(|g| g.cluster.start).collect();
                    let expected: Vec<_> = expected.into_iter().filter(|i| actual.contains(i)).collect();
                    if actual != expected {
                        let mut db = fontdb::Database::new();
                        db.load_font_data(INTER.to_vec());
                        db.load_font_data(PLEX.to_vec());
                        let mut fs = cosmic_text::FontSystem::new_with_locale_and_db("en-US".into(), db);
                        let attrs = cosmic_text::AttrsList::new(
                            &cosmic_text::Attrs::new().family(cosmic_text::Family::Name("Inter")),
                        );
                        let raw = cosmic_text::ShapeLine::new(&mut fs, text, &attrs, cosmic_text::Shaping::Advanced, 4);
                        let raw_lines =
                            raw.layout(48., width, cosmic_text::Wrap::Word, None, None, cosmic_text::Hinting::Disabled);
                        let mut glyphs: Vec<_> = raw_lines[0].glyphs.iter().filter(|g| g.w > 0.).collect();
                        glyphs.sort_by(|a, b| a.x.total_cmp(&b.x));
                        eprintln!(
                            "Upstream ShapeLine directly, auto Inter, first line visual bytes: {:?}",
                            glyphs.iter().map(|g| g.start).collect::<Vec<_>>()
                        );
                    }
                    assert_eq!(actual, expected, "UBA visual order {text:?} {direction:?} {width:?} {:?}", line.range);
                }
            }
        }
    }
}

#[test]
fn geometry_affinity_and_resource_contracts() {
    let mut e = engine();
    let mut request = Request::new("", 48., Some(600.));
    request.direction = Direction::Rtl;
    let empty = e.layout(&request).unwrap();
    assert!(empty.carets.iter().all(|c| c.x == 600.));
    request = Request::new("10:30", 48., None);
    request.direction = Direction::Rtl;
    let number = e.layout(&request).unwrap();
    assert_eq!(number.lines.len(), 1);
    assert_eq!(number.lines[0].glyphs.len(), 5);
    request = Request::new("A", 48., None);
    let letter = e.layout(&request).unwrap();
    let b = letter.ink_bounds.unwrap();
    assert!(b[1] < letter.lines[0].baseline);
    assert!(b[3] <= letter.lines[0].baseline + 0.001);
    request.features.push(Feature { tag: *b"rlig", value: 0 });
    assert!(e.layout(&request).is_err());
    request = Request::new("e\u{301}", 48., None);
    request.styles.push(Style { range: 0..1, face: LATIN, size: 48., baseline_shift: 0., paint: 0 });
    assert_eq!(e.layout(&request), Err("invalid style"));
    for size in SIZES_PT {
        for width in [Some(120.), Some(600.)] {
            let missing = Engine::new(font_set(false)).unwrap().layout(&Request::new("شَعار", size, width)).unwrap();
            assert!(missing.issues.iter().any(|i| matches!(i, Issue::UnsupportedCluster(_))));
            let plex = {
                let mut r = Request::new("office café e\u{301}", size, width);
                r.face = ARABIC;
                e.layout(&r).unwrap()
            };
            assert!(plex.lines.iter().flat_map(|l| &l.glyphs).all(|g| g.face == ARABIC));
        }
    }
}

/// Acceptance test, intentionally left red if COSMIC breaks at a bidi-span
/// boundary which UAX #14 does not permit. A spike must expose this defect.
#[test]
fn legal_line_breaks_gate() {
    let mut engine = engine();
    let mut failures = Vec::new();
    for row in &CORPUS {
        for text in row.texts {
            for size in SIZES_PT {
                for width in WIDTHS_PT {
                    let layout = engine.layout(&Request::new(text, size, Some(width))).unwrap();
                    let legal: Vec<_> = unicode_linebreak::linebreaks(text).map(|(i, _)| i).collect();
                    for line in &layout.lines {
                        let mut end = line.range.end;
                        // Layout can omit a trailing breaking space from its advance range.
                        for c in text[end..].chars() {
                            if matches!(c, ' ' | '\t' | '\r' | '\n') {
                                end += c.len_utf8();
                            } else {
                                break;
                            }
                        }
                        if end < text.len() && !legal.contains(&end) {
                            failures.push(format!("{} {size}pt/{width}pt illegal byte {end}: {text:?}", row.id));
                        }
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} illegal breaks:\n{}", failures.len(), failures.join("\n"));
}
