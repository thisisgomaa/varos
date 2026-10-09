//! Lane C provisional kit sheets and Direct Selection corner widgets.
// ---- Lane F: text adapters ----
use varos_app::shell::kit::text::ShapedUi as _;
// ---- end Lane F ----
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
    let r = kit::action(ui, Control::new(Id::new(key), label), false);
    #[cfg(test)]
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(("lane-c-test-button", label)), r.response.rect));
    r.activated
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
                    ui.shaped_label(t::panel_title("New Document"));
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
                        s.set_units(s.units.cycle());
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
                    ui.shaped_label(t::panel_title("Offset Path"));
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
pub(super) fn corners(
    ctx: &egui::Context,
    ed: &mut Editor,
    view: &View,
    ppp: f32,
    hole: egui::Rect,
    sid: Option<SessionId>,
) {
    if ed.tool != ToolKind::Direct {
        return;
    }
    let ids: Vec<_> = ed.selected_pids().into_iter().collect();
    for pid in ids {
        if ed.doc.eff_locked(pid) || ed.doc.eff_hidden(pid) || !ed.in_isolation(pid) {
            continue;
        }
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
            let id = Id::new(("corner-widget", sid, pid, corner.index));
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
            #[cfg(test)]
            ctx.data_mut(|d| d.insert_temp(Id::new(("lane-c-test-corner", corner.index)), response.rect));
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
                ctx.data_mut(|d| d.insert_temp(Id::new(("corner-field", sid)), (pid, corner.index)));
            }
        }
    }
    let id = Id::new(("corner-field", sid));
    if let Some((pid, index)) = ctx.data(|d| d.get_temp::<(u32, usize)>(id)) {
        if !ed.selected_pids().contains(&pid) || ed.doc.eff_locked(pid) || ed.doc.eff_hidden(pid) {
            ctx.data_mut(|d| d.remove::<(u32, usize)>(id));
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        t::apply(&ctx);
        ctx
    }
    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 900.))),
            events,
            ..Default::default()
        }
    }
    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    #[test]
    fn new_sheet_create_emits_settings_document_and_keeps_quick_presets() {
        let ctx = context();
        ctx.data_mut(|d| d.insert_temp(Id::new("lane-c-new"), Settings { count: 3, bleed: 3., ..Default::default() }));
        let mut commands = vec![];
        for _ in 0..2 {
            let _ = ctx.run_ui(input(vec![]), |ui| sheets(ui.ctx(), &mut commands, &mut vec![], None));
        }
        let pos = ctx.data(|d| d.get_temp::<egui::Rect>(Id::new(("lane-c-test-button", "Create")))).unwrap().center();
        for pressed in [true, false] {
            let _ = ctx.run_ui(input(vec![egui::Event::PointerMoved(pos), pointer(pos, pressed)]), |ui| {
                sheets(ui.ctx(), &mut commands, &mut vec![], None)
            });
        }
        let AppCommand::CreateDocument(s) = commands.remove(0) else { panic!("Create command missing") };
        assert_eq!(s.document().unwrap().artboards.len(), 3);
        assert!(s.document().unwrap().artboards[0].bleed > 0.);
        assert!(ctx.data(|d| d.get_temp::<Settings>(Id::new("lane-c-new"))).is_none());
        assert!(ctx
            .data(|d| d.get_temp::<egui::Rect>(Id::new(("lane-c-test-button", varos_core::board::PRESETS[0].label))))
            .is_some());
    }
    #[test]
    fn corner_widget_drag_commits_one_undo_and_keeps_authored_anchors() {
        let ctx = context();
        let mut ed = Editor::new();
        let pid = ed
            .try_execute_created(EditCommand::AddShape {
                kind: varos_core::model::ShapeKind::Rect,
                bounds: [100., 100., 100., 80.],
                parent: None,
                fill: Some([1.; 4]),
                stroke: None,
                stroke_width: 0.,
                opacity: 1.,
                name: None,
            })
            .unwrap();
        ed.set_tool(ToolKind::Direct);
        ed.dsel_path = Some(pid);
        let original = ed.doc.clone();
        let view = View { pan: [0., 0.], zoom: 1. };
        let hole = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 900.));
        for _ in 0..2 {
            let _ = ctx.run_ui(input(vec![]), |ui| corners(ui.ctx(), &mut ed, &view, 1., hole, Some(SessionId(1))));
        }
        let pos = ctx.data(|d| d.get_temp::<egui::Rect>(Id::new(("lane-c-test-corner", 0usize)))).unwrap().center();
        for events in [
            vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
            vec![egui::Event::PointerMoved(pos + egui::vec2(12., 12.))],
            vec![pointer(pos + egui::vec2(12., 12.), false)],
        ] {
            let _ = ctx.run_ui(input(events), |ui| corners(ui.ctx(), &mut ed, &view, 1., hole, Some(SessionId(1))));
        }
        assert!(ed.doc.paths[0].corners[0].radius > 0.);
        assert_eq!(ed.doc.paths[0].anchors, original.paths[0].anchors);
        ed.execute(EditCommand::Undo).unwrap();
        assert_eq!(ed.doc, original);
    }
}
