//! Lane E artboards-only screen mode. No shell state is mutated on enter/exit.
use super::*;
impl Ui {
    pub(super) fn run_presentation(
        &mut self,
        window: &Window,
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor) {
        let input = self.state.take_egui_input(window);
        let out = self.ctx.run_ui(input, |_| {});
        self.state.handle_platform_output(window, out.platform_output);
        let r = self.ctx.content_rect();
        self.board_hole = Some(r);
        self.board_px = Some(egui::Rect::from_min_max(
            (r.min.to_vec2() * out.pixels_per_point).to_pos2(),
            (r.max.to_vec2() * out.pixels_per_point).to_pos2(),
        ));
        self.repaint_at =
            out.viewport_output.get(&egui::ViewportId::ROOT).and_then(|v| Instant::now().checked_add(v.repaint_delay));
        let jobs = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let s = window.inner_size();
        (
            jobs,
            out.textures_delta,
            egui_wgpu::ScreenDescriptor { size_in_pixels: [s.width, s.height], pixels_per_point: out.pixels_per_point },
        )
    }
}
