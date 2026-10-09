//! Instant view gestures. Pure math/routing: no window, event loop or renderer is constructed.
use varos_core::geom::{pan_for_anchor, Pt, View};
use winit::event::MouseScrollDelta;

const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 40.0;
const ZOOM_NOTCH: f32 = 1.12;

#[derive(Clone, Copy)]
pub struct ZoomDrag {
    pub start: Pt,
    pub current: Pt,
    pub out: bool,
}
pub fn finish_zoom_drag(view: &mut View, drag: ZoomDrag, canvas: egui::Rect) {
    let width = (drag.current[0] - drag.start[0]).abs();
    let height = (drag.current[1] - drag.start[1]).abs();
    if width < 4.0 || height < 4.0 || drag.out {
        zoom_to(view, drag.start, view.zoom * if drag.out { 1.0 / 1.5 } else { 1.5 });
        return;
    }
    let a = view.s2w(drag.start);
    let b = view.s2w(drag.current);
    let center = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let zoom =
        (canvas.width() / (a[0] - b[0]).abs()).min(canvas.height() / (a[1] - b[1]).abs()).clamp(MIN_ZOOM, MAX_ZOOM);
    view.zoom = zoom;
    view.pan = pan_for_anchor(center, [canvas.center().x, canvas.center().y], zoom);
}
pub enum Gesture {
    Pan(Pt),
    Pinch(f64),
    SmartZoom(View),
    Scroll { delta: MouseScrollDelta, alt: bool, shift: bool },
}

/// Winit delivers incremental magnification, not a cumulative gesture scale.
/// exp(delta) composes continuously and stays positive even for large negative deltas.
pub fn pinch_factor(delta: f64) -> f32 {
    if !delta.is_finite() {
        return 1.0; // winit explicitly permits NaN
    }
    delta.clamp(-20.0, 20.0).exp() as f32
}

pub fn zoom_to(view: &mut View, screen: Pt, zoom: f32) {
    if zoom.is_nan() {
        return;
    }
    let anchor = view.s2w(screen);
    view.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    view.pan = pan_for_anchor(anchor, screen, view.zoom);
}

/// Winit deltas and View.pan are both physical pixels; preserve 1:1 movement.
fn pixel_pan(delta: [f64; 2], shift: bool) -> [f64; 2] {
    let [x, y] = delta;
    if shift {
        [x + y, 0.0]
    } else {
        [x, y]
    }
}

/// Fit → pointer-anchored 100% → Fit. An arbitrary view first returns to Fit.
/// Compare pan too: zoom alone cannot tell a panned view from a fitted one.
fn smart_zoom(view: &mut View, screen: Pt, fit: View) {
    let at_fit = (view.zoom - fit.zoom).abs() <= fit.zoom * 1e-5
        && view.pan.iter().zip(fit.pan).all(|(a, b)| (*a - b).abs() <= 0.01);
    if at_fit {
        zoom_to(view, screen, 1.0);
    } else {
        *view = fit;
    }
}

/// The shared hard boundary applies to every view gesture, including floating chrome.
/// `blocked` is Home || Ui::wants_pointer (plus the current native chrome hit).
pub fn apply(view: &mut View, screen: Pt, gesture: Gesture, blocked: bool) -> bool {
    if blocked {
        return false;
    }
    match gesture {
        Gesture::Pan(delta) => {
            if !delta.iter().all(|v| v.is_finite()) {
                return false;
            }
            view.pan = [view.pan[0] + delta[0], view.pan[1] + delta[1]];
        }
        Gesture::Pinch(delta) => {
            let factor = pinch_factor(delta);
            if factor == 1.0 {
                return false; // zero/invalid phase must not clamp a fitted view
            }
            zoom_to(view, screen, view.zoom * factor);
        }
        Gesture::SmartZoom(fit) => smart_zoom(view, screen, fit),
        Gesture::Scroll { delta, alt, shift } => {
            if alt {
                // Preserve Alt+wheel sensitivity, including the existing pixel /40 mapping.
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                zoom_to(view, screen, view.zoom * ZOOM_NOTCH.powf(y).clamp(0.2, 5.0));
            } else {
                let pan = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        if shift {
                            [(x + y) * 30.0, 0.0]
                        } else {
                            [x * 30.0, y * 30.0]
                        }
                    }
                    MouseScrollDelta::PixelDelta(p) => pixel_pan([p.x, p.y], shift).map(|v| v as f32),
                };
                view.pan = [view.pan[0] + pan[0], view.pan[1] + pan[1]];
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;

    fn assert_view(view: View, expected: View) {
        assert_eq!(view.zoom, expected.zoom);
        assert_eq!(view.pan, expected.pan);
    }

    fn assert_anchor(view: View, anchor: Pt, screen: Pt) {
        let actual = view.w2s(anchor);
        for i in 0..2 {
            assert!((actual[i] - screen[i]).abs() < 0.001, "{actual:?} != {screen:?}");
        }
    }

    #[test]
    fn hand_drag_pans_one_to_one_and_respects_chrome_and_invalid_input() {
        let mut view = View { zoom: 2.0, pan: [10.0, 20.0] };
        assert!(!apply(&mut view, [0.0, 0.0], Gesture::Pan([5.0, -7.0]), true));
        assert_eq!(view.pan, [10.0, 20.0]);
        assert!(apply(&mut view, [0.0, 0.0], Gesture::Pan([5.0, -7.0]), false));
        assert_eq!(view.pan, [15.0, 13.0]);
        assert_eq!(view.zoom, 2.0);
        assert!(!apply(&mut view, [0.0, 0.0], Gesture::Pan([f32::NAN, 0.0]), false));
    }
    #[test]
    fn pinch_is_exponential_continuous_and_handles_invalid_delta() {
        assert!(pinch_factor(0.1) > 1.0);
        assert!(pinch_factor(-0.1) < 1.0);
        assert!((pinch_factor(0.1) * pinch_factor(0.2) - pinch_factor(0.3)).abs() < 1e-6);
        for delta in [0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(pinch_factor(delta), 1.0);
            let mut view = View { zoom: 0.02, pan: [3.0, 5.0] };
            let before = view;
            assert!(!apply(&mut view, [100.0, 200.0], Gesture::Pinch(delta), false));
            assert_view(view, before);
        }
    }

    #[test]
    fn pinch_and_clamped_zoom_keep_the_pointer_world_point() {
        for (initial, delta, expected) in
            [(2.0, 0.2, 2.0 * pinch_factor(0.2)), (39.0, 100.0, 40.0), (0.06, -100.0, 0.05)]
        {
            let mut view = View { zoom: initial, pan: [23.0, -17.0] };
            let screen = [311.0, 227.0];
            let anchor = view.s2w(screen);
            assert!(apply(&mut view, screen, Gesture::Pinch(delta), false));
            assert_eq!(view.zoom, expected);
            assert_anchor(view, anchor, screen);
        }
    }

    #[test]
    fn smart_zoom_toggles_fit_and_pointer_anchored_actual_size() {
        // Both a zoomed-out and a zoomed-in Fit must toggle in either direction.
        for zoom in [0.4, 2.0] {
            let fit = View { zoom, pan: [45.0, 67.0] };
            let mut view = fit;
            let screen = [351.0, 243.0];
            let anchor = view.s2w(screen);
            apply(&mut view, screen, Gesture::SmartZoom(fit), false);
            assert_eq!(view.zoom, 1.0);
            assert_anchor(view, anchor, screen);
            apply(&mut view, screen, Gesture::SmartZoom(fit), false);
            assert_view(view, fit);
            view.pan[0] += 100.0;
            apply(&mut view, screen, Gesture::SmartZoom(fit), false);
            assert_view(view, fit);
        }
    }

    #[test]
    fn pixel_pan_is_one_to_one_in_logical_points_at_both_scales() {
        for scale in [1.0, 2.0] {
            let physical = [12.0 * scale, -8.0 * scale];
            for (shift, logical) in [(false, [12.0, -8.0]), (true, [4.0, 0.0])] {
                let mut view = View { zoom: 3.0, pan: [30.0, 50.0] };
                apply(
                    &mut view,
                    [0.0, 0.0],
                    Gesture::Scroll {
                        delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(physical[0], physical[1])),
                        alt: false,
                        shift,
                    },
                    false,
                );
                assert_eq!(view.pan, [30.0 + (logical[0] * scale) as f32, 50.0 + (logical[1] * scale) as f32]);
                assert_eq!(view.zoom, 3.0);
            }
        }
    }

    #[test]
    fn wheel_lines_and_alt_zoom_keep_existing_behavior() {
        for (shift, expected) in [(false, [60.0, -90.0]), (true, [-30.0, 0.0])] {
            let mut view = View::identity();
            apply(
                &mut view,
                [0.0, 0.0],
                Gesture::Scroll { delta: MouseScrollDelta::LineDelta(2.0, -3.0), alt: false, shift },
                false,
            );
            assert_eq!(view.pan, expected);
        }
        for delta in
            [MouseScrollDelta::LineDelta(0.0, 2.0), MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 80.0))]
        {
            let mut view = View::identity();
            let screen = [200.0, 150.0];
            apply(&mut view, screen, Gesture::Scroll { delta, alt: true, shift: true }, false);
            assert_eq!(view.zoom, ZOOM_NOTCH.powf(2.0));
            assert_anchor(view, screen, screen);
        }
    }

    #[test]
    fn blocked_gestures_leave_the_view_unchanged_and_unblocked_gestures_change_it() {
        for blocked in [true, false] {
            let before = View { zoom: 2.0, pan: [23.0, -11.0] };
            for gesture in [
                Gesture::Pinch(0.5),
                Gesture::SmartZoom(View::identity()),
                Gesture::Scroll {
                    delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(20.0, 40.0)),
                    alt: false,
                    shift: false,
                },
                Gesture::Scroll { delta: MouseScrollDelta::LineDelta(0.0, 3.0), alt: true, shift: false },
            ] {
                let mut view = before;
                assert_eq!(apply(&mut view, [100.0, 200.0], gesture, blocked), !blocked);
                if blocked {
                    assert_view(view, before);
                } else {
                    assert!(view.zoom != before.zoom || view.pan != before.pan);
                }
            }
        }
    }
}
