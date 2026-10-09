use super::super::*;
use varos_app::shell::tokens as t;

pub(crate) fn document_section(
    ui: &mut egui::Ui,
    s: &Snap,
    w: f32,
    ops: &mut Vec<Op>,
    recovery: (&crate::recovery_host::RecoveryUi, &mut Vec<AppCommand>),
) {
    board_section(ui, s, w, ops);
    hsep(ui, w);
    ui.label(micro_label("DOCUMENT"));
    label_gap(ui);
    ui.horizontal(|ui| {
        let units = varos_core::units::Unit::ALL.map(|unit| unit.suffix());
        if let Some(index) = kit::text_dropdown(
            ui,
            doc_id(ui, "document-units"),
            s.units_label,
            &units,
            t::DOC_UNITS_W,
            "Document units",
        ) {
            ops.push(Op::Units(varos_core::units::Unit::ALL[index]));
        }
        let names = s.artboard_names.iter().map(String::as_str).collect::<Vec<_>>();
        let name = names.get(s.active_artboard).copied().unwrap_or("No artboards");
        let width = (w - t::DOC_UNITS_W - t::ICON_BTN_W * 2.0 - t::PANEL_ITEM_GAP_X * 3.0).max(t::KIT_MIN_TARGET);
        if let Some(index) =
            kit::text_dropdown(ui, doc_id(ui, "document-artboards"), name, &names, width, "Active artboard")
        {
            ops.push(Op::AbActive(index));
        }
        for (action, next) in [
            (IA_PREV_BOARD, adjacent_artboard(s.active_artboard, s.artboards, false)),
            (IA_NEXT_BOARD, adjacent_artboard(s.active_artboard, s.artboards, true)),
        ] {
            let state = if next.is_some() {
                kit::IconState::Action
            } else {
                kit::IconState::Disabled("No artboard in this direction")
            };
            if action.show(ui, state) {
                if let Some(index) = next {
                    ops.push(Op::AbActive(index));
                }
            }
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t::PANEL_STRIP_GAP;
        if IA_RULERS.show(ui, kit::IconState::Toggle(s.rulers_on)) {
            ops.push(Op::ToggleRulers);
        }
        if IA_GRID.show(ui, kit::IconState::Toggle(s.snap_config.show_grid)) {
            ops.push(Op::View(varos_core::editor::view_commands::ViewAction::ToggleGrid));
        }
        strip_divider(ui);
        if IA_GUIDES.show(ui, kit::IconState::Toggle(s.guides_on)) {
            ops.push(Op::ToggleGuides);
        }
        if IA_GUIDES_LOCK.show(ui, kit::IconState::Toggle(s.guides_locked)) {
            ops.push(Op::ToggleGuidesLock);
        }
        if IA_SMART.show(ui, kit::IconState::Toggle(s.snap_config.smart)) {
            ops.push(Op::ToggleSmartGuides);
        }
        strip_divider(ui);
        if IA_SNAP.show(ui, kit::IconState::Toggle(s.snap_enabled)) {
            ops.push(Op::ToggleSnapping);
        }
        if IA_POINT.show(ui, kit::IconState::Toggle(s.snap_config.key_points)) {
            ops.push(Op::ToggleSnapPoint);
        }
        if IA_SNAP_GRID.show(ui, kit::IconState::Toggle(s.snap_config.grid)) {
            ops.push(Op::ToggleSnapGrid);
        }
    });
    ui.horizontal(|ui| {
        if IA_EDIT_BOARDS.show(ui, kit::IconState::Action) {
            ops.push(Op::Tool(ToolKind::Artboard));
        }
        if IA_FIT_BOARD.show(
            ui,
            if s.artboards == 0 { kit::IconState::Disabled("No artboard to fit") } else { kit::IconState::Action },
        ) {
            ops.push(Op::FitArtboard(s.active_artboard));
        }
        if IA_ADD_BOARD.show(ui, kit::IconState::Action) {
            ops.push(Op::AbAdd);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new("Colour RGB").color(MUTED).font(t::small()));
        });
    });
    fields::num(
        ui,
        w,
        Lab::Letter("Grid"),
        "Grid spacing",
        s.snap_config.grid_spacing,
        2,
        1.0,
        0.01..=1.0e6,
        ops,
        |spacing| {
            Op::View(varos_core::editor::view_commands::ViewAction::Grid {
                spacing,
                subdivisions: s.snap_config.grid_subdivisions,
            })
        },
    );
    fields::num(
        ui,
        w,
        Lab::Letter("Sub"),
        "Grid subdivisions",
        s.snap_config.grid_subdivisions as f32,
        0,
        1.0,
        1.0..=100.0,
        ops,
        |v| {
            Op::View(varos_core::editor::view_commands::ViewAction::Grid {
                spacing: s.snap_config.grid_spacing,
                subdivisions: v.round() as u32,
            })
        },
    );
    hsep(ui, w);
    let (recovery, commands) = recovery;
    super::autosave::settings(ui, w, recovery, commands);
    if toggle_row(ui, w, "Recovery (all documents)", recovery.enabled) {
        commands.push(AppCommand::SetRecoveryEnabled(!recovery.enabled));
    }
    ui.label(RichText::new(&recovery.status).color(MUTED).size(12.0));
    if !recovery.last_copy.is_empty() {
        ui.label(RichText::new(&recovery.last_copy).color(MUTED).size(11.0));
    }
    if !recovery.detail.is_empty() {
        ui.label(RichText::new(&recovery.detail).color(MUTED).size(11.0));
    }
    if let Some(id) = recovery.sid.filter(|_| recovery.retry) {
        ui.horizontal(|ui| {
            use varos_app::shell::kit::{self, Control};
            if kit::action(ui, Control::new(ui.id().with("recovery-retry"), "Retry"), false).activated {
                commands.push(AppCommand::RetryRecovery(id));
            }
            if kit::action(ui, Control::new(ui.id().with("recovery-save"), "Save document"), false).activated {
                commands.push(AppCommand::Save(id));
            }
        });
    }
}

/// Boundary navigation never wraps and handles the empty document honestly.
pub(crate) fn adjacent_artboard(active: usize, count: usize, next: bool) -> Option<usize> {
    if active >= count {
        return None;
    }
    if next {
        active.checked_add(1).filter(|i| *i < count)
    } else {
        active.checked_sub(1)
    }
}
