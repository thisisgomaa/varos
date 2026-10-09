//! File autosave policy: monotonic fake-clock-friendly deadlines, independent of recovery.
use super::settings::Settings;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct State {
    revision: Option<u64>,
    pub deadline: Option<Instant>,
    pub paused: bool,
    pub conflict: bool,
    pub confirmation: bool,
    pub status: String,
    pub ticket: Option<u64>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Probe {
    pub revision: u64,
    pub dirty: bool,
    pub path: bool,
    pub transaction: bool,
    pub gesture: bool,
    pub picker: bool,
    pub field: bool,
    pub bridge: bool,
    pub file_job: bool,
    pub recovery_job: bool,
    pub transition: bool,
    pub read_only: bool,
}
impl Probe {
    pub fn blocked(self) -> bool {
        self.transaction
            || self.gesture
            || self.picker
            || self.field
            || self.bridge
            || self.file_job
            || self.recovery_job
            || self.transition
            || self.read_only
            || !self.path
    }
}
impl State {
    pub fn reset(&mut self) {
        self.revision = None;
        self.deadline = None;
    }
    pub fn observe(&mut self, now: Instant, settings: Settings, p: Probe) -> Option<Instant> {
        if !settings.autosave_enabled {
            self.reset();
            return None;
        }
        if !p.transaction && self.revision != Some(p.revision) {
            self.revision = Some(p.revision);
            self.deadline = p.dirty.then_some(now + Duration::from_secs(settings.autosave_interval_seconds));
        }
        if !p.dirty {
            self.deadline = None;
            return None;
        }
        if self.paused || self.ticket.is_some() {
            return None;
        }
        if p.blocked() {
            if self.deadline.is_some_and(|at| at <= now) && (p.transaction || p.gesture || p.picker || p.field) {
                self.status = "Autosave waiting for edit to finish".into();
            }
            return None;
        }
        self.deadline
    }
    pub fn backoff(&mut self, now: Instant) {
        let retry = now + Duration::from_secs(30);
        self.deadline = Some(self.deadline.map_or(retry, |at| at.max(retry)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn probe() -> Probe {
        Probe { dirty: true, path: true, ..Default::default() }
    }
    #[test]
    fn idle_content_only_undo_clean_and_preferences() {
        let t = Instant::now();
        let s = Settings::default();
        let mut a = State::default();
        let mut p = probe();
        assert_eq!(a.observe(t, s, p), Some(t + Duration::from_secs(120)));
        assert_eq!(a.observe(t + Duration::from_secs(100), s, p), a.deadline);
        p.revision += 1;
        assert_eq!(a.observe(t + Duration::from_secs(100), s, p), Some(t + Duration::from_secs(220)));
        p.transaction = true;
        p.revision += 1;
        assert_eq!(a.observe(t + Duration::from_secs(150), s, p), None);
        p.transaction = false;
        assert_eq!(a.observe(t + Duration::from_secs(200), s, p), Some(t + Duration::from_secs(320)));
        let off = Settings { autosave_enabled: false, ..s };
        assert_eq!(a.observe(t, off, p), None);
        assert_eq!(a.observe(t, s, p), Some(t + Duration::from_secs(120)));
        p.dirty = false;
        assert_eq!(a.observe(t, s, p), None);
    }
    #[test]
    fn every_blocker_suspends_even_expired_deadline_without_polling() {
        for i in 0..11 {
            let t = Instant::now();
            let s = Settings::default();
            let mut a = State::default();
            let mut p = probe();
            a.observe(t, s, p);
            match i {
                0 => p.transaction = true,
                1 => p.gesture = true,
                2 => p.picker = true,
                3 => p.field = true,
                4 => p.bridge = true,
                5 => p.file_job = true,
                6 => p.recovery_job = true,
                7 => p.transition = true,
                8 => p.read_only = true,
                9 => p.path = false,
                _ => a.paused = true,
            }
            assert_eq!(a.observe(t + Duration::from_secs(300), s, p), None, "blocker {i}");
        }
    }
    #[test]
    fn continuous_edits_and_retry_never_shorten_the_idle_interval() {
        let t = Instant::now();
        let settings = Settings::default();
        let mut state = State::default();
        let mut p = probe();
        for i in 0..100 {
            p.revision = i;
            let now = t + Duration::from_secs(i * 20);
            assert_eq!(state.observe(now, settings, p), Some(now + Duration::from_secs(120)));
        }
        let deadline = state.deadline;
        state.backoff(t + Duration::from_secs(2000));
        assert_eq!(state.deadline, deadline);
    }
}
