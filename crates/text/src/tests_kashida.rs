//! Varos: kashida (tatweel) justification of right-to-left Arabic lines ([`crate::kashida`]),
//! and Arabic words that overflow rather than being cut. Noto Sans Arabic (OFL) is a test
//! fixture of `crates/text-arabic/fixtures`; collections are generated from it.

use super::*;
use crate::features::OtFeatures;
use crate::kashida::{Shaper, reshape_word};
use crate::shape::{SGlyph, shape_range};
use kurbo::Shape;
use vectorcraft_doc::{CharStyle, Justify, TextKind, TextObject};
use vectorcraft_geom::PathData;

const NOTO: &[u8] = include_bytes!("../../text-arabic/fixtures/NotoSansArabic.ttf");
const FAMILY: &str = "Noto Sans Arabic";
const TEXT: &str = "بسم الله الرحمن الرحيم الحمد لله رب العالمين";

fn db_with(bytes: Vec<u8>) -> FontDb {
    let db = FontDb::with_font_dirs(vec![]);
    db.set_system_fallback(false);
    db.add_font(bytes);
    db
}

fn style(family: &str, size: f64) -> CharStyle {
    CharStyle { font_family: family.into(), size, ..Default::default() }
}

fn area(s: &str, st: CharStyle, width: f64, justify: Justify) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, s, st);
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, width, 2000.0).to_path(0.1)) };
    t.xf = Affine::IDENTITY;
    t.para.justify = justify;
    t
}

/// The face's tatweel glyph.
fn tatweel(db: &FontDb) -> (u32, u32) {
    let face = db.face(FAMILY, "Regular").expect("Noto Sans Arabic");
    (face.id(), face.glyph_for('\u{0640}'))
}

/// A font collection (TTC) of `fonts`, in that order: each font's tables copied whole, their
/// offsets moved to where the font lies in the collection.
fn collection(fonts: &[&[u8]]) -> Vec<u8> {
    let align = |n: usize| (n + 3) & !3;
    let mut offsets = vec![];
    let mut at = align(12 + 4 * fonts.len());
    for f in fonts {
        offsets.push(at);
        at += align(f.len());
    }
    let mut out = b"ttcf".to_vec();
    out.extend(0x0001_0000u32.to_be_bytes());
    out.extend((fonts.len() as u32).to_be_bytes());
    for o in &offsets {
        out.extend((*o as u32).to_be_bytes());
    }
    for (f, &o) in fonts.iter().zip(&offsets) {
        out.resize(o, 0);
        let mut font = f.to_vec();
        let tables = usize::from(u16::from_be_bytes([font[4], font[5]]));
        for t in 0..tables {
            let p = 12 + 16 * t + 8;
            let v = u32::from_be_bytes([font[p], font[p + 1], font[p + 2], font[p + 3]]) + o as u32;
            font[p..p + 4].copy_from_slice(&v.to_be_bytes());
        }
        out.extend(font);
    }
    out
}

#[test]
fn justified_arabic_lines_take_kashida_in_the_paragraph_face_only() {
    let db = db_with(NOTO.to_vec());
    let (face, tat) = tatweel(&db);
    assert_ne!(tat, 0);
    let families = db.families();
    let mut tatweels = 0;
    for width in [200.0, 260.0, 300.0, 420.0, 600.0] {
        let l = layout(&db, &area(TEXT, style(FAMILY, 48.0), width, Justify::JustifyAll));
        assert!(l.glyphs.iter().all(|g| g.font_id == face && g.gid != 0), "{width}: every glyph in the paragraph's face");
        for line in &l.lines {
            let glyphs = &l.glyphs[line.glyph_start..line.glyph_end];
            for g in glyphs.iter().filter(|g| g.gid == tat) {
                tatweels += 1;
                // Zero length, at a boundary between two authored letters of the line.
                assert_eq!(g.len, 0);
                assert!(g.byte > line.start && g.byte < line.end && TEXT.is_char_boundary(g.byte));
                assert!(!g.outline.is_empty());
            }
            let src = &TEXT[line.start..line.end];
            if src.trim().contains(' ') {
                assert!((line.x1 - line.x0 - width).abs() < 0.01, "{width}: {src:?} {}", line.x1 - line.x0);
            }
        }
        // The authored text is untouched: the glyphs' bytes cover it exactly once.
        let covered: usize = l.glyphs.iter().filter(|g| g.gid != tat).map(|g| g.len).sum();
        assert!(covered <= TEXT.len());
    }
    assert!(tatweels > 0, "kashida was inserted");
    assert_eq!(db.families(), families, "layout registers no font");
}

#[test]
fn a_collection_face_takes_kashida_in_its_own_face() {
    let serif = include_bytes!("../../../assets/fonts/SourceSerif4-Regular.ttf");
    let db = db_with(collection(&[serif, NOTO]));
    let face = db.face(FAMILY, "Regular").expect("Noto Sans Arabic from the collection");
    assert_eq!(face.face_index(), 1);
    assert!(face.file_data().starts_with(b"ttcf"));
    let tat = face.glyph_for('\u{0640}');
    let l = layout(&db, &area(TEXT, style(FAMILY, 48.0), 600.0, Justify::JustifyAll));
    assert!(l.glyphs.iter().all(|g| g.font_id == face.id()), "no other face stands in for a collection face");
    assert!(l.glyphs.iter().any(|g| g.gid == tat), "kashida in the collection face");
}

#[test]
fn latin_and_digits_on_an_arabic_line_keep_their_glyphs() {
    let s = "بسم الله abc 123 الرحمن الرحيم";
    let db = db_with(NOTO.to_vec());
    let (_, tat) = tatweel(&db);
    let natural = layout(&db, &area(s, style(FAMILY, 48.0), 5000.0, Justify::Right));
    assert_eq!(natural.lines.len(), 1);
    let width = natural.lines[0].x1 - natural.lines[0].x0 + 80.0;
    let l = layout(&db, &area(s, style(FAMILY, 48.0), width, Justify::JustifyAll));
    assert_eq!(l.lines.len(), 1);
    assert!(l.glyphs.iter().any(|g| g.gid == tat), "the Arabic words took kashida");
    let latin = |l: &TextLayout| -> Vec<(u32, u32, usize, u64)> {
        l.glyphs
            .iter()
            .filter(|g| s[g.byte..].starts_with(|c: char| c.is_ascii_alphanumeric()))
            .map(|g| (g.font_id, g.gid, g.byte, g.advance.to_bits()))
            .collect()
    };
    assert_eq!(latin(&l), latin(&natural));
    assert_eq!(latin(&l).len(), 6);
    assert!((l.lines[0].x1 - l.lines[0].x0 - width).abs() < 0.01);
}

#[test]
fn carets_and_hits_skip_the_zero_length_tatweels() {
    let db = db_with(NOTO.to_vec());
    let (_, tat) = tatweel(&db);
    let l = layout(&db, &area(TEXT, style(FAMILY, 48.0), 600.0, Justify::JustifyAll));
    assert!(l.glyphs.iter().any(|g| g.gid == tat));
    for (li, line) in l.lines.iter().enumerate() {
        let glyphs = &l.glyphs[line.glyph_start..line.glyph_end];
        let mut stops: Vec<usize> = glyphs.iter().filter(|g| g.len > 0 && g.byte > line.start).map(|g| g.byte).collect();
        stops.sort_unstable();
        stops.dedup();
        let mut previous = f64::INFINITY;
        for b in stops {
            let (top, bottom) = caret_position(&l, b);
            let mid = top.midpoint(bottom);
            // Right to left: each later caret stop lies further left, tatweels included.
            assert!(mid.x < previous, "line {li}: caret at {b} at {} not left of {previous}", mid.x);
            previous = mid.x;
            assert_eq!(hit_byte(&l, mid), b, "line {li}: a click on the caret at {b}");
        }
    }
    assert!(!selection_quads(&l, 0, TEXT.len()).is_empty());
}

#[test]
fn reshaping_a_word_apart_reproduces_it_and_maps_tatweels_to_zero_length_boundaries() {
    let s = "بسم الله";
    let db = db_with(NOTO.to_vec());
    let st = style(FAMILY, 48.0);
    let runs = [(0..s.len(), &st)];
    let feats = OtFeatures::default();
    let bidi = layout::para_bidi(s, None).expect("bidi");
    let mut paragraph = vec![];
    shape_range(&db, s, 0..s.len(), &runs, &[None], &feats, &bidi.levels, &mut paragraph);
    let sh = Shaper { db: &db, text: s, runs: &runs, feats: &feats, levels: &bidi.levels, para_start: 0 };
    let word = 0.."بسم".len();
    let again = reshape_word(&sh, word.clone(), &[]).expect("word");
    let native: Vec<_> = paragraph.iter().filter(|g| g.byte < word.end).collect();
    assert_eq!(again.len(), native.len());
    for (a, b) in again.iter().zip(&native) {
        assert_eq!((a.gid, a.byte, a.len, a.adv.to_bits()), (b.gid, b.byte, b.len, b.adv.to_bits()));
    }
    assert!(native.iter().any(|g| g.tatweel), "HarfRust marks where a tatweel may go");
    let slot = "ب".len();
    let stretched = reshape_word(&sh, word.clone(), &[slot]).expect("word with a tatweel");
    let virtual_glyphs: Vec<_> = stretched.iter().filter(|g| g.ch == '\u{0640}').collect();
    assert_eq!(virtual_glyphs.len(), 1);
    assert_eq!((virtual_glyphs[0].byte, virtual_glyphs[0].len), (slot, 0));
    let authored: usize = stretched.iter().map(|g| g.len).sum();
    assert_eq!(authored, word.len());
    let width = |v: &[SGlyph]| v.iter().map(|g| g.adv).sum::<f64>();
    assert!(width(&stretched) > width(&again));
    assert!(stretched.windows(2).all(|w| w[0].byte <= w[1].byte), "logical order");
}

#[test]
fn unjustified_and_point_arabic_take_no_kashida() {
    let db = db_with(NOTO.to_vec());
    let (_, tat) = tatweel(&db);
    for justify in [Justify::Right, Justify::Left, Justify::Center] {
        let l = layout(&db, &area(TEXT, style(FAMILY, 48.0), 300.0, justify));
        assert!(l.glyphs.iter().all(|g| g.gid != tat), "{justify:?}");
    }
    let mut point = TextObject::point(Point::ZERO, TEXT, style(FAMILY, 48.0));
    point.para.justify = Justify::JustifyAll;
    assert!(layout(&db, &point).glyphs.iter().all(|g| g.gid != tat));
}

// Step 4 (Varos): Arabic words are never cut, nor letter-spaced.

#[test]
fn arabic_word_wider_than_the_box_overflows_whole_instead_of_being_cut() {
    let s = "الحمد لله رب العالمين";
    let db = db_with(NOTO.to_vec());
    // 100 pt is narrower than الحمد (about 108) and العالمين (about 156) at 48 pt.
    for justify in [Justify::Right, Justify::JustifyRight, Justify::JustifyAll] {
        let l = layout(&db, &area(s, style(FAMILY, 48.0), 100.0, justify));
        for line in &l.lines {
            assert!(line.start == 0 || s[..line.start].ends_with(' '), "{justify:?}: a line starts inside a word at {}", line.start);
            assert!(line.end == s.len() || s[..line.end].ends_with(' '), "{justify:?}: a line ends inside a word at {}", line.end);
        }
        let words: Vec<&str> = l.lines.iter().map(|line| s[line.start..line.end].trim()).collect();
        assert!(words.contains(&"الحمد") && words.contains(&"العالمين"), "{justify:?}: {words:?}");
        // The overflowing word keeps the line's right (start) edge; it overflows to the left.
        let wide = l.lines.iter().find(|line| s[line.start..line.end].trim() == "العالمين").expect("line");
        let right = l.glyphs[wide.glyph_start..wide.glyph_end].iter().map(|g| g.origin.x + g.advance).fold(f64::MIN, f64::max);
        assert!((right - 100.0).abs() < 0.01, "{justify:?}: right edge {right}");
    }
    // Latin keeps VectorCraft's rule: a word too wide for the line is broken.
    let latin = "Typographically";
    let l = layout(&db, &area(latin, style("Source Sans 3", 48.0), 100.0, Justify::Left));
    assert!(l.lines.len() > 1);
}

#[test]
fn arabic_single_word_lines_are_never_letter_spaced() {
    let s = "العالمين";
    let db = db_with(NOTO.to_vec());
    let (_, tat) = tatweel(&db);
    let natural = layout(&db, &area(s, style(FAMILY, 48.0), 5000.0, Justify::Right));
    let l = layout(&db, &area(s, style(FAMILY, 48.0), 400.0, Justify::JustifyAll));
    // Every authored glyph keeps its own advance (no gaps opened between joined letters); the
    // word is set at the line's right (start) edge.
    let advances = |l: &TextLayout| l.glyphs.iter().filter(|g| g.gid != tat).map(|g| (g.gid, g.advance.to_bits())).collect::<Vec<_>>();
    assert_eq!(advances(&l), advances(&natural));
    let right = l.glyphs.iter().map(|g| g.origin.x + g.advance).fold(f64::MIN, f64::max);
    assert!((right - 400.0).abs() < 0.01, "right edge {right}");
}
