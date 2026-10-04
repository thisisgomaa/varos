//! Kit icons are real Lucide SVGs (`assets/icons/`, ISC — see the LICENSE beside them), rasterized
//! once per context through the shared `svg::render_svg` → texture path the toolbar uses, white,
//! and tinted at paint time. Never hand-drawn glyphs (owner rule).
use super::t;
use egui::{Color32, Context, Id, Painter, Pos2, Rect, TextureHandle, TextureOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Home,
    New,
    Open,
    Remove,
    Document,
    More,
}
impl Icon {
    pub const ALL: [Icon; 6] = [Self::Home, Self::New, Self::Open, Self::Remove, Self::Document, Self::More];

    /// The Lucide icon name and its embedded upstream SVG.
    pub fn lucide(self) -> (&'static str, &'static str) {
        match self {
            Self::Home => ("house", include_str!("../../../assets/icons/house.svg")),
            Self::New => ("file-plus", include_str!("../../../assets/icons/file-plus.svg")),
            Self::Open => ("folder-open", include_str!("../../../assets/icons/folder-open.svg")),
            Self::Remove => ("x", include_str!("../../../assets/icons/x.svg")),
            Self::Document => ("file", include_str!("../../../assets/icons/file.svg")),
            Self::More => ("ellipsis", include_str!("../../../assets/icons/ellipsis.svg")),
        }
    }

    /// White straight-alpha RGBA at [`t::ICON_RASTER`]; `None` only if the embedded SVG is broken.
    pub fn rasterize(self) -> Option<(Vec<u8>, u32, u32)> {
        let white = self.lucide().1.replace("currentColor", "#ffffff");
        crate::shell::svg::render_svg(&white, t::ICON_RASTER, false)
    }

    /// The icon's texture, uploaded on first use and cached in the context.
    pub fn texture(self, ctx: &Context) -> Option<TextureHandle> {
        let id = Id::new(("varos-kit-icon", self));
        if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
            return Some(tex);
        }
        let (rgba, w, h) = self.rasterize()?;
        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let tex = ctx.load_texture(format!("kit-{}", self.lucide().0), image, TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
        Some(tex)
    }

    pub(super) fn paint(self, painter: &Painter, center: Pos2, color: Color32) {
        if let Some(tex) = self.texture(painter.ctx()) {
            let uv = Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0));
            painter.image(tex.id(), Rect::from_center_size(center, egui::vec2(t::KIT_ICON, t::KIT_ICON)), uv, color);
        }
    }
}
