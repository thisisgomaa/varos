//! macOS caption decisions; no window, event loop, or GPU needed.
use std::time::{Duration, Instant};

/// Background bar geometry permits dragging only when egui has no interaction there.
pub fn caption_drag_allowed(in_caption: bool, egui_hit: bool, dragging: bool) -> bool {
    in_caption && !egui_hit && !dragging
}

/// Positions are egui logical pixels, independent of the display's scale factor.
pub fn caption_double_click(
    previous: Option<(Instant, [f32; 2])>,
    position: [f32; 2],
    now: Instant,
    dragged: bool,
) -> bool {
    !dragged
        && previous.is_some_and(|(at, first)| {
            now.checked_duration_since(at).is_some_and(|dt| dt <= Duration::from_millis(350))
                && (position[0] - first[0]).powi(2) + (position[1] - first[1]).powi(2) <= 4.0_f32.powi(2)
        })
}

#[derive(Default)]
pub struct CaptionClicks {
    previous: Option<(Instant, [f32; 2])>,
    dragged: bool,
}

impl CaptionClicks {
    pub fn press(&mut self, position: [f32; 2], now: Instant) -> bool {
        let zoom = caption_double_click(self.previous, position, now, self.dragged);
        self.previous = if zoom { None } else { Some((now, position)) };
        self.dragged = false;
        zoom
    }

    pub fn reset_after_drag(&mut self) {
        self.previous = None;
        self.dragged = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_drag_requires_empty_bar_without_egui_hit_or_drag() {
        for in_caption in [false, true] {
            for egui_hit in [false, true] {
                for dragging in [false, true] {
                    assert_eq!(
                        caption_drag_allowed(in_caption, egui_hit, dragging),
                        (in_caption, egui_hit, dragging) == (true, false, false)
                    );
                }
            }
        }
    }

    #[test]
    fn caption_double_click_requires_nearby_timely_presses_without_drag() {
        let at = Instant::now();
        let first = Some((at, [200.0, 20.0]));
        let soon = at + Duration::from_millis(100);
        assert!(!caption_double_click(None, [200.0, 20.0], soon, false));
        assert!(caption_double_click(first, [204.0, 20.0], at + Duration::from_millis(350), false));
        assert!(!caption_double_click(first, [200.0, 20.0], at + Duration::from_millis(351), false));
        assert!(!caption_double_click(first, [204.1, 20.0], soon, false));
        assert!(!caption_double_click(first, [200.0, 24.1], soon, false));
        assert!(!caption_double_click(first, [203.0, 23.0], soon, false));
        assert!(!caption_double_click(first, [800.0, 20.0], soon, false));
        assert!(!caption_double_click(first, [200.0, 20.0], soon, true));
        assert!(!caption_double_click(Some((soon, [200.0, 20.0])), [200.0, 20.0], at, false));
    }

    #[test]
    fn caption_click_sequence_resets_after_drag_and_zoom() {
        let at = Instant::now();
        let mut clicks = CaptionClicks::default();
        let pos = [200.0, 20.0];
        assert!(!clicks.press(pos, at));
        clicks.reset_after_drag();
        assert!(clicks.previous.is_none());
        assert!(!clicks.press(pos, at + Duration::from_millis(100)));
        assert!(clicks.press(pos, at + Duration::from_millis(200)));
        assert!(clicks.previous.is_none());
        assert!(!clicks.press(pos, at + Duration::from_millis(300)));
    }
}
