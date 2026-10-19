//! Arabic kashida (tatweel) justification for VectorCraft text.
//!
//! The policy (which joins may take a tatweel, in which order, within which budget)
//! comes from Varos' `varos-text`, built without its own shaper. VectorCraft re-shapes
//! with its own HarfRust and the paragraph's own face (`crates/text/src/shape.rs`), so
//! measurement and stretching agree and no other font is ever substituted.
//!
//! Contract with `crates/text`:
//! - lines are broken by VectorCraft; only a justified, right-to-left line is stretched;
//! - [`plan`] proposes tatweel slots (source byte boundaries) and the host re-shapes the
//!   words that hold them, reporting [`Measured`] widths and HarfRust's
//!   safe-to-insert-tatweel boundaries;
//! - a virtual tatweel glyph has zero source length, at the boundary byte it fills, so
//!   carets, selection and exported text see only the authored characters.
use unicode_script::{Script, UnicodeScript};
pub use varos_text::kashida::{Candidate, Kashida, Measured, Plan, Width, WorkingText, candidates, is_harakat, joining_allows, plan};

/// `varos-text`'s Balanced policy: what every justified right-to-left Arabic paragraph took
/// before the paragraph setting existed, and still takes at Auto, strength 50 ([`policy`]).
pub const POLICY: Kashida = Kashida::Balanced;

/// A kashida policy in numbers: at most `per_word` tatweels in a word, filling at most
/// `fraction` of a line's slack (the word spaces take the rest). `varos-text`'s fixed policies
/// are [`Policy::MINIMAL`], [`Policy::BALANCED`] and [`Policy::DISPLAY`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    pub per_word: usize,
    pub fraction: f64,
}

impl Policy {
    /// `varos-text`'s `Kashida::Minimal`: one per word, a quarter of the slack.
    pub const MINIMAL: Policy = Policy { per_word: 1, fraction: 0.25 };
    /// `varos-text`'s `Kashida::Balanced`: two per word, 60 % of the slack.
    pub const BALANCED: Policy = Policy { per_word: 2, fraction: 0.6 };
    /// `varos-text`'s `Kashida::Display`: four per word, all the slack.
    pub const DISPLAY: Policy = Policy { per_word: 4, fraction: 1.0 };
    /// The most tatweels a word takes on a justified line ([`policy`] with Force at 100).
    pub const MAX_PER_WORD: usize = 5;
    /// The most tatweels a word takes when kashida elongates a line that isn't justified
    /// ([`growth_policy`]), and under any policy [`plan_with`] is given.
    pub const MAX_GROW_PER_WORD: usize = 8;
}

/// The policy that elongates a line that isn't justified (Force in Left, Right or Center
/// alignment): up to [`Policy::MAX_GROW_PER_WORD`] tatweels a word, as many as reach the line's
/// target width ([`growth_target`]).
pub const fn growth_policy() -> Policy {
    Policy { per_word: Policy::MAX_GROW_PER_WORD, fraction: 1.0 }
}

/// The width a line that isn't justified grows to under Force at `strength`: its natural width
/// grown by `strength` percent (any value from 0 up), and never past `room` (the box's width;
/// `None` for point type, which has no box). A line already wider than its room doesn't grow.
pub fn growth_target(natural: f64, strength: u32, room: Option<f64>) -> f64 {
    let grown = natural * (1.0 + f64::from(strength) / 100.0);
    match room {
        Some(room) if room.is_finite() => grown.min(room.max(natural)),
        _ => grown,
    }
}

/// The policy of a paragraph's kashida strength (0..=100; more reads as 100), `force`d or not.
///
/// Strength interpolates between `varos-text`'s fixed policies, so that 0 is Minimal, 50 is
/// Balanced (the policy before the setting existed) and 100 is Display:
///
/// | strength | per word | share of the slack |
/// |---|---|---|
/// | 0..=24 | 1 | 0.25 + 0.35 × s / 50 |
/// | 25..=50 | 2 | 0.25 + 0.35 × s / 50 (0.60 at 50) |
/// | 51..=75 | 3 | 0.60 + 0.40 × (s − 50) / 50 |
/// | 76..=100 | 4 | 0.60 + 0.40 × (s − 50) / 50 (1.00 at 100) |
///
/// Force goes deeper: one more tatweel per word (at most [`Policy::MAX_PER_WORD`]) and 0.4 more
/// of the slack (at most all of it); the layout also stretches the last line of a paragraph
/// justified with its last line aligned.
pub fn policy(strength: u8, force: bool) -> Policy {
    let s = f64::from(strength.min(100));
    let base = if s <= 50.0 {
        Policy { per_word: if s < 25.0 { 1 } else { 2 }, fraction: 0.25 + 0.35 * s / 50.0 }
    } else {
        Policy { per_word: if s <= 75.0 { 3 } else { 4 }, fraction: 0.6 + 0.4 * (s - 50.0) / 50.0 }
    };
    if force { Policy { per_word: (base.per_word + 1).min(Policy::MAX_PER_WORD), fraction: (base.fraction + 0.4).min(1.0) } } else { base }
}

/// [`plan`] with a numeric [`Policy`], spreading the tatweels across the line (owner decision
/// 2026-10-19), and never in the name of God ([`sacred_from`]).
///
/// `varos-text`'s `plan` walks every boundary in priority order once per round, so a deep policy
/// spends the whole slack on the first words of the line. Here each round gives every word of the
/// line at most one more tatweel, word after word in reading order (right to left), each word's
/// boundaries tried in `varos-text`'s priority order (after a seen, then a sad, then the rest) until
/// one is kept; rounds go on until a word holds `per_word` or the budget is spent. The checks on a
/// trial are `plan`'s: it must widen the line, stay within the budget and the target, keep every
/// glyph, and the safe boundaries are read again after each one kept.
///
/// `per_word` is bounded to [`Policy::MAX_GROW_PER_WORD`] (8), and the re-shape cap is 32 per
/// allowed tatweel per word, at least 64: 64 for Minimal and Balanced (as `plan`), 96 to 160 for 3
/// to 5 per word (justified lines), 256 for the elongation of a line that isn't justified. A line
/// never costs more than 256 re-shapes.
///
/// `plan` is generic over the width type (the COSMIC path measures in f32); hosts here measure in
/// f64 and this takes f64 only. Its comparisons use the same 0.001 epsilon; only widths within
/// an f32 ulp of a bound could compare differently than in an f32 host, which no caller here is.
pub fn plan_with<T>(
    text: &str,
    range: std::ops::Range<usize>,
    natural: f64,
    safe: std::collections::HashSet<usize>,
    target: f64,
    policy: Policy,
    mut reshape: impl FnMut(&[usize]) -> Option<Measured<f64, T>>,
) -> Result<Plan<f64, T>, &'static str> {
    use std::collections::BTreeMap;
    let all = kashida_candidates(text, range)?;
    let mut width = natural;
    let per_word = policy.per_word.min(Policy::MAX_GROW_PER_WORD);
    let fraction = if policy.fraction.is_finite() { policy.fraction.clamp(0.0, 1.0) } else { 0.0 };
    let budget = (target - natural).max(0.0) * fraction;
    let epsilon = 0.001;
    let mut safe = if per_word > 0 { safe } else { Default::default() };
    // Each word's boundaries (by the byte it starts at: reading order), best first.
    let mut words: BTreeMap<usize, Vec<Candidate>> = BTreeMap::new();
    if per_word > 0 {
        for c in all.into_iter().filter(|c| safe.contains(&c.byte)) {
            words.entry(c.word).or_default().push(c);
        }
    }
    for slots in words.values_mut() {
        slots.sort_unstable_by_key(|c| (c.priority, c.byte));
    }
    // At most 32 × 8 = 256 (see above).
    let cap = 32 * per_word.max(2);
    let mut inserted: Vec<usize> = Vec::new();
    let mut payload = None;
    let mut attempts = 0;
    'rounds: for _ in 0..per_word {
        let mut kept_any = false;
        for slots in words.values() {
            for c in slots {
                if attempts >= cap {
                    break 'rounds;
                }
                if !safe.contains(&c.byte) {
                    continue;
                }
                let mut trial = inserted.clone();
                trial.push(c.byte);
                trial.sort_unstable();
                attempts += 1;
                let Some(measured) = reshape(&trial) else { continue };
                if measured.width > width + epsilon
                    && measured.width <= natural + budget + epsilon
                    && measured.width <= target + epsilon
                    && !measured.missing_glyph
                {
                    width = measured.width;
                    safe = measured.safe;
                    payload = Some(measured.payload);
                    inserted = trial;
                    kept_any = true;
                    // One a word a round: the next word's turn.
                    break;
                }
            }
        }
        if !kept_any {
            break;
        }
    }
    Ok(Plan { slots: inserted, width, payload })
}

/// The boundaries of `text[range]` where kashida may go: `varos-text`'s [`candidates`] (joining
/// rules: between two joining letters, never inside لا, never next to harakat), less those in the
/// name of God ([`sacred_from`]).
pub fn kashida_candidates(text: &str, range: std::ops::Range<usize>) -> Result<Vec<Candidate>, &'static str> {
    let all = candidates(text, range.clone())?;
    let mut sacred: std::collections::HashMap<usize, Option<usize>> = std::collections::HashMap::new();
    Ok(all
        .into_iter()
        .filter(|c| {
            let from = *sacred.entry(c.word).or_insert_with(|| {
                let rest = &text[c.word..range.end.max(c.word)];
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                sacred_from(&rest[..end]).map(|off| c.word + off)
            });
            from.is_none_or(|from| c.byte <= from)
        })
        .collect())
}

/// Where the name of God begins in `word` (a byte offset into it), which no tatweel may enter:
/// `Some(0)` for a word that is the name (الله, لله, اللهم, with any of the proclitics و ف ب ت ك ل
/// أ: والله، بالله، فلله، تالله، أبالله) or holds a sacred ligature (ﷲ U+FDF2 and the others of
/// U+FDF0..U+FDFD); the offset of the name for a word that ends in it (عبدالله: of its الله).
/// `None`: kashida may go anywhere the joining rules allow. Harakat, tatweels and punctuation are
/// read through, and alef wasla (ٱ) is an alef.
pub fn sacred_from(word: &str) -> Option<usize> {
    if word.chars().any(|c| matches!(c, '\u{FDF0}'..='\u{FDFD}')) {
        return Some(0);
    }
    let letters: Vec<(usize, char)> = word
        .char_indices()
        .filter(|&(_, c)| c != TATWEEL && !is_harakat(c) && c.script() == Script::Arabic && c.is_alphabetic())
        .map(|(i, c)| (i, if c == '\u{0671}' { 'ا' } else { c }))
        .collect();
    let chars: Vec<char> = letters.iter().map(|l| l.1).collect();
    let name = if chars.ends_with(&['ل', 'ل', 'ه']) {
        3
    } else if chars.ends_with(&['ل', 'ل', 'ه', 'م']) {
        4
    } else {
        return None;
    };
    let rest = &chars[..chars.len() - name];
    if rest.len() <= 4 && rest.iter().all(|c| "وفبتكلأا".contains(*c)) {
        return Some(0);
    }
    // A longer word ending in the name (عبدالله): from its alef.
    let from = if rest.last() == Some(&'ا') { rest.len() - 1 } else { rest.len() };
    Some(letters[from].0)
}

/// The virtual elongation character.
pub const TATWEEL: char = '\u{0640}';

/// Is `c` written in a joining script whose words must not be cut or letter-spaced
/// (Arabic, Syriac, N'Ko, Mongolian)?
pub fn joining_script(c: char) -> bool {
    matches!(c.script(), Script::Arabic | Script::Syriac | Script::Nko | Script::Mongolian | Script::Hanifi_Rohingya)
}

/// Can the word `word` take kashida: it holds an Arabic letter and nothing but Arabic
/// letters, marks (inherited script), joiners (ZWJ/ZWNJ) and punctuation (`الله،`)? Digits
/// and other scripts keep the word as it is.
pub fn kashida_word(word: &str) -> bool {
    word.chars().any(|c| c.script() == Script::Arabic && c.is_alphabetic())
        && word.chars().all(|c| match c.script() {
            Script::Arabic => !c.is_numeric(),
            Script::Inherited => true,
            Script::Common => !c.is_numeric() && !c.is_whitespace(),
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn words_that_may_take_kashida() {
        assert!(kashida_word("الرحمن"));
        assert!(kashida_word("بِسْمِ"));
        assert!(kashida_word("الله،") && kashida_word("(الرحيم)"));
        assert!(!kashida_word("رقم١"), "Arabic-Indic digits");
        assert!(!kashida_word("abc"));
        assert!(!kashida_word("رقم1"));
        assert!(!kashida_word("١٢٣"), "digits are not letters");
        assert!(!kashida_word(""));
        assert!(joining_script('ب') && joining_script('ܐ') && !joining_script('a') && !joining_script('א'));
    }

    /// Strength 0, 50 and 100 are `varos-text`'s Minimal, Balanced and Display; Force goes deeper.
    #[test]
    fn strength_maps_onto_the_fixed_policies() {
        assert_eq!(policy(0, false), Policy::MINIMAL);
        assert_eq!(policy(50, false), Policy::BALANCED);
        assert_eq!(policy(100, false), Policy::DISPLAY);
        assert_eq!(policy(255, false), Policy::DISPLAY);
        let mut last = policy(0, false);
        for s in 1..=100 {
            let p = policy(s, false);
            assert!(p.per_word >= last.per_word && p.fraction >= last.fraction, "{s}: monotonic");
            let f = policy(s, true);
            assert!(f.per_word > p.per_word || f.per_word == Policy::MAX_PER_WORD);
            assert!(f.fraction >= p.fraction && f.fraction <= 1.0);
            last = p;
        }
        assert_eq!(policy(50, true), Policy { per_word: 3, fraction: 1.0 });
        assert_eq!(policy(100, true), Policy { per_word: 5, fraction: 1.0 });
    }

    /// The word each slot of `slots` is in (the byte it starts at).
    fn words_of(text: &str, slots: &[usize]) -> Vec<usize> {
        let all = candidates(text, 0..text.len()).unwrap();
        slots.iter().map(|s| all.iter().find(|c| c.byte == *s).unwrap().word).collect()
    }

    /// The numeric plan spreads the tatweels across the words, one a word a round, the words in
    /// reading order, and never in the name of God (it replaced `varos-text`'s plan, which walks
    /// the boundaries in priority order and filled the first words first: 2026-10-19).
    #[test]
    fn the_numeric_plan_spreads_across_words_and_spares_the_name() {
        let text = "بسم الله الرحمن الرحيم الحمد لله رب العالمين";
        let safe: HashSet<usize> = candidates(text, 0..text.len()).unwrap().into_iter().map(|c| c.byte).collect();
        let measure =
            |slots: &[usize]| Some(Measured { width: 300.0 + 3.0 * slots.len() as f64, safe: safe.clone(), missing_glyph: false, payload: () });
        let word_starts: Vec<usize> = std::iter::once(0).chain(text.match_indices(' ').map(|(i, _)| i + 1)).collect();
        let name = [word_starts[1], word_starts[5]];
        for (slack, p) in [(20.0, Policy::BALANCED), (60.0, policy(100, true)), (1000.0, policy(100, true)), (1000.0, growth_policy())] {
            let plan = plan_with(text, 0..text.len(), 300.0, safe.clone(), 300.0 + slack, p, measure).unwrap();
            let words = words_of(text, &plan.slots);
            assert!(words.iter().all(|w| !name.contains(w)), "slack {slack}: no tatweel in الله/لله: {words:?}");
            let mut per = std::collections::BTreeMap::<usize, usize>::new();
            for w in &words {
                *per.entry(*w).or_default() += 1;
            }
            // Balanced: no word has two while another word that may take one has none.
            let (most, least) = (per.values().max().copied().unwrap_or(0), per.values().min().copied().unwrap_or(0));
            assert!(most <= least + 1 || per.len() == 6, "slack {slack}: {per:?}");
            assert!(per.values().all(|n| *n <= p.per_word), "{per:?}");
            if slack >= 60.0 {
                assert!(per.len() >= 4, "slack {slack}: spread over {} words: {per:?}", per.len());
            }
        }
        // A small slack goes to the first words in reading order (right to left), one each.
        let plan = plan_with(text, 0..text.len(), 300.0, safe.clone(), 309.0, Policy::DISPLAY, measure).unwrap();
        assert_eq!(words_of(text, &plan.slots), vec![word_starts[0], word_starts[2], word_starts[3]]);
        // Within a word the classical order holds: the seen of بسم first.
        assert_eq!(plan.slots[0], text.find("م").unwrap());
        // Deeper policies take more of a large slack.
        let counts: Vec<usize> = [policy(0, false), policy(50, false), policy(100, false), policy(100, true), growth_policy()]
            .into_iter()
            .map(|p| plan_with(text, 0..text.len(), 300.0, safe.clone(), 400.0, p, measure).unwrap().slots.len())
            .collect();
        assert!(counts.windows(2).all(|w| w[0] <= w[1]) && counts[0] < counts[3], "{counts:?}");
    }

    /// The name of God and the sacred ligatures take no tatweel, with or without proclitics,
    /// harakat or punctuation; a longer word keeps kashida before the name only.
    #[test]
    fn the_name_of_god_takes_no_kashida() {
        for w in ["الله", "لله", "اللهم", "والله", "بالله", "فلله", "تالله", "ولله", "اللَّهِ", "لِلَّهِ", "ٱللَّهِ", "الله،", "(الله)", "ﷲ", "ﷺ", "أبالله"]
        {
            assert_eq!(sacred_from(w), Some(0), "{w}");
        }
        for w in ["الرحمن", "بسم", "اللهو", "الحمد", "العالمين", "لهم", "إله"] {
            assert_eq!(sacred_from(w), None, "{w}");
        }
        let w = "عبدالله";
        assert_eq!(sacred_from(w), Some(w.find('ا').unwrap()));
        let all = |t: &str| candidates(t, 0..t.len()).unwrap();
        let kept = |t: &str| kashida_candidates(t, 0..t.len()).unwrap();
        assert!(!all("الله").is_empty() && kept("الله").is_empty(), "the joining rules alone allow ل‍ل and ل‍ه");
        assert!(kept("بسم الله").iter().all(|c| c.byte < 7), "{:?}", kept("بسم الله"));
        assert!(kept(w).iter().all(|c| c.byte <= w.find('ا').unwrap()) && !kept(w).is_empty());
        // Lam-alef and a letter before its harakat never take one (the joining rules).
        let t = "سلام بَسَمَ";
        let lam_alef = t.find("ا").unwrap();
        assert!(kept(t).iter().all(|c| c.byte != lam_alef));
        assert!(kept(t).iter().all(|c| !is_harakat(t[c.byte..].chars().next().unwrap())));
        assert!(kept(t).iter().all(|c| !is_harakat(t[..c.byte].chars().next_back().unwrap())));
    }

    /// A line that isn't justified grows by its strength in percent, within its box.
    #[test]
    fn growth_targets() {
        assert_eq!(growth_target(200.0, 30, Some(1000.0)), 260.0);
        assert_eq!(growth_target(200.0, 30, None), 260.0);
        assert_eq!(growth_target(200.0, 300, None), 800.0);
        assert_eq!(growth_target(200.0, 300, Some(500.0)), 500.0, "clamped to the box");
        assert_eq!(growth_target(600.0, 30, Some(500.0)), 600.0, "an overflowing line doesn't grow");
        assert_eq!(growth_target(200.0, 0, Some(500.0)), 200.0);
    }

    /// The re-shape cap: 64 up to two per word, 32 per allowed tatweel above, and a policy asking
    /// for more than eight per word is held to eight (256 re-shapes at most), even when every trial
    /// is rejected.
    #[test]
    fn re_shapes_are_capped_and_per_word_is_bounded() {
        let text = "بسم الله الرحمن الرحيم الحمد لله رب العالمين ".repeat(20);
        let safe: HashSet<usize> = candidates(&text, 0..text.len()).unwrap().into_iter().map(|c| c.byte).collect();
        assert!(safe.len() > 200);
        for (per_word, cap) in [(1, 64), (2, 64), (3, 96), (4, 128), (5, 160), (8, 256), (99, 256)] {
            let mut calls = 0;
            let p = plan_with(
                &text,
                0..text.len(),
                100.0,
                safe.clone(),
                1000.0,
                Policy { per_word, fraction: 1.0 },
                |_: &[usize]| -> Option<Measured<f64, ()>> {
                    calls += 1;
                    None
                },
            )
            .unwrap();
            assert!(p.slots.is_empty());
            assert_eq!(calls, cap, "{per_word} per word");
        }
        // Accepted trials: never more than eight per word.
        let mut calls = 0;
        let p = plan_with(&text, 0..text.len(), 100.0, safe.clone(), 1e9, Policy { per_word: 99, fraction: 1.0 }, |slots: &[usize]| {
            calls += 1;
            Some(Measured { width: 100.0 + slots.len() as f64, safe: safe.clone(), missing_glyph: false, payload: () })
        })
        .unwrap();
        assert!(calls <= 256 && !p.slots.is_empty());
        let mut per: std::collections::HashMap<usize, usize> = Default::default();
        let words = candidates(&text, 0..text.len()).unwrap();
        for s in &p.slots {
            let w = words.iter().find(|c| c.byte == *s).map(|c| c.word).unwrap();
            *per.entry(w).or_default() += 1;
        }
        assert!(per.values().all(|n| *n <= Policy::MAX_GROW_PER_WORD), "{per:?}");
    }

    #[test]
    fn plan_in_points_prefers_seen_and_stops_at_the_budget() {
        let text = "بسم الله الرحمن الرحيم";
        let safe: HashSet<usize> = candidates(text, 0..text.len()).unwrap().into_iter().map(|c| c.byte).collect();
        assert!(!safe.is_empty());
        // Each tatweel adds 3 pt; slack 20 pt; Balanced uses at most 60 % of it.
        let p = plan(text, 0..text.len(), 300.0f64, safe.clone(), 320.0, POLICY, |slots| {
            Some(Measured { width: 300.0 + 3.0 * slots.len() as f64, safe: safe.clone(), missing_glyph: false, payload: slots.len() })
        })
        .unwrap();
        assert_eq!(p.slots.len(), 4);
        assert_eq!(p.width, 312.0);
        assert_eq!(p.payload, Some(4));
        // The first slot is after a seen (priority 0).
        let first = candidates(text, 0..text.len()).unwrap().into_iter().find(|c| c.priority == 0).map(|c| c.byte);
        assert!(p.slots.contains(&first.unwrap()));
        // Virtual text keeps the authored bytes apart from the tatweels.
        let w = WorkingText::new(text, &p.slots).unwrap();
        assert_eq!(w.text.chars().filter(|&c| c == TATWEEL).count(), 4);
        assert_eq!(w.text.replace(TATWEEL, ""), text);
    }
}
