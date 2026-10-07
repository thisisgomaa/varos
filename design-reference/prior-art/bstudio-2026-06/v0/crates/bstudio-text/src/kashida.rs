//! Kashida engine — the moat.
//!
//! Inserts U+0640 (TATWEEL) into an Arabic text run at positions HarfBuzz
//! reports as `SAFE_TO_INSERT_TATWEEL`, then re-runs shaping. Drops any
//! insertion that turned out unsafe after re-shaping (defence in depth).
//!
//! Sources:
//! - HarfBuzz PR #3762 (HB v5.1.0, July 2022) — the `safe_to_insert_tatweel`
//!   API itself.
//! - Andreas Hallberg, *Stretchable kashida and Arabic text justification
//!   in LaTeX* — http://andreasmhallberg.github.io/stretchable-kashida/.
//!   We port his joining-class pair filter as a stylistic guard on top of
//!   the structural HarfBuzz flag.
//! - Abdul Rahman Sibahi, *Thoughts on Arabic Justification* —
//!   https://ar-ms.me/thoughts/practical-arabic-justification/. Algorithm
//!   priority: jalt > kashida > inter-word.
//! - W3C ALReq §4.2 — https://www.w3.org/TR/alreq/.

use crate::{shape_run_weighted, ShapedRun, TextDirection};
#[cfg(test)]
use crate::shape_run;

/// U+0640 ARABIC TATWEEL — the kashida elongation character.
const TATWEEL: char = '\u{0640}';
/// UTF-8 byte length of [`TATWEEL`] (0xD9 0x80).
const TATWEEL_LEN: usize = 2;

/// Upper bound on tatweels inserted into a single run — far above any real
/// manual or justify request. A guard so a pathological / corrupted stored
/// count can't allocate a giant working string.
const MAX_KASHIDA: usize = 4096;

/// Hallberg's joining classes — letters that may flank a kashida insertion.
/// Source: `kashida-justification.sty`, `\XeTeXintercharclass` definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinClass {
    /// "Connects both ways" — can follow a connection. 22 letters.
    /// ي ئ ه ش س ق ف غ ع ض ص ن م ك ظ ط خ ح ج ث ت ب
    ConnectsBoth,
    /// "Connects on the left but breaks the join after." 7 letters.
    /// و ؤ ذ د ز ر ة
    ConnectsLeft,
    /// Alif variants. 4 letters. ا أ إ آ
    Alif,
    /// The single letter lām. ل
    Lam,
    /// Anything else — whitespace, punctuation, diacritics, non-Arabic.
    Other,
}

/// Classify one character using Hallberg's table. Diacritics are `Other`.
pub fn classify(ch: char) -> JoinClass {
    match ch {
        // ConnectsBoth (22)
        'ي' | 'ئ' | 'ه' | 'ش' | 'س' | 'ق' | 'ف' | 'غ' | 'ع' | 'ض' | 'ص'
        | 'ن' | 'م' | 'ك' | 'ظ' | 'ط' | 'خ' | 'ح' | 'ج' | 'ث' | 'ت' | 'ب' => JoinClass::ConnectsBoth,
        // ConnectsLeft (7)
        'و' | 'ؤ' | 'ذ' | 'د' | 'ز' | 'ر' | 'ة' => JoinClass::ConnectsLeft,
        // Alif (4)
        'ا' | 'أ' | 'إ' | 'آ' => JoinClass::Alif,
        // Lam (1)
        'ل' => JoinClass::Lam,
        _ => JoinClass::Other,
    }
}

/// True if `c` is an Arabic combining mark (harakat / tashkil / Quranic
/// annotation) that sits *on* a base letter rather than beside it. A tatweel
/// must never be inserted at a join that carries one of these marks: the
/// elongation stroke would slide the mark off its base and visually detach it.
///
/// Covers the main harakat block U+064B..U+065F (tanwin..mark) - which
/// includes shadda U+0651 and sukun U+0652 - the superscript alef U+0670, and
/// the common Quranic annotation marks in the U+06D6..U+06ED range.
pub fn is_harakat(c: char) -> bool {
    matches!(
        c,
        '\u{064B}'..='\u{065F}'   // tanwin, fatha, kasra, damma, shadda, sukun, ...
            | '\u{0670}'            // superscript alef
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E4}'
            | '\u{06E7}'..='\u{06E8}'
            | '\u{06EA}'..='\u{06ED}'
    )
}

/// Hallberg's permitted-pair table. Returns true if a kashida is *stylistically*
/// allowed between a `prev` letter and a `next` letter in source order
/// (logical, not visual).
///
/// We never insert kashida before `Alif` if the previous letter is `Lam` —
/// that would split the mandatory lām-alif ligature. This is the rule that
/// every existing tool (Adobe, Figma, Word) gets wrong.
pub fn hallberg_allows(prev: JoinClass, next: JoinClass) -> bool {
    use JoinClass::*;
    match (prev, next) {
        (ConnectsBoth, ConnectsBoth) => true,
        (ConnectsBoth, ConnectsLeft) => true,
        (ConnectsBoth, Alif) => true,
        (ConnectsBoth, Lam) => true,
        (Lam, Lam) => true,
        (Lam, ConnectsBoth) => true,
        (Lam, ConnectsLeft) => true,
        // Forbidden — the rule everyone gets wrong:
        (Lam, Alif) => false,
        // Any other pair: forbidden.
        _ => false,
    }
}

/// Result of one kashida insertion pass.
#[derive(Debug, Clone)]
pub struct KashidaResult {
    /// Re-shaped run after inserting U+0640 at the selected positions.
    pub run: ShapedRun,
    /// Source-order byte positions where U+0640 was inserted.
    pub inserted_at_byte_offsets: Vec<usize>,
    /// Number of insertions that were dropped after re-shaping because they
    /// produced new unsafe_to_concat regions (we keep our promise: zero
    /// broken ligatures).
    pub dropped_unsafe: usize,
}

/// Build a working string with a TATWEEL inserted at each original byte
/// `offset`. `offsets` must be sorted ascending, unique, and on char
/// boundaries. The tatweel that targets `offsets[i]` ends up at working-string
/// byte offset `offsets[i] + i * TATWEEL_LEN` (each earlier tatweel shifts it).
fn insert_tatweels(text: &str, offsets: &[usize]) -> String {
    let mut working = String::with_capacity(text.len() + offsets.len() * TATWEEL_LEN);
    let mut last = 0;
    for &o in offsets {
        working.push_str(&text[last..o]);
        working.push(TATWEEL);
        last = o;
    }
    working.push_str(&text[last..]);
    working
}

/// Rebuild the WORKING string a kashida result shaped against: the original
/// text with one tatweel inserted at each reported byte offset (offsets are in
/// the ORIGINAL string's byte space; duplicates mean stacked tatweels).
pub fn working_text(original: &str, inserted_at: &[usize]) -> String {
    if inserted_at.is_empty() {
        return original.to_string();
    }
    let mut offs: Vec<usize> = inserted_at.to_vec();
    offs.sort_unstable();
    let mut out = String::with_capacity(original.len() + offs.len() * 2);
    let mut oi = 0usize;
    for (b, ch) in original.char_indices() {
        while oi < offs.len() && offs[oi] == b {
            out.push('\u{0640}');
            oi += 1;
        }
        out.push(ch);
    }
    while oi < offs.len() {
        out.push('\u{0640}');
        oi += 1;
    }
    out
}


/// Inserts up to `n_kashida` U+0640 characters into `text` and re-shapes,
/// guaranteeing every surviving insertion is shaping-safe.
///
/// Strategy:
/// 1. Shape the original text and harvest harfrust's per-glyph
///    `safe_to_insert_tatweel` flag.
/// 2. Map each safe glyph back to its source byte cluster.
/// 3. Filter through Hallberg's pair table (defence in depth — guards the
///    classes HarfBuzz isn't asked to consider, like the lām-alif case).
/// 4. **Drop-unsafe pass (the real guarantee, not a counter).** Each candidate
///    is verified individually: insert one U+0640 there, re-shape, and require
///    the run to get *wider*. A tatweel is a horizontal stroke with positive
///    advance, so a sound elongation always increases total advance. When an
///    insertion instead *shatters a mandatory ligature* (e.g. lām-alif), the
///    font drops the compact ligature glyph and the run gets **narrower** —
///    that is the signal we reject on. Candidates that fail are counted in
///    `dropped_unsafe` and never used.
///
///    Why advance, not glyph-id stability or `unsafe_to_concat`: a valid kashida
///    *does* swap in elongated variant glyphs, so "the flanking glyph id changed"
///    is true for good and bad insertions alike (verified empirically — bāʼ-bāʼ
///    elongation changes both flanks yet is perfectly valid). And Amiri marks
///    nearly every glyph in a short run `unsafe_to_concat`. Run width is the one
///    font-agnostic measure that separates a real elongation from a collapse.
/// 5. Pick up to `n_kashida` of the verified-safe positions, evenly spaced,
///    insert them together, and re-shape once for the final run.
pub fn insert_kashida(
    font_bytes: &[u8],
    text: &str,
    n_kashida: usize,
) -> Result<KashidaResult, &'static str> {
    insert_kashida_weighted(font_bytes, text, n_kashida, None)
}

/// Like [`insert_kashida`], but shapes every internal pass at the supplied
/// variable-font `wght` location (see [`crate::shape_run_weighted`]). Threading
/// the weight here matters because the safe-position harvest, the per-candidate
/// "does it widen?" drop-unsafe test, and the final re-shape all use advances
/// that must be measured at the *rendered* weight — at a heavier `wght` the
/// run is wider and harakat sit differently, so a 400-weight measurement would
/// mis-place kashida and diacritics on a Bold line.
///
/// `wght == None` is byte-for-byte identical to [`insert_kashida`]: it threads
/// `None` through to [`crate::shape_run`] (no `ShaperInstance`).
pub fn insert_kashida_weighted(
    font_bytes: &[u8],
    text: &str,
    n_kashida: usize,
    wght: Option<f32>,
) -> Result<KashidaResult, &'static str> {
    if n_kashida == 0 {
        let run = shape_run_weighted(font_bytes, text, TextDirection::RightToLeft, wght)?;
        return Ok(KashidaResult {
            run,
            inserted_at_byte_offsets: Vec::new(),
            dropped_unsafe: 0,
        });
    }

    // Step 1 — shape, harvest safe positions.
    let initial = shape_run_weighted(font_bytes, text, TextDirection::RightToLeft, wght)?;

    // Pre-compute char-classified letters from source text.
    // We need a char-by-byte-offset map so we can look up "what came before
    // byte offset X" cheaply.
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    // Step 2-3 — collect candidate byte offsets.
    // `cluster` on a ShapedGlyph is the byte offset in the source string.
    let mut candidates: Vec<usize> = Vec::new();
    for g in &initial.glyphs {
        if !g.safe_to_insert_tatweel {
            continue;
        }
        let cluster_byte = g.cluster as usize;
        // Identify the prev / next characters in source order.
        let next_idx = chars.iter().position(|(b, _)| *b == cluster_byte);
        let Some(ni) = next_idx else { continue };
        if ni == 0 {
            // Beginning of text — no kashida before the first letter.
            continue;
        }
        let (_, prev_ch) = chars[ni - 1];
        let (_, next_ch) = chars[ni];

        // Diacritic (harakat) guard: never insert a tatweel at a join that
        // bears a combining mark. The candidate boundary sits between the
        // cluster ending at `prev_ch` and the one starting at `next_ch`; if
        // either side carries a mark, an elongation stroke here would slide
        // the mark off its base and detach it. Un-vocalized text has no such
        // marks, so this is a no-op there.
        if is_harakat(prev_ch) || is_harakat(next_ch) {
            continue;
        }

        let prev_cls = classify(prev_ch);
        let next_cls = classify(next_ch);

        if !hallberg_allows(prev_cls, next_cls) {
            continue;
        }
        candidates.push(cluster_byte);
    }

    if candidates.is_empty() {
        let run = shape_run_weighted(font_bytes, text, TextDirection::RightToLeft, wght)?;
        return Ok(KashidaResult {
            run,
            inserted_at_byte_offsets: Vec::new(),
            dropped_unsafe: 0,
        });
    }

    // Step 4 — sort candidates by byte offset so even-spacing spreads picks
    // across the whole line, then verify each one individually against the
    // drop-unsafe rule (single-tatweel re-shape must widen the run).
    candidates.sort_unstable();
    candidates.dedup();

    let base_advance = initial.total_x_advance;
    let mut verified: Vec<usize> = Vec::with_capacity(candidates.len());
    for &o in &candidates {
        if elongates_safely(font_bytes, text, base_advance, o, wght)? {
            verified.push(o);
        }
    }
    let dropped = candidates.len() - verified.len();

    if verified.is_empty() {
        // Every candidate collapsed a ligature — return the original, unelongated.
        let run = shape_run_weighted(font_bytes, text, TextDirection::RightToLeft, wght)?;
        return Ok(KashidaResult {
            run,
            inserted_at_byte_offsets: Vec::new(),
            dropped_unsafe: dropped,
        });
    }

    // Step 5 — distribute n_kashida tatweels across the verified-safe positions
    // and re-shape once. Up to one-per-position spreads them across the line
    // (classic kashida); any surplus is *stacked* (several tatweels per join) so
    // a short line keeps elongating instead of freezing once every safe join
    // already holds one. That "one tatweel per join" ceiling was the old bug:
    // a line with ~10 safe joins stopped widening at 10 regardless of the count.
    let want = n_kashida.min(MAX_KASHIDA);
    let picks = distribute(&verified, want);
    let working = insert_tatweels(text, &picks);
    let run = shape_run_weighted(font_bytes, &working, TextDirection::RightToLeft, wght)?;

    Ok(KashidaResult {
        run,
        inserted_at_byte_offsets: picks,
        dropped_unsafe: dropped,
    })
}

/// The drop-unsafe test for a single candidate: insert one tatweel at `offset`,
/// re-shape, and report whether the run got wider than `base_advance`.
///
/// A tatweel is a horizontal connecting stroke with strictly positive advance,
/// so a sound elongation always widens the run. The one case where it does not
/// is when the insertion shatters a compact mandatory ligature (lām-alif): the
/// font discards the tight ligature glyph, and the looser replacement plus
/// tatweel is *narrower* than the original. We reject exactly that case.
pub(crate) fn elongates_safely(
    font_bytes: &[u8],
    text: &str,
    base_advance: i32,
    offset: usize,
    wght: Option<f32>,
) -> Result<bool, &'static str> {
    let working = insert_tatweels(text, &[offset]);
    let reshaped = shape_run_weighted(font_bytes, &working, TextDirection::RightToLeft, wght)?;
    Ok(reshaped.total_x_advance > base_advance)
}

/// Distributes `n` tatweels across the ascending `pool` of verified-safe
/// offsets, returned as a sorted-ascending offset list (repeats adjacent) ready
/// for [`insert_tatweels`]. The requested count is honoured exactly: the result
/// always has `n` entries (given a non-empty pool).
///
/// * `n <= pool.len()` → `n` evenly-spread positions, one tatweel each (classic
///   kashida spread across the whole line).
/// * `n >  pool.len()` → every position carries `n / pool.len()` tatweels and
///   the remainder `n % pool.len()` is spread evenly as a +1, so the run keeps
///   widening past the safe-join count instead of saturating there.
fn distribute(pool: &[usize], n: usize) -> Vec<usize> {
    if n == 0 || pool.is_empty() {
        return Vec::new();
    }
    let len = pool.len();
    let base = n / len;
    let rem = n % len;
    // Mark exactly `rem` distinct positions to receive the +1, centred and
    // evenly spread: idx = (i*len + len/2) / rem is strictly increasing in i for
    // rem <= len (each step advances by >= len/rem >= 1), so indices are unique.
    let mut extra = vec![false; len];
    for i in 0..rem {
        extra[(i * len + len / 2) / rem] = true;
    }
    let mut out = Vec::with_capacity(n);
    for (i, &off) in pool.iter().enumerate() {
        for _ in 0..(base + extra[i] as usize) {
            out.push(off);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMIRI_REGULAR: &[u8] =
        include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");

    #[test]
    fn classify_basics() {
        assert_eq!(classify('ل'), JoinClass::Lam);
        assert_eq!(classify('ا'), JoinClass::Alif);
        assert_eq!(classify('ب'), JoinClass::ConnectsBoth);
        assert_eq!(classify('د'), JoinClass::ConnectsLeft);
        assert_eq!(classify(' '), JoinClass::Other);
        assert_eq!(classify('A'), JoinClass::Other);
    }

    #[test]
    fn is_harakat_recognizes_combining_marks() {
        // The vowel/tashkil marks and shadda/sukun (inside U+064B..U+0652).
        assert!(is_harakat('\u{064B}'), "fathatan");
        assert!(is_harakat('\u{064E}'), "fatha");
        assert!(is_harakat('\u{064F}'), "damma");
        assert!(is_harakat('\u{0650}'), "kasra");
        assert!(is_harakat('\u{0651}'), "shadda");
        assert!(is_harakat('\u{0652}'), "sukun");
        // Superscript alef and the Quranic annotation ranges.
        assert!(is_harakat('\u{0670}'), "superscript alef");
        assert!(is_harakat('\u{06D6}'));
        assert!(is_harakat('\u{06E7}'));
        assert!(is_harakat('\u{06ED}'));
        // U+0653 (maddah) sits inside U+064B..U+065F, so it IS in our set.
        assert!(is_harakat('\u{0653}'), "maddah above (inside the main block)");
        assert!(is_harakat('\u{065F}'), "top of the main block");
        // Block-boundary sanity: just outside the ranges must be false.
        assert!(!is_harakat('\u{064A}'), "yeh (base letter, just below block)");
        assert!(!is_harakat('\u{0660}'), "arabic-indic zero (just above block)");
        assert!(!is_harakat('\u{066F}'));
        // Base letters / non-Arabic are never harakat.
        assert!(!is_harakat('\u{0628}'), "beh");
        assert!(!is_harakat('\u{0644}'), "lam");
        assert!(!is_harakat('\u{0627}'), "alef");
        assert!(!is_harakat(' '));
        assert!(!is_harakat('A'));
    }

    #[test]
    fn hallberg_protects_lam_alif() {
        // The rule everyone gets wrong:
        assert!(!hallberg_allows(JoinClass::Lam, JoinClass::Alif));
    }

    #[test]
    fn hallberg_allows_common_pairs() {
        use JoinClass::*;
        assert!(hallberg_allows(ConnectsBoth, ConnectsBoth));
        assert!(hallberg_allows(ConnectsBoth, Alif));
        assert!(hallberg_allows(Lam, Lam));
        assert!(hallberg_allows(Lam, ConnectsBoth));
        assert!(!hallberg_allows(Other, Other));
    }

    #[test]
    fn insert_zero_kashida_is_noop() {
        let text = "الحمد لله رب العالمين";
        let res = insert_kashida(AMIRI_REGULAR, text, 0).unwrap();
        assert!(res.inserted_at_byte_offsets.is_empty());
        assert_eq!(res.dropped_unsafe, 0);
    }

    #[test]
    fn insert_one_kashida_in_basmala() {
        let text = "الحمد لله رب العالمين";
        let res = insert_kashida(AMIRI_REGULAR, text, 1).unwrap();
        // Should land at least one insertion in such a long line.
        assert!(
            !res.inserted_at_byte_offsets.is_empty(),
            "expected at least one kashida placement"
        );
        // Re-shaped run should be at least as wide as the original.
        let original = shape_run(AMIRI_REGULAR, text, TextDirection::RightToLeft).unwrap();
        assert!(
            res.run.total_x_advance >= original.total_x_advance,
            "kashida insertion must not shrink the line"
        );
    }

    #[test]
    fn insert_kashida_never_splits_lam_alif() {
        // "الله" contains lām-lām-hā, and the broader Basmala has multiple
        // lām-alif sequences. We assert: after kashida insertion, the
        // word "الله" (which appears at byte offset of "ل" right before "ل ل ه")
        // does not get a kashida inserted INSIDE the lām-alif sub-sequence.
        // For Phase 0 we use the simpler proxy: no kashida should be inserted
        // immediately before an Alif if the prev letter is Lām.
        let text = "الحمد لله رب العالمين";
        let res = insert_kashida(AMIRI_REGULAR, text, 5).unwrap();

        let chars: Vec<(usize, char)> = text.char_indices().collect();
        for offset in &res.inserted_at_byte_offsets {
            let ni = chars
                .iter()
                .position(|(b, _)| *b == *offset)
                .expect("offset must align with a char boundary");
            assert!(ni > 0, "no kashida before first char");
            let (_, prev) = chars[ni - 1];
            let (_, next) = chars[ni];
            assert!(
                hallberg_allows(classify(prev), classify(next)),
                "kashida inserted in a Hallberg-forbidden pair: {:?} → {:?}",
                prev, next
            );
        }
    }

    #[test]
    fn vocalized_word_skips_harakat_joins_and_preserves_marks() {
        // A fully vocalized Basmala fragment: "bismi llahi" with fatha,
        // kasra, sukun and shadda. No tatweel may land at a join that bears
        // a combining mark (that would detach the harakat from its base),
        // and the insertion must preserve the combining-mark count exactly.
        let text = "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} \u{0627}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{0650}";
        let res = insert_kashida(AMIRI_REGULAR, text, 5).unwrap();

        let chars: Vec<(usize, char)> = text.char_indices().collect();
        for offset in &res.inserted_at_byte_offsets {
            let ni = chars
                .iter()
                .position(|(b, _)| *b == *offset)
                .expect("offset must align with a char boundary");
            assert!(ni > 0, "no kashida before first char");
            let (_, prev) = chars[ni - 1];
            let (_, next) = chars[ni];
            assert!(
                !is_harakat(prev) && !is_harakat(next),
                "kashida inserted at a harakat-bearing join: {:?} -> {:?}",
                prev, next
            );
        }

        // The elongated source string must carry exactly the same number of
        // combining marks as the input (none dropped, none orphaned).
        let marks_before = text.chars().filter(|&c| is_harakat(c)).count();
        let elongated = insert_tatweels(text, &res.inserted_at_byte_offsets);
        let marks_after = elongated.chars().filter(|&c| is_harakat(c)).count();
        assert_eq!(
            marks_before, marks_after,
            "kashida insertion must preserve the combining-mark count"
        );
    }

    // ── Forced-failure corpus (defence-in-depth proof) ──────────────────────
    //
    // These tests BYPASS the candidate filter (safe_to_insert_tatweel +
    // Hallberg) and feed a known position straight into the drop-unsafe test
    // (`elongates_safely`). They prove the second layer — the re-shape width
    // check — catches a shattered mandatory ligature on its own, so the "zero
    // broken ligatures" promise does not rest on the pre-filter alone. The
    // bad inputs are synthetic by construction: a real run through
    // `insert_kashida` would never reach the lām-alif offset (Hallberg blocks
    // it first), so this is genuine defence in depth, not the live path.

    fn advance_of(text: &str) -> i32 {
        shape_run(AMIRI_REGULAR, text, TextDirection::RightToLeft)
            .unwrap()
            .total_x_advance
    }

    #[test]
    fn forced_lam_alif_insertion_is_rejected() {
        // "لا" — lām(0) + alif(2). A tatweel at offset 2 splits the mandatory
        // lām-alif ligature; the font drops the compact ligature glyph and the
        // run gets NARROWER. The width check must reject it.
        let text = "لا";
        let base = advance_of(text);
        let safe = elongates_safely(AMIRI_REGULAR, text, base, 2, None).unwrap();
        assert!(
            !safe,
            "a tatweel inside lām-alif must be rejected (it shatters the ligature)"
        );
    }

    #[test]
    fn safe_medial_insertion_is_accepted() {
        // "ببب" — three bāʼ, all medial-connecting. A tatweel between the first
        // two (offset 2) is a textbook-safe elongation that widens the run.
        // Guards against the width check over-rejecting good positions (a valid
        // kashida does swap in variant glyphs, which a glyph-id check would
        // wrongly flag — this must still pass).
        let text = "ببب";
        let base = advance_of(text);
        let safe = elongates_safely(AMIRI_REGULAR, text, base, 2, None).unwrap();
        assert!(safe, "a safe bāʼ-bāʼ elongation must be accepted");
    }

    #[test]
    fn drop_pass_refills_to_honour_count() {
        // The Basmala has ample safe joins. Asking for several kashidas must
        // yield exactly that many placements, none inside a lām-alif.
        let text = "الحمد لله رب العالمين";
        let res = insert_kashida(AMIRI_REGULAR, text, 4).unwrap();
        assert_eq!(
            res.inserted_at_byte_offsets.len(),
            4,
            "must place the requested count when safe room exists"
        );
    }

    #[test]
    fn high_count_keeps_widening_past_safe_join_count() {
        // The "stuck at 10" bug: a short line stopped widening once every safe
        // join held one tatweel (~10 on this line), so 8 and 40 rendered
        // identically. Now the surplus stacks: 40 places exactly 40 tatweels and
        // the run is strictly wider than 8.
        let text = "الحمد لله رب العالمين";
        let few = insert_kashida(AMIRI_REGULAR, text, 8).unwrap();
        let many = insert_kashida(AMIRI_REGULAR, text, 40).unwrap();
        assert_eq!(
            many.inserted_at_byte_offsets.len(),
            40,
            "the requested count must be honoured exactly (was capped at the safe-join count)"
        );
        assert!(
            many.run.total_x_advance > few.run.total_x_advance,
            "40 kashidas must render wider than 8 — equal width was the 'stuck at 10' bug"
        );
    }
}
