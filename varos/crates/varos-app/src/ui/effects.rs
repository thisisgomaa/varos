//! Provisional Phase 10 sheets and Width tool overlays; existing kit and tokens only.
use crate::app_command::SessionId;
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
    effects::{Action, Effect, WarpStyle},
    stroke::StrokeJoin,
    width_profile::WidthProfile,
    EditCommand, Editor, ToolKind, View,
};
const SHEET: &str = "w3-effect-sheet";
const LAST: &str = "w3-effect-last";
#[derive(Clone)]
struct Draft {
    sid: SessionId,
    ids: Vec<u32>,
    effect: Effect,
    revision: u64,
    error: Option<String>,
}
pub(crate) const WARPS: [(WarpStyle, &str); 15] = [
    (WarpStyle::Arc, "Arc…"),
    (WarpStyle::ArcLower, "Arc Lower…"),
    (WarpStyle::ArcUpper, "Arc Upper…"),
    (WarpStyle::Arch, "Arch…"),
    (WarpStyle::Bulge, "Bulge…"),
    (WarpStyle::ShellLower, "Shell Lower…"),
    (WarpStyle::ShellUpper, "Shell Upper…"),
    (WarpStyle::Flag, "Flag…"),
    (WarpStyle::Wave, "Wave…"),
    (WarpStyle::Fish, "Fish…"),
    (WarpStyle::Rise, "Rise…"),
    (WarpStyle::Fisheye, "Fisheye…"),
    (WarpStyle::Inflate, "Inflate…"),
    (WarpStyle::Squeeze, "Squeeze…"),
    (WarpStyle::Twist, "Twist…"),
];
fn button(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, label: &str) -> bool {
    kit::action(ui, Control::new(Id::new(id), label), false).activated
}
fn number(ui: &mut egui::Ui, id: &str, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let e = field::number_field(
        ui,
        NumberField {
            id: Id::new(("w3-effects", id)),
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
    if let Some(v) = e.commit.or(e.live).or(e.pending) {
        *value = v;
        true
    } else {
        false
    }
}
fn default_effect(name: &str) -> Option<Effect> {
    Some(match name {
        "Offset…" => Effect::Offset { delta: 10., join: StrokeJoin::Round, miter: 10. },
        "Zig Zag…" => Effect::ZigZag { size: 5., ridges: 4, smooth: false },
        "Transform Effect…" => {
            Effect::Transform { copies: 0, movement: [0., 0.], scale: [1., 1.], rotate: 0., reflect: [false, false] }
        }
        _ => Effect::Warp { style: WARPS.iter().find(|(_, n)| *n == name)?.0, bend: 50., h: 0., v: 0. },
    })
}
pub(crate) fn menu(ctx: &egui::Context, ed: &mut Editor, sid: SessionId, name: &str) -> bool {
    if name == "Width Tool" {
        ed.execute_ui(EditCommand::LiveEffects(Action::Tool));
        return true;
    }
    if name == "Width Profile…" {
        ctx.data_mut(|d| d.insert_temp(Id::new("w3-width-presets"), sid));
        return true;
    }
    let effect = if name == "Apply Last Effect" || name == "Last Effect…" {
        ctx.data(|d| d.get_temp::<Effect>(Id::new(LAST)))
    } else {
        default_effect(name)
    };
    let Some(effect) = effect else { return name == "Apply Last Effect" || name == "Last Effect…" };
    let ids: Vec<_> = ed.selected_pids().into_iter().collect();
    if ids.is_empty() || ids.len() > 1000 {
        ed.stroke_error = Some("Select paths for the effect sheet.".into());
        return true;
    }
    settle(ctx, ed);
    if name == "Apply Last Effect" {
        let paths = varos_core::effects_preview::appended(ed, &ids, &effect);
        ed.execute_ui(EditCommand::LiveEffects(Action::SetPerPath { paths }));
        return true;
    }
    ctx.data_mut(|d| d.insert_temp(Id::new(SHEET), Draft { sid, ids, effect, revision: ed.rev, error: None }));
    true
}
pub(super) fn settle(ctx: &egui::Context, ed: &mut Editor) {
    ed.execute_ui(EditCommand::LiveEffects(Action::EndPreview { accept: false }));
    ctx.data_mut(|d| {
        d.remove::<Draft>(Id::new(SHEET));
        d.remove::<SessionId>(Id::new("w3-width-presets"));
    });
}
pub(super) fn sheet(ctx: &egui::Context, ed: &mut Editor, active: Option<SessionId>) {
    if let Some(sid) = ctx.data(|d| d.get_temp::<SessionId>(Id::new("w3-width-presets"))) {
        if Some(sid) != active {
            ctx.data_mut(|d| d.remove::<SessionId>(Id::new("w3-width-presets")));
            return;
        }
        let mut close = false;
        egui::Area::new(Id::new("w3-width-sheet"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                frame().show(ui, |ui| {
                    ui.label(t::panel_title("Width Profile"));
                    for preset in WidthProfile::PRESETS {
                        if button(ui, ("w3-profile", preset.id), preset.label) {
                            let profile = if preset.id == "uniform" { None } else { WidthProfile::preset(preset.id) };
                            ed.execute_ui(EditCommand::LiveEffects(Action::Width {
                                ids: ed.selected_pids().into_iter().collect(),
                                profile,
                            }));
                            close = true;
                        }
                    }
                    if button(ui, "w3-profile-cancel", "Cancel") {
                        close = true
                    }
                });
            });
        if close {
            ctx.data_mut(|d| d.remove::<SessionId>(Id::new("w3-width-presets")));
        }
    }
    let Some(mut draft) = ctx.data(|d| d.get_temp::<Draft>(Id::new(SHEET))) else { return };
    if Some(draft.sid) != active || draft.revision != ed.rev {
        settle(ctx, ed);
        return;
    }
    let mut done = None;
    egui::Area::new(Id::new("w3-effect-modal")).order(egui::Order::Foreground).fixed_pos(ctx.content_rect().min).show(
        ctx,
        |ui| {
            ui.allocate_exact_size(ctx.content_rect().size(), egui::Sense::click());
        },
    );
    egui::Area::new(Id::new(SHEET))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            frame().show(ui, |ui| {
                ui.set_width(t::DOC_SHEET_W);
                ui.label(t::panel_title("Live Effect"));
                match &mut draft.effect {
                    Effect::Offset { delta, join, miter } => {
                        number(ui, "delta", "Offset", delta, -7200. ..=7200.);
                        for (j, label) in
                            [(StrokeJoin::Round, "Round"), (StrokeJoin::Miter, "Miter"), (StrokeJoin::Bevel, "Bevel")]
                        {
                            if button(ui, ("w3-join", label), label) {
                                *join = j;
                            }
                        }
                        number(ui, "miter", "Miter limit", miter, 1. ..=1000.);
                    }
                    Effect::ZigZag { size, ridges, smooth } => {
                        number(ui, "size", "Size", size, -10000. ..=10000.);
                        let mut r = *ridges as f32;
                        number(ui, "ridges", "Ridges", &mut r, 0. ..=100.);
                        *ridges = r.round() as u32;
                        if button(ui, "smooth", if *smooth { "Smooth points" } else { "Corner points" }) {
                            *smooth = !*smooth;
                        }
                    }
                    Effect::Transform { copies, movement, scale, rotate, reflect } => {
                        let mut c = *copies as f32;
                        number(ui, "copies", "Copies", &mut c, 0. ..=1000.);
                        *copies = c.round() as u32;
                        number(ui, "move-x", "Move X", &mut movement[0], -1e6..=1e6);
                        number(ui, "move-y", "Move Y", &mut movement[1], -1e6..=1e6);
                        number(ui, "scale-x", "Scale X", &mut scale[0], -10. ..=10.);
                        number(ui, "scale-y", "Scale Y", &mut scale[1], -10. ..=10.);
                        number(ui, "rotate", "Rotate", rotate, -36000. ..=36000.);
                        for (i, label) in ["Reflect X", "Reflect Y"].iter().enumerate() {
                            let mut control = Control::new(Id::new(("w3-reflect", i)), label);
                            control.selected = reflect[i];
                            if kit::action(ui, control, true).activated {
                                reflect[i] = !reflect[i];
                            }
                        }
                    }
                    Effect::Warp { style, bend, h, v } => {
                        let label = WARPS.iter().find(|(s, _)| s == style).map_or("Warp", |(_, n)| *n);
                        kit::notice(ui, label);
                        number(ui, "bend", "Bend %", bend, -100. ..=100.);
                        number(ui, "h", "Horizontal %", h, -100. ..=100.);
                        number(ui, "v", "Vertical %", v, -100. ..=100.);
                    }
                }
                if let Some(error) = &draft.error {
                    kit::notice(ui, error);
                }
                ui.horizontal(|ui| {
                    if button(ui, "w3-effect-cancel", "Cancel") {
                        done = Some(false)
                    }
                    if button(ui, "w3-effect-ok", "OK") {
                        done = Some(true)
                    }
                });
            });
        });
    let command =
        EditCommand::LiveEffects(Action::PreviewAppend { ids: draft.ids.clone(), effect: draft.effect.clone() });
    draft.error = ed.try_execute(command).err();
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        done = Some(false)
    }
    if let Some(accept) = done {
        if accept && draft.error.is_some() {
            ctx.data_mut(|d| d.insert_temp(Id::new(SHEET), draft));
            return;
        }
        ed.execute_ui(EditCommand::LiveEffects(Action::EndPreview { accept }));
        ctx.data_mut(|d| {
            d.remove::<Draft>(Id::new(SHEET));
            if accept {
                d.insert_temp(Id::new(LAST), draft.effect);
            }
        });
    } else {
        ctx.data_mut(|d| d.insert_temp(Id::new(SHEET), draft));
    }
}
fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(t::PANEL)
        .stroke(egui::Stroke::new(t::KIT_STROKE, t::LINE))
        .corner_radius(t::r_box())
        .inner_margin(t::KIT_PAD)
}
pub(super) fn width_points(ctx: &egui::Context, ed: &Editor, view: &View, ppp: f32, hole: egui::Rect) {
    if ed.tool != ToolKind::Width {
        return;
    }
    let painter =
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("w3-width-points"))).with_clip_rect(hole);
    for id in ed.selected_pids() {
        let Some(i) = ed.doc.pidx(id) else { continue };
        let p = &ed.doc.paths[i];
        let profile =
            p.stroke_style.width_profile.clone().unwrap_or_else(|| WidthProfile::preset("uniform").unwrap_or_default());
        for (t0, l, r) in profile.points {
            if let Some((c, n)) = varos_core::width_tool::location(ed, id, t0) {
                let screen = |p: [f32; 2]| {
                    egui::pos2((view.pan[0] + p[0] * view.zoom) / ppp, (view.pan[1] + p[1] * view.zoom) / ppp)
                };
                let left =
                    [c[0] + n[0] * p.stroke_width * 0.5 * l as f32, c[1] + n[1] * p.stroke_width * 0.5 * l as f32];
                let right =
                    [c[0] - n[0] * p.stroke_width * 0.5 * r as f32, c[1] - n[1] * p.stroke_width * 0.5 * r as f32];
                painter.line_segment([screen(left), screen(right)], egui::Stroke::new(t::KIT_STROKE, t::ACCENT));
                for q in [left, c, right] {
                    painter.circle_filled(screen(q), t::KIT_PAD, t::ACCENT);
                }
            }
        }
    }
}
impl super::Ui {
    pub(crate) fn effects_menu(&mut self, ed: &mut Editor, sid: SessionId, name: &str) -> bool {
        menu(&self.ctx, ed, sid, name)
    }
}
pub(crate) fn effects_menu_rows() -> Vec<crate::menus::Entry> {
    WARPS
        .iter()
        .enumerate()
        .map(|(i, (_, name))| crate::menus::Entry::Item {
            id: format!("effect.warp.{i}"),
            label: name,
            accel: None,
            cmd: crate::menus::MenuCmd::LaneC(name),
            check: None,
        })
        .collect()
}
