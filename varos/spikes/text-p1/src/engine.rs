//! Small public-API adapter: explicit UBA base level -> COSMIC ShapeSpan.
//! Language is explicitly reported unsupported, never silently called supported.
use crate::{INTER, PLEX};
use cosmic_text::{Attrs, AttrsList, Family, FontFeatures, FontSystem, Hinting, ShapeLine, ShapeSpan, Shaping, Wrap};
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
pub struct Request<'a> {
    pub text: &'a str,
    pub direction: Direction,
    pub language: &'a str,
    pub size: f32,
    pub width: Option<f32>,
    pub face: Face,
    pub end_align: bool,
    pub styles: Vec<Style>,
    pub features: Vec<Feature>,
}
impl<'a> Request<'a> {
    pub fn new(text: &'a str, size: f32, width: Option<f32>) -> Self {
        Self {
            text,
            size,
            width,
            direction: Direction::Auto,
            language: "und",
            face: Face::Inter,
            end_align: false,
            styles: vec![],
            features: vec![],
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

pub struct Engine {
    fonts: FontSystem,
    ids: Vec<(fontdb::ID, Face)>,
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
        Self { fonts: FontSystem::new_with_locale_and_db("en-US".into(), db), ids, ink_cache: BTreeMap::new() }
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
    fn covers(&self, face: Face, text: &str) -> bool {
        self.available(face)
            && text
                .chars()
                .filter(|c| !ignorable(*c))
                .all(|c| FontRef::new(face.bytes()).unwrap().charmap().map(c).is_some())
    }
    pub fn layout(&mut self, req: &Request<'_>) -> Result<Layout, &'static str> {
        if req.text.len() > 1_048_576 || req.styles.len() > 16_384 {
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
        let mut out = Layout {
            source: req.text.into(),
            lines: vec![],
            carets: vec![],
            issues: vec![],
            ink_bounds: None,
            levels: vec![0; req.text.len()],
        };
        if req.language != "und" {
            out.issues.push(Issue::UnsupportedLanguage(req.language.into()));
        }
        let mut offset = 0;
        let mut top = 0.;
        // Keep newline bytes in source; only paste_normalize explicitly normalizes CRLF.
        for raw in req.text.split_inclusive('\n').chain(if req.text.is_empty() || req.text.ends_with('\n') {
            Some("")
        } else {
            None
        }) {
            let text = raw.strip_suffix('\n').unwrap_or(raw);
            let text = if raw.ends_with("\r\n") { text.strip_suffix('\r').unwrap() } else { text };
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
            let attrs = Attrs::new().family(Family::Name(req.face.family())).font_features(features.clone());
            let mut attrs_list = AttrsList::new(&attrs);
            // Resolve an entire word/joining segment, not each Unicode scalar.
            // Explicit typographic style boundaries are allowed to divide that segment.
            let mut cuts: Vec<usize> = std::iter::once(0)
                .chain(text.char_indices().filter(|(_, c)| c.is_whitespace()).map(|(i, c)| i + c.len_utf8()))
                .chain([text.len()])
                .collect();
            for s in &req.styles {
                for edge in [s.range.start, s.range.end] {
                    if edge > offset && edge < offset + text.len() {
                        cuts.push(edge - offset);
                    }
                }
            }
            cuts.sort_unstable();
            cuts.dedup();
            for pair in cuts.windows(2) {
                let range = pair[0]..pair[1];
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
                    out.issues.push(Issue::Substituted { range: global.clone(), requested, actual });
                }
                for (i, g) in segment.grapheme_indices(true) {
                    if !self.covers(actual, g) {
                        out.issues.push(Issue::UnsupportedCluster(global.start + i..global.start + i + g.len()));
                    }
                }
                let size = style.map_or(req.size, |s| s.size);
                let a = Attrs::new()
                    .family(Family::Name(actual.family()))
                    .metrics(cosmic_text::Metrics::new(size, size * 1.4))
                    .font_features(features.clone());
                attrs_list.add_span(range, &a);
            }
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
                shape.spans.push(ShapeSpan::new(
                    &mut self.fonts,
                    text,
                    &attrs_list,
                    start..end,
                    rtl,
                    l,
                    Shaping::Advanced,
                ));
                start = end;
            }
            let align = if rtl ^ req.end_align { cosmic_text::Align::Right } else { cosmic_text::Align::Left };
            // Word wrapping deliberately overflows unbreakable words rather than
            // cutting a shaped joining cluster without line-edge reshaping.
            let layouts = shape.layout(req.size, req.width, Wrap::Word, Some(align), None, Hinting::Disabled);
            for line in layouts {
                let min = line.glyphs.iter().map(|g| g.start).min().unwrap_or(0);
                let max = line.glyphs.iter().map(|g| g.end).max().unwrap_or(0);
                let mut ascent = line.max_ascent.max(req.size * 0.8);
                let mut descent = line.max_descent.max(req.size * 0.2);
                for s in &req.styles {
                    if s.range.start < offset + max && s.range.end > offset + min {
                        ascent = ascent.max(line.max_ascent + s.baseline_shift);
                        descent = descent.max(line.max_descent - s.baseline_shift);
                    }
                }
                let baseline = top + ascent;
                let mut glyphs = Vec::new();
                for g in line.glyphs {
                    let face = self.ids.iter().find(|(id, _)| *id == g.font_id).ok_or("unknown resolved font")?.1;
                    let shift = req
                        .styles
                        .iter()
                        .find(|s| s.range.contains(&(offset + g.start)))
                        .map_or(0., |s| s.baseline_shift);
                    glyphs.push(Glyph {
                        id: g.glyph_id,
                        face,
                        cluster: offset + g.start..offset + g.end,
                        level: g.level.number(),
                        x: g.x,
                        y: baseline + g.y - shift,
                        advance: g.w,
                        offset: [g.x_offset * g.font_size, -g.y_offset * g.font_size],
                        size: g.font_size,
                    });
                }
                // COSMIC resolves whitespace at paragraph scope. Reapply UBA L1/L2
                // at the chosen line boundary, keeping each shaped cluster intact.
                if let Some(para) = bidi.paragraphs.first() {
                    let line_levels = bidi.reordered_levels(para, min..max);
                    reorder_clusters(&mut glyphs, &line_levels, offset);
                }
                let index = out.lines.len();
                if req.width.is_some_and(|w| line.w > w + 0.01) {
                    out.issues.push(Issue::Overflow(index));
                }
                out.lines.push(Line {
                    range: offset + min..offset + max,
                    rtl,
                    baseline,
                    ascent,
                    descent,
                    width: line.w,
                    empty_caret_x: if rtl ^ req.end_align { req.width.unwrap_or(0.) } else { 0. },
                    glyphs,
                });
                top += (ascent + descent).max(req.size * 1.4);
            }
            offset += raw.len();
        }
        for glyph in out.lines.iter().flat_map(|l| &l.glyphs) {
            let key = (glyph.face as u8, glyph.id, glyph.size.to_bits());
            let bounds = if let Some(bounds) = self.ink_cache.get(&key) {
                *bounds
            } else {
                let mut local = glyph.clone();
                local.x = 0.;
                local.y = 0.;
                local.offset = [0., 0.];
                let bounds = crate::outlines::glyph_outline(&local)?.ink_bounds();
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
                out.ink_bounds = Some(
                    out.ink_bounds.map_or(b, |a| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]),
                );
            }
        }
        build_carets(&mut out);
        Ok(out)
    }
}
fn ignorable(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200c}' | '\u{200d}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{fe0f}')
}
fn build_carets(layout: &mut Layout) {
    let legal: std::collections::BTreeSet<_> =
        layout.source.grapheme_indices(true).map(|(i, _)| i).chain([layout.source.len()]).collect();
    for (line_i, line) in layout.lines.iter().enumerate() {
        let mut clusters: BTreeMap<(usize, usize), (f32, f32, u8)> = BTreeMap::new();
        for g in &line.glyphs {
            let v = clusters.entry((g.cluster.start, g.cluster.end)).or_insert((g.x, g.x + g.advance, g.level));
            v.0 = v.0.min(g.x);
            v.1 = v.1.max(g.x + g.advance);
        }
        for ((start, end), (left, right, level)) in clusters {
            let boundaries: Vec<_> =
                layout.source[start..end].grapheme_indices(true).map(|(i, _)| start + i).chain([end]).collect();
            let count = boundaries.len().saturating_sub(1).max(1);
            for (i, byte) in boundaries.into_iter().enumerate() {
                if !legal.contains(&byte) {
                    continue;
                }
                let t = i as f32 / count as f32;
                let x = if level % 2 == 1 { right - (right - left) * t } else { left + (right - left) * t };
                if i > 0 {
                    layout.carets.push(Caret { byte, affinity: Affinity::Upstream, line: line_i, x });
                }
                if i < count {
                    layout.carets.push(Caret { byte, affinity: Affinity::Downstream, line: line_i, x });
                }
            }
        }
        if line.glyphs.is_empty() {
            for affinity in [Affinity::Upstream, Affinity::Downstream] {
                layout.carets.push(Caret { byte: line.range.start, affinity, line: line_i, x: line.empty_caret_x });
            }
        }
    }
    // Stable finite visual walk. Coincident carets retain byte+affinity identity.
    layout.carets.sort_by(|a, b| {
        a.line.cmp(&b.line).then(a.x.total_cmp(&b.x)).then(a.byte.cmp(&b.byte)).then(a.affinity.cmp(&b.affinity))
    });
    layout.carets.dedup();
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

fn reorder_clusters(glyphs: &mut Vec<Glyph>, levels: &[Level], offset: usize) {
    if glyphs.is_empty() {
        return;
    }
    let mut x = glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
    let mut groups: BTreeMap<usize, Vec<Glyph>> = BTreeMap::new();
    for g in glyphs.drain(..) {
        groups.entry(g.cluster.start).or_default().push(g);
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    let cluster_levels: Vec<_> = groups.iter().map(|(start, _)| levels[start - offset]).collect();
    for index in BidiInfo::reorder_visual(&cluster_levels) {
        let group = &mut groups[index].1;
        let left = group.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        let right = group.iter().map(|g| g.x + g.advance).fold(f32::NEG_INFINITY, f32::max);
        for mut g in group.drain(..) {
            g.x += x - left;
            g.level = cluster_levels[index].number();
            glyphs.push(g);
        }
        x += right - left;
    }
}
