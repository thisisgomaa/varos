// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The hex and alpha row: `# RRGGBB` (JetBrains Mono; eight digits set the alpha too), the alpha
//! slider (a track from transparent to the colour over a checkerboard) and its percentage. Typed
//! values commit on Enter or when the field loses the focus (Escape leaves them as they were);
//! each commit is one undo step. Alpha is the opacity of the targeted fill or stroke.

use egui::{Color32, Mesh, Pos2, Rect, Sense, Stroke, Ui, vec2};
use vectorcraft_color::Color;

use super::{Action, CTRL_R, Gesture, PAD, State, View};
use crate::theme::Tokens;

pub(crate) const ROW_H: f32 = 33.0;
const HEX_W: f32 = 56.0;
const ALPHA_W: f32 = 80.0;
const PERCENT_W: f32 = 52.0;
const CHECKER: f32 = 4.0;

/// `RRGGBB` of an sRGB colour (0..1 channels).
pub(crate) fn hex_digits(c: [f32; 3]) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{:02X}{:02X}{:02X}", b(c[0]), b(c[1]), b(c[2]))
}

/// A typed hex colour: `RGB`, `RRGGBB` or `RRGGBBAA`, with or without `#`, any case. The alpha
/// comes back only for eight digits.
pub(crate) fn parse_hex(s: &str) -> Option<([f32; 3], Option<f32>)> {
    // Match the classic panel's validation (upstream 18709b0b6): only one optional hash.
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = |a: &str| u8::from_str_radix(a, 16).ok().map(|v| v as f32 / 255.0);
    match s.len() {
        3 => {
            let e: Vec<String> = s.chars().map(|c| format!("{c}{c}")).collect();
            Some(([n(&e[0])?, n(&e[1])?, n(&e[2])?], None))
        }
        6 => Some(([n(&s[0..2])?, n(&s[2..4])?, n(&s[4..6])?], None)),
        8 => Some(([n(&s[0..2])?, n(&s[2..4])?, n(&s[4..6])?], Some(n(&s[6..8])?))),
        _ => None,
    }
}

/// A single-line text field showing `shown`; returns the typed text when it is committed (Enter or
/// focus loss, not Escape) and differs.
fn text_field(ui: &mut Ui, id: egui::Id, label: egui::Id, shown: &str, width: f32, hint: &str) -> Option<String> {
    let t = Tokens::get(ui.ctx());
    let focused = ui.memory(|m| m.has_focus(id));
    let mut buf = if focused { ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.to_string()) } else { shown.to_string() };
    let resp = egui::Frame::NONE
        .fill(t.input)
        .stroke(Stroke::new(1.0, if focused { t.accent } else { t.input_border }))
        .corner_radius(CTRL_R)
        .inner_margin(egui::Margin::symmetric(4, 4))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .desired_width(width)
                    .char_limit(9)
                    .font(egui::FontId::monospace(12.0))
                    .text_color(t.text_strong)
                    .hint_text(hint),
            )
        })
        .inner
        .labelled_by(label);
    let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
    // The field keeps its text under its id, as upstream's fields do (the shown value while it
    // hasn't the keyboard).
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    (resp.lost_focus() && !escaped && buf != shown).then_some(buf)
}

/// `#` and the hex field.
pub(super) fn hex(ui: &mut Ui, st: &mut State, v: &View, acts: &mut Vec<Action>) {
    let t = Tokens::get(ui.ctx());
    let shown = if v.mixed { String::new() } else { hex_digits(st.rgb()) };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let label = ui.label(egui::RichText::new("#").font(egui::FontId::monospace(12.0)).color(t.text_dim)).id;
        let id = ui.scope_id().with("varos-picker-hex");
        if let Some(typed) = text_field(ui, id, label, &shown, HEX_W, if v.mixed { "?" } else { "" })
            && let Some((c, alpha)) = parse_hex(&typed)
        {
            let color = Color::rgb(c[0], c[1], c[2]);
            st.adopt_rgb(c);
            acts.push(match alpha.filter(|_| v.alpha.is_some()) {
                Some(a) => {
                    st.hsva[3] = a;
                    Action::SetWithAlpha(color, a)
                }
                None => Action::Set(color),
            });
        }
    });
}

/// The alpha track `width` wide (knob room included) and its gesture.
fn alpha_slider(ui: &mut Ui, st: &mut State, v: &View, width: f32) {
    let t = Tokens::get(ui.ctx());
    let enabled = v.alpha.is_some();
    let (r, resp) = ui.allocate_exact_size(vec2(width, 24.0), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let track = Rect::from_center_size(r.center(), vec2(r.width() - super::sliders::KNOB_R * 2.0, super::sliders::TRACK_H));
    checker(&ui.painter_at(track), track, t.text_disabled, t.text_dim);
    let c = st.rgb();
    let full = Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(track.left_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(track.left_bottom(), Color32::TRANSPARENT);
    mesh.colored_vertex(track.right_top(), full);
    mesh.colored_vertex(track.right_bottom(), full);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(egui::Shape::mesh(mesh));
    // The release frame's own movement counts too (`drag_stopped`), for the alpha gesture.
    let this = st.gesture == Some(Gesture::Alpha);
    if enabled && (((resp.is_pointer_button_down_on() || resp.clicked()) && (st.gesture.is_none() || this)) || (resp.drag_stopped() && this)) {
        st.start(Gesture::Alpha);
        if let Some(p) = resp.interact_pointer_pos() {
            st.hsva[3] = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            st.alpha_change = true;
        }
    }
    let knob = egui::pos2(track.left() + st.hsva[3] * track.width(), track.center().y);
    let ink = if enabled { Color32::WHITE } else { t.text_disabled };
    ui.painter().circle(knob, super::sliders::KNOB_R, ink, Stroke::new(1.0, Color32::BLACK));
    let _ = resp.on_hover_text(tl!("Opacity"));
}

/// The percentage field of the alpha.
fn alpha_field(ui: &mut Ui, st: &mut State, v: &View, acts: &mut Vec<Action>) {
    ui.add_enabled_ui(v.alpha.is_some(), |ui| {
        let typed = super::control_corners(ui, |ui| {
            // Named for assistive technology (egui 0.37's harness checks every field has a name).
            crate::widgets::plain_field_named(ui, "varos-picker-alpha", Some(tl!("Opacity")), (st.hsva[3] * 100.0).round() as f64, "%", 0, PERCENT_W)
        });
        if let Some(a) = typed {
            let a = (a as f32 / 100.0).clamp(0.0, 1.0);
            st.hsva[3] = a;
            acts.push(Action::AlphaSet(a));
        }
    });
}

/// The row under the Wheel and Harmony tabs: hex, alpha slider, percentage. Under the Sliders
/// tab (whose header has the hex field): `A`, a slider as wide as the channel tracks, percentage.
pub(super) fn row(ui: &mut Ui, origin: Pos2, st: &mut State, v: &View, sliders: bool, acts: &mut Vec<Action>) {
    let t = Tokens::get(ui.ctx());
    let rect = Rect::from_min_size(origin + vec2(PAD, 0.0), vec2(super::W - PAD * 2.0, ROW_H));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if sliders {
            ui.label(egui::RichText::new("A").font(egui::FontId::monospace(11.0)).color(t.text_dim));
            alpha_slider(ui, st, v, super::sliders::SLIDER_W + super::sliders::KNOB_R * 2.0);
        } else {
            hex(ui, st, v, acts);
            alpha_slider(ui, st, v, ALPHA_W);
        }
        alpha_field(ui, st, v, acts);
    });
}

/// A checkerboard behind translucent colours.
pub(crate) fn checker(p: &egui::Painter, r: Rect, dark: Color32, light: Color32) {
    p.rect_filled(r, 0.0, dark);
    let (cols, rows) = ((r.width() / CHECKER).ceil() as i32, (r.height() / CHECKER).ceil() as i32);
    for gy in 0..rows {
        for gx in 0..cols {
            if (gx + gy) % 2 == 0 {
                continue;
            }
            let (x, y) = (r.left() + gx as f32 * CHECKER, r.top() + gy as f32 * CHECKER);
            let cell = Rect::from_min_max(egui::pos2(x, y), egui::pos2((x + CHECKER).min(r.right()), (y + CHECKER).min(r.bottom())));
            p.rect_filled(cell, 0.0, light);
        }
    }
}
