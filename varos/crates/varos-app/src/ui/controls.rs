use super::*;

pub(crate) fn panel_frame(margin: i8) -> egui::Frame {
    egui::Frame {
        fill: SOLID_PANEL,
        corner_radius: CornerRadius::same(RBOX), // boxes = 8 (law); 14 was the dead floating-shell look

        stroke: Stroke::new(1.0, BORDER),
        // egui 0.31+ counts the 1px stroke as padding — compensate so content sits EXACTLY where it
        // did on 0.27 (the zero-perceptible-difference bar).
        inner_margin: Margin::same(margin - 1),
        ..Default::default()
    }
}

// ───────────────────────────── shared primitives ─────────────────────────────

pub(crate) const UV01: fn() -> egui::Rect = || egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

/// The egui temp key that holds the ACTIVE document's id for the frames being laid out.
pub(crate) fn doc_salt_key() -> egui::Id {
    egui::Id::new("varos-doc-salt")
}

/// Tag this context's next frames with the active document (`Ui::run` does it every frame).
pub(crate) fn set_doc_salt(ctx: &egui::Context, doc: Option<SessionId>) {
    ctx.data_mut(|d| d.insert_temp(doc_salt_key(), doc));
}

/// A persistent widget id scoped to the ACTIVE document (DFS S1 review P1): per-widget edit state —
/// a number field's typed buffer and focus, a name field's buffer, a row's double-click memory — can
/// never follow the user into another tab.
pub(crate) fn doc_id(ui: &egui::Ui, src: impl Hash + std::fmt::Debug) -> egui::Id {
    let doc = ui.ctx().data(|d| d.get_temp::<Option<SessionId>>(doc_salt_key())).flatten();
    ui.make_persistent_id((src, doc))
}

/// The 9-point transform reference widget (3×3 dots). Click a dot to set the reference (ax, ay).
pub(crate) fn refpoint(ui: &mut egui::Ui, sz: f32, refpt: &mut (f32, f32)) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(sz, sz), egui::Sense::click());
    let p = ui.painter();
    let inset = 7.0;
    let area = egui::Rect::from_min_max(
        egui::pos2(rect.left() + inset, rect.top() + inset),
        egui::pos2(rect.right() - inset, rect.bottom() - inset),
    );
    for ay in 0..3 {
        for ax in 0..3 {
            let (fx, fy) = (ax as f32 * 0.5, ay as f32 * 0.5);
            let c = egui::pos2(area.left() + fx * area.width(), area.top() + fy * area.height());
            if (refpt.0 - fx).abs() < 0.01 && (refpt.1 - fy).abs() < 0.01 {
                p.circle_filled(c, 2.6, ACCENT);
            } else {
                p.circle_stroke(c, 2.0, Stroke::new(1.0, MUTED));
            }
        }
    }
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let nx = ((pos.x - area.left()) / area.width().max(1.0) * 2.0).round().clamp(0.0, 2.0) / 2.0;
            let ny = ((pos.y - area.top()) / area.height().max(1.0) * 2.0).round().clamp(0.0, 2.0) / 2.0;
            *refpt = (nx, ny);
        }
    }
}

/// Icon chip for the hand-made filled glyphs that are not in the registry yet (Align / Distribute and
/// the control-bar mirrors). Same target and glyph size as `kit::icon_button` (QW6: 18 in 26 × 24).
pub(crate) fn icon_btn(ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ICON_BTN_W, ICON_BTN_H), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    if let Some(t) = tex {
        ui.painter().image(
            t.id(),
            egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(ICON_LG)),
            UV01(),
            if resp.hovered() { Color32::WHITE } else { MUTED },
        );
    }
    resp.on_hover_text(tip).clicked()
}

/// A short, full-width hairline divider.
pub(crate) fn hsep(ui: &mut egui::Ui, w: f32) {
    ui.add_space(9.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.left()..=rect.right(), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(9.0);
}

pub(crate) fn icon_button(ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, active: bool) -> egui::Response {
    // 30px cells / 16px glyphs — the rail sits in the same size family as the top-bar buttons
    // (Ahmed 07-07: "التول بار ضخم عن باقي البرنامج")
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
    let painter = ui.painter();
    let rounding = CornerRadius::same(R);
    if active {
        painter.rect_filled(rect, rounding, ACCENT);
    } else if resp.hovered() {
        painter.rect_filled(rect, rounding, HOVER);
    }
    if let Some(t) = tex {
        let ir = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(ICON_MD));
        painter.image(t.id(), ir, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }
    resp
}

pub(crate) fn divider(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 1.0), egui::Sense::hover());
    ui.painter().hline((rect.left() + 5.0)..=(rect.right() - 5.0), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(3.0);
}
/// A W (`width` = true) or H numeric field. For a Direct selection (`direct`) with a ZERO extent — one
/// anchor, or a purely horizontal/vertical run of anchors — there is nothing to scale, so the field is
/// shown disabled with that reason as its tooltip instead of silently ignoring the edit (Astra F07).
pub(crate) fn dim_field(
    ui: &mut egui::Ui,
    fw: f32,
    width: bool,
    value: f32,
    direct: bool,
    ops: &mut Vec<Op>,
    mk: impl Fn(f32) -> Op,
) {
    let (lab, tip, why) = if width {
        ("W", "Width", "Width: nothing to scale (the selected anchors have no horizontal extent)")
    } else {
        ("H", "Height", "Height: nothing to scale (the selected anchors have no vertical extent)")
    };
    let enabled = !direct || value > 1e-3; // same zero-extent threshold as `Editor::set_direct_bbox`
    let tip = if enabled { tip } else { why };
    ui.add_enabled_ui(enabled, |ui| fields::num(ui, fw, Lab::Letter(lab), tip, value, 0, 1.0, 0.0..=1.0e6, ops, mk));
}
/// A read-only "label … value" settings row (Document panel): label left (MUTED), value right (MUTED).
pub(crate) fn info_row(ui: &mut egui::Ui, w: f32, label: &str, value: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        MUTED,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 4.0, rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        FontId::proportional(12.5),
        MUTED,
    );
}

/// A clickable "label … value" row (the Units cycler): hover-highlights and returns true on click.
pub(crate) fn action_row(ui: &mut egui::Ui, w: f32, label: &str, value: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    let col = if resp.hovered() { TEXT } else { MUTED };
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        col,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 4.0, rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        FontId::proportional(12.5),
        col,
    );
    resp.clicked()
}

/// One segment of the compact "ALIGN TO" switch (A4). Selected → azure fill + white text; else the
/// inset-field fill, MUTED, HOVER on hover — the same active/rest read as `icon_toggle`.
pub(crate) fn seg_btn(ui: &mut egui::Ui, w: f32, label: &str, on: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::click());
    let bg = if on {
        ACCENT
    } else if resp.hovered() {
        HOVER
    } else {
        BG_SURFACE
    };
    ui.painter().rect_filled(rect, CornerRadius::same(R), bg);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        if on { Color32::WHITE } else { MUTED },
    );
    resp.on_hover_text(tip).clicked()
}
// ───────────────────────────── artboard inspector ─────────────────────────────

/// A label + a hand-painted pill switch (the Clip / transparent / move-with toggles). Returns true on click.
pub(crate) fn toggle_row(ui: &mut egui::Ui, w: f32, label: &str, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        if on { TEXT } else { MUTED },
    );
    let pill =
        egui::Rect::from_min_size(egui::pos2(rect.right() - 36.0, rect.center().y - 9.0), egui::vec2(32.0, 18.0));
    ui.painter().rect_filled(pill, CornerRadius::same(RCAP), if on { ACCENT } else { BG_SURFACE }); // capsule = one token (tabs + toggles)
    let knob = egui::pos2(if on { pill.right() - 9.0 } else { pill.left() + 9.0 }, pill.center().y);
    ui.painter().circle_filled(knob, 6.5, Color32::WHITE);
    resp.clicked()
}
