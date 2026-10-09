//! Unicode/context invalidation and shaped-unit rebuilding.
use super::*;
pub(super) fn styles_eq(a: &[crate::engine::Style], b: &[crate::engine::Style]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.range == b.range
                && a.face == b.face
                && a.size.to_bits() == b.size.to_bits()
                && a.baseline_shift.to_bits() == b.baseline_shift.to_bits()
                && a.paint == b.paint
        })
}
pub(super) fn runs_eq(a: &[crate::engine::LanguageRun], b: &[crate::engine::LanguageRun]) -> bool {
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
pub(super) fn analyze(
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
