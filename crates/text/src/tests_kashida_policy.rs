//! Varos: the paragraph's kashida setting (Off / Auto / Force and its strength) and kashida in
//! fonts shaped with `morx` alone ([`super`]). Noto Sans Arabic (OFL) is the test fixture of
//! `crates/text-arabic/fixtures`; a `morx`-only face is generated from it.

use kurbo::{Affine, Point, Rect, Shape};
use vectorcraft_doc::text::{KashidaMode, KashidaOptions};
use vectorcraft_doc::{Burasagari, CharStyle, Justify, Mojikumi, ParaStyle, TextKind, TextObject};
use vectorcraft_geom::PathData;

use crate::{FontDb, TextLayout, layout};

const NOTO: &[u8] = include_bytes!("../../text-arabic/fixtures/NotoSansArabic.ttf");
const FAMILY: &str = "Noto Sans Arabic";
/// The Type tool fixture of `tests_kashida.rs`.
const LONG: &str = "بسم الله الرحمن الرحيم الحمد لله رب العالمين الرحمن الرحيم مالك يوم الدين إياك نعبد وإياك نستعين اهدنا الصراط المستقيم";

fn db_with(fonts: &[&[u8]]) -> FontDb {
    let db = FontDb::with_font_dirs(vec![]);
    db.set_system_fallback(false);
    for f in fonts {
        db.add_font(f.to_vec());
    }
    db
}

fn area(s: &str, st: CharStyle, width: f64, justify: Justify, kashida: KashidaOptions) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, s, st);
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, width, 4000.0).to_path(0.1)) };
    t.xf = Affine::IDENTITY;
    t.para = ParaStyle { justify, mojikumi: Mojikumi::LineEndHalf, burasagari: Burasagari::Standard, kashida, ..Default::default() };
    t
}

/// The Type tool's paragraph (default character style: the Arabic falls back to the one Arabic face).
fn type_tool(db: &FontDb, justify: Justify, kashida: KashidaOptions) -> TextLayout {
    layout(db, &area(LONG, CharStyle::default(), 120.0, justify, kashida))
}

fn tatweel(db: &FontDb, family: &str) -> (u32, u32) {
    let face = db.face(family, "Regular").unwrap();
    (face.id(), face.glyph_for('\u{0640}'))
}

/// Each line's total tatweel advance, in hundredths of a point.
fn elongation(l: &TextLayout, (face, tat): (u32, u32)) -> Vec<i64> {
    l.lines
        .iter()
        .map(|line| {
            let w: f64 = l.glyphs[line.glyph_start..line.glyph_end]
                .iter()
                .filter(|g| g.font_id == face && g.gid == tat && g.len == 0)
                .map(|g| g.advance)
                .sum();
            (w * 100.0).round() as i64
        })
        .collect()
}

fn opts(mode: KashidaMode, strength: u8) -> KashidaOptions {
    KashidaOptions { mode, strength }
}

#[test]
fn strength_pins_the_elongation_of_each_line() {
    let db = db_with(&[NOTO]);
    let tat = tatweel(&db, FAMILY);
    let per_line = |k: KashidaOptions| elongation(&type_tool(&db, Justify::JustifyAll, k), tat);
    let light = per_line(opts(KashidaMode::Auto, 0));
    let balanced = per_line(opts(KashidaMode::Auto, 50));
    let strong = per_line(opts(KashidaMode::Auto, 100));
    let forced = per_line(opts(KashidaMode::Force, 50));
    eprintln!("0: {light:?}\n50: {balanced:?}\n100: {strong:?}\nforce 50: {forced:?}");
    // Strength 50 is the policy every paragraph had before the setting: the default.
    assert_eq!(balanced, per_line(KashidaOptions::default()));
    let total = |v: &[i64]| v.iter().sum::<i64>();
    assert!(total(&light) < total(&balanced) && total(&balanced) < total(&strong), "{light:?} {balanced:?} {strong:?}");
    // Force goes deeper than Auto at the same strength.
    assert!(total(&forced) > total(&balanced));
    let forced_max = per_line(opts(KashidaMode::Force, 100));
    assert!(total(&forced_max) >= total(&strong), "{forced_max:?}");
    assert_eq!(light, PINNED_0);
    assert_eq!(balanced, PINNED_50);
    assert_eq!(strong, PINNED_100);
    // Off: none.
    assert_eq!(total(&per_line(opts(KashidaMode::Off, 100))), 0);
}

/// Tatweel advance per line (hundredths of a point; one tatweel is 3.24 pt here) of the Type tool
/// paragraph, Justify All, 120 pt wide, at strength 0, 50 (the policy before the setting) and 100.
const PINNED_0: [i64; 6] = [324, 324, 0, 324, 324, 324];
const PINNED_50: [i64; 6] = [648, 972, 648, 1296, 972, 648];
const PINNED_100: [i64; 6] = [1296, 1620, 972, 1944, 1944, 1296];

/// Justify with the last line aligned: only Force stretches the last line, with kashida alone
/// (its spaces aren't spread: it stays short of the measure).
#[test]
fn force_stretches_the_last_line_with_kashida_only() {
    let db = db_with(&[NOTO]);
    let tat = tatweel(&db, FAMILY);
    for justify in [Justify::JustifyRight, Justify::JustifyLeft, Justify::JustifyCenter] {
        let auto = type_tool(&db, justify, opts(KashidaMode::Auto, 100));
        let force = type_tool(&db, justify, opts(KashidaMode::Force, 50));
        assert_eq!(elongation(&auto, tat).last(), Some(&0), "{justify:?}: Auto leaves the last line");
        assert!(elongation(&force, tat).last().is_some_and(|w| *w > 0), "{justify:?}: Force stretches it");
        let last = force.lines.last().unwrap();
        assert!(last.x1 - last.x0 < 120.0 - 1.0, "{justify:?}: the last line isn't justified");
    }
    // Justify All already stretches it under Auto; Left/Right aligned paragraphs never.
    let right = type_tool(&db, Justify::Right, opts(KashidaMode::Force, 100));
    assert!(elongation(&right, tat).iter().all(|w| *w == 0));
}

/// Latin is set bit for bit the same whatever the kashida setting.
#[test]
fn latin_is_untouched_by_the_kashida_setting() {
    let db = db_with(&[NOTO]);
    let text = "Typography is the craft of arranging type to make written language legible, readable and appealing when displayed.";
    let set = |l: &TextLayout| {
        l.glyphs.iter().map(|g| (g.gid, g.byte, g.origin.x.to_bits(), g.origin.y.to_bits(), g.advance.to_bits())).collect::<Vec<_>>()
    };
    for justify in [Justify::JustifyAll, Justify::JustifyLeft, Justify::Left] {
        let base = set(&layout(&db, &area(text, CharStyle::default(), 160.0, justify, KashidaOptions::default())));
        for k in [opts(KashidaMode::Off, 0), opts(KashidaMode::Auto, 0), opts(KashidaMode::Auto, 100), opts(KashidaMode::Force, 100)] {
            assert_eq!(set(&layout(&db, &area(text, CharStyle::default(), 160.0, justify, k))), base, "{justify:?} {k:?}");
        }
    }
}

thread_local! {
    /// Faces [`super::morx_shaped`] treats as shaped with `morx`, for tests: no font with a working
    /// `morx` table can be generated here (write-fonts writes no AAT tables, and fonts are never
    /// committed), so the morx path is exercised on Noto (its letters join, its own tatweel) with
    /// HarfRust's safe flags ignored, and on the installed `morx` fonts when present.
    pub(super) static FORCED_MORX: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Noto Sans Arabic with an empty `morx` table added: HarfRust shapes it with `morx` (ignoring its
/// `GSUB`), so its letters no longer join.
fn empty_morx_noto() -> Vec<u8> {
    use write_fonts::types::Tag as WTag;
    let font = skrifa::FontRef::new(NOTO).unwrap();
    let name = crate::test_fonts::name_table(&[(1, "Morx Arabic"), (2, "Regular"), (4, "Morx Arabic"), (6, "MorxArabic")]).unwrap();
    let mut builder = write_fonts::FontBuilder::new();
    builder.add_raw(WTag::new(b"name"), name);
    builder.add_raw(WTag::new(b"morx"), vec![0, 2, 0, 0, 0, 0, 0, 0]);
    for r in font.table_directory.table_records() {
        let tag = WTag::new(&r.tag().to_be_bytes());
        if !builder.contains(tag) {
            builder.add_raw(tag, font.table_data(r.tag()).unwrap().as_bytes());
        }
    }
    builder.build()
}

/// A face whose `morx` shaping doesn't join its letters takes no kashida: a tatweel would hang
/// between isolated letters.
#[test]
fn a_morx_face_whose_letters_dont_join_takes_no_kashida() {
    let db = db_with(&[&empty_morx_noto(), NOTO]);
    let face = db.face("Morx Arabic", "Regular").unwrap();
    assert_eq!(face.family, "Morx Arabic");
    assert!(!super::morx_shaped(&face), "an empty morx table: no joining");
    let tat = tatweel(&db, "Morx Arabic");
    let st = CharStyle { font_family: "Morx Arabic".into(), size: 24.0, ..Default::default() };
    let l = layout(&db, &area(LONG, st, 300.0, Justify::JustifyAll, opts(KashidaMode::Force, 100)));
    assert_eq!(elongation(&l, tat).iter().sum::<i64>(), 0);
}

/// The morx path (joining rules, verified trials) on Noto, its safe flags ignored.
#[test]
fn the_morx_path_takes_verified_kashida_by_the_joining_rules() {
    let db = db_with(&[NOTO]);
    let face = db.face(FAMILY, "Regular").unwrap();
    let tat = tatweel(&db, FAMILY);
    let st = CharStyle { font_family: FAMILY.into(), size: 24.0, ..Default::default() };
    let set = |k: KashidaOptions| layout(&db, &area(LONG, st.clone(), 300.0, Justify::JustifyAll, k));
    let flags = elongation(&set(KashidaOptions::default()), tat);
    FORCED_MORX.with(|f| f.borrow_mut().push(face.id()));
    let l = set(KashidaOptions::default());
    let e = elongation(&l, tat);
    let off = set(opts(KashidaMode::Off, 50));
    FORCED_MORX.with(|f| f.borrow_mut().clear());
    eprintln!("safe flags: {flags:?}\nmorx path: {e:?}");
    assert!(e.iter().sum::<i64>() > 0, "kashida through the morx path");
    assert!(l.glyphs.iter().all(|g| g.font_id == face.id() && g.gid != 0), "its own glyphs, no .notdef");
    // The joining rules: never inside لا, never next to harakat, never after a non-joiner.
    for g in l.glyphs.iter().filter(|g| g.gid == tat.1 && g.len == 0) {
        let before = LONG[..g.byte].chars().last().unwrap();
        let after = LONG[g.byte..].chars().next().unwrap();
        assert!(vectorcraft_text_arabic::joining_allows(before, after), "a tatweel between {before} and {after}");
    }
    // Every authored glyph is the one the line had without kashida.
    let authored = |l: &TextLayout| {
        let mut v: Vec<(usize, u32)> = l.glyphs.iter().filter(|g| g.len > 0).map(|g| (g.byte, g.gid)).collect();
        v.sort_unstable();
        v
    };
    assert_eq!(authored(&l), authored(&off));
    // Justified lines fill the measure.
    for line in &l.lines[..l.lines.len() - 1] {
        assert!((line.x1 - line.x0 - 300.0).abs() < 0.01);
    }
}

/// The verification rejects a trial whose letters changed, whose count is off, or that didn't
/// grow by the tatweel.
#[test]
fn morx_trials_are_verified() {
    use crate::features::OtFeatures;
    use crate::shape::shape_range;
    let db = db_with(&[NOTO]);
    let s = "بسم";
    let st = CharStyle { font_family: FAMILY.into(), size: 24.0, ..Default::default() };
    let runs = [(0..s.len(), &st)];
    let feats = OtFeatures::default();
    let bidi = crate::layout::para_bidi(s, None).unwrap();
    let mut original = vec![];
    shape_range(&db, s, 0..s.len(), &runs, &[None], &feats, &bidi.levels, &mut original);
    let sh = super::Shaper { db: &db, text: s, runs: &runs, feats: &feats, levels: &bidi.levels, para_start: 0 };
    let slot = "ب".len();
    let trial = super::reshape_word(&sh, 0..s.len(), &[slot]).unwrap();
    assert!(super::morx_trial_ok(&original, &trial, &[slot]));
    // Wrong slot list, a changed letter, a lost glyph, a shrunken tatweel: rejected.
    assert!(!super::morx_trial_ok(&original, &trial, &[slot + 2]));
    let mut changed = trial.clone();
    changed.iter_mut().find(|g| g.len > 0).unwrap().gid += 1;
    assert!(!super::morx_trial_ok(&original, &changed, &[slot]));
    assert!(!super::morx_trial_ok(&original, &trial[1..], &[slot]));
    let mut moved = trial.clone();
    moved.iter_mut().find(|g| g.len > 0).unwrap().dy += 1.0;
    assert!(!super::morx_trial_ok(&original, &moved, &[slot]), "a moved mark or letter");
    // Two letters swapping advances keep the word's total: rejected all the same.
    let mut swapped = trial.clone();
    let letters: Vec<usize> = swapped.iter().enumerate().filter(|(_, g)| g.len > 0).map(|(i, _)| i).collect();
    let (a, b) = (letters[0], letters[letters.len() - 1]);
    let (wa, wb) = (swapped[a].adv, swapped[b].adv);
    assert!((wa - wb).abs() > 1.0, "two letters of different widths");
    swapped[a].adv = wb;
    swapped[b].adv = wa;
    assert!(!super::morx_trial_ok(&original, &swapped, &[slot]), "advances that cancel out in the total");
    let mut wide = trial.clone();
    wide.iter_mut().find(|g| g.len > 0).unwrap().adv += 5.0;
    assert!(!super::morx_trial_ok(&original, &wide, &[slot]), "a neighbour's advance changed");
    // Varos: a glyph drawn in another run (a coloured harakat that lost its colour): rejected, by
    // the morx check, the paint check of every trial and the word's own re-shape check.
    let mut repainted = trial.clone();
    repainted.iter_mut().find(|g| g.len > 0).unwrap().run += 1;
    assert!(!super::morx_trial_ok(&original, &repainted, &[slot]), "a glyph in another run");
    assert!(super::same_paint(&original, &trial));
    assert!(!super::same_paint(&original, &repainted));
    let mut other_run = original.clone();
    other_run[0].run += 1;
    assert!(super::same_setting(&original, &original));
    assert!(!super::same_setting(&original, &other_run), "the same glyphs in another run");
}

/// Varos: the morx path (joining rules, verified trials) stretches words whose harakat are
/// coloured apart, and their marks keep their colour; the lines are set as without colour.
#[test]
fn the_morx_path_keeps_the_colour_of_harakat() {
    use vectorcraft_doc::color::{Color, Paint};
    const FULLY: &str = "بِسْمِ اللَّهِ الرَّحْمَنِ الرَّحِيمِ الْحَمْدُ لِلَّهِ رَبِّ الْعَالَمِينَ الرَّحْمَنِ الرَّحِيمِ مَالِكِ يَوْمِ الدِّينِ";
    let db = db_with(&[NOTO]);
    let face = db.face(FAMILY, "Regular").unwrap();
    let tat = tatweel(&db, FAMILY);
    let red = Paint::solid(Color::rgb8(200, 30, 30));
    let st = CharStyle { font_family: FAMILY.into(), size: 24.0, ..Default::default() };
    let plain = |width| area(FULLY, st.clone(), width, Justify::JustifyAll, opts(KashidaMode::Force, 100));
    let coloured = |width| {
        let mut t = plain(width);
        for r in crate::harakat::harakat_ranges(FULLY, 0..FULLY.len()) {
            crate::edit::style_range(&mut t.runs, r.start, r.end, |st| st.fill = red.clone());
        }
        t
    };
    FORCED_MORX.with(|f| f.borrow_mut().push(face.id()));
    let mut stretched = 0;
    for width in [200.0, 260.0, 300.0, 420.0] {
        let (p, c) = (plain(width), coloured(width));
        let (a, b) = (layout(&db, &p), layout(&db, &c));
        let set = |l: &TextLayout| l.glyphs.iter().map(|g| (g.gid, g.byte, g.len, g.origin.x.to_bits(), g.origin.y.to_bits())).collect::<Vec<_>>();
        assert_eq!(set(&a), set(&b), "{width}: the same glyphs, tatweels and places");
        for g in &b.glyphs {
            let mark = g.len > 0 && g.advance.abs() < 1e-6 && !g.outline.is_empty();
            assert_eq!(c.runs[g.run].style.fill == red, mark, "{width}: glyph {} at {} (mark: {mark})", g.gid, g.byte);
        }
        let mut words: Vec<usize> = b
            .glyphs
            .iter()
            .filter(|g| g.gid == tat.1 && g.len == 0)
            .map(|g| FULLY[..g.byte].rfind(' ').map_or(0, |i| i + 1))
            .filter(|&w| FULLY[w..].split(' ').next().is_some_and(|w| w.chars().any(crate::harakat::is_harakat)))
            .collect();
        words.dedup();
        stretched += words.len();
    }
    FORCED_MORX.with(|f| f.borrow_mut().clear());
    assert!(stretched > 0, "words with coloured harakat take kashida through the morx path");
}

/// Installed `morx` fonts (when present) take kashida, every glyph their own, and the harakat
/// on the letters kashida leaves alone stay exactly where they were.
#[test]
fn installed_morx_fonts_take_kashida_and_keep_their_marks() {
    const VOCALIZED: &str = "بِسْمِ اللَّهِ الرَّحْمَنِ الرَّحِيمِ الْحَمْدُ لِلَّهِ رَبِّ الْعَالَمِينَ الرَّحْمَنِ الرَّحِيمِ مَالِكِ يَوْمِ الدِّينِ";
    let db = FontDb::with_font_dirs(crate::system_font_dirs());
    let mut report = vec![];
    for family in ["Geeza Pro", "Al Nile", "Baghdad", "KufiStandardGK", "DecoType Naskh", "Farah", "Damascus", "Diwan Kufi", "Al Bayan", "Nadeem"] {
        if !db.has_family(family) {
            continue;
        }
        let face = db.face(family, "Regular").unwrap();
        let tat = (face.id(), face.glyph_for('\u{0640}'));
        let st = CharStyle { font_family: family.into(), size: 24.0, ..Default::default() };
        let mut row = format!("{family}: morx path {}", super::morx_shaped(&face));
        for (text, name) in [(LONG, "plain"), (VOCALIZED, "vocalized")] {
            let off = layout(&db, &area(text, st.clone(), 300.0, Justify::JustifyAll, opts(KashidaMode::Off, 50)));
            let on = layout(&db, &area(text, st.clone(), 300.0, Justify::JustifyAll, opts(KashidaMode::Force, 100)));
            let total: i64 = elongation(&on, tat).iter().sum();
            let tatweels = on.glyphs.iter().filter(|g| g.gid == tat.1 && g.len == 0).count();
            row += &format!(", {name}: {tatweels} tatweels ({:.1} pt)", total as f64 / 100.0);
            assert!(on.glyphs.iter().all(|g| g.font_id == face.id() && g.gid != 0), "{family}: its own glyphs");
            // Marks keep their offsets from their pen positions in words kashida left alone, and
            // in stretched words (each trial keeps every authored glyph's offset).
            let offsets = |l: &TextLayout| {
                let mut v: Vec<(usize, u32, i64, i64)> = l
                    .glyphs
                    .iter()
                    .filter(|g| g.len > 0)
                    .map(|g| {
                        let d = g.xf.translation() - kurbo::Vec2::new(g.origin.x, l.lines[g.line].baseline);
                        (g.byte, g.gid, (d.x * 1000.0).round() as i64, (d.y * 1000.0).round() as i64)
                    })
                    .collect();
                v.sort_unstable();
                v
            };
            let (a, b) = (offsets(&off), offsets(&on));
            assert_eq!(a.len(), b.len(), "{family} {name}: the same authored glyphs");
            for (x, y) in a.iter().zip(&b) {
                assert_eq!(x, y, "{family} {name}: a glyph moved off its pen position");
            }
            if super::morx_shaped(&face) && name == "plain" {
                assert!(tatweels > 0, "{family}: kashida in a morx font");
            }
        }
        report.push(row);
    }
    eprintln!("{report:#?}");
}
