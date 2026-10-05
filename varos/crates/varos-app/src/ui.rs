//! Native GPU UI — hand-painted chrome on OUR wgpu surface via `Renderer::render_ui` (egui shares our
//! Device/Queue; no second window). egui is only canvas + input + layout; every widget is drawn by us
//! so it matches the Figma, not egui's dev-tool defaults. Pieces so far: the left TOOL RAIL and the
//! right INSPECTOR DOCK (Transform / Appearance / Fill / Stroke). Solid panels, one light GPU shadow,
//! no glass. Panels read a per-frame snapshot of the editor and push deferred `Op`s, applied to
//! `&mut Editor` after layout (no IPC, no borrow fights). varos-core itself is untouched.

use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Stroke, StrokeKind};
use std::hash::{Hash, Hasher};
use std::time::Instant;
use varos_core::editor::{AlignMode, AlignTarget, DistAxis, Editor, PaintTarget, ToolKind};
use varos_core::geom::{Pt, Rgba, View};
use varos_core::EditCommand;
use winit::event::WindowEvent;

use crate::app_command::{AppCommand, SessionId, TabView};
use winit::window::Window;

// Stage 0b (BOX_SYSTEM_PLAN §6, ruling 4): the palette now comes from the LAW ramp — the warm black
// (R ≥ G ≥ B, tokens.rs = UI_VISION_MOCKUP's :root). The old cool-gray names alias their warm
// successors while the split modules retain the established body names.
use varos_app::shell::tokens::{
    numeric_value, shortcut_label, ACCENT, ACCENT_HOVER, ACCENT_TINT, CLOSE_RED, DISABLED, HOVER, LINE as BORDER,
    LINE2 as BORDER_2, MUTED, NONE_RED, PANEL as SOLID_PANEL, R, RBOX, RCAP, ROW_HOVER, RULER_BG, SEAM,
    SURFACE as BG_SURFACE, SURFACE as SWATCH_WELL, TEXT,
};
// Icon stage 1: one icon registry + one icon button (shell::kit), one set of icon sizes (tokens).
use varos_app::shell::kit::field::Label as Lab;
use varos_app::shell::kit::icons::{
    legacy_texture, LEGACY_AL_B, LEGACY_AL_CH, LEGACY_AL_L, LEGACY_AL_M, LEGACY_AL_R, LEGACY_AL_T, LEGACY_ARTBOARD,
    LEGACY_DIRECT, LEGACY_DIST_H, LEGACY_DIST_V, LEGACY_ELLIPSE, LEGACY_EYE, LEGACY_FIT, LEGACY_L_EYE, LEGACY_L_EYEOFF,
    LEGACY_L_LOCK, LEGACY_L_SEARCH, LEGACY_L_UNLOCK, LEGACY_MENU, LEGACY_OPACITY, LEGACY_PEN, LEGACY_POLYGON,
    LEGACY_RECT, LEGACY_ROTATE, LEGACY_SCALE, LEGACY_SELECT, LEGACY_STROKEW, LEGACY_TRIANGLE,
};
use varos_app::shell::kit::{self, Icon};

mod fields;
use varos_app::shell::tokens::{ICON_BTN_H, ICON_BTN_W, ICON_LG, ICON_MD, ICON_SM};

// Lucide icon path data (white-stroked at render time), same set as the web rail.

mod bar;
mod canvas_overlay;
mod control_bar;
mod controls;
mod menus;
mod ops;
mod panels;
mod picker;
mod rail;
mod snap;
mod style;
#[cfg(test)]
mod tests;

use bar::*;
use canvas_overlay::*;
use control_bar::*;
use controls::*;
use menus::*;
pub(crate) use ops::dump_tool_icons;
use ops::*;
use panels::*;
use picker::*;
use rail::*;
use snap::*;
use style::*;

// ───────────────────────────── icon actions (icon stage 1) ─────────────────────────────

/// A shortcut hint shown in an icon button's tooltip.
#[derive(Clone, Copy)]
enum Hint {
    None,
    /// The platform primary modifier + key (⌘G on Mac, Ctrl+G elsewhere).
    Primary(&'static str),
    /// A plain key name.
    Key(&'static str),
}

/// One panel icon button: its stable key, its registry glyph, the text label it used to show (now its
/// tooltip) and its shortcut. Every icon button in the panels is drawn from [`ICON_ACTIONS`], so the
/// tooltip test covers all of them (ICON_LIBRARY_STUDY §4; owner: "icons instead of text").
#[derive(Clone, Copy)]
struct IconAction {
    key: &'static str,
    icon: Icon,
    label: &'static str,
    hint: Hint,
}
impl IconAction {
    /// The label plus its shortcut — what the button says on hover.
    fn tooltip(&self) -> String {
        match self.hint {
            Hint::None => self.label.to_string(),
            Hint::Primary(k) => format!("{} ({})", self.label, shortcut_label(k)),
            Hint::Key(k) => format!("{} ({k})", self.label),
        }
    }
    /// Draw it through the one kit control; true once on activation (pointer or Enter/Space).
    fn show(&self, ui: &mut egui::Ui, state: kit::IconState<'_>) -> bool {
        let id = ui.make_persistent_id(("icon-action", self.key));
        let r = kit::icon_button(ui, id, self.icon, &self.tooltip(), state);
        #[cfg(test)]
        tests::icon_action_tests::PROBE.with(|p| p.borrow_mut().push((self.key, id, r.response.rect)));
        r.activated
    }
}

const IA_LAYER_GROUP: IconAction =
    IconAction { key: "layer-group", icon: Icon::Group, label: "Group the selection", hint: Hint::Primary("G") };
const IA_LAYER_DELETE: IconAction =
    IconAction { key: "layer-delete", icon: Icon::Trash, label: "Delete the selection", hint: Hint::Key("Delete") };
const IA_AB_ADD: IconAction =
    IconAction { key: "ab-add", icon: Icon::ArtboardAdd, label: "Add artboard", hint: Hint::None };
const IA_AB_DUP: IconAction =
    IconAction { key: "ab-dup", icon: Icon::Duplicate, label: "Duplicate artboard", hint: Hint::None };
const IA_AB_DEL: IconAction =
    IconAction { key: "ab-del", icon: Icon::Trash, label: "Delete artboard", hint: Hint::None };
const IA_AB_LINK: IconAction =
    IconAction { key: "ab-link", icon: Icon::Link, label: "Constrain W/H", hint: Hint::None };
const IA_AB_PORTRAIT: IconAction =
    IconAction { key: "ab-portrait", icon: Icon::Portrait, label: "Portrait", hint: Hint::None };
const IA_AB_LANDSCAPE: IconAction =
    IconAction { key: "ab-landscape", icon: Icon::Landscape, label: "Landscape", hint: Hint::None };
const IA_AB_FIT: IconAction =
    IconAction { key: "ab-fit", icon: Icon::Fit, label: "Fit in window", hint: Hint::Primary("0") };
const IA_PROP_LINK: IconAction =
    IconAction { key: "prop-link", icon: Icon::Link, label: "Constrain W/H proportions", hint: Hint::None };
const IA_FLIP_H: IconAction =
    IconAction { key: "flip-h", icon: Icon::FlipH, label: "Flip horizontal", hint: Hint::None };
const IA_FLIP_V: IconAction = IconAction { key: "flip-v", icon: Icon::FlipV, label: "Flip vertical", hint: Hint::None };
const IA_NO_FILL: IconAction = IconAction { key: "no-fill", icon: Icon::Remove, label: "No paint", hint: Hint::None };
const IA_NO_STROKE: IconAction =
    IconAction { key: "no-stroke", icon: Icon::Remove, label: "No paint", hint: Hint::None };
const IA_PICKER_CLOSE: IconAction =
    IconAction { key: "picker-close", icon: Icon::Remove, label: "Close", hint: Hint::Key("Esc") };

/// Every panel icon action, for the tooltip/emission tests.
#[cfg(test)]
const ICON_ACTIONS: [IconAction; 15] = [
    IA_LAYER_GROUP,
    IA_LAYER_DELETE,
    IA_AB_ADD,
    IA_AB_DUP,
    IA_AB_DEL,
    IA_AB_LINK,
    IA_AB_PORTRAIT,
    IA_AB_LANDSCAPE,
    IA_AB_FIT,
    IA_PROP_LINK,
    IA_FLIP_H,
    IA_FLIP_V,
    IA_NO_FILL,
    IA_NO_STROKE,
    IA_PICKER_CLOSE,
];

/// A window action the custom title bar asks the host (winit) to perform.
pub enum WinAction {
    Minimize,
    ToggleMaximize,
    Close,
    /// The band's V mark (4b, macOS): the native About panel.
    About,
}

struct ToolBtn {
    pub(crate) kind: ToolKind,
    pub(crate) tip: &'static str,
    pub(crate) tex: Option<egui::TextureHandle>,
    pub(crate) group_end: bool,
}

pub struct Ui {
    ctx: egui::Context,
    state: egui_winit::State,
    pub repaint: bool,
    pub repaint_at: Option<Instant>,
    pub recovery: crate::recovery_host::RecoveryUi,
    /// Background save / export status for the status bar (`file_jobs::status_text`); when set it
    /// takes the recovery status's place.
    pub file_status: String,
    /// DFS S6: the open Export PDF sheet and each tab's last scope; 4b: the panel column's x-span as
    /// last laid out (the band's Search + V zone, and the sheet's right edge — one frame late).
    export_sheet: Option<crate::export_ui::ExportSheet>,
    panel_column: Option<egui::Rangef>,
    export_scopes: std::collections::HashMap<SessionId, varos_pdf::ExportScope>,
    tools: Vec<ToolBtn>,    // rail singletons: Object · Direct · Artboard · Pen · Eyedropper
    shapes: Vec<ToolBtn>,   // the shape tools, collapsed into one rail slot (right-click → flyout)
    shape_active: ToolKind, // which shape the shapes slot currently represents
    ic_rotate: Option<egui::TextureHandle>,
    ic_opacity: Option<egui::TextureHandle>,
    ic_strokew: Option<egui::TextureHandle>,
    ic_fit: Option<egui::TextureHandle>,
    ic_pipette: Option<egui::TextureHandle>, // real Lucide pipette (LEGACY_EYE) for the picker eyedropper (A16.2)
    align_icons: [Option<egui::TextureHandle>; 8], // align L/CH/R · T/M/B · distribute H/V
    cursor: egui::CursorIcon,                // this frame's egui cursor (read from FullOutput, not post-frame state)
    refpt: (f32, f32),                       // transform reference point (ax, ay each in {0, .5, 1})
    lock: bool,                              // constrain W/H proportions
    ab_lock: bool,                           // constrain artboard W/H proportions
    align_target: AlignTarget,               // A4: Auto (smart) | Selection | Artboard — the align reference pref
    ab_name_edit: Option<(usize, String)>,   // on-canvas rename open: artboard index + the name it opened with
    pub fit_request: Option<usize>,          // an artboard asked to be fit in the window (host applies it)
    top: TopIcons,
    pub win_action: Option<WinAction>, // a window control was clicked this frame (host acts on it)
    show_rail: bool,
    show_dock: bool,
    // DFS S1: the real tab strip's data (host → `set_tabs` every frame) and the lifecycle commands the
    // chrome raised this frame (host ← `take_app_commands`).
    doc_tabs: Vec<TabView>,
    doc_active: Option<SessionId>,
    home: bool,
    start_page: varos_app::start_page::StartPage,
    /// The pure Start model the page draws (L2); rebuilt by the host, filter applied here.
    start_model: varos_app::start::StartModel,
    recent_warning: Option<String>,
    app_cmds: Vec<AppCommand>,
    color_modal: Option<ColorModal>, // the Color Picker modal, when open
    layer_icons: LayerIcons,
    lay_collapsed: std::collections::HashSet<u32>, // collapsed container node ids (UI-only)
    lay_search: String,
    lay_rename: Option<(u32, String)>, // inline rename open: node id + the name it opened with (K3 session holds the text)
    lay_drag: Option<(u32, u32)>,      // Layers row being dragged: (node id, SOURCE section) — the
    // section decides same-section reorder vs cross-board move
    lay_anchor: Option<(u32, u32)>, // Shift-range anchor: (node id, SECTION) — mirror rows share an
    // id across sections, so the id alone picks the wrong appearance
    layer_rows_cache: Option<LayerRowsCache>,
    layer_thumb_cache: std::collections::HashMap<u32, ThumbCacheEntry>,
    // ── Stage 4: the BOX TREE hosts the whole workspace (BOX_SYSTEM_PLAN §4) ──
    shell: varos_app::shell::ShellState, // the box tree; panel bodies render through the host hook
    board_hole: Option<egui::Rect>,      // the Board pane's interior (logical pts) — the wgpu canvas hole
    pub board_px: Option<egui::Rect>,    // same, in PHYSICAL px — main.rs fits the view to it
    field_pending: Option<fields::Pending>, // K3: what the open field would commit now
}

/// The Layers-panel icon set (rasterized Lucide, white).
struct LayerIcons {
    pub(crate) eye: Option<egui::TextureHandle>,
    pub(crate) eye_off: Option<egui::TextureHandle>,
    pub(crate) lock: Option<egui::TextureHandle>,
    pub(crate) unlock: Option<egui::TextureHandle>,
    pub(crate) search: Option<egui::TextureHandle>,
}

/// Native menu-bar mirrors (macOS, docs/foundation/MAC_CHROME.md §C): the SAME state the bar's Window
/// menu rows flip, and the keyboard hand-off for a focused text field.
#[cfg(target_os = "macos")]
impl Ui {
    /// Window ▸ Tool rail / Control bar / a dockable panel — what the egui Window menu rows do.
    pub fn toggle_rail(&mut self) {
        self.show_rail = !self.show_rail;
    }
    pub fn toggle_dock(&mut self) {
        self.show_dock = !self.show_dock;
    }
    pub fn toggle_panel(&mut self, p: varos_app::shell::PanelId) {
        self.shell.toggle_panel(p);
    }
    /// The check marks those rows show.
    pub fn rail_shown(&self) -> bool {
        self.show_rail
    }
    pub fn dock_shown(&self) -> bool {
        self.show_dock
    }
    pub fn panel_open(&self, p: varos_app::shell::PanelId) -> bool {
        self.shell.is_open(p)
    }
    /// A menu shortcut that arrives while a text field is focused goes to egui, exactly as the keyboard
    /// would have delivered it (so ⌘Z still undoes typing in a field instead of the document).
    /// ⌘C / ⌘X / ⌘V become egui's clipboard EVENTS, exactly what egui-winit makes of those keys on
    /// the keyboard path (a text field reads `Copy` / `Cut` / `Paste`, never the bare key) — so copy
    /// and paste keep working in a focused field now that the Edit menu owns those shortcuts.
    pub fn forward_shortcut(&mut self, key: egui::Key, shift: bool, alt: bool) {
        let state = &mut self.state;
        if let Some(clip) = text_clipboard_event(key, || state.clipboard_text()) {
            if let Some(ev) = clip {
                self.state.egui_input_mut().events.push(ev);
            }
            return;
        }
        let modifiers = egui::Modifiers { alt, shift, mac_cmd: true, command: true, ctrl: false };
        let ev = &mut self.state.egui_input_mut().events;
        for pressed in [true, false] {
            ev.push(egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers });
        }
    }
}

impl Ui {
    pub fn new(window: &Window) -> Self {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        install_style(&ctx);
        disable_ui_keyboard_zoom(&ctx);
        // rail singletons — Artboard sits with Selection + Direct Selection (Ahmed), then Pen, Eyedropper.
        let defs: [(ToolKind, &str, &str, bool); 7] = [
            (ToolKind::Object, LEGACY_SELECT, "Selection (V)", false),
            (ToolKind::Direct, LEGACY_DIRECT, "Direct Selection (A)", false),
            (ToolKind::Artboard, LEGACY_ARTBOARD, "Artboard (Shift+O)", true), // ends the selection group
            (ToolKind::Pen, LEGACY_PEN, "Pen (P)", true),                      // ends the pen group
            (ToolKind::Rotate, LEGACY_ROTATE, "Rotate (R)", false),            // transform group ↓
            (ToolKind::Scale, LEGACY_SCALE, "Scale (S)", true),                // ends the transform group
            (ToolKind::Eyedropper, LEGACY_EYE, "Eyedropper (I)", false),
        ];
        let tools = defs
            .iter()
            .enumerate()
            .map(|(i, (kind, svg, tip, grp))| ToolBtn {
                kind: *kind,
                tip,
                tex: legacy_texture(&ctx, &format!("ic-{i}"), svg, false),
                group_end: *grp,
            })
            .collect();
        // shape tools collapse into ONE rail slot: left-click uses the current shape, right-click flyouts all four.
        let shape_defs: [(ToolKind, &str, &str); 4] = [
            (ToolKind::Rect, LEGACY_RECT, "Rectangle (M)"),
            (ToolKind::Ellipse, LEGACY_ELLIPSE, "Ellipse (L)"),
            (ToolKind::Triangle, LEGACY_TRIANGLE, "Triangle"),
            (ToolKind::Polygon, LEGACY_POLYGON, "Polygon"),
        ];
        let shapes = shape_defs
            .iter()
            .enumerate()
            .map(|(i, (kind, svg, tip))| ToolBtn {
                kind: *kind,
                tip,
                tex: legacy_texture(&ctx, &format!("ic-shape{i}"), svg, false),
                group_end: false,
            })
            .collect();
        let ic_rotate = legacy_texture(&ctx, "lbl-rot", LEGACY_ROTATE, false);
        let ic_opacity = legacy_texture(&ctx, "lbl-op", LEGACY_OPACITY, false);
        let ic_strokew = legacy_texture(&ctx, "lbl-sw", LEGACY_STROKEW, false);
        let ic_fit = legacy_texture(&ctx, "lbl-fit", LEGACY_FIT, false);
        // A16.2: reuse the real Lucide pipette (LEGACY_EYE) for the picker's in-picker eyedropper.
        let ic_pipette = legacy_texture(&ctx, "lbl-pipette", LEGACY_EYE, false);
        // A16.1: FILLED (law-verbatim solid bars) loaded via load_icon_filled so they read bold at 16px.
        let align_icons = [
            legacy_texture(&ctx, "al-l", LEGACY_AL_L, true),
            legacy_texture(&ctx, "al-ch", LEGACY_AL_CH, true),
            legacy_texture(&ctx, "al-r", LEGACY_AL_R, true),
            legacy_texture(&ctx, "al-t", LEGACY_AL_T, true),
            legacy_texture(&ctx, "al-m", LEGACY_AL_M, true),
            legacy_texture(&ctx, "al-b", LEGACY_AL_B, true),
            legacy_texture(&ctx, "dist-h", LEGACY_DIST_H, true),
            legacy_texture(&ctx, "dist-v", LEGACY_DIST_V, true),
        ];
        let top = TopIcons { menu: legacy_texture(&ctx, "tb-menu", LEGACY_MENU, false) };
        let layer_icons = LayerIcons {
            eye: legacy_texture(&ctx, "l-eye", LEGACY_L_EYE, false),
            eye_off: legacy_texture(&ctx, "l-eyeoff", LEGACY_L_EYEOFF, false),
            lock: legacy_texture(&ctx, "l-lock", LEGACY_L_LOCK, false),
            unlock: legacy_texture(&ctx, "l-unlock", LEGACY_L_UNLOCK, false),
            search: legacy_texture(&ctx, "l-search", LEGACY_L_SEARCH, false),
        };
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, window, None, None, None);
        Ui {
            ctx,
            state,
            repaint: false,
            repaint_at: None,
            recovery: Default::default(),
            file_status: String::new(),
            export_sheet: None,
            panel_column: None,
            export_scopes: Default::default(),
            tools,
            shapes,
            shape_active: ToolKind::Rect,
            ic_rotate,
            ic_opacity,
            ic_strokew,
            ic_fit,
            ic_pipette,
            align_icons,
            cursor: egui::CursorIcon::Default,
            refpt: (0.0, 0.0),
            lock: false,
            ab_lock: false,
            align_target: AlignTarget::default(),
            ab_name_edit: None,
            fit_request: None,
            top,
            win_action: None,
            show_rail: true,
            show_dock: true,
            doc_tabs: vec![],
            doc_active: None,
            home: false,
            start_page: varos_app::start_page::StartPage::new(),
            start_model: varos_app::start::StartModel::without_recovery(&Default::default(), 0, |_| false),
            recent_warning: None,
            app_cmds: vec![],
            color_modal: None,
            layer_icons,
            lay_collapsed: std::collections::HashSet::new(),
            lay_search: String::new(),
            lay_rename: None,
            lay_drag: None,
            lay_anchor: None,
            layer_rows_cache: None,
            layer_thumb_cache: std::collections::HashMap::new(),
            shell: varos_app::shell::ShellState::standard(),
            board_hole: None,
            board_px: None,
            field_pending: None,
        }
    }

    /// Feed a window event to egui. Returns true if egui consumed it (so the canvas should NOT).
    pub fn on_event(&mut self, window: &Window, ev: &WindowEvent) -> bool {
        #[cfg(windows)]
        {
            self.state.on_window_event(window, ev).consumed
        }
        #[cfg(not(windows))]
        {
            let response = self.state.on_window_event(window, ev);
            if response.repaint && !matches!(ev, WindowEvent::RedrawRequested) {
                window.request_redraw();
            }
            response.consumed
        }
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
            Some(_) => true, // hands / menus / modal float above the tree
        }
    }
    /// Empty background bar space can drag the macOS window; floating UI always owns its area.
    #[cfg(target_os = "macos")]
    pub fn caption_drag_position(&self, physical: [f32; 2]) -> Option<[f32; 2]> {
        let pos = egui::pos2(physical[0], physical[1]) / self.ctx.pixels_per_point();
        // Use the current native position, not egui's pointer position from the last frame.
        let egui_hit = self.ctx.egui_is_using_pointer()
            || self.ctx.layer_id_at(pos).is_some_and(|layer| layer.order != egui::Order::Background);
        crate::mac_caption::caption_drag_allowed(
            crate::cursors::caption_drag_hit(physical[0], physical[1]),
            egui_hit,
            self.ctx.dragged_id().is_some(),
        )
        .then_some([pos.x, pos.y])
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
    /// Is a document tab lifted in a drag right now (P16)? Esc then belongs to the tab strip (it
    /// cancels the drag) and must not also reach the canvas.
    pub fn tab_drag_active(&self) -> bool {
        self.ctx.data(|d| d.get_temp::<TabDrag>(egui::Id::new(TAB_DRAG_KEY)).is_some())
    }
    /// Is the picker's system eyedropper armed? While it is, in-window clicks are swallowed by the host
    /// so the sampled click doesn't also poke the canvas (A5 samples via a global pixel read instead).
    pub fn picking_screen(&self) -> bool {
        self.color_modal.as_ref().is_some_and(|m| m.eyedropping)
    }
    /// DFS S1: the host hands the workspace's tabs over every frame.
    /// Home on/off. Entering Home starts Start's focus fresh (resting on New, ring hidden).
    pub fn set_home(&mut self, home: bool, warning: Option<String>) {
        if self.home != home {
            egui::Popup::close_all(&self.ctx);
            varos_app::shell::kit::close_menu(&self.ctx);
            if home {
                self.start_page.reset_focus();
                // entering Home drops a document field's keyboard once; Home itself keeps egui focus
                // (its Search field) — canvas keys never reach a document behind it anyway
                // (`Workspace::document_target`)
                self.ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
            }
        }
        self.home = home;
        self.recent_warning = warning;
    }
    /// A rebuilt Start model (the host rebuilds only on Home and only when its inputs changed);
    /// keyboard focus survives by key.
    /// Start v2 thumbnails: where Home's page looks them up (startup).
    pub fn set_thumb_source(&mut self, source: std::sync::Arc<dyn varos_app::start_page::ThumbSource>) {
        self.start_page.set_thumb_source(source);
    }
    /// A thumbnail render landed (or its file went): Home asks the cache again for `key`.
    pub fn thumb_updated(&mut self, key: &varos_app::start::ThumbKey) {
        self.start_page.thumb_updated(key);
    }
    pub fn set_start_model(&mut self, model: varos_app::start::StartModel) {
        let mut model = model;
        model.carry_focus_from(&self.start_model); // keeps the user's tag / search / view
        self.start_model = model;
    }
    pub fn set_tabs(&mut self, tabs: Vec<TabView>, active: Option<SessionId>) {
        self.doc_tabs = tabs;
        self.doc_active = active;
    }
    /// DFS S6: `AppCommand::ShowExport(id)` — open the Export PDF sheet for tab `id` over `doc` (its
    /// rows and page counts are planned now, once).
    pub fn show_export(&mut self, id: SessionId, doc: &varos_core::model::Document) {
        let remembered = self.export_scopes.get(&id).copied();
        self.export_sheet = Some(crate::export_ui::ExportSheet::new(id, doc, remembered));
    }
    /// DFS S1: the lifecycle commands the chrome (tab strip, burger rows) raised since the last call.
    pub fn take_app_commands(&mut self) -> Vec<AppCommand> {
        std::mem::take(&mut self.app_cmds)
    }
    /// DFS S1 + K3: before any lifecycle command, close every Ui-side edit still open on `ed` — the open
    /// field COMMITS first (`fields::settle`; unchanged text commits nothing), then an open colour picker
    /// is CANCELLED (its live preview is not a commit). `false` = the field's text does not parse: it
    /// keeps the keyboard and its reason, nothing was touched, and the command must not run.
    pub fn settle(&mut self, ed: &mut Editor) -> bool {
        if !self.commit_fields(ed) {
            return false;
        }
        if self.color_modal.take().is_some() {
            ed.execute(EditCommand::PickerCancel);
        }
        self.lay_rename = None;
        self.ab_name_edit = None;
        true
    }
    /// K3: commit the open field into `ed` now (before a canvas press, which may change the selection
    /// the field edits). `false` = its text does not parse — it keeps the keyboard; drop the press.
    pub fn commit_fields(&mut self, ed: &mut Editor) -> bool {
        fields::settle(&self.ctx, self.doc_active, &mut self.field_pending, ed)
    }
    /// A text / number field is being edited right now.
    pub fn editing_field(&self) -> bool {
        kit::field::any_open(&self.ctx)
    }
    /// DFS S1: the active document changed — drop the Ui state that belongs to the previous document
    /// (the Layers rows cache, drag, Shift-range anchor, collapsed rows and search).
    pub fn document_switched(&mut self) {
        self.layer_rows_cache = None;
        self.lay_drag = None;
        self.lay_anchor = None;
        self.lay_collapsed.clear();
        self.lay_search.clear();
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
    /// Editor-free Start pass: no Snap, EditCommand, document panels, or canvas overlays.
    fn run_home(
        &mut self,
        window: &Window,
        maximized: bool,
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor) {
        let raw = self.state.egui_input_mut();
        raw.focused = egui_focus_seed(window.has_focus(), raw.focused);
        let input = self.state.take_egui_input(window);
        self.export_sheet = None; // Home has no document to export
        let out = self.ctx.run_ui(input, |root| {
            build_topbar(
                root,
                &self.top,
                &mut self.shell,
                &mut self.win_action,
                &self.doc_tabs,
                None,
                &mut self.app_cmds,
                &mut self.show_rail,
                &mut self.show_dock,
                &mut Default::default(),
                None,
                maximized,
                true,
                cfg!(target_os = "macos"),
            );
            home_body(
                root,
                &mut self.start_page,
                &mut self.start_model,
                self.recent_warning.as_deref(),
                &mut self.app_cmds,
            );
        });

        // K3: Home draws only its live Search field (it commits nothing); any other edit left open (an
        // invalid one a non-user command passed) is closed here — there is no document to commit into
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
        self.repaint = out.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.repaint_delay.is_zero());
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

    pub fn run(
        &mut self,
        window: &Window,
        ed: &mut Editor,
        ppp: f32,
        view: View,
        maximized: bool,
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor) {
        if self.home {
            return self.run_home(window, maximized);
        }
        // host seed of egui's focus flag from winit (startup, activation, un-occlusion alike) — see
        // `egui_focus_seed`
        let raw = self.state.egui_input_mut();
        raw.focused = egui_focus_seed(window.has_focus(), raw.focused);
        let input = self.state.take_egui_input(window);
        set_doc_salt(&self.ctx, self.doc_active); // per-widget edit state stays inside its document
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
            align: &self.align_icons,
        };
        let recovery = &self.recovery;
        let ic_fit = &self.ic_fit; // the status strip's Fit control shares the artboard panel's icon
        let ic_pipette = &self.ic_pipette; // A16.2: the real pipette for the picker's in-picker eyedropper
        let shell = &mut self.shell; // Stage 4: the box tree hosting the whole workspace
        let prev_hole = self.board_hole; // last frame's canvas hole (the seam underlay paints around it)
        let mut new_hole: Option<egui::Rect> = None;
        let top = &self.top;
        // Pointer-only frames reuse the rows wholesale. Selection/tree/search changes rebuild row state;
        // the independent thumbnail cache still avoids curve subdivision when geometry is unchanged.
        let rows_key = layer_rows_key(ed, &self.lay_collapsed, &self.lay_search);
        let mut layer_rows_cache = self.layer_rows_cache.take();
        if layer_rows_cache.as_ref().is_none_or(|cache| cache.key != rows_key) {
            let rows = build_layer_rows(ed, &self.lay_collapsed, &self.lay_search, &mut self.layer_thumb_cache);
            layer_rows_cache = Some(LayerRowsCache { key: rows_key, rows });
        }
        let layer_rows = &layer_rows_cache.as_ref().expect("layers cache is populated above").rows;
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
        let mut align_target = self.align_target;
        let mut ab_name_edit = std::mem::take(&mut self.ab_name_edit);
        let mut fit_request: Option<usize> = None;
        let mut shape_active = self.shape_active;
        let mut win_action = None;
        let mut show_rail = self.show_rail;
        let mut show_dock = self.show_dock;
        let mut snap_cfg = ed.doc.snap; // the Windows burger's snapping rows edit this (non-undoable mode flag)
        let doc_tabs = std::mem::take(&mut self.doc_tabs);
        let doc_active = self.doc_active;
        // an accumulating queue: nothing drains it until S1-D wires `take_app_commands` into the host,
        // so this frame's clicks are APPENDED to whatever earlier frames already queued.
        let mut app_cmds = std::mem::take(&mut self.app_cmds);
        let mut color_modal = std::mem::take(&mut self.color_modal);
        // the Export sheet belongs to one tab: another tab (or none) closes it
        let mut export_sheet = self.export_sheet.take().filter(|s| Some(s.sid) == doc_active);
        let panel_column = self.panel_column; // last frame's — the band is built before the tree
        let mut new_column = None;
        let export_scopes = &mut self.export_scopes;
        let status = if self.file_status.is_empty() { &self.recovery.status } else { &self.file_status };
        // egui 0.34 removed Context::run — run_ui hands the pass's root Ui (panels now show() on it)
        let out = self.ctx.run_ui(input, |root| {
            let ctx = root.ctx().clone();
            let ctx = &ctx;
            build_topbar(
                root,
                top,
                &mut *shell,
                &mut win_action,
                &doc_tabs,
                doc_active,
                &mut app_cmds,
                &mut show_rail,
                &mut show_dock,
                &mut snap_cfg,
                panel_column,
                maximized,
                false,
                cfg!(target_os = "macos"),
            );
            if let Some(sheet) = export_sheet.as_mut() {
                match crate::export_ui::draw(ctx, sheet, panel_column) {
                    crate::export_ui::SheetAction::Stay => {}
                    crate::export_ui::SheetAction::Close => export_sheet = None,
                    crate::export_ui::SheetAction::Export(id, scope) => {
                        export_scopes.insert(id, scope);
                        app_cmds.push(AppCommand::ExportPdf(id, scope));
                        export_sheet = None;
                    }
                }
            }
            build_recovery_strip(root, recovery, &mut app_cmds);
            build_statusbar(root, absnap.active, absnap.count, view.zoom, ic_fit, &mut fit_request, status);
            // ── Stage 4: the `.mid` region IS the box tree (BOX_SYSTEM_PLAN §4). The Board pane is
            // a HOLE showing the wgpu canvas below; the seam underlay paints the void around last
            // frame's hole (one-frame lag on resize, healed by the request_repaint below). ──
            {
                let mid = root.available_rect_before_wrap();
                paint_void_underlay(root.painter(), mid, prev_hole);
                let mut host = |panel: varos_app::shell::PanelId, ui: &mut egui::Ui| -> bool {
                    use varos_app::shell::PanelId as P;
                    // K3: each panel's fields form one Tab ring
                    kit::field::group(ui, panel, |ui| match panel {
                        P::Board => {
                            let rect = ui.max_rect();
                            let p = ui.painter().clone();
                            // a normal box on the void: hairline border, rounded corners patched
                            // with seam so the scene never pokes past the radius
                            corner_voids(&p, rect);
                            p.rect_stroke(rect, CornerRadius::same(RBOX), Stroke::new(1.0, BORDER), StrokeKind::Inside);
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
                                    align_target,
                                    &mut ops,
                                    &mut fit_request,
                                );
                            }
                            new_hole = Some(inner);
                            true
                        }
                        P::Properties => {
                            if snap.tool == ToolKind::Artboard {
                                panel_artboard(ui, &absnap, &mut ab_lock, &mut ops, &mut fit_request);
                            } else {
                                panel_properties(
                                    ui,
                                    &snap,
                                    &icons,
                                    &mut refpt,
                                    &mut lock,
                                    &mut ops,
                                    (recovery, &mut app_cmds),
                                );
                            }
                            true
                        }
                        P::Layers => {
                            panel_layers(
                                ui,
                                layer_rows,
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
                            panel_align(ui, &icons, &mut align_target, &mut ops);
                            true
                        }
                        P::Pathfinder => {
                            panel_pathfinder(ui, snap.pathfinder, &mut ops);
                            true
                        }
                        _ => false,
                    })
                };
                let tree_rect = editor_tree_rect(mid); // 4b: the boxes start at the band's bottom
                root.scope_builder(egui::UiBuilder::new().max_rect(tree_rect), |ui| shell.ui_hosted(ui, &mut host));
                new_column = shell.side_column_span();
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
            build_color_modal(ctx, &mut color_modal, &snap, ic_pipette, &mut ops);
            // over everything
        });
        self.color_modal = color_modal;
        self.refpt = refpt;
        self.lock = lock;
        self.ab_lock = ab_lock;
        self.align_target = align_target;
        self.ab_name_edit = ab_name_edit;
        self.lay_search = lay_search;
        self.lay_rename = lay_rename;
        self.lay_collapsed = lay_collapsed;
        self.lay_drag = lay_drag;
        self.lay_anchor = lay_anchor;
        self.layer_rows_cache = layer_rows_cache;
        self.shape_active = shape_active;
        if fit_request.is_some() {
            self.fit_request = fit_request;
        }
        self.win_action = win_action;
        self.show_rail = show_rail;
        self.show_dock = show_dock;
        self.doc_tabs = doc_tabs;
        self.app_cmds = app_cmds;
        self.export_sheet = export_sheet;
        if new_column != self.panel_column {
            self.ctx.request_repaint(); // the band's right zone catches up next frame
        }
        self.panel_column = new_column;
        ed.set_constrain_wh(lock); // A12: mirror the Properties W/H lock so canvas scale drags honour it too
                                   // OpenPicker is a UI op (it opens the modal, seeded from the target's colour) — intercept it here
                                   // K3: field commits first; while a field holds invalid text the frame's presses are dropped
        fields::finish_frame(&self.ctx, self.doc_active, &mut ops, &mut self.field_pending);
        ops.retain(|op| {
            if let Op::OpenPicker(t) = op {
                let seed = match *t {
                    MTarget::Paint(PaintTarget::Fill) => snap.fill,
                    MTarget::Paint(PaintTarget::Stroke) => snap.stroke,
                    MTarget::Ab(i) => ed.doc.artboards.get(i).and_then(|a| a.page_color),
                };
                let base = seed.unwrap_or([0.85, 0.85, 0.87, 1.0]);
                let h = rgb_to_hsv(base);
                // A6: open ONE undo step for the whole picker session; live edits mutate the doc in
                // place each frame and OK/Cancel closes the single step.
                ed.execute(EditCommand::PickerBegin);
                self.color_modal = Some(ColorModal {
                    target: *t,
                    orig: seed,
                    hsva: [h[0], h[1], h[2], base[3]],
                    chan: Chan::H,
                    tab: MTab::Picker,
                    harmony: Harmony::None,
                    eyedropping: false,
                    eyedrop_prev_down: false,
                    eyedrop_return: [h[0], h[1], h[2], base[3]],
                });
                false
            } else {
                true
            }
        });
        apply_frame(ed, snap_cfg, ops); // the band's snapping rows, then the panels' ops
        self.cursor = out.platform_output.cursor_icon; // read the REAL cursor from this frame's output

        // macOS: cursors.rs owns the OS cursor (Retina NSCursor, re-set every frame from `chrome_ck` /
        // the tool). Hand egui-winit a constant icon so it never re-writes winit's cursor — each such
        // write invalidates the view's cursor rect and AppKit would re-show that cursor over ours.
        #[cfg(target_os = "macos")]
        let out = {
            let mut out = out;
            out.platform_output.cursor_icon = egui::CursorIcon::Default;
            out
        };
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
        self.repaint_at =
            out.viewport_output.get(&egui::ViewportId::ROOT).and_then(|v| Instant::now().checked_add(v.repaint_delay));
        self.repaint = out.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.repaint_delay.is_zero());
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
