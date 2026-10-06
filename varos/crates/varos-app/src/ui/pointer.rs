use super::Ui;

impl Ui {
    /// The background box tree is chrome except for the Board's canvas hole.
    /// Floating layers (control bar, menus, cards) always own their area.
    pub fn wants_pointer(&self) -> bool {
        if self.ctx.egui_is_using_pointer() {
            return true;
        }
        let Some(pos) = self.ctx.input(|i| i.pointer.interact_pos()) else {
            return false;
        };
        chrome_owns_point(&self.ctx, self.board_hole, pos)
    }

    /// Native CursorMoved precedes gestures, before egui paints another frame.
    /// Check the current position too so entering floating chrome cannot leak a gesture.
    pub fn wants_pointer_at(&self, physical: [f32; 2]) -> bool {
        self.ctx.egui_is_using_pointer()
            || chrome_owns_point(
                &self.ctx,
                self.board_hole,
                egui::pos2(physical[0], physical[1]) / self.ctx.pixels_per_point(),
            )
    }
}

fn chrome_owns_point(ctx: &egui::Context, board_hole: Option<egui::Rect>, pos: egui::Pos2) -> bool {
    match ctx.layer_id_at(pos) {
        None => false,
        Some(l) if l.order == egui::Order::Background => !board_hole.is_some_and(|b| b.contains(pos)),
        Some(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gestures::{self, Gesture};
    use varos_core::geom::View;

    #[test]
    fn floating_chrome_and_recovery_panel_block_gestures_inside_board_bounds() {
        let ctx = egui::Context::default();
        let board = egui::Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(500.0, 400.0));
        let mut bar = egui::Rect::NOTHING;
        let mut card = egui::Rect::NOTHING;
        // Headless egui only: no Ui::new (which needs a window), EventLoop or GPU.
        for _ in 0..3 {
            let _ = ctx.run_ui(egui::RawInput::default(), |root| {
                root.set_min_size(egui::vec2(600.0, 500.0));
                bar = egui::Area::new(egui::Id::new("gesture-control-bar"))
                    .order(egui::Order::Middle)
                    .fixed_pos(egui::pos2(100.0, 40.0))
                    .show(&ctx, |ui| {
                        ui.allocate_space(egui::vec2(120.0, 36.0));
                    })
                    .response
                    .rect;
                card = egui::Area::new(egui::Id::new("gesture-recovery-card"))
                    .order(egui::Order::Middle)
                    .fixed_pos(egui::pos2(100.0, 150.0))
                    .show(&ctx, |ui| {
                        ui.allocate_space(egui::vec2(160.0, 80.0));
                    })
                    .response
                    .rect;
            });
        }
        for pos in [bar.center(), card.center(), egui::pos2(10.0, 10.0)] {
            let blocked = chrome_owns_point(&ctx, Some(board), pos);
            assert!(blocked, "chrome at {pos:?}");
            let mut view = View::identity();
            assert!(!gestures::apply(&mut view, [pos.x, pos.y], Gesture::Pinch(0.5), blocked));
            assert_eq!(view.zoom, 1.0);
            assert_eq!(view.pan, [0.0, 0.0]);
        }
        assert!(!chrome_owns_point(&ctx, Some(board), egui::pos2(400.0, 350.0)));
    }
}
