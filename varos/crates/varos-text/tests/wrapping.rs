mod common;
use common::*;
use varos_text::*;
#[test]
fn legal_units_source_coverage_and_cold_roundtrip() {
    let mut e = engine();
    for text in [
        "A\u{a0}B",
        "A\u{2067}شعار 12\u{2069}Z",
        "السعر ١٢٣٫٤٥ ج.م. (USD 12.50)",
        "A (12) — شعار!",
        "A\nB\r\nC\rD\u{2028}E\u{2029}F",
        "شعار   ",
        "A\u{2066}B\u{2069}:C",
        "A\u{a0}شعار",
    ] {
        for dir in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
            for width in [10., 120., 600., 120.] {
                let mut r = Request::new(text, 48., Some(width));
                r.direction = dir;
                // Style boundaries inside the NBSP unit cannot create an opportunity.
                if text == "A\u{a0}B" {
                    r.styles.push(Style { range: 0..1, face: ARABIC, size: 60., baseline_shift: 0., paint: 0 });
                }
                let l = e.layout(&r).unwrap();
                assert_eq!(l, engine().layout(&r).unwrap());
                let legal: Vec<_> = unicode_linebreak::linebreaks(text).map(|(i, _)| i).collect();
                let mut end = 0;
                for line in &l.lines {
                    assert_eq!(line.range.start, end, "coverage {text:?}");
                    end = line.range.end;
                    assert!(end == text.len() || legal.contains(&end), "illegal {text:?}: {end}");
                    for g in &line.glyphs {
                        assert!(g.cluster.start >= line.range.start && g.cluster.end <= line.range.end);
                    }
                }
                assert_eq!(end, text.len());
                if text == "A\u{a0}B" {
                    assert_eq!(l.lines.len(), 1);
                    assert_eq!(
                        l.issues.iter().any(|i| matches!(i, Issue::Overflow(_))),
                        l.lines[0].width > width + 0.01
                    );
                }
            }
        }
    }
}
#[test]
fn local_l1_matches_full_paragraph_oracle() {
    for text in
        CORPUS.iter().flat_map(|r| r.texts).chain(["A\u{202b}BC\u{202c}  شعار", "A\u{2067}شعار\u{2069}  "].iter())
    {
        for base in [None, Some(unicode_bidi::Level::ltr()), Some(unicode_bidi::Level::rtl())] {
            let b = unicode_bidi::BidiInfo::new(text, base);
            for p in &b.paragraphs {
                let boundaries: Vec<_> =
                    text[p.range.clone()].char_indices().map(|(i, _)| p.range.start + i).chain([p.range.end]).collect();
                for range in boundaries.windows(2) {
                    let r = range[0]..range[1];
                    assert_eq!(
                        local_line_levels(
                            text,
                            if p.level.is_rtl() { Direction::Rtl } else { Direction::Ltr },
                            r.clone()
                        )
                        .unwrap(),
                        b.reordered_levels(p, r.clone())[r].iter().map(|l| l.number()).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

#[test]
fn arabic_line_edges_match_direct_shaping_and_refit() {
    use cosmic_text::harfrust as hb;
    let font = hb::FontRef::new(PLEX).unwrap();
    let data = hb::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let mut changed_forms = 0;
    for text in ["ب\u{200b}ب\u{200b}ب", "السَّلَامُ\u{200b}بِبَّ\u{200b}قُرْآنٌ", "ب\u{200d}\u{200b}ب\u{200b}ب"]
    {
        for direction in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
            let mut r = Request::new(text, 48., None);
            r.face = ARABIC;
            r.language = "ar";
            r.direction = direction;
            let whole = engine().layout(&r).unwrap();
            for width in [1., 28., 50., 90., 120., 200.] {
                r.width = Some(width);
                let wrapped = engine().layout(&r).unwrap();
                assert!(
                    !wrapped.issues.iter().any(|issue| matches!(issue, Issue::UnsupportedCluster(_))),
                    "ZWSP is an invisible control, not a missing glyph"
                );
                for line in &wrapped.lines {
                    let part = &text[line.range.clone()];
                    let mut b = hb::UnicodeBuffer::new();
                    b.push_str(part);
                    b.set_script(hb::script::ARABIC);
                    b.set_language("ar".parse().unwrap());
                    b.set_direction(hb::Direction::RightToLeft);
                    let shaped = shaper.shape(b, &[]);
                    let scale = 48. / shaper.units_per_em() as f32;
                    let mut expected: Vec<_> = shaped
                        .glyph_infos()
                        .iter()
                        .zip(shaped.glyph_positions())
                        .map(|(g, p)| {
                            (
                                line.range.start + g.cluster as usize,
                                g.glyph_id,
                                p.x_advance as f32 * scale,
                                p.x_offset as f32 * scale,
                                -p.y_offset as f32 * scale,
                            )
                        })
                        .collect();
                    expected.sort_by_key(|g| (g.0, g.1));
                    let mut actual: Vec<_> = line
                        .glyphs
                        .iter()
                        .map(|g| (g.cluster.start, u32::from(g.id), g.advance, g.offset[0], g.offset[1]))
                        .collect();
                    actual.sort_by_key(|g| (g.0, g.1));
                    assert_eq!(actual.len(), expected.len(), "{text:?} {width}: {part:?}");
                    for (a, b) in actual.iter().zip(&expected) {
                        assert_eq!((a.0, a.1), (b.0, b.1), "{text:?} {width}: {part:?}");
                        assert!((a.2 - b.2).abs() < 0.001 && (a.3 - b.3).abs() < 0.001 && (a.4 - b.4).abs() < 0.001);
                        changed_forms += usize::from(
                            !whole
                                .lines
                                .iter()
                                .flat_map(|l| &l.glyphs)
                                .any(|g| g.cluster.start == a.0 && u32::from(g.id) == a.1),
                        );
                    }
                    let total: f32 = expected.iter().map(|g| g.2).sum();
                    assert!((line.width - total).abs() < 0.001);
                    if let Some((next, _)) = unicode_linebreak::linebreaks(text).find(|(end, _)| *end > line.range.end)
                    {
                        let mut b = hb::UnicodeBuffer::new();
                        b.push_str(&text[line.range.start..next]);
                        b.set_script(hb::script::ARABIC);
                        b.set_language("ar".parse().unwrap());
                        b.set_direction(hb::Direction::RightToLeft);
                        let next = shaper.shape(b, &[]);
                        let next_width: f32 = next.glyph_positions().iter().map(|p| p.x_advance as f32 * scale).sum();
                        assert!(
                            next_width > width,
                            "fitter stopped before a fitting next legal opportunity: {text:?}/{width}/{direction:?}"
                        );
                    }
                    if line.width > width + 0.001 {
                        assert_eq!(
                            unicode_linebreak::linebreaks(part).count(),
                            1,
                            "overflow must be exactly one legal unit: {part:?}"
                        );
                    }
                }
            }
        }
    }
    assert!(changed_forms > 0, "fixture must demonstrate actual contextual-form change at a line edge");
}
