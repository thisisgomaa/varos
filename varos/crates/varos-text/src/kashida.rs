//! Joining guards ported from Ahmed's BStudio bstudio-text/kashida.rs (June
//! 2026; his own work, relicensed with this crate as MIT OR Apache-2.0). See NOTICE.
//! Width growth is a measurement, never a substitute for HarfRust's safe flag.
//!
//! The policy ([`candidates`], [`plan`], [`WorkingText`]) is backend-neutral: it
//! never shapes. A host re-shapes the line with its own shaper and face and
//! reports [`Measured`] widths and safe-to-insert-tatweel boundaries. The COSMIC
//! adapter (`justify`, `reshape_virtual`) needs the default `cosmic` feature.
#[cfg(feature = "cosmic")]
use crate::composer::{place, ComposedLine};
#[cfg(feature = "cosmic")]
use cosmic_text::{AttrsList, FontSystem, ShapeLine};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::{Add, Mul, Range, Sub},
};
#[cfg(feature = "cosmic")]
use unicode_bidi::Level;
use unicode_segmentation::UnicodeSegmentation;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kashida {
    #[default]
    Off,
    Minimal,
    Balanced,
    Display,
}
impl Kashida {
    fn policy(self) -> (usize, f32) {
        match self {
            Self::Off => (0, 0.),
            Self::Minimal => (1, 0.25),
            Self::Balanced => (2, 0.6),
            Self::Display => (4, 1.),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JoinClass {
    Both,
    Right,
    Alif,
    Lam,
    Other,
}
fn classify(ch: char) -> JoinClass {
    match ch {
        'ي' | 'ئ' | 'ه' | 'ش' | 'س' | 'ق' | 'ف' | 'غ' | 'ع' | 'ض' | 'ص' | 'ن' | 'م' | 'ك' | 'ظ' | 'ط' | 'خ' | 'ح'
        | 'ج' | 'ث' | 'ت' | 'ب' => JoinClass::Both,
        'و' | 'ؤ' | 'ذ' | 'د' | 'ز' | 'ر' | 'ة' => JoinClass::Right,
        'ا' | 'أ' | 'إ' | 'آ' => JoinClass::Alif,
        'ل' => JoinClass::Lam,
        _ => JoinClass::Other,
    }
}
pub fn is_harakat(c: char) -> bool {
    matches!(c, '\u{064B}'..='\u{065F}'|'\u{0670}'|'\u{06D6}'..='\u{06DC}'|'\u{06DF}'..='\u{06E4}'|'\u{06E7}'..='\u{06E8}'|'\u{06EA}'..='\u{06ED}')
}
/// Conservative BStudio/Hallberg pair table; unsupported scripts stay unstretched.
pub fn joining_allows(prev: char, next: char) -> bool {
    use JoinClass::*;
    matches!((classify(prev), classify(next)), (Both, Both | Right | Alif | Lam) | (Lam, Both | Right | Lam))
}
/// ALReq-style conservative preference, not exhaustive conformance.
fn priority(prev: char) -> u8 {
    match prev {
        'س' | 'ش' => 0,
        'ص' | 'ض' => 1,
        _ => 2,
    }
}
/// Build grapheme/join/word metadata once, rather than rescanning each prefix.
fn candidate_index(text: &str, range: &Range<usize>) -> BTreeMap<usize, (u8, usize)> {
    let mut index = BTreeMap::new();
    let mut word = range.start;
    let mut previous = None;
    for (local, grapheme) in text[range.clone()].grapheme_indices(true) {
        let byte = range.start + local;
        if let (Some(prev), Some(next)) = (previous, grapheme.chars().next()) {
            if !is_harakat(prev) && !is_harakat(next) && joining_allows(prev, next) {
                index.insert(byte, (priority(prev), word));
            }
        }
        for (i, c) in grapheme.char_indices() {
            if c.is_whitespace() {
                word = byte + i + c.len_utf8();
            }
            previous = Some(c);
        }
    }
    index
}
/// A boundary inside `range` where a tatweel may join the previous grapheme to
/// the next one: `byte` is the next grapheme's start (the insertion point),
/// `priority` 0 is preferred, and `word` is the byte where its word starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub byte: usize,
    pub priority: u8,
    pub word: usize,
}
/// `range` is an ordered, in-bounds range of `text` on UTF-8 boundaries.
fn check_range(text: &str, range: &Range<usize>) -> Result<(), &'static str> {
    if range.start > range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err("invalid kashida text range");
    }
    Ok(())
}
/// Joining-legal insertion points of `text[range]`, in byte order. Whether the
/// font may take a tatweel there is the shaper's answer ([`Measured::safe`]).
/// A reversed, out-of-bounds or mid-character `range` is an error.
pub fn candidates(text: &str, range: Range<usize>) -> Result<Vec<Candidate>, &'static str> {
    check_range(text, &range)?;
    Ok(candidate_index(text, &range)
        .into_iter()
        .map(|(byte, (priority, word))| Candidate { byte, priority, word })
        .collect())
}
/// One host re-shape of a line with virtual tatweels at the trial slots.
#[derive(Clone, Debug)]
pub struct Measured<W, T> {
    /// The re-shaped line's natural width.
    pub width: W,
    /// Source byte boundaries the shaper still marks safe to insert a tatweel.
    pub safe: HashSet<usize>,
    /// Any glyph of the re-shaped line is missing (glyph id 0).
    pub missing_glyph: bool,
    /// Whatever the host keeps of the accepted shape (its glyphs, for example).
    pub payload: T,
}
/// The accepted tatweel slots (sorted source bytes), the line's width with
/// them, and the payload of the last accepted re-shape (`None`: no slot taken).
#[derive(Clone, Debug)]
pub struct Plan<W, T> {
    pub slots: Vec<usize>,
    pub width: W,
    pub payload: Option<T>,
}
/// Width arithmetic shared by the f32 COSMIC path and f64 hosts.
pub trait Width: Copy + PartialOrd + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + From<f32> {}
impl<W> Width for W where W: Copy + PartialOrd + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + From<f32>
{}
/// Choose kashida slots for the line `text[range]` whose natural width is
/// `natural` and whose shaper-safe boundaries are `safe`, growing it toward
/// `target` within the policy's budget. `reshape(slots)` re-shapes the line with
/// virtual tatweels at `slots` (sorted source bytes); `None` rejects the trial.
/// Each accepted trial must widen the line, stay within the budget and target,
/// and keep every glyph; safe boundaries are re-read after each acceptance.
/// Spaces are not stretched here: the host spreads what is left. A reversed,
/// out-of-bounds or mid-character `range` is an error (nothing is re-shaped).
pub fn plan<W: Width, T>(
    text: &str,
    range: Range<usize>,
    natural: W,
    safe: HashSet<usize>,
    target: W,
    policy: Kashida,
    mut reshape: impl FnMut(&[usize]) -> Option<Measured<W, T>>,
) -> Result<Plan<W, T>, &'static str> {
    check_range(text, &range)?;
    let mut width = natural;
    let initial = natural;
    let (per_word, fraction) = policy.policy();
    let slack = target - initial;
    let zero = W::from(0.);
    let budget = (if slack > zero { slack } else { zero }) * W::from(fraction);
    let epsilon = W::from(0.001);
    // Off performs no boundary/candidate discovery at all.
    let index = if per_word > 0 { candidate_index(text, &range) } else { BTreeMap::new() };
    let mut safe = if per_word > 0 { safe } else { HashSet::new() };
    let mut slots: Vec<_> = index.iter().filter(|(b, _)| safe.contains(b)).map(|(b, meta)| (*b, *meta)).collect();
    slots.sort_unstable_by_key(|(b, (priority, _))| (*priority, *b));
    let mut word_counts = HashMap::<usize, usize>::new();
    let mut inserted = Vec::new();
    let mut payload = None;
    let mut attempts = 0;
    for _ in 0..per_word {
        for &(slot, (_, word)) in &slots {
            if attempts >= 64 {
                break;
            }
            if word_counts.get(&word).copied().unwrap_or(0) >= per_word {
                continue;
            }
            // Revalidate after each accepted insertion, including stacked joins.
            if !safe.contains(&slot) {
                continue;
            }
            let mut trial = inserted.clone();
            trial.push(slot);
            trial.sort_unstable();
            attempts += 1;
            let Some(measured) = reshape(&trial) else { continue };
            if measured.width > width + epsilon
                && measured.width <= initial + budget + epsilon
                && measured.width <= target + epsilon
                && !measured.missing_glyph
            {
                width = measured.width;
                safe = measured.safe;
                payload = Some(measured.payload);
                *word_counts.entry(word).or_default() += 1;
                inserted = trial;
            }
        }
    }
    Ok(Plan { slots: inserted, width, payload })
}
#[cfg(feature = "cosmic")]
fn safe_slots(shape: &ShapeLine) -> HashSet<usize> {
    shape
        .spans
        .iter()
        .flat_map(|s| &s.words)
        .flat_map(|w| &w.glyphs)
        .filter(|g| g.safe_to_insert_tatweel)
        .map(|g| g.start)
        .collect()
}
/// Visual prefix counts work even when bidi/component glyph order differs from x.
#[cfg(any(feature = "cosmic", test))]
fn space_prefixes(positions: &[f32], spaces: &[usize]) -> Vec<usize> {
    let mut order: Vec<_> = (0..positions.len()).collect();
    order.sort_unstable_by(|a, b| positions[*a].total_cmp(&positions[*b]));
    let mut space_x: Vec<_> = spaces.iter().map(|i| positions[*i]).collect();
    space_x.sort_unstable_by(f32::total_cmp);
    let mut prefix = vec![0; positions.len()];
    let mut count = 0;
    for i in order {
        while count < space_x.len() && space_x[count] < positions[i] - 0.001 {
            count += 1;
        }
        prefix[i] = count;
    }
    prefix
}
#[cfg(feature = "cosmic")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn justify(
    text: &str,
    range: Range<usize>,
    original: &ShapeLine,
    size: f32,
    target: f32,
    policy: Kashida,
    mut reshape: impl FnMut(&[usize]) -> ShapeLine,
) -> ComposedLine {
    let natural = place(original, size);
    let initial = natural.w;
    let safe = if policy.policy().0 > 0 { safe_slots(original) } else { HashSet::new() };
    let chosen = plan(text, range.clone(), initial, safe, target, policy, |trial| {
        let shaped = reshape(trial);
        let measured = place(&shaped, size);
        Some(Measured {
            width: measured.w,
            safe: safe_slots(&shaped),
            missing_glyph: measured.glyphs.iter().any(|g| g.glyph_id == 0),
            payload: measured,
        })
    })
    // The composer's line ranges are always valid; an invalid one takes no kashida.
    .unwrap_or(Plan { slots: vec![], width: initial, payload: None });
    let inserted = chosen.slots;
    let mut line = chosen.payload.unwrap_or(natural);
    // Only ordinary internal spaces stretch; NBSP/tabs/trailing spaces do not.
    let content_end = range.start + text[range.clone()].trim_end().len();
    let spaces: Vec<_> = line
        .glyphs
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            (g.start > range.start && g.end <= content_end && text.get(g.start..g.end) == Some(" ")).then_some(i)
        })
        .collect();
    let slack = target - line.w;
    if slack > 0. && !spaces.is_empty() {
        let extra = slack / spaces.len() as f32;
        let positions: Vec<_> = line.glyphs.iter().map(|g| g.x).collect();
        let prefix = space_prefixes(&positions, &spaces);
        for (g, count) in line.glyphs.iter_mut().zip(prefix) {
            g.x += count as f32 * extra;
        }
        for i in spaces {
            line.glyphs[i].w += extra;
        }
        line.w = target;
    }
    let residual = target - line.w;
    ComposedLine { range, line, inserted, residual, hyphen: false }
}
/// Each working byte maps to its original byte; virtual tatweels map to the
/// insertion boundary. This mapping never changes the authored string.
#[derive(Clone, Debug)]
pub struct WorkingText {
    pub text: String,
    pub source_at: Vec<usize>,
}
impl WorkingText {
    pub fn new(source: &str, slots: &[usize]) -> Result<Self, &'static str> {
        if slots.len() > 64
            || slots.windows(2).any(|w| w[0] > w[1])
            || slots.iter().any(|b| *b == 0 || *b >= source.len() || !source.is_char_boundary(*b))
        {
            return Err("invalid virtual tatweel positions");
        }
        let mut text = String::new();
        let mut source_at = Vec::new();
        let mut si = 0;
        for (b, c) in source.char_indices() {
            while si < slots.len() && slots[si] == b {
                text.push('\u{0640}');
                source_at.extend([b, b]);
                si += 1;
            }
            text.push(c);
            source_at.extend(b..b + c.len_utf8());
        }
        source_at.push(source.len());
        Ok(Self { text, source_at })
    }
}
#[cfg(feature = "cosmic")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn reshape_virtual(
    fonts: &mut FontSystem,
    text: &str,
    attrs: &AttrsList,
    levels: &[Level],
    rtl: bool,
    range: Range<usize>,
    slots: &[usize],
    hyphen: bool,
) -> ShapeLine {
    if slots.is_empty() && !hyphen {
        return crate::engine::reshape_edge(fonts, text, attrs, levels, rtl, range);
    }
    let local_slots: Vec<_> = slots.iter().map(|b| b - range.start).collect();
    let mut map = WorkingText::new(&text[range.clone()], &local_slots).expect("internally validated slots");
    if hyphen {
        map.text.push('-');
        map.source_at.push(range.len());
    }
    let mut mapped_attrs = AttrsList::new(&attrs.get_span(range.start));
    let mut mapped_levels = Vec::with_capacity(map.text.len());
    for (i, c) in map.text.char_indices() {
        let source = (range.start + map.source_at[i]).min(range.end - 1);
        mapped_attrs.add_span(i..i + c.len_utf8(), &attrs.get_span(source));
        mapped_levels.extend(std::iter::repeat_n(levels[source], c.len_utf8()));
    }
    let mut shaped =
        crate::engine::reshape_edge(fonts, &map.text, &mapped_attrs, &mapped_levels, rtl, 0..map.text.len());
    for g in shaped.spans.iter_mut().flat_map(|s| &mut s.words).flat_map(|w| &mut w.glyphs) {
        g.start = range.start + map.source_at[g.start];
        g.end = range.start + map.source_at[g.end];
        // Virtual-only glyphs join the preceding source grapheme, not a new edit stop.
        if g.start == g.end {
            g.start = text[range.start..g.start]
                .grapheme_indices(true)
                .next_back()
                .map_or(range.start, |(i, _)| range.start + i);
        }
    }
    shaped
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The checked API on ranges the tests know are valid.
    fn candidates(text: &str, range: Range<usize>) -> Vec<Candidate> {
        super::candidates(text, range).expect("valid range")
    }
    fn plan<W: Width, T>(
        text: &str,
        range: Range<usize>,
        natural: W,
        safe: HashSet<usize>,
        target: W,
        policy: Kashida,
        reshape: impl FnMut(&[usize]) -> Option<Measured<W, T>>,
    ) -> Plan<W, T> {
        super::plan(text, range, natural, safe, target, policy, reshape).expect("valid range")
    }
    #[test]
    fn invalid_ranges_are_errors_not_panics() {
        let text = "بسم الله";
        let mid = 1; // inside the two-byte ب
        let reversed = Range { start: 4, end: 2 };
        let past = 0..text.len() + 1;
        for range in [reversed.clone(), mid..text.len(), 0..mid, past.clone()] {
            assert!(super::candidates(text, range.clone()).is_err(), "{range:?}");
            let calls = std::cell::Cell::new(0);
            let p = super::plan(text, range.clone(), 1f64, HashSet::new(), 10., Kashida::Display, |_| {
                calls.set(calls.get() + 1);
                None::<Measured<f64, ()>>
            });
            assert!(p.is_err() && calls.get() == 0, "{range:?}");
        }
        assert!(super::candidates(text, 0..0).unwrap().is_empty());
        assert!(super::candidates(text, 0..text.len()).is_ok());
    }
    #[test]
    fn indexed_candidates_match_join_and_word_oracle() {
        let text = "prefix سسس بَب سلام\tششش لا لأ bb 123 سسس";
        let range = 7..text.len();
        let index = candidate_index(text, &range);
        let mut expected = BTreeMap::new();
        for (local, _) in text[range.clone()].grapheme_indices(true).skip(1) {
            let b = range.start + local;
            let prev = text[..b].chars().next_back().unwrap();
            let next = text[b..].chars().next().unwrap();
            if !is_harakat(prev) && !is_harakat(next) && joining_allows(prev, next) {
                let word = text[..b]
                    .char_indices()
                    .rev()
                    .find(|(_, c)| c.is_whitespace())
                    .map_or(range.start, |(i, c)| i + c.len_utf8());
                expected.insert(b, (priority(prev), word));
            }
        }
        assert_eq!(index, expected);
        let long = "س".repeat(20000);
        assert_eq!(candidate_index(&long, &(0..long.len())).len(), 19999);
    }
    #[test]
    fn visual_space_prefix_matches_quadratic_oracle_with_ties_and_bidi_order() {
        let positions = [20., 0., 10., 10., 10.0005, 10.002, 30., -2.];
        let spaces = [1, 2, 3, 7];
        let expected: Vec<_> =
            positions.iter().map(|x| spaces.iter().filter(|i| positions[**i] < x - 0.001).count()).collect();
        assert_eq!(space_prefixes(&positions, &spaces), expected);
        let positions: Vec<_> = (0..20000).rev().map(|i| i as f32).collect();
        let spaces: Vec<_> = (0..20000).collect();
        assert_eq!(space_prefixes(&positions, &spaces), (0..20000).rev().collect::<Vec<_>>());
    }

    /// A synthetic shaper: every tatweel adds `unit`, every candidate stays safe,
    /// and `missing` slots produce a glyph id 0.
    fn synthetic<'a>(
        text: &'a str,
        unit: f64,
        missing: &'a [usize],
        calls: &'a std::cell::Cell<usize>,
    ) -> impl FnMut(&[usize]) -> Option<Measured<f64, Vec<usize>>> + 'a {
        move |slots| {
            calls.set(calls.get() + 1);
            Some(Measured {
                width: 100. + unit * slots.len() as f64,
                safe: candidates(text, 0..text.len()).into_iter().map(|c| c.byte).collect(),
                missing_glyph: slots.iter().any(|s| missing.contains(s)),
                payload: slots.to_vec(),
            })
        }
    }
    fn all_safe(text: &str) -> HashSet<usize> {
        candidates(text, 0..text.len()).into_iter().map(|c| c.byte).collect()
    }

    #[test]
    fn plan_prefers_seen_then_sad_and_caps_each_word() {
        // سسس has priority-0 joins; بمب has priority-2 joins.
        let text = "بمبمب سسسس";
        let calls = std::cell::Cell::new(0);
        let plan =
            plan(text, 0..text.len(), 100., all_safe(text), 200., Kashida::Balanced, synthetic(text, 1., &[], &calls));
        // Balanced: at most two per word, so two words give at most four.
        assert_eq!(plan.slots.len(), 4);
        let seen = text.find('س').unwrap();
        let in_seen = plan.slots.iter().filter(|b| **b > seen).count();
        assert_eq!(in_seen, 2, "two in each word: {:?}", plan.slots);
        // The first pass took the priority-0 (seen) joins before the others.
        let first = candidates(text, 0..text.len());
        assert_eq!(first.iter().filter(|c| c.priority == 0).count(), 3);
        assert_eq!(plan.width, 104.);
        assert_eq!(plan.payload.as_deref(), Some(&plan.slots[..]));
        assert!(plan.slots.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn plan_keeps_within_budget_and_target_and_rejects_missing_glyphs() {
        let text = "سسسس سسسس سسسس";
        let calls = std::cell::Cell::new(0);
        // Slack 10, Balanced budget 6: a 4-wide tatweel fits once.
        let p =
            plan(text, 0..text.len(), 100., all_safe(text), 110., Kashida::Balanced, synthetic(text, 4., &[], &calls));
        assert_eq!(p.slots.len(), 1);
        assert_eq!(p.width, 104.);
        // No slack: nothing taken, no payload, natural width kept.
        let p =
            plan(text, 0..text.len(), 100., all_safe(text), 100., Kashida::Display, synthetic(text, 1., &[], &calls));
        assert!(p.slots.is_empty() && p.payload.is_none() && p.width == 100.);
        // A slot whose re-shape loses a glyph is never taken.
        let first = candidates(text, 0..text.len())[0].byte;
        let p = plan(
            text,
            0..text.len(),
            100.,
            all_safe(text),
            200.,
            Kashida::Minimal,
            synthetic(text, 1., &[first], &calls),
        );
        assert!(!p.slots.contains(&first) && !p.slots.is_empty());
    }

    #[test]
    fn plan_off_unsafe_and_rejected_trials_take_nothing_and_attempts_are_capped() {
        let text = "سسسس";
        let calls = std::cell::Cell::new(0);
        let p = plan(text, 0..text.len(), 100., all_safe(text), 200., Kashida::Off, synthetic(text, 1., &[], &calls));
        assert!(p.slots.is_empty() && calls.get() == 0, "Off never re-shapes");
        // Boundaries the shaper does not mark safe are never tried.
        let p =
            plan(text, 0..text.len(), 100., HashSet::new(), 200., Kashida::Display, synthetic(text, 1., &[], &calls));
        assert!(p.slots.is_empty() && calls.get() == 0);
        // A host that refuses a trial (None) leaves the line as it was.
        let p: Plan<f64, ()> = plan(text, 0..text.len(), 100., all_safe(text), 200., Kashida::Display, |_| None);
        assert!(p.slots.is_empty() && p.width == 100.);
        // At most 64 re-shapes per line, however many joins it has.
        let long = ["سسسسس"; 40].join(" ");
        let calls = std::cell::Cell::new(0);
        let p =
            plan(&long, 0..long.len(), 100., all_safe(&long), 1e9, Kashida::Display, synthetic(&long, 1., &[], &calls));
        assert_eq!(calls.get(), 64);
        assert_eq!(p.slots.len(), 64);
    }

    #[test]
    fn plan_rereads_safe_boundaries_after_each_acceptance() {
        let text = "سسسس";
        let joins: Vec<usize> = candidates(text, 0..text.len()).iter().map(|c| c.byte).collect();
        assert_eq!(joins.len(), 3);
        // After the first tatweel the shaper withdraws every other boundary.
        let p = plan(text, 0..text.len(), 100., joins.iter().copied().collect(), 200., Kashida::Display, |slots| {
            Some(Measured { width: 100. + slots.len() as f64, safe: HashSet::new(), missing_glyph: false, payload: () })
        });
        assert_eq!(p.slots, vec![joins[0]]);
    }

    #[test]
    fn plan_matches_in_f32_and_f64() {
        let text = "بمبمب سسسس صصص";
        let a = plan(text, 0..text.len(), 100f32, all_safe(text), 107., Kashida::Balanced, |s| {
            Some(Measured {
                width: 100. + 1.5 * s.len() as f32,
                safe: all_safe(text),
                missing_glyph: false,
                payload: (),
            })
        });
        let b = plan(text, 0..text.len(), 100f64, all_safe(text), 107., Kashida::Balanced, |s| {
            Some(Measured {
                width: 100. + 1.5 * s.len() as f64,
                safe: all_safe(text),
                missing_glyph: false,
                payload: (),
            })
        });
        assert_eq!(a.slots, b.slots);
        assert_eq!(f64::from(a.width), b.width);
    }

    #[test]
    fn working_text_maps_virtual_tatweels_to_their_boundary() {
        let source = "سلام";
        let slot = source.char_indices().nth(1).unwrap().0;
        let w = WorkingText::new(source, &[slot]).unwrap();
        assert_eq!(w.text, "سـلام");
        assert_eq!(w.source_at[slot..slot + 2], [slot, slot]);
        assert_eq!(*w.source_at.last().unwrap(), source.len());
        assert!(WorkingText::new(source, &[0]).is_err());
    }
}
