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

/// One `Draw` payload: a run of scene groups, one object's prims drawn opaquely inside its layer, or
/// the layer's alpha mask. Masks are STREAMED (review P1): coverage is rasterised when the layer is
/// complete, applied to it, and dropped — never retained for every sibling before rendering.
enum Item<'a> {
    Group(&'a Group),
    Object(&'a [varos_core::Prim]),
    /// Multiply the finished layer by these groups' alpha (same as the stack's LayerEnd mask, which is
    /// applied after the layer's content and before its composite).
    Mask(&'a [Group]),
}

/// Lower a wave-3 layer group into the renderer's layer list: an appearance composite
/// (`Group::Composite` = opacity + optional alpha mask, Normal blend) or a CMYK overprint object
/// (`Group::Overprint` = one object in a Multiply layer). Nested composites become nested layers on
/// the SAME stack; runs of other groups are one `Draw` payload each (painted by the existing tiny-skia
/// scene painter).
fn lower<'a>(group: &'a Group, out: &mut Vec<LayerPrim<Vec<Item<'a>>>>) {
    match group {
        Group::Composite { opacity, members, mask } => {
            out.push(LayerPrim::LayerBegin { opacity: *opacity, blend: Blend::Normal, mask: None });
            for g in members {
                lower(g, out);
            }
            if let Some(groups) = mask {
                // a run of its own: always the layer's last draw, after every member
                out.push(LayerPrim::Draw(vec![Item::Mask(groups)]));
            }
            out.push(LayerPrim::LayerEnd);
        }
        Group::Overprint { opacity, prims } => {
            out.push(LayerPrim::LayerBegin { opacity: *opacity, blend: Blend::Multiply, mask: None });
            out.push(LayerPrim::Draw(vec![Item::Object(prims)]));
            out.push(LayerPrim::LayerEnd);
        }
        other => match out.last_mut() {
            Some(LayerPrim::Draw(run)) if !run.iter().any(|i| matches!(i, Item::Mask(_))) => {
                run.push(Item::Group(other))
            }
            _ => out.push(LayerPrim::Draw(vec![Item::Group(other)])),
        },
    }
}

/// Multiply a finished layer by an alpha mask (colour-independent, ADR: alpha masks read alpha only).
/// The 8-bit coverage surface lives only for this call.
fn apply_mask(groups: &[Group], size: [u32; 2], xf: Transform, layer: &mut [Pixel]) {
    let Some(mut pixmap) = Pixmap::new(size[0], size[1]) else {
        layer.fill([0.0; 4]);
        return;
    };
    super::draw_groups(groups, &mut pixmap, xf);
    for (p, c) in layer.iter_mut().zip(pixmap.pixels()) {
        let m = f32::from(c.alpha()) / 255.0;
        *p = p.map(|v| v * m);
    }
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
    lower(group, &mut prims);
    let Some(mut scratch) = Pixmap::new(size[0], size[1]) else { return };
    let mut pixels = vec![[0.0; 4]; size[0] as usize * size[1] as usize];
    to_float(dst.data(), &mut pixels);
    let drawn = CpuLayers::default().render(&prims, size, 1.0, COMPOSITE_LIMITS, &mut pixels, |run, layer| {
        if let [Item::Mask(groups)] = run.as_slice() {
            apply_mask(groups, size, xf, layer); // one transient coverage surface at a time
            return;
        }
        to_bytes(layer, scratch.data_mut());
        for item in run {
            match item {
                Item::Mask(_) => {}
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
        lower(&nested, &mut prims);
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
                LayerPrim::Draw(run) if matches!(run.as_slice(), [Item::Mask(_)]) => "mask",
                LayerPrim::Draw(_) => "draw",
                _ => "effect",
            })
            .collect();
        assert_eq!(shape, ["begin", "draw", "begin", "end", "mask", "end"]);
        let mut pixmap = Pixmap::new(4, 4).unwrap();
        draw_composite(&nested, &mut pixmap, Transform::identity());
        // red × opacity 0.5 × mask alpha 128/255, composited by layers::composite (Normal)
        let expect = layers::composite(Blend::Normal, [0.0; 4], [1.0, 0.0, 0.0, 1.0], 0.5 * 128.0 / 255.0);
        let px = pixmap.pixels()[5];
        assert_eq!([px.red(), px.alpha()], [(expect[0] * 255.0).round() as u8, (expect[3] * 255.0).round() as u8]);
    }

    /// Review P1 probe: 100 masked siblings at 2048². Lowering retains no per-pixel mask storage
    /// (every mask is a deferred draw, rasterised and dropped one at a time), the preflight admits
    /// the document on its real streamed cost, and the masks still apply (normal case unchanged).
    #[test]
    fn hundred_masked_siblings_stream_their_masks() {
        let square = |x: f32, c: [f32; 4]| {
            Group::Opaque(vec![varos_core::Prim::Fill {
                rings: vec![vec![[x, 0.0], [x + 2.0, 0.0], [x + 2.0, 4.0], [x, 4.0], [x, 0.0]]],
                color: c,
            }])
        };
        let sibling = |x: f32| Group::Composite {
            opacity: 1.0,
            members: vec![square(x, [1.0, 0.0, 0.0, 1.0])],
            mask: Some(vec![square(x, [0.0, 0.0, 0.0, 0.5])]),
        };
        let parent = Group::Composite { opacity: 1.0, members: (0..100).map(|_| sibling(0.0)).collect(), mask: None };
        let mut prims = Vec::new();
        lower(&parent, &mut prims);
        assert!(prims.iter().all(|p| !matches!(p, LayerPrim::LayerBegin { mask: Some(_), .. })), "no retained mask");
        let masks =
            prims.iter().filter(|p| matches!(p, LayerPrim::Draw(r) if matches!(r.as_slice(), [Item::Mask(_)]))).count();
        assert_eq!(masks, 100);
        assert!(crate::appearance_budget::check(std::slice::from_ref(&parent), [2048, 2048]).is_ok());
        // normal case: one masked sibling keeps the exact Normal composite of red × mask alpha
        let mut one = Pixmap::new(4, 4).unwrap();
        draw_composite(&sibling(0.0), &mut one, Transform::identity());
        let expect = layers::composite(Blend::Normal, [0.0; 4], [1.0, 0.0, 0.0, 1.0], 128.0 / 255.0);
        assert_eq!(one.pixels()[0].alpha(), (expect[3] * 255.0).round() as u8);
        let mut many = Pixmap::new(4, 4).unwrap();
        draw_composite(&parent, &mut many, Transform::identity());
        assert!(many.pixels()[0].alpha() > one.pixels()[0].alpha(), "every sibling composited");
        assert_eq!(many.pixels()[3].alpha(), 0, "outside every mask stays clear");
    }
}
