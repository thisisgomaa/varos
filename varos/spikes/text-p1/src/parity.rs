//! Same byte-fed fixtures for native and wasm32-unknown-unknown/Node.
use crate::{outlines::*, proof::*, *};
use std::fmt::Write;
pub fn fixtures() -> String {
    let mut engine = Engine::default();
    let mut out = String::new();
    let mut index = 0;
    for row in &CORPUS {
        for text in row.texts {
            for size in SIZES_PT {
                for width in [None, Some(120.), Some(600.)] {
                    for face in [Face::Inter, Face::Plex] {
                        for direction in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
                            let mut r = Request::new(text, size, width);
                            r.face = face;
                            r.direction = direction;
                            record(&mut engine, &r, index, &mut out);
                            index += 1;
                        }
                    }
                }
            }
        }
    }
    for language in ["ar", "fa", "ur", "und"] {
        for script in [None, Some(cosmic_text::harfrust::script::ARABIC), Some(cosmic_text::harfrust::script::LATIN)] {
            let mut r = Request::new("۶ سلام", 48., Some(120.));
            r.language = language;
            r.script = script;
            record(&mut engine, &r, index, &mut out);
            index += 1;
        }
    }
    for text in [
        "ب123ب",
        "A\u{a0}شعار",
        "A (12) — شعار!",
        "ب\u{200b}ب\u{200b}ب",
        "السَّلَامُ",
        "بِبَّ",
        "قُرْآنٌ",
        "A\nB\r\nC\rD\u{2028}E\u{2029}F",
        "A\u{2066}B\u{2069}:C",
    ] {
        for direction in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
            let mut r = Request::new(text, 48., Some(120.));
            r.direction = direction;
            record(&mut engine, &r, index, &mut out);
            index += 1;
        }
    }
    for language in ["ar", "fa", "ur", "und"] {
        let mut r = Request::new("۶۶", 48., Some(120.));
        r.language_runs = vec![LanguageRun {
            range: 0..2,
            language: language.into(),
            script: Some(cosmic_text::harfrust::script::ARABIC),
        }];
        record(&mut engine, &r, index, &mut out);
        index += 1;
    }
    context_records(&mut out);
    out
}
fn context_records(out: &mut String) {
    use cosmic_text::{harfrust as hb, Attrs, AttrsList, Family, FontSystem, ShapeWord, Shaping};
    let mut db = fontdb::Database::new();
    db.load_font_data(INTER.to_vec());
    db.load_font_data(PLEX.to_vec());
    db.load_font_data(include_bytes!("../vendor/cosmic-text/fonts/NotoSansArabic.ttf").to_vec());
    let mut fs = FontSystem::new_with_locale_and_db_and_fallback("en-US".into(), db, PinnedFallback);
    for lang in [Some("ar"), Some("fa"), Some("ur"), Some("und"), None] {
        for script in [None, Some(hb::script::ARABIC), Some(hb::script::LATIN)] {
            for text in ["ببب", "۶"] {
                let a = Attrs::new()
                    .family(Family::Name("Inter"))
                    .language(lang.map(|s| s.parse().unwrap()))
                    .script(script);
                let mut list = AttrsList::new(&a);
                if text == "ببب" {
                    list.add_span(2..4, &a.clone().family(Family::Name("Noto Sans Arabic")));
                }
                let word = ShapeWord::new(
                    &mut fs,
                    text,
                    &list,
                    0..text.len(),
                    unicode_bidi::Level::rtl(),
                    false,
                    Shaping::Advanced,
                );
                writeln!(out, "context\t{text}\t{lang:?}\t{script:?}").unwrap();
                for g in word.glyphs {
                    writeln!(
                        out,
                        "context-glyph\t{}\t{}\t{}\t{}",
                        g.glyph_id,
                        g.start,
                        g.end,
                        fs.db().face(g.font_id).unwrap().post_script_name
                    )
                    .unwrap();
                    writeln!(out, "float\t{:.9}\t{:.9}\t{:.9}", g.x_advance, g.x_offset, g.y_offset).unwrap();
                }
            }
        }
    }
}
fn record(engine: &mut Engine, r: &Request<'_>, i: usize, out: &mut String) {
    let l = engine.layout(r).unwrap();
    writeln!(
        out,
        "fixture\t{i}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}",
        r.text, r.size, r.width, r.face, r.direction, r.language, r.script
    )
    .unwrap();
    writeln!(out, "issues\t{:?}", l.issues).unwrap();
    writeln!(out, "levels\t{:?}", l.levels).unwrap();
    for line in &l.lines {
        writeln!(out, "line\t{}\t{}\t{}", line.range.start, line.range.end, line.rtl).unwrap();
        writeln!(out, "float\t{:.9}\t{:.9}\t{:.9}\t{:.9}", line.width, line.baseline, line.ascent, line.descent)
            .unwrap();
        for g in &line.glyphs {
            writeln!(out, "glyph\t{}\t{:?}\t{}\t{}\t{}", g.id, g.face, g.cluster.start, g.cluster.end, g.level)
                .unwrap();
            writeln!(
                out,
                "float\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}",
                g.x, g.y, g.advance, g.offset[0], g.offset[1], g.size
            )
            .unwrap();
        }
    }
    for c in &l.carets {
        writeln!(out, "caret\t{}\t{:?}\t{}", c.byte, c.affinity, c.line).unwrap();
        writeln!(out, "float\t{:.9}", c.x).unwrap();
    }
    // Fixed 2x pixel policy: 4x4-sampled native NonZero max coverage, mean<=1
    // and no delta>32. Record alpha bytes; compare externally before verdict.
    let outlines = layout_outlines(&l).unwrap();
    let b = ink_bounds(&outlines).unwrap_or([0., 0., 1., 1.]);
    let w = ((b[2] - b[0]) * 2.).ceil() as u32 + 12;
    let h = ((b[3] - b[1]) * 2.).ceil() as u32 + 12;
    let pix = nonzero_coverage(
        &outlines,
        w,
        h,
        tiny_skia::Transform::from_row(2., 0., 0., 2., 6. - b[0] * 2., 6. - b[1] * 2.),
        None,
        255,
    )
    .unwrap();
    writeln!(out, "pixels\t{w}\t{h}").unwrap();
    // Run-length encoding keeps blank proof margins cheap without hashing away tolerance evidence.
    let mut previous = 0;
    let mut count = 0;
    out.push_str("alpha");
    for p in pix.pixels() {
        let a = p.alpha();
        if a == previous {
            count += 1;
        } else {
            write!(out, "\t{previous}:{count}").unwrap();
            previous = a;
            count = 1;
        }
    }
    writeln!(out, "\t{previous}:{count}").unwrap();
}
/// ABI: returned pointer starts with a little-endian u32 byte length, followed
/// by UTF-8 fixture records. One invocation per fresh runtime, memory retained.
#[no_mangle]
pub extern "C" fn p1b_fixtures() -> *const u8 {
    let s = fixtures();
    let mut data = (s.len() as u32).to_le_bytes().to_vec();
    data.extend_from_slice(s.as_bytes());
    Box::leak(data.into_boxed_slice()).as_ptr()
}
