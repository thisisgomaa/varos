use super::*;

/// HAND 2 — the floating tool rail (§4.4), pinned INSIDE the board box: a normal box look (panel
/// fill + hairline + rounded, NO shadow) floating over the canvas hole. Never a tile.
pub(crate) fn board_rail(
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
                construction_tools(ui, s.tool, ops);
                divider(ui);
                fill_stroke_control(ui, s, ops); // Illustrator's fill/stroke box at the rail foot
            });
        });
}
