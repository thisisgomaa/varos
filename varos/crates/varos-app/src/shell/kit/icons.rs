//! THE icon registry (icon stage 1): every registry icon is a real Lucide SVG file under
//! `assets/icons/` (ISC — see the LICENSE beside them, which also carries the Feather MIT notice),
//! embedded with `include_str!`, rasterized once per context at [`t::ICON_RASTER`] through the shared
//! `svg::render_svg` → texture path, white, and tinted at paint time. Never hand-drawn glyphs (owner
//! rule); never Adobe artwork (ICON_LIBRARY_STUDY §6.2).
//!
//! Adding an icon: drop the upstream SVG into `assets/icons/`, add one variant, one `ALL` entry and one
//! `lucide()` arm. The tests check every variant resolves, parses and rasterizes, and that every SVG
//! file in the folder is used.
use super::t;
use egui::{Color32, Context, Id, Painter, Pos2, Rect, TextureHandle, TextureOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    // ── Start / Home / Recent ──
    Home,
    New,
    Open,
    Remove,
    Document,
    More,
    // ── panel actions (icon stage 1) ──
    /// Box header: "Change this panel to…".
    Menu,
    /// A dropdown's disclosure mark.
    ChevronDown,
    /// Artboard panel: Add artboard.
    ArtboardAdd,
    /// Artboard panel: Duplicate artboard.
    Duplicate,
    /// Delete (Layers footer, Artboard panel).
    Trash,
    /// Layers footer: Group the selection.
    Group,
    /// Constrain W/H proportions.
    Link,
    /// Fit in window.
    Fit,
    Portrait,
    Landscape,
    FlipH,
    FlipV,
}
impl Icon {
    pub const ALL: [Icon; 18] = [
        Self::Home,
        Self::New,
        Self::Open,
        Self::Remove,
        Self::Document,
        Self::More,
        Self::Menu,
        Self::ChevronDown,
        Self::ArtboardAdd,
        Self::Duplicate,
        Self::Trash,
        Self::Group,
        Self::Link,
        Self::Fit,
        Self::Portrait,
        Self::Landscape,
        Self::FlipH,
        Self::FlipV,
    ];

    /// The Lucide icon name and its embedded upstream SVG.
    pub fn lucide(self) -> (&'static str, &'static str) {
        macro_rules! svg {
            ($name:literal) => {
                ($name, include_str!(concat!("../../../assets/icons/", $name, ".svg")))
            };
        }
        match self {
            Self::Home => svg!("house"),
            Self::New => svg!("file-plus"),
            Self::Open => svg!("folder-open"),
            Self::Remove => svg!("x"),
            Self::Document => svg!("file"),
            Self::More => svg!("ellipsis"),
            Self::Menu => svg!("menu"),
            Self::ChevronDown => svg!("chevron-down"),
            Self::ArtboardAdd => svg!("square-plus"),
            Self::Duplicate => svg!("copy"),
            Self::Trash => svg!("trash-2"),
            Self::Group => svg!("folder"),
            Self::Link => svg!("link"),
            Self::Fit => svg!("maximize"),
            Self::Portrait => svg!("rectangle-vertical"),
            Self::Landscape => svg!("rectangle-horizontal"),
            Self::FlipH => svg!("square-centerline-dashed-horizontal"),
            Self::FlipV => svg!("square-centerline-dashed-vertical"),
        }
    }

    /// White straight-alpha RGBA at `px` × `px`; `None` only if the embedded SVG is broken.
    pub fn rasterize_at(self, px: u32) -> Option<(Vec<u8>, u32, u32)> {
        let white = self.lucide().1.replace("currentColor", "#ffffff");
        crate::shell::svg::render_svg(&white, px, false)
    }

    /// White straight-alpha RGBA at [`t::ICON_RASTER`] — the one raster every UI icon uses.
    pub fn rasterize(self) -> Option<(Vec<u8>, u32, u32)> {
        self.rasterize_at(t::ICON_RASTER)
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

    /// Paint the glyph centred on `center` at a display size token (`t::ICON_SM/MD/LG`), tinted `color`.
    pub fn paint(self, painter: &Painter, center: Pos2, size: f32, color: Color32) {
        if let Some(tex) = self.texture(painter.ctx()) {
            let uv = Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0));
            painter.image(tex.id(), Rect::from_center_size(center, egui::Vec2::splat(size)), uv, color);
        }
    }
}
