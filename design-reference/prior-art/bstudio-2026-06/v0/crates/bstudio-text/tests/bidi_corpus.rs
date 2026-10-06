//! Block D — BiDi corpus.
//!
//! Verifies that the Unicode Bidirectional Algorithm gives sane runs for
//! the kinds of mixed-script Arabic text Pivot is expected to handle on
//! day one: dates, brand names, URLs, parenthesised LTR fragments, etc.
//!
//! Each test asserts three properties (the minimum useful contract):
//!   1. The expected paragraph base direction.
//!   2. The expected number of runs (or at least a lower bound).
//!   3. The runs cover the full input with no gaps and no overlaps.
//!
//! Inkscape regression tests at the bottom reference real upstream bugs
//! (their bug numbers are noted in comments) that any RTL-aware text
//! tool needs to get right.

use bstudio_text::{resolve_paragraph, BidiDirection};

/// Helper: assert that runs cover the full input and are non-overlapping.
fn assert_runs_cover(text: &str, runs: &[bstudio_text::BidiRun]) {
    let total: usize = runs.iter().map(|r| r.byte_range.len()).sum();
    assert_eq!(
        total,
        text.len(),
        "runs must cover full input ({} bytes), got {}",
        text.len(),
        total
    );

    // Verify no overlaps and runs are sorted by start.
    let mut prev_end = 0;
    for run in runs {
        assert_eq!(
            run.byte_range.start, prev_end,
            "runs must be contiguous in logical order; gap or overlap at {}",
            run.byte_range.start
        );
        prev_end = run.byte_range.end;
    }
}

#[test]
fn pure_arabic() {
    let text = "السلام عليكم";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl);
    assert_eq!(r.runs.len(), 1, "pure Arabic is one run");
    assert_eq!(r.runs[0].direction, BidiDirection::Rtl);
    assert_runs_cover(text, &r.runs);
}

#[test]
fn arabic_with_english() {
    let text = "مرحبا Hello World";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl, "first strong is Arabic");
    assert!(r.runs.len() >= 2, "expected at least 2 runs, got {}", r.runs.len());
    assert!(r.runs.iter().any(|run| run.direction == BidiDirection::Ltr));
    assert!(r.runs.iter().any(|run| run.direction == BidiDirection::Rtl));
    assert_runs_cover(text, &r.runs);
}

#[test]
fn arabic_with_digits() {
    // "Year 2026 Gregorian" — digits in an Arabic sentence get their own
    // direction level. European digits (0-9) are EN per UBA, sandwiched
    // between RTL on both sides.
    let text = "السنة 2026 ميلادي";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl);
    assert!(r.runs.len() >= 2, "digits should form their own run");
    assert_runs_cover(text, &r.runs);
}

#[test]
fn arabic_with_brackets() {
    // Parentheses are neutral; their direction is resolved from context.
    // Inside an RTL paragraph they stay at RTL level — important so the
    // ( and ) glyphs swap visually.
    let text = "النص (مع أقواس) هنا";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl);
    assert_runs_cover(text, &r.runs);
    // All runs should be RTL (brackets inherit context).
    assert!(
        r.runs.iter().all(|run| run.direction == BidiDirection::Rtl),
        "neutral parens in Arabic should resolve to RTL, got {:?}",
        r.runs
    );
}

#[test]
fn arabic_with_latin_punct() {
    // Comma and period after Latin word — neutrals adjacent to LTR text.
    let text = "نص عربي, with English.";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl);
    assert!(r.runs.len() >= 2);
    assert!(r.runs.iter().any(|run| run.direction == BidiDirection::Ltr));
    assert_runs_cover(text, &r.runs);
}

#[test]
fn brand_in_middle() {
    // Brand name embedded in Arabic — a common Pivot Studio case.
    let text = "موقع Pivot Studio للتصميم";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl);
    let ltr_runs: Vec<_> = r
        .runs
        .iter()
        .filter(|run| run.direction == BidiDirection::Ltr)
        .collect();
    assert_eq!(ltr_runs.len(), 1, "exactly one LTR island for the brand");
    let brand_slice = &text[ltr_runs[0].byte_range.clone()];
    assert!(
        brand_slice.contains("Pivot"),
        "LTR run should cover 'Pivot Studio', got {:?}",
        brand_slice
    );
    assert_runs_cover(text, &r.runs);
}

#[test]
fn url_in_arabic() {
    // URLs are pure ASCII LTR; they MUST stay intact and readable in
    // visual order (no character-by-character reversal).
    let text = "زوروا https://example.com للمزيد";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl);
    assert!(r.runs.iter().any(|run| run.direction == BidiDirection::Ltr));
    assert_runs_cover(text, &r.runs);
    // The URL run should be a single contiguous LTR run.
    let ltr_runs: Vec<_> = r
        .runs
        .iter()
        .filter(|run| run.direction == BidiDirection::Ltr)
        .collect();
    assert!(!ltr_runs.is_empty());
    let combined: String = ltr_runs
        .iter()
        .map(|run| &text[run.byte_range.clone()])
        .collect();
    assert!(
        combined.contains("https://example.com"),
        "URL must be in LTR run(s), got combined LTR: {:?}",
        combined
    );
}

#[test]
fn mixed_levels() {
    // Three direction classes in one line: Arabic + English + digits.
    let text = "تاريخ Project 2026 بالميلادي";
    let r = resolve_paragraph(text);
    assert_eq!(r.base_direction, BidiDirection::Rtl);
    assert!(r.runs.len() >= 3, "expected at least 3 runs, got {}", r.runs.len());
    assert_runs_cover(text, &r.runs);
}

// ───────── Inkscape regression scenarios ─────────
// These don't fix specific Inkscape bugs in our codebase, but they
// exercise the same edge cases that bit Inkscape historically. Keeping
// them as regression tests so Pivot doesn't regress into the same holes.

#[test]
fn inkscape_1658510_paragraph_reorder() {
    // Reorder mixed paragraph: input order ≠ visual order, and we should
    // surface runs in *logical* order (the source order), letting the
    // renderer reorder visually. If runs ever come back in visual order,
    // the renderer would mirror them twice — invisible bug.
    let text = "نص one نص two";
    let r = resolve_paragraph(text);
    assert_runs_cover(text, &r.runs);
    // Runs must be sorted by byte_range.start (logical order).
    let starts: Vec<usize> = r.runs.iter().map(|run| run.byte_range.start).collect();
    let mut sorted = starts.clone();
    sorted.sort_unstable();
    assert_eq!(starts, sorted, "runs must arrive in logical order");
}

#[test]
fn inkscape_1718423_line_break_boundary() {
    // Boundaries adjacent to a level change. Make sure the level change
    // happens at the right byte boundary — off-by-one here means a glyph
    // joins the wrong run and gets shaped with the wrong script.
    let text = "abcعربي";
    let r = resolve_paragraph(text);
    assert!(r.runs.len() >= 2);
    let ltr = r
        .runs
        .iter()
        .find(|run| run.direction == BidiDirection::Ltr)
        .expect("expected an LTR run for 'abc'");
    assert_eq!(
        &text[ltr.byte_range.clone()],
        "abc",
        "LTR run must contain exactly 'abc', no Arabic bleed-in"
    );
    let rtl = r
        .runs
        .iter()
        .find(|run| run.direction == BidiDirection::Rtl)
        .expect("expected an RTL run for 'عربي'");
    assert_eq!(
        &text[rtl.byte_range.clone()],
        "عربي",
        "RTL run must contain exactly 'عربي'"
    );
}

#[test]
fn inkscape_1581275_cursor_positioning() {
    // Cursor positioning depends on byte-accurate run boundaries even
    // across UTF-8 multibyte chars. An Arabic char is 2 bytes in UTF-8;
    // a run boundary that lands inside a codepoint would explode.
    // We assert all run boundaries fall on char boundaries by reslicing —
    // if a boundary is mid-codepoint, &text[run.byte_range] panics.
    let text = "خط Pivot النهائي 100%";
    let r = resolve_paragraph(text);
    for run in &r.runs {
        // This would panic if a boundary is mid-codepoint.
        let slice = &text[run.byte_range.clone()];
        // All resulting slices must be valid UTF-8 (str slice guarantees this).
        assert!(!slice.is_empty() || run.byte_range.is_empty());
    }
    assert_runs_cover(text, &r.runs);
}

// ───────── shape_paragraph end-to-end ─────────

const AMIRI: &[u8] = include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");

#[test]
fn shape_paragraph_mixed_produces_run_per_bidi_run() {
    let text = "موقع Pivot Studio للتصميم";
    let runs = bstudio_text::shape_paragraph(AMIRI, text).expect("shape_paragraph");
    // We expect 3 runs: Arabic | "Pivot Studio" | Arabic.
    assert!(
        runs.len() >= 2,
        "expected at least 2 shaped runs for mixed text, got {}",
        runs.len()
    );
    // Each run carries a positive advance.
    for run in &runs {
        assert!(run.total_x_advance > 0, "every run must have advance > 0");
        assert!(!run.glyphs.is_empty(), "every run must have glyphs");
    }
}

#[test]
fn shape_paragraph_pure_arabic_is_single_run() {
    let text = "بسم الله الرحمن الرحيم";
    let runs = bstudio_text::shape_paragraph(AMIRI, text).expect("shape_paragraph");
    assert_eq!(runs.len(), 1, "pure Arabic should produce 1 run");
    assert_eq!(runs[0].direction, bstudio_text::TextDirection::RightToLeft);
}

#[test]
fn shape_paragraph_handles_empty_input() {
    let runs = bstudio_text::shape_paragraph(AMIRI, "").expect("shape_paragraph empty");
    assert!(runs.is_empty(), "empty input → no runs");
}
