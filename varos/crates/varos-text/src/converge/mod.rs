//! Paragraph-analysis cache and convergent line reuse, promoted from P1c.
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

mod analysis;
mod fitting;
use analysis::{analyze, runs_eq, styles_eq};
use fitting::{fit_lines, WsGraphemes};

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
