//! ---- w2-gradients: canvas stroke integration seam ----
//! Integrator: replace `coverage` with `ed.canvas_strokes.lookup(path, xf, ppu)` from
//! stroke/canvas.rs (9f14e1e), using its world-space rings and native fallback/report.
//! Do not route gradient strokes to the export evaluator after integrating that hotfix.
//! This lane predates CanvasStrokeCache; this adapter is the sole substitution point.
use crate::{
    editor::Editor,
    model::Path,
    stroke::{StrokeCoverage, StrokeError},
};
pub(crate) fn coverage(_ed: &Editor, path: &Path, ppu: f32) -> Result<StrokeCoverage, StrokeError> {
    crate::stroke::evaluate(path, (0.025 / f64::from(ppu.max(0.0001))).clamp(0.01, 0.1), &|| false)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canvas_seam_accepts_gradient_stroke_and_finite_zoom_extremes() {
        let mut ed = Editor::new();
        let id = ed.doc.nid();
        let anchors = ed.doc.build_shape(crate::model::ShapeKind::Rect, [0., 0.], [50., 50.]);
        let mut path = Path::new(id, anchors, true, None, None, 4.);
        path.stroke = crate::model::Paint::Gradient(Default::default());
        for ppu in [0., 1., 100000.] {
            assert!(!coverage(&ed, &path, ppu).unwrap().rings.is_empty());
        }
    }
}
