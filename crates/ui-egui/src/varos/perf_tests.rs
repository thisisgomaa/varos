//! Headless CPU measurements, never a window/GPU/presentation benchmark.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Instant;

use egui::{Event, Pos2, RawInput, Rect, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use super::profile;
use crate::VectorcraftApp;
use crate::native_menu::{Backend, MenuBar, NativeMenu};

struct NullMenu;
impl Backend for NullMenu {
    fn sync(&mut self, _: &MenuBar) {}
    fn drain(&mut self) -> Vec<crate::native_menu::Event> {
        vec![]
    }
}

fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, i: usize, moving: bool, ppp: f32) -> f64 {
    let events = if moving {
        let r = app.canvas_rect.unwrap_or(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)));
        vec![Event::PointerMoved(r.center() + vec2((i % 50) as f32, 0.0))]
    } else {
        vec![]
    };
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0))),
        max_texture_side: Some(8192),
        time: Some(i as f64 / 60.0),
        events,
        ..Default::default()
    };
    raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    let start = Instant::now();
    let mut output = ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    });
    // Like the other headless tests: don't leave egui's texture free callbacks alive.
    output.textures_delta.clear();
    assert!(!app.ui.status.starts_with("Internal error:"), "{}", app.ui.status);
    start.elapsed().as_secs_f64() * 1000.0
}

#[test]
#[ignore = "CPU timing: run alone with VAROS_PROFILE=1 and --nocapture"]
fn headless_ui_breakdown() {
    assert!(std::env::var_os("VAROS_PROFILE").is_some_and(|v| v == "1"));
    for (label, selected, moving, ppp) in [
        ("empty-still", false, false, 1.0),
        ("empty-pointer", false, true, 1.0),
        ("selected-pointer", true, true, 1.0),
        ("empty-pointer-retina", false, true, 2.0),
        ("selected-pointer-retina", true, true, 2.0),
    ] {
        let ctx = egui::Context::default();
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        // Exercise the Mac menu model without AppKit. Actual backend work is NOT measured.
        app.services.native_menu = Some(NativeMenu::new(Box::new(NullMenu)));
        app.canvas.worker_started = true;
        app.run("file.new", json!({"width": 800, "height": 600})).unwrap();
        if selected {
            let id = app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 150, "height": 100})).unwrap()["id"].clone();
            app.run("select.set", json!({"ids": [id]})).unwrap();
        }
        // Include cold observations, then warm fonts, layout, images, menus and artwork.
        let mut cold = Vec::new();
        for i in 0..20 {
            let ms = frame(&mut app, &ctx, i, moving, ppp);
            if i < 3 {
                cold.push(ms);
            }
        }
        eprintln!("PERF {label}: first three full egui+logic frames (font initialization included): {cold:?} ms");
        // Finish any fallback-font search before the steady-state samples; apply and warm
        // the returned font definitions without charging the samples for a cold atlas.
        app.ui_fonts.finish(&ctx);
        for i in 20..22 {
            frame(&mut app, &ctx, i, moving, ppp);
        }
        profile::capture();
        let mut times: Vec<_> = (22..222).map(|i| frame(&mut app, &ctx, i, moving, ppp)).collect();
        times.sort_by(f64::total_cmp);
        eprintln!(
            "PERF {label}: 1440x900 logical px, ppp={}, dev build, 200 warm frames; full egui+logic p50={:.3}ms p95={:.3}ms mean={:.3}ms",
            ctx.pixels_per_point(),
            times[100],
            times[190],
            times.iter().sum::<f64>() / times.len() as f64
        );
        let mut costs: Vec<_> = profile::captured().into_iter().collect();
        // A narrow candidate measurement, outside the frame samples: command shortcut table
        // construction currently occurs on pointer-only frames too.
        let start = Instant::now();
        for _ in 0..100 {
            std::hint::black_box(crate::shortcuts::all_shortcuts());
        }
        eprintln!("  candidate shortcut-table build: {:.3}ms/call (100 calls)", start.elapsed().as_secs_f64() * 10.0);
        costs.sort_by(|a, b| b.1.total_ms.total_cmp(&a.1.total_ms));
        for (name, cost) in costs {
            eprintln!("  {name}: mean={:.3}ms max={:.3}ms calls={}", cost.total_ms / cost.calls.max(1) as f64, cost.max_ms, cost.calls);
        }
    }
}

/// Test the suspected one-frame pointer queue directly, without a window or synthetic-input queue.
#[test]
fn canvas_hover_consumes_this_frames_pointer_without_editing() {
    let ctx = egui::Context::default();
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.canvas.worker_started = true;
    app.run("file.new", json!({"width": 800, "height": 600})).unwrap();
    for i in 0..20 {
        frame(&mut app, &ctx, i, false, 1.0);
    }
    let revision = app.session.active().unwrap().revision;
    for i in [20, 40] {
        let rect = app.canvas_rect.unwrap();
        let pos = rect.center() + vec2((i % 50) as f32, 0.0);
        let expected = crate::canvas::Xf::new(rect, app.view().unwrap()).to_doc(pos);
        frame(&mut app, &ctx, i, true, 1.0);
        let actual = app.hover_doc.unwrap();
        assert!((actual.x - expected.x).abs() < 1e-6 && (actual.y - expected.y).abs() < 1e-6, "current pointer: {expected:?}; consumed: {actual:?}");
        assert_eq!(app.session.active().unwrap().revision, revision, "hover alone leaves the document unchanged");
    }
}

/// The UI pass with a huge compound path selected (the v7 stress document: one compound of 13.9k
/// parts). `VAROS_STRESS_DOC=<file.vectorcraft>` names the document; its first top-level object
/// is selected. Per-section costs with `VAROS_PROFILE=1`.
#[test]
#[ignore = "CPU timing: run alone with VAROS_PROFILE=1 VAROS_STRESS_DOC=<file> and --nocapture"]
fn headless_ui_with_a_huge_compound_selected() {
    let Some(path) = std::env::var_os("VAROS_STRESS_DOC") else { return };
    let path = path.to_string_lossy().to_string();
    let ctx = egui::Context::default();
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.services.native_menu = Some(NativeMenu::new(Box::new(NullMenu)));
    app.canvas.worker_started = true;
    app.run("document.open", json!({ "path": path })).unwrap();
    let id = app.session.active().unwrap().doc.layers[0].children().unwrap()[0].id;
    for (label, select) in [("unselected", false), ("selected", true)] {
        if select {
            app.run("select.set", json!({"ids": [id.0]})).unwrap();
        }
        for i in 0..10 {
            frame(&mut app, &ctx, i, true, 1.0);
        }
        profile::capture();
        let n: usize = std::env::var("VAROS_STRESS_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(30).max(30);
        let mut times: Vec<_> = (10..10 + n).map(|i| frame(&mut app, &ctx, i, true, 1.0)).collect();
        times.sort_by(f64::total_cmp);
        eprintln!("PERF {label}: p50={:.3}ms p95={:.3}ms", times[n / 2], times[n * 95 / 100]);
        let mut costs: Vec<_> = profile::captured().into_iter().collect();
        costs.sort_by(|a, b| b.1.total_ms.total_cmp(&a.1.total_ms));
        for (name, cost) in costs.into_iter().take(14) {
            eprintln!("  {name}: mean={:.3}ms max={:.3}ms calls={}", cost.total_ms / cost.calls.max(1) as f64, cost.max_ms, cost.calls);
        }
    }
}
