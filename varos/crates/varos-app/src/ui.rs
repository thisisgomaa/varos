//! Native GPU UI spike (egui 0.27): a floating "Properties" inspector + a second panel, drawn on
//! OUR wgpu surface via `Renderer::render_ui` (egui shares our Device/Queue; no second window). The
//! existing web panels keep running in parallel. This proves the native frosted-glass path (gates 1-4).

use egui::{Color32, RichText, Stroke, Rounding, Margin};
use winit::event::WindowEvent;
use winit::window::Window;

/// Snapshot of the editor selection the panel displays (read-only for the spike).
pub struct PanelData {
    pub sel: bool,
    pub kind: String,
    pub x: f32, pub y: f32, pub w: f32, pub h: f32,
    pub fill: Option<[u8; 3]>,
    pub opacity: i32,
}

// UI_FIGMA palette
const BG_SURFACE: Color32 = Color32::from_rgb(0x26, 0x26, 0x27);
const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
const MUTED: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x8a);
const ACCENT: Color32 = Color32::from_rgb(0x0c, 0x8c, 0xe9);
const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2d);
const SOLID_PANEL: Color32 = Color32::from_rgb(0x1c, 0x1b, 0x1b);

pub struct Ui {
    ctx: egui::Context,
    state: egui_winit::State,
    pub frosted: bool,
    pub rects: Vec<egui::Rect>,   // panel rects in physical px (for the frosted-glass pass)
    pub repaint: bool,
    hex: String,                  // persistent editable field (IME-caret gate test)
}

impl Ui {
    pub fn new(window: &Window) -> Self {
        let ctx = egui::Context::default();
        let mut v = egui::Visuals::dark();
        v.window_fill = SOLID_PANEL;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.window_shadow = egui::epaint::Shadow::NONE; // we hand-paint the shadow in the frost pass
        v.window_rounding = Rounding::same(12.0);
        v.override_text_color = Some(TEXT);
        v.widgets.noninteractive.bg_fill = BG_SURFACE;
        ctx.set_visuals(v);
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, window, None, None);
        Ui { ctx, state, frosted: true, rects: vec![], repaint: false, hex: "0C8CE9".into() }
    }

    /// Feed a window event to egui. Returns true if egui consumed it (so the canvas should NOT).
    pub fn on_event(&mut self, window: &Window, ev: &WindowEvent) -> bool {
        self.state.on_window_event(window, ev).consumed
    }
    /// Is the pointer over a panel? (gate #3: canvas strokes must NOT be swallowed by panels)
    pub fn wants_pointer(&self) -> bool { self.ctx.is_pointer_over_area() || self.ctx.wants_pointer_input() }

    /// Build the UI for this frame; returns egui's tessellated output for `Renderer::render_ui`.
    pub fn run(&mut self, window: &Window, data: &PanelData, ppp: f32)
        -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor) {
        let input = self.state.take_egui_input(window);
        let mut frosted = self.frosted;
        let mut hex = std::mem::take(&mut self.hex);
        let mut rects: Vec<egui::Rect> = Vec::new();
        let out = self.ctx.run(input, |ctx| { build_ui(ctx, data, &mut frosted, &mut hex, &mut rects); });
        self.frosted = frosted;
        self.hex = hex;
        self.state.handle_platform_output(window, out.platform_output);
        // rects → physical px for the GPU frost pass
        self.rects = rects.into_iter().map(|r| egui::Rect::from_min_max(
            (r.min.to_vec2() * ppp).to_pos2(), (r.max.to_vec2() * ppp).to_pos2())).collect();
        self.repaint = out.viewport_output.get(&egui::ViewportId::ROOT)
            .map_or(false, |v| v.repaint_delay.is_zero());
        let jobs = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let sz = window.inner_size();
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [sz.width, sz.height], pixels_per_point: out.pixels_per_point };
        (jobs, out.textures_delta, screen)
    }
}

fn num_field(ui: &mut egui::Ui, label: &str, v: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(MUTED).monospace().size(11.0));
        let mut val = v;
        ui.add(egui::DragValue::new(&mut val).speed(1.0).fixed_decimals(0));
    });
}

fn build_ui(ctx: &egui::Context, data: &PanelData, frosted: &mut bool, hex: &mut String, rects: &mut Vec<egui::Rect>) {
    // panel frame: transparent fill when frosted (the GPU frost pass paints the bg behind it);
    // solid dark when not. Rounded, hairline border. Shadow is hand-painted in the frost pass.
    let frame = egui::Frame {
        fill: if *frosted { Color32::TRANSPARENT } else { SOLID_PANEL },
        rounding: Rounding::same(12.0),
        stroke: Stroke::new(1.0, BORDER),
        inner_margin: Margin::same(12.0),
        ..Default::default()
    };

    let r = egui::Window::new("Properties")
        .resizable(false).collapsible(false)
        .default_pos([520.0, 150.0]).default_width(232.0)
        .frame(frame)
        .show(ctx, |ui| {
            ui.label(RichText::new(if data.sel { &data.kind } else { "No selection" }).color(TEXT).size(13.0).strong());
            ui.add_space(8.0);
            ui.label(RichText::new("TRANSFORM").color(MUTED).size(10.0));
            ui.add_space(3.0);
            egui::Grid::new("xf").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                num_field(ui, "X", data.x); num_field(ui, "Y", data.y); ui.end_row();
                num_field(ui, "W", data.w); num_field(ui, "H", data.h); ui.end_row();
            });
            ui.add_space(8.0);
            ui.label(RichText::new("FILL").color(MUTED).size(10.0));
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let c = data.fill.unwrap_or([0x0c, 0x8c, 0xe9]);
                let (rc, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                ui.painter().rect_filled(rc, Rounding::same(4.0), Color32::from_rgb(c[0], c[1], c[2]));
                ui.add(egui::TextEdit::singleline(hex).desired_width(78.0).font(egui::TextStyle::Monospace));
                ui.label(RichText::new(format!("{}%", data.opacity)).color(MUTED).monospace().size(11.0));
            });
            ui.add_space(10.0);
            ui.checkbox(frosted, RichText::new("Frosted glass").color(TEXT).size(12.0));
        });
    if let Some(r) = r { rects.push(r.response.rect); }

    // second panel — z-order test (click a panel to bring it to front)
    let frame2 = egui::Frame { fill: if *frosted { Color32::TRANSPARENT } else { SOLID_PANEL }, ..frame };
    let r2 = egui::Window::new("Swatches")
        .resizable(false).collapsible(false)
        .default_pos([620.0, 320.0]).default_width(180.0)
        .frame(frame2)
        .show(ctx, |ui| {
            ui.label(RichText::new("SWATCHES").color(MUTED).size(10.0));
            ui.add_space(4.0);
            let cols = [0x0c8ce9u32, 0xe6e6e6, 0x141313, 0xff5c5c, 0x3ecf8e, 0xf5a623, 0x9b6cf0, 0xffffff];
            ui.horizontal_wrapped(|ui| {
                for c in cols {
                    let (rc, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                    ui.painter().rect_filled(rc, Rounding::same(4.0), Color32::from_rgb((c>>16) as u8, (c>>8) as u8, c as u8));
                    ui.painter().rect_stroke(rc, Rounding::same(4.0), Stroke::new(1.0, BORDER));
                }
            });
            ui.add_space(6.0);
            if ui.add(egui::Button::new(RichText::new("Apply").color(Color32::WHITE)).fill(ACCENT)).clicked() {}
        });
    if let Some(r2) = r2 { rects.push(r2.response.rect); }
}
