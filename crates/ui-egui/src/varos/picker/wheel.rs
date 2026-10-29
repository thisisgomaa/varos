// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The Wheel: a hue ring (red at three o'clock, counter-clockwise) around an HSV triangle whose
//! hue vertex sits on the ring at the current hue, white at hue + 120°, black at hue + 240°: the
//! triangle turns with the hue. The ring mesh is cached per size, the triangle per hue and size.
//! The geometry and its maths are the old Varos picker's, as they were.

use std::f32::consts::TAU;

use egui::{Color32, Mesh, Pos2, Rect, Sense, Stroke, Ui, vec2};

use super::{Action, Gesture, State, Tab, hsv_c32};
use crate::theme::Tokens;

/// The wheel area (the Figma `Picker / Wheel` body), its ring and triangle.
pub(crate) const BODY_H: f32 = 248.0;
pub(crate) const RING_R: f32 = 99.0;
pub(crate) const RING_BAND: f32 = 14.0;
pub(crate) const CENTER: [f32; 2] = [123.0, 118.0];
pub(crate) const TRIANGLE_R: f32 = 84.0;
const RING_MARKER: f32 = 6.0;
const TRI_MARKER: f32 = 5.0;
const MARKER_STROKE: f32 = 2.0;
const MARKER_HALO: f32 = 1.0;
const DEFAULT_SIZE: f32 = 20.0;
const DEFAULT_POS: [f32; 2] = [188.0, 12.0];
const READOUT_POS: [f32; 2] = [14.0, 210.0];
const READOUT_LINE: f32 = 14.0;
const READOUT_LABEL_W: f32 = 18.0;
/// The marker colours and the triangle's white and black vertices are colour maths, not chrome.
const WHITE: Color32 = Color32::WHITE;
const BLACK: Color32 = Color32::BLACK;

/// Cached meshes: the ring once per radius, the triangle once per hue and radius.
#[derive(Clone, Default)]
pub(crate) struct Cache {
    ring: Option<(f32, Mesh)>,
    triangle: Option<(f32, f32, Mesh)>,
    #[cfg(test)]
    pub(crate) builds: (usize, usize),
}

/// The point at hue `h` (0..1) on the circle of radius `r` around `c`: red at three o'clock,
/// counter-clockwise on screen.
pub(crate) fn ring_pos(c: Pos2, r: f32, h: f32) -> Pos2 {
    c + vec2((h * TAU).cos(), -(h * TAU).sin()) * r
}

/// The hue (0..1) of the direction from `c` to `p`.
pub(crate) fn ring_hue(c: Pos2, p: Pos2) -> f32 {
    (-(p.y - c.y)).atan2(p.x - c.x).rem_euclid(TAU) / TAU
}

/// Is `p` on the ring of outer radius `r` and thickness `band` around `c`?
pub(crate) fn ring_hit(c: Pos2, p: Pos2, r: f32, band: f32) -> bool {
    let d = p.distance(c);
    d >= r - band && d <= r
}

/// The triangle's vertices around the origin: hue, white, black.
pub(crate) fn vertices(h: f32, r: f32) -> [Pos2; 3] {
    [ring_pos(Pos2::ZERO, r, h), ring_pos(Pos2::ZERO, r, h + 1.0 / 3.0), ring_pos(Pos2::ZERO, r, h + 2.0 / 3.0)]
}

/// Where saturation `s` and value `v` sit in the triangle of hue `h` (around the origin).
pub(crate) fn sv_pos(h: f32, r: f32, s: f32, v: f32) -> Pos2 {
    let [a, b, c] = vertices(h, r);
    Pos2::ZERO + a.to_vec2() * (s * v) + b.to_vec2() * ((1.0 - s) * v) + c.to_vec2() * (1.0 - v)
}

pub(crate) fn barycentric(p: Pos2, [a, b, c]: [Pos2; 3]) -> [f32; 3] {
    let cross = |a: egui::Vec2, b: egui::Vec2| a.x * b.y - a.y * b.x;
    let det = cross(b - a, c - a);
    let y = cross(p - a, c - a) / det;
    let z = cross(b - a, p - a) / det;
    [1.0 - y - z, y, z]
}

/// Is `p` (around the origin) inside the triangle of hue `h`?
pub(crate) fn in_triangle(h: f32, r: f32, p: Pos2) -> bool {
    barycentric(p, vertices(h, r)).iter().all(|w| *w >= 0.0)
}

/// Saturation and value at `p` (around the origin). Outside drags clamp to the closest edge
/// rather than jump around an extended plane.
pub(crate) fn pos_sv(h: f32, r: f32, mut p: Pos2) -> [f32; 2] {
    let vs = vertices(h, r);
    if barycentric(p, vs).iter().any(|w| *w < 0.0) {
        p = (0..3)
            .map(|i| {
                let a = vs[i];
                let b = vs[(i + 1) % 3];
                let t = ((p - a).dot(b - a) / (b - a).length_sq()).clamp(0.0, 1.0);
                a + (b - a) * t
            })
            .min_by(|a, b| a.distance_sq(p).total_cmp(&b.distance_sq(p)))
            .unwrap_or(p);
    }
    let [a, b, _] = barycentric(p, vs);
    let v = (a + b).clamp(0.0, 1.0);
    [if v > 1e-6 { (a / v).clamp(0.0, 1.0) } else { 0.0 }, v]
}

impl Cache {
    pub(crate) fn ring(&mut self, r: f32) -> &Mesh {
        if self.ring.as_ref().is_some_and(|(size, _)| *size != r) {
            self.ring = None;
        }
        #[cfg(test)]
        if self.ring.is_none() {
            self.builds.0 += 1;
        }
        &self
            .ring
            .get_or_insert_with(|| {
                let mut mesh = Mesh::default();
                const N: u32 = 360;
                for i in 0..=N {
                    let h = i as f32 / N as f32;
                    let c = hsv_c32(h, 1.0, 1.0);
                    mesh.colored_vertex(ring_pos(Pos2::ZERO, r, h), c);
                    mesh.colored_vertex(ring_pos(Pos2::ZERO, r - RING_BAND, h), c);
                }
                for i in 0..N {
                    let a = i * 2;
                    mesh.add_triangle(a, a + 1, a + 2);
                    mesh.add_triangle(a + 1, a + 3, a + 2);
                }
                (r, mesh)
            })
            .1
    }

    pub(crate) fn triangle(&mut self, h: f32, r: f32) -> &Mesh {
        if self.triangle.as_ref().is_some_and(|(hue, size, _)| *hue != h || *size != r) {
            self.triangle = None;
        }
        #[cfg(test)]
        if self.triangle.is_none() {
            self.builds.1 += 1;
        }
        &self
            .triangle
            .get_or_insert_with(|| {
                // Vertex colours are exactly the hue, white and black: barycentric interpolation is
                // HSV's v·s hue + v·(1−s) white + (1−v) black, with no per-frame raster.
                let mut mesh = Mesh::default();
                for (p, c) in vertices(h, r).into_iter().zip([hsv_c32(h, 1.0, 1.0), WHITE, BLACK]) {
                    mesh.colored_vertex(p, c);
                }
                mesh.add_triangle(0, 1, 2);
                (h, r, mesh)
            })
            .2
    }
}

fn translated(mesh: &Mesh, c: Pos2) -> egui::Shape {
    let mut m = mesh.clone();
    m.translate(c.to_vec2());
    egui::Shape::mesh(m)
}

fn marker(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    p.circle_filled(c, r, color);
    p.circle_stroke(c, r + MARKER_HALO, Stroke::new(MARKER_HALO, BLACK));
    p.circle_stroke(c, r, Stroke::new(MARKER_STROKE, WHITE));
}

/// The ring and the triangle in `rect` (the wheel area), with the press that starts a gesture.
/// `harmony` marks the linked hues of the Harmony tab.
pub(super) fn wheel(ui: &mut Ui, rect: Rect, st: &mut State, harmony: Option<super::Rule>) {
    let simple = ui.stack().iter().any(|node| node.tags().contains("simple-picker"));
    let ring_r = if simple { 90.0 } else { RING_R };
    let triangle_r = if simple { 75.0 } else { TRIANGLE_R };
    let c = rect.min + vec2(CENTER[0], CENTER[1]);
    let hit =
        ui.interact(Rect::from_center_size(c, egui::Vec2::splat(ring_r * 2.0)), ui.scope_id().with("varos-picker-wheel"), Sense::click_and_drag());
    let pos = hit.interact_pointer_pos();
    if (hit.is_pointer_button_down_on() || hit.clicked())
        && st.gesture.is_none()
        && let Some(p) = pos
    {
        if ring_hit(c, p, ring_r, RING_BAND) {
            st.start(Gesture::Ring);
        } else if in_triangle(st.hsva[0], triangle_r, Pos2::ZERO + (p - c)) {
            st.start(Gesture::Triangle);
        }
    }
    // The release frame's own movement counts too: the commit uses where the pointer let go.
    if (hit.is_pointer_button_down_on() || hit.clicked() || hit.drag_stopped())
        && let Some(p) = pos
    {
        match st.gesture {
            Some(Gesture::Ring) => {
                st.channels = None;
                st.hsva[0] = ring_hue(c, p);
                st.change = true;
            }
            Some(Gesture::Triangle) => {
                st.channels = None;
                let [s, v] = pos_sv(st.hsva[0], triangle_r, Pos2::ZERO + (p - c));
                st.hsva[1] = s;
                st.hsva[2] = v;
                st.change = true;
            }
            _ => {}
        }
    }
    let painter = ui.painter();
    painter.add(translated(st.wheel.ring(ring_r), c));
    painter.add(translated(st.wheel.triangle(st.hsva[0], triangle_r), c));
    if let Some(rule) = harmony {
        for [h, s, v] in super::harmony::linked(rule, [st.hsva[0], st.hsva[1], st.hsva[2]]).into_iter().skip(1) {
            let at = if rule == super::Rule::Mono { c + sv_pos(h, triangle_r, s, v).to_vec2() } else { ring_pos(c, ring_r - RING_BAND / 2.0, h) };
            marker(painter, at, TRI_MARKER, hsv_c32(h, s, v));
        }
    }
    marker(painter, ring_pos(c, ring_r - RING_BAND / 2.0, st.hsva[0]), RING_MARKER, hsv_c32(st.hsva[0], 1.0, 1.0));
    marker(painter, c + sv_pos(st.hsva[0], triangle_r, st.hsva[1], st.hsva[2]).to_vec2(), TRI_MARKER, hsv_c32(st.hsva[0], st.hsva[1], st.hsva[2]));
}

/// The Wheel tab's body: the wheel, the Default button, the live readout and the Fill/Stroke
/// cluster, `BODY_H` tall.
pub(super) fn show(ui: &mut Ui, origin: Pos2, st: &mut State, v: &super::View, prefs: &super::Prefs, acts: &mut Vec<Action>) {
    let rect = Rect::from_min_size(origin, vec2(super::W, BODY_H));
    let harmony = (prefs.tab == Tab::Harmony).then_some(prefs.harmony);
    wheel(ui, rect, st, harmony);
    let t = Tokens::get(ui.ctx());
    // Default: two squares side by side, the default fill (white) and stroke (black) (D).
    let default = Rect::from_min_size(rect.min + vec2(DEFAULT_POS[0], DEFAULT_POS[1]), vec2(DEFAULT_SIZE * 2.0, DEFAULT_SIZE));
    let resp = ui.interact(default, ui.scope_id().with("varos-picker-default"), Sense::click());
    let edge = Stroke::new(1.0, if resp.hovered() { t.text } else { t.text_dim });
    for (i, fill) in [WHITE, BLACK].into_iter().enumerate() {
        let r = Rect::from_min_size(default.min + vec2(i as f32 * DEFAULT_SIZE, 0.0), egui::Vec2::splat(DEFAULT_SIZE));
        ui.painter().rect(r, egui::CornerRadius::ZERO, fill, edge, egui::StrokeKind::Inside);
    }
    if resp.on_hover_text(tl!("Default Fill and Stroke (D)")).clicked() {
        acts.push(Action::Default);
    }
    // Live readout in the slider mode's channels.
    let lines = super::sliders::readout(st, prefs.mode);
    let lift = if lines.len() > 3 { READOUT_LINE } else { 0.0 };
    for (i, (label, value)) in lines.into_iter().enumerate() {
        let pos = rect.min + vec2(READOUT_POS[0], READOUT_POS[1] - lift + i as f32 * READOUT_LINE);
        let mono = egui::FontId::monospace(11.0);
        let offset = if label.is_empty() {
            0.0
        } else {
            ui.painter().text(pos, egui::Align2::LEFT_TOP, format!("{label}:"), mono.clone(), t.text_dim);
            READOUT_LABEL_W
        };
        ui.painter().text(pos + vec2(offset, 0.0), egui::Align2::LEFT_TOP, value, mono, t.text);
    }
    super::cluster::show(ui, rect.min, v, acts);
}
