//! Lane B: exact shared gradient sampling under tiny-skia even-odd coverage.
use tiny_skia::{FillRule, Mask, Pixmap, PixmapPaint, Transform};
use varos_core::{geom::Pt, gradient::Gradient};
pub(super) fn draw(dst: &mut Pixmap, rings: &[Vec<Pt>], g: &Gradient, opacity: f32, xf: Transform) {
    let Some(path) = super::rings_path(rings, false) else { return };
    let Some(mut mask) = Mask::new(dst.width(), dst.height()) else { return };
    mask.fill_path(&path, FillRule::EvenOdd, true, xf);
    let Some(inv) = xf.invert() else { return };
    let Some(mut layer) = Pixmap::new(dst.width(), dst.height()) else { return };
    for y in 0..dst.height() {
        for x in 0..dst.width() {
            let idx = (y * dst.width() + x) as usize;
            let coverage = mask.data()[idx] as f32 / 255.;
            if coverage == 0. {
                continue;
            }
            let mut p = tiny_skia::Point::from_xy(x as f32 + 0.5, y as f32 + 0.5);
            inv.map_point(&mut p);
            let c = g.sample_point([p.x, p.y]);
            let a = c[3] * opacity * coverage;
            let b = &mut layer.data_mut()[idx * 4..idx * 4 + 4];
            for i in 0..3 {
                b[i] = (c[i] * a * 255.).round().clamp(0., 255.) as u8;
            }
            b[3] = (a * 255.).round().clamp(0., 255.) as u8;
        }
    }
    dst.draw_pixmap(0, 0, layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
}

/// Keep the frozen default-solid treatment; gradient strokes always replace their own fill.
pub(super) fn isolated_knockout(prims: &[varos_core::Prim]) -> bool {
    use varos_core::Prim;
    let has_gradient = prims.iter().any(|p| matches!(p, Prim::GradientFill { .. }));
    let translucent_stroke = prims.iter().any(|p| match p {
        Prim::StrokeCoverage { color, .. } => color[3] < 0.999,
        Prim::Stroke { color, .. } => has_gradient && color[3] < 0.999,
        Prim::GradientFill { stroke: true, gradient, .. } => {
            gradient.stops.iter().any(|s| s.colour[3] * s.opacity < 0.999)
        }
        _ => false,
    });
    translucent_stroke
        && prims.iter().any(|p| matches!(p, Prim::Fill { .. } | Prim::GradientFill { stroke: false, .. }))
}
