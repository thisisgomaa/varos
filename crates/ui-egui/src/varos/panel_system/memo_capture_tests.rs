//! Opt-in article captures: real editor, commands, actor history and system-font shaping.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::test_support::*;
use serde_json::Value;

fn run(app: &mut VectorcraftApp, command: &str, params: Value) -> Value {
    app.run(command, params).unwrap_or_else(|e| panic!("{command}: {e}"))
}

fn editor(panel: &str) -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.session.prefs.show_indic_options = true;
    run(&mut app, "file.new", json!({"width":1000,"height":700}));
    run(&mut app, "ui.panelReview", json!({"enabled":true}));
    let mut layout = Layout::new_workspace();
    for host in &mut layout.hosts {
        host.hidden = true;
    }
    for name in ["tools", "control", panel] {
        layout.apply(&Action::Show { panel: name.into() }).unwrap();
    }
    let host = layout.owner(panel).unwrap().id;
    layout.apply(&Action::Resize { host, position: [100.0, 100.0], width: 340.0, height: 660.0 }).unwrap();
    layout.apply(&Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Right } }).unwrap();
    app.ui.varos_panel_review = Some(State { layout, ..Default::default() });
    app
}

fn rect(app: &mut VectorcraftApp, x: f64, y: f64, w: f64, h: f64, color: &str) -> Value {
    let id = run(app, "shape.rectangle", json!({"x":x,"y":y,"width":w,"height":h}))["id"].clone();
    run(app, "paint.setFill", json!({"color":color}));
    run(app, "stroke.set", json!({"weight":0}));
    id
}

fn label(app: &mut VectorcraftApp, text: &str, x: f64, y: f64, size: f64, color: &str) {
    run(app, "text.create", json!({"text":text,"x":x,"y":y,"size":size,"font":"Helvetica","color":color}));
}

fn agent(app: &mut VectorcraftApp, name: &str, command: &str, params: Value) {
    let (request, _) = crate::control::ControlRequest::new("engine.execute", json!({"command":command,"params":params}));
    let request = request.with_actor(Some(json!({"session":name,"profile":name,"label":name})));
    // before/after are the actual control bridge hooks; before calls Session::set_actor.
    let pending = crate::varos::agents::before(app, &request);
    run(app, command, params);
    crate::varos::agents::after(app, pending);
}

fn capture(app: &mut VectorcraftApp, dir: &std::path::Path, name: &str) {
    app.ui.status.clear();
    // Normal workspace preference: omit the status/hint bars and their timing readouts.
    app.ui.status_bar = false;
    if let Some(view) = app.view_mut() {
        view.center = vectorcraft_geom::Point::new(500.0, 350.0);
        view.zoom = 1.0;
        view.fitted = true;
    }
    let ctx = egui::Context::default();
    let minute = vectorcraft_doc::metadata::now_unix().unwrap() / 60;
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("home-identity"), (minute, "friend".to_string(), 10_u32)));
    let pixels = super::ui_tests::software_capture_at_scale(app, 1600, 1000, &ctx, 2.0);
    assert!(!app.ui.status.contains("Internal error"), "{}", app.ui.status);
    assert_eq!(ctx.pixels_per_point(), 2.0, "native 2x rendering, not image upscaling");
    image::save_buffer(dir.join(format!("{name}.png")), &pixels, 3200, 2000, image::ColorType::Rgba8).unwrap();
}

#[test]
#[ignore = "public article software captures; set VAROS_MEMO_CAPTURE_DIR"]
fn capture_memo_screens() {
    let dir = std::path::PathBuf::from(std::env::var_os("VAROS_MEMO_CAPTURE_DIR").expect("VAROS_MEMO_CAPTURE_DIR required"));
    assert!(dir.is_absolute(), "use an absolute output path");
    std::fs::create_dir_all(&dir).unwrap();
    crate::i18n::set_current(crate::i18n::Lang::EN);
    let db = vectorcraft_text::FontDb::global();
    db.add_font(std::fs::read("/System/Library/Fonts/GeezaPro.ttc").expect("installed Geeza Pro required"));
    assert!(db.face("Geeza Pro", "Regular").is_some());

    let mut app = editor("history");
    rect(&mut app, 0.0, 0.0, 1000.0, 700.0, "#f3eee5");
    label(&mut app, "VAROS", 70.0, 110.0, 22.0, "#333333");
    label(&mut app, "Made together.", 70.0, 220.0, 64.0, "#242323");
    label(&mut app, "One document. Human + agents.", 73.0, 270.0, 23.0, "#696561");
    run(&mut app, "edit.clearHistory", json!({}));
    rect(&mut app, 75.0, 345.0, 240.0, 240.0, "#0c8ce9");
    agent(&mut app, "Codex", "shape.ellipse", json!({"x":365,"y":345,"width":240,"height":240}));
    agent(&mut app, "Codex", "paint.setFill", json!({"color":"#ef9448"}));
    agent(&mut app, "Codex", "stroke.set", json!({"weight":0}));
    agent(&mut app, "Claude Code", "shape.rectangle", json!({"x":655,"y":345,"width":240,"height":240}));
    agent(&mut app, "Claude Code", "paint.setFill", json!({"color":"#6c8f79"}));
    agent(&mut app, "Claude Code", "stroke.set", json!({"weight":0}));
    run(&mut app, "select.none", json!({}));
    let labels = crate::varos::agents::history_labels(&app, &app.session.active().unwrap().history);
    assert!(labels.iter().any(Option::is_none));
    assert!(labels.contains(&Some("Codex".into())) && labels.contains(&Some("Claude Code".into())));
    assert!(app.agents.presence.is_active());
    capture(&mut app, &dir, "history-agent");

    let mut app = editor("paragraph");
    label(&mut app, "ARABIC TYPOGRAPHY", 60.0, 75.0, 18.0, "#76706a");
    let sentence = "التصميم الجيد يحترم اللغة التي يكتب بها، والعربية تستحق أدوات تجيد كشيدتها.";
    for (mode, y, title) in [("off", 165.0, "Kashida Off"), ("auto", 425.0, "Kashida Auto / Strength 100")] {
        label(&mut app, title, 60.0, y - 35.0, 24.0, "#333333");
        let id = run(&mut app, "text.create", json!({"x":60,"y":y,"text":sentence,"size":46,"font":"Geeza Pro","color":"#242323","area":{"width":880,"height":190},"justify":"justifyAll"}))["id"].clone();
        run(&mut app, "text.setFormat", json!({"id":id,"direction":"rightToLeft","kashida":mode,"kashidaStrength":100}));
        let node = app.session.active().unwrap().doc.node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap();
        let vectorcraft_doc::NodeKind::Text(text) = &node.kind else { panic!("text node required") };
        let shaped = vectorcraft_text::layout(db, text);
        assert!(shaped.glyphs.iter().all(|g| g.gid != 0), "no missing Arabic glyphs");
        let tatweels = shaped.glyphs.iter().filter(|g| g.len == 0).count();
        eprintln!("{mode}: {} lines, {tatweels} inserted glyphs", shaped.lines.len());
        assert_eq!(tatweels > 0, mode == "auto", "Auto must really elongate this paragraph");
    }
    capture(&mut app, &dir, "kashida-auto-vs-off");

    capture_harakat(&dir, "Times New Roman", 140.0);

    let mut app = editor("color");
    rect(&mut app, 0.0, 0.0, 1000.0, 700.0, "#f3eee5");
    label(&mut app, "COLOUR STUDY", 70.0, 110.0, 20.0, "#696561");
    label(&mut app, "Shape. Colour. Print.", 70.0, 205.0, 53.0, "#242323");
    rect(&mut app, 75.0, 300.0, 250.0, 280.0, "#ed9651");
    rect(&mut app, 675.0, 300.0, 250.0, 280.0, "#668d75");
    rect(&mut app, 375.0, 300.0, 250.0, 280.0, "#0c8ce9");
    run(&mut app, "ui.colorPanel", json!({"mode":"cmyk","options":true}));
    capture(&mut app, &dir, "color-panel-cmyk");

    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    run(&mut app, "app.home", json!({"section":"templates"}));
    capture(&mut app, &dir, "home-templates");
}

fn capture_harakat(dir: &std::path::Path, font: &str, size: f64) {
    let db = vectorcraft_text::FontDb::global();
    db.load_system_fonts();
    let face = db.face(font, "Regular").expect("installed font required");
    assert_eq!(face.family, font, "do not silently substitute a missing font");
    assert!(face.path().is_some(), "use an installed font, not a bundled test fixture");
    let mut app = editor("character");
    label(&mut app, "ARABIC TYPOGRAPHY", 60.0, 75.0, 18.0, "#76706a");
    label(&mut app, "Harakat in colour", 60.0, 135.0, 40.0, "#242323");
    label(&mut app, "Independent colour. Connected letters.", 60.0, 190.0, 23.0, "#76706a");
    let id = run(&mut app, "text.create", json!({"x":500,"y":430,"text":"مُحَمَّدْ","size":size,"font":font,"color":"#242323","justify":"center"}))["id"]
        .clone();
    let shape = |app: &VectorcraftApp| {
        let node = app.session.active().unwrap().doc.node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap();
        let vectorcraft_doc::NodeKind::Text(text) = &node.kind else { panic!("text node required") };
        vectorcraft_text::layout(db, text)
    };
    let before = shape(&app);
    run(&mut app, "text.styleHarakat", json!({"id":id,"fill":"#e6812f"}));
    let after = shape(&app);
    assert!(after.glyphs.iter().all(|g| g.gid != 0 && g.font_id == face.id()), "no missing glyphs or fallback fonts");
    assert_eq!(
        before.glyphs.iter().map(|g| (&g.outline, g.xf)).collect::<Vec<_>>(),
        after.glyphs.iter().map(|g| (&g.outline, g.xf)).collect::<Vec<_>>(),
        "harakat colouring must preserve the font's original glyph positions"
    );
    eprintln!("Harakat: {font} Regular, {size} pt; colouring preserves all glyph geometry");
    capture(&mut app, dir, "harakat-colour");
}

#[test]
#[ignore = "single article capture; set VAROS_MEMO_CAPTURE_DIR"]
fn capture_memo_harakat() {
    let dir = std::path::PathBuf::from(std::env::var_os("VAROS_MEMO_CAPTURE_DIR").expect("VAROS_MEMO_CAPTURE_DIR required"));
    assert!(dir.is_absolute(), "use an absolute output path");
    std::fs::create_dir_all(&dir).unwrap();
    crate::i18n::set_current(crate::i18n::Lang::EN);
    let font = std::env::var("VAROS_MEMO_HARAKAT_FONT").unwrap_or_else(|_| "Times New Roman".into());
    let size = std::env::var("VAROS_MEMO_HARAKAT_SIZE").map_or(140.0, |s| s.parse().expect("numeric point size"));
    capture_harakat(&dir, &font, size);
}
