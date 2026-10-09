//! Image XObjects and native streams; vector appearance stays in write.rs.
use pdf_writer::{Filter, Pdf, Ref};
use std::io::Write;
use varos_core::{
    images::{self, BlobKey, BlobStore, Mime, PlacementMode},
    model::Document,
    ExportNote, ExportReport,
};
pub(crate) struct DrawImage {
    pub id: u32,
    pub r: Ref,
    pub name: String,
    pub corners: [[f32; 2]; 4],
    pub opacity: f32,
}
fn next(ids: &mut i32) -> Ref {
    *ids += 1;
    Ref::new(*ids)
}
fn compress(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    z.write_all(bytes).map_err(|e| e.to_string())?;
    z.finish().map_err(|e| e.to_string())
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    doc: &Document,
    store: &BlobStore,
    ppi: f32,
    preview: bool,
    pdf: &mut Pdf,
    ids: &mut i32,
    report: &mut ExportReport,
) -> Result<Vec<DrawImage>, String> {
    let mut result = Vec::new();
    let mut pool: std::collections::HashMap<(BlobKey, [u32; 2]), (Ref, String)> = std::collections::HashMap::new();
    for i in &doc.images {
        if images::image_hidden(doc, i.id) || doc.is_mask_source(i.id) || i.opacity == 0. {
            continue;
        }
        let blob = store.get(&i.blob).ok_or("Missing image resource")?;
        if (blob.original.is_none() || blob.pixels.width != blob.meta.px_w || blob.pixels.height != blob.meta.px_h)
            && !preview
        {
            return Err("Production PDF requires full image originals".into());
        }
        if blob.pixels.width != blob.meta.px_w || blob.pixels.height != blob.meta.px_h {
            report.notes.push(ExportNote {
                kind: "image_proxy".into(),
                object_id: Some(i.id),
                message: "Proxy-only appearance; decoded original unavailable".into(),
            });
        }
        let corners = images::world_corners(doc, i);
        let size = |a: [f32; 2], b: [f32; 2]| ((a[0] - b[0]).hypot(a[1] - b[1]) * ppi / 72.).ceil().max(1.) as u32;
        let dims =
            [blob.pixels.width.min(size(corners[1], corners[0])), blob.pixels.height.min(size(corners[3], corners[0]))];
        let pool_key = (i.blob.clone(), dims);
        let (r, name) = if let Some(pair) = pool.get(&pool_key) {
            pair.clone()
        } else {
            let r = next(ids);
            let jpeg =
                blob.meta.mime == Mime::Jpeg && blob.meta.orientation == 1 && dims == [blob.meta.px_w, blob.meta.px_h];
            if let Some(bytes) = blob.original.as_ref().filter(|bytes| jpeg && images::codec::jpeg_rgb_original(bytes))
            {
                let mut x = pdf.image_xobject(r, bytes);
                x.width(dims[0] as i32).height(dims[1] as i32).bits_per_component(8).filter(Filter::DctDecode);
                x.color_space().device_rgb();
            } else {
                let pixels = if dims == [blob.pixels.width, blob.pixels.height] {
                    (*blob.pixels).clone()
                } else {
                    report.notes.push(ExportNote {
                        kind: "image_downsample".into(),
                        object_id: Some(i.id),
                        message: format!("PDF image sampled to {} × {} at {ppi} ppi", dims[0], dims[1]),
                    });
                    images::codec::resize_pixels(&blob.pixels, dims)?
                };
                let mut rgb = Vec::with_capacity(pixels.rgba.len() / 4 * 3);
                let mut alpha = Vec::with_capacity(pixels.rgba.len() / 4);
                for p in pixels.rgba.as_chunks::<4>().0 {
                    rgb.extend_from_slice(&p[..3]);
                    alpha.push(p[3]);
                }
                let alpha_ref = next(ids);
                let alpha = compress(&alpha)?;
                let mut a = pdf.image_xobject(alpha_ref, &alpha);
                a.width(dims[0] as i32).height(dims[1] as i32).bits_per_component(8).filter(Filter::FlateDecode);
                a.color_space().device_gray();
                drop(a);
                let rgb = compress(&rgb)?;
                let mut x = pdf.image_xobject(r, &rgb);
                x.width(dims[0] as i32)
                    .height(dims[1] as i32)
                    .bits_per_component(8)
                    .s_mask(alpha_ref)
                    .filter(Filter::FlateDecode);
                x.color_space().device_rgb();
            }
            let pair = (r, format!("Im{}", r.get()));
            pool.insert(pool_key, pair.clone());
            pair
        };
        result.push(DrawImage { id: i.id, r, name, corners, opacity: i.opacity });
    }
    Ok(result)
}
type AssetRefs = Vec<(BlobKey, Option<Ref>, Ref)>;
pub(crate) fn assets(doc: &Document, store: &BlobStore, pdf: &mut Pdf, ids: &mut i32) -> Result<AssetRefs, String> {
    let mut refs = Vec::new();
    for meta in &doc.assets {
        let blob = store.get(&meta.key).ok_or("Missing manifest resource")?;
        let embed = doc.images.iter().any(|i| i.blob == meta.key && i.placement == PlacementMode::Embed);
        let original = if embed {
            let bytes = blob.original.as_ref().ok_or("Embedded image original is missing")?;
            let r = next(ids);
            pdf.stream(r, bytes);
            Some(r)
        } else {
            None
        };
        let proxy = next(ids);
        let bytes = images::codec::encode_png(&blob.proxy)?;
        pdf.stream(proxy, &bytes);
        refs.push((meta.key.clone(), original, proxy));
    }
    Ok(refs)
}
