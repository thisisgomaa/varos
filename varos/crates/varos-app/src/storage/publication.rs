//! Edit admission and the final rename share a short lock. UI invalidates BEFORE waiting;
//! the worker never waits for the UI. Encoding and fsync never hold this lock.
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
#[derive(Debug, Default)]
pub struct Gate {
    pub lock: Mutex<()>,
    epoch: AtomicU64,
}
impl Gate {
    pub fn invalidate(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }
    pub fn capture(self: &Arc<Self>) -> Permit {
        Permit { gate: self.clone(), epoch: self.epoch.load(Ordering::SeqCst) }
    }
}
#[derive(Clone, Debug)]
pub struct Permit {
    pub gate: Arc<Gate>,
    epoch: u64,
}
impl Permit {
    pub fn valid(&self) -> bool {
        self.epoch == self.gate.epoch.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_invalidates_before_publication_and_admission_is_exclusive() {
        let gate = Arc::new(Gate::default());
        let permit = gate.capture();
        assert!(permit.valid());
        let input = gate.lock.lock().unwrap();
        gate.invalidate();
        assert!(!permit.valid());
        assert!(gate.lock.try_lock().is_err());
        drop(input);
        let next = gate.capture();
        assert!(next.valid());
        assert!(gate.lock.try_lock().is_ok());
    }
}
