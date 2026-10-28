//! The Home icon shows the Home screen over open documents; Cmd+N opens the New Document dialog.

use serde_json::json;

use crate::{VectorcraftApp, canvas};

fn app_with_doc() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({})).unwrap();
    app
}

/// Draw one canvas frame; whether it drew the document (else the Home screen).
fn shows_document(app: &mut VectorcraftApp) -> bool {
    app.canvas_rect = None;
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| canvas::show(app, ui));
    out.textures_delta.clear();
    app.canvas_rect.is_some()
}

#[test]
fn home_shows_over_open_documents_until_a_document_is_chosen() {
    let mut app = app_with_doc();
    app.ui.control_bar = true; // Varos: exercise every optional editor bar.
    assert!(shows_document(&mut app));
    crate::varos::home::tests::assert_chrome(&mut app, true); // Varos: full window chrome.
    app.run("app.home", json!({})).unwrap();
    assert!(!shows_document(&mut app), "Home replaces the canvas");
    crate::varos::home::tests::assert_chrome(&mut app, false); // Varos: welcome only.
    assert_eq!(app.session.documents().len(), 1, "the document stays open");
    assert!(app.ui.dialog.is_none(), "Home is not the New Document dialog");
    // Choosing the document's tab returns to it.
    app.ui.home = None;
    assert!(shows_document(&mut app));
    // A new document (from Home's presets, New… or Open) replaces Home too.
    app.run("app.home", json!({})).unwrap();
    app.run("file.new", json!({})).unwrap();
    assert!(shows_document(&mut app));
    assert!(app.ui.home.is_none());
    crate::varos::home::tests::assert_chrome(&mut app, true); // Varos: restored after New.
}

/// Draw one canvas frame; whether it drew any text (the Home screen's labels; an empty window
/// draws none).
fn draws_text(app: &mut VectorcraftApp) -> bool {
    fn has_text(s: &egui::Shape) -> bool {
        match s {
            egui::Shape::Text(_) => true,
            egui::Shape::Vec(v) => v.iter().any(has_text),
            _ => false,
        }
    }
    app.canvas_rect = None;
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| canvas::show(app, ui));
    out.textures_delta.clear();
    out.shapes.iter().any(|c| has_text(&c.shape))
}

/// Varos: no-document welcome stays clean even when an older saved preference disabled it.
#[test]
fn home_screen_without_documents_always_shows_clean_welcome() {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
    assert!(app.session.active().is_none());
    assert!(draws_text(&mut app), "Home with no document open (the default)");
    assert!(crate::menus::home_showing(&app));
    app.session.execute("prefs.set", &json!({"key": "showHomeScreen", "value": false})).unwrap();
    assert!(draws_text(&mut app), "Varos: saved false still shows welcome");
    crate::varos::home::tests::assert_chrome(&mut app, false);
    assert!(!shows_document(&mut app));
    assert!(crate::menus::home_showing(&app), "Varos: no document is always Home");
    app.run("app.home", json!({})).unwrap();
    assert!(draws_text(&mut app), "the Home button still shows the Home screen");
    app.run("file.new", json!({})).unwrap();
    assert!(shows_document(&mut app));
    assert!(app.ui.home.is_none());
    app.run("file.close", json!({})).unwrap();
    assert!(app.session.active().is_none());
    assert!(draws_text(&mut app), "Varos: closing the last document returns Home");
    app.session.execute("prefs.set", &json!({"key": "showHomeScreen", "value": true})).unwrap();
    assert!(draws_text(&mut app), "on again: Home");
}

#[test]
fn cmd_n_opens_the_new_document_dialog() {
    assert_eq!(crate::shortcut_editor::command_for_key("Cmd+N"), Some("file.newDialog"));
    let mut app = app_with_doc();
    app.run("file.newDialog", json!({})).unwrap();
    assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("newDocument"));
    assert_eq!(app.session.documents().len(), 1, "no document is made until the dialog's OK");
}

/// Cmd+Alt+N is New Without Dialog by default: Ctrl+Alt+N on Windows and Linux, Option+Command+N
/// on macOS. No other command, tool or panel has that key by default.
#[test]
fn cmd_alt_n_is_new_without_dialog() {
    assert_eq!(crate::shortcut_editor::default_command_shortcut("file.newWithoutDialog"), Some("Cmd+Alt+N"));
    assert_eq!(crate::shortcut_editor::command_for_key("Cmd+Alt+N"), Some("file.newWithoutDialog"));
    let chord = crate::shortcuts::parse("Cmd+Alt+N").expect("parses");
    assert_eq!((chord.modifiers, chord.logical_key), (egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::N));
    let shown = crate::menus::pretty_shortcut("Cmd+Alt+N");
    assert_eq!(shown, if cfg!(target_os = "macos") { "⌥⌘N" } else { "Ctrl+Alt+N" });
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
    let listed = app.run("shortcuts.list", json!({"query": "file.newWithoutDialog"})).unwrap();
    let rows: Vec<_> = listed.as_array().expect("rows").iter().map(|r| (r["label"].clone(), r["group"].clone(), r["default"].clone())).collect();
    assert_eq!(rows, [(json!("New Without Dialog"), json!("File"), json!("Cmd+Alt+N"))], "Keyboard Shortcuts lists it under File");
    assert_eq!(app.run("shortcuts.conflicts", json!({})).unwrap(), json!([]), "no other default has the key");
}

/// Cmd+Alt+N, pressed through the shortcut handler, makes a document with the settings New
/// Document opens on (the Print category's Letter while Recent is empty, then the first Recent
/// entry), named as the dialog names it, and does not open the dialog.
#[test]
fn cmd_alt_n_makes_a_document_without_the_new_document_dialog() {
    use vectorcraft_doc::ColorMode;
    fn press(app: &mut VectorcraftApp) {
        let ctx = egui::Context::default();
        let modifiers = egui::Modifiers::COMMAND | egui::Modifiers::ALT;
        let ev = egui::Event::Key { key: egui::Key::N, physical_key: None, pressed: true, repeat: false, modifiers };
        let mut out = ctx.run_ui(egui::RawInput { events: vec![ev], ..Default::default() }, |ui| crate::shortcuts::handle(app, ui.ctx()));
        out.textures_delta.clear();
    }
    let made = |app: &VectorcraftApp| {
        let d = &app.session.active().expect("a document").doc;
        let r = d.artboards[0].rect;
        (d.title.clone(), r.width(), r.height(), d.color_mode, d.raster_effects_ppi)
    };
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
    assert!(app.session.active().is_none());
    let name = app.session.peek_untitled();
    press(&mut app);
    assert_eq!(app.session.documents().len(), 1);
    assert!(app.ui.dialog.is_none(), "the New Document dialog does not open");
    assert_eq!(made(&app), (name, 612.0, 792.0, ColorMode::Cmyk, 300.0), "Letter from the Print category, as New Document opens on");
    // Then the first Recent entry, the 400 × 300 pt document's settings.
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    let name = app.session.peek_untitled();
    press(&mut app);
    assert_eq!(app.session.documents().len(), 3);
    assert!(app.ui.dialog.is_none());
    assert_eq!(made(&app), (name, 400.0, 300.0, ColorMode::Rgb, 72.0));
    // Run as a command while a dialog is open, it leaves that dialog open.
    app.run("file.newDialog", json!({})).unwrap();
    app.run("file.newWithoutDialog", json!({})).unwrap();
    assert_eq!(app.session.documents().len(), 4);
    assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("newDocument"));
}

/// Home › Recent Files and File › Open Recent focus an open tab instead of opening a second copy
/// (#3455); `document.open` is unchanged.
#[test]
fn home_recent_reopen_focuses_an_already_open_document() {
    let services = crate::Services {
        read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))),
        write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))),
        ..Default::default()
    };
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), services);
    let d = std::env::temp_dir().join(format!("vc-home-reopen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let path = d.join("doc.vectorcraft").to_string_lossy().to_string();
    app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    app.run("shape.rectangle", json!({"x": 1, "y": 2, "width": 10, "height": 10})).unwrap();
    app.run("file.saveAs", json!({"path": path})).unwrap();
    app.run("file.new", json!({})).unwrap();
    assert_eq!(app.session.documents().len(), 2);
    assert_eq!(app.session.active_index(), Some(1));
    let r = crate::io::open_recent_reporting(&mut app, &path).unwrap();
    assert_eq!(r["activated"], true);
    assert_eq!(app.session.documents().len(), 2);
    assert_eq!(app.session.active_index(), Some(0));
    assert_eq!(app.session.doc().unwrap().doc.layers[0].children().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(d);
}
