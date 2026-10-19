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
pub use varos_text::kashida::{Candidate, Kashida, Measured, Plan, Width, WorkingText, candidates, joining_allows, plan};

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
    /// The most tatweels a word takes under any policy ([`policy`] with Force at 100).
    pub const MAX_PER_WORD: usize = 5;
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

/// [`plan`] with a numeric [`Policy`]: the same choice, order and checks as `varos-text`'s
/// (whose policies are fixed), so [`Policy::BALANCED`] plans exactly what `Kashida::Balanced`
/// does.
///
/// Two deliberate differences from the pinned `varos-text` `plan`, which caps every policy at 64
/// re-shapes: `per_word` is bounded to [`Policy::MAX_PER_WORD`] (5), and the re-shape cap is 32 per
/// allowed tatweel per word, at least 64: 64 for Minimal and Balanced (as `plan`), 96, 128 and 160
/// for 3, 4 and 5 per word, so the deeper policies aren't starved of trials on long lines. A line
/// never costs more than 160 re-shapes.
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
    use std::collections::HashMap;
    let all = candidates(text, range)?;
    let mut width = natural;
    let per_word = policy.per_word.min(Policy::MAX_PER_WORD);
    let fraction = if policy.fraction.is_finite() { policy.fraction.clamp(0.0, 1.0) } else { 0.0 };
    let budget = (target - natural).max(0.0) * fraction;
    let epsilon = 0.001;
    let mut safe = if per_word > 0 { safe } else { Default::default() };
    let mut slots: Vec<Candidate> = if per_word > 0 { all.into_iter().filter(|c| safe.contains(&c.byte)).collect() } else { vec![] };
    slots.sort_unstable_by_key(|c| (c.priority, c.byte));
    // At most 32 × 5 = 160 (see above).
    let cap = 32 * per_word.max(2);
    let mut word_counts = HashMap::<usize, usize>::new();
    let mut inserted: Vec<usize> = Vec::new();
    let mut payload = None;
    let mut attempts = 0;
    for _ in 0..per_word {
        for c in &slots {
            if attempts >= cap {
                break;
            }
            if word_counts.get(&c.word).copied().unwrap_or(0) >= per_word {
                continue;
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
                *word_counts.entry(c.word).or_default() += 1;
                inserted = trial;
            }
        }
    }
    Ok(Plan { slots: inserted, width, payload })
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

    /// At Balanced, the numeric plan is `varos-text`'s plan, slot for slot.
    #[test]
    fn the_numeric_plan_at_balanced_is_varos_texts_plan() {
        let text = "بسم الله الرحمن الرحيم الحمد لله رب العالمين";
        let safe: HashSet<usize> = candidates(text, 0..text.len()).unwrap().into_iter().map(|c| c.byte).collect();
        // Large slacks reach the second round, where a slot may take a second (stacked) tatweel.
        for (slack, each) in [(20.0, 3.0), (60.0, 2.5), (5.0, 4.0), (200.0, 1.0), (2000.0, 1.0)] {
            let measure = |slots: &[usize]| {
                Some(Measured { width: 300.0 + each * slots.len() as f64, safe: safe.clone(), missing_glyph: false, payload: slots.len() })
            };
            let a = plan(text, 0..text.len(), 300.0f64, safe.clone(), 300.0 + slack, Kashida::Balanced, measure).unwrap();
            let b = plan_with(text, 0..text.len(), 300.0, safe.clone(), 300.0 + slack, Policy::BALANCED, measure).unwrap();
            assert_eq!((a.slots, a.width, a.payload), (b.slots, b.width, b.payload), "slack {slack}");
            let m = plan(text, 0..text.len(), 300.0f64, safe.clone(), 300.0 + slack, Kashida::Minimal, measure).unwrap();
            let n = plan_with(text, 0..text.len(), 300.0, safe.clone(), 300.0 + slack, Policy::MINIMAL, measure).unwrap();
            assert_eq!(m.slots, n.slots, "Minimal, slack {slack}");
        }
        // Deeper policies take more of a large slack.
        let measure =
            |slots: &[usize]| Some(Measured { width: 300.0 + 2.0 * slots.len() as f64, safe: safe.clone(), missing_glyph: false, payload: () });
        let counts: Vec<usize> = [policy(0, false), policy(50, false), policy(100, false), policy(100, true)]
            .into_iter()
            .map(|p| plan_with(text, 0..text.len(), 300.0, safe.clone(), 400.0, p, measure).unwrap().slots.len())
            .collect();
        assert!(counts.windows(2).all(|w| w[0] <= w[1]) && counts[0] < counts[3], "{counts:?}");
    }

    /// The re-shape cap: 64 up to two per word, 32 per allowed tatweel above, and a policy asking
    /// for more than five per word is held to five (160 re-shapes at most), even when every trial
    /// is rejected.
    #[test]
    fn re_shapes_are_capped_and_per_word_is_bounded() {
        let text = "بسم الله الرحمن الرحيم الحمد لله رب العالمين ".repeat(20);
        let safe: HashSet<usize> = candidates(&text, 0..text.len()).unwrap().into_iter().map(|c| c.byte).collect();
        assert!(safe.len() > 200);
        for (per_word, cap) in [(1, 64), (2, 64), (3, 96), (4, 128), (5, 160), (99, 160)] {
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
        // Accepted trials: never more than five per word.
        let mut calls = 0;
        let p = plan_with(&text, 0..text.len(), 100.0, safe.clone(), 1e9, Policy { per_word: 99, fraction: 1.0 }, |slots: &[usize]| {
            calls += 1;
            Some(Measured { width: 100.0 + slots.len() as f64, safe: safe.clone(), missing_glyph: false, payload: () })
        })
        .unwrap();
        assert!(calls <= 160 && !p.slots.is_empty());
        let mut per: std::collections::HashMap<usize, usize> = Default::default();
        let words = candidates(&text, 0..text.len()).unwrap();
        for s in &p.slots {
            let w = words.iter().find(|c| c.byte == *s).map(|c| c.word).unwrap();
            *per.entry(w).or_default() += 1;
        }
        assert!(per.values().all(|n| *n <= Policy::MAX_PER_WORD), "{per:?}");
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
