//! Lane E provisional Navigator; existing box shell and kit typography.
use super::*;
use varos_core::view_depth::{navigator_bounds, navigator_camera, DepthAction};
#[allow(clippy::too_many_arguments)] // Panel snapshot plus deferred commands; follows incumbent panel builders.
pub(super) fn draw(
    ui: &mut egui::Ui,
    ed: &Editor,
    view: View,
    ppp: f32,
    canvas: egui::Rect,
    session: u64,
    commands: &mut Vec<Op>,
) {
    let size = egui::vec2(ui.available_width(), varos_app::shell::tokens::NAVIGATOR_HEIGHT);
    let (slot, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let rect = proxy_rect(slot);
    let response = ui.interact(rect, ui.id().with("navigator-pan"), egui::Sense::click_and_drag());
    let camera = navigator_camera(navigator_bounds(ed), [224.0, 126.0]);
    let id = egui::Id::new(("navigator-cache", session));
    let mut cache =
        ui.ctx().data_mut(|d| d.remove_temp::<crate::thumbs::navigator::NavigatorThumb>(id)).unwrap_or_default();
    if let Some(tex) = cache.get(ui.ctx(), ed, session, camera) {
        ui.painter().image(
            tex,
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            varos_app::shell::tokens::NAVIGATOR_IMAGE_TINT,
        );
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, cache));
    let to_proxy = |p: Pt| {
        let q = camera.w2s(p);
        rect.min + egui::vec2(q[0] * rect.width() / 224.0, q[1] * rect.height() / 126.0)
    };
    let a = view.s2w([canvas.min.x * ppp, canvas.min.y * ppp]);
    let b = view.s2w([canvas.max.x * ppp, canvas.max.y * ppp]);
    ui.painter().with_clip_rect(rect).rect_stroke(
        egui::Rect::from_min_max(to_proxy(a), to_proxy(b)),
        CornerRadius::ZERO,
        Stroke::new(1.0, ACCENT),
        StrokeKind::Inside,
    );
    if response.dragged() || response.clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            let q = p - rect.min;
            let center = camera.s2w([q.x * 224.0 / rect.width(), q.y * 126.0 / rect.height()]);
            commands.push(Op::View(varos_core::editor::view_commands::ViewAction::Depth(DepthAction::NavigatorPan {
                center,
            })));
        }
    }
    ui.add_space(LABEL_GAP);
    ui.label(micro_label(format!("{:.0}%", view.zoom * 100.0)));
    let (track, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), varos_app::shell::tokens::KIT_CONTROL_H),
        egui::Sense::click_and_drag(),
    );
    let t = ((view.zoom / 0.02).ln() / (64.0_f32 / 0.02).ln()).clamp(0.0, 1.0);
    ui.painter().line_segment([track.left_center(), track.right_center()], Stroke::new(1.0, BORDER));
    ui.painter().circle_filled(egui::pos2(track.left() + t * track.width(), track.center().y), f32::from(R), ACCENT);
    if response.dragged() || response.clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            let t = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            let percent = 2.0 * (3200.0_f32).powf(t);
            commands.push(Op::View(varos_core::editor::view_commands::ViewAction::Depth(DepthAction::NavigatorZoom {
                percent,
            })));
        }
    }
}

fn proxy_rect(slot: egui::Rect) -> egui::Rect {
    let proxy = varos_app::shell::tokens::NAVIGATOR_PROXY;
    let scale = (slot.width() / proxy[0] as f32).min(slot.height() / proxy[1] as f32);
    egui::Rect::from_center_size(slot.center(), egui::vec2(proxy[0] as f32, proxy[1] as f32) * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_click_dispatches_pan_without_a_drag() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        let ed = Editor::new();
        let mut ops = Vec::new();
        let mut point = egui::Pos2::ZERO;
        for frame in 0..3 {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(240.0, 240.0))),
                ..Default::default()
            };
            if frame > 0 {
                input.events = vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 1,
                        modifiers: Default::default(),
                    },
                ];
            }
            let _ = ctx.run_ui(input, |ui| {
                point = proxy_rect(egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), varos_app::shell::tokens::NAVIGATOR_HEIGHT),
                ))
                .center();
                draw(
                    ui,
                    &ed,
                    View::identity(),
                    1.0,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
                    1,
                    &mut ops,
                );
            });
        }
        let bounds = navigator_bounds(&ed);
        assert!(ops.iter().any(|op| matches!(op,
            Op::View(varos_core::editor::view_commands::ViewAction::Depth(DepthAction::NavigatorPan { center }))
                if (center[0] - bounds[0] - bounds[2] / 2.0).abs() < 0.001
                    && (center[1] - bounds[1] - bounds[3] / 2.0).abs() < 0.001)));
    }
    #[test]
    fn proxy_keeps_board_aspect_when_panel_width_changes() {
        for size in [egui::vec2(100.0, 126.0), egui::vec2(400.0, 126.0)] {
            let slot = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let rect = proxy_rect(slot);
            assert!(slot.contains_rect(rect));
            assert!((rect.width() / rect.height() - 224.0 / 126.0).abs() < 0.001);
        }
    }
    #[test]
    fn navigator_panel_settles_to_wait_without_gpu_or_window() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        let ed = Editor::new();
        let mut ops = Vec::new();
        for frame in 0..6 {
            let out = ctx.run_ui(Default::default(), |ui| {
                draw(
                    ui,
                    &ed,
                    View::identity(),
                    1.0,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
                    1,
                    &mut ops,
                )
            });
            if frame == 5 {
                let now = Instant::now();
                let next =
                    out.viewport_output.get(&egui::ViewportId::ROOT).and_then(|v| now.checked_add(v.repaint_delay));
                let plan = crate::pacing::plan(now, next, &[], false);
                assert!(!plan.redraw);
                assert_eq!(plan.flow, crate::pacing::Flow::Wait);
                assert!(ops.is_empty());
            }
        }
    }
}
