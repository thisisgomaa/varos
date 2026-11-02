//! The Tools contents as a panel-workspace host presents them: the real tool body, scrolled by
//! the host, which owns all moving, resizing and hiding; and the tool flyout placed beside it.
use super::metrics;
use crate::theme::Tokens;
use crate::{VectorcraftApp, widgets};
use egui::{Ui, pos2, vec2};

/// Real tool contents for a generic panel host. Its host owns all moving, resizing and hiding.
/// Tools stay vertical in one or two columns; a short host scrolls the entire strip.
pub(crate) fn content(app: &mut VectorcraftApp, ui: &mut Ui) {
    let bounds = ui.max_rect();
    let t = Tokens::get(ui.ctx());
    widgets::panel_scroll(
        ui,
        egui::ScrollArea::vertical().id_salt("hosted-tools-scroll").max_height(bounds.height()).auto_shrink([false, false]),
        |ui| {
            let cols = if ui.available_width() >= 2.0 * metrics::TOOL_BUTTON { 2 } else { 1 };
            crate::toolbar::body_with_layout(app, ui, cols, None, true);
            ui.vertical_centered(|ui| crate::toolbar::bottom_controls(app, ui, &t, cols));
        },
    );
    // Body sublayouts must not replace the outer host used by the drawer and flyout anchors.
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| {
        d.insert_temp(crate::edit_toolbar::toolbar_id(), bounds);
        d.insert_temp(panel_host_id(), (pass, bounds));
    });
}

fn panel_host_id() -> egui::Id {
    egui::Id::unique("panel-system-tools-host")
}

fn current_panel_host(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp::<(u64, egui::Rect)>(panel_host_id())).filter(|(pass, _)| *pass == ctx.cumulative_pass_nr()).map(|(_, rect)| rect)
}

/// Called once after the generic hosts. A hidden Tools tab has no live popup anchor.
pub(crate) fn popovers(app: &mut VectorcraftApp, ctx: &egui::Context, region: egui::Rect) {
    if current_panel_host(ctx).is_none() {
        app.ui.flyout = None;
        app.ui.tools_drawer = false;
    }
    crate::toolbar::shell_popovers(app, ctx, region);
}

/// Where the tool flyout `id` opens while the panel workspace hosts Tools: beside the live host,
/// facing the workspace; `None` leaves it to the other presenters.
pub(crate) fn flyout_at(app: &VectorcraftApp, ctx: &egui::Context, id: egui::Id, anchor: egui::Rect, default_height: f32) -> Option<egui::Pos2> {
    let host = current_panel_host(ctx).filter(|_| super::enabled(app))?;
    let region = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("shell-tools-region"))).unwrap_or_else(|| ctx.content_rect());
    let size = ctx.memory(|m| m.area_rect(id)).map_or(vec2(240.0, default_height), |r| r.size());
    Some(flyout_position(host, anchor, size, region))
}

/// Prefer the side facing the workspace; keep the popup beside its live host after moves.
fn flyout_position(host: egui::Rect, anchor: egui::Rect, size: egui::Vec2, region: egui::Rect) -> egui::Pos2 {
    let gap = 6.0;
    let right = host.right() + gap;
    let preferred = pos2(if right + size.x <= region.right() { right } else { host.left() - size.x - gap }, anchor.top());
    pos2(
        preferred.x.clamp(region.left(), (region.right() - size.x).max(region.left())),
        preferred.y.clamp(region.top(), (region.bottom() - size.y).max(region.top())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Pos2;
    use vectorcraft_engine::Session;

    #[test]
    fn hosted_paint_chips_are_small_adjacent_squares_at_both_toolbar_widths() {
        for (width, size) in [(44.0, 13.0), (80.0, metrics::TOOL_PAINT_HIT)] {
            let mut app = VectorcraftApp::new(Session::new(), Default::default());
            app.ui.varos_shell.enabled = true;
            app.ui.varos_panel_review = Some(Default::default());
            let ctx = egui::Context::default();
            let rect = egui::Rect::from_min_size(pos2(20.0, 20.0), vec2(width, 100.0));
            for _ in 0..3 {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                        ui.set_width(width);
                        crate::toolbar::shell_chips(&mut app, ui);
                    });
                });
                out.textures_delta.clear();
            }
            let mut chips = crate::toolbar::tests::widget_rects(&ctx, egui::Vec2::splat(size));
            chips.sort_by(|a, b| a.left().total_cmp(&b.left()));
            assert_eq!(chips.len(), 3);
            assert!(chips.iter().all(|chip| rect.contains_rect(*chip)));
            for pair in chips.windows(2) {
                assert_eq!(pair[0].top(), pair[1].top(), "the paint choices never stack");
                assert!((pair[1].left() - pair[0].right() - metrics::TOOL_PAINT_GAP).abs() < 0.1);
            }
        }
    }

    #[test]
    fn hosted_tools_flyout_opens_inward_at_the_right_and_bottom_edges() {
        let region = egui::Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0));
        let size = vec2(240.0, 300.0);
        let right_host = egui::Rect::from_min_size(pos2(920.0, 100.0), vec2(80.0, 600.0));
        let anchor = egui::Rect::from_min_size(pos2(940.0, 650.0), vec2(28.0, 28.0));
        let popup = egui::Rect::from_min_size(flyout_position(right_host, anchor, size, region), size);
        assert!(region.contains_rect(popup));
        assert!(popup.right() < right_host.left());
        let bottom_host = egui::Rect::from_min_size(pos2(0.0, 700.0), vec2(80.0, 100.0));
        let anchor = egui::Rect::from_min_size(pos2(20.0, 750.0), vec2(28.0, 28.0));
        let popup = egui::Rect::from_min_size(flyout_position(bottom_host, anchor, size, region), size);
        assert!(region.contains_rect(popup));
        assert!(popup.left() > bottom_host.right());
        assert!(popup.top() < anchor.top(), "a low tool's flyout stays inside the workspace");
    }
}
