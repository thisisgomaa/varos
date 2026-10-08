use super::super::*;

/// The Align pane body — THE home of align/distribute (ONE-HOME rule; the control bar mirrors it).
/// `align_target` is the A4 reference pref (Auto/Selection/Artboard); the switch below owns it and
/// every align op carries the current choice.
pub(crate) fn panel_align(ui: &mut egui::Ui, ic: &DockIcons, align_target: &mut AlignTarget, ops: &mut Vec<Op>) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(PANEL_ITEM_GAP_X, 5.0);
        // A4: the reference switch sits ABOVE the buttons — you pick what "align" means, then act.
        let _label = ui.label(micro_label("ALIGN TO"));
        label_gap(ui);
        let targets = [AlignTarget::Auto, AlignTarget::Selection, AlignTarget::Artboard];
        let help = [
            "Align to: Auto — Smart default: many objects align to each other; a single object or group aligns to the artboard",
            "Align to: Selection — Align objects within the selection's combined bounds",
            "Align to: Artboard — Align each object to the active artboard's edges",
        ];
        let selected = targets.iter().position(|target| target == align_target).unwrap_or(0);
        let _track = ui.horizontal(|ui| {
            if let Some(index) = panel_segments(
                ui,
                "align-target",
                &[(Icon::AlignAuto, help[0]), (Icon::AlignSelection, help[1]), (Icon::Frame, help[2])],
                selected,
                None,
            ) {
                *align_target = targets[index];
            }
        });
        #[cfg(test)]
        align_probes::record_gap(_label.rect, _track.response.rect);
        ui.add_space(ALIGN_SECTION_GAP);
        let _label = ui.label(micro_label("ALIGN OBJECTS"));
        label_gap(ui);
        let _controls = ui.horizontal(|ui| {
            if icon_btn(ui, &ic.align[0], "Align left") {
                ops.push(Op::Align(AlignMode::Left, *align_target));
            }
            if icon_btn(ui, &ic.align[1], "Align centre") {
                ops.push(Op::Align(AlignMode::CenterH, *align_target));
            }
            if icon_btn(ui, &ic.align[2], "Align right") {
                ops.push(Op::Align(AlignMode::Right, *align_target));
            }
            if icon_btn(ui, &ic.align[3], "Align top") {
                ops.push(Op::Align(AlignMode::Top, *align_target));
            }
            if icon_btn(ui, &ic.align[4], "Align middle") {
                ops.push(Op::Align(AlignMode::Middle, *align_target));
            }
            if icon_btn(ui, &ic.align[5], "Align bottom") {
                ops.push(Op::Align(AlignMode::Bottom, *align_target));
            }
        });
        #[cfg(test)]
        align_probes::record_gap(_label.rect, _controls.response.rect);
        ui.add_space(ALIGN_SECTION_GAP);
        let _label = ui.label(micro_label("DISTRIBUTE"));
        label_gap(ui);
        let _controls = ui.horizontal(|ui| {
            if icon_btn(ui, &ic.align[6], "Distribute horizontal centres") {
                ops.push(Op::Distribute(DistAxis::Horizontal));
            }
            if icon_btn(ui, &ic.align[7], "Distribute vertical centres") {
                ops.push(Op::Distribute(DistAxis::Vertical));
            }
        });
        #[cfg(test)]
        align_probes::record_gap(_label.rect, _controls.response.rect);
    });
}

/// The Pathfinder pane body — THE home of the boolean ops (the i_overlay engine via `Editor::pathfinder`).
pub(crate) fn panel_pathfinder(ui: &mut egui::Ui, pf: Result<(), &'static str>, ops: &mut Vec<Op>) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
        ui.label(micro_label("SHAPE MODES"));
        label_gap(ui);
        pathfinder_row(ui, ops, false, pf); // the roomier dock home
    });
}

/// The four boolean buttons (Unite / Minus Front / Intersect / Exclude) — hand-painted glyphs:
/// two overlapping squares with the op's region filled. Shared by the Pathfinder home + Shape mirror.
/// `compact` = the 26×24 control-bar mirror, sized to the other bar controls; false = the roomier dock.
/// `pf` = `Editor::pathfinder_enabled`: on `Err` every button is drawn disabled with the reason.
pub(crate) fn pathfinder_row(ui: &mut egui::Ui, ops: &mut Vec<Op>, compact: bool, pf: Result<(), &'static str>) {
    use varos_core::boolean::BoolOp;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (op, tip) in [
            (BoolOp::Unite, "Unite"),
            (BoolOp::MinusFront, "Minus Front"),
            (BoolOp::Intersect, "Intersect"),
            (BoolOp::Exclude, "Exclude"),
        ] {
            if pf_btn(ui, op, tip, compact, pf.err()) {
                ops.push(Op::Bool(op));
            }
        }
    });
}

/// One pathfinder button. Both homes use the same 16 pt ink; the bar mirror is 26×24 like Align.
/// TEXT-on-hover / MUTED at rest
/// so the boolean icons read as clearly as the SVG align/rail icons. `off` = the kit disabled state:
/// DISABLED glyph, no hover, the reason in the tooltip, never clicks.
pub(crate) fn pf_btn(
    ui: &mut egui::Ui,
    op: varos_core::boolean::BoolOp,
    tip: &str,
    compact: bool,
    off: Option<&str>,
) -> bool {
    use varos_core::boolean::BoolOp;
    let chip = if compact { egui::vec2(PF_BAR_W, PF_BAR_H) } else { egui::vec2(34.0, 28.0) };
    let sense = if off.is_some() { egui::Sense::hover() } else { egui::Sense::click() };
    let (rect, resp) = ui.allocate_exact_size(chip, sense);
    #[cfg(test)]
    tests::pathfinder_click_tests::PF_RECTS.with(|r| r.borrow_mut().push((op, rect, off.map(str::to_string))));
    let p = ui.painter();
    let hot = off.is_none() && resp.hovered();
    if hot {
        p.rect_filled(rect, CornerRadius::same(3), HOVER);
    }
    // Match the align/rail contrast: TEXT on hover, MUTED enabled at rest, DISABLED when unavailable.
    let col = pf_btn_ink(off.is_some(), hot);
    // two overlapping squares; `oa`/`ob` are symmetric about the centre so the pair stays centred in the chip
    let a = egui::Rect::from_min_size(rect.center() - egui::vec2(PF_OFFSET, PF_OFFSET), egui::Vec2::splat(PF_SQUARE));
    let b = egui::Rect::from_min_size(
        rect.center() - egui::vec2(PF_SQUARE - PF_OFFSET, PF_SQUARE - PF_OFFSET),
        egui::Vec2::splat(PF_SQUARE),
    );
    let rr = CornerRadius::same(PF_RADIUS);
    let sw = PF_STROKE;
    match op {
        BoolOp::Unite => {
            p.rect_filled(a, rr, col);
            p.rect_filled(b, rr, col);
        }
        BoolOp::MinusFront => {
            p.rect_filled(a, rr, col);
            p.rect_filled(b, rr, SOLID_PANEL);
            p.rect_stroke(b, rr, Stroke::new(sw, col), StrokeKind::Middle);
        }
        BoolOp::Intersect => {
            p.rect_stroke(a, rr, Stroke::new(sw, col), StrokeKind::Middle);
            p.rect_stroke(b, rr, Stroke::new(sw, col), StrokeKind::Middle);
            p.rect_filled(a.intersect(b), CornerRadius::ZERO, col);
        }
        BoolOp::Exclude => {
            p.rect_filled(a, rr, col);
            p.rect_filled(b, rr, col);
            p.rect_filled(a.intersect(b), CornerRadius::ZERO, SOLID_PANEL);
        }
    }
    let state = off.map_or(kit::IconState::Action, kit::IconState::Disabled);
    let help = kit::icon_tooltip(tip, state);
    off.is_none() & resp.on_hover_text(&help).on_disabled_hover_text(&help).clicked()
}

pub(crate) fn pf_btn_ink(disabled: bool, hot: bool) -> Color32 {
    if disabled {
        DISABLED
    } else if hot {
        TEXT
    } else {
        MUTED
    }
}

#[cfg(test)]
pub(crate) mod align_probes {
    use std::cell::RefCell;

    thread_local! {
        pub(super) static GAPS: RefCell<Vec<(egui::Rect, egui::Rect)>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record_gap(label: egui::Rect, controls: egui::Rect) {
        GAPS.with(|gaps| gaps.borrow_mut().push((label, controls)));
    }

    pub(crate) fn clear() {
        GAPS.with(|gaps| gaps.borrow_mut().clear());
    }

    pub(crate) fn gaps() -> Vec<(egui::Rect, egui::Rect)> {
        GAPS.with(|gaps| gaps.borrow().clone())
    }
}
