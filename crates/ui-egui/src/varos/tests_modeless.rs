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

/// The dialog-layer frames for the Preferences window test: a 1280 × 900 window.
mod window {
    use egui::{Event, Pos2, Rect, Vec2, vec2};

    use crate::VectorcraftApp;

    pub(super) const SCREEN: Vec2 = vec2(1280.0, 900.0);

    pub(super) fn frame(ctx: &egui::Context, app: &mut VectorcraftApp, events: Vec<Event>) {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), events, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| crate::dialogs::show(app, ui.ctx()));
        out.textures_delta.clear();
    }

    /// Drag with the mouse from `from` by `by`, in steps, then let go.
    pub(super) fn drag(ctx: &egui::Context, app: &mut VectorcraftApp, from: Pos2, by: Vec2) {
        frame(ctx, app, vec![Event::PointerMoved(from), super::button(from, true)]);
        for i in 1..=6 {
            frame(ctx, app, vec![Event::PointerMoved(from + by * (i as f32 / 6.0))]);
        }
        frame(ctx, app, vec![super::button(from + by, false)]);
        frame(ctx, app, vec![]);
    }

    /// Frames until a window that just opened has measured and placed itself.
    pub(super) fn settle(ctx: &egui::Context, app: &mut VectorcraftApp) {
        for _ in 0..6 {
            frame(ctx, app, vec![]);
        }
    }

    pub(super) fn rect_of(ctx: &egui::Context, id: egui::Id) -> Rect {
        ctx.memory(|m| m.area_rect(id)).expect("the dialog window was shown")
    }
}

/// The modeless Preferences window uses egui's title bar rather than the modal dialogs' heading
/// band: it moves by it, stays on screen and reopens where it was left.
#[test]
fn preferences_native_title_bar_moves_on_screen_and_remembers_position() {
    use window::{SCREEN, drag, frame, rect_of, settle};
    let screen = egui::Rect::from_min_size(Pos2::ZERO, SCREEN);
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, Default::default());
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 400})).unwrap();
    app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 80, "height": 60})).unwrap();
    let id = egui::Id::unique("dialog-preferences");
    crate::prefs_dialog::open(&mut app, None);
    settle(&ctx, &mut app);
    let start = rect_of(&ctx, id);
    assert!(start.width() < 900.0, "category list must be one vertical column: {start:?}");
    assert!(screen.contains_rect(start.shrink(0.5)), "Preferences starts fully on screen: {start:?}");
    drag(&ctx, &mut app, start.center_top() + egui::vec2(0.0, 10.0), egui::vec2(-90.0, 50.0));
    let moved = rect_of(&ctx, id);
    let delta = moved.min - start.min;
    assert!(delta.x < -80.0 && delta.y > 40.0, "native title bar moved the window: {start:?} → {moved:?}");
    assert!(screen.contains_rect(moved.shrink(0.5)));
    drag(&ctx, &mut app, moved.center_top() + egui::vec2(0.0, 10.0), egui::vec2(5000.0, 5000.0));
    let edge = rect_of(&ctx, id);
    assert!(screen.contains_rect(edge.shrink(0.5)), "Preferences stays on screen: {edge:?}");
    assert!(edge.right() > moved.right() && edge.bottom() > moved.bottom());
    app.ui.dialog = None;
    frame(&ctx, &mut app, vec![]);
    crate::prefs_dialog::open(&mut app, None);
    settle(&ctx, &mut app);
    assert!((rect_of(&ctx, id).min - edge.min).length() < 1.5, "Preferences remembers its position");
}
