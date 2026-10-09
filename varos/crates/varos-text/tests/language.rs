mod common;
use common::*;
use cosmic_text::{harfrust as hb, Attrs, AttrsList, Family, FontSystem, ShapeWord, Shaping};
use varos_text::*;

fn fonts() -> FontSystem {
    let mut db = fontdb::Database::new();
    db.load_font_data(PLEX.to_vec());
    FontSystem::new_with_locale_and_db_and_fallback("en-US".into(), db, TestFallback)
}
fn compare(
    fs: &mut FontSystem,
    text: &str,
    range: std::ops::Range<usize>,
    lang: Option<&str>,
    script: Option<hb::Script>,
    rtl: bool,
) -> Vec<u16> {
    let attrs = Attrs::new()
        .family(Family::Name("IBM Plex Sans Arabic"))
        .language(lang.map(|s| s.parse().unwrap()))
        .script(script);
    let word = ShapeWord::new(
        fs,
        text,
        &AttrsList::new(&attrs),
        range.clone(),
        if rtl { unicode_bidi::Level::rtl() } else { unicode_bidi::Level::ltr() },
        false,
        Shaping::Advanced,
    );
    let font = hb::FontRef::new(PLEX).unwrap();
    let data = hb::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let mut buffer = hb::UnicodeBuffer::new();
    buffer.set_flags(hb::BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL);
    buffer.push_str(&text[range.clone()]);
    buffer.set_pre_context(&text[..range.start]);
    buffer.set_post_context(&text[range.end..]);
    buffer.set_direction(if rtl { hb::Direction::RightToLeft } else { hb::Direction::LeftToRight });
    if let Some(l) = lang {
        buffer.set_language(l.parse().unwrap());
    }
    if let Some(s) = script {
        buffer.set_script(s);
    }
    buffer.guess_segment_properties();
    let shaped = shaper.shape(buffer, &[]);
    assert_eq!(word.glyphs.len(), shaped.glyph_infos().len());
    for ((a, b), p) in word.glyphs.iter().zip(shaped.glyph_infos()).zip(shaped.glyph_positions()) {
        assert_eq!(u32::from(a.glyph_id), b.glyph_id, "{text:?}/{lang:?}/{script:?}");
        assert_eq!(a.start, range.start + b.cluster as usize);
        assert_eq!(
            a.safe_to_insert_tatweel,
            b.safe_to_insert_tatweel(),
            "safe flag must survive shaping and cache hits"
        );
        let em = shaper.units_per_em() as f32;
        assert!((a.x_advance - p.x_advance as f32 / em).abs() < 0.00001);
        assert!((a.x_offset - p.x_offset as f32 / em).abs() < 0.00001);
        assert!((a.y_offset - p.y_offset as f32 / em).abs() < 0.00001);
    }
    word.glyphs.iter().map(|g| g.glyph_id).collect()
}
#[test]
fn language_script_oracle_and_cache_alternation() {
    let mut fs = fonts();
    for _ in 0..3 {
        for text in ["سلام", "۶", "السَّلَامُ", "می\u{200c}روم", "office"] {
            for language in [Some("ar"), Some("fa"), Some("ur"), Some("und"), None] {
                for script in [None, Some(hb::script::ARABIC), Some(hb::script::LATIN)] {
                    for rtl in [true, false] {
                        compare(&mut fs, text, 0..text.len(), language, script, rtl);
                    }
                }
            }
        }
    }
    // Audited Plex GSUB arab/URD locl lookup 6: uni06F6 -> uni0666.
    let ar = compare(&mut fs, "۶", 0..2, Some("ar"), Some(hb::script::ARABIC), true);
    let ur = compare(&mut fs, "۶", 0..2, Some("ur"), Some(hb::script::ARABIC), true);
    assert_ne!(ar, ur, "real localized-form fixture must contrast");
}
#[test]
fn language_boundary_context_and_owned_attrs() {
    let mut fs = fonts();
    let text = "ببب";
    for lang in ["ar", "fa", "ur", "und"] {
        compare(&mut fs, text, 2..4, Some(lang), None, true);
        // Same text with different surrounding context must not hit the wrong run cache.
        compare(&mut fs, " ب ", 1..3, Some(lang), None, true);
    }
    let a = Attrs::new().language(Some("ur".parse().unwrap())).script(Some(hb::script::ARABIC));
    assert_eq!(a, cosmic_text::AttrsOwned::new(&a).as_attrs());
    let mut list = AttrsList::new(&a);
    list.add_span(2..4, &a.clone().language(Some("fa".parse().unwrap())));
    let split =
        ShapeWord::new(&mut fs, text, &list, 0..text.len(), unicode_bidi::Level::rtl(), false, Shaping::Advanced);
    let full = ShapeWord::new(
        &mut fs,
        text,
        &AttrsList::new(&a),
        0..text.len(),
        unicode_bidi::Level::rtl(),
        false,
        Shaping::Advanced,
    );
    let ids = |w: cosmic_text::ShapeWord| {
        let mut v: Vec<_> = w.glyphs.iter().map(|g| (g.start, g.glyph_id)).collect();
        v.sort();
        v
    };
    assert_eq!(ids(split), ids(full));
}
#[test]
fn requested_fallback_preserves_language() {
    let mut e = engine();
    for language in ["ar", "fa", "ur", "und"] {
        let mut r = Request::new("۶", 48., None);
        r.language = language;
        r.script = Some("Arab".parse().unwrap());
        let fallback = e.layout(&r).unwrap();
        r.face = ARABIC;
        let direct = e.layout(&r).unwrap();
        assert_eq!(fallback.lines, direct.lines);
    }
}

#[test]
fn engine_mixed_language_runs_and_rejection() {
    let mut e = engine();
    let mut r = Request::new("۶۶", 48., None);
    r.face = ARABIC;
    r.language = "ar";
    r.script = Some("Arab".parse().unwrap());
    r.language_runs = vec![LanguageRun { range: 0..2, language: "ur".into(), script: Some("Arab".parse().unwrap()) }];
    let l = e.layout(&r).unwrap();
    let mut glyphs: Vec<_> = l.lines.iter().flat_map(|l| &l.glyphs).collect();
    glyphs.sort_by_key(|g| g.cluster.start);
    assert_ne!(glyphs[0].id, glyphs[1].id);
    r.language_runs.clear();
    let a = e.layout(&r).unwrap();
    assert_eq!(a, e.layout(&r).unwrap());
    r.language = "not_a_tag";
    assert_eq!(e.layout(&r), Err("invalid language"));
}

#[test]
fn real_arabic_font_boundary_matches_context_oracles() {
    let noto = include_bytes!("../assets/fonts/NotoSansArabic.ttf");
    let mut db = fontdb::Database::new();
    db.load_font_data(PLEX.to_vec());
    db.load_font_data(noto.to_vec());
    let mut fs = FontSystem::new_with_locale_and_db_and_fallback("en-US".into(), db, TestFallback);
    let text = "ببب";
    let plex = Attrs::new().family(Family::Name("IBM Plex Sans Arabic")).language(Some("ar".parse().unwrap()));
    let mut list = AttrsList::new(&plex);
    list.add_span(2..4, &plex.clone().family(Family::Name("Noto Sans Arabic")));
    let mixed = ShapeWord::new(&mut fs, text, &list, 0..6, unicode_bidi::Level::rtl(), false, Shaping::Advanced);
    for (range, bytes) in [(0..2, PLEX), (2..4, noto.as_slice()), (4..6, PLEX)] {
        let font = hb::FontRef::new(bytes).unwrap();
        let data = hb::ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let mut buffer = hb::UnicodeBuffer::new();
        buffer.push_str(&text[range.clone()]);
        buffer.set_pre_context(&text[..range.start]);
        buffer.set_post_context(&text[range.end..]);
        buffer.set_language("ar".parse().unwrap());
        buffer.set_direction(hb::Direction::RightToLeft);
        buffer.guess_segment_properties();
        let oracle = shaper.shape(buffer, &[]);
        let glyphs: Vec<_> = mixed.glyphs.iter().filter(|g| range.contains(&g.start)).collect();
        assert_eq!(glyphs.len(), oracle.glyph_infos().len());
        for ((g, info), pos) in glyphs.iter().zip(oracle.glyph_infos()).zip(oracle.glyph_positions()) {
            assert_eq!(u32::from(g.glyph_id), info.glyph_id);
            assert!((g.x_advance - pos.x_advance as f32 / shaper.units_per_em() as f32).abs() < 0.00001);
        }
    }
    assert_ne!(mixed.glyphs[0].font_id, mixed.glyphs[1].font_id);
}

#[test]
fn cosmic_fallback_retry_keeps_run_properties() {
    let mut db = fontdb::Database::new();
    db.load_font_data(INTER.to_vec());
    db.load_font_data(PLEX.to_vec());
    let plex_id = db.faces().find(|f| f.families.iter().any(|x| x.0 == "IBM Plex Sans Arabic")).unwrap().id;
    let mut fs = FontSystem::new_with_locale_and_db_and_fallback("en-US".into(), db, TestFallback);
    for language in ["ar", "ur", "fa", "und", "ur"] {
        let text = "۶";
        let expected = compare(&mut fs, text, 0..2, Some(language), Some(hb::script::ARABIC), true);
        let attrs = Attrs::new()
            .family(Family::Name("Inter"))
            .language(Some(language.parse().unwrap()))
            .script(Some(hb::script::ARABIC));
        let word = ShapeWord::new(
            &mut fs,
            text,
            &AttrsList::new(&attrs),
            0..2,
            unicode_bidi::Level::rtl(),
            false,
            Shaping::Advanced,
        );
        assert!(word.glyphs.iter().all(|g| g.font_id == plex_id));
        assert_eq!(word.glyphs.iter().map(|g| g.glyph_id).collect::<Vec<_>>(), expected);
    }
}

#[test]
fn common_digits_inherit_paragraph_script_across_bidi_runs() {
    let mut engine = engine();
    let mut r = Request::new("ب123ب", 48., None);
    r.face = ARABIC;
    r.language = "ar";
    let layout = engine.layout(&r).unwrap();
    let mut fs = fonts();
    let expected = compare(&mut fs, r.text, 2..5, Some("ar"), Some(hb::script::ARABIC), false);
    let mut actual: Vec<_> = layout
        .lines
        .iter()
        .flat_map(|l| &l.glyphs)
        .filter(|g| (2..5).contains(&g.cluster.start))
        .map(|g| (g.cluster.start, g.id))
        .collect();
    actual.sort();
    assert_eq!(actual.into_iter().map(|(_, id)| id).collect::<Vec<_>>(), expected);
    r.script = Some("Latn".parse().unwrap());
    let changed = engine.layout(&r).unwrap();
    assert_ne!(layout.lines, changed.lines);
}

#[test]
fn positioned_marks_match_direct_oracle_in_both_paragraph_directions() {
    let font = hb::FontRef::new(PLEX).unwrap();
    let data = hb::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    for text in ["السَّلَامُ", "بِبَّ", "قُرْآنٌ"] {
        let mut b = hb::UnicodeBuffer::new();
        b.push_str(text);
        b.set_script(hb::script::ARABIC);
        b.set_language("ar".parse().unwrap());
        b.set_direction(hb::Direction::RightToLeft);
        let oracle = shaper.shape(b, &[]);
        let scale = 48. / shaper.units_per_em() as f32;
        let mut x = 0.;
        let mut expected = Vec::new();
        for (g, p) in oracle.glyph_infos().iter().zip(oracle.glyph_positions()) {
            expected.push((
                g.cluster as usize,
                g.glyph_id,
                x + p.x_offset as f32 * scale,
                -p.y_offset as f32 * scale,
                p.x_advance as f32 * scale,
            ));
            x += p.x_advance as f32 * scale;
        }
        expected.sort_by_key(|g| (g.0, g.1));
        for direction in [Direction::Ltr, Direction::Rtl] {
            let mut r = Request::new(text, 48., None);
            r.face = ARABIC;
            r.language = "ar";
            r.direction = direction;
            let l = engine().layout(&r).unwrap();
            let baseline = l.lines[0].baseline;
            let mut actual: Vec<_> = l.lines[0]
                .glyphs
                .iter()
                .map(|g| (g.cluster.start, u32::from(g.id), g.x + g.offset[0], g.y + g.offset[1] - baseline, g.advance))
                .collect();
            actual.sort_by_key(|g| (g.0, g.1));
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(&expected) {
                assert_eq!((a.0, a.1), (b.0, b.1));
                assert!(
                    (a.2 - b.2).abs() < 0.001 && (a.3 - b.3).abs() < 0.001 && (a.4 - b.4).abs() < 0.001,
                    "{text:?}/{direction:?}: actual={a:?}, oracle={b:?}"
                );
            }
        }
    }
}

struct TestFallback;
impl cosmic_text::Fallback for TestFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &["Inter", "IBM Plex Sans Arabic"]
    }
    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }
    fn script_fallback(&self, _: unicode_script::Script, _: &str) -> &[&'static str] {
        &[]
    }
}
