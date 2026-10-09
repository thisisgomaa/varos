//! Lane E whole-board proxy in the thumbnail cache. One bounded raster per document revision.
use egui::TextureHandle;
use varos_core::{editor::Editor, geom::View};
#[derive(Clone, Default)]
pub struct NavigatorThumb {
    key: Option<(u64, u64)>,
    texture: Option<TextureHandle>,
}
impl NavigatorThumb {
    pub fn get(&mut self, ctx: &egui::Context, ed: &Editor, session: u64, camera: View) -> Option<egui::TextureId> {
        let key = (session, ed.rev);
        if self.key != Some(key) {
            self.key = Some(key);
            let size = varos_app::shell::tokens::NAVIGATOR_PROXY;
            let raster = super::raster::rasterize_canvas(&ed.doc, size, camera.pan, camera.zoom);
            self.texture = if raster.errors.is_empty() {
                let image = egui::ColorImage::from_rgba_premultiplied(
                    [raster.width as usize, raster.height as usize],
                    &raster.pixels,
                );
                Some(ctx.load_texture("navigator-board", image, egui::TextureOptions::LINEAR))
            } else {
                None
            };
        }
        self.texture.as_ref().map(TextureHandle::id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_proxy_does_not_request_repaint() {
        let ctx = egui::Context::default();
        let ed = Editor::new();
        let mut cache = NavigatorThumb::default();
        let camera =
            varos_core::view_depth::navigator_camera(varos_core::view_depth::navigator_bounds(&ed), [224.0, 126.0]);
        let mut ids = vec![];
        for _ in 0..6 {
            let out = ctx.run_ui(Default::default(), |_| {
                ids.push(cache.get(&ctx, &ed, 1, camera));
            });
            if ids.len() == 6 {
                assert!(out.textures_delta.set.is_empty());
                let now = std::time::Instant::now();
                let next =
                    out.viewport_output.get(&egui::ViewportId::ROOT).and_then(|v| now.checked_add(v.repaint_delay));
                assert_eq!(
                    crate::pacing::plan(now, next, &[], false),
                    crate::pacing::Plan { redraw: false, flow: crate::pacing::Flow::Wait }
                );
            }
        }
        assert_eq!(ids[0], ids[2]);
    }
}
