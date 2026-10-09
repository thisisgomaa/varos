//! Native GPU UI — hand-painted chrome on OUR wgpu surface via `Renderer::render_ui` (egui shares our
//! Device/Queue; no second window). egui is only canvas + input + layout; every widget is drawn by us
//! so it matches the Figma, not egui's dev-tool defaults. Pieces so far: the left TOOL RAIL and the
//! right INSPECTOR DOCK (Transform / Appearance / Fill / Stroke). Solid panels, one light GPU shadow,
//! no glass. Panels read a per-frame snapshot of the editor and push deferred `Op`s, applied to
//! `&mut Editor` after layout (no IPC, no borrow fights). varos-core itself is untouched.
use crate::app_command::{AppCommand, SessionId, TabView};
use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Stroke, StrokeKind};
use std::hash::{Hash, Hasher};
use std::time::Instant;
use varos_core::editor::{AlignMode, AlignTarget, DistAxis, Editor, PaintTarget, ToolKind};
use varos_core::geom::{Pt, Rgba, View};
use varos_core::EditCommand;
use winit::event::WindowEvent;
use winit::window::Window;
// The law palette (warm ramp; tokens.rs) is shared with the split UI modules.
// Legacy colour aliases retain the established body names.
use varos_app::shell::tokens::{
    micro_label, numeric_value, panel_title, shortcut_label, ACCENT, ACCENT_TINT, ALIGN_SECTION_GAP, CLOSE_RED,
    CONTROL_BAR_NAME_H, CONTROL_BAR_NAME_TEXT, CONTROL_BAR_NAME_W, DISABLED, HOVER, LABEL_GAP, LINE as BORDER, LINE2,
    LINE2 as BORDER_2, MUTED, NONE_RED, PANEL as SOLID_PANEL, PANEL_ITEM_GAP_X, PF_BAR_H, PF_BAR_W, PF_OFFSET,
    PF_RADIUS, PF_SQUARE, PF_STROKE, R, RBOX, RCAP, ROW_HOVER, RULER_BG, SEAM, SECTION_GAP_HALF, SEG_TEXT,
    SURFACE as BG_SURFACE, SURFACE as SWATCH_WELL, TEXT, TRANSFORM_REFPOINT_SIZE,
};
// Icon stage 1: one icon registry + one icon button (shell::kit), one set of icon sizes (tokens).
use varos_app::shell::kit::field::Label as Lab;
use varos_app::shell::kit::icons::{
    legacy_texture, LEGACY_AL_B, LEGACY_AL_CH, LEGACY_AL_L, LEGACY_AL_M, LEGACY_AL_R, LEGACY_AL_T, LEGACY_DIRECT,
    LEGACY_DIST_H, LEGACY_DIST_V, LEGACY_ELLIPSE, LEGACY_EYE, LEGACY_FIT, LEGACY_L_EYE, LEGACY_L_EYEOFF, LEGACY_L_LOCK,
    LEGACY_L_SEARCH, LEGACY_L_UNLOCK, LEGACY_MENU, LEGACY_OPACITY, LEGACY_PEN, LEGACY_RECT, LEGACY_ROTATE,
    LEGACY_SELECT, LEGACY_STROKEW, LEGACY_TRIANGLE,
};
use varos_app::shell::kit::{self, Icon};
mod export;
// ---- Lane C ----
pub(crate) mod fields;
mod guide_field;
// ---- Lane B w3-effects ----
mod effects;
// ---- end Lane B w3-effects ----
mod lane_c;
// ---- Lane A ----
mod appearance;
use varos_app::shell::tokens::{ICON_BTN_H, ICON_BTN_W, ICON_LG, ICON_MD, ICON_SM};
// Lucide icon path data (white-stroked at render time), same set as the web rail.
// ---- Lane F ----
mod bar;
mod canvas_overlay;
mod clipping;
mod colour_tools;
// ---- w3-cmyk ----
pub(crate) mod colour_management;
mod control_bar;
mod controls;
mod home;
mod layout;
mod menus;
pub(crate) mod ops;
mod panels;
mod picker;
mod pointer;
mod rail;
// ---- Lane D: provisional drawing UI ----
mod drawing;
mod rail_flyout;
pub(crate) use drawing::{key as drawing_key, tool_name as drawing_tool_name};
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
mod icon_actions;
mod isolation;
// ---- Lane E ----
mod navigator;
pub(crate) mod select_transform;
mod view_modes;
use icon_actions::*;
/// A window action the custom title bar asks the host (winit) to perform.
pub enum WinAction {
    Minimize,
    ToggleMaximize,
    Close,
    /// The band's V mark (4b, macOS): the native About panel.
    About,
}

pub struct Ui {
    pub text_tool: crate::text_product::TextProduct,
    ctx: egui::Context,
    state: egui_winit::State,
    /// When egui wants its next pass (`now` + its repaint delay; `None` = it wants none). The host
    /// paces it to the display refresh and draws it from the loop's plan (`pacing`).
    pub repaint_at: Option<Instant>,
    pub recovery: crate::recovery_host::RecoveryUi,
    /// Background save / export status for the status bar (`file_jobs::status_text`); when set it
    /// takes the recovery status's place.
    pub file_status: String,
    pub canvas_hint: bar::CanvasHint,
    /// DFS S6: the open Export PDF sheet and each tab's last scope; 4b: the panel column's x-span as
    /// last laid out (the band's right zone with the V mark, and the sheet's right edge — one frame late).
    export_sheet: Option<crate::export_ui::ExportSheet>,
    panel_column: Option<egui::Rangef>,
    // ---- Lane F ----
    pub phase9: crate::phase9::State,
    pub document_sheet: Option<crate::document_ui::Sheet>,
    export_scopes: std::collections::HashMap<SessionId, varos_pdf::ExportScope>,
    ic_rotate: Option<egui::TextureHandle>,
    ic_opacity: Option<egui::TextureHandle>,
    ic_strokew: Option<egui::TextureHandle>,
    ic_fit: Option<egui::TextureHandle>,
    align_icons: [Option<egui::TextureHandle>; 8], // align L/CH/R · T/M/B · distribute H/V
    cursor: egui::CursorIcon, // this frame's egui cursor (read from FullOutput, not post-frame state)
    refpt: (f32, f32),        // transform reference point (ax, ay each in {0, .5, 1})
    lock: bool,               // constrain W/H proportions
    ab_lock: bool,            // constrain artboard W/H proportions
    align_target: AlignTarget, // A4: Auto (smart) | Selection | Artboard — the align reference pref
    ab_name_edit: Option<(usize, String)>, // on-canvas rename open: artboard index + the name it opened with
    pub fit_request: Option<usize>, // an artboard asked to be fit in the window (host applies it)
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
    color_panel: Option<ColorPanel>,
    picker_board_colors: picker::BoardColors,
    picker_layout: varos_app::storage::layout::PickerLayout,
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
    pub fn toggle_panel(&mut self, p: varos_app::shell::PanelId) {
        self.shell.toggle_panel(p);
    }
    pub fn new(window: &Window) -> Self {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        install_style(&ctx);
        disable_ui_keyboard_zoom(&ctx);
        let ic_rotate = legacy_texture(&ctx, "lbl-rot", LEGACY_ROTATE, false);
        let ic_opacity = legacy_texture(&ctx, "lbl-op", LEGACY_OPACITY, false);
        let ic_strokew = legacy_texture(&ctx, "lbl-sw", LEGACY_STROKEW, false);
        let ic_fit = legacy_texture(&ctx, "lbl-fit", LEGACY_FIT, false);
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
            text_tool: Default::default(),
            ctx,
            state,
            repaint_at: None,
            recovery: Default::default(),
            file_status: String::new(),
            canvas_hint: Default::default(),
            phase9: Default::default(),
            document_sheet: None,
            export_sheet: None,
            panel_column: None,
            export_scopes: Default::default(),
            ic_rotate,
            ic_opacity,
            ic_strokew,
            ic_fit,
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
            color_panel: None,
            picker_board_colors: Default::default(),
            picker_layout: Default::default(),
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
    /// Feed a window event to egui. `consumed` = egui took it (so the canvas should NOT); `repaint` =
    /// egui wants a frame for it (the host asks for that frame, tagged — `pacing`).
    pub fn on_event(&mut self, window: &Window, ev: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_window_event(window, ev)
    }
    /// Why egui ran this frame (the repaint requests of the previous pass, `file:line reason`) —
    /// `VAROS_FRAME_DEBUG=1` only.
    pub fn repaint_causes(&self) -> Vec<String> {
        self.ctx.repaint_causes().iter().map(|c| c.to_string()).collect()
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
        // Lane D numeric sheets, Lane G text session, Lane F Preferences/Shortcuts sheets, export fields
        drawing::blocks_keyboard(&self.ctx)
            || self.text_tool.session.is_some()
            || matches!(
                self.phase9.sheet,
                Some(crate::phase9::DesktopAction::Preferences | crate::phase9::DesktopAction::Shortcuts)
            )
            || export::wants_keyboard(self)
    }
    /// Is a document tab lifted in a drag right now (P16)? Esc then belongs to the tab strip (it
    /// cancels the drag) and must not also reach the canvas.
    pub fn tab_drag_active(&self) -> bool {
        self.ctx.data(|d| d.get_temp::<TabDrag>(egui::Id::new(TAB_DRAG_KEY)).is_some())
    }
    /// DFS S1: the host hands the workspace's tabs over every frame.
    /// Home on/off. Entering Home starts Start's focus fresh (resting on New, ring hidden).
    pub fn set_home(&mut self, home: bool, warning: Option<String>) {
        if self.home != home {
            egui::Popup::close_all(&self.ctx);
            varos_app::shell::kit::close_menu(&self.ctx);
            if home {
                self.start_page.reset_focus();
                // entering Home drops a document field's keyboard once — canvas keys never reach a
                // document behind Home anyway (`Workspace::document_target`)
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
        if self.doc_active != active {
            self.picker_board_colors = Default::default();
            self.text_tool = Default::default();
        }
        self.doc_tabs = tabs;
        self.doc_active = active;
    }
    /// DFS S6: `AppCommand::ShowExport(id)` — open the Export PDF sheet for tab `s` (rows and page counts
    /// planned now, once); `selection` = Export Selection…; a tab still exporting cannot start another.
    pub fn show_export(&mut self, s: &crate::workspace::DocumentSession, selection: bool) {
        self.export_sheet = Some(crate::export_ui::ExportSheet::of(s, selection, &self.export_scopes));
    }
    /// DFS S1: the lifecycle commands the chrome (tab strip, burger rows) raised since the last call.
    pub fn take_app_commands(&mut self) -> Vec<AppCommand> {
        std::mem::take(&mut self.app_cmds)
    }
    /// DFS S1 + K3: before any lifecycle command, close every Ui-side edit still open on `ed` — the open
    /// field COMMITS first (`fields::settle`; unchanged text commits nothing), then an open colour picker
    /// finishes its current gesture. Closing never reverts paint. `false` = the field's text does not parse: it
    /// keeps the keyboard and its reason, nothing was touched, and the command must not run.
    pub fn settle(&mut self, ed: &mut Editor) -> bool {
        if !self.commit_fields(ed) {
            return false;
        }
        if let Some(m) = &mut self.color_panel {
            let mut ops = vec![];
            m.finish(&mut ops);
            apply_ops(ed, ops);
        }
        self.lay_rename = None;
        self.ab_name_edit = None;
        true
    }
    /// K3: commit the open field into `ed` now (before a canvas press, which may change the selection
    /// the field edits). `false` = its text does not parse — it keeps the keyboard; drop the press.
    pub fn commit_fields(&mut self, ed: &mut Editor) -> bool {
        if !fields::settle_text(&self.ctx, self.doc_active, &mut self.field_pending, &mut self.text_tool, ed) {
            return false;
        }
        if let Err(error) = self.text_tool.commit(ed) {
            self.text_tool.error = Some(error);
            return false;
        }
        // ---- Lane B w3-effects ----
        effects::settle(&self.ctx, ed);
        // ---- end Lane B w3-effects ----
        crate::document_ui::settle(&mut self.document_sheet, ed);
        self.commit_picker_fields(ed)
    }
    /// A text / number field is being edited right now.
    pub fn editing_field(&self) -> bool {
        kit::field::any_open(&self.ctx)
    }
    /// DFS S1: the active document changed — drop the Ui state that belongs to the previous document
    /// (the Layers rows cache, drag, Shift-range anchor, collapsed rows and search).
    pub fn document_switched(&mut self) {
        self.text_tool = Default::default();
        self.color_panel = None;
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
        if ed.view_depth.presentation {
            return self.run_presentation(window);
        }
        // host seed of egui's focus flag from winit (startup, activation, un-occlusion alike) — see
        // `egui_focus_seed`
        let raw = self.state.egui_input_mut();
        raw.focused = egui_focus_seed(window.has_focus(), raw.focused);
        let input = self.state.take_egui_input(window);
        set_doc_salt(&self.ctx, self.doc_active); // per-widget edit state stays inside its document
        layout::prepare_picker_input(
            &self.ctx,
            self.doc_active,
            &mut self.field_pending,
            &mut self.color_panel,
            ed,
            &input,
        );
        select_transform::prepare(self, ed);
        self.prepare_picker(ed);
        self.text_tool.input(&self.ctx, &input, ed, view, ppp, self.board_hole);
        let mut snap = Snap::read(ed);
        snap.text = self.text_tool.selected_text(ed);
        if let Some(error) = self.text_tool.error.take() {
            self.file_status = error;
        }
        snap.board_colors = self.picker_board_colors.read(
            ed,
            self.color_panel.is_some() && self.picker_layout.drawer_open && self.picker_layout.drawer_tab == 1,
        );
        let absnap = AbSnap::read(ed);
        let abs = ab_infos(ed);
        let presence = crate::agent_presence::frame(self.doc_active, ed, Instant::now());
        let snap_hud = ed.snap_hud.clone();
        let show_rulers = ed.show_rulers;
        let ruler_origin = ed.doc.ruler_origin;
        let ruler_reset = ed.doc.active_artboard().map(|a| [a.x, a.y]).unwrap_or([0.0, 0.0]);
        let ruler_grid = ed.adaptive_grid_step(); // tick on the SAME base-5 lattice as the dot grid
        let origin_preview = ed.origin_preview; // dashed crosshair while dragging the ruler zero-point
        let icons = DockIcons {
            rotate: &self.ic_rotate,
            opacity: &self.ic_opacity,
            strokew: &self.ic_strokew,
            align: &self.align_icons,
        };
        let recovery = &self.recovery;
        let ic_fit = &self.ic_fit; // the status strip's Fit control shares the artboard panel's icon
                                   // ---- Lane A ----
        if self.ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("appearance-open"))).unwrap_or(false) {
            self.shell.show_panel(varos_app::shell::PanelId::Properties);
        }
        let shell = &mut self.shell; // Stage 4: the box tree hosting the whole workspace
        let prev_hole = self.board_hole; // last frame's canvas hole (the seam underlay paints around it)
        let mut new_hole: Option<egui::Rect> = None;
        let top = &self.top;
        // Pointer-only frames reuse the rows wholesale. Selection/tree/search changes rebuild row state;
        let filter = layer_kind_filter(&self.ctx);
        let rows_key = layer_rows_key(ed, &self.lay_collapsed, &self.lay_search, filter);
        let mut layer_rows_cache = self.layer_rows_cache.take();
        if layer_rows_cache.as_ref().is_none_or(|cache| cache.key != rows_key) {
            let rows = build_layer_rows(ed, &self.lay_collapsed, &self.lay_search, filter, &mut self.layer_thumb_cache);
            layer_rows_cache = Some(LayerRowsCache { key: rows_key, rows });
        }
        let navigator_canvas = self.board_hole.unwrap_or_else(|| self.ctx.content_rect());
        let layer_rows = &layer_rows_cache.as_ref().expect("layers cache is populated above").rows;
        let layer_icons = &self.layer_icons;
        let mut lay_search = std::mem::take(&mut self.lay_search);
        let mut lay_rename = std::mem::take(&mut self.lay_rename);
        let mut lay_collapsed = std::mem::take(&mut self.lay_collapsed);
        let mut lay_drag = self.lay_drag;
        let mut lay_anchor = self.lay_anchor;
        let mut ops: Vec<Op> = Vec::new();
        appearance::settle(&self.ctx, &mut ops);
        let mut refpt = self.refpt;
        let mut lock = self.lock;
        let mut ab_lock = self.ab_lock;
        let mut align_target = self.align_target;
        let mut ab_name_edit = std::mem::take(&mut self.ab_name_edit);
        let mut fit_request: Option<usize> = None;
        let mut win_action = None;
        let mut show_rail = self.show_rail;
        let mut show_dock = self.show_dock;
        let mut snap_cfg = ed.doc.snap; // the Windows burger's snapping rows edit this (non-undoable mode flag)
        let has_selection = crate::lifecycle::has_selection(ed);
        clipping::seed(&self.ctx, ed);
        let doc_tabs = std::mem::take(&mut self.doc_tabs);
        let doc_active = self.doc_active;
        // an accumulating queue: nothing drains it until S1-D wires `take_app_commands` into the host,
        // so this frame's clicks are APPENDED to whatever earlier frames already queued.
        let mut app_cmds = std::mem::take(&mut self.app_cmds);
        let mut color_panel = std::mem::take(&mut self.color_panel);
        let picker_layout = &mut self.picker_layout;
        // the Export sheet belongs to one tab: another tab (or none) closes it
        let mut export_sheet = self.export_sheet.take().filter(|s| Some(s.sid) == doc_active);
        let panel_column = self.panel_column; // last frame's — the band is built before the tree
        let mut new_column = None;
        let export_scopes = &mut self.export_scopes;
        let hint = self.canvas_hint.text(doc_active, ed.rev);
        let status = if !self.file_status.is_empty() {
            &self.file_status
        } else if !hint.is_empty() {
            hint
        } else {
            &self.recovery.status
        };
        // egui 0.34 removed Context::run — run_ui hands the pass's root Ui (panels now show() on it)
        let text_tool = &mut self.text_tool;
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
                has_selection,
                panel_column,
                maximized,
                false,
                cfg!(target_os = "macos"),
            );
            // ---- Lane B w3-effects ----
            effects::sheet(ctx, ed, doc_active);
            // ---- end Lane B w3-effects ----
            lane_c::sheets(ctx, &mut app_cmds, &mut ops, doc_active);
            crate::export_ui::dispatch(ctx, &mut export_sheet, panel_column, export_scopes, &mut app_cmds);
            crate::document_ui::guides(ctx, &ed.doc, view, ppp, prev_hole);
            self.phase9.draw(ctx, &mut app_cmds, doc_active);
            crate::document_ui::draw(ctx, &mut self.document_sheet, ed, doc_active, &mut ops);
            build_statusbar(root, (absnap.active, absnap.count), view.zoom, ic_fit, &mut fit_request, status, &mut ops);
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
                            guide_field::show(ui, inner, view, ppp, ed, &mut ops);
                            if show_rail {
                                board_rail(ui.ctx(), inner, &snap, &mut ops);
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
                            build_recovery_card(ui.ctx(), rect, recovery, &mut app_cmds);
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
                        P::Navigator => {
                            navigator::draw(
                                ui,
                                ed,
                                view,
                                ppp,
                                navigator_canvas,
                                doc_active.map_or(0, |s| s.0),
                                &mut ops,
                            );
                            true
                        }
                        P::Swatches => {
                            colour_tools::panel(ui, ed, &mut ops);
                            true
                        }
                        P::Actions => {
                            self.phase9.actions(ui, &mut app_cmds, doc_active);
                            true
                        }
                        P::History => {
                            crate::phase9::history(ui, ed, doc_active, &mut app_cmds);
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
                        P::Links => crate::image_ui::panel(ui, ed, doc_active, &mut app_cmds),
                        _ => false,
                    })
                };
                let tree_rect = editor_tree_rect(mid); // 4b: the boxes start at the band's bottom
                root.scope_builder(egui::UiBuilder::new().max_rect(tree_rect), |ui| shell.ui_hosted(ui, &mut host));
                new_column = shell.side_column_span();
            }
            let hole = new_hole.unwrap_or_else(|| ctx.content_rect());
            drawing::draw(ctx, ed, hole, view, ppp);
            select_transform::draw(ctx, ed, hole);
            // ---- Lane B w3-effects ----
            effects::width_points(ctx, ed, &view, ppp, hole);
            // ---- end Lane B w3-effects ----
            lane_c::corners(ctx, ed, &view, ppp, hole, doc_active);
            isolation::draw(ctx, ed, hole);
            crate::image_ui::draw(ctx, ed, doc_active, hole, &mut app_cmds, view, ppp);
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
            text_tool.paint(ctx, ed, view, ppp);
            paint_agent_presence(ctx, view, ppp, hole, &presence);
            build_snap_hud(ctx, view, ppp, hole, &snap_hud);
            build_origin_crosshair(ctx, view, ppp, hole, origin_preview);
            if let Some(m) = color_panel.as_mut() {
                prepare_canvas_sample(m, ed, view, ppp, hole);
            }
            let sample = color_panel.as_ref().and_then(|m| picker_canvas_sample(ctx, m));
            build_color_panel(ctx, &mut color_panel, &snap, &mut ops, sample, hole, picker_layout);
            if let Some(m) = color_panel.as_mut() {
                prepare_canvas_sample(m, ed, view, ppp, hole);
            }
        });
        self.color_panel = color_panel;
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
                                   // OpenPicker is a UI op: apply_picker_frame applies the frame first, then opens the panel (K3 safe)
                                   // K3: field commits first; while a field holds invalid text the frame's presses are dropped
        fields::finish_frame(&self.ctx, self.doc_active, &mut ops, &mut self.field_pending);
        ops.retain(|op| {
            if let Op::FitArtboard(index) = op {
                self.fit_request = Some(*index);
                return false;
            }
            true
        });
        self.text_tool.finish_ops(ed, &mut ops);
        apply_picker_frame(ed, snap_cfg, ops, &mut self.color_panel);
        layout::sync_picker_open(&mut self.picker_layout, self.color_panel.as_ref());
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

// ---- Lane B w3-effects ----
pub(crate) use effects::effects_menu_rows;
// ---- end Lane B w3-effects ----
