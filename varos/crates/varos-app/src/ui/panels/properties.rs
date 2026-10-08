use super::super::*;

/// The Properties pane body (Stage 4): the old inspector dock re-housed inside the
/// [Properties|Layers] box — Transform · Appearance · Shape (a Pathfinder MIRROR).
/// Align moved out to its own panel (ONE-HOME rule).
pub(crate) fn panel_properties(
    ui: &mut egui::Ui,
    s: &Snap,
    ic: &DockIcons,
    refpt: &mut (f32, f32),
    lock: &mut bool,
    ops: &mut Vec<Op>,
    recovery: (&crate::recovery_host::RecoveryUi, &mut Vec<AppCommand>),
) {
    let full = std::ops::RangeInclusive::new(-1.0e6_f32, 1.0e6_f32);
    egui::ScrollArea::vertical().id_salt("props-body").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
            let inner = ui.available_width();
            ui.spacing_mut().item_spacing = egui::vec2(PANEL_ITEM_GAP_X, 5.0);
            #[cfg(test)]
            let content_top = ui.cursor().top();

            // Pain A15: nothing to inspect (no object, no Direct/anchor path, not mid-draft) → the compact
            // Document settings home instead of a transform panel full of zeros. Returns from THIS closure.
            if !s.sel && !s.drawing && !s.has_paint {
                document_section(ui, s, inner, ops, recovery);
                return;
            }

            let measured = s.sel || s.direct; // real numbers below (objects, or a Direct selection — Astra F07)
            ui.label(if measured { panel_title(&s.name) } else { panel_title(&s.name).color(MUTED) });
            ui.add_space(2.0);
            ui.label(micro_label("TRANSFORM"));
            label_gap(ui);

            // ── Transform block: [9-pt refpoint] [X/W · Y/H] [link] ──
            ui.horizontal(|ui| {
                refpoint(ui, TRANSFORM_REFPOINT_SIZE, refpt);
                let (ax, ay) = *refpt;
                let fw = 66.0;
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // A7: the refpoint X offset uses the WORLD AABB width (s.world_w), not the local W
                        // (s.w) — on a rotated object those differ and only the world dim gives the true
                        // on-screen reference-point position. `set_obj_bbox` reads/writes the same point.
                        let dx = s.x + ax * s.world_w;
                        fields::num(ui, fw, Lab::Letter("X"), "X position", dx, 0, 1.0, full.clone(), ops, |v| {
                            Op::SetBBox(Some(v), None, None, None, ax, ay)
                        });
                        let linked = *lock;
                        dim_field(ui, fw, true, s.w, s.direct, ops, |v| {
                            Op::SetBBox(None, None, Some(v), (linked && s.w > 0.0).then(|| s.h * v / s.w), ax, ay)
                        });
                    });
                    ui.horizontal(|ui| {
                        let dy = s.y + ay * s.world_h; // A7: world AABB height, matching the X field above
                        fields::num(ui, fw, Lab::Letter("Y"), "Y position", dy, 0, 1.0, full.clone(), ops, |v| {
                            Op::SetBBox(None, Some(v), None, None, ax, ay)
                        });
                        let linked = *lock;
                        dim_field(ui, fw, false, s.h, s.direct, ops, |v| {
                            Op::SetBBox(None, None, (linked && s.h > 0.0).then(|| s.w * v / s.h), Some(v), ax, ay)
                        });
                    });
                });
                if IA_PROP_LINK.show(ui, kit::IconState::Toggle(*lock)) {
                    *lock = !*lock;
                }
            });

            // ── Angle + flip ──
            // Rotate/flip act on OBJECTS only; for a Direct selection (Astra F07) they would silently do
            // nothing, so they are shown disabled with the reason on the rotation field's tooltip.
            ui.horizontal(|ui| {
                let (rot_tip, rot) = if s.direct {
                    ("Rotation: select the whole object (Selection tool, V) to rotate or flip", 0.0)
                } else {
                    ("Rotation", s.rot)
                };
                fields::num_disabled(
                    ui,
                    150.0,
                    Lab::Icon(ic.rotate.as_ref()),
                    rot_tip,
                    rot,
                    1,
                    0.5,
                    full.clone(),
                    s.direct,
                    ops,
                    Op::SetRot,
                );
                let flip_state = if s.direct {
                    kit::IconState::DisabledReason("Select the whole object (Selection tool, V) to rotate or flip")
                } else {
                    kit::IconState::Action
                };
                if IA_FLIP_H.show(ui, flip_state) {
                    ops.push(Op::Flip(true));
                }
                if IA_FLIP_V.show(ui, flip_state) {
                    ops.push(Op::Flip(false));
                }
            });

            hsep(ui, inner);

            // Appearance: opacity
            fields::num(
                ui,
                inner,
                Lab::Icon(ic.opacity.as_ref()),
                "Opacity %",
                s.opacity * 100.0,
                0,
                0.5,
                0.0..=100.0,
                ops,
                |v| Op::SetOpacity(v / 100.0),
            );

            hsep(ui, inner);

            // Fill / Stroke swatches + stroke weight
            paint_row(ui, PaintTarget::Fill, s.fill, s.fill_mixed, ops);
            paint_row(ui, PaintTarget::Stroke, s.stroke, s.stroke_mixed, ops);
            ui.horizontal(|ui| {
                let id = doc_id(ui, "stroke-weight-presets");
                let weights = [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0];
                let entries = ["0.25 pt", "0.5 pt", "1 pt", "2 pt", "3 pt", "4 pt", "6 pt", "8 pt", "12 pt"]
                    .map(kit::MenuEntry::Item);
                if let Some(index) = kit::menu(ui.ctx(), id, &entries) {
                    ops.push(Op::SetStrokeW(weights[index]));
                }
                fields::num(
                    ui,
                    inner - ICON_BTN_W - PANEL_ITEM_GAP_X,
                    Lab::Icon(ic.strokew.as_ref()),
                    "Stroke weight",
                    s.sw,
                    1,
                    0.2,
                    0.0..=400.0,
                    ops,
                    Op::SetStrokeW,
                );
                let r = IA_STROKE_PRESETS.show_response(ui, kit::IconState::Action);
                if r.activated {
                    kit::toggle_menu_below(ui.ctx(), id, r.response.rect);
                    let _ = kit::menu(ui.ctx(), id, &entries);
                }
            });

            hsep(ui, inner);
            ui.label(micro_label("SHAPE"));
            label_gap(ui);
            pathfinder_row(ui, ops, false, s.pathfinder); // a MIRROR of the Pathfinder home (the mockup's Shape section) — roomy dock size

            // A30 — per-element release from artboard clip. Shown only when an object is selected AND
            // some board clips (otherwise the toggle would do nothing visible). ON = clipped to the
            // board (the normal state); OFF = released, so this one element bleeds outside like Illustrator.
            // No canvas right-click menu exists yet, so the Properties dock is where this lives.
            if s.sel && s.any_clip {
                hsep(ui, inner);
                ui.label(micro_label("ARTBOARD CLIP"));
                label_gap(ui);
                if IA_OBJECT_CLIP.show(ui, kit::IconState::Toggle(!s.clip_exempt)) {
                    ops.push(Op::SetClipExempt(!s.clip_exempt));
                }
            }
            #[cfg(test)]
            property_height_probes::record(ui.min_rect().bottom() - content_top);
        });
    });
}

/// The Board section (Start v2 L5) — the ONE home for the board's own metadata, at the top of the
/// Document settings (nothing selected): Name, Description (3 wrapping rows), Tags (removable chips +
/// an inline input; comma or Enter adds, Backspace on an empty input removes the last). Every field is
/// a kit field under the K3 law (commit on blur / Enter / Tab, Esc reverts, the reason inline when the
/// core refuses the text); each commit is one undo step and makes the document dirty. Labels MUTED 12,
/// values 13 — the Start card's tag pills on the Properties rows.
pub(crate) fn board_section(ui: &mut egui::Ui, s: &Snap, w: f32, ops: &mut Vec<Op>) {
    ui.label(micro_label("BOARD"));
    label_gap(ui);
    for (label, field) in [("Name", 0), ("Description", 1), ("Tags", 2)] {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(w, varos_app::shell::tokens::BOARD_LABEL_H), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            label,
            // the proportional family is Inter 400 (lane L1): the `small` role, without needing a named face
            FontId::proportional(varos_app::shell::tokens::small().size),
            MUTED,
        );
        match field {
            0 => fields::board_name(ui, w, &s.board_name, ops),
            1 => fields::board_description(ui, w, &s.board_description, ops),
            _ => fields::board_tags(ui, w, &s.board_tags, ops),
        }
        ui.add_space(varos_app::shell::tokens::BOARD_GAP);
    }
}

#[cfg(test)]
pub(crate) mod property_height_probes {
    use std::cell::Cell;

    thread_local! {
        static HEIGHT: Cell<f32> = const { Cell::new(0.0) };
    }

    pub(crate) fn record(height: f32) {
        HEIGHT.with(|value| value.set(height));
    }

    pub(crate) fn get() -> f32 {
        HEIGHT.with(Cell::get)
    }
}
