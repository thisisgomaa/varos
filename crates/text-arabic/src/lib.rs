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

/// The kashida strength used for every justified right-to-left Arabic paragraph (no
/// paragraph attribute yet).
pub const POLICY: Kashida = Kashida::Balanced;

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
