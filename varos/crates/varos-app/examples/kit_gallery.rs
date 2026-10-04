//! Manual U0-B/C window. `cargo run -p varos-app --example kit_gallery`.
//! Never run from automated tests; production host/start wiring is a separate slice.
use std::{sync::Arc, time::Instant};
use varos_app::shell::{
    fonts,
    kit::{self, Availability, Control, Icon},
    tokens,
};
use varos_core::{geom::View, scene::Scene};
use varos_render_wgpu::Renderer;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[derive(Default)]
struct Gallery {
    runtime: Option<Runtime>,
    count: usize,
}
struct Runtime {
    window: Arc<Window>,
    renderer: Renderer,
    ctx: egui::Context,
    state: egui_winit::State,
    next_repaint: Option<Instant>,
}
impl ApplicationHandler for Gallery {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.runtime.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Varos — UI kit gallery")
                        .with_inner_size(winit::dpi::LogicalSize::new(760.0, 580.0)),
                )
                .expect("gallery window"),
        );
        let size = window.inner_size();
        let renderer = pollster::block_on(Renderer::new(window.clone(), size.width, size.height)).expect("gallery GPU");
        let ctx = egui::Context::default();
        fonts::install(&ctx);
        tokens::apply(&ctx);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.style_mut_of(theme, |s| s.text_styles = tokens::text_styles());
        }
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, &window, None, None, None);
        window.request_redraw();
        self.runtime = Some(Runtime { window, renderer, ctx, state, next_repaint: None });
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(r) = &mut self.runtime else {
            return;
        };
        if let Some(deadline) = r.next_repaint {
            if deadline <= Instant::now() {
                r.next_repaint = None;
                r.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(r) = &mut self.runtime else {
            return;
        };
        let response = r.state.on_window_event(&r.window, &event);
        if response.repaint {
            r.window.request_redraw();
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                r.renderer.resize(size.width, size.height);
                r.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let input = r.state.take_egui_input(&r.window);
                let out = r.ctx.run_ui(input, |ui| {
                    egui::Frame::new().fill(tokens::PANEL).inner_margin(24.0).show(ui, |ui| {
                        draw(ui, &mut self.count);
                    });
                });
                // Honor delayed egui repaints (e.g. disabled-reason tooltips) without polling.
                r.next_repaint = out
                    .viewport_output
                    .get(&egui::ViewportId::ROOT)
                    .and_then(|v| Instant::now().checked_add(v.repaint_delay));
                r.state.handle_platform_output(&r.window, out.platform_output);
                let jobs = r.ctx.tessellate(out.shapes, out.pixels_per_point);
                let size = r.window.inner_size();
                r.renderer.render_ui(
                    &Scene::default(),
                    View::identity(),
                    &jobs,
                    &out.textures_delta,
                    &egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [size.width, size.height],
                        pixels_per_point: out.pixels_per_point,
                    },
                );
            }
            _ => {}
        }
    }
}
fn draw(ui: &mut egui::Ui, count: &mut usize) {
    ui.set_min_size(ui.available_size());
    ui.horizontal(|ui| {
        let mut c = Control::new(egui::Id::new("home"), "Home");
        c.icon = Some(Icon::Home);
        c.selected = true;
        c.help = "Home";
        if kit::action(ui, c, true).activated {
            *count += 1;
        }
        kit::section_heading(ui, "Varos / Start components");
    });
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        for (id, label, icon, help) in [
            ("new", "New document", Icon::New, "Create a document · ⌘N"),
            ("open", "Open…", Icon::Open, "Open a .vrs file · ⌘O"),
        ] {
            let mut c = Control::new(egui::Id::new(id), label);
            c.icon = Some(icon);
            c.help = help;
            if kit::action(ui, c, false).activated {
                *count += 1;
            }
        }
    });
    ui.add_space(24.0);
    kit::section_heading(ui, "Recent documents");
    for (id, title, detail) in [
        ("recent", "Brand identity.vrs", "Documents / Varos · Today"),
        (
            "long",
            "Very long document name that stays within the available row width when the window is resized.vrs",
            "/Users/designer/Documents/Clients/An equally long project folder/Final presentations · Yesterday",
        ),
    ] {
        let mut c = Control::new(egui::Id::new(id), title);
        c.help = detail;
        if kit::list_row(ui, c, detail).activated {
            *count += 1;
        }
    }
    let mut missing = Control::new(egui::Id::new("missing"), "Moved document.vrs");
    missing.availability = Availability::Disabled("This file could not be found. Locate it or remove it from Recent.");
    kit::list_row(ui, missing, "Missing · Documents / Previous project");
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        let mut busy = Control::new(egui::Id::new("busy"), "Opening…");
        busy.availability = Availability::Busy("Opening the document. Please wait.");
        kit::action(ui, busy, false);
        let mut remove = Control::new(egui::Id::new("remove"), "Remove from Recent");
        remove.icon = Some(Icon::Remove);
        remove.help = "Remove from Recent";
        if kit::action(ui, remove, true).activated {
            *count += 1;
        }
    });
    ui.add_space(16.0);
    kit::notice(ui, "Couldn't open this file: it isn't a supported Varos document.");
    kit::notice(ui, "No recent documents. Create a document or open a .vrs file.");
    ui.add_space(24.0);
    ui.label(format!("Accepted actions: {count}"));
    kit::notice(ui, "Component gallery only. Tab / Shift-Tab to move; Enter / Space to activate.");
}
fn main() {
    EventLoop::new().expect("gallery event loop").run_app(&mut Gallery::default()).expect("gallery run");
}
