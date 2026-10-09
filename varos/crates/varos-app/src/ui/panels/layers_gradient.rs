//! Lane B: resolved thumbnail paints, sampled in the thumbnail's fitted coordinate frame.
use super::*;
use varos_core::model::Paint;
fn inside(rings: &[Vec<Pt>], p: Pt) -> bool {
    let mut hit = false;
    for ring in rings {
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                hit = !hit;
            }
        }
    }
    hit
}
pub(super) fn sample(paint: &Paint, point: Pt) -> Option<Rgba> {
    match paint {
        Paint::Gradient(g) => Some(g.sample_point(point)),
        paint => paint.solid(),
    }
}
pub(super) fn paint(p: &egui::Painter, rect: egui::Rect, shape: &ThumbShape, dim: f32) -> bool {
    if !shape.paints.iter().any(|p| matches!(p, Paint::Gradient(_))) {
        return false;
    }
    for y in 0..18 {
        for x in 0..18 {
            let point = [(x as f32 + 0.5) / 18., (y as f32 + 0.5) / 18.];
            if inside(&shape.rings, point) {
                if let Some(c) = sample(&shape.paints[0], point) {
                    p.rect_filled(
                        egui::Rect::from_min_size(rect.min + egui::vec2(x as f32, y as f32), egui::vec2(1., 1.)),
                        0.,
                        with_a(rgba_c32a(c), dim * shape.opacity),
                    );
                }
            }
        }
    }
    for ring in &shape.rings {
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
            if let Some(c) = sample(&shape.paints[1], [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]) {
                p.line_segment(
                    [rect.min + egui::vec2(a[0], a[1]) * 18., rect.min + egui::vec2(b[0], b[1]) * 18.],
                    Stroke::new(1., with_a(rgba_c32a(c), dim * shape.opacity)),
                );
            }
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnails_sample_spatial_colour_and_holes() {
        let g = varos_core::gradient::Gradient { placement: [1., 0., 0., 1., 0., 0.], ..Default::default() };
        let paint = Paint::Gradient(g);
        assert_ne!(sample(&paint, [0., 0.]), sample(&paint, [1., 0.]));
        let rings =
            vec![vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]], vec![[0.4, 0.4], [0.6, 0.4], [0.6, 0.6], [0.4, 0.6]]];
        assert!(inside(&rings, [0.2, 0.2]));
        assert!(!inside(&rings, [0.5, 0.5]));
    }
    #[test]
    fn global_changes_invalidate_thumbnail_and_resolve_colour() {
        let mut ed = Editor::new();
        ed.doc.paths.push(varos_core::model::Path::new(1, vec![], true, None, None, 1.));
        ed.doc.paths[0].fill = Paint::SwatchRef { id: 1 };
        ed.doc.swatches.push(varos_core::swatches::Swatch {
            id: 1,
            name: "Ink".into(),
            paint: Paint::Gradient(Default::default()),
            global: true,
            group: String::new(),
        });
        let before = thumb_key(&ed, &[1]);
        ed.doc.swatches[0].paint = Paint::Solid([1., 0., 0., 1.]);
        assert_ne!(before, thumb_key(&ed, &[1]));
        assert_eq!(thumb_shapes(&ed, &[1])[0].fill, Some([1., 0., 0., 1.]));
    }
}
