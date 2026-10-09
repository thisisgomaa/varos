mod common;
use common::*;
use unicode_segmentation::UnicodeSegmentation;
use varos_text::{composer::*, kashida::*, metrics::LineHeight, *};
fn fonts(noto: bool) -> FontSet {
    if !noto {
        return font_set(true);
    }
    FontSet::new(
        vec![
            FontFace::new("Inter", 400, INTER.into()).unwrap(),
            FontFace::new(
                "Noto Sans Arabic",
                400,
                include_bytes!("../assets/fonts/NotoSansArabic.ttf").as_slice().into(),
            )
            .unwrap(),
            FontFace::new("IBM Plex Sans Arabic", 400, PLEX.into()).unwrap(),
        ],
        FallbackPolicy { common: vec![ARABIC, LATIN, FaceId(2)], scripts: vec![] },
    )
    .unwrap()
}
const ARABIC: FaceId = FaceId(1);
const CASES: [&str; 8] = [
    "السلام عليكم ورحمة الله، كيف حالكم؟",
    "لا لأ لإ لآ الله سلام جميل",
    "السَّلَامُ عَلَيْكُمْ قُرْآنٌ كَرِيمٌ",
    "السعر ١٢٣٫٤٥ ج.م. (USD 12.50) اليوم",
    "Logo شعار v2، تصميم عربي 2026؟",
    "ببب سسس ششش صصص ضضض كتب مكتب",
    "موعدنا يوم الثلاثاء الساعة 10:30 صباحًا.",
    "A\u{2067}شعار 12\u{2069}Z نص عربي واضح",
];
#[test]
fn justification_matrix_two_bundled_arabic_fonts() {
    let mut count = 0;
    let mut inserted_count = 0;
    for noto in [false, true] {
        let before_font = inserted_count;
        let mut engine = Engine::new(fonts(noto)).unwrap();
        for text in CASES {
            for size in [12., 48., 200.] {
                let mut req = Request::new(text, size, None);
                req.face = ARABIC;
                let baseline = engine.layout(&req).unwrap();
                let natural = baseline.lines[0].width;
                for slack in [size * 0.3, size, size * 3.] {
                    req.width = Some(natural + slack);
                    for policy in [Kashida::Off, Kashida::Minimal, Kashida::Balanced, Kashida::Display] {
                        let options = ParagraphOptions {
                            alignment: Alignment::JustifyFull,
                            kashida: policy,
                            ..Default::default()
                        };
                        let layout = engine.compose(&req, &options).unwrap();
                        assert_eq!(layout.source, text);
                        assert_eq!(layout.lines.len(), 1);
                        let line = &layout.lines[0];
                        let advances: f32 = line.glyphs.iter().map(|g| g.advance).sum();
                        assert!((line.width - req.width.unwrap()).abs() <= 0.5, "{text} {policy:?} {line:?}");
                        assert!((advances - line.width).abs() <= 0.5, "advance mismatch {advances} {}", line.width);
                        for issue in &layout.issues {
                            if let Issue::KashidaInserted { byte, .. } = issue {
                                inserted_count += 1;
                                assert!(
                                    !baseline.lines[0]
                                        .glyphs
                                        .iter()
                                        .any(|g| g.cluster.start < *byte && *byte < g.cluster.end),
                                    "inside shaped ligature"
                                );
                                let before = text[..*byte].chars().next_back().unwrap();
                                let after = text[*byte..].chars().next().unwrap();
                                assert!(joining_allows(before, after));
                                assert!(!is_harakat(before));
                                assert!(!is_harakat(after));
                                assert!(text.grapheme_indices(true).any(|(i, _)| i == *byte));
                            }
                            assert!(
                                !matches!(issue, Issue::JustificationResidual { .. } | Issue::UnsupportedCluster(_)),
                                "{text:?} noto={noto} {issue:?}"
                            );
                        }
                        for caret in &layout.carets {
                            assert!(text
                                .grapheme_indices(true)
                                .map(|(i, _)| i)
                                .chain([text.len()])
                                .any(|b| b == caret.byte));
                            assert!(layout.hit(caret.line, caret.x).contains(&caret));
                        }
                        count += 1;
                    }
                }
            }
        }
        assert!(inserted_count > before_font, "both Arabic fonts must exercise safe insertion");
    }
    assert_eq!(count, 576);
    assert!(inserted_count > 0, "safe flag plumbing must actually insert kashida");
    eprintln!("Arabic justification: {count} configurations, {inserted_count} virtual insertions");
}
#[test]
fn composer_breaks_are_legal_cover_source_and_keep_nbsp() {
    let mut e = engine();
    for composer in [Composer::Greedy, Composer::EveryLine] {
        for direction in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
            for text in CASES.into_iter().chain(["A\u{a0}B، شعار\r\nسلام\n", "عربي   "]) {
                for width in [70., 180., 450.] {
                    let mut r = Request::new(text, 24., Some(width));
                    r.direction = direction;
                    let l = e.compose(&r, &ParagraphOptions { composer, ..Default::default() }).unwrap();
                    assert_eq!(l.source, text);
                    let mut end = 0;
                    let legal: Vec<_> = unicode_linebreak::linebreaks(text).map(|(b, _)| b).collect();
                    for line in &l.lines {
                        assert_eq!(line.range.start, end);
                        end = line.range.end;
                        assert!(end == text.len() || legal.contains(&end), "{text:?}: {end}");
                        for g in &line.glyphs {
                            assert!(g.cluster.start >= line.range.start && g.cluster.end <= line.range.end);
                        }
                        assert!(line.width <= width + 0.5 || l.issues.iter().any(|i| matches!(i, Issue::Overflow(_))));
                    }
                    assert_eq!(end, text.len());
                }
            }
        }
    }
}
#[test]
fn every_line_is_measured_and_distinct_from_greedy() {
    let mut e = engine();
    let text = "السلام عليكم ورحمة الله، هذا نص عربي طويل لاختبار توزيع الكلمات على الأسطر بشكل متوازن وجميل.";
    let mut differing = 0;
    for width in [120., 140., 160., 180., 200., 220., 240., 260.] {
        let r = Request::new(text, 24., Some(width));
        let a = e.compose(&r, &ParagraphOptions { composer: Composer::Greedy, ..Default::default() }).unwrap();
        let b = e.compose(&r, &ParagraphOptions { composer: Composer::EveryLine, ..Default::default() }).unwrap();
        if width == 180. {
            let measured = [
                (0..24, 122.688),
                (24..53, 142.992),
                (53..76, 140.568),
                (76..102, 111.888),
                (102..124, 116.184),
                (124..146, 124.920),
                (146..170, 132.528),
            ];
            assert_eq!(b.lines.len(), measured.len());
            for (line, (range, width)) in b.lines.iter().zip(measured) {
                assert_eq!(line.range, range);
                assert!((line.width - width).abs() < 0.001);
            }
        }
        if a.lines.iter().map(|l| l.range.clone()).collect::<Vec<_>>()
            != b.lines.iter().map(|l| l.range.clone()).collect::<Vec<_>>()
        {
            differing += 1;
        }
    }
    assert!(differing > 0, "total-fit must actually choose different breaks");
}
#[test]
fn alignment_modes_and_last_line_contract() {
    let mut e = engine();
    let r = Request::new("شعار عربي جميل", 24., Some(400.));
    for (alignment, factor) in [
        (Alignment::Left, 0.),
        (Alignment::Centre, 0.5),
        (Alignment::Right, 1.),
        (Alignment::JustifyLastLeft, 0.),
        (Alignment::JustifyLastCentre, 0.5),
        (Alignment::JustifyLastRight, 1.),
    ] {
        let l = e.compose(&r, &ParagraphOptions { alignment, ..Default::default() }).unwrap();
        let line = &l.lines[0];
        let left = line.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        assert!((left - (400. - line.width) * factor).abs() < 0.01, "{alignment:?}: {left}");
    }
}
#[test]
fn lam_alef_marks_digits_and_latin_unchanged_by_elongation() {
    for noto in [false, true] {
        let mut e = Engine::new(fonts(noto)).unwrap();
        let text = "لا لأ لإ لآ سسس مكتب السَّلَامُ USD 12.50 ١٢٣";
        let mut req = Request::new(text, 48., None);
        req.face = ARABIC;
        let original = e.layout(&req).unwrap();
        req.width = Some(original.lines[0].width + 100.);
        let stretched = e
            .compose(
                &req,
                &ParagraphOptions {
                    alignment: Alignment::JustifyFull,
                    kashida: Kashida::Display,
                    ..Default::default()
                },
            )
            .unwrap();
        for g in &original.lines[0].glyphs {
            let source = &text[g.cluster.clone()];
            if g.cluster.end <= text.find("سسس").unwrap() && !source.trim().is_empty()
                || source.chars().any(|c| c.is_ascii_alphanumeric() || ('٠'..='٩').contains(&c))
            {
                assert!(
                    stretched.lines[0]
                        .glyphs
                        .iter()
                        .any(|h| h.cluster == g.cluster && h.id == g.id && (h.advance - g.advance).abs() < 0.001),
                    "{source}"
                );
            }
        }
    }
}
#[test]
fn impossible_single_word_and_invalid_policy_are_explicit() {
    let mut e = engine();
    let r = Request::new("لا", 24., Some(400.));
    let l = e
        .compose(
            &r,
            &ParagraphOptions { alignment: Alignment::JustifyFull, kashida: Kashida::Display, ..Default::default() },
        )
        .unwrap();
    assert!(l.issues.iter().any(|i| matches!(i, Issue::JustificationResidual { .. })));
    assert!(!l.issues.iter().any(|i| matches!(i, Issue::KashidaInserted { .. })));
    assert!(e
        .compose(&r, &ParagraphOptions { line_height: LineHeight::Multiple(f32::NAN), ..Default::default() })
        .is_err());
    let r = Request::new("عربي جميل طويل", 24., Some(40.));
    assert_eq!(
        e.compose(&r, &ParagraphOptions { max_measurements: 1, ..Default::default() }).unwrap_err(),
        "paragraph measurement limit"
    );
}
#[test]
fn source_working_mapping_and_frame_constraints() {
    let s = "بب سلام";
    let w = WorkingText::new(s, &[2, 2]).unwrap();
    assert_eq!(w.text, "بــب سلام");
    assert_eq!(w.source_at.len(), w.text.len() + 1);
    assert_eq!(&w.source_at[2..6], &[2, 2, 2, 2]);
    assert_eq!(w.source_at.last(), Some(&s.len()));
    assert!(WorkingText::new(s, &[1]).is_err());
    let frames = FramePolicy::default();
    assert_eq!(frames.split(5, 4), 3);
    assert_eq!(frames.split(3, 2), 0);
    assert_eq!(frames.split(4, 4), 4);
    struct Good;
    impl HyphenationProvider for Good {
        fn opportunities(&self, _: &str) -> Vec<usize> {
            vec![2, 4]
        }
    }
    assert_eq!(hyphenation_points("abcdef", &Good).unwrap(), vec![2, 4]);
    struct Bad;
    impl HyphenationProvider for Bad {
        fn opportunities(&self, _: &str) -> Vec<usize> {
            vec![1]
        }
    }
    assert!(hyphenation_points("عَرَبِي", &Bad).is_err());
}
#[test]
fn mark_ink_fits_line_boxes_across_metrics_modes() {
    let mut e = engine();
    let r = Request::new("السَّلَامُ عَلَيْكُمْ\nقُرْآنٌ كَرِيمٌ Logo\nشعار", 48., Some(700.));
    for line_height in [LineHeight::Font, LineHeight::Multiple(1.), LineHeight::Multiple(1.6), LineHeight::AtLeast(10.)]
    {
        let l = e.compose(&r, &ParagraphOptions { line_height, ..Default::default() }).unwrap();
        for line in &l.lines {
            assert!(line.ascent + line.descent >= 48. * 1.3 - 0.01);
            for g in &line.glyphs {
                if let Some(b) = outlines::glyph_outline(e.font_set(), g).unwrap().ink_bounds() {
                    assert!(b[1] >= line.baseline - line.ascent - 0.01);
                    assert!(b[3] <= line.baseline + line.descent + 0.01);
                }
            }
        }
        for pair in l.lines.windows(2) {
            assert!(pair[0].baseline + pair[0].descent <= pair[1].baseline - pair[1].ascent + 0.01);
        }
    }
}
#[test]
fn split_carets_preserve_logical_visual_identity() {
    let mut e = engine();
    let r = Request::new("شعار Logo 12، نعم؟", 32., None);
    let l = e.compose(&r, &ParagraphOptions::default()).unwrap();
    let split = l
        .carets
        .iter()
        .any(|a| l.carets.iter().any(|b| a.byte == b.byte && a.affinity != b.affinity && (a.x - b.x).abs() > 1.));
    assert!(split, "direction boundaries need split carets");
    for c in &l.carets {
        assert!(l.hit(c.line, c.x).contains(&c));
    }
    assert_eq!(l.copy(0..r.text.len()), Some(r.text));
}
#[test]
fn mixed_size_style_shift_and_empty_caret_follow_composed_metrics() {
    let mut e = engine();
    let text = "Logo قُرْآنٌ";
    let mut r = Request::new(text, 24., Some(500.));
    r.styles = vec![Style { range: 0..4, face: LATIN, size: 60., baseline_shift: 10., paint: 0 }];
    let l = e.compose(&r, &ParagraphOptions::default()).unwrap();
    let line = &l.lines[0];
    for g in &line.glyphs {
        let expected = line.baseline - if g.cluster.start < 4 { 10. } else { 0. };
        assert!((g.y - expected).abs() < 0.001);
        if let Some(b) = outlines::glyph_outline(e.font_set(), g).unwrap().ink_bounds() {
            assert!(b[1] >= line.baseline - line.ascent - 0.001);
            assert!(b[3] <= line.baseline + line.descent + 0.001);
        }
    }
    for (alignment, x) in [(Alignment::Left, 0.), (Alignment::Centre, 100.), (Alignment::Right, 200.)] {
        let mut r = Request::new("", 24., Some(200.));
        r.direction = Direction::Rtl;
        let l = e.compose(&r, &ParagraphOptions { alignment, ..Default::default() }).unwrap();
        assert!(l.carets.iter().all(|c| (c.x - x).abs() < 0.001));
    }
}
#[test]
fn every_grapheme_has_a_caret_after_wrapping_and_justification() {
    let mut e = engine();
    for text in CASES.into_iter().chain(["A\u{a0}B عربي   ", "سلام\r\nعربي\n"]) {
        for direction in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
            let mut r = Request::new(text, 24., Some(180.));
            r.direction = direction;
            let l = e
                .compose(
                    &r,
                    &ParagraphOptions {
                        alignment: Alignment::JustifyFull,
                        kashida: Kashida::Balanced,
                        ..Default::default()
                    },
                )
                .unwrap();
            for byte in text.grapheme_indices(true).map(|(i, _)| i).chain([text.len()]) {
                assert!(l.carets.iter().any(|c| c.byte == byte), "missing source boundary {byte} in {text:?}");
            }
            for line in &l.lines {
                // UBA orders clusters, not the base/virtual-stroke glyphs within one.
                let lefts: Vec<_> = line
                    .glyphs
                    .chunk_by(|a, b| a.cluster.start == b.cluster.start)
                    .map(|group| group.iter().map(|g| g.x).fold(f32::INFINITY, f32::min))
                    .collect();
                assert!(lefts.windows(2).all(|x| x[0] <= x[1] + 0.01), "cluster visual order {text:?}");
            }
        }
    }
}

#[test]
fn discretionary_hyphens_are_measured_rendered_and_source_preserving() {
    struct Dictionary;
    impl HyphenationProvider for Dictionary {
        fn opportunities(&self, word: &str) -> Vec<usize> {
            if word == "abcdefgh" {
                vec![4, 4]
            } else {
                vec![]
            }
        }
    }
    let mut e = engine();
    let oracle = e.layout(&Request::new("abcd-", 24., None)).unwrap();
    let width = oracle.lines[0].width + 0.1;
    for composer in [Composer::Greedy, Composer::EveryLine] {
        for alignment in [Alignment::Left, Alignment::JustifyFull] {
            let options = ParagraphOptions { composer, alignment, ..Default::default() };
            let text = "abcdefgh\nabcdefgh";
            let r = Request::new(text, 24., Some(width));
            let l = e.compose_with_hyphenation(&r, &options, Some(&Dictionary)).unwrap();
            assert_eq!(l.copy(0..text.len()), Some(text));
            assert_eq!(l.lines.len(), 4);
            assert_eq!(l.issues.iter().filter(|i| matches!(i, Issue::DiscretionaryHyphen { .. })).count(), 2);
            assert!(l.issues.contains(&Issue::DiscretionaryHyphen { byte: 4 }));
            assert!(l.issues.contains(&Issue::DiscretionaryHyphen { byte: 13 }));
            for line in [&l.lines[0], &l.lines[2]] {
                assert!((line.width - oracle.lines[0].width).abs() < 0.001);
                assert_eq!(line.glyphs.len(), oracle.lines[0].glyphs.len());
                for (g, expected) in line.glyphs.iter().zip(&oracle.lines[0].glyphs) {
                    assert_eq!(g.id, expected.id);
                    assert!((g.advance - expected.advance).abs() < 0.001);
                    assert!(g.cluster.start >= line.range.start && g.cluster.end <= line.range.end);
                    assert!(!g.cluster.is_empty());
                }
            }
            assert!(!l.issues.iter().any(|i| matches!(i, Issue::Overflow(_))));
            for byte in 0..=text.len() {
                assert!(l.carets.iter().any(|c| c.byte == byte));
            }
            let no_provider = e.compose(&r, &options).unwrap();
            assert_eq!(no_provider.lines.len(), 2);
            assert!(no_provider.issues.iter().any(|i| matches!(i, Issue::Overflow(_))));
            for wide_width in [None, Some(1000.)] {
                let wide = e
                    .compose_with_hyphenation(&Request::new("abcdefgh", 24., wide_width), &options, Some(&Dictionary))
                    .unwrap();
                assert_eq!(wide.lines.len(), 1);
                assert!(!wide.issues.iter().any(|i| matches!(i, Issue::DiscretionaryHyphen { .. })));
            }
        }
        let narrow = e.layout(&Request::new("abcd", 24., None)).unwrap().lines[0].width + 0.1;
        let l = e
            .compose_with_hyphenation(
                &Request::new("abcdefgh", 24., Some(narrow)),
                &ParagraphOptions { composer, ..Default::default() },
                Some(&Dictionary),
            )
            .unwrap();
        assert!(l.issues.iter().any(|i| matches!(i, Issue::Overflow(_))));
        assert!(l.lines[0].width > narrow);
        assert_eq!(
            e.compose_with_hyphenation(
                &Request::new("abcdefgh", 24., Some(width)),
                &ParagraphOptions { composer, max_measurements: 1, ..Default::default() },
                Some(&Dictionary)
            )
            .unwrap_err(),
            "paragraph measurement limit"
        );
    }
}
#[test]
fn composers_reject_invalid_provider_grapheme_breaks() {
    struct Bad;
    impl HyphenationProvider for Bad {
        fn opportunities(&self, _: &str) -> Vec<usize> {
            vec![1]
        }
    }
    let mut e = engine();
    for composer in [Composer::Greedy, Composer::EveryLine] {
        for text in ["عَرَبِي", "a\u{301}bc"] {
            assert_eq!(
                e.compose_with_hyphenation(
                    &Request::new(text, 24., Some(40.)),
                    &ParagraphOptions { composer, ..Default::default() },
                    Some(&Bad)
                )
                .unwrap_err(),
                "hyphenation provider returned a non-grapheme boundary"
            );
        }
    }
}
#[test]
fn long_arabic_and_space_heavy_lines_preserve_justification() {
    let mut e = engine();
    for text in ["سسس".repeat(4000), "سلام A ".repeat(2000)] {
        let mut r = Request::new(&text, 24., None);
        let natural = e.layout(&r).unwrap().lines[0].width;
        r.width = Some(natural + 100.);
        let l = e
            .compose(
                &r,
                &ParagraphOptions { alignment: Alignment::JustifyFull, kashida: Kashida::Off, ..Default::default() },
            )
            .unwrap();
        assert_eq!(l.lines.len(), 1);
        assert_eq!(l.source, text);
        assert!(!l.issues.iter().any(|i| matches!(i, Issue::KashidaInserted { .. })));
        let expected = if text.contains(' ') { natural + 100. } else { natural };
        assert!((l.lines[0].width - expected).abs() < 0.5);
    }
}

#[test]
fn overflowing_discretionary_edge_does_not_hide_a_fitting_whole_word() {
    struct Dictionary;
    impl HyphenationProvider for Dictionary {
        fn opportunities(&self, _: &str) -> Vec<usize> {
            vec![2, 3]
        }
    }
    let mut e = engine();
    let whole = e.layout(&Request::new("abci", 24., None)).unwrap();
    let hyphen = e.layout(&Request::new("abc-", 24., None)).unwrap();
    let width = whole.lines[0].width + 0.001;
    assert!(hyphen.lines[0].width > width);
    for composer in [Composer::Greedy, Composer::EveryLine] {
        let l = e
            .compose_with_hyphenation(
                &Request::new("abci", 24., Some(width)),
                &ParagraphOptions { composer, ..Default::default() },
                Some(&Dictionary),
            )
            .unwrap();
        assert_eq!(l.lines.len(), 1);
        assert_eq!(l.lines[0].range, 0..4);
        assert!(!l.issues.iter().any(|i| matches!(i, Issue::DiscretionaryHyphen { .. } | Issue::Overflow(_))));
    }
}
