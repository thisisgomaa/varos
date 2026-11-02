//! The Tools contents as a panel-workspace host presents them: the real tool body, scrolled by
//! the host, which owns all moving, resizing and hiding; its compact Fill/Stroke pair and paint
//! chips; and the tool flyout placed beside it.
use super::{Hosting, metrics};
use crate::theme::Tokens;
use crate::{VectorcraftApp, icons, widgets};
use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_color::Paint;

/// Where the hosted Tools are this frame, for the flyout placed beside them: the workspace region
/// popovers stay inside, the first tool of the slot whose flyout is open, and that slot's button
/// (refreshed every frame, so the open flyout follows a moved host). One typed record in egui's
/// temporary memory, written and read only here.
#[derive(Clone, Debug, Default)]
struct Placement {
    region: Option<Rect>,
    flyout_tool: Option<String>,
    anchor: Option<Rect>,
}

fn placement(ctx: &egui::Context) -> Placement {
    ctx.data(|d| d.get_temp::<Placement>(egui::Id::NULL)).unwrap_or_default()
}

fn update_placement(ctx: &egui::Context, change: impl FnOnce(&mut Placement)) {
    ctx.data_mut(|d| change(d.get_temp_mut_or_default::<Placement>(egui::Id::NULL)));
}

/// Real tool contents for a generic panel host. Its host owns all moving, resizing and hiding.
/// Tools stay vertical in one or two columns; a short host scrolls the entire strip.
pub(crate) fn content(app: &mut VectorcraftApp, ui: &mut Ui) {
    let bounds = ui.max_rect();
    let t = Tokens::get(ui.ctx());
    // The tool body reports the open flyout's button through `note_slot`.
    let flyout_tool =
        app.ui.flyout.and_then(|_| ui.data(|d| d.get_temp::<Vec<String>>(egui::Id::unique("flyout-tools")))).and_then(|tools| tools.first().cloned());
    update_placement(ui.ctx(), |placement| {
        placement.flyout_tool = flyout_tool;
        placement.anchor = None;
    });
    widgets::panel_scroll(
        ui,
        egui::ScrollArea::vertical().id_salt("hosted-tools-scroll").max_height(bounds.height()).auto_shrink([false, false]),
        |ui| {
            let cols = if ui.available_width() >= 2.0 * metrics::TOOL_BUTTON { 2 } else { 1 };
            crate::toolbar::body_with_layout(app, ui, cols, None, Hosting::Workspace);
            ui.vertical_centered(|ui| crate::toolbar::bottom_controls(app, ui, &t, cols, Hosting::Workspace));
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
    update_placement(ctx, |placement| placement.region = Some(region));
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
    let placement = placement(ctx);
    let region = placement.region.unwrap_or_else(|| ctx.content_rect());
    let size = ctx.memory(|m| m.area_rect(id)).map_or(vec2(240.0, default_height), |r| r.size());
    Some(flyout_position(host, placement.anchor.unwrap_or(anchor), size, region))
}

/// The tool body's hook for each tool button: while hosted, the button of the slot whose flyout
/// is open becomes the flyout's anchor.
pub(crate) fn note_slot(ctx: &egui::Context, hosting: Hosting, slot: &[&str], button: Rect) {
    if hosting != Hosting::Workspace {
        return;
    }
    update_placement(ctx, |placement| {
        if placement.flyout_tool.as_deref().is_some_and(|tool| slot.first() == Some(&tool)) {
            placement.anchor = Some(button);
        }
    });
}

/// The hosted Fill/Stroke pair: two overlapping squares sized to the host's width (Fill at the
/// upper left, Stroke at the lower right), the swap arrow and the default-colours squares. The
/// focused well owns the overlap; each outer corner still selects its own paint.
pub(crate) fn wells(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens) {
    let size = (ui.available_width() - 2.0 * metrics::TOOL_PAINT_INSET).clamp(1.0, metrics::TOOL_PAINT_PROXY);
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let side = size * 2.0 / 3.0;
    let fill_rect = Rect::from_min_size(rect.min, vec2(side, side));
    let stroke_rect = Rect::from_min_size(rect.max - vec2(side, side), vec2(side, side));
    let swap_size = vec2(size * 0.30, size * 0.30);
    let swap = Rect::from_min_size(pos2(rect.right() - swap_size.x, rect.top()), swap_size);
    let defaults = Rect::from_min_size(pos2(rect.left(), rect.bottom() - size * 0.30), vec2(size * 0.30, size * 0.30));
    let open = app.ui.varos_picker_anchor.is_some();
    let outline = |active: bool| Stroke::new(1.0, if open && active { t.accent } else { t.button_border });
    let (fill_paint, stroke_paint) = crate::panels::current_paints(app);
    let mixed = crate::panels::mixed_paints(app, ui.ctx());
    let painter = ui.painter().clone();
    let draw_square = |r: Rect, paint: &Paint, mixed: bool, stroke: bool, active: bool| {
        painter.rect_filled(r, CornerRadius::ZERO, t.input_border);
        let chip = r.shrink(1.0);
        let hole = widgets::proxy_chip(ui, chip, paint, mixed, stroke.then_some(side * 0.25));
        painter.rect_stroke(r, CornerRadius::ZERO, outline(active), egui::StrokeKind::Inside);
        if stroke {
            painter.rect_stroke(chip, CornerRadius::ZERO, Stroke::new(1.0, t.text), egui::StrokeKind::Inside);
            painter.rect_stroke(hole, CornerRadius::ZERO, Stroke::new(1.0, t.text_dim), egui::StrokeKind::Outside);
        }
    };
    if app.session.fill_active {
        draw_square(stroke_rect, &stroke_paint, mixed.1, true, false);
        draw_square(fill_rect, &fill_paint, mixed.0, false, true);
    } else {
        draw_square(fill_rect, &fill_paint, mixed.0, false, false);
        draw_square(stroke_rect, &stroke_paint, mixed.1, true, true);
    }
    let square = size * 0.16;
    let fill = Rect::from_min_size(defaults.min + vec2(1.0, 1.0), vec2(square, square));
    let stroke = fill.translate(vec2(square * 0.55, square * 0.55));
    painter.rect_filled(stroke, CornerRadius::ZERO, Color32::BLACK);
    painter.rect_stroke(stroke, CornerRadius::ZERO, Stroke::new(1.0, t.text), egui::StrokeKind::Inside);
    painter.rect_filled(stroke.shrink(square * 0.30), CornerRadius::ZERO, t.panel);
    painter.rect_filled(fill, CornerRadius::ZERO, Color32::WHITE);
    painter.rect_stroke(fill, CornerRadius::ZERO, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    icons::paint(ui, "dc-swap", swap.shrink(size * 0.06), t.text);
    for stroke in [app.session.fill_active, !app.session.fill_active] {
        crate::varos::picker::register_well(ui.ctx(), if stroke { stroke_rect } else { fill_rect }, stroke);
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        if swap.contains(p) {
            app.run("paint.swap", json!({})).ok();
        } else if defaults.contains(p) {
            app.run("paint.default", json!({})).ok();
        } else {
            // The square in front owns the overlap; a well's outer corner stays its own.
            let front_fill = app.session.fill_active;
            let in_fill = fill_rect.contains(p);
            let in_stroke = stroke_rect.contains(p);
            let on_fill = in_fill && (front_fill || !in_stroke);
            let on_stroke = in_stroke && (!front_fill || !in_fill);
            if on_fill || on_stroke {
                let stroke = on_stroke;
                if stroke == app.session.fill_active {
                    app.run("paint.toggleActive", json!({})).ok();
                }
                crate::varos::picker::open_for(app, stroke, if stroke { stroke_rect } else { fill_rect });
            }
        }
    }
    ui.add_space(6.0);
}

/// The hosted Color, Gradient and None chips: three small adjacent squares in one row at either
/// Tools width.
pub(crate) fn chips(app: &mut VectorcraftApp, ui: &mut Ui) {
    let gap = metrics::TOOL_PAINT_GAP;
    let chip = ((ui.available_width() - 2.0 * gap) / 3.0).floor().clamp(1.0, metrics::TOOL_PAINT_HIT);
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - 3.0 * chip - 2.0 * gap) / 2.0).max(0.0));
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        for (tip, cmd) in [("Color (,)", "paint.lastColor"), ("Gradient (.)", "paint.lastGradient"), ("None (/)", "paint.none")] {
            let (r, resp) = ui.allocate_exact_size(vec2(chip, chip), Sense::click());
            let paint_rect = Rect::from_center_size(r.center(), egui::Vec2::splat(chip.min(metrics::TOOL_PAINT_CHIP)));
            match cmd {
                "paint.lastColor" => widgets::paint_chip(ui, paint_rect, &Paint::solid(app.session.last_solid)),
                "paint.lastGradient" => widgets::gradient_chip(ui, paint_rect, &app.session.last_gradient),
                _ => widgets::paint_chip(ui, paint_rect, &Paint::None),
            }
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tl!(tip)));
            if resp.on_hover_text(tl!(tip)).clicked() {
                clicked = Some(cmd);
            }
        }
    });
    if let Some(cmd) = clicked {
        app.run(cmd, json!({})).ok();
    }
    ui.add_space(6.0);
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
                        chips(&mut app, ui);
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
    fn the_open_flyout_anchors_to_its_slot_button_only_while_hosted() {
        let ctx = egui::Context::default();
        update_placement(&ctx, |placement| placement.flyout_tool = Some("pen".into()));
        let button = Rect::from_min_size(pos2(10.0, 300.0), vec2(28.0, 28.0));
        note_slot(&ctx, Hosting::Own, &["pen"], button);
        assert_eq!(placement(&ctx).anchor, None, "only a hosted body reports its buttons");
        note_slot(&ctx, Hosting::Workspace, &["selection"], button);
        assert_eq!(placement(&ctx).anchor, None, "another slot is not the anchor");
        note_slot(&ctx, Hosting::Workspace, &["pen", "curvature"], button);
        assert_eq!(placement(&ctx).anchor, Some(button));
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
