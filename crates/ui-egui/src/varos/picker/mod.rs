// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The Color panel's body in Varos: colour picker v3 (owner 2026-10-09; work order
//! `docs/foundation/work_orders/COLOR_PICKER_V3.md` in the old repo). Tabs Wheel · Sliders ·
//! Harmony · Gradient, the frameless Fill/Stroke cluster, the
//! hex and alpha row and the recent colours with their drawer.
//!
//! It edits what upstream's Color panel edits (the active proxy's colour, or a selected gradient
//! stop or freeform point: [`crate::panels::color::target_with`]) through the same commands, so
//! each gesture is one undo step: the first change of a drag begins the panel's live interaction,
//! dragging previews on the art, releasing commits. Nothing is written while nothing changed, and
//! a mixed selection is written only once something changes. A typed value or a clicked chip is
//! one step. The panel follows the selection, never changes it, and asks for no repaint of its own:
//! it redraws on input only. Alpha is the opacity of the targeted fill or stroke.

mod cluster;
mod drawer;
mod fields;
mod harmony;
mod icons;
mod sliders;
mod wheel;

pub use drawer::DrawerTab;
pub use harmony::Rule;
pub use sliders::Mode;

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};

use crate::VectorcraftApp;
use crate::panels::color::Target;
use crate::theme::Tokens;
use crate::widgets::Live;

/// The picker's width (Figma `Picker / Wheel`): the panel is this wide plus its margins.
pub(crate) const W: f32 = 240.0;
pub(crate) const HEADER_H: f32 = 32.0;
pub(crate) const PAD: f32 = 8.0;
pub(crate) const TAB_W: f32 = 26.0;
pub(crate) const TAB_H: f32 = 24.0;
pub(crate) const TAB_GAP: f32 = 4.0;
const GLYPH: f32 = 16.0;
/// Varos control corners.
pub(crate) const CTRL_R: u8 = 3;
/// A change smaller than half an 8-bit step is no change: nothing is written for it.
const SAME: f32 = 0.5 / 255.0;
/// The panel the picker is the body of.
pub(crate) const PANEL: &str = "color";

/// The picker mode, kept with its preferences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    #[default]
    Wheel,
    Sliders,
    Harmony,
    Gradient,
}

/// The picker's preferences, kept with the UI state (`ui.json`, `UiState::varos_picker`). Each
/// field reads leniently: a missing or unknown value is its default, the rest still loads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default, deserialize_with = "lenient")]
    pub tab: Tab,
    /// The Sliders tab's mode, which the Wheel's readout follows.
    #[serde(default, deserialize_with = "lenient")]
    pub mode: Mode,
    #[serde(default, deserialize_with = "lenient")]
    pub harmony: Rule,
    #[serde(default, deserialize_with = "lenient")]
    pub drawer_open: bool,
    #[serde(default, deserialize_with = "lenient")]
    pub drawer_tab: DrawerTab,
    /// Recent colours as `#rrggbb`, newest first (the session's recent colours, kept).
    #[serde(default, deserialize_with = "lenient")]
    pub recent: Vec<String>,
    /// Panel menu › Classic: upstream's sliders and spectrum instead of the picker.
    #[serde(default, deserialize_with = "lenient")]
    pub classic: bool,
}

/// Serde reader that turns a value of the wrong shape into the default instead of an error, so
/// one bad field doesn't lose the rest of the preferences.
pub fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(d: D) -> Result<T, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

/// A press-to-release gesture: one undo step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gesture {
    Ring,
    Triangle,
    Channel(usize),
    Alpha,
}

/// The target a gesture began on and which proxy (Fill or Stroke) was in front.
#[derive(Clone)]
struct Origin {
    target: Target,
    fill: bool,
}

/// What the picker last took from the document.
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    color: Option<Color>,
    mixed: bool,
    fill: bool,
    alpha: Option<f32>,
}

/// The picker's live state (egui memory, not saved): HSV is its source of truth, so a hue
/// survives greys and the triangle doesn't turn while S or V is 0.
#[derive(Clone)]
pub(crate) struct State {
    pub(crate) hsva: [f32; 4],
    seen: Option<Seen>,
    pub(crate) gesture: Option<Gesture>,
    /// The gesture has previewed a change (its release commits it).
    edited: bool,
    /// The colour changed this frame.
    pub(crate) change: bool,
    /// The alpha changed this frame.
    pub(crate) alpha_change: bool,
    /// The colour and alpha last written (or read): a change is measured from them.
    sent: Option<[f32; 3]>,
    alpha_sent: Option<f32>,
    /// The values the sliders show while the colour is the one they made.
    pub(crate) channels: Option<(Mode, [f32; 3], [f32; 4])>,
    /// The picker's colour and alpha when the gesture began: a change is measured from them when
    /// nothing was read (a mixed selection, None).
    start_rgb: [f32; 3],
    start_alpha: f32,
    /// What the gesture began on: it previews and commits there whatever happens meanwhile (X).
    origin: Option<Origin>,
    pub(crate) wheel: wheel::Cache,
    pub(crate) tracks: sliders::Cache,
}

impl Default for State {
    fn default() -> Self {
        Self {
            // Red until a colour is read, as in the old picker (a mixed selection starts from it).
            hsva: [0.0, 1.0, 1.0, 1.0],
            seen: None,
            gesture: None,
            edited: false,
            change: false,
            alpha_change: false,
            sent: None,
            alpha_sent: None,
            channels: None,
            start_rgb: [1.0, 0.0, 0.0],
            start_alpha: 1.0,
            origin: None,
            wheel: Default::default(),
            tracks: Default::default(),
        }
    }
}

impl State {
    pub(crate) fn rgb(&self) -> [f32; 3] {
        hsv_to_rgb(self.hsva[0], self.hsva[1], self.hsva[2])
    }

    /// Start `g` unless a gesture is under way.
    pub(crate) fn start(&mut self, g: Gesture) {
        if self.gesture.is_none() {
            self.gesture = Some(g);
            self.edited = false;
            self.start_rgb = self.rgb();
            self.start_alpha = self.hsva[3];
            self.origin = None;
        }
    }

    /// Take sRGB colour `c`, keeping the hue of a grey.
    pub(crate) fn adopt_rgb(&mut self, c: [f32; 3]) {
        self.channels = None;
        let [h, s, v] = rgb_to_hsv(c);
        if s > 0.001 {
            self.hsva[0] = h;
        }
        self.hsva[1] = s;
        self.hsva[2] = v;
    }

    /// The colour to write (sRGB).
    pub(crate) fn out_color(&self) -> Color {
        let [r, g, b] = self.rgb();
        Color::rgb(r, g, b)
    }

    /// Follow the document: a new selection, target or an edit made elsewhere re-seeds the
    /// picker. A colour the picker shows already (its own last write) leaves it as it is.
    fn follow(&mut self, v: &View) {
        let seen = Seen { color: v.color, mixed: v.mixed, fill: v.fill_active, alpha: v.alpha };
        if self.seen.as_ref() == Some(&seen) {
            return;
        }
        let rgb = v.color.map(|c| c.to_rgb());
        let retarget = self.seen.as_ref().is_none_or(|s| s.fill != seen.fill || s.mixed != seen.mixed);
        if let Some(c) = rgb
            && (retarget || !same(c, self.rgb()))
        {
            self.adopt_rgb(c);
        }
        if retarget || v.alpha.is_none_or(|a| (a - self.hsva[3]).abs() > SAME) {
            self.hsva[3] = v.alpha.unwrap_or(1.0);
        }
        self.sent = rgb;
        self.alpha_sent = v.alpha;
        self.seen = Some(seen);
    }
}

/// What the picker shows, read once per frame.
pub(crate) struct View {
    target: Target,
    /// The colour edited: the active proxy's, or the selected stop's or point's.
    pub(crate) color: Option<Color>,
    /// The active proxy is "?" (the selected objects' paints differ).
    pub(crate) mixed: bool,
    pub(crate) fill_active: bool,
    pub(crate) paints: (Paint, Paint),
    pub(crate) mixed_pair: (bool, bool),
    /// The targeted fill's or stroke's opacity; `None` with nothing selected that has one.
    pub(crate) alpha: Option<f32>,
}

impl View {
    fn read(app: &VectorcraftApp, ctx: &egui::Context) -> Self {
        let mixed_pair = crate::panels::mixed_paints(app, ctx);
        let fill_active = app.session.fill_active;
        let mixed = if fill_active { mixed_pair.0 } else { mixed_pair.1 };
        let target = crate::panels::color::target_with(app, mixed);
        let color = target.color();
        Self { target, color, mixed, fill_active, paints: crate::panels::current_paints(app), mixed_pair, alpha: read_alpha(app, fill_active) }
    }
}

/// What the person did this frame, applied after drawing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Action {
    /// A colour for the target (a chip, a harmony swatch, the hex field): one undo step.
    Set(Color),
    /// Eight hex digits: the colour and the alpha, one undo step.
    SetWithAlpha(Color, f32),
    /// A typed alpha.
    AlphaSet(f32),
    /// A typed channel value: the picker's colour, one undo step.
    Commit,
    /// Bring the Fill (`true`) or Stroke proxy forward (X).
    Focus(bool),
    Swap,
    Default,
    NoneTarget,
    Eyedropper,
    Gradient,
    Solid,
}

fn state_id() -> egui::Id {
    egui::Id::unique("varos-picker-state")
}

fn rect_id() -> egui::Id {
    egui::Id::unique("varos-picker-rect")
}

/// The well-opened popover's layer.
fn popover_id() -> egui::Id {
    egui::Id::unique("varos-picker-popover")
}

/// The gesture state of the picker drawn in `ui`. The popover keeps its own: a docked Color panel
/// drawn earlier in the same frame must not finish (or move) the popover's gesture, or the reverse.
fn instance_id(ui: &Ui) -> egui::Id {
    if ui.layer_id().id == popover_id() { state_id().with("popover") } else { state_id() }
}

/// Every picker instance's state id (see [`instance_id`]).
fn instances() -> [egui::Id; 2] {
    [state_id(), state_id().with("popover")]
}

/// The picker's height for `prefs`.
pub(crate) fn height(prefs: &Prefs) -> f32 {
    if prefs.tab == Tab::Gradient {
        return HEADER_H + 420.0;
    }
    let body = match prefs.tab {
        Tab::Wheel => wheel::BODY_H,
        Tab::Sliders => sliders::body_height(prefs.mode),
        Tab::Harmony => wheel::BODY_H + harmony::extra_height(prefs.harmony),
        Tab::Gradient => 420.0,
    };
    HEADER_H + body + fields::ROW_H + drawer::ROW_H + if prefs.drawer_open { drawer::DRAWER_H } else { 0.0 }
}

/// A stop's nested colour editor keeps the parent gradient mode, and edits only that stop.
pub(crate) fn show_stop(app: &mut VectorcraftApp, ui: &mut Ui) {
    let parent_mode = app.ui.varos_picker.tab;
    let key = egui::Id::unique("varos-picker-stop-mode");
    app.ui.varos_picker.tab = ui.ctx().data(|d| d.get_temp::<Tab>(key)).filter(|t| *t != Tab::Gradient).unwrap_or_default();
    show(app, ui);
    ui.ctx().data_mut(|d| d.insert_temp(key, app.ui.varos_picker.tab));
    app.ui.varos_picker.tab = parent_mode;
}

/// The Color panel's body (the `// Varos` hook at the top of [`crate::panels::color::show`]).
pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let sid = instance_id(ui);
    let mut st = ctx.data_mut(|d| d.remove_temp::<State>(sid)).unwrap_or_default();
    drawer::sync_recent(app);
    let mut prefs = app.ui.varos_picker.clone();
    let v = View::read(app, &ctx);
    if st.gesture.is_none() {
        st.follow(&v);
    }
    let mut acts = Vec::new();
    let tall = height(&prefs);
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width().max(W), tall), Sense::hover());
    let origin = pos2(outer.left() + ((outer.width() - W) / 2.0).max(0.0).round(), outer.top());
    header(
        ui,
        origin,
        &mut prefs,
        app.session.tool_id() == "eyedropper",
        app.ui.dialog.as_ref().is_some_and(|d| d.kind == "gradientStop"),
        &mut acts,
    );
    if prefs.tab == Tab::Gradient {
        if prefs != app.ui.varos_picker {
            app.ui.varos_picker = prefs;
        }
        settle(app, &ctx, &mut st, &v, acts);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(Rect::from_min_size(origin + vec2(PAD, HEADER_H + 4.0), vec2(W - PAD * 2.0, 420.0))),
            |ui| {
                crate::panels::gradient::show_picker(app, ui);
            },
        );
        let frame_nr = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| {
            d.insert_temp(rect_id(), (Rect::from_min_size(origin, vec2(W, tall)), frame_nr));
            d.insert_temp(sid.with("drawn"), frame_nr);
            d.insert_temp(sid, st);
        });
        return;
    }
    let mut y = origin.y + HEADER_H;
    let mut contents = |ui: &mut Ui| match prefs.tab {
        Tab::Wheel => {
            wheel::show(ui, pos2(origin.x, y), &mut st, &v, &prefs, &mut acts);
            y += wheel::BODY_H;
        }
        Tab::Sliders => {
            sliders::show(ui, pos2(origin.x, y), &mut st, &v, &mut prefs, &mut acts);
            y += sliders::body_height(prefs.mode);
        }
        Tab::Harmony => {
            harmony::strip(ui, pos2(origin.x, y), &mut prefs);
            y += harmony::STRIP_H;
            wheel::show(ui, pos2(origin.x, y), &mut st, &v, &prefs, &mut acts);
            y += wheel::BODY_H;
            harmony::chips(ui, pos2(origin.x, y), &st, &prefs, &mut acts);
            y += harmony::extra_height(prefs.harmony) - harmony::STRIP_H;
        }
        Tab::Gradient => {}
    };
    if crate::varos::shell::enabled(app) {
        ui.scope_builder(egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag("simple-picker")), contents);
    } else {
        contents(ui);
    }
    fields::row(ui, pos2(origin.x, y), &mut st, &v, prefs.tab == Tab::Sliders, &mut acts);
    y += fields::ROW_H;
    drawer::show(app, ui, pos2(origin.x, y), &mut prefs, &mut acts);
    // Read before `data_mut`: the context's lock is not re-entrant.
    let frame_nr = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| {
        d.insert_temp(rect_id(), (Rect::from_min_size(origin, vec2(W, tall)), frame_nr));
        d.insert_temp(sid.with("drawn"), frame_nr);
    });
    if prefs != app.ui.varos_picker {
        app.ui.varos_picker = prefs;
    }
    settle(app, &ctx, &mut st, &v, acts);
    ctx.data_mut(|d| d.insert_temp(sid, st));
}

/// The upstream swatch tiles' explicit, non-modal Gradient action.
pub(crate) fn gradient_menu(response: &egui::Response) -> bool {
    let mut open = false;
    response.context_menu(|ui| {
        if ui.button(format!("{}…", tl!("Gradient"))).clicked() {
            open = true;
            ui.close();
        }
    });
    open
}

/// Show/focus the shared Gradient panel without toggling it closed.
pub(crate) fn open_gradient(app: &mut VectorcraftApp) {
    if let Err(error) = app.run("window.panel", json!({"panel":"gradient"})) {
        app.status(error);
    }
}

/// Equal-sized mode icons and the eyedropper, without a title bar or close button.
fn header(ui: &mut Ui, origin: Pos2, prefs: &mut Prefs, eyedropper: bool, stop_editor: bool, acts: &mut Vec<Action>) {
    let t = Tokens::get(ui.ctx());
    for (i, (tab, icon, tip)) in [
        (Tab::Wheel, "palette", tl!("Wheel")),
        (Tab::Sliders, icons::SLIDERS, tl!("Sliders")),
        (Tab::Harmony, "dc-color-guide", tl!("Harmony")),
        (Tab::Gradient, "dc-gradient", tl!("Gradient")),
    ]
    .into_iter()
    .enumerate()
    {
        if stop_editor && tab == Tab::Gradient {
            continue;
        }
        let r = Rect::from_min_size(origin + vec2(PAD + i as f32 * (TAB_W + TAB_GAP), (HEADER_H - TAB_H) / 2.0), vec2(TAB_W, TAB_H));
        let resp = ui.interact(r, ui.scope_id().with(("varos-picker-tab", i)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, tab == prefs.tab, tip));
        tab_button(ui, &t, r, icon, tab == prefs.tab, resp.hovered(), true);
        if resp.on_hover_text(tip).clicked() {
            acts.push(if tab == Tab::Gradient { Action::Gradient } else { Action::Solid });
            prefs.tab = tab;
        }
    }
    let eye = Rect::from_min_size(origin + vec2(W - PAD - TAB_W, (HEADER_H - TAB_H) / 2.0), vec2(TAB_W, TAB_H));
    let resp = ui.interact(eye, ui.scope_id().with(("varos-picker-header", "pipette")), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Eyedropper Tool")));
    tab_button(ui, &t, eye, "pipette", eyedropper, resp.hovered(), true);
    if resp.on_hover_text(tl!("Eyedropper Tool")).clicked() {
        acts.push(Action::Eyedropper);
    }
    ui.painter().hline(origin.x..=origin.x + W, origin.y + HEADER_H, Stroke::new(1.0, t.divider));
}

/// An icon button of the header and the harmony strip: ON is the dark well with a bright glyph
/// (no accent), hover a lighter face.
pub(crate) fn tab_button(ui: &Ui, t: &Tokens, r: Rect, icon: &str, on: bool, hovered: bool, enabled: bool) {
    if on {
        ui.painter().rect_filled(r, CTRL_R, t.input);
    } else if hovered && enabled {
        ui.painter().rect_filled(r, CTRL_R, t.hover);
    }
    let ink = if !enabled {
        t.text_disabled
    } else if on || hovered {
        t.text
    } else {
        t.text_dim
    };
    icons::paint(ui, icon, Rect::from_center_size(r.center(), egui::Vec2::splat(GLYPH)), ink);
}

/// A colour chip (an empty well without a colour).
pub(crate) fn chip(ui: &Ui, r: Rect, c: Option<&Color>, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(r, CTRL_R, c.map_or(t.input, crate::panels::c32));
    ui.painter().rect_stroke(r, CTRL_R, Stroke::new(1.0, if hovered { t.text } else { t.border }), egui::StrokeKind::Inside);
}

/// Draw `add` with the boxes it paints at upstream's 5 pt field corners given the Varos 3 pt
/// control corners instead, leaving upstream's widget code as it is.
pub(crate) fn control_corners<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let layer = ui.layer_id();
    let start = ui.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let r = add(ui);
    ui.ctx().graphics_mut(|g| {
        let list = g.entry(layer);
        for i in start.0..list.next_idx().0 {
            list.mutate_shape(egui::layers::ShapeIdx(i), |c| {
                if let egui::Shape::Rect(rect) = &mut c.shape
                    && rect.corner_radius == egui::CornerRadius::same(5)
                {
                    rect.corner_radius = egui::CornerRadius::same(CTRL_R);
                }
            });
        }
    });
    r
}

/// Apply the frame's actions, then the gesture: a change previews (beginning the live interaction
/// on the first one), the release commits what was previewed as one undo step.
fn settle(app: &mut VectorcraftApp, ctx: &egui::Context, st: &mut State, v: &View, acts: Vec<Action>) {
    for act in acts {
        match act {
            Action::Set(c) => set_color(app, st, v, c, true),
            Action::SetWithAlpha(c, a) => {
                app.session.begin_undo_group();
                set_color(app, st, v, c, true);
                set_alpha(app, st, v, a);
                app.session.end_undo_group(false);
            }
            Action::AlphaSet(a) => set_alpha(app, st, v, a),
            Action::Commit => {
                let c = st.out_color();
                set_color(app, st, v, c, false);
            }
            Action::Focus(fill) => {
                let _ = app.run("paint.toggleActive", json!({ "fill": fill }));
            }
            Action::Swap => {
                let _ = app.run("paint.swap", json!({}));
            }
            Action::Default => {
                let _ = app.run("paint.default", json!({}));
            }
            Action::NoneTarget => {
                let _ = app.run("paint.none", json!({}));
            }
            Action::Eyedropper => {
                close(app);
                app.select_tool("eyedropper");
            }
            Action::Gradient => {
                finish(app, st);
                if !matches!(crate::panels::active_paint(app), Paint::Gradient(_)) {
                    crate::panels::live_run(
                        app,
                        "Gradient",
                        "paint.editGradient",
                        json!({"stroke": !app.session.fill_active, "kind": "linear"}),
                        Live::Released,
                    );
                    if let Err(e) = app.run("gradient.selectStop", json!({"index": 0})) {
                        app.status(e);
                    }
                }
            }
            Action::Solid => {
                finish(app, st);
                if !app.ui.dialog.as_ref().is_some_and(|d| d.kind == "gradientStop") && matches!(crate::panels::active_paint(app), Paint::Gradient(_))
                {
                    let command = if app.session.fill_active { "paint.setFill" } else { "paint.setStroke" };
                    if let Err(e) = app.run(command, json!({"color": crate::panels::color_json(&st.out_color())})) {
                        app.status(e);
                    }
                }
            }
        }
    }
    let down = ctx.input(|i| i.pointer.primary_down());
    if st.gesture.is_some() && st.origin.is_none() {
        // The gesture's first frame: it keeps this target to its end.
        st.origin = Some(Origin { target: v.target.clone(), fill: v.fill_active });
    }
    let Some(origin) = st.origin.clone() else { return };
    let colour_gesture = matches!(st.gesture, Some(Gesture::Ring | Gesture::Triangle | Gesture::Channel(_)));
    if std::mem::take(&mut st.change) && colour_gesture {
        // Measured from what was read, else (mixed, None) from the picker's own colour at the
        // press: pressing the shown marker without moving writes nothing.
        let rgb = st.rgb();
        if !same(st.sent.unwrap_or(st.start_rgb), rgb) {
            st.sent = Some(rgb);
            st.edited = true;
            let c = st.out_color();
            on_origin(app, &origin, |app| crate::panels::color::apply(app, &origin.target, c, Live::Dragging, false));
        }
    }
    if std::mem::take(&mut st.alpha_change) && st.gesture == Some(Gesture::Alpha) {
        let a = st.hsva[3];
        if (st.alpha_sent.unwrap_or(st.start_alpha) - a).abs() > SAME {
            st.alpha_sent = Some(a);
            st.edited = true;
            apply_alpha(app, origin.fill, a, Live::Dragging);
        }
    }
    if st.gesture.is_some() && !down {
        finish(app, st);
    }
}

/// End the gesture: commit what it previewed (one undo step) on the target it began on, and
/// clear it. Called on release, and before the picker is hidden mid-gesture (Escape, the panel
/// put away), so no live interaction is left open.
fn finish(app: &mut VectorcraftApp, st: &mut State) {
    if let (Some(g), true, Some(origin)) = (st.gesture, st.edited, st.origin.clone()) {
        match g {
            Gesture::Alpha => apply_alpha(app, origin.fill, st.hsva[3], Live::Released),
            _ => {
                let c = st.out_color();
                on_origin(app, &origin, |app| crate::panels::color::apply(app, &origin.target, c, Live::Released, false));
            }
        }
    }
    st.gesture = None;
    st.edited = false;
    st.origin = None;
    st.change = false;
    st.alpha_change = false;
}

/// Run `f` with the gesture's own proxy in front (the paint commands write the proxy in front),
/// then put back the one the person brought forward meanwhile.
fn on_origin(app: &mut VectorcraftApp, origin: &Origin, f: impl FnOnce(&mut VectorcraftApp)) {
    let now = app.session.fill_active;
    app.session.fill_active = origin.fill;
    f(app);
    app.session.fill_active = now;
}

/// Finish every stored gesture, if any (the picker is going away).
fn finish_pending(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if app.ui.varos_picker_anchor.is_some() && app.ui.varos_picker.tab == Tab::Gradient {
        crate::panels::gradient::finish_picker_drag(app, ctx);
    }
    for sid in instances() {
        finish_instance(app, ctx, sid);
    }
}

/// Finish the gesture stored for picker instance `sid`, if any.
fn finish_instance(app: &mut VectorcraftApp, ctx: &egui::Context, sid: egui::Id) {
    let Some(mut st) = ctx.data_mut(|d| d.remove_temp::<State>(sid)) else { return };
    if st.gesture.is_some() {
        finish(app, &mut st);
    }
    ctx.data_mut(|d| d.insert_temp(sid, st));
}

/// Write colour `c` to the target as one step, unless it is the colour there already.
fn set_color(app: &mut VectorcraftApp, st: &mut State, v: &View, c: Color, adopt: bool) {
    let rgb = c.to_rgb();
    let unchanged = !v.mixed && v.color.is_some_and(|now| same(now.to_rgb(), rgb));
    if adopt {
        st.adopt_rgb(rgb);
    }
    if unchanged {
        return;
    }
    st.sent = Some(rgb);
    crate::panels::color::apply(app, &v.target, c, Live::Released, false);
}

/// Write alpha `a` as one step, unless it is the alpha there already.
fn set_alpha(app: &mut VectorcraftApp, st: &mut State, v: &View, a: f32) {
    if v.alpha.is_none_or(|x| (x - a).abs() <= SAME) {
        return;
    }
    st.alpha_sent = Some(a);
    apply_alpha(app, v.fill_active, a, Live::Released);
}

/// The selected objects grouped by the appearance item a fill (`fill`) or stroke edit changes:
/// the Appearance panel's active item when it is of that kind, else the topmost one.
fn alpha_groups(app: &VectorcraftApp, fill: bool) -> Vec<(usize, Vec<u64>)> {
    let Some(st) = app.session.active() else { return vec![] };
    let active = app.session.appearance_item();
    let mut groups: Vec<(usize, Vec<u64>)> = Vec::new();
    for id in st.selection.subjects() {
        let Some(n) = st.doc.node(*id) else { continue };
        let ap = &n.appearance;
        let Some(i) = ap.item_of_kind(active, fill).or_else(|| ap.items.iter().rposition(|it| it.is_fill() == fill)) else { continue };
        match groups.iter_mut().find(|g| g.0 == i) {
            Some(g) => g.1.push(id.0),
            None => groups.push((i, vec![id.0])),
        }
    }
    groups
}

/// The targeted fill's or stroke's opacity (0..1) on the first selected object that has one.
pub(crate) fn read_alpha(app: &VectorcraftApp, fill: bool) -> Option<f32> {
    let st = app.session.active()?;
    let (i, ids) = alpha_groups(app, fill).into_iter().next()?;
    let n = st.doc.node(vectorcraft_doc::NodeId(*ids.first()?))?;
    Some(n.appearance.items.get(i)?.opacity())
}

/// Set the targeted fill's or stroke's opacity: live while dragging, one step on release. Objects
/// whose item sits at different places in their stacks are set together on release only.
fn apply_alpha(app: &mut VectorcraftApp, fill: bool, a: f32, phase: Live) {
    let groups = alpha_groups(app, fill);
    let params = |(i, ids): &(usize, Vec<u64>)| json!({ "index": i, "ids": ids, "opacity": a * 100.0 });
    match groups.as_slice() {
        [] => {}
        [one] => crate::panels::live_run(app, "Opacity", "appearance.setItem", params(one), phase),
        [first, rest @ ..] => {
            // Items at different places in the objects' stacks: one interaction previews every
            // update (the others on top of the first preview), and the release commits them as one
            // step.
            if app.session.begin_interaction("Opacity").is_ok() {
                match app.session.preview("appearance.setItem", &params(first)) {
                    Ok(_) => {
                        for p in rest {
                            let _ = app.session.execute("appearance.setItem", &params(p));
                        }
                    }
                    Err(e) => app.ui.status = e.to_string(),
                }
                if phase == Live::Released {
                    let _ = app.session.commit_interaction();
                }
                app.sync_views();
            }
        }
    }
}

/// Open a transient picker at a well, independently of the docked Color panel.
pub(crate) fn open_for(app: &mut VectorcraftApp, stroke: bool, anchor: Rect) {
    if let Err(e) = app.run("paint.toggleActive", json!({"fill": !stroke})) {
        app.status(e);
    }
    app.ui.varos_picker_anchor = Some(anchor);
    app.ui.varos_picker.tab = if matches!(crate::panels::active_paint(app), Paint::Gradient(_)) {
        Tab::Gradient
    } else if app.ui.varos_picker.tab == Tab::Gradient {
        Tab::Wheel
    } else {
        app.ui.varos_picker.tab
    };
}

pub(crate) fn close(app: &mut VectorcraftApp) {
    app.ui.varos_picker_anchor = None;
}

/// Wells remain switchable through the modal backdrop; their geometry is refreshed every frame.
pub(crate) fn register_well(ctx: &egui::Context, rect: Rect, stroke: bool) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| {
        let wells = d.get_temp_mut_or_default::<Vec<(Rect, bool, u64)>>(egui::Id::unique("picker-wells"));
        wells.retain(|(_, _, f)| *f == frame);
        wells.push((rect, stroke, frame));
    });
}

/// How near a sibling well must be to count as the anchor's pair (see [`popover`]).
const WELL_GAP: f32 = 12.0;

/// The tallest the picker gets in any mode with `prefs`' drawer and slider settings: the popover is
/// placed for it, so switching modes keeps the header where it opened.
fn tallest(prefs: &Prefs) -> f32 {
    [Tab::Wheel, Tab::Sliders, Tab::Harmony, Tab::Gradient].into_iter().map(|tab| height(&Prefs { tab, ..prefs.clone() })).fold(0.0, f32::max)
}

/// Render after the shell, before dialogs. The transparent backdrop owns outside clicks. Its
/// release closes the picker after the canvas has been gated for the entire gesture. While an
/// application dialog is open the picker stays visible but takes no input.
pub(crate) fn popover(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let Some(anchor) = app.ui.varos_picker_anchor else { return };
    let mut screen = ctx.content_rect();
    let shell = crate::varos::shell::enabled(app);
    if shell {
        for (hit, r) in crate::varos::shell::hit_rects(ctx) {
            match hit {
                crate::varos::shell::Hit::Tools if app.ui.varos_shell.tools_pos.is_none() => screen.min.x = screen.left().max(r.right() + 6.0),
                crate::varos::shell::Hit::Strip => screen.max.x = screen.right().min(r.left() - 6.0),
                _ => {}
            }
        }
    }
    let h = height(&app.ui.varos_picker).min(screen.height());
    let tall = tallest(&app.ui.varos_picker).min(screen.height());
    let frame_nr = ctx.cumulative_frame_nr();
    // This frame's wells, bottom-most first (the order they are drawn in).
    let wells: Vec<(Rect, bool)> = ctx
        .data(|d| d.get_temp::<Vec<(Rect, bool, u64)>>(egui::Id::unique("picker-wells")))
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, _, f)| *f == frame_nr)
        .map(|(r, stroke, _)| (r, stroke))
        .collect();
    // The well and its Fill/Stroke sibling (overlapping proxy squares, stacked or side-by-side
    // chips) stay uncovered, so the other well remains one click away.
    let pair = wells.iter().filter(|(r, _)| r.intersects(anchor.expand(WELL_GAP))).fold(anchor, |pair, (r, _)| pair.union(*r));
    let (x, y) = if pair.bottom() + 4.0 + tall <= screen.bottom() {
        // Below the wells.
        (pair.left(), pair.bottom() + 4.0)
    } else {
        // Beside them (right, else left) when the window has no room below.
        let x = if pair.right() + 4.0 + W <= screen.right() { pair.right() + 4.0 } else { pair.left() - 4.0 - W };
        (x, pair.top().min(screen.bottom() - tall))
    };
    let x = x.clamp(screen.left(), (screen.right() - W).max(screen.left()));
    let y = y.min(screen.bottom() - h).max(screen.top());
    let id = popover_id();
    let area = egui::Area::new(id).kind(egui::UiKind::Modal).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).movable(false).fade_in(false);
    let t = Tokens::get(ctx);
    let frame =
        if shell { egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)).corner_radius(10) } else { egui::Frame::NONE.fill(t.panel) };
    let body = |app: &mut VectorcraftApp, ui: &mut Ui| {
        ui.set_width(W);
        egui::ScrollArea::vertical().max_height(screen.height()).show(ui, |ui| show(app, ui));
    };
    if app.ui.dialog.is_some() {
        // Under an application dialog (its own backdrop takes the clicks): shown, inert.
        area.interactable(false).show(ctx, |ui| {
            ui.disable();
            frame.show(ui, |ui| body(app, ui));
        });
        return;
    }
    let modal = egui::Modal::new(id).area(area).backdrop_color(Color32::TRANSPARENT).frame(frame).show(ctx, |ui| body(app, ui));
    let click = ctx.input(|i| i.pointer.any_click().then(|| i.pointer.interact_pos()).flatten());
    if !modal.any_popup_open
        && !egui::Popup::is_any_open(ctx)
        && let Some(p) = click.filter(|p| !modal.response.rect.contains(*p))
    {
        // The topmost well under the click (the one drawn last) is the one seen there.
        let well = wells.iter().rev().find(|(r, _)| r.contains(p)).copied();
        let current = (anchor, !app.session.fill_active);
        if well == Some(current) {
            return;
        }
        finish_pending(app, ctx);
        if let Some((r, stroke)) = well {
            open_for(app, stroke, r);
        } else {
            close(app);
        }
    }
}

/// Called every frame before the shortcuts and the tools. A gesture whose picker was not drawn
/// last frame (put away mid-drag) is finished, so no interaction stays open. While the popover is
/// open its Escape is the picker's (true, and the caller routes it no further): it closes the
/// popover (finishing any gesture, never undoing), or is left to a focused field or open dropdown.
pub(crate) fn escape(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_frame_nr();
    for sid in instances() {
        if ctx.data(|d| d.get_temp::<u64>(sid.with("drawn"))).is_none_or(|frame| frame + 1 < now) {
            finish_instance(app, ctx, sid);
        }
    }
    if app.ui.varos_picker_anchor.is_none() || !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return false;
    }
    // A focused field or an open dropdown in the picker takes this Escape itself (left for it);
    // the tools and the rest of the app never see it.
    if ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(ctx) {
        return true;
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        finish_pending(app, ctx);
        close(app);
    }
    true
}

/// Are two sRGB colours the same to the eye (within half an 8-bit step)?
pub(crate) fn same(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() <= SAME)
}

/// HSV of an sRGB colour, all 0..1.
pub(crate) fn rgb_to_hsv([r, g, b]: [f32; 3]) -> [f32; 3] {
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let s = if mx <= 0.0 { 0.0 } else { d / mx };
    let h = if d <= 1e-6 {
        0.0
    } else {
        let h = if mx == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if mx == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h / 6.0
    };
    [h, s, mx]
}

/// sRGB of an HSV colour, all 0..1 (the hue wraps).
pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h6 = h.rem_euclid(1.0) * 6.0;
    let i = h6.floor();
    let f = h6 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

pub(crate) fn hsv_c32(h: f32, s: f32, v: f32) -> Color32 {
    let c = hsv_to_rgb(h, s, v).map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgb(c[0], c[1], c[2])
}

#[cfg(test)]
mod tests;
