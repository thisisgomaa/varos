//! Tracking moves whole visual clusters and their logical/affinity caret stops together.
use varos_core::text::TextBox;
use varos_text::{Affinity, Layout};
pub(crate) fn apply(layout: &mut Layout, text: &TextBox) {
    for (line_index, line) in layout.lines.iter_mut().enumerate() {
        let mut shift = 0.;
        let mut previous = None;
        let mut mapping = Vec::new();
        for glyph in &mut line.glyphs {
            if previous != Some(glyph.cluster.start) {
                let mut start = 0;
                let mut spacing = 0.;
                for run in &text.runs {
                    if (start..start + run.text.len()).contains(&glyph.cluster.start) {
                        spacing = run.style.letter_spacing;
                        break;
                    }
                    start += run.text.len();
                }
                if previous.is_some() {
                    shift += spacing;
                }
                previous = Some(glyph.cluster.start);
            }
            mapping.push((glyph.cluster.clone(), glyph.x, glyph.x + glyph.advance, shift));
            glyph.x += shift;
        }
        for caret in layout.carets.iter_mut().filter(|c| c.line == line_index) {
            let candidates =
                || mapping.iter().filter(|(range, _, _, _)| range.start <= caret.byte && caret.byte <= range.end);
            let preferred = candidates().filter(|(range, _, _, _)| match caret.affinity {
                Affinity::Downstream => range.start == caret.byte,
                Affinity::Upstream => range.end == caret.byte,
            });
            let distance = |a: f32, b: f32| (caret.x - caret.x.clamp(a.min(b), a.max(b))).abs();
            let best = preferred
                .min_by(|a, b| distance(a.1, a.2).total_cmp(&distance(b.1, b.2)))
                .or_else(|| candidates().min_by(|a, b| distance(a.1, a.2).total_cmp(&distance(b.1, b.2))));
            if let Some((_, _, _, shift)) = best {
                caret.x += shift;
            }
        }
        line.width += shift;
    }
}
