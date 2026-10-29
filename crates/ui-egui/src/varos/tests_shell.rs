//! The Varos shell, stages 1–4 ([`super::shell`]): through the real frame, with real clicks and
//! drags.

use egui::{Pos2, Rect, pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;
use crate::varos::shell::drag::{DragMode, Side, Slot, Target};
use crate::varos::shell::strip::StripRef;
use crate::varos::shell::{self, Hit, boxes, floats, tree};

const SCREEN: egui::Vec2 = vec2(1600.0, 1000.0);

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
    root: egui::Id,
    screen: egui::Vec2,
    /// The modifier keys held (sent as `ModifiersChanged` when they change).
    modifiers: egui::Modifiers,
    sent_modifiers: egui::Modifiers,
    /// The window has the keyboard focus.
    focused: bool,
}

fn run_ui_without_textures(ctx: &egui::Context, input: egui::RawInput, run: impl FnMut(&mut egui::Ui)) {
    let mut out = ctx.run_ui(input, run);
    out.textures_delta.clear();
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
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        // Stage 2–5 interaction regression on the historical arrangement (still supported).
        app.ui.varos_shell.tree = tree::Node::board_and_column(&["layers", "artboards"], &["properties", "color", "swatches"]);
        app.ui.varos_shell.strip = shell::strip::Strip::default_for(&app.ui.varos_shell.tree);
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut h = Self {
            app,
            ctx,
            root: egui::Id::NULL,
            time: 0.0,
            screen,
            modifiers: egui::Modifiers::NONE,
            sent_modifiers: egui::Modifiers::NONE,
            focused: true,
        };
        h.settle();
        h
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.frame_full(events);
    }

    /// One frame; all it output.
    fn frame_full(&mut self, mut events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.05;
        if self.modifiers != self.sent_modifiers {
            events.insert(0, egui::Event::ModifiersChanged(self.modifiers));
            self.sent_modifiers = self.modifiers;
        }
        let screen = Rect::from_min_size(Pos2::ZERO, self.screen);
        let input = egui::RawInput { time: Some(self.time), events, screen_rect: Some(screen), focused: self.focused, ..Default::default() };
        let app = &mut self.app;
        let root = &mut self.root;
        app.begin_host_iteration();
        let mut out = self.ctx.run_ui(input, |ui| {
            *root = ui.scope_id();
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

    fn state(&self) -> &shell::persist::VarosShell {
        &self.app.ui.varos_shell
    }

    /// Box `id`'s tabs, in order.
    fn box_tabs(&self, id: u32) -> Vec<String> {
        self.tree().find_box(id).map(|b| b.tabs.clone()).unwrap_or_default()
    }

    /// Press at `from` and move to `to` in eight frames, the button held.
    fn hold_drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for k in 1..=8 {
            self.frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0))]);
        }
    }

    /// Let go at `at`; the frames settle.
    fn release(&mut self, at: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at), button(at, false)]);
        self.settle();
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.hold_drag(from, to);
        self.release(to);
    }

    fn double_click(&mut self, at: Pos2) {
        // Well after any earlier click: two double-clicks in a row are never read as a triple.
        self.time += 1.0;
        self.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.frame(vec![button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.settle();
    }

    /// The drop line drawn mid-drag.
    fn drop_line(&self) -> (Hit, Rect) {
        self.hits().into_iter().find(|(h, _)| matches!(h, Hit::DropLine { .. })).unwrap_or_else(|| panic!("a drop line in {:?}", self.hits()))
    }

    /// Where the drag under way would land.
    fn drop_target(&self) -> Option<Target> {
        self.hits().into_iter().find_map(|(h, _)| match h {
            Hit::DropLine { target, .. } | Hit::DropAt(target) => Some(target),
            _ => None,
        })
    }

    /// A tab dragged to `to`, empty space: → the new floating panel's id.
    fn float_tab(&mut self, panel: &str, to: Pos2) -> u32 {
        let from = self.tab(panel).center();
        self.drag(from, to);
        self.state().float_of(panel).unwrap_or_else(|| panic!("{panel} floats"))
    }

    /// A Board strip icon dragged to `to`, empty space: → the lone icon's (floating strip's) id.
    fn lone_icon(&mut self, panel: &str, to: Pos2) -> u32 {
        let from = self.rect(&Hit::StripIcon(panel.into())).center();
        self.drag(from, to);
        self.state().float_strip_of(panel).unwrap_or_else(|| panic!("{panel} is a lone icon"))
    }

    /// Every box at least its minimum size (a collapsed one its row), the Board its own.
    fn assert_min_sizes(&self) {
        for (hit, r) in self.hits() {
            match hit {
                Hit::Box(id) if !self.tree().find_box(id).unwrap().collapsed => {
                    let min_w = self.tree().find_box(id).unwrap().tabs.iter().map(|p| tree::panel_min_width(p)).fold(180.0, f32::max);
                    assert!(r.width() >= min_w - 1.0 && r.height() >= tree::BOX_MIN.y - 1.0, "box {id}: {r:?}");
                }
                Hit::Board => assert!(r.width() >= tree::BOARD_MIN.x - 1.0 && r.height() >= tree::BOARD_MIN.y - 1.0, "Board: {r:?}"),
                _ => {}
            }
        }
    }
}

/// The azure rects a frame painted: (filled, outlined).
fn azure(out: &egui::FullOutput) -> (usize, usize) {
    let accent = crate::varos::theme::ACCENT;
    let rects = out.shapes.iter().filter_map(|s| match &s.shape {
        egui::Shape::Rect(r) => Some(r),
        _ => None,
    });
    rects.fold((0, 0), |(f, s), r| (f + usize::from(r.fill == accent), s + usize::from(r.stroke.width > 0.0 && r.stroke.color == accent)))
}

#[test]
fn the_shell_is_on_by_default_and_the_classic_dock_still_draws_when_off() {
    assert!(crate::state::UiState::default().varos_shell.enabled, "a fresh config is on");
    let mut h = Harness::new();
    assert!(!shell::enabled(&h.app), "the test app starts on the classic dock");
    assert!(h.hits().is_empty(), "the shell draws nothing while off");
    assert!(!crate::panel_docking::hit_rects(&h.ctx).is_empty(), "the dock draws");
    h.shell(true);
    assert!(shell::enabled(&h.app));
    assert!(!h.hits().is_empty(), "the shell draws");
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
    assert!(canvas.top() - board.top() <= 80.0 && canvas.left() - board.left() <= shell::tools::WIDTH + 30.0, "{canvas:?} in {board:?}");
    assert_eq!(h.rect(&Hit::Strip).width(), 40.0);
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
    // The Board and a vertical split still stop at their minimum; they do not collapse.
    let from = h.rect(&Hit::Divider(vec![1])).center();
    drag(&mut h, from, pos2(from.x, -100.0));
    assert!((h.rect(&Hit::Box(2)).height() - tree::BOX_MIN.y).abs() <= 1.0, "{:?}", h.rect(&Hit::Box(2)));
    let from = h.rect(&Hit::Divider(vec![])).center();
    drag(&mut h, from, pos2(-200.0, from.y));
    assert!((h.board().width() - tree::BOARD_MIN.x).abs() <= 1.0, "{:?}", h.board());
    // Dragging the right column to 50 pt clamps live at Properties' 300, then collapses it on release.
    let mut h = Harness::new();
    h.shell(true);
    let from = h.rect(&Hit::Divider(vec![])).center();
    let to = pos2(h.screen.x - 50.0, from.y);
    h.hold_drag(from, to);
    let col = h.rect(&Hit::Box(3));
    assert!((col.width() - 300.0).abs() <= 1.0, "the column stays at its minimum while the pointer is past it: {col:?}");
    assert!(h.hits().iter().any(|(hit, _)| matches!(hit, Hit::AzureHint)), "an azure hint while past the minimum");
    h.release(to);
    assert!(h.hits().iter().all(|(hit, _)| !matches!(hit, Hit::Box(3))), "the column collapsed");
    assert!(h.state().strip.group("properties").is_some() && h.state().strip.group("color").is_some());
    assert!(!h.state().collapsed_columns.is_empty());
}

#[test]
fn a_two_point_overshoot_does_not_collapse_the_column() {
    let mut h = Harness::new();
    h.shell(true);
    let from = h.rect(&Hit::Divider(vec![])).center();
    h.hold_drag(from, pos2(h.screen.x - 50.0, from.y));
    let clamped = h.rect(&Hit::Box(3)).left();
    let barely = pos2(clamped + 2.0, from.y);
    h.frame(vec![egui::Event::PointerMoved(barely)]);
    h.frame(vec![egui::Event::PointerMoved(barely)]);
    assert!(h.hits().iter().all(|(hit, _)| !matches!(hit, Hit::AzureHint)), "the hint starts at 16 pt past the minimum");
    assert!((h.rect(&Hit::Box(3)).width() - 300.0).abs() <= 1.0);
    h.release(barely);
    assert!(h.hits().iter().any(|(hit, _)| matches!(hit, Hit::Box(3))), "2 pt past the minimum does not collapse");
    assert!(h.state().collapsed_columns.is_empty());

    let mut h = Harness::new();
    h.shell(true);
    let from = h.rect(&Hit::Divider(vec![])).center();
    h.hold_drag(from, pos2(h.screen.x - 50.0, from.y));
    let clamped = h.rect(&Hit::Box(3)).left();
    let hint_at = pos2(clamped + 20.0, from.y);
    h.frame(vec![egui::Event::PointerMoved(hint_at)]);
    h.frame(vec![egui::Event::PointerMoved(hint_at)]);
    assert!(h.hits().iter().any(|(hit, _)| matches!(hit, Hit::AzureHint)), "the hint shows from 16 pt");
    h.release(hint_at);
    assert!(h.state().collapsed_columns.is_empty(), "20 pt past the minimum does not collapse");
    assert!(h.hits().iter().any(|(hit, _)| matches!(hit, Hit::Box(3))));
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
    // The workspace row runs along the top edge; the seam void is on the other three sides.
    let all = h.hits().iter().map(|(_, r)| *r).reduce(|a, b| a.union(b)).unwrap();
    let area = Rect::from_min_max(pos2(all.left() - shell::OUTER, all.top()), all.max + vec2(shell::OUTER, shell::OUTER));
    for mode in 1..=3u8 {
        h.app.ui.screen_mode = mode;
        h.settle();
        let hits = h.hits();
        assert!(
            hits.iter().all(|(hit, _)| {
                matches!(hit, Hit::Board | Hit::BoardBody) || (mode < 3 && matches!(hit, Hit::Tools | Hit::ToolsGrip | Hit::ToolsEdge))
            }),
            "mode {mode}: {hits:?}"
        );
        assert_eq!(hits.iter().any(|(hit, _)| *hit == Hit::Tools), mode < 3, "tools remain available until presentation mode");
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
fn dock_commands_change_nothing_in_the_shell() {
    let mut h = Harness::new();
    h.shell(true);
    let before = h.app.ui.varos_shell.clone();
    let (dock, toolbar, workspace) = (h.app.ui.dock, h.app.ui.toolbar, h.app.ui.workspace.clone());
    for (id, params) in [
        ("window.dock", json!({})),
        ("window.panels", json!({})),
        ("window.collapseDock", json!({})),
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
    capture_with(app, width, height, Vec::new())
}

/// [`capture`] with `script`'s events played after the 6 frames, one frame each, and 2 more frames
/// with none; the last is rasterized.
fn capture_with(app: &mut VectorcraftApp, width: u32, height: u32, script: Vec<Vec<egui::Event>>) -> Vec<u8> {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    app.fonts_ready = true;
    app.canvas.worker_started = true;
    let mut textures = std::collections::HashMap::<egui::TextureId, egui::ColorImage>::new();
    let mut shot = None;
    let frames: Vec<Vec<egui::Event>> = std::iter::repeat_with(Vec::new)
        .take(6)
        .chain(script.clone())
        .chain(std::iter::repeat_with(Vec::new).take(if script.is_empty() { 0 } else { 2 }))
        .collect();
    for (frame, events) in frames.into_iter().enumerate() {
        let raw = egui::RawInput {
            time: Some(100.0 + 0.05 * frame as f64),
            events,
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
    assert_eq!(shell.version, shell::persist::VERSION);
    let mut expected: Vec<_> =
        crate::state::ICON_PANEL_GROUPS.iter().flat_map(|g| g.iter()).filter(|p| tree.find_tab(p).is_none()).copied().collect();
    expected.push("control");
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
    assert!(flyout.left() >= h.rect(&Hit::Tools).right() && flyout.right() < h.rect(&Hit::Strip).left());
    assert!((h.rect(&Hit::Strip).left() - flyout.right() - 6.0).abs() < 1.0);
    let group = h.app.ui.varos_shell.strip.group("stroke").unwrap();
    assert!((flyout.top() - h.rect(&Hit::StripGroup(group)).top()).abs() < 1.0);
    for p in ["stroke", "gradient"] {
        h.rect(&Hit::FlyoutTab(p.into()));
    }
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::FlyoutTab("transparency".into())), "overflow tabs are not invisible hit targets");
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
    assert_eq!(h.rect(&Hit::Strip).width(), 40.0);
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
    assert!(content.x <= rect.width() - 1.0 + 0.1, "content {content:?}, flyout {rect:?}");
    let state = egui::AreaState::load(&h.ctx, egui::Id::unique("shell-flyout")).unwrap();
    assert_eq!(state.size, Some(rect.size()));
    let index = h.app.ui.varos_shell.strip.group("stroke").unwrap();
    assert_eq!(h.app.ui.varos_shell.strip.groups[index].width, rect.width());
    h.frame(vec![]);
    assert!((h.rect(&Hit::Flyout).size() - rect.size()).length() < 0.01, "opening slides but never changes size");
    h.time += 0.10;
    h.frame(vec![]);
    let settled = h.rect(&Hit::Flyout);
    h.frame(vec![]);
    assert_eq!(h.rect(&Hit::Flyout), settled);
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
    let at = h.rect(&Hit::FlyoutTab("stroke".into())).center();
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    let out = h.frame_full(vec![button(at, false)]);
    assert_eq!(h.app.ui.varos_shell.open.as_deref(), Some("stroke"));
    let nodes = out.platform_output.accesskit_update.unwrap().nodes;
    // The body also has a button named Stroke (the paint target); inspect the two
    // navigation controls by their real hit rectangles, not every same-named button.
    for hit in [Hit::StripIcon("stroke".into()), Hit::FlyoutTab("stroke".into())] {
        let rect = h.rect(&hit);
        let node = nodes
            .iter()
            .find_map(|(_, n)| {
                (n.role() == Role::Button
                    && n.label() == Some("Stroke")
                    && n.bounds().is_some_and(|b| (b.x0 - f64::from(rect.left())).abs() < 1.0 && (b.y0 - f64::from(rect.top())).abs() < 1.0))
                .then_some(n)
            })
            .expect("the strip icon and flyout tab are accessible");
        assert_eq!(node.toggled(), Some(Toggled::True), "{hit:?}");
    }
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
    assert_eq!(h.rect(&Hit::Flyout).y_range(), first.y_range(), "opening slides horizontally, never shrinks or moves vertically");
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
        run_ui_without_textures(
            &h.ctx,
            egui::RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), ..Default::default() },
            |ui| {
                egui::Area::new(egui::Id::unique("shell-flyout")).fixed_pos(at).show(ui.ctx(), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut text).id(id)).request_focus();
                });
                assert!(ui.ctx().egui_wants_keyboard_input());
                crate::shortcuts::handle(&mut h.app, ui.ctx());
            },
        );
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
            assert!((h.rect(&Hit::Flyout).size() - first.size()).length() < 0.01, "{panel} must not grow each frame");
        }
    }
}

// Stage 3: drag and drop, floats, lone icons, collapse.

#[test]
fn a_tab_dragged_onto_a_tab_row_shows_a_line_between_pills_and_lands_there() {
    let mut h = Harness::new();
    h.shell(true);
    let before = h.state().clone();
    let (props, color) = (h.tab("properties"), h.tab("color"));
    let from = h.tab("artboards").center();
    let to = pos2(f32::midpoint(props.right(), color.left()), color.center().y);
    let idle = azure(&h.frame_full(vec![]));
    h.hold_drag(from, to);
    let (hit, line) = h.drop_line();
    assert_eq!(hit, Hit::DropLine { vertical: true, target: Target::Tabs { box_id: 3, before: Some("color".into()) } });
    assert!(line.width() <= 2.5 && line.height() >= 20.0, "a thin vertical line: {line:?}");
    assert!(line.center().x >= props.right() - 1.0 && line.center().x <= color.left() + 1.0, "between the pills: {line:?}");
    assert!(h.rect(&Hit::Ghost).contains(to), "the ghost is under the pointer");
    let dragging = azure(&h.frame_full(vec![egui::Event::PointerMoved(to)]));
    assert!(dragging.0 > idle.0 && dragging.1 > idle.1, "an azure line and an azure source outline: {idle:?} → {dragging:?}");
    assert_eq!(h.state().tree, before.tree, "nothing changes before the release");
    let out = h.frame_full(vec![]);
    assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::Grabbing);
    h.release(to);
    assert_eq!(h.box_tabs(3), ["properties", "artboards", "color", "swatches"]);
    assert_eq!(h.front(3), Some("artboards"));
    assert!(!h.hits().iter().any(|(hit, _)| matches!(hit, Hit::DropLine { .. } | Hit::Ghost)), "the preview goes with the drag");
    h.state().invariants().unwrap();
    // Inside its own box: a reorder.
    let from = h.tab("swatches").center();
    let to = h.tab("properties").left_center() + vec2(3.0, 0.0);
    h.drag(from, to);
    assert_eq!(h.box_tabs(3), ["swatches", "properties", "artboards", "color"]);
    // Box 2's last tab dropped elsewhere: box 2 goes and box 3 takes the column.
    let column = h.rect(&Hit::Box(2)).union(h.rect(&Hit::Box(3)));
    let from = h.tab("layers").center();
    let to = h.tab("color").right_center() + vec2(6.0, 0.0);
    h.drag(from, to);
    assert_eq!(h.tree().find_box(2), None);
    assert_eq!(h.box_tabs(3), ["swatches", "properties", "artboards", "color", "layers"]);
    let rest = h.rect(&Hit::Box(3));
    assert!((rest.top() - column.top()).abs() < 0.5 && (rest.bottom() - column.bottom()).abs() < 0.5, "{rest:?} fills {column:?}");
    h.state().invariants().unwrap();
}

#[test]
fn a_tab_dragged_onto_a_box_edge_splits_the_box_there() {
    let mut h = Harness::new();
    h.shell(true);
    let upper = h.rect(&Hit::Box(2));
    let from = h.tab("color").center();
    let to = pos2(upper.center().x, upper.bottom() - 8.0);
    h.hold_drag(from, to);
    let (hit, line) = h.drop_line();
    assert_eq!(hit, Hit::DropLine { vertical: false, target: Target::Edge { box_id: 2, side: Side::Bottom } });
    assert!(
        line.height() <= 2.5 && (line.center().y - upper.bottom()).abs() <= 1.0 && line.width() >= upper.width() - 1.0,
        "{line:?} along {upper:?}"
    );
    h.release(to);
    let id = h.tree().find_tab("color").unwrap();
    assert!(![2, 3].contains(&id), "a box of its own");
    let (upper, new) = (h.rect(&Hit::Box(2)), h.rect(&Hit::Box(id)));
    assert!(new.top() > upper.bottom() && (new.left() - upper.left()).abs() < 0.5, "{new:?} below {upper:?}");
    assert!((new.height() - upper.height()).abs() <= 1.0, "half each: {new:?} {upper:?}");
    h.state().invariants().unwrap();
    h.assert_min_sizes();
}

#[test]
fn the_boards_four_edges_take_a_box() {
    // The right edge: inside the Board just left of its strip (over the canvas's scroll bar), and
    // in the seam outside.
    for (side, inside) in [(Side::Left, true), (Side::Top, true), (Side::Bottom, true), (Side::Right, true), (Side::Right, false)] {
        let mut h = Harness::new();
        h.shell(true);
        let board = h.board();
        let rail = h.rect(&Hit::Strip);
        let to = match side {
            Side::Left => pos2(board.left() + 8.0, board.center().y),
            Side::Right if inside => pos2(board.right() - 8.0, board.center().y),
            Side::Right => pos2(board.right() + 0.25, board.center().y),
            Side::Top => pos2(board.center().x - 100.0, board.top() + 6.0),
            Side::Bottom => pos2(board.center().x, board.bottom() - 8.0),
        };
        if side == Side::Right && inside {
            assert!(board.contains(to) && to.x < rail.left(), "inside the Board, left of its strip");
        }
        let from = h.tab("swatches").center();
        h.hold_drag(from, to);
        let vertical = matches!(side, Side::Left | Side::Right);
        assert_eq!(h.drop_line().0, Hit::DropLine { vertical, target: Target::Edge { box_id: 1, side } }, "{side:?}");
        h.release(to);
        let id = h.tree().find_tab("swatches").unwrap();
        let (b, new) = (h.board(), h.rect(&Hit::Box(id)));
        let placed = match side {
            Side::Left => new.right() < b.left(),
            Side::Right => new.left() > b.right(),
            Side::Top => new.bottom() < b.top(),
            Side::Bottom => new.top() > b.bottom(),
        };
        assert!(placed, "{side:?}: {new:?} beside {b:?}");
        h.state().invariants().unwrap();
        h.assert_min_sizes();
    }
}

#[test]
fn a_tab_dropped_on_empty_space_floats_and_moves_resizes_and_takes_tabs() {
    let mut h = Harness::new();
    h.app.run("shape.rectangle", json!({"x": 40, "y": 40, "width": 120, "height": 80})).unwrap();
    h.shell(true);
    let selected = h.selection();
    assert_eq!(selected.0, 1);
    let body = h.rect(&Hit::BoardBody);
    let to = body.left_top() + vec2(80.0, 100.0);
    let from = h.tab("color").center();
    h.hold_drag(from, to);
    assert_eq!(h.drop_target(), Some(Target::Float));
    assert!(!h.hits().iter().any(|(hit, _)| matches!(hit, Hit::DropLine { .. })), "no line: the ghost alone");
    h.release(to);
    assert_eq!(h.state().floats.len(), 1);
    let id = h.state().floats[0].panels.id;
    assert_eq!(h.state().floats[0].panels.tabs, ["color"]);
    assert_eq!(h.tree().find_tab("color"), None);
    let float = h.rect(&Hit::Float(id));
    assert!(float.contains(to), "{float:?} under {to:?}");
    assert!(h.tab("color").intersects(float), "its tab row");
    assert_eq!(h.selection(), selected, "the drop never reached the canvas");
    assert!(shell::showing(&h.app, "color"));
    h.state().invariants().unwrap();
    // A press on it never reaches the canvas either.
    h.click(float.center());
    assert_eq!(h.selection(), selected);
    // Moved by its title strip.
    let title = h.rect(&Hit::FloatTitle(id)).center();
    h.drag(title, title + vec2(120.0, 60.0));
    let moved = h.rect(&Hit::Float(id));
    assert!((moved.min - (float.min + vec2(120.0, 60.0))).length() < 1.0, "{moved:?} from {float:?}");
    // Resized from its lower-right corner, then its right edge: drawn at the new size in the
    // frame of each pointer move, the published geometry with it.
    let corner = h.rect(&Hit::FloatResize(id)).center();
    h.hold_drag(corner, corner + vec2(40.0, 30.0));
    let grown = h.rect(&Hit::Float(id));
    assert!((grown.size() - (moved.size() + vec2(40.0, 30.0))).length() < 0.5, "{grown:?} from {moved:?}");
    assert!((h.rect(&Hit::FloatResize(id)).max - grown.max).length() < 0.5, "the corner follows");
    h.frame(vec![egui::Event::PointerMoved(corner + vec2(50.0, 35.0))]);
    let grown = h.rect(&Hit::Float(id));
    assert!((grown.size() - (moved.size() + vec2(50.0, 35.0))).length() < 0.5, "same frame: {grown:?}");
    h.release(corner + vec2(50.0, 35.0));
    assert_eq!(h.rect(&Hit::Float(id)), grown);
    let edge = h.rect(&Hit::FloatResizeRight(id)).center();
    h.hold_drag(edge, edge + vec2(-30.0, 0.0));
    let narrower = h.rect(&Hit::Float(id));
    assert!((narrower.width() - (grown.width() - 30.0)).abs() < 0.5 && narrower.height() == grown.height(), "{narrower:?}");
    h.release(edge + vec2(-30.0, 0.0));
    // A tab dropped into its tab row joins it.
    let row = h.rect(&Hit::TabRow(id));
    let to = pos2(row.right() - 10.0, row.center().y);
    let from = h.tab("swatches").center();
    h.hold_drag(from, to);
    assert_eq!(h.drop_target(), Some(Target::Tabs { box_id: id, before: None }));
    h.release(to);
    assert_eq!(h.state().floats[0].panels.tabs, ["color", "swatches"]);
    assert_eq!(h.tabs().iter().filter(|p| p.as_str() == "swatches").count(), 1);
    // A tab in a box's body (not its edges) floats too.
    let b2 = h.rect(&Hit::Box(2));
    let from = h.tab("artboards").center();
    h.hold_drag(from, b2.center());
    assert_eq!(h.drop_target(), Some(Target::Float));
    h.release(b2.center());
    assert_eq!(h.state().floats.len(), 2);
    h.state().invariants().unwrap();
}

#[test]
fn a_double_click_on_a_tab_collapses_its_box_or_float_and_another_restores_it() {
    let mut h = Harness::new();
    h.shell(true);
    let (top, bottom) = (h.rect(&Hit::Box(2)), h.rect(&Hit::Box(3)));
    let at = h.tab("layers").center();
    h.double_click(at);
    assert!(h.tree().find_box(2).unwrap().collapsed);
    let (row, rest) = (h.rect(&Hit::Box(2)), h.rect(&Hit::Box(3)));
    assert_eq!(row.height(), boxes::HEADER, "its tab row only");
    assert_eq!(row.width(), top.width(), "it keeps its width");
    assert!(
        (rest.top() - row.bottom() - tree::SEAM).abs() < 0.5 && (rest.bottom() - bottom.bottom()).abs() < 0.5,
        "the sibling takes the rest: {rest:?}"
    );
    let saved = serde_json::to_value(h.state()).unwrap();
    assert!(shell::persist::from_value(&saved).tree.find_box(2).unwrap().collapsed, "saved");
    h.double_click(h.tab("layers").center());
    assert!(!h.tree().find_box(2).unwrap().collapsed);
    assert_eq!((h.rect(&Hit::Box(2)), h.rect(&Hit::Box(3))), (top, bottom));
    // A float: its body hides, its title strip and tab row stay.
    let body = h.rect(&Hit::BoardBody);
    let id = h.float_tab("color", body.center() + vec2(-150.0, 0.0));
    let whole = h.rect(&Hit::Float(id));
    h.double_click(h.tab("color").center());
    assert!(h.state().floats[0].panels.collapsed);
    assert_eq!(h.rect(&Hit::Float(id)).height(), floats::TITLE + boxes::FLOAT_HEADER);
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::FloatResize(id)), "no resizing while collapsed");
    h.double_click(h.tab("color").center());
    assert_eq!(h.rect(&Hit::Float(id)), whole);
    // The Window menu shows a collapsed box's panel whole.
    h.double_click(h.tab("swatches").center());
    assert!(h.tree().find_box(3).unwrap().collapsed);
    h.app.run("window.panel", json!({"panel": "swatches"})).unwrap();
    assert!(!h.tree().find_box(3).unwrap().collapsed);
    // Alone beside the Board, nothing would take its room: it stays whole, the status says why.
    for p in ["layers", "artboards"] {
        h.app.run("window.panel.close", json!({"panel": p})).unwrap();
    }
    h.settle();
    let whole = h.rect(&Hit::Box(3));
    h.double_click(h.tab("swatches").center());
    assert!(!h.tree().find_box(3).unwrap().collapsed);
    assert_eq!(h.rect(&Hit::Box(3)), whole);
    assert!(h.app.ui.status.contains("Collapse"), "{}", h.app.ui.status);
}

#[test]
fn a_tab_dragged_onto_the_strip_becomes_an_icon_and_an_icon_onto_a_row_a_tab() {
    let mut h = Harness::new();
    h.shell(true);
    // Over a group's grip dots: a group of its own, there.
    let first = h.state().strip.groups[1].panels[0].clone();
    let grip = h.rect(&Hit::StripGroup(1));
    let from = h.tab("swatches").center();
    h.hold_drag(from, grip.center());
    let (hit, line) = h.drop_line();
    assert_eq!(
        hit,
        Hit::DropLine { vertical: false, target: Target::Strip { strip: StripRef::Board, slot: Slot::NewGroup { before: Some(first) } } }
    );
    assert!(line.height() <= 2.5 && (line.center().y - grip.top()).abs() <= 1.5, "{line:?} at {grip:?}");
    h.release(grip.center());
    assert_eq!(h.state().strip.groups[1].panels, ["swatches"]);
    assert_eq!(h.tree().find_tab("swatches"), None);
    // On an icon's upper half: into its group, before it.
    let group = h.state().strip.groups[h.state().strip.group("stroke").unwrap()].clone();
    let stroke = h.rect(&Hit::StripIcon("stroke".into()));
    let to = pos2(stroke.center().x, stroke.top() + 4.0);
    let from = h.tab("color").center();
    h.hold_drag(from, to);
    assert_eq!(
        h.drop_line().0,
        Hit::DropLine {
            vertical: false,
            target: Target::Strip { strip: StripRef::Board, slot: Slot::Join { group_of: group.panels[0].clone(), before: Some("stroke".into()) } }
        }
    );
    h.release(to);
    let panels = &h.state().strip.groups[h.state().strip.group("stroke").unwrap()].panels;
    let at = |p: &str| panels.iter().position(|q| q == p).unwrap();
    assert_eq!(at("color") + 1, at("stroke"), "{panels:?}");
    h.state().invariants().unwrap();
    // An icon onto a tab row: a tab again.
    let icon = h.rect(&Hit::StripIcon("gradient".into())).center();
    let row = h.rect(&Hit::TabRow(3));
    let to = pos2(row.right() - 8.0, row.center().y);
    h.hold_drag(icon, to);
    assert_eq!(h.drop_target(), Some(Target::Tabs { box_id: 3, before: None }));
    h.release(to);
    assert_eq!(h.tree().find_tab("gradient"), Some(3));
    assert_eq!(h.state().strip.group("gradient"), None);
    // An icon onto a box edge: a box of its own.
    let icon = h.rect(&Hit::StripIcon("transparency".into())).center();
    let upper = h.rect(&Hit::Box(2));
    let to = pos2(upper.center().x, upper.bottom() - 8.0);
    h.drag(icon, to);
    let id = h.tree().find_tab("transparency").unwrap();
    assert!(![2, 3].contains(&id));
    h.state().invariants().unwrap();
}

#[test]
fn an_icon_dropped_on_the_board_is_a_lone_icon_with_a_flyout_a_full_panel_and_back() {
    let mut h = Harness::new();
    h.shell(true);
    let to = h.rect(&Hit::BoardBody).center() + vec2(-150.0, 0.0);
    let from = h.rect(&Hit::StripIcon("stroke".into())).center();
    h.hold_drag(from, to);
    assert_eq!(h.drop_target(), Some(Target::LoneIcon));
    h.release(to);
    let id = h.state().float_strip_of("stroke").expect("a lone icon");
    assert_eq!(h.state().float_strips.len(), 1);
    assert_eq!(h.state().float_strips[0].groups.len(), 1);
    assert_eq!(h.state().float_strips[0].groups[0].panels, ["stroke"]);
    assert_eq!(h.state().strip.group("stroke"), None, "it left the Board's strip");
    let lone = h.rect(&Hit::FloatStrip(id));
    assert!(lone.contains(to) && lone.width() == floats::STRIP_W, "{lone:?} at {to:?}");
    assert!((lone.height() - floats::pill_height(1)).abs() < 0.5, "one icon: {lone:?}");
    for hit in [Hit::FloatStripExpand(id), Hit::FloatStripGroup { strip: id, index: 0 }, Hit::FloatStripTitle(id)] {
        assert!(lone.contains_rect(h.rect(&hit)), "{hit:?}");
    }
    assert!(h.hits().iter().all(|(hit, _)| !matches!(hit, Hit::FloatStripClose(_))), "a lone icon has no ×");
    h.state().invariants().unwrap();
    // Its icon opens the flyout beside it; the icon stays; again closes it.
    let icon = h.rect(&Hit::FloatStripIcon { strip: id, panel: "stroke".into() }).center();
    h.click(icon);
    assert_eq!(h.state().open.as_deref(), Some("stroke"));
    assert!(shell::showing(&h.app, "stroke"));
    let flyout = h.rect(&Hit::Flyout);
    assert!((flyout.left() - lone.right() - 6.0).abs() < 1.0 || (lone.left() - flyout.right() - 6.0).abs() < 1.0, "{flyout:?} beside {lone:?}");
    h.rect(&Hit::FloatStripIcon { strip: id, panel: "stroke".into() });
    h.click(icon);
    assert_eq!(h.state().open, None);
    assert_eq!(h.app.ui.open_panel, None);
    // The Window menu opens it there too.
    h.app.run("window.panel", json!({"panel": "stroke"})).unwrap();
    h.settle();
    assert_eq!(h.state().open.as_deref(), Some("stroke"));
    assert_eq!(h.state().float_strip_of("stroke"), Some(id));
    h.app.run("window.panel", json!({"panel": "stroke", "toggle": true})).unwrap();
    h.settle();
    assert_eq!(h.state().open, None);
    // >>: a full floating panel; the lone icon goes.
    let expand = h.rect(&Hit::FloatStripExpand(id)).center();
    h.click(expand);
    assert!(h.state().float_strips.is_empty());
    assert_eq!(h.state().floats.len(), 1);
    assert_eq!(h.state().floats[0].panels.tabs, ["stroke"]);
    let fid = h.state().floats[0].panels.id;
    h.rect(&Hit::Float(fid));
    h.state().invariants().unwrap();
    // <<: back to the Board's strip, not another lone icon.
    let back = h.rect(&Hit::FloatToIcon(fid)).center();
    h.click(back);
    assert!(h.state().floats.is_empty());
    assert!(h.state().float_strips.is_empty());
    assert!(h.state().strip.group("stroke").is_some());
    h.state().invariants().unwrap();
}

#[test]
fn a_strip_group_dragged_by_its_grip_floats_or_moves_between_groups() {
    let mut h = Harness::new();
    h.shell(true);
    let group = h.state().strip.groups[0].clone();
    let grip = h.rect(&Hit::StripGroup(0)).center();
    let to = h.rect(&Hit::BoardBody).center();
    h.hold_drag(grip, to);
    assert_eq!(h.drop_target(), Some(Target::LoneIcon));
    h.release(to);
    assert_eq!(h.state().float_strips.len(), 1);
    assert_eq!(h.state().float_strips[0].groups, std::slice::from_ref(&group));
    let id = h.state().float_strips[0].id;
    // Its grip dragged onto the Board strip's first group: back, first.
    let grip = h.rect(&Hit::FloatStripGroup { strip: id, index: 0 }).center();
    let to = h.rect(&Hit::StripGroup(0)).center();
    h.drag(grip, to);
    assert!(h.state().float_strips.is_empty());
    assert_eq!(h.state().strip.groups[0], group);
    h.state().invariants().unwrap();
}

#[test]
fn a_floating_strip_docks_on_the_boards_strip_edge_or_between_its_groups_or_moves() {
    let mut h = Harness::new();
    h.shell(true);
    let body = h.rect(&Hit::BoardBody);
    let id = h.lone_icon("stroke", body.center() + vec2(-150.0, 0.0));
    let rail = h.rect(&Hit::Strip);
    let title = h.rect(&Hit::FloatStripTitle(id)).center();
    let to = pos2(rail.left() - 10.0, rail.center().y);
    h.hold_drag(title, to);
    let (hit, line) = h.drop_line();
    assert_eq!(hit, Hit::DropLine { vertical: true, target: Target::Dock });
    assert!((line.height() - rail.height()).abs() < 1.0 && line.width() <= 2.5, "full height along the strip: {line:?} {rail:?}");
    h.release(to);
    assert!(h.state().float_strips.is_empty());
    assert_eq!(h.state().strip.groups.last().unwrap().panels, ["stroke"]);
    h.state().invariants().unwrap();
    // Between the strip's groups: it joins there.
    let id = h.lone_icon("gradient", body.center() + vec2(-150.0, 0.0));
    let first = h.state().strip.groups[1].panels[0].clone();
    let grip = h.rect(&Hit::StripGroup(1));
    let title = h.rect(&Hit::FloatStripTitle(id)).center();
    h.hold_drag(title, grip.center());
    assert_eq!(
        h.drop_line().0,
        Hit::DropLine { vertical: false, target: Target::Strip { strip: StripRef::Board, slot: Slot::NewGroup { before: Some(first) } } }
    );
    h.release(grip.center());
    assert_eq!(h.state().strip.groups[1].panels, ["gradient"]);
    // Elsewhere it only moves.
    let id = h.lone_icon("transparency", body.center() + vec2(-150.0, 0.0));
    let lone = h.rect(&Hit::FloatStrip(id));
    let title = h.rect(&Hit::FloatStripTitle(id)).center();
    h.hold_drag(title, title + vec2(0.0, 80.0));
    assert_eq!(h.drop_target(), Some(Target::Move));
    h.release(title + vec2(0.0, 80.0));
    assert!((h.rect(&Hit::FloatStrip(id)).min - (lone.min + vec2(0.0, 80.0))).length() < 1.0);
    h.state().invariants().unwrap();
}

#[test]
fn escape_or_a_lost_focus_mid_drag_changes_nothing() {
    let mut h = Harness::new();
    h.shell(true);
    let body = h.rect(&Hit::BoardBody);
    let lone = h.lone_icon("stroke", body.center() + vec2(-150.0, 0.0));
    let before = h.state().clone();
    let rail = h.rect(&Hit::Strip);
    let row = h.rect(&Hit::TabRow(3));
    let upper = h.rect(&Hit::Box(2));
    let cases = [
        (h.tab("artboards").center(), pos2(row.right() - 8.0, row.center().y)),
        (h.tab("color").center(), pos2(upper.center().x, upper.bottom() - 8.0)),
        (h.tab("color").center(), body.center()),
        (h.tab("swatches").center(), h.rect(&Hit::StripGroup(1)).center()),
        (h.rect(&Hit::StripIcon("gradient".into())).center(), body.center() + vec2(100.0, 80.0)),
        (h.rect(&Hit::StripIcon("gradient".into())).center(), pos2(row.right() - 8.0, row.center().y)),
        (h.rect(&Hit::StripGroup(0)).center(), body.center() + vec2(100.0, 80.0)),
        (h.rect(&Hit::FloatStripTitle(lone)).center(), pos2(rail.left() - 10.0, rail.center().y)),
    ];
    for (k, (from, to)) in cases.into_iter().enumerate() {
        h.hold_drag(from, to);
        assert!(h.drop_target().is_some(), "case {k}: a target");
        h.frame(vec![escape_key()]);
        assert!(h.state().drag.is_none(), "case {k}");
        h.release(to);
        assert_eq!(*h.state(), before, "case {k}: nothing changed");
        assert!(!h.hits().iter().any(|(hit, _)| matches!(hit, Hit::DropLine { .. } | Hit::DropAt(_) | Hit::Ghost)), "case {k}");
    }
    // A pointer gone from the window cancels.
    let (from, to) = (h.tab("color").center(), body.center());
    h.hold_drag(from, to);
    h.frame(vec![egui::Event::PointerGone]);
    assert!(h.state().drag.is_none());
    h.release(to);
    assert_eq!(*h.state(), before);
    // A lost window focus cancels too.
    h.hold_drag(from, to);
    h.focused = false;
    h.frame(vec![egui::Event::PointerMoved(to)]);
    h.focused = true;
    assert!(h.state().drag.is_none());
    h.release(to);
    assert_eq!(*h.state(), before);
    // So does a release where nothing applies (the app bar).
    h.hold_drag(from, pos2(to.x, 4.0));
    assert_eq!(h.drop_target(), None);
    h.release(pos2(to.x, 4.0));
    assert_eq!(*h.state(), before);
    // Escape while a float is moved puts it back.
    let id = h.float_tab("color", body.center());
    let float = h.rect(&Hit::Float(id));
    let title = h.rect(&Hit::FloatTitle(id)).center();
    h.hold_drag(title, title + vec2(90.0, 40.0));
    assert!((h.rect(&Hit::Float(id)).min - (float.min + vec2(90.0, 40.0))).length() < 1.0, "it moves live");
    h.frame(vec![escape_key()]);
    h.release(title + vec2(90.0, 40.0));
    assert_eq!(h.rect(&Hit::Float(id)), float);
}

#[test]
fn alt_at_the_drag_start_is_the_duplicate_hook_which_moves_for_now() {
    let mut h = Harness::new();
    h.shell(true);
    let to = h.rect(&Hit::BoardBody).center();
    let from = h.tab("color").center();
    h.modifiers = egui::Modifiers::ALT;
    h.hold_drag(from, to);
    assert_eq!(h.state().drag.as_ref().map(|d| d.mode), Some(DragMode::Duplicate));
    assert!(h.app.ui.status.contains("duplicate: later"), "{}", h.app.ui.status);
    h.modifiers = egui::Modifiers::NONE;
    h.release(to);
    assert_eq!(h.state().floats.len(), 1);
    assert_eq!(h.tree().find_tab("color"), None, "moved, not copied");
    h.state().invariants().unwrap();
}

#[test]
fn floats_and_lone_icons_round_trip_through_ui_json_and_version_2_migrates() {
    let mut h = Harness::new();
    h.shell(true);
    let body = h.rect(&Hit::BoardBody);
    let id = h.float_tab("color", body.center() + vec2(-150.0, 0.0));
    h.lone_icon("stroke", body.center() + vec2(150.0, 0.0));
    h.app.ui.varos_shell.toggle_collapsed(id);
    h.app.ui.varos_shell.toggle_collapsed(2);
    h.settle();
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    assert_eq!(saved["varos_shell"]["version"], json!(shell::persist::VERSION));
    assert_eq!(saved["varos_shell"]["floats"][0]["box"]["tabs"], json!(["color"]));
    assert_eq!(saved["varos_shell"]["float_strips"][0]["groups"][0]["panels"], json!(["stroke"]));
    let back: crate::state::UiState = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(back.varos_shell.tree, h.state().tree);
    assert_eq!(back.varos_shell.strip, h.state().strip);
    assert_eq!(back.varos_shell.floats, h.state().floats);
    assert_eq!(back.varos_shell.float_strips, h.state().float_strips);
    assert!(back.varos_shell.floats[0].panels.collapsed && back.varos_shell.tree.find_box(2).unwrap().collapsed);
    back.varos_shell.invariants().unwrap();
    // Version 2 (stage 2) has no floats: the boxes and the strip load, the panels a float held
    // are icons again, and the version is 3.
    let mut v2 = saved["varos_shell"].clone();
    v2["version"] = json!(2);
    let migrated = shell::persist::from_value(&v2);
    assert_eq!(migrated.version, shell::persist::VERSION);
    assert!(migrated.floats.is_empty() && migrated.float_strips.is_empty());
    assert_eq!(migrated.tree, h.state().tree);
    assert!(migrated.strip.group("color").is_some() && migrated.strip.group("stroke").is_some());
    migrated.invariants().unwrap();
    // A float that doesn't read is dropped alone; a panel owned twice keeps its first owner.
    let mut bad = saved["varos_shell"].clone();
    bad["floats"].as_array_mut().unwrap().push(json!({"box": "nope"}));
    bad["float_strips"][0]["groups"][0]["panels"].as_array_mut().unwrap().extend([json!("layers"), json!("color"), json!("nope")]);
    let loaded = shell::persist::from_value(&bad);
    assert_eq!(loaded.floats.len(), 1);
    assert_eq!(loaded.float_strips[0].groups[0].panels, ["stroke"]);
    loaded.invariants().unwrap();
}

#[test]
fn window_panel_reaches_floats_and_lone_icons() {
    let mut h = Harness::new();
    h.shell(true);
    let checked = |app: &VectorcraftApp, p: &str| crate::menus::checked(app, "window.panel", &json!({"panel": p}));
    let body = h.rect(&Hit::BoardBody);
    let id = h.float_tab("color", body.center() + vec2(-150.0, 0.0));
    h.lone_icon("stroke", body.center() + vec2(150.0, 0.0));
    assert_eq!(checked(&h.app, "color"), Some(true));
    assert_eq!(checked(&h.app, "stroke"), Some(false), "an icon only");
    // The float's collapsed tab comes forward whole.
    h.app.ui.varos_shell.toggle_collapsed(id);
    let out = h.app.run("window.panel", json!({"panel": "color"})).unwrap();
    assert_eq!(out["box"], json!(id));
    assert!(!h.state().floats[0].panels.collapsed);
    // Toggled: it closes back to the Board's strip, and the empty float goes.
    h.app.run("window.panel", json!({"panel": "color", "toggle": true})).unwrap();
    assert!(h.state().floats.is_empty());
    assert!(h.state().strip.group("color").is_some());
    assert!(!shell::showing(&h.app, "color"));
    // A lone icon's panel opens as its flyout; Close Panel closes the flyout, the icon stays.
    h.app.run("window.panel", json!({"panel": "stroke"})).unwrap();
    h.settle();
    assert!(shell::showing(&h.app, "stroke"));
    h.rect(&Hit::Flyout);
    let out = h.app.run("window.panel.close", json!({"panel": "stroke"})).unwrap();
    assert_eq!(out["closed"], true);
    h.settle();
    assert!(!shell::showing(&h.app, "stroke"));
    assert!(h.state().float_strip_of("stroke").is_some());
    h.state().invariants().unwrap();
}

/// Software capture (no GPU) mid-drag: the Artboards tab held over the lower box's tab row, the
/// azure insertion line between Properties and Color, the ghost pill under the pointer:
/// `VAROS_SHELL_CAPTURE_DRAG=<file.png>`.
#[test]
#[ignore = "optional software visual capture; set VAROS_SHELL_CAPTURE_DRAG"]
fn capture_shell_drag() {
    let Some(file) = std::env::var_os("VAROS_SHELL_CAPTURE_DRAG").map(std::path::PathBuf::from) else { return };
    let (width, height) = (1440_u32, 900_u32);
    let mut h = Harness::sized_without_document(vec2(width as f32, height as f32));
    h.app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
    h.app.ui.control_bar = true;
    h.settle();
    h.shell(true);
    let (props, color) = (h.tab("properties"), h.tab("color"));
    let from = h.tab("artboards").center();
    let to = pos2(f32::midpoint(props.right(), color.left()), color.center().y + 6.0);
    let mut script = vec![vec![egui::Event::PointerMoved(from)], vec![button(from, true)]];
    script.extend((1..=8).map(|k| vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0))]));
    let pixels = capture_with(&mut h.app, width, height, script);
    assert!(h.app.ui.varos_shell.drag.is_some(), "captured mid-drag");
    image::save_buffer(&file, &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

/// Software capture (no GPU) with a floating panel (Color) and a lone floating icon (Stroke):
/// `VAROS_SHELL_CAPTURE_FLOAT=<file.png>`.
#[test]
#[ignore = "optional software visual capture; set VAROS_SHELL_CAPTURE_FLOAT"]
fn capture_shell_float() {
    let Some(file) = std::env::var_os("VAROS_SHELL_CAPTURE_FLOAT").map(std::path::PathBuf::from) else { return };
    let (width, height) = (1440_u32, 900_u32);
    let mut h = Harness::sized_without_document(vec2(width as f32, height as f32));
    h.app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
    h.app.ui.control_bar = true;
    h.settle();
    h.shell(true);
    let body = h.rect(&Hit::BoardBody);
    h.float_tab("color", body.left_top() + vec2(120.0, 140.0));
    h.lone_icon("stroke", body.right_top() + vec2(-60.0, 120.0));
    assert_eq!((h.state().floats.len(), h.state().float_strips.len()), (1, 1));
    let pixels = capture(&mut h.app, width, height);
    image::save_buffer(&file, &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

#[test]
fn a_float_resize_is_undone_by_escape_a_lost_focus_or_a_gone_pointer() {
    let mut h = Harness::new();
    h.shell(true);
    let body = h.rect(&Hit::BoardBody);
    let id = h.float_tab("color", body.left_top() + vec2(80.0, 100.0));
    let float = h.rect(&Hit::Float(id));
    let saved = h.state().floats[0].rect;
    for (k, cancel) in [vec![escape_key()], vec![egui::Event::PointerGone], vec![]].into_iter().enumerate() {
        let corner = h.rect(&Hit::FloatResize(id)).center();
        h.hold_drag(corner, corner + vec2(60.0, 40.0));
        assert!(h.rect(&Hit::Float(id)).width() > float.width() + 50.0, "case {k}: resizing");
        if k == 2 {
            h.focused = false;
        }
        h.frame(cancel);
        h.focused = true;
        assert!(h.state().drag.is_none(), "case {k}");
        assert_eq!(h.state().floats[0].rect, saved, "case {k}: the old size is back");
        h.release(corner + vec2(60.0, 40.0));
        assert_eq!(h.rect(&Hit::Float(id)), float, "case {k}");
    }
}

#[test]
fn a_release_after_the_window_changed_lands_on_what_is_drawn_then() {
    let mut h = Harness::new();
    h.shell(true);
    let to = pos2(h.board().right() - 8.0, h.board().center().y);
    let from = h.tab("color").center();
    h.hold_drag(from, to);
    assert_eq!(h.drop_line().0, Hit::DropLine { vertical: true, target: Target::Edge { box_id: 1, side: Side::Right } });
    // The window grows in the frame of the release: the pointer is now over the canvas, far from
    // any edge, so the drop makes a floating panel, not the split the old frame showed.
    h.screen += vec2(400.0, 0.0);
    h.release(to);
    assert!(h.rect(&Hit::BoardBody).contains(to));
    assert_eq!(h.state().floats.len(), 1, "landed on the new layout");
    assert_eq!(h.state().floats[0].panels.tabs, ["color"]);
    assert_eq!(h.tree().boxes().len(), 3, "no split from the stale layout");
    h.state().invariants().unwrap();
    // The same layout on release: it lands at once, in the release frame.
    let from = h.tab("swatches").center();
    let row = h.rect(&Hit::TabRow(2));
    let to = pos2(row.right() - 8.0, row.center().y);
    h.hold_drag(from, to);
    h.frame(vec![egui::Event::PointerMoved(to), button(to, false)]);
    assert_eq!(h.tree().find_tab("swatches"), Some(2));
}

#[test]
fn a_flyout_tab_dragged_out_outlines_its_group_and_a_cancel_reopens_the_flyout() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel": "stroke"})).unwrap();
    h.settle();
    let before = h.state().clone();
    let group = h.state().strip.group("gradient").unwrap();
    let from = h.rect(&Hit::FlyoutTab("gradient".into())).center();
    let to = h.rect(&Hit::BoardBody).center();
    h.hold_drag(from, to);
    assert_eq!(h.drop_target(), Some(Target::Float));
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::Flyout), "the flyout closes while dragging");
    let grip = h.rect(&Hit::StripGroup(group));
    let icons = ["stroke", "gradient", "transparency"].map(|p| h.rect(&Hit::StripIcon(p.into())));
    let outlined = icons.into_iter().fold(grip, Rect::union);
    assert_eq!(h.rect(&Hit::DragSource), outlined, "its group in the strip is outlined");
    h.frame(vec![escape_key()]);
    h.release(to);
    assert_eq!(h.state().open.as_deref(), Some("stroke"), "the flyout is back");
    assert_eq!(h.app.ui.open_panel.as_deref(), Some("stroke"));
    h.rect(&Hit::Flyout);
    assert_eq!(h.state().strip, before.strip);
    assert_eq!(h.state().tree, before.tree);
    // A release where nothing applies reopens it too; a real drop leaves it closed.
    let from = h.rect(&Hit::FlyoutTab("gradient".into())).center();
    h.hold_drag(from, pos2(to.x, 4.0));
    h.release(pos2(to.x, 4.0));
    assert_eq!(h.state().open.as_deref(), Some("stroke"));
    let from = h.rect(&Hit::FlyoutTab("gradient".into())).center();
    h.drag(from, to);
    assert!(h.state().float_of("gradient").is_some());
    assert_eq!(h.state().open, None);
    h.state().invariants().unwrap();
}

// ---------- Stage 4: Properties sections and pin, workspaces, maximize, reset ----------

use crate::panels::properties::{SectionHit, SectionId as S, section_dragging, section_hits};

impl Harness {
    /// Properties editor `instance`'s sections, in the order drawn.
    fn sections(&self, instance: &str) -> Vec<S> {
        section_hits(&self.ctx, instance).sections.iter().map(|s| s.id).collect()
    }

    fn section(&self, instance: &str, id: S) -> SectionHit {
        section_hits(&self.ctx, instance).sections.into_iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{instance} draws {id:?}"))
    }

    /// The texts the next frame draws.
    fn texts(&mut self) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let out = self.frame_full(vec![]);
        let mut texts = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut texts);
        }
        texts
    }

    /// The lower right box (Properties' in the default layout) given most of the column.
    fn tall_properties(&mut self) {
        self.app.ui.varos_shell.tree.set_ratio(&[1], 0.2);
        self.settle();
    }

    fn id_of(out: &serde_json::Value) -> u64 {
        out["id"].as_u64().expect("an object id")
    }
}

#[test]
fn properties_sections_keep_their_ids_across_selections_and_collapse_per_editor() {
    let mut h = Harness::new();
    h.shell(true);
    h.tall_properties();
    // Nothing selected: the document's sections.
    assert_eq!(h.sections("properties"), [S::Document, S::Appearance, S::RulersGrids, S::Guides, S::Snap, S::Preferences, S::QuickActions]);
    h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 60, "height": 40})).unwrap();
    h.settle();
    let ids = h.sections("properties");
    assert_eq!((ids.first(), ids.last()), (Some(&S::Transform), Some(&S::QuickActions)), "{ids:?}");
    assert!(ids.contains(&S::Appearance) && ids.contains(&S::Align), "{ids:?}");
    // The header: chevron, title, grip; a hairline; the body below it, inset.
    let appearance = h.section("properties", S::Appearance);
    assert_eq!(appearance.header.height(), 32.0);
    assert!(appearance.grip.right() == appearance.header.right() && appearance.grip.width() < 24.0);
    assert!(appearance.whole.height() > appearance.header.height() + 20.0, "open: {appearance:?}");
    // A click on its header collapses it, in this editor.
    h.click(appearance.header.left_center() + vec2(40.0, 0.0));
    assert!(h.state().views["properties"].collapsed.contains(&S::Appearance));
    let appearance = h.section("properties", S::Appearance);
    assert!(appearance.collapsed && appearance.whole.height() <= appearance.header.height() + 0.5, "{appearance:?}");
    // The same section of the document (nothing selected) is collapsed too: the ids are stable.
    h.app.run("select.none", json!({})).unwrap();
    h.settle();
    assert!(h.section("properties", S::Appearance).collapsed);
    assert!(!h.section("properties", S::Document).collapsed);
    // The view persists (ui.json); a click opens it again.
    let back: crate::state::UiState = serde_json::from_value(serde_json::to_value(&h.app.ui).unwrap()).unwrap();
    assert!(back.varos_shell.views["properties"].collapsed.contains(&S::Appearance));
    let header = h.section("properties", S::Appearance).header;
    h.click(header.left_center() + vec2(40.0, 0.0));
    assert!(!h.section("properties", S::Appearance).collapsed);
    // The editor never touched the tree (its headers are not tabs).
    let mut expected = tree::Node::board_and_column(&["layers", "artboards"], &["properties", "color", "swatches"]);
    expected.set_ratio(&[1], 0.2);
    assert_eq!(h.tree(), &expected);
}

#[test]
fn a_section_moves_by_its_grip_with_an_azure_line_and_escape_cancels() {
    let mut h = Harness::new();
    h.shell(true);
    h.tall_properties();
    h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 60, "height": 40})).unwrap();
    h.settle();
    let tree_before = h.tree().clone();
    let order = h.sections("properties");
    let (quick, transform) = (h.section("properties", S::QuickActions), h.section("properties", S::Transform));
    let from = quick.grip.center();
    let to = pos2(transform.header.center().x, transform.header.top() + 3.0);
    // Mid-drag: the ghost under the pointer, the azure line above Transform.
    h.hold_drag(from, to);
    assert!(section_dragging(&h.ctx));
    let hits = section_hits(&h.ctx, "properties");
    let line = hits.line.expect("an insertion line");
    assert!((line.center().y - (transform.whole.top() - 2.0)).abs() <= 1.5, "{line:?} above {transform:?}");
    assert!(line.height() <= 2.5 && line.width() >= transform.header.width() - 1.0);
    assert!(hits.ghost.is_some_and(|g| g.contains(to)), "{:?}", hits.ghost);
    let out = h.frame_full(vec![egui::Event::PointerMoved(to)]);
    assert!(azure(&out).0 >= 1, "the line is azure");
    // Escape: nothing moves.
    h.frame(vec![escape_key()]);
    assert!(!section_dragging(&h.ctx));
    h.release(to);
    assert_eq!(h.sections("properties"), order);
    assert!(section_hits(&h.ctx, "properties").line.is_none());
    // Again, released: Quick Actions comes first; the panel itself stays where it was.
    let (quick, transform) = (h.section("properties", S::QuickActions), h.section("properties", S::Transform));
    h.drag(quick.grip.center(), pos2(transform.header.center().x, transform.header.top() + 3.0));
    let moved = h.sections("properties");
    assert_eq!(moved[0], S::QuickActions, "{moved:?}");
    assert_eq!(&moved[1..], &order[..order.len() - 1]);
    assert_eq!(h.tree(), &tree_before, "the grip is not the tab");
    assert!(h.state().drag.is_none());
    // Released where it is: nothing changes.
    let align = h.section("properties", S::Align);
    h.drag(align.grip.center(), align.grip.center() + vec2(-40.0, 4.0));
    assert_eq!(h.sections("properties"), moved);
}

#[test]
fn two_properties_editors_follow_the_selection_each_with_its_own_view() {
    let mut h = Harness::new();
    h.shell(true);
    let a = Harness::id_of(&h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 60, "height": 40})).unwrap());
    let b = Harness::id_of(&h.app.run("text.create", json!({"x": 150, "y": 150, "text": "Hi", "size": 24})).unwrap());
    h.app.run("select.set", json!({"ids": [a]})).unwrap();
    h.settle();
    // ≡ › Duplicate Panel: a second editor in a box of its own below.
    let twin = shell::duplicate_properties(&mut h.app, "properties").expect("a second editor");
    assert_eq!(twin, "properties@2");
    h.settle();
    h.state().invariants().unwrap();
    assert!(h.tabs().contains(&twin));
    let twin_box = h.tree().find_tab(&twin).unwrap();
    assert!(h.rect(&Hit::Box(twin_box)).top() > h.rect(&Hit::Box(3)).bottom(), "below the first");
    assert_eq!(boxes::label(&h.app, &twin), "Properties 2");
    // No pin (deferred): both say they follow the selection.
    let texts = h.texts();
    assert_eq!(texts.iter().filter(|t| t.starts_with("Following selection")).count(), 2, "{texts:?}");
    assert!(!texts.iter().any(|t| t.starts_with("Pinned")));
    // Each has its own order and collapsed sections.
    h.app.ui.varos_shell.views.entry(twin.clone()).or_default().move_section(S::QuickActions, Some(S::Transform), S::QuickActions);
    h.app.ui.varos_shell.views.entry(twin.clone()).or_default().collapsed.insert(S::Transform);
    h.settle();
    assert_eq!(h.sections(&twin)[0], S::QuickActions);
    assert_eq!(h.sections("properties")[0], S::Transform);
    assert!(h.section(&twin, S::Transform).collapsed && !h.section("properties", S::Transform).collapsed);
    // The selection changes to the text: both follow it, each in its own order.
    h.app.run("select.set", json!({"ids": [b]})).unwrap();
    h.settle();
    let (mut mine, mut first) = (h.sections(&twin), h.sections("properties"));
    assert!(first.contains(&S::Character), "{first:?}");
    assert_ne!(mine, first, "the second editor keeps its own order");
    mine.sort();
    first.sort();
    assert_eq!(mine, first, "both follow the selection");
    // Closing the second editor removes it for good (no icon in the strip), its view with it.
    h.app.ui.varos_shell.close_tab(&twin);
    h.settle();
    assert!(!h.state().owns(&twin) && !h.state().views.contains_key(&twin));
    h.state().invariants().unwrap();
}

/// An edit made through the second editor goes to the selection, as one undo step: pressed with
/// the pointer, and through the accessibility tree (a screen reader's press).
///
/// The first editor's own sections are folded: a box's content that doesn't fit runs on under its
/// clip, and AccessKit reports those hidden widgets' bounds unclipped, so a widget of the first
/// box can lie inside the second box's rect. Exactly one Lock is then in the second box.
#[test]
fn an_edit_in_the_second_properties_editor_applies_to_the_selection_in_one_undo_step() {
    use egui::accesskit::Role;
    let mut h = Harness::new();
    h.shell(true);
    let a = Harness::id_of(&h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 60, "height": 40})).unwrap());
    h.app.run("select.set", json!({"ids": [a]})).unwrap();
    h.app.ui.varos_shell.tree.set_ratio(&[1], 0.15);
    let twin = shell::duplicate_properties(&mut h.app, "properties").unwrap();
    h.app.ui.varos_shell.views.entry("properties".into()).or_default().collapsed.extend(S::ALL);
    // Quick Actions first in the second editor, the rest folded, so Lock is in sight there.
    let view = h.app.ui.varos_shell.views.entry(twin.clone()).or_default();
    view.move_section(S::QuickActions, Some(S::Transform), S::QuickActions);
    view.collapsed.extend([S::Transform, S::Appearance, S::Align]);
    h.settle();
    let twin_box = h.rect(&Hit::Box(h.tree().find_tab(&twin).unwrap()));
    h.ctx.enable_accesskit();
    h.settle();
    // The Lock buttons centred in the second box, read from one settled frame's tree.
    let locks = |h: &mut Harness| -> Vec<(egui::accesskit::NodeId, Rect)> {
        let out = h.frame_full(vec![]);
        let nodes = out.platform_output.accesskit_update.expect("accesskit is on").nodes;
        nodes
            .iter()
            .filter(|(_, n)| n.role() == Role::Button && n.label().is_some_and(|l| l.trim() == "Lock"))
            .filter_map(|(id, n)| n.bounds().map(|b| (*id, Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32)))))
            .filter(|(_, r)| twin_box.contains(r.center()))
            .collect()
    };
    let locked = |h: &Harness| h.app.session.active().unwrap().doc.node(vectorcraft_doc::NodeId(a)).unwrap().locked;
    let found = locks(&mut h);
    assert_eq!(found.len(), 1, "one Lock in the second box: {found:?}");
    let quick = h.section(&twin, S::QuickActions).whole;
    assert!(quick.contains_rect(found[0].1), "drawn in the second editor's Quick Actions: {found:?} in {quick:?}");
    // The pointer, at the centre of the rect that frame published.
    let undo = h.app.session.active().unwrap().history.undo.len();
    h.click(found[0].1.center());
    assert!(locked(&h), "the selected rectangle is locked: {}", h.app.ui.status);
    assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo + 1, "one undo step");
    h.app.run("edit.undo", json!({})).unwrap();
    h.app.run("select.set", json!({"ids": [a]})).unwrap();
    h.settle();
    assert!(!locked(&h));
    // A screen reader's press of the same button.
    let found = locks(&mut h);
    assert_eq!(found.len(), 1, "{found:?}");
    let undo = h.app.session.active().unwrap().history.undo.len();
    let click = egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::Click,
        target_node: found[0].0,
        target_tree: egui::accesskit::TreeId::ROOT,
        data: None,
    });
    h.frame(vec![click]);
    h.settle();
    assert!(locked(&h), "{}", h.app.ui.status);
    assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo + 1, "one undo step");
}

/// `properties@2` is a panel to the window commands while the shell has it: activated, checked
/// in the Window menu, closed; gone, it is an unknown panel.
#[test]
fn window_commands_reach_the_second_properties_editor() {
    let mut h = Harness::new();
    h.shell(true);
    let twin = shell::duplicate_properties(&mut h.app, "properties").unwrap();
    h.app.ui.varos_shell.insert_tabs(3, None, std::slice::from_ref(&twin));
    h.app.ui.varos_shell.tree.activate("properties");
    h.settle();
    assert_eq!(h.box_tabs(3), ["properties", "color", "swatches", "properties@2"]);
    let checked = |app: &VectorcraftApp, p: &str| crate::menus::checked(app, "window.panel", &json!({"panel": p}));
    assert_eq!(checked(&h.app, &twin), Some(true));
    let out = h.app.run("window.panel.activate", json!({"panel": twin})).unwrap();
    assert_eq!(out["box"], json!(3));
    assert_eq!(h.front(3), Some("properties@2"));
    h.app.run("window.panel", json!({"panel": "properties"})).unwrap();
    assert_eq!(h.front(3), Some("properties"));
    let out = h.app.run("window.panel.close", json!({"panel": twin})).unwrap();
    assert_eq!(out["closed"], true);
    h.settle();
    assert!(!h.state().owns(&twin));
    assert_eq!(checked(&h.app, &twin), Some(false));
    assert!(h.app.run("window.panel.activate", json!({"panel": twin})).is_err(), "gone: unknown");
    assert!(h.app.run("window.panel", json!({"panel": "properties@9"})).is_err());
    h.state().invariants().unwrap();
}

/// A flyout's ≡ has no Duplicate Panel (it has nowhere to put the copy); a box's has.
#[test]
fn duplicate_panel_is_in_a_boxs_menu_not_a_flyouts() {
    let mut h = Harness::new();
    h.shell(true);
    let menu = h.rect(&Hit::Menu(3));
    h.click(menu.center());
    let texts = h.texts();
    assert!(texts.iter().any(|t| t.contains("Duplicate Panel")), "{texts:?}");
    h.frame(vec![escape_key()]);
    h.settle();
    // Properties back in the strip, opened as a flyout.
    h.app.run("window.panel.close", json!({"panel": "properties"})).unwrap();
    h.app.run("window.panel", json!({"panel": "properties"})).unwrap();
    h.settle();
    let menu = h.rect(&Hit::FlyoutClose).translate(vec2(23.0, 0.0));
    h.click(menu.center());
    let texts = h.texts();
    assert!(texts.iter().any(|t| t.contains("Close Panel")), "the flyout's menu is open: {texts:?}");
    assert!(!texts.iter().any(|t| t.contains("Duplicate Panel")), "{texts:?}");
}

#[test]
fn an_alt_drag_of_the_properties_tab_makes_another_editor() {
    let mut h = Harness::new();
    h.shell(true);
    let to = h.rect(&Hit::BoardBody).center();
    let from = h.tab("properties").center();
    h.modifiers = egui::Modifiers::ALT;
    h.hold_drag(from, to);
    assert_eq!(h.state().drag.as_ref().map(|d| d.mode), Some(DragMode::Duplicate));
    assert!(!h.app.ui.status.contains("duplicate: later"));
    h.modifiers = egui::Modifiers::NONE;
    h.release(to);
    assert_eq!(h.tree().find_tab("properties"), Some(3), "the first stays");
    assert_eq!(h.state().float_of("properties@2"), Some(h.state().floats[0].panels.id), "the copy floats");
    h.state().invariants().unwrap();
}

#[test]
fn the_workspace_row_switches_restores_and_remembers_layouts_keeping_the_documents() {
    let mut h = Harness::new();
    h.shell(true);
    let uid = h.app.session.active().unwrap().uid;
    assert!(!h.hits().iter().any(|(hit, _)| matches!(hit, Hit::Workspaces)));
    assert_eq!(h.state().workspace, "Design");
    // Changes in Design: Stroke a tab of the upper box, Color floating.
    h.app.ui.varos_shell.add_tab(2, "stroke");
    let body = h.rect(&Hit::BoardBody);
    h.float_tab("color", body.center());
    let design = h.state().capture("Design");
    // Typography as its preset comes; the document stays.
    h.app.run("window.workspace", json!({"name": "Typography"})).unwrap();
    h.settle();
    assert_eq!(h.state().workspace, "Typography");
    let typography = shell::workspace::preset("Typography").unwrap();
    assert_eq!((&h.state().tree, &h.state().strip), (&typography.tree, &typography.strip));
    assert!(h.state().floats.is_empty() && h.tabs().contains(&"character".to_string()));
    assert_eq!(h.app.session.documents().len(), 1);
    assert_eq!(h.app.session.active().unwrap().uid, uid);
    // A change in Typography.
    h.app.run("window.panel.close", json!({"panel": "glyphs"})).unwrap();
    h.settle();
    // Design comes back as it was left: tree, strip, floats, floating strips.
    h.app.run("window.workspace", json!({"name": "Design"})).unwrap();
    h.settle();
    let now = h.state().capture("Design");
    assert_eq!((&now.tree, &now.strip, &now.floats, &now.float_strips), (&design.tree, &design.strip, &design.floats, &design.float_strips));
    // … and Typography with its change.
    h.app.run("window.workspace", json!({"name": "typography"})).unwrap();
    assert_eq!(h.tree().find_tab("glyphs"), None);
    // Reset: Typography as its preset.
    assert_eq!(h.app.run("window.workspace.reset", json!({})).unwrap()["workspace"], "Typography");
    assert_eq!(h.state().tree, typography.tree);
    // `+`: the New Workspace dialog; OK saves the layout under the name typed.
    h.settle();
    h.app.run("window.workspace.new", json!({})).unwrap();
    h.settle();
    let d = h.app.ui.dialog.as_mut().expect("New Workspace");
    assert_eq!(d.kind, "newWorkspace");
    d.fields.insert("name".into(), json!("Mine"));
    crate::workspaces::confirm(&mut h.app).unwrap();
    h.settle();
    assert_eq!(h.state().workspace, "Mine");
    assert!(h.texts().iter().any(|t| t == "Mine"));
    assert_eq!(h.app.run("window.workspace.list", json!({})).unwrap()["custom"], json!(["Mine"]));
    assert_eq!(crate::menus::checked(&h.app, "window.workspace", &json!({"name": "Mine"})), Some(true));
    // Rename and delete (the tab's context menu runs these); a preset can't be deleted.
    h.app.run("window.workspace.rename", json!({"name": "Mine", "to": "Ours"})).unwrap();
    assert_eq!(h.state().workspace, "Ours");
    assert!(h.app.run("window.workspace.delete", json!({"name": "Design"})).is_err());
    h.app.run("window.workspace.delete", json!({"name": "Ours"})).unwrap();
    assert_eq!(h.state().workspace, "Design");
    let now = h.state().capture("Design");
    assert_eq!(now.tree, design.tree, "Design as it was left");
    // Unknown names are refused; everything survives ui.json.
    assert!(h.app.run("window.workspace", json!({"name": "Essentials Classic"})).is_err());
    h.app.run("window.workspace.new", json!({"name": "Kept"})).unwrap();
    let back: crate::state::UiState = serde_json::from_value(serde_json::to_value(&h.app.ui).unwrap()).unwrap();
    assert_eq!(back.varos_shell.workspaces, h.state().workspaces);
    assert_eq!(back.varos_shell.workspace_changes, h.state().workspace_changes);
    assert_eq!(back.varos_shell.workspace, "Kept");
    h.state().invariants().unwrap();
}

#[test]
fn reset_restores_the_default_layout_and_leaves_the_document_alone() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 60, "height": 40})).unwrap();
    h.settle();
    let st = h.app.session.active().unwrap();
    let (doc, selection, dirty) = (st.doc.clone(), st.selection.clone(), st.is_dirty());
    let hash = format!("{:?}", st.doc);
    // A changed layout: a float, a tab moved, a box maximized, a section collapsed.
    let body = h.rect(&Hit::BoardBody);
    h.float_tab("color", body.center());
    h.app.ui.varos_shell.add_tab(2, "stroke");
    h.app.ui.varos_shell.views.entry("properties".into()).or_default().collapsed.insert(S::Align);
    h.app.ui.varos_shell.toggle_maximized(2);
    h.settle();
    h.app.run("window.workspace.reset", json!({})).unwrap();
    h.settle();
    let shell = h.state();
    assert_eq!(shell.tree, tree::Node::default_layout());
    assert_eq!(shell.strip, shell::strip::Strip::default_for(&shell.tree));
    assert!(shell.floats.is_empty() && shell.float_strips.is_empty() && shell.views.is_empty() && shell.maximized.is_none());
    let st = h.app.session.active().unwrap();
    assert!(std::sync::Arc::ptr_eq(&st.doc, &doc), "the document untouched");
    assert_eq!(format!("{:?}", st.doc), hash);
    assert_eq!((&st.selection, st.is_dirty()), (&selection, dirty));
    // The Window menu's Reset names the shell's workspace.
    assert_eq!(crate::menus::display_label(&h.app, "window.workspace.reset", "Reset Essentials"), "Reset Design");
}

#[test]
fn a_box_maximizes_and_restores_to_the_identical_layout() {
    let mut h = Harness::new();
    h.shell(true);
    let before = h.tree().clone();
    let rects: Vec<(Hit, Rect)> = h.hits().into_iter().filter(|(hit, _)| matches!(hit, Hit::Box(_) | Hit::Board)).collect();
    // A double-click on the upper box's tab row, right of its pills.
    let row = h.rect(&Hit::TabRow(2));
    let pills = h.tab("artboards");
    let at = pos2(f32::midpoint(pills.right(), row.right()), row.center().y);
    h.double_click(at);
    assert_eq!(h.state().maximized, Some(2));
    assert_eq!(h.tree(), &before, "the tree is kept as it was");
    let big = h.rect(&Hit::Box(2));
    assert!((big.width() - (h.screen.x - 80.0)).abs() < 1.0);
    assert!(!h.hits().iter().any(|(hit, _)| matches!(hit, Hit::Board | Hit::Box(3) | Hit::Divider(_))), "the rest is hidden");
    assert!(!shell::showing(&h.app, "properties"));
    // Persisted.
    let back: crate::state::UiState = serde_json::from_value(serde_json::to_value(&h.app.ui).unwrap()).unwrap();
    assert_eq!(back.varos_shell.maximized, Some(2));
    // The same gesture restores it exactly.
    let row = h.rect(&Hit::TabRow(2));
    let at = pos2(row.right() - 6.0, row.center().y);
    h.double_click(at);
    assert_eq!(h.state().maximized, None);
    assert_eq!(h.tree(), &before);
    let again: Vec<(Hit, Rect)> = h.hits().into_iter().filter(|(hit, _)| matches!(hit, Hit::Box(_) | Hit::Board)).collect();
    assert_eq!(again, rects, "every box where it was");
    // The Board maximized by a double-click on its document tabs' empty space: it fills, with its
    // strip; a hidden panel asked for brings the layout back.
    h.app.ui.varos_shell.toggle_maximized(1);
    h.settle();
    assert_eq!(h.state().maximized, Some(1));
    assert_eq!(h.tree(), &before);
    h.settle();
    assert!(h.board().width() > h.screen.x - 81.0, "{:?}", h.board());
    h.rect(&Hit::Strip);
    h.app.run("window.panel", json!({"panel": "layers"})).unwrap();
    h.settle();
    assert_eq!(h.state().maximized, None);
    assert!(shell::showing(&h.app, "layers"));
    // ⌘⇧M is Object › Transform › Move, so the gesture has no key: a double-click or the ≡ menu.
    assert_eq!(crate::menus::shortcut_of("object.move"), Some("Cmd+Shift+M"));
}

/// Software capture (no GPU): two Properties editors following the selected text, the first with
/// Character collapsed, the second with Quick Actions moved first:
/// `VAROS_SHELL_CAPTURE_SECTIONS=<file.png>`.
#[test]
#[ignore = "optional software visual capture; set VAROS_SHELL_CAPTURE_SECTIONS"]
fn capture_shell_sections() {
    let Some(file) = std::env::var_os("VAROS_SHELL_CAPTURE_SECTIONS").map(std::path::PathBuf::from) else { return };
    let (width, height) = (1440_u32, 900_u32);
    let mut h = Harness::sized_without_document(vec2(width as f32, height as f32));
    h.app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
    h.app.ui.control_bar = true;
    h.app.run("shape.rectangle", json!({"x": 40, "y": 40, "width": 120, "height": 80})).unwrap();
    let b = Harness::id_of(&h.app.run("text.create", json!({"x": 60, "y": 200, "text": "Varos", "size": 32})).unwrap());
    h.settle();
    h.shell(true);
    h.app.run("select.set", json!({"ids": [b]})).unwrap();
    h.app.ui.varos_shell.tree.set_ratio(&[1], 0.15);
    let twin = shell::duplicate_properties(&mut h.app, "properties").unwrap();
    h.app.ui.varos_shell.views.entry("properties".into()).or_default().collapsed.insert(S::Character);
    h.app.ui.varos_shell.views.entry(twin).or_default().move_section(S::QuickActions, Some(S::Transform), S::QuickActions);
    h.settle();
    let pixels = capture(&mut h.app, width, height);
    image::save_buffer(&file, &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

/// Software capture (no GPU): the workspace row with more workspaces than fit (`›` at its end):
/// `VAROS_SHELL_CAPTURE_WORKSPACES=<file.png>`.
#[test]
#[ignore = "optional software visual capture; set VAROS_SHELL_CAPTURE_WORKSPACES"]
fn capture_shell_workspaces() {
    let Some(file) = std::env::var_os("VAROS_SHELL_CAPTURE_WORKSPACES").map(std::path::PathBuf::from) else { return };
    let (width, height) = (1440_u32, 900_u32);
    let mut h = Harness::sized_without_document(vec2(width as f32, height as f32));
    h.app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
    h.app.ui.control_bar = true;
    h.settle();
    h.shell(true);
    for name in [
        "Logo sketches",
        "Arabic type",
        "Packaging",
        "Social posts",
        "Print proofs",
        "Icons",
        "Brand book",
        "Web export",
        "Kashida tests",
        "Review",
        "Motion frames",
        "Client edits",
        "Archive 2026",
        "Calligraphy",
        "Posters A3",
        "Final checks",
    ] {
        h.app.ui.varos_shell.save_workspace(name).unwrap();
    }
    h.app.run("window.workspace", json!({"name": "Design"})).unwrap();
    h.settle();
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::Workspaces));
    let pixels = capture(&mut h.app, width, height);
    image::save_buffer(&file, &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

/// The Board maximizes like a box: a double-click on its document tabs' empty space, or its ≡
/// (at the top of its strip) › Maximize Board; the same gesture, or ≡ › Restore Layout, brings
/// back the identical layout.
#[test]
fn the_board_maximizes_from_its_header_and_its_menu() {
    use egui::accesskit::Role;
    let mut h = Harness::new();
    h.shell(true);
    let rects: Vec<(Hit, Rect)> = h.hits().into_iter().filter(|(hit, _)| matches!(hit, Hit::Box(_) | Hit::Board)).collect();
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::BoardHeader));
    // ≡ › Maximize Board, then ≡ › Restore Layout.
    h.ctx.enable_accesskit();
    let pick = |h: &mut Harness, label: &str| {
        let menu = h.rect(&Hit::BoardMenu);
        h.click(menu.center());
        let out = h.frame_full(vec![]);
        let nodes = out.platform_output.accesskit_update.expect("accesskit is on").nodes;
        let item = nodes
            .iter()
            .filter(|(_, n)| n.role() == Role::Button && n.label().is_some_and(|l| l.trim() == label))
            .find_map(|(_, n)| n.bounds().map(|b| Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))))
            .unwrap_or_else(|| panic!("{label} in the Board's menu"));
        h.click(item.center());
    };
    pick(&mut h, "Maximize Board");
    assert_eq!(h.state().maximized, Some(1));
    pick(&mut h, "Restore Layout");
    assert_eq!(h.state().maximized, None);
    let again: Vec<(Hit, Rect)> = h.hits().into_iter().filter(|(hit, _)| matches!(hit, Hit::Box(_) | Hit::Board)).collect();
    assert_eq!(again, rects, "every box where it was");
}

/// Save Workspace (`+`) while Design has changes: Design remembers them (switching back shows
/// them), the new workspace starts with them, Reset Design drops them.
#[test]
fn saving_a_workspace_keeps_the_changes_of_the_one_left() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.varos_shell.add_tab(2, "stroke");
    h.app.ui.varos_shell.tree.set_ratio(&[], 0.6);
    let design = h.state().capture("Design");
    h.app.run("window.workspace.new", json!({"name": "Custom"})).unwrap();
    assert_eq!(h.state().workspace, "Custom");
    h.app.run("window.workspace", json!({"name": "Typography"})).unwrap();
    h.app.run("window.workspace", json!({"name": "Design"})).unwrap();
    assert_eq!(h.state().tree, design.tree, "Design as it was left");
    h.app.run("window.workspace", json!({"name": "Custom"})).unwrap();
    assert_eq!(h.state().tree, design.tree, "Custom saved with it");
    h.app.run("window.workspace", json!({"name": "Design"})).unwrap();
    h.app.run("window.workspace.reset", json!({})).unwrap();
    assert_eq!(h.state().tree, tree::Node::default_layout());
    h.state().invariants().unwrap();
}

/// Two Properties editors with different section orders and folds come back as they were after a
/// workspace switch and back.
#[test]
fn two_editors_section_views_survive_a_workspace_switch() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 60, "height": 40})).unwrap();
    let twin = shell::duplicate_properties(&mut h.app, "properties").unwrap();
    h.app.ui.varos_shell.views.entry("properties".into()).or_default().collapsed.insert(S::Appearance);
    h.app.ui.varos_shell.views.entry(twin.clone()).or_default().move_section(S::Align, Some(S::Transform), S::QuickActions);
    h.settle();
    let (first, second) = (h.sections("properties"), h.sections(&twin));
    assert_ne!(first, second);
    let views = h.state().views.clone();
    h.app.run("window.workspace", json!({"name": "Typography"})).unwrap();
    h.settle();
    assert!(!h.state().owns(&twin), "Typography has its own editors");
    h.app.run("window.workspace", json!({"name": "Design"})).unwrap();
    h.settle();
    assert_eq!(h.state().views, views);
    assert_eq!((h.sections("properties"), h.sections(&twin)), (first, second));
    assert!(h.section("properties", S::Appearance).collapsed);
    // And through ui.json.
    let back: crate::state::UiState = serde_json::from_value(serde_json::to_value(&h.app.ui).unwrap()).unwrap();
    assert_eq!(back.varos_shell.views, views);
}

#[test]
fn shell5_tools_belong_to_board_float_and_dock_without_changing_legacy_preferences() {
    let mut h = Harness::new();
    h.app.ui.toolbar_pos = Some([70.0, 160.0]);
    h.app.ui.toolbar_double = true;
    h.shell(true);
    let tools = h.rect(&Hit::Tools);
    assert_eq!(tools.width(), 40.0);
    assert_eq!(tools.right(), h.board().left());
    assert_eq!(tools.left(), 0.0);
    let to = h.board().center();
    h.drag(h.rect(&Hit::ToolsGrip).center(), to);
    assert!(h.state().tools_pos.is_some());
    assert_eq!(h.app.ui.toolbar_pos, Some([70.0, 160.0]));
    assert!(h.app.ui.toolbar_double);
    let back = h.board().left_center() + vec2(4.0, 0.0);
    h.hold_drag(h.rect(&Hit::ToolsGrip).center(), back);
    let (hit, line) = h.drop_line();
    assert!(matches!(hit, Hit::DropLine { vertical: true, target: Target::Dock }));
    assert_eq!(line.height(), h.board().height());
    h.release(back);
    assert_eq!(h.state().tools_pos, None);
    h.shell(false);
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::Tools));
    assert_eq!(h.app.ui.toolbar_pos, Some([70.0, 160.0]));
}

#[test]
fn shell5_v4_migrates_and_v6_tools_roundtrip() {
    let s = shell::persist::VarosShell { tools_pos: Some([120.0, 190.0]), ..Default::default() };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["version"], 7);
    assert_eq!(shell::persist::from_value(&v).tools_pos, s.tools_pos);
    let old: serde_json::Value = serde_json::from_str(include_str!("shell/fixtures/v4.json")).unwrap();
    assert!(old["varos_shell"].get("tools_pos").is_none());
    let back: crate::state::UiState = serde_json::from_value(old).unwrap();
    assert_eq!(back.varos_shell.version, 7);
    assert_eq!(back.varos_shell.tools_pos, None);
    assert_eq!(back.varos_shell.workspace, "Review v4");
    assert_eq!(back.varos_shell.workspaces[0].name, "Review v4");
    assert_eq!(back.varos_shell.workspaces[0].tools_pos, None);
    assert_eq!(back.varos_shell.tree.boxes()[0].id, 41);
}

#[test]
fn shell5_status_never_intersects_boxes_or_floats_even_after_resize() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.varos_shell.tools_pos = Some([9999.0, 9999.0]);
    h.app.ui.varos_shell.floats.push(floats::Float {
        panels: tree::PanelBox { id: 999, tabs: vec!["stroke".into()], active: 0, collapsed: false },
        rect: [9999.0, 9999.0, 260.0, 360.0],
        owner_area: 1,
    });
    h.app.ui.varos_shell.strip.remove("stroke");
    for size in [SCREEN, vec2(900.0, 600.0)] {
        h.screen = size;
        h.settle();
        let status = egui::containers::panel::PanelState::load(&h.ctx, h.root.with_salt(egui::IdSalt::new("status_bar"))).unwrap().outer_rect;
        assert_eq!(status.height(), 24.0);
        for (hit, rect) in h.hits() {
            if matches!(hit, Hit::Board | Hit::Box(_) | Hit::Float(_) | Hit::FloatStrip(_) | Hit::Tools) {
                assert!(!rect.intersects(status), "{hit:?}: {rect:?} intersects {status:?}");
            }
        }
    }
}

#[test]
fn shell5_motion_visible_immediately_final_on_time_and_reduced_motion_instant() {
    let ctx = egui::Context::default();
    let id = egui::Id::unique("test-shell5-motion");
    let sample = |time, requested: Option<&str>, instant| {
        let mut result = None;
        run_ui_without_textures(&ctx, egui::RawInput { time: Some(time), ..Default::default() }, |ui| {
            ui.ctx().global_style_mut(|s| s.animation_time = if instant { 0.0 } else { 0.2 });
            result = shell::motion::flyout(ui.ctx(), id, requested);
        });
        result
    };
    assert!(sample(0.0, Some("stroke"), false).unwrap().1 > 0.0);
    assert_eq!(sample(0.091, Some("stroke"), false).unwrap().1, 1.0);
    assert!(sample(0.10, None, false).is_some());
    assert!(sample(0.161, None, false).is_none());
    assert_eq!(sample(0.2, Some("stroke"), true).unwrap().1, 1.0);
    assert!(sample(0.21, None, true).is_none());
    let opening = sample(0.3, Some("stroke"), false).unwrap();
    let closing = sample(0.32, None, false).unwrap();
    assert!(closing.1 <= opening.1, "interrupting an opening never flashes opaque");
    assert_eq!(closing.2, opening.2, "closing only fades; it does not slide");
    assert!(sample(0.4, None, false).is_none());
    let target = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 300.0));
    let region = Rect::from_min_size(Pos2::ZERO, SCREEN);
    run_ui_without_textures(&ctx, egui::RawInput { time: Some(1.0), ..Default::default() }, |ui| {
        ui.ctx().global_style_mut(|s| s.animation_time = 0.2);
        shell::motion::dropped(ui.ctx(), "stroke", pos2(300.0, 200.0));
        assert_ne!(shell::motion::settle(ui.ctx(), "stroke", target, region), target);
    });
    run_ui_without_textures(&ctx, egui::RawInput { time: Some(1.081), ..Default::default() }, |ui| {
        assert_eq!(shell::motion::settle(ui.ctx(), "stroke", target, region), target);
    });
}

#[test]
#[ignore = "optional final capture; set VAROS_SHELL_CAPTURE_FINAL"]
fn capture_shell5_final() {
    let Some(file) = std::env::var_os("VAROS_SHELL_CAPTURE_FINAL").map(std::path::PathBuf::from) else { return };
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    let pixels = capture(&mut h.app, 1440, 1000);
    image::save_buffer(file, &pixels, 1440, 1000, image::ColorType::Rgba8).unwrap();
}

#[test]
fn shell5_tool_selection_flyout_and_picker_use_existing_behaviour() {
    let mut h = Harness::new();
    h.shell(true);
    let mut buttons = crate::toolbar::tests::widget_rects(&h.ctx, vec2(28.0, 28.0));
    buttons.retain(|r| h.rect(&Hit::Tools).contains_rect(*r));
    buttons.dedup();
    assert!(buttons.len() > 4);
    h.click(buttons[3].center());
    assert_eq!(h.app.session.tool_id(), "rectangle");
    let triangle = buttons[3].right_bottom() - vec2(2.0, 2.0);
    let output = h.frame_full(vec![egui::Event::PointerMoved(triangle), button(triangle, true)]);
    assert!(h.app.ui.flyout.is_some());
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("tool-flyout"));
    assert!(egui::AreaState::load(&h.ctx, layer.id).is_some(), "flyout has an area on the press frame");
    assert!(
        output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("Rounded Rectangle"))),
        "flyout body paints on press frame"
    );
    h.frame(vec![button(triangle, false)]);
    h.app.ui.flyout = None;
    h.settle();
    let tools = h.rect(&Hit::Tools);
    let fill = h
        .ctx
        .data(|d| d.get_temp::<Vec<(Rect, bool, u64)>>(egui::Id::unique("picker-wells")))
        .unwrap_or_default()
        .into_iter()
        .filter(|(r, stroke, _)| !stroke && r.right() <= tools.right() + 1.0)
        .max_by(|a, b| a.0.top().total_cmp(&b.0.top()))
        .expect("the fill well")
        .0;
    h.click(fill.center());
    assert!(h.app.ui.varos_picker_anchor.is_some());
}

#[test]
fn shell5_app_bar_accessible_home_search_and_undo_without_workspace_switcher() {
    use egui_kittest::kittest::Queryable;
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({})).unwrap();
    app.ui.varos_shell.enabled = true;
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(crate::theme::install_fonts)
        .with_size(vec2(1600.0, 100.0))
        .build_ui_state(|ui, app: &mut VectorcraftApp| crate::chrome::app_bar(app, ui), app);
    h.run_steps(3);
    h.get_by_label("Design").click();
    h.run_steps(2);
    h.get_by_label("Typography").click();
    h.run_steps(2);
    assert_eq!(h.state().ui.varos_shell.workspace, "Typography");
    h.get_by_label("Home").click();
    h.run_steps(2);
    assert!(crate::varos::home::is_home(h.state()));
}

#[test]
fn shell5_app_bar_document_title_dirty_dot_and_geometry() {
    let mut h = Harness::new();
    h.shell(true);
    let title = h.app.session.active().unwrap().title();
    assert!(h.texts().iter().any(|s| s == &title));
    h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap();
    assert!(h.texts().iter().any(|s| s == &format!("{title} •")));
    let bar = egui::containers::panel::PanelState::load(&h.ctx, h.root.with_salt(egui::IdSalt::new("app_bar"))).unwrap().outer_rect;
    assert_eq!(bar.height(), 40.0);
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::Workspaces));
    assert!(h.board().top() >= bar.bottom());
}

#[test]
fn shell5_hidden_host_does_not_resurrect_a_closing_flyout() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.settle();
    h.shell(false);
    shell::set_enabled(&mut h.app, true);
    h.frame(vec![]);
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::Flyout));
}

#[test]
fn shell5_collapse_paints_the_flyout_through_the_close_event_then_fades_out() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.settle();
    let at = h.rect(&Hit::FlyoutClose).center();
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    h.frame(vec![button(at, false)]);
    assert!(h.state().open.is_none());
    h.rect(&Hit::Flyout);
    h.time += 0.011;
    h.frame(vec![]);
    assert!(!h.hits().iter().any(|(hit, _)| *hit == Hit::Flyout));
}

#[test]
fn shell5_review_tools_escape_workspace_and_screen_modes() {
    let mut h = Harness::new();
    h.shell(true);
    let to = h.board().center();
    h.hold_drag(h.rect(&Hit::ToolsGrip).center(), to);
    assert!(h.state().drag.is_some());
    h.frame(vec![escape_key()]);
    h.release(to);
    assert_eq!(h.state().tools_pos, None);
    h.app.ui.varos_shell.tools_pos = Some([220.0, 180.0]);
    h.app.run("window.workspace", json!({"name":"Typography"})).unwrap();
    assert_eq!(h.state().tools_pos, None);
    h.app.ui.varos_shell.tools_pos = Some([330.0, 210.0]);
    h.app.run("window.workspace", json!({"name":"Design"})).unwrap();
    assert_eq!(h.state().tools_pos, Some([220.0, 180.0]));
    h.app.run("window.workspace", json!({"name":"Typography"})).unwrap();
    assert_eq!(h.state().tools_pos, Some([330.0, 210.0]));
    for mode in [1, 2] {
        h.app.ui.screen_mode = mode;
        h.settle();
        let before = h.state().tools_pos;
        h.hold_drag(h.rect(&Hit::ToolsGrip).center(), to);
        assert!(h.state().drag.is_none());
        h.release(to);
        assert_eq!(h.state().tools_pos, before);
    }
}

#[test]
fn shell5_review_real_drop_never_moves_an_existing_box() {
    for instant in [false, true] {
        let mut h = Harness::new();
        h.shell(true);
        h.ctx.global_style_mut(|s| s.animation_time = if instant { 0.0 } else { 0.2 });
        let rect = h.rect(&Hit::Box(3));
        let to = h.tab("properties").center();
        h.hold_drag(h.tab("artboards").center(), to);
        h.frame(vec![button(to, false)]);
        for _ in 0..12 {
            assert_eq!(h.rect(&Hit::Box(3)), rect);
            h.time -= 0.04;
            h.frame(vec![]);
        }
        assert_eq!(h.front(3), Some("artboards"));
    }
}

#[test]
fn shell5_review_character_paragraph_options_in_all_hosts() {
    for (panel, key) in [("character", "ch-hide-options"), ("paragraph", "pa-hide-options")] {
        for host in 0..3 {
            let mut h = Harness::new();
            h.shell(true);
            if host == 0 {
                h.app.ui.varos_shell.add_tab(3, panel);
                h.app.ui.varos_shell.set_active(3, 3);
            } else if host == 1 {
                h.app.ui.varos_shell.take(panel);
                h.app.ui.varos_shell.floats.push(floats::Float {
                    panels: tree::PanelBox { id: 999, tabs: vec![panel.into()], active: 0, collapsed: false },
                    rect: [250.0, 200.0, 300.0, 400.0],
                    owner_area: 1,
                });
            } else {
                h.app.run("window.panel", json!({"panel":panel})).unwrap();
            }
            h.settle();
            let pill = if host == 2 { h.rect(&Hit::FlyoutTab(panel.into())) } else { h.tab(panel) };
            let at = pos2(pill.right() - 9.0, pill.center().y);
            h.click(at);
            assert!(crate::panels::pstate::<bool>(&h.ctx, key), "{panel} host {host}");
            h.click(at);
            assert!(!crate::panels::pstate::<bool>(&h.ctx, key));
        }
    }
}

#[test]
fn shell5_review_native_menu_and_chrome_hit_areas() {
    struct Native;
    impl crate::native_menu::Backend for Native {
        fn sync(&mut self, _: &crate::native_menu::MenuBar) {}
        fn drain(&mut self) -> Vec<crate::native_menu::Event> {
            Vec::new()
        }
    }
    for native in [false, true] {
        let mut h = Harness::new();
        h.shell(true);
        if native {
            h.app.services.native_menu = Some(crate::native_menu::NativeMenu::new(Box::new(Native)));
        }
        h.settle();
        assert_eq!(h.texts().iter().any(|t| t == "File"), !native);
        h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
        h.app.ui.varos_shell.take("gradient");
        h.app.ui.varos_shell.floats.push(floats::Float {
            panels: tree::PanelBox { id: 999, tabs: vec!["gradient".into()], active: 0, collapsed: false },
            rect: [250.0, 200.0, 300.0, 400.0],
            owner_area: 1,
        });
        h.app.ui.varos_shell.take("transform");
        h.app.ui.varos_shell.float_strips.push(floats::FloatStrip {
            id: 998,
            pos: [650.0, 200.0],
            groups: vec![shell::strip::Group { panels: vec!["transform".into()], temporary: false, width: 260.0, height: 360.0 }],
        });
        h.settle();
        for (hit, rect) in h.hits() {
            if matches!(
                hit,
                Hit::FlyoutClose
                    | Hit::Menu(_)
                    | Hit::BoardMenu
                    | Hit::ToolsGrip
                    | Hit::FloatClose(_)
                    | Hit::FloatToIcon(_)
                    | Hit::FloatStripClose(_)
                    | Hit::FloatStripExpand(_)
            ) {
                assert!(rect.width() >= 16.0 && rect.height() >= 16.0, "{hit:?}: {rect:?}");
            }
        }
    }
}

#[test]
fn shell5_review_reduced_motion_settle_is_exact() {
    let ctx = egui::Context::default();
    run_ui_without_textures(&ctx, Default::default(), |ui| {
        ctx.global_style_mut(|s| s.animation_time = 0.0);
        let target = Rect::from_min_size(pos2(100.0, 100.0), vec2(100.0, 100.0));
        shell::motion::dropped(&ctx, "new-box", pos2(300.0, 300.0));
        assert_eq!(shell::motion::settle(&ctx, "new-box", target, ui.max_rect()), target);
    });
}

fn review_open_tool_flyout(h: &mut Harness) {
    let mut buttons = crate::toolbar::tests::widget_rects(&h.ctx, vec2(28.0, 28.0));
    buttons.retain(|r| h.rect(&Hit::Tools).contains_rect(*r));
    buttons.dedup();
    let at = buttons[3].right_bottom() - vec2(2.0, 2.0);
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    h.frame(vec![button(at, false)]);
    h.settle();
    assert!(h.app.ui.flyout.is_some());
}

#[test]
fn shell5_review_real_close_opacity_and_tear_off() {
    let mut h = Harness::new();
    h.shell(true);
    review_open_tool_flyout(&mut h);
    let opacity = |h: &Harness, key| h.ctx.data(|d| d.get_temp::<f32>(egui::Id::unique(key))).unwrap();
    let open = opacity(&h, "review-tool-opacity");
    h.app.ui.flyout = None;
    h.time -= 0.04;
    h.frame(vec![]);
    assert!(opacity(&h, "review-tool-opacity") >= open - 0.01 / 0.06 - 0.01);
    h.settle();
    review_open_tool_flyout(&mut h);
    let area = egui::AreaState::load(&h.ctx, egui::Id::unique("tool-flyout")).unwrap().rect();
    let at = pos2(area.right() - 8.0, area.center().y);
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    h.time -= 0.04;
    h.frame(vec![button(at, false)]);
    assert_eq!(h.app.ui.floating_flyouts.len(), 1);
    assert!(h.app.ui.flyout.is_none());
    assert!(shell::motion::flyout(&h.ctx, egui::Id::unique("shell-tool-flyout-motion"), None).is_none());
    h.time -= 0.04;
    let output = h.frame_full(vec![]);
    assert!(
        !output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("Rounded Rectangle"))),
        "no docked copy during the old fade interval"
    );
    h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
    h.settle();
    let open = opacity(&h, "review-strip-opacity");
    let at = h.rect(&Hit::FlyoutClose).center();
    h.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
    h.time -= 0.04;
    h.frame(vec![button(at, false)]);
    assert!(opacity(&h, "review-strip-opacity") >= open - 0.01 / 0.06 - 0.01);
}

#[test]
fn shell5_review_strip_slide_toward_its_rail_and_exact_final_rect() {
    for floating in [true, false] {
        let mut h = Harness::new();
        h.shell(true);
        if floating {
            h.app.ui.varos_shell.strip.remove("stroke");
            h.app.ui.varos_shell.float_strips.push(floats::FloatStrip {
                id: 999,
                pos: [250.0, 200.0],
                groups: vec![shell::strip::Group { panels: vec!["stroke".into()], temporary: false, width: 260.0, height: 360.0 }],
            });
        }
        h.app.run("window.panel", json!({"panel":"stroke"})).unwrap();
        h.frame(vec![]);
        let first = h.rect(&Hit::Flyout);
        h.time += 0.10;
        h.frame(vec![]);
        let end = h.rect(&Hit::Flyout);
        if floating {
            assert!(first.left() < end.left());
        } else {
            assert!(first.left() > end.left());
        }
        h.frame(vec![]);
        assert_eq!(h.rect(&Hit::Flyout), end);
    }
}

#[test]
fn shell5_review_bottom_controls_and_fill_stroke_visibility() {
    let mut h = Harness::new();
    h.shell(true);
    let edit: Rect = h.ctx.data(|d| d.get_temp(crate::edit_toolbar::button_id())).unwrap();
    assert!(h.rect(&Hit::Tools).contains_rect(edit), "tools {:?}, edit {edit:?}", h.rect(&Hit::Tools));
    h.ctx.enable_accesskit();
    h.click(edit.center());
    let out = h.frame_full(vec![]);
    let nodes = out.platform_output.accesskit_update.unwrap().nodes;
    let bounds = nodes
        .iter()
        .find_map(|(_, n)| (n.label() == Some("Edit Toolbar") && n.bounds().is_some_and(|r| r.x1 - r.x0 > 28.0)).then(|| n.bounds().unwrap()))
        .unwrap();
    h.click(pos2(((bounds.x0 + bounds.x1) / 2.0) as f32, ((bounds.y0 + bounds.y1) / 2.0) as f32));
    assert!(h.app.ui.tools_drawer);
    h.app.ui.tools_drawer = false;
    // Leave room for the full content, so this checks visibility rather than viewport clamping.
    h.screen = vec2(SCREEN.x, 1400.0);
    h.app.ui.varos_shell.tools_pos = Some([250.0, 180.0]);
    h.settle();
    let with = h.rect(&Hit::Tools).height();
    h.app.ui.toolbar_show.fill_stroke = false;
    h.settle();
    assert_eq!(h.rect(&Hit::Tools).height(), with - 42.0, "hiding the wells drops the 8 pt gap and the 34 pt cluster");
    h.ctx.enable_accesskit();
    let out = h.frame_full(vec![]);
    let nodes = out.platform_output.accesskit_update.unwrap().nodes;
    for label in ["Color (,)", "Gradient (.)", "None (/)", "Draw Normal (Shift+D)"] {
        assert!(nodes.iter().any(|(_, n)| n.label() == Some(label)), "{label}");
    }
}

#[test]
fn shell5_review_real_drop_new_box_settles_and_reduced_motion_snaps() {
    for instant in [false, true] {
        let mut h = Harness::new();
        h.shell(true);
        h.ctx.global_style_mut(|s| s.animation_time = if instant { 0.0 } else { 0.2 });
        let to = h.rect(&Hit::Box(3)).left_center() + vec2(3.0, 0.0);
        let before: Vec<_> = h.tree().boxes().iter().map(|b| b.id).collect();
        h.hold_drag(h.tab("artboards").center(), to);
        assert!(matches!(h.drop_line().0, Hit::DropLine { target: Target::Edge { .. }, .. }));
        h.frame(vec![button(to, false)]);
        let id = h.tree().boxes().iter().find(|b| !before.contains(&b.id)).unwrap().id;
        h.time -= 0.04;
        h.frame(vec![]);
        let early = h.rect(&Hit::Box(id));
        h.time += 0.1;
        h.frame(vec![]);
        let end = h.rect(&Hit::Box(id));
        if instant {
            assert_eq!(early, end);
        } else {
            assert_ne!(early, end);
        }
        h.frame(vec![]);
        assert_eq!(h.rect(&Hit::Box(id)), end);
    }
}

#[test]
fn simple_tools_columns_shift_canvas_and_survive_workspace_roundtrip() {
    let mut h = Harness::new();
    h.shell(true);
    let mut left = Vec::new();
    for columns in 1..=3 {
        h.app.ui.varos_shell.tools_columns = columns;
        h.settle();
        assert_eq!(h.rect(&Hit::Tools).width(), 40.0 + 28.0 * f32::from(columns - 1));
        left.push(h.rect(&Hit::BoardBody).left());
    }
    assert_eq!(left[1] - left[0], 28.0);
    assert_eq!(left[2] - left[1], 28.0);
    h.app.run("window.workspace", json!({"name":"Typography"})).unwrap();
    assert_eq!(h.state().tools_columns, 1);
    h.app.run("window.workspace", json!({"name":"Design"})).unwrap();
    assert_eq!(h.state().tools_columns, 3);
    let back: crate::state::UiState = serde_json::from_value(serde_json::to_value(&h.app.ui).unwrap()).unwrap();
    assert_eq!(back.varos_shell.tools_columns, 3);
    let old: crate::state::UiState = serde_json::from_str(include_str!("shell/fixtures/v5.json")).unwrap();
    assert_eq!(old.varos_shell.version, 7);
    assert_eq!(old.varos_shell.tools_columns, 1);
    assert_eq!(old.varos_shell.tools_pos, Some([80.0, 120.0]));
}

#[test]
fn simple_control_opens_from_strip_and_command_and_floats() {
    let mut h = Harness::new();
    h.shell(true);
    assert!(!shell::showing(&h.app, "control"));
    h.app.run("window.panel", json!({"panel":"control"})).unwrap();
    h.settle();
    assert!(shell::showing(&h.app, "control"));
    assert!(h.texts().iter().any(|t| t == "Snapping"));
    h.click(h.rect(&Hit::StripIcon("control".into())).center());
    assert!(!shell::showing(&h.app, "control"));
    h.click(h.rect(&Hit::StripIcon("control".into())).center());
    assert!(shell::showing(&h.app, "control"));
    h.drag(h.rect(&Hit::FlyoutTab("control".into())).center(), h.rect(&Hit::BoardBody).center());
    assert!(h.state().float_of("control").is_some());
    h.state().invariants().unwrap();
}

#[test]
fn simple_flyout_clear_of_strips_and_no_text_glyph_controls() {
    let mut h = Harness::new();
    h.shell(true);
    h.click(h.rect(&Hit::StripIcon("stroke".into())).center());
    let flyout = h.rect(&Hit::Flyout);
    assert!(!flyout.intersects(h.rect(&Hit::Strip)));
    assert!(!flyout.intersects(h.rect(&Hit::Tools)));
    assert_eq!(flyout.width(), 240.0);
    h.app.run("window.floatTools", json!({"tool":"rectangle", "floating":true})).unwrap();
    h.settle();
    for text in h.texts() {
        assert!(!["<<", ">>", "×", "≡", "◇", "⋮"].contains(&text.as_str()), "{text}");
    }
}

#[test]
fn simple_overflow_menu_selects_hidden_tab_with_pointer() {
    use egui_kittest::kittest::Queryable;
    let tabs = vec![
        ("color".into(), "Color".into()),
        ("colorGuide".into(), "Color Guide".into()),
        ("stroke".into(), "Stroke".into()),
        ("swatches".into(), "Swatches".into()),
    ];
    let mut h = egui_kittest::Harness::builder().with_context_setup(crate::theme::install_fonts).with_size(vec2(160.0, 100.0)).build_ui_state(
        move |ui, active: &mut usize| {
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                ui.ctx().style_mut_of(theme, |s| {
                    s.interaction.tooltip_delay = 0.0;
                    s.interaction.show_tooltips_only_when_still = false;
                });
            }
            let row = Rect::from_min_size(ui.cursor().min, vec2(160.0, 36.0));
            let t = crate::theme::Tokens::get(ui.ctx());
            let out = boxes::tab_row(ui, &tabs, *active, row, &t);
            if *active == 0 && out.clicked.is_none() {
                assert!(!out.tabs[3].is_positive(), "Swatches starts in overflow");
            }
            if let Some(i) = out.clicked {
                *active = i;
            }
        },
        0usize,
    );
    h.run_steps(3);
    h.get_by_label("More tabs").hover();
    h.run_steps(4);
    let _ = h.get_by_label_contains("more panels");
    h.get_by_label("More tabs").click();
    h.run_steps(2);
    h.get_by_label("Swatches").click();
    h.run_steps(2);
    assert_eq!(*h.state(), 3);
}

#[test]
fn an_active_pill_that_does_not_fit_is_ellipsized_beside_the_overflow() {
    use egui_kittest::kittest::Queryable;
    let tabs = vec![("colorGuide".into(), "Color Guide".into())];
    let mut h = egui_kittest::Harness::builder().with_context_setup(crate::theme::install_fonts).with_size(vec2(70.0, 80.0)).build_ui_state(
        move |ui, _active: &mut usize| {
            let row = Rect::from_min_size(ui.cursor().min, vec2(70.0, 36.0));
            let t = crate::theme::Tokens::get(ui.ctx());
            let out = boxes::tab_row(ui, &tabs, 0, row, &t);
            assert!(out.tabs[0].is_positive(), "the active pill is shown, truncated");
            assert!(out.tabs[0].width() < 50.0, "narrower than the full label: {:?}", out.tabs[0]);
        },
        0usize,
    );
    h.run_steps(3);
    let _ = h.get_by_label_contains("…");
    let _ = h.get_by_label("More tabs");
}

#[test]
#[ignore = "software captures; VAROS_SHELL_CAPTURE_SIMPLE and VAROS_SHELL_CAPTURE_SIMPLE_COLS"]
fn capture_shell_simple() {
    for (key, columns) in [("VAROS_SHELL_CAPTURE_SIMPLE", 1), ("VAROS_SHELL_CAPTURE_SIMPLE_COLS", 3)] {
        let Some(path) = std::env::var_os(key).map(std::path::PathBuf::from) else { continue };
        let mut h = Harness::sized_without_document(vec2(1440.0, 900.0));
        h.app.run("file.new", json!({"width":600,"height":760})).unwrap();
        h.app.run("shape.rectangle", json!({"x":52,"y":250,"width":420,"height":230})).unwrap();
        h.app.run("paint.setFill", json!({"color":"#d65042"})).unwrap();
        h.shell(true);
        h.app.ui.varos_shell.tree = tree::Node::default_layout();
        h.app.ui.varos_shell.strip = shell::strip::Strip::default_for(&h.app.ui.varos_shell.tree);
        h.app.ui.varos_shell.tools_columns = columns;
        h.app.run("window.panel", json!({"panel":"color"})).unwrap();
        h.settle();
        let pixels = capture(&mut h.app, 1440, 900);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        image::save_buffer(path, &pixels, 1440, 900, image::ColorType::Rgba8).unwrap();
    }
}

#[test]
#[ignore = "software capture; VAROS_SHELL_CAPTURE_PICKER_OK"]
fn capture_shell_picker_ok() {
    let Some(path) = std::env::var_os("VAROS_SHELL_CAPTURE_PICKER_OK").map(std::path::PathBuf::from) else { return };
    let mut h = Harness::sized_without_document(vec2(1440.0, 900.0));
    h.app.run("file.new", json!({"width": 600, "height": 760})).unwrap();
    h.app.run("shape.rectangle", json!({"x": 52, "y": 250, "width": 420, "height": 230})).unwrap();
    h.app.run("paint.setFill", json!({"color": "#d65042"})).unwrap();
    h.shell(true);
    h.settle();
    let tools = h.rect(&Hit::Tools);
    let fill = h
        .ctx
        .data(|d| d.get_temp::<Vec<(Rect, bool, u64)>>(egui::Id::unique("picker-wells")))
        .unwrap()
        .into_iter()
        .filter(|(r, stroke, _)| !*stroke && tools.intersects(*r))
        .max_by(|a, b| a.0.top().total_cmp(&b.0.top()))
        .unwrap()
        .0;
    h.click(fill.center());
    let pixels = capture(&mut h.app, 1440, 900);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    image::save_buffer(path, &pixels, 1440, 900, image::ColorType::Rgba8).unwrap();
}

#[test]
fn simple_flat_boxes_and_pointer_divider_feedback() {
    let mut h = Harness::new();
    h.shell(true);
    let seam = h.rect(&Hit::Divider(vec![]));
    let t = crate::theme::Tokens::get(&h.ctx);
    let strokes = |out: &egui::FullOutput| -> Vec<egui::Stroke> {
        out.shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::LineSegment { points, stroke }
                    if (points[0].x - seam.center().x).abs() < 0.1
                        && (points[1].x - seam.center().x).abs() < 0.1
                        && (points[0].y - seam.top()).abs() < 0.1
                        && (points[1].y - seam.bottom()).abs() < 0.1 =>
                {
                    Some(*stroke)
                }
                _ => None,
            })
            .collect()
    };
    let out = h.frame_full(vec![egui::Event::PointerMoved(pos2(0.0, 0.0))]);
    assert_eq!(strokes(&out), vec![egui::Stroke::new(1.0, t.border)]);
    for (hit, rect) in h.hits() {
        if matches!(hit, Hit::Box(_) | Hit::Board) {
            assert!(
                !out.shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::Rect(r) if r.rect == rect && r.stroke.width > 0.0)),
                "docked frame {hit:?}"
            );
        }
    }
    h.frame(vec![egui::Event::PointerMoved(seam.center())]);
    let out = h.frame_full(vec![]);
    assert_eq!(strokes(&out), vec![egui::Stroke::new(2.0, t.hover)]);
}

#[test]
fn simple_columns_menu_uses_pointer_and_no_fixed_control_row() {
    use egui_kittest::kittest::Queryable;
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.ui.varos_shell.enabled = true;
    let mut h = egui_kittest::Harness::builder().with_context_setup(crate::theme::install_fonts).with_size(vec2(240.0, 160.0)).build_ui_state(
        |ui, app: &mut VectorcraftApp| {
            let t = crate::theme::Tokens::get(ui.ctx());
            crate::toolbar::bottom_controls(app, ui, &t, 1);
        },
        app,
    );
    h.run_steps(3);
    for (label, columns) in [("3 columns", 3), ("2 columns", 2), ("1 column", 1)] {
        h.get_by_label("Edit Toolbar").click();
        h.run_steps(2);
        h.get_by_label(label).click();
        h.run_steps(2);
        assert_eq!(h.state().ui.varos_shell.tools_columns, columns);
    }
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.control_bar = true;
    h.settle();
    assert!(!h.hits().iter().any(|(hit, _)| matches!(hit, Hit::Workspaces | Hit::BoardHeader)));
    let bar = egui::containers::panel::PanelState::load(&h.ctx, h.root.with_salt(egui::IdSalt::new("app_bar"))).unwrap().outer_rect;
    assert_eq!(h.board().top(), bar.bottom());
}

#[test]
fn simple_only_top_strip_grip_drags_and_legacy_icons_keep_their_sources() {
    let mut h = Harness::new();
    h.shell(true);
    let before = h.state().strip.clone();
    let to = h.rect(&Hit::BoardBody).center();
    h.hold_drag(h.rect(&Hit::StripGroup(1)).center(), to);
    assert!(h.state().drag.is_none(), "a separator is not a grip");
    h.release(to);
    assert_eq!(h.state().strip, before);
    h.hold_drag(h.rect(&Hit::StripGroup(0)).center(), to);
    assert!(h.state().drag.is_some());
    h.frame(vec![escape_key()]);
    h.release(to);
    assert_eq!(h.state().strip, before);
    for icon in ["chevrons-left", "chevrons-right", "sparkles", "sliders-horizontal"] {
        assert!(crate::varos::icons::source(icon).is_none(), "legacy icon must retain its original source: {icon}");
        assert!(crate::varos::icons::source(&format!("shell-{icon}")).is_some());
    }
}

#[test]
fn simple_control_docks_duplicates_and_returns_to_strip_after_close() {
    let mut h = Harness::new();
    h.shell(true);
    h.click(h.rect(&Hit::StripIcon("control".into())).center());
    let from = h.rect(&Hit::FlyoutTab("control".into())).center();
    let row = h.rect(&Hit::TabRow(2));
    h.drag(from, pos2(row.right() - 4.0, row.center().y));
    assert_eq!(h.tree().find_tab("control"), Some(2));
    assert!(h.texts().iter().any(|t| t == "Snapping"));
    let copy = shell::duplicate_properties(&mut h.app, "control").unwrap();
    assert_eq!(copy, "control@2");
    h.settle();
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    let back: crate::state::UiState = serde_json::from_value(saved).unwrap();
    assert!(back.varos_shell.owns("control@2"));
    h.app.run("window.panel.close", json!({"panel":"control"})).unwrap();
    h.settle();
    h.click(h.rect(&Hit::StripIcon("control".into())).center());
    assert!(shell::showing(&h.app, "control"));
    h.state().invariants().unwrap();
}

#[test]
fn simple_top_bar_document_tabs_switch_and_new_button_opens_dialog() {
    use egui_kittest::kittest::Queryable;
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.ui.varos_shell.enabled = true;
    app.run("file.new", json!({"width":300,"height":300})).unwrap();
    let first = app.session.active().unwrap().title();
    app.run("file.new", json!({"width":400,"height":400})).unwrap();
    let second = app.session.active().unwrap().title();
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(crate::theme::install_fonts)
        .with_size(vec2(1440.0, 900.0))
        .build_ui_state(|ui, app: &mut VectorcraftApp| crate::chrome::app_bar(app, ui), app);
    h.run_steps(3);
    h.get_by_label(&first).click();
    h.run_steps(2);
    assert_eq!(h.state().session.active_index(), Some(0));
    h.get_by_label(&second).click();
    h.run_steps(2);
    assert_eq!(h.state().session.active_index(), Some(1));
    h.get_by_label("New Document").click();
    h.run_steps(2);
    assert!(h.state().ui.dialog.is_some());
}

#[test]
fn simple_tools_dock_at_the_window_edge_with_a_panel_left_of_the_board() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.varos_shell.tree = tree::Node::default_layout();
    h.app.ui.varos_shell.strip = shell::strip::Strip::default_for(&h.app.ui.varos_shell.tree);
    h.settle();
    h.drag(h.rect(&Hit::ToolsGrip).center(), h.rect(&Hit::BoardBody).center());
    assert!(h.state().tools_pos.is_some());
    let to = pos2(4.0, h.board().center().y);
    h.hold_drag(h.rect(&Hit::ToolsGrip).center(), to);
    let (hit, line) = h.drop_line();
    assert!(matches!(hit, Hit::DropLine { vertical: true, target: Target::Dock }));
    assert_eq!(line.center().x, 0.0);
    h.release(to);
    assert_eq!(h.state().tools_pos, None);
    assert_eq!(h.rect(&Hit::Tools).left(), 0.0);
    assert!(h.board().left() > h.rect(&Hit::Tools).right() + 190.0);
}

#[test]
fn simple_review_home_has_one_bar_and_editor_keeps_its_bar() {
    let mut h = Harness::without_document();
    h.shell(true);
    let output = h.frame_full(vec![]);
    let lines: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::LineSegment { points, .. } if (points[0].y - points[1].y).abs() < 0.1 && (points[1].x - points[0].x).abs() > 1500.0 => {
                Some(points[0].y)
            }
            _ => None,
        })
        .collect();
    assert!(lines.iter().any(|y| (*y - 40.0).abs() < 0.1), "Home bar ends at 40: {lines:?}");
    assert!(!lines.iter().any(|y| *y > 40.1 && *y < 85.0), "no second bar: {lines:?}");
    h.app.run("file.new", json!({})).unwrap();
    h.settle();
    assert!(h.rect(&Hit::Tools).top() >= 40.0);
    assert!(h.ctx.data(|d| d.get_temp::<Rect>(egui::Id::unique("simple-tabs-rect"))).is_some());
}

#[test]
fn simple_review_home_bar_is_the_window_title_bar() {
    // Home's own bar replaces the app bar, so it carries the title-bar duties: window drag, room
    // for the macOS window buttons and the custom caption buttons.
    let mut h = Harness::without_document();
    h.shell(true);
    h.app.custom_titlebar = true;
    h.app.titlebar_inset = 80.0;
    let output = h.frame_full(vec![]);
    h.settle();
    let close = h.ctx.read_response(egui::Id::unique(("titlebar-caption", 2u8))).expect("Home draws the caption buttons").rect;
    assert_eq!((close.right(), close.top()), (SCREEN.x, 0.0), "Close sits in the window's corner");
    let brand = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(text) if text.galley.text() == "Varos" => Some(text.pos.x),
            _ => None,
        })
        .expect("the brand name");
    assert!(brand >= 80.0 + 30.0, "the brand clears the window buttons: {brand}");
    let empty = pos2(600.0, 20.0);
    let cmds: Vec<_> = [
        vec![egui::Event::PointerMoved(empty)],
        vec![button(empty, true)],
        vec![egui::Event::PointerMoved(empty + vec2(12.0, 4.0))],
        vec![egui::Event::PointerMoved(empty + vec2(30.0, 8.0))],
        vec![button(empty + vec2(30.0, 8.0), false)],
    ]
    .into_iter()
    .flat_map(|events| h.frame_full(events).viewport_output.remove(&egui::ViewportId::ROOT).map(|v| v.commands).unwrap_or_default())
    .collect();
    assert!(cmds.iter().any(|c| matches!(c, egui::ViewportCommand::StartDrag)), "Home's bar drags the window: {cmds:?}");
}

#[test]
fn simple_review_multi_column_tool_flyout_clears_strip() {
    for columns in [2, 3] {
        let mut h = Harness::new();
        h.shell(true);
        h.app.ui.varos_shell.tools_columns = columns;
        h.settle();
        review_open_tool_flyout(&mut h);
        let area = egui::AreaState::load(&h.ctx, egui::Id::unique("tool-flyout")).unwrap().rect();
        let strip = h.rect(&Hit::Tools);
        assert!(!area.intersects(strip), "{columns} columns: {area:?} overlaps {strip:?}");
        assert!((area.left() - strip.right() - 6.0).abs() < 0.1, "{columns} columns: {area:?} beside {strip:?}");
        assert!((area.width() - 240.0).abs() < 0.1, "{columns} columns: {area:?}");
    }
}

#[test]
fn simple_review_long_workspace_does_not_overlap_tabs() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.varos_shell.workspace = "W".repeat(60);
    h.settle();
    let output = h.frame_full(vec![]);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "W".repeat(60) && text.galley.elided)),
        "workspace label is visibly ellipsized"
    );
    let rect = |key| h.ctx.data(|d| d.get_temp::<Rect>(egui::Id::unique(key))).unwrap();
    let workspace = rect("simple-workspace-rect");
    assert!(workspace.width() <= 160.0);
    assert!(rect("simple-tabs-rect").right() <= workspace.left());
}

#[test]
fn simple_review_picker_avoids_floating_tools_and_panel_strip() {
    for tools in [true, false] {
        let mut h = Harness::new();
        h.shell(true);
        if tools {
            h.app.ui.varos_shell.tools_pos = Some([400.0, 200.0]);
        } else {
            h.app.ui.varos_shell.take("transform");
            h.app.ui.varos_shell.float_strips.push(floats::FloatStrip {
                id: 998,
                pos: [400.0, 200.0],
                groups: vec![shell::strip::Group { panels: vec!["transform".into()], temporary: false, width: 240.0, height: 360.0 }],
            });
        }
        h.settle();
        h.app.ui.varos_picker_anchor = Some(Rect::from_min_size(pos2(400.0, 170.0), vec2(20.0, 20.0)));
        h.settle();
        let picker = egui::AreaState::load(&h.ctx, egui::Id::unique("varos-picker-popover")).unwrap().rect();
        let strip = h.rect(&if tools { Hit::Tools } else { Hit::FloatStrip(998) });
        assert!(!picker.intersects(strip), "{picker:?} overlaps {strip:?}");
    }
}

#[test]
fn simple_status_bar_shows_the_page_readout_only_with_the_shell() {
    let texts = |out: &egui::FullOutput| -> Vec<String> {
        out.shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect()
    };
    let mut h = Harness::new();
    h.shell(false);
    let readout = crate::varos::pages::status(&h.app).expect("a document has a page readout");
    assert!(readout.starts_with("Page 1 of 1 · Artboard "), "{readout}");
    let off = h.frame_full(vec![]);
    assert!(!texts(&off).contains(&readout), "OFF keeps the artboard list");
    h.shell(true);
    let on = h.frame_full(vec![]);
    assert!(texts(&on).contains(&readout), "{:?}", texts(&on));
}

#[test]
fn simple_pages_tab_is_not_repeated_as_a_header() {
    let count =
        |out: &egui::FullOutput| out.shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::Text(text) if text.galley.text() == "Pages")).count();
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.varos_shell.tree = tree::Node::default_layout();
    h.app.ui.varos_shell.strip = shell::strip::Strip::default_for(&h.app.ui.varos_shell.tree);
    h.settle();
    h.rect(&Hit::Tab { box_id: 4, panel: "pages".into() });
    assert_eq!(count(&h.frame_full(vec![])), 1, "the tab names the Pages panel once");
}

#[test]
fn dropping_a_tab_or_a_float_on_the_strip_docks_it_as_an_icon() {
    let mut h = Harness::new();
    h.shell(true);
    let tab = h.tab("color").center();
    let rail = h.rect(&Hit::Strip);
    let onto = rail.center();
    h.hold_drag(tab, onto);
    assert!(matches!(h.drop_target(), Some(Target::Strip { .. })), "{:?}", h.drop_target());
    let (_, line) = h.drop_line();
    assert!(line.width() > rail.width() * 0.5, "the azure line runs along the strip: {line:?}");
    h.release(onto);
    assert!(h.state().strip.group("color").is_some());
    assert!(h.tree().find_tab("color").is_none());
    // A tab dragged out becomes a floating panel. Dropping it on the strip puts it back in the
    // tab row it left, and the preview line is that slot.
    let box_id = h.tree().find_tab("swatches").unwrap();
    let tabs_before = h.box_tabs(box_id);
    let from = h.tab("swatches").center();
    let away = h.rect(&Hit::BoardBody).center();
    h.drag(from, away);
    let id = h.state().float_of("swatches").expect("dragged out as a panel");
    let title = h.rect(&Hit::FloatTitle(id)).center();
    h.hold_drag(title, onto);
    let (hit, line) = h.drop_line();
    let row = h.rect(&Hit::TabRow(box_id));
    assert!(matches!(hit, Hit::DropLine { target: Target::Tabs { box_id: id, .. }, .. } if id == box_id), "the preview matches the restore: {hit:?}");
    assert!(row.expand(12.0).intersects(line), "the line sits on the original tab row: {line:?} {row:?}");
    h.release(onto);
    assert!(h.state().float_of("swatches").is_none());
    assert_eq!(h.box_tabs(box_id), tabs_before, "<< of a tab dragged out restores that tab");
}

#[test]
fn icons_return_to_the_slot_they_left_on_close_collapse_and_from_a_pill() {
    let mut h = Harness::new();
    h.shell(true);
    let group = h.state().strip.group("stroke").expect("stroke starts in the strip");
    let before = h.state().strip.groups[group].panels.clone();
    let body = h.rect(&Hit::BoardBody).center();
    // close / << : a strip icon floated via a pill returns to that group, not a new one at the end.
    let pill = h.lone_icon("stroke", body + vec2(-160.0, -40.0));
    h.click(h.rect(&Hit::FloatStripExpand(pill)).center());
    let fid = h.state().float_of("stroke").unwrap();
    h.click(h.rect(&Hit::FloatToIcon(fid)).center());
    assert_eq!(h.state().strip.groups[group].panels, before, "<< restores the original group");
    assert!(h.state().float_strips.is_empty());

    // Collapse to icon, after the strip icon was docked into a box, restores the same slot.
    let icon = h.rect(&Hit::StripIcon("stroke".into())).center();
    h.drag(icon, h.tab("properties").center());
    let box_id = h.tree().find_tab("stroke").unwrap();
    assert!(h.app.ui.varos_shell.collapse_box_to_strip(box_id));
    h.settle();
    assert_eq!(h.state().strip.group("stroke"), Some(group), "Collapse to icon does not append a new group");
    assert_eq!(h.state().strip.groups[group].panels, before);

    // A panel dragged out of a pill returns to that pill, not the Board strip.
    let pill = h.lone_icon("stroke", body + vec2(-160.0, -40.0));
    let layers_box = h.tree().find_tab("layers").unwrap();
    let layers_tabs = h.box_tabs(layers_box);
    let fid = h.float_tab("layers", body + vec2(30.0, 20.0));
    let icon = h.rect(&Hit::FloatStripIcon { strip: pill, panel: "stroke".into() }).center();
    h.drag(icon, h.tab("layers").center());
    assert_eq!(h.state().float_of("stroke"), Some(fid));
    h.click(h.rect(&Hit::FloatClose(fid)).center());
    assert!(h.state().float_strip_of("stroke").is_some(), "pill-origin panels do not go to the Board strip");
    assert!(h.state().strip.group("stroke").is_none());
    assert_eq!(h.box_tabs(layers_box), layers_tabs, "the tab dragged out of the box is back");

    // Close removes the pill's panels. Collapse to icon puts them back on the strip.
    let back = h.state().float_strip_of("stroke").unwrap();
    h.app.ui.varos_shell.dismiss_float_strip(back);
    h.settle();
    assert!(h.state().float_strip_of("stroke").is_none() && h.state().strip.group("stroke").is_none() && h.tree().find_tab("stroke").is_none());
    let pill = h.lone_icon("gradient", body + vec2(-120.0, 60.0));
    let grip = h.rect(&Hit::FloatStripTitle(pill)).left_center() + vec2(6.0, 0.0);
    let secondary =
        |pressed| egui::Event::PointerButton { pos: grip, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
    h.frame(vec![egui::Event::PointerMoved(grip), secondary(true)]);
    h.frame(vec![secondary(false)]);
    let out = h.frame_full(vec![]);
    let mut labels = Vec::new();
    fn walk(s: &egui::Shape, labels: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => labels.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, labels)),
            _ => {}
        }
    }
    for c in &out.shapes {
        walk(&c.shape, &mut labels);
    }
    assert!(labels.iter().any(|t| t.contains("Collapse to icon")) && labels.iter().any(|t| t == "Close"), "{labels:?}");
    h.app.ui.varos_shell.collapse_float_strip(pill);
    h.settle();
    assert!(h.state().strip.group("gradient").is_some(), "Collapse to icon docks it back");
    assert!(h.state().float_strips.is_empty());
}

#[test]
fn collapse_to_icon_is_in_the_panel_menu() {
    let mut h = Harness::new();
    h.shell(true);
    h.click(h.rect(&Hit::Menu(3)).center());
    let out = h.frame_full(vec![]);
    let mut at = None;
    for c in &out.shapes {
        walk(&c.shape, &mut at);
    }
    fn walk(s: &egui::Shape, at: &mut Option<Pos2>) {
        match s {
            egui::Shape::Text(t) if t.galley.text().contains("Collapse to icon") => {
                *at = Some(t.pos + t.galley.rect.center().to_vec2());
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, at)),
            _ => {}
        }
    }
    let at = at.expect("Collapse to icon");
    h.click(at);
    assert!(h.tree().find_box(3).is_none() || h.tree().find_box(3).is_some_and(|b| !b.tabs.iter().any(|t| t == "properties")));
    assert!(h.state().strip.group("properties").is_some());
    assert!(h.state().strip.group("color").is_some());
}

#[test]
fn a_narrow_resize_clips_each_box_to_its_own_rect() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.varos_shell.tree = tree::Node::board_and_column(&["color"], &["character"]);
    h.app.ui.varos_shell.strip = shell::strip::Strip::default_for(&h.app.ui.varos_shell.tree);
    h.settle();
    let from = h.rect(&Hit::Divider(vec![1])).center();
    h.hold_drag(from, pos2(from.x, -400.0));
    let color = h.rect(&Hit::Box(2)).shrink(4.0);
    let character = h.rect(&Hit::Box(3)).shrink(4.0);
    let out = h.frame_full(vec![]);
    for shape in &out.shapes {
        let vis = shape.shape.visual_bounding_rect().intersect(shape.clip_rect);
        if !vis.is_positive() || (vis.contains_rect(color) && vis.contains_rect(character)) {
            continue;
        }
        assert!(!(vis.intersects(color) && vis.intersects(character)), "a shape crosses both boxes: {vis:?}");
    }
    h.release(pos2(from.x, -400.0));
}

#[test]
fn the_strip_collapse_control_is_a_tile_at_the_top() {
    let mut h = Harness::new();
    h.shell(true);
    let rail = h.rect(&Hit::Strip);
    let toggle = h.rect(&Hit::StripToggle);
    let menu = h.rect(&Hit::BoardMenu);
    assert!((toggle.width() - 28.0).abs() < 0.5 && (toggle.height() - 28.0).abs() < 0.5, "{toggle:?}");
    assert!(toggle.top() - rail.top() < 2.0, "at the top: {toggle:?}");
    assert!(rail.bottom() - menu.bottom() < 2.0, "the menu is at the bottom: {menu:?}");
    assert!(menu.top() > toggle.bottom());
}

#[test]
fn dragging_the_strip_edge_outward_restores_a_collapsed_column() {
    let mut h = Harness::new();
    h.shell(true);
    let from = h.rect(&Hit::Divider(vec![])).center();
    let to = pos2(h.screen.x - 40.0, from.y);
    h.drag(from, to);
    assert!(!h.state().collapsed_columns.is_empty());
    let edge = h.rect(&Hit::StripEdge);
    let rail = h.rect(&Hit::Strip);
    h.hold_drag(edge.center(), pos2(rail.left() - 130.0, edge.center().y));
    assert!(h.hits().iter().any(|(hit, _)| matches!(hit, Hit::AzureHint)));
    h.release(pos2(rail.left() - 130.0, edge.center().y));
    assert!(h.state().collapsed_columns.is_empty());
    assert!(h.tree().find_tab("properties").is_some());
}

#[test]
fn dragging_a_two_icon_pill_ghosts_both_icons() {
    let mut h = Harness::new();
    h.shell(true);
    h.app.ui.varos_shell.take("stroke");
    h.app.ui.varos_shell.take("gradient");
    h.app.ui.varos_shell.float_strips.push(floats::FloatStrip {
        id: 7,
        pos: [480.0, 220.0],
        groups: vec![shell::strip::Group { panels: vec!["stroke".into(), "gradient".into()], temporary: false, width: 260.0, height: 360.0 }],
    });
    h.settle();
    let pill = h.rect(&Hit::FloatStrip(7));
    assert!((pill.height() - floats::pill_height(2)).abs() < 0.5, "{pill:?}");
    assert!((pill.width() - 40.0).abs() < 0.5);
    let grip = h.rect(&Hit::FloatStripTitle(7)).center();
    h.hold_drag(grip, grip + vec2(-90.0, 30.0));
    let ghost = h.rect(&Hit::Ghost);
    assert!((ghost.height() - floats::pill_height(2)).abs() < 1.0, "the ghost covers both icons: {ghost:?}");
    h.release(grip + vec2(-90.0, 30.0));
}

#[test]
fn dragging_the_tool_strip_edge_snaps_columns() {
    let mut h = Harness::new();
    h.shell(true);
    assert_eq!(h.app.ui.varos_shell.tools_columns, 1);
    let edge = h.rect(&Hit::ToolsEdge).center();
    let tools = h.rect(&Hit::Tools);
    h.hold_drag(edge, edge + vec2(30.0, 0.0));
    let hint = h.hits().into_iter().find(|(hit, _)| matches!(hit, Hit::AzureHint)).map(|(_, r)| r).expect("a snap hint");
    assert!(hint.height() < tools.height() * 0.5, "the snap hint is a short mark, not the full strip: {hint:?}");
    assert!((hint.height() - tools.height() / 3.0).abs() < 2.0, "{hint:?} vs {}", tools.height());
    h.release(edge + vec2(30.0, 0.0));
    assert_eq!(h.app.ui.varos_shell.tools_columns, 2);
    assert!((h.rect(&Hit::Tools).width() - 68.0).abs() < 1.0);
    let mut h = Harness::new();
    h.shell(true);
    let edge = h.rect(&Hit::ToolsEdge).center();
    h.drag(edge, edge + vec2(60.0, 0.0));
    assert_eq!(h.app.ui.varos_shell.tools_columns, 3);
    assert!((h.rect(&Hit::Tools).width() - 96.0).abs() < 1.0);
    let edge = h.rect(&Hit::ToolsEdge).center();
    h.drag(edge, edge + vec2(-70.0, 0.0));
    assert_eq!(h.app.ui.varos_shell.tools_columns, 1);
    assert!((h.rect(&Hit::Tools).width() - 40.0).abs() < 1.0);
}
