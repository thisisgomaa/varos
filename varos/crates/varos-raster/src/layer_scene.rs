//! Tiny-skia geometry adapter for the renderer-level layer contract.
use crate::{
    layers::{self, Blend, CpuLayers, LayerPrim, Limits, Pixel, Prim, Report},
    Raster,
};
use tiny_skia::{Pixmap, Transform};
use varos_core::Group;

/// Integration w3: limits for the appearance producer on the CPU. Admission is the preflight in
/// `appearance_budget` (actual temporary bytes, depth 12), which runs before any allocation; this
/// reservation is therefore generous so a list that passed the preflight never flattens here.
const COMPOSITE_LIMITS: Limits = Limits { depth: 12, bytes: usize::MAX / 2 };

/// One `Draw` payload: a run of scene groups, or one object's prims drawn opaquely inside its layer.
enum Item<'a> {
    Group(&'a Group),
    Object(&'a [varos_core::Prim]),
}

/// Lower a wave-3 layer group into the renderer's layer list: an appearance composite
/// (`Group::Composite` = opacity + optional alpha mask, Normal blend) or a CMYK overprint object
/// (`Group::Overprint` = one object in a Multiply layer). Nested composites become nested layers on
/// the SAME stack; runs of other groups are one `Draw` payload each (painted by the existing tiny-skia
/// scene painter).
fn lower<'a>(group: &'a Group, size: [u32; 2], xf: Transform, out: &mut Vec<LayerPrim<Vec<Item<'a>>>>) {
    match group {
        Group::Composite { opacity, members, mask } => {
            let mask = mask.as_ref().map(|groups| coverage(groups, size, xf));
            out.push(LayerPrim::LayerBegin { opacity: *opacity, blend: Blend::Normal, mask });
            for g in members {
                lower(g, size, xf, out);
            }
            out.push(LayerPrim::LayerEnd);
        }
        Group::Overprint { opacity, prims } => {
            out.push(LayerPrim::LayerBegin { opacity: *opacity, blend: Blend::Multiply, mask: None });
            out.push(LayerPrim::Draw(vec![Item::Object(prims)]));
            out.push(LayerPrim::LayerEnd);
        }
        other => match out.last_mut() {
            Some(LayerPrim::Draw(run)) => run.push(Item::Group(other)),
            _ => out.push(LayerPrim::Draw(vec![Item::Group(other)])),
        },
    }
}

/// Alpha-mask coverage (colour-independent, ADR: alpha masks read alpha only) in canvas pixel order.
fn coverage(groups: &[Group], size: [u32; 2], xf: Transform) -> Vec<f32> {
    let n = size[0] as usize * size[1] as usize;
    let Some(mut pixmap) = Pixmap::new(size[0], size[1]) else { return vec![0.0; n] };
    super::draw_groups(groups, &mut pixmap, xf);
    pixmap.pixels().iter().map(|p| f32::from(p.alpha()) / 255.0).collect()
}

fn to_float(bytes: &[u8], out: &mut [Pixel]) {
    for (p, c) in out.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *p = c.map(|v| f32::from(v) / 255.0);
    }
}
fn to_bytes(pixels: &[Pixel], out: &mut [u8]) {
    for (c, p) in out.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
        let a = (p[3].clamp(0.0, 1.0) * 255.0).round() as u8;
        // premultiplied: a channel never exceeds alpha after rounding
        *c = std::array::from_fn(|i| if i == 3 { a } else { ((p[i].clamp(0.0, 1.0) * 255.0).round() as u8).min(a) });
    }
}

/// CPU executor of `Group::Composite` and `Group::Overprint`: the render lane's `CpuLayers` (premultiplied f32,
/// `layers::composite`, mask applied once at LayerEnd) is the one compositing mechanism.
pub(crate) fn draw_composite(group: &Group, dst: &mut Pixmap, xf: Transform) {
    let size = [dst.width(), dst.height()];
    let mut prims = Vec::new();
    lower(group, size, xf, &mut prims);
    let Some(mut scratch) = Pixmap::new(size[0], size[1]) else { return };
    let mut pixels = vec![[0.0; 4]; size[0] as usize * size[1] as usize];
    to_float(dst.data(), &mut pixels);
    let drawn = CpuLayers::default().render(&prims, size, 1.0, COMPOSITE_LIMITS, &mut pixels, |run, layer| {
        to_bytes(layer, scratch.data_mut());
        for item in run {
            match item {
                Item::Group(g) => super::draw_groups(std::slice::from_ref(*g), &mut scratch, xf),
                // the object is drawn opaquely in its own layer, exactly as an isolated object is
                Item::Object(prims) if super::gradient::isolated_knockout(prims) => {
                    super::draw_knockout(prims, &mut scratch, xf)
                }
                Item::Object(prims) => super::draw_prims(prims, &mut scratch, xf),
            }
        }
        to_float(scratch.data(), layer);
    });
    if drawn.is_ok() {
        to_bytes(&pixels, dst.data_mut());
    }
}

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

    #[test]
    fn appearance_composites_lower_onto_one_layer_stack() {
        let square = |c: [f32; 4]| {
            Group::Opaque(vec![varos_core::Prim::Fill {
                rings: vec![vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]]],
                color: c,
            }])
        };
        let nested = Group::Composite {
            opacity: 0.5,
            members: vec![square([1.0, 0.0, 0.0, 1.0]), Group::Composite { opacity: 1.0, members: vec![], mask: None }],
            mask: Some(vec![square([0.0, 0.0, 0.0, 0.5])]),
        };
        let mut prims = Vec::new();
        lower(&nested, [4, 4], Transform::identity(), &mut prims);
        let shape: Vec<_> = prims
            .iter()
            .map(|p| match p {
                LayerPrim::LayerBegin { mask, .. } => {
                    if mask.is_some() {
                        "begin+mask"
                    } else {
                        "begin"
                    }
                }
                LayerPrim::LayerEnd => "end",
                LayerPrim::Draw(_) => "draw",
                _ => "effect",
            })
            .collect();
        assert_eq!(shape, ["begin+mask", "draw", "begin", "end", "end"]);
        let mut pixmap = Pixmap::new(4, 4).unwrap();
        draw_composite(&nested, &mut pixmap, Transform::identity());
        // red × opacity 0.5 × mask alpha 128/255, composited by layers::composite (Normal)
        let expect = layers::composite(Blend::Normal, [0.0; 4], [1.0, 0.0, 0.0, 1.0], 0.5 * 128.0 / 255.0);
        let px = pixmap.pixels()[5];
        assert_eq!([px.red(), px.alpha()], [(expect[0] * 255.0).round() as u8, (expect[3] * 255.0).round() as u8]);
    }
}
