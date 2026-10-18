//! Opt-in, UI-thread section timings. `VAROS_PROFILE=1` prints the ten sections with the
//! greatest total time once per second (inclusive times: nested sections must not be summed).
//! Disabled: one cached flag check, no clock read, allocation or egui state access. No extra
//! repaints are requested, so an idle window stays idle. Native only; WASM is a no-op.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[derive(Clone, Default)]
pub(super) struct Cost {
    pub calls: u64,
    pub total_ms: f64,
    pub max_ms: f64,
}

#[derive(Default)]
struct Collector {
    since: Option<Instant>,
    costs: BTreeMap<String, Cost>,
    #[cfg(test)]
    capture: bool,
}

thread_local! {
    static COLLECTOR: RefCell<Collector> = RefCell::new(Collector::default());
}

fn enabled() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var_os("VAROS_PROFILE").is_some_and(|v| v == "1"))
    }
    #[cfg(target_arch = "wasm32")]
    false
}

#[must_use]
pub(crate) struct Scope<'a>(Option<(&'a str, Instant)>);

pub(crate) fn scope(name: &str) -> Scope<'_> {
    Scope(enabled().then(|| (name, Instant::now())))
}

pub(crate) fn measure<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let _scope = scope(name);
    f()
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        let Some((name, start)) = self.0 else { return };
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        COLLECTOR.with_borrow_mut(|collector| {
            collector.since.get_or_insert(start);
            // Only allocate a name on the first call in this reporting interval.
            let cost = match collector.costs.get_mut(name) {
                Some(cost) => cost,
                None => collector.costs.entry(name.to_owned()).or_default(),
            };
            cost.calls = cost.calls.saturating_add(1);
            cost.total_ms += ms;
            cost.max_ms = cost.max_ms.max(ms);
        });
    }
}

/// Call after the UI scope has closed. Logging occurs outside section timers.
pub(crate) fn finish_frame() {
    if !enabled() {
        return;
    }
    COLLECTOR.with_borrow_mut(|collector| {
        #[cfg(test)]
        if collector.capture {
            return;
        }
        let Some(start) = collector.since else { return };
        if start.elapsed().as_secs_f64() < 1.0 {
            return;
        }
        let mut costs: Vec<_> = std::mem::take(&mut collector.costs).into_iter().collect();
        costs.sort_by(|a, b| b.1.total_ms.total_cmp(&a.1.total_ms));
        let mut stderr = std::io::stderr().lock();
        // Diagnostics must not interrupt the UI if stderr is closed or unavailable.
        let _ = writeln!(stderr, "VAROS_PROFILE {:.2}s, inclusive CPU sections (not GPU/present):", start.elapsed().as_secs_f64());
        for (name, cost) in costs.into_iter().take(10) {
            // Losing a diagnostic line is harmless; never fail an app frame over logging.
            let _ = writeln!(
                stderr,
                "  {name}: {:.3} ms/call, {:.3} ms max, {:.3} ms total / {} calls",
                cost.total_ms / cost.calls.max(1) as f64,
                cost.max_ms,
                cost.total_ms,
                cost.calls
            );
        }
        collector.since = None;
    });
}

#[cfg(test)]
pub(super) fn capture() {
    COLLECTOR.with_borrow_mut(|c| *c = Collector { capture: true, ..Default::default() });
}

#[cfg(test)]
pub(super) fn captured() -> BTreeMap<String, Cost> {
    COLLECTOR.with_borrow(|c| c.costs.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_sections_accumulate_inclusive_costs_and_counts() {
        capture();
        {
            let _outer = Scope(Some(("outer", Instant::now())));
            for _ in 0..3 {
                let _inner = Scope(Some(("inner", Instant::now())));
            }
        }
        let costs = captured();
        assert_eq!(costs.get("outer").map(|c| c.calls), Some(1));
        assert_eq!(costs.get("inner").map(|c| c.calls), Some(3));
        assert!(costs.get("outer").is_some_and(|c| c.total_ms >= costs.get("inner").map_or(0.0, |c| c.total_ms)));
    }

    #[test]
    fn absent_scope_leaves_collector_empty() {
        capture();
        drop(Scope(None));
        finish_frame();
        assert!(captured().is_empty());
    }

    #[test]
    #[ignore = "microbenchmark; run with --nocapture"]
    fn timer_overhead() {
        let start = Instant::now();
        for _ in 0..100_000 {
            measure("overhead", || std::hint::black_box(1));
        }
        eprintln!("100000 section timers: {:.3}ms; VAROS_PROFILE={:?}", start.elapsed().as_secs_f64() * 1000.0, std::env::var_os("VAROS_PROFILE"));
    }
}
