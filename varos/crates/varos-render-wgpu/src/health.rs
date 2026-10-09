//! Terminal device health: runtime GPU failures remain readable (ADR-0001).
// Adapted from PhotoCraft crates/gpu/src/health.rs@4cb7cf3 (MIT OR Apache-2.0), ArtCraft Team 2026.
use std::sync::{Arc, Mutex, PoisonError};
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum GpuHealth {
    #[default]
    Running,
    Stopped {
        reason: String,
    },
}
#[derive(Clone, Default)]
pub struct DeviceHealth(Arc<Inner>);
#[derive(Default)]
struct Inner {
    state: Mutex<GpuHealth>,
    notifier: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}
impl DeviceHealth {
    pub fn state(&self) -> GpuHealth {
        self.0.state.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
    /// Wake the host once when a runtime fault arrives, including while it is idle.
    pub fn set_notifier(&self, notify: impl Fn() + Send + Sync + 'static) {
        *self.0.notifier.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(notify));
        if !self.is_running() {
            self.notify();
        }
    }
    fn notify(&self) {
        let callback = self.0.notifier.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(callback) = callback {
            callback();
        }
    }
    pub fn stop(&self, reason: String) {
        let first = {
            let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
            if matches!(*state, GpuHealth::Running) {
                *state = GpuHealth::Stopped { reason };
                true
            } else {
                false
            }
        };
        if first {
            self.notify();
        }
    }
    pub fn is_running(&self) -> bool {
        matches!(self.state(), GpuHealth::Running)
    }
    pub fn watch(device: &wgpu::Device) -> Self {
        let health = Self::default();
        let lost = health.clone();
        device.set_device_lost_callback(move |reason, message| lost.stop(format!("{reason:?}: {message}")));
        let errors = health.clone();
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| errors.stop(error.to_string())));
        health
    }
    pub fn poll(&self, device: &wgpu::Device) -> bool {
        self.poll_action(|| device.poll(wgpu::PollType::Poll).map(|_| ()).map_err(|e| e.to_string()))
    }
    fn poll_action(&self, poll: impl FnOnce() -> Result<(), String>) -> bool {
        if !self.is_running() {
            return false;
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(poll)) {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => self.stop(error),
            Err(_) => self.stop("the device stopped responding".into()),
        }
        self.is_running()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_fault_wakes_an_idle_host_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let health = DeviceHealth::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        health.set_notifier(move || {
            count.fetch_add(1, Ordering::Relaxed);
        });
        health.stop("device lost".into());
        health.clone().stop("later".into());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        health.set_notifier(move || {
            count.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1, "fault before host registration is delivered");
    }
    #[test]
    fn poll_failure_stops_without_retrying() {
        let health = DeviceHealth::default();
        assert!(health.poll_action(|| Ok(())));
        assert!(!health.poll_action(|| Err("lost device".into())));
        let mut calls = 0;
        assert!(!health.poll_action(|| {
            calls += 1;
            Ok(())
        }));
        assert_eq!(calls, 0);
        assert_eq!(health.state(), GpuHealth::Stopped { reason: "lost device".into() });
    }
    #[test]
    fn poll_panic_becomes_a_readable_terminal_reason() {
        let health = DeviceHealth::default();
        assert!(!health.poll_action(|| panic!("driver stopped")));
        assert_eq!(health.state(), GpuHealth::Stopped { reason: "the device stopped responding".into() });
    }
    #[test]
    fn stopped_is_terminal_and_shared() {
        let h = DeviceHealth::default();
        assert!(h.is_running());
        let peer = h.clone();
        peer.stop("driver reset".into());
        h.stop("later error".into());
        assert_eq!(h.state(), GpuHealth::Stopped { reason: "driver reset".into() });
        assert!(!peer.is_running());
    }
}
