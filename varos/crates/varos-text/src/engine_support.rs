//! Shared line/caret/source invariants for cold and incremental layout.
//! Directional-edge geometry follows BStudio caret.rs (Ahmed, MPL-2.0; see NOTICE),
//! retaining Varos grapheme boundaries and both affinities instead of scalar snapping.
use crate::*;
use cosmic_text::{AttrsList, FontFeatures, FontSystem, ShapeLine, ShapeSpan, Shaping};
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;
pub(crate) fn ignorable(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{fe0f}')
}
pub(crate) fn build_carets(layout: &mut Layout, boundaries: &[usize]) {
    let mut legal = vec![false; layout.source.len() + 1];
    for &byte in boundaries {
        legal[byte] = true;
    }
    layout.carets.reserve(boundaries.len().saturating_mul(2));
    let mut cluster_boundaries = Vec::new();
    for (line_i, line) in layout.lines.iter().enumerate() {
        line_carets(line, line_i, &layout.source, &legal, &mut cluster_boundaries, &mut layout.carets);
    }
    // Stable finite visual walk. Coincident carets retain byte+affinity identity.
    layout.carets.sort_by(caret_order);
    layout.carets.dedup();
}
/// Visual caret order; `line` is the primary key, so per-line sorting is identical.
pub(crate) fn caret_order(a: &Caret, b: &Caret) -> std::cmp::Ordering {
    a.line.cmp(&b.line).then(a.x.total_cmp(&b.x)).then(a.byte.cmp(&b.byte)).then(a.affinity.cmp(&b.affinity))
}
/// Unsorted caret candidates for one line (shared by cold and convergent layout).
pub(crate) fn line_carets(
    line: &Line,
    line_i: usize,
    source: &str,
    legal: &[bool],
    cluster_boundaries: &mut Vec<usize>,
    carets: &mut Vec<Caret>,
) {
    // L2 keeps a cluster contiguous; avoid a tree allocation per cluster.
    for group in line.glyphs.chunk_by(|a, b| a.cluster == b.cluster) {
        let start = group[0].cluster.start;
        let end = group[0].cluster.end;
        let level = group[0].level;
        let left = group.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        let right = group.iter().map(|g| g.x + g.advance).fold(f32::NEG_INFINITY, f32::max);
        cluster_boundaries.clear();
        cluster_boundaries.extend(source[start..end].grapheme_indices(true).map(|(i, _)| start + i).chain([end]));
        let count = cluster_boundaries.len().saturating_sub(1).max(1);
        for (i, &byte) in cluster_boundaries.iter().enumerate() {
            if !legal[byte] {
                continue;
            }
            let t = i as f32 / count as f32;
            let x = if level % 2 == 1 { right - (right - left) * t } else { left + (right - left) * t };
            if i > 0 {
                carets.push(Caret { byte, affinity: Affinity::Upstream, line: line_i, x });
            }
            if i < count {
                carets.push(Caret { byte, affinity: Affinity::Downstream, line: line_i, x });
            }
        }
    }
    if line.glyphs.is_empty() {
        for affinity in [Affinity::Upstream, Affinity::Downstream] {
            carets.push(Caret { byte: line.range.start, affinity, line: line_i, x: line.empty_caret_x });
        }
    }
}
/// Final glyph y: `baseline + raw COSMIC y - style shift`, in the cold engine's order.
pub(crate) fn apply_baseline(line: &mut Line, top: f32, styles: &[Style]) {
    line.baseline = top + line.ascent;
    for g in &mut line.glyphs {
        let shift = styles.iter().find(|s| s.range.contains(&g.cluster.start)).map_or(0., |s| s.baseline_shift);
        g.y = line.baseline + g.y - shift;
    }
}
/// Line-edge reshaping with HarfRust context clipped to `range`. Offsets stay
/// paragraph-relative; `attrs` must describe every paragraph byte in `range`.
pub(crate) fn reshape_edge(
    fonts: &mut FontSystem,
    text: &str,
    attrs: &AttrsList,
    levels: &[Level],
    rtl: bool,
    range: Range<usize>,
) -> ShapeLine {
    let slice = &text[range.clone()];
    let mut local_attrs = AttrsList::new(&attrs.get_span(range.start));
    for (r, a) in attrs.spans() {
        let start = r.start.max(range.start);
        let end = r.end.min(range.end);
        if start < end {
            local_attrs.add_span(start - range.start..end - range.start, &a.as_attrs());
        }
    }
    let mut edge = ShapeLine::new(fonts, "", &local_attrs, Shaping::Advanced, 4);
    edge.rtl = rtl;
    let mut start = range.start;
    while start < range.end {
        let l = levels[start];
        let end = text[start..range.end]
            .char_indices()
            .skip(1)
            .map(|(i, _)| start + i)
            .find(|i| levels[*i] != l)
            .unwrap_or(range.end);
        let mut span = ShapeSpan::new(
            fonts,
            slice,
            &local_attrs,
            start - range.start..end - range.start,
            rtl,
            l,
            Shaping::Advanced,
        );
        for word in &mut span.words {
            for g in &mut word.glyphs {
                g.start += range.start;
                g.end += range.start;
            }
        }
        edge.spans.push(span);
        start = end;
    }
    edge
}
pub fn paste_normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Minimal editing evidence, independent of COSMIC's editor/keymap/undo stack.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledText {
    pub text: String,
    pub paint_per_grapheme: Vec<u32>,
}
impl StyledText {
    pub fn new(text: &str) -> Self {
        Self { text: text.into(), paint_per_grapheme: vec![0; text.graphemes(true).count()] }
    }
    pub fn replace(&mut self, range: Range<usize>, insert: &str, paint: u32) -> Result<Self, &'static str> {
        let bounds: Vec<_> = self.text.grapheme_indices(true).map(|(i, _)| i).chain([self.text.len()]).collect();
        let start = bounds.iter().position(|i| *i == range.start).ok_or("illegal start")?;
        let end = bounds.iter().position(|i| *i == range.end).ok_or("illegal end")?;
        if start > end {
            return Err("reversed range");
        }
        let undo = self.clone();
        let mut candidate = self.text.clone();
        candidate.replace_range(range, insert);
        let mut paints = self.paint_per_grapheme.clone();
        paints.splice(start..end, std::iter::repeat_n(paint, insert.graphemes(true).count()));
        if candidate.graphemes(true).count() != paints.len() {
            return Err("edit merges graphemes: unsupported");
        }
        self.text = candidate;
        self.paint_per_grapheme = paints;
        Ok(undo)
    }
}

pub(crate) fn reorder_clusters(glyphs: &mut Vec<Glyph>, levels: &[Level], offset: usize) {
    if glyphs.is_empty() {
        return;
    }
    let mut x = glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
    // Stable sorting changes cluster order but preserves mark/base order.
    glyphs.sort_by_key(|g| g.cluster.start);
    let logical = std::mem::take(glyphs);
    glyphs.reserve(logical.len());
    let mut groups = Vec::new();
    let mut start = 0;
    for group in logical.chunk_by(|a, b| a.cluster.start == b.cluster.start) {
        groups.push(start..start + group.len());
        start += group.len();
    }
    let cluster_levels: Vec<_> = groups.iter().map(|r| levels[logical[r.start].cluster.start - offset]).collect();
    for index in BidiInfo::reorder_visual(&cluster_levels) {
        let group = &logical[groups[index].clone()];
        let left = group.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        let right = group.iter().map(|g| g.x + g.advance).fold(f32::NEG_INFINITY, f32::max);
        for mut g in group.iter().cloned() {
            g.x += x - left;
            g.level = cluster_levels[index].number();
            glyphs.push(g);
        }
        x += right - left;
    }
}

/// A paragraph without its trailing mandatory-break characters.
pub(crate) fn paragraphs_trim(raw: &str) -> &str {
    raw.trim_end_matches(['\n', '\r', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}'])
}
/// Mandatory UAX #14 boundaries retain their original source bytes (including CRLF).
pub fn paragraphs(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (end, opportunity) in unicode_linebreak::linebreaks(text) {
        if opportunity == unicode_linebreak::BreakOpportunity::Mandatory {
            out.push(&text[start..end]);
            start = end;
        }
    }
    if text.is_empty() || text.ends_with(['\n', '\r', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}']) {
        out.push("");
    }
    out
}
/// Line-local upstream UBA L1 snapshot over retained paragraph analysis.
pub(crate) fn line_levels_from(
    text: &str,
    classes: &[unicode_bidi::BidiClass],
    levels: &[Level],
    level: Level,
    range: Range<usize>,
) -> Vec<Level> {
    let len = range.len();
    let local = BidiInfo {
        text: &text[range.clone()],
        original_classes: classes[range.clone()].to_vec(),
        levels: levels[range].to_vec(),
        paragraphs: vec![],
    };
    local.reordered_levels(&unicode_bidi::ParagraphInfo { range: 0..len, level }, 0..len)
}

fn validate_language(language: &str) -> Result<(), &'static str> {
    // Bounded BCP-47 syntax subset used by this experiment (no grandfathered tags).
    if language.len() > 63 {
        return Err("invalid language");
    }
    let mut parts = language.split('-');
    let first = parts.next().unwrap_or("");
    if !(2..=8).contains(&first.len())
        || !first.bytes().all(|b| b.is_ascii_alphabetic())
        || parts.any(|part| part.is_empty() || part.len() > 8 || !part.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return Err("invalid language");
    }
    Ok(())
}

/// Shared validation for cold layout and cached updates, before any source publication.
pub(crate) fn validate_request(req: &Request<'_>) -> Result<(FontFeatures, Vec<usize>), &'static str> {
    if req.text.len() > 1_048_576
        || req.styles.len() > 16_384
        || req.language_runs.len() > 16_384
        || req.features.len() > 128
    {
        return Err("resource limit");
    }
    if !req.size.is_finite()
        || !(0.1..=2000.).contains(&req.size)
        || req.width.is_some_and(|w| !w.is_finite() || w <= 0.)
    {
        return Err("invalid metrics");
    }
    let mut features = FontFeatures::new();
    for feature in &req.features {
        if feature.value == 0 && [*b"rlig", *b"init", *b"medi", *b"fina", *b"isol"].contains(&feature.tag) {
            return Err("required joining feature cannot be disabled");
        }
        features.set(cosmic_text::FeatureTag::new(&feature.tag), feature.value);
    }
    let boundaries: Vec<_> = req.text.grapheme_indices(true).map(|(i, _)| i).chain([req.text.len()]).collect();
    let mut previous_language_end = 0;
    validate_language(req.language)?;
    for run in &req.language_runs {
        validate_language(&run.language)?;
        if run.range.start < previous_language_end
            || run.range.start >= run.range.end
            || !boundaries.contains(&run.range.start)
            || !boundaries.contains(&run.range.end)
        {
            return Err("invalid language range");
        }
        previous_language_end = run.range.end;
    }
    let mut previous_end = 0;
    for s in &req.styles {
        if s.range.start < previous_end
            || s.range.start >= s.range.end
            || !boundaries.contains(&s.range.start)
            || !boundaries.contains(&s.range.end)
            || !s.size.is_finite()
            || !(0.1..=2000.).contains(&s.size)
            || !s.baseline_shift.is_finite()
            || s.baseline_shift.abs() > 2000.
        {
            return Err("invalid style");
        }
        previous_end = s.range.end;
    }
    Ok((features, boundaries))
}

/// Linear paragraph itemization: Common/Inherited graphemes inherit the preceding
/// strong script, or the first following strong script at paragraph start. Explicit
/// run overrides take precedence. No cut is introduced inside an extended grapheme.
pub(crate) fn script_ranges(text: &str) -> Vec<(Range<usize>, Option<cosmic_text::harfrust::Script>)> {
    use unicode_script::{Script, UnicodeScript};
    let strong = |c: char| {
        let s = c.script();
        (!matches!(s, Script::Common | Script::Inherited | Script::Unknown)).then_some(s)
    };
    let mut current = text.chars().find_map(strong);
    let to_hb = |s: Option<Script>| s.and_then(|s| s.short_name().parse().ok());
    let mut start = 0;
    let mut ranges = Vec::new();
    for (i, g) in text.grapheme_indices(true) {
        if let Some(script) = g.chars().find_map(strong) {
            if current != Some(script) {
                if i > start {
                    ranges.push((start..i, to_hb(current)));
                }
                start = i;
                current = Some(script);
            }
        }
    }
    ranges.push((start..text.len(), to_hb(current)));
    ranges
}

#[derive(Clone, Copy, Debug)]
pub enum CaretMove {
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
}
impl Layout {
    pub fn caret_move(&self, current: &Caret, motion: CaretMove) -> Option<&Caret> {
        let index = self.carets.iter().position(|c| c == current)?;
        let words: Vec<usize> =
            self.source.split_word_bound_indices().map(|(i, _)| i).chain([self.source.len()]).collect();
        match motion {
            CaretMove::Home => self.carets.iter().find(|c| c.line == current.line),
            CaretMove::End => self.carets.iter().rev().find(|c| c.line == current.line),
            CaretMove::Left | CaretMove::WordLeft => self.carets[..index]
                .iter()
                .rev()
                .find(|c| c.byte != current.byte && (matches!(motion, CaretMove::Left) || words.contains(&c.byte))),
            CaretMove::Right | CaretMove::WordRight => self.carets[index + 1..]
                .iter()
                .find(|c| c.byte != current.byte && (matches!(motion, CaretMove::Right) || words.contains(&c.byte))),
        }
    }
    /// Y-down rectangles per selected visual cluster; bidi ranges may yield several rectangles.
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<[f32; 4]> {
        if range.start >= range.end || self.source.get(range.clone()).is_none() {
            return vec![];
        }
        let mut rects = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            for group in line.glyphs.chunk_by(|a, b| a.cluster == b.cluster) {
                let cluster = &group[0].cluster;
                let start = range.start.max(cluster.start);
                let end = range.end.min(cluster.end);
                if start >= end {
                    continue;
                }
                let x_at = |byte, affinity| {
                    self.carets.iter().find(|c| c.line == i && c.byte == byte && c.affinity == affinity).map(|c| c.x)
                };
                if let (Some(a), Some(b)) = (x_at(start, Affinity::Downstream), x_at(end, Affinity::Upstream)) {
                    rects.push([a.min(b), line.baseline - line.ascent, a.max(b), line.baseline + line.descent]);
                }
            }
        }
        rects
    }
}
impl Engine {
    /// Bounded display-only truncation at logical grapheme boundaries, reshaped with ellipsis.
    pub fn elide(&mut self, req: &Request<'_>, width: f32) -> Result<Layout, &'static str> {
        self.validate_faces(req)?;
        validate_request(req)?;
        if !width.is_finite() || width <= 0. {
            return Err("invalid elision width");
        }
        let boundaries: Vec<_> =
            req.text.grapheme_indices(true).map(|(i, _)| i).chain([req.text.len()]).filter(|i| *i <= 4096).collect();
        let shape = |engine: &mut Self, end: usize, ellipsis: bool| {
            let text = format!("{}{}", &req.text[..end], if ellipsis { "…" } else { "" });
            let mut r = req.clone();
            r.text = &text;
            r.width = None;
            r.styles.retain(|s| s.range.start < end);
            for s in &mut r.styles {
                s.range.end = s.range.end.min(end);
            }
            r.language_runs.retain(|s| s.range.start < end);
            for s in &mut r.language_runs {
                s.range.end = s.range.end.min(end);
            }
            engine.layout(&r)
        };
        if req.text.len() <= 4096 {
            let full = shape(self, req.text.len(), false)?;
            if full.lines.iter().all(|l| l.width <= width) {
                return Ok(full);
            }
        }
        let mut low = 0;
        let mut high = boundaries.len() - 1;
        while low < high {
            let mid = (low + high).div_ceil(2);
            let layout = shape(self, boundaries[mid], true)?;
            if layout.lines.iter().all(|l| l.width <= width) {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        let mut layout = shape(self, boundaries[low], true)?;
        if layout.lines.iter().any(|l| l.width > width) {
            layout.issues.push(Issue::Overflow(0));
        }
        Ok(layout)
    }
}

/// UBA L1 levels for a UTF-8 line range; backend bidi types remain private.
pub fn local_line_levels(text: &str, direction: Direction, range: Range<usize>) -> Result<Vec<u8>, &'static str> {
    if text.get(range.clone()).is_none() {
        return Err("invalid line range");
    }
    let base = match direction {
        Direction::Auto => None,
        Direction::Ltr => Some(Level::ltr()),
        Direction::Rtl => Some(Level::rtl()),
    };
    let bidi = BidiInfo::new(text, base);
    let paragraph = bidi
        .paragraphs
        .iter()
        .find(|p| p.range.start <= range.start && p.range.end >= range.end)
        .ok_or("line crosses paragraph")?;
    Ok(line_levels_from(text, &bidi.original_classes, &bidi.levels, paragraph.level, range)
        .iter()
        .map(|l| l.number())
        .collect())
}
