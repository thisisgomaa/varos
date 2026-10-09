use super::*;

/// HAND 1 — the floating control bar (§4.4/§3.5), BORN in Stage 4. Its CONTENT follows the moment:
/// selection → transform/appearance/align + pathfinder mirrors · Artboard tool → page mirrors · Pen
/// mid-draft → the drawing hint. **Owner 2026-10-08 («مش عاوزها تظهر لو مفيش فيها حاجة أختار منها»):
/// when there is nothing to act on (no selection, not the Artboard tool, not drawing) the bar is not
/// shown at all** — this replaces the 07-07 "never vanishes" rule.
#[allow(clippy::too_many_arguments)] // hand-painted bar: each arg is live UI state
pub(crate) fn board_ctlbar(
    ctx: &egui::Context,
    board: egui::Rect,
    s: &Snap,
    ab: &AbSnap,
    ic: &DockIcons,
    fit_icon: &Option<egui::TextureHandle>,
    align_target: AlignTarget,
    ops: &mut Vec<Op>,
    fit_request: &mut Option<usize>,
) {
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    if ctlbar_hidden(s) {
        return;
    }
    egui::Area::new(egui::Id::new("hand1-ctlbar"))
        .order(egui::Order::Middle)
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(board.center().x, board.top() + 22.0))
        .show(ctx, |ui| {
            egui::Frame {
                fill: SOLID_PANEL,
                stroke: Stroke::new(1.0, BORDER),
                corner_radius: CornerRadius::same(RBOX),
                inner_margin: Margin::symmetric(10, 5),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // Pin the bar to the law's fixed 36px height (UI_VISION_MOCKUP.html:67 `.ctlbar{height:36px}`):
                    // the frame's 5+5 vertical margin + a 26px content row = 36, regardless of which context
                    // (Artboard / selection / idle) fills the bar, so its height never shifts (A14.2).
                    ui.set_min_height(26.0);
                    if s.tool == ToolKind::Artboard {
                        // page mirrors: name · X/Y/W/H · count · Fit
                        let i = ab.active;
                        ui.label(RichText::new("Artboard").color(MUTED).size(11.5));
                        control_bar_name(ui, &ab.name, TEXT);
                        bar_sep(ui);
                        let fw = 64.0;
                        fields::num(ui, fw, Lab::Letter("X"), "X position", ab.x, 0, 1.0, full.clone(), ops, |v| {
                            Op::AbRect(i, Some(v), None, None, None)
                        });
                        fields::num(ui, fw, Lab::Letter("Y"), "Y position", ab.y, 0, 1.0, full.clone(), ops, |v| {
                            Op::AbRect(i, None, Some(v), None, None)
                        });
                        fields::num(ui, fw, Lab::Letter("W"), "Width", ab.w, 0, 1.0, 1.0..=1.0e6, ops, |v| {
                            Op::AbRect(i, None, None, Some(v), None)
                        });
                        fields::num(ui, fw, Lab::Letter("H"), "Height", ab.h, 0, 1.0, 1.0..=1.0e6, ops, |v| {
                            Op::AbRect(i, None, None, None, Some(v))
                        });
                        bar_sep(ui);
                        ctl_ab_color(ui, ab.color, ab.id, ops);
                        if IA_CLIP.show(
                            ui,
                            if ab.count == 0 {
                                kit::IconState::Disabled("No artboard to clip")
                            } else {
                                kit::IconState::Toggle(ab.clip)
                            },
                        ) {
                            ops.push(Op::AbClip(i));
                        }
                        if IA_MOVE.show(ui, kit::IconState::Toggle(ab.move_art)) {
                            ops.push(Op::AbMoveArt(!ab.move_art));
                        }
                        bar_sep(ui);
                        ui.label(
                            RichText::new(format!("{} / {}", i + 1, ab.count)).color(MUTED).monospace().size(11.0),
                        );
                        if icon_btn(ui, fit_icon, "Fit in window") {
                            *fit_request = Some(i);
                        }
                    } else if s.sel && !s.drawing {
                        // …unless the Pen is mid-draft: an active path OWNS the bar even if the old
                        // selection lingered (select an object → press P → draw). Without `!s.drawing`
                        // the stale object's props hid the "Drawing path…" status (FB6).
                        control_bar_name(ui, &s.name, MUTED);
                        let fw = 64.0;
                        fields::num(ui, fw, Lab::Letter("X"), "X position", s.x, 0, 1.0, full.clone(), ops, |v| {
                            Op::SetBBox(Some(v), None, None, None, 0.0, 0.0)
                        });
                        fields::num(ui, fw, Lab::Letter("Y"), "Y position", s.y, 0, 1.0, full.clone(), ops, |v| {
                            Op::SetBBox(None, Some(v), None, None, 0.0, 0.0)
                        });
                        fields::num(ui, fw, Lab::Letter("W"), "Width", s.w, 0, 1.0, 0.0..=1.0e6, ops, |v| {
                            Op::SetBBox(None, None, Some(v), None, 0.0, 0.0)
                        });
                        fields::num(ui, fw, Lab::Letter("H"), "Height", s.h, 0, 1.0, 0.0..=1.0e6, ops, |v| {
                            Op::SetBBox(None, None, None, Some(v), 0.0, 0.0)
                        });
                        // the real rotation icon, matching the Properties dock (A14c)
                        fields::num(
                            ui,
                            62.0,
                            Lab::Icon(ic.rotate.as_ref()),
                            "Rotation",
                            s.rot,
                            1,
                            0.5,
                            full.clone(),
                            ops,
                            Op::SetRot,
                        );
                        bar_sep(ui);
                        ctl_chip(ui, s.fill, PaintTarget::Fill, s.fill_mixed, ops);
                        ctl_chip(ui, s.stroke, PaintTarget::Stroke, s.stroke_mixed, ops);
                        fields::num(
                            ui,
                            74.0,
                            Lab::Icon(ic.opacity.as_ref()),
                            "Opacity %",
                            s.opacity * 100.0,
                            0,
                            0.5,
                            0.0..=100.0,
                            ops,
                            |v| Op::SetOpacity(v / 100.0),
                        );
                        bar_sep(ui);
                        let al = [
                            (0usize, AlignMode::Left, "Align left"),
                            (1, AlignMode::CenterH, "Align centre"),
                            (2, AlignMode::Right, "Align right"),
                            (4, AlignMode::Middle, "Align middle"),
                        ];
                        for (i, m, tip) in al {
                            if icon_btn(ui, &ic.align[i], tip) {
                                ops.push(Op::Align(m, align_target)); // mirrors the dock's target pref (A4)
                            }
                        }
                        bar_sep(ui);
                        pathfinder_row(ui, ops, true, s.pathfinder); // compact bar mirror — the essential shape modes, in reach (Ahmed 07-07)
                    } else if s.direct && !s.drawing {
                        // Astra F07: a Direct selection with no object selection (e.g. an anchor grabbed
                        // straight off a deselected path) — name it and show its REAL bounds. Only controls
                        // that act on a Direct selection are mirrored here: X/Y/W/H (moves / scales the
                        // selected anchors via `SetObjectBounds`) and paint. Rotation, align and pathfinder
                        // work on objects, so they stay in the object branch above.
                        control_bar_name(ui, &s.name, MUTED);
                        let fw = 64.0;
                        fields::num(ui, fw, Lab::Letter("X"), "X position", s.x, 0, 1.0, full.clone(), ops, |v| {
                            Op::SetBBox(Some(v), None, None, None, 0.0, 0.0)
                        });
                        fields::num(ui, fw, Lab::Letter("Y"), "Y position", s.y, 0, 1.0, full.clone(), ops, |v| {
                            Op::SetBBox(None, Some(v), None, None, 0.0, 0.0)
                        });
                        dim_field(ui, fw, true, s.w, true, ops, |v| Op::SetBBox(None, None, Some(v), None, 0.0, 0.0));
                        dim_field(ui, fw, false, s.h, true, ops, |v| Op::SetBBox(None, None, None, Some(v), 0.0, 0.0));
                        bar_sep(ui);
                        ctl_chip(ui, s.fill, PaintTarget::Fill, s.fill_mixed, ops);
                        ctl_chip(ui, s.stroke, PaintTarget::Stroke, s.stroke_mixed, ops);
                    } else {
                        // idle: the current tool + a quiet hint — the bar keeps its place. While the Pen is
                        // mid-draft the hint reflects the ACT, not the (still-empty) selection (P9).
                        ui.label(RichText::new(crate::tool_name(s.tool)).color(TEXT).size(11.5));
                        let hint = if s.drawing { "Drawing path\u{2026}" } else { "No selection" };
                        ui.label(RichText::new(hint).color(MUTED).size(11.5));
                    }
                });
            });
        });
}

/// Fixed-width, left-aligned control-bar name. Long names elide in-place and retain the full tooltip.
pub(crate) fn control_bar_name(ui: &mut egui::Ui, name: &str, color: Color32) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(CONTROL_BAR_NAME_W, CONTROL_BAR_NAME_H), egui::Sense::hover());
    let font = FontId::proportional(CONTROL_BAR_NAME_TEXT);
    let fits = |text: &str| ui.painter().layout_no_wrap(text.to_owned(), font.clone(), color).size().x <= rect.width();
    let mut shown = name.to_owned();
    if !fits(&shown) {
        while !shown.is_empty() {
            shown.pop();
            let candidate = format!("{shown}…");
            if fits(&candidate) {
                shown = candidate;
                break;
            }
        }
    }
    ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, shown, font, color);
    resp.on_hover_text(name);
}
/// 1×16 vertical hairline separator inside the control bar (§3.5 vsep).
pub(crate) fn bar_sep(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(1.0, 16.0), egui::Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, BORDER));
}

/// A 17×17 colour chip (§3.5): click focuses; double-click opens the Color Picker for that target (a MIRROR of Appearance).
pub(crate) fn ctl_chip(ui: &mut egui::Ui, color: Option<Rgba>, target: PaintTarget, mixed: bool, ops: &mut Vec<Op>) {
    let (sw, resp) = ui.allocate_exact_size(egui::vec2(17.0, 17.0), egui::Sense::click());
    let round = CornerRadius::same(2);
    match color {
        Some(c) => {
            if c[3] < 0.999 {
                checker(&ui.painter_at(sw), sw, 4.0);
            }
            ui.painter().rect_filled(sw, round, rgba_c32a(c));
        }
        None => {
            ui.painter().rect_filled(sw, round, SWATCH_WELL);
            ui.painter().line_segment(
                [sw.left_bottom() + egui::vec2(2.0, -2.0), sw.right_top() + egui::vec2(-2.0, 2.0)],
                Stroke::new(1.4, NONE_RED),
            );
        }
    }
    if mixed {
        mixed_swatch(ui.painter(), sw);
    }
    ui.painter().rect_stroke(sw, round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    if resp.clicked() {
        ops.push(Op::PaintFocus(target));
    }
    if resp.double_clicked() {
        ops.push(Op::OpenPicker(MTarget::Paint(target)));
    }
    resp.on_hover_text(match target {
        PaintTarget::Fill => "Fill",
        PaintTarget::Stroke => "Stroke",
    });
}

/// Control-bar page colour field. Click opens the existing Color Picker for the active artboard.
pub(crate) fn ctl_ab_color(ui: &mut egui::Ui, color: Option<Rgba>, id: u32, ops: &mut Vec<Op>) {
    let (sw, resp) = ui.allocate_exact_size(egui::vec2(17.0, 17.0), egui::Sense::click());
    let round = CornerRadius::same(2);
    match color {
        Some(c) => {
            if c[3] < 0.999 {
                checker(&ui.painter_at(sw), sw, 4.0);
            }
            ui.painter().rect_filled(sw, round, rgba_c32a(c));
        }
        None => {
            ui.painter().rect_filled(sw, round, SWATCH_WELL);
            ui.painter().line_segment(
                [sw.left_bottom() + egui::vec2(2.0, -2.0), sw.right_top() + egui::vec2(-2.0, 2.0)],
                Stroke::new(1.4, NONE_RED),
            );
        }
    }
    ui.painter().rect_stroke(sw, round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    if resp.clicked() {
        ops.push(Op::OpenPicker(MTarget::Ab(id)));
    }
    resp.on_hover_text("Page colour");
    ui.label(
        RichText::new(color.map(hex_of).unwrap_or_else(|| "Transparent".into())).color(TEXT).monospace().size(11.0),
    );
}

/// Illustrator's fill/stroke control: overlapping FILL square (top-left) + STROKE ring (bottom-right);
/// the focused target draws ON TOP with an accent edge. Click a swatch to focus it (X toggles) ·
/// double-click opens the picker; the ⤡ arrows swap the colours (Shift+X) · the mini pair resets to white/black (D). None = red slash.
pub(crate) fn fill_stroke_control(ui: &mut egui::Ui, s: &Snap, ops: &mut Vec<Op>) {
    // 30-cell rail scale (Ahmed 07-07): 20px swatches overlapping inside a 30×40 slot
    let (area, _) = ui.allocate_exact_size(egui::vec2(30.0, 40.0), egui::Sense::hover());
    let p = ui.painter().clone();
    let fr = egui::Rect::from_min_size(area.min + egui::vec2(0.0, 3.0), egui::vec2(20.0, 20.0)); // fill
    let sr = egui::Rect::from_min_size(area.min + egui::vec2(10.0, 15.0), egui::vec2(20.0, 20.0)); // stroke
    let rr = CornerRadius::same(4);
    let slash = |p: &egui::Painter, r: egui::Rect| {
        p.line_segment(
            [r.left_bottom() + egui::vec2(2.5, -2.5), r.right_top() + egui::vec2(-2.5, 2.5)],
            Stroke::new(1.6, NONE_RED),
        )
    };
    let draw_fill = |p: &egui::Painter, active: bool| {
        p.rect_filled(fr.expand(2.0), CornerRadius::same(5), SOLID_PANEL); // swatch separation halo (sized to the swatch, not a control token)
        match s.fill {
            Some(c) => {
                if c[3] < 0.999 {
                    checker(p, fr, 5.0);
                }
                p.rect_filled(fr, rr, rgba_c32a(c));
            }
            None => {
                p.rect_filled(fr, rr, SWATCH_WELL);
                slash(p, fr);
            }
        }
        if s.fill_mixed {
            mixed_swatch(p, fr);
        }
        p.rect_stroke(
            fr,
            rr,
            Stroke::new(if active { 1.5 } else { 1.0 }, if active { ACCENT } else { BORDER_2 }),
            StrokeKind::Middle,
        );
    };
    let draw_stroke = |p: &egui::Painter, active: bool| {
        p.rect_filled(sr.expand(2.0), CornerRadius::same(5), SOLID_PANEL); // swatch separation halo
        let hole = sr.shrink(6.0);
        let hole_round = CornerRadius::same(2);
        match s.stroke {
            Some(c) => {
                if c[3] < 0.999 {
                    checker(p, sr, 5.0);
                }
                p.rect_filled(sr, rr, rgba_c32a(c));
                p.rect_filled(hole, hole_round, SOLID_PANEL);
            }
            None => {
                p.rect_filled(sr, rr, SWATCH_WELL);
                p.rect_filled(hole, hole_round, SOLID_PANEL);
                slash(p, sr);
            }
        }
        if s.stroke_mixed {
            mixed_swatch(p, sr);
            p.rect_filled(hole, hole_round, SOLID_PANEL);
        }
        p.rect_stroke(hole, hole_round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
        p.rect_stroke(
            sr,
            rr,
            Stroke::new(if active { 1.5 } else { 1.0 }, if active { ACCENT } else { BORDER_2 }),
            StrokeKind::Middle,
        );
    };
    let fill_on_top = s.paint == PaintTarget::Fill;
    if fill_on_top {
        draw_stroke(&p, false);
        draw_fill(&p, true);
    } else {
        draw_fill(&p, false);
        draw_stroke(&p, true);
    }
    // click → focus the swatch under the pointer (the TOP one wins in the overlap)
    let resp = ui.interact(area, ui.id().with("fs"), egui::Sense::click());
    let hit = |pos: egui::Pos2| -> Option<PaintTarget> {
        let (top, bot, tt, bt) = if fill_on_top {
            (fr, sr, PaintTarget::Fill, PaintTarget::Stroke)
        } else {
            (sr, fr, PaintTarget::Stroke, PaintTarget::Fill)
        };
        if top.contains(pos) {
            Some(tt)
        } else if bot.contains(pos) {
            Some(bt)
        } else {
            None
        }
    };
    if resp.clicked() {
        if let Some(t) = resp.interact_pointer_pos().and_then(hit) {
            ops.push(Op::PaintFocus(t));
            if resp.double_clicked() {
                ops.push(Op::OpenPicker(MTarget::Paint(t)));
            }
        }
    }
    resp.on_hover_text("Fill / Stroke — click to focus; double-click to edit (X toggles focus)");
    // swap (Shift+X): a tiny hand-painted double-headed arrow, top-right
    let swr = egui::Rect::from_min_size(area.min + egui::vec2(20.0, 0.0), egui::vec2(10.0, 10.0));
    let rsw = ui.interact(swr, ui.id().with("fs-swap"), egui::Sense::click());
    let sc = if rsw.hovered() { Color32::WHITE } else { MUTED };
    let (a, b) = (swr.center() + egui::vec2(-4.0, 2.2), swr.center() + egui::vec2(4.0, -2.2));
    p.line_segment([a, b], Stroke::new(1.3, sc));
    p.add(egui::Shape::convex_polygon(
        vec![b + egui::vec2(-3.5, -0.5), b + egui::vec2(-0.5, 3.0), b],
        sc,
        Stroke::NONE,
    ));
    p.add(egui::Shape::convex_polygon(vec![a + egui::vec2(3.5, 0.5), a + egui::vec2(0.5, -3.0), a], sc, Stroke::NONE));
    if rsw.clicked() {
        ops.push(Op::SwapColors);
    }
    rsw.on_hover_text("Swap fill & stroke (Shift+X)");
    // default (D): the mini white/black pair, bottom-left
    let dfr = egui::Rect::from_min_size(area.min + egui::vec2(0.0, 28.0), egui::vec2(12.0, 12.0));
    let rdf = ui.interact(dfr, ui.id().with("fs-def"), egui::Sense::click());
    let m1 = egui::Rect::from_min_size(dfr.min, egui::vec2(7.0, 7.0));
    let m2 = egui::Rect::from_min_size(dfr.min + egui::vec2(4.5, 4.5), egui::vec2(7.0, 7.0));
    p.rect_filled(m2, CornerRadius::same(2), Color32::from_gray(30));
    p.rect_stroke(
        m2,
        CornerRadius::same(2),
        Stroke::new(1.0, if rdf.hovered() { Color32::WHITE } else { BORDER_2 }),
        StrokeKind::Middle,
    );
    p.rect_filled(m1, CornerRadius::same(2), Color32::from_gray(242));
    p.rect_stroke(
        m1,
        CornerRadius::same(2),
        Stroke::new(1.0, if rdf.hovered() { Color32::WHITE } else { BORDER_2 }),
        StrokeKind::Middle,
    );
    if rdf.clicked() {
        ops.push(Op::DefaultPaint);
    }
    rdf.on_hover_text("Default colours (D)");
}

/// One rail slot standing in for all four shape tools. Left-click uses the current shape; right-click
/// opens a flyout of all four (Illustrator tool-group behaviour). A corner mark hints at the flyout.
pub(crate) fn shape_slot(
    ui: &mut egui::Ui,
    shapes: &[ToolBtn],
    shape_active: &mut ToolKind,
    s: &Snap,
    ops: &mut Vec<Op>,
) {
    let cur = shapes.iter().find(|t| t.kind == *shape_active).unwrap_or(&shapes[0]);
    let is_active = shapes.iter().any(|t| t.kind == s.tool);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
    let rounding = CornerRadius::same(R);
    if is_active {
        ui.painter().rect_filled(rect, rounding, ACCENT);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, rounding, HOVER);
    }
    if let Some(t) = &cur.tex {
        ui.painter().image(
            t.id(),
            egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(ICON_MD)),
            UV01(),
            icon_ink(is_active, resp.hovered()),
        );
    }
    // tiny flyout marker — a corner triangle bottom-right, like Illustrator's grouped tools
    let c = rect.right_bottom() + egui::vec2(-3.5, -3.5);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![c, c + egui::vec2(-4.5, 0.0), c + egui::vec2(0.0, -4.5)],
        icon_ink(is_active, resp.hovered()),
        Stroke::NONE,
    ));
    if resp.clicked() {
        ops.push(Op::Tool(*shape_active));
    }
    resp.clone().on_hover_text("Shapes \u{2014} click to use \u{00b7} right-click for more");
    let pop = ui.make_persistent_id("shape-flyout");
    if resp.secondary_clicked() {
        menu_toggle(ui, pop);
    }
    menu_below(ui, pop, &resp, None, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(5.0); // the menu frame has no horizontal padding — give the icons air
            for t in shapes {
                if icon_button(ui, &t.tex, s.tool == t.kind).on_hover_text(t.tip).clicked() {
                    *shape_active = t.kind;
                    ops.push(Op::Tool(t.kind));
                    menu_set(ui, pop, false);
                }
            }
            ui.add_space(5.0);
        });
    });
}

/// Idle = nothing to choose from (owner 2026-10-08): no selection (object or direct), not the Artboard
/// tool, and no path being drawn. Then the bar stays hidden instead of showing "Select (V) · No selection".
pub(crate) fn ctlbar_hidden(s: &Snap) -> bool {
    s.tool != ToolKind::Artboard && !s.sel && !s.direct && !s.drawing
}

#[cfg(test)]
mod idle_tests {
    use super::*;
    #[test]
    fn control_bar_hides_when_there_is_nothing_to_act_on() {
        let mut ed = varos_core::Editor::new();
        ed.tool = ToolKind::Object;
        let mut s = Snap::read(&ed);
        s.sel = false;
        s.direct = false;
        s.drawing = false;
        assert!(ctlbar_hidden(&s));
        s.drawing = true;
        assert!(!ctlbar_hidden(&s));
        s.drawing = false;
        s.sel = true;
        assert!(!ctlbar_hidden(&s));
        s.sel = false;
        s.direct = true;
        assert!(!ctlbar_hidden(&s));
        s.direct = false;
        s.tool = ToolKind::Artboard;
        assert!(!ctlbar_hidden(&s));
    }
}
