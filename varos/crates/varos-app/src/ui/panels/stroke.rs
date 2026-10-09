//! Properties is the Stroke home; the control bar mirrors these same checked commands.
//! Provisional kit composition, owner design review pending. No new shortcuts or visual primitives.
// ---- Lane F: shaped chrome ----
use varos_app::shell::kit::text::ShapedUi as _;
// ---- end Lane F ----
use super::super::*;
use varos_app::shell::tokens::{
    STROKE_CHOICE_W, STROKE_MIRROR_WEIGHT_W, STROKE_POPUP_H, STROKE_POPUP_MARGIN, STROKE_POPUP_W,
};
use varos_core::stroke::{ArrowAlign, ArrowHead, StrokeAlign, StrokeCap, StrokeJoin, StrokeStyle};

#[derive(Clone, Default)]
struct StrokeNumberGesture {
    value: Option<f32>,
}
impl StrokeNumberGesture {
    fn update(
        &mut self,
        live: Option<f32>,
        commit: Option<f32>,
        closed: bool,
        editing: bool,
        down: bool,
    ) -> Option<f32> {
        if let Some(v) = commit {
            self.value = None;
            return Some(v);
        }
        if closed {
            self.value = None;
            return None;
        }
        if let Some(v) = live {
            // The kit rebases typed arrow-key steps immediately; preserve that single-step
            // behaviour. Pointer scrubs retain their accumulator until the release frame.
            if editing {
                self.value = None;
                return Some(v);
            }
            self.value = Some(v);
        }
        if !down && !editing {
            return self.value.take();
        }
        None
    }
}
fn choice(ui: &mut egui::Ui, key: &str, label: &str, names: &[&str], selected: usize) -> Option<usize> {
    let id = doc_id(ui, key);
    let value = names.get(selected).copied().unwrap_or("Mixed");
    let text = varos_app::i18n::message(
        ui.ctx(),
        "{label}: {value}",
        &[
            ("label", &varos_app::i18n::translate(ui.ctx(), label)),
            ("value", &varos_app::i18n::translate(ui.ctx(), value)),
        ],
    );
    kit::text_dropdown(ui, id, &text, names, STROKE_CHOICE_W, label)
}
#[allow(clippy::too_many_arguments)]
fn number(
    ui: &mut egui::Ui,
    w: f32,
    s: &StrokeStyle,
    tip: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    disabled: bool,
    ops: &mut Vec<Op>,
    field: StrokeField,
    change: impl Fn(&mut StrokeStyle, f32),
) {
    ui.vertical(|ui| {
        ui.shaped_label(micro_label(tip));
        use varos_app::shell::kit::field::{self as kf, NumberField};
        // Keep numeric gestures local until release/commit: one checked batch, one undo step.
        let id = doc_id(ui, ("p2-stroke-number", kf::home(ui), tip));
        let gesture_id = id.with("gesture");
        let mut gesture = ui.ctx().data(|d| d.get_temp::<StrokeNumberGesture>(gesture_id)).unwrap_or_default();
        let e = kf::number_field(
            ui,
            NumberField {
                id,
                width: w,
                label: Lab::Letter(""),
                tip,
                value: gesture.value.unwrap_or(value),
                decimals: 2,
                speed: 0.1,
                range,
                disabled,
            },
        );
        #[cfg(test)]
        fields::tests::probe(tip, e.rect);
        let mk = |v| {
            let mut next = s.clone();
            change(&mut next, v);
            Op::SetStrokeField(field, next)
        };
        if let Some(v) = e.pending {
            ops.push(Op::FieldPending(id, Box::new(mk(v))));
        }
        if let Some(v) = gesture.update(e.live, e.commit, e.closed, e.editing, ui.input(|i| i.pointer.primary_down())) {
            ops.push(Op::Field(Box::new(mk(v))));
        }
        ui.ctx().data_mut(|d| d.insert_temp(gesture_id, gesture));
    });
}
pub(crate) fn stroke_section(ui: &mut egui::Ui, s: &Snap, ic: &DockIcons, w: f32, ops: &mut Vec<Op>) {
    if let Some(error) = &s.stroke_error {
        kit::notice(ui, error);
    }
    let disclosure = doc_id(ui, "stroke-style-disclosure");
    let mut open = ui.ctx().data(|d| d.get_temp::<bool>(disclosure)).unwrap_or(false);
    ui.horizontal(|ui| {
        let id = doc_id(ui, "stroke-weight-presets");
        let weights = [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0];
        let entries =
            ["0.25 pt", "0.5 pt", "1 pt", "2 pt", "3 pt", "4 pt", "6 pt", "8 pt", "12 pt"].map(kit::MenuEntry::Item);
        if let Some(i) = kit::menu(ui.ctx(), id, &entries) {
            ops.push(Op::SetStrokeW(weights[i]));
        }
        fields::num(
            ui,
            w - ICON_BTN_W * 2.0 - PANEL_ITEM_GAP_X * 2.0,
            Lab::Icon(ic.strokew.as_ref()),
            "Stroke weight",
            s.sw,
            2,
            0.2,
            0.0..=1.0e6,
            ops,
            Op::SetStrokeW,
        );
        if kit::icon_button(ui, disclosure, Icon::PickerSliders, "Stroke options", kit::IconState::Toggle(open))
            .activated
        {
            open = !open;
            ui.ctx().data_mut(|d| d.insert_temp(disclosure, open));
        }
        let r = IA_STROKE_PRESETS.show_response(ui, kit::IconState::Action);
        if r.activated {
            kit::toggle_menu_below(ui.ctx(), id, r.response.rect);
            let _ = kit::menu(ui.ctx(), id, &entries);
        }
    });
    if !open {
        return;
    }
    if s.stroke_style_mixed {
        ui.shaped_label(micro_label("MIXED STYLES"));
    }
    let style = &s.stroke_style;
    // Show mixed state without assigning a representative object's unrelated fields to its peers.
    ui.add_enabled_ui(s.has_paint, |ui| {
        ui.horizontal(|ui| {
            let caps = [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square];
            if let Some(i) = choice(
                ui,
                "stroke-cap",
                "Cap",
                &["Butt", "Round", "Square"],
                if s.stroke_field_mixed(StrokeField::Cap) {
                    usize::MAX
                } else {
                    caps.iter().position(|v| *v == style.cap).unwrap_or(1)
                },
            ) {
                let mut next = style.clone();
                next.cap = caps[i];
                ops.push(Op::SetStrokeField(StrokeField::Cap, next));
            }
            let joins = [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel];
            if let Some(i) = choice(
                ui,
                "stroke-join",
                "Join",
                &["Miter", "Round", "Bevel"],
                if s.stroke_field_mixed(StrokeField::Join) {
                    usize::MAX
                } else {
                    joins.iter().position(|v| *v == style.join).unwrap_or(1)
                },
            ) {
                let mut next = style.clone();
                next.join = joins[i];
                ops.push(Op::SetStrokeField(StrokeField::Join, next));
            }
        });
        number(
            ui,
            w,
            style,
            "Miter limit",
            style.miter_limit,
            1.0..=1000.0,
            style.join != StrokeJoin::Miter,
            ops,
            StrokeField::Miter,
            |s, v| s.miter_limit = v,
        );
        ui.horizontal(|ui| {
            for (label, value) in
                [("Center", StrokeAlign::Center), ("Inside", StrokeAlign::Inside), ("Outside", StrokeAlign::Outside)]
            {
                let mut control = kit::Control::new(doc_id(ui, ("stroke-align", label)), label);
                control.selected = !s.stroke_field_mixed(StrokeField::Align) && style.align == value;
                if kit::action(ui, control, false).activated {
                    let mut next = style.clone();
                    next.align = value;
                    ops.push(Op::SetStrokeField(StrokeField::Align, next));
                }
            }
        });
        if toggle_row(ui, w, "Dashed", !style.dash.is_empty()) {
            let mut next = style.clone();
            next.dash = if next.dash.is_empty() { vec![6.0, 3.0] } else { vec![] };
            ops.push(Op::SetStrokeStyle(style.clone(), next));
        }
        if !style.dash.is_empty() {
            for pair in 0..3 {
                ui.horizontal(|ui| {
                    for side in 0..2 {
                        let index = pair * 2 + side;
                        let tip = varos_app::i18n::message(
                            ui.ctx(),
                            "{label} {number}",
                            &[
                                (
                                    "label",
                                    &varos_app::i18n::translate(ui.ctx(), if side == 0 { "Dash" } else { "Gap" }),
                                ),
                                ("number", &(pair + 1).to_string()),
                            ],
                        );
                        number(
                            ui,
                            (w - PANEL_ITEM_GAP_X) * 0.5,
                            style,
                            &tip,
                            style.dash.get(index).copied().unwrap_or(0.0),
                            0.0..=1.0e6,
                            false,
                            ops,
                            StrokeField::Dash(index),
                            move |s, v| {
                                if v == 0.0 && index >= s.dash.len() {
                                    return;
                                }
                                while s.dash.len() <= index {
                                    s.dash.extend([6.0, 3.0]);
                                }
                                s.dash[index] = v;
                            },
                        );
                    }
                });
            }
            if toggle_row(ui, w, "Align to corners", style.align_dashes_to_corners) {
                let mut next = style.clone();
                next.align_dashes_to_corners = !next.align_dashes_to_corners;
                ops.push(Op::SetStrokeStyle(style.clone(), next));
            }
            number(
                ui,
                w,
                style,
                "Dash phase",
                style.dash_phase,
                -1.0e6..=1.0e6,
                style.align_dashes_to_corners,
                ops,
                StrokeField::Phase,
                |s, v| s.dash_phase = v,
            );
        }
        let mut names = vec!["None".to_owned()];
        names.extend(ArrowHead::ALL.iter().map(|h| format!("{h:?}")));
        let refs: Vec<_> = names.iter().map(String::as_str).collect();
        for end in 0..2 {
            let current = if end == 0 { style.arrows.start } else { style.arrows.end };
            let index = current.and_then(|h| ArrowHead::ALL.iter().position(|v| *v == h)).map_or(0, |i| i + 1);
            if let Some(i) = choice(
                ui,
                if end == 0 { "stroke-start-head" } else { "stroke-end-head" },
                if end == 0 { "Start" } else { "End" },
                &refs,
                if s.stroke_field_mixed(if end == 0 { StrokeField::Start } else { StrokeField::End }) {
                    usize::MAX
                } else {
                    index
                },
            ) {
                let mut next = style.clone();
                let head = i.checked_sub(1).map(|i| ArrowHead::ALL[i]);
                if end == 0 {
                    next.arrows.start = head;
                } else {
                    next.arrows.end = head;
                }
                ops.push(Op::SetStrokeField(if end == 0 { StrokeField::Start } else { StrokeField::End }, next));
            }
            let scale = if end == 0 { style.arrows.scale_start } else { style.arrows.scale_end };
            number(
                ui,
                w,
                style,
                if end == 0 { "Start scale %" } else { "End scale %" },
                scale * 100.0,
                1.0..=10000.0,
                false,
                ops,
                if end == 0 { StrokeField::ScaleStart } else { StrokeField::ScaleEnd },
                move |s, v| {
                    if end == 0 {
                        s.arrows.scale_start = v / 100.0;
                    } else {
                        s.arrows.scale_end = v / 100.0;
                    }
                },
            );
        }
        if let Some(i) = choice(
            ui,
            "stroke-arrow-align",
            "Placement",
            &["Tip", "Extend"],
            if s.stroke_field_mixed(StrokeField::ArrowAlign) {
                usize::MAX
            } else {
                usize::from(style.arrows.align == ArrowAlign::Extend)
            },
        ) {
            let mut next = style.clone();
            next.arrows.align = if i == 0 { ArrowAlign::Tip } else { ArrowAlign::Extend };
            ops.push(Op::SetStrokeField(StrokeField::ArrowAlign, next));
        }
        ui.horizontal(|ui| {
            if kit::action(ui, kit::Control::new(doc_id(ui, "stroke-swap"), "Swap heads"), false).activated {
                ops.push(Op::SwapStrokeHeads);
            }
            if kit::action(ui, kit::Control::new(doc_id(ui, "stroke-reset"), "Reset style"), false).activated {
                ops.push(Op::ResetStrokeStyle);
            }
        });
    });
    if s.stroke_open && style.align != StrokeAlign::Center {
        ui.shaped_label(micro_label("OPEN PATHS USE CENTER"));
    }
    if s.stroke_no_tangent && (style.arrows.start.is_some() || style.arrows.end.is_some()) {
        ui.shaped_label(micro_label("POINTS IGNORE ARROWHEADS"));
    }
    if s.stroke_closed && (style.arrows.start.is_some() || style.arrows.end.is_some()) {
        ui.shaped_label(micro_label("CLOSED PATHS IGNORE HEADS"));
    }
}
pub(crate) fn stroke_mirror(ui: &mut egui::Ui, s: &Snap, ic: &DockIcons, ops: &mut Vec<Op>) {
    if s.stroke_style_mixed {
        ui.shaped_label(micro_label("MIXED STYLES"));
    }
    let style = &s.stroke_style;
    fields::num(
        ui,
        STROKE_MIRROR_WEIGHT_W,
        Lab::Letter("W"),
        "Stroke weight",
        s.sw,
        2,
        0.2,
        0.0..=1.0e6,
        ops,
        Op::SetStrokeW,
    );
    let caps = [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square];
    if let Some(i) = choice(
        ui,
        "bar-stroke-cap",
        "Cap",
        &["Butt", "Round", "Square"],
        if s.stroke_field_mixed(StrokeField::Cap) {
            usize::MAX
        } else {
            caps.iter().position(|v| *v == style.cap).unwrap_or(1)
        },
    ) {
        let mut next = style.clone();
        next.cap = caps[i];
        ops.push(Op::SetStrokeField(StrokeField::Cap, next));
    }
    let joins = [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel];
    if let Some(i) = choice(
        ui,
        "bar-stroke-join",
        "Join",
        &["Miter", "Round", "Bevel"],
        if s.stroke_field_mixed(StrokeField::Join) {
            usize::MAX
        } else {
            joins.iter().position(|v| *v == style.join).unwrap_or(1)
        },
    ) {
        let mut next = style.clone();
        next.join = joins[i];
        ops.push(Op::SetStrokeField(StrokeField::Join, next));
    }
    let aligns = [StrokeAlign::Center, StrokeAlign::Inside, StrokeAlign::Outside];
    if let Some(i) = choice(
        ui,
        "bar-stroke-align",
        "Align",
        &["Center", "Inside", "Outside"],
        if s.stroke_field_mixed(StrokeField::Align) {
            usize::MAX
        } else {
            aligns.iter().position(|v| *v == style.align).unwrap_or(0)
        },
    ) {
        let mut next = style.clone();
        next.align = aligns[i];
        ops.push(Op::SetStrokeField(StrokeField::Align, next));
    }
    let owner = doc_id(ui, "stroke-mirror-options");
    let mut open = ui.ctx().data(|d| d.get_temp::<bool>(owner)).unwrap_or(false);
    let button = kit::icon_button(ui, owner, Icon::PickerSliders, "All Stroke options", kit::IconState::Toggle(open));
    if button.activated {
        open = !open;
    }
    if open {
        let viewport = ui.ctx().content_rect();
        let position = egui::pos2(
            button
                .response
                .rect
                .left()
                .clamp(viewport.left(), (viewport.right() - STROKE_POPUP_W).max(viewport.left())),
            button.response.rect.bottom(),
        );
        let popup = egui::Area::new(owner.with("panel")).order(egui::Order::Foreground).fixed_pos(position).show(
            ui.ctx(),
            |ui| {
                panel_frame(STROKE_POPUP_MARGIN).show(ui, |ui| {
                    ui.set_width(STROKE_POPUP_W);
                    ui.shaped_label(panel_title("Stroke"));
                    egui::ScrollArea::vertical().id_salt(owner).max_height(STROKE_POPUP_H).show(ui, |ui| {
                        let disclosure = doc_id(ui, "stroke-style-disclosure");
                        ui.ctx().data_mut(|d| d.insert_temp(disclosure, true));
                        stroke_section(ui, s, ic, STROKE_POPUP_W, ops);
                    });
                });
            },
        );
        if !kit::menu_open(ui.ctx())
            && ui.input(|i| {
                i.pointer.any_pressed()
                    && i.pointer
                        .interact_pos()
                        .is_some_and(|p| !popup.response.rect.contains(p) && !button.response.rect.contains(p))
            })
        {
            open = false;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(owner, open));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor() -> Editor {
        let mut ed = Editor::new();
        let mut doc = varos_core::format::decode_model(
            include_bytes!("../../../../varos-core/tests/fixtures/v5/cap_Butt.json"),
            None,
            &varos_core::format::Limits::DEFAULT,
        )
        .unwrap()
        .doc;
        let mut second = doc.paths[0].clone();
        second.id = 50;
        for (i, anchor) in second.anchors.iter_mut().enumerate() {
            anchor.id = 51 + i as u32;
        }
        second.stroke_style.cap = StrokeCap::Square;
        second.stroke_style.dash = vec![6.0, 3.0];
        doc.paths.push(second);
        doc.ids = 200;
        doc.sync_tree();
        ed.replace_doc(doc);
        ed.objsel.extend([10, 50]);
        ed
    }
    #[test]
    fn typed_arrow_step_is_not_lost_on_unchanged_blur() {
        let mut gesture = StrokeNumberGesture::default();
        assert_eq!(gesture.update(Some(101.0), None, false, true, false), Some(101.0));
        assert_eq!(gesture.update(None, None, true, false, false), None);
    }

    #[test]
    fn real_kit_scrub_across_frames_commits_once_on_release() {
        let mut ed = editor();
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        let frame = |ed: &mut Editor, events: Vec<egui::Event>| {
            fields::tests::clear_probes();
            let snap = Snap::read(ed);
            let mut ops = vec![];
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0))),
                    ..Default::default()
                },
                |ui| {
                    number(
                        ui,
                        200.0,
                        &snap.stroke_style,
                        "Start scale %",
                        snap.stroke_style.arrows.scale_start * 100.0,
                        1.0..=10000.0,
                        false,
                        &mut ops,
                        StrokeField::ScaleStart,
                        |s, v| s.arrows.scale_start = v / 100.0,
                    );
                },
            );
            let rect = fields::tests::probed_rect("Start scale %", 0);
            apply_ops(ed, ops);
            rect
        };
        let rect = frame(&mut ed, vec![]);
        frame(&mut ed, vec![]);
        let start = rect.center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let original = ed.doc.clone();
        let rev = ed.rev;
        frame(&mut ed, vec![egui::Event::PointerMoved(start), button(start, true)]);
        for dx in [20.0, 40.0, 60.0] {
            frame(&mut ed, vec![egui::Event::PointerMoved(start + egui::vec2(dx, 0.0))]);
            assert_eq!(ed.doc, original);
            assert_eq!(ed.rev, rev);
        }
        frame(&mut ed, vec![button(start + egui::vec2(60.0, 0.0), false)]);
        assert_eq!(ed.rev, rev + 1);
        assert!(ed.doc.paths.iter().all(|p| p.stroke_style.arrows.scale_start > 1.0));
        ed.undo();
        assert_eq!(ed.doc, original);
    }

    #[test]
    fn numeric_multi_frame_scrub_publishes_once_and_dash_preserves_other_entries() {
        let mut ed = editor();
        ed.doc.paths[0].stroke_style.dash = vec![6.0, 3.0];
        ed.doc.paths[1].stroke_style.dash = vec![10.0, 8.0];
        let original = ed.doc.clone();
        let rev = ed.rev;
        let mut gesture = StrokeNumberGesture::default();
        for v in [6.2, 6.7, 7.0] {
            assert_eq!(gesture.update(Some(v), None, false, false, true), None);
            assert_eq!(ed.rev, rev);
        }
        let v = gesture.update(None, None, false, false, false).unwrap();
        let mut next = ed.doc.paths[0].stroke_style.clone();
        next.dash[0] = v;
        apply_ops(&mut ed, vec![Op::SetStrokeField(StrokeField::Dash(0), next)]);
        assert_eq!(ed.rev, rev + 1);
        assert_eq!(ed.doc.paths[0].stroke_style.dash, vec![7.0, 3.0]);
        assert_eq!(ed.doc.paths[1].stroke_style.dash, vec![7.0, 8.0]);
        ed.undo();
        assert_eq!(ed.doc, original);
        assert_eq!(gesture.update(None, None, false, false, false), None);
    }

    #[test]
    fn mixed_field_and_reset_each_have_one_undo_and_keep_selection() {
        let mut ed = editor();
        let original = ed.doc.clone();
        let selected = ed.objsel.clone();
        let s = Snap::read(&ed);
        assert!(s.stroke_style_mixed);
        let mut next = s.stroke_style.clone();
        next.join = StrokeJoin::Bevel;
        apply_ops(&mut ed, vec![Op::SetStrokeStyle(s.stroke_style, next)]);
        assert!(ed.doc.paths.iter().all(|p| p.stroke_style.join == StrokeJoin::Bevel));
        assert_eq!(ed.doc.paths[1].stroke_style.dash, [6.0, 3.0]);
        assert_eq!(ed.objsel, selected);
        ed.undo();
        assert_eq!(ed.doc, original);
        apply_ops(&mut ed, vec![Op::ResetStrokeStyle]);
        assert!(ed.doc.paths.iter().all(|p| p.stroke_style.is_default()));
        ed.undo();
        assert_eq!(ed.doc, original);
    }
    #[test]
    fn disclosure_keeps_existing_properties_compact_and_expands_the_kit_controls() {
        let ed = editor();
        let s = Snap::read(&ed);
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        varos_app::shell::tokens::apply(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 1000.0))),
            ..Default::default()
        };
        let mut heights = Vec::new();
        let mut ops = Vec::new();
        for open in [false, true] {
            let _ = ctx.run_ui(input.clone(), |ui| {
                let none = None;
                let align = std::array::from_fn(|_| None);
                let ic = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
                heights.push(
                    ui.scope(|ui| {
                        let disclosure = doc_id(ui, "stroke-style-disclosure");
                        ui.ctx().data_mut(|d| d.insert_temp(disclosure, open));
                        stroke_section(ui, &s, &ic, 250.0, &mut ops)
                    })
                    .response
                    .rect
                    .height(),
                );
            });
        }
        assert!(ops.is_empty());
        assert!(heights[0] <= ICON_BTN_H + PANEL_ITEM_GAP_X);
        assert!(heights[1] > heights[0] + 200.0);
    }
    #[test]
    fn rejected_style_has_a_visible_diagnostic_and_no_partial_edit() {
        let mut ed = editor();
        let original = ed.doc.clone();
        let base = Snap::read(&ed).stroke_style;
        let mut next = base.clone();
        next.dash = vec![0.0001, 0.0001];
        apply_ops(&mut ed, vec![Op::SetStrokeStyle(base, next)]);
        assert_eq!(ed.doc, original);
        assert!(Snap::read(&ed).stroke_error.unwrap().contains("limit_exceeded"));
        ed.replace_doc(original);
        assert!(ed.stroke_error.is_none());
    }
    #[test]
    fn control_bar_opens_the_complete_shared_stroke_controls_headlessly() {
        let ed = editor();
        let s = Snap::read(&ed);
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        varos_app::shell::tokens::apply(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            ..Default::default()
        };
        let mut ops = Vec::new();
        let mut owner = egui::Id::NULL;
        for _ in 0..2 {
            let _ = ctx.run_ui(input.clone(), |ui| {
                ui.horizontal(|ui| {
                    owner = doc_id(ui, "stroke-mirror-options");
                    ui.ctx().data_mut(|d| d.insert_temp(owner, true));
                    let none = None;
                    let align = std::array::from_fn(|_| None);
                    let ic = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
                    stroke_mirror(ui, &s, &ic, &mut ops);
                });
            });
        }
        assert!(ctx.data(|d| d.get_temp::<bool>(owner)).unwrap());
        assert!(ops.is_empty());
    }
    #[test]
    fn choosing_the_representative_value_unifies_mixed_caps_and_swap_uses_each_paths_heads() {
        let mut ed = editor();
        let original = ed.doc.clone();
        let snap = Snap::read(&ed);
        assert!(snap.stroke_field_mixed(StrokeField::Cap));
        apply_ops(&mut ed, vec![Op::SetStrokeField(StrokeField::Cap, snap.stroke_style.clone())]);
        assert!(ed.doc.paths.iter().all(|p| p.stroke_style.cap == snap.stroke_style.cap));
        ed.undo();
        assert_eq!(ed.doc, original);
        ed.doc.paths[0].stroke_style.arrows.start = Some(ArrowHead::Triangle);
        ed.doc.paths[0].stroke_style.arrows.end = Some(ArrowHead::Bar);
        ed.doc.paths[1].stroke_style.arrows.start = Some(ArrowHead::Circle);
        ed.doc.paths[1].stroke_style.arrows.end = Some(ArrowHead::Diamond);
        ed.doc.paths[1].stroke_style.arrows.scale_start = 2.0;
        ed.doc.paths[1].stroke_style.arrows.scale_end = 0.5;
        let original = ed.doc.clone();
        apply_ops(&mut ed, vec![Op::SwapStrokeHeads]);
        assert_eq!(ed.doc.paths[0].stroke_style.arrows.start, Some(ArrowHead::Bar));
        assert_eq!(ed.doc.paths[1].stroke_style.arrows.start, Some(ArrowHead::Diamond));
        assert_eq!(ed.doc.paths[1].stroke_style.arrows.scale_end, 2.0);
        ed.undo();
        assert_eq!(ed.doc, original);
    }
}
