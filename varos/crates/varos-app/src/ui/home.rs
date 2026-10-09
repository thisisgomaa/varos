//! Existing Home renderer moved for the ui.rs line cap.
use super::*;
impl Ui {
    pub(super) fn run_home(
        &mut self,
        window: &Window,
        maximized: bool,
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor) {
        let raw = self.state.egui_input_mut();
        raw.focused = egui_focus_seed(window.has_focus(), raw.focused);
        let input = self.state.take_egui_input(window);
        self.start_page.recovery_status.clone_from(&self.recovery.footer);
        self.export_sheet = None; // Home has no document to export
        let out = self.ctx.run_ui(input, |root| {
            build_home_frame(
                root,
                &self.top,
                &mut self.shell,
                &mut self.win_action,
                &self.doc_tabs,
                &mut self.app_cmds,
                &mut self.show_rail,
                &mut self.show_dock,
                &mut self.start_page,
                &mut self.start_model,
                self.recent_warning.as_deref(),
                maximized,
            );
            self.phase9.draw(root.ctx(), &mut self.app_cmds, None);
            // Lane C: New Document / export sheets are reachable from Home too (integration w2)
            lane_c::sheets(root.ctx(), &mut self.app_cmds, &mut Vec::new(), None);
        });

        // K3: Home draws no document field; any edit left open (an invalid one a non-user command
        // passed) is closed here — there is no document to commit into
        let _ = kit::field::end_frame(&self.ctx);
        self.field_pending = None;
        self.board_hole = None;
        self.board_px = None;
        self.cursor = out.platform_output.cursor_icon;
        #[cfg(target_os = "macos")]
        let out = {
            let mut out = out;
            out.platform_output.cursor_icon = egui::CursorIcon::Default;
            out
        };
        self.state.handle_platform_output(window, out.platform_output);
        self.repaint_at =
            out.viewport_output.get(&egui::ViewportId::ROOT).and_then(|v| Instant::now().checked_add(v.repaint_delay));
        let jobs = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let size = window.inner_size();
        (
            jobs,
            out.textures_delta,
            egui_wgpu::ScreenDescriptor {
                size_in_pixels: [size.width, size.height],
                pixels_per_point: out.pixels_per_point,
            },
        )
    }
}
