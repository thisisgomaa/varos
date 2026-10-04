//! Manual Start v2 — Boards window (lane L4), for the moderator's screenshot and hand checks:
//! `cargo run -p varos-app --example start_gallery` (1512 × 982, the mockup's size).
//! `VAROS_START_EMPTY=1` shows the first-launch page; `VAROS_START_LIST=1` starts in the list view.
//! Fake data: the mockup's ten boards (board 4 Missing), one Recovered row, simple generated
//! thumbnails. The gallery plays the model: it applies filter / search / view / remove intents and
//! prints the rest. Never run from automated tests.
use std::{sync::Arc, time::Instant};
use varos_app::{
    shell::{fonts, tokens},
    start_page::StartPage,
    start_view::{demo, StartIntent, StartView, ViewMode},
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
    empty: bool,
    filter: Option<String>,
    search: String,
    view: ViewMode,
    removed: Vec<String>,
    recovered: bool,
    thumbs: Vec<egui::TextureHandle>,
}
impl Model {
    fn view(&self) -> StartView {
        if self.empty {
            return StartView::default();
        }
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        let mut v = demo::view(&home, Some(3), self.filter.as_deref(), &self.search);
        v.view = self.view;
        v.cards.retain(|c| !self.removed.contains(&c.key));
        for c in &mut v.cards {
            let i: usize = c.key.trim_start_matches("board-").parse().unwrap_or(0);
            c.thumb = self.thumbs.get(i).map(|t| t.id());
        }
        v.total -= self.removed.len();
        if !self.recovered {
            v.recovered.clear();
        }
        v
    }
    fn apply(&mut self, intent: StartIntent) {
        match intent {
            StartIntent::SetTagFilter(f) => self.filter = f,
            StartIntent::Search(s) => self.search = s,
            StartIntent::SetView(v) => self.view = v,
            StartIntent::Remove(k) => self.removed.push(k),
            StartIntent::Recover(_) | StartIntent::Discard(_) => self.recovered = false,
            other => println!("intent: {other:?}"),
        }
    }
}

/// A plain generated stand-in for lane L3's thumbnails: artboards (or free shapes) on transparent.
fn thumb(ctx: &egui::Context, i: usize) -> egui::TextureHandle {
    type Art = (&'static [(f32, f32)], [u8; 3], [u8; 3]);
    const ART: [Art; 10] = [
        (&[(1080.0, 1350.0), (1080.0, 1920.0)], [0x16, 0x23, 0x3d], [0xe9, 0xc4, 0x6a]),
        (&[], [0xec, 0xe6, 0xdc], [0xd0, 0x62, 0x3f]),
        (&[(842.0, 1191.0)], [0xec, 0xe4, 0xd6], [0xe4, 0x57, 0x2e]),
        (&[], [0xd6, 0xd0, 0xc8], [0xd6, 0xd0, 0xc8]),
        (&[(1080.0, 1920.0)], [0xe8, 0x55, 0x3f], [0xf6, 0xe7, 0xd2]),
        (&[(1050.0, 600.0), (1050.0, 600.0)], [0x2a, 0x23, 0x21], [0xe7, 0xd3, 0xb0]),
        (&[], [0x2f, 0x6f, 0x6a], [0xe7, 0xd3, 0xb0]),
        (&[(595.0, 842.0)], [0xff, 0xff, 0xff], [0x1b, 0x19, 0x19]),
        (&[(1080.0, 1080.0)], [0x1f, 0x4d, 0x3a], [0xe3, 0xb0, 0x4b]),
        (&[], [0xec, 0xe6, 0xdc], [0xd0, 0x62, 0x3f]),
    ];
    let (w, h) = (488usize, 198usize);
    let mut px = vec![egui::Color32::TRANSPARENT; w * h];
    let (boards, bg, accent) = ART[i % ART.len()];
    let mut put = |x: usize, y: usize, c: [u8; 3]| {
        if x < w && y < h {
            px[y * w + x] = egui::Color32::from_rgb(c[0], c[1], c[2]);
        }
    };
    if boards.is_empty() {
        // free artwork: a row of shapes
        for k in 0..4 {
            let cx = 60 + k * 120;
            for y in 40..160 {
                for x in cx - 50..cx + 50 {
                    let (dx, dy) = (x as f32 - cx as f32, y as f32 - 100.0);
                    if dx * dx + dy * dy < 2500.0 {
                        put(x, y, if k % 2 == 0 { bg } else { accent });
                    }
                }
            }
        }
    } else {
        let gap = 160.0;
        let total_w: f32 = boards.iter().map(|b| b.0).sum::<f32>() + gap * (boards.len() - 1) as f32;
        let total_h = boards.iter().map(|b| b.1).fold(0.0, f32::max);
        let s = (w as f32 / total_w).min(h as f32 / total_h);
        let mut x0 = (w as f32 - total_w * s) / 2.0;
        for (bw, bh) in boards {
            let (rw, rh) = ((bw * s) as usize, (bh * s) as usize);
            let (ox, oy) = (x0 as usize, (h - rh) / 2);
            for y in 0..rh {
                for x in 0..rw {
                    let (dx, dy) = (x as f32 - rw as f32 * 0.6, y as f32 - rh as f32 * 0.35);
                    let inside = dx * dx + dy * dy < (rw as f32 * 0.28).powi(2);
                    put(ox + x, oy + y, if inside { accent } else { bg });
                }
            }
            x0 += (bw + gap) * s;
        }
    }
    let image = egui::ColorImage::new([w, h], px);
    ctx.load_texture(format!("thumb-{i}"), image, egui::TextureOptions::LINEAR)
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
        self.model.thumbs = (0..10).map(|i| thumb(&ctx, i)).collect();
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
                let view = self.model.view();
                let mut intents = vec![];
                let page = &mut self.page;
                let out = r.ctx.run_ui(input, |ui| {
                    // a stand-in for the app's top bar: the void with the Search pill where the mockup has it
                    let full = ui.max_rect();
                    let bar = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 28.0));
                    ui.painter().rect_filled(bar, 0.0, tokens::SEAM);
                    let search =
                        egui::Rect::from_min_size(egui::pos2(full.right() - 453.0, 2.0), egui::vec2(200.0, 24.0));
                    intents.extend(page.search_box(ui, search, &view));
                    let area = egui::Rect::from_min_max(egui::pos2(full.left(), bar.bottom()), full.max);
                    intents.extend(page.draw_in(ui, area, &view));
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
    let model = Model {
        empty: std::env::var_os("VAROS_START_EMPTY").is_some(),
        filter: None,
        search: String::new(),
        view: if std::env::var_os("VAROS_START_LIST").is_some() { ViewMode::List } else { ViewMode::Grid },
        removed: vec![],
        recovered: true,
        thumbs: vec![],
    };
    let mut gallery = Gallery { runtime: None, page, model };
    EventLoop::new().expect("gallery event loop").run_app(&mut gallery).expect("gallery run");
}
