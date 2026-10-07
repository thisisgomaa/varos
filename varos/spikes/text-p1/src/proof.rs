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
    /// Only populated by the historical conversion diagnostic.
    pub polygon_max_delta: Option<u8>,
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
    diff.polygon_max_delta = Some(
        flat_reference
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(converted.data().as_chunks::<4>().0.iter())
            .map(|(a, b)| a[3].abs_diff(b[3]))
            .max()
            .unwrap_or(0),
    );
    diff.mean_delta = sum as f64 / diff.pixels as f64;
    Ok((diff, direct, converted))
}

/// Native NonZero contract. Each glyph has independent winding coverage so
/// opposite font conventions cannot cancel. Same-paint glyph coverage is max
/// composed before opacity is applied once (explicit prototype policy).
/// Flatten tolerance 0.005 pt at 2x; unchanged P1 alpha thresholds.
/// Native Winding at 4x4 samples/output pixel then a premultiplied box filter.
pub fn nonzero_diff(outlines: &[Outline], scale: f32) -> Result<(Diff, Pixmap, Pixmap), &'static str> {
    let bounds = ink_bounds(outlines).unwrap_or([0., 0., 1., 1.]);
    let width = ((bounds[2] - bounds[0]) * scale).ceil() as u32 + 12;
    let height = ((bounds[3] - bounds[1]) * scale).ceil() as u32 + 12;
    if u64::from(width) * u64::from(height) > 64_000_000 {
        return Err("proof raster resource limit");
    }
    let transform = Transform::from_row(scale, 0., 0., scale, 6. - bounds[0] * scale, 6. - bounds[1] * scale);
    let direct = nonzero_coverage(outlines, width, height, transform, None, 255)?;
    let flat = nonzero_coverage(outlines, width, height, transform, Some(0.01 / scale), 255)?;
    let diff = alpha_diff(&direct, &flat);
    Ok((diff, direct, flat))
}
pub fn alpha_diff(a: &Pixmap, b: &Pixmap) -> Diff {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let mut d = Diff { pixels: (a.width() * a.height()) as usize, ..Diff::default() };
    let mut sum = 0u64;
    for (a, b) in a.pixels().iter().zip(b.pixels()) {
        let delta = a.alpha().abs_diff(b.alpha());
        d.differing += usize::from(delta > 0);
        d.severe += usize::from(delta > 32);
        d.max_delta = d.max_delta.max(delta);
        sum += u64::from(delta);
        d.interior_flips += usize::from((a.alpha() == 255 && b.alpha() == 0) || (b.alpha() == 255 && a.alpha() == 0));
    }
    d.mean_delta = sum as f64 / d.pixels as f64;
    d
}
/// Native CPU policy: 4x4 subpixel samples, winding per glyph, max union,
/// box-filter to the requested output scale, then apply opacity once.
/// The 64M intermediate-pixel cap bounds the working allocation; this is a
/// headless quality prototype, not a real-time/GPU implementation.
pub fn nonzero_coverage(
    outlines: &[Outline],
    width: u32,
    height: u32,
    transform: Transform,
    flatten: Option<f32>,
    alpha: u8,
) -> Result<Pixmap, &'static str> {
    let w = width.checked_mul(4).ok_or("proof raster resource limit")?;
    let h = height.checked_mul(4).ok_or("proof raster resource limit")?;
    let t = Transform::from_row(
        transform.sx * 4.,
        transform.ky * 4.,
        transform.kx * 4.,
        transform.sy * 4.,
        transform.tx * 4.,
        transform.ty * 4.,
    );
    let high = nonzero_coverage_single_sample(outlines, w, h, t, flatten, 255)?;
    let mut output = downsample_4x(&high)?;
    for v in output.data_mut() {
        *v = ((u16::from(*v) * u16::from(alpha) + 127) / 255) as u8;
    }
    Ok(output)
}
/// Fixed box filter shared by external-renderer comparisons. The input is
/// premultiplied RGBA; no gamma conversion, threshold, dilation or pixel ignore.
pub fn downsample_4x(input: &Pixmap) -> Result<Pixmap, &'static str> {
    if !input.width().is_multiple_of(4) || !input.height().is_multiple_of(4) {
        return Err("4x dimensions required");
    }
    let (w, h) = (input.width() / 4, input.height() / 4);
    let mut output = Pixmap::new(w, h).ok_or("pixmap")?;
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 4];
            for dy in 0..4 {
                for dx in 0..4 {
                    let i = (((y * 4 + dy) * input.width() + x * 4 + dx) * 4) as usize;
                    for (s, v) in sum.iter_mut().zip(&input.data()[i..i + 4]) {
                        *s += u32::from(*v);
                    }
                }
            }
            let i = ((y * w + x) * 4) as usize;
            for (v, s) in output.data_mut()[i..i + 4].iter_mut().zip(sum) {
                *v = ((s + 8) / 16) as u8;
            }
        }
    }
    Ok(output)
}
/// Historical single-sample renderer, retained for independent diagnostics.
pub fn nonzero_coverage_single_sample(
    outlines: &[Outline],
    width: u32,
    height: u32,
    transform: Transform,
    flatten: Option<f32>,
    alpha: u8,
) -> Result<Pixmap, &'static str> {
    if u64::from(width) * u64::from(height) > 64_000_000 {
        return Err("proof raster resource limit");
    }
    let mut result = Pixmap::new(width, height).ok_or("pixmap")?;
    let mut scratch = result.clone();
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    for outline in outlines {
        scratch.fill(tiny_skia::Color::TRANSPARENT);
        let path = match flatten {
            Some(t) => rings_path(&outline.flatten(t)),
            None => outline.path(),
        };
        if let Some(path) = path {
            scratch.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
        for (dst, src) in result.data_mut().iter_mut().zip(scratch.data()) {
            *dst = (*dst).max(*src);
        }
    }
    for v in result.data_mut() {
        *v = ((u16::from(*v) * u16::from(alpha) + 127) / 255) as u8;
    }
    Ok(result)
}
