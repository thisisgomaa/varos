//! The Control bar as a panel-workspace host presents it: one horizontally scrolling row of the
//! classic contents (`chrome::control_contents`) and four pinned quick-access buttons.
use crate::theme::Tokens;
use crate::{VectorcraftApp, icons};
use egui::{Sense, Stroke, Ui, vec2};
use serde_json::json;

/// The classic Control contents switch to their compact, single-row form while hosted.
pub(crate) fn hosted(app: &VectorcraftApp) -> bool {
    super::enabled(app)
}

/// The hosted row. The host owns horizontal scrolling: keep a finite viewport width for the
/// context-sensitive inline fields, while allowing this one row to overflow.
pub(crate) fn row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let viewport_width = ui.clip_rect().width().max(1.0);
    ui.horizontal(|ui| {
        ui.set_max_width(viewport_width);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().button_padding = vec2(6.0, 3.0);
        crate::chrome::control_contents(app, ui);
    });
}

/// Four fixed-width controls remain reachable while the contextual fields scroll.
pub(crate) const QUICK_ACCESS_WIDTH: f32 = 112.0;

pub(crate) fn quick_access(app: &mut VectorcraftApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if quick_button(ui, "grid", tl!("Snap to Grid"), app.ui.view.snap_to_grid).clicked() {
            crate::menus::invoke(app, "view.snapToGrid", json!({}));
        }
        let pixel = quick_button(ui, "pixel", tl!("Snap to Pixel"), app.ui.view.snap_to_pixel);
        if pixel.clicked() {
            crate::menus::invoke(app, "view.snapToPixel", json!({}));
        }
        if app.ui.view.snap_to_grid {
            pixel.on_hover_text(tl!("Snap to Grid takes priority over Snap to Pixel"));
        }
        // This opens options; it is deliberately not an invented master snapping switch.
        let snapping = quick_button(ui, "snapping", tl!("Snapping"), false);
        egui::Popup::menu(&snapping).show(|ui| crate::chrome::snapping_options(app, ui));
        let menu = quick_button(ui, "menu", tl!("Control options"), false);
        egui::Popup::menu(&menu).show(|ui| {
            crate::chrome::snapping_options(app, ui);
            ui.separator();
            if ui.button(tl!("Preferences")).clicked() {
                crate::menus::invoke(app, "edit.preferences", json!({}));
                ui.close();
            }
        });
    });
}

fn quick_button(ui: &mut Ui, key: &str, label: &str, selected: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
    let response = ui.interact(rect, ui.make_persistent_id(("control-quick", key)), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(rect.shrink(2.0), 3.0, if selected { t.tool_active } else { t.hover });
    }
    let color = if selected { t.text_strong } else { t.icon };
    let glyph = rect.shrink(6.0);
    match key {
        "grid" => icons::paint(ui, "layout-grid", glyph, color),
        "menu" => icons::paint(ui, "menu", glyph, color),
        "pixel" => {
            // Original pixel-grid symbol: a selected pixel surrounded by eight cells.
            for row in 0..3 {
                for col in 0..3 {
                    let cell = egui::Rect::from_min_size(glyph.min + vec2(1.0 + col as f32 * 5.0, 1.0 + row as f32 * 5.0), vec2(3.0, 3.0));
                    if row == 1 && col == 1 {
                        ui.painter().rect_filled(cell, 0.0, color);
                    } else {
                        ui.painter().rect_stroke(cell, 0.0, Stroke::new(0.8, color), egui::StrokeKind::Inside);
                    }
                }
            }
        }
        "snapping" => {
            // Original U-shaped magnet with a small disclosure arrow, drawn in theme tint.
            let at = |x, y| glyph.min + vec2(x, y);
            ui.painter().add(egui::Shape::line(
                vec![at(1.5, 2.0), at(1.5, 10.0), at(3.0, 13.0), at(5.5, 14.0), at(8.0, 13.0), at(9.5, 10.0), at(9.5, 2.0)],
                Stroke::new(2.0, color),
            ));
            ui.painter().line_segment([at(0.0, 5.0), at(3.0, 5.0)], Stroke::new(1.0, t.panel));
            ui.painter().line_segment([at(8.0, 5.0), at(11.0, 5.0)], Stroke::new(1.0, t.panel));
            ui.painter().add(egui::Shape::line(vec![at(12.0, 7.0), at(14.0, 9.0), at(16.0, 7.0)], Stroke::new(1.0, color)));
        }
        _ => {}
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label));
    response.on_hover_text(label)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{Harness, texts};
    use super::*;
    use egui::{Event, Modifiers, Pos2, Rect, pos2};
    use vectorcraft_engine::Session;

    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        fn labels(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(text) => out.push((text.galley.text().to_owned(), egui::Rect::from_min_size(text.pos, text.galley.size()))),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| labels(shape, out)),
                _ => {}
            }
        }
        let mut out = ctx.run_ui(
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(640.0, 600.0))), events, ..Default::default() },
            |ui| quick_access(app, ui),
        );
        let mut text = vec![];
        for shape in &out.shapes {
            labels(&shape.shape, &mut text);
        }
        out.textures_delta.clear();
        text
    }

    fn click(app: &mut VectorcraftApp, ctx: &egui::Context, pos: egui::Pos2) {
        frame(app, ctx, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(app, ctx, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }]);
        }
    }

    #[test]
    fn compact_control_quick_access_toggles_real_flags_and_exposes_glyph_options() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.ui.view.snap_to_grid = false;
        app.ui.view.snap_to_pixel = false;
        app.ui.view.snap_to_glyph = false;
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let mut buttons: Vec<egui::Rect> = ctx.viewport(|vp| {
            vp.prev_pass
                .widgets
                .layers()
                .flat_map(|(_, widgets)| widgets.iter())
                .filter(|w| w.rect.size() == vec2(28.0, 28.0))
                .map(|w| w.rect)
                .collect()
        });
        buttons.sort_by(|a, b| a.left().total_cmp(&b.left()));
        buttons.dedup();
        assert_eq!(buttons.len(), 4);
        assert_eq!(buttons[3].right() - buttons[0].left(), QUICK_ACCESS_WIDTH);
        click(&mut app, &ctx, buttons[0].center());
        assert!(app.ui.view.snap_to_grid);
        click(&mut app, &ctx, buttons[1].center());
        assert!(app.ui.view.snap_to_pixel);
        click(&mut app, &ctx, buttons[0].center());
        assert!(!app.ui.view.snap_to_grid);
        click(&mut app, &ctx, buttons[2].center());
        assert!(egui::Popup::is_any_open(&ctx));
        frame(&mut app, &ctx, vec![]);
        let labels = frame(&mut app, &ctx, vec![]);
        for expected in ["Smart Guides", "Snap to Point", "Snap to Glyph", "Alignment Guides", "Snap to Tangent", "Snap to Perpendicular"] {
            assert!(labels.iter().any(|(label, _)| label == expected), "missing {expected}: {labels:?}");
        }
        let glyph = labels.iter().find(|(label, _)| label == "Snap to Glyph").unwrap().1;
        click(&mut app, &ctx, glyph.center());
        assert!(app.ui.view.snap_to_glyph);
    }

    /// G2: hosted at a normal width, Control creates X/Y/W/H and the horizontal overflow reaches them.
    #[test]
    fn hosted_control_keeps_xywh_reachable_by_its_horizontal_overflow() {
        let mut h = Harness::sized(vec2(1000.0, 800.0));
        h.select_rectangle();
        h.settle();
        let control = h.layout().owner("control").unwrap().id;
        let host = h.state().hits.iter().find(|(name, _)| *name == format!("host:{control}")).unwrap().1;
        let row = pos2(host.left() + 200.0, host.center().y);
        let scroll = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(-5000.0, 0.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: egui::MouseWheelSource::Unknown,
        };
        h.frame(vec![Event::PointerMoved(row), scroll]);
        let output = h.settle();
        let painted = texts(&output);
        for label in ["X:", "Y:", "W:", "H:"] {
            let (_, rect, clip) =
                painted.iter().find(|(text, _, _)| text == label).unwrap_or_else(|| panic!("{label} exists in the hosted Control row"));
            assert!(host.contains_rect(*rect) && clip.contains_rect(*rect), "{label} is scrolled into the host: {rect:?} in {host:?}");
        }
    }

    /// G2: in a host narrower than the quick-access reserve, everything stays inside the host.
    #[test]
    fn a_narrow_control_host_scrolls_its_quick_access_instead_of_painting_past_it() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("ui.panelReview", json!({"enabled": true})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        // Narrower than the quick-access reserve: before the fix the row shrank to one point and the
        // pinned buttons ran past the host's right edge, where nothing could bring them back.
        let rect = Rect::from_min_size(pos2(20.0, 40.0), vec2(110.0, 40.0));
        let mut run = |events: Vec<Event>| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 400.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                assert!(super::super::content::show(&mut app, ui, "control", rect));
            });
            out.textures_delta.clear();
        };
        for _ in 0..3 {
            run(vec![]);
        }
        let buttons = || -> Vec<Rect> {
            let mut found: Vec<Rect> = ctx.viewport(|vp| {
                vp.prev_pass
                    .widgets
                    .layers()
                    .flat_map(|(_, widgets)| widgets.iter())
                    .filter(|w| w.rect.size() == vec2(28.0, 28.0))
                    .map(|w| w.rect)
                    .collect()
            });
            found.sort_by(|a, b| a.left().total_cmp(&b.left()));
            found.dedup();
            found
        };
        assert_eq!(buttons().len(), 4, "the quick-access buttons are drawn");
        assert!(buttons()[0].left() >= rect.left(), "the row starts inside the host");
        let scroll = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(-5000.0, 0.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: egui::MouseWheelSource::Unknown,
        };
        run(vec![Event::PointerMoved(rect.center()), scroll]);
        for _ in 0..3 {
            run(vec![Event::PointerMoved(rect.center())]);
        }
        let last = *buttons().last().unwrap();
        assert!(rect.contains_rect(last), "scrolling the row reaches the last quick-access button: {last:?} in {rect:?}");
    }
}
