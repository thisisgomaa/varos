//! Home interactions exercised through real egui input and accessibility.
use super::*;

pub(crate) fn assert_chrome(app: &mut VectorcraftApp, document: bool) {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    app.fonts_ready = true;
    app.canvas.worker_started = true;
    app.canvas_rect = None;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
        max_texture_side: Some(8192),
        ..Default::default()
    };
    let mut root = egui::Id::NULL;
    let mut out = ctx.run_ui(raw, |ui| {
        root = ui.scope_id();
        app.ui(ui);
    });
    out.textures_delta.clear();
    assert!(!app.ui.status.starts_with("Internal error:"), "{}", app.ui.status);
    // egui 0.37: a panel's id is its parent Ui's scope id salted with the panel's name.
    let panel = |id: &str| egui::containers::panel::PanelState::load(&ctx, root.with_salt(egui::IdSalt::new(id))).is_some();
    assert!(panel("app_bar"), "title bar remains available");
    for id in ["toolbar", "control_bar", "status_bar", "hint_bar"] {
        assert_eq!(panel(id), document, "{id}: editor chrome follows the Home gate");
    }
    // The dock (craft-ui's dock area since the 2026-10-26 merge): its icons show with a document.
    let icons = crate::panel_docking::hit_rects(&ctx).iter().any(|(hit, _)| matches!(hit, craft_ui::docking::Hit::Icon(_)));
    assert_eq!(icons, document, "dock: editor chrome follows the Home gate");
    assert_eq!(app.canvas_rect.is_some(), document);
    let texts: Vec<_> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text()),
            _ => None,
        })
        .collect();
    assert_eq!(texts.contains(&"Pick up where you left off, or start something new."), !document);
    if !document {
        for label in ["New board", "Open…", "Recent"] {
            assert!(texts.contains(&label), "Home keeps {label}: {texts:?}");
        }
        assert!(!texts.contains(&"File"), "in-window editor menus stay hidden");
    }
}

use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn app() -> VectorcraftApp {
    let services = crate::Services {
        read: Some(Box::new(|p| std::fs::read(p).map_err(|e| e.to_string()))),
        write: Some(Box::new(|p, b| std::fs::write(p, b).map_err(|e| e.to_string()))),
        ..Default::default()
    };
    VectorcraftApp::new(vectorcraft_engine::Session::new(), services)
}
fn harness(app: VectorcraftApp) -> Harness<'static, VectorcraftApp> {
    Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_context_setup(|ctx| {
            crate::theme::install_fonts(ctx);
            crate::theme::apply(ctx, crate::theme::Brightness::Dark);
            egui_extras::install_image_loaders(ctx);
            ctx.all_styles_mut(|s| {
                s.interaction.tooltip_delay = 0.0;
                s.interaction.tooltip_grace_time = 0.0;
                s.interaction.show_tooltips_only_when_still = false;
            });
        })
        .build_ui_state(
            |ui, app| {
                let rect = ui.max_rect();
                if is_home(app) {
                    show(app, ui, rect);
                } else {
                    ui.label("Document");
                }
            },
            app,
        )
}
fn click(h: &mut Harness<'_, VectorcraftApp>, label: &str) {
    h.get_by_role_and_label(egui::accesskit::Role::Button, label).click();
    h.run_steps(3);
}
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("varos-home-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn file(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, vectorcraft_format::save(&vectorcraft_doc::Document::new(120.0, 80.0), false)).unwrap();
        p
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn texts(h: &Harness<'_, VectorcraftApp>) -> Vec<String> {
    h.output().shapes.iter().filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some(t.galley.text().to_string()) } else { None }).collect()
}

#[test]
fn sections_actions_and_templates_run_real_commands() {
    let mut h = harness(app());
    h.run_steps(2);
    for label in ["Recents", "Projects", "Templates", "Getting started", "Preferences", "New board", "Open…"] {
        h.get_by_role_and_label(egui::accesskit::Role::Button, label);
    }
    click(&mut h, "Templates");
    assert_eq!(h.state().ui.home_view.section, Section::Templates);
    click(&mut h, "Business card");
    let doc = &h.state().session.active().unwrap().doc;
    let board = doc.artboards[0].rect;
    assert!((board.width() - 85.0 * 72.0 / 25.4).abs() < 0.01);
    assert!((board.height() - 55.0 * 72.0 / 25.4).abs() < 0.01);
    let mut h = harness(app());
    click(&mut h, "New board");
    assert!(h.state().session.active().unwrap().doc.artboards.is_empty());
    let mut h = harness(app());
    click(&mut h, "Preferences");
    assert!(h.state().ui.dialog.is_some());
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let mut a = app();
    a.services.open_async = Some(Box::new(move || {
        c.fetch_add(1, Ordering::Relaxed);
    }));
    let mut h = harness(a);
    click(&mut h, "Open…");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn disabled_sharing_does_nothing_and_explains_why() {
    for label in ["Shared with me", "Join with a code"] {
        let mut h = harness(app());
        h.run_steps(2);
        let node = h.get_by_role_and_label(egui::accesskit::Role::Button, label);
        assert!(node.accesskit_node().is_disabled());
        node.hover();
        h.run_steps(4);
        assert!(texts(&h).iter().any(|s| s == "Sharing comes later"), "{:?}", texts(&h));
        // Send physical input even though accessibility correctly marks the widget disabled.
        h.get_by_role_and_label(egui::accesskit::Role::Button, label).click();
        h.run_steps(3);
        assert!(h.state().session.documents().is_empty());
        assert_eq!(h.state().ui.home_view.section, Section::Recents);
        assert!(h.state().ui.dialog.is_none());
    }
}

#[test]
fn recent_card_opens_and_context_menu_removes_or_reveals() {
    let dir = Folder::new();
    let path = dir.file("Poster.varos");
    let mut a = app();
    a.ui.recent_files = vec![path.to_string_lossy().into_owned()];
    let mut h = harness(a);
    click(&mut h, "Poster.varos");
    assert_eq!(h.state().session.active().unwrap().path.as_deref(), path.to_str());
    assert!(!is_home(h.state()));
    h.state_mut().run("app.home", json!({})).unwrap();
    h.run_steps(3);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Poster.varos").click_secondary();
    h.run_steps(3);
    let revealed = Arc::new(std::sync::Mutex::new(String::new()));
    let r = revealed.clone();
    h.state_mut().services.reveal = Some(Box::new(move |p| {
        *r.lock().unwrap() = p.into();
        Ok(())
    }));
    click(&mut h, "Show in Finder");
    assert_eq!(*revealed.lock().unwrap(), path.to_string_lossy());
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Poster.varos").click_secondary();
    h.run_steps(3);
    click(&mut h, "Remove from Recents");
    assert!(h.state().ui.recent_files.is_empty());
    assert!(path.exists());
}

#[test]
fn sort_orders_are_stable_and_persist() {
    use std::time::{Duration, UNIX_EPOCH};
    let make = |name: &str, modified: u64, created: u64| File {
        path: PathBuf::from(name),
        name: name.into(),
        modified: Some(UNIX_EPOCH + Duration::from_secs(modified)),
        created: Some(UNIX_EPOCH + Duration::from_secs(created)),
        artboards: None,
        thumbnail: None,
    };
    let original = vec![make("B.varos", 20, 30), make("A.varos", 30, 10), make("C.varos", 10, 20)];
    for (order, expected) in [
        (Sort::Name, ["A.varos", "B.varos", "C.varos"]),
        (Sort::Modified, ["A.varos", "B.varos", "C.varos"]),
        (Sort::Created, ["B.varos", "C.varos", "A.varos"]),
    ] {
        let mut files = original.clone();
        data::sort(&mut files, order);
        assert_eq!(files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), expected);
    }
    let mut h = harness(app());
    h.get_by_label("Last modified").click();
    h.run_steps(3);
    h.get_by_label("Name").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.home_preferences.sort, Sort::Name);
    let saved = serde_json::to_value(&h.state().ui).unwrap();
    let restored: crate::state::UiState = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.home_preferences.sort, Sort::Name);
}

#[test]
fn projects_pick_folders_list_varos_files_and_dim_missing_folders() {
    let dir = Folder::new();
    dir.file("A.varos");
    dir.file("B.VAROS");
    dir.file("Ignore.svg");
    std::fs::create_dir(dir.0.join("folder.varos")).unwrap();
    let mut a = app();
    let path = dir.0.to_string_lossy().into_owned();
    let (shown, _) = crate::tests_picks::off_the_ui_thread(&mut a.services);
    let mut h = harness(a);
    click(&mut h, "Add project…");
    assert!(h.state().ui.home_preferences.projects.is_empty());
    assert_eq!(shown.requests(), vec![crate::picks::PickRequest::Folder]);
    shown.answer(&[&path]);
    let ctx = h.ctx.clone();
    crate::picks::poll(h.state_mut(), &ctx);
    h.run_steps(3);
    assert_eq!(h.state().ui.home_preferences.projects, vec![dir.0.canonicalize().unwrap()]);
    let name = dir.0.file_name().unwrap().to_str().unwrap();
    click(&mut h, name);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "A.varos");
    h.get_by_role_and_label(egui::accesskit::Role::Button, "B.VAROS");
    assert!(h.query_by_label("Ignore.svg").is_none());
    let missing = dir.0.join("Missing");
    h.state_mut().ui.home_preferences.projects.push(missing);
    h.state_mut().ui.home_view.generation += 1;
    h.run_steps(3);
    assert!(h.get_by_role_and_label(egui::accesskit::Role::Button, "Missing").accesskit_node().is_disabled());
    let t = Tokens::get(&h.ctx);
    assert!(
        h.output()
            .shapes
            .iter()
            .any(|s| matches!(&s.shape,egui::Shape::Text(t2) if t2.galley.text()=="Not found" && t2.fallback_color==t.text_disabled))
    );
}

#[test]
fn tips_hide_persist_and_help_restores_them() {
    let mut h = harness(app());
    click(&mut h, "Hide");
    assert!(h.state().ui.home_preferences.tips_hidden);
    assert!(!texts(&h).iter().any(|s| s == "Write in Arabic"));
    let saved = serde_json::to_value(&h.state().ui).unwrap();
    let restored: crate::state::UiState = serde_json::from_value(saved).unwrap();
    assert!(restored.home_preferences.tips_hidden);
    h.state_mut().run("help.gettingStarted", json!({})).unwrap();
    h.run_steps(3);
    assert!(!h.state().ui.home_preferences.tips_hidden);
    assert!(texts(&h).iter().any(|s| s == "Write in Arabic"));
    assert!(crate::menus::UI_COMMANDS.iter().any(|(id, ..)| *id == "help.gettingStarted"));
}

#[test]
fn search_filters_files_projects_and_templates_and_escape_clears() {
    let dir = Folder::new();
    let a = dir.file("Poster.varos");
    let b = dir.file("Logo.varos");
    let mut app = app();
    app.ui.recent_files = vec![a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned()];
    app.ui.home_preferences.projects.push(dir.0.clone());
    let mut h = harness(app);
    h.get_by_role(egui::accesskit::Role::TextInput).focus();
    h.event(egui::Event::Text("poster".into()));
    h.run_steps(3);
    assert!(h.state().ui.home_view.query == "poster");
    assert!(texts(&h).iter().any(|s| s == "Poster.varos"));
    assert!(h.query_by_label("Logo.varos").is_none());
    h.get_by_role_and_label(egui::accesskit::Role::Button, "A4 poster");
    assert!(h.query_by_label("Business card").is_none());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().ui.home_view.query.is_empty());
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Logo.varos");
}

#[test]
fn greeting_uses_local_hour_boundaries_and_catalogs_cover_home() {
    for (hour, expected) in
        [(0, "evening"), (4, "evening"), (5, "morning"), (11, "morning"), (12, "afternoon"), (17, "afternoon"), (18, "evening"), (23, "evening")]
    {
        assert_eq!(data::greeting(hour, "Sam"), format!("Good {expected}, Sam"));
    }
    let ar = crate::i18n::Lang::from_code("ar").unwrap();
    assert_eq!(crate::i18n::tr(ar, "Sharing comes later"), "ستتوفر المشاركة لاحقًا");
}

#[test]
fn arrow_navigation_and_return_open_the_selected_card() {
    let dir = Folder::new();
    let a = dir.file("A.varos");
    let b = dir.file("B.varos");
    let mut app = app();
    app.ui.recent_files = vec![a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned()];
    app.ui.home_preferences.sort = Sort::Name;
    let mut h = harness(app);
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    assert!(h.state().ui.home_view.selected.as_ref().unwrap().ends_with("B.varos"));
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().session.active().unwrap().path.as_deref(), b.to_str());
}

#[derive(Debug)]
struct DropFile(PathBuf);
impl egui::DroppedFile for DropFile {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}
#[test]
fn drop_anywhere_on_home_opens_and_invalid_files_report_an_error() {
    let dir = Folder::new();
    let path = dir.file("Dropped.varos");
    for at in [pos2(20.0, 20.0), pos2(100.0, 400.0), pos2(1200.0, 800.0)] {
        let mut a = app();
        a.run("file.new", json!({})).unwrap();
        a.canvas_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1440.0, 900.0)));
        a.run("app.home", json!({})).unwrap();
        a.open_dropped_files(vec![Arc::new(DropFile(path.clone()))], Some(at), false);
        assert_eq!(a.session.active().unwrap().path.as_deref(), path.to_str());
    }
    for name in ["Bad.varos", "Unsupported.xyz"] {
        let bad = dir.0.join(name);
        std::fs::write(&bad, b"not a document").unwrap();
        for with_document in [false, true] {
            let mut a = app();
            if with_document {
                a.run("file.new", json!({})).unwrap();
                a.run("app.home", json!({})).unwrap();
            }
            a.open_dropped_files(vec![Arc::new(DropFile(bad.clone()))], None, false);
            assert!(is_home(&a));
            assert!(a.ui.dialog.is_none(), "Home drop errors must be toast only: {name}");
            let message = a.ui.status.clone();
            assert!(message.contains(name));
            let mut h = harness(a);
            h.run_steps(2);
            assert!(texts(&h).iter().any(|s| s == &message), "the error toast is visible");
        }
        let mut editor = app();
        editor.run("file.new", json!({})).unwrap();
        editor.open_dropped_files(vec![Arc::new(DropFile(bad))], None, false);
        assert!(editor.ui.dialog.is_some(), "editor open failures keep their existing modal");
    }
}

#[test]
fn save_and_open_write_real_png_thumbnails_and_cards_show_them() {
    let dir = Folder::new();
    let path = dir.0.join("Saved.varos");
    let mut a = app();
    a.run("file.new", json!({"width":120,"height":80})).unwrap();
    a.run("shape.rectangle", json!({"x":10,"y":10,"width":50,"height":30})).unwrap();
    a.run("file.saveAs", json!({"path":path})).unwrap();
    let root = data::cache_root().unwrap();
    let png = data::cache_path(&root, &path);
    assert_eq!(&std::fs::read(&png).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    let img = image::open(&png).unwrap().to_rgba8();
    assert!(img.pixels().any(|p| p.0 != [255, 255, 255, 255]));
    assert_eq!(img.width(), 480);
    let file = File::read(path.clone());
    assert_eq!(file.artboards, Some(1));
    assert_eq!(file.thumbnail.as_deref(), Some(png.as_path()));
    a.run("app.home", json!({})).unwrap();
    let mut h = harness(a);
    h.run_steps(3);
    let tex = thumbnail(&h.ctx, &png).unwrap();
    assert!(h.output().shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.fill_texture_id()==tex.id())));
    std::fs::remove_file(&png).unwrap();
    let mut a = app();
    crate::io::open_reporting(&mut a, path.to_str().unwrap()).unwrap();
    assert!(png.exists());
    std::fs::remove_file(&png).unwrap();
    std::fs::remove_file(png.with_extension("json")).unwrap();
}

#[test]
fn home_shortcuts_do_not_nudge_or_open_tool_dialogs_behind_home() {
    let mut a = app();
    a.run("file.new", json!({})).unwrap();
    a.run("shape.rectangle", json!({"x":10,"y":20,"width":50,"height":30})).unwrap();
    a.run("select.all", json!({})).unwrap();
    a.run("app.home", json!({})).unwrap();
    let before = vectorcraft_format::save(&a.session.active().unwrap().doc, false);
    let ctx = egui::Context::default();
    for key in [egui::Key::ArrowRight, egui::Key::Enter, egui::Key::Delete] {
        let raw = egui::RawInput {
            events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| crate::shortcuts::handle(&mut a, ui.ctx()));
        out.textures_delta.clear();
        assert!(a.ui.dialog.is_none());
        assert_eq!(vectorcraft_format::save(&a.session.active().unwrap().doc, false), before);
    }
    for key in [egui::Key::N, egui::Key::O] {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        a.services.open_async = Some(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }));
        let modifiers = egui::Modifiers::COMMAND;
        let raw = egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
            ],
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| crate::shortcuts::handle(&mut a, ui.ctx()));
        out.textures_delta.clear();
        if key == egui::Key::N {
            assert_eq!(a.ui.dialog.take().unwrap().kind, "newDocument");
        } else {
            assert_eq!(calls.load(Ordering::Relaxed), 1);
        }
    }
}

#[test]
fn home_input_leaves_a_hidden_text_edit_unchanged_from_the_first_frame() {
    let press = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    for event in
        [egui::Event::Text("x".into()), press(egui::Key::Backspace), press(egui::Key::Delete), press(egui::Key::Enter), press(egui::Key::ArrowRight)]
    {
        let mut a = app();
        a.run("file.new", json!({})).unwrap();
        a.run("text.create", json!({"x":20,"y":40,"text":"Hello"})).unwrap();
        a.select_tool("type");
        a.session.apply_actions(vec![vectorcraft_tools::Action::Notify("text.editNew".into())]).unwrap();
        assert!(a.session.tool_wants_text());
        let before = vectorcraft_format::save(&a.session.active().unwrap().doc, false);
        a.run("app.home", json!({})).unwrap();
        let ctx = egui::Context::default();
        // No empty warm-up frame: the very first Home frame carries the real event.
        let raw = egui::RawInput { events: vec![event.clone()], ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| crate::shortcuts::handle(&mut a, ui.ctx()));
        out.textures_delta.clear();
        assert_eq!(vectorcraft_format::save(&a.session.active().unwrap().doc, false), before, "{event:?}");
    }
}

#[test]
fn home_scrolls_in_a_short_window_and_scrolled_cards_still_open() {
    let dir = Folder::new();
    let mut a = app();
    a.ui.recent_files = (0..12).map(|i| dir.file(&format!("Board {i:02}.varos")).to_string_lossy().into_owned()).collect();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    egui_extras::install_image_loaders(&ctx);
    let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 400.0));
    let frame = |a: &mut VectorcraftApp, time, events| {
        let mut out =
            ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(time), events, ..Default::default() }, |ui| show(a, ui, screen));
        out.textures_delta.clear();
        out
    };
    let visible = |out: &egui::FullOutput, label: &str| {
        out.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label && s.clip_rect.contains_rect(t.visual_bounding_rect()) => Some(t.visual_bounding_rect()),
            _ => None,
        })
    };
    let out = frame(&mut a, 0.0, vec![]);
    assert!(visible(&out, "Board 11.varos").is_none());
    // Scroll partway: the last row can be brought into view above Projects/tips.
    let wheel = egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -1250.0),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
        source: egui::MouseWheelSource::Unknown,
    };
    frame(&mut a, 0.1, vec![egui::Event::PointerMoved(pos2(600.0, 300.0)), wheel]);
    let mut out = frame(&mut a, 0.2, vec![]);
    for i in 3..30 {
        out = frame(&mut a, i as f64 / 10.0, vec![]);
    }
    let target = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with("Board ") && s.clip_rect.contains_rect(t.visual_bounding_rect()) => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        })
        .expect("a scrolled card is visible");
    let button = |pressed| egui::Event::PointerButton { pos: target, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(&mut a, 3.0, vec![egui::Event::PointerMoved(target), button(true)]);
    frame(&mut a, 3.1, vec![button(false)]);
    assert!(a.session.active().is_some());
}

#[test]
fn thumbnail_reloads_and_rerenders_if_the_source_changes_during_rendering() {
    thumbnail_source_change(true);
}

#[test]
fn thumbnail_reloads_if_the_queued_stamp_is_already_stale() {
    thumbnail_source_change(false);
}

fn thumbnail_source_change(during_render: bool) {
    let dir = Folder::new();
    let root = dir.0.join("thumbs");
    let path = dir.0.join("Racing.varos");
    let old = vectorcraft_doc::Document::new(120.0, 80.0);
    let mut new = vectorcraft_doc::Document::new(60.0, 160.0);
    let mut second = new.artboards[0].clone();
    second.id += 1;
    new.artboards.push(second);
    let save = |doc: &vectorcraft_doc::Document, seconds| {
        std::fs::write(&path, vectorcraft_format::save(doc, false)).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)))
            .unwrap();
    };
    save(&old, 100);
    let queued = data::stamp(&path).unwrap();
    if !during_render {
        save(&new, 200);
    }
    let mut renders = Vec::new();
    let png = data::write_thumbnail_with(&root, &path, &old, queued, |doc| {
        let pixels = data::render_thumbnail(doc)?;
        renders.push(doc.artboards[0].rect);
        if during_render && renders.len() == 1 {
            // The old pixels exist, but publication must detect this intervening save.
            save(&new, 200);
        }
        Ok(pixels)
    })
    .unwrap();
    if during_render {
        assert_eq!(renders, [old.artboards[0].rect, new.artboards[0].rect]);
    } else {
        assert_eq!(renders, [new.artboards[0].rect], "a stale queued snapshot is never rendered as current");
    }
    let image = image::open(&png).unwrap();
    assert_eq!((image.width(), image.height()), (135, 360), "only the freshly rendered first artboard is published");
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(png.with_extension("json")).unwrap()).unwrap();
    assert_eq!(manifest["stamp"], json!(data::stamp(&path).unwrap()));
    assert_eq!(manifest["artboards"], 2);
}
