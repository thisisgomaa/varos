//! BiDi resolution (UAX #9) — Block D.
//!
//! Wraps `unicode-bidi` to give the rest of the engine a stable, small API
//! that mirrors how we already think about runs and directions. The shaper
//! consumes one [`BidiRun`] at a time; visual-order reassembly happens in
//! [`crate::shape_paragraph`].
//!
//! ## Why this exists
//! Pivot's moat is correct Arabic typography. Any input with a non-Arabic
//! token — digits, brand names in Latin, URLs, punctuation embedded in
//! Arabic — needs the full Unicode Bidirectional Algorithm before shaping.
//! Skipping this is why mixed-script Arabic in Adobe/Figma is brittle.

use unicode_bidi::{BidiInfo, Level, ParagraphInfo};

use crate::TextDirection;

/// Direction of a paragraph or run.
///
/// Distinct from [`TextDirection`] in lib.rs only in spelling — kept narrow
/// here so callers can do `match res.base_direction { Direction::Rtl => …}`
/// without importing the larger enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl From<Direction> for TextDirection {
    fn from(d: Direction) -> Self {
        match d {
            Direction::Ltr => TextDirection::LeftToRight,
            Direction::Rtl => TextDirection::RightToLeft,
        }
    }
}

impl From<Level> for Direction {
    fn from(level: Level) -> Self {
        if level.is_rtl() {
            Direction::Rtl
        } else {
            Direction::Ltr
        }
    }
}

/// One contiguous run of text with a single resolved BiDi level.
#[derive(Debug, Clone)]
pub struct BidiRun {
    /// Byte range into the original input string. `text[range]` is the run.
    pub byte_range: core::ops::Range<usize>,
    /// UBA level (even = LTR, odd = RTL). Exposed in case the caller wants
    /// fine-grained ordering; most callers only need [`direction`].
    pub level: u8,
    /// Convenience: direction inferred from `level`.
    pub direction: Direction,
}

/// Output of [`resolve_paragraph`].
#[derive(Debug, Clone)]
pub struct BidiResolution {
    /// The paragraph's base direction (from the first strong character, or
    /// LTR if none, per UBA P2/P3).
    pub base_direction: Direction,
    /// Runs in **logical order** (the order they appear in the source).
    /// Visual reordering is the shaper's concern, not the resolver's.
    pub runs: Vec<BidiRun>,
}

/// Resolve UBA on a single paragraph and return its runs.
///
/// Multi-paragraph input is not supported here; split on `\n` or `\u{2029}`
/// before calling. (Phase 0 only ever passes single lines.)
pub fn resolve_paragraph(text: &str) -> BidiResolution {
    let info = BidiInfo::new(text, None);

    // Empty input — return an empty LTR resolution rather than panicking.
    if info.paragraphs.is_empty() || text.is_empty() {
        return BidiResolution {
            base_direction: Direction::Ltr,
            runs: Vec::new(),
        };
    }

    let para: &ParagraphInfo = &info.paragraphs[0];
    let base_direction = Direction::from(para.level);

    // visual_runs() returns ranges in visual order. For the moat we want
    // logical order so the shaper can iterate the source naturally, so we
    // walk `levels[]` ourselves and group consecutive same-level runs.
    let mut runs = Vec::new();
    let mut start = para.range.start;
    let mut cur_level = info.levels[start];
    let end = para.range.end;

    // Walk char boundaries — `levels` is per-char in UAX #9 but
    // unicode-bidi indexes it per byte, with non-leading bytes copying
    // the leading byte's level. We can scan byte-by-byte safely.
    for i in (start + 1)..end {
        if info.levels[i] != cur_level {
            runs.push(BidiRun {
                byte_range: start..i,
                level: cur_level.number(),
                direction: Direction::from(cur_level),
            });
            start = i;
            cur_level = info.levels[i];
        }
    }
    runs.push(BidiRun {
        byte_range: start..end,
        level: cur_level.number(),
        direction: Direction::from(cur_level),
    });

    BidiResolution {
        base_direction,
        runs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_is_handled() {
        let r = resolve_paragraph("");
        assert!(r.runs.is_empty());
        assert_eq!(r.base_direction, Direction::Ltr);
    }

    #[test]
    fn pure_latin_is_one_ltr_run() {
        let r = resolve_paragraph("hello world");
        assert_eq!(r.base_direction, Direction::Ltr);
        assert_eq!(r.runs.len(), 1);
        assert_eq!(r.runs[0].direction, Direction::Ltr);
    }

    #[test]
    fn pure_arabic_is_one_rtl_run() {
        let r = resolve_paragraph("السلام عليكم");
        assert_eq!(r.base_direction, Direction::Rtl);
        assert_eq!(r.runs.len(), 1);
        assert_eq!(r.runs[0].direction, Direction::Rtl);
    }

    #[test]
    fn runs_cover_full_input() {
        let text = "مرحبا Hello عربي";
        let r = resolve_paragraph(text);
        // Concatenating all run ranges should equal the input length.
        let total: usize = r.runs.iter().map(|run| run.byte_range.len()).sum();
        assert_eq!(total, text.len());
        // First strong char is Arabic → base RTL.
        assert_eq!(r.base_direction, Direction::Rtl);
        // At least one LTR run for "Hello".
        assert!(r.runs.iter().any(|run| run.direction == Direction::Ltr));
        // At least one RTL run for the Arabic.
        assert!(r.runs.iter().any(|run| run.direction == Direction::Rtl));
    }
}
