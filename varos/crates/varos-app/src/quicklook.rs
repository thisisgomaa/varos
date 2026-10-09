//! Lane F: prepare a bounded CPU thumbnail on the file worker, cache it, then embed it.
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
static LIVE_CACHE: OnceLock<PathBuf> = OnceLock::new();
/// Configured by desktop startup only; headless tests never touch owner app-data.
pub fn enable_cache(root: PathBuf) {
    let _ = LIVE_CACHE.set(root);
}
pub fn model_digest(doc: &varos_core::model::Document) -> Result<String, String> {
    let bytes =
        varos_core::format::encode_model(doc, &varos_core::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect())
}
pub fn with_cached_preview(doc: &varos_core::model::Document, path: &Path, pdf: Vec<u8>) -> Result<Vec<u8>, String> {
    match LIVE_CACHE.get() {
        Some(root) => with_preview_at(doc, path, pdf, root),
        None => Ok(pdf),
    }
}
fn with_preview_at(
    doc: &varos_core::model::Document,
    path: &Path,
    pdf: Vec<u8>,
    root: &Path,
) -> Result<Vec<u8>, String> {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let key = crate::thumbs::ThumbKey(canonical.to_string_lossy().into_owned());
    let cached = crate::thumbs::preview_cache_path(root, &key);
    let digest = model_digest(doc)?;
    let cached_png = read_cached_png(&cached).ok().filter(|_| {
        read_bounded(&cached.with_extension("model-sha256"), 64).ok().as_deref() == Some(digest.as_bytes())
    });
    let bytes = match cached_png {
        Some(bytes) => bytes,
        None => {
            let png = varos_raster::rasterize(Arc::new(doc.clone()), [varos_raster::WIDTH, varos_raster::HEIGHT])
                .into_result()?
                .encode_png()?;
            // Optional disk cache cannot make a writable native destination unsaveable.
            if std::fs::create_dir_all(root).is_ok() {
                let _ = persist(&cached, &png);
                let _ = persist(&cached.with_extension("model-sha256"), digest.as_bytes());
            }
            png
        }
    };
    let next = varos_pdf::quicklook::embed_preview_next(&pdf, &bytes)?;
    varos_pdf::load_vrs_bytes(&next, &varos_core::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    Ok(next)
}
const MAX_PNG: usize = 2 * 1024 * 1024;
fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("Preview cache exceeds byte limit".into());
    }
    Ok(bytes)
}
fn read_cached_png(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = read_bounded(path, MAX_PNG)?;
    validate_png(&bytes)?;
    Ok(bytes)
}
fn validate_png(bytes: &[u8]) -> Result<(), String> {
    // PNG-only, strict thumbnail dimensions and a decoder allocation budget before decode.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(varos_raster::WIDTH);
    limits.max_image_height = Some(varos_raster::HEIGHT);
    limits.max_alloc = Some(8 * 1024 * 1024);
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())?;
    Ok(())
}
fn persist(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use varos_app::storage::{
        checksum::new_nonce,
        durable::{self, RealFs, WriteOutcome},
    };
    match durable::write_replace(&RealFs, path, bytes, &new_nonce()).map_err(|e| e.reason())? {
        WriteOutcome::Durable => Ok(()),
        WriteOutcome::ReplacedUnconfirmed(e) => Err(format!("Preview cache durability unconfirmed: {e}")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_preview_is_cached_and_embedded_without_model_changes() {
        let dir = std::env::temp_dir().join(format!("varos-quicklook-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::create_dir_all(&dir).unwrap();
        let doc = varos_core::board::new_board();
        let pdf = varos_pdf::write_pdf_checked(&doc, &varos_core::format::Limits::DEFAULT).unwrap();
        let next = with_preview_at(&doc, &dir.join("board.vrs"), pdf, &dir).unwrap();
        assert!(varos_pdf::quicklook::preview(&next).unwrap().unwrap().starts_with(b"\x89PNG"));
        let loaded = varos_pdf::load_vrs_bytes(&next, &varos_core::format::Limits::DEFAULT).unwrap().doc;
        assert_eq!(model_digest(&loaded).unwrap(), model_digest(&doc).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod fix_round_tests {
    use super::*;
    #[test]
    fn oversized_cache_and_thumbnail_dimensions_are_refused_before_decode() {
        let path =
            std::env::temp_dir().join(format!("varos-preview-bounds-{}", varos_app::storage::checksum::new_nonce()));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(128 * 1024 * 1024).unwrap();
        assert!(read_cached_png(&path).unwrap_err().contains("byte limit"));
        std::fs::remove_file(path).unwrap();
        let image = image::RgbaImage::new(varos_raster::WIDTH + 1, 1);
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        assert!(validate_png(bytes.get_ref()).is_err());
        assert!(validate_png(b"broken").is_err());
    }
    #[test]
    fn unusable_cache_directory_does_not_prevent_native_save() {
        let root = std::env::temp_dir()
            .join(format!("varos-preview-unwritable-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::write(&root, b"a file cannot be a cache directory").unwrap();
        let doc = varos_core::board::new_board();
        let original = varos_pdf::write_pdf_checked(&doc, &varos_core::format::Limits::DEFAULT).unwrap();
        let saved = with_preview_at(&doc, &root.with_extension("vrs"), original, &root).unwrap();
        assert!(varos_pdf::quicklook::preview(&saved).unwrap().is_some());
        assert!(varos_pdf::load_vrs_bytes(&saved, &varos_core::format::Limits::DEFAULT).unwrap().doc.content_eq(&doc));
        std::fs::remove_file(root).unwrap();
    }
}
