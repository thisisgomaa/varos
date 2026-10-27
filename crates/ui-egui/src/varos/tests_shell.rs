//! The Varos shell, stage 1 ([`super::shell`]): through the real frame, with real clicks and drags.

use egui::{Pos2, Rect, pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;
use crate::varos::shell::{self, Hit, tree};

const SCREEN: egui::Vec2 = vec2(1600.0, 1000.0);

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
    screen: egui::Vec2,
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

impl Harness {
    /// A window with one document open.
    fn new() -> Self {
        let mut h = Self::without_document();
        h.app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        h.settle();
        h
    }

    /// A window with no document (Home).
    fn without_document() -> Self {
        Self::sized_without_document(SCREEN)
    }

    /// A window `screen` big with no document (Home).
    fn sized_without_document(screen: egui::Vec2) -> Self {
        let app = VectorcraftApp::new(Session::new(), Default::default());
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self { app, ctx, time: 0.0, screen };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.frame_full(events);
    }

    /// One frame; all it output.
    fn frame_full(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.05;
        let screen = Rect::from_min_size(Pos2::ZERO, self.screen);
        let input = egui::RawInput { time: Some(self.time), events, screen_rect: Some(screen), ..Default::default() };
        let app = &mut self.app;
        app.begin_host_iteration();
        let mut out = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
        out
    }

    /// The selected objects and their bounds (as text).
    fn selection(&self) -> (usize, String) {
        let st = self.app.session.active().unwrap();
        (st.selection.objects.len(), format!("{:?}", st.doc.bounds_of(&st.selection.objects, true)))
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    /// Preferences › User Interface › Varos shell (new) set to `on` and OK'd.
    fn shell(&mut self, on: bool) {
        crate::prefs_dialog::open(&mut self.app, Some("User Interface"));
        let d = self.app.ui.dialog.as_mut().unwrap();
        assert_eq!(d.fields.get(shell::PREF_KEY), Some(&json!(self.app.ui.varos_shell.enabled)), "the dialog shows the switch's state");
        d.fields.insert(shell::PREF_KEY.into(), json!(on));
        crate::prefs_dialog::confirm(&mut self.app).unwrap();
        assert_eq!(shell::enabled(&self.app), on);
        self.settle();
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.settle();
    }

    fn hits(&self) -> Vec<(Hit, Rect)> {
        shell::hit_rects(&self.ctx)
    }

    fn rect(&self, hit: &Hit) -> Rect {
        self.hits().into_iter().find_map(|(h, r)| (h == *hit).then_some(r)).unwrap_or_else(|| panic!("{hit:?} is drawn"))
    }

    fn board(&self) -> Rect {
        self.rect(&Hit::Board)
    }

    /// The panels with a tab drawn, in drawing order.
    fn tabs(&self) -> Vec<String> {
        self.hits()
            .into_iter()
            .filter_map(|(h, _)| match h {
                Hit::Tab { panel, .. } => Some(panel),
                _ => None,
            })
            .collect()
    }

    fn tab(&self, panel: &str) -> Rect {
        self.hits()
            .into_iter()
            .find_map(|(h, r)| matches!(&h, Hit::Tab { panel: p, .. } if p == panel).then_some(r))
            .unwrap_or_else(|| panic!("{panel}'s tab is drawn"))
    }

    fn tree(&self) -> &tree::Node {
        &self.app.ui.varos_shell.tree
    }

    fn front(&self, box_id: u32) -> Option<&str> {
        self.tree().find_box(box_id).and_then(tree::PanelBox::front)
    }
}

#[test]
fn the_switch_is_off_by_default_and_the_dock_draws() {
    let h = Harness::new();
    assert!(!shell::enabled(&h.app));
    assert!(crate::panel_docking::valid(&crate::panel_docking::layout_of(&h.app.ui)));
    assert!(h.hits().is_empty(), "the shell draws nothing while off");
    assert!(!crate::panel_docking::hit_rects(&h.ctx).is_empty(), "the dock draws");
}

#[test]
fn the_board_holds_the_canvas_beside_the_two_default_boxes() {
    let mut h = Harness::new();
    h.shell(true);
    let board = h.board();
    let canvas = h.app.canvas_rect.expect("the canvas was drawn");
    assert!(board.contains_rect(canvas), "the canvas lies in the Board: {canvas:?} in {board:?}");
    // The Board body reserves the full strip and its seam, plus document tabs, rulers and scroll bars.
    assert!(board.right() - canvas.right() <= 54.0 + tree::SEAM && board.bottom() - canvas.bottom() <= 20.0, "{canvas:?} in {board:?}");
    assert!(canvas.top() - board.top() <= 80.0 && canvas.left() - board.left() <= 30.0, "{canvas:?} in {board:?}");
    assert_eq!(h.rect(&Hit::Strip).width(), 34.0);
    assert!(h.rect(&Hit::Strip).left() - canvas.right() >= tree::SEAM);
    assert_eq!(h.rect(&Hit::BoardBody), canvas, "the canvas body is published apart from the Board");
    assert_eq!(h.tabs(), ["layers", "artboards", "properties", "color", "swatches"]);
    assert_eq!((h.front(2), h.front(3)), (Some("layers"), Some("properties")));
    // Board | right column at 72 %, one seam between, the right column split in two.
    let right = h.rect(&Hit::Box(2)).union(h.rect(&Hit::Box(3)));
    assert!((right.left() - board.right() - tree::SEAM).abs() < 0.5);
    assert!(((board.width()) / (board.width() + right.width()) - tree::BOARD_RATIO).abs() < 0.01);
    assert!((h.rect(&Hit::Box(3)).top() - h.rect(&Hit::Box(2)).bottom() - tree::SEAM).abs() < 0.5);
}

#[test]
fn a_click_on_a_tab_switches_the_box() {
    let mut h = Harness::new();
    h.shell(true);
    let at = h.tab("artboards").center();
    h.click(at);
    assert_eq!(h.front(2), Some("artboards"));
    let at = h.tab("swatches").center();
    h.click(at);
    assert_eq!(h.front(3), Some("swatches"));
    assert_eq!(h.front(2), Some("artboards"), "the other box keeps its tab");
    let at = h.tab("layers").center();
    h.click(at);
    assert_eq!(h.front(2), Some("layers"));
}

#[test]
fn window_panel_opens_brings_forward_and_closes_tabs() {
    let mut h = Harness::new();
    h.shell(true);
    let checked = |app: &VectorcraftApp, p: &str| crate::menus::checked(app, "window.panel", &json!({"panel": p}));
    assert_eq!(checked(&h.app, "stroke"), Some(false));
    h.app.run("window.panel", json!({"panel": "stroke"})).unwrap();
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
    assert_eq!(checked(&h.app, "stroke"), Some(true));
    h.settle();
    assert!(shell::showing(&h.app, "stroke"));
    // Again without toggle: it only comes forward.
    h.app.run("window.panel", json!({"panel": "properties"})).unwrap();
    assert_eq!(h.front(3), Some("properties"));
    h.app.run("window.panel", json!({"panel": "stroke"})).unwrap();
    assert_eq!(h.tree().find_tab("stroke"), None);
    // The Window menu's item toggles: it closes.
    h.app.run("window.panel", json!({"panel": "stroke", "toggle": true})).unwrap();
    assert_eq!(h.tree().find_tab("stroke"), None);
    h.settle();
    assert!(!shell::showing(&h.app, "stroke"));
    // … and opens again.
    h.app.run("window.panel", json!({"panel": "Stroke", "toggle": true})).unwrap();
    assert_eq!(h.tree().find_tab("stroke"), None);
    assert!(h.app.run("window.panel", json!({"panel": "noSuchPanel"})).is_err());
    // Closing both of the top box's tabs: the bottom box takes the whole column.
    let column = h.rect(&Hit::Box(2)).union(h.rect(&Hit::Box(3)));
    h.app.run("window.panel.close", json!({"panel": "layers"})).unwrap();
    h.app.run("window.panel.close", json!({"panel": "artboards"})).unwrap();
    h.settle();
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::Box(2)));
    let rest = h.rect(&Hit::Box(3));
    assert!((rest.top() - column.top()).abs() < 0.5 && (rest.bottom() - column.bottom()).abs() < 0.5, "{rest:?} fills {column:?}");
}

#[test]
fn a_divider_drag_resizes_in_the_same_frame_and_a_double_click_resets() {
    let mut h = Harness::new();
    h.shell(true);
    let board0 = h.board();
    let canvas0 = h.app.canvas_rect.unwrap();
    let from = h.rect(&Hit::Divider(vec![])).center();
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    for k in 1..=6 {
        let dx = -20.0 * k as f32;
        h.frame(vec![egui::Event::PointerMoved(from + vec2(dx, 0.0))]);
        // The frame that moved the pointer drew the Board and the canvas at the new size.
        let board = h.board();
        assert!((board.right() - (board0.right() + dx)).abs() <= 1.0, "step {k}: {board:?} from {board0:?}");
        let canvas = h.app.canvas_rect.unwrap();
        assert!((canvas.right() - (canvas0.right() + dx)).abs() <= 1.0, "step {k}: {canvas:?}");
    }
    let to = from + vec2(-120.0, 0.0);
    h.frame(vec![button(to, false)]);
    h.settle();
    let ratio = h.tree().ratio(&[]).unwrap();
    assert!(ratio < tree::BOARD_RATIO - 0.05, "{ratio}");
    assert!((h.board().right() - (board0.right() - 120.0)).abs() <= 1.0);
    // A double-click on the divider puts the default back.
    let at = h.rect(&Hit::Divider(vec![])).center();
    h.frame(vec![egui::Event::PointerMoved(at)]);
    h.frame(vec![button(at, true)]);
    h.frame(vec![button(at, false)]);
    h.frame(vec![button(at, true)]);
    h.frame(vec![button(at, false)]);
    h.settle();
    assert_eq!(h.tree().ratio(&[]), Some(tree::BOARD_RATIO));
    assert_eq!(h.board(), board0);
}

#[test]
fn dividers_stop_at_the_boxes_minimum_sizes() {
    let mut h = Harness::new();
    h.shell(true);
    let drag = |h: &mut Harness, from: Pos2, to: Pos2| {
        h.frame(vec![egui::Event::PointerMoved(from)]);
        h.frame(vec![button(from, true)]);
        for k in 1..=8 {
            h.frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0))]);
        }
        h.frame(vec![button(to, false)]);
        h.settle();
    };
    let from = h.rect(&Hit::Divider(vec![])).center();
    drag(&mut h, from, pos2(SCREEN.x + 200.0, from.y));
    assert!((h.rect(&Hit::Box(3)).width() - tree::BOX_MIN.x).abs() <= 1.0, "{:?}", h.rect(&Hit::Box(3)));
    let from = h.rect(&Hit::Divider(vec![])).center();
    drag(&mut h, from, pos2(-200.0, from.y));
    assert!((h.board().width() - tree::BOARD_MIN.x).abs() <= 1.0, "{:?}", h.board());
    let from = h.rect(&Hit::Divider(vec![1])).center();
    drag(&mut h, from, pos2(from.x, -100.0));
    assert!((h.rect(&Hit::Box(2)).height() - tree::BOX_MIN.y).abs() <= 1.0, "{:?}", h.rect(&Hit::Box(2)));
}

#[test]
fn switching_off_brings_the_dock_back_and_both_layouts_persist() {
    let mut h = Harness::new();
    let dock_before = crate::panel_docking::layout_of(&h.app.ui);
    h.shell(true);
    h.app.run("window.panel", json!({"panel": "stroke"})).unwrap();
    h.settle();
    h.shell(false);
    assert!(h.app.ui.docking.is_none() || crate::panel_docking::valid(&crate::panel_docking::layout_of(&h.app.ui)));
    assert_eq!(crate::panel_docking::layout_of(&h.app.ui), dock_before, "the shell never touched the dock's layout");
    assert!(crate::panel_docking::hit_rects(&h.ctx).iter().any(|(hit, _)| matches!(hit, craft_ui::docking::Hit::Tab(p) if p == "properties")));
    // Off, `window.panel` is the dock's again (an icon panel opens as its card).
    h.app.run("window.panel", json!({"panel": "gradient"})).unwrap();
    h.settle();
    assert!(h.app.ui.varos_panel_stack.cards.iter().any(|c| c == "gradient"));
    assert_eq!(h.tree().find_tab("gradient"), None);
    // On again: the strip ownership persists; open flyouts are transient.
    h.shell(true);
    assert_eq!(h.tree().find_tab("stroke"), None);
    assert!(h.app.ui.varos_shell.strip.group("stroke").is_some());
}

#[test]
fn home_and_the_full_screen_modes_show_the_board_alone() {
    let mut h = Harness::without_document();
    h.shell(true);
    assert!(crate::varos::home::is_home(&h.app));
    let hits = h.hits();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, Hit::Board);
    h.app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
    h.settle();
    let area = h.hits().iter().map(|(_, r)| *r).reduce(|a, b| a.union(b)).unwrap().expand(shell::OUTER);
    for mode in 1..=3u8 {
        h.app.ui.screen_mode = mode;
        h.settle();
        let hits = h.hits();
        assert!(hits.iter().all(|(hit, _)| matches!(hit, Hit::Board | Hit::BoardBody)), "mode {mode}: {hits:?}");
        assert!(h.board().contains_rect(h.rect(&Hit::BoardBody)), "mode {mode}");
        if mode == 1 {
            assert_eq!(h.board(), area, "Full Screen Mode with Menu Bar: the Board fills the central region");
        }
    }
    h.app.ui.screen_mode = 0;
    h.settle();
    assert_eq!(h.tabs().len(), 5);
}

#[test]
fn dock_and_workspace_commands_change_nothing_in_the_shell() {
    let mut h = Harness::new();
    h.shell(true);
    let before = h.app.ui.varos_shell.clone();
    let (dock, toolbar, workspace) = (h.app.ui.dock, h.app.ui.toolbar, h.app.ui.workspace.clone());
    for (id, params) in [
        ("window.dock", json!({})),
        ("window.panels", json!({})),
        ("window.collapseDock", json!({})),
        ("window.workspace", json!({"name": "Essentials Classic"})),
        ("window.panel.float", json!({"panel": "layers"})),
    ] {
        h.app.ui.status.clear();
        assert!(h.app.run(id, params).is_ok(), "{id}");
        assert!(h.app.ui.status.contains("not in the Varos shell yet"), "{id}: {}", h.app.ui.status);
    }
    h.settle();
    assert_eq!(h.app.ui.varos_shell, before);
    assert_eq!((h.app.ui.dock, h.app.ui.toolbar, h.app.ui.workspace.clone()), (dock, toolbar, workspace));
}

#[test]
fn the_switch_round_trips_through_preferences_and_ui_json() {
    let mut h = Harness::new();
    h.shell(true);
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    assert_eq!(saved["varos_shell"]["enabled"], json!(true));
    let back: crate::state::UiState = serde_json::from_value(saved).unwrap();
    assert!(back.varos_shell.enabled);
    assert_eq!(back.varos_shell.tree, h.app.ui.varos_shell.tree);
    h.shell(false);
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    assert_eq!(saved["varos_shell"]["enabled"], json!(false));
}

#[test]
fn code_that_sets_open_panel_opens_the_strip_flyout() {
    let mut h = Harness::new();
    h.shell(true);
    // As the control bar's Transparency link, Pattern Options or a workspace preset do.
    h.app.ui.open_panel = Some("stroke".into());
    h.frame(vec![]);
    assert_eq!(h.tree().find_tab("stroke"), None);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("stroke"));
    assert!(h.hits().iter().any(|(hit, _)| *hit == Hit::Flyout), "drawn in the same frame");
    // A panel already in a box comes to the front there.
    h.app.ui.open_panel = Some("artboards".into());
    h.frame(vec![]);
    assert_eq!(h.front(2), Some("artboards"));
    assert_eq!(h.tree().find_tab("artboards"), Some(2));
    // A label works as `window.panel`'s does; an unknown name is dropped.
    h.app.ui.open_panel = Some("Transparency".into());
    h.frame(vec![]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("transparency"));
    h.app.ui.open_panel = Some("noSuchPanel".into());
    h.frame(vec![]);
    assert_eq!(h.app.ui.open_panel, None);
    h.app.ui.varos_shell.tree.invariants().unwrap();
}

#[test]
fn hit_rects_describe_the_frame_drawn_and_clear_when_off() {
    let mut h = Harness::new();
    h.shell(true);
    let board0 = h.board();
    let seam0 = h.rect(&Hit::Divider(vec![]));
    let from = seam0.center();
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    for k in 1..=4 {
        let dx = -25.0 * k as f32;
        h.frame(vec![egui::Event::PointerMoved(from + vec2(dx, 0.0))]);
        // The seam and the boxes as drawn in this drag frame, not the frame before.
        let seam = h.rect(&Hit::Divider(vec![]));
        assert!((seam.center().x - (from.x + dx)).abs() <= 1.0, "step {k}: {seam:?}");
        assert!((seam.left() - h.board().right()).abs() < 0.5, "step {k}");
        assert!((h.rect(&Hit::Divider(vec![1])).left() - seam.right()).abs() < 0.5, "step {k}: the right column's seam moved too");
    }
    h.frame(vec![button(from + vec2(-100.0, 0.0), false)]);
    // The release frame of a double-click: reset and published at once.
    let at = h.rect(&Hit::Divider(vec![])).center();
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    h.frame(vec![button(at, false)]);
    h.frame(vec![button(at, true)]);
    h.frame(vec![button(at, false)]);
    assert_eq!(h.rect(&Hit::Divider(vec![])), seam0);
    assert_eq!(h.board(), board0);
    // Off: the shell drew nothing, so it has no controls.
    crate::varos::shell::set_enabled(&mut h.app, false);
    h.frame(vec![]);
    assert!(h.hits().is_empty(), "{:?}", h.hits());
    crate::varos::shell::set_enabled(&mut h.app, true);
    h.frame(vec![]);
    assert!(!h.hits().is_empty());
}

#[test]
fn a_clicked_tab_is_lit_on_its_release_frame() {
    use egui::accesskit::{Role, Toggled};
    let mut h = Harness::new();
    h.shell(true);
    h.ctx.enable_accesskit();
    h.settle();
    let at = h.tab("artboards").center();
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    let out = h.frame_full(vec![button(at, false)]);
    assert_eq!(h.front(2), Some("artboards"));
    let nodes = out.platform_output.accesskit_update.expect("accesskit is on").nodes;
    let selected = |label: &str| {
        nodes
            .iter()
            .find(|(_, n)| n.role() == Role::Button && n.label() == Some(label))
            .map(|(_, n)| n.toggled())
            .unwrap_or_else(|| panic!("{label}'s tab"))
    };
    assert_eq!(selected("Artboards"), Some(Toggled::True), "lit in the frame of the click");
    assert_eq!(selected("Layers"), Some(Toggled::False));
    // The pills published are the ones painted (the front pill is semibold, so wider).
    let wide = h.tab("artboards").width();
    h.settle();
    assert_eq!(h.tab("artboards").width(), wide);
}

#[test]
fn the_seams_show_the_resize_cursor() {
    let mut h = Harness::new();
    h.shell(true);
    let main = h.rect(&Hit::Divider(vec![])).center();
    let out = h.frame_full(vec![egui::Event::PointerMoved(main)]);
    let out = if out.platform_output.cursor_icon == egui::CursorIcon::Default { h.frame_full(vec![]) } else { out };
    assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal);
    let column = h.rect(&Hit::Divider(vec![1])).center();
    h.frame(vec![egui::Event::PointerMoved(column)]);
    let out = h.frame_full(vec![]);
    assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::ResizeVertical);
    let inside = h.rect(&Hit::Box(3)).center();
    h.frame(vec![egui::Event::PointerMoved(inside)]);
    let out = h.frame_full(vec![]);
    assert!(!matches!(out.platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal | egui::CursorIcon::ResizeVertical));
}

#[test]
fn presses_on_seams_and_tab_rows_never_reach_the_canvas() {
    let mut h = Harness::new();
    h.app.run("shape.rectangle", json!({"x": 40, "y": 40, "width": 120, "height": 80})).unwrap();
    h.shell(true);
    let selected = h.selection();
    assert_eq!(selected.0, 1, "the new rectangle is selected");
    let drag = |h: &mut Harness, from: Pos2, to: Pos2| {
        h.frame(vec![egui::Event::PointerMoved(from)]);
        h.frame(vec![button(from, true)]);
        for k in 1..=4 {
            h.frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 4.0))]);
        }
        h.frame(vec![button(to, false)]);
        h.settle();
    };
    // Every seam, pressed and dragged toward the canvas.
    for path in [vec![], vec![1]] {
        let seam = h.rect(&Hit::Divider(path.clone())).center();
        drag(&mut h, seam, seam + vec2(-40.0, -40.0));
        assert_eq!(h.selection(), selected, "seam {path:?}");
    }
    // Each tab row's empty end, pressed and dragged over the Board.
    for id in [2, 3] {
        let row = h.rect(&Hit::TabRow(id));
        let last = h
            .hits()
            .iter()
            .filter(|(hit, _)| matches!(hit, Hit::Tab { box_id, .. } if *box_id == id))
            .map(|(_, r)| r.right())
            .fold(row.left(), f32::max);
        assert!(row.right() - last > 10.0, "row {id} has an empty end");
        let at = pos2(row.right() - 4.0, row.center().y);
        let to = h.board().center();
        drag(&mut h, at, to);
        assert_eq!(h.selection(), selected, "tab row {id}");
    }
    // The check bites: a click on the canvas's empty pasteboard deselects.
    let body = h.rect(&Hit::BoardBody);
    h.click(body.min + vec2(8.0, 8.0));
    assert_eq!(h.selection().0, 0, "a click on the canvas reaches it");
}

/// Software capture (no GPU) of the window as the app draws it into a fresh dark-themed context:
/// 6 frames, rasterized; RGBA pixels.
fn capture(app: &mut VectorcraftApp, width: u32, height: u32) -> Vec<u8> {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    app.fonts_ready = true;
    app.canvas.worker_started = true;
    let mut textures = std::collections::HashMap::<egui::TextureId, egui::ColorImage>::new();
    let mut shot = None;
    for frame in 0..6 {
        let raw = egui::RawInput {
            time: Some(100.0 + 0.05 * frame as f64),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height as f32))),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        app.begin_host_iteration();
        let mut out = ctx.run_ui(raw, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                if let Some([x, y]) = delta.pos {
                    let target = textures.get_mut(id).unwrap();
                    for row in 0..image.size[1] {
                        for col in 0..image.size[0] {
                            target.pixels[(y + row) * target.size[0] + x + col] = image.pixels[row * image.size[0] + col];
                        }
                    }
                } else {
                    textures.insert(*id, (**image).clone());
                }
            }
        }
        out.textures_delta.clear();
        shot = Some(ctx.tessellate(out.shapes, out.pixels_per_point));
    }
    crate::varos::home::capture::raster(&shot.unwrap(), &textures, width, height)
}

/// Software capture (no GPU) of the window with the Varos shell on, a selected rectangle (the
/// control bar shows) and the five default tabs: `VAROS_SHELL_CAPTURE=<file.png>`; a second
/// capture, after the main seam is dragged 200 points left, goes to `<file>-dragged.png`.
#[test]
#[ignore = "optional software visual capture; set VAROS_SHELL_CAPTURE"]
fn capture_shell() {
    let Some(file) = std::env::var_os("VAROS_SHELL_CAPTURE").map(std::path::PathBuf::from) else { return };
    let (width, height) = (1440_u32, 900_u32);
    let mut h = Harness::sized_without_document(vec2(width as f32, height as f32));
    h.app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
    h.app.ui.control_bar = true; // as the owner's ui.json
    h.app.run("shape.rectangle", json!({"x": 40, "y": 40, "width": 120, "height": 80})).unwrap();
    h.settle();
    h.shell(true);
    assert_eq!(h.tabs(), ["layers", "artboards", "properties", "color", "swatches"]);
    if std::env::var_os("VAROS_SHELL_CAPTURE_FLYOUT").is_some() {
        h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    }
    let pixels = capture(&mut h.app, width, height);
    image::save_buffer(&file, &pixels, width, height, image::ColorType::Rgba8).unwrap();
    // The main seam dragged 200 points left.
    let board = h.board();
    let from = h.rect(&Hit::Divider(vec![])).center();
    let to = from - vec2(200.0, 0.0);
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    for k in 1..=8 {
        h.frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0))]);
    }
    h.frame(vec![button(to, false)]);
    h.settle();
    assert!((h.board().right() - (board.right() - 200.0)).abs() <= 1.0, "{:?} from {board:?}", h.board());
    let pixels = capture(&mut h.app, width, height);
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("capture");
    let dragged = file.with_file_name(format!("{stem}-dragged.png"));
    image::save_buffer(&dragged, &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

#[test]
fn strip_defaults_migrate_and_ownership_moves() {
    let tree = tree::Node::default_layout();
    let mut shell = shell::persist::from_value(&json!({"version":1,"enabled":true,"tree":tree}));
    assert_eq!(shell.version, 2);
    let expected: Vec<_> = crate::state::ICON_PANEL_GROUPS.iter().flat_map(|g| g.iter()).filter(|p| tree.find_tab(p).is_none()).copied().collect();
    assert_eq!(shell.strip.groups.iter().flat_map(|g| g.panels.iter().map(String::as_str)).collect::<Vec<_>>(), expected);
    assert!(shell.add_tab(3, "stroke"));
    assert!(shell.strip.group("stroke").is_none());
    assert!(shell.close_tab("stroke"));
    assert_eq!(shell.strip.groups.last().unwrap().panels.last().unwrap(), "stroke");
}

#[test]
fn strip_click_switch_close_resize_and_persist() {
    let mut h = Harness::new();
    h.shell(true);
    let stroke = h.rect(&Hit::StripIcon("stroke".into())).center();
    h.click(stroke);
    let flyout = h.rect(&Hit::Flyout);
    assert!(h.board().contains_rect(flyout));
    assert!((flyout.right() - h.rect(&Hit::Strip).left()).abs() < 1.0);
    let group = h.app.ui.varos_shell.strip.group("stroke").unwrap();
    assert!((flyout.top() - h.rect(&Hit::StripGroup(group)).top()).abs() < 1.0);
    for p in ["stroke", "gradient", "transparency"] {
        h.rect(&Hit::FlyoutTab(p.into()));
    }
    h.click(stroke);
    assert_eq!(h.app.ui.open_panel, None);
    h.click(stroke);
    let gradient = h.rect(&Hit::StripIcon("gradient".into())).center();
    h.frame(vec![egui::Event::PointerMoved(gradient), button(gradient, true)]);
    h.frame(vec![button(gradient, false)]);
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("gradient"));
    h.settle();
    let widths: Vec<_> = h.app.ui.varos_shell.strip.groups.iter().map(|g| g.width).collect();
    let edge = h.rect(&Hit::FlyoutResize).center();
    h.frame(vec![egui::Event::PointerMoved(edge)]);
    h.frame(vec![button(edge, true)]);
    h.frame(vec![egui::Event::PointerMoved(edge - vec2(40.0, 0.0))]);
    assert!(h.rect(&Hit::Flyout).width() > flyout.width() + 30.0);
    h.frame(vec![button(edge - vec2(40.0, 0.0), false)]);
    for (i, g) in h.app.ui.varos_shell.strip.groups.iter().enumerate() {
        if i != group {
            assert_eq!(g.width, widths[i]);
        }
    }
    h.settle();
    let at = h.rect(&Hit::Flyout).center();
    h.frame(vec![
        egui::Event::PointerMoved(at),
        egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() },
    ]);
    assert_eq!(h.app.ui.open_panel, None);
    let toggle = h.rect(&Hit::StripToggle).center();
    h.frame(vec![egui::Event::PointerMoved(toggle), button(toggle, true)]);
    h.frame(vec![button(toggle, false)]);
    assert!(h.rect(&Hit::Strip).width() > 34.0);
    assert!(h.rect(&Hit::Strip).width() <= 180.0);
    let saved = serde_json::to_value(&h.app.ui.varos_shell).unwrap();
    let back = shell::persist::from_value(&saved);
    assert_eq!(back.strip, h.app.ui.varos_shell.strip);
    assert!(back.open.is_none());
    let toggle = h.rect(&Hit::StripToggle).center();
    h.click(toggle);
    assert_eq!(h.rect(&Hit::Strip).width(), 34.0);
}

#[test]
fn flyout_blocks_canvas_and_window_toggle_tracks_it() {
    let mut h = Harness::new();
    h.app.run("shape.rectangle", json!({"x":40,"y":40,"width":120,"height":80})).unwrap();
    h.shell(true);
    let selected = h.selection();
    assert_eq!(selected.0, 1, "canvas blocking requires a selected object");
    for shown in [true, false, true] {
        h.app.run("window.panel", json!({"panel":"stroke","toggle":true})).unwrap();
        assert_eq!(shell::showing(&h.app, "stroke"), shown);
    }
    h.settle();
    let at = h.rect(&Hit::Flyout).max - vec2(15.0, 15.0);
    h.click(at);
    assert_eq!(h.selection(), selected);
    let at = h.rect(&Hit::FlyoutClose).center();
    h.click(at);
    assert!(h.app.ui.open_panel.is_none());
}

#[test]
fn flyout_outside_click_obeys_preference_and_escape_stays_local() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.settle();
    let outside = h.rect(&Hit::BoardBody).min + vec2(10.0, 10.0);
    h.app.session.prefs.auto_collapse_icon_panels = false;
    h.click(outside);
    assert!(shell::showing(&h.app, "stroke"));
    h.frame(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    assert!(shell::showing(&h.app, "stroke"), "Escape over the canvas belongs to the canvas");
    h.app.session.prefs.auto_collapse_icon_panels = true;
    h.click(outside);
    assert!(!shell::showing(&h.app, "stroke"));
    assert!(h.app.ui.open_panel.is_none());
}

#[test]
fn strip_load_drops_duplicate_and_unknown_owners_and_resets_open_state() {
    let mut state = shell::persist::VarosShell { open: Some("stroke".into()), ..Default::default() };
    state.strip.groups[0].panels.extend(["stroke".into(), "layers".into(), "unknown-panel".into()]);
    state.strip.groups[0].width = -20.0;
    let saved = serde_json::to_value(&state).unwrap();
    let loaded = shell::persist::from_value(&saved);
    let ids: Vec<_> = loaded.strip.groups.iter().flat_map(|g| &g.panels).collect();
    assert_eq!(ids.iter().filter(|p| p.as_str() == "stroke").count(), 1);
    assert!(!ids.iter().any(|p| p.as_str() == "layers" || p.as_str() == "unknown-panel"));
    assert!(loaded.open.is_none());
    assert_eq!(loaded.strip.groups[0].width, 190.0);
}

fn escape_key() -> egui::Event {
    egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }
}

#[test]
fn flyout_first_open_fits_stroke_and_area_matches_visible_bounds() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.frame(vec![]);
    let rect = h.rect(&Hit::Flyout);
    let content = h.ctx.data(|d| d.get_temp::<egui::Vec2>(egui::Id::unique("shell-flyout-content"))).unwrap();
    assert!(content.x <= rect.width() - 7.0 + 1.0, "content {content:?}, flyout {rect:?}");
    let state = egui::AreaState::load(&h.ctx, egui::Id::unique("shell-flyout")).unwrap();
    assert_eq!(state.size, Some(rect.size()));
    let index = h.app.ui.varos_shell.strip.group("stroke").unwrap();
    assert_eq!(h.app.ui.varos_shell.strip.groups[index].width, rect.width());
    h.frame(vec![]);
    assert_eq!(h.rect(&Hit::Flyout), rect, "no visible sizing motion");
}

#[test]
fn flyout_clicked_icon_and_tab_are_active_on_release() {
    use egui::accesskit::{Role, Toggled};
    let mut h = Harness::new();
    h.shell(true);
    h.ctx.enable_accesskit();
    h.settle();
    let at = h.rect(&Hit::StripIcon("gradient".into())).center();
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    let out = h.frame_full(vec![button(at, false)]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("gradient"));
    let nodes = out.platform_output.accesskit_update.unwrap().nodes;
    let active: Vec<_> = nodes.iter().filter(|(_, n)| n.role() == Role::Button && n.label() == Some("Gradient")).collect();
    assert_eq!(active.len(), 2, "icon and flyout tab");
    assert!(active.iter().all(|(_, n)| n.toggled() == Some(Toggled::True)));
    let at = h.rect(&Hit::FlyoutTab("transparency".into())).center();
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    let out = h.frame_full(vec![button(at, false)]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("transparency"));
    let nodes = out.platform_output.accesskit_update.unwrap().nodes;
    assert!(
        nodes.iter().filter(|(_, n)| n.role() == Role::Button && n.label() == Some("Transparency")).all(|(_, n)| n.toggled() == Some(Toggled::True))
    );
}

#[test]
fn close_commands_return_box_tabs_and_remove_temporary_panels() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"layers","toggle":true})).unwrap();
    assert!(h.tree().find_tab("layers").is_none());
    assert!(h.app.ui.varos_shell.strip.group("layers").is_some());
    for panel in ["stroke", "docInfo"] {
        h.app.run("window.panel", json!({"panel":panel})).unwrap();
        h.frame(vec![]);
        if panel == "docInfo" {
            assert!(h.app.ui.varos_shell.strip.groups.last().unwrap().temporary);
            assert!(!h.app.ui.varos_shell.strip.groups.last().unwrap().panels.contains(&"stroke".into()));
        }
        let out = h.app.run("window.panel.close", json!({"panel":panel})).unwrap();
        assert_eq!(out["closed"], true);
        assert!(h.app.ui.varos_shell.open.is_none());
        assert_eq!(h.app.ui.varos_shell.strip.group(panel).is_some(), panel == "stroke");
    }
}

#[test]
fn home_and_fullscreen_close_and_refuse_flyouts() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.frame(vec![]);
    h.app.ui.screen_mode = 1;
    h.frame(vec![]);
    assert!(h.app.ui.varos_shell.open.is_none());
    let out = h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    assert_eq!(out["shown"], false);
    assert!(h.app.ui.status.contains("hidden"));
    let mut home = Harness::without_document();
    home.shell(true);
    shell::strip::open(&mut home.app, "stroke");
    home.frame(vec![]);
    assert!(home.app.ui.varos_shell.open.is_none());
    let out = home.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    assert_eq!(out["shown"], false);
}

#[test]
fn sanitize_removes_gaps_and_restores_missing_default_owners() {
    let tree = tree::Node::default_layout();
    let mut strip = shell::strip::Strip::default_for(&tree);
    strip.remove("stroke");
    strip.groups[0].panels.clear();
    strip.sanitize(&tree);
    assert!(strip.groups.iter().all(|g| !g.panels.is_empty()));
    for p in crate::state::ICON_PANEL_GROUPS.iter().flat_map(|g| g.iter()) {
        assert!(tree.find_tab(p).is_some() || strip.group(p).is_some(), "{p}");
    }
}

#[test]
fn flyout_near_bottom_moves_up_without_shrinking() {
    let mut h = Harness::new();
    h.shell(true);
    // Put Stroke last, below all other icons, while keeping the rail scrollable.
    h.app.ui.varos_shell.strip.remove("stroke");
    h.app.ui.varos_shell.strip.append("stroke");
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.frame(vec![]);
    let first = h.rect(&Hit::Flyout);
    let group = h.app.ui.varos_shell.strip.group("stroke").unwrap();
    let top = h.rect(&Hit::StripGroup(group)).top();
    assert!(top + first.height() > h.board().bottom());
    assert!((first.bottom() - h.board().bottom()).abs() < 1.0);
    assert!(first.top() < top);
    h.frame(vec![]);
    assert_eq!(h.rect(&Hit::Flyout), first);
}

#[test]
fn escape_yields_to_dialog_palette_popup_and_focused_text() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.settle();
    let at = h.rect(&Hit::Flyout).center();
    h.app.ui.palette_open = true;
    h.frame(vec![egui::Event::PointerMoved(at), escape_key()]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
    crate::prefs_dialog::open(&mut h.app, Some("User Interface"));
    h.frame(vec![escape_key()]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
    h.app.ui.dialog = None;
    h.app.ui.palette_open = false;
    h.settle();
    egui::Popup::open_id(&h.ctx, egui::Id::unique("shell-test-popup"));
    h.frame(vec![egui::Event::PointerMoved(at), escape_key()]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
    egui::Popup::close_all(&h.ctx);
    // A real text editor on the flyout layer owns Escape before shortcuts run.
    let id = egui::Id::unique("shell-test-text");
    let mut text = String::from("editing");
    for events in [vec![], vec![escape_key()]] {
        h.app.begin_host_iteration();
        let _ = h.ctx.run_ui(egui::RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), ..Default::default() }, |ui| {
            egui::Area::new(egui::Id::unique("shell-flyout")).fixed_pos(at).show(ui.ctx(), |ui| {
                ui.add(egui::TextEdit::singleline(&mut text).id(id)).request_focus();
            });
            assert!(ui.ctx().egui_wants_keyboard_input());
            crate::shortcuts::handle(&mut h.app, ui.ctx());
        });
    }
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
}

#[test]
fn panel_body_request_is_followed_on_next_frame() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"appearance"})).unwrap();
    h.settle();
    let out = h.frame_full(vec![]);
    let flyout = h.rect(&Hit::Flyout);
    let at = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Stroke:" && flyout.contains(t.pos) => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        })
        .expect("Appearance's real Stroke link");
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    h.frame(vec![button(at, false)]);
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("stroke"));
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("appearance"), "body writes survive to the next frame");
    h.frame(vec![]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
    h.rect(&Hit::FlyoutTab("stroke".into()));
}

#[test]
fn flyouts_keep_their_first_presented_height_on_open_and_switch() {
    let mut h = Harness::new();
    h.shell(true);
    for panel in ["stroke", "gradient", "transparency", "properties", "layers", "libraries"] {
        if h.tree().find_tab(panel).is_some() {
            h.app.run("window.panel.close", json!({"panel":panel})).unwrap();
        }
        h.app.run("window.panel", json!({"panel":panel})).unwrap();
        h.frame(vec![]);
        let first = h.rect(&Hit::Flyout);
        for _ in 0..4 {
            h.frame(vec![]);
            assert_eq!(h.rect(&Hit::Flyout), first, "{panel} must not grow each frame");
        }
    }
}
