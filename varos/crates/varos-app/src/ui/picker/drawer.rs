use super::*;
pub(crate) fn show(ui: &mut egui::Ui, m: &mut ColorPanel, s: &Snap, layout: &mut PickerLayout, ops: &mut Vec<Op>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(m.width(), t::PICKER_SWATCH_ROW_H), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.top(), t::hairline());
    for i in 0..if m.mini() { 7 } else { 9 } {
        let r = egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    t::PICKER_PAD + i as f32 * (t::PICKER_SWATCH + t::PICKER_SWATCH_GAP),
                    (t::PICKER_SWATCH_ROW_H - t::PICKER_SWATCH) / 2.0,
                ),
            egui::Vec2::splat(t::PICKER_SWATCH),
        );
        let color = s.recent.get(i).copied();
        if let Some(c) = color {
            if c[3] < 1.0 {
                checker(&ui.painter_at(r), r, t::PICKER_CHECKER);
            }
        }
        ui.painter().rect(
            r,
            t::r_ctrl(),
            color.map(rgba_c32a).unwrap_or(t::SURFACE),
            t::hairline(),
            StrokeKind::Inside,
        );
        if color.is_none() {
            mixed_swatch(ui.painter(), r);
        }
        let response = ui.interact(r, ui.id().with(("recent", i)), egui::Sense::click());
        if let Some(c) = color {
            if response.clicked() && !kit::field::blocked(ui.ctx()) {
                ops.push(Op::PickerSet(m.target, c));
            }
            response.on_hover_text(hex_of(c));
        } else {
            response.on_hover_text("Recent colour — empty");
        }
    }
    let r = egui::Rect::from_center_size(
        egui::pos2(rect.right() - t::PICKER_PAD - t::PICKER_GLYPH / 2.0, rect.center().y),
        egui::Vec2::splat(t::PICKER_GLYPH),
    );
    Icon::ChevronDown.paint(ui.painter(), r.center(), t::PICKER_GLYPH, t::MUTED);
    if ui
        .interact(r, ui.id().with("drawer"), egui::Sense::click())
        .on_hover_text("Recent / Board / Document colours")
        .clicked()
        && !kit::field::blocked(ui.ctx())
    {
        layout.drawer_open = !layout.drawer_open;
    }
    if layout.drawer_open {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(m.width(), t::PICKER_DRAWER_H), egui::Sense::hover());
        let mut drawer_ui = ui.new_child(egui::UiBuilder::new().id_salt("picker-drawer").max_rect(rect));
        drawer_ui.set_clip_rect(rect);
        egui::ScrollArea::vertical().max_height(t::PICKER_DRAWER_H).show(&mut drawer_ui, |ui| {
            if let Some(tab) = segmented_text(
                ui,
                ui.id().with("drawer-tabs"),
                m.width() / 3.0,
                &["Recent", "Board", "Document"],
                None,
                layout.drawer_tab as usize,
            ) {
                layout.drawer_tab = tab as u8;
            }
            let colors = source(s, layout.drawer_tab);
            ui.horizontal_wrapped(|ui| {
                for c in colors {
                    let (r, response) =
                        ui.allocate_exact_size(egui::Vec2::splat(t::PICKER_SWATCH), egui::Sense::click());
                    if c[3] < 1.0 {
                        checker(&ui.painter_at(r), r, t::PICKER_CHECKER);
                    }
                    ui.painter().rect_filled(r, t::r_ctrl(), rgba_c32a(*c));
                    if response.on_hover_text(hex_of(*c)).clicked() && !kit::field::blocked(ui.ctx()) {
                        ops.push(Op::PickerSet(m.target, *c));
                    }
                }
            });
            if colors.is_empty() {
                kit::text(ui, "No colours yet", t::mono(), t::MUTED);
            }
        });
    }
}

pub(super) fn source(s: &Snap, tab: u8) -> &[Rgba] {
    match tab {
        0 => &s.recent,
        1 => &s.board_colors,
        _ => &s.doc_colors,
    }
}
