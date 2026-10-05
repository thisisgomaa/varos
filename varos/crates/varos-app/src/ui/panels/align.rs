use super::super::*;

/// The Align pane body — THE home of align/distribute (ONE-HOME rule; the control bar mirrors it).
/// `align_target` is the A4 reference pref (Auto/Selection/Artboard); the switch below owns it and
/// every align op carries the current choice.
pub(crate) fn panel_align(ui: &mut egui::Ui, ic: &DockIcons, align_target: &mut AlignTarget, ops: &mut Vec<Op>) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
        // A4: the reference switch sits ABOVE the buttons — you pick what "align" means, then act.
        ui.label(RichText::new("ALIGN TO").color(MUTED).size(10.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let segs = [
                (
                    AlignTarget::Auto,
                    "Auto",
                    "Smart default: many objects align to each other; a single object or group aligns to the artboard",
                ),
                (AlignTarget::Selection, "Selection", "Align objects within the selection's combined bounds"),
                (AlignTarget::Artboard, "Artboard", "Align each object to the active artboard's edges"),
            ];
            for (t, label, tip) in segs {
                if seg_btn(ui, 60.0, label, *align_target == t, tip) {
                    *align_target = t;
                }
            }
        });
        ui.add_space(6.0);
        ui.label(RichText::new("ALIGN OBJECTS").color(MUTED).size(10.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
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
        ui.add_space(4.0);
        ui.label(RichText::new("DISTRIBUTE").color(MUTED).size(10.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if icon_btn(ui, &ic.align[6], "Distribute horizontal centres") {
                ops.push(Op::Distribute(DistAxis::Horizontal));
            }
            if icon_btn(ui, &ic.align[7], "Distribute vertical centres") {
                ops.push(Op::Distribute(DistAxis::Vertical));
            }
        });
    });
}

/// The Pathfinder pane body — THE home of the boolean ops (the i_overlay engine via `Editor::pathfinder`).
pub(crate) fn panel_pathfinder(ui: &mut egui::Ui, pf: Result<(), &'static str>, ops: &mut Vec<Op>) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
        ui.label(RichText::new("SHAPE MODES").color(MUTED).size(10.0).strong());
        ui.add_space(2.0);
        pathfinder_row(ui, ops, false, pf); // the roomier dock home
        ui.add_space(4.0);
        ui.label(RichText::new("Unite \u{b7} Minus Front \u{b7} Intersect \u{b7} Exclude").color(MUTED).size(10.5));
    });
}

/// The four boolean buttons (Unite / Minus Front / Intersect / Exclude) — hand-painted glyphs:
/// two overlapping squares with the op's region filled. Shared by the Pathfinder home + Shape mirror.
/// `compact` = the control-bar mirror (26×26, sized to the other bar controls); false = the roomier dock.
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

/// One pathfinder button. Dock: 34×28 chip, 14px squares. Bar mirror (`compact`): 26×26 chip, 12px
/// squares — so it no longer towers over the 24–26px bar controls (A14.1). WHITE-on-hover / MUTED at rest
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
    let chip = if compact { egui::vec2(26.0, 26.0) } else { egui::vec2(34.0, 28.0) };
    let (rect, resp) = ui.add_enabled_ui(off.is_none(), |ui| ui.allocate_exact_size(chip, egui::Sense::click())).inner;
    #[cfg(test)]
    tests::pathfinder_click_tests::PF_RECTS.with(|r| r.borrow_mut().push((op, rect, off.map(str::to_string))));
    let p = ui.painter();
    let hot = off.is_none() && resp.hovered();
    if hot {
        p.rect_filled(rect, CornerRadius::same(3), HOVER);
    }
    // Match the align/rail contrast: white on hover, MUTED enabled at rest, DISABLED when unavailable.
    let col = pf_btn_ink(off.is_some(), hot);
    // two overlapping squares; `oa`/`ob` are symmetric about the centre so the pair stays centred in the chip
    let sq = if compact { 12.0 } else { 14.0 };
    let (oax, oay) = if compact { (9.5, 7.75) } else { (11.0, 9.0) };
    let a = egui::Rect::from_min_size(rect.center() - egui::vec2(oax, oay), egui::vec2(sq, sq));
    let b = egui::Rect::from_min_size(rect.center() - egui::vec2(sq - oax, sq - oay), egui::vec2(sq, sq));
    let rr = CornerRadius::same(2);
    let sw = if compact { 1.3 } else { 1.4 }; // bolder outline than the old 1.0 hairline
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
        Color32::WHITE
    } else {
        MUTED
    }
}
