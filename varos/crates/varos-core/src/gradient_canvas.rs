//! ---- w2-gradients: canvas stroke integration seam ----
//! Integration w2: bound to THE stroke seam (`stroke::canvas_seam`) — main's `stroke/canvas.rs`
//! cache/cap/back-off on the canvas (world rings), strict evaluation for export (local rings).
use crate::{
    editor::Editor,
    model::Path,
    stroke::{StrokeCoverage, StrokeError},
};
use std::sync::Arc;
pub(crate) fn coverage(ed: &Editor, path: &Path, ppu: f32, canvas: bool) -> Result<Arc<StrokeCoverage>, StrokeError> {
    crate::stroke::canvas_seam(ed, path, ppu, canvas)
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
        // Strict export evaluation (main's tolerance rule) at an ordinary export scale.
        assert!(!coverage(&ed, &path, 1., false).unwrap().rings.is_empty());
        for ppu in [0., 1., 100000.] {
            // The canvas route is main's hotfix: a degenerate zoom may give up (the scene then draws the
            // native fallback with a muted hint), but it never panics or loops.
            match coverage(&ed, &path, ppu, true) {
                Ok(c) => assert!(!c.rings.is_empty()),
                Err(e) => assert!(ppu == 0. && e == StrokeError::LimitExceeded, "{ppu}: {e:?}"),
            }
        }
        // The canvas route is the hotfix cache: a repeated frame does not re-evaluate.
        coverage(&ed, &path, 1., true).unwrap();
        let before = ed.canvas_stroke_cache.evaluations();
        coverage(&ed, &path, 1., true).unwrap();
        assert_eq!(ed.canvas_stroke_cache.evaluations(), before);
    }
}
