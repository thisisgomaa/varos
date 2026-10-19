//! General › Smooth Zoom (`smoothZoom`, off by default): with it off, every wheel, Cmd/Alt-wheel
//! and pinch zoom lands in the frame its input arrives, with no in-between frames. Professionals
//! read a gliding zoom as lag (owner, 2026-10-19). With it on, the canvas keeps upstream's glide:
//! egui spreads a mouse wheel's notches over about a tenth of a second, and with Zoom with Mouse
//! Wheel the canvas glides each notch in itself.
//!
//! Keyboard zoom (Zoom In/Out, Actual Size, Fit), the Zoom tool's clicks and marquee and the
//! Navigator were already instant; the Zoom tool's press-and-hold zoom is Performance › Animated
//! Zoom, a separate preference.

use egui::{Event, InputState, Modifiers, MouseWheelUnit, TouchPhase, Ui, Vec2};

/// The modifiers a wheel gesture counts as held, as egui's own wheel state keeps them: a trackpad
/// gesture (its `Start` to its `End`) keeps the modifiers it began with, plus any pressed during
/// it, so its momentum after Cmd or Alt is let go still zooms; a wheel turn outside a gesture
/// takes each event's own modifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Gesture {
    in_touch: bool,
    mods: Modifiers,
}

impl Gesture {
    /// Follow wheel event `phase` with `mods` held; the modifiers its delta counts with (none for
    /// a `Start`, `End` or `Cancel`, which carry no motion, as in egui).
    pub(crate) fn step(&mut self, phase: TouchPhase, mods: Modifiers) -> Option<Modifiers> {
        match phase {
            TouchPhase::Start => {
                *self = Self { in_touch: true, mods };
                None
            }
            TouchPhase::Move => {
                if self.in_touch {
                    self.mods |= mods;
                } else {
                    self.mods = mods;
                }
                Some(self.mods)
            }
            TouchPhase::End | TouchPhase::Cancel => {
                *self = Self::default();
                None
            }
        }
    }
}

/// This frame's zoom factor and scroll when Smooth Zoom is off, replacing what the canvas's
/// gliding `wheel` gave (`factor`, `scroll`). `wheel_zooms`: General › Zoom with Mouse Wheel;
/// `alt`: the last wheel turn was an Alt-wheel one; `glide`: the zoom (natural log) still to glide
/// in, which lands now and is cleared; `line`, `page`: points per wheel line and page;
/// `alt_rate`: how much one point of Alt-wheel zooms (as `exp(points × alt_rate)`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn instant(
    ui: &Ui,
    wheel_zooms: bool,
    alt: bool,
    glide: &mut f64,
    line: f32,
    page: f32,
    alt_rate: f64,
    (factor, scroll): (f64, Vec2),
) -> (f64, Vec2) {
    let id = egui::Id::new("varos-zoom-wheel-gesture");
    let mut gesture: Gesture = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let out = ui.input(|i| frame(i, &mut gesture, wheel_zooms, alt, glide, line, page, alt_rate, (factor, scroll)));
    ui.data_mut(|d| d.insert_temp(id, gesture));
    out
}

#[allow(clippy::too_many_arguments)]
fn frame(
    i: &InputState,
    gesture: &mut Gesture,
    wheel_zooms: bool,
    alt: bool,
    glide: &mut f64,
    line: f32,
    page: f32,
    alt_rate: f64,
    (factor, scroll): (f64, Vec2),
) -> (f64, Vec2) {
    if wheel_zooms {
        // The canvas reads the raw wheel events itself; only its glide is left to land. The
        // gesture is still followed, for when the preference changes.
        for e in &i.events {
            if let Event::MouseWheel { phase, modifiers, .. } = e {
                gesture.step(*phase, *modifiers);
            }
        }
        let rest = std::mem::take(glide);
        return (factor * rest.exp(), scroll);
    }
    // egui spreads Cmd-wheel notches (its zoom) and Alt-wheel notches (a scroll the canvas turns
    // into zoom) over several frames: take the raw events instead, classified as egui does.
    let cmd_rate = f64::from(egui::InputOptions::default().scroll_zoom_speed);
    let (mut zoom, pan) = match i.multi_touch() {
        Some(t) => (f64::from(t.zoom_delta), t.translation_delta),
        None => (i.events.iter().filter_map(|e| if let Event::Zoom(f) = e { Some(f64::from(*f)) } else { None }).product(), Vec2::ZERO),
    };
    let mut alt_turn = false;
    for e in &i.events {
        if let Event::MouseWheel { unit, delta, modifiers, phase } = e {
            let Some(held) = gesture.step(*phase, *modifiers) else { continue };
            let d = match unit {
                MouseWheelUnit::Point => *delta,
                MouseWheelUnit::Line => *delta * line,
                MouseWheelUnit::Page => *delta * page,
            };
            let points = f64::from(d.x + d.y);
            if held.command {
                zoom *= (points * cmd_rate).exp();
            } else if held.alt {
                zoom *= (points * alt_rate).exp();
                alt_turn = true;
            }
        }
    }
    // An Alt-wheel turn zoomed above; what egui still glides of it (or passes on as a scroll) is
    // dropped, not scrolled. egui itself counts a Cmd turn as zoom, never as a scroll.
    let scroll = if alt || alt_turn { Vec2::ZERO } else { i.smooth_scroll_delta };
    (zoom, scroll + pan)
}
