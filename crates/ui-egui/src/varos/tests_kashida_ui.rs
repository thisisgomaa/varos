//! Varos: the Paragraph panel's kashida rows ([`super`]), driven headless.

use serde_json::json;
use vectorcraft_doc::NodeKind;
use vectorcraft_doc::text::{KashidaMode, KashidaOptions};
use vectorcraft_engine::Session;

use super::{KashidaRects, controls};
use crate::VectorcraftApp;

fn app_with(text: &str) -> (VectorcraftApp, u64) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
    let id = app.run("text.create", json!({"x": 20, "y": 50, "text": text, "area": {"width": 300, "height": 200}, "justify": "justifyAll"})).unwrap()
        ["id"]
        .as_u64()
        .unwrap();
    app.session.execute("select.set", &json!({"ids": [id]})).unwrap();
    (app, id)
}

fn kashida(app: &VectorcraftApp, id: u64) -> KashidaOptions {
    match &app.session.active().unwrap().doc.node(vectorcraft_doc::NodeId(id)).unwrap().kind {
        NodeKind::Text(t) => t.para_at(0).kashida,
        _ => panic!("text"),
    }
}

fn steps(app: &VectorcraftApp) -> usize {
    app.session.active().unwrap().history.undo.len()
}

fn input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput { events, screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 400.0))), ..Default::default() }
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// Clicking Force, then dragging the strength slider, changes the paragraph's kashida, each as
/// one undo step; undo puts each back.
#[test]
fn force_and_a_strength_drag_change_the_paragraph_and_undo_restores_them() {
    let (mut app, id) = app_with("بسم الله الرحمن الرحيم الحمد لله رب العالمين");
    assert_eq!(kashida(&app, id), KashidaOptions::default());
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let frame = |app: &mut VectorcraftApp, events: Vec<egui::Event>| -> KashidaRects {
        let mut rects = None;
        let mut out = ctx.run_ui(input(events), |ui| {
            let para = crate::panels::character::text_style(app).unwrap().1;
            rects = Some(controls(app, ui, &para));
        });
        out.textures_delta.clear();
        rects.unwrap()
    };
    let r = frame(&mut app, vec![]);
    let before = steps(&app);
    // Force.
    let force = r.segments[2].center();
    frame(&mut app, vec![egui::Event::PointerMoved(force), button(force, true)]);
    frame(&mut app, vec![button(force, false)]);
    assert_eq!(kashida(&app, id), KashidaOptions { mode: KashidaMode::Force, strength: 50 });
    assert_eq!(steps(&app), before + 1, "one step");
    // Drag the strength from the middle of the rail toward its right end.
    let r = frame(&mut app, vec![]);
    assert!(r.slider.width() > 40.0 && r.field.left() >= r.slider.right(), "{:?} {:?}", r.slider, r.field);
    // The rail's bar, 5 pt in from its ends.
    let rail = r.slider.width() - 10.0;
    let (y, x0) = (r.slider.center().y, r.slider.left() + 5.0);
    let at = |share: f32| egui::pos2(x0 + rail * share, y);
    frame(&mut app, vec![egui::Event::PointerMoved(at(0.5)), button(at(0.5), true)]);
    for share in [0.6, 0.7, 0.8, 0.9] {
        frame(&mut app, vec![egui::Event::PointerMoved(at(share))]);
    }
    frame(&mut app, vec![button(at(0.9), false)]);
    frame(&mut app, vec![]);
    let dragged = kashida(&app, id);
    assert_eq!(dragged.mode, KashidaMode::Force);
    assert!(dragged.strength > 70, "{dragged:?}");
    assert_eq!(steps(&app), before + 2, "the whole drag is one step");
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(kashida(&app, id), KashidaOptions { mode: KashidaMode::Force, strength: 50 }, "one undo restores the strength");
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(kashida(&app, id), KashidaOptions::default());
}

/// The Paragraph panel shows the controls for a right-to-left paragraph only.
#[test]
fn the_paragraph_panel_shows_kashida_for_right_to_left_paragraphs() {
    let texts = |app: &mut VectorcraftApp| {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(input(vec![]), |ui| crate::panels::paragraph::show(app, ui));
        out.textures_delta.clear();
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let (mut arabic, _) = app_with("بسم الله الرحمن الرحيم");
    let shown = texts(&mut arabic);
    // Icon + label buttons; the strength field reads 50.
    for label in ["Off", "Auto", "Force", "50"] {
        assert!(shown.iter().any(|t| t == label), "{label} in {shown:?}");
    }
    let (mut latin, _) = app_with("Typography");
    assert!(!texts(&mut latin).iter().any(|t| t == "Force"), "no kashida controls for left-to-right type");
}

/// The command the controls use: values are checked, clamped and undoable.
#[test]
fn set_format_takes_the_kashida_settings() {
    let (mut app, id) = app_with("بسم الله");
    app.run("text.setFormat", json!({"kashida": "off"})).unwrap();
    assert_eq!(kashida(&app, id).mode, KashidaMode::Off);
    // No upper bound (owner 2026-10-19): justified lines read it as 100, Force elongates by it.
    app.run("text.setFormat", json!({"kashidaStrength": 250})).unwrap();
    assert_eq!((kashida(&app, id).strength, kashida(&app, id).strength()), (250, 100));
    app.run("text.setFormat", json!({"kashidaStrength": -5})).unwrap();
    assert_eq!(kashida(&app, id).strength, 0);
    app.run("text.setFormat", json!({"kashidaStrength": 12.6})).unwrap();
    assert_eq!(kashida(&app, id).strength, 13);
    assert!(app.run("text.setFormat", json!({"kashida": "maybe"})).is_err());
    assert!(app.run("text.setFormat", json!({"kashidaStrength": "x"})).is_err());
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(kashida(&app, id).strength, 0);
}

/// The slider's rail spans 0–100, and its number field takes more: typing 250 there sets 250.
#[test]
fn the_strength_field_takes_more_than_100() {
    let (mut app, id) = app_with("بسم الله الرحمن الرحيم");
    app.run("text.setFormat", json!({"kashida": "force"})).unwrap();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let frame = |app: &mut VectorcraftApp, events: Vec<egui::Event>| {
        let mut rects = None;
        let mut out = ctx.run_ui(input(events), |ui| {
            let para = crate::panels::character::text_style(app).unwrap().1;
            rects = Some(controls(app, ui, &para));
        });
        out.textures_delta.clear();
        rects.unwrap()
    };
    let r = frame(&mut app, vec![]);
    // The number field after the rail.
    let field = r.field.center();
    frame(&mut app, vec![egui::Event::PointerMoved(field)]);
    frame(&mut app, vec![button(field, true)]);
    frame(&mut app, vec![button(field, false)]);
    let key = |key, pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    // Select what the field holds and type over it.
    let all = egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
    frame(&mut app, vec![all, egui::Event::Text("250".into())]);
    frame(&mut app, vec![key(egui::Key::Enter, true), key(egui::Key::Enter, false)]);
    frame(&mut app, vec![]);
    assert_eq!(kashida(&app, id), KashidaOptions { mode: KashidaMode::Force, strength: 250 });
    // It shows 250, not 100.
    let mut out = ctx.run_ui(input(vec![]), |ui| {
        let para = crate::panels::character::text_style(&app).unwrap().1;
        controls(&mut app, ui, &para);
    });
    out.textures_delta.clear();
    let shown: Vec<String> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    assert!(shown.iter().any(|t| t == "250"), "{shown:?}");
}

fn app_with_two(first: &str, second: &str) -> (VectorcraftApp, [u64; 2]) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 800, "height": 600})).unwrap();
    let ids = [(first, 50), (second, 300)].map(|(text, y)| {
        app.run("text.create", json!({"x": 20, "y": y, "text": text, "area": {"width": 300, "height": 200}, "justify": "justifyAll"})).unwrap()["id"]
            .as_u64()
            .unwrap()
    });
    app.session.execute("select.set", &json!({"ids": ids})).unwrap();
    (app, ids)
}

/// Paragraphs that differ show as mixed, and a click sets every one of them, even to the mode the
/// first already has.
#[test]
fn a_mixed_selection_shows_mixed_and_a_click_sets_every_paragraph() {
    let (mut app, [a, b]) = app_with_two("بسم الله الرحمن الرحيم", "الحمد لله رب العالمين");
    app.session.execute("select.set", &json!({"ids": [b]})).unwrap();
    app.run("text.setFormat", json!({"kashida": "off", "kashidaStrength": 80})).unwrap();
    app.session.execute("select.set", &json!({"ids": [a, b]})).unwrap();
    assert_eq!((kashida(&app, a).mode, kashida(&app, b).mode), (KashidaMode::Auto, KashidaMode::Off));
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let frame = |app: &mut VectorcraftApp, events: Vec<egui::Event>| -> (KashidaRects, Vec<String>) {
        let mut rects = None;
        let mut out = ctx.run_ui(input(events), |ui| {
            let para = crate::panels::character::text_style(app).unwrap().1;
            rects = Some(controls(app, ui, &para));
        });
        out.textures_delta.clear();
        let texts = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        (rects.unwrap(), texts)
    };
    let (r, texts) = frame(&mut app, vec![]);
    assert!(r.mixed_mode && r.mixed_strength, "{r:?}");
    // Nothing lit; the modes the paragraphs have are tinted; the field reads "–".
    let get = |r: &KashidaRects, key: &str| r.controls.iter().find(|c| c.key == key).cloned().unwrap();
    for (key, tinted) in [("kashida-off", true), ("kashida-auto", true), ("kashida-force", false)] {
        let c = get(&r, key);
        assert!(!c.lit && c.tinted == tinted, "{c:?}");
    }
    assert!(texts.iter().any(|t| t == "–") && !texts.iter().any(|t| t == "50" || t == "80"), "{texts:?}");
    // Auto: the first paragraph has it already; the second takes it too.
    let auto = r.segments[1].center();
    frame(&mut app, vec![egui::Event::PointerMoved(auto), button(auto, true)]);
    frame(&mut app, vec![button(auto, false)]);
    assert_eq!((kashida(&app, a).mode, kashida(&app, b).mode), (KashidaMode::Auto, KashidaMode::Auto));
    let (r, _) = frame(&mut app, vec![]);
    assert!(!r.mixed_mode && r.mixed_strength, "modes agree now, strengths still differ");
    assert!(get(&r, "kashida-auto").lit && !get(&r, "kashida-off").tinted, "{:?}", r.controls);
    // One step undoes it.
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(kashida(&app, b).mode, KashidaMode::Off);
}

/// The controls show when any paragraph the panel acts on runs right to left, not only the first.
#[test]
fn the_controls_show_when_any_selected_paragraph_is_right_to_left() {
    let shows = |app: &mut VectorcraftApp| {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(input(vec![]), |ui| crate::panels::paragraph::show(app, ui));
        out.textures_delta.clear();
        out.shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == "Force"))
    };
    let (mut mixed, _) = app_with_two("Typography first", "بسم الله الرحمن الرحيم");
    assert!(shows(&mut mixed), "Latin first, Arabic second");
    let targets = super::targeted(&mixed);
    assert_eq!(targets.iter().map(|p| p.rtl).collect::<Vec<_>>(), [false, true]);
    let (mut latin, _) = app_with_two("Typography", "Kerning");
    assert!(!shows(&mut latin));
    // A text with a Latin paragraph and an Arabic one.
    let (mut paras, _) = app_with("Typography\nبسم الله الرحمن الرحيم");
    assert!(shows(&mut paras), "the second paragraph runs right to left");
}
