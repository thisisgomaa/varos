//! Colour picker v3: a Board hand, with one transaction per gesture.
use super::*;
use varos_app::shell::tokens as t;
use varos_app::storage::layout::PickerLayout;
mod board_cache;
pub(crate) use board_cache::BoardColors;
mod cluster;
mod drawer;
mod fields;
mod gradient;
mod harmony;
mod harmony_rules;
mod mini;
mod panel;
pub(crate) use panel::build_color_panel;
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
    Gradient,
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
    gradient: gradient::State,
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
    pub(crate) config: Config,
    harmony: varos_app::storage::layout::HarmonyRule,
    gesture: Option<Gesture>,
    edited: bool,
    cache: WheelCache,
    slider_cache: sliders::TrackCache,
    mode: varos_app::storage::layout::PickerMode,
    channel_state: Option<(modes::Mode, Rgba, [f32; 4])>,
}
#[derive(Clone, Copy)]
pub(crate) enum Config {
    Full,
    Mini(egui::Rect),
}
impl ColorPanel {
    pub(crate) fn mini(&self) -> bool {
        matches!(self.config, Config::Mini(_))
    }
    fn width(&self) -> f32 {
        if self.mini() {
            t::PICKER_MINI_W
        } else {
            t::PICKER_W
        }
    }

    pub(crate) fn new(target: MTarget, seed: Option<Rgba>, mixed: bool) -> Self {
        let c = seed.unwrap_or([1.0, 0.0, 0.0, 1.0]);
        let h = rgb_to_hsv(c);
        Self {
            gradient: Default::default(),
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
            config: Config::Full,
            harmony: Default::default(),
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
        if self.gradient.dragging.take().is_some() || self.gradient.editing {
            ops.push(Op::Colour(varos_core::colour_commands::ColourCommand::Commit));
            self.gradient.editing = false;
        }
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
    if let Some(m) = panel.as_mut() {
        m.selection = ed.selected_pids();
        m.config = Config::Full;
        gradient::seed(m, ed);
    }
}
/// Selection and external paint edits re-seed; our live drag retains its HSV (including grey hue).
pub(crate) fn follow_selection(m: &mut ColorPanel, ed: &mut Editor) {
    gradient::seed(m, ed);
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
    let blobs = &ed.blobs;
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
    let raster = varos_raster::rasterize_canvas_with_images(
        &snapshot,
        blobs,
        [(hole.width() * ppp).ceil() as u32, (hole.height() * ppp).ceil() as u32],
        [view.pan[0] - min[0], view.pan[1] - min[1]],
        view.zoom,
    );
    let Ok(raster) = raster.into_result() else {
        return;
    };
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

pub(crate) fn snap_target_color(s: &Snap, t: PaintTarget) -> Option<Rgba> {
    match t {
        PaintTarget::Fill => s.fill,
        PaintTarget::Stroke => s.stroke,
    }
}

#[cfg(test)]
mod tests;
