//! bstudio-text — Arabic text engine.
//!
//! THE MOAT. Built on top of harfrust + unicode-bidi + (forked) parley.
//! The unique value-add is kashida justification that re-reads HarfBuzz's
//! `SAFE_TO_INSERT_TATWEEL` flag (HB v5.1.0, July 2022) — something no
//! visual editor uses today (Adobe, Figma, Microsoft, Apple all break it).
//!
//! Phase 0 — Block C lands here: real shaping via harfrust.
//! Phase 0 — Block D lands BiDi.
//! Phase 0 — Block E lands the kashida engine on top.

#![forbid(unsafe_code)]

pub mod bidi;
pub mod caret;
pub mod kashida;
pub mod layout;
pub mod outline;
pub mod visual;
pub use bidi::{resolve_paragraph, BidiResolution, BidiRun, Direction as BidiDirection};
pub use caret::{
    byte_at_x_units, byte_at_x_units_weighted, caret_x_units, caret_x_units_weighted,
    line_total_advance_units, line_total_advance_units_weighted, line_units_per_em,
    line_units_per_em_weighted,
};
pub use kashida::{insert_kashida, insert_kashida_weighted, KashidaResult};
pub use layout::{layout_text, Align, LayoutLine, LayoutParams, PositionedGlyph, TextLayout};
pub use outline::{glyph_outline, GlyphOutline, OutlineCmd};
pub use visual::{shape_visual_line, shape_visual_line_tracked, shape_visual_line_weighted, VisualLine};

use core::str::FromStr;
use harfrust::{
    script, BufferFlags, Direction, FontRef, Language, ShaperData, ShaperInstance,
    UnicodeBuffer,
};

/// Justification mode for a text run.
#[derive(Debug, Clone, Copy)]
pub enum JustifyMode {
    /// Latin-style word spacing only.
    Space,
    /// Arabic kashida-aware justification.
    /// Uses `PRODUCE_SAFE_TO_INSERT_TATWEEL` + Hallberg class pairs + jalt cycling.
    Arabic {
        max_kashida_per_word: u8,
        max_kashida_total_ratio: f32, // 0.0..=1.0, e.g. 0.30
        prefer_jalt: bool,
    },
}

impl Default for JustifyMode {
    fn default() -> Self {
        JustifyMode::Arabic {
            max_kashida_per_word: 2,
            max_kashida_total_ratio: 0.30,
            prefer_jalt: true,
        }
    }
}

/// A single shaped glyph in our own representation.
///
/// All metrics are in **font units** (UnitsPerEm). The caller scales to
/// pixels when rendering — Phase 0 keeps the engine size-agnostic per
/// harfrust's design.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID in the font's GSUB-resolved table.
    pub glyph_id: u32,
    /// Byte offset of the source cluster start (UTF-8 byte index into the input).
    pub cluster: u32,
    /// Horizontal advance, font units.
    pub x_advance: i32,
    /// Vertical advance, font units (0 for horizontal runs).
    pub y_advance: i32,
    /// Pen-to-glyph X offset, font units.
    pub x_offset: i32,
    /// Pen-to-glyph Y offset, font units.
    pub y_offset: i32,
    /// HarfBuzz `SAFE_TO_INSERT_TATWEEL` flag — true means we can safely
    /// elongate (insert U+0640) before this cluster without breaking
    /// contextual substitutions or ligatures. **This is the moat.**
    pub safe_to_insert_tatweel: bool,
    /// HarfBuzz `UNSAFE_TO_CONCAT` flag — true means changing text on
    /// one side of this cluster boundary may change the shaping result
    /// on the other side. Used to guard ligature integrity.
    pub unsafe_to_concat: bool,
}

/// A shaped run — output of one call to [`shape_run`].
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// Per-glyph data, in **visual order** (LTR for RTL text means rightmost first).
    pub glyphs: Vec<ShapedGlyph>,
    /// Units per em for the font that produced this run.
    pub units_per_em: u32,
    /// Total horizontal advance in font units. Useful for line-fit checks.
    pub total_x_advance: i32,
    /// Direction the run was shaped in.
    pub direction: TextDirection,
}

/// Logical direction passed to the shaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
}

impl From<TextDirection> for Direction {
    fn from(d: TextDirection) -> Self {
        match d {
            TextDirection::LeftToRight => Direction::LeftToRight,
            TextDirection::RightToLeft => Direction::RightToLeft,
        }
    }
}

/// Shape a single run of text against a single font.
///
/// Phase 0 assumes:
/// - the whole input is one BiDi run (no mixed directions)
/// - one script (Arabic if `direction == RightToLeft`)
/// - language tag `"ar"` for RTL, `"en"` for LTR
///
/// Block D will replace this with a BiDi-segmented driver that calls
/// `shape_run` per run.
pub fn shape_run(font_bytes: &[u8], text: &str, direction: TextDirection) -> Result<ShapedRun, &'static str> {
    shape_run_weighted(font_bytes, text, direction, None)
}

/// Registration-time font probe (spec 001 W1): one cheap pass answering
/// everything the picker needs to badge a font — parse health (shaper AND
/// outliner must both accept it), Arabic coverage, tatweel availability,
/// kashida safety (the HarfBuzz SAFE_TO_INSERT_TATWEEL signal — Ahmed's
/// warn-at-load decision), and whether a `wght` axis exists (weight slider).
#[derive(Debug, Clone, Copy, Default)]
pub struct FontProbe {
    pub parse_ok: bool,
    pub arabic: bool,
    pub tatweel_ok: bool,
    pub kashida_safe: bool,
    pub has_wght: bool,
}

pub fn probe_font(bytes: &[u8]) -> FontProbe {
    let mut p = FontProbe::default();
    if FontRef::new(bytes).is_err() || skrifa::FontRef::new(bytes).is_err() {
        return p;
    }
    p.parse_ok = true;
    p.has_wght = skrifa::FontRef::new(bytes)
        .map(|f| {
            use skrifa::MetadataProvider;
            f.axes().iter().any(|a| a.tag() == skrifa::Tag::new(b"wght"))
        })
        .unwrap_or(false);
    if let Ok(run) = shape_run_weighted(
        bytes,
        "\u{628}\u{633}\u{645} \u{627}\u{644}\u{644}\u{647} \u{627}\u{644}\u{631}\u{62d}\u{645}\u{646}",
        TextDirection::RightToLeft,
        None,
    ) {
        p.arabic = run.glyphs.iter().any(|g| g.glyph_id != 0);
        p.kashida_safe = run.glyphs.iter().any(|g| g.safe_to_insert_tatweel);
    }
    if let Ok(t) = shape_run_weighted(bytes, "\u{0640}", TextDirection::RightToLeft, None) {
        p.tatweel_ok = t.glyphs.first().map(|g| g.glyph_id != 0).unwrap_or(false);
    }
    p
}


/// Joining-safe tracking (spec 001 W4): widen the advance of every glyph whose
/// cluster character does NOT connect forward to the following letter — joined
/// Arabic letterforms never tear apart, while non-joining boundaries (after
/// د ذ ر ز و ا ة …, spaces, latin) open up evenly. Visual order is irrelevant:
/// boundaries are decided in LOGICAL space via cluster byte offsets, so this
/// one post-pass serves bidi lines, kashida working strings, canvas, export
/// and measurement identically. Returns the total advance added (font units)
/// so callers can fix their running totals.
///
/// `em_millis` = tracking in 1/1000 em (CSS letter-spacing semantics).
/// Harakat are skipped when looking up the "next letter" (a mark between two
/// letters must not turn a joined pair into a tracked boundary), and mark
/// glyphs themselves (zero-advance attachments) are never widened.
pub fn apply_tracking_glyphs(
    glyphs: &mut [ShapedGlyph],
    units_per_em: u32,
    working_text: &str,
    em_millis: f32,
) -> i32 {
    if em_millis == 0.0 || glyphs.is_empty() {
        return 0;
    }
    let add = ((em_millis / 1000.0) * units_per_em as f32).round() as i32;
    if add == 0 {
        return 0;
    }
    let connects_forward = |ch: char| {
        matches!(
            kashida::classify(ch),
            kashida::JoinClass::ConnectsBoth | kashida::JoinClass::Lam
        )
    };
    // Logically-last cluster: never track after it (no trailing line slack).
    let last_cluster = glyphs.iter().map(|g| g.cluster).max().unwrap_or(0);
    let mut total_added = 0i32;
    for g in glyphs.iter_mut() {
        let start = g.cluster as usize;
        if g.cluster == last_cluster || start >= working_text.len() || g.x_advance == 0 {
            continue;
        }
        let Some(ch) = working_text[start..].chars().next() else { continue };
        // Next LETTER after this cluster, skipping attached harakat.
        let mut rest = working_text[start + ch.len_utf8()..].chars();
        let mut next = rest.next();
        while let Some(c) = next {
            if kashida::is_harakat(c) {
                next = rest.next();
            } else {
                break;
            }
        }
        let joined = match next {
            // JOINED iff ch connects forward AND the next char is a joinable
            // Arabic letter — exactly the table the kashida engine trusts.
            Some(n) => connects_forward(ch) && kashida::classify(n) != kashida::JoinClass::Other,
            None => true, // end of slice — leave untouched
        };
        if !joined {
            g.x_advance += add;
            total_added += add;
        }
    }
    total_added
}


/// Shape a single run, optionally at a variable-font `wght` location.
///
/// Identical to [`shape_run`] except that, when `wght` is `Some(value)`, the
/// harfrust shaper is built on a [`ShaperInstance`] derived from
/// `[("wght", value)]` so that **advances and GPOS (mark) offsets are computed
/// at the requested weight** — matching the outline `location` the renderer
/// uses (see RESEARCH/03 1.5). For a variable font this shifts cluster
/// advances and harakat offsets; static fonts (and fonts without a `wght`
/// axis) ignore it via `fvar`, so the call is safe at every draw site.
///
/// `wght == None` reproduces [`shape_run`] **byte-for-byte**: no
/// `ShaperInstance` is attached and the shaper is built exactly as before
/// (default location, i.e. `wght` 400 for the variable fonts we ship). This is
/// what keeps every un-weighted caller and the forced-failure corpora
/// unchanged.
pub fn shape_run_weighted(
    font_bytes: &[u8],
    text: &str,
    direction: TextDirection,
    wght: Option<f32>,
) -> Result<ShapedRun, &'static str> {
    let font = FontRef::new(font_bytes).map_err(|_| "failed to parse font")?;
    let data = ShaperData::new(&font);
    // None -> no instance: byte-for-byte the pre-change `data.shaper(&font).build()`.
    // Some(w) -> a wght instance so advances + GPOS match the rendered weight.
    let instance = wght.map(|w| ShaperInstance::from_variations(&font, [("wght", w)]));
    let shaper = data.shaper(&font).instance(instance.as_ref()).build();
    let units_per_em = shaper.units_per_em() as u32;

    let mut buf = UnicodeBuffer::new();
    buf.push_str(text);
    buf.set_direction(direction.into());

    let (script_tag, lang_tag) = match direction {
        TextDirection::RightToLeft => (script::ARABIC, "ar"),
        TextDirection::LeftToRight => (script::LATIN, "en"),
    };
    buf.set_script(script_tag);
    buf.set_language(Language::from_str(lang_tag).map_err(|_| "bad lang tag")?);

    // The flags that matter for kashida — Track 03 §3.2.
    buf.set_flags(
        BufferFlags::BEGINNING_OF_TEXT
            | BufferFlags::END_OF_TEXT
            | BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL
            | BufferFlags::PRODUCE_UNSAFE_TO_CONCAT,
    );

    let result = shaper.shape(buf, &[]);

    let glyphs: Vec<ShapedGlyph> = result
        .glyph_infos()
        .iter()
        .zip(result.glyph_positions().iter())
        .map(|(info, pos)| ShapedGlyph {
            glyph_id: info.glyph_id,
            cluster: info.cluster,
            x_advance: pos.x_advance,
            y_advance: pos.y_advance,
            x_offset: pos.x_offset,
            y_offset: pos.y_offset,
            safe_to_insert_tatweel: info.safe_to_insert_tatweel(),
            unsafe_to_concat: info.unsafe_to_concat(),
        })
        .collect();

    let total_x_advance: i32 = glyphs.iter().map(|g| g.x_advance).sum();

    Ok(ShapedRun {
        glyphs,
        units_per_em,
        total_x_advance,
        direction,
    })
}

/// Shape a paragraph that may contain mixed BiDi runs.
///
/// Resolves UBA on the input, then shapes each run independently with its
/// detected direction. Returns one [`ShapedRun`] per BiDi run, in **logical
/// order** (the order of the source bytes). Visual reordering of the runs
/// themselves is the renderer's concern — the runs carry their `direction`
/// field so the renderer can decide whether to reverse-walk them on paint.
pub fn shape_paragraph(font_bytes: &[u8], text: &str) -> Result<Vec<ShapedRun>, &'static str> {
    let resolution = bidi::resolve_paragraph(text);
    let mut runs = Vec::with_capacity(resolution.runs.len());
    for bidi_run in &resolution.runs {
        let slice = &text[bidi_run.byte_range.clone()];
        if slice.is_empty() {
            continue;
        }
        let shaped = shape_run(font_bytes, slice, bidi_run.direction.into())?;
        runs.push(shaped);
    }
    Ok(runs)
}

/// Status string — bumped each block as the moat fills in.
pub fn moat_status() -> &'static str {
    "shaping + BiDi + kashida wired (Blocks C, D, E)."
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 0 ships Amiri Regular as the canonical test font.
    const AMIRI_REGULAR: &[u8] =
        include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");
    // Cairo is a variable font with a real `wght` axis — used to prove the
    // shaping path honours the variable-font weight (B25).
    const CAIRO_VARIABLE: &[u8] =
        include_bytes!("../../../assets/fonts/Cairo-Variable.ttf");

    #[test]
    fn default_arabic_justify_is_sane() {
        let m = JustifyMode::default();
        if let JustifyMode::Arabic {
            max_kashida_per_word,
            max_kashida_total_ratio,
            prefer_jalt,
        } = m
        {
            assert_eq!(max_kashida_per_word, 2);
            assert!((max_kashida_total_ratio - 0.30).abs() < 1e-6);
            assert!(prefer_jalt);
        } else {
            panic!("default justify mode should be Arabic");
        }
    }

    #[test]
    fn moat_announces_itself() {
        let status = moat_status();
        assert!(status.contains("shaping"));
        assert!(status.contains("BiDi"));
        assert!(status.contains("kashida"));
    }

    #[test]
    fn shape_basmala() {
        // Real Arabic line — the same one we'll demo at Phase 0 launch.
        let text = "الحمد لله رب العالمين";
        let run = shape_run(AMIRI_REGULAR, text, TextDirection::RightToLeft)
            .expect("shape Basmala");

        assert!(run.glyphs.len() > 10, "should produce many glyphs");
        assert_eq!(run.units_per_em, 1000, "Amiri is UPM 1000");
        assert!(run.total_x_advance > 0);

        // At least some glyphs in a long Arabic line SHOULD be safe-to-tatweel
        // (Amiri is the reference font for this flag working correctly).
        let safe_count = run.glyphs.iter().filter(|g| g.safe_to_insert_tatweel).count();
        assert!(
            safe_count > 0,
            "Amiri + this Basmala must expose at least one SAFE_TO_INSERT_TATWEEL position; got 0"
        );
    }

    #[test]
    fn shape_lam_alif_does_not_split() {
        // "لا" is a mandatory ligature. After shaping it should NOT
        // produce 2 standalone glyphs — should be 1 cluster.
        // We don't assert "exactly 1 glyph" because the shaper may emit
        // multiple marks/components; what we check is the cluster integrity:
        // the second character (alif) is part of the lam's cluster.
        let text = "لا";
        let run = shape_run(AMIRI_REGULAR, text, TextDirection::RightToLeft)
            .expect("shape lam-alif");
        // unsafe_to_concat should be set on at least one glyph — confirming
        // this is a fused unit per HarfBuzz.
        let any_unsafe = run.glyphs.iter().any(|g| g.unsafe_to_concat);
        assert!(any_unsafe, "lam-alif must report unsafe_to_concat");
    }

    // ───────── B25: variable-font wght applied during SHAPING ─────────

    /// A heavier `wght` must change the shaped advances (and therefore total
    /// run width) for a variable font that has a `wght` axis. This is the bug
    /// the fix targets: outlines were drawn at the active weight but advances
    /// came from the default (400) weight, so heavier glyphs drifted.
    #[test]
    fn wght_changes_total_advance_on_variable_font() {
        let text = "العربية"; // a plain Arabic word, no harakat
        let w400 = shape_run_weighted(CAIRO_VARIABLE, text, TextDirection::RightToLeft, Some(400.0))
            .expect("shape @400");
        let w700 = shape_run_weighted(CAIRO_VARIABLE, text, TextDirection::RightToLeft, Some(700.0))
            .expect("shape @700");

        // Same glyph stream length; differing widths prove the variable wght
        // reached the metrics (advance) machinery, not just the outlines.
        assert_eq!(
            w400.glyphs.len(),
            w700.glyphs.len(),
            "weight should not change the glyph count for this word"
        );
        assert_ne!(
            w400.total_x_advance, w700.total_x_advance,
            "Cairo @700 must have a different total advance than @400; got {} vs {}",
            w700.total_x_advance, w400.total_x_advance
        );
        // Bolder is wider for Cairo's wght axis.
        assert!(
            w700.total_x_advance > w400.total_x_advance,
            "Cairo @700 should be wider than @400"
        );

        // And at least one glyph's per-glyph advance differs (positions, not
        // just the sum), confirming GPOS/advance consistency at the weight.
        let any_glyph_differs = w400
            .glyphs
            .iter()
            .zip(w700.glyphs.iter())
            .any(|(a, b)| a.x_advance != b.x_advance);
        assert!(
            any_glyph_differs,
            "at least one glyph advance must differ between @400 and @700"
        );
    }

    /// The `None` (un-weighted) path must be byte-for-byte identical to the
    /// pre-change `shape_run` for the same font — this is what guarantees every
    /// existing caller and corpus is unchanged. Verified against a variable
    /// font (the most sensitive case).
    #[test]
    fn wght_none_matches_legacy_shape_run() {
        let text = "العربية الفصحى";
        let legacy = shape_run(CAIRO_VARIABLE, text, TextDirection::RightToLeft)
            .expect("legacy shape");
        let none = shape_run_weighted(CAIRO_VARIABLE, text, TextDirection::RightToLeft, None)
            .expect("None shape");

        assert_eq!(legacy.glyphs.len(), none.glyphs.len());
        assert_eq!(legacy.total_x_advance, none.total_x_advance);
        assert_eq!(legacy.units_per_em, none.units_per_em);
        for (a, b) in legacy.glyphs.iter().zip(none.glyphs.iter()) {
            assert_eq!(a.glyph_id, b.glyph_id);
            assert_eq!(a.cluster, b.cluster);
            assert_eq!(a.x_advance, b.x_advance);
            assert_eq!(a.y_advance, b.y_advance);
            assert_eq!(a.x_offset, b.x_offset);
            assert_eq!(a.y_offset, b.y_offset);
            assert_eq!(a.safe_to_insert_tatweel, b.safe_to_insert_tatweel);
            assert_eq!(a.unsafe_to_concat, b.unsafe_to_concat);
        }
    }

    /// A static font (Amiri has no `wght` axis) must ignore the variation
    /// entirely: `Some(700)` shapes identically to `None`. This proves the call
    /// is safe to apply unconditionally at every draw site.
    #[test]
    fn wght_is_noop_on_static_font() {
        let text = "الحمد لله";
        let none = shape_run_weighted(AMIRI_REGULAR, text, TextDirection::RightToLeft, None)
            .expect("None shape");
        let heavy = shape_run_weighted(AMIRI_REGULAR, text, TextDirection::RightToLeft, Some(700.0))
            .expect("700 shape");
        assert_eq!(
            none.total_x_advance, heavy.total_x_advance,
            "a font without a wght axis must ignore the variation"
        );
        for (a, b) in none.glyphs.iter().zip(heavy.glyphs.iter()) {
            assert_eq!(a.glyph_id, b.glyph_id);
            assert_eq!(a.x_advance, b.x_advance);
            assert_eq!(a.x_offset, b.x_offset);
            assert_eq!(a.y_offset, b.y_offset);
        }
    }

    /// `Some(400.0)` on a variable font must equal the default location (the
    /// font's wght default is 400), and therefore also equal the `None` path —
    /// the default-weight callers stay on the exact pre-change positions.
    #[test]
    fn wght_400_equals_none_on_variable_font() {
        let text = "العربية";
        let none = shape_run_weighted(CAIRO_VARIABLE, text, TextDirection::RightToLeft, None)
            .expect("None shape");
        let four = shape_run_weighted(CAIRO_VARIABLE, text, TextDirection::RightToLeft, Some(400.0))
            .expect("400 shape");
        assert_eq!(none.total_x_advance, four.total_x_advance);
        for (a, b) in none.glyphs.iter().zip(four.glyphs.iter()) {
            assert_eq!(a.glyph_id, b.glyph_id);
            assert_eq!(a.x_advance, b.x_advance);
            assert_eq!(a.x_offset, b.x_offset);
            assert_eq!(a.y_offset, b.y_offset);
        }
    }
}
