//! P1c: paragraph-analysis cache and convergent line reuse. Throwaway spike code.
//!
//! A paragraph keeps two layers:
//! 1. **Analysis** for one text revision and non-width settings: full-paragraph
//!    UBA (classes/levels), grapheme boundaries, scripts, UAX #14 opportunities,
//!    cut-segment attributes and legal fitting units holding shaped glyphs. It
//!    survives width/alignment changes.
//! 2. **Fitted lines** for one width/alignment.
//!
//! An edit recomputes the cheap O(n) Unicode analyses exactly (bidi levels may
//! change anywhere), diffs them against the previous revision, and reshapes only
//! the bidi level runs that touch a difference plus HarfRust's five-scalar context.
//! Fitting restarts at the first line whose inputs (including its one-unit
//! look-ahead) touch that window, and stops when a new line starts at a cached
//! line start beyond the window; later lines are reused with shifted offsets.
//! Cached paragraph analyses/lines are never mutated before commit, so cancellation
//! keeps the published cache intact. Shared memo caches (the attribute interner,
//! the engine span/shape-run caches, the font system) are still written; they are
//! content-keyed, so those writes cannot change any result.
//!
//! The fitter is a port of the patched COSMIC `layout_with_breaks`; equality with
//! the unmodified cold engine is enforced by tests, never assumed.
use crate::engine::{
    apply_baseline, caret_order, line_carets, paragraphs_trim, reshape_edge, script_ranges, validate_request, Caret,
    Engine, Issue, Layout, Line, Request,
};
use cosmic_text::{
    Align, Attrs, AttrsList, AttrsOwned, Family, FontFeatures, Hinting, LayoutLine, ShapeGlyph, ShapeLine, ShapeSpan,
    ShapeWord, Shaping, Wrap,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use unicode_bidi::{BidiClass, BidiInfo, Level};
use unicode_linebreak::BreakOpportunity;

type Frag = (Level, bool, ShapeGlyph);
type Script = Option<cosmic_text::harfrust::Script>;

/// Work counters for one `Incremental::layout` call (summed over paragraphs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkCounters {
    /// Paragraph analyses built from scratch / updated from a previous revision /
    /// reused unchanged (width or alignment only).
    pub analysis_full: usize,
    pub analysis_incremental: usize,
    pub analysis_reused: usize,
    /// Bidi level runs passed to shaping (the span cache may still answer) and bytes.
    pub runs_shaped: usize,
    pub bytes_shaped: usize,
    pub segments_rebuilt: usize,
    pub units_rebuilt: usize,
    pub units_total: usize,
    pub lines_refit: usize,
    pub lines_reused: usize,
    /// Line-edge reshaping callbacks (unsafe-to-break boundaries).
    pub edge_reshapes: usize,
    /// Paragraphs handed to the unmodified cold engine (multi-paragraph bidi input).
    pub fallback: usize,
    /// Internal splice inconsistencies recovered by a full analysis rebuild.
    pub internal_rebuilds: usize,
    /// Re-fitted lines whose unsafe-edge greedy look-ahead read past their final end.
    pub lookahead_beyond_last: usize,
    /// Cached prefix lines refused only because their look-ahead touched the window.
    pub lookahead_guarded: usize,
    pub cancel_checks: usize,
    /// Longest uninterrupted stretch between cancellation polls in this call
    /// (entry→first poll and last poll→return included) and where it occurred.
    pub max_poll_gap_ms: f64,
    pub max_poll_gap_at: String,
    /// Stage profile: (from poll, to poll, total ms, max ms, count).
    pub stretches: Vec<(&'static str, &'static str, f64, f64, usize)>,
}

#[derive(Clone, Debug)]
struct Segment {
    range: Range<usize>,
    attrs: Rc<AttrsOwned>,
    issues: Vec<Issue>,
}
impl Segment {
    fn shifted(&self, delta: isize) -> Self {
        let s = |r: &Range<usize>| r.start.wrapping_add_signed(delta)..r.end.wrapping_add_signed(delta);
        Self {
            range: s(&self.range),
            attrs: self.attrs.clone(),
            issues: self
                .issues
                .iter()
                .map(|i| match i {
                    Issue::Substituted { range, requested, actual } => {
                        Issue::Substituted { range: s(range), requested: *requested, actual: *actual }
                    }
                    Issue::UnsupportedCluster(range) => Issue::UnsupportedCluster(s(range)),
                    other => other.clone(),
                })
                .collect(),
        }
    }
}

/// One text revision's analysis. Units hold glyphs with offsets relative to the
/// unit start, so unchanged units are shared (`Rc`) across revisions untouched.
struct Analysis {
    settings: String,
    styles: Vec<crate::engine::Style>,
    language_runs: Vec<crate::engine::LanguageRun>,
    size: f32,
    raw: String,
    trimmed: usize,
    boundaries: Vec<usize>,
    legal: Vec<bool>,
    para: Option<Level>,
    level_number: u8,
    rtl: bool,
    levels: Vec<Level>,
    raw_levels: Vec<Level>,
    classes: Vec<BidiClass>,
    scripts: Vec<(Range<usize>, Script)>,
    segments: Vec<Segment>,
    ends: Vec<(usize, BreakOpportunity)>,
    unit_width: Vec<f32>,
    unit_glyphs: Vec<Rc<Vec<Frag>>>,
    glyph_unsafe: Vec<bool>,
    default_attrs: AttrsOwned,
    template: ShapeLine,
    /// Segment attributes interned across this paragraph's revisions (bounded).
    interner: Rc<RefCell<HashMap<AttrsOwned, Rc<AttrsOwned>>>>,
}

/// A fitted line in line-relative form: clusters/carets relative to its start,
/// glyph `y` is COSMIC's raw value (baseline applied on output).
struct CachedLine {
    len: usize,
    line: Line,
    overflow: bool,
    carets: Vec<Caret>,
}

pub(crate) struct ParagraphState {
    kind: Kind,
}
enum Kind {
    Converged(Converged),
    /// Unusual input (several UBA paragraphs inside one UAX #14 paragraph):
    /// the cold engine result, keyed by its full settings.
    Fallback {
        key: String,
        layout: Layout,
    },
}
struct Converged {
    a: Rc<Analysis>,
    width: Option<f32>,
    end_align: bool,
    lines: Vec<Rc<CachedLine>>,
    line_start: Vec<usize>,
    line_first: Vec<usize>,
    line_last: Vec<usize>,
    /// Furthest unit index each line's fitting READ (greedy look-ahead, refit
    /// probes, unsafe flag at its end); `ends.len()` = reached the paragraph end.
    line_read: Vec<usize>,
}

/// Cooperative cancellation. Every poll is labelled; the stretch between two
/// polls is accumulated per (from, to) label pair, giving both the longest
/// uninterruptible slice and a cheap stage profile (no allocation per poll).
pub(crate) struct Cancel<'a> {
    f: &'a dyn Fn() -> bool,
    pub(crate) checks: usize,
    last: std::time::Instant,
    last_label: &'static str,
    /// (from, to, total ms, max ms, count)
    pub(crate) stretches: Vec<(&'static str, &'static str, f64, f64, usize)>,
}
impl<'a> Cancel<'a> {
    pub(crate) fn new(f: &'a dyn Fn() -> bool) -> Self {
        Self { f, checks: 0, last: std::time::Instant::now(), last_label: "entry", stretches: Vec::new() }
    }
    fn stretch(&mut self, to: &'static str) {
        let now = std::time::Instant::now();
        let ms = now.duration_since(self.last).as_secs_f64() * 1000.;
        let from = self.last_label;
        match self.stretches.iter_mut().find(|s| std::ptr::eq(s.0, from) && std::ptr::eq(s.1, to)) {
            Some(s) => {
                s.2 += ms;
                s.3 = s.3.max(ms);
                s.4 += 1;
            }
            None => self.stretches.push((from, to, ms, ms, 1)),
        }
        self.last = now;
        self.last_label = to;
    }
    pub(crate) fn check(&mut self, label: &'static str) -> Result<(), &'static str> {
        self.checks += 1;
        self.stretch(label);
        if (self.f)() {
            Err("cancelled")
        } else {
            Ok(())
        }
    }
    /// Close the final stretch (last poll -> return).
    pub(crate) fn finish(&mut self) {
        self.stretch("return");
    }
    pub(crate) fn max_gap(&self) -> (f64, String) {
        self.stretches
            .iter()
            .max_by(|a, b| a.3.total_cmp(&b.3))
            .map_or((0., String::new()), |s| (s.3, format!("{} -> {}", s.0, s.1)))
    }
}

fn settings_key(req: &Request<'_>) -> String {
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
        req.direction,
        req.language,
        req.script,
        req.size.to_bits(),
        req.face,
        req.features
    )
}

impl ParagraphState {
    /// Retained payload estimate (excludes allocator overhead; shared `Rc` data counted once per state).
    pub(crate) fn bytes(&self) -> usize {
        match &self.kind {
            Kind::Fallback { key, layout } => crate::incremental::layout_bytes(layout) + key.capacity(),
            Kind::Converged(c) => {
                let a = &c.a;
                let frag = std::mem::size_of::<Frag>();
                std::mem::size_of::<Analysis>()
                    + a.raw.capacity()
                    + a.settings.capacity()
                    + a.boundaries.capacity() * std::mem::size_of::<usize>()
                    + a.legal.capacity()
                    + a.levels.capacity()
                    + a.raw_levels.capacity()
                    + a.classes.capacity() * std::mem::size_of::<BidiClass>()
                    + a.scripts.capacity() * std::mem::size_of::<(Range<usize>, Script)>()
                    + a.segments.capacity() * std::mem::size_of::<Segment>()
                    + a.interner.borrow().len() * 2 * std::mem::size_of::<AttrsOwned>()
                    + a.ends.capacity() * std::mem::size_of::<(usize, BreakOpportunity)>()
                    + a.unit_width.capacity() * 4
                    + a.unit_glyphs
                        .iter()
                        .map(|u| u.capacity() * frag + 2 * std::mem::size_of::<usize>())
                        .sum::<usize>()
                    + a.glyph_unsafe.capacity()
                    + c.lines
                        .iter()
                        .map(|l| {
                            std::mem::size_of::<CachedLine>()
                                + l.line.glyphs.capacity() * std::mem::size_of::<crate::engine::Glyph>()
                                + l.carets.capacity() * std::mem::size_of::<Caret>()
                        })
                        .sum::<usize>()
                    + c.line_start.capacity() * 3 * std::mem::size_of::<usize>()
            }
        }
    }

    /// Lay out one UAX #14 paragraph (`req.text` = raw paragraph incl. its break),
    /// reusing `old` where its inputs are provably unchanged. Equivalent to the cold
    /// `Engine::layout(req)` except that the trailing empty sub-paragraph of a raw
    /// paragraph ending in a break is omitted (the document wrapper drops it).
    pub(crate) fn layout(
        engine: &mut Engine,
        old: Option<&ParagraphState>,
        req: &Request<'_>,
        cancel: &mut Cancel<'_>,
        work: &mut WorkCounters,
    ) -> Result<ParagraphState, &'static str> {
        let (features, boundaries) = validate_request(req)?;
        let language = req.language.parse::<cosmic_text::harfrust::Language>().map_err(|_| "invalid language")?;
        cancel.check("para_validate")?;
        let settings = settings_key(req);
        let old = match old.map(|o| &o.kind) {
            Some(Kind::Converged(c)) if c.a.settings == settings => Some(c),
            _ => None,
        };
        // Width/alignment-only change (or nothing): reuse the analysis as is.
        let same_analysis = old.filter(|o| {
            o.a.raw == req.text
                && styles_eq(&o.a.styles, &req.styles)
                && runs_eq(&o.a.language_runs, &req.language_runs)
        });
        let (a, window, delta) = if let Some(o) = same_analysis {
            work.analysis_reused += 1;
            (o.a.clone(), None, 0)
        } else {
            let analysis = match analyze(
                engine,
                req,
                features.clone(),
                boundaries.clone(),
                language.clone(),
                settings.clone(),
                old.map(|o| &*o.a),
                cancel,
                work,
            ) {
                // An inconsistent incremental splice must never fail the edit:
                // rebuild this paragraph's analysis from scratch instead.
                Err(e) if e.starts_with("internal:") => {
                    work.internal_rebuilds += 1;
                    work.analysis_incremental = work.analysis_incremental.saturating_sub(1);
                    analyze(engine, req, features, boundaries, language, settings, None, cancel, work)?
                }
                other => other?,
            };
            match analysis {
                Some((a, window, delta)) => (Rc::new(a), window, delta),
                None => {
                    work.fallback += 1;
                    let layout = engine.layout(req)?;
                    let key = format!("{}/{:?}/{:?}", settings_key(req), req.width.map(f32::to_bits), req.end_align);
                    let key = format!("{key}/{:?}/{:?}", req.styles, req.language_runs);
                    return Ok(ParagraphState { kind: Kind::Fallback { key, layout } });
                }
            }
        };
        work.units_total += a.ends.len();
        // Lines: reuse only with identical width/alignment.
        let old_lines =
            old.filter(|o| o.width.map(f32::to_bits) == req.width.map(f32::to_bits) && o.end_align == req.end_align);
        let converged = fit_lines(engine, &a, req, old_lines, window, delta, cancel, work)?;
        Ok(ParagraphState { kind: Kind::Converged(converged) })
    }

    /// Paragraph-local layout identical to the cold engine's (ink bounds omitted:
    /// the document wrapper computes them over all lines).
    pub(crate) fn output(&self, cancel: &mut Cancel<'_>) -> Result<Layout, &'static str> {
        let c = match &self.kind {
            Kind::Fallback { layout, .. } => return Ok(layout.clone()),
            Kind::Converged(c) => c,
        };
        let a = &c.a;
        let mut out = Layout {
            source: a.raw.clone(),
            lines: Vec::with_capacity(c.lines.len()),
            carets: Vec::with_capacity(c.lines.iter().map(|l| l.carets.len()).sum()),
            issues: a.segments.iter().flat_map(|s| s.issues.iter().cloned()).collect(),
            levels: vec![a.level_number; a.raw.len()],
            ink_bounds: None,
        };
        for (i, l) in a.levels.iter().enumerate() {
            out.levels[i] = l.number();
        }
        cancel.check("output_start")?;
        let mut top = 0.;
        for (k, cl) in c.lines.iter().enumerate() {
            if k % 512 == 511 {
                cancel.check("output_lines")?;
            }
            let start = c.line_start[k];
            let mut line = Line {
                range: start..start + cl.len,
                rtl: cl.line.rtl,
                baseline: 0.,
                ascent: cl.line.ascent,
                descent: cl.line.descent,
                width: cl.line.width,
                empty_caret_x: cl.line.empty_caret_x,
                glyphs: cl.line.glyphs.clone(),
            };
            for g in &mut line.glyphs {
                g.cluster.start += start;
                g.cluster.end += start;
            }
            apply_baseline(&mut line, top, &a.styles);
            if cl.overflow {
                out.issues.push(Issue::Overflow(k));
            }
            top += (line.ascent + line.descent).max(a.size * 1.4);
            out.lines.push(line);
            out.carets.extend(cl.carets.iter().map(|caret| Caret {
                byte: caret.byte + start,
                line: k,
                ..caret.clone()
            }));
        }
        if let Some(last) = out.lines.last_mut() {
            last.range.end = a.raw.len();
        }
        Ok(out)
    }
}

fn styles_eq(a: &[crate::engine::Style], b: &[crate::engine::Style]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.range == b.range
                && a.face == b.face
                && a.size.to_bits() == b.size.to_bits()
                && a.baseline_shift.to_bits() == b.baseline_shift.to_bits()
                && a.paint == b.paint
        })
}
fn runs_eq(a: &[crate::engine::LanguageRun], b: &[crate::engine::LanguageRun]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| a.range == b.range && a.language == b.language && a.script == b.script)
}

/// Map an old-revision byte to the new revision; `None` inside the edited span.
fn map_pos(p: usize, prefix: usize, old_suffix_start: usize, delta: isize) -> Option<usize> {
    if p <= prefix {
        Some(p)
    } else if p >= old_suffix_start {
        Some(p.wrapping_add_signed(delta))
    } else {
        None
    }
}

/// Style and language ranges must map consistently through the edit; otherwise the
/// paragraph is analysed from scratch (still correct, just not incremental).
fn ranges_map(
    old: &[(Range<usize>, String)],
    new: &[(Range<usize>, String)],
    prefix: usize,
    old_suffix_start: usize,
    delta: isize,
) -> bool {
    old.len() == new.len()
        && old.iter().zip(new).all(|((r, v), (nr, nv))| {
            v == nv
                && map_pos(r.start, prefix, old_suffix_start, delta) == Some(nr.start)
                && map_pos(r.end, prefix, old_suffix_start, delta) == Some(nr.end)
        })
}

struct Diff {
    lo: usize,
    hi: usize,
}
impl Diff {
    fn mark(&mut self, lo: usize, hi: usize) {
        self.lo = self.lo.min(lo);
        self.hi = self.hi.max(hi);
    }
}

/// Compare a per-byte array over the unchanged prefix and suffix.
fn diff_bytes<T: PartialEq>(d: &mut Diff, new: &[T], old: &[T], prefix: usize, suffix: usize) {
    if let Some(i) = new[..prefix].iter().zip(&old[..prefix]).position(|(a, b)| a != b) {
        d.mark(i, i + 1);
    }
    let (n, o) = (new.len(), old.len());
    if let Some(k) = new[n - suffix..].iter().rev().zip(old[o - suffix..].iter().rev()).position(|(a, b)| a != b) {
        d.mark(n - k - 1, n - k);
    }
}

/// Compare sorted position lists: entries before `prefix` directly, entries after
/// `suffix_from` (new coordinates) through `delta`.
fn diff_list<T: PartialEq + Copy>(
    d: &mut Diff,
    new: &[(usize, T)],
    old: &[(usize, T)],
    prefix: usize,
    suffix_from: usize,
    delta: isize,
) {
    let mut i = 0;
    while i < new.len() && i < old.len() && new[i].0 < prefix && new[i] == old[i] {
        i += 1;
    }
    let lo = [new.get(i), old.get(i)].into_iter().flatten().map(|e| e.0).filter(|p| *p < prefix).min();
    if let Some(p) = lo {
        d.mark(p, p + 1);
    }
    let (n, o) = (new.len(), old.len());
    let mut k = 0;
    while k < n
        && k < o
        && new[n - 1 - k].0 > suffix_from
        && new[n - 1 - k].0 == old[o - 1 - k].0.saturating_add_signed(delta)
        && new[n - 1 - k].1 == old[o - 1 - k].1
    {
        k += 1;
    }
    let hi = [(k < n).then(|| new[n - 1 - k].0), (k < o).then(|| old[o - 1 - k].0.saturating_add_signed(delta))]
        .into_iter()
        .flatten()
        .filter(|p| *p > suffix_from)
        .max();
    if let Some(p) = hi {
        d.mark(p.saturating_sub(1), p + 1);
    }
}

/// Per-byte script values over prefix/suffix (two-pointer walk over ranges).
fn diff_scripts(
    d: &mut Diff,
    new: &[(Range<usize>, Script)],
    old: &[(Range<usize>, Script)],
    prefix: usize,
    new_suffix_start: usize,
    delta: isize,
) {
    let (mut pos, mut i, mut j) = (0, 0, 0);
    while pos < prefix && i < new.len() && j < old.len() {
        if new[i].1 != old[j].1 {
            d.mark(pos, pos + 1);
            break;
        }
        let end = new[i].0.end.min(old[j].0.end).min(prefix);
        pos = end;
        if new[i].0.end == end {
            i += 1;
        }
        if old[j].0.end == end {
            j += 1;
        }
    }
    let n_len = new.last().map_or(0, |r| r.0.end);
    let (mut pos, mut i, mut j) = (n_len, new.len(), old.len());
    while pos > new_suffix_start && i > 0 && j > 0 {
        let (a, b) = (&new[i - 1], &old[j - 1]);
        if a.1 != b.1 {
            d.mark(pos - 1, pos);
            break;
        }
        let start = a.0.start.max(b.0.start.saturating_add_signed(delta)).max(new_suffix_start);
        pos = start;
        if a.0.start == start {
            i -= 1;
        }
        if b.0.start.saturating_add_signed(delta) == start {
            j -= 1;
        }
    }
    // Range starts are segment cuts as well (two ranges may share a value).
    let ns: Vec<_> = new.iter().map(|r| (r.0.start, ())).collect();
    let os: Vec<_> = old.iter().map(|r| (r.0.start, ())).collect();
    diff_list(d, &ns, &os, prefix, new_suffix_start, delta);
}

fn back_chars(text: &str, pos: usize, n: usize) -> usize {
    text[..pos].char_indices().rev().nth(n.saturating_sub(1)).map_or(0, |(i, _)| i)
}
fn forward_chars(text: &str, pos: usize, n: usize) -> usize {
    text[pos..].char_indices().nth(n).map_or(text.len(), |(i, _)| pos + i)
}

/// Build (old = None) or update the analysis. Returns the rebuilt byte window in
/// new coordinates (`None` = nothing differs) and the byte delta of the edit.
/// `Ok(None)` requests the cold-engine fallback.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn analyze(
    engine: &mut Engine,
    req: &Request<'_>,
    features: FontFeatures,
    boundaries: Vec<usize>,
    language: cosmic_text::harfrust::Language,
    settings: String,
    old: Option<&Analysis>,
    cancel: &mut Cancel<'_>,
    work: &mut WorkCounters,
) -> Result<Option<(Analysis, Option<Range<usize>>, isize)>, &'static str> {
    let raw = req.text;
    let text = paragraphs_trim(raw);
    let tn = text.len();
    let base = match req.direction {
        crate::engine::Direction::Auto => None,
        crate::engine::Direction::Ltr => Some(Level::ltr()),
        crate::engine::Direction::Rtl => Some(Level::rtl()),
    };
    let bidi = BidiInfo::new(text, base);
    cancel.check("bidi")?;
    if bidi.paragraphs.len() > 1 {
        return Ok(None);
    }
    let para = bidi.paragraphs.first().map(|p| p.level);
    let level = para.unwrap_or(base.unwrap_or(Level::ltr()));
    let rtl = level.is_rtl();
    let levels = bidi.paragraphs.first().map_or_else(Vec::new, |p| bidi.reordered_levels(p, p.range.clone()));
    let BidiInfo { original_classes: classes, levels: raw_levels, .. } = bidi;
    cancel.check("bidi_levels")?;
    let scripts = script_ranges(text);
    cancel.check("scripts")?;
    let mut ends: Vec<_> = unicode_linebreak::linebreaks(text).collect();
    if ends.last().map(|x| x.0) != Some(tn) {
        ends.push((tn, BreakOpportunity::Mandatory));
    }
    let mut legal = vec![false; raw.len() + 1];
    for &b in &boundaries {
        legal[b] = true;
    }
    cancel.check("linebreaks")?;
    let attrs = Attrs::new()
        .family(Family::Name(engine.font_set.face(req.face).ok_or("invalid face id")?.family))
        .weight(cosmic_text::Weight(engine.font_set.face(req.face).unwrap().weight))
        .language(Some(language))
        .script(req.script.map(crate::Script::cosmic))
        .font_features(features.clone());
    let default_attrs = AttrsOwned::new(&attrs);
    let mut template = ShapeLine::new(&mut engine.fonts, "", &AttrsList::new(&attrs), Shaping::Advanced, 4);
    template.rtl = rtl;

    // Dirty window detection against the previous revision.
    let mut window = Some(0..tn);
    let mut delta = 0isize;
    let incremental = old.filter(|o| {
        o.trimmed > 0 && tn > 0 && o.para.is_some() == para.is_some() && o.rtl == rtl && o.levels.len() == o.trimmed
    });
    if let Some(o) = incremental {
        let ot = &o.raw[..o.trimmed];
        let mut pt = ot.bytes().zip(text.bytes()).take_while(|(a, b)| a == b).count();
        while !text.is_char_boundary(pt) || !ot.is_char_boundary(pt) {
            pt -= 1;
        }
        let max_s = ot.len().min(tn) - pt;
        let mut st = ot.bytes().rev().zip(text.bytes().rev()).take(max_s).take_while(|(a, b)| a == b).count();
        while !text.is_char_boundary(tn - st) || !ot.is_char_boundary(ot.len() - st) {
            st -= 1;
        }
        delta = tn as isize - o.trimmed as isize;
        let old_suffix_start = o.trimmed - st;
        let new_suffix_start = tn - st;
        // Style/language ranges: consistent mapping required, else rebuild all.
        let sv = |s: &[crate::engine::Style]| -> Vec<(Range<usize>, String)> {
            s.iter()
                .map(|s| (s.range.clone(), format!("{:?}/{}/{}", s.face, s.size.to_bits(), s.baseline_shift.to_bits())))
                .collect()
        };
        let rv = |s: &[crate::engine::LanguageRun]| -> Vec<(Range<usize>, String)> {
            s.iter().map(|s| (s.range.clone(), format!("{}/{:?}", s.language, s.script))).collect()
        };
        let mapped = ranges_map(&sv(&o.styles), &sv(&req.styles), pt, old_suffix_start, delta)
            && ranges_map(&rv(&o.language_runs), &rv(&req.language_runs), pt, old_suffix_start, delta);
        if mapped {
            let mut d =
                Diff { lo: if pt < new_suffix_start || o.trimmed != tn { pt } else { tn }, hi: new_suffix_start };
            if d.lo > d.hi {
                d.hi = d.lo;
            }
            diff_bytes(&mut d, &levels, &o.levels, pt, st);
            diff_bytes(&mut d, &raw_levels, &o.raw_levels, pt, st);
            diff_bytes(&mut d, &classes, &o.classes, pt, st);
            // Grapheme boundaries (raw coordinates, clamped to the trimmed text).
            let nb: Vec<_> = boundaries.iter().map(|b| (*b, ())).collect();
            let ob: Vec<_> = o.boundaries.iter().map(|b| (*b, ())).collect();
            diff_list(&mut d, &nb, &ob, pt, new_suffix_start, delta);
            diff_list(&mut d, &ends, &o.ends, pt, new_suffix_start, delta);
            diff_scripts(&mut d, &scripts, &o.scripts, pt, new_suffix_start, delta);
            let (lo, hi) = (d.lo.min(tn), d.hi.min(tn));
            if lo >= hi && o.trimmed == tn {
                window = None;
            } else {
                // Char-align, then add HarfRust's five-scalar shaping context.
                let mut lo = lo.min(hi);
                while !text.is_char_boundary(lo) {
                    lo -= 1;
                }
                let mut hi = hi;
                while !text.is_char_boundary(hi) {
                    hi += 1;
                }
                let mut ws = back_chars(text, lo, 6);
                let mut we = forward_chars(text, hi, 6);
                // Close over segment, level-run and unit partitions.
                loop {
                    let (s0, e0) = (ws, we);
                    // Segments (cuts) before/after the window are the old ones.
                    let k = o.segments.partition_point(|s| s.range.end <= ws);
                    if let Some(s) = o.segments.get(k) {
                        ws = ws.min(s.range.start);
                    }
                    if we < tn {
                        let op = we.wrapping_add_signed(-delta);
                        let k = o.segments.partition_point(|s| s.range.end <= op);
                        if let Some(s) = o.segments.get(k) {
                            if s.range.start < op {
                                we = we.max(s.range.end.wrapping_add_signed(delta));
                            }
                        }
                    }
                    // Level runs (new levels).
                    if ws < tn {
                        while ws > 0 && levels[ws - 1] == levels[ws] {
                            ws -= 1;
                        }
                    }
                    if we > 0 {
                        while we < tn && levels[we] == levels[we - 1] {
                            we += 1;
                        }
                    }
                    // Fitting units (new opportunities).
                    let i = ends.partition_point(|e| e.0 <= ws);
                    ws = if i == 0 { 0 } else { ends[i - 1].0 };
                    if we > 0 {
                        let j = ends.partition_point(|e| e.0 < we);
                        we = ends[j].0;
                    }
                    while !text.is_char_boundary(ws) {
                        ws -= 1;
                    }
                    while !text.is_char_boundary(we) {
                        we += 1;
                    }
                    if (ws, we) == (s0, e0) {
                        break;
                    }
                }
                // The window must sit on old segment and unit boundaries.
                let old_we = we.wrapping_add_signed(-delta);
                let seg_ok = o.segments.binary_search_by(|s| s.range.start.cmp(&ws)).is_ok()
                    && (we == tn || o.segments.binary_search_by(|s| s.range.start.cmp(&old_we)).is_ok());
                let unit_ok = (ws == 0 || o.ends.binary_search_by(|e| e.0.cmp(&ws)).is_ok())
                    && (we == tn || o.ends.binary_search_by(|e| e.0.cmp(&old_we)).is_ok());
                window = if seg_ok && unit_ok && ws <= pt.min(lo) && we >= hi { Some(ws..we) } else { Some(0..tn) };
            }
        }
    }
    let full = window == Some(0..tn) || old.is_none() || incremental.is_none();
    if full {
        window = Some(0..tn);
        work.analysis_full += 1;
    } else {
        work.analysis_incremental += 1;
    }
    cancel.check("diff_window")?;

    let mut a = Analysis {
        settings,
        styles: req.styles.clone(),
        language_runs: req.language_runs.clone(),
        size: req.size,
        raw: raw.to_owned(),
        trimmed: tn,
        boundaries,
        legal,
        para,
        level_number: level.number(),
        rtl,
        levels,
        raw_levels,
        classes,
        scripts,
        segments: vec![],
        ends,
        unit_width: vec![],
        unit_glyphs: vec![],
        glyph_unsafe: vec![],
        default_attrs,
        template,
        interner: old.map_or_else(Default::default, |o| o.interner.clone()),
    };
    let Some(w) = window.clone() else {
        let o = old.expect("window None only with an old analysis");
        a.segments = o.segments.clone();
        a.unit_width = o.unit_width.clone();
        a.unit_glyphs = o.unit_glyphs.clone();
        a.glyph_unsafe = o.glyph_unsafe.clone();
        return Ok(Some((a, None, delta)));
    };

    // Segments in the window.
    let mut cuts = vec![w.start];
    for (i, g) in text[w.clone()].grapheme_indices_ws() {
        if g {
            cuts.push(w.start + i);
        }
    }
    for edge in req
        .styles
        .iter()
        .flat_map(|s| [s.range.start, s.range.end])
        .chain(req.language_runs.iter().flat_map(|r| [r.range.start, r.range.end]))
    {
        if edge > 0 && edge < tn && edge > w.start && edge < w.end {
            cuts.push(edge);
        }
    }
    cuts.extend(a.scripts.iter().map(|(r, _)| r.start).filter(|s| *s >= w.start && *s < w.end));
    cuts.push(w.end);
    cuts.sort_unstable();
    cuts.dedup();
    cancel.check("cuts")?;
    let mut new_segments = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let range = pair[0]..pair[1];
        let mut issues = vec![];
        let seg = engine.segment_attrs(req, 0, text, &a.scripts, &attrs, &features, range.clone(), &mut issues);
        let owned = AttrsOwned::new(&seg);
        let mut interner = a.interner.borrow_mut();
        if interner.len() >= 1024 {
            interner.clear();
        }
        let attrs = interner.entry(owned.clone()).or_insert_with(|| Rc::new(owned)).clone();
        drop(interner);
        new_segments.push(Segment { range, attrs, issues });
        if new_segments.len() % 1024 == 0 {
            cancel.check("segments")?;
        }
    }
    work.segments_rebuilt += new_segments.len();
    cancel.check("segments")?;
    // Window attributes list: paragraph defaults plus the window's segments.
    let mut window_attrs = AttrsList::new(&attrs);
    for (i, s) in new_segments.iter().enumerate() {
        window_attrs.add_span(s.range.clone(), &s.attrs.as_attrs());
        if i % 1024 == 1023 {
            cancel.check("window_attrs")?;
        }
    }
    let spans = window_attrs.spans();
    cancel.check("window_spans")?;
    // Units in the window.
    let (u0, u1) = if w == (0..tn) {
        (0, a.ends.len())
    } else {
        (a.ends.partition_point(|e| e.0 <= w.start), a.ends.partition_point(|e| e.0 < w.end) + 1)
    };
    let unit_start = |ends: &[(usize, BreakOpportunity)], i: usize| if i == 0 { 0 } else { ends[i - 1].0 };
    let mut glyphs: Vec<Vec<Frag>> = vec![Vec::new(); u1 - u0];
    let mut widths = vec![0f32; u1 - u0];
    let mut unsafe_window = vec![false; w.len()];
    let mut start = w.start;
    let mut shaped = 0;
    while start < w.end {
        let l = a.levels[start];
        let end =
            text[start..].char_indices().skip(1).map(|(i, _)| start + i).find(|i| a.levels[*i] != l).unwrap_or(tn);
        if end > w.end {
            return Err("internal: level run crosses the convergent window");
        }
        let span = engine.shape_span_cached(text, &window_attrs, &spans, start..end, rtl, l);
        work.runs_shaped += 1;
        work.bytes_shaped += end - start;
        for word in &span.words {
            for g in &word.glyphs {
                let u = a.ends.partition_point(|e| e.0 <= g.start);
                let base = unit_start(&a.ends, u);
                widths[u - u0] += g.width(req.size);
                unsafe_window[g.start - w.start] |= g.unsafe_to_break;
                unsafe_window[g.start + 1 - w.start..g.end - w.start].fill(true);
                let mut g = g.clone();
                g.start -= base;
                g.end -= base;
                glyphs[u - u0].push((span.level, word.blank, g));
            }
        }
        start = end;
        shaped += 1;
        if shaped % 64 == 0 {
            cancel.check("shaping")?;
        }
    }
    work.units_rebuilt += u1 - u0;
    let new_units: Vec<_> = glyphs
        .into_iter()
        .map(|mut g| {
            g.shrink_to_fit();
            Rc::new(g)
        })
        .collect();
    if let (Some(o), false) = (old, full) {
        #[cfg(test)]
        if FORCE_INTERNAL.with(std::cell::Cell::get) {
            return Err("internal: forced by test");
        }
        let old_we = w.end.wrapping_add_signed(-delta);
        let k0 = o.segments.partition_point(|s| s.range.end <= w.start);
        let k1 = o.segments.partition_point(|s| s.range.start < old_we);
        a.segments = o.segments[..k0].to_vec();
        a.segments.extend(new_segments);
        a.segments.extend(o.segments[k1..].iter().map(|s| s.shifted(delta)));
        let ou1 = o.ends.partition_point(|e| e.0 <= old_we);
        let ou1 = if w.end == tn { o.ends.len() } else { ou1 };
        a.unit_width = o.unit_width[..u0].to_vec();
        a.unit_width.extend(widths);
        a.unit_width.extend_from_slice(&o.unit_width[ou1..]);
        a.unit_glyphs = o.unit_glyphs[..u0].to_vec();
        a.unit_glyphs.extend(new_units);
        a.unit_glyphs.extend_from_slice(&o.unit_glyphs[ou1..]);
        a.glyph_unsafe = o.glyph_unsafe[..w.start].to_vec();
        a.glyph_unsafe.extend(unsafe_window);
        a.glyph_unsafe.extend_from_slice(&o.glyph_unsafe[old_we..]);
        if a.unit_width.len() != a.ends.len() || a.glyph_unsafe.len() != tn + 1 {
            return Err("internal: convergent splice mismatch");
        }
    } else {
        a.segments = new_segments;
        a.unit_width = widths;
        a.unit_glyphs = new_units;
        a.glyph_unsafe = unsafe_window;
        a.glyph_unsafe.push(false);
    }
    Ok(Some((a, Some(w), delta)))
}

/// Whitespace-led grapheme ends within a slice (the engine's segment cuts).
trait WsGraphemes {
    fn grapheme_indices_ws(&self) -> Vec<(usize, bool)>;
}
impl WsGraphemes for str {
    fn grapheme_indices_ws(&self) -> Vec<(usize, bool)> {
        use unicode_segmentation::UnicodeSegmentation;
        self.grapheme_indices(true)
            .map(|(i, g)| (i + g.len(), g.chars().next().is_some_and(char::is_whitespace)))
            .collect()
    }
}

fn unsafe_at(a: &Analysis, text: &str, b: usize) -> bool {
    b != 0
        && b != text.len()
        && (a.glyph_unsafe[b] || text[..b].chars().next_back().is_some_and(|c| !c.is_whitespace()))
}

fn unit_fragments(a: &Analysis, first: usize, last: usize) -> Vec<Frag> {
    let mut out = Vec::with_capacity((first..last).map(|i| a.unit_glyphs[i].len()).sum());
    for i in first..last {
        let base = if i == 0 { 0 } else { a.ends[i - 1].0 };
        for (l, b, g) in a.unit_glyphs[i].iter() {
            let mut g = g.clone();
            g.start += base;
            g.end += base;
            out.push((*l, *b, g));
        }
    }
    out
}

/// Attributes for a line-edge reshape, from the retained segments.
fn line_attrs(a: &Analysis, range: Range<usize>) -> AttrsList {
    let mut list = AttrsList::new(&a.default_attrs.as_attrs());
    let k = a.segments.partition_point(|s| s.range.end <= range.start);
    for s in &a.segments[k..] {
        if s.range.start >= range.end {
            break;
        }
        list.add_span(s.range.clone(), &s.attrs.as_attrs());
    }
    list
}

/// Port of the patched COSMIC `place_legal_fragments`.
fn place_fragments(
    template: &ShapeLine,
    mut fragments: Vec<Frag>,
    font_size: f32,
    width: Option<f32>,
    align: Align,
) -> LayoutLine {
    fragments.sort_by_key(|(_, _, g)| g.start);
    let mut line = template.clone();
    line.spans.clear();
    for (level, blank, g) in fragments {
        if line.spans.last().is_none_or(|s| s.level != level) {
            line.spans.push(ShapeSpan { level, words: Vec::new(), decoration_spans: Vec::new() });
        }
        let span = line.spans.last_mut().unwrap();
        if span.words.last().is_none_or(|w| w.blank != blank) {
            span.words.push(ShapeWord { blank, glyphs: Vec::new() });
        }
        span.words.last_mut().unwrap().glyphs.push(g);
    }
    let rtl = line.rtl;
    for span in &mut line.spans {
        if rtl != span.level.is_rtl() {
            for word in &mut span.words {
                word.glyphs.sort_by_key(|g| std::cmp::Reverse(g.start));
            }
            span.words.reverse();
        }
    }
    let mut laid = line.layout(font_size, None, Wrap::Word, Some(Align::Left), None, Hinting::Disabled);
    let mut item = laid.remove(0);
    let shift = match align {
        Align::Right => width.unwrap_or(item.w) - item.w,
        Align::Center => (width.unwrap_or(item.w) - item.w) * 0.5,
        _ => 0.,
    };
    for g in &mut item.glyphs {
        g.x += shift;
    }
    item
}

#[allow(clippy::too_many_arguments)]
fn fit_lines(
    engine: &mut Engine,
    a: &Rc<Analysis>,
    req: &Request<'_>,
    old: Option<&Converged>,
    window: Option<Range<usize>>,
    delta: isize,
    cancel: &mut Cancel<'_>,
    work: &mut WorkCounters,
) -> Result<Converged, &'static str> {
    let text = &a.raw[..a.trimmed];
    let n = a.ends.len();
    let tn = text.len();
    let mut out = Converged {
        a: a.clone(),
        width: req.width,
        end_align: req.end_align,
        lines: vec![],
        line_start: vec![],
        line_first: vec![],
        line_last: vec![],
        line_read: vec![],
    };
    // Nothing differs and lines match: reuse everything.
    let (window, old) = match (window, old) {
        (None, Some(o)) => {
            out.lines = o.lines.clone();
            out.line_start = o.line_start.clone();
            out.line_first = o.line_first.clone();
            out.line_last = o.line_last.clone();
            out.line_read = o.line_read.clone();
            work.lines_reused += out.lines.len();
            return Ok(out);
        }
        (None, None) => (0..0, None),
        (Some(w), o) => (w, o),
    };
    // Prefix lines whose inputs precede the window. A line read every unit up to
    // `line_read` (incl. the unsafe-path greedy look-ahead, which can exceed its
    // final end) and the unsafe flag at that unit's end byte; all must lie before
    // the window. The final-end bound alone is insufficient (review 2026-10-07).
    let mut first = 0;
    let old_n = old.map_or(0, |o| o.a.ends.len());
    if let Some(o) = old {
        let dep = |unit: usize| if unit < old_n { o.a.ends[unit].0 } else { o.a.trimmed + 1 };
        let mut k = 0;
        while k < o.lines.len() && dep(o.line_read[k]) < window.start {
            k += 1;
        }
        // Diagnostics: prefix lines the final-end bound alone would have reused.
        let mut by_last = k;
        while by_last < o.lines.len() && dep(o.line_last[by_last]) < window.start {
            by_last += 1;
        }
        work.lookahead_guarded += by_last - k;
        out.lines.extend_from_slice(&o.lines[..k]);
        out.line_start.extend_from_slice(&o.line_start[..k]);
        out.line_first.extend_from_slice(&o.line_first[..k]);
        out.line_last.extend_from_slice(&o.line_last[..k]);
        out.line_read.extend_from_slice(&o.line_read[..k]);
        work.lines_reused += k;
        first = if k == 0 { 0 } else { o.line_last[k - 1] };
    }
    let du = n as isize - old_n as isize;
    let font_size = req.size;
    let width = req.width;
    let align = if a.rtl ^ req.end_align { Align::Right } else { Align::Left };
    let levels = &a.levels;
    let rtl = a.rtl;
    let para_bidi = a.para.map(|p| (text, &a.classes[..], &a.raw_levels[..], p));
    let mut scratch = Vec::new();
    cancel.check("fit_start")?;
    while first < n {
        // Convergence: a new line start beyond the window that is a cached line start.
        if let Some(o) = old {
            let b = if first == 0 { 0 } else { a.ends[first - 1].0 };
            if b >= window.end && b > 0 {
                let ob = b.wrapping_add_signed(-delta);
                if let Ok(j) = o.line_start.binary_search(&ob) {
                    if o.line_first[j].wrapping_add_signed(du) == first {
                        out.lines.extend_from_slice(&o.lines[j..]);
                        out.line_start.extend(o.line_start[j..].iter().map(|s| s.wrapping_add_signed(delta)));
                        out.line_first.extend(o.line_first[j..].iter().map(|s| s.wrapping_add_signed(du)));
                        out.line_last.extend(o.line_last[j..].iter().map(|s| s.wrapping_add_signed(du)));
                        out.line_read.extend(o.line_read[j..].iter().map(|s| s.wrapping_add_signed(du)));
                        work.lines_reused += o.lines.len() - j;
                        return Ok(out);
                    }
                }
            }
        }
        cancel.check("fit_line")?;
        let mut last = first + 1;
        let mut used = a.unit_width[first];
        while last < n
            && a.ends[last - 1].1 != BreakOpportunity::Mandatory
            && width.is_none_or(|w| used + a.unit_width[last] <= w)
        {
            used += a.unit_width[last];
            last += 1;
        }
        let greedy = last;
        let source_start = if first == 0 { 0 } else { a.ends[first - 1].0 };
        let line = if !unsafe_at(a, text, source_start)
            && !unsafe_at(a, text, a.ends[last - 1].0)
            && (last == n || a.ends[last - 1].1 == BreakOpportunity::Mandatory || !unsafe_at(a, text, a.ends[last].0))
        {
            place_fragments(&a.template, unit_fragments(a, first, last), font_size, width, align)
        } else {
            let mut place = |last: usize| {
                let source = source_start..a.ends[last - 1].0;
                let fragments = if unsafe_at(a, text, source.start) || unsafe_at(a, text, source.end) {
                    work.edge_reshapes += 1;
                    let attrs = line_attrs(a, source.clone());
                    reshape_edge(&mut engine.fonts, text, &attrs, levels, rtl, source)
                        .spans
                        .into_iter()
                        .flat_map(|s| {
                            let level = s.level;
                            s.words.into_iter().flat_map(move |w| {
                                let blank = w.blank;
                                w.glyphs.into_iter().map(move |g| (level, blank, g))
                            })
                        })
                        .collect()
                } else {
                    unit_fragments(a, first, last)
                };
                place_fragments(&a.template, fragments, font_size, width, align)
            };
            let mut line = place(last);
            while last > first + 1 && width.is_some_and(|w| line.w > w) {
                last -= 1;
                line = place(last);
            }
            while last < n && a.ends[last - 1].1 != BreakOpportunity::Mandatory {
                if width.is_some_and(|w| line.w + a.unit_width[last] > w)
                    && !unsafe_at(a, text, source_start)
                    && !unsafe_at(a, text, a.ends[last - 1].0)
                    && !unsafe_at(a, text, a.ends[last].0)
                {
                    break;
                }
                let next = place(last + 1);
                if width.is_some_and(|w| next.w > w) {
                    break;
                }
                last += 1;
                line = next;
            }
            line
        };
        let source = source_start..a.ends[last - 1].0;
        let (mut placed, overflow) = engine.place_line(req, 0, rtl, source.clone(), line, para_bidi)?;
        let mut carets = Vec::new();
        line_carets(&placed, 0, &a.raw, &a.legal, &mut scratch, &mut carets);
        carets.sort_by(caret_order);
        carets.dedup();
        carets.shrink_to_fit();
        for c in &mut carets {
            c.byte -= source.start;
        }
        for g in &mut placed.glyphs {
            g.cluster.start -= source.start;
            g.cluster.end -= source.start;
        }
        placed.range = 0..source.len();
        out.lines.push(Rc::new(CachedLine { len: source.len(), line: placed, overflow, carets }));
        out.line_start.push(source.start);
        out.line_first.push(first);
        out.line_last.push(last);
        // `greedy` (< n) was read via its width and end flag; the forward probe
        // reads `last` likewise. `n` marks "read through the paragraph end".
        out.line_read.push(greedy.max(last).min(n));
        work.lookahead_beyond_last += usize::from(greedy > last);
        work.lines_refit += 1;
        first = last;
    }
    debug_assert!(out.line_start.last().is_none_or(|s| *s <= tn));
    Ok(out)
}

#[cfg(test)]
thread_local! {
    static FORCE_INTERNAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
mod tests {
    use crate::{incremental::Incremental, Engine, Request};

    /// An internal splice inconsistency must fall back to a full analysis rebuild
    /// for that paragraph, never fail the edit; the result still equals cold layout.
    #[test]
    fn internal_splice_error_rebuilds_instead_of_failing() {
        let text = "Logo شعار 12 ".repeat(60);
        let mut cache = Incremental::new(crate::test_fonts(), 32 * 1024 * 1024).unwrap();
        cache.layout(&Request::new(&text, 48., Some(300.)), || false).unwrap();
        let mut edited = text.clone();
        edited.insert(text.len() / 2 + 1, 'X');
        let r = Request::new(&edited, 48., Some(300.));
        super::FORCE_INTERNAL.with(|f| f.set(true));
        let result = cache.layout(&r, || false);
        super::FORCE_INTERNAL.with(|f| f.set(false));
        assert_eq!(result, Engine::new(crate::test_fonts()).unwrap().layout(&r));
        let w = &cache.counters.work;
        assert_eq!((w.internal_rebuilds, w.analysis_full, w.analysis_incremental), (1, 1, 0), "{w:?}");
    }
}
