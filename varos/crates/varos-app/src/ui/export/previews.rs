//! Sheet-owned bounded decoder. Completion events drive decoding; no per-card threads or filesystem polling.
use crate::thumbs::{Lookup, ThumbKey, ThumbService};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, SystemTime};
use varos_raster::export::Asset;

const LIMIT: usize = 16;
type Pixels = HashMap<String, Option<Option<egui::ColorImage>>>;
struct Request {
    key: String,
    asset: Asset,
    blobs: Arc<varos_core::images::BlobStore>,
}
struct Runtime {
    tx: mpsc::SyncSender<Request>,
    cancelled: Arc<AtomicBool>,
    pixels: Arc<Mutex<Pixels>>,
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
#[derive(Clone, Default)]
pub struct Previews(Arc<Mutex<Option<Runtime>>>);
impl std::fmt::Debug for Previews {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExportPreviews")
    }
}
impl PartialEq for Previews {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Previews {
    pub fn pixels(
        &self,
        ctx: &egui::Context,
        key: String,
        asset: &Asset,
        blobs: &varos_core::images::BlobStore,
    ) -> Option<egui::ColorImage> {
        let mut slot = self.0.lock().ok()?;
        if slot.is_none() {
            let root = varos_app::storage::paths::AppLayout::current()?.thumbs().join("Export");
            *slot = start(root, ctx.clone());
        }
        let runtime = slot.as_ref()?;
        let mut pixels = runtime.pixels.lock().ok()?;
        if let Some(value) = pixels.get(&key) {
            return value.as_ref().and_then(Clone::clone);
        }
        if pixels.values().filter(|p| p.is_none()).count() >= LIMIT {
            return None;
        }
        pixels.insert(key.clone(), None);
        if runtime
            .tx
            .try_send(Request { key: key.clone(), asset: asset.clone(), blobs: Arc::new(blobs.clone()) })
            .is_err()
        {
            pixels.remove(&key);
        }
        None
    }
}
fn start(root: PathBuf, ctx: egui::Context) -> Option<Runtime> {
    let (tx, rx) = mpsc::sync_channel::<Request>(LIMIT);
    let cancelled = Arc::new(AtomicBool::new(false));
    let pixels = Arc::new(Mutex::new(HashMap::new()));
    let stop = cancelled.clone();
    let output = pixels.clone();
    std::thread::Builder::new()
        .name("varos-export-decoder".into())
        .spawn(move || {
            let Some(service) = ThumbService::at(root) else { return };
            let mut pending = HashSet::new();
            loop {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                // Idle blocks indefinitely. Only outstanding renders require a timed completion check.
                let request = if pending.is_empty() {
                    rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                } else {
                    rx.recv_timeout(Duration::from_millis(20))
                };
                match request {
                    Ok(request) => {
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                        let key = ThumbKey(request.key);
                        if let Some(Lookup::Fresh(path)) = service.lookup(&key, SystemTime::UNIX_EPOCH) {
                            complete(&output, &ctx, &stop, key.0, Some(path));
                        } else if service.request_export(
                            key.clone(),
                            request.asset,
                            request.blobs,
                            SystemTime::UNIX_EPOCH,
                        ) {
                            pending.insert(key);
                        } else {
                            complete(&output, &ctx, &stop, key.0, None);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                for done in service.try_completions() {
                    pending.remove(&done.key);
                    complete(&output, &ctx, &stop, done.key.0, done.path);
                }
            }
            for key in pending {
                service.forget(&key);
            }
            // Dropping the service discards queued renders; at most the active render can finish.
        })
        .ok()?;
    Some(Runtime { tx, cancelled, pixels })
}
fn complete(output: &Mutex<Pixels>, ctx: &egui::Context, stop: &AtomicBool, key: String, path: Option<PathBuf>) {
    if stop.load(Ordering::Acquire) {
        return;
    }
    let color = path
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|image| {
            let rgba = image.to_rgba8();
            egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
        });
    if stop.load(Ordering::Acquire) {
        return;
    }
    if let Ok(mut pixels) = output.lock() {
        pixels.insert(key, Some(color));
    }
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_cache_finishes_once_and_drop_cancels_pending_decoder() {
        let root = std::env::temp_dir().join(format!("export-decoder-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::write(&root, b"blocked directory").unwrap();
        let runtime = start(root.join("cache"), egui::Context::default()).unwrap();
        let doc = varos_pdf::load_vrs(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-cli/tests/fixtures/v3_nested_group.vrs"),
        )
        .unwrap();
        let asset = varos_raster::export::plan(&doc, &varos_raster::export::Scope::WholeBoard).unwrap().remove(0);
        runtime.pixels.lock().unwrap().insert("failed".into(), None);
        runtime.tx.send(Request { key: "failed".into(), asset, blobs: Default::default() }).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while runtime.pixels.lock().unwrap().get("failed") != Some(&Some(None)) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let stopped = runtime.cancelled.clone();
        let tx = runtime.tx.clone();
        drop(runtime);
        assert!(stopped.load(Ordering::Acquire));
        drop(tx);
        std::fs::remove_file(root).unwrap();
    }
    #[test]
    fn admission_is_bounded_and_sheet_drop_signals_shutdown() {
        let (tx, _rx) = mpsc::sync_channel(LIMIT);
        let cancelled = Arc::new(AtomicBool::new(false));
        let pixels = Arc::new(Mutex::new(HashMap::new()));
        let previews =
            Previews(Arc::new(Mutex::new(Some(Runtime { tx, cancelled: cancelled.clone(), pixels: pixels.clone() }))));
        let doc = varos_pdf::load_vrs(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-cli/tests/fixtures/v3_nested_group.vrs"),
        )
        .unwrap();
        let asset = varos_raster::export::plan(&doc, &varos_raster::export::Scope::WholeBoard).unwrap().remove(0);
        for index in 0..100 {
            previews.pixels(&egui::Context::default(), index.to_string(), &asset, &Default::default());
        }
        assert_eq!(pixels.lock().unwrap().len(), LIMIT);
        drop(previews);
        assert!(cancelled.load(Ordering::Acquire));
    }
}
