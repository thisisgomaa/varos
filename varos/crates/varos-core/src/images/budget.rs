//! Process-wide decoded residency and serialized decoder scratch admission.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
pub const CPU_BYTES: usize = 256 * 1024 * 1024;
static USED: AtomicUsize = AtomicUsize::new(0);
pub(crate) static DECODE: Mutex<()> = Mutex::new(());
#[derive(Debug, PartialEq)]
pub struct CpuLease {
    bytes: usize,
}
impl Drop for CpuLease {
    fn drop(&mut self) {
        USED.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
pub fn reserve(bytes: usize) -> Result<Arc<CpuLease>, String> {
    USED.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| old.checked_add(bytes).filter(|n| *n <= CPU_BYTES))
        .map_err(|_| "Process image decode cache exceeds 256 MiB".to_owned())?;
    Ok(Arc::new(CpuLease { bytes }))
}
pub fn charged_bytes() -> usize {
    USED.load(Ordering::Acquire)
}
pub(crate) fn charge(w: u32, h: u32) -> Result<Arc<CpuLease>, String> {
    let original = super::dimensions(w, h)?;
    let factor = (256. / w.max(h) as f64).min(1.);
    let pw = (w as f64 * factor).round().max(1.) as u32;
    let ph = (h as f64 * factor).round().max(1.) as u32;
    reserve(original + super::dimensions(pw, ph)?)
}
