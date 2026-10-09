//! Resource-aware native container and appearance export. Binary originals never enter model JSON.
use crate::export::PageSpec;
use std::sync::atomic::AtomicBool;
use varos_core::{
    format::{encode_model, Limits, LoadError, Loaded},
    images::{Blob, BlobStore, PlacementMode},
    model::Document,
    ExportReport,
};
pub fn write_vrs(doc: &Document, store: &BlobStore, limits: &Limits) -> Result<Vec<u8>, String> {
    let model = encode_model(doc, limits).map_err(|e| e.to_string())?;
    // Validate immutable pinned resources; never re-decode against the process residency lease.
    for meta in &doc.assets {
        let blob = store.get(&meta.key).ok_or("Missing manifest resource")?;
        if blob.meta != *meta {
            return Err("Image metadata mismatch".into());
        }
        if let Some(bytes) = &blob.original {
            varos_core::images::codec::check_original(bytes, meta)?;
        }
    }
    let (bytes, _) = crate::write::write_resource_pages(
        doc,
        store,
        &crate::write::native_pages(doc),
        Some(&model),
        300.,
        false,
        &AtomicBool::new(false),
    )?;
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err("Image container exceeds file limit".into());
    }
    crate::read::parse_pdf(&bytes, limits).map_err(|e| e.to_string())?;
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
    crate::write::write_resource_pages(doc, store, pages, None, ppi, preview, cancel)
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
        let decoded_proxy = varos_core::images::codec::decode(proxy).map_err(|e| {
            if e == "Process image decode cache exceeds 256 MiB" {
                LoadError::TooLarge {
                    limit: varos_core::format::LimitKind::DecodedStreams,
                    found: (varos_core::images::budget::charged_bytes()
                        + meta.proxy_w as usize * meta.proxy_h as usize * 8) as u64,
                    max: varos_core::images::budget::CPU_BYTES as u64,
                }
            } else {
                bad(&e)
            }
        })?;
        if (decoded_proxy.blob.pixels.width, decoded_proxy.blob.pixels.height) != (meta.proxy_w, meta.proxy_h) {
            return Err(bad("Image proxy dimensions mismatch"));
        }
        let blob = if let Some(original) = original {
            varos_core::images::codec::check_original(original, meta).map_err(|e| bad(&e))?;
            match varos_core::images::codec::decode(original) {
                Ok(decoded) => {
                    if decoded.blob.meta != *meta {
                        return Err(bad("Image original metadata/hash mismatch"));
                    }
                    Blob { proxy: decoded_proxy.blob.pixels, ..decoded.blob }
                }
                Err(e) if e == "Process image decode cache exceeds 256 MiB" => {
                    store.load_notes.push(format!("{}: decoded budget unavailable; proxy preview", meta.key.0));
                    Blob {
                        meta: meta.clone(),
                        original: Some(std::sync::Arc::from(original)),
                        pixels: decoded_proxy.blob.pixels.clone(),
                        proxy: decoded_proxy.blob.pixels,
                    }
                }
                Err(e) => return Err(bad(&e)),
            }
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
