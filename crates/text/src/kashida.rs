//! Kashida (tatweel) justification of right-to-left Arabic lines (Varos).
//!
//! VectorCraft breaks the lines; a justified right-to-left line is then grown with virtual
//! tatweels before its word spaces are spread. Which joins take one, in which order and how
//! much of the slack they may fill is Varos' policy ([`vectorcraft_text_arabic`]); the
//! measuring is done here, by re-shaping the words with VectorCraft's own HarfRust and the
//! paragraph's own face (collection index and variation instance included), so the
//! stretched line is set in exactly the face that measured it. No other font is ever used.
//!
//! A word is re-shaped as a whole with U+0640 inserted at the chosen boundaries and spliced
//! back into the paragraph's glyphs.
//!
//! How much a line takes is the paragraph's kashida setting ([`vectorcraft_doc::text::KashidaOptions`],
//! mapped to a numeric policy by [`arabic::policy`]).
//!
//! Where a tatweel may go is HarfRust's answer (its safe-to-insert-tatweel flag) for OpenType
//! Arabic fonts. Fonts it shapes with `morx` tables never get that flag ([`morx_shaped`]: Geeza
//! Pro, Al Nile, Baghdad, Damascus, KufiStandardGK, DecoType Naskh...); for them the joining rules
//! propose the boundaries (`varos-text`'s candidates: between two joining letters, never inside
//! لا, never before a base's own harakat, never after a letter that doesn't join to the left) and every trial
//! re-shape is verified against the word as the paragraph set it ([`morx_trial_ok`]) before it
//! can be kept. A virtual tatweel glyph has zero source length at the
//! boundary byte it fills: the caret, selection and exported text see only the authored
//! characters (as with the hyphen of a hyphenated line).

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use unicode_bidi::Level;
use vectorcraft_doc::{CharPosition, CharStyle, InlineArt, JustificationOptions, Justify};
use vectorcraft_text_arabic::{self as arabic, Measured, Policy, WorkingText};

use crate::features::OtFeatures;
use crate::fontdb::FontDb;
use crate::justify::{Flex, apply_desired};
use crate::shape::{SGlyph, shape_range};

const EPS: f64 = 1e-6;

/// A word's glyphs set again, with their [`Flex`].
type Setting = (Vec<SGlyph>, Vec<Flex>);

/// What a paragraph was shaped with, as the layout holds it.
pub(crate) struct Shaper<'a> {
    pub db: &'a FontDb,
    pub text: &'a str,
    pub runs: &'a [(Range<usize>, &'a CharStyle)],
    pub feats: &'a OtFeatures,
    /// The paragraph's resolved bidi levels, from `para_start`.
    pub levels: &'a [Level],
    pub para_start: usize,
}

/// Is a line of a paragraph aligned `justify` justified (its last line: `last`)?
pub(crate) fn justified(justify: Justify, last: bool) -> bool {
    match justify {
        Justify::JustifyLeft | Justify::JustifyCenter | Justify::JustifyRight => !last,
        Justify::JustifyAll => true,
        Justify::Auto | Justify::Left | Justify::Center | Justify::Right => false,
    }
}

/// Does glyph `g` belong to a joining script (Arabic and the like), whose words are never cut
/// and never letter-spaced?
pub(crate) fn joins(g: &SGlyph) -> bool {
    g.inline.is_none() && (arabic::joining_script(g.ch) || g.ch == arabic::TATWEEL)
}

/// Is `next` a virtual tatweel (zero length, at the byte of the letter it extends) drawn in the
/// cluster of `letter`? Upstream treats a zero-length glyph (a list marker) as a cluster of its
/// own; a kashida's tatweel stays with its letter, in line breaking and in visual order.
pub(crate) fn extends(letter: &SGlyph, next: &SGlyph) -> bool {
    letter.len > 0 && next.len == 0 && next.ch == arabic::TATWEEL && next.inline.is_none()
}

/// The end of the word the overflowing glyph `j` is in, with the spaces after it: where a line
/// ends that holds a joined word too wide for it, which overflows whole rather than being cut.
pub(crate) fn word_end(g: &[SGlyph], j: usize) -> usize {
    let mut k = j;
    while k < g.len() && !g[k].is_space() {
        k += 1;
        if g[k - 1].break_after() {
            break;
        }
    }
    while k < g.len() && g[k].is_space() {
        k += 1;
    }
    k
}

/// Re-shape the source word `word` with a virtual tatweel before each of `slots` (source byte
/// boundaries inside it, sorted). The glyphs come back in logical order with source bytes; a
/// tatweel glyph has zero length at its slot. `None` when the word can't be shaped apart.
pub(crate) fn reshape_word(sh: &Shaper<'_>, word: Range<usize>, slots: &[usize]) -> Option<Vec<SGlyph>> {
    let source = sh.text.get(word.clone())?;
    let local: Vec<usize> = slots.iter().map(|s| s.checked_sub(word.start)).collect::<Option<_>>()?;
    let working = WorkingText::new(source, &local).ok()?;
    // Where source byte `b` (word-local) lies in the working text: each tatweel before it moves it.
    let tatweel = arabic::TATWEEL.len_utf8();
    let at = |b: usize| b + local.iter().filter(|&&s| s < b).count() * tatweel;
    let mut runs = Vec::new();
    let mut global = Vec::new();
    for (k, (r, st)) in sh.runs.iter().enumerate() {
        let (a, b) = (r.start.max(word.start), r.end.min(word.end));
        if a < b {
            runs.push((at(a - word.start)..at(b - word.start), *st));
            global.push(k);
        }
    }
    let inlines: Vec<Option<&InlineArt>> = vec![None; runs.len()];
    let levels: Vec<Level> = (0..working.text.len())
        .map(|i| {
            let source = word.start + working.source_at.get(i).copied().unwrap_or(0);
            sh.levels.get(source.checked_sub(sh.para_start)?).copied()
        })
        .collect::<Option<_>>()?;
    let mut out = Vec::new();
    shape_range(sh.db, &working.text, 0..working.text.len(), &runs, &inlines, sh.feats, &levels, &mut out);
    for g in &mut out {
        let start = *working.source_at.get(g.byte)?;
        let end = working.source_at.get(g.byte + g.len).copied().unwrap_or(source.len());
        g.byte = word.start + start;
        g.len = end.saturating_sub(start);
        g.run = *global.get(g.run)?;
    }
    // A tatweel and the letter after it now share that letter's byte, so the layout's visual
    // order treats them as one cluster and keeps their order inside it, which for a
    // right-to-left cluster is left to right. The tatweel goes after the letter's glyphs: drawn
    // to the letter's right, between it and the letter it extends.
    out.sort_by_key(|g| (g.byte, g.len == 0));
    Some(out)
}

/// A word of the line that may take kashida: its glyphs `glyphs` (indices into the paragraph)
/// and its source bytes.
struct Word {
    glyphs: Range<usize>,
    bytes: Range<usize>,
    width: f64,
    /// The face the paragraph set it in: every re-shaped glyph must come from it too.
    face: u32,
    /// HarfRust shapes the face with `morx` ([`morx_shaped`]): its slots come from the joining
    /// rules and each trial is verified ([`morx_trial_ok`]).
    morx: bool,
    /// Varos: its glyphs are drawn in several runs that differ only in paint (harakat coloured
    /// apart, `crate::harakat`): each trial must keep every glyph's run ([`same_paint`]).
    painted: bool,
}

/// Does HarfRust shape `face` with its `morx` (or older `mort`) tables, joining its letters?
/// HarfRust uses them for horizontal text whenever the font has them, ignoring any `GSUB` (as
/// HarfBuzz does: Damascus has both and is shaped with `morx`), and its `morx` shaping never marks
/// a join safe for a tatweel: the joining rules propose the slots instead. A face whose letters
/// don't join takes no kashida (a tatweel would hang between isolated letters). Cached per face.
pub(crate) fn morx_shaped(face: &crate::fontdb::FontFace) -> bool {
    use skrifa::raw::types::Tag;
    #[cfg(test)]
    if tests::FORCED_MORX.with(|f| f.borrow().contains(&face.id())) {
        return true;
    }
    static CACHE: std::sync::Mutex<Option<HashMap<u32, bool>>> = std::sync::Mutex::new(None);
    if let Some(v) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&face.id()).copied()) {
        return v;
    }
    let v =
        face.skrifa().is_some_and(|f| f.table_data(Tag::new(b"morx")).is_some() || f.table_data(Tag::new(b"mort")).is_some()) && joins_letters(face);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = cache.get_or_insert_with(HashMap::new);
    if map.len() > 4096 {
        map.clear();
    }
    map.insert(face.id(), v);
    v
}

/// Does shaping join `face`'s letters: ببب isn't three isolated behs side by side?
fn joins_letters(face: &crate::fontdb::FontFace) -> bool {
    use harfrust::{Direction, ShapeOptions, UnicodeBuffer};
    let Some(hb) = face.hb() else { return false };
    let shaper = face.shaper.shaper(&hb).instance(face.instance.as_ref()).build();
    let gids = |text: &str| {
        let mut buf = UnicodeBuffer::new();
        buf.push_str(text);
        buf.set_direction(Direction::RightToLeft);
        buf.guess_segment_properties();
        shaper.shape(buf, ShapeOptions::new()).glyph_infos().iter().map(|g| g.glyph_id).collect::<Vec<u32>>()
    };
    let (one, three) = (gids("ب"), gids("ببب"));
    !one.is_empty() && !one.contains(&0) && !three.contains(&0) && three != one.repeat(3)
}

/// A trial re-shape `trial` of a word in a face shaped with `morx`, with tatweels at `slots`, against
/// the word as the paragraph set it (`original`). Kept only when:
/// (a) it has exactly one more glyph per tatweel, and each tatweel is a glyph of its own (zero
///     source length, U+0640);
/// (b) every authored glyph is unchanged: same glyph id (the letters beside each tatweel kept
///     their joining forms), same cluster, the same offsets (marks stay where they were) and the
///     same run (a harakat coloured apart keeps its colour);
/// (c) no glyph is `.notdef`;
/// (d) every authored glyph keeps its advance (within a font unit), and the word grew by the
///     tatweels' advances, within a font unit each.
pub(crate) fn morx_trial_ok(original: &[SGlyph], trial: &[SGlyph], slots: &[usize]) -> bool {
    morx_rejection(original, trial, slots).is_none()
}

fn morx_rejection(original: &[SGlyph], trial: &[SGlyph], slots: &[usize]) -> Option<&'static str> {
    let k = slots.len();
    if trial.len() != original.len() + k || trial.iter().any(|g| g.gid == 0) {
        return Some("morx verification: glyph count changed (ligature decomposition) or missing glyph");
    }
    let tatweels: Vec<&SGlyph> = trial.iter().filter(|g| g.len == 0).collect();
    if tatweels.len() != k || tatweels.iter().any(|g| g.ch != arabic::TATWEEL || g.adv <= 0.0) {
        return Some("morx verification: tatweel not a separate positive-advance glyph");
    }
    let mut at: Vec<usize> = tatweels.iter().map(|g| g.byte).collect();
    at.sort_unstable();
    if at != slots {
        return Some("morx verification: tatweel moved from requested slot");
    }
    let authored = trial.iter().filter(|g| g.len > 0);
    // One font unit, in points.
    let unit_of = |g: &SGlyph| g.sx.abs() + EPS;
    // Each authored glyph on its own, advance included: a gain in one letter and a loss in
    // another must not cancel out in the word's total.
    let same = authored.clone().count() == original.len()
        && authored.zip(original).all(|(a, b)| {
            a.gid == b.gid
                && a.face.id() == b.face.id()
                && a.byte == b.byte
                && a.len == b.len
                && a.run == b.run
                && (a.dx - b.dx).abs() < EPS
                && (a.dy - b.dy).abs() < EPS
                && (a.adv - b.adv).abs() <= unit_of(b)
        });
    let grown: f64 = trial.iter().map(|g| g.adv).sum::<f64>() - original.iter().map(|g| g.adv).sum::<f64>();
    let added: f64 = tatweels.iter().map(|g| g.adv).sum();
    let unit: f64 = tatweels.iter().map(|g| g.sx.abs()).sum();
    if !same {
        Some("morx verification: authored glyph, cluster, mark offset or advance changed")
    } else if (grown - added).abs() > unit + EPS {
        Some("morx verification: growth differs from tatweel advances")
    } else {
        None
    }
}

/// AAT ligatures may split at a legal join. Require stable contextual forms on a
/// second insertion, unchanged glyphs outside that ligature, and actual ink contact
/// at both ends of every inserted bar. No font-specific glyph IDs are hard-coded.
fn verified_morx_trial(sh: &Shaper<'_>, word: Range<usize>, original: &[SGlyph], trial: &[SGlyph], slots: &[usize]) -> bool {
    if morx_trial_ok(original, trial, slots) {
        return true;
    }
    let split: Vec<_> =
        original.iter().filter(|g| slots.iter().any(|s| g.byte < *s && *s < g.byte + g.len)).map(|g| g.byte..g.byte + g.len).collect();
    if split.is_empty() || slots.len() >= 64 {
        return false;
    }
    let outside = |g: &SGlyph| g.len > 0 && !split.iter().any(|r| r.start <= g.byte && g.byte < r.end);
    let unchanged: Vec<_> = original.iter().filter(|g| outside(g)).cloned().collect();
    let kept: Vec<_> = trial.iter().filter(|g| outside(g)).cloned().collect();
    if !same_setting(&unchanged, &kept) {
        return false;
    }
    // Every authored byte is still covered, and no mark was swallowed by a virtual glyph.
    let coverage = |glyphs: &[SGlyph]| {
        let mut ranges: Vec<_> = glyphs.iter().filter(|g| g.len > 0).map(|g| (g.byte, g.byte + g.len)).collect();
        ranges.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (a, b) in ranges {
            if let Some(last) = merged.last_mut()
                && a <= last.1
            {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        merged
    };
    if coverage(original) != coverage(trial) {
        return false;
    }
    // Contextual mark glyphs may change when the ligature splits (Diwan's dammatan),
    // but their source cluster must still contain their base, or be the mark's own
    // character-level cluster. They must never become a zero-length virtual glyph.
    let Some(source) = sh.text.get(word.clone()) else { return false };
    let mut base = word.start;
    for (offset, ch) in source.char_indices() {
        let byte = word.start + offset;
        if arabic::is_harakat(ch) {
            if !trial.iter().any(|g| g.len > 0 && (g.byte <= base || g.byte == byte) && g.byte + g.len >= byte + ch.len_utf8()) {
                return false;
            }
        } else {
            base = byte;
        }
    }
    let Some(&slot) = slots.last() else { return false };
    let mut deeper_slots = slots.to_vec();
    deeper_slots.push(slot);
    let Some(deeper) = reshape_word(sh, word, &deeper_slots) else { return false };
    let authored: Vec<_> = trial.iter().filter(|g| g.len > 0).cloned().collect();
    let stable = morx_trial_ok(&authored, &deeper, &deeper_slots);
    let connected = bars_touch_neighbours(sh.db, trial);
    stable && connected
}

fn bars_touch_neighbours(db: &FontDb, glyphs: &[SGlyph]) -> bool {
    use kurbo::{Affine, Point, Shape};
    if glyphs.len() > 512 || glyphs.iter().any(|g| !g.adv.is_finite() || g.rotation != 0.0 || g.bshift != 0.0) {
        return false;
    }
    let mut clusters: Vec<&[SGlyph]> = Vec::new();
    let mut start = 0;
    while let Some(first) = glyphs.get(start) {
        let count = glyphs.iter().skip(start).take_while(|g| g.byte == first.byte).count();
        let Some(cluster) = glyphs.get(start..start + count) else { return false };
        clusters.push(cluster);
        start += count;
    }
    let mut placed = Vec::new();
    let mut pen = 0.0;
    for g in clusters.into_iter().rev().flatten() {
        if g.adv > 0.0 {
            let path = (Affine::translate((pen + g.dx, g.dy)) * Affine::scale_non_uniform(g.sx, g.sy)) * db.outline(&g.face, g.gid).as_ref().clone();
            if path.elements().len() > 2048 || !path.is_finite() {
                return false;
            }
            placed.push((g.len == 0, pen, g.adv, path));
        }
        pen += g.adv;
    }
    for (i, (virtual_, _x, advance, path)) in placed.iter().enumerate() {
        if !virtual_ {
            continue;
        }
        let Some(left) = i.checked_sub(1).and_then(|i| placed.get(i)) else { return false };
        let Some(right) = placed.get(i + 1) else { return false };
        let bounds = path.bounding_box();
        let touches = |neighbour: &kurbo::BezPath, left: bool| {
            let other = neighbour.bounding_box();
            let delta = 0.001_f64.min(advance * 0.01);
            let x0 = bounds.x0.max(other.x0) - delta;
            let x1 = bounds.x1.min(other.x1) + delta;
            let y0 = bounds.y0.max(other.y0);
            let y1 = bounds.y1.min(other.y1);
            if x1 < x0 || y1 <= y0 {
                return false;
            }
            (0..=16).any(|column| {
                (1..32).any(|row| {
                    let x = x0 + (x1 - x0) * f64::from(column) / 16.0;
                    let y = y0 + (y1 - y0) * f64::from(row) / 32.0;
                    let side = if left { -delta } else { delta };
                    (neighbour.winding(Point::new(x, y)) != 0 && path.winding(Point::new(x, y)) != 0)
                        || (neighbour.winding(Point::new(x + side, y)) != 0 && path.winding(Point::new(x - side, y)) != 0)
                })
            })
        };
        if !touches(&left.3, true) || !touches(&right.3, false) {
            return false;
        }
    }
    true
}

/// The same glyphs set the same way, each in the same run (re-shaping a word apart must
/// reproduce the paragraph's, harakat coloured apart included).
fn same_setting(a: &[SGlyph], b: &[SGlyph]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.gid == b.gid
                && a.byte == b.byte
                && a.len == b.len
                && a.run == b.run
                && a.face.id() == b.face.id()
                && (a.adv - b.adv).abs() < EPS
                && (a.dx - b.dx).abs() < EPS
                && (a.dy - b.dy).abs() < EPS
        })
}

/// Varos: the authored glyphs (tatweels aside) of a word re-shaped with tatweels, `trial`, are
/// drawn in the runs the paragraph drew them in (`original`): each cluster's glyphs in the same
/// runs. A trial whose character-level pass fell back would draw a coloured harakat in its
/// letter's run (`crate::harakat`).
fn same_paint(original: &[SGlyph], trial: &[SGlyph]) -> bool {
    let runs = |v: &[SGlyph]| {
        let mut r: Vec<(usize, usize)> = v.iter().filter(|g| g.len > 0).map(|g| (g.byte, g.run)).collect();
        r.sort_unstable();
        r
    };
    runs(original) == runs(trial)
}

/// A word's glyphs re-shaped with `slots`, at the paragraph's Desired justification values,
/// with their [`Flex`]; their leading is the word's.
fn reshaped(sh: &Shaper<'_>, word: &Word, slots: &[usize], options: &JustificationOptions, leading: f64) -> Option<Setting> {
    let mut glyphs = reshape_word(sh, word.bytes.clone(), slots)?;
    for g in &mut glyphs {
        g.leading = leading;
    }
    let flex = apply_desired(&mut glyphs, options, true);
    Some((glyphs, flex))
}

/// Can the style `st` of a run be set again apart from its paragraph, glyph for glyph?
fn plain(st: &CharStyle) -> bool {
    st.tracking == 0.0
        && st.kerning.is_none()
        && st.h_scale == 100.0
        && st.v_scale == 100.0
        && !st.all_caps
        && st.small_caps.is_none()
        && st.position == CharPosition::Normal
}

/// Grow the justified right-to-left line `sg[i..end]` (its visible glyphs `sg[i..trimmed]`,
/// `width` points wide) toward its width with kashida, before the word spaces take the rest.
/// The words that take a tatweel are re-shaped and spliced into `sg` and `flex`; returns how
/// many glyphs the line gained (0: nothing changed). Lines it can't set alike (tabs, inline
/// graphics, a word cut at the line's start or end) and words it can't (other scripts, digits,
/// several runs or faces, tracking, kerning, scaling, caps, super/subscript) keep their glyphs.
/// `policy` is how much of the slack the tatweels may fill and how many a word takes; `width` is
/// the width the line grows toward (its measure when justified, its elongation target when not).
#[allow(clippy::too_many_arguments)]
pub(crate) fn stretch_line(
    sh: &Shaper<'_>,
    sg: &mut Vec<SGlyph>,
    flex: &mut Vec<Flex>,
    options: &JustificationOptions,
    policy: Policy,
    i: usize,
    trimmed: usize,
    end: usize,
    width: f64,
) -> isize {
    let Some(line) = sg.get(i..trimmed) else { return 0 };
    let natural: f64 = line.iter().map(|g| g.adv).sum();
    if line.is_empty()
        || !width.is_finite()
        || width - natural <= EPS
        || line.iter().any(|g| g.ch == '\t' || g.inline.is_some() || g.tcy.is_some())
        // A word cut at either end of the line (a fragment) is not stretched.
        || (i > 0 && !sg[i - 1].break_after())
        || (end < sg.len() && end > 0 && !sg[end - 1].break_after())
    {
        return 0;
    }
    // The words that may take kashida: one face and run, Arabic letters only, plainly set, and
    // re-shaped apart exactly as the paragraph set them.
    let mut words = Vec::new();
    let mut j = i;
    while j < trimmed {
        if sg[j].is_space() {
            j += 1;
            continue;
        }
        let a = j;
        while j < trimmed && !sg[j].is_space() {
            j += 1;
        }
        let glyphs = &sg[a..j];
        let first = &glyphs[0];
        let bytes = glyphs.iter().map(|g| g.byte).min().unwrap_or(0)..glyphs.iter().map(|g| g.byte + g.len).max().unwrap_or(0);
        // One run, or runs that differ only in paint (harakat coloured apart: `crate::harakat`).
        let same_run = |g: &SGlyph| {
            g.run == first.run
                || matches!((sh.runs.get(g.run), sh.runs.get(first.run)), (Some((_, a)), Some((_, b))) if crate::harakat::paint_only(a, b))
        };
        let ok = glyphs.iter().all(|g| g.level.is_rtl() && g.face.id() == first.face.id() && same_run(g) && g.gid != 0)
            // The face's own tatweel: never one borrowed from another font.
            && first.face.glyph_for(arabic::TATWEEL) != 0
            && sh.runs.get(first.run).is_some_and(|(_, st)| plain(st))
            && sh.text.get(bytes.clone()).is_some_and(arabic::kashida_word);
        if !ok {
            continue;
        }
        let word = Word {
            glyphs: a..j,
            bytes,
            width: glyphs.iter().map(|g| g.adv).sum(),
            face: first.face.id(),
            morx: morx_shaped(&first.face),
            painted: glyphs.iter().any(|g| g.run != first.run),
        };
        let same = reshaped(sh, &word, &[], options, first.leading).is_some_and(|(again, _)| same_setting(&again, glyphs));
        if same {
            words.push(word);
        }
    }
    if words.is_empty() {
        return 0;
    }
    let safe = |glyphs: &[SGlyph]| glyphs.iter().filter(|g| g.tatweel && g.len > 0).map(|g| g.byte).collect::<Vec<_>>();
    // A morx-only face's boundaries: the joining rules', each trial verified below.
    let joining = |w: &Word| {
        arabic::kashida_candidates(sh.text, w.bytes.clone()).map(|c| c.into_iter().map(|c| c.byte).collect::<Vec<_>>()).unwrap_or_default()
    };
    let mut preflight_remaining = 64usize;
    let native_safe: Vec<Vec<usize>> = words
        .iter()
        .map(|w| {
            if w.morx {
                return joining(w);
            }
            let mut slots = safe(&sg[w.glyphs.clone()]);
            // HarfRust can withhold the initial flag on a marked ligature (Amiri أثرٌ),
            // yet flag the re-shaped join. Accept only that explicit post-insertion flag,
            // with every mark still in its authored base cluster.
            if sh.text.get(w.bytes.clone()).is_some_and(|t| t.chars().any(arabic::is_harakat)) {
                for at in joining(w).into_iter().take(8) {
                    if preflight_remaining == 0 {
                        break;
                    }
                    preflight_remaining -= 1;
                    if !slots.contains(&at)
                        && reshape_word(sh, w.bytes.clone(), &[at])
                            .is_some_and(|trial| safe(&trial).contains(&at) && marks_keep_base(&sg[w.glyphs.clone()], &trial))
                    {
                        slots.push(at);
                    }
                }
            }
            slots
        })
        .collect();
    let originals: Vec<Vec<SGlyph>> = words.iter().map(|w| if w.morx || w.painted { sg[w.glyphs.clone()].to_vec() } else { vec![] }).collect();
    let leading: Vec<f64> = words.iter().map(|w| sg[w.glyphs.start].leading).collect();
    let line_bytes = words[0].bytes.start.min(sg[i].byte)..words.iter().map(|w| w.bytes.end).max().unwrap_or(0);
    let mut cache: HashMap<(usize, Vec<usize>), Option<Setting>> = HashMap::new();
    let Ok(plan) = arabic::plan_with(sh.text, line_bytes, natural, native_safe.iter().flatten().copied().collect(), width, policy, |slots| {
        let mut width = natural;
        let mut safe_now = HashSet::new();
        let mut missing = false;
        let mut payload = Vec::new();
        for (k, word) in words.iter().enumerate() {
            let mine: Vec<usize> = slots.iter().copied().filter(|s| word.bytes.start < *s && *s < word.bytes.end).collect();
            if mine.is_empty() {
                safe_now.extend(native_safe[k].iter().copied());
                continue;
            }
            let set = cache.entry((k, mine.clone())).or_insert_with(|| reshaped(sh, word, &mine, options, leading[k])).clone()?;
            if word.morx && !verified_morx_trial(sh, word.bytes.clone(), &originals[k], &set.0, &mine) {
                return None;
            }
            if !word.morx && !marks_keep_base(&sg[word.glyphs.clone()], &set.0) {
                return None;
            }
            // Varos: a coloured harakat keeps its colour in the stretched word.
            if word.painted && !same_paint(&originals[k], &set.0) {
                return None;
            }
            width += set.0.iter().map(|g| g.adv).sum::<f64>() - word.width;
            if word.morx {
                safe_now.extend(native_safe[k].iter().copied());
            } else {
                safe_now.extend(safe(&set.0));
            }
            // A missing glyph, or one from another face (a fallback), rejects the trial.
            missing |= set.0.iter().any(|g| g.gid == 0 || g.face.id() != word.face);
            payload.push((k, set));
        }
        // Every slot lies in a word that may take one.
        let placed: usize =
            payload.iter().map(|(k, _)| slots.iter().filter(|s| words[*k].bytes.start < **s && **s < words[*k].bytes.end).count()).sum();
        (placed == slots.len()).then_some(Measured { width, safe: safe_now, missing_glyph: missing, payload })
    }) else {
        return 0;
    };
    let Some(payload) = plan.payload else { return 0 };
    let mut grown = 0isize;
    // Splice from the last word back, so the earlier glyph indices stay put.
    for (k, (glyphs, word_flex)) in payload.into_iter().rev() {
        let at = words[k].glyphs.clone();
        grown += glyphs.len() as isize - at.len() as isize;
        flex.splice(at.clone(), word_flex);
        sg.splice(at, glyphs);
    }
    grown
}

#[cfg(test)]
#[path = "tests_kashida_policy.rs"]
mod tests;

/// Varos: headless explanation of font eligibility, independent of line slack/strength.
#[derive(Debug)]
pub struct KashidaDiagnostic {
    pub word: String,
    pub morx: bool,
    pub font: String,
    pub slots: Vec<(usize, &'static str)>,
}

/// Inspect each word in the requested face, without fallback or mutation of authored text.
pub fn diagnostics(db: &FontDb, text: &str, style: &CharStyle) -> Vec<KashidaDiagnostic> {
    let mut result = Vec::new();
    let mut offset = 0;
    for word in text.split_whitespace() {
        let Some(local) = text.get(offset..).and_then(|s| s.find(word)) else { break };
        offset += local;
        let runs = [(0..word.len(), style)];
        let levels = vec![Level::rtl(); word.len()];
        let feats = OtFeatures::default();
        let sh = Shaper { db, text: word, runs: &runs, feats: &feats, levels: &levels, para_start: 0 };
        let original = reshape_word(&sh, 0..word.len(), &[]).unwrap_or_default();
        let face = original.first().map(|g| &g.face);
        let morx = face.is_some_and(|f| morx_shaped(f));
        let allowed = arabic::kashida_candidates(word, 0..word.len()).unwrap_or_default();
        let slots = arabic::candidates(word, 0..word.len())
            .unwrap_or_default()
            .iter()
            .map(|c| {
                let reason = if !allowed.iter().any(|a| a.byte == c.byte) {
                    "policy: sacred name, article or short stem"
                } else if face.is_none_or(|f| f.glyph_for(arabic::TATWEEL) == 0) {
                    "font has no tatweel"
                } else if !morx
                    && !original.iter().any(|g| g.byte == c.byte && g.tatweel)
                    && !reshape_word(&sh, 0..word.len(), &[c.byte]).is_some_and(|trial| {
                        word.chars().any(arabic::is_harakat)
                            && trial.iter().any(|g| g.byte == c.byte && g.tatweel && g.len > 0)
                            && marks_keep_base(&original, &trial)
                    })
                {
                    "shaper did not mark slot safe"
                } else if let Some(trial) = reshape_word(&sh, 0..word.len(), &[c.byte]) {
                    if trial.iter().any(|g| g.gid == 0 || face.is_none_or(|f| f.id() != g.face.id())) {
                        "missing glyph or fallback face"
                    } else if morx && !verified_morx_trial(&sh, 0..word.len(), &original, &trial, &[c.byte]) {
                        morx_rejection(&original, &trial, &[c.byte]).unwrap_or("accepted")
                    } else if trial.iter().map(|g| g.adv).sum::<f64>() <= original.iter().map(|g| g.adv).sum::<f64>() + EPS {
                        "no positive elongation"
                    } else {
                        "accepted"
                    }
                } else {
                    "word could not be reshaped"
                };
                (offset + c.byte, reason)
            })
            .collect();
        result.push(KashidaDiagnostic { word: word.into(), morx, font: face.map_or_else(String::new, |f| f.family.clone()), slots });
        offset += word.len();
    }
    result
}

/// Varos: join the outlines of an elongated word, preserving glyph metadata for carets and
/// text extraction. Paint boundaries are never crossed. A bounded failed union keeps the art.
pub(crate) fn continuous_outlines(layout: &mut crate::TextLayout, text: &str, db: &FontDb) {
    use kurbo::BezPath;
    use linesweeper::topology::Topology;
    let mut groups: std::collections::BTreeSet<(usize, usize, usize, usize)> = Default::default();
    for (index, g) in layout.glyphs.iter().enumerate() {
        if layout.marker_of(index).is_some() || g.len != 0 || db.face_by_id(g.font_id).is_none_or(|f| f.glyph_for(arabic::TATWEEL) != g.gid) {
            continue;
        }
        let Some(before) = text.get(..g.byte) else { continue };
        let Some(after) = text.get(g.byte..) else { continue };
        let start = before.rfind(char::is_whitespace).map_or(0, |i| i + before.get(i..).and_then(|s| s.chars().next()).map_or(0, char::len_utf8));
        let end = g.byte + after.find(char::is_whitespace).unwrap_or(after.len());
        groups.insert((start, end, g.line, g.run));
    }
    for (start, end, line, run) in groups {
        let indices: Vec<_> = layout
            .glyphs
            .iter()
            .enumerate()
            .filter(|(_, g)| g.byte >= start && g.byte < end && g.line == line && g.run == run && g.advance > 0.0)
            .map(|(i, _)| i)
            .collect();
        let mut path = BezPath::new();
        for &i in &indices {
            if let Some(g) = layout.glyphs.get(i) {
                path.extend(g.outline.iter());
            }
        }
        // Bound quadratic sweep work on untrusted font outlines.
        if path.elements().len() > 2048 || !path.is_finite() {
            continue;
        }
        let Ok(top) = Topology::<i32>::from_path(&path, 1e-7) else {
            log::warn!("kashida outline union refused font geometry");
            continue;
        };
        let mut merged = BezPath::new();
        for contour in top.contours(|w| *w != 0).contours() {
            merged.extend(contour.path.iter());
        }
        if merged.is_empty() {
            continue;
        }
        for (n, i) in indices.into_iter().enumerate() {
            if let Some(g) = layout.glyphs.get_mut(i) {
                g.outline = if n == 0 { merged.clone() } else { BezPath::new() };
            }
        }
    }
}

/// Marks may reposition for a new contextual base form, but cannot move to a virtual tatweel.
fn marks_keep_base(original: &[SGlyph], trial: &[SGlyph]) -> bool {
    let marks = |glyphs: &[SGlyph]| {
        let mut marks: Vec<_> = glyphs.iter().filter(|g| g.len > 0 && g.adv.abs() < EPS).map(|g| (g.gid, g.byte, g.len, g.run)).collect();
        marks.sort_unstable();
        marks
    };
    marks(original) == marks(trial)
}
