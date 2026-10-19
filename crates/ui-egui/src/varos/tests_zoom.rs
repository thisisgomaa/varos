//! General › Smooth Zoom ([`super::zoom`]): off by default, every zoom lands in one frame.

use egui::{Pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    app
}

fn run(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let mut raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), ..Default::default() };
    app.raw_input_hook(&mut raw);
    let mut out = ctx.run_ui(raw, |ui| crate::canvas::show(app, ui));
    out.textures_delta.clear();
}

/// One wheel notch up over the canvas with `mods` (`"cmd"`, `"alt"`, `""`): the zoom ratio of
/// the first frame that zoomed, and after 40 more frames.
fn notch(app: &mut VectorcraftApp, ctx: &egui::Context, mods: &str) -> (f64, f64) {
    run(app, ctx);
    let at = app.canvas_rect.unwrap().center() + vec2(30.0, 20.0);
    let before = app.view().unwrap().zoom;
    let mut p = json!({"x": at.x, "y": at.y, "dy": 1});
    if !mods.is_empty() {
        p[mods] = json!(true);
    }
    crate::tests_synthetic::control(app, ctx, "ui.wheel", p);
    let mut first = 1.0;
    for _ in 0..3 {
        run(app, ctx);
        first = app.view().unwrap().zoom / before;
        if first != 1.0 {
            break;
        }
    }
    for _ in 0..40 {
        run(app, ctx);
    }
    (first, app.view().unwrap().zoom / before)
}

#[test]
fn smooth_zoom_is_off_by_default() {
    assert!(!vectorcraft_engine::Prefs::default().smooth_zoom);
    let spec = vectorcraft_engine::cmd::prefscmds::PREF_SPECS.iter().find(|s| s.key == "smoothZoom").expect("a Preferences row");
    assert_eq!((spec.category, spec.section, spec.label), ("General", "Options", "Smooth Zoom"));
}

/// Off: a Cmd-wheel or Alt-wheel notch, and with Zoom with Mouse Wheel a plain notch, changes the
/// view's scale in the one frame the notch arrives; the frames after it change nothing.
#[test]
fn without_smooth_zoom_a_notch_zooms_in_one_frame() {
    for (wheel_zooms, mods) in [(false, "cmd"), (false, "alt"), (true, ""), (true, "alt")] {
        let mut app = app();
        app.session.execute("prefs.set", &json!({"key": "zoomWithMouseWheel", "value": wheel_zooms})).unwrap();
        let ctx = egui::Context::default();
        let (first, done) = notch(&mut app, &ctx, mods);
        assert!(done > 1.01, "{wheel_zooms} {mods:?}: the notch zooms in ({done})");
        assert_eq!(first, done, "{wheel_zooms} {mods:?}: all of it in the first frame");
    }
}

/// On: the same notches glide in over several frames (upstream's behaviour), to the same zoom.
#[test]
fn with_smooth_zoom_a_notch_glides_to_the_same_zoom() {
    for (wheel_zooms, mods) in [(false, "cmd"), (false, "alt"), (true, "")] {
        let ctx = egui::Context::default();
        let mut off = app();
        off.session.execute("prefs.set", &json!({"key": "zoomWithMouseWheel", "value": wheel_zooms})).unwrap();
        let (_, instant) = notch(&mut off, &ctx, mods);
        let ctx = egui::Context::default();
        let mut on = app();
        on.session.execute("prefs.set", &json!({"key": "zoomWithMouseWheel", "value": wheel_zooms})).unwrap();
        on.session.execute("prefs.set", &json!({"key": "smoothZoom", "value": true})).unwrap();
        let (first, done) = notch(&mut on, &ctx, mods);
        assert!(first > 1.0 && first < done, "{wheel_zooms} {mods:?}: the first frame takes part of it ({first} of {done})");
        assert!((done / instant - 1.0).abs() < 1e-3, "{wheel_zooms} {mods:?}: the same zoom either way ({done} vs {instant})");
    }
}

/// A pinch (and Zoom In) were instant before; they stay so with Smooth Zoom off.
#[test]
fn pinch_and_zoom_in_land_at_once() {
    let mut app = app();
    let ctx = egui::Context::default();
    run(&mut app, &ctx);
    let at = app.canvas_rect.unwrap().center();
    let before = app.view().unwrap().zoom;
    app.synthetic.push(egui::Event::PointerMoved(at));
    run(&mut app, &ctx);
    app.synthetic.push(egui::Event::Zoom(1.25));
    run(&mut app, &ctx);
    let first = app.view().unwrap().zoom / before;
    run(&mut app, &ctx);
    assert!((first - 1.25).abs() < 1e-6 && app.view().unwrap().zoom / before == first, "{first}");
    let before = app.view().unwrap().zoom;
    app.run("view.zoomIn", json!({})).unwrap();
    assert!(app.view().unwrap().zoom > before);
}

/// A Cmd (or Alt) trackpad gesture keeps zooming after the key is let go mid-gesture, as egui
/// counts it: its momentum events carry no modifier but belong to the gesture that began with
/// one. Each such frame zooms about the pointer and scrolls nothing; a gesture begun without a
/// modifier scrolls.
#[test]
fn a_gesture_keeps_zooming_after_its_modifier_is_let_go() {
    for (key, zooms) in [("cmd", true), ("alt", true), ("", false)] {
        let mut app = app();
        let ctx = egui::Context::default();
        run(&mut app, &ctx);
        let rect = app.canvas_rect.unwrap();
        let at = rect.center() + vec2(30.0, 20.0);
        app.synthetic.push(egui::Event::PointerMoved(at));
        run(&mut app, &ctx);
        let held = match key {
            "cmd" => egui::Modifiers::COMMAND,
            "alt" => egui::Modifiers::ALT,
            _ => egui::Modifiers::NONE,
        };
        let wheel = |phase, modifiers| egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, 4.0), phase, modifiers };
        let steps = [
            (egui::TouchPhase::Start, held),
            (egui::TouchPhase::Move, held),
            // The key is let go: momentum events come without it.
            (egui::TouchPhase::Move, egui::Modifiers::NONE),
            (egui::TouchPhase::Move, egui::Modifiers::NONE),
            (egui::TouchPhase::End, egui::Modifiers::NONE),
        ];
        for (k, (phase, mods)) in steps.into_iter().enumerate() {
            let before = *app.view().unwrap();
            app.synthetic.push(wheel(phase, mods));
            run(&mut app, &ctx);
            let after = *app.view().unwrap();
            if phase != egui::TouchPhase::Move {
                continue;
            }
            let ratio = after.zoom / before.zoom;
            let moved = crate::canvas::Xf::new(rect, &after).to_doc(at).distance(crate::canvas::Xf::new(rect, &before).to_doc(at));
            if zooms {
                assert!(ratio > 1.001 && moved < 1e-6, "{key} step {k}: zooms about the pointer ({ratio}, {moved})");
            } else {
                assert!((ratio - 1.0).abs() < 1e-12 && moved > 0.5, "{key} step {k}: scrolls ({ratio}, {moved})");
            }
        }
    }
}
