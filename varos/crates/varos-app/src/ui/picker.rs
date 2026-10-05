use super::*;

/// Where the modal's colour lands on OK.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MTarget {
    Paint(PaintTarget),
    Ab(usize),
}

/// The spectrum-slider channel (the Photoshop/Illustrator radio mechanic): the selected channel becomes
/// the vertical slider, and the big field shows the remaining two axes.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Chan {
    H,
    S,
    B,
    R,
    G,
    Bl,
}

/// Which geometry the modal shows: the field-plane Picker or the harmony Wheel.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MTab {
    Picker,
    Wheel,
}

/// Colour-wheel harmony rule. Hue-rotation sets keep S,V; Mono varies brightness. (htmlcolorcodes/
/// colordesigner math.)
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Harmony {
    None,
    Complementary,
    Analogous,
    Split,
    Triadic,
    Tetradic,
    Square,
    Mono,
}
impl Harmony {
    const ALL: [(Harmony, &'static str); 8] = [
        (Harmony::None, "None"),
        (Harmony::Complementary, "Comp"),
        (Harmony::Analogous, "Analog"),
        (Harmony::Split, "Split"),
        (Harmony::Triadic, "Triad"),
        (Harmony::Tetradic, "Tetra"),
        (Harmony::Square, "Square"),
        (Harmony::Mono, "Mono"),
    ];
    /// Hue offsets (degrees) for the non-base members — empty for None/Mono.
    fn offsets(self) -> &'static [f32] {
        match self {
            Harmony::Complementary => &[180.0],
            Harmony::Analogous => &[-30.0, 30.0],
            Harmony::Split => &[150.0, 210.0],
            Harmony::Triadic => &[120.0, 240.0],
            Harmony::Tetradic => &[60.0, 180.0, 240.0],
            Harmony::Square => &[90.0, 180.0, 270.0],
            _ => &[],
        }
    }
}

/// Every colour in a harmony set (base first). Hue modes rotate hue at fixed S,V; Mono steps brightness.
pub(crate) fn harmony_set(h: Harmony, base: [f32; 3]) -> Vec<[f32; 3]> {
    match h {
        Harmony::None => vec![base],
        Harmony::Mono => {
            [1.0, 0.78, 0.56, 0.36].iter().map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)]).collect()
        }
        _ => {
            let mut v = vec![base];
            for off in h.offsets() {
                v.push([(base[0] + off / 360.0).rem_euclid(1.0), base[1], base[2]]);
            }
            v
        }
    }
}

/// The professional Color Picker modal (opened by double-clicking any colour swatch).
/// Live HSVA is the single source of truth while open; OK commits once, Cancel discards. The whole
/// interaction is ONE undo step (A6): `EditCommand::PickerBegin` on open, live paint each frame, and a
/// single `picker_commit` / `picker_cancel` on close.
pub(crate) struct ColorModal {
    pub(crate) target: MTarget,
    pub(crate) orig: Option<Rgba>,
    pub(crate) hsva: [f32; 4],
    pub(crate) chan: Chan,
    pub(crate) tab: MTab,
    pub(crate) harmony: Harmony,
    // A5 — system eyedropper: armed while sampling a pixel from anywhere on screen.
    pub(crate) eyedropping: bool,
    pub(crate) eyedrop_prev_down: bool, // previous global-LMB state (edge-detect the commit click)
    pub(crate) eyedrop_return: [f32; 4], // hsva to restore if the eyedrop is aborted (Esc)
}

pub(crate) fn hex_of(c: Rgba) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8
    )
}

// ── HSV ↔ RGB (all channels 0..1) — the picker keeps live HSV as its source of truth (Decision 2) ──
pub(crate) fn rgb_to_hsv(c: Rgba) -> [f32; 3] {
    let (r, g, b) = (c[0], c[1], c[2]);
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
    let c = hsv_to_rgb(h, s, v);
    Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
}
pub(crate) fn rgba_c32a(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}
pub(crate) fn parse_hex(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_start_matches('#');
    let n = |a: &str| u8::from_str_radix(a, 16).ok().map(|v| v as f32 / 255.0);
    match s.len() {
        3 => {
            let e: Vec<String> = s.chars().map(|c| format!("{c}{c}")).collect();
            Some([n(&e[0])?, n(&e[1])?, n(&e[2])?, 1.0])
        }
        6 => Some([n(&s[0..2])?, n(&s[2..4])?, n(&s[4..6])?, 1.0]),
        8 => Some([n(&s[0..2])?, n(&s[2..4])?, n(&s[4..6])?, n(&s[6..8])?]),
        _ => None,
    }
}
/// Grey checkerboard behind translucent colours.
pub(crate) fn checker(p: &egui::Painter, r: egui::Rect, sq: f32) {
    p.rect_filled(r, CornerRadius::ZERO, Color32::from_gray(90));
    let (cols, rows) = ((r.width() / sq).ceil() as i32, (r.height() / sq).ceil() as i32);
    for gy in 0..rows {
        for gx in 0..cols {
            if (gx + gy) % 2 == 0 {
                continue;
            }
            let (x, y) = (r.left() + gx as f32 * sq, r.top() + gy as f32 * sq);
            let cell = egui::Rect::from_min_max(
                egui::pos2(x, y),
                egui::pos2((x + sq).min(r.right()), (y + sq).min(r.bottom())),
            );
            p.rect_filled(cell, CornerRadius::ZERO, Color32::from_gray(140));
        }
    }
}
pub(crate) fn rail_thumb(p: &egui::Painter, r: egui::Rect, y: f32) {
    let t = egui::Rect::from_min_max(egui::pos2(r.left() - 2.0, y - 2.5), egui::pos2(r.right() + 2.0, y + 2.5));
    p.rect(t, CornerRadius::same(2), Color32::TRANSPARENT, Stroke::new(2.0, Color32::WHITE), StrokeKind::Middle);
    p.rect_stroke(
        t.expand(1.0),
        CornerRadius::same(3),
        Stroke::new(1.0, Color32::from_black_alpha(90)),
        StrokeKind::Middle,
    );
}

/// A labelled row of small clickable swatches (hand-painted; checker under translucent colours).
/// Returns the clicked colour. Hidden entirely when the list is empty.
pub(crate) fn swatch_strip(ui: &mut egui::Ui, label: &str, colors: &[Rgba]) -> Option<Rgba> {
    if colors.is_empty() {
        return None;
    }
    let mut out = None;
    ui.add_space(2.0);
    ui.label(RichText::new(label).color(MUTED).size(10.5));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for c in colors {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::click());
            let round = CornerRadius::same(3);
            if c[3] < 0.999 {
                checker(&ui.painter_at(r), r, 4.0);
            }
            ui.painter().rect_filled(r, round, rgba_c32a(*c));
            ui.painter().rect_stroke(
                r,
                round,
                Stroke::new(1.0, if resp.hovered() { Color32::WHITE } else { BORDER_2 }),
                StrokeKind::Middle,
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                out = Some(*c);
            }
        }
    });
    out
}

/// Fill / Stroke row: a hand-painted swatch (double-click → the Color Picker modal), + hex + clear ×.
pub(crate) fn paint_row(ui: &mut egui::Ui, target: PaintTarget, color: Option<Rgba>, ops: &mut Vec<Op>) {
    ui.horizontal(|ui| {
        // A18: name the target so the two rows read as Fill / Stroke at a glance (fixed column → swatches align)
        let label = match target {
            PaintTarget::Fill => "Fill",
            PaintTarget::Stroke => "Stroke",
        };
        let (lr, _) = ui.allocate_exact_size(egui::vec2(44.0, 18.0), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(lr.left(), lr.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.5),
            MUTED,
        );
        let (sw, resp) = ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::click());
        let round = CornerRadius::same(4);
        let p = ui.painter();
        match color {
            Some(c) => {
                if c[3] < 0.999 {
                    checker(&ui.painter_at(sw), sw, 5.0);
                }
                p.rect_filled(sw, round, rgba_c32a(c));
            }
            None => {
                p.rect_filled(sw, round, SWATCH_WELL);
                p.line_segment(
                    [sw.left_bottom() + egui::vec2(2.0, -2.0), sw.right_top() + egui::vec2(-2.0, 2.0)],
                    Stroke::new(1.6, NONE_RED),
                );
            } // None = red slash
        }
        p.rect_stroke(sw, round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        // single click = focus the target (X toggles) · DOUBLE-click = open the Color Picker modal
        if resp.clicked() {
            ops.push(Op::PaintFocus(target));
        }
        if resp.double_clicked() {
            ops.push(Op::OpenPicker(MTarget::Paint(target)));
        }
        resp.on_hover_text("Double-click to edit the colour");
        ui.add_space(8.0);
        ui.label(RichText::new(color.map(hex_of).unwrap_or_else(|| "None".into())).color(TEXT).monospace().size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let clear = match target {
                PaintTarget::Fill => IA_NO_FILL,
                PaintTarget::Stroke => IA_NO_STROKE,
            };
            if clear.show(ui, kit::IconState::Action) {
                ops.push(Op::Paint(target, None));
            }
        });
    });
}

// ───────────────────────────── the Color Picker modal ─────────────────────────────

/// Field-plane / spectrum-slider axis positions (all 0..1) for the current colour under a channel radio.
pub(crate) fn pick_get(chan: Chan, hsva: [f32; 4]) -> (f32, f32, f32) {
    let c = hsv_to_rgb(hsva[0], hsva[1], hsva[2]);
    match chan {
        Chan::H => (hsva[1], hsva[2], hsva[0]),
        Chan::S => (hsva[0], hsva[2], hsva[1]),
        Chan::B => (hsva[0], hsva[1], hsva[2]),
        Chan::R => (c[2], c[1], c[0]), // field x=Blue · y=Green · slider=Red (Photoshop convention)
        Chan::G => (c[2], c[0], c[1]), // x=Blue · y=Red
        Chan::Bl => (c[0], c[1], c[2]), // x=Red  · y=Green
    }
}
/// Write plane/slider positions back into the live HSVA (grey RGB results keep the current hue).
pub(crate) fn pick_set(chan: Chan, hsva: &mut [f32; 4], px: f32, py: f32, sl: f32) {
    match chan {
        Chan::H => {
            hsva[0] = sl.min(0.9999);
            hsva[1] = px;
            hsva[2] = py;
        }
        Chan::S => {
            hsva[0] = px.min(0.9999);
            hsva[1] = sl;
            hsva[2] = py;
        }
        Chan::B => {
            hsva[0] = px.min(0.9999);
            hsva[1] = py;
            hsva[2] = sl;
        }
        _ => {
            let rgb = match chan {
                Chan::R => [sl, py, px],
                Chan::G => [py, sl, px],
                _ => [px, py, sl],
            };
            let h = rgb_to_hsv([rgb[0], rgb[1], rgb[2], 1.0]);
            if h[1] > 0.001 {
                hsva[0] = h[0];
            }
            hsva[1] = h[1];
            hsva[2] = h[2];
        }
    }
}
/// Colour of a field/slider sample point (px, py, sl each 0..1) under a channel radio.
pub(crate) fn pick_rgb(chan: Chan, px: f32, py: f32, sl: f32) -> [f32; 3] {
    match chan {
        Chan::H => hsv_to_rgb(sl, px, py),
        Chan::S => hsv_to_rgb(px, sl, py),
        Chan::B => hsv_to_rgb(px, py, sl),
        Chan::R => [sl, py, px],
        Chan::G => [py, sl, px],
        Chan::Bl => [px, py, sl],
    }
}
pub(crate) fn rgb_c32(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
}

/// A hand-painted radio dot (the channel selectors). Returns true on click.
pub(crate) fn radio_dot(ui: &mut egui::Ui, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(15.0, 25.0), egui::Sense::click());
    let c = r.center();
    ui.painter().circle_stroke(
        c,
        5.0,
        Stroke::new(
            1.2,
            if on {
                ACCENT
            } else if resp.hovered() {
                TEXT
            } else {
                MUTED
            },
        ),
    );
    if on {
        ui.painter().circle_filled(c, 2.6, ACCENT);
    }
    resp.clicked()
}

/// Screen position of an H×S point on a wheel of radius `rad` centred at `c` (red at 12 o'clock,
/// hue increasing clockwise; saturation = radius).
pub(crate) fn wheel_pos(c: egui::Pos2, rad: f32, h: f32, s: f32) -> egui::Pos2 {
    let a = h * std::f32::consts::TAU;
    egui::pos2(c.x + a.sin() * s * rad, c.y - a.cos() * s * rad)
}

/// The Wheel view: an H×S disc (brightness-tinted) + brightness rail + alpha rail + harmony rule pills +
/// clickable result chips. Draggable base handle; ghost handles for the linked harmony members. Edits the
/// same live `hsva`. Returns nothing — mutates the modal in place.
pub(crate) fn build_wheel(ui: &mut egui::Ui, m: &mut ColorModal) {
    let v = m.hsva[2];
    ui.horizontal_top(|ui| {
        // ── the H×S disc (a fan mesh from a grey centre to full-sat rim; radial interp is EXACT for HSV) ──
        // 240 (not 236) so the Wheel colour area matches the Picker plane exactly — no ~4px tab-switch jump (A17 P1).
        let d = 240.0;
        let (dr, dresp) = ui.allocate_exact_size(egui::vec2(d, d), egui::Sense::click_and_drag());
        let c = dr.center();
        let rad = d * 0.5 - 1.0;
        let n = 96u32;
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(c, hsv_c32(0.0, 0.0, v)); // centre = grey at the current brightness
        for i in 0..=n {
            let h = i as f32 / n as f32;
            mesh.colored_vertex(wheel_pos(c, rad, h, 1.0), hsv_c32(h, 1.0, v));
        }
        for i in 1..=n {
            mesh.add_triangle(0, i, i + 1);
        }
        ui.painter_at(dr).add(egui::Shape::mesh(mesh));
        ui.painter().circle_stroke(c, rad + 0.5, Stroke::new(1.0, BORDER_2));
        // harmony ghost handles (linked, display-only) then the draggable base handle on top
        let set = harmony_set(m.harmony, [m.hsva[0], m.hsva[1], m.hsva[2]]);
        for gc in set.iter().skip(1) {
            let p = wheel_pos(c, rad, gc[0], gc[1]);
            ui.painter().circle_filled(p, 4.5, rgb_c32(hsv_to_rgb(gc[0], gc[1], gc[2])));
            ui.painter().circle_stroke(p, 4.5, Stroke::new(1.5, Color32::from_white_alpha(200)));
        }
        let bp = wheel_pos(c, rad, m.hsva[0], m.hsva[1]);
        ui.painter().circle_stroke(bp, 6.5, Stroke::new(2.0, Color32::WHITE));
        ui.painter().circle_stroke(bp, 7.5, Stroke::new(1.0, Color32::from_black_alpha(120)));
        if dresp.is_pointer_button_down_on() || dresp.dragged() {
            if let Some(p) = dresp.interact_pointer_pos() {
                let (dx, dy) = (p.x - c.x, p.y - c.y);
                m.hsva[0] = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
                m.hsva[1] = ((dx * dx + dy * dy).sqrt() / rad).clamp(0.0, 1.0);
                if m.hsva[0] >= 1.0 {
                    m.hsva[0] = 0.9999;
                }
            }
        }
        // ── brightness rail (black → full colour) ──
        let (br, brr) = ui.allocate_exact_size(egui::vec2(16.0, d), egui::Sense::click_and_drag());
        let mut bm = egui::Mesh::default();
        let top = hsv_c32(m.hsva[0], m.hsva[1], 1.0);
        bm.colored_vertex(br.left_top(), top);
        bm.colored_vertex(br.right_top(), top);
        bm.colored_vertex(br.right_bottom(), Color32::BLACK);
        bm.colored_vertex(br.left_bottom(), Color32::BLACK);
        bm.add_triangle(0, 1, 2);
        bm.add_triangle(0, 2, 3);
        ui.painter_at(br).add(egui::Shape::mesh(bm));
        ui.painter().rect_stroke(br, CornerRadius::ZERO, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        rail_thumb(ui.painter(), br, br.top() + (1.0 - v) * d);
        if brr.is_pointer_button_down_on() || brr.dragged() {
            if let Some(p) = brr.interact_pointer_pos() {
                m.hsva[2] = (1.0 - (p.y - br.top()) / d).clamp(0.0, 1.0);
            }
        }
        // ── alpha rail ──
        let (ar, arr) = ui.allocate_exact_size(egui::vec2(16.0, d), egui::Sense::click_and_drag());
        checker(&ui.painter_at(ar), ar, 6.0);
        let solid = hsv_c32(m.hsva[0], m.hsva[1], m.hsva[2]);
        let mut am = egui::Mesh::default();
        am.colored_vertex(ar.left_top(), solid);
        am.colored_vertex(ar.right_top(), solid);
        am.colored_vertex(ar.right_bottom(), Color32::TRANSPARENT);
        am.colored_vertex(ar.left_bottom(), Color32::TRANSPARENT);
        am.add_triangle(0, 1, 2);
        am.add_triangle(0, 2, 3);
        ui.painter_at(ar).add(egui::Shape::mesh(am));
        ui.painter().rect_stroke(ar, CornerRadius::ZERO, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        rail_thumb(ui.painter(), ar, ar.top() + (1.0 - m.hsva[3]) * d);
        if arr.is_pointer_button_down_on() || arr.dragged() {
            if let Some(p) = arr.interact_pointer_pos() {
                m.hsva[3] = (1.0 - (p.y - ar.top()) / d).clamp(0.0, 1.0);
            }
        }
    });
    ui.add_space(4.0);
    // ── harmony rule pills ──
    ui.label(RichText::new("HARMONY").color(MUTED).size(10.5));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for (rule, label) in Harmony::ALL {
            let on = m.harmony == rule;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(52.0, 22.0), egui::Sense::click());
            let rr = CornerRadius::same(R);
            if on {
                ui.painter().rect_filled(r, rr, ACCENT);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, rr, HOVER);
            } else {
                ui.painter().rect_stroke(r, rr, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
            }
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                if on { Color32::WHITE } else { TEXT },
            );
            if resp.clicked() {
                m.harmony = rule;
            }
        }
    });
    // ── harmony result chips (click to adopt as the current colour) ──
    if m.harmony != Harmony::None {
        ui.add_space(3.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 4.0);
            for gc in harmony_set(m.harmony, [m.hsva[0], m.hsva[1], m.hsva[2]]) {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(30.0, 22.0), egui::Sense::click());
                // radius R (not the stray r=4) so every control in the dialog shares one radius (A17 P4)
                ui.painter().rect_filled(r, CornerRadius::same(R), rgb_c32(hsv_to_rgb(gc[0], gc[1], gc[2])));
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(R),
                    Stroke::new(1.0, if resp.hovered() { Color32::WHITE } else { BORDER_2 }),
                    StrokeKind::Middle,
                );
                let rgb = hsv_to_rgb(gc[0], gc[1], gc[2]);
                resp.clone().on_hover_text(hex_of([rgb[0], rgb[1], rgb[2], 1.0]));
                if resp.clicked() {
                    m.hsva[0] = gc[0];
                    m.hsva[1] = gc[1];
                    m.hsva[2] = gc[2];
                }
            }
        });
    }
}

/// A hand-painted dialog button. `primary` = accent OK.
pub(crate) fn dlg_btn(ui: &mut egui::Ui, label: &str, primary: bool, w: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    let rr = CornerRadius::same(R);
    if primary {
        ui.painter().rect_filled(r, rr, if resp.hovered() { ACCENT_HOVER } else { ACCENT });
    } else {
        ui.painter().rect_filled(r, rr, if resp.hovered() { HOVER } else { BG_SURFACE });
        ui.painter().rect_stroke(r, rr, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    }
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.5),
        if primary { Color32::WHITE } else { TEXT },
    );
    resp.clicked()
}

/// Adopt an RGBA colour into the modal's live HSVA. Greys (no chroma) keep the current hue so the
/// wheel/plane handle doesn't snap to red. Shared by the RECENT/DOCUMENT strips, the current-half
/// restore, and the eyedropper.
pub(crate) fn modal_adopt(m: &mut ColorModal, c: Rgba) {
    let h = rgb_to_hsv(c);
    if h[1] > 0.001 {
        m.hsva[0] = h[0];
    }
    m.hsva[1] = h[1];
    m.hsva[2] = h[2];
    m.hsva[3] = c[3];
}

/// The picker header's eyedropper button — the REAL Lucide pipette texture (A16.2), the same
/// glyph as the Eyedropper tool, so the app shows ONE pipette everywhere. Accent while armed (A5).
pub(crate) fn eyedropper_btn(ui: &mut egui::Ui, pipette: &Option<egui::TextureHandle>, armed: bool) -> bool {
    if !crate::cursors::SCREEN_EYEDROPPER {
        // No screen sampling on this platform yet (MAC_SHELL_PORT.md): show the tool, dimmed and
        // inert, instead of arming a pick that could never land.
        let (r, resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::hover());
        if let Some(t) = pipette {
            ui.painter().image(
                t.id(),
                egui::Rect::from_center_size(r.center(), egui::Vec2::splat(ICON_SM)),
                UV01(),
                MUTED.gamma_multiply(0.4),
            );
        }
        resp.on_hover_text("Screen eyedropper is Windows-only for now");
        return false;
    }
    let (r, resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
    let rr = CornerRadius::same(R);
    if armed {
        ui.painter().rect_filled(r, rr, ACCENT);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, rr, HOVER);
    }
    if let Some(t) = pipette {
        ui.painter().image(
            t.id(),
            egui::Rect::from_center_size(r.center(), egui::Vec2::splat(ICON_SM)),
            UV01(),
            if armed { Color32::WHITE } else { MUTED },
        );
    }
    resp.on_hover_text("Eyedropper \u{2014} sample a colour from anywhere on screen").clicked()
}

/// A19 — the Fill / Stroke (or Page-colour) target indicator + switch. For a paint target it renders a
/// two-segment toggle: the ACTIVE segment carries the accent (the selected-state), and clicking the
/// other one switches which paint the picker edits (reseeded from that target's current colour). For an
/// artboard target it's a plain "Page Color" label. Mutates `m.target`/`m.orig`/`hsva` on a switch.
pub(crate) fn target_indicator(ui: &mut egui::Ui, m: &mut ColorModal, snap: &Snap) {
    match m.target {
        MTarget::Paint(active) => {
            for (t, label) in [(PaintTarget::Fill, "Fill"), (PaintTarget::Stroke, "Stroke")] {
                let on = active == t;
                let (r, resp) = ui.allocate_exact_size(egui::vec2(52.0, 22.0), egui::Sense::click());
                let rr = CornerRadius::same(R);
                if on {
                    ui.painter().rect_filled(r, rr, ACCENT);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, rr, HOVER);
                } else {
                    ui.painter().rect_stroke(r, rr, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
                }
                // a colour dot on the active segment so it reads as "this is the swatch you're editing"
                let tx = if on {
                    let dot = egui::pos2(r.left() + 11.0, r.center().y);
                    match snap_target_color(snap, t) {
                        Some(cc) => {
                            ui.painter().circle_filled(dot, 4.0, rgba_c32a(cc));
                        }
                        None => {
                            ui.painter().circle_stroke(dot, 4.0, Stroke::new(1.0, Color32::WHITE));
                            ui.painter().line_segment(
                                [dot + egui::vec2(-2.8, 2.8), dot + egui::vec2(2.8, -2.8)],
                                Stroke::new(1.2, NONE_RED),
                            );
                        }
                    }
                    r.center().x + 6.0
                } else {
                    r.center().x
                };
                ui.painter().text(
                    egui::pos2(tx, r.center().y),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(11.5),
                    if on { Color32::WHITE } else { MUTED },
                );
                if resp.clicked() && !on {
                    // switch the target within the SAME undo session; reseed from its current colour
                    m.target = MTarget::Paint(t);
                    let seed = snap_target_color(snap, t);
                    m.orig = seed;
                    if let Some(c) = seed {
                        modal_adopt(m, c);
                    }
                }
            }
        }
        MTarget::Ab(_) => {
            let (r, _) = ui.allocate_exact_size(egui::vec2(96.0, 22.0), egui::Sense::hover());
            ui.painter().rect_stroke(r, CornerRadius::same(R), Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "Page Color", FontId::proportional(11.5), TEXT);
        }
    }
}

/// The current colour of a paint target from this frame's snapshot (Fill / Stroke).
pub(crate) fn snap_target_color(snap: &Snap, t: PaintTarget) -> Option<Rgba> {
    match t {
        PaintTarget::Fill => snap.fill,
        PaintTarget::Stroke => snap.stroke,
    }
}

/// The professional Color Picker dialog — a FLOATING palette (Ahmed, 07-02: no scrim, the canvas stays
/// fully usable beside it; drag any empty spot to move it, and it remembers its position). The field
/// plane + spectrum slider are channel-radio driven (the Photoshop/Illustrator mechanic); alpha rail;
/// split new/current preview (click the current half to restore); hex (Enter/blur) + A% + HSB/RGB fields;
/// RECENT/DOCUMENT strips. A6: the target updates LIVE as you drag, the whole interaction is ONE undo
/// step (OK commits it, Cancel/Esc reverts to the value at open). A19: a Fill/Stroke indicator + switch.
/// A17: an in-picker eyedropper that samples anywhere on screen (A5). Enter = OK when no field focused.
pub(crate) fn build_color_modal(
    ctx: &egui::Context,
    modal: &mut Option<ColorModal>,
    snap: &Snap,
    pipette: &Option<egui::TextureHandle>,
    ops: &mut Vec<Op>,
) {
    if modal.is_none() {
        return;
    }
    let screen = ctx.content_rect();
    let (mut ok, mut cancel) = (false, false);
    {
        let m = modal.as_mut().unwrap();
        let dw = 508.0;
        let pos = egui::pos2((screen.center().x - dw * 0.5 - 14.0).round(), 84.0);
        egui::Area::new(egui::Id::new("cm-dialog"))
            .order(egui::Order::Foreground)
            .movable(true)
            .default_pos(pos)
            .constrain(true)
            .show(ctx, |ui| {
                panel_frame(14).show(ui, |ui| {
                    ui.set_width(dw);
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    // ── header: A19 Fill/Stroke (or Page) target indicator · Picker|Wheel tabs ·
                    //    A17 eyedropper · close. The target indicator IS the title — it names, and lets
                    //    you switch, what the picker edits. ──
                    ui.horizontal(|ui| {
                        target_indicator(ui, m, snap);
                        ui.add_space(12.0);
                        for (tab, label) in [(MTab::Picker, "Picker"), (MTab::Wheel, "Wheel")] {
                            let on = m.tab == tab;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(54.0, 22.0), egui::Sense::click());
                            let rr = CornerRadius::same(R);
                            // ONE active language across the dialog: accent FILL + white text (like Fill/Stroke
                            // & the harmony pills), not the old accent-outline variant (A17 P3).
                            if on {
                                ui.painter().rect_filled(r, rr, ACCENT);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, rr, HOVER);
                            }
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                label,
                                FontId::proportional(12.0),
                                if on { Color32::WHITE } else { MUTED },
                            );
                            if resp.clicked() {
                                m.tab = tab;
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if IA_PICKER_CLOSE.show(ui, kit::IconState::Action) {
                                cancel = true;
                            }
                            if eyedropper_btn(ui, pipette, m.eyedropping) {
                                // arm the system eyedropper; the initiating click is still down, so
                                // remember that and commit on the NEXT fresh global press (A5).
                                m.eyedropping = true;
                                m.eyedrop_prev_down = true;
                                m.eyedrop_return = m.hsva;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    let (px, py, sl) = pick_get(m.chan, m.hsva);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            if m.tab == MTab::Wheel {
                                build_wheel(ui, m);
                            } else {
                                ui.horizontal_top(|ui| {
                                    // ── the field plane (axes follow the channel radio) ──
                                    let (pw, ph) = (240.0, 240.0);
                                    let (pr, presp) =
                                        ui.allocate_exact_size(egui::vec2(pw, ph), egui::Sense::click_and_drag());
                                    let (nx, ny) = (24usize, 16usize);
                                    let mut mesh = egui::Mesh::default();
                                    for gy in 0..=ny {
                                        for gx in 0..=nx {
                                            let fx = gx as f32 / nx as f32;
                                            let fy = gy as f32 / ny as f32;
                                            mesh.colored_vertex(
                                                egui::pos2(pr.left() + fx * pw, pr.top() + fy * ph),
                                                rgb_c32(pick_rgb(m.chan, fx, 1.0 - fy, sl)),
                                            );
                                        }
                                    }
                                    for gy in 0..ny as u32 {
                                        for gx in 0..nx as u32 {
                                            let w1 = nx as u32 + 1;
                                            let i = gy * w1 + gx;
                                            mesh.add_triangle(i, i + 1, i + w1 + 1);
                                            mesh.add_triangle(i, i + w1 + 1, i + w1);
                                        }
                                    }
                                    ui.painter_at(pr).add(egui::Shape::mesh(mesh));
                                    ui.painter().rect_stroke(
                                        pr,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0, BORDER_2),
                                        StrokeKind::Middle,
                                    );
                                    let mp = egui::pos2(pr.left() + px * pw, pr.top() + (1.0 - py) * ph);
                                    ui.painter().circle_stroke(mp, 6.0, Stroke::new(2.0, Color32::WHITE));
                                    ui.painter().circle_stroke(
                                        mp,
                                        7.0,
                                        Stroke::new(1.0, Color32::from_black_alpha(110)),
                                    );
                                    if presp.is_pointer_button_down_on() || presp.dragged() {
                                        if let Some(p) = presp.interact_pointer_pos() {
                                            pick_set(
                                                m.chan,
                                                &mut m.hsva,
                                                ((p.x - pr.left()) / pw).clamp(0.0, 1.0),
                                                (1.0 - (p.y - pr.top()) / ph).clamp(0.0, 1.0),
                                                sl,
                                            );
                                        }
                                    }
                                    // ── the channel spectrum slider (contextual gradient) ──
                                    let (sr, sresp) =
                                        ui.allocate_exact_size(egui::vec2(16.0, ph), egui::Sense::click_and_drag());
                                    let stops = 32;
                                    let mut sm = egui::Mesh::default();
                                    for i in 0..=stops {
                                        let t = i as f32 / stops as f32;
                                        let sv = if m.chan == Chan::H { t } else { 1.0 - t };
                                        let cc = rgb_c32(pick_rgb(m.chan, px, py, sv));
                                        let y = sr.top() + t * ph;
                                        sm.colored_vertex(egui::pos2(sr.left(), y), cc);
                                        sm.colored_vertex(egui::pos2(sr.right(), y), cc);
                                    }
                                    for i in 0..stops as u32 {
                                        let a = i * 2;
                                        sm.add_triangle(a, a + 1, a + 3);
                                        sm.add_triangle(a, a + 3, a + 2);
                                    }
                                    ui.painter_at(sr).add(egui::Shape::mesh(sm));
                                    ui.painter().rect_stroke(
                                        sr,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0, BORDER_2),
                                        StrokeKind::Middle,
                                    );
                                    rail_thumb(
                                        ui.painter(),
                                        sr,
                                        sr.top() + (if m.chan == Chan::H { sl } else { 1.0 - sl }) * ph,
                                    );
                                    if sresp.is_pointer_button_down_on() || sresp.dragged() {
                                        if let Some(p) = sresp.interact_pointer_pos() {
                                            let t = ((p.y - sr.top()) / ph).clamp(0.0, 1.0);
                                            let nsl = if m.chan == Chan::H { t.min(0.9999) } else { 1.0 - t };
                                            pick_set(m.chan, &mut m.hsva, px, py, nsl);
                                        }
                                    }
                                    // ── alpha rail ──
                                    let (ar, arr) =
                                        ui.allocate_exact_size(egui::vec2(16.0, ph), egui::Sense::click_and_drag());
                                    checker(&ui.painter_at(ar), ar, 6.0);
                                    let solid = hsv_c32(m.hsva[0], m.hsva[1], m.hsva[2]);
                                    let mut am = egui::Mesh::default();
                                    am.colored_vertex(ar.left_top(), solid);
                                    am.colored_vertex(ar.right_top(), solid);
                                    am.colored_vertex(ar.right_bottom(), Color32::TRANSPARENT);
                                    am.colored_vertex(ar.left_bottom(), Color32::TRANSPARENT);
                                    am.add_triangle(0, 1, 2);
                                    am.add_triangle(0, 2, 3);
                                    ui.painter_at(ar).add(egui::Shape::mesh(am));
                                    ui.painter().rect_stroke(
                                        ar,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0, BORDER_2),
                                        StrokeKind::Middle,
                                    );
                                    rail_thumb(ui.painter(), ar, ar.top() + (1.0 - m.hsva[3]) * ph);
                                    if arr.is_pointer_button_down_on() || arr.dragged() {
                                        if let Some(p) = arr.interact_pointer_pos() {
                                            m.hsva[3] = (1.0 - (p.y - ar.top()) / ph).clamp(0.0, 1.0);
                                        }
                                    }
                                }); // close the Picker plane+slider+alpha row
                            } // close: else (the Picker tab)
                        }); // close the left-region vertical (Picker plane OR Wheel)
                        ui.add_space(16.0);
                        // ── right column (shared by both tabs); the 16px gap above sets the colour
                        //    "art" zone apart from the numeric "controls" zone (A17 P6) ──
                        ui.vertical(|ui| {
                            ui.set_width(176.0);
                            // new / current split preview + OK / Cancel
                            ui.horizontal_top(|ui| {
                                let (swr, swresp) =
                                    ui.allocate_exact_size(egui::vec2(44.0, 58.0), egui::Sense::click());
                                let c = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
                                let newc = [c[0], c[1], c[2], m.hsva[3]];
                                let topr = egui::Rect::from_min_max(swr.min, egui::pos2(swr.right(), swr.center().y));
                                let botr = egui::Rect::from_min_max(egui::pos2(swr.left(), swr.center().y), swr.max);
                                if newc[3] < 0.999 {
                                    checker(&ui.painter_at(topr), topr, 5.0);
                                }
                                ui.painter().rect_filled(topr, CornerRadius::ZERO, rgba_c32a(newc));
                                match m.orig {
                                    Some(oc) => {
                                        if oc[3] < 0.999 {
                                            checker(&ui.painter_at(botr), botr, 5.0);
                                        }
                                        ui.painter().rect_filled(botr, CornerRadius::ZERO, rgba_c32a(oc));
                                    }
                                    None => {
                                        ui.painter().rect_filled(botr, CornerRadius::ZERO, SWATCH_WELL);
                                        ui.painter().line_segment(
                                            [
                                                botr.left_bottom() + egui::vec2(2.0, -2.0),
                                                botr.right_top() + egui::vec2(-2.0, 2.0),
                                            ],
                                            Stroke::new(1.4, NONE_RED),
                                        );
                                    }
                                }
                                ui.painter().rect_stroke(
                                    swr,
                                    CornerRadius::ZERO,
                                    Stroke::new(1.0, BORDER_2),
                                    StrokeKind::Middle,
                                );
                                if swresp.clicked() {
                                    if let Some(p) = swresp.interact_pointer_pos() {
                                        if p.y > swr.center().y {
                                            if let Some(oc) = m.orig {
                                                let h = rgb_to_hsv(oc);
                                                if h[1] > 0.001 {
                                                    m.hsva[0] = h[0];
                                                }
                                                m.hsva[1] = h[1];
                                                m.hsva[2] = h[2];
                                                m.hsva[3] = oc[3];
                                            }
                                        }
                                    }
                                }
                                swresp.on_hover_text("new / current \u{2014} click the bottom half to restore");
                                ui.vertical(|ui| {
                                    if dlg_btn(ui, "OK", true, 118.0) {
                                        ok = true;
                                    }
                                    if dlg_btn(ui, "Cancel", false, 118.0) {
                                        cancel = true;
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            // hex + alpha
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("#").color(MUTED).monospace().size(12.5));
                                let rgbc = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
                                let shown = hex_of([rgbc[0], rgbc[1], rgbc[2], 1.0]);
                                // commit on Enter/blur only — no colour-jumping through 3-digit parses mid-typing
                                if let Some(c2) = fields::hex(ui, 64.0, shown.trim_start_matches('#')) {
                                    let h = rgb_to_hsv(c2);
                                    if h[1] > 0.001 {
                                        m.hsva[0] = h[0];
                                    }
                                    m.hsva[1] = h[1];
                                    m.hsva[2] = h[2];
                                    m.hsva[3] = c2[3];
                                }
                                if let Some(v) = fields::num_value(
                                    ui,
                                    60.0,
                                    Lab::Letter("A"),
                                    "cm-a",
                                    m.hsva[3] * 100.0,
                                    0.0..=100.0,
                                ) {
                                    m.hsva[3] = v / 100.0;
                                }
                                ui.label(RichText::new("%").color(MUTED).size(11.0));
                            });
                            // HSB rows (radio → that channel drives the slider; field shows the other two)
                            for (chan, lab, tip, max, val, suf) in [
                                (Chan::H, "H", "cm-h", 360.0, m.hsva[0] * 360.0, "\u{00b0}"),
                                (Chan::S, "S", "cm-s", 100.0, m.hsva[1] * 100.0, "%"),
                                (Chan::B, "B", "cm-b", 100.0, m.hsva[2] * 100.0, "%"),
                            ] {
                                ui.horizontal(|ui| {
                                    if radio_dot(ui, m.chan == chan) {
                                        m.chan = chan;
                                    }
                                    if let Some(v) = fields::num_value(ui, 76.0, Lab::Letter(lab), tip, val, 0.0..=max)
                                    {
                                        match chan {
                                            Chan::H => m.hsva[0] = (v / 360.0).min(0.9999),
                                            Chan::S => m.hsva[1] = v / 100.0,
                                            _ => m.hsva[2] = v / 100.0,
                                        }
                                    }
                                    ui.label(RichText::new(suf).color(MUTED).size(11.0));
                                });
                            }
                            // RGB rows
                            let rgbv = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
                            for (i, (chan, lab, tip)) in
                                [(Chan::R, "R", "cm-r"), (Chan::G, "G", "cm-g"), (Chan::Bl, "B", "cm-bl")]
                                    .into_iter()
                                    .enumerate()
                            {
                                ui.horizontal(|ui| {
                                    if radio_dot(ui, m.chan == chan) {
                                        m.chan = chan;
                                    }
                                    if let Some(v) =
                                        fields::num_value(ui, 76.0, Lab::Letter(lab), tip, rgbv[i] * 255.0, 0.0..=255.0)
                                    {
                                        let mut c2 = rgbv;
                                        c2[i] = v / 255.0;
                                        let h = rgb_to_hsv([c2[0], c2[1], c2[2], 1.0]);
                                        if h[1] > 0.001 {
                                            m.hsva[0] = h[0];
                                        }
                                        m.hsva[1] = h[1];
                                        m.hsva[2] = h[2];
                                    }
                                });
                            }
                        });
                    });
                    // ── recent + document strips ──
                    let mut adopt = None;
                    if let Some(c) = swatch_strip(ui, "RECENT", &snap.recent) {
                        adopt = Some(c);
                    }
                    if let Some(c) = swatch_strip(ui, "DOCUMENT", &snap.doc_colors) {
                        adopt = Some(c);
                    }
                    if let Some(c) = adopt {
                        modal_adopt(m, c); // greys keep the current hue (no snap-to-red)
                    }
                });
            });
        // A5 — system eyedropper: while armed, sample the pixel under the OS cursor each frame (works
        // over ANY window) and preview it live; a fresh global left-click (edge-detected) commits it.
        if m.eyedropping {
            ctx.request_repaint(); // keep polling the global cursor + button while armed
            if let Some(c) = crate::cursors::screen_color_at_cursor() {
                modal_adopt(m, c);
            }
            let down = crate::cursors::left_button_down();
            if down && !m.eyedrop_prev_down {
                m.eyedropping = false; // committed — the sampled colour is already live in hsva
            }
            m.eyedrop_prev_down = down;
        }
        // keyboard: Esc = Cancel (or abort the eyedropper) · Enter = OK (only when no field is focused)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if m.eyedropping {
                m.eyedropping = false;
                m.hsva = m.eyedrop_return; // abort the pick: restore the colour before the eyedropper
            } else {
                cancel = true;
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && ctx.memory(|mem| mem.focused().is_none()) {
            ok = true;
        }
        // A6 — LIVE preview: apply the current colour to the target EVERY frame (no new undo step; the
        // session opened with EditCommand::PickerBegin). OK folds the whole drag into ONE step + remembers it;
        // Cancel/Esc reverts to the value at open.
        let c = hsv_to_rgb(m.hsva[0], m.hsva[1], m.hsva[2]);
        let col = [c[0], c[1], c[2], m.hsva[3]];
        if cancel {
            ops.push(Op::PickerCancel);
        } else {
            ops.push(Op::PickerLive(m.target, col));
            if ok {
                let cur = match m.target {
                    MTarget::Paint(t) => Some(t),
                    MTarget::Ab(_) => None,
                };
                ops.push(Op::PickerCommit(cur, col));
            }
        }
    }
    if ok || cancel {
        *modal = None;
    }
}
