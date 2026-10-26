//! The Control bar and the fields after the egui 0.37 / craft-ui bump (owner report on v11,
//! 2026-10-26): no field takes the keyboard (text selected, azure ring) unless clicked, fields as
//! tall as the controls beside them with their text centred, the Snapping menu a link label.

use egui::{Pos2, Rect, pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

impl Harness {
    /// A themed window with a rectangle selected and the Control bar on.
    fn new() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.ui.control_bar = true;
        app.run("shape.rectangle", json!({"x": 40, "y": 40, "width": 120, "height": 80})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
        let mut h = Self { app, ctx, time: 0.0 };
        for _ in 0..4 {
            h.frame(vec![]);
        }
        h
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> Vec<egui::epaint::ClippedShape> {
        self.time += 0.05;
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
        let input = egui::RawInput { time: Some(self.time), events, screen_rect: Some(screen), ..Default::default() };
        let app = &mut self.app;
        app.begin_host_iteration();
        let mut out = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
        out.shapes
    }

    /// The widget with the keyboard focus, if any, and the rect it was drawn at.
    fn focused(&self) -> Option<Rect> {
        let id = self.ctx.memory(|m| m.focused())?;
        self.ctx.read_response(id).map(|r| r.rect)
    }
}

/// A dialog opened over the Control bar gives the keyboard to its own first field, never to the
/// Control bar's Opacity field drawn before it (it took the dialog's "focus the first field" flag:
/// its text came up selected in an azure ring, and the dialog's field had no focus).
#[test]
fn a_dialog_never_lends_its_focus_to_the_control_bar() {
    let mut h = Harness::new();
    assert_eq!(h.focused(), None, "nothing has the keyboard at rest");
    crate::menus::invoke(&mut h.app, "object.rotate", json!({}));
    for _ in 0..3 {
        h.frame(vec![]);
    }
    let bar_bottom = 70.0;
    let focused = h.focused().expect("the dialog's first field has the keyboard");
    assert!(focused.top() > bar_bottom, "the focus is in the dialog, not the Control bar: {focused:?}");
    // Closed with Escape in its measuring frame: no field anywhere takes the keyboard afterwards.
    h.frame(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    assert!(h.app.ui.dialog.is_none());
    crate::menus::invoke(&mut h.app, "object.rotate", json!({}));
    h.frame(vec![]);
    h.app.ui.dialog = None;
    for _ in 0..3 {
        h.frame(vec![]);
    }
    assert_eq!(h.focused(), None, "a dialog closed in its first frame leaves no field focused");
}

/// Single-line fields are as tall as the controls beside them (24 points with their frame, the
/// numeric boxes 26) with their text centred, as before egui 0.37 (whose text edits reach the
/// interact height and put their text at the top).
#[test]
fn single_line_fields_keep_their_height_and_centre_their_text() {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    let mut rects = (Rect::NOTHING, Rect::NOTHING);
    for frame in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput { time: Some(frame as f64), ..Default::default() }, |ui| {
            let framed = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 4)).show(ui, |ui| {
                let mut s = "100%".to_string();
                ui.add(egui::TextEdit::singleline(&mut s).frame(egui::Frame::NONE).desired_width(50.0))
            });
            rects = (framed.response.rect, framed.inner.rect);
        });
        out.textures_delta.clear();
    }
    let (outer, edit) = rects;
    assert!(outer.height() <= 25.0, "a framed field is as tall as its text and margins: {outer:?}");
    assert!(edit.height() < 20.0, "the text edit is as tall as its text: {edit:?}");
    let _ = pos2(0.0, 0.0);
}

fn text_at(shapes: &[egui::epaint::ClippedShape], text: &str) -> Option<Rect> {
    fn walk(s: &egui::Shape, text: &str, out: &mut Option<Rect>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, text, out)),
            egui::Shape::Text(t) if t.galley.text() == text => *out = Some(s.visual_bounding_rect()),
            _ => {}
        }
    }
    let mut out = None;
    shapes.iter().for_each(|c| walk(&c.shape, text, &mut out));
    out
}

/// The keys the owner uses with a selection (Tab and Shift+Tab twice, Escape, V, Cmd+Z, Cmd+A)
/// and clicks on the Control bar's link labels give no field the keyboard.
#[test]
fn keys_and_link_labels_give_no_field_the_keyboard() {
    let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
    for (name, ev) in [
        ("Tab", key(egui::Key::Tab, egui::Modifiers::NONE)),
        ("Shift+Tab", key(egui::Key::Tab, egui::Modifiers::SHIFT)),
        ("Escape", key(egui::Key::Escape, egui::Modifiers::NONE)),
        ("V", key(egui::Key::V, egui::Modifiers::NONE)),
        ("Cmd+Z", key(egui::Key::Z, egui::Modifiers::COMMAND)),
        ("Cmd+A", key(egui::Key::A, egui::Modifiers::COMMAND)),
    ] {
        let mut h = Harness::new();
        for round in 0..2 {
            h.frame(vec![ev.clone()]);
            h.frame(vec![]);
            h.frame(vec![]);
            assert_eq!(h.focused(), None, "{name} (press {round}) leaves no field focused");
        }
    }
    let mut h = Harness::new();
    let shapes = h.frame(vec![]);
    for label in ["Opacity:", "Style:"] {
        let at = text_at(&shapes, label).unwrap_or_else(|| panic!("{label} is drawn")).center();
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(at), button(true)]);
        h.frame(vec![button(false)]);
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(h.focused(), None, "a click on {label} opens its panel and focuses no field");
    }
}

/// The Snapping menu is a link label as "Opacity:" and "Style:" (no framed button), and still
/// opens its menu.
#[test]
fn the_snapping_menu_is_a_link_label() {
    let mut h = Harness::new();
    let shapes = h.frame(vec![]);
    let label = text_at(&shapes, "Snapping").expect("Snapping is drawn");
    let framed = shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            r.rect.contains_rect(label)
                && r.rect.width() < label.width() + 40.0
                && r.fill.a() > 0
                && r.fill != crate::theme::Tokens::get(&h.ctx).panel
        }
        _ => false,
    });
    assert!(!framed, "no button frame around Snapping");
    let at = label.center();
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    h.frame(vec![egui::Event::PointerMoved(at), button(true)]);
    h.frame(vec![button(false)]);
    let shapes = h.frame(vec![]);
    assert!(text_at(&shapes, "Snap to Grid").is_some(), "its menu opens");
}
