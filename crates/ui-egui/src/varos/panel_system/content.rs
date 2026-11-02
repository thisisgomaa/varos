//! Adapters for the existing Tools and Control contents. The generic host supplies their chrome
//! and owns grouping, hiding and gestures; these adapters only draw the working controls.
use super::metrics;
use crate::VectorcraftApp;
use egui::{Align, Id, Layout, Rect, Ui, UiBuilder, UiStackInfo};

/// The measured content height of a body whose descriptor `fits_content` (the side Color panel's
/// sliders/spectrum, independent of the popup's wheel preferences); `None` for the others.
pub(crate) fn fitted_height(ctx: &egui::Context, panel: &str) -> Option<f32> {
    if !super::registry::get(panel).is_some_and(|spec| spec.fits_content) {
        return None;
    }
    match crate::varos::shell::persist::instance_kind(panel) {
        "color" => Some(crate::varos::color_panel::content_height(ctx).unwrap_or(300.0)),
        _ => None,
    }
}

/// How a Properties editor arranges its sections this frame: their order, and whether they move
/// by their grips.
pub(crate) struct PropertiesSections {
    pub(crate) order: Vec<crate::panels::properties::SectionId>,
    pub(crate) movable: bool,
}

/// The presenter's Properties capability. Sections follow the user's saved custom order and show
/// their move grips, except in the panel workspace: it shows each context's canonical order
/// (`present`, as the editor lists them) without grips, and forgets any section move in progress
/// (the custom order stays saved for the legacy shell); section bodies and collapse state are
/// shared by both.
pub(crate) fn properties_sections(
    app: &VectorcraftApp,
    ctx: &egui::Context,
    view: &crate::panels::properties::PropertiesView,
    present: &[crate::panels::properties::SectionId],
) -> PropertiesSections {
    let movable = !super::enabled(app);
    if !movable {
        crate::panels::properties::forget_section_drag(ctx);
    }
    PropertiesSections { order: if movable { view.ordered(present) } else { present.to_vec() }, movable }
}

/// Draw special panel content inside the host's body rectangle. `false` uses the ordinary panel
/// body path. Call before that fallback, not inside its additional scrolling wrapper.
pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut Ui, panel: &str, rect: Rect) -> bool {
    let kind = crate::varos::shell::persist::instance_kind(panel);
    if !matches!(kind, "tools" | "control") {
        return false;
    }
    if !rect.is_positive() || !rect.is_finite() {
        return true;
    }
    ui.scope_builder(
        UiBuilder::new()
            .scope_id(Id::unique(("panel-system-special-content", panel)))
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min))
            .ui_stack_info(UiStackInfo::default().with_tag("shell-panel")),
        |ui| {
            ui.set_clip_rect(rect.intersect(ui.clip_rect()));
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            let style = ui.style_mut();
            style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(metrics::FONT));
            style.text_styles.insert(egui::TextStyle::Button, egui::FontId::new(11.0, egui::FontFamily::Name("ui-medium".into())));
            match kind {
                "tools" => super::tools_host::content(app, ui),
                "control" => {
                    let height = egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(4, 3))
                        .show(ui, |ui| {
                            // The quick-access buttons stay pinned at the right while the host
                            // leaves the scrolling row a usable width; a narrower host scrolls
                            // them with the row, so nothing paints past the host.
                            let reserve = super::control_bar::QUICK_ACCESS_WIDTH + 8.0;
                            let pinned = ui.available_width() >= reserve + MIN_CONTROL_ROW;
                            let width = if pinned { ui.available_width() - reserve } else { ui.available_width() }.max(1.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let row = ui.allocate_ui_with_layout(egui::vec2(width, 28.0), Layout::top_down(Align::Min), |ui| {
                                    egui::ScrollArea::horizontal()
                                        .id_salt("hosted-control-scroll")
                                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                                        .auto_shrink([false, true])
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                super::control_bar::row(app, ui);
                                                if !pinned {
                                                    ui.add_space(8.0);
                                                    super::control_bar::quick_access(app, ui);
                                                }
                                            });
                                        })
                                        .content_size
                                        .y
                                });
                                if pinned {
                                    ui.add_space(8.0);
                                    super::control_bar::quick_access(app, ui);
                                }
                                row.inner.max(28.0)
                            })
                            .inner
                        })
                        .inner
                        + 6.0;
                    if height.is_finite() && height > 0.0 {
                        ui.ctx().data_mut(|d| d.insert_temp(control_height_id(panel), height.ceil()));
                    }
                }
                _ => {}
            }
        },
    );
    true
}

/// The narrowest scrolling Control row that still keeps the quick-access buttons pinned beside it.
const MIN_CONTROL_ROW: f32 = 96.0;

fn control_height_id(panel: &str) -> Id {
    Id::unique(("panel-system-control-content-height", panel))
}

/// Single-row Control contents plus its six points of frame padding, excluding host chrome.
/// Returns the last valid measurement for this exact panel instance, including after a frame.
/// Layout fitting must query only Controls whose full body was drawn in the current frame.
pub(crate) fn control_height(ctx: &egui::Context, panel: &str) -> Option<f32> {
    ctx.data(|d| d.get_temp::<f32>(control_height_id(panel))).filter(|height| height.is_finite() && *height > 0.0)
}

/// Popups must be drawn once after all hosts, using the current Tools body's anchor geometry.
pub(crate) fn popovers(app: &mut VectorcraftApp, ctx: &egui::Context, workspace: Rect) {
    super::tools_host::popovers(app, ctx, workspace);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Pos2, vec2};
    use serde_json::json;
    use vectorcraft_engine::Session;

    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.ui.varos_shell.enabled = true;
        app.ui.varos_panel_review = Some(Default::default());
        app
    }

    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, rect: Rect, time: f64, events: Vec<Event>) {
        let workspace = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 1000.0));
        let input = egui::RawInput { screen_rect: Some(workspace), time: Some(time), events, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            assert!(show(app, ui, "tools", rect));
            popovers(app, ui.ctx(), workspace);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn tools_keep_one_or_two_vertical_columns_even_in_a_wide_host() {
        for (width, columns) in [(44.0, 1), (80.0, 2), (1200.0, 2)] {
            let mut app = app();
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let rect = Rect::from_min_size(egui::pos2(30.0, 50.0), vec2(width, 240.0));
            for n in 0..3 {
                frame(&mut app, &ctx, rect, n as f64, vec![]);
            }
            let mut tools = crate::toolbar::tests::widget_rects(&ctx, egui::Vec2::splat(metrics::TOOL_BUTTON));
            let slots = crate::toolbar::slots(&app).len();
            assert!(tools.len() >= slots, "the real tool buttons are drawn at width {width}");
            tools.truncate(slots);
            let mut positions: Vec<f32> = tools.iter().map(|rect| rect.left()).collect();
            positions.sort_by(f32::total_cmp);
            positions.dedup_by(|a, b| (*a - *b).abs() < 0.1);
            assert_eq!(positions.len(), columns, "Tools stays vertical at width {width}");
            let last_tool_bottom = tools.iter().map(Rect::bottom).fold(f32::NEG_INFINITY, f32::max);
            let edit = ctx.data(|d| d.get_temp::<Rect>(crate::edit_toolbar::button_id())).unwrap();
            assert!(edit.top() > last_tool_bottom, "Edit Toolbar remains below the tool columns at width {width}");
        }
    }

    #[test]
    fn a_short_vertical_tools_host_scrolls_to_its_bottom_controls() {
        for width in [44.0, 80.0, 1200.0] {
            let mut app = app();
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let rect = Rect::from_min_size(egui::pos2(30.0, 50.0), vec2(width, 240.0));
            for n in 0..3 {
                frame(&mut app, &ctx, rect, n as f64, vec![]);
            }
            frame(&mut app, &ctx, rect, 3.0, crate::toolbar::tests::wheel(rect.center(), 4000.0));
            for n in 4..12 {
                frame(&mut app, &ctx, rect, n as f64, vec![]);
            }
            let edit = ctx.data(|d| d.get_temp::<Rect>(crate::edit_toolbar::button_id())).unwrap();
            assert!(rect.contains_rect(edit), "the short host at width {width} scrolls all the way to Edit Toolbar: {edit:?}");
            let anchor = ctx.data(|d| d.get_temp::<Rect>(crate::edit_toolbar::toolbar_id())).unwrap();
            assert_eq!(anchor, rect, "the drawer follows the generic host, not an inner scrolled row");
        }
    }

    #[test]
    fn control_height_stays_one_row_independent_of_width_and_host_minimum() {
        let mut app = app();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let workspace = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 1000.0));
        let mut measured = Vec::new();
        for height in [300.0, 80.0] {
            for _ in 0..3 {
                let input = egui::RawInput { screen_rect: Some(workspace), ..Default::default() };
                let mut out = ctx.run_ui(input, |ui| {
                    assert!(show(&mut app, ui, "control", Rect::from_min_size(egui::pos2(20.0, 40.0), vec2(1200.0, height))));
                    assert!(show(&mut app, ui, "control@2", Rect::from_min_size(egui::pos2(20.0, 400.0), vec2(300.0, 300.0))));
                });
                out.textures_delta.clear();
            }
            let wide = control_height(&ctx, "control").unwrap();
            let narrow = control_height(&ctx, "control@2").unwrap();
            assert!(wide < 80.0, "the wide Control reports its row, not the host height: {wide}");
            assert!((narrow - wide).abs() < 1.1, "narrow Control stays one row: {narrow} vs {wide}");
            assert!(control_height(&ctx, "control@3").is_none(), "instances do not share measurements");
            measured.push(wide);
        }
        assert_eq!(measured[0], measured[1], "changing only the host height cannot inflate intrinsic Control height");
    }

    #[test]
    fn panel_workspace_keeps_context_order_and_collapsing_without_section_grips() {
        use crate::panels::properties::{PropertiesView, SectionHits, SectionId, section_hits, show_editor};
        use egui::PointerButton;
        fn editor_frame(app: &mut VectorcraftApp, ctx: &egui::Context, view: &mut PropertiesView, events: Vec<Event>) -> SectionHits {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 900.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| show_editor(app, ui, "properties", view));
            out.textures_delta.clear();
            section_hits(ctx, "properties")
        }

        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.ui.varos_shell.enabled = true;
        app.ui.varos_panel_review = Some(Default::default());
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut view = PropertiesView { collapsed: SectionId::ALL.into_iter().collect(), ..Default::default() };
        view.order.reverse();
        let saved_order = view.order.clone();
        editor_frame(&mut app, &ctx, &mut view, vec![]);
        let hits = editor_frame(&mut app, &ctx, &mut view, vec![]);
        assert_eq!(
            hits.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
            [
                SectionId::Document,
                SectionId::Appearance,
                SectionId::RulersGrids,
                SectionId::Guides,
                SectionId::Snap,
                SectionId::Preferences,
                SectionId::QuickActions
            ]
        );
        assert!(hits.sections.iter().all(|s| !s.grip.is_positive()), "fixed sections expose no drag handle");
        assert_eq!(view.order, saved_order, "the legacy custom order is preserved");

        let at = hits.sections.first().unwrap().header.center();
        let press = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
        editor_frame(&mut app, &ctx, &mut view, vec![Event::PointerMoved(at), press(true)]);
        editor_frame(&mut app, &ctx, &mut view, vec![press(false)]);
        assert!(!view.collapsed.contains(&SectionId::Document), "the fixed header still expands its section");

        app.run("shape.rectangle", json!({"x": 0, "y": 0, "width": 40, "height": 40})).unwrap();
        let hits = editor_frame(&mut app, &ctx, &mut view, vec![]);
        assert_eq!(
            hits.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
            [SectionId::Transform, SectionId::Appearance, SectionId::Align, SectionId::QuickActions]
        );
        assert_eq!(view.order, saved_order);

        app.ui.varos_panel_review = None;
        let hits = editor_frame(&mut app, &ctx, &mut view, vec![]);
        assert_eq!(hits.sections.first().unwrap().id, SectionId::QuickActions, "the legacy shell retains its custom order");
        assert!(hits.sections.iter().all(|s| s.grip.is_positive()));
    }
}
