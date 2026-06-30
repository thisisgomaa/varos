//! Native GPU UI — hand-painted chrome on OUR wgpu surface via `Renderer::render_ui` (egui shares our
//! Device/Queue; no second window). egui is only canvas + input + layout; every widget is drawn by us
//! so it matches the Figma, not egui's dev-tool defaults. Pieces so far: the left TOOL RAIL and the
//! right INSPECTOR DOCK (Transform / Appearance / Fill / Stroke). Solid panels, one light GPU shadow,
//! no glass. Panels read a per-frame snapshot of the editor and push deferred `Op`s, applied to
//! `&mut Editor` after layout (no IPC, no borrow fights). varos-core itself is untouched.

use std::time::Instant;
use egui::{Align, Align2, Color32, FontId, Layout, Margin, RichText, Rounding, Stroke};
use varos_core::editor::{AlignMode, DistAxis, Editor, PaintTarget, ToolKind};
use varos_core::geom::{Rgba, View};
use winit::event::WindowEvent;
use winit::window::Window;

// UI_FIGMA palette (§1)
const SOLID_PANEL: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x22);
const BG: Color32          = Color32::from_rgb(0x14, 0x13, 0x13); // app background / top bar
const CLOSE_RED: Color32   = Color32::from_rgb(0xe8, 0x11, 0x23); // window close hover // panel body
const BG_SURFACE: Color32  = Color32::from_rgb(0x26, 0x26, 0x27); // fields
const BORDER: Color32      = Color32::from_rgb(0x2a, 0x2a, 0x2d); // hairline
const BORDER_2: Color32    = Color32::from_rgb(0x3a, 0x3b, 0x3d); // hover/focus border
const HOVER: Color32       = Color32::from_rgb(0x2e, 0x2e, 0x31); // hover bg
const ACCENT: Color32      = Color32::from_rgb(0x0c, 0x8c, 0xe9); // active tool
const TEXT: Color32        = Color32::from_rgb(0xe6, 0xe6, 0xe6); // primary text
const MUTED: Color32       = Color32::from_rgb(0x8a, 0x8a, 0x8a); // labels
const FAINT: Color32       = Color32::from_rgb(0x7c, 0x7c, 0x7c); // field labels / faint

// Lucide icon path data (white-stroked at render time), same set as the web rail.
const IC_SELECT: &str = r#"<path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"/>"#;
const IC_DIRECT: &str = r#"<path d="M12.586 12.586 19 19"/><path d="M3.688 3.037a.497.497 0 0 0-.651.651l6.5 15.999a.501.501 0 0 0 .947-.062l1.569-6.083a2 2 0 0 1 1.448-1.479l6.124-1.579a.5.5 0 0 0 .063-.947z"/>"#;
const IC_PEN: &str = r#"<path d="M15.707 21.293a1 1 0 0 1-1.414 0l-1.586-1.586a1 1 0 0 1 0-1.414l5.586-5.586a1 1 0 0 1 1.414 0l1.586 1.586a1 1 0 0 1 0 1.414z"/><path d="m18 13-1.375-6.874a1 1 0 0 0-.746-.776L3.235 2.028a1 1 0 0 0-1.207 1.207L5.35 15.879a1 1 0 0 0 .776.746L13 18"/><path d="m2.3 2.3 7.286 7.286"/><circle cx="11" cy="11" r="2"/>"#;
const IC_RECT: &str = r#"<rect width="18" height="18" x="3" y="3" rx="2"/>"#;
const IC_ELLIPSE: &str = r#"<circle cx="12" cy="12" r="10"/>"#;
const IC_TRIANGLE: &str = r#"<path d="M13.73 4a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/>"#;
const IC_EYE: &str = r#"<path d="m12 9-8.414 8.414A2 2 0 0 0 3 18.828v1.344a2 2 0 0 1-.586 1.414A2 2 0 0 1 3.828 21h1.344a2 2 0 0 0 1.414-.586L15 12"/><path d="m18 9 .4.4a1 1 0 1 1-3 3l-3.8-3.8a1 1 0 1 1 3-3l.4.4 3.4-3.4a1 1 0 1 1 3 3z"/><path d="m2 22 .414-.414"/>"#;
// field-label icons (Illustrator-style, gray): rotation · opacity · stroke weight
const IC_ROTATE: &str = r#"<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>"#;
const IC_OPACITY: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a10 10 0 0 1 0 20z" fill="white" stroke="none"/>"#;
const IC_STROKEW: &str = r#"<path d="M3 7h18" stroke-width="1.3"/><path d="M3 12h18" stroke-width="2.4"/><path d="M3 17h18" stroke-width="3.8"/>"#;
// transform-row icons: constrain (link) · flip horizontal · flip vertical
const IC_LINK: &str = r#"<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>"#;
const IC_FLIPH: &str = r#"<path d="M8 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h3"/><path d="M16 3h3a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-3"/><path d="M12 20v2"/><path d="M12 14v2"/><path d="M12 8v2"/><path d="M12 2v2"/>"#;
const IC_FLIPV: &str = r#"<path d="M21 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v3"/><path d="M21 16v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-3"/><path d="M4 12H2"/><path d="M10 12H8"/><path d="M16 12h-2"/><path d="M22 12h-2"/>"#;
// object-alignment icons: align L / centre-H / R · T / middle / B, then distribute H / V
const IC_AL_L: &str = r#"<line x1="3" y1="4" x2="3" y2="20"/><rect x="3" y="6" width="14" height="4" rx="1"/><rect x="3" y="14" width="9" height="4" rx="1"/>"#;
const IC_AL_CH: &str = r#"<line x1="12" y1="4" x2="12" y2="20"/><rect x="5" y="6" width="14" height="4" rx="1"/><rect x="7.5" y="14" width="9" height="4" rx="1"/>"#;
const IC_AL_R: &str = r#"<line x1="21" y1="4" x2="21" y2="20"/><rect x="7" y="6" width="14" height="4" rx="1"/><rect x="12" y="14" width="9" height="4" rx="1"/>"#;
const IC_AL_T: &str = r#"<line x1="4" y1="3" x2="20" y2="3"/><rect x="6" y="3" width="4" height="14" rx="1"/><rect x="14" y="3" width="4" height="9" rx="1"/>"#;
const IC_AL_M: &str = r#"<line x1="4" y1="12" x2="20" y2="12"/><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="7.5" width="4" height="9" rx="1"/>"#;
const IC_AL_B: &str = r#"<line x1="4" y1="21" x2="20" y2="21"/><rect x="6" y="7" width="4" height="14" rx="1"/><rect x="14" y="12" width="4" height="9" rx="1"/>"#;
const IC_DIST_H: &str = r#"<rect x="3" y="6" width="3" height="12" rx="1"/><rect x="10.5" y="6" width="3" height="12" rx="1"/><rect x="18" y="6" width="3" height="12" rx="1"/>"#;
const IC_DIST_V: &str = r#"<rect x="6" y="3" width="12" height="3" rx="1"/><rect x="6" y="10.5" width="12" height="3" rx="1"/><rect x="6" y="18" width="12" height="3" rx="1"/>"#;
// top-bar icons: menu (☰). Window min/max/close are painted directly in `winctl` (crisp Win11 glyphs).
const IC_MENU: &str = r#"<path d="M4 12h16"/><path d="M4 6h16"/><path d="M4 18h16"/>"#;
// top-bar content icons: search · layout · panels checklist · new-tab · tab-close · check
const IC_SEARCH: &str = r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#;
const IC_LAYOUT: &str = r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 9h18"/><path d="M9 21V9"/>"#;
const IC_PANELS: &str = r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M15 3v18"/>"#;
const IC_PLUS: &str = r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#;
const IC_X: &str = r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
const IC_CHECK: &str = r#"<path d="M20 6 9 17l-5-5"/>"#;
const IC_MAGNET: &str = r#"<path d="m6 15-4-4 6.75-6.77a7.79 7.79 0 0 1 11 11L13 22l-4-4 6.39-6.36a2.14 2.14 0 0 0-3-3L6 15"/><path d="m5 8 4 4"/><path d="m12 15 4 4"/>"#;
// Artboard tool (Lucide "frame" — a bold # that reads clearly at 20px) · hexagon (polygon shape) ·
// portrait/landscape page · "fit in window" frame
const IC_ARTBOARD: &str = r#"<path d="M22 6H2"/><path d="M22 18H2"/><path d="M6 2v20"/><path d="M18 2v20"/>"#;
const IC_POLYGON: &str = r#"<path d="M21 16.05V7.95a2 2 0 0 0-1-1.73l-7-4.04a2 2 0 0 0-2 0l-7 4.04A2 2 0 0 0 3 7.95v8.1a2 2 0 0 0 1 1.73l7 4.04a2 2 0 0 0 2 0l7-4.04a2 2 0 0 0 1-1.73Z"/>"#;
const IC_PORTRAIT: &str = r#"<rect x="7" y="3" width="10" height="18" rx="1"/>"#;
const IC_LANDSCAPE: &str = r#"<rect x="3" y="7" width="18" height="10" rx="1"/>"#;
const IC_FIT: &str = r#"<path d="M8 3H5a2 2 0 0 0-2 2v3"/><path d="M21 8V5a2 2 0 0 0-2-2h-3"/><path d="M3 16v3a2 2 0 0 0 2 2h3"/><path d="M16 21h3a2 2 0 0 0 2-2v-3"/>"#;
// ⋮ on-canvas artboard menu — FILLED dots (own svg; lucide() forces stroke-only)
const IC_DOTS_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><circle cx="12" cy="5" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="12" cy="19" r="2"/></svg>"##;
/// Page-size presets shown in the artboard panel: (label, w, h) in world points (px == pt @72ppi).
const AB_PRESETS: [(&str, f32, f32); 5] = [
    ("Square \u{2014} 1080", 1080.0, 1080.0),
    ("Screen \u{2014} 1920\u{00d7}1080", 1920.0, 1080.0),
    ("A4 \u{2014} 595\u{00d7}842", 595.0, 842.0),
    ("Letter \u{2014} 612\u{00d7}792", 612.0, 792.0),
    ("Story \u{2014} 1080\u{00d7}1920", 1080.0, 1920.0),
];

/// A change requested by a panel this frame; applied to the editor after layout.
enum Op {
    Tool(ToolKind),
    SetBBox(Option<f32>, Option<f32>, Option<f32>, Option<f32>, f32, f32), // nx,ny,nw,nh + ref ax,ay
    SetRot(f32), SetOpacity(f32), SetStrokeW(f32),
    Paint(PaintTarget, Option<Rgba>),
    Flip(bool),
    Align(AlignMode), Distribute(DistAxis),
    // ---- artboard ops (i = artboard index) ----
    AbActive(usize),
    AbRect(usize, Option<f32>, Option<f32>, Option<f32>, Option<f32>), // x,y,w,h (each optional)
    AbName(usize, String),
    AbColor(usize, Option<Rgba>),   // None = transparent page
    AbClip(usize),                  // toggle
    AbOrient(usize),                // swap w/h
    AbAdd, AbDup(usize), AbDel(usize), AbCount(usize),
    AbMoveArt(bool),
    RulerOrigin(Option<varos_core::geom::Pt>), // Some = set zero-point (snapped) + show crosshair; None = end drag
}

/// A window action the custom title bar asks the host (winit) to perform.
pub enum WinAction { Minimize, ToggleMaximize, Close }

struct TopIcons {
    menu: Option<egui::TextureHandle>,   // min/max/close are painted glyphs now (see `winctl`), not textures
    search: Option<egui::TextureHandle>, layout: Option<egui::TextureHandle>, panels: Option<egui::TextureHandle>,
    plus: Option<egui::TextureHandle>, x: Option<egui::TextureHandle>, check: Option<egui::TextureHandle>,
    magnet: Option<egui::TextureHandle>,
}

/// Read-only snapshot of the editor for this frame's panels.
struct Snap {
    tool: ToolKind,
    name: String, sel: bool,
    x: f32, y: f32, w: f32, h: f32, rot: f32,
    fill: Option<Rgba>, stroke: Option<Rgba>, sw: f32, opacity: f32,
}
impl Snap {
    fn read(ed: &Editor) -> Self {
        let n = ed.objsel.len();
        let (sel, x, y, w, h) = match ed.obj_bbox() {
            Some((x0, y0, x1, y1)) if n > 0 => (true, x0, y0, x1 - x0, y1 - y0),
            _ => (false, 0.0, 0.0, 0.0, 0.0),
        };
        let first = ed.objsel.iter().copied().filter_map(|p| ed.doc.pidx(p)).next();
        let (fill, stroke, sw, opacity) = match first {
            Some(pi) => { let p = &ed.doc.paths[pi]; (p.fill, p.stroke, p.stroke_width, p.opacity) }
            None => (ed.cur_fill, ed.cur_stroke, ed.cur_sw, 1.0),
        };
        let name = if n == 0 { "No selection".into() }
            else if n == 1 { first.and_then(|pi| ed.doc.paths[pi].name.clone()).unwrap_or_else(|| "Path".into()) }
            else { format!("{n} objects") };
        Snap { tool: ed.tool, name, sel, x, y, w, h, rot: ed.obj_angle.to_degrees(), fill, stroke, sw, opacity }
    }
}

/// Read-only snapshot of the ACTIVE artboard for the artboard property panel.
struct AbSnap {
    count: usize, active: usize,
    name: String, x: f32, y: f32, w: f32, h: f32,
    color: Option<Rgba>, clip: bool, move_art: bool,
}
impl AbSnap {
    fn read(ed: &Editor) -> Self {
        let count = ed.doc.artboards.len();
        let active = if count == 0 { 0 } else { ed.doc.active.min(count - 1) };
        let ab = ed.doc.active_artboard();
        AbSnap {
            count, active,
            name: ab.map(|a| a.name.clone()).unwrap_or_default(),
            x: ab.map(|a| a.x).unwrap_or(0.0), y: ab.map(|a| a.y).unwrap_or(0.0),
            w: ab.map(|a| a.w).unwrap_or(0.0), h: ab.map(|a| a.h).unwrap_or(0.0),
            color: ab.and_then(|a| a.page_color), clip: ab.map(|a| a.clip).unwrap_or(false),
            move_art: ed.doc.move_art_with_ab,
        }
    }
}

/// One artboard's on-canvas label info (for the name + ⋮ chrome painted over the board).
struct AbInfo { i: usize, name: String, x: f32, y: f32, w: f32, h: f32, transparent: bool, clip: bool }
fn ab_infos(ed: &Editor) -> Vec<AbInfo> {
    ed.doc.artboards.iter().enumerate().map(|(i, a)| AbInfo {
        i, name: a.name.clone(), x: a.x, y: a.y, w: a.w, h: a.h,
        transparent: a.page_color.is_none(), clip: a.clip,
    }).collect()
}

struct ToolBtn { kind: ToolKind, tip: &'static str, tex: Option<egui::TextureHandle>, group_end: bool }

pub struct Ui {
    ctx: egui::Context,
    state: egui_winit::State,
    pub frosted: bool,          // kept for the renderer signature; false (no glass)
    pub rects: Vec<egui::Rect>, // panel rects in physical px (for the GPU shadow pass)
    pub repaint: bool,
    tools: Vec<ToolBtn>,        // rail singletons: Object · Direct · Artboard · Pen · Eyedropper
    shapes: Vec<ToolBtn>,       // the shape tools, collapsed into one rail slot (right-click → flyout)
    shape_active: ToolKind,     // which shape the shapes slot currently represents
    ic_rotate: Option<egui::TextureHandle>,
    ic_opacity: Option<egui::TextureHandle>,
    ic_strokew: Option<egui::TextureHandle>,
    ic_link: Option<egui::TextureHandle>,
    ic_fliph: Option<egui::TextureHandle>,
    ic_flipv: Option<egui::TextureHandle>,
    ic_portrait: Option<egui::TextureHandle>,
    ic_landscape: Option<egui::TextureHandle>,
    ic_fit: Option<egui::TextureHandle>,
    ab_dots: Option<egui::TextureHandle>,
    align_icons: [Option<egui::TextureHandle>; 8], // align L/CH/R · T/M/B · distribute H/V
    cursor: egui::CursorIcon, // this frame's egui cursor (read from FullOutput, not post-frame state)
    refpt: (f32, f32),        // transform reference point (ax, ay each in {0, .5, 1})
    lock: bool,               // constrain W/H proportions
    ab_lock: bool,            // constrain artboard W/H proportions
    ab_name_edit: Option<(usize, String)>, // inline rename in progress (artboard index + buffer)
    pub fit_request: Option<usize>,        // an artboard asked to be fit in the window (host applies it)
    top: TopIcons,
    pub win_action: Option<WinAction>, // a window control was clicked this frame (host acts on it)
    show_rail: bool,
    show_dock: bool,
    tabs: Vec<String>,
    tab_active: usize,
    logo: Option<egui::TextureHandle>,
    splash_start: Option<Instant>, // startup loading screen; None once it has faded out
    last_splash: bool,             // did this frame draw the splash (host renders it transparent)?
}

/// Rasterize a Lucide icon (white) to an egui texture once.
fn load_icon(ctx: &egui::Context, name: &str, svg_inner: &str) -> Option<egui::TextureHandle> {
    crate::cursors::render_svg(&lucide(svg_inner), 96, false).map(|(rgba, w, h)| {
        ctx.load_texture(name, egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba),
            egui::TextureOptions::LINEAR)
    })
}

impl Ui {
    pub fn new(window: &Window) -> Self {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        install_style(&ctx);
        // rail singletons — Artboard sits with Selection + Direct Selection (Ahmed), then Pen, Eyedropper.
        let defs: [(ToolKind, &str, &str, bool); 5] = [
            (ToolKind::Object,     IC_SELECT,   "Selection (V)",        false),
            (ToolKind::Direct,     IC_DIRECT,   "Direct Selection (A)", false),
            (ToolKind::Artboard,   IC_ARTBOARD, "Artboard (Shift+O)",   true),  // ends the selection group
            (ToolKind::Pen,        IC_PEN,      "Pen (P)",              true),  // ends the pen group
            (ToolKind::Eyedropper, IC_EYE,      "Eyedropper (I)",       false),
        ];
        let tools = defs.iter().enumerate().map(|(i, (kind, svg, tip, grp))| {
            ToolBtn { kind: *kind, tip, tex: load_icon(&ctx, &format!("ic-{i}"), svg), group_end: *grp }
        }).collect();
        // shape tools collapse into ONE rail slot: left-click uses the current shape, right-click flyouts all four.
        let shape_defs: [(ToolKind, &str, &str); 4] = [
            (ToolKind::Rect,     IC_RECT,     "Rectangle (M)"),
            (ToolKind::Ellipse,  IC_ELLIPSE,  "Ellipse (L)"),
            (ToolKind::Triangle, IC_TRIANGLE, "Triangle"),
            (ToolKind::Polygon,  IC_POLYGON,  "Polygon"),
        ];
        let shapes = shape_defs.iter().enumerate().map(|(i, (kind, svg, tip))| {
            ToolBtn { kind: *kind, tip, tex: load_icon(&ctx, &format!("ic-shape{i}"), svg), group_end: false }
        }).collect();
        let ic_rotate = load_icon(&ctx, "lbl-rot", IC_ROTATE);
        let ic_opacity = load_icon(&ctx, "lbl-op", IC_OPACITY);
        let ic_strokew = load_icon(&ctx, "lbl-sw", IC_STROKEW);
        let ic_link = load_icon(&ctx, "lbl-link", IC_LINK);
        let ic_fliph = load_icon(&ctx, "lbl-fh", IC_FLIPH);
        let ic_flipv = load_icon(&ctx, "lbl-fv", IC_FLIPV);
        let ic_portrait = load_icon(&ctx, "lbl-portrait", IC_PORTRAIT);
        let ic_landscape = load_icon(&ctx, "lbl-landscape", IC_LANDSCAPE);
        let ic_fit = load_icon(&ctx, "lbl-fit", IC_FIT);
        let ab_dots = crate::cursors::render_svg(IC_DOTS_SVG, 96, false).map(|(rgba, w, h)| {
            ctx.load_texture("ab-dots", egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba), egui::TextureOptions::LINEAR)
        });
        let align_icons = [
            load_icon(&ctx, "al-l", IC_AL_L), load_icon(&ctx, "al-ch", IC_AL_CH), load_icon(&ctx, "al-r", IC_AL_R),
            load_icon(&ctx, "al-t", IC_AL_T), load_icon(&ctx, "al-m", IC_AL_M), load_icon(&ctx, "al-b", IC_AL_B),
            load_icon(&ctx, "dist-h", IC_DIST_H), load_icon(&ctx, "dist-v", IC_DIST_V),
        ];
        let top = TopIcons {
            menu: load_icon(&ctx, "tb-menu", IC_MENU),
            search: load_icon(&ctx, "tb-search", IC_SEARCH),
            layout: load_icon(&ctx, "tb-layout", IC_LAYOUT),
            panels: load_icon(&ctx, "tb-panels", IC_PANELS),
            plus: load_icon(&ctx, "tb-plus", IC_PLUS),
            x: load_icon(&ctx, "tb-x", IC_X),
            check: load_icon(&ctx, "tb-check", IC_CHECK),
            magnet: load_icon(&ctx, "tb-magnet", IC_MAGNET),
        };
        let logo = image::load_from_memory(include_bytes!("../icon.png")).ok().map(|im| {
            let rgba = im.into_rgba8(); let (w, h) = rgba.dimensions();
            ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw()), egui::TextureOptions::LINEAR)
        });
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, window, None, None);
        Ui { ctx, state, frosted: false, rects: vec![], repaint: false, tools, shapes, shape_active: ToolKind::Rect, ic_rotate, ic_opacity, ic_strokew,
             ic_link, ic_fliph, ic_flipv, ic_portrait, ic_landscape, ic_fit, ab_dots, align_icons,
             cursor: egui::CursorIcon::Default, refpt: (0.0, 0.0), lock: false,
             ab_lock: false, ab_name_edit: None, fit_request: None,
             top, win_action: None, show_rail: true, show_dock: true, tabs: vec!["Untitled-1".into()], tab_active: 0,
             logo, splash_start: Some(Instant::now()), last_splash: false }
    }

    /// Feed a window event to egui. Returns true if egui consumed it (so the canvas should NOT).
    pub fn on_event(&mut self, window: &Window, ev: &WindowEvent) -> bool {
        self.state.on_window_event(window, ev).consumed
    }
    /// Is the pointer over a panel / popup? (canvas strokes must NOT be swallowed by the panels)
    pub fn wants_pointer(&self) -> bool { self.ctx.is_pointer_over_area() || self.ctx.wants_pointer_input() }
    /// Is a text field actually focused? Only THEN should keys go to egui instead of canvas shortcuts.
    /// (Gate canvas shortcuts on this, NOT on egui's generic "consumed" — otherwise an Arabic-layout
    /// keypress, which egui receives as a Text event, would swallow V/A/P and the rest.)
    pub fn wants_keyboard(&self) -> bool { self.ctx.wants_keyboard_input() }
    /// Is the pointer over a scrubbable number field? (so the host shows the ↔ resize cursor)
    pub fn scrub_hover(&self) -> bool { self.cursor == egui::CursorIcon::ResizeHorizontal }
    /// (Re)start the startup splash timer — call right before revealing the window.
    pub fn start_splash(&mut self) { self.splash_start = Some(Instant::now()); }
    /// Did the last `run` build the splash? (host renders it on a transparent surface, over the desktop)
    pub fn splashing(&self) -> bool { self.last_splash }

    /// Build + lay out the panels; user changes are applied straight to the editor. Returns egui's
    /// tessellated output for `Renderer::render_ui`.
    pub fn run(&mut self, window: &Window, ed: &mut Editor, ppp: f32, view: View)
        -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor) {
        let input = self.state.take_egui_input(window);
        let snap = Snap::read(ed);
        let absnap = AbSnap::read(ed);
        let abs = ab_infos(ed);
        let snap_hud = ed.snap_hud.clone();
        let show_rulers = ed.show_rulers;
        let ruler_origin = ed.doc.ruler_origin;
        let ruler_reset = ed.doc.active_artboard().map(|a| [a.x, a.y]).unwrap_or([0.0, 0.0]);
        let ruler_grid = ed.adaptive_grid_step();   // tick on the SAME base-5 lattice as the dot grid
        let origin_preview = ed.origin_preview;      // dashed crosshair while dragging the ruler zero-point
        let tools = &self.tools;
        let shapes = &self.shapes;
        let icons = DockIcons { rotate: &self.ic_rotate, opacity: &self.ic_opacity, strokew: &self.ic_strokew,
            link: &self.ic_link, fliph: &self.ic_fliph, flipv: &self.ic_flipv, align: &self.align_icons };
        let ab_icons = AbIcons { link: &self.ic_link, portrait: &self.ic_portrait,
            landscape: &self.ic_landscape, fit: &self.ic_fit };
        let ab_dots = &self.ab_dots;
        let top = &self.top;
        let mut ops: Vec<Op> = Vec::new();
        let mut rects: Vec<egui::Rect> = Vec::new();
        let mut refpt = self.refpt;
        let mut lock = self.lock;
        let mut ab_lock = self.ab_lock;
        let mut ab_name_edit = std::mem::take(&mut self.ab_name_edit);
        let mut fit_request: Option<usize> = None;
        let mut shape_active = self.shape_active;
        let mut win_action = None;
        let mut show_rail = self.show_rail;
        let mut show_dock = self.show_dock;
        let mut snap_cfg = ed.doc.snap;   // the magnet menu edits this; written back after layout (mode flag)
        let mut tabs = std::mem::take(&mut self.tabs);
        let mut tab_active = self.tab_active;
        let splash = self.splash_start.map(|t| t.elapsed().as_secs_f32());
        let splashing = splash.map_or(false, |e| e < SPLASH_DUR);
        let logo = &self.logo;
        let out = self.ctx.run(input, |ctx| {
            if splashing {
                if let Some(e) = splash { build_splash(ctx, e, logo); } // only the floating card
            } else {
                build_topbar(ctx, top, &mut win_action, &mut tabs, &mut tab_active, &mut show_rail, &mut show_dock, &mut snap_cfg);
                if show_rulers { build_rulers(ctx, view, ppp, ruler_grid, ruler_origin, ruler_reset, &mut ops); }
                if show_rail { build_rail(ctx, tools, shapes, &mut shape_active, &snap, &mut ops, &mut rects); }
                if show_dock {
                    if snap.tool == ToolKind::Artboard {
                        build_ab_dock(ctx, &absnap, &ab_icons, &mut ab_lock, &mut ops, &mut rects, &mut fit_request);
                    } else {
                        build_dock(ctx, &snap, &icons, &mut refpt, &mut lock, &mut ops, &mut rects);
                    }
                }
                // on-canvas page chrome: a name label + ⋮ menu pinned over each artboard (any tool)
                build_ab_chrome(ctx, view, ppp, &abs, absnap.active, snap.tool == ToolKind::Artboard,
                                absnap.count, ab_dots, &mut ops, &mut ab_name_edit, &mut fit_request);
                build_snap_hud(ctx, view, ppp, &snap_hud);
                build_origin_crosshair(ctx, view, ppp, origin_preview);
            }
        });
        self.last_splash = splashing;
        if let Some(e) = splash { if e >= SPLASH_DUR { self.splash_start = None; } }
        self.refpt = refpt;
        self.lock = lock;
        self.ab_lock = ab_lock;
        self.ab_name_edit = ab_name_edit;
        self.shape_active = shape_active;
        if fit_request.is_some() { self.fit_request = fit_request; }
        self.win_action = win_action;
        self.show_rail = show_rail;
        self.show_dock = show_dock;
        self.tabs = tabs;
        self.tab_active = tab_active;
        ed.doc.snap = snap_cfg;   // commit the magnet-menu toggles (a non-undoable mode flag)
        apply_ops(ed, ops);
        self.cursor = out.platform_output.cursor_icon; // read the REAL cursor from this frame's output
        self.state.handle_platform_output(window, out.platform_output);
        self.rects = rects.into_iter().map(|r| egui::Rect::from_min_max(
            (r.min.to_vec2() * ppp).to_pos2(), (r.max.to_vec2() * ppp).to_pos2())).collect();
        self.repaint = out.viewport_output.get(&egui::ViewportId::ROOT).map_or(false, |v| v.repaint_delay.is_zero())
            || splash.map_or(false, |e| e < SPLASH_DUR); // keep animating the splash
        let jobs = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let sz = window.inner_size();
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [sz.width, sz.height], pixels_per_point: out.pixels_per_point };
        (jobs, out.textures_delta, screen)
    }
}

// ───────────────────────────── fonts / style / frame ─────────────────────────────

fn lucide(inner: &str) -> String {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" \
             stroke=\"#ffffff\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{inner}</svg>")
}

fn install_fonts(ctx: &egui::Context) {
    let mut f = egui::FontDefinitions::default();
    if let Ok(b) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
        f.font_data.insert("ui".to_owned(), egui::FontData::from_owned(b));
        f.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "ui".to_owned());
    }
    if let Ok(b) = std::fs::read("C:/Windows/Fonts/consola.ttf") {
        f.font_data.insert("mono".to_owned(), egui::FontData::from_owned(b));
        f.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "mono".to_owned());
    }
    ctx.set_fonts(f);
}

fn install_style(ctx: &egui::Context) {
    use egui::{FontFamily, TextStyle};
    let mut s = (*ctx.style()).clone();
    s.text_styles = [
        (TextStyle::Heading,   FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Body,      FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button,    FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Small,     FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ].into();
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.window_shadow = egui::epaint::Shadow::NONE;  // GPU pass owns the panel shadow
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.window_fill = SOLID_PANEL;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_rounding = Rounding::same(10.0);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0x0c, 0x8c, 0xe9, 90);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    s.visuals = v;
    ctx.set_style(s);
}

fn panel_frame(margin: f32) -> egui::Frame {
    egui::Frame {
        fill: SOLID_PANEL,
        rounding: Rounding::same(14.0),
        stroke: Stroke::new(1.0, BORDER),
        inner_margin: Margin::same(margin),
        ..Default::default()
    }
}

// ───────────────────────────── shared primitives ─────────────────────────────

/// A field's prefix: a compact letter (X/Y/W/H) or a small gray icon (rotation/opacity/stroke).
enum Lab<'a> { Letter(&'a str), Icon(Option<&'a egui::TextureHandle>) }

const UV01: fn() -> egui::Rect = || egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

/// Number field. A dim label column, then a rounded box holding the value CENTERED. The WHOLE box is
/// one interactive target via `ui.interact` (the exact mechanism the tool-rail buttons use): drag it to
/// scrub (↔ cursor), single-click to type (value pre-selected). Returns Some(new) on change.
/// (Blender ‹ › steppers come back once the core drag/type is confirmed.)
fn num_field(ui: &mut egui::Ui, w: f32, lab: Lab, tip: &str, value: f32, decimals: usize, _step: f32, speed: f32,
             range: std::ops::RangeInclusive<f32>) -> Option<f32> {
    let mut out = None;
    let (lo, hi) = (*range.start(), *range.end());
    let (row, _) = ui.allocate_exact_size(egui::vec2(w, 25.0), egui::Sense::hover());
    let p = ui.painter().clone();
    let labw = 22.0;
    match lab {
        Lab::Letter(s) => { p.text(egui::pos2(row.left() + labw - 5.0, row.center().y), Align2::RIGHT_CENTER, s, FontId::proportional(11.5), FAINT); }
        Lab::Icon(Some(t)) => { p.image(t.id(),
            egui::Rect::from_center_size(egui::pos2(row.left() + labw - 11.0, row.center().y), egui::vec2(14.0, 14.0)), UV01(), MUTED); }
        Lab::Icon(None) => {}
    }
    let bx = egui::Rect::from_min_max(egui::pos2(row.left() + labw + 2.0, row.top()), row.max);
    let id = ui.make_persistent_id(("numf", tip));
    let r5 = Rounding::same(5.0);
    // 'just entered' flag (set on click) survives the one frame until the TextEdit claims focus.
    let just = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    let editing = just || ui.memory(|m| m.has_focus(id));
    if editing {
        p.rect(bx, r5, Color32::from_rgb(0x17, 0x17, 0x1a), Stroke::new(1.0, ACCENT)); // dark "input well"
        let mut buf = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| format!("{value:.decimals$}"));
        let te = ui.put(bx.shrink2(egui::vec2(8.0, 3.0)),
            egui::TextEdit::singleline(&mut buf).id(id).frame(false).font(egui::FontId::proportional(13.0)).text_color(TEXT));
        if just { te.request_focus(); ui.data_mut(|d| d.remove::<bool>(id)); }
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
        if te.lost_focus() {
            if let Ok(v) = buf.trim().parse::<f32>() { out = Some(v.clamp(lo, hi)); }
            ui.data_mut(|d| { d.remove::<String>(id); d.remove::<bool>(id); });
        }
    } else {
        let resp = ui.interact(bx, id.with("box"), egui::Sense::click_and_drag());
        let hot = resp.hovered() || resp.dragged();
        if hot { p.rect(bx, r5, HOVER, Stroke::new(1.0, BORDER_2)); } else { p.rect_filled(bx, r5, BG_SURFACE); }
        p.text(bx.center(), Align2::CENTER_CENTER, format!("{value:.decimals$}"), FontId::proportional(13.0), TEXT);
        if hot { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal); }
        if resp.dragged() {
            let dx = resp.drag_delta().x;
            if dx != 0.0 { out = Some((value + dx * speed).clamp(lo, hi)); }
        }
        if resp.clicked() {
            let s = format!("{value:.decimals$}");
            // pre-select all so typing replaces the value (Blender behavior)
            let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            let n = s.chars().count();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
            st.store(ui.ctx(), id);
            ui.data_mut(|d| { d.insert_temp(id, s); d.insert_temp(id, true); });
            ui.memory_mut(|m| m.request_focus(id));
        }
        if !tip.is_empty() { resp.on_hover_text(tip); }
    }
    out
}

/// Tiny hand-painted glyph button (e.g. the clear-paint ×). Returns true on click.
fn mini_btn(ui: &mut egui::Ui, glyph: &str, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if resp.hovered() { ui.painter().rect_filled(rect, Rounding::same(5.0), HOVER); }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(14.0), MUTED);
    resp.on_hover_text(tip).clicked()
}

/// The 9-point transform reference widget (3×3 dots). Click a dot to set the reference (ax, ay).
fn refpoint(ui: &mut egui::Ui, sz: f32, refpt: &mut (f32, f32)) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(sz, sz), egui::Sense::click());
    let p = ui.painter();
    let inset = 7.0;
    let area = egui::Rect::from_min_max(egui::pos2(rect.left() + inset, rect.top() + inset), egui::pos2(rect.right() - inset, rect.bottom() - inset));
    for ay in 0..3 { for ax in 0..3 {
        let (fx, fy) = (ax as f32 * 0.5, ay as f32 * 0.5);
        let c = egui::pos2(area.left() + fx * area.width(), area.top() + fy * area.height());
        if (refpt.0 - fx).abs() < 0.01 && (refpt.1 - fy).abs() < 0.01 { p.circle_filled(c, 2.6, ACCENT); }
        else { p.circle_stroke(c, 2.0, Stroke::new(1.0, MUTED)); }
    }}
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let nx = ((pos.x - area.left()) / area.width().max(1.0) * 2.0).round().clamp(0.0, 2.0) / 2.0;
            let ny = ((pos.y - area.top()) / area.height().max(1.0) * 2.0).round().clamp(0.0, 2.0) / 2.0;
            *refpt = (nx, ny);
        }
    }
}

/// Small icon toggle (e.g. the constrain-proportions link). Accent when on.
fn icon_toggle(ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, on: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    if on { ui.painter().rect_filled(rect, Rounding::same(5.0), ACCENT); }
    else if resp.hovered() { ui.painter().rect_filled(rect, Rounding::same(5.0), HOVER); }
    if let Some(t) = tex { ui.painter().image(t.id(),
        egui::Rect::from_center_size(rect.center(), egui::vec2(15.0, 15.0)), UV01(),
        if on { Color32::WHITE } else { MUTED }); }
    resp.on_hover_text(tip).clicked()
}

/// Small icon action button (e.g. flip). White on hover.
fn icon_btn(ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 24.0), egui::Sense::click());
    if resp.hovered() { ui.painter().rect_filled(rect, Rounding::same(5.0), HOVER); }
    if let Some(t) = tex { ui.painter().image(t.id(),
        egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)), UV01(),
        if resp.hovered() { Color32::WHITE } else { MUTED }); }
    resp.on_hover_text(tip).clicked()
}

/// A short, full-width hairline divider.
fn hsep(ui: &mut egui::Ui, w: f32) {
    ui.add_space(9.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.left()..=rect.right(), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(9.0);
}

fn rgba_to_c32(c: Rgba) -> Color32 { Color32::from_rgb((c[0]*255.0) as u8, (c[1]*255.0) as u8, (c[2]*255.0) as u8) }
fn c32_to_rgba(c: Color32) -> Rgba { [c.r() as f32/255.0, c.g() as f32/255.0, c.b() as f32/255.0, 1.0] }
fn hex_of(c: Rgba) -> String { format!("#{:02X}{:02X}{:02X}", (c[0]*255.0).round() as u8, (c[1]*255.0).round() as u8, (c[2]*255.0).round() as u8) }

/// Fill / Stroke row: swatch (opens egui's picker) + hex + clear ×.
fn paint_row(ui: &mut egui::Ui, target: PaintTarget, color: Option<Rgba>, ops: &mut Vec<Op>) {
    ui.horizontal(|ui| {
        let mut c = color.map(rgba_to_c32).unwrap_or(Color32::from_gray(40));
        if egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::Opaque).changed() {
            ops.push(Op::Paint(target, Some(c32_to_rgba(c))));
        }
        ui.add_space(8.0);
        ui.label(RichText::new(color.map(hex_of).unwrap_or_else(|| "None".into())).color(TEXT).monospace().size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if mini_btn(ui, "×", "No paint") { ops.push(Op::Paint(target, None)); }
        });
    });
}

// ───────────────────────────── tool rail ─────────────────────────────

fn icon_button(ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
    let painter = ui.painter();
    let rounding = Rounding::same(9.0);
    if active { painter.rect_filled(rect, rounding, ACCENT); }
    else if resp.hovered() { painter.rect_filled(rect, rounding, HOVER); }
    if let Some(t) = tex {
        let ir = egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0));
        painter.image(t.id(), ir, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }
    resp
}

fn divider(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 1.0), egui::Sense::hover());
    ui.painter().hline((rect.left() + 7.0)..=(rect.right() - 7.0), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(3.0);
}

// ───────────────────────────── startup splash ─────────────────────────────

const SPLASH_DUR: f32 = 1.55;  // total seconds on screen
const SPLASH_HOLD: f32 = 1.20; // fully-opaque hold before the cross-fade
const SPLASH_FADE: f32 = 0.35; // scrim + card fade out
const SPLASH_IN: f32 = 0.18;   // card ease-in at the very start

fn with_a(c: Color32, a: f32) -> Color32 { Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0).clamp(0.0, 255.0) as u8) }
fn rgba_a(r: u8, g: u8, b: u8, a: f32) -> Color32 { Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0).clamp(0.0, 255.0) as u8) }

/// Photoshop-style startup splash: a centered card (logo + wordmark + version + tagline + progress +
/// an abstract "vector" art panel) on a dark scrim, drawn on a Foreground layer. Fades into the editor.
fn build_splash(ctx: &egui::Context, e: f32, logo: &Option<egui::TextureHandle>) {
    let ease_in = { let t = (e / SPLASH_IN).clamp(0.0, 1.0); 1.0 - (1.0 - t).powi(2) };
    let fade = ((e - SPLASH_HOLD) / SPLASH_FADE).clamp(0.0, 1.0);
    let ca = (ease_in * (1.0 - fade)).clamp(0.0, 1.0); // card alpha
    if ca <= 0.001 { return; }

    // The card floats on the window's transparent surface (no dark scrim) → it sits over the desktop.
    let scr = ctx.screen_rect();
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("splash")));
    let card = egui::Rect::from_center_size(scr.center() + egui::vec2(0.0, 2.0), egui::vec2(520.0, 300.0));
    // soft drop shadow — many faint layers (smooth, light), spread wide with a quadratic fade-out
    let n = 16;
    for i in 0..n {
        let f = i as f32 / (n - 1) as f32;
        let grow = 1.0 + f * 40.0;
        let off = 3.0 + f * 9.0;
        let a = (1.0 - f).powi(2) * (6.0 / 255.0) * ca;
        p.rect_filled(card.translate(egui::vec2(0.0, off)).expand(grow), Rounding::same(16.0 + grow), rgba_a(0, 0, 0, a));
    }
    p.rect_filled(card, Rounding::same(16.0), with_a(SOLID_PANEL, ca));
    p.rect_stroke(card, Rounding::same(16.0), Stroke::new(1.0, with_a(BORDER, ca)));
    p.line_segment([card.left_top() + egui::vec2(16.0, 1.0), card.right_top() + egui::vec2(-16.0, 1.0)], Stroke::new(1.0, rgba_a(255, 255, 255, 0.04 * ca)));

    let (l, t) = (card.left(), card.top());
    // logo + wordmark
    if let Some(tex) = logo {
        p.image(tex.id(), egui::Rect::from_center_size(egui::pos2(l + 45.0, t + 52.0), egui::vec2(34.0, 34.0)), UV01(), rgba_a(255, 255, 255, ca));
    }
    p.text(egui::pos2(l + 72.0, t + 47.0), Align2::LEFT_CENTER, "Varos", FontId::proportional(26.0), with_a(TEXT, ca));
    p.text(egui::pos2(l + 73.0, t + 69.0), Align2::LEFT_CENTER, "\u{3b1} \u{b7} pre-alpha", FontId::monospace(11.5), with_a(MUTED, ca));
    p.text(egui::pos2(l + 28.0, t + 104.0), Align2::LEFT_TOP, "Arabic-first vector design.", FontId::proportional(13.5), with_a(TEXT, 0.82 * ca));
    p.hline((l + 28.0)..=(l + 282.0), t + 150.0, Stroke::new(1.0, with_a(BORDER, ca)));
    // progress bar (fills 0→1 over the hold, ease-out)
    let bar = egui::Rect::from_min_size(egui::pos2(l + 28.0, t + 174.0), egui::vec2(196.0, 4.0));
    p.rect_filled(bar, Rounding::same(2.0), with_a(BG_SURFACE, ca));
    let prog = { let tt = (e / SPLASH_HOLD).clamp(0.0, 1.0); 1.0 - (1.0 - tt).powi(2) };
    let fw = (bar.width() * prog).max(8.0);
    p.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(fw, 4.0)), Rounding::same(2.0), with_a(ACCENT, ca));
    p.circle_filled(egui::pos2(bar.left() + fw, bar.center().y), 3.0, rgba_a(0x0c, 0x8c, 0xe9, 0.59 * ca));
    p.text(egui::pos2(l + 236.0, t + 176.0), Align2::LEFT_CENTER, "loading\u{2026}", FontId::proportional(11.0), with_a(MUTED, ca));
    p.text(egui::pos2(l + 28.0, card.bottom() - 22.0), Align2::LEFT_BOTTOM, "\u{a9} 2026 Varos \u{b7} pre-alpha \u{b7} built with wgpu + egui", FontId::proportional(10.5), rgba_a(0x60, 0x60, 0x64, ca));

    // ── abstract "vector editor" art panel (right) ──
    let a = egui::Rect::from_min_max(egui::pos2(card.right() - 224.0, t + 28.0), egui::pos2(card.right() - 28.0, card.bottom() - 28.0));
    p.rect_filled(a, Rounding::same(10.0), with_a(BG_SURFACE, ca));
    p.rect_stroke(a, Rounding::same(10.0), Stroke::new(1.0, with_a(BORDER, ca)));
    let pa = p.with_clip_rect(a);
    // diagonal accent corner-glow (faint, top-left of the panel)
    for i in 0..3 { pa.circle_filled(a.min + egui::vec2(6.0, 6.0), 70.0 - i as f32 * 18.0, rgba_a(0x0c, 0x8c, 0xe9, (0.05 + i as f32 * 0.025) * ca)); }
    // two ghosted "artboards"
    let ab = |x: f32, y: f32| egui::Rect::from_min_size(egui::pos2(a.left() + x, a.top() + y), egui::vec2(116.0, 92.0));
    pa.rect_filled(ab(30.0, 44.0), Rounding::same(8.0), rgba_a(255, 255, 255, 0.03 * ca));
    pa.rect_stroke(ab(30.0, 44.0), Rounding::same(8.0), Stroke::new(1.0, rgba_a(255, 255, 255, 0.07 * ca)));
    pa.rect_filled(ab(56.0, 84.0), Rounding::same(8.0), rgba_a(0x0c, 0x8c, 0xe9, 0.055 * ca));
    pa.rect_stroke(ab(56.0, 84.0), Rounding::same(8.0), Stroke::new(1.0, with_a(ACCENT, 0.45 * ca)));
    // ghost "V" monogram
    let vc = a.center();
    pa.line_segment([vc + egui::vec2(-42.0, -38.0), vc + egui::vec2(0.0, 44.0)], Stroke::new(10.0, rgba_a(255, 255, 255, 0.055 * ca)));
    pa.line_segment([vc + egui::vec2(42.0, -38.0), vc + egui::vec2(0.0, 44.0)], Stroke::new(10.0, rgba_a(255, 255, 255, 0.055 * ca)));
    // a pen-tool cubic Bézier with anchors + handles (the "this is a vector editor" tell)
    let (p0, p1, p2, p3) = (egui::pos2(a.left() + 26.0, a.bottom() - 54.0), egui::pos2(a.left() + 66.0, a.top() + 58.0),
                            egui::pos2(a.right() - 66.0, a.bottom() - 30.0), egui::pos2(a.right() - 26.0, a.top() + 70.0));
    let cub = |s: f32| { let u = 1.0 - s;
        egui::pos2(u*u*u*p0.x + 3.0*u*u*s*p1.x + 3.0*u*s*s*p2.x + s*s*s*p3.x, u*u*u*p0.y + 3.0*u*u*s*p1.y + 3.0*u*s*s*p2.y + s*s*s*p3.y) };
    let curve: Vec<egui::Pos2> = (0..=24).map(|i| cub(i as f32 / 24.0)).collect();
    pa.add(egui::Shape::line(curve, Stroke::new(2.0, with_a(ACCENT, ca))));
    pa.line_segment([p0, p1], Stroke::new(1.0, rgba_a(0x0c, 0x8c, 0xe9, 0.47 * ca)));
    pa.line_segment([p3, p2], Stroke::new(1.0, rgba_a(0x0c, 0x8c, 0xe9, 0.47 * ca)));
    for cp in [p1, p2] { pa.circle_stroke(cp, 2.5, Stroke::new(1.0, with_a(ACCENT, ca))); }
    for an in [p0, p3] {
        pa.rect_filled(egui::Rect::from_center_size(an, egui::vec2(6.0, 6.0)), Rounding::same(1.0), with_a(ACCENT, ca));
        pa.rect_stroke(egui::Rect::from_center_size(an, egui::vec2(6.0, 6.0)), Rounding::same(1.0), Stroke::new(1.0, rgba_a(255, 255, 255, 0.78 * ca)));
    }
}

// ───────────────────────────── custom title bar ─────────────────────────────

/// One window-control button (min/max/close): 46×40, no rounding, hover fill + icon.
#[derive(Clone, Copy)]
enum Cap { Min, Max, Close }

/// A window caption button. The glyph is PAINTED directly (crisp 1px lines like Windows 11 / Chrome),
/// not an SVG texture — the Lucide minus rendered with round caps looked like a fat pill, not a clean dash.
fn winctl(ui: &mut egui::Ui, p: &egui::Painter, rect: egui::Rect, cap: Cap,
          key: &str, hover_bg: Color32, white_on_hover: bool) -> bool {
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    let hov = resp.hovered();
    if hov { p.rect_filled(rect, Rounding::ZERO, hover_bg); }
    let col = if white_on_hover && hov { Color32::WHITE } else { TEXT };
    let s = Stroke::new(1.0, col);
    let c = rect.center();
    match cap {
        Cap::Min => { let y = c.y.round() + 0.5; p.line_segment([egui::pos2(c.x - 5.0, y), egui::pos2(c.x + 5.0, y)], s); }
        Cap::Max => { p.rect_stroke(egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)), Rounding::ZERO, s); }
        Cap::Close => {
            p.line_segment([c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)], s);
            p.line_segment([c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)], s);
        }
    }
    resp.clicked()
}

/// A 34×30 top-bar icon button (menu/search/layout/panels). Returns its Response.
fn topbtn(ui: &mut egui::Ui, p: &egui::Painter, rect: egui::Rect, tex: &Option<egui::TextureHandle>, key: &str, active: bool) -> egui::Response {
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    let r6 = Rounding::same(6.0);
    if active { p.rect_filled(rect, r6, BG_SURFACE); } else if resp.hovered() { p.rect_filled(rect, r6, HOVER); }
    let col = if active || resp.hovered() { TEXT } else { MUTED };
    if let Some(t) = tex { p.image(t.id(), egui::Rect::from_center_size(rect.center(), egui::vec2(17.0, 17.0)), UV01(), col); }
    resp
}

/// One document tab. Returns (activate_clicked, close_clicked).
fn tab_item(ui: &mut egui::Ui, p: &egui::Painter, rect: egui::Rect, label: &str, active: bool, tex_x: &Option<egui::TextureHandle>, key: &str) -> (bool, bool) {
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    if active {
        p.rect_filled(rect, Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 }, SOLID_PANEL);
        p.hline(rect.left()..=rect.right(), rect.top() + 1.0, Stroke::new(2.0, ACCENT));
    } else if resp.hovered() {
        let inner = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + 5.0), rect.max);
        p.rect_filled(inner, Rounding::same(6.0), BG_SURFACE);
    }
    p.text(egui::pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), if active { TEXT } else { MUTED });
    let x_r = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), egui::vec2(18.0, 18.0));
    let xr = ui.interact(x_r, ui.id().with((key, "x")), egui::Sense::click());
    if xr.hovered() { p.rect_filled(x_r, Rounding::same(4.0), HOVER); }
    if let Some(t) = tex_x { p.image(t.id(), egui::Rect::from_center_size(x_r.center(), egui::vec2(11.0, 11.0)), UV01(), if xr.hovered() { TEXT } else { MUTED }); }
    (resp.clicked(), xr.clicked())
}

fn menu_row(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::click());
    if resp.hovered() { ui.painter().rect_filled(rect, Rounding::same(6.0), BG_SURFACE); }
    ui.painter().text(egui::pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), TEXT);
    if !shortcut.is_empty() { ui.painter().text(egui::pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, shortcut, FontId::monospace(11.0), MUTED); }
    resp.clicked()
}

/// A checklist row (panels show/hide). Returns true on click.
fn check_row(ui: &mut egui::Ui, label: &str, checked: bool, tex_check: &Option<egui::TextureHandle>) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::click());
    if resp.hovered() { ui.painter().rect_filled(rect, Rounding::same(6.0), BG_SURFACE); }
    let box_r = egui::Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), egui::vec2(16.0, 16.0));
    if checked {
        ui.painter().rect_filled(box_r, Rounding::same(4.0), ACCENT);
        if let Some(t) = tex_check { ui.painter().image(t.id(), egui::Rect::from_center_size(box_r.center(), egui::vec2(12.0, 12.0)), UV01(), Color32::WHITE); }
    } else {
        ui.painter().rect_stroke(box_r, Rounding::same(4.0), Stroke::new(1.0, BORDER_2));
    }
    ui.painter().text(egui::pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), TEXT);
    resp.clicked()
}

/// Custom top bar (the native caption is stripped in WM_NCCALCSIZE): menu · tabs · drag · right tools ·
/// window controls. Interactive rects are published as exclusions so the OS hit-test makes them HTCLIENT
/// (egui handles them) while the empty band is HTCAPTION (the OS drags/snaps the window).
fn build_topbar(ctx: &egui::Context, top: &TopIcons, win_action: &mut Option<WinAction>,
                tabs: &mut Vec<String>, tab_active: &mut usize, show_rail: &mut bool, show_dock: &mut bool,
                snap: &mut varos_core::model::SnapConfig) {
    let h = 40.0;
    let frame = egui::Frame { fill: BG, inner_margin: Margin::ZERO, ..Default::default() };
    egui::TopBottomPanel::top("topbar").exact_height(h).frame(frame).show(ctx, |ui| {
        let bar = ui.max_rect();
        let p = ui.painter().clone();
        let cy = bar.center().y;
        p.hline(bar.left()..=bar.right(), bar.bottom() - 0.5, Stroke::new(1.0, BORDER));
        let mut excl: Vec<egui::Rect> = Vec::new();

        // window controls (min · max · close)
        let bw = 46.0;
        let close_r = egui::Rect::from_min_max(egui::pos2(bar.right() - bw, bar.top()), egui::pos2(bar.right(), bar.bottom()));
        let max_r = egui::Rect::from_min_max(egui::pos2(bar.right() - 2.0 * bw, bar.top()), egui::pos2(bar.right() - bw, bar.bottom()));
        let min_r = egui::Rect::from_min_max(egui::pos2(bar.right() - 3.0 * bw, bar.top()), egui::pos2(bar.right() - 2.0 * bw, bar.bottom()));
        if winctl(ui, &p, min_r, Cap::Min, "wc-min", BG_SURFACE, false) { *win_action = Some(WinAction::Minimize); }
        if winctl(ui, &p, max_r, Cap::Max, "wc-max", BG_SURFACE, false) { *win_action = Some(WinAction::ToggleMaximize); }
        if winctl(ui, &p, close_r, Cap::Close, "wc-close", CLOSE_RED, true) { *win_action = Some(WinAction::Close); }
        excl.extend([min_r, max_r, close_r]);

        // right tools (search · layout · panels), to the left of the window controls
        let (tb, th) = (34.0, 30.0);
        let cell = |rx: f32| egui::Rect::from_min_max(egui::pos2(rx - tb, cy - th / 2.0), egui::pos2(rx, cy + th / 2.0));
        let panels_id = ui.make_persistent_id("panels_menu");
        let menu_id = ui.make_persistent_id("app_menu");
        let panels_r = cell(min_r.left() - 8.0);
        let panels_active = ui.memory(|m| m.is_popup_open(panels_id)) || !*show_rail || !*show_dock;
        let pr = topbtn(ui, &p, panels_r, &top.panels, "tb-panels", panels_active);
        if pr.clicked() { ui.memory_mut(|m| m.toggle_popup(panels_id)); }
        let layout_r = cell(panels_r.left() - 2.0);
        topbtn(ui, &p, layout_r, &top.layout, "tb-layout", false).on_hover_text("Layout");
        let search_r = cell(layout_r.left() - 2.0);
        topbtn(ui, &p, search_r, &top.search, "tb-search", false).on_hover_text("Search");
        // magnet = the Snapping quick-menu (Illustrator layout)
        let magnet_id = ui.make_persistent_id("snap_menu");
        let magnet_r = cell(search_r.left() - 2.0);
        let magnet_active = ui.memory(|m| m.is_popup_open(magnet_id)) || snap.smart || snap.grid;
        let magr = topbtn(ui, &p, magnet_r, &top.magnet, "tb-magnet", magnet_active);
        if magr.clicked() { ui.memory_mut(|m| m.toggle_popup(magnet_id)); }
        excl.extend([panels_r, layout_r, search_r, magnet_r]);

        // menu button (left)
        let menu_r = egui::Rect::from_min_max(egui::pos2(bar.left() + 6.0, cy - 15.0), egui::pos2(bar.left() + 40.0, cy + 15.0));
        let mr = topbtn(ui, &p, menu_r, &top.menu, "tb-menu", ui.memory(|m| m.is_popup_open(menu_id)));
        if mr.clicked() { ui.memory_mut(|m| m.toggle_popup(menu_id)); }
        excl.push(menu_r);

        // tabs + new-tab
        let tabs_right = magnet_r.left() - 12.0;
        let mut tx = menu_r.right() + 8.0;
        let tw = 154.0;
        let (mut to_close, mut to_activate) = (None, None);
        for i in 0..tabs.len() {
            if tx + tw > tabs_right { break; }
            let trect = egui::Rect::from_min_max(egui::pos2(tx, bar.top()), egui::pos2(tx + tw, bar.bottom()));
            let (click, close) = tab_item(ui, &p, trect, &tabs[i], i == *tab_active, &top.x, &format!("tab{i}"));
            if click { to_activate = Some(i); }
            if close { to_close = Some(i); }
            excl.push(trect);
            tx += tw + 2.0;
        }
        let plus_r = egui::Rect::from_min_max(egui::pos2(tx, cy - 14.0), egui::pos2(tx + 28.0, cy + 14.0));
        if tx + 28.0 <= tabs_right {
            if topbtn(ui, &p, plus_r, &top.plus, "tb-plus", false).clicked() {
                tabs.push(format!("Untitled-{}", tabs.len() + 1)); *tab_active = tabs.len() - 1;
            }
            excl.push(plus_r);
        }
        if let Some(i) = to_activate { *tab_active = i; }
        if let Some(i) = to_close { if tabs.len() > 1 { tabs.remove(i); if *tab_active >= tabs.len() { *tab_active = tabs.len() - 1; } } }

        // dropdowns
        egui::popup_below_widget(ui, menu_id, &mr, |ui| {
            ui.set_min_width(196.0);
            menu_row(ui, "New", "Ctrl+N"); menu_row(ui, "Open\u{2026}", "Ctrl+O"); menu_row(ui, "Save", "Ctrl+S");
            ui.add_space(4.0); let (sr, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().hline(sr.left() + 4.0..=sr.right() - 4.0, sr.center().y, Stroke::new(1.0, BORDER)); ui.add_space(4.0);
            menu_row(ui, "Export\u{2026}", "");
        });
        egui::popup_below_widget(ui, panels_id, &pr, |ui| {
            ui.set_min_width(186.0);
            if check_row(ui, "Tool rail", *show_rail, &top.check) { *show_rail = !*show_rail; }
            if check_row(ui, "Inspector", *show_dock, &top.check) { *show_dock = !*show_dock; }
        });
        // Snapping quick-menu (Illustrator "Snapping" popover)
        egui::popup_below_widget(ui, magnet_id, &magr, |ui| {
            ui.set_min_width(204.0);
            if check_row(ui, "Snap to Grid", snap.grid, &top.check) { snap.grid = !snap.grid; }
            if check_row(ui, "Snap to Point", snap.key_points, &top.check) { snap.key_points = !snap.key_points; }
            ui.add_space(4.0);
            let (sr, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().hline(sr.left() + 4.0..=sr.right() - 4.0, sr.center().y, Stroke::new(1.0, BORDER)); ui.add_space(4.0);
            if check_row(ui, "Smart Guides  (Ctrl+U)", snap.smart, &top.check) { snap.smart = !snap.smart; }
            if check_row(ui, "    Alignment Guides", snap.alignment_guides, &top.check) { snap.alignment_guides = !snap.alignment_guides; }
            if check_row(ui, "    Geometric Guides", snap.object_geometry, &top.check) { snap.object_geometry = !snap.object_geometry; }
        });

        // publish caption height + interactive (non-drag) rects, in physical px
        let ppp = ctx.pixels_per_point();
        let px: Vec<[i32; 4]> = excl.iter()
            .map(|r| [(r.left() * ppp) as i32, (r.top() * ppp) as i32, (r.right() * ppp) as i32, (r.bottom() * ppp) as i32]).collect();
        crate::cursors::set_caption((h * ppp) as i32, &px);
    });
}

fn build_rail(ctx: &egui::Context, tools: &[ToolBtn], shapes: &[ToolBtn], shape_active: &mut ToolKind,
              s: &Snap, ops: &mut Vec<Op>, rects: &mut Vec<egui::Rect>) {
    // the shapes slot mirrors whichever shape tool is actually active (so the M/L keys update it too)
    if shapes.iter().any(|t| t.kind == s.tool) { *shape_active = s.tool; }
    let r = egui::Window::new("tools").title_bar(false).resizable(false)
        .anchor(Align2::LEFT_CENTER, egui::vec2(16.0, 0.0)).frame(panel_frame(7.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for t in tools {
                if icon_button(ui, &t.tex, s.tool == t.kind).on_hover_text(t.tip).clicked() {
                    ops.push(Op::Tool(t.kind));
                }
                if t.group_end { divider(ui); }
                if t.kind == ToolKind::Pen {            // the SHAPES slot sits right after Pen
                    shape_slot(ui, shapes, shape_active, s, ops);
                    divider(ui);
                }
            }
        });
    if let Some(r) = r { rects.push(r.response.rect); }
}

/// One rail slot standing in for all four shape tools. Left-click uses the current shape; right-click
/// opens a flyout of all four (Illustrator tool-group behaviour). A corner mark hints at the flyout.
fn shape_slot(ui: &mut egui::Ui, shapes: &[ToolBtn], shape_active: &mut ToolKind, s: &Snap, ops: &mut Vec<Op>) {
    let cur = shapes.iter().find(|t| t.kind == *shape_active).unwrap_or(&shapes[0]);
    let is_active = shapes.iter().any(|t| t.kind == s.tool);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
    let rounding = Rounding::same(9.0);
    if is_active { ui.painter().rect_filled(rect, rounding, ACCENT); }
    else if resp.hovered() { ui.painter().rect_filled(rect, rounding, HOVER); }
    if let Some(t) = &cur.tex {
        ui.painter().image(t.id(), egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)), UV01(), Color32::WHITE);
    }
    // tiny flyout marker — a corner triangle bottom-right, like Illustrator's grouped tools
    let c = rect.right_bottom() + egui::vec2(-4.0, -4.0);
    ui.painter().add(egui::Shape::convex_polygon(vec![c, c + egui::vec2(-5.0, 0.0), c + egui::vec2(0.0, -5.0)],
        if is_active { Color32::WHITE } else { MUTED }, Stroke::NONE));
    if resp.clicked() { ops.push(Op::Tool(*shape_active)); }
    resp.clone().on_hover_text("Shapes \u{2014} click to use \u{00b7} right-click for more");
    let pop = ui.make_persistent_id("shape-flyout");
    if resp.secondary_clicked() { ui.memory_mut(|m| m.toggle_popup(pop)); }
    egui::popup_below_widget(ui, pop, &resp, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for t in shapes {
                if icon_button(ui, &t.tex, s.tool == t.kind).on_hover_text(t.tip).clicked() {
                    *shape_active = t.kind; ops.push(Op::Tool(t.kind)); ui.memory_mut(|m| m.close_popup());
                }
            }
        });
    });
}

// ───────────────────────────── inspector dock ─────────────────────────────

struct DockIcons<'a> {
    rotate: &'a Option<egui::TextureHandle>, opacity: &'a Option<egui::TextureHandle>, strokew: &'a Option<egui::TextureHandle>,
    link: &'a Option<egui::TextureHandle>, fliph: &'a Option<egui::TextureHandle>, flipv: &'a Option<egui::TextureHandle>,
    align: &'a [Option<egui::TextureHandle>; 8],
}

fn build_dock(ctx: &egui::Context, s: &Snap, ic: &DockIcons, refpt: &mut (f32, f32), lock: &mut bool,
              ops: &mut Vec<Op>, rects: &mut Vec<egui::Rect>) {
    let inner = 214.0;
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    let r = egui::Window::new("dock").title_bar(false).resizable(false)
        .anchor(Align2::RIGHT_CENTER, egui::vec2(-16.0, 0.0)).frame(panel_frame(13.0))
        .show(ctx, |ui| {
            ui.set_width(inner);
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
            ui.label(RichText::new(&s.name).color(if s.sel { TEXT } else { MUTED }).size(12.5).strong());
            ui.add_space(2.0);
            ui.label(RichText::new("TRANSFORM").color(MUTED).size(10.0).strong());
            ui.add_space(2.0);

            // ── Transform block: [9-pt refpoint] [X/W · Y/H] [link] ──
            ui.horizontal(|ui| {
                refpoint(ui, 38.0, refpt);
                let (ax, ay) = *refpt;
                let fw = 66.0;
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let dx = s.x + ax * s.w;
                        if let Some(v) = num_field(ui, fw, Lab::Letter("X"), "X position", dx, 0, 1.0, 1.0, full.clone()) { ops.push(Op::SetBBox(Some(v), None, None, None, ax, ay)); }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("W"), "Width", s.w, 0, 1.0, 1.0, 0.0..=1.0e6) {
                            if *lock && s.w > 0.0 { ops.push(Op::SetBBox(None, None, Some(v), Some(s.h * v / s.w), ax, ay)); }
                            else { ops.push(Op::SetBBox(None, None, Some(v), None, ax, ay)); }
                        }
                    });
                    ui.horizontal(|ui| {
                        let dy = s.y + ay * s.h;
                        if let Some(v) = num_field(ui, fw, Lab::Letter("Y"), "Y position", dy, 0, 1.0, 1.0, full.clone()) { ops.push(Op::SetBBox(None, Some(v), None, None, ax, ay)); }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("H"), "Height", s.h, 0, 1.0, 1.0, 0.0..=1.0e6) {
                            if *lock && s.h > 0.0 { ops.push(Op::SetBBox(None, None, Some(s.w * v / s.h), Some(v), ax, ay)); }
                            else { ops.push(Op::SetBBox(None, None, None, Some(v), ax, ay)); }
                        }
                    });
                });
                if icon_toggle(ui, ic.link, *lock, "Constrain W/H proportions") { *lock = !*lock; }
            });

            // ── Angle + flip ──
            ui.horizontal(|ui| {
                if let Some(v) = num_field(ui, 150.0, Lab::Icon(ic.rotate.as_ref()), "Rotation", s.rot, 1, 1.0, 0.5, full.clone()) { ops.push(Op::SetRot(v)); }
                if icon_btn(ui, ic.fliph, "Flip horizontal") { ops.push(Op::Flip(true)); }
                if icon_btn(ui, ic.flipv, "Flip vertical") { ops.push(Op::Flip(false)); }
            });

            hsep(ui, inner);

            // Appearance: opacity
            if let Some(v) = num_field(ui, inner, Lab::Icon(ic.opacity.as_ref()), "Opacity %", s.opacity * 100.0, 0, 1.0, 0.5, 0.0..=100.0) { ops.push(Op::SetOpacity(v / 100.0)); }

            hsep(ui, inner);

            // Fill / Stroke swatches + stroke weight
            paint_row(ui, PaintTarget::Fill, s.fill, ops);
            paint_row(ui, PaintTarget::Stroke, s.stroke, ops);
            if let Some(v) = num_field(ui, inner, Lab::Icon(ic.strokew.as_ref()), "Stroke weight", s.sw, 1, 0.5, 0.2, 0.0..=400.0) { ops.push(Op::SetStrokeW(v)); }

            hsep(ui, inner);
            ui.label(RichText::new("ALIGN").color(MUTED).size(10.0).strong());
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if icon_btn(ui, &ic.align[0], "Align left") { ops.push(Op::Align(AlignMode::Left)); }
                if icon_btn(ui, &ic.align[1], "Align centre") { ops.push(Op::Align(AlignMode::CenterH)); }
                if icon_btn(ui, &ic.align[2], "Align right") { ops.push(Op::Align(AlignMode::Right)); }
                if icon_btn(ui, &ic.align[3], "Align top") { ops.push(Op::Align(AlignMode::Top)); }
                if icon_btn(ui, &ic.align[4], "Align middle") { ops.push(Op::Align(AlignMode::Middle)); }
                if icon_btn(ui, &ic.align[5], "Align bottom") { ops.push(Op::Align(AlignMode::Bottom)); }
            });
            ui.horizontal(|ui| {
                if icon_btn(ui, &ic.align[6], "Distribute horizontal centres") { ops.push(Op::Distribute(DistAxis::Horizontal)); }
                if icon_btn(ui, &ic.align[7], "Distribute vertical centres") { ops.push(Op::Distribute(DistAxis::Vertical)); }
            });
        });
    if let Some(r) = r { rects.push(r.response.rect); }
}

// ───────────────────────────── artboard inspector ─────────────────────────────

struct AbIcons<'a> {
    link: &'a Option<egui::TextureHandle>,
    portrait: &'a Option<egui::TextureHandle>,
    landscape: &'a Option<egui::TextureHandle>,
    fit: &'a Option<egui::TextureHandle>,
}

/// A single-line text field bound to an external value (artboard name). While unfocused it tracks the
/// model value; once focused it edits a temp buffer; commits the buffer on focus loss (returns it).
fn name_field(ui: &mut egui::Ui, w: f32, value: &str, id_src: &str) -> Option<String> {
    let id = ui.make_persistent_id(("abname", id_src));
    let editing = ui.memory(|m| m.has_focus(id));
    let mut buf = if editing { ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| value.to_string()) } else { value.to_string() };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::hover());
    ui.painter().rect(rect, Rounding::same(6.0), BG_SURFACE, Stroke::new(1.0, if editing { ACCENT } else { BORDER }));
    let te = ui.put(rect.shrink2(egui::vec2(8.0, 3.0)),
        egui::TextEdit::singleline(&mut buf).id(id).frame(false).font(FontId::proportional(13.0)).text_color(TEXT));
    if te.has_focus() { ui.data_mut(|d| d.insert_temp(id, buf.clone())); }
    let mut out = None;
    if te.lost_focus() { out = Some(buf.clone()); ui.data_mut(|d| d.remove::<String>(id)); }
    out
}

/// A label + a hand-painted pill switch (the Clip / transparent / move-with toggles). Returns true on click.
fn toggle_row(ui: &mut egui::Ui, w: f32, label: &str, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    if resp.hovered() { ui.painter().rect_filled(rect, Rounding::same(6.0), HOVER); }
    ui.painter().text(egui::pos2(rect.left() + 4.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), if on { TEXT } else { MUTED });
    let pill = egui::Rect::from_min_size(egui::pos2(rect.right() - 36.0, rect.center().y - 9.0), egui::vec2(32.0, 18.0));
    ui.painter().rect_filled(pill, Rounding::same(9.0), if on { ACCENT } else { BG_SURFACE });
    let knob = egui::pos2(if on { pill.right() - 9.0 } else { pill.left() + 9.0 }, pill.center().y);
    ui.painter().circle_filled(knob, 6.5, Color32::WHITE);
    resp.clicked()
}

/// A small text button (Add / Duplicate / Delete). `disabled` greys it out and swallows clicks.
fn pill_btn(ui: &mut egui::Ui, label: &str, disabled: bool) -> bool {
    let w = ui.available_width().min(64.0).max(40.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    let hot = resp.hovered() && !disabled;
    ui.painter().rect(rect, Rounding::same(6.0), if hot { HOVER } else { BG_SURFACE }, Stroke::new(1.0, BORDER));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), if disabled { FAINT } else { TEXT });
    resp.clicked() && !disabled
}

/// The Artboard inspector dock (shown in place of the object inspector when the Artboard tool is active).
fn build_ab_dock(ctx: &egui::Context, s: &AbSnap, ic: &AbIcons, ab_lock: &mut bool,
                 ops: &mut Vec<Op>, rects: &mut Vec<egui::Rect>, fit_request: &mut Option<usize>) {
    let inner = 214.0;
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    let i = s.active;
    let r = egui::Window::new("ab-dock").title_bar(false).resizable(false)
        .anchor(Align2::RIGHT_CENTER, egui::vec2(-16.0, 0.0)).frame(panel_frame(13.0))
        .show(ctx, |ui| {
            ui.set_width(inner);
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Artboard").color(TEXT).size(13.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} / {}", i + 1, s.count)).color(MUTED).size(11.5));
                });
            });
            if let Some(v) = name_field(ui, inner, &s.name, "dock") { ops.push(Op::AbName(i, v)); }

            ui.add_space(2.0);
            ui.label(RichText::new("SIZE").color(MUTED).size(10.0).strong());
            // preset dropdown
            let preset_id = ui.make_persistent_id("ab-preset");
            let (prect, presp) = ui.allocate_exact_size(egui::vec2(inner, 26.0), egui::Sense::click());
            ui.painter().rect(prect, Rounding::same(6.0), BG_SURFACE, Stroke::new(1.0, if presp.hovered() { BORDER_2 } else { BORDER }));
            ui.painter().text(egui::pos2(prect.left() + 10.0, prect.center().y), Align2::LEFT_CENTER, "Presets\u{2026}", FontId::proportional(12.5), TEXT);
            ui.painter().text(egui::pos2(prect.right() - 10.0, prect.center().y), Align2::RIGHT_CENTER, "\u{25be}", FontId::proportional(11.0), MUTED);
            if presp.clicked() { ui.memory_mut(|m| m.toggle_popup(preset_id)); }
            egui::popup_below_widget(ui, preset_id, &presp, |ui| {
                ui.set_min_width(inner);
                for (label, w, h) in AB_PRESETS {
                    if menu_row(ui, label, "") { ops.push(Op::AbRect(i, None, None, Some(w), Some(h))); ui.memory_mut(|m| m.close_popup()); }
                }
            });
            // W / H + constrain
            ui.horizontal(|ui| {
                let fw = 70.0;
                if let Some(v) = num_field(ui, fw, Lab::Letter("W"), "Width", s.w, 0, 1.0, 1.0, 1.0..=1.0e6) {
                    if *ab_lock && s.w > 0.0 { ops.push(Op::AbRect(i, None, None, Some(v), Some(s.h * v / s.w))); }
                    else { ops.push(Op::AbRect(i, None, None, Some(v), None)); }
                }
                if let Some(v) = num_field(ui, fw, Lab::Letter("H"), "Height", s.h, 0, 1.0, 1.0, 1.0..=1.0e6) {
                    if *ab_lock && s.h > 0.0 { ops.push(Op::AbRect(i, None, None, Some(s.w * v / s.h), Some(v))); }
                    else { ops.push(Op::AbRect(i, None, None, None, Some(v))); }
                }
                if icon_toggle(ui, ic.link, *ab_lock, "Constrain W/H") { *ab_lock = !*ab_lock; }
            });
            // orientation + fit
            ui.horizontal(|ui| {
                let portrait = s.h >= s.w;
                if icon_toggle(ui, ic.portrait, portrait, "Portrait") && !portrait { ops.push(Op::AbOrient(i)); }
                if icon_toggle(ui, ic.landscape, !portrait, "Landscape") && portrait { ops.push(Op::AbOrient(i)); }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_btn(ui, ic.fit, "Fit in window") { *fit_request = Some(i); }
                });
            });
            // X / Y
            ui.horizontal(|ui| {
                let fw = 70.0;
                if let Some(v) = num_field(ui, fw, Lab::Letter("X"), "X position", s.x, 0, 1.0, 1.0, full.clone()) { ops.push(Op::AbRect(i, Some(v), None, None, None)); }
                if let Some(v) = num_field(ui, fw, Lab::Letter("Y"), "Y position", s.y, 0, 1.0, 1.0, full.clone()) { ops.push(Op::AbRect(i, None, Some(v), None, None)); }
            });

            hsep(ui, inner);
            // page colour + transparent
            ui.horizontal(|ui| {
                let mut c = s.color.map(rgba_to_c32).unwrap_or(Color32::from_gray(40));
                if egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::Opaque).changed() {
                    ops.push(Op::AbColor(i, Some(c32_to_rgba(c))));
                }
                ui.add_space(8.0);
                ui.label(RichText::new(match s.color { Some(c) => hex_of(c), None => "Transparent".into() }).color(TEXT).monospace().size(12.0));
            });
            if toggle_row(ui, inner, "Transparent page", s.color.is_none()) {
                ops.push(Op::AbColor(i, if s.color.is_none() { Some([1.0, 1.0, 1.0, 1.0]) } else { None }));
            }

            hsep(ui, inner);
            if let Some(v) = num_field(ui, inner, Lab::Letter("#"), "Artboard count", s.count as f32, 0, 1.0, 0.1, 1.0..=200.0) {
                ops.push(Op::AbCount(v.round().max(1.0) as usize));
            }
            if toggle_row(ui, inner, "Clip to page", s.clip) { ops.push(Op::AbClip(i)); }
            if toggle_row(ui, inner, "Move artwork with artboard", s.move_art) { ops.push(Op::AbMoveArt(!s.move_art)); }

            hsep(ui, inner);
            ui.horizontal(|ui| {
                if pill_btn(ui, "+ Add", false) { ops.push(Op::AbAdd); }
                if pill_btn(ui, "Duplicate", false) { ops.push(Op::AbDup(i)); }
                if pill_btn(ui, "Delete", s.count <= 1) { ops.push(Op::AbDel(i)); }
            });
        });
    if let Some(r) = r { rects.push(r.response.rect); }
}

/// On-canvas page chrome: a name label (top-left of each page) + a ⋮ button opening the edit menu. The
/// menu is the ungated way to edit a page from ANY tool (Decision 8); selecting a page by clicking its
/// name only works in the Artboard tool. Positions are pinned to each page via the view transform.
fn build_ab_chrome(ctx: &egui::Context, view: View, ppp: f32, abs: &[AbInfo], active: usize, tool_ab: bool,
                   count: usize, dots: &Option<egui::TextureHandle>, ops: &mut Vec<Op>,
                   name_edit: &mut Option<(usize, String)>, fit_request: &mut Option<usize>) {
    let scr = ctx.screen_rect();
    let mut clear_edit = false;
    for ab in abs {
        // top-left of the page in screen POINTS (view maps world→physical px; egui works in points)
        let tl = view.w2s([ab.x, ab.y]);
        let pos = egui::pos2((tl[0] / ppp).max(2.0), (tl[1] / ppp - 24.0).max(2.0));
        if pos.x > scr.right() - 8.0 || pos.y > scr.bottom() - 8.0 { continue; }   // page is off to the side
        let is_active = ab.i == active;
        egui::Area::new(egui::Id::new(("ab-chrome", ab.i))).fixed_pos(pos).order(egui::Order::Middle).interactable(true).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let renaming = matches!(&*name_edit, Some((j, _)) if *j == ab.i);
                if renaming {
                    if let Some((_, buf)) = name_edit.as_mut() {
                        let te = ui.add(egui::TextEdit::singleline(buf).desired_width(120.0).font(FontId::proportional(12.5)).text_color(TEXT));
                        if te.lost_focus() { ops.push(Op::AbName(ab.i, buf.clone())); clear_edit = true; } else { te.request_focus(); }
                    }
                } else {
                    let col = if is_active { TEXT } else { MUTED };
                    let lbl = ui.add(egui::Label::new(RichText::new(&ab.name).color(col).size(12.5)).sense(egui::Sense::click()));
                    if lbl.clicked() && tool_ab { ops.push(Op::AbActive(ab.i)); }
                    if lbl.double_clicked() { *name_edit = Some((ab.i, ab.name.clone())); }
                }
                let (dr, dresp) = ui.allocate_exact_size(egui::vec2(15.0, 18.0), egui::Sense::click());
                if dresp.hovered() { ui.painter().rect_filled(dr, Rounding::same(4.0), HOVER); }
                if let Some(t) = dots { ui.painter().image(t.id(), egui::Rect::from_center_size(dr.center(), egui::vec2(12.0, 12.0)), UV01(), if dresp.hovered() || is_active { TEXT } else { MUTED }); }
                let menu_id = ui.make_persistent_id(("ab-menu", ab.i));
                if dresp.clicked() { ui.memory_mut(|m| m.toggle_popup(menu_id)); }
                egui::popup_below_widget(ui, menu_id, &dresp, |ui| {
                    ui.set_min_width(170.0);
                    if menu_row(ui, "Rename", "") { *name_edit = Some((ab.i, ab.name.clone())); ui.memory_mut(|m| m.close_popup()); }
                    if menu_row(ui, "Duplicate", "") { ops.push(Op::AbDup(ab.i)); ui.memory_mut(|m| m.close_popup()); }
                    if menu_row(ui, if ab.h >= ab.w { "Make landscape" } else { "Make portrait" }, "") { ops.push(Op::AbOrient(ab.i)); ui.memory_mut(|m| m.close_popup()); }
                    if menu_row(ui, if ab.transparent { "White background" } else { "Transparent" }, "") {
                        ops.push(Op::AbColor(ab.i, if ab.transparent { Some([1.0, 1.0, 1.0, 1.0]) } else { None })); ui.memory_mut(|m| m.close_popup());
                    }
                    if menu_row(ui, if ab.clip { "Unclip" } else { "Clip to page" }, "") { ops.push(Op::AbClip(ab.i)); ui.memory_mut(|m| m.close_popup()); }
                    if menu_row(ui, "Fit in window", "") { *fit_request = Some(ab.i); ui.memory_mut(|m| m.close_popup()); }
                    if count > 1 && menu_row(ui, "Delete", "") { ops.push(Op::AbDel(ab.i)); ui.memory_mut(|m| m.close_popup()); }
                });
            });
        });
    }
    if clear_edit { *name_edit = None; }
}

// ───────────────────────────── rulers ─────────────────────────────

const RULER: f32 = 18.0;   // ruler strip thickness in points

/// Decimals needed to print multiples of `grid` cleanly (grid is a power of 5: 25→0, 0.2→1, 0.04→2).
fn ruler_dec(grid: f32) -> usize { (-grid.log10()).ceil().max(0.0) as usize }

fn fmt_ruler(v: f32, grid: f32, dec: usize) -> String {
    let v = if v.abs() < grid * 0.001 { 0.0 } else { v };   // kill -0
    format!("{:.*}", dec, v)
}

/// Top + left rulers (Ctrl+R). Ticks sit on the SAME base-5 lattice as the dot grid (`grid` = the finest
/// visible dot spacing) so they line up exactly; every 5th tick is labeled. Numbers read relative to
/// `origin` (top-left, Y-down). A live cyan tick tracks the pointer; the corner box drag-sets the origin
/// (snapped to page corners / grid) and double-click resets it. egui panels, so canvas input is gated.
fn build_rulers(ctx: &egui::Context, view: View, ppp: f32, grid: f32, origin: [f32; 2], reset: [f32; 2], ops: &mut Vec<Op>) {
    let frame = egui::Frame { fill: BG, inner_margin: Margin::ZERO, ..Default::default() };
    let num_font = FontId::proportional(9.5);
    let dec = ruler_dec(grid);
    // label every Nth grid-tick so numbers stay ~70 pts apart at ANY zoom (dense, never a vast gap). N is a
    // grid multiple, so labels still land on dots; small N → rounder numbers (×2 = 250s, not ×5 = 625s).
    let ms_pts = grid * view.zoom / ppp.max(1e-6);      // one grid-tick in screen points
    let label_every = [1i64, 2, 5, 10, 20, 50, 100, 200].into_iter().find(|&n| n as f32 * ms_pts >= 70.0).unwrap_or(200);
    let pointer = ctx.pointer_latest_pos();

    // top (horizontal) ruler — ticks at WORLD multiples of `grid` (land on the dots), label every 5th
    egui::TopBottomPanel::top("ruler-h").exact_height(RULER).frame(frame).show(ctx, |ui| {
        let r = ui.max_rect();
        let p = ui.painter_at(r);
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, BORDER));
        let x_lo = r.left() + RULER;                 // ticks start after the corner box
        let v_lo = view.s2w([x_lo * ppp, 0.0])[0] - origin[0];
        let v_hi = view.s2w([r.right() * ppp, 0.0])[0] - origin[0];
        let m1 = (v_hi / grid).ceil() as i64;
        let mut m = (v_lo / grid).floor() as i64;
        while m <= m1 {
            let val = m as f32 * grid;               // value shown — relative to the origin, so m==0 is ZERO
            let sx = view.w2s([origin[0] + val, 0.0])[0] / ppp;
            let (big, zero) = (m.rem_euclid(label_every) == 0, m == 0);
            m += 1;
            if sx < x_lo - 0.5 || sx > r.right() + 0.5 { continue; }
            let h = if zero { 12.0 } else if big { 9.0 } else { 5.0 };
            p.vline(sx, (r.bottom() - h)..=r.bottom(), Stroke::new(1.0, if zero { TEXT } else if big { MUTED } else { BORDER_2 }));
            if big { p.text(egui::pos2(sx + 2.5, r.top() + 1.0), Align2::LEFT_TOP, fmt_ruler(val, grid, dec), num_font.clone(), if zero { TEXT } else { MUTED }); }
        }
        if let Some(pt) = pointer { p.vline(pt.x, r.y_range(), Stroke::new(1.0, ACCENT)); }
        // corner box: drag sets the origin (snapped), double-click resets it
        let corner = egui::Rect::from_min_size(r.left_top(), egui::vec2(RULER, RULER));
        let resp = ui.interact(corner, ui.id().with("ruler-corner"), egui::Sense::click_and_drag());
        p.rect_filled(corner, Rounding::ZERO, BG);
        p.vline(corner.right() - 0.5, r.y_range(), Stroke::new(1.0, BORDER));
        let c = corner.center();
        p.line_segment([egui::pos2(c.x - 3.0, c.y), egui::pos2(c.x + 3.0, c.y)], Stroke::new(1.0, MUTED));
        p.line_segment([egui::pos2(c.x, c.y - 3.0), egui::pos2(c.x, c.y + 3.0)], Stroke::new(1.0, MUTED));
        if resp.double_clicked() { ops.push(Op::RulerOrigin(Some(reset))); ops.push(Op::RulerOrigin(None)); }
        else if resp.dragged() { if let Some(pp) = ctx.pointer_latest_pos() { ops.push(Op::RulerOrigin(Some(view.s2w([pp.x * ppp, pp.y * ppp])))); } }
        if resp.drag_stopped() { ops.push(Op::RulerOrigin(None)); }
    });

    // left (vertical) ruler — numbers rotated 90° (read upward), like Illustrator
    egui::SidePanel::left("ruler-v").exact_width(RULER).resizable(false).frame(frame).show(ctx, |ui| {
        let r = ui.max_rect();
        let p = ui.painter_at(r);
        p.vline(r.right() - 0.5, r.y_range(), Stroke::new(1.0, BORDER));
        let v_lo = view.s2w([0.0, r.top() * ppp])[1] - origin[1];
        let v_hi = view.s2w([0.0, r.bottom() * ppp])[1] - origin[1];
        let m1 = (v_hi / grid).ceil() as i64;
        let mut m = (v_lo / grid).floor() as i64;
        while m <= m1 {
            let val = m as f32 * grid;               // value shown — relative to the origin, so m==0 is ZERO
            let sy = view.w2s([0.0, origin[1] + val])[1] / ppp;
            let (big, zero) = (m.rem_euclid(label_every) == 0, m == 0);
            m += 1;
            if sy < r.top() - 0.5 || sy > r.bottom() + 0.5 { continue; }
            let w = if zero { 12.0 } else if big { 9.0 } else { 5.0 };
            p.hline((r.right() - w)..=r.right(), sy, Stroke::new(1.0, if zero { TEXT } else if big { MUTED } else { BORDER_2 }));
            if big {
                let col = if zero { TEXT } else { MUTED };
                let galley = p.layout_no_wrap(fmt_ruler(val, grid, dec), num_font.clone(), col);
                let mut ts = egui::epaint::TextShape::new(egui::pos2(r.left() + 2.0, sy + galley.size().x / 2.0), galley, col);
                ts.angle = -std::f32::consts::FRAC_PI_2;
                p.add(ts);
            }
        }
        if let Some(pt) = pointer { p.hline(r.x_range(), pt.y, Stroke::new(1.0, ACCENT)); }
    });
}

/// While the ruler zero-point is being dragged, a full-canvas DASHED crosshair (vertical = X, horizontal
/// = Y) marks where the new origin will land — drawn at the SNAPPED position, so the snap to a corner /
/// anchor / grid dot is visible and you can see exactly where (0,0) is going.
fn build_origin_crosshair(ctx: &egui::Context, view: View, ppp: f32, preview: Option<varos_core::geom::Pt>) {
    let Some(w) = preview else { return; };
    let s = view.w2s(w);
    let (sx, sy) = (s[0] / ppp, s[1] / ppp);
    let scr = ctx.screen_rect();
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("origin-cross")));
    let stroke = Stroke::new(1.0, ACCENT);
    for seg in egui::Shape::dashed_line(&[egui::pos2(sx, scr.top()), egui::pos2(sx, scr.bottom())], stroke, 5.0, 4.0) { p.add(seg); }
    for seg in egui::Shape::dashed_line(&[egui::pos2(scr.left(), sy), egui::pos2(scr.right(), sy)], stroke, 5.0, 4.0) { p.add(seg); }
}

/// The live measurement HUD — a small pill near the cursor showing the drag readout (X/Y position now;
/// W×H / angle later). Pure feedback on a foreground layer; no interaction, never blocks the canvas.
fn build_snap_hud(ctx: &egui::Context, view: View, ppp: f32, hud: &Option<(varos_core::geom::Pt, String)>) {
    let (wp, text) = match hud { Some(h) => h, None => return };
    let sp = view.w2s(*wp);
    let anchor = egui::pos2(sp[0] / ppp + 15.0, sp[1] / ppp - 26.0);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("snap-hud")));
    let font = FontId::proportional(12.0);
    let galley = p.layout_no_wrap(text.clone(), font.clone(), TEXT);
    let rect = egui::Rect::from_min_size(anchor, galley.size() + egui::vec2(14.0, 7.0));
    p.rect_filled(rect, Rounding::same(6.0), SOLID_PANEL);
    p.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0, BORDER));
    p.text(rect.center(), Align2::CENTER_CENTER, text, font, TEXT);
}

// ───────────────────────────── apply ─────────────────────────────

fn apply_ops(ed: &mut Editor, ops: Vec<Op>) {
    for op in ops {
        match op {
            Op::Tool(t) => ed.set_tool(t),
            Op::SetBBox(nx, ny, nw, nh, ax, ay) => ed.set_obj_bbox(nx, ny, nw, nh, ax, ay),
            Op::SetRot(d) => ed.set_obj_rotation(d),
            Op::SetOpacity(o) => ed.set_opacity(o.clamp(0.0, 1.0)),
            Op::SetStrokeW(w) => set_stroke_width(ed, w),
            Op::Paint(tg, c) => { ed.paint = tg; ed.apply_paint(c); }
            Op::Flip(h) => ed.flip(h),
            Op::Align(m) => ed.align(m),
            Op::Distribute(a) => ed.distribute(a),
            Op::AbActive(i) => ed.ab_set_active(i),
            Op::AbRect(i, x, y, w, h) => ed.ab_set_rect(i, x, y, w, h),
            Op::AbName(i, s) => ed.ab_rename(i, s),
            Op::AbColor(i, c) => ed.ab_set_color(i, c),
            Op::AbClip(i) => ed.ab_toggle_clip(i),
            Op::AbOrient(i) => ed.ab_orient(i),
            Op::AbAdd => ed.ab_add(),
            Op::AbDup(i) => ed.ab_duplicate(i),
            Op::AbDel(i) => ed.ab_delete(i),
            Op::AbCount(n) => ed.ab_set_count(n),
            Op::AbMoveArt(on) => ed.ab_set_move_art(on),
            Op::RulerOrigin(Some(p)) => { ed.doc.ruler_origin = ed.snap_origin(p); ed.origin_preview = Some(ed.doc.ruler_origin); }
            Op::RulerOrigin(None) => ed.origin_preview = None,
        }
    }
}

/// stroke width across the object selection — pub fields only (varos-core untouched).
fn set_stroke_width(ed: &mut Editor, w: f32) {
    let pids: Vec<u32> = ed.objsel.iter().copied().collect();
    if pids.is_empty() { ed.cur_sw = w.max(0.0); return; }
    ed.begin();
    for pid in pids { if let Some(pi) = ed.doc.pidx(pid) { ed.doc.paths[pi].stroke_width = w.max(0.0); } }
    ed.dirty = true; ed.commit();
}

/// Dev-only: composite the rail to a PNG so the icon rasterization can be eyeballed without the
/// native window. `varos.exe --dump-tool-icons <path>`.
pub fn dump_tool_icons(path: &str) {
    let icons = [IC_SELECT, IC_DIRECT, IC_PEN, IC_RECT, IC_ELLIPSE, IC_TRIANGLE, IC_EYE, IC_ROTATE, IC_OPACITY, IC_STROKEW, IC_LINK, IC_FLIPH, IC_FLIPV];
    let n = icons.len() as u32;
    let (pad, btn, gap, icon) = (7u32, 40u32, 4u32, 24u32);
    let w = btn + pad * 2;
    let h = pad * 2 + btn * n + gap * (n - 1);
    let panel = [0x1fu8, 0x1f, 0x22, 255];
    let accent = [0x0cu8, 0x8c, 0xe9, 255];
    let mut img = vec![0u8; (w * h * 4) as usize];
    for px in img.chunks_mut(4) { px.copy_from_slice(&panel); }
    for (i, svg) in icons.iter().enumerate() {
        let by = pad + i as u32 * (btn + gap);
        if i == 2 { for yy in by..by + btn { for xx in pad..pad + btn {
            let o = ((yy * w + xx) * 4) as usize; img[o..o + 4].copy_from_slice(&accent);
        }}}
        if let Some((rgba, iw, ih)) = crate::cursors::render_svg(&lucide(svg), icon, false) {
            let (ox, oy) = (pad + (btn - iw) / 2, by + (btn - ih) / 2);
            for yy in 0..ih { for xx in 0..iw {
                let si = ((yy * iw + xx) * 4) as usize; let a = rgba[si + 3] as u32;
                if a == 0 { continue; }
                let di = (((oy + yy) * w + (ox + xx)) * 4) as usize;
                for c in 0..3 { img[di + c] = ((rgba[si + c] as u32 * a + img[di + c] as u32 * (255 - a)) / 255) as u8; }
                img[di + 3] = 255;
            }}
        }
    }
    if let Some(im) = image::RgbaImage::from_raw(w, h, img) { let _ = im.save(path); }
}
