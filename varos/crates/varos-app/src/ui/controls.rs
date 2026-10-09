// ---- Lane F: shaped chrome ----
// ---- Lane F: text adapters ----
use varos_app::shell::kit::text::ShapedResponse as _;
// ---- end Lane F ----
use varos_app::shell::kit::text::ShapedPainter as _;
// ---- end Lane F ----
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
            icon_ink(false, resp.hovered()),
        );
    }
    resp.shaped_hover_text(tip).clicked()
}

/// A short, full-width hairline divider.
pub(crate) fn hsep(ui: &mut egui::Ui, w: f32) {
    ui.add_space(SECTION_GAP_HALF);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.left()..=rect.right(), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(SECTION_GAP_HALF);
}

/// Add only the unaccounted portion of a requested visible vertical gap. `egui` inserts its current
/// item spacing between the surrounding widgets, so adding the full target here would double-count it.
pub(crate) fn visible_gap(ui: &mut egui::Ui, target: f32) {
    ui.add_space((target - ui.spacing().item_spacing.y).max(0.0));
}

pub(crate) fn label_gap(ui: &mut egui::Ui) {
    visible_gap(ui, LABEL_GAP);
}

pub(crate) fn icon_ink(active: bool, hot: bool) -> Color32 {
    if active {
        Color32::WHITE
    } else if hot {
        TEXT
    } else {
        MUTED
    }
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
    fields::num_disabled(ui, fw, Lab::Letter(lab), tip, value, 0, 1.0, 0.0..=1.0e6, !enabled, ops, mk);
}
/// Ink for a segment: selected or hovered uses TEXT, otherwise MUTED.
pub(crate) fn seg_ink(on: bool, hot: bool) -> Color32 {
    if on || hot {
        TEXT
    } else {
        MUTED
    }
}

/// One shared grey `.seg` track with visible text labels. Labels are not repeated as tooltips.
pub(crate) fn segmented_text(
    ui: &mut egui::Ui,
    id: egui::Id,
    segment_w: f32,
    labels: &[&str],
    help: Option<&[&str]>,
    selected: usize,
) -> Option<usize> {
    let segment = egui::vec2(segment_w, varos_app::shell::tokens::SEG_BTN_H);
    let size = kit::board::segmented_size(labels.len(), segment);
    let (track, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let frame = kit::board::segmented_frame(ui, id, track, labels, selected, segment, help);
    for (i, label) in labels.iter().enumerate() {
        ui.painter().shaped_text(
            frame.rects[i].center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(SEG_TEXT),
            seg_ink(i == selected, frame.hot[i]),
        );
    }
    frame.chosen
}
// ───────────────────────────── artboard inspector ─────────────────────────────

/// A label + a hand-painted pill switch (the Clip / transparent / move-with toggles). Returns true on click.
pub(crate) fn toggle_row(ui: &mut egui::Ui, w: f32, label: &str, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    ui.painter().shaped_text(
        egui::pos2(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        TEXT,
    );
    let pill =
        egui::Rect::from_min_size(egui::pos2(rect.right() - 36.0, rect.center().y - 9.0), egui::vec2(32.0, 18.0));
    ui.painter().rect_filled(pill, CornerRadius::same(RCAP), toggle_track(on)); // capsule = one token (tabs + toggles)
    let knob = egui::pos2(if on { pill.right() - 9.0 } else { pill.left() + 9.0 }, pill.center().y);
    ui.painter().circle_filled(knob, 6.5, toggle_knob(on));
    resp.clicked()
}

pub(crate) fn toggle_track(on: bool) -> Color32 {
    if on {
        ACCENT
    } else {
        LINE2
    }
}

pub(crate) fn toggle_knob(on: bool) -> Color32 {
    if on {
        TEXT
    } else {
        MUTED
    }
}

/// Panel track, using the shared board kit (20 pt paint / ≥24 pt hit).
pub(crate) fn panel_segments(
    ui: &mut egui::Ui,
    key: &str,
    segments: &[(Icon, &str)],
    selected: usize,
    width: Option<f32>,
) -> Option<usize> {
    use varos_app::shell::{kit::board, tokens as t};
    let segment = egui::vec2(t::PANEL_SEG_W, t::PANEL_SEG_H);
    let mut size = board::segmented_size(segments.len(), segment);
    if let Some(width) = width {
        size.x = width;
    }
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    board::segmented_sized(ui, doc_id(ui, key), rect, segments, selected, None, segment, t::PANEL_SEG_GLYPH).0
}

/// LINE2 divider separating families inside a compact icon strip.
pub(crate) fn strip_divider(ui: &mut egui::Ui) {
    use varos_app::shell::tokens as t;
    ui.add_space(t::PANEL_ITEM_GAP_X);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(t::KIT_STROKE, t::PANEL_DIVIDER_H), egui::Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(t::KIT_STROKE, t::LINE2));
    ui.add_space(t::PANEL_ITEM_GAP_X);
}

pub(crate) fn wants_keyboard(ctx: &egui::Context) -> bool {
    kit::menu_open(ctx) || ctx.egui_wants_keyboard_input()
}
