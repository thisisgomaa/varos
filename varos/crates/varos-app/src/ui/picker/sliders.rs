//! Channel tracks cache sRGB meshes per (mode, channels, alpha). Web controls snap to six values.
// ---- Lane F: shaped chrome ----
use varos_app::shell::kit::text::ShapedPainter as _;
// ---- end Lane F ----
use super::*;
use modes::{channels, Mode};
use varos_app::shell::kit::field::{self as kf, Label, NumberField};
#[derive(Default)]
pub(super) struct TrackCache {
    key: Option<(Mode, [f32; 4], f32)>,
    meshes: Vec<egui::Mesh>,
}
impl TrackCache {
    fn get(&mut self, mode: Mode, color: Rgba, values: [f32; 4]) -> &[egui::Mesh] {
        if self.key != Some((mode, values, color[3])) {
            self.meshes = channels(mode)
                .iter()
                .enumerate()
                .map(|(channel, (_, lo, hi))| {
                    let mut mesh = egui::Mesh::default();
                    const STEPS: u32 = 64;
                    for i in 0..=STEPS {
                        let f = i as f32 / STEPS as f32;
                        let mut v = values;
                        v[channel] = lo + (hi - lo) * f;
                        if mode == Mode::Web {
                            v[channel] = (v[channel] / 51.0).round() * 51.0;
                        }
                        let c = rgba_c32a(modes::rgb(mode, v, color[3]));
                        mesh.colored_vertex(egui::pos2(f * t::PICKER_SLIDER_W, 0.0), c);
                        mesh.colored_vertex(egui::pos2(f * t::PICKER_SLIDER_W, t::PICKER_ALPHA_TRACK), c);
                    }
                    for i in 0..STEPS {
                        let n = i * 2;
                        mesh.add_triangle(n, n + 1, n + 2);
                        mesh.add_triangle(n + 1, n + 3, n + 2);
                    }
                    mesh
                })
                .collect();
            self.key = Some((mode, values, color[3]));
        }
        &self.meshes
    }
}
pub(super) fn body_height(mode: Mode) -> f32 {
    t::PICKER_SLIDER_HEADER_H + channels(mode).len() as f32 * t::PICKER_SLIDER_ROW_H
}
pub(super) fn show(ui: &mut egui::Ui, m: &mut ColorPanel, s: &Snap, layout: &mut PickerLayout, ops: &mut Vec<Op>) {
    let (body, _) = ui.allocate_exact_size(egui::vec2(t::PICKER_W, body_height(m.mode)), egui::Sense::hover());
    let header = egui::Rect::from_min_size(
        body.min + egui::Vec2::splat(t::PICKER_PAD),
        egui::vec2(t::PICKER_MODE_W, t::PICKER_SLIDER_HEADER_H),
    );
    let mut left = ui.new_child(egui::UiBuilder::new().id_salt("slider-header").max_rect(header));
    let mode_index = modes::MODES.iter().position(|mode| *mode == m.mode).unwrap();
    if let Some(i) = kit::text_dropdown(
        &mut left,
        ui.id().with("picker-mode"),
        modes::NAMES[mode_index],
        &modes::NAMES,
        t::PICKER_MODE_W,
        "Colour mode",
    )
    .filter(|_| !kf::blocked(ui.ctx()))
    {
        m.finish(ops);
        layout.mode = modes::MODES[i];
        m.mode = layout.mode;
    }
    left.add_space(t::PICKER_PAD);
    fields::hex(&mut left, m, ops);
    cluster::show(ui, body.min + egui::vec2(0.0, -t::PICKER_SLIDER_CLUSTER_Y), m, s, ops);
    for (index, &(label, lo, hi)) in channels(m.mode).iter().enumerate() {
        let y = body.top() + t::PICKER_SLIDER_HEADER_H + index as f32 * t::PICKER_SLIDER_ROW_H;
        let row = egui::Rect::from_min_size(
            egui::pos2(body.left() + t::PICKER_PAD, y),
            egui::vec2(t::PICKER_W - t::PICKER_PAD * 2.0, t::PICKER_SLIDER_ROW_H),
        );
        ui.painter().shaped_text(row.left_center(), Align2::LEFT_CENTER, label, t::mono(), t::MUTED);
        let track = egui::Rect::from_center_size(
            egui::pos2(row.left() + t::PICKER_SLIDER_LABEL_W + t::PICKER_SLIDER_W / 2.0, row.center().y),
            egui::vec2(t::PICKER_SLIDER_W, t::PICKER_ALPHA_TRACK),
        );
        let hit = ui.interact(
            track.expand(t::PICKER_ALPHA_KNOB),
            ui.id().with(("channel", index)),
            if kf::blocked(ui.ctx()) { egui::Sense::hover() } else { egui::Sense::click_and_drag() },
        );
        #[cfg(test)]
        super::super::fields::tests::probe(&format!("picker channel {index}"), track);
        if hit.is_pointer_button_down_on() || hit.clicked() {
            m.start(Gesture::Slider, ops);
            if let Some(pos) = hit.interact_pointer_pos() {
                let value = lo + (hi - lo) * ((pos.x - track.left()) / track.width()).clamp(0., 1.);
                m.adopt_channel(index, value);
            }
        }
        let channel_values = m.channel_values();
        let mut mesh = m.slider_cache.get(m.mode, m.color(), channel_values)[index].clone();
        mesh.translate(track.min.to_vec2());
        ui.painter().add(egui::Shape::mesh(mesh));
        let value = channel_values[index];
        let knob = egui::pos2(track.left() + (value - lo) / (hi - lo) * track.width(), track.center().y);
        ui.painter().circle(knob, t::PICKER_ALPHA_KNOB, t::PICKER_WHITE, Stroke::new(t::KIT_STROKE, t::PICKER_BLACK));
        let field_rect = egui::Rect::from_min_max(egui::pos2(track.right() + t::PICKER_PAD, row.top()), row.max);
        let mut field_ui = ui.new_child(egui::UiBuilder::new().id_salt(("channel-value", index)).max_rect(field_rect));
        let edit = kf::number_value(
            &mut field_ui,
            NumberField {
                id: doc_id(ui, format!("picker-channel-{index}")),
                width: field_rect.width(),
                label: Label::Letter(""),
                tip: label,
                value,
                decimals: 0,
                speed: 1.0,
                range: lo..=hi,
                disabled: false,
            },
            (m.mode == Mode::Web).then_some(51.0),
        );
        if let Some(value) = edit.live.or(edit.commit).filter(|_| !kf::blocked(ui.ctx())) {
            m.adopt_channel(index, value);
            m.change_requested = false;
        }
        let convert = |v| m.channel_color(index, v);
        fields::route(
            kf::Edit {
                id: edit.id,
                rect: edit.rect,
                commit: edit.commit.map(convert),
                pending: edit.pending.map(convert),
                editing: edit.editing,
                closed: edit.closed,
                live: edit.live.map(convert),
            },
            m,
            ops,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracks_rebuild_only_for_mode_channels_or_alpha() {
        let mut cache = TrackCache::default();
        let c = [0.4, 0.6, 0.8, 1.];
        let first = cache.get(Mode::Rgb, c, modes::values(Mode::Rgb, c))[0].vertices.as_ptr();
        for _ in 0..16 {
            assert_eq!(first, cache.get(Mode::Rgb, c, modes::values(Mode::Rgb, c))[0].vertices.as_ptr());
        }
        assert_eq!(cache.get(Mode::Cmyk, c, modes::values(Mode::Cmyk, c)).len(), 4);
        cache.get(Mode::Lab, [0.1, 0.2, 0.3, 1.], modes::values(Mode::Lab, [0.1, 0.2, 0.3, 1.]));
        assert_eq!(cache.key, Some((Mode::Lab, modes::values(Mode::Lab, [0.1, 0.2, 0.3, 1.]), 1.)));
    }
}

#[test]
fn grey_wheel_hue_updates_saturation_track_on_return_to_sliders() {
    let mut m = ColorPanel::new(MTarget::Paint(PaintTarget::Fill), Some([0.5, 0.5, 0.5, 1.]), false);
    let mut cache = TrackCache::default();
    let old = cache.get(m.mode, m.color(), m.channel_values())[1].vertices.clone();
    m.tab = Tab::Wheel;
    m.hsva[0] = 0.5;
    m.tab = Tab::Sliders;
    let new = cache.get(m.mode, m.color(), m.channel_values())[1].vertices.clone();
    assert_ne!(old.last().unwrap().color, new.last().unwrap().color);
    assert_eq!(new.last().unwrap().color, rgba_c32a([0., 0.5, 0.5, 1.]));
    m.hsva[3] = 0.5;
    assert_ne!(
        new.last().unwrap().color,
        cache.get(m.mode, m.color(), m.channel_values())[1].vertices.last().unwrap().color
    );
}
