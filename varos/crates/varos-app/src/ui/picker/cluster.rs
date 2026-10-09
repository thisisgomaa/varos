use super::*;
/// Mixed paint has diagonal stripes, distinct from the alpha checkerboard.
pub(crate) fn mixed_swatch(p: &egui::Painter, r: egui::Rect) {
    let p = p.with_clip_rect(r);
    p.rect_filled(r, CornerRadius::ZERO, SWATCH_WELL);
    let mut x = r.left() - r.height();
    while x < r.right() {
        p.line_segment([egui::pos2(x, r.bottom()), egui::pos2(x + r.height(), r.top())], Stroke::new(2.0, MUTED));
        x += 6.0;
    }
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
    if !s.is_ascii() {
        return None;
    }
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
    p.rect_filled(r, CornerRadius::ZERO, t::CHECKER_DARK);
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
            p.rect_filled(cell, CornerRadius::ZERO, t::CHECKER_LIGHT);
        }
    }
}
/// Fill / Stroke row: a hand-painted swatch (click focuses, double-click opens the Color Picker), + hex + clear ×.
pub(crate) fn paint_row(ui: &mut egui::Ui, target: PaintTarget, color: Option<Rgba>, mixed: bool, ops: &mut Vec<Op>) {
    ui.horizontal(|ui| {
        let (sw, resp) = ui.allocate_exact_size(egui::vec2(ICON_BTN_W, ICON_BTN_H), egui::Sense::click());
        #[cfg(test)]
        paint_probes::record(target, sw);
        let round = CornerRadius::same(R);
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
        if mixed {
            mixed_swatch(p, sw);
        }
        if target == PaintTarget::Stroke {
            p.rect_filled(sw.shrink(varos_app::shell::tokens::SWATCH_RING_INSET), round, SOLID_PANEL);
        }
        p.rect_stroke(sw, round, Stroke::new(1.0, if resp.hovered() { MUTED } else { BORDER_2 }), StrokeKind::Middle);
        // Owner parity: click focuses; double-click opens.
        if resp.clicked() {
            ops.push(Op::PaintFocus(target));
        }
        if resp.double_clicked() {
            ops.push(Op::OpenPicker(MTarget::Paint(target)));
        }
        resp.on_hover_text(match target {
            PaintTarget::Fill => "Fill — click to focus; double-click to edit",
            PaintTarget::Stroke => "Stroke — click to focus; double-click to edit",
        });
        ui.add_space(8.0);
        kit::text(
            ui,
            &if mixed { "Mixed".into() } else { color.map(hex_of).unwrap_or_else(|| "None".into()) },
            t::mono(),
            t::TEXT,
        );
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

pub(crate) fn show(ui: &mut egui::Ui, origin: egui::Pos2, m: &ColorPanel, s: &Snap, ops: &mut Vec<Op>) {
    #[cfg(test)]
    super::super::fields::tests::probe(
        "picker cluster",
        egui::Rect::from_min_size(origin, egui::Vec2::splat(t::PICKER_FILL_R)),
    );

    // Owner parity (hand test 2026-10-09): the focused target is painted IN FRONT (Illustrator/Affinity):
    // Fill focused → fill circle over the stroke ring; Stroke focused → ring over the circle.
    let front = cluster_front(m);
    let stroke = (PaintTarget::Stroke, t::PICKER_STROKE_CENTER, t::PICKER_STROKE_R);
    let fill = (PaintTarget::Fill, t::PICKER_FILL_CENTER, t::PICKER_FILL_R);
    let order = if front == PaintTarget::Stroke { [fill, stroke] } else { [stroke, fill] };
    for (target, point, radius) in order {
        let center = origin + egui::vec2(point[0], point[1]);
        let rect = egui::Rect::from_center_size(center, egui::Vec2::splat(radius * 2.0));
        let live = m.target == MTarget::Paint(target) && (m.change_requested || m.edited);
        let color = if live { Some(m.color()) } else { snap_target_color(s, target) };
        let mixed = !live && s.target_mixed(target);
        let p = ui.painter();
        p.circle_filled(center, radius, t::SURFACE);
        if let Some(c) = color {
            p.circle_filled(center, radius, rgba_c32a(c));
        }
        if mixed {
            // Diagonal chords preserve the existing Mixed pattern without a rectangular frame.
            let unit = std::f32::consts::FRAC_1_SQRT_2;
            let tangent = egui::vec2(unit, -unit);
            let normal = egui::vec2(unit, unit);
            let mut d = -radius;
            while d <= radius {
                let length = (radius * radius - d * d).max(0.0).sqrt();
                let mid = center + normal * d;
                p.line_segment(
                    [mid - tangent * length, mid + tangent * length],
                    Stroke::new(t::PICKER_MIXED_STROKE, t::MUTED),
                );
                d += t::PICKER_MIXED_GAP;
            }
        } else if color.is_none() {
            p.line_segment(
                [center + egui::vec2(-radius / 2.0, radius / 2.0), center + egui::vec2(radius / 2.0, -radius / 2.0)],
                Stroke::new(t::PICKER_FOCUS, t::NONE_RED),
            );
        }
        if target == PaintTarget::Stroke {
            p.circle_filled(center, radius - t::PICKER_STROKE_BAND, t::PANEL);
        }
        p.circle_stroke(
            center,
            radius,
            Stroke::new(t::PICKER_FOCUS, if m.target == MTarget::Paint(target) { t::ACCENT } else { t::LINE2 }),
        );
        let response = ui.interact(rect, ui.id().with(("target", target as u8)), egui::Sense::click());
        let hit = ui.input(|i| i.pointer.interact_pos()).and_then(|p| cluster_target(origin, p, front));
        if response.clicked() {
            if let Some(target) = hit {
                ops.push(Op::PaintFocus(target));
            }
        }
        response.on_hover_text(if hit == Some(PaintTarget::Stroke) { "Stroke (X)" } else { "Fill (X)" });
    }
    for (icon, point, size, tip) in [
        (Icon::PickerSwap, t::PICKER_SWAP_POS, t::PICKER_GLYPH, "Swap Fill / Stroke (⇧X)"),
        (Icon::PickerNone, t::PICKER_NONE_POS, t::PICKER_NONE_SIZE, "None (/)"),
    ] {
        let center = origin + egui::vec2(point[0], point[1]);
        if icon == Icon::PickerNone {
            ui.painter().circle_filled(center, size / 2.0, t::TEXT);
            icon.paint(ui.painter(), center, size, t::NONE_RED);
        } else {
            icon.paint(ui.painter(), center, size, t::MUTED);
        }
        if ui
            .interact(
                egui::Rect::from_center_size(center, egui::Vec2::splat(size)),
                ui.id().with(tip),
                egui::Sense::click(),
            )
            .on_hover_text(tip)
            .clicked()
        {
            match icon {
                Icon::PickerSwap => ops.push(Op::SwapColors),
                _ => match m.target {
                    MTarget::Paint(t) => ops.push(Op::Paint(t, None)),
                    MTarget::Ab(id) => ops.push(Op::AbColorId(id, None)),
                },
            }
        }
    }
}

/// The target painted in front: the focused paint target (Fill when the panel edits a page colour).
pub(crate) fn cluster_front(m: &ColorPanel) -> PaintTarget {
    match m.target {
        MTarget::Paint(target) => target,
        MTarget::Ab(_) => PaintTarget::Fill,
    }
}

/// Resolve painted circles. The `front` target wins where the fill circle and the stroke ring overlap,
/// matching what is painted on top; the Stroke ring stays hittable inside Fill's bounding-square corner.
pub(crate) fn cluster_target(origin: egui::Pos2, p: egui::Pos2, front: PaintTarget) -> Option<PaintTarget> {
    let fill = origin + egui::vec2(t::PICKER_FILL_CENTER[0], t::PICKER_FILL_CENTER[1]);
    let stroke = origin + egui::vec2(t::PICKER_STROKE_CENTER[0], t::PICKER_STROKE_CENTER[1]);
    let on_fill = p.distance(fill) <= t::PICKER_FILL_R;
    let d = p.distance(stroke);
    let on_stroke = (t::PICKER_STROKE_R - t::PICKER_STROKE_BAND..=t::PICKER_STROKE_R).contains(&d);
    match (on_fill, on_stroke) {
        (true, true) => Some(front),
        (true, false) => Some(PaintTarget::Fill),
        (false, true) => Some(PaintTarget::Stroke),
        (false, false) => None,
    }
}

#[cfg(test)]
pub(crate) mod paint_probes {
    use std::cell::RefCell;

    use super::PaintTarget;

    thread_local! {
        pub(crate) static SWATCHES: RefCell<Vec<(PaintTarget, egui::Rect)>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn record(target: PaintTarget, rect: egui::Rect) {
        SWATCHES.with(|swatches| swatches.borrow_mut().push((target, rect)));
    }

    pub(crate) fn clear() {
        SWATCHES.with(|swatches| swatches.borrow_mut().clear());
    }

    pub(crate) fn swatches() -> Vec<(PaintTarget, egui::Rect)> {
        SWATCHES.with(|swatches| swatches.borrow().clone())
    }
}
