//! Small public-API adapter: explicit UBA base level -> COSMIC ShapeSpan.
//! P1b patched run language/script and paragraph-level legal wrapping.
use crate::{FaceId, FontSet, Script};
use cosmic_text::{Attrs, AttrsList, AttrsOwned, Family, FontFeatures, FontSystem, ShapeLine, ShapeSpan, Shaping};
use skrifa::{FontRef, MetadataProvider};
use std::collections::BTreeMap;
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

pub use crate::engine_support::*;
pub use crate::text_types::*;

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
    ids: Vec<(fontdb::ID, FaceId)>,
    coverage: Vec<Vec<u64>>,
    pub(crate) font_set: FontSet,
    ink_cache: BTreeMap<(FaceId, u16, u32), Option<[f32; 4]>>,
}
impl Engine {
    pub fn new(font_set: FontSet) -> Result<Self, &'static str> {
        let mut db = fontdb::Database::new();
        let mut ids = Vec::new();
        let mut coverage = Vec::new();
        for (index, face) in font_set.faces().iter().enumerate() {
            let before: Vec<_> = db.faces().map(|f| f.id).collect();
            db.load_font_data(face.bytes.to_vec());
            let loaded: Vec<_> = db.faces().filter(|f| !before.contains(&f.id)).collect();
            if loaded.len() != 1
                || loaded[0].weight.0 != face.weight
                || !loaded[0].families.iter().any(|f| f.0 == face.family)
            {
                return Err("font metadata mismatch or collection");
            }
            ids.push((loaded[0].id, FaceId(index)));
            let mut bits = vec![0u64; 0x110000 / 64];
            for (c, _) in FontRef::new(&face.bytes).map_err(|_| "invalid font")?.charmap().mappings() {
                if char::from_u32(c).is_some() {
                    bits[c as usize / 64] |= 1u64 << (c % 64);
                }
            }
            coverage.push(bits);
        }
        db.set_sans_serif_family(font_set.faces()[0].family);
        let fallback = font_set.cosmic_fallback();
        Ok(Self {
            span_cache: std::collections::HashMap::new(),
            span_cache_bytes: 0,
            span_hits: 0,
            span_misses: 0,
            fonts: FontSystem::new_with_locale_and_db_and_fallback("und".into(), db, fallback),
            ids,
            coverage,
            font_set,
            ink_cache: BTreeMap::new(),
        })
    }
    pub fn font_set(&self) -> &FontSet {
        &self.font_set
    }
    pub(crate) fn validate_faces(&self, req: &Request<'_>) -> Result<(), &'static str> {
        if !self.available(req.face) || req.styles.iter().any(|s| !self.available(s.face)) {
            return Err("invalid face id");
        }
        Ok(())
    }
    /// Payload estimate, excluding allocator/BTree overhead and immutable font bytes.
    pub fn cache_bytes(&self) -> usize {
        self.fonts.shape_run_cache.payload_bytes()
            + self.span_cache_bytes
            + self.coverage.iter().map(|bits| bits.capacity() * std::mem::size_of::<u64>()).sum::<usize>()
            + self.ink_cache.len()
                * (std::mem::size_of::<(FaceId, u16, u32)>() + std::mem::size_of::<Option<[f32; 4]>>())
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
    fn available(&self, face: FaceId) -> bool {
        self.ids.iter().any(|(_, f)| *f == face)
    }
    pub(crate) fn covers(&self, face: FaceId, text: &str) -> bool {
        self.available(face)
            && text.chars().filter(|c| !ignorable(*c)).all(|c| {
                let c = c as usize;
                self.coverage[face.0][c / 64] & (1u64 << (c % 64)) != 0
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
        self.layout_options(req, &mut checkpoint, None, None)
    }
    /// Paragraph composition is opt-in; incremental layout retains its original policy.
    pub fn compose(
        &mut self,
        req: &Request<'_>,
        options: &crate::composer::ParagraphOptions,
    ) -> Result<Layout, &'static str> {
        self.compose_with_hyphenation(req, options, None)
    }
    /// Optional dictionary seam; selected hyphens remain virtual, with source byte diagnostics.
    pub fn compose_with_hyphenation(
        &mut self,
        req: &Request<'_>,
        options: &crate::composer::ParagraphOptions,
        provider: Option<&dyn crate::composer::HyphenationProvider>,
    ) -> Result<Layout, &'static str> {
        options.validate()?;
        self.layout_options(req, &mut |_| {}, Some(options), provider)
    }
    fn layout_options(
        &mut self,
        req: &Request<'_>,
        checkpoint: &mut dyn FnMut(&'static str),
        options: Option<&crate::composer::ParagraphOptions>,
        provider: Option<&dyn crate::composer::HyphenationProvider>,
    ) -> Result<Layout, &'static str> {
        self.validate_faces(req)?;
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
                .family(Family::Name(self.font_set.face(req.face).unwrap().family))
                .weight(cosmic_text::Weight(self.font_set.face(req.face).unwrap().weight))
                .language(Some(language.clone()))
                .script(req.script.map(Script::cosmic))
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
            let layouts = if let Some(options) = options {
                crate::composer::compose_shaped(
                    text,
                    req.size,
                    req.width,
                    options,
                    provider,
                    |range, slots, hyphen| {
                        crate::kashida::reshape_virtual(
                            &mut self.fonts,
                            text,
                            &attrs_list,
                            &levels,
                            rtl,
                            range,
                            slots,
                            hyphen,
                        )
                    },
                )?
            } else {
                shape
                    .layout_with_breaks(text, &opportunities, req.size, req.width, align, |range| {
                        reshape_edge(&mut self.fonts, text, &attrs_list, &levels, rtl, range)
                    })
                    .into_iter()
                    .map(|(range, line)| crate::composer::ComposedLine {
                        range,
                        line,
                        inserted: vec![],
                        residual: 0.,
                        hyphen: false,
                    })
                    .collect()
            };
            checkpoint("legal_layout");
            let para_bidi =
                bidi.paragraphs.first().map(|p| (bidi.text, &bidi.original_classes[..], &bidi.levels[..], p.level));
            for composed in layouts {
                let index = out.lines.len();
                if composed.residual.abs() > 0.5 {
                    out.issues.push(Issue::JustificationResidual { line: index, residual: composed.residual });
                }
                if composed.hyphen {
                    out.issues.push(Issue::DiscretionaryHyphen { byte: offset + composed.range.end });
                }
                for byte in composed.inserted {
                    out.issues.push(Issue::KashidaInserted { byte: offset + byte, count: 1 });
                }
                let (mut line, overflow) =
                    self.place_line(req, offset, rtl, composed.range, composed.line, para_bidi)?;
                if let Some(options) = options {
                    line.empty_caret_x = options.alignment.shift(req.width.unwrap_or(0.));
                }
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
        if let Some(options) = options {
            crate::metrics::apply(&self.font_set, &mut out, req.size, options.line_height)?;
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
        let script = segment.chars().find_map(|c| {
            use unicode_script::UnicodeScript;
            let s = c.script();
            (!matches!(s, unicode_script::Script::Common | unicode_script::Script::Inherited))
                .then_some(s.short_name().as_bytes().try_into().unwrap())
        });
        let actual = std::iter::once(requested)
            .chain(self.font_set.fallback_for(script).iter().copied())
            .find(|face| self.covers(*face, segment))
            .unwrap_or(requested);
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
        let explicit_script = language_run.map_or(req.script, |run| run.script).map(Script::cosmic);
        let script_index = scripts.partition_point(|(r, _)| r.end <= range.start);
        let run_script = explicit_script.or_else(|| scripts.get(script_index).and_then(|(_, script)| *script));
        let mut a = Attrs::new()
            .family(Family::Name(self.font_set.face(actual).unwrap().family))
            .weight(cosmic_text::Weight(self.font_set.face(actual).unwrap().weight))
            .metrics(cosmic_text::Metrics::new(size, size * 1.4))
            .script(run_script)
            .font_features(features.clone());
        a.language = language_run
            .map_or_else(|| paragraph.language.clone(), |run| Some(std::sync::Arc::new(run.language.parse().unwrap())));
        a
    }
    /// Converts one fitted COSMIC line into Varos glyphs with line-local UBA L1/L2
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
            let key = (glyph.face, glyph.id, glyph.size.to_bits());
            let bounds = if let Some(bounds) = self.ink_cache.get(&key) {
                *bounds
            } else {
                let mut local = glyph.clone();
                local.x = 0.;
                local.y = 0.;
                local.offset = [0., 0.];
                let bounds = crate::outlines::glyph_outline(&self.font_set, &local)?.ink_bounds();
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
