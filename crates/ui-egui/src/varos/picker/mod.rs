// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The Color panel's body in Varos: colour picker v3 (owner 2026-10-09; work order
//! `docs/foundation/work_orders/COLOR_PICKER_V3.md` in the old repo). Tabs Wheel · Sliders ·
//! Harmony · Gradient (disabled until gradient paints land), the frameless Fill/Stroke cluster, the
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

/// The picker's tab, kept with its preferences. Gradient is not a choice yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    #[default]
    Wheel,
    Sliders,
    Harmony,
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
    Close,
}

fn state_id() -> egui::Id {
    egui::Id::new("varos-picker-state")
}

fn rect_id() -> egui::Id {
    egui::Id::new("varos-picker-rect")
}

/// The picker's height for `prefs`.
pub(crate) fn height(prefs: &Prefs) -> f32 {
    let body = match prefs.tab {
        Tab::Wheel => wheel::BODY_H,
        Tab::Sliders => sliders::body_height(prefs.mode),
        Tab::Harmony => wheel::BODY_H + harmony::extra_height(prefs.harmony),
    };
    HEADER_H + body + fields::ROW_H + drawer::ROW_H + if prefs.drawer_open { drawer::DRAWER_H } else { 0.0 }
}

/// The Color panel's body (the `// Varos` hook at the top of [`crate::panels::color::show`]).
pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let mut st = ctx.data_mut(|d| d.remove_temp::<State>(state_id())).unwrap_or_default();
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
    header(ui, origin, &mut prefs, app.session.tool_id() == "eyedropper", &mut acts);
    let mut y = origin.y + HEADER_H;
    match prefs.tab {
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
    }
    fields::row(ui, pos2(origin.x, y), &mut st, &v, prefs.tab == Tab::Sliders, &mut acts);
    y += fields::ROW_H;
    drawer::show(app, ui, pos2(origin.x, y), &mut prefs, &mut acts);
    // Read before `data_mut`: the context's lock is not re-entrant.
    let frame_nr = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(rect_id(), (Rect::from_min_size(origin, vec2(W, tall)), frame_nr)));
    if prefs != app.ui.varos_picker {
        app.ui.varos_picker = prefs;
    }
    settle(app, &ctx, &mut st, &v, acts);
    ctx.data_mut(|d| d.insert_temp(state_id(), st));
}

/// The tab strip, the eyedropper and ×.
fn header(ui: &mut Ui, origin: Pos2, prefs: &mut Prefs, eyedropper: bool, acts: &mut Vec<Action>) {
    let t = Tokens::get(ui.ctx());
    let tabs = [
        (Some(Tab::Wheel), "palette", tl!("Wheel")),
        (Some(Tab::Sliders), icons::SLIDERS, tl!("Sliders")),
        (Some(Tab::Harmony), "dc-color-guide", tl!("Harmony")),
        (None, "dc-gradient", tl!("Gradient (coming soon)")),
    ];
    for (i, (tab, icon, tip)) in tabs.into_iter().enumerate() {
        let r = Rect::from_min_size(origin + vec2(PAD + i as f32 * (TAB_W + TAB_GAP), (HEADER_H - TAB_H) / 2.0), vec2(TAB_W, TAB_H));
        let resp = ui.interact(r, ui.id().with(("varos-picker-tab", i)), if tab.is_some() { Sense::click() } else { Sense::hover() });
        tab_button(ui, &t, r, icon, tab == Some(prefs.tab), resp.hovered(), tab.is_some());
        if resp.on_hover_text(tip).clicked()
            && let Some(tab) = tab
        {
            prefs.tab = tab;
        }
    }
    let close = Rect::from_min_size(origin + vec2(W - PAD - TAB_W, (HEADER_H - TAB_H) / 2.0), vec2(TAB_W, TAB_H));
    let eye = close.translate(vec2(-(TAB_W + TAB_GAP), 0.0));
    for (r, icon, tip, on, action) in
        [(eye, "pipette", tl!("Eyedropper Tool"), eyedropper, Action::Eyedropper), (close, "x", tl!("Close"), false, Action::Close)]
    {
        let resp = ui.interact(r, ui.id().with(("varos-picker-header", icon)), Sense::click());
        tab_button(ui, &t, r, icon, on, resp.hovered(), true);
        if resp.on_hover_text(tip).clicked() {
            acts.push(action);
        }
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
            Action::Eyedropper => app.select_tool("eyedropper"),
            Action::Close => close(app),
        }
    }
    let down = ctx.input(|i| i.pointer.primary_down());
    let colour_gesture = matches!(st.gesture, Some(Gesture::Ring | Gesture::Triangle | Gesture::Channel(_)));
    if std::mem::take(&mut st.change) && colour_gesture {
        let rgb = st.rgb();
        if v.mixed || st.sent.is_none_or(|s| !same(s, rgb)) {
            st.sent = Some(rgb);
            st.edited = true;
            crate::panels::color::apply(app, &v.target, st.out_color(), Live::Dragging, false);
        }
    }
    if std::mem::take(&mut st.alpha_change) && st.gesture == Some(Gesture::Alpha) {
        let a = st.hsva[3];
        if st.alpha_sent.is_none_or(|s| (s - a).abs() > SAME) {
            st.alpha_sent = Some(a);
            st.edited = true;
            apply_alpha(app, v.fill_active, a, Live::Dragging);
        }
    }
    if let Some(g) = st.gesture
        && !down
    {
        if st.edited {
            match g {
                Gesture::Alpha => apply_alpha(app, v.fill_active, st.hsva[3], Live::Released),
                _ => crate::panels::color::apply(app, &v.target, st.out_color(), Live::Released, false),
            }
        }
        st.gesture = None;
        st.edited = false;
    }
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
        many if phase == Live::Released => {
            let calls: Vec<Value> = many.iter().map(params).collect();
            app.session.begin_undo_group();
            for p in calls {
                let _ = app.run("appearance.setItem", p);
            }
            app.session.end_undo_group(false);
        }
        _ => {}
    }
}

/// Show the Color panel for the Fill (`stroke` false) or Stroke: a double-click on a proxy square.
/// It focuses that proxy and opens the panel when it isn't showing (never the modal Color Picker).
pub(crate) fn open_for(app: &mut VectorcraftApp, stroke: bool) {
    if app.session.fill_active == stroke {
        let _ = app.run("paint.toggleActive", json!({ "fill": !stroke }));
    }
    if crate::menus::checked(app, "window.panel", &json!({ "panel": PANEL })) != Some(true) {
        let _ = app.run("window.panel", json!({ "panel": PANEL }));
    }
}

/// Put the Color panel away (× and Escape). It never undoes anything.
pub(crate) fn close(app: &mut VectorcraftApp) {
    if crate::menus::checked(app, "window.panel", &json!({ "panel": PANEL })) == Some(true) {
        let _ = app.run("window.panel", json!({ "panel": PANEL, "toggle": true }));
        // The panel stack follows at once (else on its next frame).
        crate::varos::panel_stack::reconcile(app);
    }
}

/// Escape with the pointer over the picker (drawn last frame) and no field or popup taking it
/// closes the Color panel; true when it did. Called before the shortcuts and the tools see Escape.
pub(crate) fn escape(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    let Some((rect, frame)) = ctx.data(|d| d.get_temp::<(Rect, u64)>(rect_id())) else { return false };
    if frame + 1 < ctx.cumulative_frame_nr() || ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(ctx) {
        return false;
    }
    let over = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| rect.contains(p));
    if !over || !ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        return false;
    }
    close(app);
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
