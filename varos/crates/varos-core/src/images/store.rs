//! Immutable session resources, deliberately unreachable from the Serde document graph.
use super::{AssetMeta, BlobKey};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
pub const MAX_AXIS: u32 = 16_384;
pub const MAX_PIXELS: u64 = 32_000_000;
pub const MAX_ORIGINAL: usize = 16 * 1024 * 1024;
pub const MAX_DOCUMENT_ORIGINALS: usize = 32 * 1024 * 1024;
pub const MAX_SESSION: usize = 256 * 1024 * 1024;
pub const MAX_PROXIES: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq)]
pub struct Pixels {
    pub budget: Option<Arc<super::budget::CpuLease>>,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Blob {
    pub meta: AssetMeta,
    pub original: Option<Arc<[u8]>>,
    pub pixels: Arc<Pixels>,
    pub proxy: Arc<Pixels>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlobStore {
    pub load_notes: Vec<String>,
    pub document_dir: Option<std::path::PathBuf>,
    entries: HashMap<BlobKey, Arc<Blob>>,
}
pub fn content_key(bytes: &[u8]) -> BlobKey {
    BlobKey(Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect())
}
pub fn dimensions(w: u32, h: u32) -> Result<usize, String> {
    if w == 0 || h == 0 || w > MAX_AXIS || h > MAX_AXIS || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err("Image exceeds 16,384 pixels per axis or 32 million pixels".into());
    }
    usize::try_from(u64::from(w) * u64::from(h) * 4).map_err(|_| "Image allocation overflow".into())
}
impl BlobStore {
    pub fn get(&self, key: &BlobKey) -> Option<&Arc<Blob>> {
        self.entries.get(key)
    }
    pub fn retained_bytes(&self) -> usize {
        self.entries
            .values()
            .map(|b| b.original.as_ref().map_or(0, |v| v.len()) + b.pixels.rgba.len() + b.proxy.rgba.len())
            .sum()
    }
    pub fn original_bytes(&self) -> usize {
        self.entries.values().map(|b| b.original.as_ref().map_or(0, |v| v.len())).sum()
    }
    pub fn insert(&mut self, blob: Blob) -> Result<BlobKey, String> {
        let key = blob.meta.key.clone();
        if let Some(old) = self.entries.get(&key) {
            if old.original.is_none() && blob.original.is_some() && old.meta == blob.meta {
                let mut candidate = self.clone();
                candidate.entries.remove(&key);
                let key = candidate.insert(blob)?;
                *self = candidate;
                return Ok(key);
            }
            if old.original != blob.original || old.meta != blob.meta {
                return Err("Image identity collision".into());
            }
            return Ok(key);
        }
        if !key.valid() || blob.meta.encoded_len == 0 || blob.meta.encoded_len > MAX_ORIGINAL {
            return Err("Invalid image identity".into());
        }
        if blob
            .original
            .as_ref()
            .is_some_and(|b| b.len() > MAX_ORIGINAL || b.len() != blob.meta.encoded_len || content_key(b) != key)
        {
            return Err("Invalid original image hash/size".into());
        }
        if dimensions(blob.pixels.width, blob.pixels.height)? != blob.pixels.rgba.len()
            || blob.original.is_some()
                && (blob.pixels.width != blob.meta.px_w || blob.pixels.height != blob.meta.px_h)
                && !Arc::ptr_eq(&blob.pixels, &blob.proxy)
        {
            return Err("Image dimensions disagree".into());
        }
        if blob.proxy.width != blob.meta.proxy_w
            || blob.proxy.height != blob.meta.proxy_h
            || blob.proxy.width > 256
            || blob.proxy.height > 256
            || dimensions(blob.proxy.width, blob.proxy.height)? != blob.proxy.rgba.len()
        {
            return Err("Invalid image proxy".into());
        }
        let added = blob.original.as_ref().map_or(0, |v| v.len()) + blob.pixels.rgba.len() + blob.proxy.rgba.len();
        if self.retained_bytes().checked_add(added).is_none_or(|n| n > MAX_SESSION) {
            return Err("Image session memory budget reached; undo was retained".into());
        }
        self.entries.insert(key.clone(), Arc::new(blob));
        Ok(key)
    }
    pub fn retire(&mut self, retired: &HashSet<BlobKey>, pins: &HashSet<BlobKey>) {
        self.entries.retain(|key, _| !retired.contains(key) || pins.contains(key));
    }
    pub fn collect(&mut self, pins: &HashSet<BlobKey>) {
        self.entries.retain(|k, _| pins.contains(k));
    }
}
