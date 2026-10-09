//! Colour picker v3: a Board hand, with one transaction per gesture.
use super::*;
use varos_app::shell::tokens as t;
use varos_app::storage::layout::PickerLayout;
mod cluster;
mod drawer;
mod fields;
mod modes;
mod sliders;
mod wheel;
pub(crate) use cluster::*;
pub(crate) use wheel::WheelCache;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MTarget {
    Paint(PaintTarget),
    Ab(u32),
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tab {
    Wheel,
    Sliders,
    Harmony,
}
#[derive(Clone, Copy, PartialEq)]
enum Gesture {
    Ring,
    Triangle,
    Alpha,
    Sample,
    Slider,
}

pub(crate) struct ColorPanel {
    pub(crate) target: MTarget,
    pub(crate) hsva: Rgba,
    pub(crate) mixed: bool,
    pub(crate) last_sent: Option<Rgba>,
    pub(crate) change_requested: bool,
    pub(crate) eyedropping: bool,
    pub(crate) sampling: Option<CanvasSampling>,
    arm_snapshot: Option<std::sync::Arc<varos_core::model::Document>>,
    pub(super) disarmed_press: bool,
    selection: std::collections::HashSet<u32>,
    tab: Tab,
    gesture: Option<Gesture>,
    edited: bool,
    cache: WheelCache,
    slider_cache: sliders::TrackCache,
    mode: varos_app::storage::layout::PickerMode,
    channel_state: Option<(modes::Mode, Rgba, [f32; 4])>,
}
impl ColorPanel {
    pub(crate) fn new(target: MTarget, seed: Option<Rgba>, mixed: bool) -> Self {
        let c = seed.unwrap_or([1.0, 0.0, 0.0, 1.0]);
        let h = rgb_to_hsv(c);
        Self {
            target,
            hsva: [h[0], h[1], h[2], c[3]],
            mixed,
            last_sent: seed,
            change_requested: false,
            eyedropping: false,
            sampling: None,
            arm_snapshot: None,
            disarmed_press: false,
            selection: Default::default(),
            tab: Tab::Wheel,
            gesture: None,
            edited: false,
            cache: WheelCache::default(),
            slider_cache: Default::default(),
            mode: Default::default(),
            channel_state: None,
        }
    }
    pub(crate) fn reseed(&mut self, target: MTarget, seed: Option<Rgba>, mixed: bool) {
        let fresh = Self::new(target, seed, mixed);
        let hue = self.hsva[0];
        let same_target = self.target == target;
        self.target = target;
        self.hsva = fresh.hsva;
        if same_target && (seed.is_none() || self.hsva[1] < 0.001) {
            self.hsva[0] = hue;
        }
        self.mixed = mixed;
        self.last_sent = seed;
        self.change_requested = false;
        self.gesture = None;
        self.edited = false;
        self.eyedropping = false;
        self.sampling = None;
        self.arm_snapshot = None;
    }
    pub(crate) fn sample_active(&self) -> bool {
        self.eyedropping || self.gesture == Some(Gesture::Sample)
    }
    pub(crate) fn gesture_active(&self) -> bool {
        self.gesture.is_some()
    }
    pub(crate) fn arm(&mut self) {
        self.sampling = None;
        self.arm_snapshot = None;
        self.eyedropping = true;
    }
    fn color(&self) -> Rgba {
        let c = hsv_to_rgb(self.hsva[0], self.hsva[1], self.hsva[2]);
        [c[0], c[1], c[2], self.hsva[3]]
    }
    fn adopt(&mut self, c: Rgba) {
        self.channel_state = None;
        let h = rgb_to_hsv(c);
        if h[1] > 0.001 {
            self.hsva[0] = h[0];
        }
        self.hsva[1] = h[1];
        self.hsva[2] = h[2];
        self.hsva[3] = c[3];
        self.change_requested = true;
    }
    fn channel_values(&self) -> [f32; 4] {
        self.channel_state.filter(|(mode, c, _)| *mode == self.mode && same_color(*c, self.color())).map_or_else(
            || {
                let mut v = modes::values(self.mode, self.color());
                if self.mode == modes::Mode::Hsb {
                    v[1] = self.hsva[1] * 100.0;
                    v[2] = self.hsva[2] * 100.0;
                }
                if matches!(self.mode, modes::Mode::Hsb | modes::Mode::Hsl) {
                    v[0] = self.hsva[0] * 360.0;
                }
                v
            },
            |(_, _, v)| v,
        )
    }
    fn channel_color(&self, index: usize, value: f32) -> Rgba {
        let mut v = self.channel_values();
        v[index] = if self.mode == modes::Mode::Web { (value / 51.0).round() * 51.0 } else { value };
        modes::rgb(self.mode, v, self.hsva[3])
    }
    fn adopt_channel(&mut self, index: usize, value: f32) {
        let mut v = self.channel_values();
        v[index] = if self.mode == modes::Mode::Web { (value / 51.0).round() * 51.0 } else { value };
        let c = modes::rgb(self.mode, v, self.hsva[3]);
        let changed = self.mixed || !same_color(c, self.color());
        self.adopt(c);
        self.change_requested = changed;
        if matches!(self.mode, modes::Mode::Hsb | modes::Mode::Hsl) {
            self.hsva[0] = v[0] / 360.0;
        }
        if self.mode == modes::Mode::Hsb {
            self.hsva[1] = v[1] / 100.0;
            self.hsva[2] = v[2] / 100.0;
        }
        self.channel_state = Some((self.mode, self.color(), v));
    }
    fn live_color(&mut self) -> Option<Rgba> {
        if !std::mem::take(&mut self.change_requested) {
            return None;
        }
        let c = self.color();
        if !self.mixed && self.last_sent.is_some_and(|o| same_color(o, c)) {
            return None;
        }
        self.mixed = false;
        self.last_sent = Some(c);
        self.edited = true;
        Some(c)
    }
    fn start(&mut self, gesture: Gesture, ops: &mut Vec<Op>) {
        if self.gesture.is_none() {
            self.gesture = Some(gesture);
            self.edited = false;
            ops.push(Op::PickerBegin);
        }
    }
    fn accept_sample(&mut self, ops: &mut Vec<Op>) {
        self.eyedropping = false;
        if self.gesture == Some(Gesture::Sample) {
            self.gesture = Some(Gesture::Slider);
            self.finish(ops);
        }
    }
    pub(crate) fn finish(&mut self, ops: &mut Vec<Op>) {
        self.eyedropping = false;
        self.sampling = None;
        self.arm_snapshot = None;
        if self.gesture == Some(Gesture::Sample) {
            self.gesture = None;
            self.change_requested = false;
            self.edited = false;
            self.last_sent = None;
            ops.push(Op::PickerCancel);
            return;
        }
        if self.gesture.take().is_some() {
            if let Some(c) = self.live_color() {
                ops.push(Op::PickerLive(self.target, c));
            }
            if self.edited {
                ops.push(Op::PickerCommit(self.target, self.color()));
            } else {
                ops.push(Op::PickerFinish);
            }
            self.edited = false;
        }
    }
}
fn same_color(a: Rgba, b: Rgba) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6)
}
fn seed(target: MTarget, snap: &Snap, ed: &Editor) -> (Option<Rgba>, bool) {
    match target {
        MTarget::Paint(t) => (snap_target_color(snap, t), snap.target_mixed(t)),
        MTarget::Ab(id) => (ed.doc.artboards.iter().find(|a| a.id == id).and_then(|a| a.page_color), false),
    }
}
pub(crate) fn open_picker(panel: &mut Option<ColorPanel>, target: MTarget, ed: &mut Editor) {
    if let Some(m) = panel {
        let mut ops = vec![];
        m.finish(&mut ops);
        apply_ops(ed, ops);
    }
    if let MTarget::Paint(t) = target {
        ed.set_paint_target(t);
    }
    let (c, mixed) = seed(target, &Snap::read(ed), ed);
    if let Some(m) = panel {
        m.reseed(target, c, mixed);
    } else {
        *panel = Some(ColorPanel::new(target, c, mixed));
    }
    panel.as_mut().unwrap().selection = ed.selected_pids();
}
/// Selection and external paint edits re-seed; our live drag retains its HSV (including grey hue).
pub(crate) fn follow_selection(m: &mut ColorPanel, ed: &mut Editor) {
    let snap = Snap::read(ed);
    let target = match m.target {
        MTarget::Paint(_) => MTarget::Paint(snap.paint),
        MTarget::Ab(id) if ed.doc.artboards.iter().any(|a| a.id == id) => m.target,
        _ => MTarget::Paint(snap.paint),
    };
    let (c, mixed) = seed(target, &snap, ed);
    let selection = ed.selected_pids();
    let changed = target != m.target
        || selection != m.selection
        || (m.gesture.is_none()
            && (mixed != m.mixed
                || match (c, m.last_sent) {
                    (Some(a), Some(b)) => !same_color(a, b),
                    (None, None) => false,
                    _ => true,
                }));
    if changed {
        let mut ops = vec![];
        let missing = matches!(m.target, MTarget::Ab(id) if ed.doc.artboard_index(id).is_none());
        // Cancelling the paint snapshot must preserve the foreign artboard deletion/reorder.
        let pages = missing.then(|| (ed.doc.artboards.clone(), ed.doc.active, ed.doc.ids));
        if missing {
            m.gesture = None;
            m.eyedropping = false;
            ops.push(Op::PickerCancel);
        } else {
            m.finish(&mut ops);
        }
        apply_ops(ed, ops);
        if let Some((pages, active, ids)) = pages {
            ed.doc.artboards = pages;
            ed.doc.active = active;
            ed.doc.ids = ed.doc.ids.max(ids);
        }
        let (c, mixed) = seed(target, &Snap::read(ed), ed);
        m.reseed(target, c, mixed);
        m.selection = selection;
    }
}
pub(crate) struct CanvasSampling {
    key: ([f32; 2], f32, f32, egui::Rect),
    pub(crate) raster: varos_raster::Raster,
    #[cfg(test)]
    pub(crate) builds: usize,
}

/// Build on arming and view changes only. The live preview never replaces this snapshot.
pub(crate) fn prepare_canvas_sample(m: &mut ColorPanel, ed: &Editor, view: View, ppp: f32, hole: egui::Rect) {
    if !m.eyedropping {
        m.arm_snapshot = None;
        m.sampling = None;
        return;
    }
    if m.arm_snapshot.is_none() {
        m.arm_snapshot = Some(std::sync::Arc::new(ed.doc.clone()));
    }
    let snapshot = m.arm_snapshot.as_ref().unwrap().clone();
    if crate::cursors::SCREEN_EYEDROPPER {
        m.sampling = None;
        return;
    }
    let key = (view.pan, view.zoom, ppp, hole);
    if m.sampling.as_ref().is_some_and(|s| s.key == key) {
        return;
    }
    #[cfg(test)]
    let builds = m.sampling.as_ref().map_or(1, |s| s.builds + 1);
    m.sampling = None;
    let min = [hole.min.x * ppp, hole.min.y * ppp];
    let raster = varos_raster::rasterize_canvas(
        &snapshot,
        [(hole.width() * ppp).ceil() as u32, (hole.height() * ppp).ceil() as u32],
        [view.pan[0] - min[0], view.pan[1] - min[1]],
        view.zoom,
    );
    m.sampling = Some(CanvasSampling {
        key,
        raster,
        #[cfg(test)]
        builds,
    });
}

pub(crate) fn picker_canvas_sample(ctx: &egui::Context, m: &ColorPanel) -> Option<Rgba> {
    let sampling = m.sampling.as_ref()?;
    let (_, _, ppp, hole) = sampling.key;
    let pos = ctx.input(|i| i.pointer.hover_pos())?;
    if !hole.contains(pos) || ctx.layer_id_at(pos).is_some_and(|l| l.order != egui::Order::Background) {
        return None;
    }
    sampling.raster.sample([(pos.x - hole.min.x) * ppp, (pos.y - hole.min.y) * ppp])
}

/// The Board's second hand. Only the header's spare space drags; body gestures edit colour.
pub(crate) fn build_color_panel(
    ctx: &egui::Context,
    panel: &mut Option<ColorPanel>,
    s: &Snap,
    ops: &mut Vec<Op>,
    sample: Option<Rgba>,
    board: egui::Rect,
    layout: &mut PickerLayout,
) {
    let Some(m) = panel else {
        return;
    };
    let field_open = kit::field::any_open(ctx);
    let menu_open = kit::menu_open(ctx);
    m.mode = layout.mode;
    let mut close = false;
    let height = if m.tab == Tab::Sliders {
        t::PICKER_HEADER_H + sliders::body_height(m.mode) + t::PICKER_FIELD_ROW_H + t::PICKER_SWATCH_ROW_H
    } else {
        t::PICKER_H
    };
    let height = height + if layout.drawer_open { t::PICKER_DRAWER_H } else { 0.0 };
    let pos = layout
        .position
        .map(|p| board.min + egui::vec2(p[0], p[1]))
        .unwrap_or(board.min + egui::vec2(t::PICKER_DEFAULT_OFFSET[0], t::PICKER_DEFAULT_OFFSET[1]));
    let pos = egui::pos2(
        pos.x.clamp(board.left(), (board.right() - t::PICKER_W).max(board.left())),
        pos.y.clamp(board.top(), (board.bottom() - height).max(board.top())),
    );
    let panel_rect = egui::Rect::from_min_size(pos, egui::vec2(t::PICKER_W, height));
    let response = egui::Area::new(egui::Id::new("hand2-colour-picker"))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .movable(false)
        .constrain_to(board)
        .show(ctx, |ui| {
            ui.set_clip_rect(board);
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            panel_frame(0).show(ui, |ui| {
                ui.set_width(t::PICKER_W);
                let (header, _) =
                    ui.allocate_exact_size(egui::vec2(t::PICKER_W, t::PICKER_HEADER_H), egui::Sense::hover());
                for (i, (icon, tab, tip)) in [
                    (Icon::PickerWheel, Some(Tab::Wheel), "Wheel"),
                    (Icon::PickerSliders, Some(Tab::Sliders), "Sliders"),
                    (Icon::PickerHarmony, Some(Tab::Harmony), "Harmony"),
                    (Icon::PickerGradient, None, "Gradient — coming with the gradient engine"),
                ]
                .into_iter()
                .enumerate()
                {
                    let r = egui::Rect::from_min_size(
                        header.min
                            + egui::vec2(
                                t::PICKER_PAD + i as f32 * (t::PICKER_TAB_W + t::PICKER_TAB_GAP),
                                (t::PICKER_HEADER_H - t::PICKER_TAB_H) / 2.0,
                            ),
                        egui::vec2(t::PICKER_TAB_W, t::PICKER_TAB_H),
                    );
                    let response = ui.interact(
                        r,
                        ui.id().with(tip),
                        if tab.is_some() && !kit::field::blocked(ui.ctx()) {
                            egui::Sense::click()
                        } else {
                            egui::Sense::hover()
                        },
                    );
                    let on = tab == Some(m.tab);
                    if on || response.hovered() {
                        ui.painter().rect_filled(r, t::r_ctrl(), if on { t::TOGGLE_WELL } else { t::HOVER });
                    }
                    icon.paint(
                        ui.painter(),
                        r.center(),
                        t::PICKER_GLYPH,
                        if tab.is_none() {
                            t::DISABLED
                        } else if on || response.hovered() {
                            t::TEXT
                        } else {
                            t::MUTED
                        },
                    );
                    if response.on_hover_text(tip).clicked() {
                        if let Some(tab) = tab {
                            m.tab = tab;
                        }
                    }
                }
                let drag = egui::Rect::from_min_max(
                    header.min + egui::vec2(t::PICKER_PAD + 4.0 * (t::PICKER_TAB_W + t::PICKER_TAB_GAP), 0.0),
                    header.right_top() + egui::vec2(-2.0 * t::PICKER_TAB_W - t::PICKER_PAD, header.height()),
                );
                let drag_response = ui.interact(
                    drag,
                    ui.id().with("header-drag"),
                    if kit::field::blocked(ui.ctx()) { egui::Sense::hover() } else { egui::Sense::drag() },
                );
                if drag_response.dragged() {
                    let p = pos + drag_response.drag_delta() - board.min;
                    layout.position = Some([p.x, p.y]);
                }
                let mut header_ui =
                    ui.new_child(egui::UiBuilder::new().id_salt("header-actions").max_rect(egui::Rect::from_min_max(
                        header.right_top()
                            + egui::vec2(
                                -2.0 * t::PICKER_TAB_W - t::PICKER_PAD,
                                (t::PICKER_HEADER_H - t::PICKER_TAB_H) / 2.0,
                            ),
                        header.max,
                    )));
                header_ui.horizontal(|ui| {
                    if kit::icon_button_sized(
                        ui,
                        ui.id().with("eye"),
                        Icon::Pipette,
                        "Eyedropper (I)",
                        kit::IconState::Toggle(m.eyedropping),
                        egui::vec2(t::PICKER_TAB_W, t::PICKER_TAB_H),
                        t::PICKER_GLYPH,
                    )
                    .activated
                    {
                        if m.eyedropping {
                            m.eyedropping = false;
                            m.finish(ops);
                        } else if !m.disarmed_press && !kit::field::blocked(ui.ctx()) {
                            m.arm();
                        }
                    }
                    close = IA_PICKER_CLOSE.show_sized(
                        ui,
                        kit::IconState::Action,
                        egui::vec2(t::PICKER_TAB_W, t::PICKER_TAB_H),
                        t::PICKER_GLYPH,
                    );
                });
                ui.painter().hline(header.x_range(), header.bottom(), t::hairline());
                if m.tab == Tab::Wheel {
                    wheel::show(ui, m, s, ops);
                } else if m.tab == Tab::Sliders {
                    sliders::show(ui, m, s, layout, ops);
                } else {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(t::PICKER_W, t::PICKER_BODY_H), egui::Sense::hover());
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "Soon", t::mono(), t::MUTED);
                }
                ui.add_space((t::PICKER_FIELD_ROW_H - t::FIELD_H) / 2.0);
                fields::show(ui, m, ops, m.tab != Tab::Sliders);
                ui.add_space((t::PICKER_FIELD_ROW_H - t::FIELD_H) / 2.0);
                drawer::show(ui, m, s, layout, ops);
            });
        });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("picker-panel-rect"), panel_rect));
    #[cfg(test)]
    super::fields::tests::probe("picker panel", response.response.rect);
    if ctx.input(|i| i.pointer.primary_released()) {
        m.disarmed_press = false;
    }
    if layout.position.is_none() {
        let p = response.response.rect.min - board.min;
        layout.position = Some([p.x, p.y]);
    }
    if m.eyedropping && !kit::field::blocked(ctx) {
        let sample = if crate::cursors::SCREEN_EYEDROPPER { crate::cursors::screen_color_at_cursor() } else { sample };
        if let Some(c) = sample {
            m.start(Gesture::Sample, ops);
            m.adopt(c);
        }
        let down = ctx.input(|i| i.pointer.primary_pressed() && !i.key_down(egui::Key::Space));
        if down
            && sample.is_some()
            && ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
                board.contains(p)
                    && !response.response.rect.contains(p)
                    && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)
            })
        {
            m.accept_sample(ops);
        }
    }
    if let Some(c) = m.live_color() {
        ops.push(Op::PickerLive(m.target, c));
    }
    if m.gesture.is_some_and(|g| g != Gesture::Sample) && !ctx.input(|i| i.pointer.primary_down()) {
        m.finish(ops);
    }
    if !field_open
        && !menu_open
        && (m.eyedropping || ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| response.response.rect.contains(p))))
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        if m.eyedropping {
            m.finish(ops);
        } else {
            close = true;
        }
    }
    if close && !kit::field::blocked(ctx) {
        m.finish(ops);
        ops.push(Op::PickerClose);
        *panel = None;
        layout.open = false;
    }
}

pub(crate) fn snap_target_color(s: &Snap, t: PaintTarget) -> Option<Rgba> {
    match t {
        PaintTarget::Fill => s.fill,
        PaintTarget::Stroke => s.stroke,
    }
}

#[cfg(test)]
mod tests;
