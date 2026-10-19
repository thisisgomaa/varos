//! Varos: harakat (Arabic vowel and other combining marks) styled apart from their letters.
//!
//! A designer colours the harakat apart from the letters (or hides them with no fill) without
//! breaking the letters' joining or the marks' attachment: runs that differ only in paint
//! ([`paint_only`]) are not a shaping boundary at a harakat ([`paint_only_mark_boundary`]), so the
//! letter and its marks are still shaped together, glyph for glyph as before. Each glyph then
//! takes the run of its own character: the same text is shaped once more at HarfRust's character
//! cluster level, which gives every mark glyph its own character (a ligature or a composed mark
//! keeps its first character). Renderers and exporters read the glyph's run as they always did,
//! so they need no change. The text itself is never changed.

use vectorcraft_doc::CharStyle;

/// Is `c` an Arabic combining mark: harakat (fatha, damma, kasra, tanwin, shadda, sukun), the
/// superscript alef, Quranic annotation marks, and the extended-A marks?
pub fn is_harakat(c: char) -> bool {
    matches!(c as u32, 0x0610..=0x061A | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x06E7..=0x06E8 | 0x06EA..=0x06ED | 0x08D3..=0x08FF)
}

/// Do character styles `a` and `b` differ, if at all, only in paint: fill, stroke and how the
/// stroke is drawn, overprint, and the character style name? Those are not read by the shaper
/// and change no glyph, advance or position.
pub fn paint_only(a: &CharStyle, b: &CharStyle) -> bool {
    let mut b = b.clone();
    b.fill = a.fill.clone();
    b.stroke = a.stroke.clone();
    b.stroke_width = a.stroke_width;
    b.overprint_fill = a.overprint_fill;
    b.overprint_stroke = a.overprint_stroke;
    b.stroke_cap = a.stroke_cap;
    b.stroke_join = a.stroke_join;
    b.stroke_miter_limit = a.stroke_miter_limit;
    b.stroke_dash = a.stroke_dash.clone();
    b.stroke_align = a.stroke_align;
    b.style_name = a.style_name.clone();
    *a == b
}

/// Is the run boundary at byte `at` of `text`, between styles `before` and `after`, one that
/// stays inside a shaping cluster: they differ only in paint and a harakat is on either side?
pub(crate) fn paint_only_mark_boundary(text: &str, at: usize, before: &CharStyle, after: &CharStyle) -> bool {
    let next = text.get(at..).and_then(|s| s.chars().next());
    let previous = text.get(..at).and_then(|s| s.chars().next_back());
    (next.is_some_and(is_harakat) || previous.is_some_and(is_harakat)) && paint_only(before, after)
}

/// The byte ranges of the harakat in `text[range]`, each run of consecutive harakat as one range.
pub fn harakat_ranges(text: &str, range: std::ops::Range<usize>) -> Vec<std::ops::Range<usize>> {
    let Some(part) = text.get(range.clone()) else { return vec![] };
    let mut out: Vec<std::ops::Range<usize>> = vec![];
    for (i, c) in part.char_indices().filter(|(_, c)| is_harakat(*c)) {
        let (a, b) = (range.start + i, range.start + i + c.len_utf8());
        match out.last_mut() {
            Some(last) if last.end == a => last.end = b,
            _ => out.push(a..b),
        }
    }
    out
}

#[cfg(test)]
#[path = "tests_arabic_essentials.rs"]
mod tests;
