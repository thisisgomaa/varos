//! General › Smooth Zoom (`smoothZoom`, off by default): with it off, every wheel, Cmd/Alt-wheel
//! and pinch zoom lands in the frame its input arrives, with no in-between frames. Professionals
//! read a gliding zoom as lag (owner, 2026-10-19). With it on, the canvas keeps upstream's glide:
//! egui spreads a mouse wheel's notches over about a tenth of a second, and with Zoom with Mouse
//! Wheel the canvas glides each notch in itself.
//!
//! Keyboard zoom (Zoom In/Out, Actual Size, Fit), the Zoom tool's clicks and marquee and the
//! Navigator were already instant; the Zoom tool's press-and-hold zoom is Performance › Animated
//! Zoom, a separate preference.

use egui::{Event, InputState, MouseWheelUnit, Vec2};

/// This frame's zoom factor and scroll when Smooth Zoom is off, replacing what the canvas's
/// gliding `wheel` gave (`factor`, `scroll`). `wheel_zooms`: General › Zoom with Mouse Wheel;
/// `alt`: the last wheel turn was an Alt-wheel one; `glide`: the zoom (natural log) still to glide
/// in, which lands now and is cleared; `line`, `page`: points per wheel line and page;
/// `alt_rate`: how much one point of Alt-wheel zooms (as `exp(points × alt_rate)`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn instant(
    i: &InputState,
    wheel_zooms: bool,
    alt: bool,
    glide: &mut f64,
    line: f32,
    page: f32,
    alt_rate: f64,
    (factor, scroll): (f64, Vec2),
) -> (f64, Vec2) {
    if wheel_zooms {
        // The canvas reads the raw wheel events itself; only its glide is left to land.
        let rest = std::mem::take(glide);
        return (factor * rest.exp(), scroll);
    }
    // egui spreads Cmd-wheel notches (its zoom) and Alt-wheel notches (a scroll the canvas turns
    // into zoom) over several frames: take the raw events instead.
    let cmd_rate = f64::from(egui::InputOptions::default().scroll_zoom_speed);
    let (mut zoom, pan) = match i.multi_touch() {
        Some(t) => (f64::from(t.zoom_delta), t.translation_delta),
        None => (i.events.iter().filter_map(|e| if let Event::Zoom(f) = e { Some(f64::from(*f)) } else { None }).product(), Vec2::ZERO),
    };
    for e in &i.events {
        if let Event::MouseWheel { unit, delta, modifiers, .. } = e {
            let d = match unit {
                MouseWheelUnit::Point => *delta,
                MouseWheelUnit::Line => *delta * line,
                MouseWheelUnit::Page => *delta * page,
            };
            let points = f64::from(d.x + d.y);
            if modifiers.command {
                zoom *= (points * cmd_rate).exp();
            } else if modifiers.alt {
                zoom *= (points * alt_rate).exp();
            }
        }
    }
    // An Alt-wheel turn zoomed above; what egui still glides of it is dropped, not scrolled.
    let scroll = if alt { Vec2::ZERO } else { i.smooth_scroll_delta };
    (zoom, scroll + pan)
}
