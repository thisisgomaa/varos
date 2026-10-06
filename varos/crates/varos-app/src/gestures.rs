//! Instant view gestures. Pure math/routing: no window, event loop or renderer is constructed.
use varos_core::geom::{pan_for_anchor, Pt, View};
use winit::event::MouseScrollDelta;

const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 40.0;
const ZOOM_NOTCH: f32 = 1.12;

pub enum Gesture {
    Pinch(f64),
    SmartZoom(View),
    Scroll { delta: MouseScrollDelta, scale_factor: f64, alt: bool, shift: bool },
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

/// Physical winit pixel deltas → logical points. View.pan itself is in physical pixels.
fn pixel_pan_logical(delta: [f64; 2], scale_factor: f64, shift: bool) -> [f64; 2] {
    let [x, y] = delta.map(|v| v / scale_factor);
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
        Gesture::Pinch(delta) => {
            let factor = pinch_factor(delta);
            if factor == 1.0 {
                return false; // zero/invalid phase must not clamp a fitted view
            }
            zoom_to(view, screen, view.zoom * factor);
        }
        Gesture::SmartZoom(fit) => smart_zoom(view, screen, fit),
        Gesture::Scroll { delta, scale_factor, alt, shift } => {
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
                    MouseScrollDelta::PixelDelta(p) => {
                        pixel_pan_logical([p.x, p.y], scale_factor, shift).map(|v| (v * scale_factor) as f32)
                    }
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
                assert_eq!(pixel_pan_logical(physical, scale, shift), logical);
                let mut view = View { zoom: 3.0, pan: [30.0, 50.0] };
                apply(
                    &mut view,
                    [0.0, 0.0],
                    Gesture::Scroll {
                        delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(physical[0], physical[1])),
                        scale_factor: scale,
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
                Gesture::Scroll { delta: MouseScrollDelta::LineDelta(2.0, -3.0), scale_factor: 2.0, alt: false, shift },
                false,
            );
            assert_eq!(view.pan, expected);
        }
        for delta in
            [MouseScrollDelta::LineDelta(0.0, 2.0), MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 80.0))]
        {
            let mut view = View::identity();
            let screen = [200.0, 150.0];
            apply(&mut view, screen, Gesture::Scroll { delta, scale_factor: 2.0, alt: true, shift: true }, false);
            assert_eq!(view.zoom, ZOOM_NOTCH.powf(2.0));
            assert_anchor(view, screen, screen);
        }
    }

    #[test]
    fn home_and_chrome_block_every_view_gesture() {
        // Chrome includes the floating control bar and recovery card; all use the same boundary.
        for (home, chrome) in [(true, false), (false, true), (true, true)] {
            let before = View { zoom: 2.0, pan: [23.0, -11.0] };
            for gesture in [
                Gesture::Pinch(0.5),
                Gesture::SmartZoom(View::identity()),
                Gesture::Scroll {
                    delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(20.0, 40.0)),
                    scale_factor: 2.0,
                    alt: false,
                    shift: false,
                },
                Gesture::Scroll {
                    delta: MouseScrollDelta::LineDelta(0.0, 3.0),
                    scale_factor: 1.0,
                    alt: true,
                    shift: false,
                },
            ] {
                let mut view = before;
                assert!(!apply(&mut view, [100.0, 200.0], gesture, home || chrome));
                assert_view(view, before);
            }
        }
    }
}
