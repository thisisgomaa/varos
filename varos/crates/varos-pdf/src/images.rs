//! Resource-aware native container and appearance export. Binary originals never enter model JSON.
use crate::export::PageSpec;
use pdf_writer::{Content, Finish, Name, Pdf, Ref};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};
use varos_core::{
    format::{encode_model, Limits, LoadError, Loaded},
    images::{Blob, BlobKey, BlobStore, PlacementMode},
    model::Document,
    scene::{Group, Prim},
    Editor, ExportNote, ExportReport,
};
fn next(id: &mut i32) -> Ref {
    *id += 1;
    Ref::new(*id)
}
pub fn write_vrs(doc: &Document, store: &BlobStore, limits: &Limits) -> Result<Vec<u8>, String> {
    if doc.images.is_empty() {
        return crate::write_pdf_checked(doc, limits).map_err(|e| e.to_string());
    }
    let model = encode_model(doc, limits).map_err(|e| e.to_string())?;
    let (bytes, _) =
        write(doc, store, &crate::write::native_pages(doc), Some(&model), 300., false, &AtomicBool::new(false))?;
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err("Image container exceeds file limit".into());
    }
    // Symmetric gate checks binary streams, aggregate decoded bytes, metadata/hash and PDF structure.
    crate::load_vrs_bytes(&bytes, limits).map_err(|e| e.to_string())?;
    Ok(bytes)
}
pub fn export_pdf(
    doc: &Document,
    store: &BlobStore,
    pages: &[PageSpec],
    ppi: f32,
    preview: bool,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, ExportReport), String> {
    write(doc, store, pages, None, ppi, preview, cancel)
}
#[allow(clippy::too_many_arguments)]
fn write(
    doc: &Document,
    store: &BlobStore,
    pages: &[PageSpec],
    model: Option<&str>,
    ppi: f32,
    preview: bool,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, ExportReport), String> {
    if !ppi.is_finite() || !(1.0..=2400.).contains(&ppi) {
        return Err("Invalid PDF image ppi".into());
    }
    varos_core::format::validate(doc, &Limits::DEFAULT).map_err(|e| e.to_string())?;
    let mut ed = Editor::new();
    ed.replace_doc(doc.clone());
    ed.blobs = store.clone();
    let scene = varos_core::scene::build_artwork_scene(&ed, 4.);
    if !scene.errors.is_empty() {
        return Err(scene.errors.join("; "));
    }
    let mut report = ExportReport::default();
    report.notes.extend(scene.report.notes);
    if model.is_none() {
        report.notes.extend(varos_core::images::export_notes(doc, store));
    }
    if !doc.paths.is_empty() {
        report.notes.push(ExportNote {
            kind: "image_pdf_vector_sampling".into(),
            object_id: None,
            message: "Image-aware PDF uses scene geometry at 0.25 pt sampling".into(),
        });
    }
    let mut pdf = Pdf::new();
    let mut ids = 0;
    let catalog = next(&mut ids);
    let tree = next(&mut ids);
    let mut pool = HashMap::new();
    for i in &doc.images {
        if varos_core::images::image_hidden(doc, i.id) || i.opacity == 0. {
            continue;
        }

        let blob = store.get(&i.blob).ok_or("Missing image resource")?;
        if blob.original.is_none() && !preview && model.is_none() {
            return Err("Production PDF requires full image originals".into());
        }
        if blob.original.is_none() {
            report.notes.push(ExportNote {
                kind: "image_proxy".into(),
                object_id: Some(i.id),
                message: "Proxy-only image; original unavailable".into(),
            });
        }
        let p = &blob.pixels;
        let c = varos_core::images::world_corners(doc, i);
        let sx = (c[1][0] - c[0][0]).hypot(c[1][1] - c[0][1]) * ppi / 72.;
        let sy = (c[3][0] - c[0][0]).hypot(c[3][1] - c[0][1]) * ppi / 72.;
        let dims = [p.width.min(sx.ceil().max(1.) as u32), p.height.min(sy.ceil().max(1.) as u32)];
        let pool_key = (i.blob.clone(), dims);
        if pool.contains_key(&pool_key) {
            continue;
        }
        let sampled;
        if dims != [p.width, p.height] {
            sampled = varos_core::images::codec::resize_pixels(p, dims)?;
            report.notes.push(ExportNote {
                kind: "image_downsample".into(),
                object_id: Some(i.id),
                message: format!("PDF image sampled to {} × {} at {ppi} ppi", dims[0], dims[1]),
            });
        } else {
            sampled = (**p).clone();
        }
        let r = next(&mut ids);
        let alpha = next(&mut ids);
        let mut rgb = Vec::with_capacity(sampled.rgba.len() / 4 * 3);
        let mut a = Vec::with_capacity(sampled.rgba.len() / 4);
        for px in sampled.rgba.as_chunks::<4>().0 {
            rgb.extend_from_slice(&px[..3]);
            a.push(px[3]);
        }
        {
            let mut x = pdf.image_xobject(alpha, &a);
            x.width(dims[0] as i32).height(dims[1] as i32).bits_per_component(8);
            x.color_space().device_gray();
        }
        {
            let mut x = pdf.image_xobject(r, &rgb);
            x.width(dims[0] as i32).height(dims[1] as i32).bits_per_component(8).s_mask(alpha);
            x.color_space().device_rgb();
        }
        pool.insert(pool_key, (r, format!("Im{}", r.get())));
    }
    let mut page_ids = vec![];
    for page in pages {
        if cancel.load(Ordering::Relaxed) {
            return Err("PDF export cancelled".into());
        }
        let [x, y, w, h] = page.rect;
        if ![x, y, w, h].iter().all(|v| v.is_finite()) || w <= 0. || h <= 0. {
            return Err("Invalid PDF page".into());
        }
        let page_id = next(&mut ids);
        let content = next(&mut ids);
        let mut c = Content::new();
        c.save_state();
        c.transform([1., 0., 0., -1., -x, y + h]);
        if let Some(bg) = page.background {
            c.set_fill_rgb(bg[0], bg[1], bg[2]);
            c.rect(x, y, w, h);
            c.fill_nonzero();
        }
        let mut alphas = vec![];
        paint(&scene.content, &mut c, &pool, &mut pdf, &mut ids, &mut alphas, doc, store, ppi)?;
        c.restore_state();
        pdf.stream(content, &c.finish());
        let mut p = pdf.page(page_id);
        p.parent(tree).media_box(pdf_writer::Rect::new(0., 0., w, h)).contents(content);
        {
            let mut res = p.resources();
            {
                let mut xo = res.x_objects();
                for (r, name) in pool.values() {
                    xo.pair(Name(name.as_bytes()), *r);
                }
            }
            {
                let mut gs = res.ext_g_states();
                for (r, name) in &alphas {
                    gs.pair(Name(name.as_bytes()), *r);
                }
            }
        }
        p.finish();
        page_ids.push(page_id);
    }
    pdf.pages(tree).kids(page_ids.iter().copied()).count(page_ids.len() as i32);
    let mut asset_refs = vec![];
    if model.is_some() {
        for meta in &doc.assets {
            let blob = store.get(&meta.key).ok_or("Missing manifest resource")?;
            let embed = doc.images.iter().any(|i| i.blob == meta.key && i.placement == PlacementMode::Embed);
            let original = if embed {
                let bytes = blob.original.as_ref().ok_or("Embedded image original is missing")?;
                let r = next(&mut ids);
                pdf.stream(r, bytes);
                Some(r)
            } else {
                None
            };
            let proxy = next(&mut ids);
            let bytes = varos_core::images::codec::encode_png(&blob.proxy)?;
            pdf.stream(proxy, &bytes);
            asset_refs.push((meta.key.clone(), original, proxy));
        }
    }
    let model_id = if let Some(model) = model {
        let r = next(&mut ids);
        pdf.embedded_file(r, model.as_bytes()).subtype(Name(b"application/json"));
        Some(r)
    } else {
        None
    };
    let mut cat = pdf.catalog(catalog);
    cat.pages(tree);
    if let Some(model_id) = model_id {
        cat.pair(Name(b"VAROS_Model"), model_id)
            .pair(Name(b"VAROS_SchemaVersion"), varos_core::format::FORMAT_VERSION as i32);
        let mut assets = cat.insert(Name(b"VAROS_Assets")).dict();
        for (key, original, proxy) in asset_refs {
            let mut entry = assets.insert(Name(key.0.as_bytes())).dict();
            if let Some(r) = original {
                entry.pair(Name(b"Original"), r);
            }
            entry.pair(Name(b"Proxy"), proxy);
        }
    }
    cat.finish();
    let bytes = pdf.finish();
    if bytes.len() > Limits::DEFAULT.max_file_bytes as usize {
        return Err("Image PDF exceeds file limit".into());
    }
    Ok((bytes, report))
}
#[allow(clippy::too_many_arguments)]
fn paint(
    groups: &[Group],
    c: &mut Content,
    pool: &HashMap<(BlobKey, [u32; 2]), (Ref, String)>,
    pdf: &mut Pdf,
    ids: &mut i32,
    alphas: &mut Vec<(Ref, String)>,
    _doc: &Document,
    store: &BlobStore,
    ppi: f32,
) -> Result<(), String> {
    for group in groups {
        match group {
            Group::Clip { mask_rings, members } => {
                c.save_state();
                rings(c, mask_rings);
                c.clip_even_odd();
                c.end_path();
                paint(members, c, pool, pdf, ids, alphas, _doc, store, ppi)?;
                c.restore_state();
            }
            _ => {
                for prim in group.prims() {
                    match prim {
                        Prim::Image { key, corners, opacity, .. } => {
                            let p = &store.get(key).ok_or("Missing PDF image")?.pixels;
                            let v = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) * ppi / 72.;
                            let dims = [
                                p.width.min(v(corners[1], corners[0]).ceil().max(1.) as u32),
                                p.height.min(v(corners[3], corners[0]).ceil().max(1.) as u32),
                            ];
                            let (_, name) = pool.get(&(key.clone(), dims)).ok_or("Missing PDF image sampling")?;
                            c.save_state();
                            alpha(c, pdf, ids, alphas, *opacity);
                            let a = corners[0];
                            let b = corners[1];
                            let d = corners[3];
                            c.transform([b[0] - a[0], b[1] - a[1], a[0] - d[0], a[1] - d[1], d[0], d[1]]);
                            c.x_object(Name(name.as_bytes()));
                            c.restore_state();
                        }
                        Prim::Fill { rings: r, color } | Prim::StrokeCoverage { rings: r, color, .. } => {
                            c.save_state();
                            alpha(c, pdf, ids, alphas, color[3]);
                            c.set_fill_rgb(color[0], color[1], color[2]);
                            rings(c, r);
                            c.fill_even_odd();
                            c.restore_state();
                        }
                        Prim::Stroke { pts, width, color, .. } if pts.len() > 1 => {
                            c.save_state();
                            alpha(c, pdf, ids, alphas, color[3]);
                            c.set_stroke_rgb(color[0], color[1], color[2]);
                            c.set_line_width(*width);
                            c.move_to(pts[0][0], pts[0][1]);
                            for p in &pts[1..] {
                                c.line_to(p[0], p[1]);
                            }
                            c.stroke();
                            c.restore_state();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}
fn rings(c: &mut Content, rings: &[Vec<[f32; 2]>]) {
    for r in rings {
        if let Some(p) = r.first() {
            c.move_to(p[0], p[1]);
            for p in &r[1..] {
                c.line_to(p[0], p[1]);
            }
            c.close_path();
        }
    }
}
fn alpha(c: &mut Content, pdf: &mut Pdf, ids: &mut i32, alphas: &mut Vec<(Ref, String)>, a: f32) {
    if a < 0.999 {
        let id = next(ids);
        let name = format!("Ga{}", id.get());
        pdf.ext_graphics(id).non_stroking_alpha(a).stroking_alpha(a);
        c.set_parameters(Name(name.as_bytes()));
        alphas.push((id, name));
    }
}
/// Called only after bounded PDF preflight and strict metadata decode.
pub(crate) fn load_assets(
    pdf: &lopdf::Document,
    catalog: &lopdf::Dictionary,
    loaded: &mut Loaded,
    limits: &Limits,
) -> Result<(), LoadError> {
    let bad = |s: &str| LoadError::MalformedPdf(s.into());
    if loaded.doc.assets.is_empty() {
        if catalog.has(b"VAROS_Assets") {
            return Err(bad("Unexpected asset catalog"));
        }
        return Ok(());
    }
    let assets =
        catalog.get(b"VAROS_Assets").and_then(lopdf::Object::as_dict).map_err(|_| bad("Missing asset catalog"))?;
    if assets.len() != loaded.doc.assets.len() {
        return Err(bad("Asset catalog count mismatch"));
    }
    let mut total = encode_model(&loaded.doc, limits).map_err(|e| bad(&e.to_string()))?.len();
    let mut store = BlobStore::default();
    for meta in &loaded.doc.assets {
        let entry = assets
            .get(meta.key.0.as_bytes())
            .and_then(lopdf::Object::as_dict)
            .map_err(|_| bad("Missing asset entry"))?;
        if entry.iter().any(|(k, _)| k.as_slice() != b"Original" && k.as_slice() != b"Proxy") {
            return Err(bad("Unknown asset catalog key"));
        }
        let mut stream = |name: &[u8], max: usize| -> Result<Option<&[u8]>, LoadError> {
            let Ok(o) = entry.get(name) else { return Ok(None) };
            let (_, o) = pdf.dereference(o).map_err(|_| bad("Bad image stream reference"))?;
            let s = o.as_stream().map_err(|_| bad("Asset is not a stream"))?;
            if s.dict.has(b"Filter") || s.content.len() > max {
                return Err(bad("Unsupported or oversized asset stream"));
            }
            total = total.checked_add(s.content.len()).ok_or_else(|| bad("Asset stream size overflow"))?;
            if total > limits.max_decoded_stream_bytes {
                return Err(bad("Aggregate image stream limit"));
            }
            Ok(Some(&s.content))
        };
        let original = stream(b"Original", varos_core::images::MAX_ORIGINAL)?;
        let proxy = stream(b"Proxy", 512 * 1024)?.ok_or_else(|| bad("Missing image proxy"))?;
        let decoded_proxy = varos_core::images::codec::decode(proxy).map_err(|e| bad(&e))?;
        if (decoded_proxy.blob.pixels.width, decoded_proxy.blob.pixels.height) != (meta.proxy_w, meta.proxy_h) {
            return Err(bad("Image proxy dimensions mismatch"));
        }
        let blob = if let Some(original) = original {
            let decoded = varos_core::images::codec::decode(original).map_err(|e| bad(&e))?;
            if decoded.blob.meta != *meta {
                return Err(bad("Image original metadata/hash mismatch"));
            }
            Blob { proxy: decoded_proxy.blob.pixels, ..decoded.blob }
        } else {
            if loaded.doc.images.iter().any(|i| i.blob == meta.key && i.placement == PlacementMode::Embed) {
                return Err(bad("Embedded image original stream missing"));
            }
            Blob {
                meta: meta.clone(),
                original: None,
                pixels: decoded_proxy.blob.pixels.clone(),
                proxy: decoded_proxy.blob.pixels,
            }
        };
        store.insert(blob).map_err(|e| bad(&e))?;
    }
    loaded.blobs = store;
    Ok(())
}

/// Asset-aware export; legacy vector-only writer remains byte-stable.
pub fn export_with_options(
    doc: &Document,
    store: &BlobStore,
    plan: &crate::ExportPlan,
    options: &crate::PdfOptions,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, ExportReport), String> {
    if doc.images.is_empty() {
        return crate::export_pdf_with_options(doc, plan, options, cancel);
    }
    options.validate()?;
    let mut report = ExportReport::default();
    if options.marks != crate::PdfOptions::default().marks || options.boxes != crate::PdfOptions::default().boxes {
        return Err("Image PDF printer marks/boxes are not supported yet; use standard PDF options".into());
    }
    let (bytes, appearance) = export_pdf(doc, store, &plan.pages, options.image_ppi as f32, false, cancel)?;
    report.notes.extend(appearance.notes);
    Ok((bytes, report))
}
