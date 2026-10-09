//! Lane B: owned drawing defaults, with compatible legacy solid-colour channels.
use crate::{
    editor::{Editor, PaintTarget},
    geom::Rgba,
    model::{Paint, Path},
};
#[derive(Clone, Default)]
pub(crate) struct CurrentPaints([Option<Current>; 2]);
#[derive(Clone)]
struct Current {
    paint: Paint,
    colour: Option<Rgba>,
    bounds: Option<[f32; 4]>,
}
pub(crate) fn bounds(path: &Path) -> Option<[f32; 4]> {
    let mut points = path
        .anchors
        .iter()
        .chain(path.holes.iter().flatten())
        .flat_map(|a| [Some(a.p), a.hin, a.hout].into_iter().flatten());
    let first = points.next()?;
    let mut b = [first[0], first[1], first[0], first[1]];
    for q in points {
        b[0] = b[0].min(q[0]);
        b[1] = b[1].min(q[1]);
        b[2] = b[2].max(q[0]);
        b[3] = b[3].max(q[1]);
    }
    Some(b)
}
/// Owned gradients fit the receiving path; global references remain linked as authored.
pub(crate) fn fit(paint: Paint, source: Option<[f32; 4]>, dest: &Path) -> Paint {
    let (Paint::Gradient(g), Some(a), Some(b)) = (&paint, source, bounds(dest)) else { return paint };
    let w = (a[2] - a[0]).max(0.001);
    let h = (a[3] - a[1]).max(0.001);
    let sx = if b[2] > b[0] { (b[2] - b[0]) / w } else { 1. };
    let sy = if b[3] > b[1] { (b[3] - b[1]) / h } else { 1. };
    Paint::Gradient(g.mapped(|q| [b[0] + (q[0] - a[0]) * sx, b[1] + (q[1] - a[1]) * sy]))
}
fn index(target: PaintTarget) -> usize {
    if target == PaintTarget::Fill {
        0
    } else {
        1
    }
}
impl Editor {
    pub fn current_paint(&self, target: PaintTarget) -> Paint {
        let colour = if target == PaintTarget::Fill { self.cur_fill } else { self.cur_stroke };
        self.current_paints.0[index(target)]
            .as_ref()
            .filter(|current| current.colour == colour)
            .map_or_else(|| Paint::from_opt(colour), |current| current.paint.clone())
    }
    pub fn set_current_paint(&mut self, target: PaintTarget, paint: Paint) {
        let colour = paint.resolved(&self.doc).representative();
        if target == PaintTarget::Fill {
            self.cur_fill = colour;
        } else {
            self.cur_stroke = colour;
        }
        self.current_paints.0[index(target)] = Some(Current { paint, colour, bounds: None });
    }
    pub(crate) fn set_sampled_paint(&mut self, target: PaintTarget, paint: Paint, source: &Path) {
        self.set_current_paint(target, paint);
        if let Some(current) = self.current_paints.0[index(target)].as_mut() {
            current.bounds = bounds(source);
        }
    }
    pub(crate) fn inherit_current_paints(&self, path: &mut Path) {
        for target in [PaintTarget::Fill, PaintTarget::Stroke] {
            let source = self.current_paints.0[index(target)].as_ref().and_then(|c| c.bounds);
            let paint = fit(self.current_paint(target), source, path);
            if target == PaintTarget::Fill {
                path.fill = paint;
            } else {
                path.stroke = paint;
            }
        }
    }
    pub(crate) fn materialize_current_swatch(&mut self, id: u32, paint: &Paint) {
        for current in self.current_paints.0.iter_mut().flatten() {
            if current.paint == (Paint::SwatchRef { id }) {
                current.paint = paint.clone();
            }
        }
    }
    pub(crate) fn refresh_drawing_paints(&mut self, pi: usize) {
        let mut path = self.doc.paths[pi].clone();
        self.inherit_current_paints(&mut path);
        self.doc.paths[pi].fill = path.fill;
        self.doc.paths[pi].stroke = path.stroke;
    }
}
