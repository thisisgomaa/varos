//! Small public-API adapter: explicit UBA base level -> COSMIC ShapeSpan.
//! P1b patched run language/script and paragraph-level legal wrapping.
use crate::{INTER, PLEX};
use cosmic_text::{Attrs, AttrsList, AttrsOwned, Family, FontFeatures, FontSystem, ShapeLine, ShapeSpan, Shaping};
use skrifa::{FontRef, MetadataProvider};
use std::collections::BTreeMap;
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Auto,
    Ltr,
    Rtl,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Inter,
    Plex,
}
impl Face {
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Inter => INTER,
            Self::Plex => PLEX,
        }
    }
    pub fn family(self) -> &'static str {
        match self {
            Self::Inter => "Inter",
            Self::Plex => "IBM Plex Sans Arabic",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Style {
    pub range: Range<usize>,
    pub face: Face,
    pub size: f32,
    pub baseline_shift: f32,
    pub paint: u32,
}
#[derive(Clone, Debug)]
pub struct Feature {
    pub tag: [u8; 4],
    pub value: u32,
}
#[derive(Clone, Debug)]
pub struct LanguageRun {
    pub range: Range<usize>,
    pub language: String,
    pub script: Option<cosmic_text::harfrust::Script>,
}
#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub text: &'a str,
    pub direction: Direction,
    pub language: &'a str,
    pub script: Option<cosmic_text::harfrust::Script>,
    pub size: f32,
    pub width: Option<f32>,
    pub face: Face,
    pub end_align: bool,
    pub styles: Vec<Style>,
    pub features: Vec<Feature>,
    pub language_runs: Vec<LanguageRun>,
}
impl<'a> Request<'a> {
    pub fn new(text: &'a str, size: f32, width: Option<f32>) -> Self {
        Self {
            text,
            size,
            width,
            direction: Direction::Auto,
            language: "und",
            script: None,
            face: Face::Inter,
            end_align: false,
            styles: vec![],
            features: vec![],
            language_runs: vec![],
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    UnsupportedLanguage(String),
    Substituted { range: Range<usize>, requested: Face, actual: Face },
    UnsupportedCluster(Range<usize>),
    Overflow(usize),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub id: u16,
    pub face: Face,
    pub cluster: Range<usize>,
    pub level: u8,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub offset: [f32; 2],
    pub size: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Affinity {
    Upstream,
    Downstream,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Caret {
    pub byte: usize,
    pub affinity: Affinity,
    pub line: usize,
    pub x: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub range: Range<usize>,
    pub rtl: bool,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub width: f32,
    pub empty_caret_x: f32,
    pub glyphs: Vec<Glyph>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub source: String,
    pub lines: Vec<Line>,
    pub carets: Vec<Caret>,
    pub issues: Vec<Issue>,
    /// Resolved UBA embedding levels per source byte. Newline bytes retain base level.
    pub levels: Vec<u8>,
    pub ink_bounds: Option<[f32; 4]>,
}
impl Layout {
    pub fn copy(&self, range: Range<usize>) -> Option<&str> {
        self.source.get(range)
    }
    pub fn caret(&self, byte: usize, affinity: Affinity) -> Vec<&Caret> {
        self.carets.iter().filter(|c| c.byte == byte && c.affinity == affinity).collect()
    }
    /// Coincident bidi/control stops are inherently ambiguous. Return all ties;
    /// a host must retain affinity and traversal identity, not pretend x is bijective.
    pub fn hit(&self, line: usize, x: f32) -> Vec<&Caret> {
        let best = self.carets.iter().filter(|c| c.line == line).map(|c| (c.x - x).abs()).fold(f32::INFINITY, f32::min);
        self.carets.iter().filter(|c| c.line == line && ((c.x - x).abs() - best).abs() < 0.001).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SpanKey {
    text: String,
    pre: String,
    post: String,
    default_attrs: AttrsOwned,
    attrs: Vec<(Range<usize>, AttrsOwned)>,
    level: u8,
    rtl: bool,
}
pub struct Engine {
    span_cache: std::collections::HashMap<SpanKey, ShapeSpan>,
    span_cache_bytes: usize,
    span_hits: usize,
    span_misses: usize,
    pub(crate) fonts: FontSystem,
    ids: Vec<(fontdb::ID, Face)>,
    coverage: [Vec<u64>; 2],
    ink_cache: BTreeMap<(u8, u16, u32), Option<[f32; 4]>>,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new(true)
    }
}
impl Engine {
    pub fn new(with_plex: bool) -> Self {
        let mut db = fontdb::Database::new();
        db.load_font_data(INTER.to_vec());
        if with_plex {
            db.load_font_data(PLEX.to_vec());
        }
        db.set_sans_serif_family("Inter");
        let ids = db
            .faces()
            .map(|f| (f.id, if f.families.iter().any(|f| f.0 == "Inter") { Face::Inter } else { Face::Plex }))
            .collect();
        let coverage = [Face::Inter, Face::Plex].map(|face| {
            // Fixed Unicode scalar bitset: no pointer chasing for each character
            // in every fallback/unsupported-cluster coverage query.
            let mut bits = vec![0u64; 0x110000 / 64];
            for (c, _) in FontRef::new(face.bytes()).unwrap().charmap().mappings() {
                if char::from_u32(c).is_some() {
                    bits[c as usize / 64] |= 1u64 << (c % 64);
                }
            }
            bits
        });
        Self {
            span_cache: std::collections::HashMap::new(),
            span_cache_bytes: 0,
            span_hits: 0,
            span_misses: 0,
            fonts: FontSystem::new_with_locale_and_db_and_fallback("en-US".into(), db, PinnedFallback),
            ids,
            coverage,
            ink_cache: BTreeMap::new(),
        }
    }
    /// Payload estimate, excluding allocator/BTree overhead and immutable font bytes.
    pub fn cache_bytes(&self) -> usize {
        self.fonts.shape_run_cache.payload_bytes()
            + self.span_cache_bytes
            + self.coverage.iter().map(|bits| bits.capacity() * std::mem::size_of::<u64>()).sum::<usize>()
            + self.ink_cache.len() * (std::mem::size_of::<(u8, u16, u32)>() + std::mem::size_of::<Option<[f32; 4]>>())
    }
    pub fn shape_span_cache_stats(&self) -> (usize, usize) {
        (self.span_hits, self.span_misses)
    }
    /// Reuse a resolved bidi span only with identical text, full attributes,
    /// direction and HarfRust's bounded surrounding context. No arbitrary chunks.
    pub(crate) fn shape_span_cached(
        &mut self,
        text: &str,
        attrs: &AttrsList,
        spans: &[(&Range<usize>, &AttrsOwned)],
        range: Range<usize>,
        rtl: bool,
        level: Level,
    ) -> ShapeSpan {
        let first = spans.partition_point(|(r, _)| r.end <= range.start);
        let key = SpanKey {
            text: text[range.clone()].into(),
            pre: text[..range.start].chars().rev().take(5).collect::<String>().chars().rev().collect(),
            post: text[range.end..].chars().take(5).collect(),
            default_attrs: AttrsOwned::new(&attrs.get_span(range.start)),
            attrs: spans[first..]
                .iter()
                .take_while(|(r, _)| r.start < range.end)
                .map(|(r, a)| {
                    (r.start.max(range.start) - range.start..r.end.min(range.end) - range.start, (*a).clone())
                })
                .collect(),
            level: level.number(),
            rtl,
        };
        if let Some(cached) = self.span_cache.get(&key) {
            self.span_hits += 1;
            let mut cached = cached.clone();
            for w in &mut cached.words {
                for g in &mut w.glyphs {
                    g.start += range.start;
                    g.end += range.start;
                }
            }
            for (r, _) in &mut cached.decoration_spans {
                r.start += range.start;
                r.end += range.start;
            }
            return cached;
        }
        self.span_misses += 1;
        let shaped = ShapeSpan::new(&mut self.fonts, text, attrs, range.clone(), rtl, level, Shaping::Advanced);
        let mut local = shaped.clone();
        for w in &mut local.words {
            for g in &mut w.glyphs {
                g.start -= range.start;
                g.end -= range.start;
            }
        }
        for (r, _) in &mut local.decoration_spans {
            r.start -= range.start;
            r.end -= range.start;
        }
        // Retained payload estimate, separate from process RSS/temporary work.
        let attrs_bytes = |a: &AttrsOwned| {
            std::mem::size_of::<AttrsOwned>()
                + format!("{:?}", a.family_owned).len()
                + a.language.as_ref().map_or(0, |l| l.as_str().len())
                + a.font_features.features.capacity() * std::mem::size_of::<cosmic_text::Feature>()
        };
        let bytes = std::mem::size_of::<SpanKey>()
            + key.text.capacity()
            + key.pre.capacity()
            + key.post.capacity()
            + attrs_bytes(&key.default_attrs)
            + key.attrs.iter().map(|(_, a)| attrs_bytes(a) + std::mem::size_of::<Range<usize>>()).sum::<usize>()
            + std::mem::size_of::<ShapeSpan>()
            + local.words.capacity() * std::mem::size_of::<cosmic_text::ShapeWord>()
            + local
                .words
                .iter()
                .map(|w| w.glyphs.capacity() * std::mem::size_of::<cosmic_text::ShapeGlyph>())
                .sum::<usize>()
            + local.decoration_spans.capacity()
                * std::mem::size_of::<(Range<usize>, cosmic_text::GlyphDecorationData)>();
        const LIMIT: usize = 8 * 1024 * 1024;
        if bytes <= LIMIT {
            if self.span_cache_bytes + bytes > LIMIT || self.span_cache.len() >= 4096 {
                self.span_cache.clear();
                self.span_cache_bytes = 0;
            }
            self.span_cache_bytes += bytes;
            self.span_cache.insert(key, local);
        }
        shaped
    }
    pub fn validate_font(bytes: &[u8]) -> Result<(), &'static str> {
        let font = FontRef::new(bytes).map_err(|_| "invalid font")?;
        if font.outline_glyphs().iter().next().is_none() {
            return Err("no outlines");
        }
        Ok(())
    }
    fn available(&self, face: Face) -> bool {
        self.ids.iter().any(|(_, f)| *f == face)
    }
    pub(crate) fn covers(&self, face: Face, text: &str) -> bool {
        self.available(face)
            && text.chars().filter(|c| !ignorable(*c)).all(|c| {
                let c = c as usize;
                self.coverage[face as usize][c / 64] & (1u64 << (c % 64)) != 0
            })
    }
    pub fn layout(&mut self, req: &Request<'_>) -> Result<Layout, &'static str> {
        self.layout_instrumented(req, |_| {})
    }
    /// Optional headless profiling seam; ordinary layout monomorphizes a no-op.
    pub fn layout_instrumented(
        &mut self,
        req: &Request<'_>,
        mut checkpoint: impl FnMut(&'static str),
    ) -> Result<Layout, &'static str> {
        let (features, boundaries) = validate_request(req)?;
        checkpoint("validate");
        let mut out = Layout {
            source: req.text.into(),
            lines: vec![],
            carets: vec![],
            issues: vec![],
            ink_bounds: None,
            levels: vec![0; req.text.len()],
        };
        let language = req.language.parse::<cosmic_text::harfrust::Language>().map_err(|_| "invalid language")?;
        let mut offset = 0;
        let mut top = 0.;
        // Keep newline bytes in source; only paste_normalize explicitly normalizes CRLF.
        for raw in paragraphs(req.text) {
            let text = paragraphs_trim(raw);
            let base = match req.direction {
                Direction::Auto => None,
                Direction::Ltr => Some(Level::ltr()),
                Direction::Rtl => Some(Level::rtl()),
            };
            let bidi = BidiInfo::new(text, base);
            let level = bidi.paragraphs.first().map_or(base.unwrap_or(Level::ltr()), |p| p.level);
            let rtl = level.is_rtl();
            out.levels[offset..offset + raw.len()].fill(level.number());
            let levels = bidi.paragraphs.first().map_or_else(Vec::new, |p| bidi.reordered_levels(p, p.range.clone()));
            for (i, l) in levels.iter().enumerate() {
                out.levels[offset + i] = l.number();
            }
            checkpoint("bidi");
            let attrs = Attrs::new()
                .family(Family::Name(req.face.family()))
                .language(Some(language.clone()))
                .script(req.script)
                .font_features(features.clone());
            let mut attrs_list = AttrsList::new(&attrs);
            // Resolve an entire word/joining segment, not each Unicode scalar.
            // Explicit typographic style boundaries are allowed to divide that segment.
            let scripts = script_ranges(text);
            let mut cuts: Vec<usize> = std::iter::once(0)
                .chain(
                    text.grapheme_indices(true)
                        .filter(|(_, g)| g.chars().next().is_some_and(char::is_whitespace))
                        .map(|(i, g)| i + g.len()),
                )
                .chain([text.len()])
                .collect();
            for s in &req.styles {
                for edge in [s.range.start, s.range.end] {
                    if edge > offset && edge < offset + text.len() {
                        cuts.push(edge - offset);
                    }
                }
            }
            for run in &req.language_runs {
                for edge in [run.range.start, run.range.end] {
                    if edge > offset && edge < offset + text.len() {
                        cuts.push(edge - offset);
                    }
                }
            }
            cuts.extend(scripts.iter().map(|(r, _)| r.start));
            cuts.sort_unstable();
            cuts.dedup();
            for pair in cuts.windows(2) {
                let range = pair[0]..pair[1];
                let a =
                    self.segment_attrs(req, offset, text, &scripts, &attrs, &features, range.clone(), &mut out.issues);
                attrs_list.add_span(range, &a);
            }
            checkpoint("attributes");
            let attr_spans = attrs_list.spans();
            let mut shape = ShapeLine::new(&mut self.fonts, "", &attrs_list, Shaping::Advanced, 4);
            shape.rtl = rtl;
            // Public ShapeSpan takes explicit resolved level. No hidden source controls.
            let mut start = 0;
            while start < text.len() {
                let l = levels[start];
                let end = text[start..]
                    .char_indices()
                    .skip(1)
                    .map(|(i, _)| start + i)
                    .find(|i| levels[*i] != l)
                    .unwrap_or(text.len());
                shape.spans.push(self.shape_span_cached(text, &attrs_list, &attr_spans, start..end, rtl, l));
                start = end;
            }
            checkpoint("shape_spans");
            let align = if rtl ^ req.end_align { cosmic_text::Align::Right } else { cosmic_text::Align::Left };
            // Word wrapping deliberately overflows unbreakable words rather than
            // cutting a shaped joining cluster without line-edge reshaping.
            let opportunities: Vec<_> = unicode_linebreak::linebreaks(text).collect();
            let layouts = shape.layout_with_breaks(text, &opportunities, req.size, req.width, align, |range| {
                reshape_edge(&mut self.fonts, text, &attrs_list, &levels, rtl, range)
            });
            checkpoint("legal_layout");
            let para_bidi =
                bidi.paragraphs.first().map(|p| (bidi.text, &bidi.original_classes[..], &bidi.levels[..], p.level));
            for (source_range, line) in layouts {
                let (mut line, overflow) = self.place_line(req, offset, rtl, source_range, line, para_bidi)?;
                apply_baseline(&mut line, top, &req.styles);
                if overflow {
                    out.issues.push(Issue::Overflow(out.lines.len()));
                }
                top += (line.ascent + line.descent).max(req.size * 1.4);
                out.lines.push(line);
            }
            if let Some(last) = out.lines.last_mut() {
                last.range.end = offset + raw.len();
            }
            offset += raw.len();
            checkpoint("placements");
        }
        out.ink_bounds = self.bounds_for(&out.lines)?;
        checkpoint("ink_bounds");
        build_carets(&mut out, &boundaries);
        checkpoint("carets");
        Ok(out)
    }
    /// One cut segment's resolved face/size/script/language attributes plus its
    /// coverage issues. Shared by the cold engine and the convergent paragraph cache.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn segment_attrs(
        &self,
        req: &Request<'_>,
        offset: usize,
        text: &str,
        scripts: &[(Range<usize>, Option<cosmic_text::harfrust::Script>)],
        paragraph: &Attrs<'_>,
        features: &FontFeatures,
        range: Range<usize>,
        issues: &mut Vec<Issue>,
    ) -> Attrs<'static> {
        let global = offset + range.start..offset + range.end;
        let style = req.styles.iter().find(|s| s.range.contains(&global.start));
        let requested = style.map_or(req.face, |s| s.face);
        let segment = &text[range.clone()];
        let actual = if self.covers(requested, segment) {
            requested
        } else if self.covers(Face::Plex, segment) {
            Face::Plex
        } else if self.covers(Face::Inter, segment) {
            Face::Inter
        } else {
            requested
        };
        if requested != actual {
            issues.push(Issue::Substituted { range: global.clone(), requested, actual });
        }
        for (i, g) in segment.grapheme_indices(true) {
            if !self.covers(actual, g) {
                issues.push(Issue::UnsupportedCluster(global.start + i..global.start + i + g.len()));
            }
        }
        let size = style.map_or(req.size, |s| s.size);
        let language_run = req.language_runs.iter().find(|run| run.range.contains(&global.start));
        let explicit_script = language_run.map_or(req.script, |run| run.script);
        let script_index = scripts.partition_point(|(r, _)| r.end <= range.start);
        let run_script = explicit_script.or_else(|| scripts.get(script_index).and_then(|(_, script)| *script));
        let mut a = Attrs::new()
            .family(Family::Name(actual.family()))
            .metrics(cosmic_text::Metrics::new(size, size * 1.4))
            .script(run_script)
            .font_features(features.clone());
        a.language = language_run
            .map_or_else(|| paragraph.language.clone(), |run| Some(std::sync::Arc::new(run.language.parse().unwrap())));
        a
    }
    /// Converts one fitted COSMIC line into spike glyphs with line-local UBA L1/L2
    /// cluster order. Glyph `y` stays COSMIC's raw value until [`apply_baseline`].
    /// Returns whether the line overflows the requested width.
    #[allow(clippy::type_complexity)]
    pub(crate) fn place_line(
        &self,
        req: &Request<'_>,
        offset: usize,
        rtl: bool,
        source_range: Range<usize>,
        line: cosmic_text::LayoutLine,
        bidi: Option<(&str, &[unicode_bidi::BidiClass], &[Level], Level)>,
    ) -> Result<(Line, bool), &'static str> {
        let min = source_range.start;
        let max = source_range.end;
        let mut ascent = line.max_ascent.max(req.size * 0.8);
        let mut descent = line.max_descent.max(req.size * 0.2);
        for s in &req.styles {
            if s.range.start < offset + max && s.range.end > offset + min {
                ascent = ascent.max(line.max_ascent + s.baseline_shift);
                descent = descent.max(line.max_descent - s.baseline_shift);
            }
        }
        let mut glyphs = Vec::with_capacity(line.glyphs.len());
        for g in line.glyphs {
            let face = self.ids.iter().find(|(id, _)| *id == g.font_id).ok_or("unknown resolved font")?.1;
            glyphs.push(Glyph {
                id: g.glyph_id,
                face,
                cluster: offset + g.start..offset + g.end,
                level: g.level.number(),
                x: g.x,
                y: g.y,
                advance: g.w,
                offset: [g.x_offset * g.font_size, -g.y_offset * g.font_size],
                size: g.font_size,
            });
        }
        // COSMIC resolves whitespace at paragraph scope. Reapply UBA L1/L2
        // at the chosen line boundary, keeping each shaped cluster intact.
        if let Some((text, classes, levels, level)) = bidi {
            let line_levels = line_levels_from(text, classes, levels, level, min..max);
            reorder_clusters(&mut glyphs, &line_levels, offset + min);
        }
        let overflow = req.width.is_some_and(|w| line.w > w + 0.01);
        Ok((
            Line {
                range: offset + min..offset + max,
                rtl,
                baseline: 0.,
                ascent,
                descent,
                width: line.w,
                empty_caret_x: if rtl ^ req.end_align { req.width.unwrap_or(0.) } else { 0. },
                glyphs,
            },
            overflow,
        ))
    }
    pub(crate) fn bounds_for(&mut self, lines: &[Line]) -> Result<Option<[f32; 4]>, &'static str> {
        let mut bounds_out: Option<[f32; 4]> = None;
        for glyph in lines.iter().flat_map(|l| &l.glyphs) {
            let key = (glyph.face as u8, glyph.id, glyph.size.to_bits());
            let bounds = if let Some(bounds) = self.ink_cache.get(&key) {
                *bounds
            } else {
                let mut local = glyph.clone();
                local.x = 0.;
                local.y = 0.;
                local.offset = [0., 0.];
                let bounds = crate::outlines::glyph_outline(&local)?.ink_bounds();
                if self.ink_cache.len() >= 16384 {
                    self.ink_cache.clear();
                }
                self.ink_cache.insert(key, bounds);
                bounds
            };
            if let Some(mut b) = bounds {
                for i in [0, 2] {
                    b[i] += glyph.x + glyph.offset[0];
                }
                for i in [1, 3] {
                    b[i] += glyph.y + glyph.offset[1];
                }
                bounds_out =
                    Some(bounds_out.map_or(b, |a| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]));
            }
        }
        Ok(bounds_out)
    }
}
fn ignorable(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{fe0f}')
}
fn build_carets(layout: &mut Layout, boundaries: &[usize]) {
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
            return Err("edit merges graphemes: unsupported in spike");
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
/// Use upstream L1 on a line-local snapshot, avoiding its whole-paragraph clone.
/// Paragraph analysis still happens once over the original text.
pub fn local_line_levels(bidi: &BidiInfo<'_>, level: Level, range: Range<usize>) -> Vec<Level> {
    line_levels_from(bidi.text, &bidi.original_classes, &bidi.levels, level, range)
}
/// [`local_line_levels`] over retained paragraph analysis slices.
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

/// Fixed byte-fed fixture fallback, identical on every target.
pub struct PinnedFallback;
impl cosmic_text::Fallback for PinnedFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &["Inter", "IBM Plex Sans Arabic"]
    }
    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }
    fn script_fallback(&self, script: unicode_script::Script, _locale: &str) -> &[&'static str] {
        if script == unicode_script::Script::Arabic {
            &["IBM Plex Sans Arabic"]
        } else {
            &["Inter"]
        }
    }
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
