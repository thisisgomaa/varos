//! Varos: harakat styled apart from their letters ([`super`]), and right-to-left Arabic on a path.
//! Noto Sans Arabic (OFL) is the fixture of `crates/text-arabic/fixtures`; installed fonts are
//! checked when present.

use std::sync::Arc;

use kurbo::{Affine, BezPath, Point, Rect, Shape};
use vectorcraft_doc::color::{Color, Paint};
use vectorcraft_doc::{CharStyle, Justify, TextKind, TextObject};
use vectorcraft_geom::PathData;

use super::*;
use crate::{FontDb, FontFace, TextLayout, layout};

const NOTO: &[u8] = include_bytes!("../../text-arabic/fixtures/NotoSansArabic.ttf");
const FAMILY: &str = "Noto Sans Arabic";
const VOCALIZED: &str = "بِسْمِ اللَّهِ الرَّحْمَنِ الرَّحِيمِ";

fn db_with(fonts: &[&[u8]]) -> FontDb {
    let db = FontDb::with_font_dirs(vec![]);
    db.set_system_fallback(false);
    for f in fonts {
        db.add_font(f.to_vec());
    }
    db
}

fn red() -> Paint {
    Paint::solid(Color::rgb8(200, 30, 30))
}

fn style(family: &str) -> CharStyle {
    CharStyle { font_family: family.into(), size: 48.0, ..Default::default() }
}

/// `text` in `family`, its harakat filled with `fill` (`None`: as the letters).
fn text_with_harakat(text: &str, family: &str, fill: Option<Paint>) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, text, style(family));
    if let Some(fill) = fill {
        for r in harakat_ranges(text, 0..text.len()) {
            crate::edit::style_range(&mut t.runs, r.start, r.end, |st| st.fill = fill.clone());
        }
    }
    t
}

/// One glyph's geometry, bit for bit: face, glyph id, cluster, pen position, transform, advance.
type Geometry = (u32, u32, usize, usize, [u64; 2], [u64; 6], u64);

/// Every glyph's [`Geometry`].
fn geometry(l: &TextLayout) -> Vec<Geometry> {
    l.glyphs
        .iter()
        .map(|g| {
            (g.font_id, g.gid, g.byte, g.len, [g.origin.x.to_bits(), g.origin.y.to_bits()], g.xf.as_coeffs().map(f64::to_bits), g.advance.to_bits())
        })
        .collect()
}

/// The glyphs drawn in the run filled `fill`.
fn painted(t: &TextObject, l: &TextLayout, fill: &Paint) -> usize {
    l.glyphs.iter().filter(|g| t.runs.get(g.run).is_some_and(|r| r.style.fill == *fill)).count()
}

#[test]
fn paint_only_differences_are_told_apart() {
    let a = style(FAMILY);
    let mut b = a.clone();
    b.fill = red();
    b.stroke = red();
    b.stroke_width = 2.0;
    assert!(paint_only(&a, &b) && paint_only(&a, &a));
    let mut c = a.clone();
    c.size = 12.0;
    assert!(!paint_only(&a, &c));
    let mut d = b.clone();
    d.baseline_shift = 3.0;
    assert!(!paint_only(&a, &d), "a raised harakat is not paint only");
    assert!(paint_only_mark_boundary("بِ", "ب".len(), &a, &b));
    assert!(!paint_only_mark_boundary("بب", "ب".len(), &a, &b), "between two letters");
    assert!(!paint_only_mark_boundary("e\u{301}", 1, &a, &b), "a Latin accent keeps upstream's rule");
    assert_eq!(harakat_ranges("بِسْمِ", 0.."بِسْمِ".len()), [2..4, 6..8, 10..12]);
    assert_eq!(harakat_ranges("اللَّهِ", 0.."اللَّهِ".len()), [6..10, 12..14], "shadda and fatha together");
}

/// A range is normalised as `edit::style_range` takes one: clamped, each end moved back to the
/// start of its character, put in order. Never a silent empty result for a usable range.
#[test]
fn harakat_ranges_normalise_their_range() {
    let word = "بِسْمِ"; // ب 0..2, kasra 2..4, س 4..6, sukun 6..8, م 8..10, kasra 10..12
    let all = harakat_ranges(word, 0..word.len());
    assert_eq!(harakat_ranges(word, word.len()..0), all, "reversed");
    assert_eq!(harakat_ranges(word, 0..usize::MAX), all, "past the end");
    assert!(harakat_ranges(word, usize::MAX..usize::MAX).is_empty(), "wholly past the end");
    assert_eq!(harakat_ranges(word, 1..word.len()), all, "a start inside ب moves back to it");
    assert_eq!(harakat_ranges(word, 0..7), [std::ops::Range { start: 2, end: 4 }], "an end inside the sukun moves back before it");
    assert_eq!(harakat_ranges(word, 3..9), [2..4, 6..8], "both ends inside characters");
    assert_eq!(harakat_ranges(word, std::ops::Range { start: 9, end: 3 }), [2..4, 6..8], "reversed, inside characters");
    assert!(harakat_ranges(word, 4..4).is_empty(), "empty");
}

/// The character-level pass names each glyph's own character only when it sets the same glyphs
/// the same way, and only a character in that glyph's cluster: repeated glyph ids alone don't do.
#[test]
fn own_chars_need_the_same_setting_and_each_character_in_its_cluster() {
    // بَبَب right to left: final beh (cluster 8), fatha and medial beh (cluster 4), fatha and
    // initial beh (cluster 0). The two fathas share a glyph id, and so would two behs.
    let raw = [(10, 8, 500, 0, 0), (20, 4, 0, -30, 400), (11, 4, 300, 0, 0), (20, 0, 0, -40, 420), (12, 0, 450, 0, 0)];
    let by_char = |own: [u32; 5]| raw.iter().zip(own).map(|(g, c)| (g.0, c, g.2, 0, g.3, g.4)).collect::<Vec<_>>();
    let ok = by_char([8, 6, 4, 2, 0]);
    assert_eq!(own_chars(&raw, &ok, 10), [8, 6, 4, 2, 0]);
    // The fathas' characters swapped (the same glyph ids): each is outside its glyph's cluster,
    // so both keep their cluster's (their letter's) character, never the other fatha's.
    assert_eq!(own_chars(&raw, &by_char([8, 2, 4, 6, 0]), 10), [8, 4, 4, 0, 0]);
    // A character before its glyph's cluster, or past the segment's end: the cluster's own.
    assert_eq!(own_chars(&raw, &by_char([8, 3, 4, 2, 0]), 10), [8, 4, 4, 2, 0]);
    assert_eq!(own_chars(&raw, &by_char([9, 6, 4, 2, 0]), 9), [8, 6, 4, 2, 0]);
    // The same ids set otherwise (an advance or an offset changed, another glyph, one short):
    // nothing is borrowed.
    let mut moved = ok.clone();
    moved[1].5 += 1;
    assert!(own_chars(&raw, &moved, 10).is_empty(), "a mark moved");
    let mut wider = ok.clone();
    wider[2].2 += 1;
    assert!(own_chars(&raw, &wider, 10).is_empty(), "an advance changed");
    let mut other = ok.clone();
    other[0].0 = 13;
    assert!(own_chars(&raw, &other, 10).is_empty(), "another glyph");
    assert!(own_chars(&raw, &ok[1..], 10).is_empty(), "a glyph short");
}

/// A word whose letters and marks repeat the same glyph ids (بَبَب): each fatha takes its own
/// colour, on its own letter, and every glyph is set as without colour.
#[test]
fn repeated_glyphs_take_their_own_harakat_colour() {
    const WORD: &str = "بَبَب"; // ب 0, fatha 2, ب 4, fatha 6, ب 8
    let db = db_with(&[NOTO]);
    let green = Paint::solid(Color::rgb8(30, 140, 70));
    let plain = text_with_harakat(WORD, FAMILY, None);
    let mut coloured = plain.clone();
    crate::edit::style_range(&mut coloured.runs, 2, 4, |st| st.fill = green.clone());
    crate::edit::style_range(&mut coloured.runs, 6, 8, |st| st.fill = red());
    let (a, b) = (layout(&db, &plain), layout(&db, &coloured));
    assert_eq!(geometry(&a), geometry(&b), "joining and mark positions unchanged");
    let fill = |g: &crate::PositionedGlyph| coloured.runs[g.run].style.fill.clone();
    let marks: Vec<&crate::PositionedGlyph> = b.glyphs.iter().filter(|g| g.advance.abs() < 1e-6 && !g.outline.is_empty()).collect();
    assert_eq!(marks.len(), 2, "two fathas");
    assert_eq!(marks[0].gid, marks[1].gid, "the same glyph id twice");
    for m in &marks {
        let want = if m.byte == 0 { green.clone() } else { red() };
        assert!(m.byte == 0 || m.byte == 4, "a fatha in its letter's cluster: {}", m.byte);
        assert_eq!(fill(m), want, "the fatha of the letter at {} keeps its own colour", m.byte);
    }
    let letters = plain.runs[0].style.fill.clone();
    assert!(b.glyphs.iter().filter(|g| g.advance.abs() >= 1e-6).all(|g| fill(g) == letters), "letters keep theirs");
}

/// Coloured harakat: the letters keep their joining and the marks their attachment, glyph for
/// glyph and bit for bit; every mark glyph (and only the marks) is drawn in the harakat's run.
#[test]
fn harakat_coloured_apart_keep_every_glyph_and_take_their_own_colour() {
    let db = db_with(&[NOTO]);
    let plain = text_with_harakat(VOCALIZED, FAMILY, None);
    let coloured = text_with_harakat(VOCALIZED, FAMILY, Some(red()));
    assert!(coloured.runs.len() > 1);
    let (a, b) = (layout(&db, &plain), layout(&db, &coloured));
    assert_eq!(geometry(&a), geometry(&b), "joining and mark positions unchanged");
    // The mark glyphs (no advance, some ink) are red, and only they: a shadda with its fatha or
    // kasra is one glyph in Noto Sans Arabic (15 harakat, 12 mark glyphs).
    let marks = b.glyphs.iter().filter(|g| g.advance.abs() < 1e-6 && !g.outline.is_empty()).count();
    assert_eq!(marks, 12);
    assert_eq!(painted(&coloured, &b, &red()), marks);
    for g in b.glyphs.iter().filter(|g| coloured.runs[g.run].style.fill == red()) {
        assert!(g.advance.abs() < 1e-6, "only marks are red: glyph {} at {}", g.gid, g.byte);
    }
    // Hidden: no fill, the same glyphs.
    let hidden = text_with_harakat(VOCALIZED, FAMILY, Some(Paint::None));
    let h = layout(&db, &hidden);
    assert_eq!(geometry(&a), geometry(&h));
    assert_eq!(painted(&hidden, &h, &Paint::None), marks);
    // A harakat set in another size is still shaped apart (upstream's rule for real style changes).
    let mut sized = text_with_harakat(VOCALIZED, FAMILY, None);
    let first = harakat_ranges(VOCALIZED, 0..VOCALIZED.len())[0].clone();
    crate::edit::style_range(&mut sized.runs, first.start, first.end, |st| st.size = 30.0);
    assert_ne!(geometry(&layout(&db, &sized)), geometry(&a));
}

/// Justified kashida lines with coloured harakat set the same words, glyphs and tatweels, and the
/// marks of stretched words keep their colour.
#[test]
fn coloured_harakat_keep_kashida() {
    let db = db_with(&[NOTO]);
    let justified = |t: &mut TextObject, width: f64| {
        t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, width, 2000.0).to_path(0.1)) };
        t.xf = Affine::IDENTITY;
        t.para.justify = Justify::JustifyAll;
    };
    // Harakat on the first words only: the others leave room for tatweels.
    const PARTLY: &str = "بِسْمِ اللَّهِ الرحمن الرحيم الحمد لله رب العالمين";
    let tat = db.face(FAMILY, "Regular").unwrap().glyph_for('\u{0640}');
    let mut tatweels = 0;
    for width in [200.0, 260.0, 300.0, 420.0, 600.0] {
        let mut plain = text_with_harakat(PARTLY, FAMILY, None);
        let mut coloured = text_with_harakat(PARTLY, FAMILY, Some(red()));
        justified(&mut plain, width);
        justified(&mut coloured, width);
        let (a, b) = (layout(&db, &plain), layout(&db, &coloured));
        tatweels += a.glyphs.iter().filter(|g| g.gid == tat).count();
        assert_eq!(geometry(&a), geometry(&b), "{width}");
    }
    assert!(tatweels > 0, "the text takes kashida");
    // Harakat on every word: the stretched words carry coloured marks, which keep their colour.
    let mut stretched = 0;
    for width in [200.0, 260.0, 300.0, 420.0, 600.0] {
        let mut plain = text_with_harakat(FULLY, FAMILY, None);
        let mut coloured = text_with_harakat(FULLY, FAMILY, Some(red()));
        justified(&mut plain, width);
        justified(&mut coloured, width);
        let (a, b) = (layout(&db, &plain), layout(&db, &coloured));
        assert_eq!(geometry(&a), geometry(&b), "{width}");
        stretched += stretched_words_with_harakat(FULLY, &b, tat);
        assert_marks_keep(&coloured, &b, &red(), &format!("{width}"));
    }
    assert!(stretched > 0, "words with coloured harakat take kashida");
}

/// Harakat on every word.
const FULLY: &str = "بِسْمِ اللَّهِ الرَّحْمَنِ الرَّحِيمِ الْحَمْدُ لِلَّهِ رَبِّ الْعَالَمِينَ الرَّحْمَنِ الرَّحِيمِ مَالِكِ يَوْمِ الدِّينِ";

/// The words of `text` with harakat that `l` stretched with a tatweel (glyph `tat`).
fn stretched_words_with_harakat(text: &str, l: &TextLayout, tat: u32) -> usize {
    let mut words: Vec<usize> = l
        .glyphs
        .iter()
        .filter(|g| g.gid == tat && g.len == 0)
        .map(|g| text[..g.byte].rfind(' ').map_or(0, |i| i + 1))
        .filter(|&start| text[start..].split(' ').next().is_some_and(|w| w.chars().any(is_harakat)))
        .collect();
    words.dedup();
    words.len()
}

/// Every mark glyph of `l` (no advance, some ink) is drawn in `fill`, and only the marks.
fn assert_marks_keep(t: &TextObject, l: &TextLayout, fill: &Paint, what: &str) {
    for g in &l.glyphs {
        let mark = g.len > 0 && g.advance.abs() < 1e-6 && !g.outline.is_empty();
        assert_eq!(t.runs[g.run].style.fill == *fill, mark, "{what}: glyph {} at {} (mark: {mark})", g.gid, g.byte);
    }
}

/// The fonts installed here (when present): coloured harakat change no glyph, position or
/// advance, and the marks take the colour. The table is printed (`--nocapture`).
#[test]
fn installed_fonts_keep_their_glyphs_with_coloured_harakat() {
    const FONTS: &[&str] = &[
        "Geeza Pro",
        "SF Arabic",
        "Al Nile",
        "Baghdad",
        "Damascus",
        "KufiStandardGK",
        "DecoType Naskh",
        "Diwan Thuluth",
        "Farah",
        "Noto Nastaliq Urdu",
        "Arial",
        "Times New Roman",
        "Tahoma",
        "Noto Naskh Arabic",
        "Noto Kufi Arabic",
        "Amiri",
        "Aref Ruqaa",
        "IBM Plex Sans Arabic",
    ];
    let db = FontDb::with_font_dirs(crate::system_font_dirs());
    let harakat = VOCALIZED.chars().filter(|c| is_harakat(*c)).count();
    let bare: String = VOCALIZED.chars().filter(|c| !is_harakat(*c)).collect();
    let mut rows = vec![];
    for &family in FONTS {
        if !db.has_family(family) {
            continue;
        }
        let face: Arc<FontFace> = db.face(family, "Regular").unwrap();
        let plain = text_with_harakat(VOCALIZED, &face.family, None);
        let coloured = text_with_harakat(VOCALIZED, &face.family, Some(red()));
        let (a, b) = (layout(&db, &plain), layout(&db, &coloured));
        assert_eq!(geometry(&a), geometry(&b), "{family}: coloured harakat changed the glyphs");
        let red_glyphs = painted(&coloured, &b, &red());
        assert!(red_glyphs > 0, "{family}: the harakat take their colour");
        // No letter takes the harakat's colour: no red glyph is one the text sets without harakat.
        let letters: Vec<u32> = layout(&db, &text_with_harakat(&bare, &face.family, None)).glyphs.iter().map(|g| g.gid).collect();
        let red_letters: Vec<(u32, usize)> =
            b.glyphs.iter().filter(|g| coloured.runs[g.run].style.fill == red() && letters.contains(&g.gid)).map(|g| (g.gid, g.byte)).collect();
        assert!(red_letters.is_empty(), "{family}: letters in the harakat colour {red_letters:?}");
        rows.push(format!("{family}: {} glyphs identical; {red_glyphs} glyphs in the harakat colour ({harakat} harakat)", b.glyphs.len()));
    }
    eprintln!("{rows:#?}");
}

/// A straight path from `from` to `to`.
fn line(from: (f64, f64), to: (f64, f64)) -> PathData {
    let mut p = BezPath::new();
    p.move_to(from);
    p.line_to(to);
    PathData::from_bezpath(&p)
}

fn on_path(text: &str, path: PathData) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, text, style(FAMILY));
    t.kind = TextKind::OnPath { path, start: 0.0, end: None };
    t.xf = Affine::IDENTITY;
    // As new type has it: aligned to the start of its direction.
    t.para.justify = Justify::Auto;
    t
}

/// Right-to-left Arabic on a path: the glyphs are the ones the line sets (joined, with their
/// marks), in the same visual order along the path, every authored character drawn once, the text
/// aligned to the path's end (Auto, right to left); reversed and closed paths keep them too.
#[test]
fn right_to_left_arabic_on_a_path_keeps_its_glyphs_and_order() {
    let db = db_with(&[NOTO]);
    let face = db.face(FAMILY, "Regular").unwrap();
    let flat = layout(&db, &TextObject::point(Point::ZERO, VOCALIZED, style(FAMILY)));
    // The line's glyphs in visual order (left to right).
    let mut visual: Vec<&crate::PositionedGlyph> = flat.glyphs.iter().collect();
    visual.sort_by(|a, b| a.origin.x.total_cmp(&b.origin.x).then(a.byte.cmp(&b.byte)));
    // The clusters in that order (a letter and its marks share one), and the glyphs as a set.
    let ids = |v: &[&crate::PositionedGlyph]| {
        let mut c: Vec<usize> = v.iter().map(|g| g.byte).collect();
        c.dedup();
        c
    };
    let set = |l: &TextLayout| {
        let mut v: Vec<u32> = l.glyphs.iter().map(|g| g.gid).collect();
        v.sort_unstable();
        v
    };
    let length = 900.0;
    let forward = layout(&db, &on_path(VOCALIZED, line((0.0, 100.0), (length, 100.0))));
    assert!(forward.on_path && !forward.overflow);
    assert!(forward.glyphs.iter().all(|g| g.font_id == face.id() && g.gid != 0 && g.origin.x.is_finite() && g.origin.y.is_finite()));
    let mut along: Vec<&crate::PositionedGlyph> = forward.glyphs.iter().collect();
    along.sort_by(|a, b| a.origin.x.total_cmp(&b.origin.x).then(a.byte.cmp(&b.byte)));
    assert_eq!(ids(&along), ids(&visual), "the same clusters in the same visual order along the path");
    assert_eq!(set(&forward), set(&flat), "the same glyphs: joined forms and marks");
    // Every authored character is drawn exactly once (by the clusters).
    let mut bytes: Vec<(usize, usize)> = forward.glyphs.iter().map(|g| (g.byte, g.len)).collect();
    bytes.sort_unstable();
    bytes.dedup();
    assert_eq!(bytes.iter().map(|(_, l)| l).sum::<usize>(), VOCALIZED.len());
    // Right to left: the first character is the furthest along; Auto ends the text at the path's end.
    let first = forward.glyphs.iter().filter(|g| g.byte == 0).map(|g| g.origin.x).fold(f64::MIN, f64::max);
    assert!(forward.glyphs.iter().all(|g| g.origin.x <= first + 1e-6));
    let first_advance = forward.glyphs.iter().filter(|g| g.byte == 0).map(|g| g.advance).fold(0.0, f64::max);
    assert!(first <= length + 1.0 && length - first <= first_advance + 1.0, "Auto aligns right-to-left text to the path's end: {first}");
    // Marks sit by their letters (within an em of the cluster's pen position).
    for g in &forward.glyphs {
        let base = forward.glyphs.iter().filter(|b| b.byte == g.byte).map(|b| b.origin.x).fold(f64::MAX, f64::min);
        assert!((g.xf.translation().x - base).abs() < 48.0 * 1.5, "a mark far from its letter at {}", g.byte);
    }
    // The path drawn the other way: the same glyphs, in the same order along it.
    let reversed = layout(&db, &on_path(VOCALIZED, line((length, 100.0), (0.0, 100.0))));
    let mut back: Vec<&crate::PositionedGlyph> = reversed.glyphs.iter().collect();
    back.sort_by(|a, b| (length - a.origin.x).total_cmp(&(length - b.origin.x)).then(a.byte.cmp(&b.byte)));
    assert_eq!(ids(&back), ids(&visual), "reversed path");
    assert_eq!(set(&reversed), set(&flat));
    assert!(reversed.glyphs.iter().all(|g| (g.angle.abs() - std::f64::consts::PI).abs() < 1e-3), "upside down along a reversed line");
    // A closed path: every glyph, finite.
    let circle = PathData::from_bezpath(&kurbo::Circle::new((300.0, 300.0), 200.0).to_path(0.1));
    let round = layout(&db, &on_path(VOCALIZED, circle));
    assert!(!round.overflow);
    assert_eq!(set(&round), set(&flat), "closed path: the same glyphs");
    assert!(round.glyphs.iter().all(|g| g.origin.x.is_finite() && g.origin.y.is_finite()));
}
