//! Idle discipline for the event loop (owner report 2026-10-07: the Mac ran warm while Varos sat
//! idle). A work tool at rest draws nothing and sleeps in `ControlFlow::Wait`.
//!
//! Two different needs wake the loop, and they are kept apart here:
//!  - a **frame** (`window.request_redraw()`): only when something visible changed — input, a
//!    command that ran, a status line that changed, or egui asking for one (a tooltip delay, the
//!    caret blink while a field has focus);
//!  - a **turn** (an `AboutToWait` pass, no frame): background deadlines — the 30-second recovery
//!    copy, the launch-scan poll, the "Saving…" status delay, the Keep Waiting question, a finished
//!    file job waiting to be applied. A turn that changes something visible asks for its own frame
//!    through the normal change checks; a timer by itself never draws.
//!
//! [`plan`] is the one decision (pure, unit-tested); [`FrameStats`] is the opt-in instrumentation:
//! `VAROS_FRAME_DEBUG=1` prints one stderr line per elapsed second of activity — frames, passes,
//! presents, the `ControlFlow` chosen, every redraw request by reason, egui's repaint causes and
//! what woke the loop. A loop at rest prints nothing (it is asleep); the next line covers the whole
//! quiet interval.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// How the loop waits after a pass (mirrors winit's `ControlFlow` without depending on it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Sleep until an OS event or a proxy wake.
    Wait,
    /// Sleep until this instant (a past instant = one more pass at once, still no frame).
    WaitUntil(Instant),
}

/// The decision after one `AboutToWait` pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Ask for a frame now: egui's own deadline (tooltip delay, caret blink) is due.
    pub redraw: bool,
    pub flow: Flow,
}

/// The loop's wake decision. `egui_at` = when egui wants its next frame (`Ui::repaint_at`);
/// `background` = every background deadline (none of which draws); `turn_now` = background work
/// is already waiting (a file result to apply) — one more pass, not a frame.
pub fn plan(now: Instant, egui_at: Option<Instant>, background: &[Option<Instant>], turn_now: bool) -> Plan {
    let egui_due = egui_at.is_some_and(|t| t <= now);
    let egui_next = egui_at.filter(|_| !egui_due);
    let soonest = background.iter().copied().flatten().chain(egui_next).chain(turn_now.then_some(now)).min();
    Plan { redraw: egui_due, flow: soonest.map_or(Flow::Wait, Flow::WaitUntil) }
}

/// One display refresh (`millihertz` from the window's monitor); 60 Hz when the OS does not say.
/// Clamped to 30–240 Hz so a bogus report can neither stall nor unleash the loop.
pub fn refresh_interval(millihertz: Option<u32>) -> Duration {
    let hz = millihertz.map_or(60.0, |mhz| f64::from(mhz) / 1000.0).clamp(30.0, 240.0);
    Duration::from_secs_f64(1.0 / hz)
}

/// egui's own next pass (`Ui::repaint_at`: zero for its settle passes and animations, a delay for a
/// tooltip or the caret blink), never sooner than one refresh after the frame that asked
/// (`frame_start`). Input does not go through here — a pointer move still draws at once; only the
/// chains egui drives itself are held to the display rate (the present mode is `Immediate`, so
/// nothing else bounds them).
pub fn paced(egui_at: Option<Instant>, frame_start: Instant, interval: Duration) -> Option<Instant> {
    egui_at.map(|t| t.max(frame_start + interval))
}

/// Does this window event change what we draw? egui-winit asks for a repaint on every event; a window
/// move or being covered changes none of our pixels (the uncover redraw is its own request).
pub fn event_needs_frame(event: &winit::event::WindowEvent) -> bool {
    use winit::event::WindowEvent as W;
    !matches!(event, W::RedrawRequested | W::Moved(_) | W::Occluded(true))
}

/// Did two frames paint exactly the same egui output? (`VAROS_FRAME_DEBUG=1`'s `repeats`.) A paint
/// callback cannot be compared, so it never counts as the same.
pub fn same_paint(a: &[egui::ClippedPrimitive], b: &[egui::ClippedPrimitive]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.clip_rect == y.clip_rect
                && match (&x.primitive, &y.primitive) {
                    (egui::epaint::Primitive::Mesh(m), egui::epaint::Primitive::Mesh(n)) => m == n,
                    _ => false,
                }
        })
}

/// One interval's counters (`VAROS_FRAME_DEBUG=1`).
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Counters {
    pub frames: u32,
    pub passes: u32,
    pub presents: u32,
    /// Frames that painted exactly what the previous frame painted (same egui meshes, no texture
    /// change, same scene) — measured only, nothing is skipped.
    pub repeats: u32,
    /// CPU time spent in `AboutToWait` passes (the loop's own bookkeeping, frames excluded).
    pub pass_time: Duration,
    pub waits: u32,
    pub wait_untils: u32,
    /// `window.request_redraw()` calls by reason.
    pub redraw: BTreeMap<&'static str, u32>,
    /// Why egui ran the frames it asked for (`Context::repaint_causes`, file:line).
    pub egui: BTreeMap<String, u32>,
    /// What woke the loop (NewEvents cause, user events, window-event kinds).
    pub wakes: BTreeMap<&'static str, u32>,
}

impl Counters {
    /// Nothing at all happened.
    #[cfg(test)]
    pub fn is_quiet(&self) -> bool {
        *self == Counters::default()
    }
}

/// The instrumentation. Disabled = every call is a branch on a bool.
pub struct FrameStats {
    enabled: bool,
    since: Instant,
    now: Counters,
    last_flow: Option<Flow>,
}

/// The reporting interval.
pub const INTERVAL: Duration = Duration::from_secs(1);

impl FrameStats {
    pub fn new(enabled: bool, now: Instant) -> Self {
        Self { enabled, since: now, now: Counters::default(), last_flow: None }
    }
    /// `VAROS_FRAME_DEBUG=1`.
    pub fn from_env() -> Self {
        Self::new(std::env::var_os("VAROS_FRAME_DEBUG").is_some_and(|v| v == "1"), Instant::now())
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn redraw(&mut self, why: &'static str) {
        if self.enabled {
            *self.now.redraw.entry(why).or_default() += 1;
        }
    }
    pub fn wake(&mut self, what: &'static str) {
        if self.enabled {
            *self.now.wakes.entry(what).or_default() += 1;
        }
    }
    pub fn frame(&mut self) {
        if self.enabled {
            self.now.frames += 1;
        }
    }
    pub fn present(&mut self) {
        if self.enabled {
            self.now.presents += 1;
        }
    }
    pub fn repeat(&mut self, same: bool) {
        if self.enabled && same {
            self.now.repeats += 1;
        }
    }
    pub fn pass(&mut self, took: Duration) {
        if self.enabled {
            self.now.passes += 1;
            self.now.pass_time += took;
        }
    }
    pub fn egui_causes(&mut self, causes: impl IntoIterator<Item = String>) {
        if self.enabled {
            for c in causes {
                *self.now.egui.entry(c).or_default() += 1;
            }
        }
    }
    pub fn flow(&mut self, flow: Flow) {
        if self.enabled {
            match flow {
                Flow::Wait => self.now.waits += 1,
                Flow::WaitUntil(_) => self.now.wait_untils += 1,
            }
            self.last_flow = Some(flow);
        }
    }
    /// The counters so far (tests).
    #[cfg(test)]
    pub fn counters(&self) -> &Counters {
        &self.now
    }
    /// Close the interval when it is at least [`INTERVAL`] old: the report line (or `None`).
    pub fn tick(&mut self, now: Instant) -> Option<String> {
        if !self.enabled || now.saturating_duration_since(self.since) < INTERVAL {
            return None;
        }
        let secs = now.saturating_duration_since(self.since).as_secs_f64();
        let c = std::mem::take(&mut self.now);
        self.since = now;
        let map = |m: &BTreeMap<&'static str, u32>| m.iter().map(|(k, v)| format!("{k}×{v}")).collect::<Vec<_>>();
        let egui: Vec<String> = c.egui.iter().map(|(k, v)| format!("{k}×{v}")).collect();
        let flow = match self.last_flow {
            None => "-".to_string(),
            Some(Flow::Wait) => "Wait".into(),
            Some(Flow::WaitUntil(t)) => format!("WaitUntil(+{}ms)", t.saturating_duration_since(now).as_millis()),
        };
        // wall clock (Unix seconds) so a line can be matched to an outside measurement
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Some(format!(
            "[varos-frame] t={unix} {secs:.1}s frames={} ({:.1}/s) presents={} repeats={} passes={} ({:.1}/s, {:.2} ms total) flow wait={} until={} last={flow} | redraw: {} | egui: {} | wakes: {}",
            c.frames,
            f64::from(c.frames) / secs,
            c.presents,
            c.repeats,
            c.passes,
            f64::from(c.passes) / secs,
            c.pass_time.as_secs_f64() * 1000.0,
            c.waits,
            c.wait_untils,
            map(&c.redraw).join(" "),
            egui.join(" "),
            map(&c.wakes).join(" "),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn at_rest_the_loop_sleeps_without_a_frame() {
        let now = Instant::now();
        assert_eq!(plan(now, None, &[None, None, None], false), Plan { redraw: false, flow: Flow::Wait });
    }

    #[test]
    fn a_background_deadline_wakes_a_turn_never_a_frame() {
        let now = Instant::now();
        // the 30-second recovery copy, still ahead
        let tick = now + 30_000 * MS;
        assert_eq!(plan(now, None, &[Some(tick), None], false), Plan { redraw: false, flow: Flow::WaitUntil(tick) });
        // already due (the deadline passed while the pass ran): one more pass at once — no frame
        let due = now - MS;
        assert_eq!(plan(now, None, &[Some(due)], false), Plan { redraw: false, flow: Flow::WaitUntil(due) });
        // a finished file job waits to be applied: a turn now, no frame
        assert_eq!(plan(now, None, &[], true), Plan { redraw: false, flow: Flow::WaitUntil(now) });
    }

    #[test]
    fn egui_deadlines_draw_only_when_due() {
        let now = Instant::now();
        let blink = now + 500 * MS;
        let p = plan(now, Some(blink), &[Some(now + 30_000 * MS)], false);
        assert_eq!(p, Plan { redraw: false, flow: Flow::WaitUntil(blink) }, "the earliest deadline wins");
        let p = plan(blink, Some(blink), &[Some(now + 30_000 * MS)], false);
        assert_eq!(p, Plan { redraw: true, flow: Flow::WaitUntil(now + 30_000 * MS) }, "due → one frame");
    }

    /// The idle loop with a fake clock: N passes with nothing pending ask for no frame and stay in
    /// `Wait`; the counters show passes only.
    #[test]
    fn idle_passes_produce_no_frames_with_a_fake_clock() {
        let t0 = Instant::now();
        let mut stats = FrameStats::new(true, t0);
        let mut egui_at: Option<Instant> = None;
        for i in 0..1000u64 {
            let now = t0 + Duration::from_millis(i * 37);
            stats.pass(Duration::ZERO);
            let p = plan(now, egui_at, &[None, None, None, None], false);
            if p.redraw {
                stats.redraw("egui-timer");
                egui_at = None;
            }
            stats.flow(p.flow);
            assert_eq!(p.flow, Flow::Wait);
        }
        let c = stats.counters();
        assert_eq!((c.frames, c.presents, c.wait_untils), (0, 0, 0));
        assert!(c.redraw.is_empty(), "{:?}", c.redraw);
        assert_eq!(c.passes, 1000);
    }

    #[test]
    fn egui_self_driven_passes_are_held_to_the_display_rate() {
        let t0 = Instant::now();
        let hz120 = refresh_interval(Some(120_000));
        assert!((hz120.as_secs_f64() - 1.0 / 120.0).abs() < 1e-9);
        assert_eq!(refresh_interval(None), Duration::from_secs_f64(1.0 / 60.0), "unknown → 60 Hz");
        assert_eq!(refresh_interval(Some(0)), Duration::from_secs_f64(1.0 / 30.0), "bogus → clamped");
        // zero delay (settle pass / smoothing): one refresh after the frame began
        assert_eq!(paced(Some(t0), t0, hz120), Some(t0 + hz120));
        // a longer delay (caret blink, tooltip) is kept as asked
        assert_eq!(paced(Some(t0 + 500 * MS), t0, hz120), Some(t0 + 500 * MS));
        // nothing asked → nothing scheduled
        assert_eq!(paced(None, t0, hz120), None);
        // a chain of self-driven passes over one second: at most the refresh rate
        let (mut t, mut frames) = (t0, 0);
        while t < t0 + Duration::from_secs(1) {
            frames += 1;
            t = paced(Some(t), t, hz120).unwrap(); // every pass asks for another at once
        }
        assert!(frames <= 121, "{frames} self-driven frames in one second at 120 Hz");
    }

    #[test]
    fn window_moves_and_covering_draw_nothing() {
        use winit::dpi::PhysicalPosition;
        use winit::event::WindowEvent as W;
        assert!(!event_needs_frame(&W::Moved(PhysicalPosition::new(10, 10))));
        assert!(!event_needs_frame(&W::Occluded(true)));
        assert!(!event_needs_frame(&W::RedrawRequested));
        assert!(event_needs_frame(&W::Occluded(false)));
        assert!(event_needs_frame(&W::Focused(true)));
    }

    #[test]
    fn report_line_covers_the_whole_quiet_interval_and_resets() {
        let t0 = Instant::now();
        let mut stats = FrameStats::new(true, t0);
        stats.frame();
        stats.redraw("input");
        assert_eq!(stats.tick(t0 + 500 * MS), None, "under a second: no line yet");
        let line = stats.tick(t0 + 12_000 * MS).expect("a line after the interval");
        assert!(line.contains("12.0s frames=1"), "{line}");
        assert!(line.contains("input×1"), "{line}");
        assert!(stats.counters().is_quiet(), "counters reset after a line");
        let mut off = FrameStats::new(false, t0);
        off.frame();
        assert!(off.counters().is_quiet() && off.tick(t0 + 5_000 * MS).is_none(), "disabled = no-op");
    }
}
