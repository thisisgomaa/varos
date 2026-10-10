//! Lane E: provisional native kit options; owner design review pending.
use super::ops::Op;
use crate::{
    app_command::SessionId,
    menus::{Entry, MenuCmd},
};
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
    live::{self, Action, Axis, Envelope, Kind, Orientation, Repeat, Warp},
    EditCommand, Editor, ToolKind,
};
#[derive(Clone)]
struct Draft {
    sid: SessionId,
    action: Action,
}
fn button(ui: &mut egui::Ui, label: &str) -> bool {
    kit::action(ui, Control::new(ui.id().with(label), label), false).activated
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    let r = field::number_field(
        ui,
        NumberField {
            id: ui.id().with(label),
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
    if let Some(v) = r.commit.or(r.live) {
        *value = v;
    }
}
fn count(ui: &mut egui::Ui, label: &str, value: &mut u16, min: f32) {
    let mut v = f32::from(*value);
    number(ui, label, &mut v, min..=1000.0);
    *value = v.round() as u16;
}
fn make(ed: &Editor, kind: Kind) -> Action {
    Action::Make { paths: ed.doc.paths.iter().filter(|p| ed.objsel.contains(&p.id)).map(|p| p.id).collect(), kind }
}
fn blend() -> Kind {
    Kind::Blend { spine: None, steps: 8, orientation: Orientation::Page }
}
pub(crate) fn key(ed: &mut Editor, code: &str, cmd: bool, shift: bool, alt: bool) -> bool {
    if code == "KeyW" && !cmd && !shift && !alt {
        ed.set_tool(ToolKind::Blend);
        return true;
    }
    if code == "KeyB" && cmd && alt {
        let action =
            if shift { live::selected_node(ed).map(|node| Action::Release { node }) } else { Some(make(ed, blend())) };
        if let Some(a) = action {
            ed.colour_error = live::execute(ed, a).err();
        }
        return true;
    }
    false
}
impl super::Ui {
    pub(crate) fn live_menu(&mut self, sid: SessionId, ed: &mut Editor, name: &str) {
        let target = live::selected_node(ed);
        let action = match name {
            "Live:Blend Make" => Some(make(ed, blend())),
            "Live:Spine" => target.and_then(|node| {
                let paths: Vec<_> =
                    ed.objsel.iter().copied().filter(|id| ed.doc.top_group_of_path(*id).is_none()).collect();
                if paths.len() == 1 {
                    Some(Action::Spine { node, path: paths[0] })
                } else {
                    None
                }
            }),
            "Live:Release" => target.map(|node| Action::Release { node }),
            "Live:Expand" => target.map(|node| Action::Expand { node }),
            "Live:Options" => target.and_then(|node| {
                ed.doc.node(node).and_then(|n| {
                    if let varos_core::model::NodeKind::Live(kind) = n.kind {
                        Some(Action::Options { node, kind })
                    } else {
                        None
                    }
                })
            }),
            "Live:Radial" => Some(make(ed, Kind::Repeat { repeat: Repeat::Radial { count: 6, radius: 100. } })),
            "Live:Grid" => Some(make(ed, Kind::Repeat { repeat: Repeat::Grid { rows: 2, cols: 2, gap: [20., 20.] } })),
            "Live:Mirror" => Some(make(ed, Kind::Repeat { repeat: Repeat::Mirror { axis: Axis::Vertical } })),
            "Live:Warp" => {
                Some(make(ed, Kind::Envelope { envelope: Envelope::Warp { preset: Warp::Arc, bend: 0.25 } }))
            }
            "Live:Mesh" => {
                let b = ed.obj_bbox();
                b.map(|(x, y, x1, y1)| {
                    make(
                        ed,
                        Kind::Envelope { envelope: Envelope::Mesh { points: [[x, y], [x1, y], [x, y1], [x1, y1]] } },
                    )
                })
            }
            _ => None,
        };
        if let Some(action) = action {
            if matches!(action, Action::Release { .. } | Action::Expand { .. } | Action::Spine { .. })
                || name == "Live:Blend Make"
            {
                ed.colour_error = live::execute(ed, action).err();
            } else {
                self.ctx.data_mut(|d| d.insert_temp(Id::new("lane-e-live-options"), Draft { sid, action }));
            }
        } else {
            ed.colour_error = Some("Select a live node or source paths first".into());
        }
    }
}
pub(super) fn sheet(ctx: &egui::Context, ed: &Editor, sid: Option<SessionId>, ops: &mut Vec<Op>) {
    let id = Id::new("lane-e-live-options");
    let Some(mut draft) = ctx.data(|d| d.get_temp::<Draft>(id)) else {
        return;
    };
    if sid != Some(draft.sid) {
        ctx.data_mut(|d| d.remove::<Draft>(id));
        return;
    }
    let mut close = false;
    egui::Area::new(id.with("block")).order(egui::Order::Foreground).fixed_pos(ctx.content_rect().min).show(
        ctx,
        |ui| {
            ui.allocate_exact_size(ctx.content_rect().size(), egui::Sense::click());
        },
    );
    egui::Area::new(id).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(
        ctx,
        |ui| {
            egui::Frame::new()
                .fill(t::PANEL)
                .stroke(egui::Stroke::new(t::KIT_STROKE, t::LINE))
                .corner_radius(t::r_box())
                .inner_margin(t::KIT_PAD)
                .show(ui, |ui| {
                    ui.set_width(t::DOC_SHEET_W);
                    ui.label(t::panel_title("Live Object Options"));
                    let kind = match &mut draft.action {
                        Action::Make { kind, .. } | Action::Options { kind, .. } => kind,
                        _ => return,
                    };
                    match kind {
                        Kind::Blend { steps, orientation, .. } => {
                            count(ui, "Steps", steps, 0.);
                            if button(
                                ui,
                                if *orientation == Orientation::Page { "Align to Page" } else { "Align to Path" },
                            ) {
                                *orientation = if *orientation == Orientation::Page {
                                    Orientation::Path
                                } else {
                                    Orientation::Page
                                };
                            }
                        }
                        Kind::Repeat { repeat } => match repeat {
                            Repeat::Radial { count: n, radius } => {
                                count(ui, "Count", n, 1.);
                                number(ui, "Radius", radius, 0.0..=1e6);
                            }
                            Repeat::Grid { rows, cols, gap } => {
                                count(ui, "Rows", rows, 1.);
                                count(ui, "Columns", cols, 1.);
                                number(ui, "Horizontal gap", &mut gap[0], 0.0..=1e6);
                                number(ui, "Vertical gap", &mut gap[1], 0.0..=1e6);
                            }
                            Repeat::Mirror { axis } => {
                                if button(ui, if *axis == Axis::Vertical { "Vertical Axis" } else { "Horizontal Axis" })
                                {
                                    *axis = if *axis == Axis::Vertical { Axis::Horizontal } else { Axis::Vertical };
                                }
                            }
                        },
                        Kind::Envelope { envelope } => match envelope {
                            Envelope::Warp { preset, bend } => {
                                for (p, label) in [(Warp::Arc, "Arc"), (Warp::Flag, "Flag"), (Warp::Bulge, "Bulge")] {
                                    if button(ui, label) {
                                        *preset = p;
                                    }
                                }
                                number(ui, "Bend", bend, -1.0..=1.0);
                            }
                            Envelope::Mesh { points } => {
                                for (i, p) in points.iter_mut().enumerate() {
                                    ui.push_id(i, |ui| {
                                        number(ui, "X", &mut p[0], -1e6..=1e6);
                                        number(ui, "Y", &mut p[1], -1e6..=1e6);
                                    });
                                }
                            }
                        },
                    }
                    let error = live::check(ed, &draft.action).err();
                    if let Some(reason) = &error {
                        kit::notice(ui, reason);
                    }
                    ui.horizontal(|ui| {
                        if button(ui, "Cancel") {
                            close = true;
                        }
                        let mut c = Control::new(ui.id().with("apply"), "Apply");
                        c.availability =
                            error.as_deref().map(kit::Availability::Disabled).unwrap_or(kit::Availability::Enabled);
                        if kit::action(ui, c, false).activated {
                            ops.push(Op::DocumentSetup(EditCommand::Live(draft.action.clone())));
                            close = true;
                        }
                    });
                });
        },
    );
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    ctx.data_mut(|d| {
        if close {
            d.remove::<Draft>(id);
        } else {
            d.insert_temp(id, draft);
        }
    });
}
pub(crate) fn menu_rows() -> Vec<Entry> {
    fn item(label: &'static str, name: &'static str) -> Entry {
        Entry::Item { id: format!("lane-e-{name}"), label, accel: None, cmd: MenuCmd::LaneC(name), check: None }
    }
    let mut rows = vec![
        Entry::Sub {
            label: "Blend",
            items: vec![
                item("Make", "Live:Blend Make"),
                item("Release", "Live:Release"),
                item("Options…", "Live:Options"),
                item("Replace Spine", "Live:Spine"),
                item("Expand", "Live:Expand"),
            ],
        },
        Entry::Sub {
            label: "Repeat",
            items: vec![
                item("Radial…", "Live:Radial"),
                item("Grid…", "Live:Grid"),
                item("Mirror…", "Live:Mirror"),
                item("Options…", "Live:Options"),
                item("Release", "Live:Release"),
            ],
        },
        Entry::Sub {
            label: "Envelope Distort",
            items: vec![
                item("Make with Warp…", "Live:Warp"),
                item("Make with Mesh…", "Live:Mesh"),
                item("Options…", "Live:Options"),
                item("Release", "Live:Release"),
            ],
        },
    ];
    for (family, row) in rows.iter_mut().enumerate() {
        if let Entry::Sub { items, .. } = row {
            for item in items {
                if let Entry::Item { id, accel, cmd, .. } = item {
                    *id = format!("{id}-{family}");
                    if let MenuCmd::LaneC(name) = cmd {
                        if *name == "Live:Blend Make" || (family == 0 && *name == "Live:Release") {
                            *accel = Some(crate::menus::Accel {
                                code: winit::keyboard::KeyCode::KeyB,
                                cmd: true,
                                shift: *name == "Live:Release",
                                alt: true,
                            });
                        }
                    }
                }
            }
        }
    }
    rows
}
