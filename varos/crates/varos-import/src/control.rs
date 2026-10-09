//! Cooperative conversion guard. Publication is always refused after cancellation/timeout.
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[derive(Clone)]
struct Budget {
    cancel: Arc<AtomicBool>,
    start: Instant,
}
thread_local! { static CURRENT:RefCell<Option<Budget>>=const {RefCell::new(None)}; }
pub(crate) fn checkpoint() -> Result<(), String> {
    CURRENT.with(|b| {
        if let Some(b) = b.borrow().as_ref() {
            if b.cancel.load(Ordering::Acquire) {
                return Err("Import cancelled".into());
            }
            if b.start.elapsed() > Duration::from_secs(10) {
                return Err("Import conversion exceeded ten-second budget".into());
            }
        }
        Ok(())
    })
}
struct Restore(Option<Budget>);
impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|b| *b.borrow_mut() = self.0.take());
    }
}
pub fn import_cancellable(
    bytes: &[u8],
    format: crate::Format,
    options: crate::ImportOptions,
    cancel: Arc<AtomicBool>,
) -> Result<(varos_core::model::Document, crate::ImportReport), String> {
    let old = CURRENT.with(|b| b.replace(Some(Budget { cancel, start: Instant::now() })));
    let _restore = Restore(old);
    checkpoint()?;
    let out = crate::interchange::convert(bytes, format, options)?;
    checkpoint()?;
    Ok(out)
}
