//! Caret / layout geometry — maps a source byte offset ⇆ an x pixel on a
//! visually-shaped line, so the web editor can draw its OWN caret over OUR
//! kashida-free render instead of leaning on a browser `<textarea>`.
//!
//! ## Why this module exists
//! [`crate::visual::shape_visual_line`] lays a line out in absolute **visual**
//! (left-to-right) order; x is measured in font units from the line's LEFT edge
//! (leftmost glyph's left edge = 0) and the painted line spans
//! `[0, total_x_advance]`. The editor needs two directions of that mapping:
//! * forward — a cursor at source byte `b` → the caret's x (to paint it),
//! * inverse — a click at x → the nearest source byte (to place the caret).
//!
//! ## Caret model (intentionally narrow)
//! A **single** caret. Full UAX #9 visual caret *movement* across direction
//! boundaries (the "press →, jump across an RTL island" behaviour) is explicitly
//! OUT of scope and deferred — we only answer "where does the caret for byte `b`
//! sit" and its inverse.
//!
//! We build a set of **caret stops**: source-byte boundary → x (font units, from
//! the left edge). For every glyph we add the boundary on each of its two source
//! sides, choosing the visual edge by the *run* direction at that byte:
//! * an RTL boundary's "start" side is the glyph's RIGHT edge, its "end" side the
//!   LEFT edge (text grows leftward);
//! * an LTR boundary is the mirror.
//! Plus the two line ends (`0` and `line.len()`) anchored by the base direction.
//! Within a run adjacent glyphs share an edge, so a shared byte boundary always
//! resolves to one x (proven in the run-internal case); at a run boundary we key
//! by byte so the map stays single-valued.
//!
//! This is **additive** — nothing here changes how a line shapes or paints.

use crate::bidi::{resolve_paragraph, Direction};
use crate::visual::shape_visual_line_weighted;
use std::collections::BTreeMap;

/// Snap a byte index to the nearest UTF-8 char boundary **at or before** it,
/// clamped to `0..=line.len()`. `line.len()` is always a boundary.
fn snap_to_boundary(line: &str, byte: usize) -> usize {
    let b = byte.min(line.len());
    if b == line.len() || line.is_char_boundary(b) {
        return b;
    }
    // Walk back to the previous boundary (at most 3 bytes for UTF-8).
    let mut p = b;
    while p > 0 && !line.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Direction of the run whose `byte_range` contains `byte`, falling back to the
/// paragraph base direction when no run does (line end, or a gap).
fn dir_at(runs: &[crate::bidi::BidiRun], base: Direction, byte: usize) -> Direction {
    for r in runs {
        if r.byte_range.contains(&byte) {
            return r.direction;
        }
    }
    base
}

/// Build the caret-stop table for a line: source-byte boundary → x (font units
/// from the left edge). Also returns `total_x_advance` (the line's painted
/// width) so callers needn't reshape to learn it.
///
/// Construction (per the module doc): prefix-sum glyph advances to get each
/// glyph's left/right edge in visual order, then for every glyph register a stop
/// on each of its two source-byte sides using the run direction at that byte to
/// pick the visual edge. The two line ends are always present, anchored by base
/// direction. A `BTreeMap` keeps boundaries ordered and single-valued.
fn build_stops(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    wght: Option<f32>,
) -> Result<(BTreeMap<usize, i32>, i32), &'static str> {
    let vl = shape_visual_line_weighted(font_bytes, line, n_kashida, wght)?;
    let total = vl.total_x_advance;

    let mut stops: BTreeMap<usize, i32> = BTreeMap::new();

    // Empty line: the only caret position is byte 0 at x = 0.
    if line.is_empty() {
        stops.insert(0, 0);
        return Ok((stops, total));
    }

    let res = resolve_paragraph(line);
    let base = res.base_direction;

    // Left edge of each glyph in visual order: left_x[i] = sum of advances of
    // glyphs 0..i. Right edge of glyph i = left_x[i] + advance[i].
    let mut left_x = 0i32;
    for g in &vl.glyphs {
        let l = left_x;
        let r = left_x + g.x_advance;

        // Source byte of this glyph's cluster, snapped to a real char boundary
        // (kashida-injected RTL clusters live in working-string space; snapping
        // pins them back onto a source boundary and ignores tatweel-only bytes).
        let c = snap_to_boundary(line, g.cluster as usize);
        let d = dir_at(&res.runs, base, c);

        // "start" side of the cluster (byte c).
        let start_x = if d == Direction::Rtl { r } else { l };
        stops.entry(c).or_insert(start_x);

        // "end" side of the cluster: the next distinct char boundary after c.
        // Use the next char boundary in the *source* string so the stop lands on
        // a real inter-character gap; for the typical 1-glyph-per-char case this
        // is the adjacent character. Clamp at line end.
        let end_byte = next_char_boundary(line, c);
        let end_x = if d == Direction::Rtl { l } else { r };
        stops.entry(end_byte).or_insert(end_x);

        left_x = r;
    }

    // Line ends, anchored by base direction. These are authoritative — overwrite
    // any glyph-derived value at the two extremes so the contract holds exactly:
    //   base Rtl → byte 0 = total (right edge), byte len = 0 (left edge)
    //   base Ltr → byte 0 = 0,                  byte len = total
    let (x0, xlen) = match base {
        Direction::Rtl => (total, 0),
        Direction::Ltr => (0, total),
    };
    stops.insert(0, x0);
    stops.insert(line.len(), xlen);

    Ok((stops, total))
}

/// Next UTF-8 char boundary strictly after `byte` (clamped to `line.len()`).
fn next_char_boundary(line: &str, byte: usize) -> usize {
    let mut p = byte + 1;
    while p < line.len() && !line.is_char_boundary(p) {
        p += 1;
    }
    p.min(line.len())
}

/// x of the text caret (in FONT UNITS, from the line's left edge) for the cursor
/// sitting at source byte offset `byte` (`0..=line.len()`). Built on
/// [`shape_visual_line`].
///
/// `byte` is snapped to a UTF-8 char boundary at/below it and clamped to the
/// line. Returns the stop x for that boundary; if the exact boundary is not a
/// known stop, returns the x of the nearest known boundary by byte distance.
/// Empty line → `Ok(0)`.
pub fn caret_x_units(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    byte: usize,
) -> Result<i32, &'static str> {
    caret_x_units_weighted(font_bytes, line, n_kashida, byte, None)
}

/// Like [`caret_x_units`], but builds the caret-stop table at the given
/// variable-font `wght` so cursor positions land on the *rendered* (weighted)
/// glyph edges. `wght == None` is byte-for-byte identical to [`caret_x_units`].
pub fn caret_x_units_weighted(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    byte: usize,
    wght: Option<f32>,
) -> Result<i32, &'static str> {
    let (stops, _total) = build_stops(font_bytes, line, n_kashida, wght)?;
    if line.is_empty() {
        return Ok(0);
    }
    let b = snap_to_boundary(line, byte);
    if let Some(&x) = stops.get(&b) {
        return Ok(x);
    }
    // Nearest known boundary by byte distance; ties → smaller byte.
    let mut best: Option<(usize, i32)> = None; // (byte_distance, x)
    for (&sb, &sx) in &stops {
        let dist = sb.abs_diff(b);
        match best {
            Some((bd, _)) if dist >= bd => {}
            _ => best = Some((dist, sx)),
        }
    }
    Ok(best.map(|(_, x)| x).unwrap_or(0))
}

/// Inverse hit-test: the source byte offset whose caret x (font units, from the
/// left) is nearest to `x_units`. Used to place the caret from a click.
///
/// Nearest stop by Euclidean distance on x; ties → smaller byte. Empty line →
/// `Ok(0)`.
pub fn byte_at_x_units(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    x_units: i32,
) -> Result<usize, &'static str> {
    byte_at_x_units_weighted(font_bytes, line, n_kashida, x_units, None)
}

/// Like [`byte_at_x_units`], but hit-tests against the caret-stop table built at
/// the given variable-font `wght`. `wght == None` is byte-for-byte identical to
/// [`byte_at_x_units`].
pub fn byte_at_x_units_weighted(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    x_units: i32,
    wght: Option<f32>,
) -> Result<usize, &'static str> {
    let (stops, _total) = build_stops(font_bytes, line, n_kashida, wght)?;
    if line.is_empty() {
        return Ok(0);
    }
    // BTreeMap iterates by ascending byte, so the first minimum encountered is
    // already the smallest byte for that distance → correct tie-break.
    let mut best: Option<(i64, usize)> = None; // (x_distance, byte)
    for (&sb, &sx) in &stops {
        let dist = (sx as i64 - x_units as i64).abs();
        match best {
            Some((bd, _)) if dist >= bd => {}
            _ => best = Some((dist, sb)),
        }
    }
    Ok(best.map(|(_, b)| b).unwrap_or(0))
}

/// The line's painted width in font units (`VisualLine::total_x_advance`). JS
/// needs it to convert a left-origin caret x into the renderer's right-anchored
/// absolute x.
pub fn line_total_advance_units(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
) -> Result<i32, &'static str> {
    line_total_advance_units_weighted(font_bytes, line, n_kashida, None)
}

/// Like [`line_total_advance_units`] at the given variable-font `wght`.
/// `wght == None` is byte-for-byte identical to [`line_total_advance_units`].
pub fn line_total_advance_units_weighted(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    wght: Option<f32>,
) -> Result<i32, &'static str> {
    Ok(shape_visual_line_weighted(font_bytes, line, n_kashida, wght)?.total_x_advance)
}

/// The font's units-per-em for this line (`VisualLine::units_per_em`). JS scales
/// font units → px with `font_size_px / upem`.
pub fn line_units_per_em(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
) -> Result<u32, &'static str> {
    line_units_per_em_weighted(font_bytes, line, n_kashida, None)
}

/// Like [`line_units_per_em`] at the given variable-font `wght`. Units-per-em is
/// weight-independent, so this is provided only for call-site symmetry.
/// `wght == None` is byte-for-byte identical to [`line_units_per_em`].
pub fn line_units_per_em_weighted(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    wght: Option<f32>,
) -> Result<u32, &'static str> {
    Ok(shape_visual_line_weighted(font_bytes, line, n_kashida, wght)?.units_per_em)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Same canonical Phase-0 test font, loaded exactly like visual.rs.
    const AMIRI: &[u8] = include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");

    /// Byte range of `sub` within `line`.
    fn range_of(line: &str, sub: &str) -> core::ops::Range<usize> {
        let s = line.find(sub).expect("substring present");
        s..s + sub.len()
    }

    /// All char-boundary byte offsets of `line`, ascending, including 0 and len.
    fn boundaries(line: &str) -> Vec<usize> {
        let mut v: Vec<usize> = (0..=line.len())
            .filter(|&b| line.is_char_boundary(b))
            .collect();
        v.dedup();
        v
    }

    #[test]
    fn pure_rtl_caret_decreases_left_to_right() {
        let line = "السلام عليكم";
        let total = line_total_advance_units(AMIRI, line, 0).unwrap();

        // Base RTL: byte 0 is the rightmost edge, byte len is the leftmost.
        assert_eq!(caret_x_units(AMIRI, line, 0, 0).unwrap(), total);
        assert_eq!(caret_x_units(AMIRI, line, 0, line.len()).unwrap(), 0);

        // Walking boundaries left→right in BYTE order, the caret x must strictly
        // decrease (RTL: higher byte = further left = smaller x).
        let bs = boundaries(line);
        let xs: Vec<i32> = bs
            .iter()
            .map(|&b| caret_x_units(AMIRI, line, 0, b).unwrap())
            .collect();
        for w in xs.windows(2) {
            assert!(
                w[0] > w[1],
                "RTL caret x must strictly decrease with byte: {xs:?}"
            );
        }
    }

    #[test]
    fn pure_ltr_caret_increases_left_to_right() {
        let line = "Hello";
        let total = line_total_advance_units(AMIRI, line, 0).unwrap();

        assert_eq!(caret_x_units(AMIRI, line, 0, 0).unwrap(), 0);
        assert_eq!(caret_x_units(AMIRI, line, 0, line.len()).unwrap(), total);

        let bs = boundaries(line);
        let xs: Vec<i32> = bs
            .iter()
            .map(|&b| caret_x_units(AMIRI, line, 0, b).unwrap())
            .collect();
        for w in xs.windows(2) {
            assert!(
                w[0] < w[1],
                "LTR caret x must strictly increase with byte: {xs:?}"
            );
        }
    }

    #[test]
    fn mixed_digit_island_sits_left_and_reads_ltr() {
        let line = "القاهرة مصر 2026";
        let total = line_total_advance_units(AMIRI, line, 0).unwrap();

        // Base is RTL.
        assert_eq!(caret_x_units(AMIRI, line, 0, 0).unwrap(), total);
        assert_eq!(caret_x_units(AMIRI, line, 0, line.len()).unwrap(), 0);

        let digits = range_of(line, "2026");
        let arabic = range_of(line, "القاهرة");

        // Caret stops strictly INSIDE each source range (exclude the shared
        // endpoints, whose x is governed by the neighbouring run/base and would
        // muddy a strict left/right island comparison).
        let inside = |r: &core::ops::Range<usize>| -> Vec<i32> {
            boundaries(line)
                .into_iter()
                .filter(|&b| b > r.start && b < r.end)
                .map(|b| caret_x_units(AMIRI, line, 0, b).unwrap())
                .collect()
        };
        let digit_xs = inside(&digits);
        let arabic_xs = inside(&arabic);
        assert!(!digit_xs.is_empty() && !arabic_xs.is_empty());

        // The "2026" LTR island is the visual-LEFT island: every digit-interior
        // stop is left of every Arabic-interior stop.
        let max_digit = *digit_xs.iter().max().unwrap();
        let min_arabic = *arabic_xs.iter().min().unwrap();
        assert!(
            max_digit < min_arabic,
            "digit island must sit left of Arabic (max_digit={max_digit} < min_arabic={min_arabic})"
        );

        // Within "2026" the caret x increases with byte (LTR sub-run). Include
        // the run's own interior boundaries in ascending byte order.
        let mut prev: Option<i32> = None;
        for b in boundaries(line)
            .into_iter()
            .filter(|&b| b > digits.start && b < digits.end)
        {
            let x = caret_x_units(AMIRI, line, 0, b).unwrap();
            if let Some(p) = prev {
                assert!(p < x, "within 2026 caret x must increase with byte");
            }
            prev = Some(x);
        }
    }

    #[test]
    fn byte_at_x_round_trips_to_equal_x() {
        let line = "القاهرة مصر 2026";
        // A handful of boundaries spread across the line.
        for &b in &boundaries(line) {
            let x = caret_x_units(AMIRI, line, 0, b).unwrap();
            let back = byte_at_x_units(AMIRI, line, 0, x).unwrap();
            // Nearest-stop: the returned byte's caret x must equal the queried x
            // (a coincident stop may map to a different byte but same x).
            let back_x = caret_x_units(AMIRI, line, 0, back).unwrap();
            assert_eq!(
                back_x, x,
                "round-trip byte {b} (x={x}) returned byte {back} (x={back_x})"
            );
        }
    }

    #[test]
    fn empty_line_returns_zero() {
        let line = "";
        assert_eq!(caret_x_units(AMIRI, line, 0, 0).unwrap(), 0);
        assert_eq!(caret_x_units(AMIRI, line, 0, 5).unwrap(), 0);
        assert_eq!(byte_at_x_units(AMIRI, line, 0, 0).unwrap(), 0);
        assert_eq!(byte_at_x_units(AMIRI, line, 0, 1234).unwrap(), 0);
    }

    #[test]
    fn helpers_report_upem_and_total() {
        let line = "السلام عليكم";
        assert_eq!(line_units_per_em(AMIRI, line, 0).unwrap(), 1000); // Amiri UPM
        assert!(line_total_advance_units(AMIRI, line, 0).unwrap() > 0);
        // Empty line still reports a real upem and zero advance.
        assert_eq!(line_units_per_em(AMIRI, "", 0).unwrap(), 1000);
        assert_eq!(line_total_advance_units(AMIRI, "", 0).unwrap(), 0);
    }

    #[test]
    fn non_char_boundary_byte_snaps_back() {
        // Arabic letters are 2 bytes each in UTF-8; byte 1 is mid-codepoint.
        let line = "السلام عليكم";
        let x0 = caret_x_units(AMIRI, line, 0, 0).unwrap();
        let x1 = caret_x_units(AMIRI, line, 0, 1).unwrap(); // snaps back to 0
        assert_eq!(x0, x1, "a mid-codepoint byte must snap to the previous boundary");
    }
}
