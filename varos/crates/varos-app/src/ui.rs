//! Native GPU UI — hand-painted chrome on OUR wgpu surface via `Renderer::render_ui` (egui shares our
//! Device/Queue; no second window). egui is only canvas + input + layout; every widget is drawn by us
//! so it matches the Figma, not egui's dev-tool defaults. Pieces so far: the left TOOL RAIL and the
//! right INSPECTOR DOCK (Transform / Appearance / Fill / Stroke). Solid panels, one light GPU shadow,
//! no glass. Panels read a per-frame snapshot of the editor and push deferred `Op`s, applied to
//! `&mut Editor` after layout (no IPC, no borrow fights). varos-core itself is untouched.

use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Stroke, StrokeKind};
use std::time::Instant;
use varos_core::editor::{AlignMode, DistAxis, Editor, PaintTarget, ToolKind};
use varos_core::geom::{Pt, Rgba, View};
use winit::event::WindowEvent;
use winit::window::Window;

// Stage 0b (BOX_SYSTEM_PLAN §6, ruling 4): the palette now comes from the LAW ramp — the warm black
// (R ≥ G ≥ B, tokens.rs = UI_VISION_MOCKUP's :root). The old cool-gray names alias their warm
// successors so this 4k-line file needs no body edits; Stage 4's re-cut chrome uses the law names.
use varos_app::shell::tokens::{
    ACCENT, ACCENT_HOVER, ACCENT_TINT, CLOSE_RED, FAINT, HOVER, INPUT_WELL, LINE as BORDER, LINE2 as BORDER_2, MUTED,
    NONE_RED, PANEL as SOLID_PANEL, R, RBOX, RCAP, ROW_HOVER, RULER_BG, SEAM, SURFACE as BG_SURFACE,
    SURFACE as SWATCH_WELL, TEXT, VOID_HOVER,
};

// Lucide icon path data (white-stroked at render time), same set as the web rail.
const IC_SELECT: &str = r#"<path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"/>"#;
const IC_DIRECT: &str = r#"<path d="M12.586 12.586 19 19"/><path d="M3.688 3.037a.497.497 0 0 0-.651.651l6.5 15.999a.501.501 0 0 0 .947-.062l1.569-6.083a2 2 0 0 1 1.448-1.479l6.124-1.579a.5.5 0 0 0 .063-.947z"/>"#;
const IC_PEN: &str = r#"<path d="M15.707 21.293a1 1 0 0 1-1.414 0l-1.586-1.586a1 1 0 0 1 0-1.414l5.586-5.586a1 1 0 0 1 1.414 0l1.586 1.586a1 1 0 0 1 0 1.414z"/><path d="m18 13-1.375-6.874a1 1 0 0 0-.746-.776L3.235 2.028a1 1 0 0 0-1.207 1.207L5.35 15.879a1 1 0 0 0 .776.746L13 18"/><path d="m2.3 2.3 7.286 7.286"/><circle cx="11" cy="11" r="2"/>"#;
const IC_RECT: &str = r#"<rect width="18" height="18" x="3" y="3" rx="2"/>"#;
const IC_ELLIPSE: &str = r#"<circle cx="12" cy="12" r="10"/>"#;
const IC_TRIANGLE: &str = r#"<path d="M13.73 4a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/>"#;
const IC_EYE: &str = r#"<path d="m12 9-8.414 8.414A2 2 0 0 0 3 18.828v1.344a2 2 0 0 1-.586 1.414A2 2 0 0 1 3.828 21h1.344a2 2 0 0 0 1.414-.586L15 12"/><path d="m18 9 .4.4a1 1 0 1 1-3 3l-3.8-3.8a1 1 0 1 1 3-3l.4.4 3.4-3.4a1 1 0 1 1 3 3z"/><path d="m2 22 .414-.414"/>"#;
// Layers-panel icons (Lucide): eye / eye-off · lock / lock-open · new-layer + · new-sublayer · trash
const IC_L_EYE: &str = r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"#;
const IC_L_EYEOFF: &str = r#"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>"#;
const IC_L_LOCK: &str =
    r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"#;
const IC_L_UNLOCK: &str =
    r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/>"#;
const IC_L_GROUP: &str = r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#;
const IC_L_TRASH: &str = r#"<path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><line x1="10" x2="10" y1="11" y2="17"/><line x1="14" x2="14" y1="11" y2="17"/>"#;
const IC_L_SEARCH: &str = r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#;

// field-label icons (Illustrator-style, gray): rotation · opacity · stroke weight
const IC_ROTATE: &str = r#"<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>"#;
// transform-tool rail icon: scale (move-diagonal)
const IC_SCALE: &str = r#"<path d="M19 13v6h-6"/><path d="M5 11V5h6"/><path d="m5 5 14 14"/>"#;
const IC_OPACITY: &str =
    r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a10 10 0 0 1 0 20z" fill="white" stroke="none"/>"#;
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
const IC_SEARCH: &str = IC_L_SEARCH; // identical glyph to the Layers search — one source, no dup literal
const IC_PLUS: &str = r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#;
const IC_X: &str = r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
const IC_MAGNET: &str = r#"<path d="m6 15-4-4 6.75-6.77a7.79 7.79 0 0 1 11 11L13 22l-4-4 6.39-6.36a2.14 2.14 0 0 0-3-3L6 15"/><path d="m5 8 4 4"/><path d="m12 15 4 4"/>"#;
// Artboard tool (Lucide "frame" — a bold # that reads clearly at 20px) · hexagon (polygon shape) ·
// portrait/landscape page · "fit in window" frame
const IC_ARTBOARD: &str = r#"<path d="M22 6H2"/><path d="M22 18H2"/><path d="M6 2v20"/><path d="M18 2v20"/>"#;
const IC_POLYGON: &str = r#"<path d="M21 16.05V7.95a2 2 0 0 0-1-1.73l-7-4.04a2 2 0 0 0-2 0l-7 4.04A2 2 0 0 0 3 7.95v8.1a2 2 0 0 0 1 1.73l7 4.04a2 2 0 0 0 2 0l7-4.04a2 2 0 0 0 1-1.73Z"/>"#;
const IC_PORTRAIT: &str = r#"<rect x="7" y="3" width="10" height="18" rx="1"/>"#;
const IC_LANDSCAPE: &str = r#"<rect x="3" y="7" width="18" height="10" rx="1"/>"#;
const IC_FIT: &str = r#"<path d="M8 3H5a2 2 0 0 0-2 2v3"/><path d="M21 8V5a2 2 0 0 0-2-2h-3"/><path d="M3 16v3a2 2 0 0 0 2 2h3"/><path d="M16 21h3a2 2 0 0 0 2-2v-3"/>"#;
// ⋮ on-canvas artboard menu — FILLED dots (own svg; lucide() forces stroke-only)
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
    SetRot(f32),
    SetOpacity(f32),
    SetStrokeW(f32),
    Paint(PaintTarget, Option<Rgba>),
    Recent(Rgba),            // remember a committed colour in the picker MRU strip
    PaintFocus(PaintTarget), // rail fill/stroke control: focus the target (X toggles)
    SwapColors,              // Shift+X
    DefaultPaint,            // D — white fill / black stroke
    OpenPicker(MTarget),     // double-click a swatch → open the Color Picker modal for it
    // ---- layers panel (node ids) — the SIMPLE panel (07-03 pivot) ----
    LayerSelectSet(Vec<u32>), // plain click / Shift-range: select these rows' art (replace)
    LayerToggle(u32),         // Ctrl+click a row: toggle its art in/out of the selection
    LayerEye(u32),
    LayerLock(u32),
    LayerRename(u32, String),
    LayerGroup,
    LayerDeleteSel,                                 // footer: Group the selection · Delete the selection
    LayerMove(Vec<u32>, u32, u8), // drag-drop: srcs (a multi-selection travels together), target, zone
    LayerDupMove(Vec<u32>, u32, u8), // Alt+drag: duplicate the rows' art into the target
    LayerMoveBoard(Vec<u32>, Option<usize>, usize), // cross-section drop: srcs, source board, target board
    Flip(bool),
    Align(AlignMode),
    Distribute(DistAxis),
    Bool(varos_core::boolean::BoolOp), // Pathfinder home + the Properties "Shape" mirror
    // ---- artboard ops (i = artboard index) ----
    AbActive(usize),
    AbRect(usize, Option<f32>, Option<f32>, Option<f32>, Option<f32>), // x,y,w,h (each optional)
    AbName(usize, String),
    AbColor(usize, Option<Rgba>), // None = transparent page
    AbClip(usize),                // toggle
    AbEye(usize),                 // board eye — the Layers section header (piece C)
    AbLock(usize),                // board padlock — the Layers section header (piece C)
    AbOrient(usize),              // swap w/h
    AbAdd,
    AbDup(usize),
    AbDel(usize),
    AbCount(usize),
    AbMoveArt(bool),
    RulerOrigin(Option<varos_core::geom::Pt>), // Some = set zero-point (snapped) + show crosshair; None = end drag
    GuidePreview(bool, varos_core::geom::Pt),  // ruler drag-out: (vertical, world) → live snapped guide preview
    GuideCommit,                               // drop the previewed guide into the document
}

/// A window action the custom title bar asks the host (winit) to perform.
pub enum WinAction {
    Minimize,
    ToggleMaximize,
    Close,
}

// ───────────────────────────── colour-picker modal state ─────────────────────────────

/// Where the modal's colour lands on OK.
#[derive(Clone, Copy)]
enum MTarget {
    Paint(PaintTarget),
    Ab(usize),
}

/// The spectrum-slider channel (the Photoshop/Illustrator radio mechanic): the selected channel becomes
/// the vertical slider, and the big field shows the remaining two axes.
#[derive(Clone, Copy, PartialEq)]
enum Chan {
    H,
    S,
    B,
    R,
    G,
    Bl,
}

/// Which geometry the modal shows: the field-plane Picker or the harmony Wheel.
#[derive(Clone, Copy, PartialEq)]
enum MTab {
    Picker,
    Wheel,
}

/// Colour-wheel harmony rule. Hue-rotation sets keep S,V; Mono varies brightness. (htmlcolorcodes/
/// colordesigner math.)
#[derive(Clone, Copy, PartialEq)]
enum Harmony {
    None,
    Complementary,
    Analogous,
    Split,
    Triadic,
    Tetradic,
    Square,
    Mono,
}
impl Harmony {
    const ALL: [(Harmony, &'static str); 8] = [
        (Harmony::None, "None"),
        (Harmony::Complementary, "Comp"),
        (Harmony::Analogous, "Analog"),
        (Harmony::Split, "Split"),
        (Harmony::Triadic, "Triad"),
        (Harmony::Tetradic, "Tetra"),
        (Harmony::Square, "Square"),
        (Harmony::Mono, "Mono"),
    ];
    /// Hue offsets (degrees) for the non-base members — empty for None/Mono.
    fn offsets(self) -> &'static [f32] {
        match self {
            Harmony::Complementary => &[180.0],
            Harmony::Analogous => &[-30.0, 30.0],
            Harmony::Split => &[150.0, 210.0],
            Harmony::Triadic => &[120.0, 240.0],
            Harmony::Tetradic => &[60.0, 180.0, 240.0],
            Harmony::Square => &[90.0, 180.0, 270.0],
            _ => &[],
        }
    }
}

/// Every colour in a harmony set (base first). Hue modes rotate hue at fixed S,V; Mono steps brightness.
fn harmony_set(h: Harmony, base: [f32; 3]) -> Vec<[f32; 3]> {
    match h {
        Harmony::None => vec![base],
        Harmony::Mono => {
            [1.0, 0.78, 0.56, 0.36].iter().map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)]).collect()
        }
        _ => {
            let mut v = vec![base];
            for off in h.offsets() {
                v.push([(base[0] + off / 360.0).rem_euclid(1.0), base[1], base[2]]);
            }
            v
        }
    }
}

/// The professional Color Picker modal (opened by double-clicking any colour swatch).
/// Live HSVA is the single source of truth while open; OK commits once, Cancel discards.
struct ColorModal {
    target: MTarget,
    orig: Option<Rgba>,
    hsva: [f32; 4],
    chan: Chan,
    tab: MTab,
    harmony: Harmony,
}

struct TopIcons {
    menu: Option<egui::TextureHandle>, // min/max/close are painted glyphs now (see `winctl`), not textures
    search: Option<egui::TextureHandle>,
    plus: Option<egui::TextureHandle>,
    x: Option<egui::TextureHandle>,
    magnet: Option<egui::TextureHandle>,
}

// ───────────────────────────── layers panel snapshot ─────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum LKind {
    Board, // an ARTBOARD section header (derived from doc.artboards, not a scene node)
    Layer,
    Group,
    Path,
}

/// One drawable shape in a row's thumbnail — its outline rings (normalised into the row's COMBINED
/// bbox, 0..1, Y down) plus its own paint. A leaf has one; a Layer/Group stacks all its art (Ahmed:
/// the container thumbnail is a real mini-preview of everything inside, like Illustrator).
struct ThumbShape {
    rings: Vec<Vec<Pt>>,
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
}

/// One rendered row of the Layers panel (a flattened, display-ordered view of the scene tree,
/// SECTIONED by artboard — Ahmed 07-06: the panel splits by page, like Figma; membership is derived
/// from geometry via `node_boards`, mirror rows for straddlers, floaters loose at the bottom).
struct LRow {
    id: u32, // node id; Board headers use u32::MAX - board index (never collides with node ids)
    depth: u16,
    kind: LKind,
    sec: u32, // section: the artboard index, u32::MAX = the floater strip (salts egui ids for mirrors)
    name: String,
    hidden: bool,
    locked: bool, // OWN flags (drive the toggle icons)
    eff_hidden: bool,
    eff_locked: bool, // cascaded (drive dimming + "forced" look)
    has_children: bool,
    collapsed: bool,
    selected: bool,         // any of the row's art is selected on canvas
    full_sel: bool,         // ALL of the row's art is selected (children read this off their parent)
    drag_sel: bool,         // top-most fully-selected row — the unit a multi-row drag picks up
    active: bool,           // the active (target) layer / the active artboard on Board headers
    thumb: Vec<ThumbShape>, // real mini-preview, back→front; empty = no art (blank box)
}

/// Auto-name for a leaf path (Illustrator angle-bracket style) unless the user renamed it.
fn path_auto_name(p: &varos_core::model::Path) -> String {
    p.name.clone().unwrap_or_else(|| "<Path>".into())
}

/// Flatten the scene tree into display rows (roots front-first, pre-order; collapsed subtrees skipped).
/// `search` (lowercased) keeps only matching rows + their ancestors. Thumbs are unit-square outlines.
/// Board-header sentinel row id (u32::MAX - board index) — node ids are small sequential, never near MAX.
fn board_row_id(bi: usize) -> u32 {
    u32::MAX - bi as u32
}
fn build_layer_rows(ed: &Editor, collapsed: &std::collections::HashSet<u32>, search: &str) -> Vec<LRow> {
    use varos_core::model::NodeKind;
    let q = search.trim().to_lowercase();
    let mut rows: Vec<LRow> = Vec::new();
    let mut parent: Vec<Option<usize>> = Vec::new(); // parallel: each row's parent ROW index

    #[allow(clippy::too_many_arguments)] // recursive row emitter: tree cursor + section + two out-params
    fn walk(
        ed: &Editor,
        nid: u32,
        depth: u16,
        sec: u32,
        sf: (bool, bool), // the SECTION's board (hidden, locked) — dims/forces this instance's rows
        par: Option<usize>,
        collapsed: &std::collections::HashSet<u32>,
        rows: &mut Vec<LRow>,
        parent: &mut Vec<Option<usize>>,
    ) {
        let Some(n) = ed.doc.node(nid) else { return };
        let anc_hidden = || {
            let mut c = n.parent;
            while let Some(i) = c {
                let Some(x) = ed.doc.node(i) else { break };
                if x.hidden {
                    return true;
                }
                c = x.parent;
            }
            false
        };
        let anc_locked = || {
            let mut c = n.parent;
            while let Some(i) = c {
                let Some(x) = ed.doc.node(i) else { break };
                if x.locked {
                    return true;
                }
                c = x.parent;
            }
            false
        };
        let (kind, name) = match n.kind {
            NodeKind::Layer => (LKind::Layer, n.name.clone()),
            NodeKind::Group => (LKind::Group, if n.name.is_empty() { "<Group>".into() } else { n.name.clone() }),
            NodeKind::Path(pid) => (
                LKind::Path,
                ed.doc.pidx(pid).map(|pi| path_auto_name(&ed.doc.paths[pi])).unwrap_or_else(|| "<Path>".into()),
            ),
        };
        // thumbnail = every path under this node, composited in z-order (back→front) into one bbox —
        // a leaf shows itself; a Layer/Group shows a true preview of its contents.
        let mut paths = ed.doc.node_paths(nid);
        paths.sort_by_key(|pid| ed.doc.pidx(*pid).unwrap_or(usize::MAX)); // back→front z
        let thumb = thumb_shapes(ed, &paths);
        let full_sel = !paths.is_empty() && paths.iter().all(|p| ed.objsel.contains(p));
        // the top-most fully-selected row is the multi-drag unit (its parent isn't fully selected)
        let drag_sel = full_sel && !par.map(|pi| rows[pi].full_sel).unwrap_or(false);
        rows.push(LRow {
            id: nid,
            depth,
            kind,
            sec,
            name,
            hidden: n.hidden,
            locked: n.locked,
            eff_hidden: n.hidden || anc_hidden() || sf.0,
            eff_locked: n.locked || anc_locked() || sf.1,
            has_children: !n.children.is_empty(),
            collapsed: collapsed.contains(&nid),
            selected: paths.iter().any(|p| ed.objsel.contains(p)),
            full_sel,
            drag_sel,
            active: nid == ed.doc.active_layer,
            thumb,
        });
        parent.push(par);
        let me = rows.len() - 1;
        if !collapsed.contains(&nid) {
            for &c in &n.children {
                walk(ed, c, depth + 1, sec, sf, Some(me), collapsed, rows, parent);
            }
        }
    }
    // The TOP-LEVEL items (the implicit root Layer stays a model-only container — 07-03 pivot; a
    // non-Layer legacy root counts as an item itself).
    let mut top: Vec<u32> = Vec::new();
    for &r in &ed.doc.roots {
        match ed.doc.node(r) {
            Some(n) if matches!(n.kind, NodeKind::Layer) => top.extend(n.children.iter().copied()),
            _ => top.push(r),
        }
    }
    // SECTIONS BY ARTBOARD (Ahmed 07-06, Figma-style): one header per board; a top-level item lists
    // under every board its subtree stands on (mirror rows for straddlers — same object, same state);
    // items on no board float LOOSE at the bottom, under no header — visibly outside every page and
    // outside export.
    let memb: Vec<(u32, Vec<usize>)> = top.iter().map(|&nid| (nid, ed.doc.node_boards(nid))).collect();
    for (bi, ab) in ed.doc.artboards.iter().enumerate() {
        let hid = board_row_id(bi);
        let members: Vec<u32> = memb.iter().filter(|(_, bs)| bs.contains(&bi)).map(|(n, _)| *n).collect();
        rows.push(LRow {
            id: hid,
            depth: 0,
            kind: LKind::Board,
            sec: bi as u32,
            name: ab.name.clone(),
            hidden: ab.hidden,
            locked: ab.locked,
            eff_hidden: ab.hidden,
            eff_locked: ab.locked,
            has_children: !members.is_empty(),
            collapsed: collapsed.contains(&hid),
            selected: false,
            full_sel: false,
            drag_sel: false,
            active: bi == ed.doc.active,
            thumb: vec![],
        });
        parent.push(None);
        let me = rows.len() - 1;
        if !collapsed.contains(&hid) {
            for nid in members {
                walk(ed, nid, 1, bi as u32, (ab.hidden, ab.locked), Some(me), collapsed, &mut rows, &mut parent);
            }
        }
    }
    for (nid, bs) in &memb {
        if bs.is_empty() {
            walk(ed, *nid, 0, u32::MAX, (false, false), None, collapsed, &mut rows, &mut parent);
        }
    }

    if q.is_empty() {
        return rows;
    }
    // keep matches + all their ancestors (so hierarchy stays readable)
    let mut keep = vec![false; rows.len()];
    for i in 0..rows.len() {
        if rows[i].name.to_lowercase().contains(&q) {
            keep[i] = true;
            let mut p = parent[i];
            while let Some(pi) = p {
                keep[pi] = true;
                p = parent[pi];
            }
        }
    }
    rows.into_iter().zip(keep).filter(|(_, k)| *k).map(|(r, _)| r).collect()
}
/// One path's raw thumbnail ingredients before bbox-fitting: `(rings, fill, stroke)`.
type RawThumb = (Vec<Vec<Pt>>, Option<Rgba>, Option<Rgba>);
/// Build a row's thumbnail: gather every path (already in back→front z order), collect its outline
/// rings + paint in pixel space, then fit the ONE combined bbox to the unit square (Y down, shorter
/// axis centred) so the composite preview keeps each shape's real position, size and colour.
fn thumb_shapes(ed: &Editor, pids_zorder: &[u32]) -> Vec<ThumbShape> {
    let mut raw: Vec<RawThumb> = Vec::new();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &pid in pids_zorder {
        let Some(pi) = ed.doc.pidx(pid) else { continue };
        let p = &ed.doc.paths[pi];
        let mut rings = vec![ed.doc.outline_px(pi, 1.0)];
        for h in &p.holes {
            rings.push(varos_core::model::Document::ring_px(h, true, 1.0));
        }
        for r in &rings {
            for q in r {
                x0 = x0.min(q[0]);
                y0 = y0.min(q[1]);
                x1 = x1.max(q[0]);
                y1 = y1.max(q[1]);
            }
        }
        raw.push((rings, p.fill.solid(), p.stroke.solid())); // Paint → the UI snapshot's Option<Rgba>
    }
    if raw.is_empty() {
        return vec![];
    }
    let (w, h) = ((x1 - x0).max(1e-3), (y1 - y0).max(1e-3));
    let s = 1.0 / w.max(h);
    let (ox, oy) = ((1.0 - w * s) * 0.5, (1.0 - h * s) * 0.5); // centre the shorter axis
    raw.into_iter()
        .map(|(rings, fill, stroke)| ThumbShape {
            rings: rings
                .into_iter()
                .map(|r| r.into_iter().map(|q| [ox + (q[0] - x0) * s, oy + (q[1] - y0) * s]).collect())
                .collect(),
            fill,
            stroke,
        })
        .collect()
}

/// Read-only snapshot of the editor for this frame's panels.
struct Snap {
    tool: ToolKind,
    name: String,
    sel: bool,
    drawing: bool, // Pen mid-draft (an open path is active) — drives the "Drawing path…" status (P9)
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rot: f32,
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    sw: f32,
    opacity: f32,
    paint: PaintTarget, // which target has focus (the rail control + X)
    recent: Vec<Rgba>,
    doc_colors: Vec<Rgba>, // the picker's swatch strips (MRU + derived document scan)
}
impl Snap {
    fn read(ed: &Editor) -> Self {
        let n = ed.objsel.len();
        let (sel, x, y, w, h) = match ed.obj_bbox() {
            Some((x0, y0, x1, y1)) if n > 0 => (true, x0, y0, x1 - x0, y1 - y0),
            _ => (false, 0.0, 0.0, 0.0, 0.0),
        };
        // fill/stroke/weight/opacity follow the EFFECTIVE paint selection (object sel, a Direct path-level
        // selection, or a selected anchor's path) — not objsel alone, so the Direct tool shows real colours.
        let repr = ed.repr_path();
        let (fill, stroke, sw, opacity) = match repr {
            Some(pi) => {
                let p = &ed.doc.paths[pi];
                (p.fill.solid(), p.stroke.solid(), p.stroke_width, p.opacity)
            }
            None => (ed.cur_fill, ed.cur_stroke, ed.cur_sw, 1.0),
        };
        let name = if n == 0 {
            match repr {
                Some(pi) => ed.doc.paths[pi].name.clone().unwrap_or_else(|| "Path".into()),
                None => "No selection".into(),
            }
        } else if n == 1 {
            repr.and_then(|pi| ed.doc.paths[pi].name.clone()).unwrap_or_else(|| "Path".into())
        } else {
            format!("{n} objects")
        };
        Snap {
            tool: ed.tool,
            name,
            sel,
            drawing: ed.tool == ToolKind::Pen && ed.active.is_some(),
            x,
            y,
            w,
            h,
            rot: ed.obj_angle.to_degrees(),
            fill,
            stroke,
            sw,
            opacity,
            paint: ed.paint,
            recent: ed.recent_colors.clone(),
            doc_colors: ed.document_colors(),
        }
    }
}

/// Read-only snapshot of the ACTIVE artboard for the artboard property panel.
struct AbSnap {
    count: usize,
    active: usize,
    name: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: Option<Rgba>,
    clip: bool,
    move_art: bool,
}
impl AbSnap {
    fn read(ed: &Editor) -> Self {
        let count = ed.doc.artboards.len();
        let active = if count == 0 { 0 } else { ed.doc.active.min(count - 1) };
        let ab = ed.doc.active_artboard();
        AbSnap {
            count,
            active,
            name: ab.map(|a| a.name.clone()).unwrap_or_default(),
            x: ab.map(|a| a.x).unwrap_or(0.0),
            y: ab.map(|a| a.y).unwrap_or(0.0),
            w: ab.map(|a| a.w).unwrap_or(0.0),
            h: ab.map(|a| a.h).unwrap_or(0.0),
            color: ab.and_then(|a| a.page_color),
            clip: ab.map(|a| a.clip).unwrap_or(false),
            move_art: ed.doc.move_art_with_ab,
        }
    }
}

/// One artboard's on-canvas label info (for the name + ⋮ chrome painted over the board).
struct AbInfo {
    i: usize,
    name: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    transparent: bool,
    clip: bool,
    hidden: bool,
}
fn ab_infos(ed: &Editor) -> Vec<AbInfo> {
    ed.doc
        .artboards
        .iter()
        .enumerate()
        .map(|(i, a)| AbInfo {
            i,
            name: a.name.clone(),
            x: a.x,
            y: a.y,
            w: a.w,
            h: a.h,
            transparent: a.page_color.is_none(),
            clip: a.clip,
            hidden: a.hidden,
        })
        .collect()
}

struct ToolBtn {
    kind: ToolKind,
    tip: &'static str,
    tex: Option<egui::TextureHandle>,
    group_end: bool,
}

pub struct Ui {
    ctx: egui::Context,
    state: egui_winit::State,
    pub repaint: bool,
    tools: Vec<ToolBtn>,    // rail singletons: Object · Direct · Artboard · Pen · Eyedropper
    shapes: Vec<ToolBtn>,   // the shape tools, collapsed into one rail slot (right-click → flyout)
    shape_active: ToolKind, // which shape the shapes slot currently represents
    ic_rotate: Option<egui::TextureHandle>,
    ic_opacity: Option<egui::TextureHandle>,
    ic_strokew: Option<egui::TextureHandle>,
    ic_link: Option<egui::TextureHandle>,
    ic_fliph: Option<egui::TextureHandle>,
    ic_flipv: Option<egui::TextureHandle>,
    ic_portrait: Option<egui::TextureHandle>,
    ic_landscape: Option<egui::TextureHandle>,
    ic_fit: Option<egui::TextureHandle>,
    align_icons: [Option<egui::TextureHandle>; 8], // align L/CH/R · T/M/B · distribute H/V
    cursor: egui::CursorIcon, // this frame's egui cursor (read from FullOutput, not post-frame state)
    refpt: (f32, f32),        // transform reference point (ax, ay each in {0, .5, 1})
    lock: bool,               // constrain W/H proportions
    ab_lock: bool,            // constrain artboard W/H proportions
    ab_name_edit: Option<(usize, String)>, // inline rename in progress (artboard index + buffer)
    pub fit_request: Option<usize>, // an artboard asked to be fit in the window (host applies it)
    top: TopIcons,
    pub win_action: Option<WinAction>, // a window control was clicked this frame (host acts on it)
    show_rail: bool,
    show_dock: bool,
    tabs: Vec<String>,
    tab_active: usize,
    logo: Option<egui::TextureHandle>,
    splash_start: Option<Instant>,   // startup loading screen; None once it has faded out
    last_splash: bool,               // did this frame draw the splash (host renders it transparent)?
    color_modal: Option<ColorModal>, // the Color Picker modal, when open
    layer_icons: LayerIcons,
    lay_collapsed: std::collections::HashSet<u32>, // collapsed container node ids (UI-only)
    lay_search: String,
    lay_rename: Option<(u32, String)>, // inline rename in progress (node id + buffer)
    lay_drag: Option<(u32, u32)>,      // Layers row being dragged: (node id, SOURCE section) — the
    // section decides same-section reorder vs cross-board move
    lay_anchor: Option<(u32, u32)>, // Shift-range anchor: (node id, SECTION) — mirror rows share an
    // id across sections, so the id alone picks the wrong appearance
    // ── Stage 4: the BOX TREE hosts the whole workspace (BOX_SYSTEM_PLAN §4) ──
    shell: varos_app::shell::ShellState, // the box tree; panel bodies render through the host hook
    board_hole: Option<egui::Rect>,      // the Board pane's interior (logical pts) — the wgpu canvas hole
    pub board_px: Option<egui::Rect>,    // same, in PHYSICAL px — main.rs fits the view to it
}

/// The Layers-panel icon set (rasterized Lucide, white).
struct LayerIcons {
    eye: Option<egui::TextureHandle>,
    eye_off: Option<egui::TextureHandle>,
    lock: Option<egui::TextureHandle>,
    unlock: Option<egui::TextureHandle>,
    grp: Option<egui::TextureHandle>,
    trash: Option<egui::TextureHandle>,
    search: Option<egui::TextureHandle>,
}

/// Rasterize a Lucide icon (white) to an egui texture once.
fn load_icon(ctx: &egui::Context, name: &str, svg_inner: &str) -> Option<egui::TextureHandle> {
    crate::cursors::render_svg(&lucide(svg_inner), 96, false).map(|(rgba, w, h)| {
        ctx.load_texture(
            name,
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba),
            egui::TextureOptions::LINEAR,
        )
    })
}

impl Ui {
    pub fn new(window: &Window) -> Self {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        install_style(&ctx);
        // rail singletons — Artboard sits with Selection + Direct Selection (Ahmed), then Pen, Eyedropper.
        let defs: [(ToolKind, &str, &str, bool); 7] = [
            (ToolKind::Object, IC_SELECT, "Selection (V)", false),
            (ToolKind::Direct, IC_DIRECT, "Direct Selection (A)", false),
            (ToolKind::Artboard, IC_ARTBOARD, "Artboard (Shift+O)", true), // ends the selection group
            (ToolKind::Pen, IC_PEN, "Pen (P)", true),                      // ends the pen group
            (ToolKind::Rotate, IC_ROTATE, "Rotate (R)", false),            // transform group ↓
            (ToolKind::Scale, IC_SCALE, "Scale (S)", true),                // ends the transform group
            (ToolKind::Eyedropper, IC_EYE, "Eyedropper (I)", false),
        ];
        let tools = defs
            .iter()
            .enumerate()
            .map(|(i, (kind, svg, tip, grp))| ToolBtn {
                kind: *kind,
                tip,
                tex: load_icon(&ctx, &format!("ic-{i}"), svg),
                group_end: *grp,
            })
            .collect();
        // shape tools collapse into ONE rail slot: left-click uses the current shape, right-click flyouts all four.
        let shape_defs: [(ToolKind, &str, &str); 4] = [
            (ToolKind::Rect, IC_RECT, "Rectangle (M)"),
            (ToolKind::Ellipse, IC_ELLIPSE, "Ellipse (L)"),
            (ToolKind::Triangle, IC_TRIANGLE, "Triangle"),
            (ToolKind::Polygon, IC_POLYGON, "Polygon"),
        ];
        let shapes = shape_defs
            .iter()
            .enumerate()
            .map(|(i, (kind, svg, tip))| ToolBtn {
                kind: *kind,
                tip,
                tex: load_icon(&ctx, &format!("ic-shape{i}"), svg),
                group_end: false,
            })
            .collect();
        let ic_rotate = load_icon(&ctx, "lbl-rot", IC_ROTATE);
        let ic_opacity = load_icon(&ctx, "lbl-op", IC_OPACITY);
        let ic_strokew = load_icon(&ctx, "lbl-sw", IC_STROKEW);
        let ic_link = load_icon(&ctx, "lbl-link", IC_LINK);
        let ic_fliph = load_icon(&ctx, "lbl-fh", IC_FLIPH);
        let ic_flipv = load_icon(&ctx, "lbl-fv", IC_FLIPV);
        let ic_portrait = load_icon(&ctx, "lbl-portrait", IC_PORTRAIT);
        let ic_landscape = load_icon(&ctx, "lbl-landscape", IC_LANDSCAPE);
        let ic_fit = load_icon(&ctx, "lbl-fit", IC_FIT);
        let align_icons = [
            load_icon(&ctx, "al-l", IC_AL_L),
            load_icon(&ctx, "al-ch", IC_AL_CH),
            load_icon(&ctx, "al-r", IC_AL_R),
            load_icon(&ctx, "al-t", IC_AL_T),
            load_icon(&ctx, "al-m", IC_AL_M),
            load_icon(&ctx, "al-b", IC_AL_B),
            load_icon(&ctx, "dist-h", IC_DIST_H),
            load_icon(&ctx, "dist-v", IC_DIST_V),
        ];
        let top = TopIcons {
            menu: load_icon(&ctx, "tb-menu", IC_MENU),
            search: load_icon(&ctx, "tb-search", IC_SEARCH),
            plus: load_icon(&ctx, "tb-plus", IC_PLUS),
            x: load_icon(&ctx, "tb-x", IC_X),
            magnet: load_icon(&ctx, "tb-magnet", IC_MAGNET),
        };
        let logo = image::load_from_memory(include_bytes!("../icon.png")).ok().map(|im| {
            let rgba = im.into_rgba8();
            let (w, h) = rgba.dimensions();
            ctx.load_texture(
                "logo",
                egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw()),
                egui::TextureOptions::LINEAR,
            )
        });
        let layer_icons = LayerIcons {
            eye: load_icon(&ctx, "l-eye", IC_L_EYE),
            eye_off: load_icon(&ctx, "l-eyeoff", IC_L_EYEOFF),
            lock: load_icon(&ctx, "l-lock", IC_L_LOCK),
            unlock: load_icon(&ctx, "l-unlock", IC_L_UNLOCK),
            grp: load_icon(&ctx, "l-group", IC_L_GROUP),
            trash: load_icon(&ctx, "l-trash", IC_L_TRASH),
            search: load_icon(&ctx, "l-search", IC_L_SEARCH),
        };
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, window, None, None, None);
        Ui {
            ctx,
            state,
            repaint: false,
            tools,
            shapes,
            shape_active: ToolKind::Rect,
            ic_rotate,
            ic_opacity,
            ic_strokew,
            ic_link,
            ic_fliph,
            ic_flipv,
            ic_portrait,
            ic_landscape,
            ic_fit,
            align_icons,
            cursor: egui::CursorIcon::Default,
            refpt: (0.0, 0.0),
            lock: false,
            ab_lock: false,
            ab_name_edit: None,
            fit_request: None,
            top,
            win_action: None,
            show_rail: true,
            show_dock: true,
            tabs: vec!["Untitled-1".into()],
            tab_active: 0,
            logo,
            splash_start: Some(Instant::now()),
            last_splash: false,
            color_modal: None,
            layer_icons,
            lay_collapsed: std::collections::HashSet::new(),
            lay_search: String::new(),
            lay_rename: None,
            lay_drag: None,
            lay_anchor: None,
            shell: varos_app::shell::ShellState::standard(),
            board_hole: None,
            board_px: None,
        }
    }

    /// Feed a window event to egui. Returns true if egui consumed it (so the canvas should NOT).
    pub fn on_event(&mut self, window: &Window, ev: &WindowEvent) -> bool {
        self.state.on_window_event(window, ev).consumed
    }
    /// Is the pointer over chrome (a box, ruler, bar, hand, menu)? The canvas owns everything else.
    /// Stage 4: the box tree paints the whole workspace on the BACKGROUND layer, so the old
    /// `is_pointer_over_egui` (panels-only) is wrong — instead: any floating layer wins; on the
    /// background, everything is chrome EXCEPT the Board pane's interior (the wgpu canvas hole).
    pub fn wants_pointer(&self) -> bool {
        if self.ctx.egui_is_using_pointer() {
            return true; // a live widget interaction (field scrub, splitter drag, open menu)
        }
        let Some(pos) = self.ctx.input(|i| i.pointer.interact_pos()) else {
            return false;
        };
        match self.ctx.layer_id_at(pos) {
            None => false,
            Some(l) if l.order == egui::Order::Background => !self.board_hole.is_some_and(|b| b.contains(pos)),
            Some(_) => true, // hands / menus / modal / splash float above the tree
        }
    }
    /// Is a text field actually focused? Only THEN should keys go to egui instead of canvas shortcuts.
    /// (Gate canvas shortcuts on this, NOT on egui's generic "consumed" — otherwise an Arabic-layout
    /// keypress, which egui receives as a Text event, would swallow V/A/P and the rest.)
    pub fn wants_keyboard(&self) -> bool {
        self.ctx.egui_wants_keyboard_input()
    }
    /// Is the Color Picker modal open? (canvas shortcuts must be fully gated off while it is)
    pub fn modal_open(&self) -> bool {
        self.color_modal.is_some()
    }
    /// The 🔖 slice: the host names tab 0 after the open document ("name" / "name *").
    pub fn set_doc_tab(&mut self, name: String) {
        if self.tabs.is_empty() {
            self.tabs.push(name);
        } else {
            self.tabs[0] = name;
        }
    }
    /// The native cursor the UI chrome wants this frame — egui's icon mapped onto our Win32 set.
    /// Box-seam resizes (egui_tiles splitters) and number-field scrubs get their arrows; everything
    /// else on chrome is the plain arrow (Illustrator shows an arrow over buttons, not a hand).
    pub fn chrome_ck(&self) -> crate::cursors::CK {
        use crate::cursors::CK;
        use egui::CursorIcon as C;
        match self.cursor {
            C::ResizeHorizontal | C::ResizeColumn | C::ResizeEast | C::ResizeWest => CK::ResizeH,
            C::ResizeVertical | C::ResizeRow | C::ResizeNorth | C::ResizeSouth => CK::ResizeV,
            C::ResizeNeSw | C::ResizeNorthEast | C::ResizeSouthWest => CK::ResizeNE,
            C::ResizeNwSe | C::ResizeNorthWest | C::ResizeSouthEast => CK::ResizeNW,
            C::Grabbing => CK::Grab,
            _ => CK::Select,
        }
    }
    /// (Re)start the startup splash timer — call right before revealing the window.
    pub fn start_splash(&mut self) {
        self.splash_start = Some(Instant::now());
    }
    /// Did the last `run` build the splash? (host renders it on a transparent surface, over the desktop)
    pub fn splashing(&self) -> bool {
        self.last_splash
    }

    /// Build + lay out the panels; user changes are applied straight to the editor. Returns egui's
    /// tessellated output for `Renderer::render_ui`.
    pub fn run(
        &mut self,
        window: &Window,
        ed: &mut Editor,
        ppp: f32,
        view: View,
        maximized: bool,
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor) {
        let input = self.state.take_egui_input(window);
        let snap = Snap::read(ed);
        let absnap = AbSnap::read(ed);
        let abs = ab_infos(ed);
        let snap_hud = ed.snap_hud.clone();
        let show_rulers = ed.show_rulers;
        let ruler_origin = ed.doc.ruler_origin;
        let ruler_reset = ed.doc.active_artboard().map(|a| [a.x, a.y]).unwrap_or([0.0, 0.0]);
        let ruler_grid = ed.adaptive_grid_step(); // tick on the SAME base-5 lattice as the dot grid
        let origin_preview = ed.origin_preview; // dashed crosshair while dragging the ruler zero-point
        let tools = &self.tools;
        let shapes = &self.shapes;
        let icons = DockIcons {
            rotate: &self.ic_rotate,
            opacity: &self.ic_opacity,
            strokew: &self.ic_strokew,
            link: &self.ic_link,
            fliph: &self.ic_fliph,
            flipv: &self.ic_flipv,
            align: &self.align_icons,
        };
        let ab_icons = AbIcons {
            link: &self.ic_link,
            portrait: &self.ic_portrait,
            landscape: &self.ic_landscape,
            fit: &self.ic_fit,
        };
        let ic_fit = &self.ic_fit; // the status strip's Fit control shares the artboard panel's icon
        let shell = &mut self.shell; // Stage 4: the box tree hosting the whole workspace
        let prev_hole = self.board_hole; // last frame's canvas hole (the seam underlay paints around it)
        let mut new_hole: Option<egui::Rect> = None;
        let top = &self.top;
        // layers snapshot (built before the closure — like Snap/AbSnap)
        let layer_rows = build_layer_rows(ed, &self.lay_collapsed, &self.lay_search);
        let layer_icons = &self.layer_icons;
        let mut lay_search = std::mem::take(&mut self.lay_search);
        let mut lay_rename = std::mem::take(&mut self.lay_rename);
        let mut lay_collapsed = std::mem::take(&mut self.lay_collapsed);
        let mut lay_drag = self.lay_drag;
        let mut lay_anchor = self.lay_anchor;
        let mut ops: Vec<Op> = Vec::new();
        let mut refpt = self.refpt;
        let mut lock = self.lock;
        let mut ab_lock = self.ab_lock;
        let mut ab_name_edit = std::mem::take(&mut self.ab_name_edit);
        let mut fit_request: Option<usize> = None;
        let mut shape_active = self.shape_active;
        let mut win_action = None;
        let mut show_rail = self.show_rail;
        let mut show_dock = self.show_dock;
        let mut snap_cfg = ed.doc.snap; // the magnet menu edits this; written back after layout (mode flag)
        let mut tabs = std::mem::take(&mut self.tabs);
        let mut tab_active = self.tab_active;
        let splash = self.splash_start.map(|t| t.elapsed().as_secs_f32());
        let splashing = splash.is_some_and(|e| e < SPLASH_DUR);
        let logo = &self.logo;
        let mut color_modal = std::mem::take(&mut self.color_modal);
        // egui 0.34 removed Context::run — run_ui hands the pass's root Ui (panels now show() on it)
        let out = self.ctx.run_ui(input, |root| {
            let ctx = root.ctx().clone();
            let ctx = &ctx;
            if splashing {
                if let Some(e) = splash {
                    build_splash(ctx, e, logo);
                } // only the floating card
            } else {
                build_topbar(
                    root,
                    top,
                    &mut *shell,
                    &mut win_action,
                    &mut tabs,
                    &mut tab_active,
                    &mut show_rail,
                    &mut show_dock,
                    &mut snap_cfg,
                    maximized,
                );
                build_statusbar(root, absnap.active, absnap.count, view.zoom, ic_fit, &mut fit_request);
                // ── Stage 4: the `.mid` region IS the box tree (BOX_SYSTEM_PLAN §4). The Board pane is
                // a HOLE showing the wgpu canvas below; the seam underlay paints the void around last
                // frame's hole (one-frame lag on resize, healed by the request_repaint below). ──
                {
                    let mid = root.available_rect_before_wrap();
                    paint_void_underlay(root.painter(), mid, prev_hole);
                    let mut host = |panel: varos_app::shell::PanelId, ui: &mut egui::Ui| -> bool {
                        use varos_app::shell::PanelId as P;
                        match panel {
                            P::Board => {
                                let rect = ui.max_rect();
                                let p = ui.painter().clone();
                                // a normal box on the void: hairline border, rounded corners patched
                                // with seam so the scene never pokes past the radius
                                corner_voids(&p, rect);
                                p.rect_stroke(
                                    rect,
                                    CornerRadius::same(RBOX),
                                    Stroke::new(1.0, BORDER),
                                    StrokeKind::Inside,
                                );
                                let mut inner = rect.shrink(1.0);
                                if show_rulers {
                                    board_rulers(ui, inner, view, ppp, ruler_grid, ruler_origin, ruler_reset, &mut ops);
                                    inner = egui::Rect::from_min_max(inner.min + egui::vec2(RULER, RULER), inner.max);
                                }
                                if show_rail {
                                    board_rail(ui.ctx(), inner, tools, shapes, &mut shape_active, &snap, &mut ops);
                                }
                                if show_dock {
                                    board_ctlbar(
                                        ui.ctx(),
                                        inner,
                                        &snap,
                                        &absnap,
                                        &icons,
                                        ic_fit,
                                        &mut ops,
                                        &mut fit_request,
                                    );
                                }
                                new_hole = Some(inner);
                                true
                            }
                            P::Properties => {
                                if snap.tool == ToolKind::Artboard {
                                    panel_artboard(ui, &absnap, &ab_icons, &mut ab_lock, &mut ops, &mut fit_request);
                                } else {
                                    panel_properties(ui, &snap, &icons, &mut refpt, &mut lock, &mut ops);
                                }
                                true
                            }
                            P::Layers => {
                                panel_layers(
                                    ui,
                                    &layer_rows,
                                    layer_icons,
                                    &mut lay_search,
                                    &mut lay_rename,
                                    &mut lay_collapsed,
                                    &mut lay_drag,
                                    &mut lay_anchor,
                                    &mut ops,
                                );
                                true
                            }
                            P::Align => {
                                panel_align(ui, &icons, &mut ops);
                                true
                            }
                            P::Pathfinder => {
                                panel_pathfinder(ui, &mut ops);
                                true
                            }
                            _ => false,
                        }
                    };
                    // the boxes FLOAT in the void (Ahmed 07-07): an outer breath of HALF the
                    // box-to-box seam on the sides/top; the bottom breath lives INSIDE the taller
                    // statusbar so its text centres in the visual strip
                    let g = varos_app::shell::tokens::SEAM_GAP * 0.5;
                    let tree_rect =
                        egui::Rect::from_min_max(mid.min + egui::vec2(g, g), egui::pos2(mid.right() - g, mid.bottom()));
                    root.scope_builder(egui::UiBuilder::new().max_rect(tree_rect), |ui| shell.ui_hosted(ui, &mut host));
                }
                // on-canvas overlays are CONFINED to the Board hole (Ahmed 07-07): page chrome, snap
                // HUD and origin crosshair clip/cull at its edges instead of roaming the window
                let hole = new_hole.unwrap_or_else(|| ctx.content_rect());
                build_ab_chrome(
                    ctx,
                    view,
                    ppp,
                    hole,
                    &abs,
                    absnap.active,
                    snap.tool == ToolKind::Artboard,
                    absnap.count,
                    &mut ops,
                    &mut ab_name_edit,
                    &mut fit_request,
                );
                build_snap_hud(ctx, view, ppp, hole, &snap_hud);
                build_origin_crosshair(ctx, view, ppp, hole, origin_preview);
                build_color_modal(ctx, &mut color_modal, &snap, &mut ops); // over everything
            }
        });
        self.color_modal = color_modal;
        self.last_splash = splashing;
        if let Some(e) = splash {
            if e >= SPLASH_DUR {
                self.splash_start = None;
            }
        }
        self.refpt = refpt;
        self.lock = lock;
        self.ab_lock = ab_lock;
        self.ab_name_edit = ab_name_edit;
        self.lay_search = lay_search;
        self.lay_rename = lay_rename;
        self.lay_collapsed = lay_collapsed;
        self.lay_drag = lay_drag;
        self.lay_anchor = lay_anchor;
        self.shape_active = shape_active;
        if fit_request.is_some() {
            self.fit_request = fit_request;
        }
        self.win_action = win_action;
        self.show_rail = show_rail;
        self.show_dock = show_dock;
        self.tabs = tabs;
        self.tab_active = tab_active;
        ed.doc.snap = snap_cfg; // commit the magnet-menu toggles (a non-undoable mode flag)
                                // OpenPicker is a UI op (it opens the modal, seeded from the target's colour) — intercept it here
        ops.retain(|op| {
            if let Op::OpenPicker(t) = op {
                let seed = match *t {
                    MTarget::Paint(PaintTarget::Fill) => snap.fill,
                    MTarget::Paint(PaintTarget::Stroke) => snap.stroke,
                    MTarget::Ab(i) => ed.doc.artboards.get(i).and_then(|a| a.page_color),
                };
                let base = seed.unwrap_or([0.85, 0.85, 0.87, 1.0]);
                let h = rgb_to_hsv(base);
                self.color_modal = Some(ColorModal {
                    target: *t,
                    orig: seed,
                    hsva: [h[0], h[1], h[2], base[3]],
                    chan: Chan::H,
                    tab: MTab::Picker,
                    harmony: Harmony::None,
                });
                false
            } else {
                true
            }
        });
        apply_ops(ed, ops);
        self.cursor = out.platform_output.cursor_icon; // read the REAL cursor from this frame's output
        self.state.handle_platform_output(window, out.platform_output);
        // Stage 4: publish the canvas hole. Logical for the pointer test, physical for main.rs's view
        // fits. A changed hole (box resized/dragged) repaints once more so the underlay catches up.
        if new_hole != self.board_hole {
            self.ctx.request_repaint();
        }
        self.board_hole = new_hole;
        self.board_px = new_hole.map(|r| {
            egui::Rect::from_min_max(
                (r.min.to_vec2() * out.pixels_per_point).to_pos2(),
                (r.max.to_vec2() * out.pixels_per_point).to_pos2(),
            )
        });
        self.repaint = out.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.repaint_delay.is_zero())
            || splash.is_some_and(|e| e < SPLASH_DUR); // keep animating the splash
        let jobs = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let sz = window.inner_size();
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [sz.width, sz.height],
            pixels_per_point: out.pixels_per_point,
        };
        (jobs, out.textures_delta, screen)
    }
}

// ───────────────────────────── fonts / style / frame ─────────────────────────────

fn lucide(inner: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" \
             stroke=\"#ffffff\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{inner}</svg>"
    )
}

fn install_fonts(ctx: &egui::Context) {
    // §3.4: ui = Segoe UI Variable Text (Win11) → Segoe UI; mono = Cascadia Code → Consolas.
    let mut f = egui::FontDefinitions::default();
    let first = |names: &[&str]| names.iter().find_map(|n| std::fs::read(format!("C:/Windows/Fonts/{n}")).ok());
    if let Some(b) = first(&["SegUIVar.ttf", "segoeuivf.ttf", "segoeui.ttf"]) {
        f.font_data.insert("ui".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(b)));
        f.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "ui".to_owned());
    }
    if let Some(b) = first(&["CascadiaCode.ttf", "CASCADIA.TTF", "consola.ttf"]) {
        f.font_data.insert("mono".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(b)));
        f.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "mono".to_owned());
    }
    ctx.set_fonts(f);
}

fn install_style(ctx: &egui::Context) {
    use egui::{FontFamily, TextStyle};
    ctx.set_theme(egui::Theme::Dark);
    // Stage 4: the shell law is the base (warm visuals + INSTANT + thin overlay scrollbars +
    // tight seam grab) — the app only adds its text ramp on top.
    varos_app::shell::tokens::apply(ctx);
    let mut s = (*ctx.style_of(egui::Theme::Dark)).clone();
    s.text_styles = [
        (TextStyle::Heading, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ]
    .into();
    ctx.set_style_of(egui::Theme::Dark, s.clone());
    ctx.set_style_of(egui::Theme::Light, s);
}

// ───────────────────────────── hand-rolled dropdown menus ─────────────────────────────
// egui 0.35 removed the old memory-popup API (toggle_popup/is_popup_open/popup_below_widget). We
// hand-roll the replacement on Area — same look as the 0.27 popups (our Visuals drove those) and
// our close rules: open = a temp bool at `id`; closes on Escape or a primary press outside menu+anchor.

fn menu_open(ui: &egui::Ui, id: egui::Id) -> bool {
    ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false))
}
fn menu_set(ui: &egui::Ui, id: egui::Id, open: bool) {
    ui.data_mut(|d| d.insert_temp(id, open));
}
fn menu_toggle(ui: &egui::Ui, id: egui::Id) {
    let v = !menu_open(ui, id);
    menu_set(ui, id, v);
}

/// The dropdown body anchored below `anchor` (only when open). `flush = Some(bar_bottom)` renders
/// it as an EXTENSION of the app bar (Ahmed 07-07): seam fill, square top corners hanging straight
/// off the bar's bottom edge, and the shared edge erased — bar and menu read as ONE dark surface.
fn menu_below(
    ui: &egui::Ui,
    id: egui::Id,
    anchor: &egui::Response,
    flush: Option<f32>,
    add: impl FnOnce(&mut egui::Ui),
) {
    if !menu_open(ui, id) {
        return;
    }
    let ctx = ui.ctx().clone();
    // Illustrator-crisp chrome (Ahmed 07-07): sharp MENU_R corners, vertical padding only — the
    // rows run edge-to-edge so their hover strips touch the border. Flush menus keep the seam
    // fill + square top so they read as extensions of the app bar.
    let frame = egui::Frame {
        fill: if flush.is_some() { SEAM } else { SOLID_PANEL },
        stroke: Stroke::new(1.0, BORDER),
        corner_radius: if flush.is_some() {
            CornerRadius { nw: 0, ne: 0, sw: MENU_R, se: MENU_R }
        } else {
            CornerRadius::same(MENU_R)
        },
        inner_margin: Margin::symmetric(0, MENU_PAD_V),
        ..Default::default()
    };
    let pos = match flush {
        Some(y) => egui::pos2(anchor.rect.left(), y),
        None => anchor.rect.left_bottom() + egui::vec2(0.0, 4.0),
    };
    let out = egui::Area::new(id.with("menu")).order(egui::Order::Foreground).fixed_pos(pos).constrain(true).show(
        &ctx,
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0; // contiguous rows — separators bring their own air
            let r = frame.show(ui, |ui| add(ui)).response.rect;
            if flush.is_some() {
                // erase the top border segment — the menu melts into the bar, 100% one colour
                ui.painter().hline(r.left() + 1.0..=r.right() - 1.0, r.top() + 0.5, Stroke::new(1.5, SEAM));
            }
            r
        },
    );
    let rect = out.inner;
    let close = ctx.input(|i| i.key_pressed(egui::Key::Escape))
        || ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer.interact_pos().is_some_and(|p| !rect.expand(4.0).contains(p) && !anchor.rect.contains(p))
        });
    if close {
        ctx.data_mut(|d| d.insert_temp(id, false));
    }
}

fn panel_frame(margin: i8) -> egui::Frame {
    egui::Frame {
        fill: SOLID_PANEL,
        corner_radius: CornerRadius::same(RBOX), // boxes = 8 (law); 14 was the dead floating-shell look

        stroke: Stroke::new(1.0, BORDER),
        // egui 0.31+ counts the 1px stroke as padding — compensate so content sits EXACTLY where it
        // did on 0.27 (the zero-perceptible-difference bar).
        inner_margin: Margin::same(margin - 1),
        ..Default::default()
    }
}

// ───────────────────────────── shared primitives ─────────────────────────────

/// A field's prefix: a compact letter (X/Y/W/H) or a small gray icon (rotation/opacity/stroke).
enum Lab<'a> {
    Letter(&'a str),
    Icon(Option<&'a egui::TextureHandle>),
}

const UV01: fn() -> egui::Rect = || egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

/// Number field. A dim label column, then a rounded box holding the value CENTERED. The WHOLE box is
/// one interactive target via `ui.interact` (the exact mechanism the tool-rail buttons use): drag it to
/// scrub (↔ cursor), single-click to type (value pre-selected). Returns Some(new) on change.
/// (Blender ‹ › steppers come back once the core drag/type is confirmed.)
#[allow(clippy::too_many_arguments)] // hand-painted widget: geometry + behaviour knobs, split deferred with ui.rs
fn num_field(
    ui: &mut egui::Ui,
    w: f32,
    lab: Lab,
    tip: &str,
    value: f32,
    decimals: usize,
    _step: f32,
    speed: f32,
    range: std::ops::RangeInclusive<f32>,
) -> Option<f32> {
    let mut out = None;
    let (lo, hi) = (*range.start(), *range.end());
    let (row, _) = ui.allocate_exact_size(egui::vec2(w, 25.0), egui::Sense::hover());
    let p = ui.painter().clone();
    let labw = 22.0;
    match lab {
        Lab::Letter(s) => {
            p.text(
                egui::pos2(row.left() + labw - 5.0, row.center().y),
                Align2::RIGHT_CENTER,
                s,
                FontId::proportional(11.5),
                FAINT,
            );
        }
        Lab::Icon(Some(t)) => {
            p.image(
                t.id(),
                egui::Rect::from_center_size(
                    egui::pos2(row.left() + labw - 11.0, row.center().y),
                    egui::vec2(14.0, 14.0),
                ),
                UV01(),
                MUTED,
            );
        }
        Lab::Icon(None) => {}
    }
    let bx = egui::Rect::from_min_max(egui::pos2(row.left() + labw + 2.0, row.top()), row.max);
    let id = ui.make_persistent_id(("numf", tip));
    let r5 = CornerRadius::same(R);
    // 'just entered' flag (set on click) survives the one frame until the TextEdit claims focus.
    let just = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    let editing = just || ui.memory(|m| m.has_focus(id));
    if editing {
        p.rect(bx, r5, INPUT_WELL, Stroke::new(1.0, ACCENT), StrokeKind::Middle); // dark "input well"
        let mut buf = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| format!("{value:.decimals$}"));
        let te = ui.put(
            bx.shrink2(egui::vec2(8.0, 3.0)),
            egui::TextEdit::singleline(&mut buf)
                .id(id)
                .frame(egui::Frame::NONE)
                .font(egui::FontId::proportional(13.0))
                .text_color(TEXT),
        );
        if just {
            te.request_focus();
            ui.data_mut(|d| d.remove::<bool>(id));
        }
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
        // arrow nudge while focused: ↑/↓ = ±1 · Shift+↑/↓ = ±10 (Illustrator) — applies live
        let dv = ui.input_mut(|i| {
            let mut d = 0.0;
            if i.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowUp) {
                d += 10.0;
            }
            if i.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowDown) {
                d -= 10.0;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                d += 1.0;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                d -= 1.0;
            }
            d
        });
        if dv != 0.0 {
            let nv = (buf.trim().parse::<f32>().unwrap_or(value) + dv).clamp(lo, hi);
            let s = format!("{nv:.decimals$}");
            // keep the text selected so the next keystroke still replaces (same as click-to-type)
            let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(s.chars().count()),
            )));
            st.store(ui.ctx(), id);
            ui.data_mut(|d| d.insert_temp(id, s));
            out = Some(nv);
        }
        if te.lost_focus() {
            if let Ok(v) = buf.trim().parse::<f32>() {
                out = Some(v.clamp(lo, hi));
            }
            ui.data_mut(|d| {
                d.remove::<String>(id);
                d.remove::<bool>(id);
            });
        }
    } else {
        let resp = ui.interact(bx, id.with("box"), egui::Sense::click_and_drag());
        let hot = resp.hovered() || resp.dragged();
        if hot {
            p.rect(bx, r5, HOVER, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        } else {
            p.rect_filled(bx, r5, BG_SURFACE);
        }
        p.text(bx.center(), Align2::CENTER_CENTER, format!("{value:.decimals$}"), FontId::proportional(13.0), TEXT);
        if hot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if resp.dragged() {
            let dx = resp.drag_delta().x;
            if dx != 0.0 {
                out = Some((value + dx * speed).clamp(lo, hi));
            }
        }
        if resp.clicked() {
            let s = format!("{value:.decimals$}");
            // pre-select all so typing replaces the value (Blender behavior)
            let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            let n = s.chars().count();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(n),
            )));
            st.store(ui.ctx(), id);
            ui.data_mut(|d| {
                d.insert_temp(id, s);
                d.insert_temp(id, true);
            });
            ui.memory_mut(|m| m.request_focus(id));
        }
        if !tip.is_empty() {
            resp.on_hover_text(tip);
        }
    }
    out
}

/// Tiny hand-painted glyph button (e.g. the clear-paint ×). Returns true on click.
fn mini_btn(ui: &mut egui::Ui, glyph: &str, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(14.0), MUTED);
    resp.on_hover_text(tip).clicked()
}

/// The 9-point transform reference widget (3×3 dots). Click a dot to set the reference (ax, ay).
fn refpoint(ui: &mut egui::Ui, sz: f32, refpt: &mut (f32, f32)) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(sz, sz), egui::Sense::click());
    let p = ui.painter();
    let inset = 7.0;
    let area = egui::Rect::from_min_max(
        egui::pos2(rect.left() + inset, rect.top() + inset),
        egui::pos2(rect.right() - inset, rect.bottom() - inset),
    );
    for ay in 0..3 {
        for ax in 0..3 {
            let (fx, fy) = (ax as f32 * 0.5, ay as f32 * 0.5);
            let c = egui::pos2(area.left() + fx * area.width(), area.top() + fy * area.height());
            if (refpt.0 - fx).abs() < 0.01 && (refpt.1 - fy).abs() < 0.01 {
                p.circle_filled(c, 2.6, ACCENT);
            } else {
                p.circle_stroke(c, 2.0, Stroke::new(1.0, MUTED));
            }
        }
    }
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
    if on {
        ui.painter().rect_filled(rect, CornerRadius::same(R), ACCENT);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    if let Some(t) = tex {
        ui.painter().image(
            t.id(),
            egui::Rect::from_center_size(rect.center(), egui::vec2(15.0, 15.0)),
            UV01(),
            if on { Color32::WHITE } else { MUTED },
        );
    }
    resp.on_hover_text(tip).clicked()
}

/// Small icon action button (e.g. flip). White on hover.
fn icon_btn(ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 24.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    if let Some(t) = tex {
        ui.painter().image(
            t.id(),
            egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)),
            UV01(),
            if resp.hovered() { Color32::WHITE } else { MUTED },
        );
    }
    resp.on_hover_text(tip).clicked()
}

/// A short, full-width hairline divider.
fn hsep(ui: &mut egui::Ui, w: f32) {
    ui.add_space(9.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.left()..=rect.right(), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(9.0);
}

fn hex_of(c: Rgba) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8
    )
}

// ── HSV ↔ RGB (all channels 0..1) — the picker keeps live HSV as its source of truth (Decision 2) ──
fn rgb_to_hsv(c: Rgba) -> [f32; 3] {
    let (r, g, b) = (c[0], c[1], c[2]);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let s = if mx <= 0.0 { 0.0 } else { d / mx };
    let h = if d <= 1e-6 {
        0.0
    } else {
        let h = if mx == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if mx == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h / 6.0
    };
    [h, s, mx]
}
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h6 = h.rem_euclid(1.0) * 6.0;
    let i = h6.floor();
    let f = h6 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}
fn hsv_c32(h: f32, s: f32, v: f32) -> Color32 {
    let c = hsv_to_rgb(h, s, v);
    Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
}
fn rgba_c32a(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}
fn parse_hex(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_start_matches('#');
    let n = |a: &str| u8::from_str_radix(a, 16).ok().map(|v| v as f32 / 255.0);
    match s.len() {
        3 => {
            let e: Vec<String> = s.chars().map(|c| format!("{c}{c}")).collect();
            Some([n(&e[0])?, n(&e[1])?, n(&e[2])?, 1.0])
        }
        6 => Some([n(&s[0..2])?, n(&s[2..4])?, n(&s[4..6])?, 1.0]),
        8 => Some([n(&s[0..2])?, n(&s[2..4])?, n(&s[4..6])?, n(&s[6..8])?]),
        _ => None,
    }
}
/// Grey checkerboard behind translucent colours.
fn checker(p: &egui::Painter, r: egui::Rect, sq: f32) {
    p.rect_filled(r, CornerRadius::ZERO, Color32::from_gray(90));
    let (cols, rows) = ((r.width() / sq).ceil() as i32, (r.height() / sq).ceil() as i32);
    for gy in 0..rows {
        for gx in 0..cols {
            if (gx + gy) % 2 == 0 {
                continue;
            }
            let (x, y) = (r.left() + gx as f32 * sq, r.top() + gy as f32 * sq);
            let cell = egui::Rect::from_min_max(
                egui::pos2(x, y),
                egui::pos2((x + sq).min(r.right()), (y + sq).min(r.bottom())),
            );
            p.rect_filled(cell, CornerRadius::ZERO, Color32::from_gray(140));
        }
    }
}
fn rail_thumb(p: &egui::Painter, r: egui::Rect, y: f32) {
    let t = egui::Rect::from_min_max(egui::pos2(r.left() - 2.0, y - 2.5), egui::pos2(r.right() + 2.0, y + 2.5));
    p.rect(t, CornerRadius::same(2), Color32::TRANSPARENT, Stroke::new(2.0, Color32::WHITE), StrokeKind::Middle);
    p.rect_stroke(
        t.expand(1.0),
        CornerRadius::same(3),
        Stroke::new(1.0, Color32::from_black_alpha(90)),
        StrokeKind::Middle,
    );
}

/// A labelled row of small clickable swatches (hand-painted; checker under translucent colours).
/// Returns the clicked colour. Hidden entirely when the list is empty.
fn swatch_strip(ui: &mut egui::Ui, label: &str, colors: &[Rgba]) -> Option<Rgba> {
    if colors.is_empty() {
        return None;
    }
    let mut out = None;
    ui.add_space(2.0);
    ui.label(RichText::new(label).color(FAINT).size(10.5));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for c in colors {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::click());
            let round = CornerRadius::same(3);
            if c[3] < 0.999 {
                checker(&ui.painter_at(r), r, 4.0);
            }
            ui.painter().rect_filled(r, round, rgba_c32a(*c));
            ui.painter().rect_stroke(
                r,
                round,
                Stroke::new(1.0, if resp.hovered() { Color32::WHITE } else { BORDER_2 }),
                StrokeKind::Middle,
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                out = Some(*c);
            }
        }
    });
    out
}

/// Fill / Stroke row: a hand-painted swatch (double-click → the Color Picker modal), + hex + clear ×.
fn paint_row(ui: &mut egui::Ui, target: PaintTarget, color: Option<Rgba>, ops: &mut Vec<Op>) {
    ui.horizontal(|ui| {
        // A18: name the target so the two rows read as Fill / Stroke at a glance (fixed column → swatches align)
        let label = match target {
            PaintTarget::Fill => "Fill",
            PaintTarget::Stroke => "Stroke",
        };
        let (lr, _) = ui.allocate_exact_size(egui::vec2(44.0, 18.0), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(lr.left(), lr.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.5),
            MUTED,
        );
        let (sw, resp) = ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::click());
        let round = CornerRadius::same(4);
        let p = ui.painter();
        match color {
            Some(c) => {
                if c[3] < 0.999 {
                    checker(&ui.painter_at(sw), sw, 5.0);
                }
                p.rect_filled(sw, round, rgba_c32a(c));
            }
            None => {
                p.rect_filled(sw, round, SWATCH_WELL);
                p.line_segment(
                    [sw.left_bottom() + egui::vec2(2.0, -2.0), sw.right_top() + egui::vec2(-2.0, 2.0)],
                    Stroke::new(1.6, NONE_RED),
                );
            } // None = red slash
        }
        p.rect_stroke(sw, round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        // single click = focus the target (X toggles) · DOUBLE-click = open the Color Picker modal
        if resp.clicked() {
            ops.push(Op::PaintFocus(target));
        }
        if resp.double_clicked() {
            ops.push(Op::OpenPicker(MTarget::Paint(target)));
        }
        resp.on_hover_text("Double-click to edit the colour");
        ui.add_space(8.0);
        ui.label(RichText::new(color.map(hex_of).unwrap_or_else(|| "None".into())).color(TEXT).monospace().size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if mini_btn(ui, "×", "No paint") {
                ops.push(Op::Paint(target, None));
            }
        });
    });
}

// ───────────────────────────── the Color Picker modal ─────────────────────────────

/// Field-plane / spectrum-slider axis positions (all 0..1) for the current colour under a channel radio.
fn pick_get(chan: Chan, hsva: [f32; 4]) -> (f32, f32, f32) {
    let c = hsv_to_rgb(hsva[0], hsva[1], hsva[2]);
    match chan {
        Chan::H => (hsva[1], hsva[2], hsva[0]),
        Chan::S => (hsva[0], hsva[2], hsva[1]),
        Chan::B => (hsva[0], hsva[1], hsva[2]),
        Chan::R => (c[2], c[1], c[0]), // field x=Blue · y=Green · slider=Red (Photoshop convention)
        Chan::G => (c[2], c[0], c[1]), // x=Blue · y=Red
        Chan::Bl => (c[0], c[1], c[2]), // x=Red  · y=Green
    }
}
/// Write plane/slider positions back into the live HSVA (grey RGB results keep the current hue).
fn pick_set(chan: Chan, hsva: &mut [f32; 4], px: f32, py: f32, sl: f32) {
    match chan {
        Chan::H => {
            hsva[0] = sl.min(0.9999);
            hsva[1] = px;
            hsva[2] = py;
        }
        Chan::S => {
            hsva[0] = px.min(0.9999);
            hsva[1] = sl;
            hsva[2] = py;
        }
        Chan::B => {
            hsva[0] = px.min(0.9999);
            hsva[1] = py;
            hsva[2] = sl;
        }
        _ => {
            let rgb = match chan {
                Chan::R => [sl, py, px],
                Chan::G => [py, sl, px],
                _ => [px, py, sl],
            };
            let h = rgb_to_hsv([rgb[0], rgb[1], rgb[2], 1.0]);
            if h[1] > 0.001 {
                hsva[0] = h[0];
            }
            hsva[1] = h[1];
            hsva[2] = h[2];
        }
    }
}
/// Colour of a field/slider sample point (px, py, sl each 0..1) under a channel radio.
fn pick_rgb(chan: Chan, px: f32, py: f32, sl: f32) -> [f32; 3] {
    match chan {
        Chan::H => hsv_to_rgb(sl, px, py),
        Chan::S => hsv_to_rgb(px, sl, py),
        Chan::B => hsv_to_rgb(px, py, sl),
        Chan::R => [sl, py, px],
        Chan::G => [py, sl, px],
        Chan::Bl => [px, py, sl],
    }
}
fn rgb_c32(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
}

/// A hand-painted radio dot (the channel selectors). Returns true on click.
fn radio_dot(ui: &mut egui::Ui, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(15.0, 25.0), egui::Sense::click());
    let c = r.center();
    ui.painter().circle_stroke(
        c,
        5.0,
        Stroke::new(
            1.2,
            if on {
                ACCENT
            } else if resp.hovered() {
                TEXT
            } else {
                MUTED
            },
        ),
    );
    if on {
        ui.painter().circle_filled(c, 2.6, ACCENT);
    }
    resp.clicked()
}

/// Screen position of an H×S point on a wheel of radius `rad` centred at `c` (red at 12 o'clock,
/// hue increasing clockwise; saturation = radius).
fn wheel_pos(c: egui::Pos2, rad: f32, h: f32, s: f32) -> egui::Pos2 {
    let a = h * std::f32::consts::TAU;
    egui::pos2(c.x + a.sin() * s * rad, c.y - a.cos() * s * rad)
}

/// The Wheel view: an H×S disc (brightness-tinted) + brightness rail + alpha rail + harmony rule pills +
/// clickable result chips. Draggable base handle; ghost handles for the linked harmony members. Edits the
/// same live `hsva`. Returns nothing — mutates the modal in place.
fn build_wheel(ui: &mut egui::Ui, m: &mut ColorModal) {
    let v = m.hsva[2];
    ui.horizontal_top(|ui| {
        // ── the H×S disc (a fan mesh from a grey centre to full-sat rim; radial interp is EXACT for HSV) ──
        let d = 236.0;
        let (dr, dresp) = ui.allocate_exact_size(egui::vec2(d, d), egui::Sense::click_and_drag());
        let c = dr.center();
        let rad = d * 0.5 - 1.0;
        let n = 96u32;
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(c, hsv_c32(0.0, 0.0, v)); // centre = grey at the current brightness
        for i in 0..=n {
            let h = i as f32 / n as f32;
            mesh.colored_vertex(wheel_pos(c, rad, h, 1.0), hsv_c32(h, 1.0, v));
        }
        for i in 1..=n {
            mesh.add_triangle(0, i, i + 1);
        }
        ui.painter_at(dr).add(egui::Shape::mesh(mesh));
        ui.painter().circle_stroke(c, rad + 0.5, Stroke::new(1.0, BORDER_2));
        // harmony ghost handles (linked, display-only) then the draggable base handle on top
        let set = harmony_set(m.harmony, [m.hsva[0], m.hsva[1], m.hsva[2]]);
        for gc in set.iter().skip(1) {
            let p = wheel_pos(c, rad, gc[0], gc[1]);
            ui.painter().circle_filled(p, 4.5, rgb_c32(hsv_to_rgb(gc[0], gc[1], gc[2])));
            ui.painter().circle_stroke(p, 4.5, Stroke::new(1.5, Color32::from_white_alpha(200)));
        }
        let bp = wheel_pos(c, rad, m.hsva[0], m.hsva[1]);
        ui.painter().circle_stroke(bp, 6.5, Stroke::new(2.0, Color32::WHITE));
        ui.painter().circle_stroke(bp, 7.5, Stroke::new(1.0, Color32::from_black_alpha(120)));
        if dresp.is_pointer_button_down_on() || dresp.dragged() {
            if let Some(p) = dresp.interact_pointer_pos() {
                let (dx, dy) = (p.x - c.x, p.y - c.y);
                m.hsva[0] = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
                m.hsva[1] = ((dx * dx + dy * dy).sqrt() / rad).clamp(0.0, 1.0);
                if m.hsva[0] >= 1.0 {
                    m.hsva[0] = 0.9999;
                }
            }
        }
        // ── brightness rail (black → full colour) ──
        let (br, brr) = ui.allocate_exact_size(egui::vec2(16.0, d), egui::Sense::click_and_drag());
        let mut bm = egui::Mesh::default();
        let top = hsv_c32(m.hsva[0], m.hsva[1], 1.0);
        bm.colored_vertex(br.left_top(), top);
        bm.colored_vertex(br.right_top(), top);
        bm.colored_vertex(br.right_bottom(), Color32::BLACK);
        bm.colored_vertex(br.left_bottom(), Color32::BLACK);
        bm.add_triangle(0, 1, 2);
        bm.add_triangle(0, 2, 3);
        ui.painter_at(br).add(egui::Shape::mesh(bm));
        ui.painter().rect_stroke(br, CornerRadius::ZERO, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        rail_thumb(ui.painter(), br, br.top() + (1.0 - v) * d);
        if brr.is_pointer_button_down_on() || brr.dragged() {
            if let Some(p) = brr.interact_pointer_pos() {
                m.hsva[2] = (1.0 - (p.y - br.top()) / d).clamp(0.0, 1.0);
            }
        }
        // ── alpha rail ──
        let (ar, arr) = ui.allocate_exact_size(egui::vec2(16.0, d), egui::Sense::click_and_drag());
        checker(&ui.painter_at(ar), ar, 6.0);
        let solid = hsv_c32(m.hsva[0], m.hsva[1], m.hsva[2]);
        let mut am = egui::Mesh::default();
        am.colored_vertex(ar.left_top(), solid);
        am.colored_vertex(ar.right_top(), solid);
        am.colored_vertex(ar.right_bottom(), Color32::TRANSPARENT);
        am.colored_vertex(ar.left_bottom(), Color32::TRANSPARENT);
        am.add_triangle(0, 1, 2);
        am.add_triangle(0, 2, 3);
        ui.painter_at(ar).add(egui::Shape::mesh(am));
        ui.painter().rect_stroke(ar, CornerRadius::ZERO, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        rail_thumb(ui.painter(), ar, ar.top() + (1.0 - m.hsva[3]) * d);
        if arr.is_pointer_button_down_on() || arr.dragged() {
            if let Some(p) = arr.interact_pointer_pos() {
                m.hsva[3] = (1.0 - (p.y - ar.top()) / d).clamp(0.0, 1.0);
            }
        }
    });
    ui.add_space(4.0);
    // ── harmony rule pills ──
    ui.label(RichText::new("HARMONY").color(FAINT).size(10.5));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for (rule, label) in Harmony::ALL {
            let on = m.harmony == rule;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(52.0, 22.0), egui::Sense::click());
            let rr = CornerRadius::same(R);
            if on {
                ui.painter().rect_filled(r, rr, ACCENT);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, rr, HOVER);
            } else {
                ui.painter().rect_stroke(r, rr, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
            }
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                if on { Color32::WHITE } else { TEXT },
            );
            if resp.clicked() {
                m.harmony = rule;
            }
        }
    });
    // ── harmony result chips (click to adopt as the current colour) ──
    if m.harmony != Harmony::None {
        ui.add_space(3.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 4.0);
            for gc in harmony_set(m.harmony, [m.hsva[0], m.hsva[1], m.hsva[2]]) {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(30.0, 22.0), egui::Sense::click());
                ui.painter().rect_filled(r, CornerRadius::same(4), rgb_c32(hsv_to_rgb(gc[0], gc[1], gc[2])));
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(4),
                    Stroke::new(1.0, if resp.hovered() { Color32::WHITE } else { BORDER_2 }),
                    StrokeKind::Middle,
                );
                let rgb = hsv_to_rgb(gc[0], gc[1], gc[2]);
                resp.clone().on_hover_text(hex_of([rgb[0], rgb[1], rgb[2], 1.0]));
                if resp.clicked() {
                    m.hsva[0] = gc[0];
                    m.hsva[1] = gc[1];
                    m.hsva[2] = gc[2];
                }
            }
        });
    }
}

/// A hand-painted dialog button. `primary` = accent OK.
fn dlg_btn(ui: &mut egui::Ui, label: &str, primary: bool, w: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    let rr = CornerRadius::same(R);
    if primary {
        ui.painter().rect_filled(r, rr, if resp.hovered() { ACCENT_HOVER } else { ACCENT });
    } else {
        ui.painter().rect_filled(r, rr, if resp.hovered() { HOVER } else { BG_SURFACE });
        ui.painter().rect_stroke(r, rr, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    }
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.5),
        if primary { Color32::WHITE } else { TEXT },
    );
    resp.clicked()
}

/// The professional Color Picker dialog — a FLOATING palette (Ahmed, 07-02: no scrim, the canvas stays
/// fully usable beside it; drag any empty spot to move it, and it remembers its position). The field
/// plane + spectrum slider are channel-radio driven (the Photoshop/Illustrator mechanic); alpha rail;
/// split new/current preview (click the current half to restore); hex (Enter/blur) + A% + HSB/RGB fields;
/// RECENT/DOCUMENT strips. OK commits ONE op + the MRU push · Cancel/Esc discards · Enter = OK when no
/// field is focused (the host reserves Esc/Enter for the dialog while it is open).
fn build_color_modal(ctx: &egui::Context, modal: &mut Option<ColorModal>, snap: &Snap, ops: &mut Vec<Op>) {
    if modal.is_none() {
        return;
    }
    let screen = ctx.content_rect();
    let (mut ok, mut cancel) = (false, false);
    {
        let m = modal.as_mut().unwrap();
        let dw = 508.0;
        let pos = egui::pos2((screen.center().x - dw * 0.5 - 14.0).round(), 84.0);
        egui::Area::new(egui::Id::new("cm-dialog"))
            .order(egui::Order::Foreground)
            .movable(true)
            .default_pos(pos)
            .constrain(true)
            .show(ctx, |ui| {
                panel_frame(14).show(ui, |ui| {
                    ui.set_width(dw);
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
                    // ── header: title + Picker|Wheel tabs + close ──
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Color Picker").color(TEXT).strong().size(13.5));
                        ui.add_space(10.0);
                        for (tab, label) in [(MTab::Picker, "Picker"), (MTab::Wheel, "Wheel")] {
                            let on = m.tab == tab;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(56.0, 22.0), egui::Sense::click());
                            let rr = CornerRadius::same(R);
                            if on {
                                ui.painter().rect_filled(r, rr, BG_SURFACE);
                                ui.painter().rect_stroke(r, rr, Stroke::new(1.0, ACCENT), StrokeKind::Middle);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, rr, HOVER);
                            }
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                label,
                                FontId::proportional(12.0),
                                if on { TEXT } else { MUTED },
                            );
                            if resp.clicked() {
                                m.tab = tab;
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if mini_btn(ui, "×", "Close (Esc)") {
                                cancel = true;
                            }
                        });
                    });
                    ui.add_space(2.0);
                    let (px, py, sl) = pick_get(m.chan, m.hsva);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            if m.tab == MTab::Wheel {
                                build_wheel(ui, m);
                            } else {
                                ui.horizontal_top(|ui| {
                                    // ── the field plane (axes follow the channel radio) ──
                                    let (pw, ph) = (240.0, 240.0);
                                    let (pr, presp) =
                                        ui.allocate_exact_size(egui::vec2(pw, ph), egui::Sense::click_and_drag());
                                    let (nx, ny) = (24usize, 16usize);
                                    let mut mesh = egui::Mesh::default();
                                    for gy in 0..=ny {
                                        for gx in 0..=nx {
                                            let fx = gx as f32 / nx as f32;
                                            let fy = gy as f32 / ny as f32;
                                            mesh.colored_vertex(
                                                egui::pos2(pr.left() + fx * pw, pr.top() + fy * ph),
                                                rgb_c32(pick_rgb(m.chan, fx, 1.0 - fy, sl)),
                                            );
                                        }
                                    }
                                    for gy in 0..ny as u32 {
                                        for gx in 0..nx as u32 {
                                            let w1 = nx as u32 + 1;
                                            let i = gy * w1 + gx;
                                            mesh.add_triangle(i, i + 1, i + w1 + 1);
                                            mesh.add_triangle(i, i + w1 + 1, i + w1);
                                        }
                                    }
                                    ui.painter_at(pr).add(egui::Shape::mesh(mesh));
                                    ui.painter().rect_stroke(
                                        pr,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0, BORDER_2),
                                        StrokeKind::Middle,
                                    );
                                    let mp = egui::pos2(pr.left() + px * pw, pr.top() + (1.0 - py) * ph);
                                    ui.painter().circle_stroke(mp, 6.0, Stroke::new(2.0, Color32::WHITE));
                                    ui.painter().circle_stroke(
                                        mp,
                                        7.0,
                                        Stroke::new(1.0, Color32::from_black_alpha(110)),
                                    );
                                    if presp.is_pointer_button_down_on() || presp.dragged() {
                                        if let Some(p) = presp.interact_pointer_pos() {
                                            pick_set(
                                                m.chan,
                                                &mut m.hsva,
                                                ((p.x - pr.left()) / pw).clamp(0.0, 1.0),
                                                (1.0 - (p.y - pr.top()) / ph).clamp(0.0, 1.0),
                                                sl,
                                            );
                                        }
                                    }
                                    // ── the channel spectrum slider (contextual gradient) ──
                                    let (sr, sresp) =
                                        ui.allocate_exact_size(egui::vec2(16.0, ph), egui::Sense::click_and_drag());
                                    let stops = 32;
                                    let mut sm = egui::Mesh::default();
                                    for i in 0..=stops {
                                        let t = i as f32 / stops as f32;
                                        let sv = if m.chan == Chan::H { t } else { 1.0 - t };
                                        let cc = rgb_c32(pick_rgb(m.chan, px, py, sv));
                                        let y = sr.top() + t * ph;
                                        sm.colored_vertex(egui::pos2(sr.left(), y), cc);
                                        sm.colored_vertex(egui::pos2(sr.right(), y), cc);
                                    }
                                    for i in 0..stops as u32 {
                                        let a = i * 2;
                                        sm.add_triangle(a, a + 1, a + 3);
                                        sm.add_triangle(a, a + 3, a + 2);
                                    }
                                    ui.painter_at(sr).add(egui::Shape::mesh(sm));
                                    ui.painter().rect_stroke(
                                        sr,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0, BORDER_2),
                                        StrokeKind::Middle,
                                    );
                                    rail_thumb(
                                        ui.painter(),
                                        sr,
                                        sr.top() + (if m.chan == Chan::H { sl } else { 1.0 - sl }) * ph,
                                    );
                                    if sresp.is_pointer_button_down_on() || sresp.dragged() {
                                        if let Some(p) = sresp.interact_pointer_pos() {
                                            let t = ((p.y - sr.top()) / ph).clamp(0.0, 1.0);
                                            let nsl = if m.chan == Chan::H { t.min(0.9999) } else { 1.0 - t };
                                            pick_set(m.chan, &mut m.hsva, px, py, nsl);
                                        }
                                    }
                                    // ── alpha rail ──
                                    let (ar, arr) =
                                        ui.allocate_exact_size(egui::vec2(16.0, ph), egui::Sense::click_and_drag());
                                    checker(&ui.painter_at(ar), ar, 6.0);
                                    let solid = hsv_c32(m.hsva[0], m.hsva[1], m.hsva[2]);
                                    let mut am = egui::Mesh::default();
                                    am.colored_vertex(ar.left_top(), solid);
                                    am.colored_vertex(ar.right_top(), solid);
                                    am.colored_vertex(ar.right_bottom(), Color32::TRANSPARENT);
                                    am.colored_vertex(ar.left_bottom(), Color32::TRANSPARENT);
                                    am.add_triangle(0, 1, 2);
                                    am.add_triangle(0, 2, 3);
                                    ui.painter_at(ar).add(egui::Shape::mesh(am));
                                    ui.painter().rect_stroke(
                                        ar,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0, BORDER_2),
                                        StrokeKind::Middle,
                                    );
                                    rail_thumb(ui.painter(), ar, ar.top() + (1.0 - m.hsva[3]) * ph);
                                    if arr.is_pointer_button_down_on() || arr.dragged() {
                                        if let Some(p) = arr.interact_pointer_pos() {
                                            m.hsva[3] = (1.0 - (p.y - ar.top()) / ph).clamp(0.0, 1.0);
                                        }
                                    }
                                }); // close the Picker plane+slider+alpha row
                            } // close: else (the Picker tab)
                        }); // close the left-region vertical (Picker plane OR Wheel)
                            // ── right column (shared by both tabs) ──
                        ui.vertical(|ui| {
                            ui.set_width(176.0);
                            // new / current split preview + OK / Cancel
                            ui.horizontal_top(|ui| {
                                let (swr, swresp) =
                                    ui.allocate_exact_size(egui::vec2(44.0, 58.0), egui::Sense::click());
                                let c = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
                                let newc = [c[0], c[1], c[2], m.hsva[3]];
                                let topr = egui::Rect::from_min_max(swr.min, egui::pos2(swr.right(), swr.center().y));
                                let botr = egui::Rect::from_min_max(egui::pos2(swr.left(), swr.center().y), swr.max);
                                if newc[3] < 0.999 {
                                    checker(&ui.painter_at(topr), topr, 5.0);
                                }
                                ui.painter().rect_filled(topr, CornerRadius::ZERO, rgba_c32a(newc));
                                match m.orig {
                                    Some(oc) => {
                                        if oc[3] < 0.999 {
                                            checker(&ui.painter_at(botr), botr, 5.0);
                                        }
                                        ui.painter().rect_filled(botr, CornerRadius::ZERO, rgba_c32a(oc));
                                    }
                                    None => {
                                        ui.painter().rect_filled(botr, CornerRadius::ZERO, SWATCH_WELL);
                                        ui.painter().line_segment(
                                            [
                                                botr.left_bottom() + egui::vec2(2.0, -2.0),
                                                botr.right_top() + egui::vec2(-2.0, 2.0),
                                            ],
                                            Stroke::new(1.4, NONE_RED),
                                        );
                                    }
                                }
                                ui.painter().rect_stroke(
                                    swr,
                                    CornerRadius::ZERO,
                                    Stroke::new(1.0, BORDER_2),
                                    StrokeKind::Middle,
                                );
                                if swresp.clicked() {
                                    if let Some(p) = swresp.interact_pointer_pos() {
                                        if p.y > swr.center().y {
                                            if let Some(oc) = m.orig {
                                                let h = rgb_to_hsv(oc);
                                                if h[1] > 0.001 {
                                                    m.hsva[0] = h[0];
                                                }
                                                m.hsva[1] = h[1];
                                                m.hsva[2] = h[2];
                                                m.hsva[3] = oc[3];
                                            }
                                        }
                                    }
                                }
                                swresp.on_hover_text("new / current \u{2014} click the bottom half to restore");
                                ui.vertical(|ui| {
                                    if dlg_btn(ui, "OK", true, 118.0) {
                                        ok = true;
                                    }
                                    if dlg_btn(ui, "Cancel", false, 118.0) {
                                        cancel = true;
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            // hex + alpha
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("#").color(FAINT).monospace().size(12.5));
                                let hid = egui::Id::new("cm-hex");
                                let rgbc = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
                                let mut buf = ui.data_mut(|d| d.get_temp::<String>(hid)).unwrap_or_else(|| {
                                    hex_of([rgbc[0], rgbc[1], rgbc[2], 1.0]).trim_start_matches('#').to_string()
                                });
                                let te = ui.add(
                                    egui::TextEdit::singleline(&mut buf)
                                        .desired_width(64.0)
                                        .font(egui::FontId::monospace(12.5))
                                        .text_color(TEXT),
                                );
                                // commit on Enter/blur only — no colour-jumping through 3-digit parses mid-typing
                                if te.lost_focus() {
                                    if let Some(c2) = parse_hex(&buf) {
                                        let h = rgb_to_hsv(c2);
                                        if h[1] > 0.001 {
                                            m.hsva[0] = h[0];
                                        }
                                        m.hsva[1] = h[1];
                                        m.hsva[2] = h[2];
                                        m.hsva[3] = c2[3];
                                    }
                                    ui.data_mut(|d| d.remove::<String>(hid));
                                } else if te.has_focus() {
                                    ui.data_mut(|d| d.insert_temp(hid, buf.clone()));
                                } else {
                                    ui.data_mut(|d| d.remove::<String>(hid));
                                }
                                if let Some(v) = num_field(
                                    ui,
                                    60.0,
                                    Lab::Letter("A"),
                                    "cm-a",
                                    m.hsva[3] * 100.0,
                                    0,
                                    1.0,
                                    1.0,
                                    0.0..=100.0,
                                ) {
                                    m.hsva[3] = v / 100.0;
                                }
                                ui.label(RichText::new("%").color(FAINT).size(11.0));
                            });
                            // HSB rows (radio → that channel drives the slider; field shows the other two)
                            for (chan, lab, tip, max, val, suf) in [
                                (Chan::H, "H", "cm-h", 360.0, m.hsva[0] * 360.0, "\u{00b0}"),
                                (Chan::S, "S", "cm-s", 100.0, m.hsva[1] * 100.0, "%"),
                                (Chan::B, "B", "cm-b", 100.0, m.hsva[2] * 100.0, "%"),
                            ] {
                                ui.horizontal(|ui| {
                                    if radio_dot(ui, m.chan == chan) {
                                        m.chan = chan;
                                    }
                                    if let Some(v) =
                                        num_field(ui, 76.0, Lab::Letter(lab), tip, val, 0, 1.0, 1.0, 0.0..=max)
                                    {
                                        match chan {
                                            Chan::H => m.hsva[0] = (v / 360.0).min(0.9999),
                                            Chan::S => m.hsva[1] = v / 100.0,
                                            _ => m.hsva[2] = v / 100.0,
                                        }
                                    }
                                    ui.label(RichText::new(suf).color(FAINT).size(11.0));
                                });
                            }
                            // RGB rows
                            let rgbv = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
                            for (i, (chan, lab, tip)) in
                                [(Chan::R, "R", "cm-r"), (Chan::G, "G", "cm-g"), (Chan::Bl, "B", "cm-bl")]
                                    .into_iter()
                                    .enumerate()
                            {
                                ui.horizontal(|ui| {
                                    if radio_dot(ui, m.chan == chan) {
                                        m.chan = chan;
                                    }
                                    if let Some(v) = num_field(
                                        ui,
                                        76.0,
                                        Lab::Letter(lab),
                                        tip,
                                        rgbv[i] * 255.0,
                                        0,
                                        1.0,
                                        1.0,
                                        0.0..=255.0,
                                    ) {
                                        let mut c2 = rgbv;
                                        c2[i] = v / 255.0;
                                        let h = rgb_to_hsv([c2[0], c2[1], c2[2], 1.0]);
                                        if h[1] > 0.001 {
                                            m.hsva[0] = h[0];
                                        }
                                        m.hsva[1] = h[1];
                                        m.hsva[2] = h[2];
                                    }
                                });
                            }
                        });
                    });
                    // ── recent + document strips ──
                    let mut adopt = None;
                    if let Some(c) = swatch_strip(ui, "RECENT", &snap.recent) {
                        adopt = Some(c);
                    }
                    if let Some(c) = swatch_strip(ui, "DOCUMENT", &snap.doc_colors) {
                        adopt = Some(c);
                    }
                    if let Some(c) = adopt {
                        let h = rgb_to_hsv(c);
                        if h[1] > 0.001 {
                            m.hsva[0] = h[0];
                        } // greys keep the current hue (no snap-to-red)
                        m.hsva[1] = h[1];
                        m.hsva[2] = h[2];
                        m.hsva[3] = c[3];
                    }
                });
            });
        // keyboard: Esc = Cancel · Enter = OK (only when no field is focused)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && ctx.memory(|mem| mem.focused().is_none()) {
            ok = true;
        }
        if ok {
            let c = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
            let col = [c[0], c[1], c[2], m.hsva[3]];
            match m.target {
                MTarget::Paint(t) => ops.push(Op::Paint(t, Some(col))),
                MTarget::Ab(i) => ops.push(Op::AbColor(i, Some(col))),
            }
            ops.push(Op::Recent(col));
        }
    }
    if ok || cancel {
        *modal = None;
    }
}

// ───────────────────────────── tool rail ─────────────────────────────

fn icon_button(ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, active: bool) -> egui::Response {
    // 30px cells / 16px glyphs — the rail sits in the same size family as the top-bar buttons
    // (Ahmed 07-07: "التول بار ضخم عن باقي البرنامج")
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
    let painter = ui.painter();
    let rounding = CornerRadius::same(R);
    if active {
        painter.rect_filled(rect, rounding, ACCENT);
    } else if resp.hovered() {
        painter.rect_filled(rect, rounding, HOVER);
    }
    if let Some(t) = tex {
        let ir = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0));
        painter.image(t.id(), ir, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }
    resp
}

fn divider(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 1.0), egui::Sense::hover());
    ui.painter().hline((rect.left() + 5.0)..=(rect.right() - 5.0), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(3.0);
}

// ───────────────────────────── startup splash ─────────────────────────────

const SPLASH_DUR: f32 = 1.55; // total seconds on screen (STATIC — no fade in/out, no animation; Ahmed 07-08)

fn with_a(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0).clamp(0.0, 255.0) as u8)
}
fn rgba_a(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0).clamp(0.0, 255.0) as u8)
}

/// Photoshop-style startup splash: a centered card (logo + wordmark + version + tagline + progress +
/// an abstract "vector" art panel) on a dark scrim, drawn on a Foreground layer. Fades into the editor.
fn build_splash(ctx: &egui::Context, _e: f32, logo: &Option<egui::TextureHandle>) {
    // STATIC splash (Ahmed 07-08): full opacity the whole time, gone instantly at SPLASH_DUR — no
    // fade, no ease, no animation. `ca` stays only so the shared alpha helpers read cleanly; it is 1.0.
    let ca = 1.0f32;

    // The card floats on the window's transparent surface (no dark scrim) → it sits over the desktop.
    let scr = ctx.content_rect();
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("splash")));
    let card = egui::Rect::from_center_size(scr.center() + egui::vec2(0.0, 2.0), egui::vec2(520.0, 300.0));
    // rule 2 — NOT ONE SHADOW: the 1px hairline below is the only separation ("الفصل بخط شعرة، مش ضل")
    p.rect_filled(card, CornerRadius::same(16), with_a(SOLID_PANEL, ca));
    p.rect_stroke(card, CornerRadius::same(16), Stroke::new(1.0, with_a(BORDER, ca)), StrokeKind::Middle);
    p.line_segment(
        [card.left_top() + egui::vec2(16.0, 1.0), card.right_top() + egui::vec2(-16.0, 1.0)],
        Stroke::new(1.0, rgba_a(255, 255, 255, 0.04 * ca)),
    );

    let (l, t) = (card.left(), card.top());
    // logo + wordmark
    if let Some(tex) = logo {
        p.image(
            tex.id(),
            egui::Rect::from_center_size(egui::pos2(l + 45.0, t + 52.0), egui::vec2(34.0, 34.0)),
            UV01(),
            rgba_a(255, 255, 255, ca),
        );
    }
    p.text(egui::pos2(l + 72.0, t + 47.0), Align2::LEFT_CENTER, "Varos", FontId::proportional(26.0), with_a(TEXT, ca));
    p.text(
        egui::pos2(l + 73.0, t + 69.0),
        Align2::LEFT_CENTER,
        "\u{3b1} \u{b7} pre-alpha",
        FontId::monospace(11.5),
        with_a(MUTED, ca),
    );
    p.text(
        egui::pos2(l + 28.0, t + 104.0),
        Align2::LEFT_TOP,
        "Arabic-first vector design.",
        FontId::proportional(13.5),
        with_a(TEXT, 0.82 * ca),
    );
    p.hline((l + 28.0)..=(l + 282.0), t + 150.0, Stroke::new(1.0, with_a(BORDER, ca)));
    p.text(
        egui::pos2(l + 28.0, card.bottom() - 22.0),
        Align2::LEFT_BOTTOM,
        "\u{a9} 2026 Varos \u{b7} pre-alpha \u{b7} built with wgpu + egui",
        FontId::proportional(10.5),
        rgba_a(0x60, 0x60, 0x64, ca),
    );

    // ── abstract "vector editor" art panel (right) ──
    let a = egui::Rect::from_min_max(
        egui::pos2(card.right() - 224.0, t + 28.0),
        egui::pos2(card.right() - 28.0, card.bottom() - 28.0),
    );
    p.rect_filled(a, CornerRadius::same(10), with_a(BG_SURFACE, ca));
    p.rect_stroke(a, CornerRadius::same(10), Stroke::new(1.0, with_a(BORDER, ca)), StrokeKind::Middle);
    let pa = p.with_clip_rect(a);
    // two ghosted "artboards" (warm-white ghosts — azure is a scalpel, never splash decoration)
    let ab = |x: f32, y: f32| egui::Rect::from_min_size(egui::pos2(a.left() + x, a.top() + y), egui::vec2(116.0, 92.0));
    pa.rect_filled(ab(30.0, 44.0), CornerRadius::same(8), rgba_a(255, 255, 255, 0.03 * ca));
    pa.rect_stroke(
        ab(30.0, 44.0),
        CornerRadius::same(8),
        Stroke::new(1.0, rgba_a(255, 255, 255, 0.07 * ca)),
        StrokeKind::Middle,
    );
    pa.rect_filled(ab(56.0, 84.0), CornerRadius::same(8), rgba_a(255, 255, 255, 0.045 * ca));
    pa.rect_stroke(
        ab(56.0, 84.0),
        CornerRadius::same(8),
        Stroke::new(1.0, rgba_a(255, 255, 255, 0.1 * ca)),
        StrokeKind::Middle,
    );
    // ghost "V" monogram
    let vc = a.center();
    pa.line_segment(
        [vc + egui::vec2(-42.0, -38.0), vc + egui::vec2(0.0, 44.0)],
        Stroke::new(10.0, rgba_a(255, 255, 255, 0.055 * ca)),
    );
    pa.line_segment(
        [vc + egui::vec2(42.0, -38.0), vc + egui::vec2(0.0, 44.0)],
        Stroke::new(10.0, rgba_a(255, 255, 255, 0.055 * ca)),
    );
    // a pen-tool cubic Bézier with anchors + handles (the "this is a vector editor" tell)
    let (p0, p1, p2, p3) = (
        egui::pos2(a.left() + 26.0, a.bottom() - 54.0),
        egui::pos2(a.left() + 66.0, a.top() + 58.0),
        egui::pos2(a.right() - 66.0, a.bottom() - 30.0),
        egui::pos2(a.right() - 26.0, a.top() + 70.0),
    );
    let cub = |s: f32| {
        let u = 1.0 - s;
        egui::pos2(
            u * u * u * p0.x + 3.0 * u * u * s * p1.x + 3.0 * u * s * s * p2.x + s * s * s * p3.x,
            u * u * u * p0.y + 3.0 * u * u * s * p1.y + 3.0 * u * s * s * p2.y + s * s * s * p3.y,
        )
    };
    let curve: Vec<egui::Pos2> = (0..=24).map(|i| cub(i as f32 / 24.0)).collect();
    pa.add(egui::Shape::line(curve, Stroke::new(2.0, rgba_a(255, 255, 255, 0.34 * ca))));
    pa.line_segment([p0, p1], Stroke::new(1.0, rgba_a(255, 255, 255, 0.16 * ca)));
    pa.line_segment([p3, p2], Stroke::new(1.0, rgba_a(255, 255, 255, 0.16 * ca)));
    for cp in [p1, p2] {
        pa.circle_stroke(cp, 2.5, Stroke::new(1.0, rgba_a(255, 255, 255, 0.34 * ca)));
    }
    for an in [p0, p3] {
        pa.rect_filled(
            egui::Rect::from_center_size(an, egui::vec2(6.0, 6.0)),
            CornerRadius::same(1),
            rgba_a(255, 255, 255, 0.34 * ca),
        );
        pa.rect_stroke(
            egui::Rect::from_center_size(an, egui::vec2(6.0, 6.0)),
            CornerRadius::same(1),
            Stroke::new(1.0, rgba_a(255, 255, 255, 0.78 * ca)),
            StrokeKind::Middle,
        );
    }
}

// ───────────────────────────── custom title bar ─────────────────────────────

/// One window-control button (min/max/close): 46×40, no rounding, hover fill + icon.
#[derive(Clone, Copy)]
enum Cap {
    Min,
    Max,
    Restore,
    Close,
}

/// A window caption button. The glyph is PAINTED directly (crisp 1px lines like Windows 11 / Chrome),
/// not an SVG texture — the Lucide minus rendered with round caps looked like a fat pill, not a clean dash.
fn winctl(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    cap: Cap,
    key: &str,
    hover_bg: Color32,
    white_on_hover: bool,
) -> bool {
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    let hov = resp.hovered();
    if hov {
        p.rect_filled(rect, CornerRadius::ZERO, hover_bg);
    }
    let col = if white_on_hover && hov { Color32::WHITE } else { TEXT };
    let s = Stroke::new(1.0, col);
    let c = rect.center();
    match cap {
        Cap::Min => {
            let y = c.y.round() + 0.5;
            p.line_segment([egui::pos2(c.x - 5.0, y), egui::pos2(c.x + 5.0, y)], s);
        }
        Cap::Max => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)),
                CornerRadius::ZERO,
                s,
                StrokeKind::Middle,
            );
        }
        Cap::Restore => {
            // two overlapping windows — the "restore down" glyph shown WHILE maximized
            p.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(c.x - 5.0, c.y - 2.0), egui::vec2(7.0, 7.0)),
                CornerRadius::ZERO,
                s,
                StrokeKind::Middle,
            ); // front
            p.line_segment([egui::pos2(c.x - 2.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y - 5.0)], s); // back top edge
            p.line_segment([egui::pos2(c.x + 5.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y + 2.0)], s);
            // back right edge
        }
        Cap::Close => {
            p.line_segment([c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)], s);
            p.line_segment([c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)], s);
        }
    }
    resp.clicked()
}

/// A 34×30 top-bar icon button (menu/search/layout/panels). Returns its Response.
fn topbtn(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    tex: &Option<egui::TextureHandle>,
    key: &str,
    active: bool,
) -> egui::Response {
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    let rr = CornerRadius::same(3); // §3.5: control radius r
    if active {
        p.rect_filled(rect, rr, BG_SURFACE);
    } else if resp.hovered() {
        p.rect_filled(rect, rr, HOVER);
    }
    let col = if active || resp.hovered() { TEXT } else { MUTED };
    if let Some(t) = tex {
        p.image(t.id(), egui::Rect::from_center_size(rect.center(), egui::vec2(17.0, 17.0)), UV01(), col);
    }
    resp
}

/// §3.5 app-bar text button (pad 5 12, radius r): solid = surface fill + line2 border;
/// ghost = bare muted text that lights on hover. Laid out from its RIGHT edge; returns its Response.
fn bar_btn(ui: &mut egui::Ui, p: &egui::Painter, right: f32, cy: f32, label: &str, ghost: bool) -> egui::Response {
    let f = FontId::proportional(12.0);
    let gw = p.layout_no_wrap(label.to_owned(), f.clone(), TEXT).size().x;
    let rect = egui::Rect::from_min_max(egui::pos2(right - gw - 24.0, cy - 13.0), egui::pos2(right, cy + 13.0));
    let resp = ui.interact(rect, ui.id().with(("bar-btn", label)), egui::Sense::click());
    let rr = CornerRadius::same(3);
    if ghost {
        if resp.hovered() {
            p.rect_filled(rect, rr, HOVER);
        }
    } else {
        p.rect_filled(rect, rr, if resp.hovered() { HOVER } else { BG_SURFACE });
        p.rect_stroke(rect, rr, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    }
    let col = if ghost && !resp.hovered() { MUTED } else { TEXT };
    p.text(rect.center(), Align2::CENTER_CENTER, label, f, col);
    resp
}

/// §3.5 search pill: 🔍 Search · [Ctrl K] — a surface capsule sitting on the void.
/// Laid out from its RIGHT edge; returns its rect. (Visual mirror — search lands with its home.)
fn search_pill(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    right: f32,
    cy: f32,
    icon: &Option<egui::TextureHandle>,
) -> egui::Rect {
    let f = FontId::proportional(11.5);
    let fk = FontId::monospace(10.0);
    let sw = p.layout_no_wrap("Search".into(), f.clone(), FAINT).size().x;
    let kw = p.layout_no_wrap("Ctrl K".into(), fk.clone(), MUTED).size().x + 8.0;
    let w = 9.0 + 13.0 + 6.0 + sw + 8.0 + kw + 9.0;
    let rect = egui::Rect::from_min_max(egui::pos2(right - w, cy - 12.0), egui::pos2(right, cy + 12.0));
    let _ = ui.interact(rect, ui.id().with("tb-kpill"), egui::Sense::hover());
    let rr = CornerRadius::same(3);
    p.rect_filled(rect, rr, BG_SURFACE);
    p.rect_stroke(rect, rr, Stroke::new(1.0, BORDER), StrokeKind::Middle);
    let mut x = rect.left() + 9.0;
    if let Some(t) = icon {
        p.image(t.id(), egui::Rect::from_center_size(egui::pos2(x + 6.5, cy), egui::vec2(13.0, 13.0)), UV01(), FAINT);
    }
    x += 13.0 + 6.0;
    let tr = p.text(egui::pos2(x, cy), Align2::LEFT_CENTER, "Search", f, FAINT);
    x = tr.right() + 8.0;
    let krect = egui::Rect::from_min_size(egui::pos2(x, cy - 8.0), egui::vec2(kw, 16.0));
    p.rect_stroke(krect, CornerRadius::same(2), Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    p.text(krect.center(), Align2::CENTER_CENTER, "Ctrl K", fk, MUTED);
    rect
}

/// One document tab. Returns (activate_clicked, close_clicked).
fn tab_item(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    active: bool,
    tex_x: &Option<egui::TextureHandle>,
    key: &str,
) -> (bool, bool) {
    // Brave chip in the void (§3.5): active = filled panel block; inactive = bare muted text,
    // hover = a whisper of white. No accent — azure is a scalpel, not a tab decoration.
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    let rr = CornerRadius::same(RBOX);
    if active {
        p.rect_filled(rect, rr, SOLID_PANEL);
    } else if resp.hovered() {
        p.rect_filled(rect, rr, VOID_HOVER);
    }
    p.text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        if active { TEXT } else { MUTED },
    );
    let x_r = egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, rect.center().y), egui::vec2(18.0, 18.0));
    let xr = ui.interact(x_r, ui.id().with((key, "x")), egui::Sense::click());
    if xr.hovered() {
        p.rect_filled(x_r, CornerRadius::same(4), HOVER);
    }
    if let Some(t) = tex_x {
        p.image(
            t.id(),
            egui::Rect::from_center_size(x_r.center(), egui::vec2(11.0, 11.0)),
            UV01(),
            if xr.hovered() { TEXT } else { FAINT },
        );
    }
    (resp.clicked(), xr.clicked())
}

// ── menu metrics (Ahmed 07-07: "مساحات محسوبة بالمللي") — ONE place, Illustrator-crisp ──
// Rows are contiguous (no inter-row gap), the hover strip bleeds edge-to-edge and is SQUARE, text
// always starts after a fixed ✓-gutter so every menu lines up, shortcuts hang right in mono.
const MENU_ROW_H: f32 = 26.0; // row height
const MENU_GUTTER: f32 = 28.0; // left column reserved for ✓ marks — text aligns after it, always
const MENU_PAD_V: i8 = 5; // frame's vertical padding (rows themselves are full-bleed)
const MENU_R: u8 = 4; // outer radius — sharp, a work tool (was 10: "ناعمة ومايعة")

/// One menu row: label left (after the gutter), shortcut right. Returns true on click.
fn menu_row(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), MENU_ROW_H), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, HOVER); // full-bleed, square — AI-crisp
    }
    ui.painter().text(
        egui::pos2(rect.left() + MENU_GUTTER, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        TEXT,
    );
    if !shortcut.is_empty() {
        ui.painter().text(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::monospace(10.5),
            MUTED,
        );
    }
    resp.clicked()
}

/// A toggle row: same skeleton as `menu_row`, with a hand-drawn ✓ in the gutter when on —
/// Illustrator's Window-menu look (NOT a checkbox; Ahmed 07-07).
fn check_row(ui: &mut egui::Ui, label: &str, checked: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), MENU_ROW_H), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, HOVER);
    }
    if checked {
        let c = egui::pos2(rect.left() + 14.0, rect.center().y);
        let knee = c + egui::vec2(-1.2, 2.8);
        ui.painter().line_segment([c + egui::vec2(-4.0, -0.2), knee], Stroke::new(1.6, TEXT));
        ui.painter().line_segment([knee, c + egui::vec2(4.2, -3.4)], Stroke::new(1.6, TEXT));
    }
    ui.painter().text(
        egui::pos2(rect.left() + MENU_GUTTER, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        TEXT,
    );
    resp.clicked()
}

/// Full-width menu separator with even breath above/below.
fn menu_sep(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(r.left()..=r.right(), r.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(4.0);
}

/// Custom top bar (the native caption is stripped in WM_NCCALCSIZE): menu · tabs · drag · right tools ·
/// window controls. Interactive rects are published as exclusions so the OS hit-test makes them HTCLIENT
/// (egui handles them) while the empty band is HTCAPTION (the OS drags/snaps the window).
#[allow(clippy::too_many_arguments)] // hand-painted panel builder: each arg is live UI state, split deferred with ui.rs
fn build_topbar(
    root: &mut egui::Ui,
    top: &TopIcons,
    shell: &mut varos_app::shell::ShellState,
    win_action: &mut Option<WinAction>,
    tabs: &mut Vec<String>,
    tab_active: &mut usize,
    show_rail: &mut bool,
    show_dock: &mut bool,
    snap: &mut varos_core::model::SnapConfig,
    maximized: bool,
) {
    let h = 46.0; // a touch taller so the bar and its flush dropdowns breathe (Ahmed 07-07)
                  // Stage 1 (BOX_SYSTEM_PLAN §3.5): the app bar IS the void — seam fill, no hairline; the doc tabs
                  // are Brave-style chips floating in it and the window caps are flush 42px void cells.
    let frame = egui::Frame { fill: SEAM, inner_margin: Margin::ZERO, ..Default::default() };
    // no separator line — the bar melts into the void below it (Ahmed 07-07 "في خط لسا موجود")
    egui::Panel::top("topbar").exact_size(h).frame(frame).show_separator_line(false).show(root, |ui| {
        let bar = ui.max_rect();
        let p = ui.painter().clone();
        let cy = bar.center().y;
        let mut excl: Vec<egui::Rect> = Vec::new();

        // window controls (min · max · close)
        let bw = 42.0;
        let close_r =
            egui::Rect::from_min_max(egui::pos2(bar.right() - bw, bar.top()), egui::pos2(bar.right(), bar.bottom()));
        let max_r = egui::Rect::from_min_max(
            egui::pos2(bar.right() - 2.0 * bw, bar.top()),
            egui::pos2(bar.right() - bw, bar.bottom()),
        );
        let min_r = egui::Rect::from_min_max(
            egui::pos2(bar.right() - 3.0 * bw, bar.top()),
            egui::pos2(bar.right() - 2.0 * bw, bar.bottom()),
        );
        if winctl(ui, &p, min_r, Cap::Min, "wc-min", HOVER, false) {
            *win_action = Some(WinAction::Minimize);
        }
        if winctl(ui, &p, max_r, if maximized { Cap::Restore } else { Cap::Max }, "wc-max", HOVER, false) {
            *win_action = Some(WinAction::ToggleMaximize);
        }
        if winctl(ui, &p, close_r, Cap::Close, "wc-close", CLOSE_RED, true) {
            *win_action = Some(WinAction::Close);
        }
        excl.extend([min_r, max_r, close_r]);

        // right cluster (§3.5), right→left: window caps · [snapping] · Window · Share · Export · search pill
        let window_id = ui.make_persistent_id("window_menu");
        let menu_id = ui.make_persistent_id("app_menu");
        // magnet = the Snapping quick-menu (Illustrator layout)
        let magnet_id = ui.make_persistent_id("snap_menu");
        let magnet_r = egui::Rect::from_center_size(egui::pos2(min_r.left() - 6.0 - 14.0, cy), egui::vec2(28.0, 28.0));
        let magnet_active = menu_open(ui, magnet_id) || snap.smart || snap.grid;
        let magr = topbtn(ui, &p, magnet_r, &top.magnet, "tb-magnet", magnet_active);
        if magr.clicked() {
            menu_toggle(ui, magnet_id);
        }
        // Window — every panel one click away, landing in an AUTOMATIC spot (Ahmed 07-07; replaces
        // the old layout/panels buttons)
        let winb = bar_btn(ui, &p, magnet_r.left() - 8.0, cy, "Window", true);
        if winb.clicked() {
            menu_toggle(ui, window_id);
        }
        // Share (solid) + Export (ghost) — the mockup pair; visual MIRRORS for now (like the burger's
        // menu rows: the look lands in Stage 1, the wiring lands with its home)
        let share = bar_btn(ui, &p, winb.rect.left() - 8.0, cy, "Share", false);
        let export = bar_btn(ui, &p, share.rect.left() - 8.0, cy, "Export", true);
        // search pill: 🔍 Search · Ctrl K — a surface capsule on the void (visual mirror too)
        let kpill_r = search_pill(ui, &p, export.rect.left() - 8.0, cy, &top.search);
        excl.extend([magnet_r, winb.rect, share.rect, export.rect, kpill_r]);

        // burger — a flush 36×40 void cell at the far left (§3.5)
        let menu_r = egui::Rect::from_min_size(egui::pos2(bar.left() + 4.0, bar.top()), egui::vec2(36.0, h));
        let mr = ui.interact(menu_r, ui.id().with("tb-menu"), egui::Sense::click());
        let mopen = menu_open(ui, menu_id);
        if mopen || mr.hovered() {
            p.rect_filled(menu_r, CornerRadius::ZERO, HOVER);
        }
        if let Some(t) = &top.menu {
            let col = if mopen || mr.hovered() { TEXT } else { MUTED };
            p.image(t.id(), egui::Rect::from_center_size(menu_r.center(), egui::vec2(17.0, 17.0)), UV01(), col);
        }
        if mr.clicked() {
            menu_toggle(ui, menu_id);
        }
        excl.push(menu_r);

        // doc tabs — Brave chips floating in the void: h28, gap 4, width fits the name (§3.5)
        let tabs_right = kpill_r.left() - 12.0;
        let mut tx = menu_r.right() + 8.0;
        let (mut to_close, mut to_activate) = (None, None);
        for (i, tab) in tabs.iter().enumerate() {
            let gw = p.layout_no_wrap(tab.clone(), FontId::proportional(12.0), TEXT).size().x;
            let tw = (12.0 + gw + 8.0 + 18.0 + 4.0).clamp(76.0, 220.0);
            if tx + tw > tabs_right {
                break;
            }
            let trect = egui::Rect::from_min_size(egui::pos2(tx, cy - 14.0), egui::vec2(tw, 28.0));
            let (click, close) = tab_item(ui, &p, trect, tab, i == *tab_active, &top.x, &format!("tab{i}"));
            if click {
                to_activate = Some(i);
            }
            if close {
                to_close = Some(i);
            }
            excl.push(trect);
            tx += tw + 4.0;
        }
        let plus_r = egui::Rect::from_center_size(egui::pos2(tx + 16.0, cy), egui::vec2(32.0, 28.0));
        if tx + 32.0 <= tabs_right {
            if topbtn(ui, &p, plus_r, &top.plus, "tb-plus", false).clicked() {
                tabs.push(format!("Untitled-{}", tabs.len() + 1));
                *tab_active = tabs.len() - 1;
            }
            excl.push(plus_r);
        }
        if let Some(i) = to_activate {
            *tab_active = i;
        }
        if let Some(i) = to_close {
            if tabs.len() > 1 {
                tabs.remove(i);
                if *tab_active >= tabs.len() {
                    *tab_active = tabs.len() - 1;
                }
            }
        }

        // dropdowns — the app-bar menus are FLUSH seam extensions of the bar (Ahmed 07-07): same
        // colour, no separating line, hanging straight off its bottom edge. FIXED widths — measured,
        // never elastic (the intrinsic-width try read the whole screen and blew the menus wide open).
        let flush = Some(bar.bottom());
        menu_below(ui, menu_id, &mr, flush, |ui| {
            ui.set_width(210.0);
            menu_row(ui, "New", "Ctrl+N");
            menu_row(ui, "Open\u{2026}", "Ctrl+O");
            menu_row(ui, "Save", "Ctrl+S");
            menu_sep(ui);
            menu_row(ui, "Export\u{2026}", "");
        });
        // the Window menu: chrome toggles up top, then EVERY dockable panel — ✓ = it's in the
        // layout; click = open in an automatic spot / surface its tab / close (boxtree::toggle_panel)
        menu_below(ui, window_id, &winb, flush, |ui| {
            ui.set_width(200.0);
            let mut hit = false; // a chosen item closes the menu (Illustrator; P7)
            if check_row(ui, "Tool rail", *show_rail) {
                *show_rail = !*show_rail;
                hit = true;
            }
            if check_row(ui, "Control bar", *show_dock) {
                *show_dock = !*show_dock;
                hit = true;
            }
            menu_sep(ui);
            for pnl in varos_app::shell::PanelId::DOCKABLE {
                if check_row(ui, pnl.title(), shell.is_open(pnl)) {
                    shell.toggle_panel(pnl);
                    hit = true;
                }
            }
            if hit {
                menu_set(ui, window_id, false);
            }
        });
        // Snapping quick-menu (Illustrator "Snapping" popover)
        menu_below(ui, magnet_id, &magr, flush, |ui| {
            ui.set_width(216.0);
            let mut hit = false; // a chosen item closes the menu (Illustrator; P7)
            if check_row(ui, "Snap to Grid", snap.grid) {
                snap.grid = !snap.grid;
                hit = true;
            }
            if check_row(ui, "Snap to Point", snap.key_points) {
                snap.key_points = !snap.key_points;
                hit = true;
            }
            menu_sep(ui);
            if check_row(ui, "Smart Guides  (Ctrl+U)", snap.smart) {
                snap.smart = !snap.smart;
                hit = true;
            }
            if check_row(ui, "    Alignment Guides", snap.alignment_guides) {
                snap.alignment_guides = !snap.alignment_guides;
                hit = true;
            }
            if check_row(ui, "    Geometric Guides", snap.object_geometry) {
                snap.object_geometry = !snap.object_geometry;
                hit = true;
            }
            if hit {
                menu_set(ui, magnet_id, false);
            }
        });

        // publish caption height + interactive (non-drag) rects, in physical px
        let ppp = ui.ctx().pixels_per_point();
        let px: Vec<[i32; 4]> = excl
            .iter()
            .map(|r| {
                [(r.left() * ppp) as i32, (r.top() * ppp) as i32, (r.right() * ppp) as i32, (r.bottom() * ppp) as i32]
            })
            .collect();
        crate::cursors::set_caption((h * ppp) as i32, &px);
    });
}

/// Seam-fill the `.mid` region EXCEPT the canvas hole (the Board pane's interior, where the wgpu
/// scene shows through). The void between boxes is exactly this underlay showing in the gaps.
fn paint_void_underlay(p: &egui::Painter, mid: egui::Rect, hole: Option<egui::Rect>) {
    match hole.map(|h| h.intersect(mid)).filter(|h| h.is_positive()) {
        None => {
            p.rect_filled(mid, CornerRadius::ZERO, SEAM);
        }
        Some(h) => {
            let (l, r) = (mid.left(), mid.right());
            p.rect_filled(egui::Rect::from_min_max(mid.min, egui::pos2(r, h.top())), CornerRadius::ZERO, SEAM);
            p.rect_filled(egui::Rect::from_min_max(egui::pos2(l, h.bottom()), mid.max), CornerRadius::ZERO, SEAM);
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(l, h.top()), egui::pos2(h.left(), h.bottom())),
                CornerRadius::ZERO,
                SEAM,
            );
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(h.right(), h.top()), egui::pos2(r, h.bottom())),
                CornerRadius::ZERO,
                SEAM,
            );
        }
    }
}

/// Patch the four corners of the Board box with seam-coloured "square minus quarter-arc" wedges, so
/// the raw wgpu scene never pokes past the box's rounded silhouette.
fn corner_voids(p: &egui::Painter, rect: egui::Rect) {
    let r = RBOX as f32;
    let n = 8; // arc samples — plenty at 8px
    let corners = [
        (rect.left_top(), egui::vec2(1.0, 1.0)),
        (rect.right_top(), egui::vec2(-1.0, 1.0)),
        (rect.right_bottom(), egui::vec2(-1.0, -1.0)),
        (rect.left_bottom(), egui::vec2(1.0, -1.0)),
    ];
    for (c, dir) in corners {
        let centre = c + egui::vec2(r * dir.x, r * dir.y);
        let mut pts = vec![c];
        for i in 0..=n {
            let a = std::f32::consts::FRAC_PI_2 * i as f32 / n as f32;
            let (s, co) = a.sin_cos();
            // sweep the quarter arc between the two edge tangent points
            pts.push(centre - egui::vec2(co * r * dir.x, s * r * dir.y));
        }
        p.add(egui::Shape::convex_polygon(pts, SEAM, Stroke::NONE));
    }
}

/// Stage 1 (§3.5): the status strip — void chrome like the app bar (h 25, seam, 11px faint).
/// Left = the beginner shortcut hints; right = artboard i/n · Fit (clickable) · zoom %.
fn build_statusbar(
    root: &mut egui::Ui,
    ab_active: usize,
    ab_count: usize,
    zoom: f32,
    fit_icon: &Option<egui::TextureHandle>,
    fit_request: &mut Option<usize>,
) {
    let frame = egui::Frame { fill: SEAM, inner_margin: Margin::ZERO, ..Default::default() };
    // 31 = 25 of bar + the 6pt float-gap under the boxes, folded IN so the text centres in the
    // strip the eye actually sees (Ahmed 07-07: "مش متوسطنة في الارتفاع")
    egui::Panel::bottom("statusbar").exact_size(31.0).frame(frame).show_separator_line(false).show(root, |ui| {
        let bar = ui.max_rect();
        let p = ui.painter().clone();
        let cy = bar.center().y;
        let f11 = FontId::proportional(11.0);
        let m11 = FontId::monospace(11.0);
        // left: shortcut hints — keys muted, prose faint (the mockup's <b> pattern)
        let mut x = bar.left() + 10.0;
        for (s, muted) in [
            ("V", true),
            (" select    ·    ", false),
            ("A", true),
            (" direct    ·    ", false),
            ("Alt", true),
            ("+drag duplicates", false),
        ] {
            let r = p.text(egui::pos2(x, cy), Align2::LEFT_CENTER, s, f11.clone(), if muted { MUTED } else { FAINT });
            x = r.right();
        }
        // right, laid right→left: zoom % · Fit · Artboard i/n (gap 14)
        let zr = p.text(
            egui::pos2(bar.right() - 10.0, cy),
            Align2::RIGHT_CENTER,
            format!("{:.0}%", zoom * 100.0),
            m11.clone(),
            MUTED,
        );
        let fw = 13.0 + 4.0 + p.layout_no_wrap("Fit".into(), f11.clone(), FAINT).size().x;
        let fit_r = egui::Rect::from_min_size(egui::pos2(zr.left() - 14.0 - fw, cy - 9.0), egui::vec2(fw, 18.0));
        let fresp = ui.interact(fit_r, ui.id().with("st-fit"), egui::Sense::click());
        let fcol = if fresp.hovered() { TEXT } else { FAINT };
        if let Some(t) = fit_icon {
            p.image(
                t.id(),
                egui::Rect::from_center_size(egui::pos2(fit_r.left() + 6.5, cy), egui::vec2(13.0, 13.0)),
                UV01(),
                fcol,
            );
        }
        p.text(egui::pos2(fit_r.left() + 17.0, cy), Align2::LEFT_CENTER, "Fit", f11.clone(), fcol);
        if fresp.clicked() {
            *fit_request = Some(ab_active);
        }
        if ab_count > 0 {
            let nr = p.text(
                egui::pos2(fit_r.left() - 14.0, cy),
                Align2::RIGHT_CENTER,
                format!("{} / {}", ab_active + 1, ab_count),
                m11,
                MUTED,
            );
            p.text(egui::pos2(nr.left() - 4.0, cy), Align2::RIGHT_CENTER, "Artboard", f11, FAINT);
        }
    });
}

/// HAND 2 — the floating tool rail (§4.4), pinned INSIDE the board box: a normal box look (panel
/// fill + hairline + rounded, NO shadow) floating over the canvas hole. Never a tile.
fn board_rail(
    ctx: &egui::Context,
    board: egui::Rect,
    tools: &[ToolBtn],
    shapes: &[ToolBtn],
    shape_active: &mut ToolKind,
    s: &Snap,
    ops: &mut Vec<Op>,
) {
    // the shapes slot mirrors whichever shape tool is actually active (so the M/L keys update it too)
    if shapes.iter().any(|t| t.kind == s.tool) {
        *shape_active = s.tool;
    }
    egui::Area::new(egui::Id::new("hand2-rail"))
        .order(egui::Order::Middle)
        .pivot(Align2::LEFT_CENTER)
        .fixed_pos(egui::pos2(board.left() + 16.0, board.center().y)) // ALWAYS vertically centred (Ahmed 07-07)
        .show(ctx, |ui| {
            egui::Frame {
                fill: SOLID_PANEL,
                stroke: Stroke::new(1.0, BORDER),
                corner_radius: CornerRadius::same(RBOX),
                inner_margin: Margin::same(6),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for t in tools {
                    if icon_button(ui, &t.tex, s.tool == t.kind).on_hover_text(t.tip).clicked() {
                        ops.push(Op::Tool(t.kind));
                    }
                    if t.group_end {
                        divider(ui);
                    }
                    if t.kind == ToolKind::Pen {
                        // the SHAPES slot sits right after Pen
                        shape_slot(ui, shapes, shape_active, s, ops);
                        divider(ui);
                    }
                }
                divider(ui);
                fill_stroke_control(ui, s, ops); // Illustrator's fill/stroke box at the rail foot
            });
        });
}

/// HAND 1 — the floating control bar (§4.4/§3.5), BORN in Stage 4. FIXED presence (Ahmed 07-07):
/// the bar never vanishes — its CONTENT follows the moment. Selection → transform/appearance/align
/// + pathfinder mirrors · Artboard tool → page mirrors · otherwise the tool name and a quiet hint.
#[allow(clippy::too_many_arguments)] // hand-painted bar: each arg is live UI state
fn board_ctlbar(
    ctx: &egui::Context,
    board: egui::Rect,
    s: &Snap,
    ab: &AbSnap,
    ic: &DockIcons,
    fit_icon: &Option<egui::TextureHandle>,
    ops: &mut Vec<Op>,
    fit_request: &mut Option<usize>,
) {
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    egui::Area::new(egui::Id::new("hand1-ctlbar"))
        .order(egui::Order::Middle)
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(board.center().x, board.top() + 22.0))
        .show(ctx, |ui| {
            egui::Frame {
                fill: SOLID_PANEL,
                stroke: Stroke::new(1.0, BORDER),
                corner_radius: CornerRadius::same(RBOX),
                inner_margin: Margin::symmetric(10, 5),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if s.tool == ToolKind::Artboard {
                        // page mirrors: name · X/Y/W/H · count · Fit
                        let i = ab.active;
                        ui.label(RichText::new("Artboard").color(MUTED).size(11.5));
                        ui.label(RichText::new(&ab.name).color(TEXT).size(11.5));
                        bar_sep(ui);
                        let fw = 64.0;
                        if let Some(v) =
                            num_field(ui, fw, Lab::Letter("X"), "X position", ab.x, 0, 1.0, 1.0, full.clone())
                        {
                            ops.push(Op::AbRect(i, Some(v), None, None, None));
                        }
                        if let Some(v) =
                            num_field(ui, fw, Lab::Letter("Y"), "Y position", ab.y, 0, 1.0, 1.0, full.clone())
                        {
                            ops.push(Op::AbRect(i, None, Some(v), None, None));
                        }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("W"), "Width", ab.w, 0, 1.0, 1.0, 1.0..=1.0e6) {
                            ops.push(Op::AbRect(i, None, None, Some(v), None));
                        }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("H"), "Height", ab.h, 0, 1.0, 1.0, 1.0..=1.0e6) {
                            ops.push(Op::AbRect(i, None, None, None, Some(v)));
                        }
                        bar_sep(ui);
                        ui.label(
                            RichText::new(format!("{} / {}", i + 1, ab.count)).color(MUTED).monospace().size(11.0),
                        );
                        if icon_btn(ui, fit_icon, "Fit in window") {
                            *fit_request = Some(i);
                        }
                    } else if s.sel {
                        ui.label(RichText::new(&s.name).color(MUTED).size(11.5));
                        let fw = 64.0;
                        if let Some(v) =
                            num_field(ui, fw, Lab::Letter("X"), "X position", s.x, 0, 1.0, 1.0, full.clone())
                        {
                            ops.push(Op::SetBBox(Some(v), None, None, None, 0.0, 0.0));
                        }
                        if let Some(v) =
                            num_field(ui, fw, Lab::Letter("Y"), "Y position", s.y, 0, 1.0, 1.0, full.clone())
                        {
                            ops.push(Op::SetBBox(None, Some(v), None, None, 0.0, 0.0));
                        }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("W"), "Width", s.w, 0, 1.0, 1.0, 0.0..=1.0e6) {
                            ops.push(Op::SetBBox(None, None, Some(v), None, 0.0, 0.0));
                        }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("H"), "Height", s.h, 0, 1.0, 1.0, 0.0..=1.0e6) {
                            ops.push(Op::SetBBox(None, None, None, Some(v), 0.0, 0.0));
                        }
                        if let Some(v) = num_field(
                            ui,
                            62.0,
                            Lab::Icon(ic.rotate.as_ref()), // real rotation icon, matching the Properties dock (A14c)
                            "Rotation",
                            s.rot,
                            1,
                            1.0,
                            0.5,
                            full.clone(),
                        ) {
                            ops.push(Op::SetRot(v));
                        }
                        bar_sep(ui);
                        ctl_chip(ui, s.fill, PaintTarget::Fill, ops);
                        ctl_chip(ui, s.stroke, PaintTarget::Stroke, ops);
                        if let Some(v) = num_field(
                            ui,
                            74.0,
                            Lab::Letter("Op"),
                            "Opacity %",
                            s.opacity * 100.0,
                            0,
                            1.0,
                            0.5,
                            0.0..=100.0,
                        ) {
                            ops.push(Op::SetOpacity(v / 100.0));
                        }
                        bar_sep(ui);
                        let al = [
                            (0usize, AlignMode::Left, "Align left"),
                            (1, AlignMode::CenterH, "Align centre"),
                            (2, AlignMode::Right, "Align right"),
                            (4, AlignMode::Middle, "Align middle"),
                        ];
                        for (i, m, tip) in al {
                            if icon_btn(ui, &ic.align[i], tip) {
                                ops.push(Op::Align(m));
                            }
                        }
                        bar_sep(ui);
                        pathfinder_row(ui, ops); // the essential shape modes, in reach (Ahmed 07-07)
                    } else {
                        // idle: the current tool + a quiet hint — the bar keeps its place. While the Pen is
                        // mid-draft the hint reflects the ACT, not the (still-empty) selection (P9).
                        ui.label(RichText::new(crate::tool_name(s.tool)).color(TEXT).size(11.5));
                        let hint = if s.drawing { "Drawing path\u{2026}" } else { "No selection" };
                        ui.label(RichText::new(hint).color(FAINT).size(11.5));
                    }
                });
            });
        });
}

/// 1×16 vertical hairline separator inside the control bar (§3.5 vsep).
fn bar_sep(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(1.0, 16.0), egui::Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, BORDER));
}

/// A 17×17 colour chip (§3.5): click = open the Color Picker for that target (a MIRROR of Appearance).
fn ctl_chip(ui: &mut egui::Ui, color: Option<Rgba>, target: PaintTarget, ops: &mut Vec<Op>) {
    let (sw, resp) = ui.allocate_exact_size(egui::vec2(17.0, 17.0), egui::Sense::click());
    let round = CornerRadius::same(2);
    match color {
        Some(c) => {
            if c[3] < 0.999 {
                checker(&ui.painter_at(sw), sw, 4.0);
            }
            ui.painter().rect_filled(sw, round, rgba_c32a(c));
        }
        None => {
            ui.painter().rect_filled(sw, round, SWATCH_WELL);
            ui.painter().line_segment(
                [sw.left_bottom() + egui::vec2(2.0, -2.0), sw.right_top() + egui::vec2(-2.0, 2.0)],
                Stroke::new(1.4, NONE_RED),
            );
        }
    }
    ui.painter().rect_stroke(sw, round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    if resp.clicked() {
        ops.push(Op::OpenPicker(MTarget::Paint(target)));
    }
    resp.on_hover_text(match target {
        PaintTarget::Fill => "Fill",
        PaintTarget::Stroke => "Stroke",
    });
}

/// Illustrator's fill/stroke control: overlapping FILL square (top-left) + STROKE ring (bottom-right);
/// the focused target draws ON TOP with an accent edge. Click a swatch to focus it (X toggles) ·
/// the ⤡ arrows swap the colours (Shift+X) · the mini pair resets to white/black (D). None = red slash.
fn fill_stroke_control(ui: &mut egui::Ui, s: &Snap, ops: &mut Vec<Op>) {
    // 30-cell rail scale (Ahmed 07-07): 20px swatches overlapping inside a 30×40 slot
    let (area, _) = ui.allocate_exact_size(egui::vec2(30.0, 40.0), egui::Sense::hover());
    let p = ui.painter().clone();
    let fr = egui::Rect::from_min_size(area.min + egui::vec2(0.0, 3.0), egui::vec2(20.0, 20.0)); // fill
    let sr = egui::Rect::from_min_size(area.min + egui::vec2(10.0, 15.0), egui::vec2(20.0, 20.0)); // stroke
    let rr = CornerRadius::same(4);
    let slash = |p: &egui::Painter, r: egui::Rect| {
        p.line_segment(
            [r.left_bottom() + egui::vec2(2.5, -2.5), r.right_top() + egui::vec2(-2.5, 2.5)],
            Stroke::new(1.6, NONE_RED),
        )
    };
    let draw_fill = |p: &egui::Painter, active: bool| {
        p.rect_filled(fr.expand(2.0), CornerRadius::same(5), SOLID_PANEL); // swatch separation halo (sized to the swatch, not a control token)
        match s.fill {
            Some(c) => {
                if c[3] < 0.999 {
                    checker(p, fr, 5.0);
                }
                p.rect_filled(fr, rr, rgba_c32a(c));
            }
            None => {
                p.rect_filled(fr, rr, SWATCH_WELL);
                slash(p, fr);
            }
        }
        p.rect_stroke(
            fr,
            rr,
            Stroke::new(if active { 1.5 } else { 1.0 }, if active { ACCENT } else { BORDER_2 }),
            StrokeKind::Middle,
        );
    };
    let draw_stroke = |p: &egui::Painter, active: bool| {
        p.rect_filled(sr.expand(2.0), CornerRadius::same(5), SOLID_PANEL); // swatch separation halo
        let hole = sr.shrink(6.0);
        match s.stroke {
            Some(c) => {
                if c[3] < 0.999 {
                    checker(p, sr, 5.0);
                }
                p.rect_filled(sr, rr, rgba_c32a(c));
                p.rect_filled(hole, CornerRadius::same(2), SOLID_PANEL);
            }
            None => {
                p.rect_filled(sr, rr, SWATCH_WELL);
                p.rect_filled(hole, CornerRadius::same(2), SOLID_PANEL);
                slash(p, sr);
            }
        }
        p.rect_stroke(hole, CornerRadius::same(2), Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        p.rect_stroke(
            sr,
            rr,
            Stroke::new(if active { 1.5 } else { 1.0 }, if active { ACCENT } else { BORDER_2 }),
            StrokeKind::Middle,
        );
    };
    let fill_on_top = s.paint == PaintTarget::Fill;
    if fill_on_top {
        draw_stroke(&p, false);
        draw_fill(&p, true);
    } else {
        draw_fill(&p, false);
        draw_stroke(&p, true);
    }
    // click → focus the swatch under the pointer (the TOP one wins in the overlap)
    let resp = ui.interact(area, ui.id().with("fs"), egui::Sense::click());
    let hit = |pos: egui::Pos2| -> Option<PaintTarget> {
        let (top, bot, tt, bt) = if fill_on_top {
            (fr, sr, PaintTarget::Fill, PaintTarget::Stroke)
        } else {
            (sr, fr, PaintTarget::Stroke, PaintTarget::Fill)
        };
        if top.contains(pos) {
            Some(tt)
        } else if bot.contains(pos) {
            Some(bt)
        } else {
            None
        }
    };
    if resp.clicked() {
        if let Some(t) = resp.interact_pointer_pos().and_then(hit) {
            ops.push(Op::PaintFocus(t));
        }
    }
    // double-click a swatch → the Color Picker modal for that target (Illustrator)
    if resp.double_clicked() {
        if let Some(t) = resp.interact_pointer_pos().and_then(hit) {
            ops.push(Op::OpenPicker(MTarget::Paint(t)));
        }
    }
    resp.on_hover_text("Fill / Stroke — click to focus (X) · double-click to edit");
    // swap (Shift+X): a tiny hand-painted double-headed arrow, top-right
    let swr = egui::Rect::from_min_size(area.min + egui::vec2(20.0, 0.0), egui::vec2(10.0, 10.0));
    let rsw = ui.interact(swr, ui.id().with("fs-swap"), egui::Sense::click());
    let sc = if rsw.hovered() { Color32::WHITE } else { MUTED };
    let (a, b) = (swr.center() + egui::vec2(-4.0, 2.2), swr.center() + egui::vec2(4.0, -2.2));
    p.line_segment([a, b], Stroke::new(1.3, sc));
    p.add(egui::Shape::convex_polygon(
        vec![b + egui::vec2(-3.5, -0.5), b + egui::vec2(-0.5, 3.0), b],
        sc,
        Stroke::NONE,
    ));
    p.add(egui::Shape::convex_polygon(vec![a + egui::vec2(3.5, 0.5), a + egui::vec2(0.5, -3.0), a], sc, Stroke::NONE));
    if rsw.clicked() {
        ops.push(Op::SwapColors);
    }
    rsw.on_hover_text("Swap fill & stroke (Shift+X)");
    // default (D): the mini white/black pair, bottom-left
    let dfr = egui::Rect::from_min_size(area.min + egui::vec2(0.0, 28.0), egui::vec2(12.0, 12.0));
    let rdf = ui.interact(dfr, ui.id().with("fs-def"), egui::Sense::click());
    let m1 = egui::Rect::from_min_size(dfr.min, egui::vec2(7.0, 7.0));
    let m2 = egui::Rect::from_min_size(dfr.min + egui::vec2(4.5, 4.5), egui::vec2(7.0, 7.0));
    p.rect_filled(m2, CornerRadius::same(2), Color32::from_gray(30));
    p.rect_stroke(
        m2,
        CornerRadius::same(2),
        Stroke::new(1.0, if rdf.hovered() { Color32::WHITE } else { BORDER_2 }),
        StrokeKind::Middle,
    );
    p.rect_filled(m1, CornerRadius::same(2), Color32::from_gray(242));
    p.rect_stroke(
        m1,
        CornerRadius::same(2),
        Stroke::new(1.0, if rdf.hovered() { Color32::WHITE } else { BORDER_2 }),
        StrokeKind::Middle,
    );
    if rdf.clicked() {
        ops.push(Op::DefaultPaint);
    }
    rdf.on_hover_text("Default colours (D)");
}

/// One rail slot standing in for all four shape tools. Left-click uses the current shape; right-click
/// opens a flyout of all four (Illustrator tool-group behaviour). A corner mark hints at the flyout.
fn shape_slot(ui: &mut egui::Ui, shapes: &[ToolBtn], shape_active: &mut ToolKind, s: &Snap, ops: &mut Vec<Op>) {
    let cur = shapes.iter().find(|t| t.kind == *shape_active).unwrap_or(&shapes[0]);
    let is_active = shapes.iter().any(|t| t.kind == s.tool);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
    let rounding = CornerRadius::same(R);
    if is_active {
        ui.painter().rect_filled(rect, rounding, ACCENT);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, rounding, HOVER);
    }
    if let Some(t) = &cur.tex {
        ui.painter().image(
            t.id(),
            egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)),
            UV01(),
            Color32::WHITE,
        );
    }
    // tiny flyout marker — a corner triangle bottom-right, like Illustrator's grouped tools
    let c = rect.right_bottom() + egui::vec2(-3.5, -3.5);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![c, c + egui::vec2(-4.5, 0.0), c + egui::vec2(0.0, -4.5)],
        if is_active { Color32::WHITE } else { MUTED },
        Stroke::NONE,
    ));
    if resp.clicked() {
        ops.push(Op::Tool(*shape_active));
    }
    resp.clone().on_hover_text("Shapes \u{2014} click to use \u{00b7} right-click for more");
    let pop = ui.make_persistent_id("shape-flyout");
    if resp.secondary_clicked() {
        menu_toggle(ui, pop);
    }
    menu_below(ui, pop, &resp, None, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(5.0); // the menu frame has no horizontal padding — give the icons air
            for t in shapes {
                if icon_button(ui, &t.tex, s.tool == t.kind).on_hover_text(t.tip).clicked() {
                    *shape_active = t.kind;
                    ops.push(Op::Tool(t.kind));
                    menu_set(ui, pop, false);
                }
            }
            ui.add_space(5.0);
        });
    });
}

// ───────────────────────────── inspector dock ─────────────────────────────

struct DockIcons<'a> {
    rotate: &'a Option<egui::TextureHandle>,
    opacity: &'a Option<egui::TextureHandle>,
    strokew: &'a Option<egui::TextureHandle>,
    link: &'a Option<egui::TextureHandle>,
    fliph: &'a Option<egui::TextureHandle>,
    flipv: &'a Option<egui::TextureHandle>,
    align: &'a [Option<egui::TextureHandle>; 8],
}

/// A reveal-on-hover column toggle (eye / lock). `marked` = the persistent state that always shows its
/// glyph (hidden / locked); otherwise the glyph appears only when the row is hovered. `forced` = the
/// state is inherited from an ancestor (drawn dim, the "you can't change it here" cue). Returns clicked.
#[allow(clippy::too_many_arguments)] // hand-painted widget: geometry + behaviour knobs, split deferred with ui.rs
fn col_toggle(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    row_hovered: bool,
    marked: bool,
    forced: bool,
    marked_tex: &Option<egui::TextureHandle>,
    hint_tex: &Option<egui::TextureHandle>,
    tip: &str,
) -> bool {
    let resp = ui.interact(rect, ui.id().with(("col", rect.left() as i32, rect.top() as i32)), egui::Sense::click());
    if marked || forced || row_hovered {
        let (tex, col) = if marked || forced {
            (marked_tex, if forced && !marked { Color32::from_gray(74) } else { TEXT })
        } else {
            (hint_tex, MUTED)
        };
        if let Some(t) = tex {
            ui.painter().image(
                t.id(),
                egui::Rect::from_center_size(rect.center(), egui::vec2(14.0, 14.0)),
                UV01(),
                col,
            );
        }
    }
    resp.on_hover_text(tip).clicked()
}

/// The Layers panel — the SIMPLE (Photoshop/Affinity) VIEW of the scene tree (07-03 pivot), docked UNDER
/// the inspector (`dock_below`) and growing downward. Row = eye · lock · disclosure · thumbnail · name.
/// Click=select · Ctrl=toggle · Shift=range · dbl=rename · drag=reorder/nest · Alt+drag=duplicate.
/// Header: title + search. Footer: Group · Delete.
#[allow(clippy::too_many_arguments)] // hand-painted panel builder: each arg is live UI state, split deferred with ui.rs
fn panel_layers(
    ui: &mut egui::Ui,
    rows: &[LRow],
    ic: &LayerIcons,
    search: &mut String,
    rename: &mut Option<(u32, String)>,
    collapsed: &mut std::collections::HashSet<u32>,
    drag: &mut Option<(u32, u32)>,
    anchor: &mut Option<(u32, u32)>,
    ops: &mut Vec<Op>,
) {
    let pane = ui.max_rect();
    let w = pane.width();
    // search row ≈ 42 + footer 34 — the list gets everything in between (the box grows, the list grows)
    let list_h = (pane.height() - 42.0 - 34.0).max(60.0);
    // columns: eye · lock · [disclosure · thumb · name]. No identity bar, no target/select gutter.
    let (eye_w, lock_w) = (26.0, 22.0);
    let body_x0 = eye_w + lock_w + 8.0;
    {
        {
            let hairline = |ui: &mut egui::Ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
                ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, BORDER));
            };
            // ── header: search only (the box header already carries the "Layers" title) ──
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(11.0);
                let (sr, _) = ui.allocate_exact_size(egui::vec2(w - 22.0, 26.0), egui::Sense::hover());
                ui.painter().rect(sr, CornerRadius::same(R), BG_SURFACE, Stroke::new(1.0, BORDER), StrokeKind::Middle);
                if let Some(t) = &ic.search {
                    ui.painter().image(
                        t.id(),
                        egui::Rect::from_center_size(
                            egui::pos2(sr.left() + 14.0, sr.center().y),
                            egui::vec2(13.0, 13.0),
                        ),
                        UV01(),
                        MUTED,
                    );
                }
                ui.put(
                    egui::Rect::from_min_max(egui::pos2(sr.left() + 28.0, sr.top()), sr.max)
                        .shrink2(egui::vec2(2.0, 3.0)),
                    egui::TextEdit::singleline(search)
                        .frame(egui::Frame::NONE)
                        .hint_text("Search")
                        .font(egui::FontId::proportional(12.5))
                        .text_color(TEXT),
                );
            });
            ui.add_space(8.0);
            hairline(ui);

            // ── the row list ──
            let row_h = 26.0;
            // drag bookkeeping — ROW drag only (reorder / nest). Grabbing a row that is part of the
            // (fully-selected) multi-selection lifts the WHOLE selection: its top-most fully-selected
            // rows travel together in panel order (Photoshop). An unselected/partial row lifts alone.
            // forbidden = every payload node + its whole subtree; the model re-guards. Alt = duplicate.
            let ptr = ui.input(|i| i.pointer.interact_pos());
            let payload: Vec<u32> = drag
                .map(|(s, _)| {
                    if rows.iter().any(|r| r.id == s && r.drag_sel) {
                        let mut seen = std::collections::HashSet::new();
                        rows.iter().filter(|r| r.drag_sel && seen.insert(r.id)).map(|r| r.id).collect()
                    } else {
                        vec![s]
                    }
                })
                .unwrap_or_default();
            let src_is_layer = payload.iter().any(|&s| rows.iter().any(|r| r.id == s && r.kind == LKind::Layer));
            let forbidden: std::collections::HashSet<u32> = {
                let mut set = std::collections::HashSet::new();
                for &s in &payload {
                    if let Some(si) = rows.iter().position(|r| r.id == s) {
                        set.insert(s);
                        let sd = rows[si].depth;
                        for r in &rows[si + 1..] {
                            if r.depth > sd {
                                set.insert(r.id);
                            } else {
                                break;
                            }
                        }
                    }
                }
                set
            };
            let mut drop_ind: Option<(u32, u8, egui::Rect, u16)> = None; // target id, zone, rect, depth
                                                                         // scroll by wheel + bar only — NEVER by dragging content (that would fight our row drag-drop)
            let scroll_src = egui::scroll_area::ScrollSource {
                scroll_bar: true,
                drag: egui::scroll_area::DragScroll::Never,
                mouse_wheel: true,
            };
            egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, false]).scroll_source(scroll_src).show(
                ui,
                |ui| {
                    if rows.is_empty() {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::hover());
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            if search.trim().is_empty() { "No layers yet" } else { "No matching layers" },
                            FontId::proportional(12.0),
                            FAINT,
                        );
                    }
                    // last top-level row + last drawn rect — "drop below the list = send to the bottom"
                    let (mut last_top, mut last_rect) = (None::<u32>, None::<egui::Rect>);
                    let mut prev_sec = 0u32;
                    let mut rename_shown = false; // mirror rows: only the first instance opens the editor
                    for (ri, row) in rows.iter().enumerate() {
                        // the floater strip (art on NO board — outside export) separates with a hairline
                        if ri > 0 && row.sec == u32::MAX && prev_sec != u32::MAX {
                            ui.add_space(4.0);
                            let (hr, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
                            ui.painter().hline(hr.x_range(), hr.center().y, Stroke::new(1.0, BORDER));
                            ui.add_space(4.0);
                        }
                        prev_sec = row.sec;
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, row_h), egui::Sense::click_and_drag());
                        if resp.drag_started() && row.kind != LKind::Board {
                            *drag = Some((row.id, row.sec));
                        }
                        // decide the drop zone. SAME section: top third = Before, bottom third = After,
                        // middle = Into (containers only; a Layer can't nest into a Group); leaves halve.
                        // ANOTHER board's section (its header or any row) = zone 3: move the art onto
                        // that page spatially — drop_ind.0 then holds the TARGET SECTION, not a node id.
                        if let (Some((_, ssec)), Some(pp)) = (*drag, ptr) {
                            if !payload.contains(&row.id) && rect.contains(pp) {
                                if row.sec != ssec && row.sec != u32::MAX {
                                    drop_ind = Some((row.sec, 3, rect, row.depth));
                                } else if row.sec == ssec && row.kind != LKind::Board && !forbidden.contains(&row.id) {
                                    let f = ((pp.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                                    let into_ok =
                                        row.kind != LKind::Path && !(src_is_layer && row.kind == LKind::Group);
                                    let zone = if into_ok {
                                        if f < 0.30 {
                                            0
                                        } else if f > 0.70 {
                                            2
                                        } else {
                                            1
                                        }
                                    } else if f < 0.5 {
                                        0
                                    } else {
                                        2
                                    };
                                    drop_ind = Some((row.id, zone, rect, row.depth));
                                }
                            }
                        }
                        let p = ui.painter_at(rect);
                        let dim = if row.eff_hidden { 0.42 } else { 1.0 };
                        let hov = resp.hovered();
                        // state: SELECTED (its art is in the canvas selection) is the strong highlight; the
                        // active layer keeps a subtle accent edge even when nothing on it is selected.
                        // Board headers read as a SECTION strip; the active board keeps the accent edge.
                        if row.kind == LKind::Board {
                            p.rect_filled(rect, CornerRadius::ZERO, if hov { ROW_HOVER } else { BG_SURFACE });
                            if row.active {
                                p.rect_filled(
                                    egui::Rect::from_min_size(rect.min, egui::vec2(2.0, row_h)),
                                    CornerRadius::ZERO,
                                    ACCENT,
                                );
                            }
                        } else if row.selected {
                            p.rect_filled(rect, CornerRadius::ZERO, ACCENT_TINT);
                            p.rect_filled(
                                egui::Rect::from_min_size(rect.min, egui::vec2(2.0, row_h)),
                                CornerRadius::ZERO,
                                ACCENT,
                            );
                        } else if hov {
                            p.rect_filled(rect, CornerRadius::ZERO, ROW_HOVER);
                        } else if row.active {
                            p.rect_filled(
                                egui::Rect::from_min_size(rect.min, egui::vec2(2.0, row_h)),
                                CornerRadius::ZERO,
                                with_a(ACCENT, 0.5),
                            );
                        }
                        // eye + lock (reveal on hover; hidden/locked persist). On a Board header they
                        // act on the whole PAGE (piece C): hide the board + its art / lock its art.
                        {
                            let eye = egui::Rect::from_min_size(rect.min, egui::vec2(eye_w, row_h));
                            let lok =
                                egui::Rect::from_min_size(rect.min + egui::vec2(eye_w, 0.0), egui::vec2(lock_w, row_h));
                            let board = row.kind == LKind::Board;
                            if col_toggle(
                                ui,
                                eye,
                                hov,
                                row.hidden,
                                row.eff_hidden && !row.hidden,
                                &ic.eye_off,
                                &ic.eye,
                                if board { "Show/Hide board" } else { "Show/Hide" },
                            ) {
                                ops.push(if board { Op::AbEye(row.sec as usize) } else { Op::LayerEye(row.id) });
                            }
                            if col_toggle(
                                ui,
                                lok,
                                hov,
                                row.locked,
                                row.eff_locked && !row.locked,
                                &ic.lock,
                                &ic.unlock,
                                if board { "Lock/Unlock board" } else { "Lock/Unlock" },
                            ) {
                                ops.push(if board { Op::AbLock(row.sec as usize) } else { Op::LayerLock(row.id) });
                            }
                        }
                        // indent guides (~6% white) + disclosure
                        for lvl in 0..row.depth {
                            p.vline(
                                rect.left() + body_x0 + lvl as f32 * 14.0 + 2.0,
                                rect.y_range(),
                                Stroke::new(1.0, Color32::from_white_alpha(16)),
                            );
                        }
                        let mut x = rect.left() + body_x0 + row.depth as f32 * 14.0;
                        if row.has_children {
                            let c = egui::pos2(x + 3.0, rect.center().y);
                            let pts = if row.collapsed {
                                vec![c + egui::vec2(-3.0, -4.0), c + egui::vec2(3.0, 0.0), c + egui::vec2(-3.0, 4.0)]
                            } else {
                                vec![c + egui::vec2(-4.0, -2.5), c + egui::vec2(4.0, -2.5), c + egui::vec2(0.0, 3.5)]
                            };
                            p.add(egui::Shape::convex_polygon(pts, with_a(MUTED, dim), Stroke::NONE));
                            if ui
                                .interact(
                                    egui::Rect::from_center_size(c, egui::vec2(16.0, row_h)),
                                    ui.id().with(("disc", row.id)),
                                    egui::Sense::click(),
                                )
                                .clicked()
                            {
                                if row.collapsed {
                                    collapsed.remove(&row.id);
                                } else {
                                    collapsed.insert(row.id);
                                }
                            }
                        }
                        x += 14.0;
                        // thumbnail — one path or a whole container, composited as a real mini-preview
                        // (Ahmed: no more folder chip; the thin identity bar carries "which layer"). Empty
                        // containers/paths draw nothing — the bar alone identifies them.
                        let thumb =
                            egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 9.0), egui::vec2(18.0, 18.0));
                        if !row.thumb.is_empty() {
                            let translucent = row.thumb.iter().any(|s| s.fill.is_none_or(|c| c[3] < 0.999));
                            if translucent {
                                checker(&p, thumb, 4.5);
                            }
                            p.rect(
                                thumb,
                                CornerRadius::same(2),
                                if translucent { Color32::TRANSPARENT } else { Color32::from_gray(24) },
                                Stroke::new(1.0, with_a(BORDER_2, 0.8)),
                                StrokeKind::Middle,
                            );
                            for sh in &row.thumb {
                                let fill = sh.fill.map(|c| with_a(rgba_c32a(c), dim)).unwrap_or(Color32::TRANSPARENT);
                                let stroke = sh
                                    .stroke
                                    .map(|c| with_a(rgba_c32a(c), dim))
                                    .unwrap_or(Color32::from_gray((160.0 * dim) as u8));
                                for ring in &sh.rings {
                                    if ring.len() >= 3 {
                                        let pts: Vec<egui::Pos2> =
                                            ring.iter().map(|q| thumb.min + egui::vec2(q[0], q[1]) * 18.0).collect();
                                        p.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(1.0, stroke)));
                                    }
                                }
                            }
                        }
                        x += if row.kind == LKind::Board { 6.0 } else { 24.0 };
                        // name (auto-names muted; user/layer names bright) — or inline rename
                        let name_rect = egui::Rect::from_min_max(
                            egui::pos2(x, rect.top()),
                            egui::pos2(rect.right() - 10.0, rect.bottom()),
                        );
                        let renaming = !rename_shown && rename.as_ref().is_some_and(|(id, _)| *id == row.id);
                        if renaming {
                            rename_shown = true;
                            let buf = &mut rename.as_mut().unwrap().1;
                            let te = ui.put(
                                name_rect.shrink2(egui::vec2(2.0, 4.0)),
                                egui::TextEdit::singleline(buf)
                                    .frame(egui::Frame::NONE)
                                    .font(egui::FontId::proportional(12.5))
                                    .text_color(TEXT),
                            );
                            te.request_focus();
                            if te.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let v = std::mem::take(buf);
                                ops.push(if row.kind == LKind::Board {
                                    Op::AbName(row.sec as usize, v)
                                } else {
                                    Op::LayerRename(row.id, v)
                                });
                                *rename = None;
                            }
                        } else {
                            let auto = row.name.starts_with('<');
                            let (size, base) = match row.kind {
                                LKind::Board => (12.5, TEXT),
                                LKind::Layer => (12.5, TEXT),
                                _ if auto => (12.0, MUTED),
                                _ => (12.0, Color32::from_gray(208)),
                            };
                            let s = elide(&row.name, name_rect.width(), size);
                            p.text(
                                egui::pos2(name_rect.left(), rect.center().y),
                                Align2::LEFT_CENTER,
                                s,
                                FontId::proportional(size),
                                with_a(base, dim),
                            );
                        }
                        // selection — click / Ctrl-toggle / Shift-range act on the ROW (the 07-03 bug fix).
                        // A Board header click makes that board ACTIVE instead (new art lands there).
                        if resp.clicked() && !renaming {
                            if row.kind == LKind::Board {
                                ops.push(Op::AbActive(row.sec as usize));
                            } else {
                                let (ctrl, shift) =
                                    ui.input(|i| (i.modifiers.command || i.modifiers.ctrl, i.modifiers.shift));
                                if ctrl {
                                    ops.push(Op::LayerToggle(row.id));
                                    *anchor = Some((row.id, row.sec));
                                } else if shift {
                                    // resolve the anchor to its OWN section's appearance — a mirror
                                    // row's id exists in several sections and the first-by-id match
                                    // ranges from the wrong one (07-06 review fix #2). Fallback: the
                                    // first appearance (membership changed), else the clicked row.
                                    let a = anchor
                                        .and_then(|(aid, asec)| {
                                            rows.iter()
                                                .position(|r| r.id == aid && r.sec == asec)
                                                .or_else(|| rows.iter().position(|r| r.id == aid))
                                        })
                                        .unwrap_or(ri);
                                    let (lo, hi) = if a <= ri { (a, ri) } else { (ri, a) };
                                    ops.push(Op::LayerSelectSet(
                                        rows[lo..=hi].iter().filter(|r| r.kind != LKind::Board).map(|r| r.id).collect(),
                                    ));
                                } else {
                                    ops.push(Op::LayerSelectSet(vec![row.id]));
                                    *anchor = Some((row.id, row.sec));
                                }
                            }
                        }
                        if resp.double_clicked() {
                            *rename = Some((row.id, row.name.clone()));
                        }
                        // the lifted rows read as "picked up" — the whole payload dims while dragged
                        // (a mirror dims on BOTH appearances — it IS the same object)
                        if drag.is_some() && payload.contains(&row.id) {
                            p.rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(120));
                        }
                        // a droppable "top-level item": a member row directly under a header (depth 1)
                        // or a loose floater (depth 0) — same-section rule as everywhere else
                        let is_top_item = row.kind != LKind::Board
                            && ((row.sec == u32::MAX && row.depth == 0) || (row.sec != u32::MAX && row.depth == 1));
                        if is_top_item && drag.is_some_and(|(_, ssec)| ssec == row.sec) {
                            last_top = Some(row.id);
                        }
                        last_rect = Some(rect);
                    }
                    // below the last row = drop at the very bottom of the stack (the Photoshop feel)
                    if drag.is_some() && drop_ind.is_none() {
                        if let (Some(pp), Some(tid), Some(lr)) = (ptr, last_top, last_rect) {
                            if !forbidden.contains(&tid) && pp.y > lr.bottom() && ui.clip_rect().contains(pp) {
                                drop_ind = Some((tid, 2, lr, 0));
                            }
                        }
                    }
                    // ── drop indicator: a nest box for Into, an indented ACCENT line for Before/After ──
                    if drag.is_some() {
                        if let Some((_, zone, trect, depth)) = drop_ind {
                            let dp = ui.painter();
                            if zone == 1 || zone == 3 {
                                // Into a container / onto ANOTHER board's section — the same "lands
                                // inside this" ring (zone 3 rows read as "move to that page")
                                dp.rect(
                                    trect.shrink(1.5),
                                    CornerRadius::same(3),
                                    Color32::TRANSPARENT,
                                    Stroke::new(2.0, ACCENT),
                                    StrokeKind::Inside,
                                );
                            } else {
                                let y = if zone == 0 { trect.top() + 1.0 } else { trect.bottom() - 1.0 };
                                let ix = trect.left() + body_x0 + depth as f32 * 13.0;
                                dp.hline(ix..=(trect.right() - 10.0), y, Stroke::new(2.0, ACCENT));
                                dp.circle_filled(egui::pos2(ix, y), 3.0, ACCENT);
                            }
                        }
                    }
                },
            );
            // release: zone 3 = move the art onto the target BOARD (spatial; drop_ind.0 = section);
            // otherwise Alt = duplicate into the target row, else reorder / nest. The whole payload
            // (the multi-selection) travels in one undoable op.
            if let Some((_, src_sec)) = *drag {
                if ui.input(|i| i.pointer.any_released()) {
                    if let Some((tid, zone, _, _)) = drop_ind {
                        if zone == 3 {
                            let src_board = (src_sec != u32::MAX).then_some(src_sec as usize);
                            ops.push(Op::LayerMoveBoard(payload.clone(), src_board, tid as usize));
                        } else if ui.input(|i| i.modifiers.alt) {
                            ops.push(Op::LayerDupMove(payload.clone(), tid, zone));
                        } else {
                            ops.push(Op::LayerMove(payload.clone(), tid, zone));
                        }
                    }
                    *drag = None;
                }
            }
            if drag.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            hairline(ui);
            // ── footer: Group · Delete ──
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.add_space(11.0);
                let fbtn = |ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, tip: &str| -> bool {
                    let (rr, rp) = ui.allocate_exact_size(egui::vec2(30.0, 24.0), egui::Sense::click());
                    if rp.hovered() {
                        ui.painter().rect_filled(rr, CornerRadius::same(R), HOVER);
                    }
                    if let Some(t) = tex {
                        ui.painter().image(
                            t.id(),
                            egui::Rect::from_center_size(rr.center(), egui::vec2(15.0, 15.0)),
                            UV01(),
                            if rp.hovered() { TEXT } else { MUTED },
                        );
                    }
                    rp.on_hover_text(tip).clicked()
                };
                if fbtn(ui, &ic.grp, "Group the selection (Ctrl+G)") {
                    ops.push(Op::LayerGroup);
                }
                if fbtn(ui, &ic.trash, "Delete") {
                    ops.push(Op::LayerDeleteSel);
                }
            });
        }
    }
}

/// Truncate a name with a trailing "…" so it fits `avail` px at `size` (rough per-glyph estimate).
fn elide(name: &str, avail: f32, size: f32) -> String {
    let per = size * 0.55;
    let max = (avail / per).floor() as usize;
    if name.chars().count() <= max || max < 2 {
        return name.to_string();
    }
    let mut s: String = name.chars().take(max.saturating_sub(1)).collect();
    s.push('\u{2026}');
    s
}

/// The Properties pane body (Stage 4): the old inspector dock re-housed inside the
/// [Properties|Layers] box — Transform · Appearance · Shape (a Pathfinder MIRROR).
/// Align moved out to its own panel (ONE-HOME rule).
fn panel_properties(
    ui: &mut egui::Ui,
    s: &Snap,
    ic: &DockIcons,
    refpt: &mut (f32, f32),
    lock: &mut bool,
    ops: &mut Vec<Op>,
) {
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    egui::ScrollArea::vertical().id_salt("props-body").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
            let inner = ui.available_width();
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
                        if let Some(v) =
                            num_field(ui, fw, Lab::Letter("X"), "X position", dx, 0, 1.0, 1.0, full.clone())
                        {
                            ops.push(Op::SetBBox(Some(v), None, None, None, ax, ay));
                        }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("W"), "Width", s.w, 0, 1.0, 1.0, 0.0..=1.0e6) {
                            if *lock && s.w > 0.0 {
                                ops.push(Op::SetBBox(None, None, Some(v), Some(s.h * v / s.w), ax, ay));
                            } else {
                                ops.push(Op::SetBBox(None, None, Some(v), None, ax, ay));
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        let dy = s.y + ay * s.h;
                        if let Some(v) =
                            num_field(ui, fw, Lab::Letter("Y"), "Y position", dy, 0, 1.0, 1.0, full.clone())
                        {
                            ops.push(Op::SetBBox(None, Some(v), None, None, ax, ay));
                        }
                        if let Some(v) = num_field(ui, fw, Lab::Letter("H"), "Height", s.h, 0, 1.0, 1.0, 0.0..=1.0e6) {
                            if *lock && s.h > 0.0 {
                                ops.push(Op::SetBBox(None, None, Some(s.w * v / s.h), Some(v), ax, ay));
                            } else {
                                ops.push(Op::SetBBox(None, None, None, Some(v), ax, ay));
                            }
                        }
                    });
                });
                if icon_toggle(ui, ic.link, *lock, "Constrain W/H proportions") {
                    *lock = !*lock;
                }
            });

            // ── Angle + flip ──
            ui.horizontal(|ui| {
                if let Some(v) =
                    num_field(ui, 150.0, Lab::Icon(ic.rotate.as_ref()), "Rotation", s.rot, 1, 1.0, 0.5, full.clone())
                {
                    ops.push(Op::SetRot(v));
                }
                if icon_btn(ui, ic.fliph, "Flip horizontal") {
                    ops.push(Op::Flip(true));
                }
                if icon_btn(ui, ic.flipv, "Flip vertical") {
                    ops.push(Op::Flip(false));
                }
            });

            hsep(ui, inner);

            // Appearance: opacity
            if let Some(v) = num_field(
                ui,
                inner,
                Lab::Icon(ic.opacity.as_ref()),
                "Opacity %",
                s.opacity * 100.0,
                0,
                1.0,
                0.5,
                0.0..=100.0,
            ) {
                ops.push(Op::SetOpacity(v / 100.0));
            }

            hsep(ui, inner);

            // Fill / Stroke swatches + stroke weight
            paint_row(ui, PaintTarget::Fill, s.fill, ops);
            paint_row(ui, PaintTarget::Stroke, s.stroke, ops);
            if let Some(v) =
                num_field(ui, inner, Lab::Icon(ic.strokew.as_ref()), "Stroke weight", s.sw, 1, 0.5, 0.2, 0.0..=400.0)
            {
                ops.push(Op::SetStrokeW(v));
            }

            hsep(ui, inner);
            ui.label(RichText::new("SHAPE").color(MUTED).size(10.0).strong());
            ui.add_space(2.0);
            pathfinder_row(ui, ops); // a MIRROR of the Pathfinder home (the mockup's Shape section)
        });
    });
}

/// The Align pane body — THE home of align/distribute (ONE-HOME rule; the control bar mirrors it).
fn panel_align(ui: &mut egui::Ui, ic: &DockIcons, ops: &mut Vec<Op>) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
        ui.label(RichText::new("ALIGN OBJECTS").color(MUTED).size(10.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if icon_btn(ui, &ic.align[0], "Align left") {
                ops.push(Op::Align(AlignMode::Left));
            }
            if icon_btn(ui, &ic.align[1], "Align centre") {
                ops.push(Op::Align(AlignMode::CenterH));
            }
            if icon_btn(ui, &ic.align[2], "Align right") {
                ops.push(Op::Align(AlignMode::Right));
            }
            if icon_btn(ui, &ic.align[3], "Align top") {
                ops.push(Op::Align(AlignMode::Top));
            }
            if icon_btn(ui, &ic.align[4], "Align middle") {
                ops.push(Op::Align(AlignMode::Middle));
            }
            if icon_btn(ui, &ic.align[5], "Align bottom") {
                ops.push(Op::Align(AlignMode::Bottom));
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("DISTRIBUTE").color(MUTED).size(10.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if icon_btn(ui, &ic.align[6], "Distribute horizontal centres") {
                ops.push(Op::Distribute(DistAxis::Horizontal));
            }
            if icon_btn(ui, &ic.align[7], "Distribute vertical centres") {
                ops.push(Op::Distribute(DistAxis::Vertical));
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Align to").color(MUTED).size(11.5));
            ui.label(RichText::new("Selection").color(FAINT).size(11.5)); // honest: the engine aligns within the selection
        });
    });
}

/// The Pathfinder pane body — THE home of the boolean ops (the i_overlay engine via `Editor::pathfinder`).
fn panel_pathfinder(ui: &mut egui::Ui, ops: &mut Vec<Op>) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
        ui.label(RichText::new("SHAPE MODES").color(MUTED).size(10.0).strong());
        ui.add_space(2.0);
        pathfinder_row(ui, ops);
        ui.add_space(4.0);
        ui.label(RichText::new("Unite \u{b7} Minus Front \u{b7} Intersect \u{b7} Exclude").color(FAINT).size(10.5));
    });
}

/// The four boolean buttons (Unite / Minus Front / Intersect / Exclude) — hand-painted glyphs:
/// two overlapping squares with the op's region filled. Shared by the Pathfinder home + Shape mirror.
fn pathfinder_row(ui: &mut egui::Ui, ops: &mut Vec<Op>) {
    use varos_core::boolean::BoolOp;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (op, tip) in [
            (BoolOp::Unite, "Unite"),
            (BoolOp::MinusFront, "Minus Front"),
            (BoolOp::Intersect, "Intersect"),
            (BoolOp::Exclude, "Exclude"),
        ] {
            if pf_btn(ui, op, tip) {
                ops.push(Op::Bool(op));
            }
        }
    });
}

/// One pathfinder button: 34×28 hover chip; the glyph = two 12px squares overlapped by 4.
fn pf_btn(ui: &mut egui::Ui, op: varos_core::boolean::BoolOp, tip: &str) -> bool {
    use varos_core::boolean::BoolOp;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 28.0), egui::Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), HOVER);
    }
    let col = if resp.hovered() { TEXT } else { MUTED };
    let a = egui::Rect::from_min_size(rect.center() - egui::vec2(10.0, 8.0), egui::vec2(12.0, 12.0));
    let b = egui::Rect::from_min_size(rect.center() - egui::vec2(2.0, 4.0), egui::vec2(12.0, 12.0));
    let rr = CornerRadius::same(2);
    match op {
        BoolOp::Unite => {
            p.rect_filled(a, rr, col);
            p.rect_filled(b, rr, col);
        }
        BoolOp::MinusFront => {
            p.rect_filled(a, rr, col);
            p.rect_filled(b, rr, SOLID_PANEL);
            p.rect_stroke(b, rr, Stroke::new(1.0, col), StrokeKind::Middle);
        }
        BoolOp::Intersect => {
            p.rect_stroke(a, rr, Stroke::new(1.0, col), StrokeKind::Middle);
            p.rect_stroke(b, rr, Stroke::new(1.0, col), StrokeKind::Middle);
            p.rect_filled(a.intersect(b), CornerRadius::ZERO, col);
        }
        BoolOp::Exclude => {
            p.rect_filled(a, rr, col);
            p.rect_filled(b, rr, col);
            p.rect_filled(a.intersect(b), CornerRadius::ZERO, SOLID_PANEL);
        }
    }
    resp.on_hover_text(tip).clicked()
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
    let mut buf = if editing {
        ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| value.to_string())
    } else {
        value.to_string()
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(R),
        BG_SURFACE,
        Stroke::new(1.0, if editing { ACCENT } else { BORDER }),
        StrokeKind::Middle,
    );
    let te = ui.put(
        rect.shrink2(egui::vec2(8.0, 3.0)),
        egui::TextEdit::singleline(&mut buf)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(FontId::proportional(13.0))
            .text_color(TEXT),
    );
    if te.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    }
    let mut out = None;
    if te.lost_focus() {
        out = Some(buf.clone());
        ui.data_mut(|d| d.remove::<String>(id));
    }
    out
}

/// A label + a hand-painted pill switch (the Clip / transparent / move-with toggles). Returns true on click.
fn toggle_row(ui: &mut egui::Ui, w: f32, label: &str, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        if on { TEXT } else { MUTED },
    );
    let pill =
        egui::Rect::from_min_size(egui::pos2(rect.right() - 36.0, rect.center().y - 9.0), egui::vec2(32.0, 18.0));
    ui.painter().rect_filled(pill, CornerRadius::same(RCAP), if on { ACCENT } else { BG_SURFACE }); // capsule = one token (tabs + toggles)
    let knob = egui::pos2(if on { pill.right() - 9.0 } else { pill.left() + 9.0 }, pill.center().y);
    ui.painter().circle_filled(knob, 6.5, Color32::WHITE);
    resp.clicked()
}

/// A small text button (Add / Duplicate / Delete). `disabled` greys it out and swallows clicks.
fn pill_btn(ui: &mut egui::Ui, label: &str, disabled: bool) -> bool {
    let w = ui.available_width().clamp(40.0, 64.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    let hot = resp.hovered() && !disabled;
    ui.painter().rect(
        rect,
        CornerRadius::same(R),
        if hot { HOVER } else { BG_SURFACE },
        Stroke::new(1.0, BORDER),
        StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        if disabled { FAINT } else { TEXT },
    );
    resp.clicked() && !disabled
}

/// The Artboard inspector (Stage 4: the Properties pane's body while the Artboard tool is active).
fn panel_artboard(
    ui: &mut egui::Ui,
    s: &AbSnap,
    ic: &AbIcons,
    ab_lock: &mut bool,
    ops: &mut Vec<Op>,
    fit_request: &mut Option<usize>,
) {
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    let i = s.active;
    egui::ScrollArea::vertical().id_salt("ab-body").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
            let inner = ui.available_width();
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Artboard").color(TEXT).size(13.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} / {}", i + 1, s.count)).color(MUTED).size(11.5));
                });
            });
            if let Some(v) = name_field(ui, inner, &s.name, "dock") {
                ops.push(Op::AbName(i, v));
            }

            ui.add_space(2.0);
            ui.label(RichText::new("SIZE").color(MUTED).size(10.0).strong());
            // preset dropdown
            let preset_id = ui.make_persistent_id("ab-preset");
            let (prect, presp) = ui.allocate_exact_size(egui::vec2(inner, 26.0), egui::Sense::click());
            ui.painter().rect(
                prect,
                CornerRadius::same(R),
                BG_SURFACE,
                Stroke::new(1.0, if presp.hovered() { BORDER_2 } else { BORDER }),
                StrokeKind::Middle,
            );
            ui.painter().text(
                egui::pos2(prect.left() + 10.0, prect.center().y),
                Align2::LEFT_CENTER,
                "Presets\u{2026}",
                FontId::proportional(12.5),
                TEXT,
            );
            ui.painter().text(
                egui::pos2(prect.right() - 10.0, prect.center().y),
                Align2::RIGHT_CENTER,
                "\u{25be}",
                FontId::proportional(11.0),
                MUTED,
            );
            if presp.clicked() {
                menu_toggle(ui, preset_id);
            }
            menu_below(ui, preset_id, &presp, None, |ui| {
                ui.set_width(inner);
                for (label, w, h) in AB_PRESETS {
                    if menu_row(ui, label, "") {
                        ops.push(Op::AbRect(i, None, None, Some(w), Some(h)));
                        menu_set(ui, preset_id, false);
                    }
                }
            });
            // W / H + constrain
            ui.horizontal(|ui| {
                let fw = 70.0;
                if let Some(v) = num_field(ui, fw, Lab::Letter("W"), "Width", s.w, 0, 1.0, 1.0, 1.0..=1.0e6) {
                    if *ab_lock && s.w > 0.0 {
                        ops.push(Op::AbRect(i, None, None, Some(v), Some(s.h * v / s.w)));
                    } else {
                        ops.push(Op::AbRect(i, None, None, Some(v), None));
                    }
                }
                if let Some(v) = num_field(ui, fw, Lab::Letter("H"), "Height", s.h, 0, 1.0, 1.0, 1.0..=1.0e6) {
                    if *ab_lock && s.h > 0.0 {
                        ops.push(Op::AbRect(i, None, None, Some(s.w * v / s.h), Some(v)));
                    } else {
                        ops.push(Op::AbRect(i, None, None, None, Some(v)));
                    }
                }
                if icon_toggle(ui, ic.link, *ab_lock, "Constrain W/H") {
                    *ab_lock = !*ab_lock;
                }
            });
            // orientation + fit
            ui.horizontal(|ui| {
                let portrait = s.h >= s.w;
                if icon_toggle(ui, ic.portrait, portrait, "Portrait") && !portrait {
                    ops.push(Op::AbOrient(i));
                }
                if icon_toggle(ui, ic.landscape, !portrait, "Landscape") && portrait {
                    ops.push(Op::AbOrient(i));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_btn(ui, ic.fit, "Fit in window") {
                        *fit_request = Some(i);
                    }
                });
            });
            // X / Y
            ui.horizontal(|ui| {
                let fw = 70.0;
                if let Some(v) = num_field(ui, fw, Lab::Letter("X"), "X position", s.x, 0, 1.0, 1.0, full.clone()) {
                    ops.push(Op::AbRect(i, Some(v), None, None, None));
                }
                if let Some(v) = num_field(ui, fw, Lab::Letter("Y"), "Y position", s.y, 0, 1.0, 1.0, full.clone()) {
                    ops.push(Op::AbRect(i, None, Some(v), None, None));
                }
            });

            hsep(ui, inner);
            // page colour + transparent — same hand-painted picker
            ui.horizontal(|ui| {
                let col = s.color;
                let (sw, resp) = ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::click());
                let round = CornerRadius::same(4);
                match col {
                    Some(c) => {
                        if c[3] < 0.999 {
                            checker(&ui.painter_at(sw), sw, 5.0);
                        }
                        ui.painter().rect_filled(sw, round, rgba_c32a(c));
                    }
                    None => {
                        ui.painter().rect_filled(sw, round, SWATCH_WELL);
                        ui.painter().line_segment(
                            [sw.left_bottom() + egui::vec2(2.0, -2.0), sw.right_top() + egui::vec2(-2.0, 2.0)],
                            Stroke::new(1.6, NONE_RED),
                        );
                    }
                }
                ui.painter().rect_stroke(sw, round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
                // click → the Color Picker modal (a settings row: no focus semantics to preserve)
                if resp.clicked() || resp.double_clicked() {
                    ops.push(Op::OpenPicker(MTarget::Ab(i)));
                }
                let _ = col;
                ui.add_space(8.0);
                ui.label(
                    RichText::new(match s.color {
                        Some(c) => hex_of(c),
                        None => "Transparent".into(),
                    })
                    .color(TEXT)
                    .monospace()
                    .size(12.0),
                );
            });
            if toggle_row(ui, inner, "Transparent page", s.color.is_none()) {
                ops.push(Op::AbColor(i, if s.color.is_none() { Some([1.0, 1.0, 1.0, 1.0]) } else { None }));
            }

            hsep(ui, inner);
            if let Some(v) =
                num_field(ui, inner, Lab::Letter("#"), "Artboard count", s.count as f32, 0, 1.0, 0.1, 1.0..=200.0)
            {
                ops.push(Op::AbCount(v.round().max(1.0) as usize));
            }
            if toggle_row(ui, inner, "Clip to page", s.clip) {
                ops.push(Op::AbClip(i));
            }
            if toggle_row(ui, inner, "Move artwork with artboard", s.move_art) {
                ops.push(Op::AbMoveArt(!s.move_art));
            }

            hsep(ui, inner);
            ui.horizontal(|ui| {
                if pill_btn(ui, "+ Add", false) {
                    ops.push(Op::AbAdd);
                }
                if pill_btn(ui, "Duplicate", false) {
                    ops.push(Op::AbDup(i));
                }
                if pill_btn(ui, "Delete", s.count <= 1) {
                    ops.push(Op::AbDel(i));
                }
            });
        });
    });
}

/// On-canvas page chrome: a name label (top-left of each page) + a ⋮ button opening the edit menu. The
/// menu is the ungated way to edit a page from ANY tool (Decision 8); selecting a page by clicking its
/// name only works in the Artboard tool. Positions are pinned to each page via the view transform.
#[allow(clippy::too_many_arguments)] // hand-painted panel builder: each arg is live UI state, split deferred with ui.rs
fn build_ab_chrome(
    ctx: &egui::Context,
    view: View,
    ppp: f32,
    hole: egui::Rect,
    abs: &[AbInfo],
    active: usize,
    tool_ab: bool,
    count: usize,
    ops: &mut Vec<Op>,
    name_edit: &mut Option<(usize, String)>,
    fit_request: &mut Option<usize>,
) {
    let mut clear_edit = false;
    for ab in abs {
        if ab.hidden {
            continue; // board eye OFF → the name chrome vanishes with the page
        }
        // Page corners in screen POINTS (view maps world→physical px; egui works in points). The
        // chrome is PINNED to its page — no clamping, no following the viewport (Ahmed 07-07:
        // "عاوزه ثابت زيه زي الأرت بورد") — and CLIPS at the hole so it never paints over rulers,
        // seams or panels. Name + size sit top-LEFT; the ⋯ settings sit top-RIGHT.
        let tl = view.w2s([ab.x, ab.y]);
        let tr = view.w2s([ab.x + ab.w, ab.y]);
        let y = tl[1] / ppp - 24.0;
        let is_active = ab.i == active;
        let out_v = y + 20.0 < hole.top() || y > hole.bottom() - 8.0;

        // ── name + size (top-left): pure INFO — never a click target, never selects (Ahmed 07-07) ──
        let pos = egui::pos2(tl[0] / ppp, y);
        let renaming = matches!(&*name_edit, Some((j, _)) if *j == ab.i);
        if !(out_v || pos.x + 30.0 < hole.left() || pos.x > hole.right() - 8.0) {
            egui::Area::new(egui::Id::new(("ab-chrome", ab.i)))
                .fixed_pos(pos)
                .order(egui::Order::Middle)
                .constrain(false)
                // interactable only while the rename field is up — otherwise pure paint that lets
                // clicks AND scroll pass straight through to the canvas (Ahmed 07-06)
                .interactable(tool_ab && renaming)
                .show(ctx, |ui| {
                    ui.set_clip_rect(hole); // hard edge: nothing bleeds past the board box
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if renaming {
                            if let Some((_, buf)) = name_edit.as_mut() {
                                let te = ui.add(
                                    egui::TextEdit::singleline(buf)
                                        .desired_width(110.0)
                                        .font(FontId::proportional(11.5))
                                        .text_color(TEXT),
                                );
                                if te.lost_focus() {
                                    ops.push(Op::AbName(ab.i, buf.clone()));
                                    clear_edit = true;
                                } else {
                                    te.request_focus();
                                }
                            }
                        } else {
                            let col = if is_active { TEXT } else { MUTED };
                            ui.label(RichText::new(&ab.name).color(col).size(11.0));
                        }
                        // the page size, quietly beside the name (Ahmed 07-07 "المقاس مكتوب جمبه")
                        ui.label(
                            RichText::new(format!("{:.0} \u{00d7} {:.0}", ab.w, ab.h))
                                .color(FAINT)
                                .monospace()
                                .size(10.0),
                        );
                    });
                });
        }

        // ── ⋯ settings (top-right): three HORIZONTAL dots, a touch bigger (Ahmed 07-07) ──
        let dpos = egui::pos2(tr[0] / ppp - 26.0, y);
        if !(out_v || dpos.x + 26.0 < hole.left() || dpos.x > hole.right() - 8.0) {
            egui::Area::new(egui::Id::new(("ab-dots", ab.i)))
                .fixed_pos(dpos)
                .order(egui::Order::Middle)
                .constrain(false)
                .interactable(tool_ab)
                .show(ctx, |ui| {
                    ui.set_clip_rect(hole);
                    let (dr, dresp) = ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::click());
                    if dresp.hovered() {
                        ui.painter().rect_filled(dr, CornerRadius::same(4), HOVER);
                    }
                    let col = if dresp.hovered() || is_active { TEXT } else { MUTED };
                    for k in [-1.0f32, 0.0, 1.0] {
                        ui.painter().circle_filled(egui::pos2(dr.center().x + k * 6.5, dr.center().y), 1.8, col);
                    }
                    let menu_id = ui.make_persistent_id(("ab-menu", ab.i));
                    if dresp.clicked() {
                        menu_toggle(ui, menu_id);
                    }
                    menu_below(ui, menu_id, &dresp, None, |ui| {
                        ui.set_width(190.0);
                        if menu_row(ui, "Rename", "") {
                            *name_edit = Some((ab.i, ab.name.clone()));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, "Duplicate", "") {
                            ops.push(Op::AbDup(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, if ab.h >= ab.w { "Make landscape" } else { "Make portrait" }, "") {
                            ops.push(Op::AbOrient(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, if ab.transparent { "White background" } else { "Transparent" }, "") {
                            ops.push(Op::AbColor(ab.i, if ab.transparent { Some([1.0, 1.0, 1.0, 1.0]) } else { None }));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, if ab.clip { "Unclip" } else { "Clip to page" }, "") {
                            ops.push(Op::AbClip(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, "Fit in window", "") {
                            *fit_request = Some(ab.i);
                            menu_set(ui, menu_id, false);
                        }
                        if count > 1 && menu_row(ui, "Delete", "") {
                            ops.push(Op::AbDel(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                    });
                });
        }
    }
    if clear_edit {
        *name_edit = None;
    }
}

// ───────────────────────────── rulers ─────────────────────────────

const RULER: f32 = 18.0; // ruler strip thickness in points

/// Decimals needed to print multiples of `grid` cleanly (grid is a power of 5: 25→0, 0.2→1, 0.04→2).
fn ruler_dec(grid: f32) -> usize {
    (-grid.log10()).ceil().max(0.0) as usize
}

fn fmt_ruler(v: f32, grid: f32, dec: usize) -> String {
    let v = if v.abs() < grid * 0.001 { 0.0 } else { v }; // kill -0
    format!("{:.*}", dec, v)
}

/// Top + left rulers (Ctrl+R), drawn INSIDE the Board box (Stage 4): two 18px strips hugging its top
/// and left edges (§3.5: bg = ruler_bg). Ticks sit on the SAME base-5 lattice as the dot grid; every
/// big tick is labeled. Numbers read relative to `origin`; a live tick tracks the pointer; the corner
/// box drag-sets the origin (snapped) and double-click resets it.
#[allow(clippy::too_many_arguments)]
fn board_rulers(
    ui: &mut egui::Ui,
    board: egui::Rect,
    view: View,
    ppp: f32,
    grid: f32,
    origin: [f32; 2],
    reset: [f32; 2],
    ops: &mut Vec<Op>,
) {
    let num_font = FontId::proportional(9.5);
    let dec = ruler_dec(grid);
    // label every Nth grid-tick so numbers stay ~70 pts apart at ANY zoom (dense, never a vast gap). N is a
    // grid multiple, so labels still land on dots; small N → rounder numbers (×2 = 250s, not ×5 = 625s).
    let ms_pts = grid * view.zoom / ppp.max(1e-6); // one grid-tick in screen points
    let label_every =
        [1i64, 2, 5, 10, 20, 50, 100, 200].into_iter().find(|&n| n as f32 * ms_pts >= 70.0).unwrap_or(200);
    let pointer = ui.ctx().pointer_latest_pos();

    // top (horizontal) ruler — ticks at WORLD multiples of `grid` (land on the dots), label every 5th
    {
        let r = egui::Rect::from_min_max(board.left_top(), egui::pos2(board.right(), board.top() + RULER));
        let p = ui.painter_at(r);
        p.rect_filled(r, CornerRadius::ZERO, RULER_BG);
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, BORDER));
        let x_lo = r.left() + RULER; // ticks start after the corner box
        let v_lo = view.s2w([x_lo * ppp, 0.0])[0] - origin[0];
        let v_hi = view.s2w([r.right() * ppp, 0.0])[0] - origin[0];
        let m1 = (v_hi / grid).ceil() as i64;
        let mut m = (v_lo / grid).floor() as i64;
        while m <= m1 {
            let val = m as f32 * grid; // value shown — relative to the origin, so m==0 is ZERO
            let sx = view.w2s([origin[0] + val, 0.0])[0] / ppp;
            let (big, zero) = (m.rem_euclid(label_every) == 0, m == 0);
            m += 1;
            if sx < x_lo - 0.5 || sx > r.right() + 0.5 {
                continue;
            }
            let h = if zero {
                12.0
            } else if big {
                9.0
            } else {
                5.0
            };
            p.vline(
                sx,
                (r.bottom() - h)..=r.bottom(),
                Stroke::new(
                    1.0,
                    if zero {
                        TEXT
                    } else if big {
                        MUTED
                    } else {
                        BORDER_2
                    },
                ),
            );
            if big {
                p.text(
                    egui::pos2(sx + 2.5, r.top() + 1.0),
                    Align2::LEFT_TOP,
                    fmt_ruler(val, grid, dec),
                    num_font.clone(),
                    if zero { TEXT } else { MUTED },
                );
            }
        }
        if let Some(pt) = pointer {
            p.vline(pt.x, r.y_range(), Stroke::new(1.0, ACCENT));
        }
        // drag off the strip body → pull out a HORIZONTAL guide (follows the pointer onto the canvas)
        let body = egui::Rect::from_min_max(egui::pos2(r.left() + RULER, r.top()), r.right_bottom());
        let dr = ui.interact(body, ui.id().with("ruler-h-body"), egui::Sense::drag());
        if dr.dragged() {
            if let Some(pp) = ui.ctx().pointer_latest_pos() {
                ops.push(Op::GuidePreview(false, view.s2w([pp.x * ppp, pp.y * ppp])));
            }
        }
        if dr.drag_stopped() {
            ops.push(Op::GuideCommit);
        }
        // corner box: drag sets the origin (snapped), double-click resets it
        let corner = egui::Rect::from_min_size(r.left_top(), egui::vec2(RULER, RULER));
        let resp = ui.interact(corner, ui.id().with("ruler-corner"), egui::Sense::click_and_drag());
        p.rect_filled(corner, CornerRadius::ZERO, RULER_BG);
        p.vline(corner.right() - 0.5, r.y_range(), Stroke::new(1.0, BORDER));
        let c = corner.center();
        p.line_segment([egui::pos2(c.x - 3.0, c.y), egui::pos2(c.x + 3.0, c.y)], Stroke::new(1.0, MUTED));
        p.line_segment([egui::pos2(c.x, c.y - 3.0), egui::pos2(c.x, c.y + 3.0)], Stroke::new(1.0, MUTED));
        if resp.double_clicked() {
            ops.push(Op::RulerOrigin(Some(reset)));
            ops.push(Op::RulerOrigin(None));
        } else if resp.dragged() {
            if let Some(pp) = ui.ctx().pointer_latest_pos() {
                ops.push(Op::RulerOrigin(Some(view.s2w([pp.x * ppp, pp.y * ppp]))));
            }
        }
        if resp.drag_stopped() {
            ops.push(Op::RulerOrigin(None));
        }
    }

    // left (vertical) ruler — numbers rotated 90° (read upward), like Illustrator
    {
        let r = egui::Rect::from_min_max(
            egui::pos2(board.left(), board.top() + RULER),
            egui::pos2(board.left() + RULER, board.bottom()),
        );
        let p = ui.painter_at(r);
        p.rect_filled(r, CornerRadius::ZERO, RULER_BG);
        p.vline(r.right() - 0.5, r.y_range(), Stroke::new(1.0, BORDER));
        let v_lo = view.s2w([0.0, r.top() * ppp])[1] - origin[1];
        let v_hi = view.s2w([0.0, r.bottom() * ppp])[1] - origin[1];
        let m1 = (v_hi / grid).ceil() as i64;
        let mut m = (v_lo / grid).floor() as i64;
        while m <= m1 {
            let val = m as f32 * grid; // value shown — relative to the origin, so m==0 is ZERO
            let sy = view.w2s([0.0, origin[1] + val])[1] / ppp;
            let (big, zero) = (m.rem_euclid(label_every) == 0, m == 0);
            m += 1;
            if sy < r.top() - 0.5 || sy > r.bottom() + 0.5 {
                continue;
            }
            let w = if zero {
                12.0
            } else if big {
                9.0
            } else {
                5.0
            };
            p.hline(
                (r.right() - w)..=r.right(),
                sy,
                Stroke::new(
                    1.0,
                    if zero {
                        TEXT
                    } else if big {
                        MUTED
                    } else {
                        BORDER_2
                    },
                ),
            );
            if big {
                let col = if zero { TEXT } else { MUTED };
                let galley = p.layout_no_wrap(fmt_ruler(val, grid, dec), num_font.clone(), col);
                let mut ts =
                    egui::epaint::TextShape::new(egui::pos2(r.left() + 2.0, sy + galley.size().x / 2.0), galley, col);
                ts.angle = -std::f32::consts::FRAC_PI_2;
                p.add(ts);
            }
        }
        if let Some(pt) = pointer {
            p.hline(r.x_range(), pt.y, Stroke::new(1.0, ACCENT));
        }
        // drag off the strip → pull out a VERTICAL guide (follows the pointer onto the canvas)
        let dr = ui.interact(r, ui.id().with("ruler-v-body"), egui::Sense::drag());
        if dr.dragged() {
            if let Some(pp) = ui.ctx().pointer_latest_pos() {
                ops.push(Op::GuidePreview(true, view.s2w([pp.x * ppp, pp.y * ppp])));
            }
        }
        if dr.drag_stopped() {
            ops.push(Op::GuideCommit);
        }
    }
}

/// While the ruler zero-point is being dragged, a full-canvas DASHED crosshair (vertical = X, horizontal
/// = Y) marks where the new origin will land — drawn at the SNAPPED position, so the snap to a corner /
/// anchor / grid dot is visible and you can see exactly where (0,0) is going.
fn build_origin_crosshair(
    ctx: &egui::Context,
    view: View,
    ppp: f32,
    hole: egui::Rect,
    preview: Option<varos_core::geom::Pt>,
) {
    let Some(w) = preview else {
        return;
    };
    let s = view.w2s(w);
    let (sx, sy) = (s[0] / ppp, s[1] / ppp);
    let scr = hole; // Stage 4: the crosshair belongs to the canvas — never over the boxes
    let p = ctx
        .layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("origin-cross")))
        .with_clip_rect(hole);
    let stroke = Stroke::new(1.0, ACCENT);
    for seg in egui::Shape::dashed_line(&[egui::pos2(sx, scr.top()), egui::pos2(sx, scr.bottom())], stroke, 5.0, 4.0) {
        p.add(seg);
    }
    for seg in egui::Shape::dashed_line(&[egui::pos2(scr.left(), sy), egui::pos2(scr.right(), sy)], stroke, 5.0, 4.0) {
        p.add(seg);
    }
}

/// The live measurement HUD — a small pill near the cursor showing the drag readout (X/Y position now;
/// W×H / angle later). Pure feedback on a foreground layer; no interaction, never blocks the canvas.
fn build_snap_hud(
    ctx: &egui::Context,
    view: View,
    ppp: f32,
    hole: egui::Rect,
    hud: &Option<(varos_core::geom::Pt, String)>,
) {
    let (wp, text) = match hud {
        Some(h) => h,
        None => return,
    };
    let sp = view.w2s(*wp);
    let anchor = egui::pos2(sp[0] / ppp + 15.0, sp[1] / ppp - 26.0);
    // Stage 4: the HUD rides the canvas — clip so it never floats over the boxes
    let p =
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("snap-hud"))).with_clip_rect(hole);
    let font = FontId::proportional(12.0);
    let galley = p.layout_no_wrap(text.clone(), font.clone(), TEXT);
    let rect = egui::Rect::from_min_size(anchor, galley.size() + egui::vec2(14.0, 7.0));
    p.rect_filled(rect, CornerRadius::same(R), SOLID_PANEL);
    p.rect_stroke(rect, CornerRadius::same(R), Stroke::new(1.0, BORDER), StrokeKind::Middle);
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
            Op::Paint(tg, c) => {
                ed.paint = tg;
                ed.apply_paint(c);
            }
            Op::Recent(c) => ed.push_recent(c),
            Op::PaintFocus(t) => ed.paint = t,
            Op::SwapColors => ed.swap_colors(),
            Op::DefaultPaint => ed.default_paint(),
            Op::OpenPicker(_) => {} // UI-only: intercepted in run() (opens the modal); never reaches here
            Op::LayerSelectSet(nids) => ed.layer_select_set(&nids),
            Op::LayerToggle(n) => ed.layer_toggle(n),
            Op::LayerEye(n) => ed.layer_toggle_hidden(n),
            Op::LayerLock(n) => ed.layer_toggle_locked(n),
            Op::LayerRename(n, s) => ed.layer_rename(n, s),
            Op::LayerGroup => ed.group_selection(),
            Op::LayerDeleteSel => ed.layer_delete_selection(),
            Op::LayerMove(srcs, target, zone) => {
                let pos = match zone {
                    0 => varos_core::model::DropPos::Before,
                    1 => varos_core::model::DropPos::Into,
                    _ => varos_core::model::DropPos::After,
                };
                ed.layer_move(&srcs, target, pos);
            }
            Op::LayerMoveBoard(srcs, sb, tb) => ed.layer_move_to_board(&srcs, sb, tb),
            Op::AbEye(i) => ed.ab_toggle_hidden(i),
            Op::AbLock(i) => ed.ab_toggle_locked(i),
            Op::LayerDupMove(srcs, target, zone) => {
                let pos = match zone {
                    0 => varos_core::model::DropPos::Before,
                    1 => varos_core::model::DropPos::Into,
                    _ => varos_core::model::DropPos::After,
                };
                ed.layer_dup_move(&srcs, target, pos);
            }
            Op::Flip(h) => ed.flip(h),
            Op::Align(m) => ed.align(m),
            Op::Distribute(a) => ed.distribute(a),
            Op::Bool(op) => ed.pathfinder(op),
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
            Op::RulerOrigin(Some(p)) => {
                ed.doc.ruler_origin = ed.snap_origin(p);
                ed.origin_preview = Some(ed.doc.ruler_origin);
            }
            Op::RulerOrigin(None) => ed.origin_preview = None,
            Op::GuidePreview(vertical, p) => ed.set_guide_preview(vertical, p),
            Op::GuideCommit => ed.commit_guide(),
        }
    }
}

/// stroke width across the object selection — pub fields only (varos-core untouched).
fn set_stroke_width(ed: &mut Editor, w: f32) {
    let pids: Vec<u32> = ed.objsel.iter().copied().collect();
    if pids.is_empty() {
        ed.cur_sw = w.max(0.0);
        return;
    }
    ed.begin();
    for pid in pids {
        if let Some(pi) = ed.doc.pidx(pid) {
            ed.doc.paths[pi].stroke_width = w.max(0.0);
        }
    }
    ed.dirty = true;
    ed.commit();
}

/// Dev-only: composite the rail to a PNG so the icon rasterization can be eyeballed without the
/// native window. `varos.exe --dump-tool-icons <path>`.
pub fn dump_tool_icons(path: &str) {
    let icons = [
        IC_SELECT,
        IC_DIRECT,
        IC_PEN,
        IC_RECT,
        IC_ELLIPSE,
        IC_TRIANGLE,
        IC_EYE,
        IC_ROTATE,
        IC_OPACITY,
        IC_STROKEW,
        IC_LINK,
        IC_FLIPH,
        IC_FLIPV,
    ];
    let n = icons.len() as u32;
    let (pad, btn, gap, icon) = (7u32, 40u32, 4u32, 24u32);
    let w = btn + pad * 2;
    let h = pad * 2 + btn * n + gap * (n - 1);
    let panel = [0x1fu8, 0x1f, 0x22, 255];
    let accent = [0x0cu8, 0x8c, 0xe9, 255];
    let mut img = vec![0u8; (w * h * 4) as usize];
    for px in img.chunks_mut(4) {
        px.copy_from_slice(&panel);
    }
    for (i, svg) in icons.iter().enumerate() {
        let by = pad + i as u32 * (btn + gap);
        if i == 2 {
            for yy in by..by + btn {
                for xx in pad..pad + btn {
                    let o = ((yy * w + xx) * 4) as usize;
                    img[o..o + 4].copy_from_slice(&accent);
                }
            }
        }
        if let Some((rgba, iw, ih)) = crate::cursors::render_svg(&lucide(svg), icon, false) {
            let (ox, oy) = (pad + (btn - iw) / 2, by + (btn - ih) / 2);
            for yy in 0..ih {
                for xx in 0..iw {
                    let si = ((yy * iw + xx) * 4) as usize;
                    let a = rgba[si + 3] as u32;
                    if a == 0 {
                        continue;
                    }
                    let di = (((oy + yy) * w + (ox + xx)) * 4) as usize;
                    for c in 0..3 {
                        img[di + c] = ((rgba[si + c] as u32 * a + img[di + c] as u32 * (255 - a)) / 255) as u8;
                    }
                    img[di + 3] = 255;
                }
            }
        }
    }
    if let Some(im) = image::RgbaImage::from_raw(w, h, img) {
        let _ = im.save(path);
    }
}
