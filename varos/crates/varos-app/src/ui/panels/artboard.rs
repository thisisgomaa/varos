use super::super::*;

/// Page-size presets shown in the artboard panel: (label, w, h) in world points (px == pt @72ppi).
pub(crate) const AB_PRESETS: [(&str, f32, f32); 5] = [
    ("Square \u{2014} 1080", 1080.0, 1080.0),
    ("Screen \u{2014} 1920\u{00d7}1080", 1920.0, 1080.0),
    ("A4 \u{2014} 595\u{00d7}842", 595.0, 842.0),
    ("Letter \u{2014} 612\u{00d7}792", 612.0, 792.0),
    ("Story \u{2014} 1080\u{00d7}1920", 1080.0, 1920.0),
];

/// A change requested by a panel this frame; applied to the editor after layout.
/// The Artboard inspector (Stage 4: the Properties pane's body while the Artboard tool is active).
pub(crate) fn panel_artboard(
    ui: &mut egui::Ui,
    s: &AbSnap,
    ab_lock: &mut bool,
    ops: &mut Vec<Op>,
    fit_request: &mut Option<usize>,
) {
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    let i = s.active;
    egui::ScrollArea::vertical().id_salt("ab-body").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
            let inner = ui.available_width();
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.horizontal(|ui| {
                ui.label(panel_title("Artboard"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // a free canvas (New board) has no artboard: "0 / 0", not "1 / 0"
                    let at = if s.count == 0 { 0 } else { i + 1 };
                    ui.label(RichText::new(format!("{at} / {}", s.count)).color(MUTED).font(numeric_value(11.5)));
                });
            });
            fields::name(ui, inner, &s.name, "dock", ops, |v| Op::AbName(i, v));

            ui.add_space(2.0);
            ui.label(micro_label("SIZE"));
            // preset dropdown
            let entries = AB_PRESETS.iter().map(|(label, _, _)| *label).collect::<Vec<_>>();
            if let Some(index) = kit::text_dropdown(
                ui,
                ui.make_persistent_id("ab-preset"),
                "Presets…",
                &entries,
                inner,
                "Artboard presets",
            ) {
                let (_, w, h) = AB_PRESETS[index];
                ops.push(Op::AbRect(i, None, None, Some(w), Some(h)));
            }
            // W / H + constrain
            ui.horizontal(|ui| {
                let fw = 70.0;
                let lock = *ab_lock;
                fields::num(ui, fw, Lab::Letter("W"), "Width", s.w, 0, 1.0, 1.0..=1.0e6, ops, |v| {
                    Op::AbRect(i, None, None, Some(v), (lock && s.w > 0.0).then(|| s.h * v / s.w))
                });
                fields::num(ui, fw, Lab::Letter("H"), "Height", s.h, 0, 1.0, 1.0..=1.0e6, ops, |v| {
                    Op::AbRect(i, None, None, (lock && s.h > 0.0).then(|| s.w * v / s.h), Some(v))
                });
                if IA_AB_LINK.show(ui, kit::IconState::Toggle(*ab_lock)) {
                    *ab_lock = !*ab_lock;
                }
            });
            // orientation + fit
            ui.horizontal(|ui| {
                let portrait = s.h >= s.w;
                if IA_AB_PORTRAIT.show(ui, kit::IconState::Toggle(portrait)) && !portrait {
                    ops.push(Op::AbOrient(i));
                }
                if IA_AB_LANDSCAPE.show(ui, kit::IconState::Toggle(!portrait)) && portrait {
                    ops.push(Op::AbOrient(i));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if IA_AB_FIT.show(ui, kit::IconState::Action) {
                        *fit_request = Some(i);
                    }
                });
            });
            // X / Y
            ui.horizontal(|ui| {
                let fw = 70.0;
                fields::num(ui, fw, Lab::Letter("X"), "X position", s.x, 0, 1.0, full.clone(), ops, |v| {
                    Op::AbRect(i, Some(v), None, None, None)
                });
                fields::num(ui, fw, Lab::Letter("Y"), "Y position", s.y, 0, 1.0, full.clone(), ops, |v| {
                    Op::AbRect(i, None, Some(v), None, None)
                });
            });

            hsep(ui, inner);
            // page colour + transparent — same hand-painted picker
            ui.horizontal(|ui| {
                let col = s.color;
                let (sw, resp) = ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::click());
                let round = CornerRadius::same(4);
                match col {
                    Some(c) => {
                        if c[3] < 0.999 {
                            checker(&ui.painter_at(sw), sw, 5.0);
                        }
                        ui.painter().rect_filled(sw, round, rgba_c32a(c));
                    }
                    None => {
                        ui.painter().rect_filled(sw, round, SWATCH_WELL);
                        ui.painter().line_segment(
                            [sw.left_bottom() + egui::vec2(2.0, -2.0), sw.right_top() + egui::vec2(-2.0, 2.0)],
                            Stroke::new(1.6, NONE_RED),
                        );
                    }
                }
                ui.painter().rect_stroke(sw, round, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
                // click → the Color Picker panel (a settings row: no focus semantics to preserve)
                if resp.clicked() || resp.double_clicked() {
                    ops.push(Op::OpenPicker(MTarget::Ab(s.id)));
                }
                let _ = col;
                ui.add_space(8.0);
                ui.label(
                    RichText::new(match s.color {
                        Some(c) => hex_of(c),
                        None => "Transparent".into(),
                    })
                    .color(TEXT)
                    .monospace()
                    .size(12.0),
                );
            });
            if IA_TRANSPARENT.show(
                ui,
                if s.count == 0 {
                    kit::IconState::Disabled("No artboard to change")
                } else {
                    kit::IconState::Toggle(s.color.is_none())
                },
            ) {
                ops.push(Op::AbColor(i, if s.color.is_none() { Some([1.0, 1.0, 1.0, 1.0]) } else { None }));
            }

            hsep(ui, inner);
            fields::num(ui, inner, Lab::Letter("#"), "Artboard count", s.count as f32, 0, 0.1, 1.0..=200.0, ops, |v| {
                Op::AbCount(v.round().max(1.0) as usize)
            });
            if IA_CLIP.show(
                ui,
                if s.count == 0 {
                    kit::IconState::Disabled("No artboard to clip")
                } else {
                    kit::IconState::Toggle(s.clip)
                },
            ) {
                ops.push(Op::AbClip(i));
            }
            if IA_MOVE.show(ui, kit::IconState::Toggle(s.move_art)) {
                ops.push(Op::AbMoveArt(!s.move_art));
            }

            hsep(ui, inner);
            // Add · Duplicate · Delete — icon buttons (icon stage 1: they were "+ Add" / "Duplicate" /
            // "Delete" text pills, ICON_LIBRARY_STUDY T23–T25; the words are their tooltips now).
            ui.horizontal(|ui| {
                if IA_AB_ADD.show(ui, kit::IconState::Action) {
                    ops.push(Op::AbAdd);
                }
                let dup = if s.count == 0 {
                    kit::IconState::Disabled("there is no artboard to duplicate")
                } else {
                    kit::IconState::Action
                };
                if IA_AB_DUP.show(ui, dup) {
                    ops.push(Op::AbDup(i));
                }
                let del = if s.count <= 1 {
                    kit::IconState::Disabled("the last artboard can't be deleted")
                } else {
                    kit::IconState::Action
                };
                if IA_AB_DEL.show(ui, del) {
                    ops.push(Op::AbDel(i));
                }
            });
        });
    });
}

/// On-canvas page chrome: a name label (top-left of each page) + a ⋮ button opening the edit menu. The
/// menu is the ungated way to edit a page from ANY tool (Decision 8); selecting a page by clicking its
/// name only works in the Artboard tool. Positions are pinned to each page via the view transform.
#[allow(clippy::too_many_arguments)] // hand-painted panel builder: each arg is live UI state, split deferred with ui.rs
pub(crate) fn build_ab_chrome(
    ctx: &egui::Context,
    view: View,
    ppp: f32,
    hole: egui::Rect,
    abs: &[AbInfo],
    active: usize,
    tool_ab: bool,
    count: usize,
    ops: &mut Vec<Op>,
    name_edit: &mut Option<(usize, String)>,
    fit_request: &mut Option<usize>,
) {
    let mut clear_edit = false;
    for ab in abs {
        if ab.hidden {
            continue; // board eye OFF → the name chrome vanishes with the page
        }
        // Page corners in screen POINTS (view maps world→physical px; egui works in points). The
        // chrome is PINNED to its page — no clamping, no following the viewport (Ahmed 07-07:
        // "عاوزه ثابت زيه زي الأرت بورد") — and CLIPS at the hole so it never paints over rulers,
        // seams or panels. Name + size sit top-LEFT; the ⋯ settings sit top-RIGHT.
        let tl = view.w2s([ab.x, ab.y]);
        let tr = view.w2s([ab.x + ab.w, ab.y]);
        let y = tl[1] / ppp - 24.0;
        let is_active = ab.i == active;
        let out_v = y + 20.0 < hole.top() || y > hole.bottom() - 8.0;

        // ── name + size (top-left): pure INFO — never a click target, never selects (Ahmed 07-07) ──
        let pos = egui::pos2(tl[0] / ppp, y);
        let renaming = matches!(&*name_edit, Some((j, _)) if *j == ab.i);
        if !(out_v || pos.x + 30.0 < hole.left() || pos.x > hole.right() - 8.0) {
            egui::Area::new(egui::Id::new(("ab-chrome", ab.i)))
                .fixed_pos(pos)
                .order(egui::Order::Middle)
                .constrain(false)
                // interactable only while the rename field is up — otherwise pure paint that lets
                // clicks AND scroll pass straight through to the canvas (Ahmed 07-06)
                .interactable(tool_ab && renaming)
                .show(ctx, |ui| {
                    ui.set_clip_rect(hole); // hard edge: nothing bleeds past the board box
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if renaming {
                            if let Some((_, seed)) = name_edit.as_ref() {
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(110.0, 22.0), egui::Sense::hover());
                                let id = doc_id(ui, ("ab-chrome-name", ab.i));
                                if fields::rename(ui, id, rect, seed, 11.5, true, ops, |v| Op::AbName(ab.i, v)) {
                                    clear_edit = true;
                                }
                            }
                        } else {
                            let col = if is_active { TEXT } else { MUTED };
                            ui.label(RichText::new(&ab.name).color(col).size(11.0));
                        }
                        // the page size, quietly beside the name (Ahmed 07-07 "المقاس مكتوب جمبه")
                        ui.label(
                            RichText::new(format!("{:.0} \u{00d7} {:.0}", ab.w, ab.h))
                                .color(MUTED)
                                .font(numeric_value(10.0)),
                        );
                    });
                });
        }

        // ── ⋯ settings (top-right): three HORIZONTAL dots, a touch bigger (Ahmed 07-07) ──
        let dpos = egui::pos2(tr[0] / ppp - 26.0, y);
        if !(out_v || dpos.x + 26.0 < hole.left() || dpos.x > hole.right() - 8.0) {
            egui::Area::new(egui::Id::new(("ab-dots", ab.i)))
                .fixed_pos(dpos)
                .order(egui::Order::Middle)
                .constrain(false)
                .interactable(tool_ab)
                .show(ctx, |ui| {
                    ui.set_clip_rect(hole);
                    let (dr, dresp) = ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::click());
                    if dresp.hovered() {
                        ui.painter().rect_filled(dr, CornerRadius::same(4), HOVER);
                    }
                    let col = if dresp.hovered() || is_active { TEXT } else { MUTED };
                    for k in [-1.0f32, 0.0, 1.0] {
                        ui.painter().circle_filled(egui::pos2(dr.center().x + k * 6.5, dr.center().y), 1.8, col);
                    }
                    let menu_id = ui.make_persistent_id(("ab-menu", ab.i));
                    if dresp.clicked() {
                        menu_toggle(ui, menu_id);
                    }
                    menu_below(ui, menu_id, &dresp, None, |ui| {
                        ui.set_width(190.0);
                        if menu_row(ui, "Rename", "") {
                            *name_edit = Some((ab.i, ab.name.clone()));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, "Duplicate", "") {
                            ops.push(Op::AbDup(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, if ab.h >= ab.w { "Make landscape" } else { "Make portrait" }, "") {
                            ops.push(Op::AbOrient(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, if ab.transparent { "White background" } else { "Transparent" }, "") {
                            ops.push(Op::AbColor(ab.i, if ab.transparent { Some([1.0, 1.0, 1.0, 1.0]) } else { None }));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, if ab.clip { "Unclip" } else { "Clip to page" }, "") {
                            ops.push(Op::AbClip(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                        if menu_row(ui, "Fit in window", "") {
                            *fit_request = Some(ab.i);
                            menu_set(ui, menu_id, false);
                        }
                        if count > 1 && menu_row(ui, "Delete", "") {
                            ops.push(Op::AbDel(ab.i));
                            menu_set(ui, menu_id, false);
                        }
                    });
                });
        }
    }
    if clear_edit {
        *name_edit = None;
    }
}
