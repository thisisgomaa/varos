//! Compact configuration of the same wheel/fields/drawer and transaction state machine.
use super::*;
pub(super) fn build(
    ctx: &egui::Context,
    panel: &mut Option<ColorPanel>,
    s: &Snap,
    ops: &mut Vec<Op>,
    sample: Option<Rgba>,
    board: egui::Rect,
    layout: &mut PickerLayout,
) {
    let Some(m) = panel else {
        return;
    };
    let Config::Mini(anchor) = m.config else {
        return;
    };
    if !matches!(m.target, MTarget::Ab(_)) {
        m.finish(ops);
        ops.push(Op::PickerClose);
        *panel = None;
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("picker-mini"), false));
        return;
    }
    if ctx.input(|i| i.pointer.primary_released()) {
        m.disarmed_press = false;
    }
    let field_open = kit::field::any_open(ctx);
    let menu_open = kit::menu_open(ctx);
    let height = t::PICKER_MINI_BODY_H
        + t::PICKER_FIELD_ROW_H
        + t::PICKER_SWATCH_ROW_H
        + if layout.drawer_open { t::PICKER_DRAWER_H } else { 0.0 };
    // Popovers can originate in the side panel, outside the Board hole.
    let screen = ctx.content_rect();
    let pos = egui::pos2(
        (anchor.right() - t::PICKER_MINI_W)
            .clamp(screen.left(), (screen.right() - t::PICKER_MINI_W).max(screen.left())),
        (anchor.bottom() + t::KIT_MENU_GAP).clamp(screen.top(), (screen.bottom() - height).max(screen.top())),
    );
    let response = egui::Area::new(egui::Id::new("mini-colour-picker"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .movable(false)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            panel_frame(0).show(ui, |ui| {
                ui.set_width(t::PICKER_MINI_W);
                wheel::show(ui, m, s, ops);
                ui.add_space((t::PICKER_FIELD_ROW_H - t::FIELD_H) / 2.0);
                fields::show(ui, m, ops, true);
                ui.add_space((t::PICKER_FIELD_ROW_H - t::FIELD_H) / 2.0);
                drawer::show(ui, m, s, layout, ops);
            });
        });
    let rect = response.response.rect;
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("picker-panel-rect"), rect));
    #[cfg(test)]
    super::super::fields::tests::probe("picker mini", rect);
    let outside = ctx.input(|i| {
        i.pointer.primary_pressed() && i.pointer.hover_pos().is_some_and(|p| !rect.contains(p) && !anchor.contains(p))
    }) && !m.eyedropping;
    let escape = !m.eyedropping
        && !field_open
        && !menu_open
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let close = panel::settle(ctx, m, ops, sample, board, rect, field_open, menu_open, escape || outside);
    if close && !kit::field::blocked(ctx) {
        m.finish(ops);
        ops.push(Op::PickerClose);
        *panel = None;
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("picker-mini"), false));
    }
}
