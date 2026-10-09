// ---- Lane F: text adapters ----
use varos_app::shell::kit::text::ShapedResponse as _;
// ---- end Lane F ----
use super::*;
/// The Board's second hand. Only the header's spare space drags; body gestures edit colour.
pub(crate) fn build_color_panel(
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
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("picker-mini"), m.mini()));
    if m.mini() {
        mini::build(ctx, panel, s, ops, sample, board, layout);
        return;
    }
    let field_open = kit::field::any_open(ctx);
    let menu_open = kit::menu_open(ctx);
    m.mode = layout.mode;
    m.harmony = layout.harmony;
    let mut close = false;
    let height = if m.tab == Tab::Gradient {
        t::GRADIENT_BODY_H + t::PICKER_HEADER_H
    } else if m.tab == Tab::Sliders {
        t::PICKER_HEADER_H
            + sliders::body_height(m.mode)
            + if m.mode == modes::Mode::Cmyk { managed::height() } else { 0. }
            + t::PICKER_FIELD_ROW_H
            + t::PICKER_SWATCH_ROW_H
    } else {
        t::PICKER_H
    };
    let height = height
        + if m.tab == Tab::Harmony {
            t::PICKER_HARMONY_STRIP_H
                + t::PICKER_HARMONY_ROW_H
                + t::COLOUR_GUIDE_ROW_H
                    * harmony_rules::linked(m.harmony, [m.hsva[0], m.hsva[1], m.hsva[2]]).len() as f32
        } else {
            0.0
        };
    let height = height + if layout.drawer_open { t::PICKER_DRAWER_H } else { 0.0 };
    let pos = layout
        .position
        .map(|p| board.min + egui::vec2(p[0], p[1]))
        .unwrap_or(board.min + egui::vec2(t::PICKER_DEFAULT_OFFSET[0], t::PICKER_DEFAULT_OFFSET[1]));
    let pos = egui::pos2(
        pos.x.clamp(board.left(), (board.right() - t::PICKER_W).max(board.left())),
        pos.y.clamp(board.top(), (board.bottom() - height).max(board.top())),
    );
    let panel_rect = egui::Rect::from_min_size(pos, egui::vec2(t::PICKER_W, height));
    let response = egui::Area::new(egui::Id::new("hand2-colour-picker"))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .movable(false)
        .constrain_to(board)
        .show(ctx, |ui| {
            ui.set_clip_rect(board);
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            panel_frame(0).show(ui, |ui| {
                ui.set_width(t::PICKER_W);
                let (header, _) =
                    ui.allocate_exact_size(egui::vec2(t::PICKER_W, t::PICKER_HEADER_H), egui::Sense::hover());
                #[cfg(test)]
                super::super::fields::tests::probe("picker header", header);
                for (i, (icon, tab, tip)) in [
                    (Icon::PickerWheel, Some(Tab::Wheel), "Wheel"),
                    (Icon::PickerSliders, Some(Tab::Sliders), "Sliders"),
                    (Icon::PickerHarmony, Some(Tab::Harmony), "Harmony"),
                    (Icon::PickerGradient, Some(Tab::Gradient), "Gradient"),
                ]
                .into_iter()
                .enumerate()
                {
                    let r = egui::Rect::from_min_size(
                        header.min
                            + egui::vec2(
                                t::PICKER_PAD + i as f32 * (t::PICKER_TAB_W + t::PICKER_TAB_GAP),
                                (t::PICKER_HEADER_H - t::PICKER_TAB_H) / 2.0,
                            ),
                        egui::vec2(t::PICKER_TAB_W, t::PICKER_TAB_H),
                    );
                    let response = ui.interact(
                        r,
                        ui.id().with(tip),
                        if tab.is_some() && !kit::field::blocked(ui.ctx()) {
                            egui::Sense::click()
                        } else {
                            egui::Sense::hover()
                        },
                    );
                    let on = tab == Some(m.tab);
                    if on || response.hovered() {
                        ui.painter().rect_filled(r, t::r_ctrl(), if on { t::TOGGLE_WELL } else { t::HOVER });
                    }
                    icon.paint(
                        ui.painter(),
                        r.center(),
                        t::PICKER_GLYPH,
                        if tab.is_none() {
                            t::DISABLED
                        } else if on || response.hovered() {
                            t::TEXT
                        } else {
                            t::MUTED
                        },
                    );
                    if response.shaped_hover_text(tip).clicked() {
                        if let Some(tab) = tab {
                            if tab != m.tab {
                                m.finish(ops);
                            }
                            m.tab = tab;
                        }
                    }
                }
                let drag = egui::Rect::from_min_max(
                    header.min + egui::vec2(t::PICKER_PAD + 4.0 * (t::PICKER_TAB_W + t::PICKER_TAB_GAP), 0.0),
                    header.right_top() + egui::vec2(-2.0 * t::PICKER_TAB_W - t::PICKER_PAD, header.height()),
                );
                let drag_response = ui.interact(
                    drag,
                    ui.id().with("header-drag"),
                    if kit::field::blocked(ui.ctx()) { egui::Sense::hover() } else { egui::Sense::drag() },
                );
                if drag_response.dragged() {
                    let p = pos + drag_response.drag_delta() - board.min;
                    layout.position = Some([p.x, p.y]);
                }
                let mut header_ui =
                    ui.new_child(egui::UiBuilder::new().id_salt("header-actions").max_rect(egui::Rect::from_min_max(
                        header.right_top()
                            + egui::vec2(
                                -2.0 * t::PICKER_TAB_W - t::PICKER_PAD,
                                (t::PICKER_HEADER_H - t::PICKER_TAB_H) / 2.0,
                            ),
                        header.max,
                    )));
                header_ui.horizontal(|ui| {
                    if kit::icon_button_sized(
                        ui,
                        ui.id().with("eye"),
                        Icon::Pipette,
                        "Eyedropper (I)",
                        kit::IconState::Toggle(m.eyedropping),
                        egui::vec2(t::PICKER_TAB_W, t::PICKER_TAB_H),
                        t::PICKER_GLYPH,
                    )
                    .activated
                    {
                        if m.eyedropping {
                            m.eyedropping = false;
                            m.finish(ops);
                        } else if !m.disarmed_press && !kit::field::blocked(ui.ctx()) {
                            m.arm();
                        }
                    }
                    close = IA_PICKER_CLOSE.show_sized(
                        ui,
                        kit::IconState::Action,
                        egui::vec2(t::PICKER_TAB_W, t::PICKER_TAB_H),
                        t::PICKER_GLYPH,
                    );
                });
                ui.painter().hline(header.x_range(), header.bottom(), t::hairline());
                if m.tab == Tab::Wheel {
                    wheel::show(ui, m, s, ops);
                } else if m.tab == Tab::Sliders {
                    sliders::show(ui, m, s, layout, ops);
                    // ---- w3-cmyk ----
                    managed::show(ui, m, ops);
                } else if m.tab == Tab::Gradient {
                    gradient::show(ui, m, ops);
                } else {
                    harmony::show(ui, m, s, layout, ops);
                }
                ui.add_space((t::PICKER_FIELD_ROW_H - t::FIELD_H) / 2.0);
                if m.tab != Tab::Gradient {
                    fields::show(ui, m, ops, m.tab != Tab::Sliders);
                }
                ui.add_space((t::PICKER_FIELD_ROW_H - t::FIELD_H) / 2.0);
                drawer::show(ui, m, s, layout, ops);
            });
        });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("picker-panel-rect"), panel_rect));
    #[cfg(test)]
    super::super::fields::tests::probe("picker panel", response.response.rect);
    if ctx.input(|i| i.pointer.primary_released()) {
        m.disarmed_press = false;
    }
    if layout.position.is_none() {
        let p = response.response.rect.min - board.min;
        layout.position = Some([p.x, p.y]);
    }
    close = settle(ctx, m, ops, sample, board, response.response.rect, field_open, menu_open, close);
    if close && !kit::field::blocked(ctx) {
        m.finish(ops);
        ops.push(Op::PickerClose);
        *panel = None;
        layout.open = false;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn settle(
    ctx: &egui::Context,
    m: &mut ColorPanel,
    ops: &mut Vec<Op>,
    sample: Option<Rgba>,
    board: egui::Rect,
    rect: egui::Rect,
    field_open: bool,
    menu_open: bool,
    mut close: bool,
) -> bool {
    if m.eyedropping && !kit::field::blocked(ctx) {
        let sample = if crate::cursors::SCREEN_EYEDROPPER { crate::cursors::screen_color_at_cursor() } else { sample };
        if let Some(c) = sample {
            m.start(Gesture::Sample, ops);
            m.adopt(c);
        }
        let down = ctx.input(|i| i.pointer.primary_pressed() && !i.key_down(egui::Key::Space));
        if down
            && sample.is_some()
            && ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
                board.contains(p)
                    && !rect.contains(p)
                    && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)
            })
        {
            m.accept_sample(ops);
        }
    }
    if let Some(c) = m.live_color() {
        ops.push(Op::PickerLive(m.target, c));
    }
    if m.gesture.is_some_and(|g| g != Gesture::Sample) && !ctx.input(|i| i.pointer.primary_down()) {
        m.finish(ops);
    }
    if !field_open
        && !menu_open
        && (m.eyedropping || ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p))))
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        if m.eyedropping {
            m.finish(ops);
        } else {
            close = true;
        }
    }
    close
}
