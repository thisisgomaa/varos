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
        canvas_hint: Default::default(),
        document_sheet: None,
        export_sheet: None,
        panel_column: None,
        export_scopes: Default::default(),
        ic_rotate: None,
        ic_opacity: None,
        ic_strokew: None,
        ic_fit: None,
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
        color_panel: None,
        picker_board_colors: Default::default(),
        picker_layout: Default::default(),
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

#[test]
fn picker_layout_restores_open_relative_position_and_drawer_without_document_edits() {
    let mut ui = headless_ui();
    let layout = varos_app::storage::layout::Layout {
        picker: varos_app::storage::layout::PickerLayout {
            open: true,
            position: Some([28.0, 60.0]),
            drawer_open: true,
            drawer_tab: 2,
            mode: Default::default(),
            harmony: Default::default(),
        },
        ..Default::default()
    };
    ui.restore_shell_layout(layout.clone());
    let mut ed = varos_core::editor::Editor::new();
    let before = ed.doc.clone();
    ui.prepare_picker(&mut ed);
    assert!(ui.picker_open());
    assert!(ui.color_panel.is_some());
    assert!(!ed.transaction_open());
    assert_eq!(ui.shell_layout(), layout);
    ui.arm_picker(&mut ed);
    assert!(ui.picking_screen());
    ui.toggle_picker(&mut ed);
    assert!(!ui.picker_open());
    assert_eq!(ed.doc, before);
    ui.restore_shell_layout(Default::default());
    assert!(!ui.picker_open());
    assert_eq!(ui.picker_layout.position, None);
    assert!(!ui.picker_layout.drawer_open);
}

#[test]
fn window_colour_reports_big_panel_preference_while_mini_is_open() {
    let mut ui = headless_ui();
    let mut ed = varos_core::editor::Editor::new();
    ed.doc.artboards.push(varos_core::model::Artboard { id: 40, ..Default::default() });
    ui.picker_layout.open = true;
    let snap = ed.doc.snap;
    apply_picker_frame(&mut ed, snap, vec![Op::OpenMini(40, egui::Rect::NOTHING)], &mut ui.color_panel);
    assert!(ui.color_panel.as_ref().unwrap().mini());
    assert!(ui.picker_open());
    assert!(ui.shell_layout().picker.open);
    ui.color_panel = None;
    ui.prepare_picker(&mut ed);
    assert!(!ui.color_panel.as_ref().unwrap().mini());
    ui.picker_layout.open = false;
    apply_picker_frame(&mut ed, snap, vec![Op::OpenMini(40, egui::Rect::NOTHING)], &mut ui.color_panel);
    assert!(!ui.picker_open());
    ui.color_panel = None;
    ui.prepare_picker(&mut ed);
    assert!(ui.color_panel.is_none());
}
