//! Lane H: one arc-length coordinate map for curved glyphs and editing affordances.
use crate::{flow::ArcPath, Composed};
use varos_core::{
    model::Document,
    text::Alignment,
    typography::{Binding, PathEffect},
};

pub struct PathMap {
    pub(crate) arc: ArcPath,
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) align: f32,
    pub(crate) baseline: f32,
    pub(crate) offset: f32,
    pub(crate) flip: bool,
    pub(crate) effect: PathEffect,
}
impl PathMap {
    pub fn from_document(doc: &Document, id: u32, composed: &Composed) -> Result<Option<Self>, String> {
        let Some(Binding::Path { path, start, end, offset, flip, effect }) =
            doc.typography.frames.get(&id).and_then(|f| f.binding)
        else {
            return Ok(None);
        };
        let index = doc.pidx(path).ok_or("missing text path")?;
        let shape = &doc.paths[index];
        let frame_text = doc.text_boxes.iter().find(|t| t.id == id).ok_or("missing text frame")?;
        let arc = ArcPath::new(
            crate::flow::geometry(doc, frame_text, path)?.into_iter().next().unwrap_or_default(),
            shape.closed,
        )?;
        let end = (arc.length - end).max(start);
        let root = varos_core::typography::story_root(doc, id)?;
        let text = doc.text_boxes.iter().find(|t| t.id == root).ok_or("missing text source")?;
        let text = doc.typography.resolved(text)?;
        let line = composed.layout.lines.first();
        let slack = end - start - line.map_or(0., |l| l.width);
        let align = match text.para.align {
            Alignment::Right => slack,
            Alignment::Centre => slack * 0.5,
            _ => 0.,
        }
        .max(0.);
        Ok(Some(Self { arc, start, end, align, baseline: line.map_or(0., |l| l.baseline), offset, flip, effect }))
    }
    /// Rigid cluster placement keeps Arabic marks attached to the base tangent.
    pub fn at_anchor(&self, q: [f32; 2], anchor: f32) -> Option<[f32; 2]> {
        let center = anchor + self.align;
        let d = if self.flip { self.end - center } else { self.start + center };
        let (p, mut tangent) = self.arc.sample(d)?;
        if self.flip {
            tangent = [-tangent[0], -tangent[1]];
        }
        let normal = [-tangent[1], tangent[0]];
        let x = q[0] - anchor;
        let y = q[1] - self.baseline + self.offset;
        Some(match self.effect {
            PathEffect::Rainbow => [p[0] + tangent[0] * x + normal[0] * y, p[1] + tangent[1] * x + normal[1] * y],
            PathEffect::Skew => [p[0] + x + normal[0] * y, p[1] + tangent[1] * x + normal[1] * y],
        })
    }
    pub fn point(&self, q: [f32; 2]) -> Option<[f32; 2]> {
        self.at_anchor(q, q[0])
    }
    /// Project a pointer onto the baseline, returning the shaper's logical editing coordinates.
    pub fn unmap(&self, q: [f32; 2]) -> [f32; 2] {
        let d = self.arc.nearest(q);
        [if self.flip { self.end - d - self.align } else { d - self.start - self.align }, self.baseline]
    }
}
