//! Tiny-skia geometry adapter for the renderer-level layer contract.
use crate::{
    layers::{self, CpuLayers, Limits, Pixel, Prim, Report},
    Raster,
};
use tiny_skia::{Pixmap, Transform};

/// Render an explicit layer draw list headlessly. All geometry/effects use the same zoom bucket.
/// Object revisions must describe the complete input layer and pan; see layers::Prim::Blur.
#[allow(clippy::too_many_arguments)]
pub fn rasterize_layers(
    cache: &mut CpuLayers,
    prims: &[Prim],
    size: [u32; 2],
    pan: [f32; 2],
    zoom: f32,
    background: Pixel,
    limits: Limits,
) -> Result<(Raster, Report), String> {
    layers::validate(prims, size, zoom)?;
    if !pan.iter().all(|v| v.is_finite())
        || background.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        || background[..3].iter().any(|v| *v > background[3])
    {
        return Err("invalid layer pan/background".into());
    }
    let mut dst = vec![background; size[0] as usize * size[1] as usize];
    let zoom = layers::bucket_zoom(zoom);
    let xf = Transform::from_row(zoom, 0.0, 0.0, zoom, pan[0], pan[1]);
    let mut error = None;
    let report = cache.render(prims, size, zoom, limits, &mut dst, |p, pixels| {
        let Some(dim) = tiny_skia::IntSize::from_wh(size[0], size[1]) else {
            error = Some("invalid layer dimensions");
            return;
        };
        let bytes: Vec<_> = pixels.iter().flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
        let Some(mut pixmap) = Pixmap::from_vec(bytes, dim) else {
            error = Some("layer allocation failed");
            return;
        };
        super::draw_prims(std::slice::from_ref(p), &mut pixmap, xf);
        for (p, bytes) in pixels.iter_mut().zip(pixmap.data().as_chunks::<4>().0) {
            *p = std::array::from_fn(|i| bytes[i] as f32 / 255.0);
        }
    })?;
    if let Some(e) = error {
        return Err(e.into());
    }
    let pixels = dst.iter().flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
    Ok((Raster { width: size[0], height: size[1], pixels, errors: Vec::new() }, report))
}
#[cfg(test)]
mod tests {
    use super::*;
    use layers::Blend;
    #[test]
    fn tiny_skia_geometry_uses_layer_mask_and_opacity() {
        let prims = [
            Prim::LayerBegin { opacity: 0.5, blend: Blend::Multiply, mask: Some(vec![0.5; 16]) },
            Prim::Draw(varos_core::Prim::Fill {
                rings: vec![vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]]],
                color: [1.0, 0.0, 0.0, 1.0],
            }),
            Prim::LayerEnd,
        ];
        let (r, report) =
            rasterize_layers(&mut CpuLayers::default(), &prims, [4, 4], [0.0; 2], 1.0, [1.0; 4], Limits::default())
                .unwrap();
        assert_eq!(&r.pixels[20..24], &[255, 191, 191, 255]);
        assert_eq!(report.passes, 1);
    }
}
