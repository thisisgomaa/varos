//! Joining guards ported from Ahmed's BStudio bstudio-text/kashida.rs (June
//! 2026, MPL-2.0; covered portions also under GPL-3.0 per MPL §3.3). See NOTICE.
//! Width growth is a measurement, never a substitute for HarfRust's safe flag.
use crate::composer::{place, ComposedLine};
use cosmic_text::{AttrsList, FontSystem, ShapeLine};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::Range,
};
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
    let mut line = place(original, size);
    let initial = line.w;
    let (per_word, fraction) = policy.policy();
    let budget = (target - initial).max(0.) * fraction;
    // Off performs no boundary/candidate discovery at all.
    let index = if per_word > 0 { candidate_index(text, &range) } else { BTreeMap::new() };
    let mut safe = if per_word > 0 { safe_slots(original) } else { HashSet::new() };
    let mut slots: Vec<_> = index.iter().filter(|(b, _)| safe.contains(b)).map(|(b, meta)| (*b, *meta)).collect();
    slots.sort_unstable_by_key(|(b, (priority, _))| (*priority, *b));
    let mut word_counts = HashMap::<usize, usize>::new();
    let mut inserted = Vec::new();
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
            let shaped = reshape(&trial);
            let measured = place(&shaped, size);
            if measured.w > line.w + 0.001
                && measured.w <= initial + budget + 0.001
                && measured.w <= target + 0.001
                && measured.glyphs.iter().all(|g| g.glyph_id != 0)
            {
                line = measured;
                safe = safe_slots(&shaped);
                *word_counts.entry(word).or_default() += 1;
                inserted = trial;
            }
        }
    }
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
}
