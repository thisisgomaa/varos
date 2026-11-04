// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The Sliders tab: a mode menu (HSB · HSL · RGB · CMYK · Lab · Web) and the hex field on the left,
//! the Fill/Stroke cluster on the right, then one row per channel: its letter, a track showing the
//! colours that channel reaches at the current values, a knob and an editable value.
//!
//! CMYK is colour managed and writes CMYK, as upstream's Classic CMYK sliders do: the sliders show
//! a CMYK colour's own inks (a spot or global colour's too) and any other colour's separation in
//! the working CMYK space ([`Color::to_cmyk_managed`], as upstream's Color Picker dialog shows it;
//! Classic seeds an RGB colour with the profile-free [`Color::to_cmyk`]). An edit writes the CMYK
//! colour Classic's [`classic::from_components`] builds, typed values exactly, and the tracks and
//! the shown colour go through the active colour settings ([`Color::to_rgb`]). A spot colour is
//! written only by an edit, which makes it a CMYK process colour (as Classic's CMYK mode does).
//! HSB, HSL, RGB and Web write RGB. Lab is still the old Varos picker's sRGB/XYZ D65 maths, not
//! colour managed, and writes RGB.

use egui::{Mesh, Pos2, Rect, Sense, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
use vectorcraft_color::Color;

use super::{Action, Gesture, PAD, State, View, hsv_to_rgb, rgb_to_hsv, same};
use crate::panels::color as classic;
use crate::theme::Tokens;
use crate::varos::theme_components::FOUNDATION;

pub(crate) const HEADER_H: f32 = 72.0;
pub(crate) const ROW_H: f32 = FOUNDATION.control_height;
const LABEL_W: f32 = 18.0;
pub(crate) const SLIDER_W: f32 = 140.0;
pub(crate) const TRACK_H: f32 = 8.0;
pub(crate) const KNOB_R: f32 = 7.0;
const MODE_W: f32 = 100.0;
/// The cluster sits this much higher than on the Wheel tab: top right of the Sliders header.
const CLUSTER_LIFT: f32 = 188.0;

/// The slider mode, kept with the picker's preferences; the readout follows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Hsb,
    Hsl,
    Rgb,
    Cmyk,
    Lab,
    Web,
}

pub(crate) const MODES: [Mode; 6] = [Mode::Hsb, Mode::Hsl, Mode::Rgb, Mode::Cmyk, Mode::Lab, Mode::Web];
/// Model names, shown as they are.
pub(crate) const NAMES: [&str; 6] = ["HSB", "HSL", "RGB", "CMYK", "Lab", "Web"];

/// Each channel's letter and range.
pub(crate) fn channels(mode: Mode) -> &'static [(&'static str, f32, f32)] {
    match mode {
        Mode::Hsb => &[("H", 0., 360.), ("S", 0., 100.), ("B", 0., 100.)],
        Mode::Hsl => &[("H", 0., 360.), ("S", 0., 100.), ("L", 0., 100.)],
        Mode::Rgb | Mode::Web => &[("R", 0., 255.), ("G", 0., 255.), ("B", 0., 255.)],
        Mode::Cmyk => &[("C", 0., 100.), ("M", 0., 100.), ("Y", 0., 100.), ("K", 0., 100.)],
        Mode::Lab => &[("L", 0., 100.), ("a", -128., 127.), ("b", -128., 127.)],
    }
}

/// The channel values of sRGB colour `c` in `mode` (CMYK: its working CMYK inks).
pub(crate) fn values(mode: Mode, c: [f32; 3]) -> [f32; 4] {
    let [h, s, v] = rgb_to_hsv(c);
    match mode {
        Mode::Hsb => [h * 360., s * 100., v * 100., 0.],
        Mode::Hsl => {
            let l = v * (1. - s / 2.);
            let d = l.min(1. - l);
            [h * 360., if d > 0. { (v - l) / d * 100. } else { 0. }, l * 100., 0.]
        }
        Mode::Rgb | Mode::Web => [c[0] * 255., c[1] * 255., c[2] * 255., 0.],
        Mode::Cmyk => inks(&Color::rgb(c[0], c[1], c[2])),
        Mode::Lab => {
            let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
            let [r, g, b] = [lin(c[0]), lin(c[1]), lin(c[2])];
            let f = |v: f32| if v > 216. / 24389. { v.cbrt() } else { v * 24389. / 3132. + 16. / 116. };
            let x = f((0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047);
            let y = f(0.2126729 * r + 0.7151522 * g + 0.072175 * b);
            let z = f((0.0193339 * r + 0.119192 * g + 0.9503041 * b) / 1.08883);
            [116. * y - 16., 500. * (x - y), 200. * (y - z), 0.]
        }
    }
}

/// The sRGB colour of channel values `v` in `mode` (CMYK: the inks shown through the colour settings).
pub(crate) fn rgb(mode: Mode, v: [f32; 4]) -> [f32; 3] {
    let c = match mode {
        Mode::Hsb => hsv_to_rgb(v[0] / 360., v[1] / 100., v[2] / 100.),
        Mode::Hsl => {
            let l = v[2] / 100.;
            let b = l + v[1] / 100. * l.min(1. - l);
            hsv_to_rgb(v[0] / 360., if b > 0. { 2. * (1. - l / b) } else { 0. }, b)
        }
        Mode::Rgb | Mode::Web => [v[0] / 255., v[1] / 255., v[2] / 255.],
        Mode::Cmyk => cmyk(v).to_rgb(),
        Mode::Lab => {
            let y = (v[0] + 16.) / 116.;
            let x = y + v[1] / 500.;
            let z = y - v[2] / 200.;
            let f = |v: f32| if v > 6. / 29. { v * v * v } else { (v - 16. / 116.) * 3132. / 24389. };
            let [x, y, z] = [f(x) * 0.95047, f(y), f(z) * 1.08883];
            let enc = |v: f32| if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1. / 2.4) - 0.055 };
            [
                enc(3.2404542 * x - 1.5371385 * y - 0.4985314 * z),
                enc(-0.969266 * x + 1.8760108 * y + 0.041556 * z),
                enc(0.0556434 * x - 0.2040259 * y + 1.0572252 * z),
            ]
        }
    };
    c.map(|x| x.clamp(0., 1.))
}

/// The CMYK inks (%) of `c`: a CMYK colour's own, any other through the working CMYK profile.
pub(crate) fn inks(c: &Color) -> [f32; 4] {
    c.to_cmyk_managed(vectorcraft_color::cms::active().settings().intent).map(|v| v * 100.)
}

/// The CMYK colour of inks `v` (%), built as the Classic sliders build it.
pub(crate) fn cmyk(v: [f32; 4]) -> Color {
    classic::from_components(classic::Mode::Cmyk, &v)
}

/// Web snaps each channel to the six web-safe levels.
fn snap(mode: Mode, value: f32) -> f32 {
    if mode == Mode::Web { (value / 51.0).round() * 51.0 } else { value }
}

/// The values the sliders show: kept while the colour is the one they made (a hue survives S = 0),
/// in CMYK the inks of the colour held (exact for a CMYK colour), else computed from the colour.
pub(crate) fn values_for(st: &State, mode: Mode) -> [f32; 4] {
    let now = st.rgb();
    if let Some((m, c, v)) = st.channels
        && m == mode
        && same(c, now)
    {
        return v;
    }
    if mode == Mode::Cmyk
        && let Some(c) = st.shown()
    {
        return inks(&c);
    }
    let mut v = values(mode, now);
    if mode == Mode::Hsb {
        v[1] = st.hsva[1] * 100.;
        v[2] = st.hsva[2] * 100.;
    }
    if matches!(mode, Mode::Hsb | Mode::Hsl) {
        v[0] = st.hsva[0] * 360.;
    }
    v
}

/// Set channel `i` to `value`: the colour follows, the other channels hold. In CMYK the picker then
/// holds the CMYK colour to write ([`State::out_color`]).
pub(crate) fn adopt_channel(st: &mut State, mode: Mode, i: usize, value: f32) {
    let mut v = values_for(st, mode);
    v[i] = snap(mode, value);
    st.adopt_rgb(rgb(mode, v));
    if mode == Mode::Cmyk {
        st.held = Some(cmyk(v));
    }
    if matches!(mode, Mode::Hsb | Mode::Hsl) {
        st.hsva[0] = (v[0] / 360.).rem_euclid(1.0);
    }
    if mode == Mode::Hsb {
        st.hsva[1] = v[1] / 100.;
        st.hsva[2] = v[2] / 100.;
    }
    st.channels = Some((mode, st.rgb(), v));
    st.change = true;
}

/// The live readout's lines: label and value (Web: the hex alone).
pub(crate) fn readout(st: &State, mode: Mode) -> Vec<(&'static str, String)> {
    if mode == Mode::Web {
        let c = st.rgb();
        return vec![("", Color::rgb(c[0], c[1], c[2]).to_hex().to_uppercase())];
    }
    let v = values_for(st, mode);
    channels(mode).iter().enumerate().map(|(i, (name, _, _))| (*name, format!("{:.0}", v[i]))).collect()
}

/// Track meshes per (mode, values, alpha): rebuilt only when one of them changes.
#[derive(Clone, Default)]
pub(crate) struct Cache {
    key: Option<(Mode, [f32; 4])>,
    meshes: Vec<Mesh>,
    #[cfg(test)]
    pub(crate) builds: usize,
}

impl Cache {
    pub(crate) fn get(&mut self, mode: Mode, values: [f32; 4]) -> &[Mesh] {
        if self.key != Some((mode, values)) {
            self.meshes = channels(mode)
                .iter()
                .enumerate()
                .map(|(channel, (_, lo, hi))| {
                    let mut mesh = Mesh::default();
                    const STEPS: u32 = 64;
                    for i in 0..=STEPS {
                        let f = i as f32 / STEPS as f32;
                        let mut v = values;
                        v[channel] = snap(mode, lo + (hi - lo) * f);
                        let [r, g, b] = rgb(mode, v);
                        let c = egui::Color32::from_rgb((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8);
                        mesh.colored_vertex(egui::pos2(f * SLIDER_W, 0.0), c);
                        mesh.colored_vertex(egui::pos2(f * SLIDER_W, TRACK_H), c);
                    }
                    for i in 0..STEPS {
                        let n = i * 2;
                        mesh.add_triangle(n, n + 1, n + 2);
                        mesh.add_triangle(n + 1, n + 3, n + 2);
                    }
                    mesh
                })
                .collect();
            self.key = Some((mode, values));
            #[cfg(test)]
            {
                self.builds += 1;
            }
        }
        &self.meshes
    }
}

/// The Sliders tab's height for `mode`.
pub(crate) fn body_height(mode: Mode) -> f32 {
    HEADER_H + channels(mode).len() as f32 * ROW_H
}

/// The Sliders tab's body at `origin`.
pub(super) fn show(ui: &mut Ui, origin: Pos2, st: &mut State, v: &View, prefs: &mut super::Prefs, acts: &mut Vec<Action>) {
    let t = Tokens::get(ui.ctx());
    let mode = prefs.mode;
    let header = Rect::from_min_size(origin + vec2(PAD, PAD), vec2(MODE_W, HEADER_H - PAD));
    ui.scope_builder(egui::UiBuilder::new().max_rect(header).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        let index = MODES.iter().position(|m| *m == mode).unwrap_or(0);
        if let Some(i) = super::control_corners(ui, |ui| {
            crate::widgets::dropdown_names_named(ui, "varos-picker-mode", Some(tl!("Color Mode")), NAMES[index], &NAMES, MODE_W)
        }) {
            prefs.mode = MODES[i];
        }
        super::fields::hex(ui, st, v, acts);
    });
    super::cluster::show(ui, origin - vec2(0.0, CLUSTER_LIFT), v, acts);
    for (index, &(label, lo, hi)) in channels(mode).iter().enumerate() {
        let row = Rect::from_min_size(origin + vec2(PAD, HEADER_H + index as f32 * ROW_H), vec2(super::W - PAD * 2.0, ROW_H));
        ui.painter().text(row.left_center(), egui::Align2::LEFT_CENTER, label, egui::FontId::monospace(FOUNDATION.control_font), t.text_dim);
        let track = Rect::from_min_size(egui::pos2(row.left() + LABEL_W, row.center().y - TRACK_H / 2.0), vec2(SLIDER_W, TRACK_H));
        let hit = ui.interact(track.expand(KNOB_R), ui.scope_id().with(("varos-picker-channel", index)), Sense::click_and_drag());
        // The release frame's own movement counts too (`drag_stopped`), for this channel's gesture.
        let this = st.gesture == Some(Gesture::Channel(index));
        if ((hit.is_pointer_button_down_on() || hit.clicked()) && (st.gesture.is_none() || this)) || (hit.drag_stopped() && this) {
            st.start(Gesture::Channel(index));
            if let Some(p) = hit.interact_pointer_pos() {
                adopt_channel(st, mode, index, lo + (hi - lo) * ((p.x - track.left()) / track.width()).clamp(0., 1.));
            }
        }
        let values = values_for(st, mode);
        let mut mesh = st.tracks.get(mode, values)[index].clone();
        mesh.translate(track.min.to_vec2());
        ui.painter().add(egui::Shape::mesh(mesh));
        let knob = egui::pos2(track.left() + (values[index] - lo) / (hi - lo) * track.width(), track.center().y);
        ui.painter().circle(knob, KNOB_R, egui::Color32::WHITE, Stroke::new(1.0, egui::Color32::BLACK));
        let field = Rect::from_min_max(egui::pos2(track.right() + PAD, row.top()), row.max);
        // CMYK shows a tenth of a percent, so a typed 12.5 reads back as it is stored.
        let places = if mode == Mode::Cmyk { 1 } else { 0 };
        let scale = 10f32.powi(places as i32);
        let typed = ui
            .scope_builder(egui::UiBuilder::new().max_rect(field).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                super::control_corners(ui, |ui| {
                    crate::widgets::plain_field_named(
                        ui,
                        ("varos-picker-value", index),
                        Some(label),
                        ((values[index] * scale).round() / scale) as f64,
                        "",
                        places,
                        field.width(),
                    )
                })
            })
            .inner;
        if let Some(value) = typed {
            adopt_channel(st, mode, index, (value as f32).clamp(lo, hi));
            acts.push(Action::Commit);
        }
    }
}
