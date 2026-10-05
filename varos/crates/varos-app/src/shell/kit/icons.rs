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

pub const LEGACY_SELECT: &str = r#"<path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"/>"#;
pub const LEGACY_DIRECT: &str = r#"<path d="M12.586 12.586 19 19"/><path d="M3.688 3.037a.497.497 0 0 0-.651.651l6.5 15.999a.501.501 0 0 0 .947-.062l1.569-6.083a2 2 0 0 1 1.448-1.479l6.124-1.579a.5.5 0 0 0 .063-.947z"/>"#;
pub const LEGACY_PEN: &str = r#"<path d="M15.707 21.293a1 1 0 0 1-1.414 0l-1.586-1.586a1 1 0 0 1 0-1.414l5.586-5.586a1 1 0 0 1 1.414 0l1.586 1.586a1 1 0 0 1 0 1.414z"/><path d="m18 13-1.375-6.874a1 1 0 0 0-.746-.776L3.235 2.028a1 1 0 0 0-1.207 1.207L5.35 15.879a1 1 0 0 0 .776.746L13 18"/><path d="m2.3 2.3 7.286 7.286"/><circle cx="11" cy="11" r="2"/>"#;
pub const LEGACY_RECT: &str = r#"<rect width="18" height="18" x="3" y="3" rx="2"/>"#;
pub const LEGACY_ELLIPSE: &str = r#"<circle cx="12" cy="12" r="10"/>"#;
pub const LEGACY_TRIANGLE: &str = r#"<path d="M13.73 4a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/>"#;
pub const LEGACY_EYE: &str = r#"<path d="m12 9-8.414 8.414A2 2 0 0 0 3 18.828v1.344a2 2 0 0 1-.586 1.414A2 2 0 0 1 3.828 21h1.344a2 2 0 0 0 1.414-.586L15 12"/><path d="m18 9 .4.4a1 1 0 1 1-3 3l-3.8-3.8a1 1 0 1 1 3-3l.4.4 3.4-3.4a1 1 0 1 1 3 3z"/><path d="m2 22 .414-.414"/>"#;
// Layers-panel icons (Lucide): eye / eye-off · lock / lock-open · new-layer + · new-sublayer · trash
pub const LEGACY_L_EYE: &str = r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"#;
pub const LEGACY_L_EYEOFF: &str = r#"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>"#;
pub const LEGACY_L_LOCK: &str =
    r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"#;
pub const LEGACY_L_UNLOCK: &str =
    r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/>"#;
pub const LEGACY_L_SEARCH: &str = r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#;

// field-label icons (Illustrator-style, gray): rotation · opacity · stroke weight
pub const LEGACY_ROTATE: &str =
    r#"<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>"#;
// transform-tool rail icon: scale (move-diagonal)
pub const LEGACY_SCALE: &str = r#"<path d="M19 13v6h-6"/><path d="M5 11V5h6"/><path d="m5 5 14 14"/>"#;
pub const LEGACY_OPACITY: &str =
    r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a10 10 0 0 1 0 20z" fill="white" stroke="none"/>"#;
pub const LEGACY_STROKEW: &str = r#"<path d="M3 7h18" stroke-width="1.3"/><path d="M3 12h18" stroke-width="2.4"/><path d="M3 17h18" stroke-width="3.8"/>"#;
// object-alignment icons: align L / centre-H / R · T / middle / B, then distribute H / V.
// FILLED glyphs — law-verbatim from UI_VISION_MOCKUP.html:176–181 (a solid guide bar + two solid bars);
// loaded filled (white fill, no stroke) so they read bold at 16px like the mockup.
pub const LEGACY_AL_L: &str = r#"<path d="M4 3.5h1.6v17H4z"/><rect x="7.5" y="6.5" width="9" height="4"/><rect x="7.5" y="13.5" width="13" height="4"/>"#;
pub const LEGACY_AL_CH: &str = r#"<path d="M11.2 3.5h1.6v17h-1.6z"/><rect x="7" y="6.5" width="10" height="4"/><rect x="4.5" y="13.5" width="15" height="4"/>"#;
pub const LEGACY_AL_R: &str = r#"<path d="M18.4 3.5H20v17h-1.6z"/><rect x="7.5" y="6.5" width="9" height="4"/><rect x="3.5" y="13.5" width="13" height="4"/>"#;
pub const LEGACY_AL_T: &str = r#"<path d="M3.5 4h17v1.6h-17z"/><rect x="6.5" y="7.5" width="4" height="9"/><rect x="13.5" y="7.5" width="4" height="13"/>"#;
pub const LEGACY_AL_M: &str = r#"<path d="M3.5 11.2h17v1.6h-17z"/><rect x="6.5" y="7" width="4" height="10"/><rect x="13.5" y="4.5" width="4" height="15"/>"#;
pub const LEGACY_AL_B: &str = r#"<path d="M3.5 18.4h17V20h-17z"/><rect x="6.5" y="7.5" width="4" height="9"/><rect x="13.5" y="3.5" width="4" height="13"/>"#;
// FILLED distribute glyphs — same language (three solid bars; no rx so they read as bars, not pills)
pub const LEGACY_DIST_H: &str = r#"<rect x="3" y="6" width="3" height="12"/><rect x="10.5" y="6" width="3" height="12"/><rect x="18" y="6" width="3" height="12"/>"#;
pub const LEGACY_DIST_V: &str = r#"<rect x="6" y="3" width="12" height="3"/><rect x="6" y="10.5" width="12" height="3"/><rect x="6" y="18" width="12" height="3"/>"#;
// top-bar icons: menu (☰). Window min/max/close are painted directly in `winctl` (crisp Win11 glyphs).
pub const LEGACY_MENU: &str = r#"<path d="M4 12h16"/><path d="M4 6h16"/><path d="M4 18h16"/>"#;
// (4b: the band's search / plus / × / magnet glyphs now come from the kit registry, `Icon`)
// Artboard tool (Lucide "frame" — a bold # that reads clearly at 20px) · hexagon (polygon shape) ·
// portrait/landscape page · "fit in window" frame
pub const LEGACY_ARTBOARD: &str = r#"<path d="M22 6H2"/><path d="M22 18H2"/><path d="M6 2v20"/><path d="M18 2v20"/>"#;
pub const LEGACY_POLYGON: &str = r#"<path d="M21 16.05V7.95a2 2 0 0 0-1-1.73l-7-4.04a2 2 0 0 0-2 0l-7 4.04A2 2 0 0 0 3 7.95v8.1a2 2 0 0 0 1 1.73l7 4.04a2 2 0 0 0 2 0l7-4.04a2 2 0 0 0 1-1.73Z"/>"#;
pub const LEGACY_FIT: &str = r#"<path d="M8 3H5a2 2 0 0 0-2 2v3"/><path d="M21 8V5a2 2 0 0 0-2-2h-3"/><path d="M3 16v3a2 2 0 0 0 2 2h3"/><path d="M16 21h3a2 2 0 0 0 2-2v-3"/>"#;
pub fn legacy_svg(inner: &str, filled: bool) -> String {
    if filled {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"#ffffff\" stroke=\"none\">{inner}</svg>")
    } else {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"#ffffff\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{inner}</svg>")
    }
}

pub fn legacy_texture(ctx: &Context, name: &str, inner: &str, filled: bool) -> Option<TextureHandle> {
    let id = Id::new(("varos-legacy-icon", name, filled));
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(tex);
    }
    let svg = legacy_svg(inner, filled);
    let (rgba, w, h) = crate::shell::svg::render_svg(&svg, t::ICON_RASTER, false)?;
    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    let tex = ctx.load_texture(name, image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

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
    // ── Start v2 — Boards (L4) ──
    /// Recent boards: grid view.
    Grid,
    /// Recent boards: list view.
    List,
    /// The Recovered band.
    History,
    /// The status line's "Recovery on".
    Shield,
    /// A Missing board's well ("File not found").
    FileQuestion,
    /// The Custom… preset.
    Plus,
    /// The top bar's "Search boards".
    Search,
}
impl Icon {
    pub const ALL: [Icon; 25] = [
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
        Self::Grid,
        Self::List,
        Self::History,
        Self::Shield,
        Self::FileQuestion,
        Self::Plus,
        Self::Search,
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
            Self::Grid => svg!("layout-grid"),
            Self::List => svg!("list"),
            Self::History => svg!("history"),
            Self::Shield => svg!("shield-check"),
            Self::FileQuestion => svg!("file-question-mark"),
            Self::Plus => svg!("plus"),
            Self::Search => svg!("search"),
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
