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

/// Justified kashida lines with coloured harakat set the same words, glyphs and tatweels.
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
