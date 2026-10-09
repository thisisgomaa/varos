//! Legal-line fitting and convergence.
use super::*;
/// Whitespace-led grapheme ends within a slice (the engine's segment cuts).
pub(super) trait WsGraphemes {
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
pub(super) fn fit_lines(
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
