//! Visual-order line shaping — the bidi-correct single-line driver.
//!
//! ## Why this module exists
//! The live inline-editor render path used to shape an *entire* line as one
//! forced-RTL run (`insert_kashida` → `shape_run(.., RightToLeft)`). That has
//! NO bidi itemization, so any embedded left-to-right token inside Arabic —
//! European digits, Arabic-Indic digits, a Latin brand name, a percentage —
//! came back in RTL visual order and rendered **reversed** ("القاهرة مصر 2026"
//! committed as "...6202").
//!
//! [`shape_visual_line`] fixes that at run granularity using the UAX #9
//! resolver that already lives in [`crate::bidi`]:
//!
//! 1. Resolve the paragraph → per-level runs in **logical** order.
//! 2. Shape each run independently in its own direction. harfrust returns each
//!    run's glyphs already in that run's own visual order (leftmost-first); we
//!    only rebase each glyph's `cluster` to a byte offset into the FULL line.
//! 3. Reorder the **run sequence** into visual order with UAX #9 rule L2
//!    (reverse maximal sub-sequences of runs with level ≥ L, for L from
//!    `max_level` down to 1). Concatenating each run's already-visual glyphs
//!    then yields the whole line in correct left-to-right visual order.
//! 4. Apply kashida **only inside RTL runs**, reusing the existing kashida
//!    machinery per-run. Pure-RTL lines behave exactly as before.
//!
//! This is **additive**: `insert_kashida`, `shape_run`, `shape_paragraph` and
//! `layout_text` are untouched. The render path in `bstudio-web` will be
//! re-wired onto this in a later mission.

use crate::bidi::{resolve_paragraph, Direction};
use crate::kashida::insert_kashida_weighted;
use crate::{shape_run_weighted, ShapedGlyph, ShapedRun};
#[cfg(test)]
use crate::kashida::insert_kashida;
#[cfg(test)]
use crate::shape_run;

/// UTF-8 byte length of U+0640 ARABIC TATWEEL — mirrors `kashida::TATWEEL_LEN`
/// (that const is private; the value 2 is structural, 0xD9 0x80).
const TATWEEL_LEN: usize = 2;

/// A single line shaped into glyphs in correct **visual order**
/// (left-to-right on screen), with proper bidi itemization.
///
/// Designed as a near drop-in for the existing render path: it exposes exactly
/// what `render_arabic_line` / `render_arabic_line_2c` read today.
///
/// * Per glyph (in [`glyphs`](Self::glyphs)): `glyph_id`, `cluster`,
///   `x_advance`, `x_offset`, `y_offset` — same [`ShapedGlyph`] type the old
///   path walked, just already concatenated in final visual L→R order.
/// * [`total_x_advance`](Self::total_x_advance) and
///   [`units_per_em`](Self::units_per_em) — same role as `ShapedRun`'s fields.
/// * [`tatweel_glyph_indices`](Self::tatweel_glyph_indices) — which glyphs in
///   [`glyphs`](Self::glyphs) are inserted kashida (U+0640), so the 2-colour
///   renderer recolours them. This is the robust replacement for the legacy
///   `cluster == off+i*2` check, which silently misses *stacked* tatweels that
///   harfrust merges into one cluster.
#[derive(Debug, Clone)]
pub struct VisualLine {
    /// Every glyph of the line, already concatenated in final visual order
    /// (leftmost glyph first). Each glyph's `cluster` is the run's local shaped
    /// byte offset plus the run's start in the full line, so callers keep
    /// glyph→source mapping. For an RTL run that received kashida this offset is
    /// in the run's *post-insertion* working-string space (the same space the
    /// legacy single-run path used), so a kashida glyph carries a cluster
    /// distinct from every base letter within its run.
    pub glyphs: Vec<ShapedGlyph>,
    /// Units per em of the font that produced the line.
    pub units_per_em: u32,
    /// Sum of every glyph's `x_advance`, in font units — the line's painted
    /// width before scaling. (Equals the old `run.total_x_advance` for a line.)
    pub total_x_advance: i32,
    /// Base direction of the paragraph (UAX #9 P2/P3). Informational; the glyph
    /// stream is already absolute L→R so the renderer no longer needs it to
    /// decide a walk direction.
    pub base_direction: Direction,
    /// Ascending indices into [`glyphs`](Self::glyphs) of the glyphs that are an
    /// inserted kashida (tatweel, U+0640). The 2-colour renderer walks glyphs by
    /// index and recolours those in this set. Font-independent and correct under
    /// stacked tatweels (derived from working-string byte coverage, not a single
    /// cluster value that GSUB/cluster-merge can move).
    pub tatweel_glyph_indices: Vec<usize>,
}

/// Shape a SINGLE line (must not contain `'\n'`) into glyphs in correct visual
/// (left-to-right) order using proper UAX #9 bidi, applying kashida only inside
/// RTL runs.
///
/// `n_kashida` is the total tatweel budget requested for the line; it is
/// distributed across the RTL run(s) (largest-RTL-run-first). Pure-RTL lines
/// receive the full budget on their single run and therefore behave exactly as
/// `insert_kashida(font, line, n_kashida)` did before.
///
/// Returns [`VisualLine`] — see its docs for the field↔renderer mapping.
pub fn shape_visual_line(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
) -> Result<VisualLine, &'static str> {
    shape_visual_line_weighted(font_bytes, line, n_kashida, None)
}

/// Like [`shape_visual_line`], but shapes at the given variable-font `wght`
/// location (see [`crate::shape_run_weighted`]). Every per-run shaping pass —
/// the LTR/no-kashida `shape_run` and the RTL kashida machinery — is run at
/// `wght`, so the advances summed into `total_x_advance`, the per-glyph
/// `x_advance`/`x_offset`/`y_offset` (GPOS marks) and the kashida placement all
/// match the weight the outline renderer draws at. This is the fix for the
/// Medium/Bold drift + mis-stacked harakat described in RESEARCH/03 §1.5.
///
/// `wght == None` is byte-for-byte identical to [`shape_visual_line`]: it
/// threads `None` to [`crate::shape_run_weighted`] /
/// [`crate::kashida::insert_kashida_weighted`], attaching no `ShaperInstance`.
pub fn shape_visual_line_weighted(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    wght: Option<f32>,
) -> Result<VisualLine, &'static str> {
    shape_visual_line_tracked(font_bytes, line, n_kashida, wght, 0.0)
}

/// [`shape_visual_line_weighted`] + joining-safe TRACKING (spec 001 W4).
/// `tracking_em_millis` widens non-joining boundaries only — applied PER RUN
/// while clusters still index the run's own working string (original slice +
/// inserted tatweels), the only space where the boundary lookup is exact.
pub fn shape_visual_line_tracked(
    font_bytes: &[u8],
    line: &str,
    n_kashida: usize,
    wght: Option<f32>,
    tracking_em_millis: f32,
) -> Result<VisualLine, &'static str> {
    let res = resolve_paragraph(line);

    // Empty / whitespace-only logical resolution → empty visual line.
    if res.runs.is_empty() {
        // Still report a real units_per_em so callers can scale safely.
        let upem = units_per_em_of(font_bytes)?;
        return Ok(VisualLine {
            glyphs: Vec::new(),
            units_per_em: upem,
            total_x_advance: 0,
            base_direction: res.base_direction,
            tatweel_glyph_indices: Vec::new(),
        });
    }

    // Decide how many kashidas each RTL run gets (largest RTL run first).
    let per_run_kashida = distribute_kashida(line, &res.runs, n_kashida);

    // --- Step 2: shape every run in logical order, rebasing clusters. ---
    // Per run we keep: its level, its glyphs (already in the run's own visual
    // order, clusters rebased into the full line), and which of those glyphs
    // are inserted-tatweel (kashida) glyphs.
    struct ShapedBidiRun {
        level: u8,
        glyphs: Vec<ShapedGlyph>,
        /// Indices into `glyphs` (this run's local vec) that are tatweel glyphs.
        tatweel_local: Vec<usize>,
    }

    let mut units_per_em = 0u32;
    let mut shaped_runs: Vec<ShapedBidiRun> = Vec::with_capacity(res.runs.len());

    for (i, run) in res.runs.iter().enumerate() {
        let slice = &line[run.byte_range.clone()];
        let base = run.byte_range.start;

        if slice.is_empty() {
            shaped_runs.push(ShapedBidiRun {
                level: run.level,
                glyphs: Vec::new(),
                tatweel_local: Vec::new(),
            });
            continue;
        }

        let want = per_run_kashida[i];

        // Shape the run. For an RTL run with budget we run the full kashida
        // machinery on the run slice; `inserted_orig_offsets` are insertion
        // points in the run-slice's own byte space (what `insert_kashida`
        // reports). The returned glyph clusters index the WORKING string
        // (slice + inserted tatweels); we keep that space and add `base`.
        let (mut shaped, inserted_orig_offsets): (ShapedRun, Vec<usize>) =
            if run.direction == Direction::Rtl && want > 0 {
                let kr = insert_kashida_weighted(font_bytes, slice, want, wght)?;
                (kr.run, kr.inserted_at_byte_offsets)
            } else {
                // LTR run, or RTL run with no kashida budget: plain shape in the
                // run's own direction. No tatweels → working == original space.
                let r = shape_run_weighted(font_bytes, slice, run.direction.into(), wght)?;
                (r, Vec::new())
            };
        // Tracking (spec 001 W4): applied HERE, while clusters index the run's
        // working string — the only space where the boundary lookup is exact
        // even under kashida insertion.
        if tracking_em_millis != 0.0 {
            let working = crate::kashida::working_text(slice, &inserted_orig_offsets);
            let added = crate::apply_tracking_glyphs(
                &mut shaped.glyphs,
                shaped.units_per_em,
                &working,
                tracking_em_millis,
            );
            shaped.total_x_advance += added;
        }

        if units_per_em == 0 {
            units_per_em = shaped.units_per_em;
        }

        // Identify tatweel glyphs in WORKING-string cluster space BEFORE
        // rebasing — robust to harfrust merging stacked tatweels into one
        // cluster (a per-cluster `off+i*2` match silently misses the merged
        // ones; byte coverage does not). See `tatweel_glyphs_by_coverage`.
        let tatweel_local =
            tatweel_glyphs_by_coverage(&shaped.glyphs, &inserted_orig_offsets, run.direction);

        // Rebase glyph clusters (working-string space) into the FULL line by
        // adding the run's start. Clusters of run `i` stay confined to
        // `[base, base + working_len_i)`, so runs never collide in cluster space
        // and the renderer can still map a glyph to its run.
        let mut glyphs = shaped.glyphs;
        for g in &mut glyphs {
            g.cluster += base as u32;
        }

        shaped_runs.push(ShapedBidiRun {
            level: run.level,
            glyphs,
            tatweel_local,
        });
    }

    if units_per_em == 0 {
        units_per_em = units_per_em_of(font_bytes)?;
    }

    // --- Step 3: reorder the RUN SEQUENCE into visual order (UAX #9 L2). ---
    // Operate on an index permutation; reverse maximal contiguous spans whose
    // run level >= L, for L from max_level down to 1.
    let mut order: Vec<usize> = (0..shaped_runs.len()).collect();
    let max_level = shaped_runs.iter().map(|r| r.level).max().unwrap_or(0);
    for lvl in (1..=max_level).rev() {
        reverse_runs_at_level(&mut order, &shaped_runs, lvl, |r| r.level);
    }

    // --- Concatenate each run's (already-visual) glyphs in the visual run
    // order, translating each run's local tatweel indices to FINAL indices. ---
    let mut glyphs: Vec<ShapedGlyph> = Vec::new();
    let mut tatweel_glyph_indices: Vec<usize> = Vec::new();
    for &idx in &order {
        let offset = glyphs.len();
        for &li in &shaped_runs[idx].tatweel_local {
            tatweel_glyph_indices.push(offset + li);
        }
        glyphs.extend_from_slice(&shaped_runs[idx].glyphs);
    }

    let total_x_advance: i32 = glyphs.iter().map(|g| g.x_advance).sum();

    Ok(VisualLine {
        glyphs,
        units_per_em,
        total_x_advance,
        base_direction: res.base_direction,
        tatweel_glyph_indices,
    })
}

/// Identify which glyphs of a freshly-shaped (possibly kashida-injected) run are
/// inserted-tatweel glyphs, returned as ascending indices into `glyphs`.
///
/// `inserted_orig_offsets` are the tatweel insertion points in the run slice's
/// ORIGINAL byte space (as `insert_kashida` reports them); the i-th insertion
/// lands at WORKING-string byte `off_i + i*TATWEEL_LEN` (each earlier tatweel
/// shifts later ones). A glyph "covers" the half-open working-byte span from its
/// own cluster to the next cluster boundary in *logical* order; a glyph is a
/// tatweel glyph iff its coverage contains an inserted-tatweel byte.
///
/// Using byte coverage (not a direct `cluster == off+i*2` test) is what makes
/// this correct when harfrust merges several **stacked** tatweels at one join
/// into a single glyph cluster — the merged glyph still covers all their bytes.
fn tatweel_glyphs_by_coverage(
    glyphs: &[ShapedGlyph],
    inserted_orig_offsets: &[usize],
    direction: Direction,
) -> Vec<usize> {
    if inserted_orig_offsets.is_empty() || glyphs.is_empty() {
        return Vec::new();
    }
    // Working-string byte position of each inserted tatweel.
    let tatweel_bytes: Vec<usize> = inserted_orig_offsets
        .iter()
        .enumerate()
        .map(|(k, &off)| off + k * TATWEEL_LEN)
        .collect();

    // Cluster boundaries in logical order. harfrust emits RTL glyphs in visual
    // (leftmost-first) order, i.e. logical order reversed; LTR glyphs are
    // already in logical order. Build the logical-ordered cluster list so each
    // glyph's coverage is [cluster, next_logical_cluster).
    let n = glyphs.len();
    let logical: Vec<usize> = match direction {
        Direction::Rtl => (0..n).rev().collect(),
        Direction::Ltr => (0..n).collect(),
    };

    // For each glyph, coverage end = the next distinct higher cluster among all
    // glyphs (since stacked tatweels can share a cluster, "next in logical
    // order" can equal this cluster; use the next strictly-greater boundary).
    let max_cluster = glyphs.iter().map(|g| g.cluster as usize).max().unwrap_or(0);
    let mut is_tat = vec![false; n];
    for (pos, &gi) in logical.iter().enumerate() {
        let start = glyphs[gi].cluster as usize;
        // Find the next cluster strictly greater than `start` walking forward in
        // logical order; if none, coverage runs to one past the max cluster.
        let mut end = max_cluster + TATWEEL_LEN; // generous upper bound
        for &gj in &logical[pos + 1..] {
            let c = glyphs[gj].cluster as usize;
            if c > start {
                end = c;
                break;
            }
        }
        if tatweel_bytes.iter().any(|&t| t >= start && t < end) {
            is_tat[gi] = true;
        }
    }

    (0..n).filter(|&i| is_tat[i]).collect()
}

/// UAX #9 rule L2, one level pass over a run-index permutation: reverse every
/// maximal contiguous sub-sequence of runs whose level `>= lvl`.
///
/// `order` is the current visual permutation of run indices; `level_of` reads a
/// run's resolved level. Generic over the run payload so it is unit-testable.
fn reverse_runs_at_level<R>(
    order: &mut [usize],
    runs: &[R],
    lvl: u8,
    level_of: impl Fn(&R) -> u8,
) {
    let n = order.len();
    let mut i = 0;
    while i < n {
        if level_of(&runs[order[i]]) >= lvl {
            let start = i;
            let mut j = i + 1;
            while j < n && level_of(&runs[order[j]]) >= lvl {
                j += 1;
            }
            order[start..j].reverse();
            i = j;
        } else {
            i += 1;
        }
    }
}

/// Split a total kashida budget across the resolved runs, charging only RTL
/// runs and favouring the run with the most byte content (a reasonable proxy
/// for "the run with the most safe joins"). Visual-order correctness is the
/// priority; justify fidelity is secondary, so a simple largest-run-first
/// spread is sufficient.
///
/// Returns a per-run vector aligned with `runs` (LTR runs always get 0).
fn distribute_kashida(line: &str, runs: &[crate::bidi::BidiRun], total: usize) -> Vec<usize> {
    let mut out = vec![0usize; runs.len()];
    if total == 0 {
        return out;
    }

    // Indices of RTL runs, sorted by descending byte length (largest first).
    let mut rtl: Vec<usize> = runs
        .iter()
        .enumerate()
        .filter(|(_, r)| r.direction == Direction::Rtl && !line[r.byte_range.clone()].is_empty())
        .map(|(i, _)| i)
        .collect();
    if rtl.is_empty() {
        return out;
    }
    rtl.sort_by_key(|&i| core::cmp::Reverse(runs[i].byte_range.len()));

    // Proportional spread by byte length, with any rounding remainder handed to
    // the largest run so the requested total is honoured exactly.
    let total_len: usize = rtl.iter().map(|&i| runs[i].byte_range.len()).sum();
    let mut assigned = 0usize;
    for (k, &i) in rtl.iter().enumerate() {
        let share = if k + 1 == rtl.len() {
            total - assigned
        } else {
            // floor(total * len_i / total_len)
            (total * runs[i].byte_range.len()) / total_len.max(1)
        };
        out[i] = share;
        assigned += share;
    }
    out
}

/// Read a font's units-per-em without shaping anything — used for empty lines.
fn units_per_em_of(font_bytes: &[u8]) -> Result<u32, &'static str> {
    use harfrust::{FontRef, ShaperData};
    let font = FontRef::new(font_bytes).map_err(|_| "failed to parse font")?;
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    Ok(shaper.units_per_em() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidi::Direction;
    use crate::TextDirection;

    // Reuse the canonical Phase-0 test font, loaded exactly like the layout /
    // bidi_corpus tests do (relative to this crate's `src/` directory).
    const AMIRI_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");

    /// Running-x position of each glyph (font units), leftmost glyph at x=0.
    /// Pairs each glyph with the x of its LEFT edge in the final visual stream.
    fn xs(line: &VisualLine) -> Vec<(usize, i32)> {
        let mut out = Vec::with_capacity(line.glyphs.len());
        let mut x = 0i32;
        for g in &line.glyphs {
            out.push((g.cluster as usize, x));
            x += g.x_advance;
        }
        out
    }

    /// All glyphs whose source byte offset falls inside `range`, paired with
    /// their left-edge x, in stream order.
    fn glyphs_in_range(
        line: &VisualLine,
        range: core::ops::Range<usize>,
    ) -> Vec<(usize, i32)> {
        xs(line)
            .into_iter()
            .filter(|(c, _)| range.contains(c))
            .collect()
    }

    fn byte_range_of<'a>(haystack: &'a str, needle: &str) -> core::ops::Range<usize> {
        let start = haystack.find(needle).expect("substring present");
        start..start + needle.len()
    }

    /// True if `v` is strictly ascending in its first tuple element.
    fn ascending_cluster(v: &[(usize, i32)]) -> bool {
        v.windows(2).all(|w| w[0].0 < w[1].0)
    }
    /// True if `v` is strictly descending in its first tuple element.
    fn descending_cluster(v: &[(usize, i32)]) -> bool {
        v.windows(2).all(|w| w[0].0 > w[1].0)
    }
    /// True if `v` is non-decreasing in x (the stream is already L→R so x only
    /// ever advances; equal advances of 0-width marks are tolerated).
    fn ascending_x(v: &[(usize, i32)]) -> bool {
        v.windows(2).all(|w| w[0].1 <= w[1].1)
    }

    #[test]
    fn empty_line_is_empty() {
        let vl = shape_visual_line(AMIRI_REGULAR, "", 0).unwrap();
        assert!(vl.glyphs.is_empty());
        assert_eq!(vl.total_x_advance, 0);
        assert!(vl.units_per_em > 0, "must still report a real upem");
    }

    #[test]
    fn pure_latin_is_left_to_right() {
        let line = "Hello";
        let vl = shape_visual_line(AMIRI_REGULAR, line, 0).unwrap();
        let v = xs(&vl);
        assert!(!v.is_empty());
        // Latin: cluster ascends as x ascends (reads left-to-right).
        assert!(ascending_cluster(&v), "Latin clusters must ascend L→R: {v:?}");
        assert!(ascending_x(&v));
        assert_eq!(vl.base_direction, Direction::Ltr);
    }

    #[test]
    fn pure_arabic_is_right_to_left() {
        let line = "السلام عليكم";
        let vl = shape_visual_line(AMIRI_REGULAR, line, 0).unwrap();
        // Compare against the legacy single-run RTL shaping: a pure-RTL line
        // must be byte-for-byte the same glyph stream as before (no regression).
        let legacy = shape_run(AMIRI_REGULAR, line, TextDirection::RightToLeft).unwrap();
        assert_eq!(
            vl.glyphs.len(),
            legacy.glyphs.len(),
            "pure-RTL glyph count must equal legacy single-run shaping"
        );
        for (a, b) in vl.glyphs.iter().zip(legacy.glyphs.iter()) {
            assert_eq!(a.glyph_id, b.glyph_id, "pure-RTL must match legacy glyph ids");
            assert_eq!(a.cluster, b.cluster, "pure-RTL must match legacy clusters");
        }
        assert_eq!(vl.total_x_advance, legacy.total_x_advance);
        assert_eq!(vl.base_direction, Direction::Rtl);

        // And the Arabic letters read RTL: cluster descends as x ascends.
        let v = xs(&vl);
        assert!(descending_cluster(&v), "Arabic clusters must descend as x rises: {v:?}");
        assert!(ascending_x(&v));
    }

    #[test]
    fn arabic_with_european_digits_orders_visually() {
        // THE headline bug. "القاهرة مصر 2026":
        //  - the digit run "2026" must read 2,0,2,6 L→R  → ASCENDING cluster
        //    with ASCENDING x, AND sit at the visual LEFT end of the line.
        //  - Arabic runs read RTL → DESCENDING cluster as x ascends.
        let line = "القاهرة مصر 2026";
        let vl = shape_visual_line(AMIRI_REGULAR, line, 0).unwrap();

        let digits = byte_range_of(line, "2026");
        let dg = glyphs_in_range(&vl, digits.clone());
        assert!(dg.len() >= 2, "expected the digit glyphs, got {dg:?}");
        assert!(
            ascending_cluster(&dg),
            "digits must read 2,0,2,6 L→R (ascending cluster): {dg:?}"
        );
        assert!(ascending_x(&dg), "digit x must ascend with cluster: {dg:?}");

        // Digit run sits at the visual LEFT end: every digit glyph is left of
        // every non-digit glyph.
        let max_digit_x = dg.iter().map(|(_, x)| *x).max().unwrap();
        let min_other_x = xs(&vl)
            .into_iter()
            .filter(|(c, _)| !digits.contains(c))
            .map(|(_, x)| x)
            .min()
            .unwrap();
        assert!(
            max_digit_x < min_other_x,
            "the LTR digit run must sit at the visual LEFT end (max_digit_x={max_digit_x} < min_other_x={min_other_x})"
        );

        // An Arabic word reads RTL. Check "القاهرة".
        let ar = glyphs_in_range(&vl, byte_range_of(line, "القاهرة"));
        assert!(ar.len() >= 2);
        assert!(
            descending_cluster(&ar),
            "Arabic word must read RTL (descending cluster as x rises): {ar:?}"
        );
        assert!(ascending_x(&ar));
    }

    #[test]
    fn arabic_indic_digits_keep_order() {
        // "السعر ٢٥٠ جنيه" — Arabic-Indic digits ٢٥٠ are an LTR number run and
        // must keep their 2,5,0 order (ascending cluster, ascending x).
        let line = "السعر ٢٥٠ جنيه";
        let vl = shape_visual_line(AMIRI_REGULAR, line, 0).unwrap();
        let num = glyphs_in_range(&vl, byte_range_of(line, "٢٥٠"));
        assert!(num.len() >= 2, "expected the Arabic-Indic digit glyphs: {num:?}");
        assert!(
            ascending_cluster(&num),
            "Arabic-Indic digits must keep order 2,5,0 (ascending cluster): {num:?}"
        );
        assert!(ascending_x(&num));
    }

    #[test]
    fn latin_word_inside_arabic_not_reversed() {
        // "مرحبا Hello" — the Latin word must not be reversed: H,e,l,l,o reads
        // L→R (ascending cluster + ascending x) and sits at the visual LEFT
        // (base is RTL, so the embedded LTR island lands on the left).
        let line = "مرحبا Hello";
        let vl = shape_visual_line(AMIRI_REGULAR, line, 0).unwrap();
        let hello = byte_range_of(line, "Hello");
        let hg = glyphs_in_range(&vl, hello.clone());
        assert!(hg.len() >= 4, "expected ~5 Latin glyphs, got {hg:?}");
        assert!(
            ascending_cluster(&hg),
            "'Hello' must not be reversed (ascending cluster L→R): {hg:?}"
        );
        assert!(ascending_x(&hg));

        // Latin island on the visual left of the Arabic.
        let max_hello_x = hg.iter().map(|(_, x)| *x).max().unwrap();
        let min_arabic_x = xs(&vl)
            .into_iter()
            .filter(|(c, _)| !hello.contains(c))
            .map(|(_, x)| x)
            .min()
            .unwrap();
        assert!(
            max_hello_x < min_arabic_x,
            "embedded LTR 'Hello' must sit at the visual left (max_hello_x={max_hello_x} < min_arabic_x={min_arabic_x})"
        );
    }

    #[test]
    fn number_percent_run_correct() {
        // "اطلب 100% الآن" — "100%" is a number/percent LTR run; its glyphs must
        // read L→R (ascending cluster, ascending x) and not be mirrored.
        let line = "اطلب 100% الآن";
        let vl = shape_visual_line(AMIRI_REGULAR, line, 0).unwrap();
        // "100" is unambiguously the EN number; "%" attaches to it. Assert on
        // the digits at least (the % may resolve at the run's level).
        let hundred = glyphs_in_range(&vl, byte_range_of(line, "100"));
        assert!(hundred.len() >= 2, "expected the '100' glyphs: {hundred:?}");
        assert!(
            ascending_cluster(&hundred),
            "'100' must read L→R (ascending cluster): {hundred:?}"
        );
        assert!(ascending_x(&hundred));

        // Both Arabic words read RTL.
        let lhs = glyphs_in_range(&vl, byte_range_of(line, "اطلب"));
        assert!(descending_cluster(&lhs), "'اطلب' must read RTL: {lhs:?}");
    }

    #[test]
    fn whole_line_x_is_monotonic_nondecreasing() {
        // Sanity: the final stream is absolute L→R, so x never goes backwards
        // for ANY of our mixed inputs.
        for line in [
            "القاهرة مصر 2026",
            "السعر ٢٥٠ جنيه",
            "مرحبا Hello",
            "اطلب 100% الآن",
            "Hello",
            "السلام عليكم",
        ] {
            let vl = shape_visual_line(AMIRI_REGULAR, line, 0).unwrap();
            let v = xs(&vl);
            assert!(ascending_x(&v), "x must be non-decreasing for {line:?}: {v:?}");
            // Clusters must cover every glyph and stay within the line bytes.
            assert!(
                vl.glyphs.iter().all(|g| (g.cluster as usize) < line.len()),
                "every glyph cluster must index into the line for {line:?}"
            );
        }
    }

    #[test]
    fn kashida_applies_only_within_rtl_and_marks_tatweel() {
        // Mixed line with a generous kashida budget. We assert (font-agnostic,
        // by x position):
        //  1. every tatweel glyph (identified by index) sits at the VISUAL
        //     position of the Arabic run(s) — to the RIGHT of every digit glyph
        //     — never inside the LTR digit run (digits render at the visual
        //     left), proving kashida was applied only within RTL;
        //  2. the line still reads correctly (digits L→R at the left);
        //  3. at least one tatweel is actually inserted (room exists).
        let line = "القاهرة مصر 2026";
        let vl = shape_visual_line(AMIRI_REGULAR, line, 6).unwrap();

        // Left-edge x of each glyph, by index.
        let mut x_by_index = Vec::with_capacity(vl.glyphs.len());
        {
            let mut x = 0i32;
            for g in &vl.glyphs {
                x_by_index.push(x);
                x += g.x_advance;
            }
        }

        // Identify the LTR digit block as the LEADING visual run of glyphs whose
        // cluster falls in the digit source bytes [22, 26). The base is RTL and
        // "2026" is the only LTR island, so its glyphs are placed leftmost and
        // contiguously. (Arabic glyphs can also carry working-string clusters in
        // [22,26) under kashida, but those appear later in the stream — taking
        // the LEADING prefix isolates the digits cleanly.)
        let digits = byte_range_of(line, "2026");
        let mut digit_block_end = 0usize;
        while digit_block_end < vl.glyphs.len()
            && digits.contains(&(vl.glyphs[digit_block_end].cluster as usize))
        {
            digit_block_end += 1;
        }
        // It must be exactly the four digits 2,0,2,6 at clusters 22..26, L→R.
        assert_eq!(digit_block_end, 4, "expected a 4-glyph leading digit block");
        for k in 0..4 {
            assert_eq!(
                vl.glyphs[k].cluster as usize,
                22 + k,
                "digit glyph {k} must carry cluster {} (ascending L→R)",
                22 + k
            );
        }
        let max_digit_x = x_by_index[digit_block_end - 1];

        // (1) every tatweel glyph is AFTER the digit block (Arabic region) and is
        //     painted to the right of every digit glyph.
        for &gi in &vl.tatweel_glyph_indices {
            assert!(gi < vl.glyphs.len(), "tatweel index in range");
            assert!(
                gi >= digit_block_end,
                "a tatweel glyph (index {gi}) must not be inside the LTR digit block"
            );
            let tx = x_by_index[gi];
            assert!(
                tx > max_digit_x,
                "kashida must appear in the Arabic run, right of the digits (tatweel x={tx}, max_digit_x={max_digit_x})"
            );
        }

        // (2) a long Arabic word + budget should land at least one tatweel.
        assert!(
            !vl.tatweel_glyph_indices.is_empty(),
            "expected at least one kashida insertion in a line with an Arabic run and budget 6"
        );
    }

    #[test]
    fn pure_rtl_kashida_matches_legacy_path() {
        // For a pure-RTL line, shape_visual_line(.., n) must equal the legacy
        // insert_kashida(.., n) glyph stream exactly — the wiring swap must be a
        // no-op for the pure-Arabic case the editor ships today. For a single
        // RTL run with base offset 0, `shape_visual_line` paints byte-identically
        // to legacy `insert_kashida`: same glyphs (ids, clusters, advances,
        // offsets) in the same order.
        let line = "الحمد لله رب العالمين";
        let n = 4;
        let vl = shape_visual_line(AMIRI_REGULAR, line, n).unwrap();
        let legacy = insert_kashida(AMIRI_REGULAR, line, n).unwrap();
        assert_eq!(
            vl.glyphs.len(),
            legacy.run.glyphs.len(),
            "pure-RTL kashida glyph count must match legacy insert_kashida"
        );
        for (a, b) in vl.glyphs.iter().zip(legacy.run.glyphs.iter()) {
            assert_eq!(a.glyph_id, b.glyph_id, "painted glyph id must match legacy");
            assert_eq!(a.cluster, b.cluster, "pure-RTL cluster must match legacy (base 0)");
            assert_eq!(a.x_advance, b.x_advance, "painted advance must match legacy");
            assert_eq!(a.x_offset, b.x_offset);
            assert_eq!(a.y_offset, b.y_offset);
        }
        assert_eq!(vl.total_x_advance, legacy.run.total_x_advance);

        // The tatweel glyph set must equal what the same coverage detection
        // finds on the legacy run (the ground truth for "which glyphs are
        // tatweel"), and it must be non-empty for this long line + budget.
        let legacy_tat = tatweel_glyphs_by_coverage(
            &legacy.run.glyphs,
            &legacy.inserted_at_byte_offsets,
            Direction::Rtl,
        );
        assert_eq!(
            vl.tatweel_glyph_indices, legacy_tat,
            "tatweel glyph indices must match coverage detection on the legacy run"
        );
        assert!(
            !vl.tatweel_glyph_indices.is_empty(),
            "a long Basmala with budget 4 must mark some tatweel glyphs"
        );
        // Every marked glyph must really be one harfrust produced for an inserted
        // tatweel: its cluster must be one of the working-string tatweel bytes,
        // OR (stacked-merge case) a cluster that a tatweel byte falls into.
        let tat_bytes: Vec<usize> = legacy
            .inserted_at_byte_offsets
            .iter()
            .enumerate()
            .map(|(i, &off)| off + i * TATWEEL_LEN)
            .collect();
        for &gi in &vl.tatweel_glyph_indices {
            let c = vl.glyphs[gi].cluster as usize; // base 0 here
            assert!(
                tat_bytes.iter().any(|&t| t >= c && t < c + 2 * (n + 1)),
                "marked glyph at cluster {c} should bound an inserted tatweel byte {tat_bytes:?}"
            );
        }
    }

    #[test]
    fn l2_reverse_runs_matches_uax9_example() {
        // UAX #9 rule L2 worked example on levels alone. Logical run levels
        // [0,1,2,1,0] (a level-2 island nested in a level-1 island) reorder to
        // the visual run order [0, 3, 2, 1, 4] after reversing level>=2 then
        // level>=1. This locks the L2 implementation independent of fonts.
        struct R(u8);
        let runs = [R(0), R(1), R(2), R(1), R(0)];
        let mut order: Vec<usize> = (0..runs.len()).collect();
        let max = runs.iter().map(|r| r.0).max().unwrap();
        for lvl in (1..=max).rev() {
            reverse_runs_at_level(&mut order, &runs, lvl, |r| r.0);
        }
        assert_eq!(order, vec![0, 3, 2, 1, 4], "L2 run reorder mismatch");
    }

    /// Minimal `ShapedGlyph` carrying only the `cluster` the coverage detector
    /// reads; other fields are irrelevant for that function.
    fn g(cluster: u32) -> ShapedGlyph {
        ShapedGlyph {
            glyph_id: 0,
            cluster,
            x_advance: 0,
            y_advance: 0,
            x_offset: 0,
            y_offset: 0,
            safe_to_insert_tatweel: false,
            unsafe_to_concat: false,
        }
    }

    #[test]
    fn coverage_detects_stacked_tatweels_merged_into_one_cluster() {
        // Reproduces the real Amiri case: kashida picks [4, 6, 10, 10] — TWO
        // tatweels stacked at original offset 10. Working tatweel bytes are then
        // 4, 8, 14, 16 (off_k + k*2). harfrust merges the two stacked tatweels
        // (working 14 and 16) into ONE glyph with cluster 14. A naive
        // `cluster == off+i*2` check would miss the byte-16 marker entirely; the
        // coverage check catches it because that glyph covers [14, next=18).
        //
        // RTL glyph stream (leftmost-first = logical reversed). Clusters, in
        // visual order, of: a letter at 18, the merged tatweel at 14, a tatweel
        // at 8, letters at 6/4/2/0 plus tatweels at 8 and 4. We model the merged
        // case minimally:
        let glyphs = vec![
            g(18), // base letter (covers [18, ..))
            g(14), // MERGED tatweel for working bytes 14 AND 16
            g(12), // base letter
            g(8),  // tatweel (working byte 8)
            g(6),  // base letter
            g(4),  // tatweel (working byte 4)
            g(2),  // base letter
            g(0),  // base letter
        ];
        // Original insertion offsets as insert_kashida reports them (ascending,
        // with the stacked pair as a repeat).
        let inserted = [4usize, 6, 10, 10];
        // Sanity: working bytes = 4, 8, 14, 16.
        let tat = tatweel_glyphs_by_coverage(&glyphs, &inserted, Direction::Rtl);
        // Indices 1 (covers 14 & 16), 3 (covers 8), 5 (covers 4) are tatweels.
        // Index 0 covers [18,inf) — contains no tatweel byte. We deliberately do
        // NOT include a glyph whose coverage spans byte 4..8 as a base letter to
        // avoid ambiguity in this synthetic case.
        assert_eq!(tat, vec![1, 3, 5], "merged stacked tatweel must be detected");
    }

    #[test]
    fn coverage_empty_without_insertions() {
        let glyphs = vec![g(0), g(2), g(4)];
        assert!(tatweel_glyphs_by_coverage(&glyphs, &[], Direction::Rtl).is_empty());
        assert!(tatweel_glyphs_by_coverage(&[], &[0], Direction::Rtl).is_empty());
    }
}
