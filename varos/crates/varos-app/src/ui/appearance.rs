//! Lane A: provisional Appearance home, built solely from existing kit fields/actions.
use super::{ops::Op, Snap};
use egui::{Id, Sense};
use varos_app::shell::{
    kit::{
        self,
        field::{self, Label, NumberField},
        Control,
    },
    tokens as t,
};
use varos_core::{
    appearance::{BaseSlot, Look, StackItem},
    appearance_edits::{AppearanceEdit as A, MaskEdit as M},
    model::{GroupRole, Paint},
    EditCommand, Editor,
};
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub node: u32,
    pub path: Option<u32>,
    pub stack: Vec<StackItem>,
    pub fill: Paint,
    pub stroke: Paint,
    pub display_fill: Paint,
    pub display_stroke: Paint,
    pub display_stack: Vec<StackItem>,
    pub look: Option<Look>,
    pub role: GroupRole,
}
impl Snapshot {
    pub fn read(ed: &Editor) -> Option<Self> {
        let node = ed
            .selected_image_groups()
            .next()
            .or_else(|| ed.repr_path().and_then(|i| ed.doc.node_of_path(ed.doc.paths[i].id)))?;
        let n = ed.doc.node(node)?;
        let path = if let varos_core::model::NodeKind::Path(id) = n.kind {
            ed.doc.pidx(id).map(|i| &ed.doc.paths[i])
        } else {
            None
        };
        Some(Self {
            node,
            path: path.map(|p| p.id),
            stack: path.map_or_else(Vec::new, |p| p.appearance().stack()),
            fill: path.map_or(Paint::None, |p| p.fill.clone()),
            stroke: path.map_or(Paint::None, |p| p.stroke.clone()),
            display_fill: path.map_or(Paint::None, |p| p.fill.resolved(&ed.doc)),
            display_stroke: path.map_or(Paint::None, |p| p.stroke.resolved(&ed.doc)),
            display_stack: path.map_or_else(Vec::new, |p| {
                p.appearance()
                    .stack()
                    .into_iter()
                    .map(|mut item| {
                        if let StackItem::Fill { paint, .. } | StackItem::Stroke { paint, .. } = &mut item {
                            *paint = paint.resolved(&ed.doc);
                        }
                        item
                    })
                    .collect()
            }),
            look: n.look,
            role: n.role,
        })
    }
}
fn action(ui: &mut egui::Ui, key: impl std::hash::Hash + std::fmt::Debug, label: &str) -> bool {
    kit::action(ui, Control::new(Id::new(key), label), false).activated
}
fn added_paint(paint: &Paint) -> Paint {
    if matches!(paint, Paint::None) {
        Paint::Solid([0., 0., 0., 1.])
    } else {
        paint.clone()
    }
}
fn edit(ops: &mut Vec<Op>, edit: A) {
    ops.push(Op::DocumentSetup(EditCommand::Appearance(edit)));
}
// The kit owns editing; route pending typing through the common settlement path.
fn number_edit(ui: &egui::Ui, e: field::Edit<f32>, ops: &mut Vec<Op>, mk: impl Fn(f32) -> A) {
    let gesture = Id::new("appearance-scrubbing");
    if let Some(v) = e.live {
        if ui.input(|i| i.pointer.any_down()) {
            let started = ui.ctx().data_mut(|d| {
                let started = d.get_temp::<bool>(gesture).unwrap_or(false);
                d.insert_temp(gesture, true);
                started
            });
            ops.push(Op::DocumentSetupLive(EditCommand::Appearance(mk(v)), !started));
        } else {
            edit(ops, mk(v));
        }
    }
    if let Some(v) = e.commit {
        ops.push(Op::Field(Box::new(Op::DocumentSetup(EditCommand::Appearance(mk(v))))));
    }
    if let Some(v) = e.pending {
        ops.push(Op::FieldPending(e.id, Box::new(Op::DocumentSetup(EditCommand::Appearance(mk(v))))));
    }
}
pub(crate) fn settle(ctx: &egui::Context, ops: &mut Vec<Op>) {
    let id = Id::new("appearance-scrubbing");
    if !ctx.input(|i| i.pointer.any_down()) && ctx.data_mut(|d| d.remove_temp::<bool>(id)).unwrap_or(false) {
        ops.push(Op::DocumentSetupFinish);
    }
}
pub(crate) fn section(ui: &mut egui::Ui, s: &Snap, ops: &mut Vec<Op>) {
    let marker = ui.label(egui::RichText::new("APPEARANCE").font(t::small()).color(t::MUTED));
    if ui.ctx().data_mut(|d| d.remove_temp::<bool>(Id::new("appearance-jump"))).unwrap_or(false) {
        ui.scroll_to_rect(marker.rect, Some(egui::Align::Min));
    }
    let Some(s) = &s.appearance else { return };
    if let Some(path) = s.path {
        ui.horizontal(|ui| {
            if action(ui, ("appearance-add-fill", path), "Add fill") {
                edit(ops, A::AddFill { path, paint: added_paint(&s.fill) });
            }
            if action(ui, ("appearance-add-stroke", path), "Add stroke") {
                edit(ops, A::AddStroke { path, paint: added_paint(&s.stroke), width: t::KIT_STROKE });
            }
        });
        for (index, item) in s.stack.iter().enumerate().rev() {
            let (label, paint) = match item {
                StackItem::Base(BaseSlot::Fill, _) => ("Base fill", &s.fill),
                StackItem::Base(BaseSlot::Stroke, _) => ("Base stroke", &s.stroke),
                StackItem::Fill { paint, .. } => ("Fill", paint),
                StackItem::Stroke { paint, .. } => ("Stroke", paint),
            };
            let display_paint = match &s.display_stack[index] {
                StackItem::Base(BaseSlot::Fill, _) => &s.display_fill,
                StackItem::Base(BaseSlot::Stroke, _) => &s.display_stroke,
                StackItem::Fill { paint, .. } | StackItem::Stroke { paint, .. } => paint,
            };
            let opacity = item.opts().opacity;
            ui.push_id((path, index), |ui| {
                ui.horizontal(|ui| {
                    let label_response =
                        ui.allocate_response(egui::vec2(t::APPEARANCE_LABEL_W, t::KIT_CONTROL_H), Sense::drag());
                    ui.painter().text(
                        label_response.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        t::small(),
                        t::TEXT,
                    );
                    if label_response.drag_started() {
                        ui.ctx().data_mut(|d| d.insert_temp(Id::new("appearance-drag"), (path, index)));
                    }
                    if ui.input(|i| i.pointer.any_released()) && label_response.hovered() {
                        if let Some((pid, from)) =
                            ui.ctx().data_mut(|d| d.remove_temp::<(u32, usize)>(Id::new("appearance-drag")))
                        {
                            if pid == path && from != index {
                                edit(ops, A::Reorder { path, from, to: index });
                            }
                        }
                    }
                    if action(ui, "visibility", if item.opts().visible { "On" } else { "Off" }) {
                        edit(
                            ops,
                            A::SetEntry { path, index, paint: paint.clone(), opacity, visible: !item.opts().visible },
                        );
                    }
                    let e = field::number_field(
                        ui,
                        NumberField {
                            id: Id::new(("appearance-opacity", path, index)),
                            width: t::APPEARANCE_OPACITY_W,
                            label: Label::Letter("%"),
                            tip: "Entry opacity",
                            value: opacity * 100.,
                            decimals: 0,
                            speed: 1.,
                            range: 0.0..=100.,
                            disabled: false,
                        },
                    );
                    number_edit(ui, e, ops, |v| A::SetEntry {
                        path,
                        index,
                        paint: paint.clone(),
                        opacity: v / 100.,
                        visible: item.opts().visible,
                    });
                    if !matches!(item, StackItem::Base(_, _)) && action(ui, "delete", "Delete") {
                        edit(ops, A::Delete { path, index });
                    }
                });
                let colour = display_paint.representative().unwrap_or([0.; 4]);
                if let StackItem::Base(slot, _) = item {
                    let target = match slot {
                        BaseSlot::Fill => super::PaintTarget::Fill,
                        BaseSlot::Stroke => super::PaintTarget::Stroke,
                    };
                    super::picker::paint_row(ui, target, display_paint.representative(), false, ops);
                } else {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(t::APPEARANCE_SWATCH_W, t::KIT_CONTROL_H), Sense::click());
                    ui.painter().rect_filled(rect, t::r_ctrl(), super::rgba_c32a(colour));
                    response.clone().on_hover_text("Edit entry paint");
                    if response.clicked() {
                        let id = Id::new(("appearance-colour", path, index));
                        let open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
                        ui.ctx().data_mut(|d| d.insert_temp(id, !open));
                    }
                }
                if ui.ctx().data(|d| d.get_temp::<bool>(Id::new(("appearance-colour", path, index)))).unwrap_or(false) {
                    ui.horizontal_wrapped(|ui| {
                        for (channel, label) in ["R", "G", "B", "A"].iter().enumerate() {
                            let e = field::number_field(
                                ui,
                                NumberField {
                                    id: Id::new(("appearance-channel", path, index, channel)),
                                    width: t::APPEARANCE_CHANNEL_W,
                                    label: Label::Letter(label),
                                    tip: "Entry solid colour channel",
                                    value: colour[channel] * 100.,
                                    decimals: 0,
                                    speed: 1.,
                                    range: 0.0..=100.,
                                    disabled: false,
                                },
                            );
                            number_edit(ui, e, ops, |v| {
                                let mut c = colour;
                                c[channel] = v / 100.;
                                A::SetEntry {
                                    path,
                                    index,
                                    paint: Paint::Solid(c),
                                    opacity,
                                    visible: item.opts().visible,
                                }
                            });
                        }
                        if action(ui, "no-paint", "None") {
                            edit(
                                ops,
                                A::SetEntry { path, index, paint: Paint::None, opacity, visible: item.opts().visible },
                            );
                        }
                    });
                }
            });
        }
        if action(ui, ("appearance-expand", path), "Expand Appearance") {
            edit(ops, A::Expand { path });
        }
    } else {
        let look = s.look.unwrap_or_default();
        let e = field::number_field(
            ui,
            NumberField {
                id: Id::new(("group-opacity", s.node)),
                width: ui.available_width(),
                label: Label::Letter("%"),
                tip: "Group opacity",
                value: look.opacity * 100.,
                decimals: 0,
                speed: 1.,
                range: 0.0..=100.,
                disabled: false,
            },
        );
        number_edit(ui, e, ops, |v| A::SetLook { node: s.node, look: Some(Look { opacity: v / 100., ..look }) });
        if action(ui, ("group-isolate", s.node), if look.isolate { "Isolated" } else { "Isolate" }) {
            edit(ops, A::SetLook { node: s.node, look: Some(Look { isolate: !look.isolate, ..look }) });
        }
    }
    ui.horizontal(|ui| {
        if action(ui, ("mask-add", s.node), "Add mask") {
            ops.push(Op::DocumentSetup(EditCommand::Mask(M::Begin { node: s.node, alpha: true })));
        }
        if s.role.is_mask_group() {
            if action(ui, ("mask-mode", s.node), if s.role == GroupRole::MaskAlpha { "Alpha" } else { "Clip" }) {
                ops.push(Op::DocumentSetup(EditCommand::Mask(M::Mode {
                    node: s.node,
                    alpha: s.role != GroupRole::MaskAlpha,
                })));
            }
            if action(ui, ("mask-release", s.node), "Release mask") {
                ops.push(Op::DocumentSetup(EditCommand::Mask(M::Release { node: s.node })));
            }
        }
    });
}
pub(crate) fn row(ui: &mut egui::Ui, rect: egui::Rect, node: u32, fx: bool, mask: bool, ops: &mut Vec<Op>) {
    let label = if fx && mask {
        "fx ⌘7"
    } else if mask {
        "⌘7"
    } else if fx {
        "fx"
    } else {
        "+ mask"
    };
    let response = ui.interact(rect, Id::new(("appearance-row", node)), Sense::click());
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        t::small(),
        if fx || mask { t::TEXT } else { t::MUTED },
    );
    if response.clicked() {
        if fx || mask {
            ops.push(Op::LayerSelectSet(vec![node]));
            ui.ctx().data_mut(|d| {
                d.insert_temp(Id::new("appearance-jump"), true);
                d.insert_temp(Id::new("appearance-open"), true);
            });
        } else {
            ops.push(Op::DocumentSetup(EditCommand::Mask(M::Begin { node, alpha: true })));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(live: Option<f32>, commit: Option<f32>, pending: Option<f32>) -> field::Edit<f32> {
        field::Edit {
            id: Id::new("appearance-test-field"),
            rect: egui::Rect::NOTHING,
            live,
            commit,
            pending,
            closed: false,
            editing: pending.is_some(),
        }
    }
    #[test]
    fn kit_commits_and_pending_use_common_field_settlement() {
        let ctx = egui::Context::default();
        let mut ops = vec![];
        let _ = ctx.run_ui(Default::default(), |ui| {
            number_edit(ui, report(None, Some(30.), Some(40.)), &mut ops, |v| A::SetLook {
                node: 1,
                look: Some(Look { opacity: v / 100., isolate: true }),
            });
        });
        assert!(matches!(&ops[0], Op::Field(_)));
        assert!(matches!(&ops[1], Op::FieldPending(_, _)));
    }
    #[test]
    fn scrub_ops_coalesce_into_one_command_transaction() {
        let mut ed = Editor::new();
        let before = ed.doc.clone();
        let rev = ed.rev;
        super::super::apply_ops(
            &mut ed,
            vec![Op::DocumentSetupLive(
                EditCommand::Appearance(A::SetLook { node: 1, look: Some(Look { opacity: 0.2, isolate: true }) }),
                true,
            )],
        );
        for opacity in [0.4, 0.7] {
            super::super::apply_ops(
                &mut ed,
                vec![Op::DocumentSetupLive(
                    EditCommand::Appearance(A::SetLook { node: 1, look: Some(Look { opacity, isolate: true }) }),
                    false,
                )],
            );
        }
        super::super::apply_ops(&mut ed, vec![Op::DocumentSetupFinish]);
        assert_eq!(ed.rev, rev + 1);
        ed.execute(EditCommand::Undo).unwrap();
        assert_eq!(ed.doc, before);
    }
    #[test]
    fn snapshot_options_keep_authored_global_swatches() {
        use varos_core::{
            model::{Anchor, Path},
            swatches::Swatch,
        };
        let mut ed = Editor::new();
        let id = ed.doc.nid();
        let anchors = [[0., 0.], [20., 0.], [20., 20.]]
            .into_iter()
            .map(|p| Anchor { id: ed.doc.nid(), p, hin: None, hout: None, smooth: false })
            .collect();
        let mut path = Path::new(id, anchors, true, None, None, 2.);
        let reference = Paint::SwatchRef { id: 5000 };
        path.fill = reference.clone();
        path.stroke = reference.clone();
        path.stack = varos_core::appearance::base_stack();
        path.stack.push(StackItem::Fill { paint: reference.clone(), opts: Default::default() });
        ed.doc.swatches.push(Swatch {
            id: 5000,
            name: "Global".into(),
            paint: Paint::Solid([0.2, 0.4, 0.6, 1.]),
            global: true,
            group: String::new(),
        });
        ed.doc.paths.push(path);
        ed.doc.sync_tree();
        ed.layer_select_set(&[ed.doc.node_of_path(id).unwrap()]);
        let snapshot = Snapshot::read(&ed).unwrap();
        assert_eq!(snapshot.display_fill, Paint::Solid([0.2, 0.4, 0.6, 1.]));
        for (index, item) in snapshot.stack.iter().enumerate() {
            let paint = match item {
                StackItem::Base(BaseSlot::Fill, _) => snapshot.fill.clone(),
                StackItem::Base(BaseSlot::Stroke, _) => snapshot.stroke.clone(),
                StackItem::Fill { paint, .. } | StackItem::Stroke { paint, .. } => paint.clone(),
            };
            ed.try_execute(EditCommand::Appearance(A::SetEntry {
                path: id,
                index,
                paint,
                opacity: 0.4,
                visible: false,
            }))
            .unwrap();
        }
        let path = &ed.doc.paths[ed.doc.pidx(id).unwrap()];
        assert_eq!(path.fill, reference);
        assert_eq!(path.stroke, reference);
        assert!(matches!(&path.stack[2], StackItem::Fill { paint, .. } if *paint == reference));
        ed.doc.swatches[0].paint = Paint::Solid([1., 0., 0., 1.]);
        let snapshot = Snapshot::read(&ed).unwrap();
        assert_eq!(snapshot.display_fill, Paint::Solid([1., 0., 0., 1.]));
    }
}
