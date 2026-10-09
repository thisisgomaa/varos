//! Image scene consumer; shares transforms, clipping, alpha and filtering with vector groups.
use std::sync::Arc;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use varos_core::{
    images::{BlobStore, Pixels},
    model::Document,
    scene::build_artwork_scene,
    Editor,
};
pub(crate) fn draw(
    p: &Pixels,
    c: [[f32; 2]; 4],
    opacity: f32,
    clip: Option<[f32; 4]>,
    dst: &mut Pixmap,
    view: Transform,
) {
    let mut rgba = p.rgba.to_vec();
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        for v in &mut px[..3] {
            *v = ((u16::from(*v) * a + 127) / 255) as u8;
        }
    }
    let Some(size) = tiny_skia::IntSize::from_wh(p.width, p.height) else { return };
    let Some(src) = Pixmap::from_vec(rgba, size) else { return };
    let w = p.width as f32;
    let h = p.height as f32;
    let image = Transform::from_row(
        (c[1][0] - c[0][0]) / w,
        (c[1][1] - c[0][1]) / w,
        (c[3][0] - c[0][0]) / h,
        (c[3][1] - c[0][1]) / h,
        c[0][0],
        c[0][1],
    );
    let mask = clip.and_then(|r| {
        let rect = tiny_skia::Rect::from_ltrb(r[0], r[1], r[2], r[3])?;
        let path = tiny_skia::PathBuilder::from_rect(rect);
        let mut mask = tiny_skia::Mask::new(dst.width(), dst.height())?;
        mask.fill_path(&path, tiny_skia::FillRule::Winding, true, view);
        Some(mask)
    });
    if clip.is_some() && mask.is_none() {
        return;
    }
    dst.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &PixmapPaint { opacity, quality: FilterQuality::Bilinear, ..Default::default() },
        view.pre_concat(image),
        mask.as_ref(),
    );
}
pub fn rasterize_with_images(
    doc: &Document,
    blobs: &BlobStore,
    size: [u32; 2],
    pan: [f32; 2],
    ppu: f32,
    background: Option<[f32; 4]>,
) -> Result<crate::Raster, String> {
    if size.contains(&0)
        || size[0] as u64 * size[1] as u64 > 32_000_000
        || !ppu.is_finite()
        || ppu <= 0.
        || !pan.iter().all(|v| v.is_finite())
    {
        return Err("Invalid or oversized raster target".into());
    }
    let mut ed = Editor::new();
    ed.replace_doc(doc.clone());
    ed.blobs = blobs.clone();
    let scene = build_artwork_scene(&ed, ppu);
    if !scene.errors.is_empty() {
        return Err(scene.errors.join("; "));
    }
    let mut dst = Pixmap::new(size[0], size[1]).ok_or("Raster allocation refused")?;
    if let Some(c) = background {
        dst.fill(tiny_skia::Color::from_rgba(c[0], c[1], c[2], c[3]).ok_or("Invalid background")?);
    }
    super::draw_groups(&scene.content, &mut dst, Transform::from_row(ppu, 0., 0., ppu, pan[0], pan[1]));
    Ok(crate::Raster { width: dst.width(), height: dst.height(), pixels: dst.take(), errors: vec![] })
}
/// Render only the target, bake its appearance once, then publish one undoable replacement.
pub fn rasterize_object(ed: &mut Editor, id: u32, ppi: f32, background: Option<[f32; 4]>) -> Result<(), String> {
    use varos_core::{
        images::{ImageAffine, ImageEdit},
        EditCommand,
    };
    if !ppi.is_finite() || !(1.0..=2400.).contains(&ppi) {
        return Err("Invalid rasterize ppi".into());
    }
    let is_image = ed.doc.images.iter().any(|i| i.id == id);
    let group = ed.doc.node(id).filter(|n| n.kind == varos_core::model::NodeKind::Group).map(|n| n.id);
    if !is_image && ed.doc.pidx(id).is_none() && group.is_none() {
        return Err("Object not found".into());
    }
    if ed.doc.eff_locked(id) || ed.doc.eff_hidden(id) {
        return Err("Object is hidden or locked".into());
    }
    let mut source = ed.doc.clone();
    let mut ids: std::collections::HashSet<u32> = [id].into_iter().collect();
    if let Some(group) = group {
        let mut pending = vec![group];
        while let Some(id) = pending.pop() {
            if let Some(n) = ed.doc.node(id) {
                match n.kind {
                    varos_core::model::NodeKind::Path(p) | varos_core::model::NodeKind::Image(p) => {
                        ids.insert(p);
                    }
                    _ => {}
                }
                pending.extend(n.children.iter().copied());
            }
        }
    }
    for p in &mut source.paths {
        if !ids.contains(&p.id) && !ed.doc.is_mask_source(p.id) {
            p.hidden = true;
        }
    }
    varos_core::images::hide_unselected(&mut source, &ids);
    let mut view = Editor::new();
    view.replace_doc(source.clone());
    view.blobs = ed.blobs.clone();
    let scene = build_artwork_scene(&view, ppi / 72.);
    if !scene.errors.is_empty() {
        return Err(scene.errors.join("; "));
    }
    let [x, y, x1, y1] = super::scene_bounds(&scene.content).ok_or("Object paints nothing")?;
    let (w, h) = ((x1 - x).max(0.01), (y1 - y).max(0.01));
    let scale = ppi / 72.;
    let size = [(w * scale).ceil().max(1.) as u32, (h * scale).ceil().max(1.) as u32];
    let raster = rasterize_with_images(&source, &ed.blobs, size, [-x * scale, -y * scale], scale, background)?;
    let mut rgba = raster.pixels;
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        for v in &mut px[..3] {
            *v = (*v as u32 * 255 + a / 2).checked_div(a).unwrap_or(0).min(255) as u8;
        }
    }
    let bytes = varos_core::images::codec::encode_png(&Pixels {
        budget: None,
        width: raster.width,
        height: raster.height,
        rgba: Arc::from(rgba),
    })?;
    let mut decoded = varos_core::images::codec::decode(&bytes)?;
    decoded.ppi = [ppi; 2];
    let parent = ed.doc.node_of_path(id).or(group).and_then(|n| ed.doc.node(n)).and_then(|n| n.parent);
    let mut chain = vec![];
    let mut node = parent;
    while let Some(id) = node {
        let n = ed.doc.node(id).ok_or("Missing ancestor")?;
        chain.push(n.xform);
        node = n.parent;
    }
    let mut points = [[x, y], [x + w, y], [x, y + h]];
    for xf in chain.into_iter().rev() {
        points = points.map(|p| xf.inverse_apply(p));
    }
    let mut staged = ed.clone();
    let mut image = varos_core::images::stage(&mut staged, decoded, [0.; 2], Default::default(), None)?;
    let xf = ImageAffine {
        a: (points[1][0] - points[0][0]) / image.px_w as f32,
        b: (points[1][1] - points[0][1]) / image.px_w as f32,
        c: (points[2][0] - points[0][0]) / image.px_h as f32,
        d: (points[2][1] - points[0][1]) / image.px_h as f32,
        e: points[0][0],
        f: points[0][1],
    };
    image.xform = xf;
    staged.try_execute(EditCommand::Image(ImageEdit::Rasterized { target: id, image }))?;
    *ed = staged;
    Ok(())
}

pub fn fitted(
    doc: &Document,
    blobs: &BlobStore,
    size: [u32; 2],
    artboard: Option<u32>,
) -> Result<crate::Raster, String> {
    let bounds = if let Some(id) = artboard {
        let i = doc.artboard_index(id).ok_or("Unknown artboard")?;
        let a = &doc.artboards[i];
        [a.x, a.y, a.w, a.h]
    } else {
        let mut ed = Editor::new();
        ed.replace_doc(doc.clone());
        ed.blobs = blobs.clone();
        let scene = build_artwork_scene(&ed, 1.);
        if !scene.errors.is_empty() {
            return Err(scene.errors.join("; "));
        }
        {
            let [x, y, x1, y1] = super::scene_bounds(&scene.content).unwrap_or([0., 0., 72., 72.]);
            [x, y, (x1 - x).max(1.), (y1 - y).max(1.)]
        }
    };
    let [x, y, w, h] = bounds;
    let scale = (size[0] as f32 / w).min(size[1] as f32 / h);
    rasterize_with_images(
        doc,
        blobs,
        size,
        [(size[0] as f32 - w * scale) * 0.5 - x * scale, (size[1] as f32 - h * scale) * 0.5 - y * scale],
        scale,
        None,
    )
}
