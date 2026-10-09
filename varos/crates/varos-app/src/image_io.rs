//! Durable native save of the captured document/resource pair.
use crate::lifecycle::SaveOutcome;
use std::path::Path;
use varos_app::storage::{
    checksum::new_nonce,
    durable::{io_reason, write_replace_published, Fingerprint, FsPort, WriteOutcome},
};
use varos_core::{format::Limits, images::BlobStore, model::Document};
pub fn save(
    fs: &dyn FsPort,
    doc: &Document,
    store: &BlobStore,
    path: &Path,
) -> Result<(SaveOutcome, Option<Fingerprint>), String> {
    let bytes = varos_pdf::images::write_vrs(doc, store, &Limits::DEFAULT)?;
    // Lane F × w2-images: the optional Quick Look preview for image documents too
    let bytes = crate::quicklook::with_cached_preview_resources(doc, store, path, bytes)?;
    let mut published = None;
    let outcome = write_replace_published(fs, path, &bytes, &new_nonce(), &mut published).map_err(|e| e.reason())?;
    Ok((
        match outcome {
            WriteOutcome::Durable => SaveOutcome::Durable,
            WriteOutcome::ReplacedUnconfirmed(e) => SaveOutcome::ReplacedUnconfirmed(io_reason(&e)),
        },
        published,
    ))
}
