//! Pure paragraph composition over COSMIC shaped lines and paragraph UAX #14.
//! Total-fit/fitness idea adapted from VectorCraft crates/text/src/composer.rs
//! (ArtCraft Team and contributors, 2026, MIT OR Apache-2.0). See varos/NOTICE.
//! Edge widths are re-shaped, rather than estimated from independent words.
use crate::{kashida, metrics::LineHeight};
use cosmic_text::{Align, Hinting, LayoutLine, ShapeLine, Wrap};
use std::ops::Range;
use unicode_linebreak::{linebreaks, BreakOpportunity};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Composer {
    #[default]
    Greedy,
    EveryLine,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Centre,
    #[default]
    Right,
    JustifyLastLeft,
    JustifyLastCentre,
    JustifyLastRight,
    JustifyFull,
}
impl Alignment {
    fn justify(self, last: bool) -> bool {
        matches!(self, Self::JustifyFull)
            || (!last && matches!(self, Self::JustifyLastLeft | Self::JustifyLastCentre | Self::JustifyLastRight))
    }
    pub(crate) fn shift(self, slack: f32) -> f32 {
        match self {
            Self::Centre | Self::JustifyLastCentre => slack * 0.5,
            Self::Right | Self::JustifyLastRight => slack,
            _ => 0.,
        }
    }
}
/// Constraints for the future frame allocator. A paragraph cannot have widows
/// or orphans until it crosses a frame; never fake them by inserting line breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePolicy {
    pub widows: usize,
    pub orphans: usize,
}
impl Default for FramePolicy {
    fn default() -> Self {
        Self { widows: 2, orphans: 2 }
    }
}
impl FramePolicy {
    /// Number of lines that may enter this frame; zero means defer the paragraph.
    pub fn split(self, lines: usize, capacity: usize) -> usize {
        if lines <= capacity {
            return lines;
        }
        let take = capacity.min(lines.saturating_sub(self.widows));
        if take < self.orphans {
            0
        } else {
            take
        }
    }
}
#[derive(Clone, Debug)]
pub struct ParagraphOptions {
    pub composer: Composer,
    pub alignment: Alignment,
    pub kashida: kashida::Kashida,
    pub line_height: LineHeight,
    pub frames: FramePolicy,
    /// Exact edge reshapes per paragraph. Refuse rather than silently downgrade.
    pub max_measurements: usize,
}
impl Default for ParagraphOptions {
    fn default() -> Self {
        Self {
            composer: Composer::Greedy,
            alignment: Alignment::Right,
            kashida: kashida::Kashida::Off,
            line_height: LineHeight::default(),
            frames: FramePolicy::default(),
            max_measurements: 32768,
        }
    }
}
impl ParagraphOptions {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        self.line_height.validate()?;
        if self.max_measurements == 0
            || self.max_measurements > 131072
            || self.frames.widows == 0
            || self.frames.orphans == 0
        {
            return Err("invalid paragraph resource/frame policy");
        }
        Ok(())
    }
}
/// Dictionary seam. Providers return logical UTF-8 discretionary positions;
/// composition shapes a virtual hyphen at selected breaks without changing source.
pub trait HyphenationProvider {
    fn opportunities(&self, word: &str) -> Vec<usize>;
}
pub fn hyphenation_points(word: &str, provider: &dyn HyphenationProvider) -> Result<Vec<usize>, &'static str> {
    let mut points = provider.opportunities(word);
    let boundaries: Vec<_> = word.grapheme_indices(true).map(|(i, _)| i).collect();
    if points.iter().any(|i| *i == 0 || *i >= word.len() || boundaries.binary_search(i).is_err()) {
        return Err("hyphenation provider returned a non-grapheme boundary");
    }
    points.sort_unstable();
    points.dedup();
    Ok(points)
}

pub(crate) struct ComposedLine {
    pub range: Range<usize>,
    pub line: LayoutLine,
    pub inserted: Vec<usize>,
    pub residual: f32,
    pub hyphen: bool,
}
/// Unbounded Word avoids upstream Wrap::None's opposing-run loss.
pub(crate) fn place(shape: &ShapeLine, size: f32) -> LayoutLine {
    shape.layout(size, None, Wrap::Word, Some(Align::Left), None, Hinting::Disabled).remove(0)
}

pub(crate) fn compose_shaped(
    text: &str,
    size: f32,
    width: Option<f32>,
    options: &ParagraphOptions,
    provider: Option<&dyn HyphenationProvider>,
    mut shape: impl FnMut(Range<usize>, &[usize], bool) -> ShapeLine,
) -> Result<Vec<ComposedLine>, &'static str> {
    let mut ends: Vec<_> = linebreaks(text).map(|(b, kind)| (b, kind, false)).collect();
    if ends.last().map(|e| e.0) != Some(text.len()) {
        ends.push((text.len(), BreakOpportunity::Mandatory, false));
    }
    if width.is_some() {
        if let Some(provider) = provider {
            for (start, word) in text.unicode_word_indices() {
                for local in hyphenation_points(word, provider)? {
                    ends.push((start + local, BreakOpportunity::Allowed, true));
                }
            }
            ends.sort_unstable_by_key(|e| (e.0, e.2));
            ends.dedup_by_key(|e| e.0); // Natural breaks win over discretionary ones.
        }
    }
    let mut measurements = 0;
    let mut measure = |range: Range<usize>| -> Result<f32, &'static str> {
        measurements += 1;
        if measurements > options.max_measurements {
            return Err("paragraph measurement limit");
        }
        let hyphen = ends.binary_search_by_key(&range.end, |e| e.0).ok().is_some_and(|i| ends[i].2);
        Ok(place(&shape(range, &[], hyphen), size).w)
    };
    let breaks = match (options.composer, width) {
        (_, None) => ends.iter().filter(|e| e.1 == BreakOpportunity::Mandatory).map(|e| e.0).collect(),
        (Composer::Greedy, Some(w)) => greedy(&ends, w, &mut measure)?,
        (Composer::EveryLine, Some(w)) => total_fit(&ends, w, &mut measure)?,
    };
    let mut start = 0;
    let mut output = Vec::new();
    for (index, &end) in breaks.iter().enumerate() {
        let range = start..end;
        let hyphen = ends.binary_search_by_key(&end, |e| e.0).ok().is_some_and(|i| ends[i].2);
        let original = shape(range.clone(), &[], hyphen);
        let natural = place(&original, size);
        let target = width.unwrap_or(natural.w);
        let mut result = if width.is_some() && options.alignment.justify(index + 1 == breaks.len()) {
            kashida::justify(text, range.clone(), &original, size, target, options.kashida, |slots| {
                shape(range.clone(), slots, hyphen)
            })
        } else {
            ComposedLine { range: range.clone(), line: natural, inserted: vec![], residual: 0., hyphen }
        };
        result.hyphen = hyphen;
        let shift = if options.alignment.justify(index + 1 == breaks.len()) {
            0.
        } else {
            options.alignment.shift(target - result.line.w)
        };
        for g in &mut result.line.glyphs {
            g.x += shift;
        }
        output.push(result);
        start = end;
    }
    Ok(output)
}
fn greedy(
    ends: &[(usize, BreakOpportunity, bool)],
    width: f32,
    measure: &mut impl FnMut(Range<usize>) -> Result<f32, &'static str>,
) -> Result<Vec<usize>, &'static str> {
    let mut out = Vec::new();
    let mut first = 0;
    while first < ends.len() {
        let start = if first == 0 { 0 } else { ends[first - 1].0 };
        let mut last = first; // Preserve explicit overflow when no edge fits.
        for (j, end) in ends.iter().enumerate().skip(first) {
            let natural = measure(start..end.0)?;
            if natural <= width {
                last = j;
            } else if !end.2 {
                break;
            }
            // A discretionary edge includes a hyphen and can be wider than the
            // following natural edge (e.g. a final narrow i). Keep looking until
            // the natural word boundary before deciding that the word overflows.
            if end.1 == BreakOpportunity::Mandatory {
                break;
            }
        }
        out.push(ends[last].0);
        first = last + 1;
    }
    Ok(out)
}
/// Bounded Knuth–Plass-style DP: total squared badness and adjacent fitness
/// penalties, retaining four fitness alternatives per legal boundary.
fn total_fit(
    ends: &[(usize, BreakOpportunity, bool)],
    width: f32,
    measure: &mut impl FnMut(Range<usize>) -> Result<f32, &'static str>,
) -> Result<Vec<usize>, &'static str> {
    let n = ends.len();
    if n > 4096 {
        return Err("every-line breakpoint limit");
    }
    let mut costs = vec![[f64::INFINITY; 4]; n + 1];
    let mut prev = vec![[None; 4]; n + 1];
    costs[0][1] = 0.;
    for i in 0..n {
        if costs[i].iter().all(|c| !c.is_finite()) {
            continue;
        }
        let start = if i == 0 { 0 } else { ends[i - 1].0 };
        for j in i..n {
            let natural = measure(start..ends[j].0)?;
            if natural > width && j > i {
                if ends[j].2 {
                    continue;
                }
                break;
            }
            let ratio = ((width - natural) / width).max(0.);
            let fitness: usize = if ratio < 0.15 {
                0
            } else if ratio < 0.4 {
                1
            } else if ratio < 0.75 {
                2
            } else {
                3
            };
            let last = ends[j].1 == BreakOpportunity::Mandatory;
            let badness = if natural > width {
                1e8
            } else if last {
                f64::from((width / 3. - natural).max(0.) / width).powi(2) * 10000.
            } else {
                f64::from(ratio).powi(2) * 10000.
            };
            for f in 0..4 {
                let adjacent = if i > 0 && fitness.abs_diff(f) > 1 { 1000. } else { 0. };
                let cost = costs[i][f] + 10. + badness + adjacent + if ends[j].2 { 100. } else { 0. };
                if cost < costs[j + 1][fitness] {
                    costs[j + 1][fitness] = cost;
                    prev[j + 1][fitness] = Some((i, f));
                }
            }
            if last {
                break;
            }
        }
    }
    let mut fit = (0..4).min_by(|a, b| costs[n][*a].total_cmp(&costs[n][*b])).ok_or("no paragraph fitness state")?;
    let mut at = n;
    let mut out = Vec::new();
    while at > 0 {
        out.push(ends[at - 1].0);
        let (p, f) = prev[at][fit].ok_or("no legal paragraph composition")?;
        at = p;
        fit = f;
    }
    out.reverse();
    Ok(out)
}
