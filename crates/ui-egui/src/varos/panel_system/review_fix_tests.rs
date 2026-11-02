//! Review findings of 2026-11-02 (Sol S1–S8, Grok G2/G4), checked through the real app frame with
//! the panel workspace presenting the editor. The rest of the suite mostly builds the app without
//! it, so these are the presenter's own compatibility checks.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{
    State, metrics,
    model::{Action, Edge, Layout, Source, Target},
};
use crate::VectorcraftApp;
use egui::{Event, Key, Modifiers, Pos2, Rect, pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
    screen: egui::Vec2,
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

fn texts(output: &egui::FullOutput) -> Vec<(String, Rect, Rect)> {
    fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect, Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect(), clip)),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in &output.shapes {
        collect(&shape.shape, shape.clip_rect, &mut out);
    }
    out
}

impl Harness {
    /// The app with the panel workspace on (Tools left, Control top, Layers and Swatches floating).
    fn new() -> Self {
        Self::sized(vec2(1600.0, 1000.0))
    }

    fn sized(screen: egui::Vec2) -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("ui.panelReview", json!({"enabled": true})).unwrap();
        assert!(super::enabled(&app), "the presenter is on for these checks");
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self { app, ctx, time: 0.0, screen };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 0.05;
        let input = egui::RawInput {
            time: Some(self.time),
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            focused: true,
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let app = &mut self.app;
        app.begin_host_iteration();
        let mut out = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
        assert!(!self.app.ui.status.starts_with("Internal error:"), "{}", self.app.ui.status);
        out
    }

    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..4 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at), button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.settle();
    }

    fn state(&self) -> &State {
        self.app.ui.varos_panel_review.as_ref().unwrap()
    }

    fn layout(&self) -> &Layout {
        &self.state().layout
    }

    fn visible(&self, panel: &str) -> bool {
        self.layout().owner(panel).is_some_and(|h| !h.hidden)
    }

    fn select_rectangle(&mut self) {
        let id = self.app.run("shape.rectangle", json!({"x": 20, "y": 20, "width": 60, "height": 40})).unwrap()["id"].clone();
        self.app.run("select.set", json!({"ids": [id]})).unwrap();
    }
}

/// S1: Control bar links (Opacity, Style) and other `open_panel` requests show their panel.
#[test]
fn control_bar_opacity_link_opens_transparency_in_the_panel_workspace() {
    let mut h = Harness::new();
    h.select_rectangle();
    assert!(!h.visible("transparency"));
    let output = h.settle();
    let (_, opacity, _) = texts(&output).into_iter().find(|(text, _, _)| text == "Opacity:").expect("the hosted Control bar draws Opacity:");
    h.click(opacity.center());
    assert!(h.visible("transparency"), "clicking Opacity: shows Transparency");
    assert!(h.app.ui.open_panel.is_none(), "the request is consumed");
    assert!(crate::menus::checked(&h.app, "window.panel", &json!({"panel": "transparency"})).unwrap());
    // Any other caller of the compatibility request is followed too.
    h.app.ui.open_panel = Some("graphicStyles".into());
    h.settle();
    assert!(h.visible("graphicStyles"));
    assert!(h.app.ui.open_panel.is_none());
}

/// S2: a library panel opened in the workspace is open for its own lifecycle commands.
#[test]
fn library_panels_follow_their_lifecycle_in_the_panel_workspace() {
    let mut h = Harness::new();
    let panel = h.app.run("window.swatchLibrary", json!({"library": "pastels"})).unwrap()["panel"].as_str().unwrap().to_string();
    h.settle();
    assert!(h.visible(&panel), "the library panel shows in the presented layout");
    assert!(crate::panels::library_panel::is_open(&h.app.ui, &panel));
    let settings = h.app.run("ui.libraryPanel", json!({"panel": panel, "view": "largeList", "persistent": true})).unwrap();
    assert_eq!((settings["open"].as_bool(), settings["persistent"].as_bool()), (Some(true), Some(true)));
    // Showing another library keeps the panel's place under its new identity.
    let host = h.layout().owner(&panel).unwrap().id;
    let renamed = h.app.run("ui.libraryPanel", json!({"panel": panel, "library": "brights"})).unwrap()["panel"].as_str().unwrap().to_string();
    assert_eq!(renamed, "library:swatches:brights");
    assert_eq!(h.layout().owner(&renamed).map(|h| h.id), Some(host));
    assert!(h.layout().owner(&panel).is_none());
    h.settle();
    // Closing every swatch library forgets their panels.
    h.app.run("window.swatchLibrary", json!({"library": null})).unwrap();
    assert!(h.layout().owner(&renamed).is_none(), "a closed library panel leaves the layout");
    assert!(!crate::panels::library_panel::is_open(&h.app.ui, &renamed));
    h.layout().validate().unwrap();
    // A persistent library comes back at startup.
    crate::panels::library_panel::restore(&mut h.app);
    assert!(h.visible(&renamed), "restore floats the persistent library back into the layout");
    h.settle();
}

/// S3: Presentation Mode presents the artwork alone: no Tools, Control or panels to edit with.
#[test]
fn presentation_mode_hides_all_workspace_chrome() {
    let mut h = Harness::new();
    h.select_rectangle();
    h.app.run("window.panel", json!({"panel": "color"})).unwrap();
    let before = h.settle();
    assert!(texts(&before).iter().any(|(text, _, _)| text == "Opacity:"));
    assert!(!h.state().hits.is_empty());
    h.app.run("view.presentation", json!({})).unwrap();
    assert_eq!(h.app.ui.screen_mode, 3);
    let output = h.settle();
    assert!(h.state().hits.is_empty(), "no panel host is drawn: {:?}", h.state().hits.iter().map(|(n, _)| n).collect::<Vec<_>>());
    assert!(!texts(&output).iter().any(|(text, _, _)| text == "Opacity:" || text == "Color"), "no Control or panel content is painted");
    let canvas = h.app.canvas_rect.expect("the canvas is drawn");
    assert!(canvas.width() >= h.screen.x - 1.0 && canvas.height() >= h.screen.y - 1.0, "no block reserves canvas space: {canvas:?}");
    // Leaving Presentation Mode restores the same workspace.
    let layout = h.layout().clone();
    h.app.run("view.presentation", json!({})).unwrap();
    h.settle();
    assert_eq!(h.layout(), &layout);
    assert!(!h.state().hits.is_empty());
}

/// S4: Window-menu items and panel keys toggle a shown panel off again.
#[test]
fn window_menu_toggle_and_panel_keys_hide_a_shown_panel() {
    let mut h = Harness::new();
    h.app.run("window.panel", json!({"panel": "color", "toggle": true})).unwrap();
    assert!(h.visible("color"));
    h.app.run("window.panel", json!({"panel": "color", "toggle": true})).unwrap();
    assert!(!h.visible("color"), "a checked Window item hides its panel");
    // Toggling one tab off leaves its group's other tabs shown.
    h.app.run("window.panel", json!({"panel": "align"})).unwrap();
    h.app.run("window.panel.move", json!({"panel": "align", "anchor": "swatches", "zone": "tab"})).unwrap();
    assert_eq!(h.layout().owner("align").map(|h| h.id), h.layout().owner("swatches").map(|h| h.id));
    h.app.run("window.panel", json!({"panel": "align", "toggle": true})).unwrap();
    assert!(!h.visible("align") && h.visible("swatches"), "only the toggled panel hides");
    h.layout().validate().unwrap();
    // The real panel key (Shift+F7, Align) shows it, then hides it again.
    let press = |h: &mut Harness| {
        let events = vec![Event::ModifiersChanged(Modifiers::SHIFT), key(Key::F7, Modifiers::SHIFT)];
        h.ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| crate::shortcuts::handle(&mut h.app, ui.ctx())).textures_delta.clear();
    };
    press(&mut h);
    assert!(h.visible("align"), "Shift+F7 shows Align");
    press(&mut h);
    assert!(!h.visible("align"), "Shift+F7 again hides Align");
}

/// S5: Tab hides Tools, Control and every panel; Shift+Tab hides the panels only, one panel at a
/// time, so a panel attached beside Tools hides while Tools stays.
#[test]
fn tab_hides_everything_and_shift_tab_hides_panels_beside_tools_too() {
    let mut h = Harness::new();
    let tools = h.layout().owner("tools").unwrap().id;
    let layers = h.layout().owner("layers").unwrap().id;
    h.app
        .run(
            "ui.panelReview.action",
            json!({"action": Action::Move { source: Source::Host(layers), target: Target::Attach { host: tools, edge: Edge::Right } }}),
        )
        .unwrap();
    assert_eq!(h.layout().owner("layers").map(|h| h.id), h.layout().owner("tools").map(|h| h.id), "Layers is attached beside Tools");
    h.settle();
    let arranged = h.layout().clone();
    h.app.run("window.panels", json!({})).unwrap();
    assert!(h.visible("tools") && h.visible("control"), "Shift+Tab leaves Tools and Control");
    assert!(!h.visible("layers") && !h.visible("swatches"), "Shift+Tab hides Layers beside Tools and the floating Swatches");
    h.settle();
    h.app.run("window.panels", json!({})).unwrap();
    assert_eq!(h.layout(), &arranged, "a second Shift+Tab restores the exact arrangement");
    h.app.run("window.dock", json!({})).unwrap();
    for panel in ["tools", "control", "layers", "swatches"] {
        assert!(!h.visible(panel), "Tab hides {panel}");
    }
    h.settle();
    h.app.run("window.dock", json!({})).unwrap();
    assert_eq!(h.layout(), &arranged, "a second Tab restores everything it hid");
    // Through the real keys.
    let press = |h: &mut Harness, modifiers: Modifiers| {
        h.frame(vec![Event::ModifiersChanged(modifiers), key(Key::Tab, modifiers)]);
        h.frame(vec![Event::ModifiersChanged(Modifiers::NONE)]);
    };
    press(&mut h, Modifiers::SHIFT);
    assert!(h.visible("tools") && !h.visible("layers"), "Shift+Tab key");
    press(&mut h, Modifiers::SHIFT);
    press(&mut h, Modifiers::NONE);
    assert!(!h.visible("tools") && !h.visible("control") && !h.visible("swatches"), "Tab key");
    press(&mut h, Modifiers::NONE);
    assert_eq!(h.layout(), &arranged);
}

/// S6: collapsing Tools by command narrows it to one column.
#[test]
fn tools_collapse_command_changes_the_tools_width() {
    let mut h = Harness::new();
    h.app.run("window.toolbarColumns", json!({"double": true})).unwrap();
    assert_eq!(h.layout().tools_width(), Some(metrics::TOOLS_MAX_WIDTH));
    h.app.run("window.panel.collapse", json!({"panel": "tools", "collapsed": true})).unwrap();
    assert_eq!(h.layout().tools_width(), Some(metrics::TOOLS_MIN_WIDTH));
    h.app.run("window.panel.collapse", json!({"panel": "tools", "collapsed": false})).unwrap();
    assert_eq!(h.layout().tools_width(), Some(metrics::TOOLS_MAX_WIDTH));
}

/// S8: the Varos shell switch is the one owner: off, the classic dock presents and the panel
/// workspace neither draws nor answers; back on, the same workspace returns.
#[test]
fn turning_the_varos_shell_off_in_preferences_hands_the_editor_to_the_classic_dock() {
    let mut h = Harness::new();
    let layout = h.layout().clone();
    crate::prefs_dialog::open(&mut h.app, None);
    h.app.ui.dialog.as_mut().unwrap().fields.insert(crate::varos::shell::PREF_KEY.into(), json!(false));
    crate::prefs_dialog::apply(&mut h.app).unwrap();
    h.app.ui.dialog = None;
    assert!(!super::enabled(&h.app));
    h.app.ui.varos_panel_review.as_mut().unwrap().hits.clear();
    h.settle();
    assert!(h.state().hits.is_empty(), "the panel workspace draws nothing while the shell is off");
    assert!(super::command(&mut h.app, "window.panel.layout", &json!({})).is_none(), "nor answers panel commands");
    h.app.run("window.panel", json!({"panel": "color"})).unwrap();
    assert!(!h.visible("color"), "the classic dock handled Window › Color");
    assert!(crate::panel_docking::showing(&h.app.ui, "color"));
    crate::prefs_dialog::open(&mut h.app, None);
    h.app.ui.dialog.as_mut().unwrap().fields.insert(crate::varos::shell::PREF_KEY.into(), json!(true));
    crate::prefs_dialog::apply(&mut h.app).unwrap();
    h.app.ui.dialog = None;
    assert!(super::enabled(&h.app));
    h.settle();
    assert_eq!(h.layout(), &layout, "the saved workspace returns unchanged");
    assert!(!h.state().hits.is_empty());
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
        let (_, rect, clip) = painted.iter().find(|(text, _, _)| text == label).unwrap_or_else(|| panic!("{label} exists in the hosted Control row"));
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
            assert!(super::content::show(&mut app, ui, "control", rect));
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

/// G4: Apply sends the fields the user edited, even back at their opening value; a document
/// switch skips only the units; Escape in a field keeps Preferences and its draft.
#[test]
fn preferences_apply_sends_touched_fields_and_escape_in_a_field_keeps_the_draft() {
    let mut h = Harness::new();
    crate::prefs_dialog::open(&mut h.app, Some("General"));
    h.settle();
    let opening = h.app.session.prefs.keyboard_increment;
    // Another editor changes the setting while Preferences is open.
    h.app.run("prefs.set", json!({"key": "keyboardIncrement", "value": opening + 6.0})).unwrap();
    let output = h.frame(vec![]);
    let (_, label, _) =
        texts(&output).into_iter().find(|(text, _, _)| text == "Keyboard Increment:").expect("the General page shows Keyboard Increment");
    // The row's label column is 210 points wide; the field follows it.
    let field = Rect::from_min_size(pos2(label.left() + 214.0, label.center().y - 9.0), vec2(80.0, 18.0));
    // The user types 5, then puts the field back at its opening value.
    for typed in [format!("{}", opening + 4.0), format!("{opening}")] {
        h.click(field.center());
        h.frame(vec![key(Key::A, Modifiers::COMMAND)]);
        h.frame(vec![Event::Text(typed)]);
        h.frame(vec![key(Key::Enter, Modifiers::NONE)]);
        h.settle();
    }
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().fields["keyboardIncrement"].as_f64(), Some(opening));
    h.app.run("preferences.apply", json!({})).unwrap();
    assert_eq!(h.app.session.prefs.keyboard_increment, opening, "the edited field wins over the outside change");
    // Escape in a focused field leaves the field, not Preferences, and keeps the draft.
    h.click(field.center());
    h.frame(vec![key(Key::A, Modifiers::COMMAND)]);
    h.frame(vec![Event::Text(format!("{}", opening + 2.0))]);
    h.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    h.click(field.center());
    assert!(h.ctx.egui_wants_keyboard_input(), "the field has the keyboard");
    h.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    h.settle();
    let d = h.app.ui.dialog.as_ref().expect("Escape in a field keeps Preferences open");
    assert_eq!(d.fields["keyboardIncrement"].as_f64(), Some(opening + 2.0), "the unapplied draft is kept");
    assert!(!h.ctx.egui_wants_keyboard_input(), "the field gave up the keyboard");
    // Units of a document that is no longer active are skipped; the other edits apply.
    let units = h.app.ui.dialog.as_ref().unwrap().fields["unitsGeneral"].clone();
    let other = if units == "millimeters" { "points" } else { "millimeters" };
    // An agent's edit through the control channel counts as the user's.
    let (request, _) = crate::control::ControlRequest::new(
        "ui.dialog.set",
        json!({"fields": [{"field": "unitsGeneral", "value": other}, {"field": "keyboardIncrement", "value": 3}]}),
    );
    let crate::control::Outcome::Done(reply) = crate::control::handle(&mut h.app, &h.ctx, &request) else { panic!("ui.dialog.set replies at once") };
    assert_eq!(reply["ok"], true, "{reply}");
    let before: Vec<_> = h.app.session.documents().iter().map(|d| d.doc.units).collect();
    h.app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    let result = h.app.run("preferences.apply", json!({})).unwrap();
    assert_eq!(result["skipped"], json!(["unitsGeneral"]));
    assert_eq!(h.app.session.prefs.keyboard_increment, 3.0, "unrelated edits still apply");
    assert_eq!(h.app.session.documents()[..before.len()].iter().map(|d| d.doc.units).collect::<Vec<_>>(), before);
    assert!(h.app.ui.dialog.is_some());
}
