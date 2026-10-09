use super::*;

/// Lane D: kit rail groups with remembered tools and press/right-click flyouts.
pub(crate) fn board_rail(ctx: &egui::Context, board: egui::Rect, s: &Snap, ops: &mut Vec<Op>) {
    egui::Area::new(egui::Id::new("hand2-rail"))
        .order(egui::Order::Middle)
        .pivot(Align2::LEFT_CENTER)
        .fixed_pos(egui::pos2(board.left() + varos_app::shell::tokens::DRAW_RAIL_OFFSET, board.center().y))
        .show(ctx, |ui| {
            super::drawing::frame().show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = varos_app::shell::tokens::KIT_GAP;
                for group in super::rail_flyout::GROUPS {
                    super::rail_flyout::slot(ui, group, s.tool, ops);
                }
                divider(ui);
                fill_stroke_control(ui, s, ops);
            });
        });
}
