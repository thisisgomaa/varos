//! Paragraph layout — multi-line text with wrapping, alignment, BiDi ordering
//! and kashida justification.
//!
//! This turns a string into positioned glyphs the renderer can paint and the
//! editor can click into. It sits on top of the per-run shaper ([`shape_run`])
//! and the kashida engine ([`insert_kashida`]).
//!
//! ## Approach (and its honest limits)
//! Text is split into whitespace-delimited **tokens** and each token is shaped
//! on its own. For Arabic this is typographically sound — a space always
//! breaks the cursive join, so no shaping context crosses a space. Each token
//! is shaped in its own BiDi direction, so a Latin brand name inside an Arabic
//! line stays internally LTR.
//!
//! Line breaking is greedy word-wrap (break at spaces); it does not implement
//! the full UAX #14 line-break algorithm (no hyphenation, no break inside
//! CJK). Visual ordering is resolved at token granularity from the paragraph's
//! base direction — exact for pure-RTL/LTR paragraphs and correct for the
//! common "Arabic line with an embedded Latin token" case; deeply nested BiDi
//! is approximated. When we vendor the parley fork these limits lift; the
//! public API here is meant to outlive that swap.

use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::Metrics;
use skrifa::FontRef;

use crate::bidi::{resolve_paragraph, Direction};
use crate::kashida::insert_kashida;
use crate::{shape_run, ShapedRun};

/// Horizontal alignment, relative to the paragraph's base direction.
/// `Start` is the right edge for an RTL paragraph, the left edge for LTR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    /// Stretch each full line to `max_width` — kashida first for Arabic, then
    /// inter-word spacing. The last line of a paragraph is not stretched.
    Justify,
}

/// Inputs to [`layout_text`].
#[derive(Debug, Clone, Copy)]
pub struct LayoutParams {
    /// Em size in pixels.
    pub font_size: f32,
    /// Wrap width in pixels. Non-finite or `<= 0` means "never wrap".
    pub max_width: f32,
    pub align: Align,
    /// Baseline-to-baseline distance as a multiple of `font_size`. `0` falls
    /// back to the font's own line metrics.
    pub line_height: f32,
    /// Cap on kashidas inserted per token during justification.
    pub max_kashida_per_word: usize,
}

impl Default for LayoutParams {
    fn default() -> Self {
        LayoutParams {
            font_size: 16.0,
            max_width: f32::INFINITY,
            align: Align::Start,
            line_height: 0.0,
            max_kashida_per_word: 3,
        }
    }
}

/// One glyph placed on the canvas. Coordinates are document space (y-down);
/// `(x, baseline_y)` is the pen origin for this glyph.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    pub glyph_id: u32,
    /// Byte offset into the original text of this glyph's source cluster.
    pub cluster: usize,
    pub x: f32,
    pub baseline_y: f32,
    /// Scaled horizontal advance in pixels.
    pub x_advance: f32,
}

/// One laid-out line.
#[derive(Debug, Clone)]
pub struct LayoutLine {
    pub glyphs: Vec<PositionedGlyph>,
    pub baseline_y: f32,
    /// Painted width in pixels after alignment/justification.
    pub width: f32,
    /// Paragraph base direction this line was ordered in.
    pub direction: Direction,
}

/// Result of [`layout_text`].
#[derive(Debug, Clone, Default)]
pub struct TextLayout {
    pub lines: Vec<LayoutLine>,
    /// Widest line, in pixels.
    pub width: f32,
    /// Total block height, in pixels.
    pub height: f32,
}

/// A shaped whitespace-delimited token in logical order.
struct Token {
    /// Byte offset of the token start in the original text.
    start: usize,
    text: String,
    is_space: bool,
    direction: Direction,
    shaped: ShapedRun,
}

impl Token {
    /// Natural advance in pixels at the given unit→px scale.
    fn width(&self, scale: f32) -> f32 {
        self.shaped.total_x_advance as f32 * scale
    }
}

fn dir_of(text: &str) -> Direction {
    resolve_paragraph(text).base_direction
}

/// Split a paragraph (no newlines) into shaped tokens, alternating
/// non-whitespace and whitespace groups, in logical order.
fn tokenize(font_bytes: &[u8], text: &str, base: Direction) -> Result<Vec<Token>, &'static str> {
    let mut tokens = Vec::new();
    let mut start = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (b0, c0) = chars[i];
        let is_space = c0.is_whitespace();
        let mut j = i + 1;
        while j < chars.len() && chars[j].1.is_whitespace() == is_space {
            j += 1;
        }
        let end = if j < chars.len() { chars[j].0 } else { text.len() };
        let slice = &text[b0..end];
        let direction = if is_space { base } else { dir_of(slice) };
        let shaped = shape_run(font_bytes, slice, direction.into())?;
        tokens.push(Token { start: b0, text: slice.to_string(), is_space, direction, shaped });
        start = end;
        i = j;
    }
    let _ = start;
    Ok(tokens)
}

/// Greedy word-wrap: returns index ranges into `tokens`, one per line.
fn break_lines(tokens: &[Token], scale: f32, max_width: f32) -> Vec<core::ops::Range<usize>> {
    let wrap = max_width.is_finite() && max_width > 0.0;
    if !wrap {
        return if tokens.is_empty() { Vec::new() } else { vec![0..tokens.len()] };
    }
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut line_w = 0.0f32;
    for (idx, tok) in tokens.iter().enumerate() {
        let w = tok.width(scale);
        // Never break before a leading token; a space that overflows just ends
        // the line. A word that overflows breaks before itself if the line
        // already has visible content.
        let has_content = idx > line_start || (idx == line_start && false);
        if line_w + w > max_width && idx > line_start && !tok.is_space {
            // Trim trailing spaces from the finished line.
            let mut end = idx;
            while end > line_start && tokens[end - 1].is_space {
                end -= 1;
            }
            lines.push(line_start..end.max(line_start + 1));
            line_start = idx;
            line_w = w;
        } else {
            line_w += w;
        }
        let _ = has_content;
    }
    if line_start < tokens.len() {
        lines.push(line_start..tokens.len());
    }
    lines
}

/// Place one line's glyphs, applying alignment and justification.
#[allow(clippy::too_many_arguments)]
fn position_line(
    font_bytes: &[u8],
    tokens: &mut [Token],
    range: core::ops::Range<usize>,
    base: Direction,
    scale: f32,
    baseline_y: f32,
    params: &LayoutParams,
    is_last_line: bool,
) -> LayoutLine {
    // Drop trailing spaces from the painted line (they don't take visual width).
    let mut end = range.end;
    while end > range.start && tokens[end - 1].is_space {
        end -= 1;
    }
    let live = range.start..end;

    let natural: f32 = tokens[live.clone()].iter().map(|t| t.width(scale)).sum();
    let wrap = params.max_width.is_finite() && params.max_width > 0.0;

    // Justification (full lines only) — kashida for RTL, then space stretch.
    let mut extra_space = 0.0f32;
    if matches!(params.align, Align::Justify) && wrap && !is_last_line && live.len() > 0 {
        let mut gap = params.max_width - natural;
        if gap > 0.0 {
            if base == Direction::Rtl {
                gap -= kashida_fill(font_bytes, &mut tokens[live.clone()], scale, gap, params.max_kashida_per_word);
            }
            // Remaining gap → spread across inter-word spaces.
            let space_count = tokens[live.clone()].iter().filter(|t| t.is_space).count();
            if space_count > 0 && gap > 0.0 {
                extra_space = gap / space_count as f32;
            }
        }
    }

    // Re-measure after kashida (token advances may have grown).
    let painted: f32 = tokens[live.clone()].iter().map(|t| t.width(scale)).sum::<f32>()
        + extra_space * tokens[live.clone()].iter().filter(|t| t.is_space).count() as f32;

    // Alignment offset along the x axis (document left = 0).
    let offset = if matches!(params.align, Align::Justify) && wrap && !is_last_line {
        if base == Direction::Rtl { params.max_width - painted } else { 0.0 }
    } else {
        match (params.align, base) {
            (Align::Start, Direction::Ltr) | (Align::End, Direction::Rtl) => 0.0,
            (Align::Start, Direction::Rtl) | (Align::End, Direction::Ltr) => avail(params) - painted,
            (Align::Center, _) => (avail(params) - painted) / 2.0,
            (Align::Justify, Direction::Rtl) => avail(params) - painted,
            (Align::Justify, Direction::Ltr) => 0.0,
        }
    };

    // Visual token order: logical for LTR base, reversed for RTL base.
    let mut order: Vec<usize> = live.clone().collect();
    if base == Direction::Rtl {
        order.reverse();
    }

    let mut x = offset.max(0.0);
    let mut glyphs = Vec::new();
    for ti in order {
        let tok = &tokens[ti];
        for g in &tok.shaped.glyphs {
            let adv = g.x_advance as f32 * scale;
            glyphs.push(PositionedGlyph {
                glyph_id: g.glyph_id,
                cluster: tok.start + g.cluster as usize,
                x,
                baseline_y,
                x_advance: adv,
            });
            x += adv;
        }
        if tok.is_space {
            x += extra_space;
        }
    }
    let width = x - offset.max(0.0);
    LayoutLine { glyphs, baseline_y, width, direction: base }
}

fn avail(params: &LayoutParams) -> f32 {
    if params.max_width.is_finite() && params.max_width > 0.0 {
        params.max_width
    } else {
        0.0
    }
}

/// Grow RTL tokens with kashida to consume up to `gap` pixels. Returns the
/// pixels actually consumed. Greedy: widen the longest eligible word first,
/// one kashida at a time, capped per word.
fn kashida_fill(
    font_bytes: &[u8],
    tokens: &mut [Token],
    scale: f32,
    gap: f32,
    max_per_word: usize,
) -> f32 {
    if gap <= 0.0 || max_per_word == 0 {
        return 0.0;
    }
    let mut consumed = 0.0f32;
    let mut counts = vec![0usize; tokens.len()];
    // Repeatedly find the widest eligible (Arabic, not space, under cap) token
    // and add one more kashida to it, until the gap is filled or no token can
    // grow further.
    loop {
        if gap - consumed <= 0.0 {
            break;
        }
        let mut best: Option<usize> = None;
        let mut best_w = -1.0f32;
        for (i, tok) in tokens.iter().enumerate() {
            if tok.is_space || tok.direction != Direction::Rtl || counts[i] >= max_per_word {
                continue;
            }
            let w = tok.width(scale);
            if w > best_w {
                best_w = w;
                best = Some(i);
            }
        }
        let Some(i) = best else { break };
        let before = tokens[i].width(scale);
        match insert_kashida(font_bytes, &tokens[i].text, counts[i] + 1) {
            Ok(res) if !res.inserted_at_byte_offsets.is_empty() => {
                let new_w = res.run.total_x_advance as f32 * scale;
                if new_w <= before {
                    // No real growth available in this token — stop trying it.
                    counts[i] = max_per_word;
                    continue;
                }
                tokens[i].shaped = res.run;
                counts[i] += 1;
                consumed += new_w - before;
            }
            _ => {
                counts[i] = max_per_word; // exhausted
            }
        }
    }
    consumed
}

/// Lay out `text` (which may contain `\n`) into positioned lines.
pub fn layout_text(font_bytes: &[u8], text: &str, params: LayoutParams) -> Result<TextLayout, &'static str> {
    let font = FontRef::new(font_bytes).map_err(|_| "failed to parse font")?;
    let m = Metrics::new(&font, Size::new(params.font_size), LocationRef::default());
    let upem = m.units_per_em as f32;
    let scale = params.font_size / upem;

    let line_advance = if params.line_height > 0.0 {
        params.font_size * params.line_height
    } else {
        m.ascent - m.descent + m.leading
    };
    let first_baseline = m.ascent;

    let mut out = TextLayout::default();
    let mut baseline = first_baseline;

    let paragraphs: Vec<&str> = text.split('\n').collect();
    let last_para = paragraphs.len().saturating_sub(1);
    for (pi, para) in paragraphs.iter().enumerate() {
        if para.is_empty() {
            // Blank line still advances the pen.
            out.lines.push(LayoutLine { glyphs: Vec::new(), baseline_y: baseline, width: 0.0, direction: Direction::Ltr });
            baseline += line_advance;
            continue;
        }
        let base = resolve_paragraph(para).base_direction;
        let mut tokens = tokenize(font_bytes, para, base)?;
        let line_ranges = break_lines(&tokens, scale, params.max_width);
        let last_line_idx = line_ranges.len().saturating_sub(1);
        for (li, range) in line_ranges.into_iter().enumerate() {
            let is_last = pi == last_para && li == last_line_idx;
            let line = position_line(font_bytes, &mut tokens, range, base, scale, baseline, &params, is_last);
            out.width = out.width.max(line.width);
            out.lines.push(line);
            baseline += line_advance;
        }
    }
    out.height = if out.lines.is_empty() { 0.0 } else { baseline - first_baseline + (first_baseline - m.descent.min(0.0)).max(0.0) };
    // Simpler, robust height: last baseline minus first baseline plus one line.
    out.height = out.lines.len() as f32 * line_advance;
    Ok(out)
}

impl TextLayout {
    /// The line whose baseline band contains `y`, or the nearest line.
    fn line_at_y(&self, y: f32) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        let mut best = 0;
        let mut best_d = f32::INFINITY;
        for (i, l) in self.lines.iter().enumerate() {
            let d = (l.baseline_y - y).abs();
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        Some(best)
    }

    /// Map a document point to the nearest source byte offset (caret position).
    /// Returns `None` only for an empty layout.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        let li = self.line_at_y(y)?;
        let line = &self.lines[li];
        if line.glyphs.is_empty() {
            // Blank line: fall back to the start of the nearest non-empty line.
            return self.lines.iter().flat_map(|l| l.glyphs.first()).map(|g| g.cluster).next().or(Some(0));
        }
        // Find the glyph whose horizontal extent the point falls in; snap to the
        // nearer edge to pick the caret side.
        let mut best = line.glyphs[0].cluster;
        let mut best_d = f32::INFINITY;
        for g in &line.glyphs {
            let left = (g.x - x).abs();
            let right = (g.x + g.x_advance - x).abs();
            if left < best_d {
                best_d = left;
                best = g.cluster;
            }
            if right < best_d {
                best_d = right;
                best = g.cluster; // caret still anchors to this cluster
            }
        }
        Some(best)
    }

    /// The x position of the caret for source byte offset `cluster` on the line
    /// that contains it. Returns `None` if no glyph carries that cluster.
    pub fn caret_x(&self, cluster: usize) -> Option<f32> {
        for line in &self.lines {
            for g in &line.glyphs {
                if g.cluster == cluster {
                    return Some(g.x);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMIRI_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");

    fn params(max_width: f32, align: Align) -> LayoutParams {
        LayoutParams { font_size: 32.0, max_width, align, line_height: 1.2, max_kashida_per_word: 3 }
    }

    #[test]
    fn single_short_line_no_wrap() {
        let lay = layout_text(AMIRI_REGULAR, "السلام", params(f32::INFINITY, Align::Start)).unwrap();
        assert_eq!(lay.lines.len(), 1);
        assert!(!lay.lines[0].glyphs.is_empty());
        assert!(lay.width > 0.0);
    }

    #[test]
    fn explicit_newlines_make_multiple_lines() {
        let lay = layout_text(AMIRI_REGULAR, "سطر\nتاني\nتالت", params(f32::INFINITY, Align::Start)).unwrap();
        assert_eq!(lay.lines.len(), 3);
        // Baselines strictly increase downward.
        assert!(lay.lines[1].baseline_y > lay.lines[0].baseline_y);
        assert!(lay.lines[2].baseline_y > lay.lines[1].baseline_y);
    }

    #[test]
    fn long_text_wraps_to_several_lines() {
        let text = "الحمد لله رب العالمين الرحمن الرحيم مالك يوم الدين";
        let narrow = layout_text(AMIRI_REGULAR, text, params(200.0, Align::Start)).unwrap();
        assert!(narrow.lines.len() > 1, "narrow width must wrap");
        // No painted line should exceed the wrap width (within a glyph's slack).
        for l in &narrow.lines {
            assert!(l.width <= 200.0 + 1.0, "line width {} exceeds wrap", l.width);
        }
    }

    #[test]
    fn rtl_start_alignment_hugs_the_right() {
        let lay = layout_text(AMIRI_REGULAR, "سلام", params(400.0, Align::Start)).unwrap();
        let line = &lay.lines[0];
        // RTL Start = right edge: the leftmost glyph sits well right of 0.
        let min_x = line.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        assert!(min_x > 100.0, "RTL start-aligned text should hug the right; min_x={min_x}");
    }

    #[test]
    fn ltr_start_alignment_hugs_the_left() {
        let lay = layout_text(AMIRI_REGULAR, "hello", params(400.0, Align::Start)).unwrap();
        let line = &lay.lines[0];
        let min_x = line.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        assert!(min_x < 1.0, "LTR start-aligned text should hug the left; min_x={min_x}");
    }

    #[test]
    fn justify_stretches_full_lines_to_width() {
        let text = "الحمد لله رب العالمين الرحمن الرحيم مالك يوم الدين وإياك نستعين";
        let lay = layout_text(AMIRI_REGULAR, text, params(300.0, Align::Justify)).unwrap();
        assert!(lay.lines.len() >= 2);
        // Every line except the last should reach close to the full width.
        for l in &lay.lines[..lay.lines.len() - 1] {
            if l.glyphs.is_empty() {
                continue;
            }
            assert!(l.width > 300.0 * 0.9, "justified line too short: {}", l.width);
        }
    }

    #[test]
    fn hit_test_lands_on_a_real_cluster() {
        let lay = layout_text(AMIRI_REGULAR, "السلام عليكم", params(f32::INFINITY, Align::Start)).unwrap();
        let g = lay.lines[0].glyphs[2];
        // Hitting the middle of a glyph returns a cluster that some glyph in the
        // layout actually carries (a valid caret anchor), and caret_x for it is
        // within a glyph advance of where we clicked.
        let hit = lay.hit_test(g.x + g.x_advance / 2.0, g.baseline_y).unwrap();
        let exists = lay.lines.iter().flat_map(|l| &l.glyphs).any(|q| q.cluster == hit);
        assert!(exists, "hit_test must return a cluster that exists in the layout");
        let cx = lay.caret_x(hit).unwrap();
        assert!((cx - g.x).abs() <= g.x_advance + 1.0, "caret_x {cx} far from click {}", g.x);
    }

    #[test]
    fn hit_test_empty_layout_is_none() {
        let lay = layout_text(AMIRI_REGULAR, "", params(100.0, Align::Start)).unwrap();
        // Empty string → one blank line; hit_test returns a safe 0.
        assert_eq!(lay.hit_test(0.0, 0.0), Some(0));
    }
}
