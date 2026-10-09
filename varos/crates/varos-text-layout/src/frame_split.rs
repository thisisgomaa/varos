//! Lane H: widow/orphan constraints belong to the frame allocator, never line breaking.
use crate::{flow::tail, Composed, TextLayout};
use varos_core::{model::Document, text::TextBox, typography::Frame};
impl TextLayout {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn constrain_split(
        &mut self,
        doc: &Document,
        text: &TextBox,
        frame: &Frame,
        output: &mut Composed,
        used: &mut usize,
        zoom: f32,
    ) -> Result<(), String> {
        let Some(next) = frame.next else { return Ok(()) };
        let source = text.source();
        if *used == 0 || *used >= source.len() || source[..*used].ends_with('\n') {
            return Ok(());
        }
        let paragraph_start = source[..*used].rfind('\n').map_or(0, |i| i + 1);
        let first = output
            .layout
            .lines
            .iter()
            .position(|l| l.range.start >= paragraph_start)
            .unwrap_or(output.layout.lines.len());
        let target = doc.text_boxes.iter().find(|t| t.id == next).ok_or("missing text frame")?;
        let mut next_frame = doc.typography.frames.get(&next).cloned().unwrap_or_default();
        if next_frame.features.is_empty() {
            next_frame.features = frame.features.clone();
        }
        let mut keep = output.layout.lines.len();
        while keep > first {
            // The default engine policy is two lines on each side of a paragraph split.
            if keep - first < 2 {
                keep = first;
                break;
            }
            let mut rest = tail(text, *used);
            rest.id = next;
            rest.frame = target.frame;
            rest.box_kind = target.box_kind;
            let (preview, _) = self.compose_frame(doc, &rest, &next_frame, zoom)?;
            let boundary = rest.source().find('\n').unwrap_or(rest.source().len());
            if preview.layout.lines.iter().filter(|l| l.range.start < boundary).count() >= 2 {
                break;
            }
            keep -= 1;
            *used = output.layout.lines[keep].range.start;
        }
        if keep < output.layout.lines.len() {
            *used = output.layout.lines[keep].range.start;
            output.layout.lines.truncate(keep);
            output.layout.carets.retain(|c| c.line < keep);
            output.layout.issues.retain(|issue| match issue {
                varos_text::Issue::KashidaInserted { byte, .. } => *byte < *used,
                varos_text::Issue::JustificationResidual { line, .. } => *line < keep,
                varos_text::Issue::UnsupportedCluster(range) => range.start < *used,
                _ => true,
            });
            output.paths = self.flow_outlines(text, &output.layout, output.origin, zoom)?;
            output.layout.ink_bounds = crate::flow::bounds(&output.paths);
            output.overset = true;
        }
        Ok(())
    }
}
