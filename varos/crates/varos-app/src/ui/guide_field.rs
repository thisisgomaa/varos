use super::*;
use varos_app::shell::tokens::{GUIDE_FIELD_INSET, GUIDE_FIELD_PAD, GUIDE_FIELD_W, GUIDE_HIT_HALF};
use varos_core::editor::view_commands::ViewAction;
/// Double-click an unlocked ruler guide to edit its coordinate through the ordinary K3 field.
pub(crate) fn show(ui: &mut egui::Ui, board: egui::Rect, view: View, ppp: f32, ed: &Editor, ops: &mut Vec<Op>) {
    let key = doc_id(ui, "guide-position-editor");
    let mut open = ui.data(|d| d.get_temp::<usize>(key));
    if !ed.guides_hidden && !ed.doc.guides_locked {
        for (index, guide) in ed.doc.guides.iter().enumerate() {
            let screen = view.w2s(if guide.vertical { [guide.pos, 0.0] } else { [0.0, guide.pos] });
            let rect = if guide.vertical {
                egui::Rect::from_min_max(
                    egui::pos2(screen[0] / ppp - GUIDE_HIT_HALF, board.top()),
                    egui::pos2(screen[0] / ppp + GUIDE_HIT_HALF, board.bottom()),
                )
            } else {
                egui::Rect::from_min_max(
                    egui::pos2(board.left(), screen[1] / ppp - GUIDE_HIT_HALF),
                    egui::pos2(board.right(), screen[1] / ppp + GUIDE_HIT_HALF),
                )
            }
            .intersect(board);
            if rect.is_positive() && ui.interact(rect, key.with(index), egui::Sense::click()).double_clicked() {
                open = Some(index);
            }
        }
    }
    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        open = None;
    }
    if let Some(index) = open {
        if let Some(guide) = ed.doc.guides.get(index).filter(|_| !ed.guides_hidden && !ed.doc.guides_locked) {
            egui::Area::new(key.with("field"))
                .order(egui::Order::Foreground)
                .fixed_pos(board.left_top() + egui::vec2(GUIDE_FIELD_INSET, GUIDE_FIELD_INSET))
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(SOLID_PANEL)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(RBOX)
                        .inner_margin(Margin::same(GUIDE_FIELD_PAD))
                        .show(ui, |ui| {
                            fields::num(
                                ui,
                                GUIDE_FIELD_W,
                                Lab::Letter(if guide.vertical { "X" } else { "Y" }),
                                "Guide position",
                                guide.pos,
                                2,
                                1.0,
                                -1.0e6..=1.0e6,
                                ops,
                                |position| Op::View(ViewAction::GuidePosition { index, position }),
                            );
                        });
                });
        } else {
            open = None;
        }
    }
    ui.data_mut(|d| {
        if let Some(index) = open {
            d.insert_temp(key, index);
        } else {
            d.remove::<usize>(key);
        }
    });
}
