use super::*;
use std::f32::consts::TAU;

#[derive(Default)]
pub(crate) struct WheelCache {
    ring: Option<(f32, egui::Mesh)>,
    triangle: Option<(f32, f32, egui::Mesh)>,
}
/// Red at three o'clock, counter-clockwise in screen coordinates.
pub(crate) fn ring_pos(c: egui::Pos2, r: f32, h: f32) -> egui::Pos2 {
    c + egui::vec2((h * TAU).cos(), -(h * TAU).sin()) * r
}
pub(crate) fn ring_hue(c: egui::Pos2, p: egui::Pos2) -> f32 {
    (-(p.y - c.y)).atan2(p.x - c.x).rem_euclid(TAU) / TAU
}
pub(crate) fn ring_hit(c: egui::Pos2, p: egui::Pos2, r: f32, band: f32) -> bool {
    let d = p.distance(c);
    d >= r - band && d <= r
}
pub(crate) fn vertices(h: f32, r: f32) -> [egui::Pos2; 3] {
    [
        ring_pos(egui::Pos2::ZERO, r, h),
        ring_pos(egui::Pos2::ZERO, r, h + 1.0 / 3.0),
        ring_pos(egui::Pos2::ZERO, r, h + 2.0 / 3.0),
    ]
}
pub(crate) fn sv_pos(h: f32, r: f32, s: f32, v: f32) -> egui::Pos2 {
    let [a, b, c] = vertices(h, r);
    egui::Pos2::ZERO + a.to_vec2() * (s * v) + b.to_vec2() * ((1.0 - s) * v) + c.to_vec2() * (1.0 - v)
}
fn barycentric(p: egui::Pos2, [a, b, c]: [egui::Pos2; 3]) -> [f32; 3] {
    let cross = |a: egui::Vec2, b: egui::Vec2| a.x * b.y - a.y * b.x;
    let det = cross(b - a, c - a);
    let y = cross(p - a, c - a) / det;
    let z = cross(b - a, p - a) / det;
    [1.0 - y - z, y, z]
}
/// Outside drags clamp to the closest edge rather than jump around an extended plane.
pub(crate) fn pos_sv(h: f32, r: f32, mut p: egui::Pos2) -> [f32; 2] {
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
            .unwrap();
    }
    let [a, b, _] = barycentric(p, vs);
    let v = (a + b).clamp(0.0, 1.0);
    [if v > 1e-6 { (a / v).clamp(0.0, 1.0) } else { 0.0 }, v]
}
impl WheelCache {
    fn ring(&mut self, r: f32) -> &egui::Mesh {
        if self.ring.as_ref().is_none_or(|(size, _)| *size != r) {
            let mut mesh = egui::Mesh::default();
            const N: u32 = 360;
            for i in 0..=N {
                let h = i as f32 / N as f32;
                let c = hsv_c32(h, 1.0, 1.0);
                mesh.colored_vertex(ring_pos(egui::Pos2::ZERO, r, h), c);
                mesh.colored_vertex(ring_pos(egui::Pos2::ZERO, r - t::PICKER_RING_BAND, h), c);
            }
            for i in 0..N {
                let a = i * 2;
                mesh.add_triangle(a, a + 1, a + 2);
                mesh.add_triangle(a + 1, a + 3, a + 2);
            }
            self.ring = Some((r, mesh));
        }
        &self.ring.as_ref().unwrap().1
    }
    fn triangle(&mut self, h: f32, r: f32) -> &egui::Mesh {
        if self.triangle.as_ref().is_none_or(|(hue, size, _)| *hue != h || *size != r) {
            // Vertex colours are exactly H, white and black. Barycentric interpolation is HSV's
            // v*s hue + v*(1-s) white + (1-v) black; no per-frame raster or tessellation needed.
            let mut mesh = egui::Mesh::default();
            for (p, c) in vertices(h, r).into_iter().zip([hsv_c32(h, 1.0, 1.0), t::PICKER_WHITE, t::PICKER_BLACK]) {
                mesh.colored_vertex(p, c);
            }
            mesh.add_triangle(0, 1, 2);
            self.triangle = Some((h, r, mesh));
        }
        &self.triangle.as_ref().unwrap().2
    }
}
fn translated(mesh: &egui::Mesh, c: egui::Pos2) -> egui::Mesh {
    let mut m = mesh.clone();
    m.translate(c.to_vec2());
    m
}
fn marker(p: &egui::Painter, c: egui::Pos2, r: f32, color: Color32) {
    p.circle_filled(c, r, color);
    p.circle_stroke(c, r + t::PICKER_MARKER_HALO, Stroke::new(t::PICKER_MARKER_HALO, t::PICKER_BLACK));
    p.circle_stroke(c, r, Stroke::new(t::PICKER_MARKER_STROKE, t::PICKER_WHITE));
}
pub(crate) fn show(ui: &mut egui::Ui, m: &mut ColorPanel, s: &Snap, ops: &mut Vec<Op>) {
    let (width, height, ring_r, triangle_r, center) = if m.mini() {
        (
            t::PICKER_MINI_W,
            t::PICKER_MINI_BODY_H,
            t::PICKER_MINI_RING_R,
            t::PICKER_MINI_TRIANGLE_R,
            t::PICKER_MINI_CENTER,
        )
    } else {
        (t::PICKER_W, t::PICKER_BODY_H, t::PICKER_RING_R, t::PICKER_TRIANGLE_R, t::PICKER_RING_CENTER)
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    #[cfg(test)]
    super::super::fields::tests::probe("picker wheel", rect);
    let c = rect.min + egui::vec2(center[0], center[1]);
    let hit = ui.interact(
        egui::Rect::from_center_size(c, egui::Vec2::splat(ring_r * 2.0)),
        ui.id().with("wheel"),
        if kit::field::blocked(ui.ctx()) { egui::Sense::hover() } else { egui::Sense::click_and_drag() },
    );
    let pos = hit.interact_pointer_pos();
    if (hit.is_pointer_button_down_on() || hit.clicked()) && m.gesture.is_none() {
        if let Some(p) = pos {
            if ring_hit(c, p, ring_r, t::PICKER_RING_BAND) {
                m.start(Gesture::Ring, ops);
            } else if barycentric(egui::Pos2::ZERO + (p - c), vertices(m.hsva[0], triangle_r)).iter().all(|w| *w >= 0.0)
            {
                m.start(Gesture::Triangle, ops);
            }
        }
    }
    if hit.is_pointer_button_down_on() || hit.clicked() {
        if let Some(p) = pos {
            match m.gesture {
                Some(Gesture::Ring) => {
                    m.channel_state = None;
                    m.hsva[0] = ring_hue(c, p);
                    m.change_requested = true;
                }
                Some(Gesture::Triangle) => {
                    m.channel_state = None;
                    let [s, v] = pos_sv(m.hsva[0], triangle_r, egui::Pos2::ZERO + (p - c));
                    m.hsva[1] = s;
                    m.hsva[2] = v;
                    m.change_requested = true;
                }
                _ => {}
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(translated(m.cache.ring(ring_r), c)));
    ui.painter().add(egui::Shape::mesh(translated(m.cache.triangle(m.hsva[0], triangle_r), c)));
    if m.tab == Tab::Harmony && !m.mini() {
        for [h, s, v] in harmony_rules::linked(m.harmony, [m.hsva[0], m.hsva[1], m.hsva[2]]).into_iter().skip(1) {
            let pos = if m.harmony == varos_app::storage::layout::HarmonyRule::Mono {
                c + sv_pos(h, triangle_r, s, v).to_vec2()
            } else {
                ring_pos(c, ring_r - t::PICKER_RING_BAND / 2.0, h)
            };
            #[cfg(test)]
            super::super::fields::tests::probe(
                "harmony marker",
                egui::Rect::from_center_size(pos, egui::Vec2::splat(t::PICKER_TRI_MARKER)),
            );
            marker(ui.painter(), pos, t::PICKER_TRI_MARKER, hsv_c32(h, s, v));
        }
    }
    let color = hsv_c32(m.hsva[0], m.hsva[1], m.hsva[2]);
    marker(
        ui.painter(),
        ring_pos(c, ring_r - t::PICKER_RING_BAND / 2.0, m.hsva[0]),
        t::PICKER_RING_MARKER,
        hsv_c32(m.hsva[0], 1.0, 1.0),
    );
    marker(
        ui.painter(),
        c + sv_pos(m.hsva[0], triangle_r, m.hsva[1], m.hsva[2]).to_vec2(),
        t::PICKER_TRI_MARKER,
        color,
    );
    hit.on_hover_text("Hue ring / saturation and brightness triangle");
    if m.mini() {
        return;
    }
    let default = egui::Rect::from_min_size(
        rect.min + egui::vec2(t::PICKER_DEFAULT_POS[0], t::PICKER_DEFAULT_POS[1]),
        egui::vec2(t::PICKER_DEFAULT_SIZE * 2.0, t::PICKER_DEFAULT_SIZE),
    );
    for (i, col) in
        [varos_core::editor::DEFAULT_FILL, varos_core::editor::DEFAULT_STROKE].map(rgba_c32a).into_iter().enumerate()
    {
        let r = egui::Rect::from_min_size(
            default.min + egui::vec2(i as f32 * t::PICKER_DEFAULT_SIZE, 0.0),
            egui::Vec2::splat(t::PICKER_DEFAULT_SIZE),
        );
        ui.painter().rect(r, CornerRadius::ZERO, col, Stroke::new(t::KIT_STROKE, t::MUTED), StrokeKind::Inside);
    }
    if ui
        .interact(default, ui.id().with("default"), egui::Sense::click())
        .on_hover_text("Default colours (D)")
        .clicked()
    {
        ops.push(Op::DefaultPaint);
    }
    for (i, (label, value)) in modes::readout(m.mode, m.color(), m.channel_values()).into_iter().enumerate() {
        let pos = rect.min
            + egui::vec2(
                t::PICKER_READOUT_POS[0],
                t::PICKER_READOUT_POS[1] - if m.mode == modes::Mode::Cmyk { t::PICKER_READOUT_LINE } else { 0.0 }
                    + i as f32 * t::PICKER_READOUT_LINE,
            );
        let offset = if label.is_empty() { 0.0 } else { t::FIELD_LABEL_W };
        if !label.is_empty() {
            ui.painter().text(pos, Align2::LEFT_TOP, format!("{label}:"), t::mono(), t::MUTED);
        }
        ui.painter().text(pos + egui::vec2(offset, 0.0), Align2::LEFT_TOP, value, t::mono(), t::TEXT);
    }
    cluster::show(ui, rect.min, m, s, ops);
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn cached_meshes_rebuild_only_on_size_or_hue_change() {
        let mut cache = WheelCache::default();
        let ring = cache.ring(t::PICKER_RING_R).vertices.as_ptr();
        let triangle = cache.triangle(0.25, t::PICKER_TRIANGLE_R).vertices.as_ptr();
        for _ in 0..32 {
            assert_eq!(cache.ring(t::PICKER_RING_R).vertices.as_ptr(), ring);
            assert_eq!(cache.triangle(0.25, t::PICKER_TRIANGLE_R).vertices.as_ptr(), triangle);
        }
        cache.triangle(0.5, t::PICKER_TRIANGLE_R);
        assert_eq!(cache.triangle.as_ref().unwrap().0, 0.5);
        assert_eq!(cache.ring(t::PICKER_RING_R).vertices.as_ptr(), ring);
        cache.ring(t::PICKER_RING_R - 1.0);
        assert_eq!(cache.ring.as_ref().unwrap().0, t::PICKER_RING_R - 1.0);
        let start = std::time::Instant::now();
        for i in 0..1000 {
            cache.triangle(i as f32 / 1000.0, t::PICKER_TRIANGLE_R);
        }
        let mean = start.elapsed() / 1000;
        assert!(mean < std::time::Duration::from_millis(1));
        println!("picker triangle rebuild mean over 1000 hues: {mean:?}");
    }
}
