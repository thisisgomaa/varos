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
    let cached_png = std::fs::read(&cached)
        .ok()
        .filter(|bytes| bytes.len() <= 2 * 1024 * 1024 && image::load_from_memory(bytes).is_ok());
    if cached_png.is_none()
        || std::fs::read_to_string(cached.with_extension("model-sha256")).ok().as_deref() != Some(&digest)
    {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let png = varos_raster::rasterize(Arc::new(doc.clone()), [varos_raster::WIDTH, varos_raster::HEIGHT])
            .into_result()?
            .encode_png()?;
        persist(&cached, &png)?;
        persist(&cached.with_extension("model-sha256"), digest.as_bytes())?;
    }
    let bytes = std::fs::read(cached).map_err(|e| e.to_string())?;
    let next = varos_pdf::quicklook::embed_preview_next(&pdf, &bytes)?;
    varos_pdf::load_vrs_bytes(&next, &varos_core::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    Ok(next)
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
