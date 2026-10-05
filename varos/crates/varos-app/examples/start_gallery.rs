//! Manual Start v2 — Boards window, for the moderator's screenshot and hand checks:
//! `cargo run -p varos-app --example start_gallery` (1512 × 982, the mockup's size).
//! `VAROS_START_EMPTY=1` shows the first-launch page; `VAROS_START_LIST=1` starts in the list view.
//! Fake data: the mockup's ten boards through the real Recent cache and `StartModel` (board 4 Missing),
//! one Recovered row. The gallery plays the host: filter actions go to the model, Remove rebuilds it,
//! the rest are printed. Never run from automated tests.
use std::{sync::Arc, time::Instant};
use varos_app::{
    shell::{fonts, tokens},
    start::{StartAction, StartModel, StartView},
    start_page::{demo, StartPage},
    storage::recents::Recents,
};
use varos_core::{geom::View, scene::Scene};
use varos_render_wgpu::Renderer;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct Model {
    home: std::path::PathBuf,
    recents: Recents,
    recovered: bool,
    model: StartModel,
}
impl Model {
    fn rebuild(&mut self) {
        let gone = demo::path(&self.home, 3);
        let rows = if self.recovered { vec![demo::recovered()] } else { vec![] };
        let mut next = StartModel::build(&self.recents, demo::NOW, |p| p == gone, rows);
        next.carry_focus_from(&self.model);
        self.model = next;
    }
    fn apply(&mut self, action: StartAction) {
        if self.model.apply(&action) {
            return;
        }
        match action {
            StartAction::RemoveRecent(p) => {
                self.recents.remove(&p);
                self.rebuild();
            }
            StartAction::Recover(_) | StartAction::DiscardRecovery(_) => {
                self.recovered = false;
                self.rebuild();
            }
            other => println!("action: {other:?}"),
        }
    }
}

struct Runtime {
    window: Arc<Window>,
    renderer: Renderer,
    ctx: egui::Context,
    state: egui_winit::State,
    next_repaint: Option<Instant>,
}
struct Gallery {
    runtime: Option<Runtime>,
    page: StartPage,
    model: Model,
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
                        .with_title("Varos — Start v2 gallery")
                        .with_inner_size(winit::dpi::LogicalSize::new(1512.0, 982.0)),
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
        match r.next_repaint {
            Some(deadline) if deadline <= Instant::now() => {
                r.next_repaint = None;
                r.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(deadline) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(r) = &mut self.runtime else {
            return;
        };
        if r.state.on_window_event(&r.window, &event).repaint {
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
                let model = &self.model.model;
                let mut intents = vec![];
                let page = &mut self.page;
                let out = r.ctx.run_ui(input, |ui| {
                    // a stand-in for the app's top bar: the void with the Search pill where the mockup has it
                    let full = ui.max_rect();
                    let bar = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 28.0));
                    ui.painter().rect_filled(bar, 0.0, tokens::SEAM);
                    let search =
                        egui::Rect::from_min_size(egui::pos2(full.right() - 453.0, 2.0), egui::vec2(200.0, 24.0));
                    intents.extend(page.search_box(ui, search, model));
                    let area = egui::Rect::from_min_max(egui::pos2(full.left(), bar.bottom()), full.max);
                    intents.extend(page.draw_in(ui, area, model, None));
                });
                let changed = !intents.is_empty();
                for i in intents {
                    self.model.apply(i);
                }
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
                if changed {
                    r.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
fn main() {
    let mut page = StartPage::new();
    page.command_keys = true;
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let empty = std::env::var_os("VAROS_START_EMPTY").is_some();
    let recents = if empty { Recents::default() } else { demo::recents(&home) };
    let blank = StartModel::without_recovery(&Recents::default(), demo::NOW, |_| false);
    let mut model = Model { home, recents, recovered: !empty, model: blank };
    model.rebuild();
    if std::env::var_os("VAROS_START_LIST").is_some() {
        model.model.apply(&StartAction::SetView(StartView::List));
    }
    let mut gallery = Gallery { runtime: None, page, model };
    EventLoop::new().expect("gallery event loop").run_app(&mut gallery).expect("gallery run");
}
