//! Modeless Preferences leaves the editor live: artwork still travels between document tabs
//! while it is open; a modal dialog still stops the transfer. (The picker's side is covered in
//! `picker/tests.rs`.)

use egui::{Event, PointerButton, Pos2};
use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_geom::Point;

use crate::{VectorcraftApp, canvas, chrome};

/// One real tab-strip and canvas frame.
fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, time: f64, events: Vec<Event>) -> Vec<egui::Rect> {
    app.sync_views();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 700.0))),
        time: Some(time),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(raw, |ui| {
        canvas::release_over_panel(app, ui.ctx());
        egui::CentralPanel::default().show(ui, |ui| {
            chrome::doc_tabs(app, ui);
            canvas::show(app, ui);
        });
    });
    out.textures_delta.clear();
    let mut titles: Vec<_> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().contains(" @ ") => Some(egui::Rect::from_min_size(t.pos, t.galley.size())),
            _ => None,
        })
        .collect();
    titles.sort_by(|a, b| a.left().total_cmp(&b.left()));
    titles
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// Drag the source rectangle onto the second tab, dwell, and drop it on that canvas, with
/// `dialog` open throughout. Returns whether the drop placed the artwork in the target (one new
/// undo step there) and whether the second tab became active.
fn drag_to_second_tab(dialog: impl FnOnce(&mut VectorcraftApp)) -> (VectorcraftApp, bool, bool) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 300, "name": "Logo"})).unwrap();
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap();
    app.run("file.new", json!({"width": 400, "height": 300, "name": "Flyer"})).unwrap();
    let target_undo = app.session.active().unwrap().history.undo.len();
    app.session.set_active(0);
    app.select_tool("selection");
    app.ui.task_bar = false;
    app.ui.view.rulers = false;
    dialog(&mut app);
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let tabs = frame(&mut app, &ctx, 0.0, vec![]);
    assert_eq!(tabs.len(), 2);
    let grab = canvas::Xf::new(app.canvas_rect.unwrap(), app.view().unwrap()).to_screen(Point::new(100.0, 100.0));
    frame(&mut app, &ctx, 0.05, vec![Event::PointerMoved(grab), button(grab, true)]);
    frame(&mut app, &ctx, 0.1, vec![Event::PointerMoved(grab + egui::vec2(20.0, 10.0))]);
    let tab = tabs[1].center();
    frame(&mut app, &ctx, 0.2, vec![Event::PointerMoved(tab)]);
    // The canvas hands the artwork to the tab strip on the frame after it leaves the canvas.
    frame(&mut app, &ctx, 0.25, vec![]);
    frame(&mut app, &ctx, 0.8, vec![]);
    let switched = app.session.active_index() == Some(1);
    let drop = canvas::Xf::new(app.canvas_rect.unwrap(), app.view().unwrap()).to_screen(Point::new(170.0, 120.0));
    frame(&mut app, &ctx, 0.9, vec![Event::PointerMoved(drop)]);
    frame(&mut app, &ctx, 1.0, vec![button(drop, false)]);
    let placed = app.session.documents().get(1).is_some_and(|d| d.history.undo.len() == target_undo + 1);
    (app, placed, switched)
}

#[test]
fn artwork_travels_between_tabs_while_modeless_preferences_is_open() {
    let (app, placed, switched) = drag_to_second_tab(|app| crate::prefs_dialog::open(app, None));
    assert!(switched, "holding the artwork over the tab activates it under modeless Preferences");
    assert!(placed, "the drop places the artwork in the target");
    assert!(app.ui.dialog.as_ref().is_some_and(|d| d.kind == "preferences"), "Preferences stays open");

    let (_, placed, switched) = drag_to_second_tab(|app| app.ui.dialog = Some(crate::state::Dialog::new("rectangle", json!({}))));
    assert!(!switched && !placed, "a modal dialog still stops the transfer");
}
