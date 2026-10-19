//! Varos: Arabic facts about fonts, for the font menus' Arabic filter, samples and markers.
//!
//! * [`ArabicSupport`]: whether a face can set Arabic (its character map has alef U+0627 and beh
//!   U+0628, and shaping joins: beh between two behs takes another glyph than beh alone), and how
//!   it shapes Arabic (OpenType `GSUB` with an `arab` script, or only Apple-style `morx` tables:
//!   kashida can't be inserted safely in those, see `crate::kashida`).
//! * [`FontTraits::arabic`](crate::FontTraits), read while the installed fonts are cataloged, is
//!   a cheap first pass for families not loaded yet: the character map has alef and beh (the
//!   scan reads each face's `cmap`; past its size cap, the OS/2 Unicode ranges and the family's
//!   names stand in). Joining needs the font loaded: [`FontDb::arabic_support`] says for sure.
//! * [`guess_arabic_style`]: a best-effort grouping (Naskh, Kufi, Ruqʿah, Diwani, Thuluth, Modern
//!   Sans) from family-name keywords. Fonts carry no reliable Arabic classification: OS/2 family
//!   class and PANOSE describe the Latin design of a bilingual font (Arial's PANOSE says sans
//!   serif while its Arabic is a Naskh-like design), so they are not used. A family the guess
//!   can't place is [`ArabicStyle::Unknown`], and the menus still list it under "All Arabic".
//! * [`arabic_sample_layout`]: an Arabic sample set right to left through the real layout, in the
//!   face itself (no glyph from another font, no `.notdef`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use harfrust::{Direction, ShapeOptions, UnicodeBuffer};
use kurbo::Point;
use skrifa::MetadataProvider;
use skrifa::raw::TableProvider;
use skrifa::raw::types::Tag;

use crate::{FontDb, FontFace, TextLayout};

/// The menus' Arabic sample: "peace upon the Arabic script", with harakat (fatha, damma tanween,
/// shadda with kasra), the lam-alef ligature (سلام) and a final yeh with shadda.
pub const ARABIC_SAMPLE: &str = "سَلامٌ على الخَطِّ العَرَبيّ";
/// [`ARABIC_SAMPLE`] without harakat, for faces that have the letters but not the marks.
pub const ARABIC_SAMPLE_PLAIN: &str = "سلام على الخط العربي";

/// How a face sets Arabic. All `false` for a face without Arabic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArabicSupport {
    /// Its character map has alef (U+0627) and beh (U+0628).
    pub covers: bool,
    /// Shaping joins letters: ببب isn't three isolated behs side by side.
    pub joins: bool,
    /// Lam followed by alef takes its lam-alef form (لا: a ligature, or joined forms), not the two
    /// isolated letters. Reported, not required.
    pub lam_alef: bool,
    /// Its `GSUB` table lists the `arab` script (OpenType Arabic shaping).
    pub gsub_arabic: bool,
    /// It has a `morx` (or older `mort`) table: HarfRust then shapes horizontal text with it and
    /// ignores any `GSUB` ([`ArabicSupport::morx_shaped`]).
    pub morx: bool,
}

impl ArabicSupport {
    /// The face can set Arabic: it has the letters and they join.
    pub fn capable(self) -> bool {
        self.covers && self.joins
    }

    /// HarfRust shapes the face's Arabic with its `morx` tables (AAT shaping), as it does
    /// whenever a font has them, whatever its `GSUB` says (HarfRust's `ot_shape.rs`: morx for
    /// horizontal text when present). Damascus, which has both, is one; so are Geeza Pro, Al Nile,
    /// Baghdad and KufiStandardGK. HarfRust then marks no join safe for a tatweel.
    pub fn morx_shaped(self) -> bool {
        self.covers && self.morx
    }
}

/// Is `c` an Arabic letter or mark (the Arabic blocks, the supplement, extended-A/B and the
/// presentation forms)?
pub fn is_arabic_char(c: char) -> bool {
    matches!(c as u32, 0x0600..=0x06FF | 0x0750..=0x077F | 0x0870..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF)
}

/// Is `text` Arabic, as a paragraph's direction is found (Unicode bidi rule P2): its first letter
/// is Arabic.
pub fn is_arabic_text(text: &str) -> bool {
    text.chars().find(|c| c.is_alphabetic()).is_some_and(is_arabic_char)
}

/// Is `c` an Arabic combining mark (harakat, shadda, sukun, superscript alef...)?
pub fn is_arabic_mark(c: char) -> bool {
    matches!(c as u32, 0x0610..=0x061A | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x06E7..=0x06E8 | 0x06EA..=0x06ED)
}

/// The Arabic hint of [`crate::FontTraits`]: from the character map when the font has one (a
/// loaded face, or a scanned one whose `cmap` was read), else from the OS/2 Unicode ranges
/// (Arabic, Arabic Presentation Forms-A and -B) and from `names` (normalized family and PostScript
/// names): one written in Arabic letters, or holding an Arabic keyword.
pub(crate) fn arabic_hint(font: &skrifa::FontRef<'_>, names: &[&str]) -> bool {
    if font.table_data(Tag::new(b"cmap")).is_some() {
        let map = font.charmap();
        return map.map('\u{0627}').is_some() && map.map('\u{0628}').is_some();
    }
    let ranges = font.os2().ok().map(|o| (o.ul_unicode_range_1(), o.ul_unicode_range_2(), o.ul_unicode_range_3()));
    let by_ranges = ranges.is_some_and(|(r1, r2, r3)| r1 & (1 << 13) != 0 || r2 & (1 << 31) != 0 || r3 & (1 << 3) != 0);
    by_ranges || names.iter().any(|n| n.chars().any(is_arabic_char) || ARABIC_KEYWORDS.iter().any(|k| n.contains(k)))
}

/// Family-name words that mark an Arabic font (normalized: lowercase, no spaces).
const ARABIC_KEYWORDS: &[&str] = &["arab", "naskh", "kufi", "ruqaa", "riqa", "diwani", "thuluth", "nastaliq", "nastaleeq"];

/// The glyphs HarfRust shapes `text` into in `face`, right to left: (glyph id, cluster, x
/// advance, x offset, y offset), in visual order.
pub(crate) fn shape_rtl(face: &FontFace, text: &str) -> Vec<(u32, u32, i32, i32, i32)> {
    let Some(hb) = face.hb() else { return vec![] };
    let shaper = face.shaper.shaper(&hb).instance(face.instance.as_ref()).build();
    let mut buf = UnicodeBuffer::new();
    buf.push_str(text);
    buf.set_direction(Direction::RightToLeft);
    buf.guess_segment_properties();
    let gb = shaper.shape(buf, ShapeOptions::new());
    gb.glyph_infos().iter().zip(gb.glyph_positions()).map(|(i, p)| (i.glyph_id, i.cluster, p.x_advance, p.x_offset, p.y_offset)).collect()
}

/// How `face` sets Arabic ([`ArabicSupport`]): its character map, two short shapings and its
/// table list. Cached per face (by [`FontFace::id`]).
pub fn face_arabic_support(face: &FontFace) -> ArabicSupport {
    static CACHE: Mutex<Option<HashMap<u32, ArabicSupport>>> = Mutex::new(None);
    /// Faces kept before the cache starts over.
    const MAX: usize = 4096;
    if let Some(s) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&face.id()).copied()) {
        return s;
    }
    let s = compute_support(face);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = cache.get_or_insert_with(HashMap::new);
    if map.len() >= MAX {
        map.clear();
    }
    map.insert(face.id(), s);
    s
}

fn compute_support(face: &FontFace) -> ArabicSupport {
    let Some(font) = face.skrifa() else { return ArabicSupport::default() };
    let map = font.charmap();
    let covers = map.map('\u{0627}').is_some() && map.map('\u{0628}').is_some();
    let gsub_arabic =
        font.gsub().ok().and_then(|g| g.script_list().ok()).is_some_and(|l| l.script_records().iter().any(|r| r.script_tag() == Tag::new(b"arab")));
    let morx = font.table_data(Tag::new(b"morx")).is_some() || font.table_data(Tag::new(b"mort")).is_some();
    if !covers {
        return ArabicSupport { gsub_arabic, morx, ..Default::default() };
    }
    // Fonts draw a letter with one glyph or several (Noto Naskh: the dot apart from the body),
    // and some ligate (Mishafi): joining is told by the glyphs of the joined letters not being
    // the isolated letters' glyphs side by side.
    let gids = |text: &str| -> Option<Vec<u32>> {
        let g: Vec<u32> = shape_rtl(face, text).into_iter().map(|g| g.0).collect();
        (!g.is_empty() && !g.contains(&0)).then_some(g)
    };
    let apart = |letters: &[&str]| -> Option<Vec<u32>> {
        // Visual order, right to left: the last letter's glyphs come first.
        let mut v = vec![];
        for l in letters.iter().rev() {
            v.extend(gids(l)?);
        }
        Some(v)
    };
    let joined = |text: &str, letters: &[&str]| matches!((gids(text), apart(letters)), (Some(j), Some(a)) if j != a);
    let joins = joined("ببب", &["ب", "ب", "ب"]);
    let lam_alef = joined("لا", &["ل", "ا"]);
    ArabicSupport { covers, joins, lam_alef, gsub_arabic, morx }
}

/// A best-effort Arabic style group, from family-name keywords ([`guess_arabic_style`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ArabicStyle {
    Naskh,
    Kufi,
    /// Ruqʿah (Riqʿa).
    Ruqaa,
    Diwani,
    Thuluth,
    /// Modern sans designs (IBM Plex Sans Arabic, Noto Sans Arabic).
    ModernSans,
    /// The name says nothing the guess knows: listed under "All Arabic" only.
    #[default]
    Unknown,
}

impl ArabicStyle {
    /// The groups a menu filters by (all but `Unknown`).
    pub const ALL: [ArabicStyle; 6] =
        [ArabicStyle::Naskh, ArabicStyle::Kufi, ArabicStyle::Ruqaa, ArabicStyle::Diwani, ArabicStyle::Thuluth, ArabicStyle::ModernSans];
}

/// The style group `names` (a family's names: English and Arabic) suggest. Best effort: the
/// first group whose keyword a name holds, the more specific calligraphic styles first (Diwan
/// Thuluth is Thuluth, Diwan Kufi is Kufi), else [`ArabicStyle::Unknown`]. Never used to hide a
/// font, only to narrow a list the user asked to narrow.
pub fn guess_arabic_style(names: &[&str]) -> ArabicStyle {
    const GROUPS: &[(ArabicStyle, &[&str])] = &[
        (ArabicStyle::Ruqaa, &["ruqaa", "ruqah", "ruqa", "riqa", "rikaa", "reqaa", "رقعة", "رقعه"]),
        (ArabicStyle::Thuluth, &["thuluth", "thulth", "tholoth", "ثلث"]),
        (ArabicStyle::Diwani, &["diwani", "dewani", "ديواني"]),
        (ArabicStyle::Kufi, &["kufi", "kufic", "koufi", "كوفي"]),
        // Amiri and Scheherazade are Naskh revivals; their names don't say so.
        (ArabicStyle::Naskh, &["naskh", "nask", "نسخ", "amiri", "scheherazade"]),
        (ArabicStyle::ModernSans, &["sans", "grotesk", "سانس"]),
    ];
    let names: Vec<String> = names.iter().map(|n| n.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()).collect();
    GROUPS.iter().find(|(_, words)| names.iter().any(|n| words.iter().any(|w| n.contains(w)))).map_or(ArabicStyle::Unknown, |(style, _)| *style)
}

/// The Arabic sample `face` can show: [`ARABIC_SAMPLE`], else [`ARABIC_SAMPLE_PLAIN`] when it
/// lacks some harakat; `None` when it can't set Arabic.
pub fn arabic_sample_for(face: &FontFace) -> Option<&'static str> {
    if !face_arabic_support(face).capable() {
        return None;
    }
    [ARABIC_SAMPLE, ARABIC_SAMPLE_PLAIN].into_iter().find(|s| covers_text(face, s))
}

/// Does `face` map every character of `text` but spaces?
pub fn covers_text(face: &FontFace, text: &str) -> bool {
    !text.trim().is_empty() && text.chars().filter(|c| !c.is_whitespace()).all(|c| face.covers(c))
}

/// `text` as point type in `face` at `size` points, laid out by the real pipeline (bidi, shaping,
/// marks): `None` when any glyph would come from another font or be `.notdef` (a sample must show
/// the face itself), or the face can't set Arabic.
pub fn arabic_sample_layout(db: &FontDb, face: &FontFace, text: &str, size: f64) -> Option<TextLayout> {
    if !face_arabic_support(face).capable() || !covers_text(face, text) {
        return None;
    }
    let style = vectorcraft_doc::CharStyle { font_family: face.family.clone(), font_style: face.style.clone(), size, ..Default::default() };
    let t = vectorcraft_doc::TextObject::point(Point::ZERO, text, style);
    let layout = crate::layout(db, &t);
    let own = |g: &crate::PositionedGlyph| g.font_id == face.id() && (g.gid != 0 || text[g.byte..].starts_with(char::is_whitespace));
    (!layout.glyphs.is_empty() && layout.glyphs.iter().all(own)).then_some(layout)
}

impl FontDb {
    /// How `family` sets Arabic, loading it if need be (its Regular face, else its closest style:
    /// loading one style loads every face of an installed family), as
    /// [`FontDb::known_arabic_support`] then sees it. `None` when the family isn't available.
    pub fn arabic_support(&self, family: &str) -> Option<ArabicSupport> {
        if let Some(s) = self.known_arabic_support(family) {
            return Some(s);
        }
        let (canonical, _) = self.canonical(family, "");
        self.face(&canonical, "Regular").filter(|f| f.family.eq_ignore_ascii_case(&canonical))?;
        self.known_arabic_support(family)
    }

    /// How the loaded faces of `family` set Arabic, aggregated as "any face" ([`aggregate`]);
    /// `None` when none is loaded. Never loads a font. The support is kept per face (by its id,
    /// never reused), so it goes with the faces a rescan drops and can't outlive them.
    pub fn known_arabic_support(&self, family: &str) -> Option<ArabicSupport> {
        let faces = self.loaded_faces(family);
        (!faces.is_empty()).then(|| aggregate(faces.iter().map(|f| face_arabic_support(f))))
    }

    /// May `family` set Arabic, without loading it: the catalog's character-map hint (any face),
    /// and when the family is loaded, whether its letters really join.
    pub fn may_set_arabic(&self, family: &str) -> bool {
        self.family_traits(family).arabic && self.known_arabic_support(family).is_none_or(ArabicSupport::capable)
    }

    /// The families the font menus list that set Arabic, of `style` when given (best effort,
    /// [`guess_arabic_style`]). Loads every candidate family: for tests and background checks.
    pub fn arabic_families(&self, style: Option<ArabicStyle>) -> Vec<String> {
        self.menu_family_list()
            .iter()
            .filter(|f| self.may_set_arabic(f) && self.arabic_support(f).is_some_and(ArabicSupport::capable))
            .filter(|f| style.is_none_or(|s| self.arabic_style(f) == s))
            .cloned()
            .collect()
    }

    /// The family's Arabic name from its name table (typographic or legacy family name in another
    /// language than English, written in Arabic letters), when it has one.
    pub fn arabic_family_name(&self, family: &str) -> Option<String> {
        let native = self.family_display_name(family, false);
        (native.chars().any(is_arabic_char) && !native.eq_ignore_ascii_case(family)).then_some(native)
    }

    /// The best-effort style group of `family` ([`guess_arabic_style`] on its English and Arabic
    /// names).
    pub fn arabic_style(&self, family: &str) -> ArabicStyle {
        let native = self.family_display_name(family, false);
        guess_arabic_style(&[family, &native])
    }
}

/// A family's support from its faces' as "any face": the fields of the faces that can set Arabic
/// (or of all of them, when none can) put together. One style that can't set Arabic (an italic
/// without it) never takes it from the family, and which style was looked at first doesn't matter.
pub fn aggregate(faces: impl IntoIterator<Item = ArabicSupport>) -> ArabicSupport {
    let all: Vec<ArabicSupport> = faces.into_iter().collect();
    let capable: Vec<ArabicSupport> = all.iter().copied().filter(|s| s.capable()).collect();
    let pool = if capable.is_empty() { &all } else { &capable };
    pool.iter().fold(ArabicSupport::default(), |a, s| ArabicSupport {
        covers: a.covers || s.covers,
        joins: a.joins || s.joins,
        lam_alef: a.lam_alef || s.lam_alef,
        gsub_arabic: a.gsub_arabic || s.gsub_arabic,
        morx: a.morx || s.morx,
    })
}

/// What a shaped Arabic phrase in a face looks like, for checking a font the way a user would see
/// it ([`check_phrase`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhraseCheck {
    /// Glyphs laid out, and characters in the phrase (spaces excluded from both). Fewer glyphs
    /// than characters: ligatures (لا, the Allah ligature) or marks composed together.
    pub glyphs: usize,
    pub chars: usize,
    /// Glyphs from another face (fallback), and `.notdef` glyphs.
    pub foreign: usize,
    pub notdef: usize,
    /// Harakat characters in the phrase, and mark glyphs laid out: the glyphs of a cluster holding
    /// harakat other than its letter (one glyph may draw shadda with fatha; a font that draws a
    /// letter's dots apart adds those).
    pub harakat: usize,
    pub marks: usize,
    /// Mark glyphs the shaper moved off the pen position (a non-zero x or y offset: mark
    /// positioning at work), and those moved vertically.
    pub offset_marks: usize,
    pub raised_or_lowered: usize,
    /// Mark glyphs whose ink is neither wholly above the baseline nor hanging below it, or not
    /// over their letter.
    pub misplaced_marks: usize,
}

/// Lay `text` out in `face` (point type at 48 pt) and check it ([`PhraseCheck`]).
pub fn check_phrase(db: &FontDb, face: &Arc<FontFace>, text: &str) -> PhraseCheck {
    let size = 48.0;
    let style = vectorcraft_doc::CharStyle { font_family: face.family.clone(), font_style: face.style.clone(), size, ..Default::default() };
    let t = vectorcraft_doc::TextObject::point(Point::ZERO, text, style);
    check_layout(&crate::layout(db, &t), text, face.id(), size)
}

/// Check the layout `l` of `text` (its plain text), set in the face `face_id` at `size` points
/// ([`PhraseCheck`]).
pub fn check_layout(l: &TextLayout, text: &str, face_id: u32, size: f64) -> PhraseCheck {
    use kurbo::Shape;
    let ink: Vec<&crate::PositionedGlyph> =
        l.glyphs.iter().filter(|g| !text.get(g.byte..).is_some_and(|s| s.starts_with(char::is_whitespace))).collect();
    let mut out = PhraseCheck {
        glyphs: ink.len(),
        chars: text.chars().filter(|c| !c.is_whitespace()).count(),
        foreign: ink.iter().filter(|g| g.font_id != face_id).count(),
        notdef: ink.iter().filter(|g| g.gid == 0).count(),
        harakat: text.chars().filter(|c| is_arabic_mark(*c)).count(),
        ..Default::default()
    };
    let starts_with_mark = |g: &crate::PositionedGlyph| text.get(g.byte..).and_then(|s| s.chars().next()).is_some_and(is_arabic_mark);
    for g in &ink {
        let cluster: Vec<&&crate::PositionedGlyph> = ink.iter().filter(|o| o.byte == g.byte).collect();
        let source = text.get(g.byte..g.byte + g.len).unwrap_or_default();
        // A mark: a glyph of a cluster holding harakat other than the cluster's letter (its widest
        // glyph; some shapers give marks an advance), or a glyph whose own cluster is a mark.
        let base = if cluster.len() > 1 && source.chars().any(is_arabic_mark) {
            match cluster.iter().max_by(|a, b| a.advance.total_cmp(&b.advance)) {
                Some(b) if !std::ptr::eq(**b, *g) => Some(**b),
                _ => continue,
            }
        } else if starts_with_mark(g) {
            ink.iter().filter(|b| b.byte < g.byte && !starts_with_mark(b)).max_by_key(|b| b.byte).copied()
        } else {
            continue;
        };
        if g.outline.is_empty() {
            continue;
        }
        out.marks += 1;
        let Some(line) = l.lines.get(g.line) else { continue };
        let shift = g.xf.translation() - kurbo::Vec2::new(g.origin.x, line.baseline);
        if shift.hypot() > 1e-6 {
            out.offset_marks += 1;
        }
        if shift.y.abs() > 1e-6 {
            out.raised_or_lowered += 1;
        }
        let mb = g.outline.bounding_box();
        let above = mb.y1 < line.baseline;
        let below = mb.y0 > line.baseline - size * 0.1;
        let over = base.is_some_and(|b| {
            let bb = b.outline.bounding_box();
            (bb.x0 - size * 0.15..=bb.x1 + size * 0.15).contains(&mb.center().x)
        });
        if !(above || below) || !over {
            out.misplaced_marks += 1;
        }
    }
    out
}

#[cfg(test)]
#[path = "tests_arabic_fonts.rs"]
mod tests;
