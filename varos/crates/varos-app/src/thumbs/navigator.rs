//! Lane E whole-board proxy in the thumbnail cache. One bounded raster per document revision.
use egui::TextureHandle;
use varos_core::{editor::Editor, geom::View};
#[derive(Clone, Default)]
pub struct NavigatorThumb {
    key: Option<(u64, u64)>,
    texture: Option<TextureHandle>,
    camera: Option<View>,
    document: Option<varos_core::model::Document>,
}
impl NavigatorThumb {
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        ed: &Editor,
        session: u64,
        camera: View,
    ) -> Option<(egui::TextureId, View)> {
        let key = (session, ed.rev);
        // Live gestures mutate document values before rev advances. Compare the published snapshot.
        let same_camera = self.camera.is_some_and(|old| old.pan == camera.pan && old.zoom == camera.zoom);
        if self.key != Some(key) || self.document.as_ref() != Some(&ed.doc) || !same_camera {
            self.key = Some(key);
            self.camera = Some(camera);
            self.document = Some(ed.doc.clone());
            let size = varos_app::shell::tokens::NAVIGATOR_PROXY;
            let raster = super::raster::rasterize_canvas_with_images(&ed.doc, &ed.blobs, size, camera.pan, camera.zoom);
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
        self.texture.as_ref().zip(self.camera).map(|(texture, camera)| (texture.id(), camera))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_artwork_and_artboard_edits_publish_matching_proxy_camera() {
        let ctx = egui::Context::default();
        let mut ed = Editor::new();
        use varos_core::model::{Anchor, Path};
        ed.doc.paths.push(Path::new(
            10,
            vec![
                Anchor { id: 11, p: [10.0, 10.0], hin: None, hout: None, smooth: false },
                Anchor { id: 12, p: [30.0, 10.0], hin: None, hout: None, smooth: false },
                Anchor { id: 13, p: [30.0, 30.0], hin: None, hout: None, smooth: false },
            ],
            true,
            Some([1.0, 0.0, 0.0, 1.0]),
            None,
            1.0,
        ));
        ed.doc.sync_tree();
        ed.doc.artboards.push(varos_core::model::Artboard::default());
        let rev = ed.rev;
        let mut cache = NavigatorThumb::default();
        let camera = |ed: &Editor| {
            varos_core::view_depth::navigator_camera(varos_core::view_depth::navigator_bounds(ed), [224.0, 126.0])
        };
        let mut ids = Vec::new();
        for edit in 0..4 {
            if edit == 1 {
                for a in &mut ed.doc.paths[0].anchors {
                    a.p[0] += 5000.0;
                }
            }
            if edit == 2 {
                ed.doc.artboards[0].w += 7000.0;
            }
            let expected = camera(&ed);
            let out = ctx.run_ui(Default::default(), |_| {
                let (texture, published) = cache.get(&ctx, &ed, 1, expected).expect("proxy");
                assert_eq!(published.pan, expected.pan);
                assert_eq!(published.zoom, expected.zoom);
                ids.push(texture);
            });
            assert_eq!(ed.rev, rev);
            assert_eq!(out.textures_delta.set.is_empty(), edit == 3);
        }
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
        assert_eq!(ids[2], ids[3]);
    }
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
        assert_eq!(ids[0].map(|x| x.0), ids[2].map(|x| x.0));
    }
}
