//! Lane C provisional kit sheets and Direct Selection corner widgets.
use super::ops::Op;
use crate::app_command::{AppCommand, SessionId};
use egui::Id;
use varos_app::shell::{
    kit::{
        self,
        field::{self, Label, NumberField},
        Control,
    },
    tokens as t,
};
use varos_core::{
    live_corners::{CornerParam, Kind},
    new_document::{Layout, Settings},
    path_advanced::Action,
    EditCommand, Editor, ToolKind, View,
};
fn button(ui: &mut egui::Ui, key: impl std::hash::Hash + std::fmt::Debug, label: &str) -> bool {
    kit::action(ui, Control::new(Id::new(key), label), false).activated
}
fn number(
    ui: &mut egui::Ui,
    key: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let e = field::number_field(
        ui,
        NumberField {
            id: Id::new(key),
            width: t::DOC_SHEET_FIELD_W,
            label: Label::Letter(label),
            tip: label,
            value: *value,
            decimals: 2,
            speed: 1.,
            range,
            disabled: false,
        },
    );
    if let Some(v) = e.commit.or(e.live) {
        *value = v;
        true
    } else {
        false
    }
}
fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(t::PANEL)
        .stroke(egui::Stroke::new(t::KIT_STROKE, t::LINE))
        .corner_radius(t::r_box())
        .inner_margin(t::KIT_PAD)
}
pub(super) fn sheets(
    ctx: &egui::Context,
    commands: &mut Vec<AppCommand>,
    ops: &mut Vec<Op>,
    active: Option<SessionId>,
) {
    if ctx.data(|d| {
        d.get_temp::<Settings>(Id::new("lane-c-new")).is_some()
            || d.get_temp::<(SessionId, f32, varos_core::stroke::StrokeJoin, f32)>(Id::new("lane-c-offset")).is_some()
    }) {
        egui::Area::new(Id::new("lane-c-modal")).order(egui::Order::Foreground).fixed_pos(ctx.content_rect().min).show(
            ctx,
            |ui| {
                ui.allocate_exact_size(ctx.content_rect().size(), egui::Sense::click());
            },
        );
    }
    let id = Id::new("lane-c-new");
    if let Some(mut s) = ctx.data(|d| d.get_temp::<Settings>(id)) {
        let mut close = false;
        egui::Area::new(id).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(
            ctx,
            |ui| {
                frame().show(ui, |ui| {
                    ui.set_width(t::DOC_SHEET_W);
                    ui.label(t::panel_title("New Document"));
                    ui.horizontal(|ui| {
                        for (i, label) in ["Print", "Web", "Mobile", "Social"].iter().enumerate() {
                            if button(ui, ("new-category", i), label) {
                                s = Settings::category(i);
                            }
                        }
                    });
                    number(ui, "new-width", "Width", &mut s.width, 0.01..=1e6);
                    number(ui, "new-height", "Height", &mut s.height, 0.01..=1e6);
                    if button(ui, "new-unit", s.units.label()) {
                        s.units = s.units.cycle();
                    }
                    let mut count = s.count as f32;
                    number(ui, "new-count", "Artboards", &mut count, 1.0..=100.0);
                    s.count = count.round() as usize;
                    ui.horizontal(|ui| {
                        for (layout, label) in
                            [(Layout::Grid, "Grid"), (Layout::Row, "Row"), (Layout::Column, "Column")]
                        {
                            if button(ui, ("new-layout", label), label) {
                                s.layout = layout;
                            }
                        }
                    });
                    let mut columns = s.columns as f32;
                    number(ui, "new-columns", "Columns", &mut columns, 1.0..=100.0);
                    s.columns = columns.round() as usize;
                    number(ui, "new-spacing", "Spacing", &mut s.spacing, 0.0..=7200.0);
                    number(ui, "new-bleed", "Bleed", &mut s.bleed, 0.0..=7200.0);
                    number(ui, "new-ppi", "Raster effects ppi", &mut s.ppi, 1.0..=9600.0);
                    kit::notice(ui, "RGB");
                    if let Err(reason) = s.document() {
                        kit::notice(ui, &reason);
                    }
                    ui.horizontal(|ui| {
                        if button(ui, "new-cancel", "Cancel") {
                            close = true;
                        }
                        if button(ui, "new-create", "Create") && s.document().is_ok() {
                            commands.push(AppCommand::CreateDocument(s.clone()));
                            close = true;
                        }
                    });
                    kit::notice(ui, "Quick presets");
                    ui.horizontal_wrapped(|ui| {
                        for preset in varos_core::board::PRESETS {
                            if button(ui, ("quick-preset", preset.id), preset.label) {
                                commands.push(AppCommand::NewWithPreset(preset.id));
                                close = true;
                            }
                        }
                    });
                });
            },
        );
        close |= !field::any_open(ctx) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        ctx.data_mut(|d| {
            if close {
                d.remove::<Settings>(id);
            } else {
                d.insert_temp(id, s);
            }
        });
    }
    let id = Id::new("lane-c-offset");
    if let Some((sid, mut delta, mut join, mut miter)) =
        ctx.data(|d| d.get_temp::<(SessionId, f32, varos_core::stroke::StrokeJoin, f32)>(id))
    {
        if active != Some(sid) {
            ctx.data_mut(|d| d.remove::<(SessionId, f32, varos_core::stroke::StrokeJoin, f32)>(id));
            return;
        }
        let mut close = false;
        egui::Area::new(id).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(
            ctx,
            |ui| {
                frame().show(ui, |ui| {
                    ui.label(t::panel_title("Offset Path"));
                    number(ui, "offset-delta", "Offset", &mut delta, -7200.0..=7200.0);
                    ui.horizontal(|ui| {
                        for (value, label) in [
                            (varos_core::stroke::StrokeJoin::Round, "Round"),
                            (varos_core::stroke::StrokeJoin::Miter, "Miter"),
                            (varos_core::stroke::StrokeJoin::Bevel, "Bevel"),
                        ] {
                            if button(ui, ("offset-join", label), label) {
                                join = value;
                            }
                        }
                    });
                    number(ui, "offset-miter", "Miter limit", &mut miter, 1.0..=1000.0);
                    if button(ui, "offset-apply", "Apply") {
                        ops.push(Op::DocumentSetup(EditCommand::PathAdvanced(Action::Offset { delta, join, miter })));
                        close = true;
                    }
                    if button(ui, "offset-cancel", "Cancel") {
                        close = true;
                    }
                });
            },
        );
        close |= !field::any_open(ctx) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        ctx.data_mut(|d| {
            if close {
                d.remove::<(SessionId, f32, varos_core::stroke::StrokeJoin, f32)>(id);
            } else {
                d.insert_temp(id, (sid, delta, join, miter));
            }
        });
    }
}
pub(super) fn corners(ctx: &egui::Context, ed: &mut Editor, view: &View, ppp: f32, hole: egui::Rect) {
    if ed.tool != ToolKind::Direct {
        return;
    }
    let ids: Vec<_> = ed.selected_pids().into_iter().collect();
    for pid in ids {
        let Some(pi) = ed.doc.pidx(pid) else { continue };
        let path = ed.doc.paths[pi].clone();
        let xf = ed.doc.unit_xform(pid);
        for corner in varos_core::live_corners::corners(&path) {
            let param = path.corners.get(corner.index).copied().unwrap_or_default();
            let at = corner.centre(if param.radius > 0. {
                param.radius
            } else {
                (t::LIVE_CORNER_OFFSET * ppp / view.zoom).min(corner.max_radius as f32)
            });
            let screen = view.w2s(xf.apply(at));
            let pos = egui::pos2(screen[0] / ppp, screen[1] / ppp);
            if !hole.contains(pos) {
                continue;
            }
            let id = Id::new(("corner-widget", pid, corner.index));
            let response = egui::Area::new(id)
                .order(egui::Order::Foreground)
                .fixed_pos(pos - egui::vec2(t::LIVE_CORNER_RADIUS, t::LIVE_CORNER_RADIUS))
                .show(ctx, |ui| {
                    let (rect, r) = ui.allocate_exact_size(
                        egui::vec2(t::LIVE_CORNER_RADIUS * 2., t::LIVE_CORNER_RADIUS * 2.),
                        egui::Sense::click_and_drag(),
                    );
                    ui.painter().circle_stroke(
                        rect.center(),
                        t::LIVE_CORNER_RADIUS,
                        egui::Stroke::new(t::KIT_STROKE, t::ACCENT),
                    );
                    r
                })
                .inner;
            if response.drag_started() {
                ed.begin();
                ctx.data_mut(|d| d.insert_temp(id.with("initial"), param.radius));
            }
            if response.dragged() {
                let initial = ctx.data(|d| d.get_temp::<f32>(id.with("initial"))).unwrap_or(param.radius);
                let delta = response.drag_delta() * ppp / view.zoom;
                let origin = xf.inverse_apply([0., 0.]);
                let local = xf.inverse_apply([delta.x, delta.y]);
                let radius = corner.drag_radius([local[0] - origin[0], local[1] - origin[1]], initial);
                let mut params = path.corners.clone();
                params.resize(path.anchors.len(), CornerParam::default());
                params[corner.index].radius = radius;
                ed.execute_ui(EditCommand::SetCornersLive { path: pid, corners: params });
            }
            if response.drag_stopped() {
                ed.finish_document_setup();
            }
            if response.clicked() {
                ctx.data_mut(|d| d.insert_temp(Id::new("corner-field"), (pid, corner.index)));
            }
        }
    }
    let id = Id::new("corner-field");
    if let Some((pid, index)) = ctx.data(|d| d.get_temp::<(u32, usize)>(id)) {
        let Some(pi) = ed.doc.pidx(pid) else { return };
        let path = ed.doc.paths[pi].clone();
        if index >= path.anchors.len() {
            return;
        }
        let mut p = path.corners.get(index).copied().unwrap_or_default();
        let mut changed = false;
        let mut close = false;
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(hole.right_top() - egui::vec2(t::DOC_SHEET_W, t::KIT_PAD))
            .show(ctx, |ui| {
                frame().show(ui, |ui| {
                    changed |= number(ui, "corner-radius", "Radius", &mut p.radius, 0.0..=1e6);
                    ui.horizontal(|ui| {
                        for (kind, label) in
                            [(Kind::Round, "Round"), (Kind::Inverted, "Inverted"), (Kind::Chamfer, "Chamfer")]
                        {
                            if button(ui, ("corner-kind", label), label) {
                                p.kind = kind;
                                changed = true;
                            }
                        }
                    });
                    close = button(ui, "corner-done", "Done");
                });
            });
        if changed {
            let mut params = path.corners.clone();
            params.resize(path.anchors.len(), CornerParam::default());
            params[index] = p;
            ed.execute_ui(EditCommand::SetCorners { path: pid, corners: params });
        }
        if close {
            ctx.data_mut(|d| d.remove::<(u32, usize)>(id));
        }
    }
}

impl super::Ui {
    pub fn lane_c_new_document(&mut self) {
        self.ctx.data_mut(|d| d.insert_temp(Id::new("lane-c-new"), Settings::default()));
    }
    pub fn lane_c_offset(&mut self, sid: SessionId) {
        self.ctx.data_mut(|d| {
            d.insert_temp(Id::new("lane-c-offset"), (sid, 10.0f32, varos_core::stroke::StrokeJoin::Round, 10.0f32))
        });
    }
}

pub(super) fn scale_strokes(ui: &mut egui::Ui, on: bool, ops: &mut Vec<Op>) {
    let mut c = Control::new(Id::new("scale-strokes"), "Scale strokes");
    c.selected = on;
    c.icon = Some(kit::Icon::Link);
    if kit::action(ui, c, true).activated {
        ops.push(Op::DocumentSetup(EditCommand::SetScaleStrokes(!on)));
    }
}
