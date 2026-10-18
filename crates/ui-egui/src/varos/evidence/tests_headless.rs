//! Public-API headless runner linked against the normal dev build, without rebuilding the
//! engine for test-fonts. No window/GPU. VAROS_PROFILE=1 provides section breakdowns on stderr.
use std::time::Instant;

use egui::{Event, Pos2, RawInput, Rect, vec2};
use vectorcraft_engine::Session;
use vectorcraft_ui_egui::VectorcraftApp;
use vectorcraft_ui_egui::native_menu::{Backend, MenuBar, NativeMenu};

struct NullMenu;
impl Backend for NullMenu {
    fn sync(&mut self, _: &MenuBar) {}
    fn drain(&mut self) -> Vec<vectorcraft_ui_egui::native_menu::Event> {
        vec![]
    }
}

fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, i: usize, ppp: f32) -> Result<f64, String> {
    let p = app.canvas_rect.map_or(Pos2::new(400.0, 400.0), |r| r.center()) + vec2((i % 50) as f32, 0.0);
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0))),
        max_texture_side: Some(8192),
        time: Some(i as f64 / 60.0),
        events: vec![Event::PointerMoved(p)],
        ..Default::default()
    };
    raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    let start = Instant::now();
    let mut output = ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    });
    output.textures_delta.clear();
    if app.ui.status.starts_with("Internal error:") {
        return Err(app.ui.status.clone());
    }
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

fn main() -> Result<(), String> {
    let case = std::env::args().nth(1).unwrap_or_else(|| "empty".into());
    let (selected, ppp) = match case.as_str() {
        "empty" => (false, 1.0),
        "selected" => (true, 1.0),
        "empty-retina" => (false, 2.0),
        "selected-retina" => (true, 2.0),
        _ => return Err("case: empty|selected|empty-retina|selected-retina".into()),
    };
    let ctx = egui::Context::default();
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.services.native_menu = Some(NativeMenu::new(Box::new(NullMenu)));
    app.run("file.new", r#"{"width":800,"height":600}"#.parse().map_err(|_| "invalid built-in params")?)?;
    if selected {
        let result = app.run("shape.rectangle", r#"{"x":100,"y":100,"width":150,"height":100}"#.parse().map_err(|_| "invalid built-in params")?)?;
        let id = result.get("id").ok_or("rectangle did not return an id")?;
        app.run("select.set", format!(r#"{{"ids":[{id}]}}"#).parse().map_err(|_| "invalid built-in params")?)?;
    }
    let warm = Instant::now();
    let mut i = 0;
    while i < 20 || warm.elapsed().as_secs_f64() < 1.2 {
        frame(&mut app, &ctx, i, ppp)?;
        i += 1;
    }
    eprintln!("STEADY {case}; 1440x900 points, ppp={}; 1.2s warmup; fake native services", ctx.pixels_per_point());
    let start = Instant::now();
    let mut times = Vec::new();
    let mut ui_ms = 0.0;
    for n in 0..20_000 {
        times.push(frame(&mut app, &ctx, i + n, ppp)?);
        ui_ms += app.perf.frame_ms;
        if times.len() >= 200 && start.elapsed().as_secs_f64() >= 2.2 {
            break;
        }
    }
    times.sort_by(f64::total_cmp);
    let count = times.len().max(1);
    println!(
        "RESULT {case} frames={count} full_mean={:.3}ms p50={:.3}ms p95={:.3}ms status_ui_mean={:.3}ms",
        times.iter().sum::<f64>() / count as f64,
        times.get(count / 2).copied().unwrap_or_default(),
        times.get(count * 95 / 100).copied().unwrap_or_default(),
        ui_ms / count as f64
    );
    Ok(())
}
