//! Slice 4E/4D provisional controls, pending owner design review; kit owns all interaction/paint.
use super::super::*;
use varos_app::shell::kit::{self, icons::Icon, IconState};
use varos_core::planar::PathfinderOp;

pub(crate) fn construction_row(ui: &mut egui::Ui, pf: Result<(), &'static str>, ops: &mut Vec<Op>) {
    ui.horizontal(|ui| {
        for (operation, icon, label) in [
            (PathfinderOp::Divide, Icon::PathDivide, "Divide"),
            (PathfinderOp::Trim, Icon::PathTrim, "Trim"),
            (PathfinderOp::Merge, Icon::PathMerge, "Merge"),
            (PathfinderOp::Crop, Icon::Crop, "Crop"),
            (PathfinderOp::Outline, Icon::PathOutline, "Outline"),
            (PathfinderOp::MinusBack, Icon::PathMinusBack, "Minus Back"),
        ] {
            let state = pf.err().map_or(IconState::Action, IconState::Disabled);
            if kit::icon_button_sized(
                ui,
                ui.id().with(label),
                icon,
                label,
                state,
                egui::vec2(PF_BAR_W, PF_BAR_H),
                ICON_LG,
            )
            .activated
            {
                ops.push(Op::Pathfinder(operation));
            }
        }
    });
    if kit::icon_button(ui, ui.id().with("divide-below"), Icon::PathDivide, "Divide Objects Below", IconState::Action)
        .activated
    {
        ops.push(Op::DivideObjectsBelow);
    }
}
pub(crate) fn construction_tools(ui: &mut egui::Ui, active: ToolKind, ops: &mut Vec<Op>) {
    for (tool, icon, label) in [
        (ToolKind::ShapeBuilder, Icon::PathMerge, "Shape Builder (Shift+M) — Alt-drag deletes"),
        (ToolKind::Scissors, Icon::PathScissors, "Scissors (C)"),
        (ToolKind::Knife, Icon::PathKnife, "Knife"),
        (ToolKind::Eraser, Icon::PathEraser, "Eraser (Shift+E)"),
    ] {
        let state = IconState::Tool(active == tool);
        if kit::icon_button(ui, ui.id().with(label), icon, label, state).activated {
            ops.push(Op::Tool(tool));
        }
    }
}
