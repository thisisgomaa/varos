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
//! back into the paragraph's glyphs. A virtual tatweel glyph has zero source length at the
//! boundary byte it fills: the caret, selection and exported text see only the authored
//! characters (as with the hyphen of a hyphenated line).

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use unicode_bidi::Level;
use vectorcraft_doc::{CharPosition, CharStyle, InlineArt, JustificationOptions, Justify};
use vectorcraft_text_arabic::{self as arabic, Measured, WorkingText};

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
    g.inline.is_none() && arabic::joining_script(g.ch)
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
    let at = |b: usize| b + local.iter().filter(|&&s| s <= b).count() * tatweel;
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
    Some(out)
}

/// A word of the line that may take kashida: its glyphs `glyphs` (indices into the paragraph)
/// and its source bytes.
struct Word {
    glyphs: Range<usize>,
    bytes: Range<usize>,
    width: f64,
}

/// The same glyphs set the same way (re-shaping a word apart must reproduce the paragraph's).
fn same_setting(a: &[SGlyph], b: &[SGlyph]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.gid == b.gid
                && a.byte == b.byte
                && a.len == b.len
                && a.face.id() == b.face.id()
                && (a.adv - b.adv).abs() < EPS
                && (a.dx - b.dx).abs() < EPS
                && (a.dy - b.dy).abs() < EPS
        })
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
#[allow(clippy::too_many_arguments)]
pub(crate) fn stretch_line(
    sh: &Shaper<'_>,
    sg: &mut Vec<SGlyph>,
    flex: &mut Vec<Flex>,
    options: &JustificationOptions,
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
        let ok = glyphs.iter().all(|g| g.level.is_rtl() && g.face.id() == first.face.id() && g.run == first.run && g.gid != 0)
            && sh.runs.get(first.run).is_some_and(|(_, st)| plain(st))
            && sh.text.get(bytes.clone()).is_some_and(arabic::kashida_word);
        if !ok {
            continue;
        }
        let word = Word { glyphs: a..j, bytes, width: glyphs.iter().map(|g| g.adv).sum() };
        let same = reshaped(sh, &word, &[], options, first.leading).is_some_and(|(again, _)| same_setting(&again, glyphs));
        if same {
            words.push(word);
        }
    }
    if words.is_empty() {
        return 0;
    }
    let safe = |glyphs: &[SGlyph]| glyphs.iter().filter(|g| g.tatweel && g.len > 0).map(|g| g.byte).collect::<Vec<_>>();
    let native_safe: Vec<Vec<usize>> = words.iter().map(|w| safe(&sg[w.glyphs.clone()])).collect();
    let leading: Vec<f64> = words.iter().map(|w| sg[w.glyphs.start].leading).collect();
    let line_bytes = words[0].bytes.start.min(sg[i].byte)..words.iter().map(|w| w.bytes.end).max().unwrap_or(0);
    let mut cache: HashMap<(usize, Vec<usize>), Option<Setting>> = HashMap::new();
    let plan = arabic::plan(sh.text, line_bytes, natural, native_safe.iter().flatten().copied().collect(), width, arabic::POLICY, |slots| {
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
            width += set.0.iter().map(|g| g.adv).sum::<f64>() - word.width;
            safe_now.extend(safe(&set.0));
            missing |= set.0.iter().any(|g| g.gid == 0);
            payload.push((k, set));
        }
        // Every slot lies in a word that may take one.
        let placed: usize =
            payload.iter().map(|(k, _)| slots.iter().filter(|s| words[*k].bytes.start < **s && **s < words[*k].bytes.end).count()).sum();
        (placed == slots.len()).then_some(Measured { width, safe: safe_now, missing_glyph: missing, payload })
    });
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
