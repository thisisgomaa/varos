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
    let document = app.session.active().unwrap().doc.clone();
    let active_key = crate::menus::home_key(&app);
    app.run("app.home", json!({})).unwrap();
    assert!(crate::varos::home::is_home(&app));
    assert_eq!(app.ui.home, Some(active_key));
    let r = app.run("app.home", json!({"action":"open", "path":path})).unwrap();
    assert_eq!(r["activated"], true);
    assert_eq!(crate::menus::home_key(&app), active_key, "reopening the active tab does not change document identity");
    assert!(!crate::varos::home::is_home(&app), "successful Home open returns to the already-active editor");
    assert!(app.ui.home.is_none());
    assert_eq!(app.session.documents().len(), 2);
    assert_eq!(app.session.active_index(), Some(0));
    assert!(std::sync::Arc::ptr_eq(&app.session.active().unwrap().doc, &document));
    assert!(shows_document(&mut app));
    let _ = std::fs::remove_dir_all(d);
}

/// Exercise the real whole-window route, not only the canvas/Home renderer.
fn panel_home_input(app: &mut VectorcraftApp, ctx: &egui::Context, time: &mut f64, events: Vec<egui::Event>) -> egui::FullOutput {
    *time += 0.1;
    let raw = egui::RawInput {
        time: Some(*time),
        events,
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 1000.0))),
        max_texture_side: Some(8192),
        focused: true,
        ..Default::default()
    };
    app.begin_host_iteration();
    let mut out = ctx.run_ui(raw, |ui| app.ui(ui));
    out.textures_delta.clear();
    assert!(!app.ui.status.starts_with("Internal error:"), "{}", app.ui.status);
    out
}

fn panel_home_frames(app: &mut VectorcraftApp, ctx: &egui::Context, time: &mut f64) -> Vec<String> {
    fn collect(shape: &egui::Shape, text: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => text.push(t.galley.text().to_string()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, text)),
            _ => {}
        }
    }
    let mut text = Vec::new();
    for _ in 0..5 {
        let out = panel_home_input(app, ctx, time, vec![]);
        text.clear();
        for shape in &out.shapes {
            collect(&shape.shape, &mut text);
        }
    }
    assert!(!app.ui.status.starts_with("Internal error:"), "{}", app.ui.status);
    text
}

#[test]
fn panel_workspace_preferences_keeps_canvas_and_shortcuts_live_but_text_entry_is_local() {
    use egui::{Event, Key, Modifiers, PointerButton, Rect, pos2, vec2};
    let pointer = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    let key = |key, pressed| Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Modifiers::NONE };
    let mut app = app_with_doc();
    let ctx = panel_home_context(&mut app);
    let mut time = 0.0;
    panel_home_frames(&mut app, &ctx, &mut time);
    app.select_tool("rectangle");
    app.run("edit.preferences", json!({"category": "Hyphenation"})).unwrap();
    panel_home_frames(&mut app, &ctx, &mut time);
    let preferences = ctx.memory(|m| m.area_rect(egui::Id::unique("dialog-preferences"))).expect("Preferences is drawn");
    let canvas = app.canvas_rect.expect("the document canvas remains drawn");
    let size = vec2(90.0, 60.0);
    let from = [canvas.left_top() + vec2(20.0, 20.0), pos2(canvas.left() + 20.0, canvas.bottom() - size.y - 20.0)]
        .into_iter()
        .find(|from| {
            let drag = Rect::from_min_size(*from, size);
            canvas.contains_rect(drag) && !preferences.expand(5.0).intersects(drag)
        })
        .expect("a drawing region remains outside Preferences");
    let to = from + size;
    panel_home_input(&mut app, &ctx, &mut time, vec![Event::PointerMoved(from)]);
    panel_home_input(&mut app, &ctx, &mut time, vec![pointer(from, true)]);
    for step in 1..=4 {
        panel_home_input(&mut app, &ctx, &mut time, vec![Event::PointerMoved(from + size * (step as f32 / 4.0))]);
    }
    panel_home_input(&mut app, &ctx, &mut time, vec![pointer(to, false)]);
    panel_home_frames(&mut app, &ctx, &mut time);
    assert_eq!(
        app.session.active().unwrap().doc.layers[0].children().unwrap().len(),
        1,
        "a real pointer drag draws behind the open Preferences window"
    );
    assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("preferences"));

    assert!(!app.session.tool_busy(), "the canvas drawing gesture has finished before the shortcut");
    assert!(!ctx.egui_wants_keyboard_input(), "canvas interaction leaves no text field focused: {:?}", ctx.memory(|m| m.focused()));
    // Plain letter shortcuts consume the accompanying text event, so keyboard layouts
    // keep their own letter mapping; a key-down without typed text is not a native P.
    panel_home_input(&mut app, &ctx, &mut time, vec![key(Key::P, true), Event::Text("p".into())]);
    panel_home_input(&mut app, &ctx, &mut time, vec![key(Key::P, false)]);
    assert_eq!(app.session.tool_id(), "pen", "an unfocused Preferences window does not swallow the editor shortcut");

    // Find the actual text input by its accessibility role inside Preferences, rather
    // than guessing coordinates or directly requesting focus on a synthetic widget.
    ctx.enable_accesskit();
    let out = panel_home_input(&mut app, &ctx, &mut time, vec![]);
    let tree = out.platform_output.accesskit_update.expect("the window accessibility tree");
    let fields: Vec<_> = ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|widget| {
                preferences.contains_rect(widget.rect)
                    && tree.nodes.iter().any(|(id, node)| *id == widget.id.accesskit_id() && node.role() == egui::accesskit::Role::TextInput)
            })
            .copied()
            .collect()
    });
    assert_eq!(fields.len(), 1, "Hyphenation has one Exceptions text field");
    let field = fields[0];
    let at = field.rect.center();
    panel_home_input(&mut app, &ctx, &mut time, vec![Event::PointerMoved(at), pointer(at, true)]);
    panel_home_input(&mut app, &ctx, &mut time, vec![pointer(at, false)]);
    assert_eq!(ctx.memory(|m| m.focused()), Some(field.id), "the pointer focused the real Preferences input");
    let document = app.session.active().unwrap().doc.clone();
    panel_home_input(&mut app, &ctx, &mut time, vec![key(Key::V, true), Event::Text("v".into())]);
    panel_home_input(&mut app, &ctx, &mut time, vec![key(Key::V, false)]);
    assert_eq!(app.ui.dialog.as_ref().unwrap().str("hyphenationExceptions"), "v", "text reaches the Preferences draft");
    assert_eq!(app.session.tool_id(), "pen", "typing V in Preferences does not select the canvas Selection tool");
    assert!(std::sync::Arc::ptr_eq(&app.session.active().unwrap().doc, &document), "typing in Preferences does not edit the artwork");
    assert!(app.ui.varos_panel_review.is_some());
}

fn panel_home_context(app: &mut VectorcraftApp) -> egui::Context {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    app.canvas.worker_started = true;
    app.custom_titlebar = true;
    app.ui.varos_shell.enabled = true;
    app.ui.varos_panel_review = Some(Default::default());
    app.ui.varos_panel_review.as_mut().unwrap().layout = crate::varos::panel_system::model::Layout::studio_workspace().unwrap();
    ctx
}

fn assert_panel_home_is_clean(app: &mut VectorcraftApp, ctx: &egui::Context, text: &[String]) {
    assert!(crate::varos::home::is_home(app));
    assert!(app.canvas_rect.is_none(), "Home cannot retain document canvas geometry");
    assert!(text.iter().any(|s| s == "Pick up where you left off, or start something new."));
    assert_eq!(text.iter().filter(|s| s.as_str() == "Varos").count(), 1, "Home paints its own single title bar");
    for label in ["New board", "Open…", "Recent"] {
        assert!(text.iter().any(|s| s == label), "Home keeps {label}");
    }
    for editor_label in ["Properties", "Layers", "Swatches", "Control", "No Selection", "Document Setup", "File"] {
        assert!(!text.iter().any(|s| s == editor_label), "Home must not paint editor content: {editor_label}");
    }
    let state = app.run("ui.panelReview.inspect", json!({})).unwrap();
    assert_eq!(state["hits"], json!([]), "no stale editor hit targets remain on Home");
    assert_eq!(state["dragging"], false);
    let title = ctx.read_response(egui::Id::unique("titlebar-drag")).unwrap();
    assert!(title.rect.left() <= 1.0 && title.rect.right() >= 1439.0, "Home title bar fills the window, not the dock-reduced canvas");
}

#[test]
fn panel_workspace_home_hides_editor_hosts_and_restores_the_unchanged_layout() {
    use crate::varos::panel_system::model::{Action, Mode, Source, Target};
    let mut app = app_with_doc();
    app.run("shape.rectangle", json!({"x":10,"y":20,"width":30,"height":40})).unwrap();
    app.run("file.new", json!({"width":300,"height":200})).unwrap();
    let ctx = panel_home_context(&mut app);
    let layout = &mut app.ui.varos_panel_review.as_mut().unwrap().layout;
    let swatches = layout.group_of("swatches").unwrap().id;
    layout.apply(&Action::Move { source: Source::Group(swatches), target: Target::Float { position: [200.0, 140.0] } }).unwrap();
    let floating = layout.owner("swatches").unwrap().id;
    layout.apply(&Action::SetMode { host: floating, mode: Mode::Full }).unwrap();
    let mut time = 0.0;
    panel_home_frames(&mut app, &ctx, &mut time);
    assert!(app.canvas_rect.is_some());
    assert!(!app.run("ui.panelReview.inspect", json!({})).unwrap()["hits"].as_array().unwrap().is_empty());
    let layout = app.ui.varos_panel_review.as_ref().unwrap().layout.clone();
    let store = app.ui.varos_panel_review.as_ref().unwrap().workspaces.clone();
    let documents: Vec<_> = app.session.documents().iter().map(|d| d.doc.clone()).collect();
    let active = app.session.active_index();
    app.ui.varos_picker_anchor = Some(egui::Rect::from_min_size(egui::pos2(20.0, 300.0), egui::vec2(24.0, 24.0)));
    app.run("app.home", json!({})).unwrap();
    let text = panel_home_frames(&mut app, &ctx, &mut time);
    assert_panel_home_is_clean(&mut app, &ctx, &text);
    assert!(app.ui.varos_picker_anchor.is_none(), "an editor picker cannot cover Home");
    assert_eq!(app.ui.varos_panel_review.as_ref().unwrap().layout, layout);
    assert_eq!(app.ui.varos_panel_review.as_ref().unwrap().workspaces, store);
    assert_eq!(app.session.active_index(), active);
    assert_eq!(app.session.documents().len(), documents.len());
    for (actual, expected) in app.session.documents().iter().zip(&documents) {
        assert!(std::sync::Arc::ptr_eq(&actual.doc, expected), "Home does not mutate or close open artwork");
    }
    // Home's explicit return command restores this exact editor and saved arrangement.
    app.run("app.home", json!({"action": "back"})).unwrap();
    let text = panel_home_frames(&mut app, &ctx, &mut time);
    assert!(app.canvas_rect.is_some());
    assert!(text.iter().any(|s| s == "Properties"));
    assert!(text.iter().any(|s| s == "Layers"));
    assert!(text.iter().any(|s| s == "Swatches"));
    assert_eq!(app.ui.varos_panel_review.as_ref().unwrap().layout, layout);
    assert_eq!(app.ui.varos_panel_review.as_ref().unwrap().workspaces, store);
    assert!(!app.run("ui.panelReview.inspect", json!({})).unwrap()["hits"].as_array().unwrap().is_empty());
}

#[test]
fn panel_workspace_home_without_documents_never_reserves_editor_docks() {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
    let ctx = panel_home_context(&mut app);
    let mut time = 0.0;
    let layout = app.ui.varos_panel_review.as_ref().unwrap().layout.clone();
    let text = panel_home_frames(&mut app, &ctx, &mut time);
    assert_panel_home_is_clean(&mut app, &ctx, &text);
    assert_eq!(app.ui.varos_panel_review.as_ref().unwrap().layout, layout);
    app.run("file.new", json!({})).unwrap();
    panel_home_frames(&mut app, &ctx, &mut time);
    assert!(app.canvas_rect.is_some());
    app.run("file.close", json!({})).unwrap();
    assert!(app.session.active().is_none());
    let text = panel_home_frames(&mut app, &ctx, &mut time);
    assert_panel_home_is_clean(&mut app, &ctx, &text);
    assert_eq!(app.ui.varos_panel_review.as_ref().unwrap().layout, layout);
}
