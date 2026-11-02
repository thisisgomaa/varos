//! Drag regressions inspect emitted content shapes, not only panel hit rectangles.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::test_support::*;

impl Harness {
    /// Only Swatches shows (the slice with Layers hidden).
    fn swatches_alone() -> Self {
        let mut h = Self::slice();
        h.action(Action::Hide { host: h.host_id("layers") });
        h
    }

    fn swatches_host(&self) -> u64 {
        self.host_id("swatches")
    }
}

fn text_rect(output: &FullOutput, label: &str) -> (Rect, Rect) {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            Shape::Text(text) if text.galley.text() == label => Some((text.visual_bounding_rect(), shape.clip_rect)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing emitted text {label}"))
}

#[test]
fn floating_panel_contents_move_with_chrome_on_every_held_drag_frame() {
    for from_tab in [false, true] {
        let mut h = Harness::swatches_alone();
        let initial = h.frame(vec![]);
        let host = h.hit(&format!("host:{}", h.swatches_host()));
        let labels = ["Swatches", "Recent Colors", "Swatch Tiles"];
        let originals = labels.map(|label| text_rect(&initial, label).0.min - host.min);
        let grab = h.hit(&if from_tab { "tab:swatches".into() } else { format!("host-grip:{}", h.swatches_host()) }).center();
        h.frame(vec![Event::PointerMoved(grab)]);
        h.frame(vec![button(grab, true)]);
        for step in 1..=10 {
            let pointer = grab + vec2(17.0 * step as f32, 11.0 * step as f32);
            let output = h.frame(vec![Event::PointerMoved(pointer)]);
            if step < 2 {
                continue;
            }
            assert!(h.state().gesture.is_some());
            let frame = h.hit(&format!("host:{}", h.swatches_host()));
            for (label, original) in labels.iter().zip(originals) {
                let (painted, clip) = text_rect(&output, label);
                let delta = painted.min - frame.min - original;
                assert!(delta.length() < 1.5, "{label} detached from its chrome during drag: {delta:?}");
                assert!(frame.expand(1.0).contains_rect(clip), "{label} paint clip escaped its moving host");
            }
        }
    }
}

#[test]
fn detaching_a_dock_releases_its_canvas_reservation_during_the_visible_drag() {
    let mut h = Harness::swatches_alone();
    h.app
        .run(
            "ui.panelReview.action",
            json!({"action":Action::Move { source: Source::Host(h.swatches_host()), target: Target::Dock { edge: Edge::Right } }}),
        )
        .unwrap();
    for _ in 0..5 {
        h.frame(vec![]);
    }
    let docked_canvas = h.app.canvas_rect.unwrap();
    let docked = h.hit(&format!("host:{}", h.swatches_host()));
    let from = h.hit(&format!("host-grip:{}", h.swatches_host())).center();
    h.frame(vec![Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    for step in 1..=8 {
        h.frame(vec![Event::PointerMoved(from.lerp(pos2(650.0, 300.0), step as f32 / 8.0))]);
    }
    assert!(h.state().gesture.is_some());
    assert_eq!(h.state().layout.owner("swatches").unwrap().dock, Some(Edge::Right), "the committed layout stays unchanged until release");
    let moved = h.hit(&format!("host:{}", h.swatches_host()));
    assert!(moved.right() < docked.left(), "the preview is now floating away from the dock");
    let canvas = h.app.canvas_rect.unwrap();
    assert!(
        canvas.right() > docked_canvas.right() + docked.width() - 2.0,
        "the preview removed the dock but left its empty canvas reservation: {canvas:?} vs {docked_canvas:?}"
    );
}

#[test]
fn releasing_a_dock_drag_outside_the_workspace_restores_chrome_and_canvas_in_that_frame() {
    for outside in [pos2(650.0, 2.0), pos2(650.0, 999.0)] {
        let mut h = Harness::swatches_alone();
        let host = h.swatches_host();
        h.app
            .run("ui.panelReview.action", json!({"action":Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Right } }}))
            .unwrap();
        for _ in 0..5 {
            h.frame(vec![]);
        }
        let layout = h.state().layout.clone();
        let dock = h.hit(&format!("host:{host}"));
        let canvas = h.app.canvas_rect.unwrap();
        let from = h.hit(&format!("host-grip:{host}")).center();
        h.frame(vec![Event::PointerMoved(from)]);
        h.frame(vec![button(from, true)]);
        for step in 1..=8 {
            h.frame(vec![Event::PointerMoved(from.lerp(pos2(650.0, 300.0), step as f32 / 8.0))]);
        }
        assert!(h.state().gesture.is_some());
        h.frame(vec![Event::PointerMoved(outside)]);
        let release = h.frame(vec![button(outside, false)]);
        assert!(h.state().gesture.is_none());
        assert_eq!(h.state().layout, layout);
        assert_eq!(h.hit(&format!("host:{host}")), dock, "the release frame must not retain a floating preview");
        assert_eq!(h.app.canvas_rect.unwrap(), canvas, "the release frame must restore the dock's canvas reservation");
        let (tab, clip) = text_rect(&release, "Swatches");
        assert!(dock.contains_rect(tab) && dock.contains_rect(clip), "painted chrome is restored with its hit geometry");
    }
}
