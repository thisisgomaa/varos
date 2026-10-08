use super::super::*;
use super::{Layout, Ui};

struct NoDisplay;
impl raw_window_handle::HasDisplayHandle for NoDisplay {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::Unavailable)
    }
}

fn headless_ui() -> Ui {
    let ctx = egui::Context::default();
    Ui {
        ctx: ctx.clone(),
        state: egui_winit::State::new(ctx, egui::ViewportId::ROOT, &NoDisplay, None, None, None),
        repaint_at: None,
        recovery: Default::default(),
        file_status: String::new(),
        export_sheet: None,
        panel_column: None,
        export_scopes: Default::default(),
        tools: vec![],
        shapes: vec![],
        shape_active: ToolKind::Rect,
        ic_rotate: None,
        ic_opacity: None,
        ic_strokew: None,
        ic_fit: None,
        ic_pipette: None,
        align_icons: Default::default(),
        cursor: egui::CursorIcon::Default,
        refpt: (0.0, 0.0),
        lock: false,
        ab_lock: false,
        align_target: super::super::AlignTarget::default(),
        ab_name_edit: None,
        fit_request: None,
        top: super::super::TopIcons { menu: None },
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
        layer_icons: super::super::LayerIcons { eye: None, eye_off: None, lock: None, unlock: None, search: None },
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

#[test]
fn restoring_default_resets_tree_rail_control_bar_and_derived_span() {
    let mut ui = headless_ui();
    ui.shell.toggle_panel(varos_app::shell::PanelId::Align);
    ui.show_rail = false;
    ui.show_dock = false;
    ui.panel_column = Some(egui::Rangef::new(10.0, 20.0));
    assert_ne!(ui.shell_layout(), Layout::default());
    ui.restore_shell_layout(Layout::default());
    assert_eq!(ui.shell_layout(), Layout::default());
    assert_eq!(ui.panel_column, None);
}
