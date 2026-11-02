//! The one headless harness for the panel workspace's frame tests: the real app frame on a fixed
//! screen, the drawn hit rectangles, pointer gestures and layout inspection. Each test file keeps
//! only its own fixtures and assertions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub(super) use super::{State, geometry, model, registry};
pub(super) use crate::VectorcraftApp;
pub(super) use egui::{Event, FullOutput, Key, Modifiers, Pos2, Rect, Shape, pos2, vec2};
pub(super) use model::{Action, Edge, Layout, Mode, Source, Target};
pub(super) use serde_json::json;
pub(super) use vectorcraft_engine::Session;

/// The fixed screen the frame tests draw on, unless a test sizes its own.
pub(super) const SCREEN: egui::Vec2 = egui::vec2(1600.0, 1000.0);

pub(super) struct Harness {
    pub app: VectorcraftApp,
    pub ctx: egui::Context,
    pub time: f64,
    pub focused: bool,
    pub screen: egui::Vec2,
    /// The region the docked bars frame (from Control's top-left to Tools' bottom), when both show.
    pub workspace: Rect,
    /// Every panel owner when the harness started; [`Harness::assert_owners`] keeps them all.
    pub owners: Vec<String>,
}

pub(super) fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

pub(super) fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

/// Every text shape an output painted: its text, visual rectangle and clip rectangle.
pub(super) fn texts(output: &FullOutput) -> Vec<(String, Rect, Rect)> {
    fn collect(shape: &Shape, clip: Rect, out: &mut Vec<(String, Rect, Rect)>) {
        match shape {
            Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect(), clip)),
            Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in &output.shapes {
        collect(&shape.shape, shape.clip_rect, &mut out);
    }
    out
}

pub(super) fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1.1, "expected {expected}, got {actual}");
}

/// Every panel the layout owns, sorted.
pub(super) fn owners(layout: &Layout) -> Vec<String> {
    let mut panels: Vec<String> = layout.hosts.iter().flat_map(|h| model::groups(&h.root)).flat_map(|g| g.panels.iter().cloned()).collect();
    panels.sort_unstable();
    panels
}

impl Harness {
    /// The Design workspace (`ui.panelReview`) presenting a 300 x 300 document.
    pub fn workspace() -> Self {
        Self::start(SCREEN, None)
    }

    /// [`Harness::workspace`] on a screen of `screen`.
    pub fn sized(screen: egui::Vec2) -> Self {
        Self::start(screen, None)
    }

    /// Only the two floating blocks, Layers and Swatches (`Layout::new_slice`).
    pub fn slice() -> Self {
        Self::start(SCREEN, Some(Layout::new_slice()))
    }

    /// The workspace on, `state` presented, and no frame run yet.
    pub fn unsettled(state: State) -> Self {
        let mut h = Self::app();
        h.app.ui.varos_panel_review = Some(state);
        h.owners = owners(&h.state().layout);
        h
    }

    fn app() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("ui.panelReview", json!({"enabled": true})).unwrap();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        assert!(super::enabled(&app), "the presenter is on for these checks");
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        Self { app, ctx, time: 0.0, focused: true, screen: SCREEN, workspace: Rect::NOTHING, owners: Vec::new() }
    }

    fn start(screen: egui::Vec2, layout: Option<Layout>) -> Self {
        let mut h = Self::app();
        h.screen = screen;
        if let Some(layout) = layout {
            h.app.ui.varos_panel_review = Some(State { layout, ..Default::default() });
        }
        h.settle();
        h.owners = owners(h.layout());
        h.measure_workspace();
        h
    }

    /// Recompute [`Harness::workspace`] from the drawn Control and Tools hosts.
    pub fn measure_workspace(&mut self) {
        let host =
            |panel: &str| self.layout().owner(panel).map(|h| format!("host:{}", h.id)).filter(|hit| self.has_hit(hit)).map(|hit| self.hit(&hit));
        if let (Some(control), Some(tools)) = (host("control"), host("tools")) {
            self.workspace = Rect::from_min_max(control.min, pos2(control.right(), tools.bottom()));
        }
    }

    pub fn frame(&mut self, events: Vec<Event>) -> FullOutput {
        self.time += 0.05;
        let input = egui::RawInput {
            time: Some(self.time),
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            focused: self.focused,
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let app = &mut self.app;
        app.begin_host_iteration();
        let mut out = self.ctx.run_ui(input, |ui| {
            app.tick(ui.ctx());
            app.ui(ui);
        });
        // No renderer consumes texture uploads here.
        out.textures_delta.clear();
        assert!(!self.app.ui.status.starts_with("Internal error:"), "{}", self.app.ui.status);
        out
    }

    /// Let deferred work (fitting, follow-up requests) settle; the last frame's output.
    pub fn settle(&mut self) -> FullOutput {
        for _ in 0..4 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }

    pub fn state(&self) -> &State {
        self.app.ui.varos_panel_review.as_ref().unwrap()
    }

    pub fn layout(&self) -> &Layout {
        &self.state().layout
    }

    /// The tab widths the last frame measured from the drawn labels.
    pub fn tabs(&self) -> &geometry::TabWidths {
        &self.state().tab_widths
    }

    pub fn hit(&self, name: &str) -> Rect {
        self.state().hits.iter().find_map(|(key, rect)| (key == name).then_some(*rect)).unwrap_or_else(|| panic!("missing drawn hit {name}"))
    }

    pub fn has_hit(&self, name: &str) -> bool {
        self.state().hits.iter().any(|(key, _)| key == name)
    }

    pub fn host_id(&self, panel: &str) -> u64 {
        self.layout().owner(panel).unwrap().id
    }

    pub fn host_rect(&self, panel: &str) -> Rect {
        self.hit(&format!("host:{}", self.host_id(panel)))
    }

    pub fn group_id(&self, panel: &str) -> u64 {
        self.layout().group_of(panel).unwrap().id
    }

    pub fn group_rect(&self, panel: &str) -> Rect {
        let group = self.group_id(panel);
        geometry::measure(self.layout(), self.workspace, self.tabs()).into_iter().flat_map(|host| host.groups).find(|g| g.id == group).unwrap().rect
    }

    pub fn tab(&self, panel: &str) -> Rect {
        self.hit(&format!("tab:{panel}"))
    }

    pub fn mode(&self, panel: &str) -> Mode {
        self.layout().column_mode(self.group_id(panel)).unwrap()
    }

    pub fn key(&self, panel: &str) -> u64 {
        self.layout().column_key(self.group_id(panel)).unwrap()
    }

    pub fn visible(&self, panel: &str) -> bool {
        self.layout().owner(panel).is_some_and(|h| !h.hidden)
    }

    pub fn canvas(&self) -> Rect {
        self.app.canvas_rect.expect("the real canvas is drawn")
    }

    pub fn action(&mut self, action: Action) {
        self.app.run("ui.panelReview.action", json!({"action": action})).unwrap();
        self.settle();
    }

    pub fn select_rectangle(&mut self) {
        let id = self.app.run("shape.rectangle", json!({"x": 20, "y": 20, "width": 60, "height": 40})).unwrap()["id"].clone();
        self.app.run("select.set", json!({"ids": [id]})).unwrap();
    }

    pub fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.settle();
    }

    /// Press at `from` and move to `to`; the drawn handle must have begun a gesture.
    pub fn hold_drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for step in 1..=8 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (step as f32 / 8.0))]);
        }
        assert!(
            self.state().gesture.is_some(),
            "the drawn handle begins a gesture: from={from:?}, to={to:?}, hits={:?}",
            self.state()
                .hits
                .iter()
                .filter(|(name, _)| name.starts_with("host:") || name.starts_with("host-grip:") || name.starts_with("resize-"))
                .collect::<Vec<_>>()
        );
    }

    /// Release at `at`; pointer-up finishes the gesture.
    pub fn release(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at), button(at, false)]);
        self.settle();
        assert!(self.state().gesture.is_none(), "pointer-up finishes the gesture");
    }

    pub fn drag(&mut self, from: Pos2, to: Pos2) {
        self.hold_drag(from, to);
        self.release(to);
    }

    /// Click the collapse/expand arrow of `panel`'s column.
    pub fn toggle(&mut self, panel: &str) {
        self.click(self.hit(&format!("column-mode:{}", self.key(panel))).center());
    }

    /// The layout is valid and still owns exactly the panels the harness started with, hidden ones
    /// included.
    pub fn assert_owners(&self) {
        self.layout().validate().unwrap();
        assert_eq!(owners(self.layout()), self.owners, "every panel retains exactly one owner");
    }
}
