//! CPU-only proof and winding comparison. No application renderer is constructed.
use crate::outlines::{even_odd_regions, ink_bounds, rings_path, Outline};
use tiny_skia::{FillRule, Paint, Pixmap, Transform};
#[derive(Debug, Clone, Default)]
pub struct Diff {
    pub pixels: usize,
    pub differing: usize,
    pub severe: usize,
    pub max_delta: u8,
    pub mean_delta: f64,
    pub interior_flips: usize,
    pub polygon_max_delta: u8,
}
/// Compare direct cubic nonzero coverage with converted flattened even-odd regions.
/// AA boundary tolerance is recorded separately from interior/topological defects.
pub fn winding_diff(outlines: &[Outline], scale: f32) -> Result<(Diff, Pixmap, Pixmap), &'static str> {
    let bounds = ink_bounds(outlines).unwrap_or([0., 0., 1., 1.]);
    let width = ((bounds[2] - bounds[0]) * scale).ceil() as u32 + 12;
    let height = ((bounds[3] - bounds[1]) * scale).ceil() as u32 + 12;
    if u64::from(width) * u64::from(height) > 64_000_000 {
        return Err("proof raster resource limit");
    }
    let mut direct = Pixmap::new(width, height).ok_or("pixmap")?;
    let mut converted = direct.clone();
    let transform = Transform::from_row(scale, 0., 0., scale, 6. - bounds[0] * scale, 6. - bounds[1] * scale);
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    // A single nonzero path reproduces union coverage without per-glyph AA seams.
    let all = Outline { commands: outlines.iter().flat_map(|o| o.commands.clone()).collect() };
    if let Some(path) = all.path() {
        direct.fill_path(&path, &paint, FillRule::Winding, transform, None);
    }
    if let Some(path) = rings_path(&even_odd_regions(outlines, 0.01 / scale)) {
        converted.fill_path(&path, &paint, FillRule::EvenOdd, transform, None);
    }
    let mut flat_reference = Pixmap::new(width, height).ok_or("pixmap")?;
    let flat_rings: Vec<_> = outlines.iter().flat_map(|o| o.flatten(0.01 / scale)).collect();
    if let Some(path) = rings_path(&flat_rings) {
        flat_reference.fill_path(&path, &paint, FillRule::Winding, transform, None);
    }
    let mut diff = Diff { pixels: (width * height) as usize, ..Diff::default() };
    let mut sum = 0u64;
    for (a, b) in direct.data().as_chunks::<4>().0.iter().zip(converted.data().as_chunks::<4>().0.iter()) {
        let d = a[3].abs_diff(b[3]);
        diff.differing += usize::from(d > 0);
        diff.severe += usize::from(d > 32);
        diff.interior_flips += usize::from((a[3] == 255 && b[3] == 0) || (b[3] == 255 && a[3] == 0));
        diff.max_delta = diff.max_delta.max(d);
        sum += u64::from(d);
    }
    diff.polygon_max_delta = flat_reference
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(converted.data().as_chunks::<4>().0.iter())
        .map(|(a, b)| a[3].abs_diff(b[3]))
        .max()
        .unwrap_or(0);
    diff.mean_delta = sum as f64 / diff.pixels as f64;
    Ok((diff, direct, converted))
}
