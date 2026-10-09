//! Provisional 4A kit sheets and small tool-options popovers; owner design review pending.
// ---- Lane F: shaped chrome ----
use varos_app::shell::kit::text::ShapedUi as _;
// ---- end Lane F ----
use varos_app::shell::{
    kit::{
        self,
        field::{self, Label, NumberField},
    },
    tokens as t,
};
use varos_core::{
    select_transform::{LayerAction, Transform},
    EditCommand, Editor, ToolKind,
};

fn action(ui: &mut egui::Ui, label: &str) -> bool {
    kit::action(ui, kit::Control::new(super::doc_id(ui, ("4a", label)), label), false).activated
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let e = field::number_field(
        ui,
        NumberField {
            id: super::doc_id(ui, ("4a-field", label)),
            width: t::SLICE4A_TOOLS_FIELD_W,
            label: Label::Letter(label),
            tip: label,
            value: *value,
            decimals: 2,
            speed: 1.,
            range,
            disabled: false,
        },
    );
    if let Some(v) = e.live.or(e.commit).or(e.pending) {
        *value = v;
        true
    } else {
        false
    }
}
fn toggle(ui: &mut egui::Ui, label: &str, value: &mut bool) -> bool {
    let mut c = kit::Control::new(super::doc_id(ui, ("4a-toggle", label)), label);
    c.selected = *value;
    if kit::action(ui, c, false).activated {
        *value = !*value;
        true
    } else {
        false
    }
}
fn frame() -> egui::Frame {
    egui::Frame {
        fill: t::PANEL,
        stroke: egui::Stroke::new(t::KIT_STROKE, t::LINE),
        corner_radius: egui::CornerRadius::same(t::RBOX),
        inner_margin: egui::Margin::same(t::SLICE4A_TOOLS_MARGIN),
        ..Default::default()
    }
}

pub(super) fn draw(ctx: &egui::Context, ed: &mut Editor, hole: egui::Rect) {
    // One compact home for the provisional tools, options, transform sheets and layer operations.
    egui::Area::new(egui::Id::new("4a-tools-home"))
        .order(egui::Order::Middle)
        .fixed_pos(hole.right_top() + egui::vec2(-t::SLICE4A_TOOLS_HOME_W, t::KIT_PAD))
        .show(ctx, |ui| {
            frame().show(ui, |ui| {
                ui.set_width(t::SLICE4A_TOOLS_HOME_W - t::KIT_PAD * 2.);
                let open_id = super::doc_id(ui, "4a-options-open");
                let mut open = ctx.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
                if action(ui, "Transform / selection options") {
                    open = !open;
                }
                ctx.data_mut(|d| d.insert_temp(open_id, open));
                if !open {
                    return;
                }
                if options(ui, ed) {
                    return;
                }
                egui::ScrollArea::vertical()
                    .max_height((hole.height() - t::KIT_CONTROL_H * 3.).max(t::KIT_CONTROL_H))
                    .show(ui, |ui| {
                        for (name, tool) in [
                            ("Reflect (O)", ToolKind::Reflect),
                            ("Shear", ToolKind::Shear),
                            ("Free Transform (E)", ToolKind::FreeTransform),
                            ("Magic Wand (Y)", ToolKind::MagicWand),
                        ] {
                            if action(ui, name) {
                                ed.set_tool(tool);
                            }
                        }
                        for (name, tool) in [
                            ("Rotate…", ToolKind::Rotate),
                            ("Scale…", ToolKind::Scale),
                            ("Reflect…", ToolKind::Reflect),
                            ("Shear…", ToolKind::Shear),
                            ("Transform Each…", ToolKind::FreeTransform),
                        ] {
                            if action(ui, name) && !ed.objsel.is_empty() {
                                ed.select_transform.dialog = Some(tool);
                            }
                        }
                        for (name, action_kind) in [
                            ("Release to Layers (Sequence)", LayerAction::ReleaseSequence),
                            ("Release to Layers (Build)", LayerAction::ReleaseBuild),
                            ("Collect in New Layer", LayerAction::Collect),
                            ("Merge Selected Layers", LayerAction::Merge),
                            ("Flatten Artwork", LayerAction::Flatten),
                            ("Locate Object", LayerAction::Locate),
                            ("Hide Others", LayerAction::HideOthers),
                            ("Lock Others", LayerAction::LockOthers),
                        ] {
                            if action(ui, name) {
                                let _ = action_kind;
                                menu(ed, name);
                            }
                        }
                    });
            });
        });
    let Some(tool) = ed.select_transform.dialog else { return };
    let id = egui::Id::new((
        "4a-sheet",
        ctx.data(|d| d.get_temp::<Option<crate::app_command::SessionId>>(super::doc_salt_key())),
    ));
    let fresh = ed.select_transform.preview.is_none();
    let mut spec = ctx.data(|d| d.get_temp::<Transform>(id).unwrap_or_default());
    if fresh {
        spec = Transform { origin: ed.pivot_point(), each: tool == ToolKind::FreeTransform, ..Default::default() };
        ed.execute_ui(EditCommand::TransformBegin);
    }
    let title = match tool {
        ToolKind::Rotate => "Rotate",
        ToolKind::Scale => "Scale",
        ToolKind::Reflect => "Reflect",
        ToolKind::Shear => "Shear",
        _ => "Transform Each",
    };
    let mut close = None;
    egui::Area::new(id.with("canvas-blocker")).order(egui::Order::Foreground).fixed_pos(hole.min).show(ctx, |ui| {
        ui.allocate_exact_size(hole.size(), egui::Sense::click());
    });
    egui::Area::new(id).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(
        ctx,
        |ui| {
            frame().show(ui, |ui| {
                ui.set_width(t::SLICE4A_TOOLS_SHEET_W);
                ui.shaped_label(t::panel_title(title));
                let mut changed = false;
                if tool == ToolKind::Scale || spec.each {
                    let mut x = spec.scale[0] * 100.;
                    let mut y = spec.scale[1] * 100.;
                    changed |= number(ui, "Horizontal %", &mut x, 0.01..=10000.);
                    changed |= number(ui, "Vertical %", &mut y, 0.01..=10000.);
                    spec.scale = [x / 100., y / 100.];
                }
                if tool == ToolKind::Rotate || spec.each {
                    changed |= number(ui, "Angle °", &mut spec.angle, -360.0..=360.0);
                }
                if tool == ToolKind::Reflect || spec.each {
                    let mut axis = spec.reflect.unwrap_or(90.);
                    if spec.each {
                        let mut reflect = spec.reflect.is_some();
                        changed |= toggle(ui, "Reflect", &mut reflect);
                        spec.reflect = reflect.then_some(axis);
                    }
                    changed |= number(ui, "Reflect axis °", &mut axis, -360.0..=360.0);
                    if tool == ToolKind::Reflect || spec.reflect.is_some() {
                        spec.reflect = Some(axis);
                    }
                }
                if tool == ToolKind::Shear {
                    changed |= number(ui, "Shear °", &mut spec.shear, -89.0..=89.0);
                    changed |= number(ui, "Axis °", &mut spec.shear_axis, -360.0..=360.0);
                }
                if spec.each {
                    changed |= number(ui, "Move X", &mut spec.movement[0], -1.0e6..=1.0e6);
                    changed |= number(ui, "Move Y", &mut spec.movement[1], -1.0e6..=1.0e6);
                    let old = spec.random;
                    toggle(ui, "Random", &mut spec.random);
                    changed |= old != spec.random;
                }
                if changed || fresh {
                    ed.execute_ui(EditCommand::TransformLive(spec));
                }
                ui.horizontal(|ui| {
                    if action(ui, "Cancel") {
                        close = Some(true);
                    }
                    if action(ui, "Copy") {
                        spec.copy = true;
                        ed.execute_ui(EditCommand::TransformLive(spec));
                        close = Some(false);
                    }
                    if action(ui, "Apply") {
                        close = Some(false);
                    }
                });
            });
        },
    );
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = Some(true);
    }
    ctx.data_mut(|d| d.insert_temp(id, spec));
    if let Some(cancel) = close {
        ed.execute_ui(if cancel { EditCommand::TransformCancel } else { EditCommand::TransformCommit });
    }
}

pub(crate) fn menu(ed: &mut Editor, name: &str) {
    let tool = match name {
        "Rotate…" => Some(ToolKind::Rotate),
        "Scale…" => Some(ToolKind::Scale),
        "Reflect…" => Some(ToolKind::Reflect),
        "Shear…" => Some(ToolKind::Shear),
        "Transform Each…" => Some(ToolKind::FreeTransform),
        _ => None,
    };
    if let Some(tool) = tool {
        if !ed.selected_pids().is_empty() {
            ed.select_transform.dialog = Some(tool);
        }
        return;
    }
    let action = match name {
        "Release to Layers (Sequence)" => LayerAction::ReleaseSequence,
        "Release to Layers (Build)" => LayerAction::ReleaseBuild,
        "Collect in New Layer" => LayerAction::Collect,
        "Merge Selected Layers" => LayerAction::Merge,
        "Flatten Artwork" => LayerAction::Flatten,
        "Locate Object" => LayerAction::Locate,
        "Hide Others" => LayerAction::HideOthers,
        "Lock Others" => LayerAction::LockOthers,
        _ => return,
    };
    let mut nodes: Vec<_> = ed.objsel.iter().filter_map(|p| ed.doc.unit_of(*p)).collect();
    if matches!(action, LayerAction::ReleaseSequence | LayerAction::ReleaseBuild | LayerAction::Merge) {
        nodes = nodes.into_iter().map(|n| ed.doc.layer_ancestor(n)).collect();
    }
    if nodes.is_empty() {
        nodes.push(ed.doc.active_layer);
    }
    nodes.sort_unstable();
    nodes.dedup();
    ed.execute_ui(EditCommand::LayerFamily { action, nodes });
}

fn locate_id(ctx: &egui::Context) -> egui::Id {
    egui::Id::new((
        "4a-locate-row",
        ctx.data(|d| d.get_temp::<Option<crate::app_command::SessionId>>(super::doc_salt_key())),
    ))
}
pub(super) fn prepare(ui: &mut super::Ui, ed: &mut Editor) {
    let Some(node) = ed.select_transform.located.take() else { return };
    ui.lay_search.clear();
    let mut at = Some(node);
    while let Some(n) = at {
        ui.lay_collapsed.remove(&n);
        at = ed.doc.node(n).and_then(|n| n.parent);
    }
    // Artboard header sentinels can fold the same tree under several board sections.
    for i in 0..ed.doc.artboards.len() {
        ui.lay_collapsed.remove(&(u32::MAX - i as u32));
    }
    let key = locate_id(&ui.ctx);
    ui.ctx.data_mut(|d| d.insert_temp(key, node));
    if !ui.shell.is_open(varos_app::shell::PanelId::Layers) {
        ui.shell.toggle_panel(varos_app::shell::PanelId::Layers);
    }
}
pub(crate) fn locate_row(ui: &mut egui::Ui, node: u32, rect: egui::Rect) {
    let id = locate_id(ui.ctx());
    if ui.ctx().data(|d| d.get_temp::<u32>(id)) == Some(node) {
        ui.scroll_to_rect(rect, Some(egui::Align::Center));
        ui.ctx().data_mut(|d| d.remove::<u32>(id));
    }
}

fn options(ui: &mut egui::Ui, ed: &mut Editor) -> bool {
    if ed.tool == ToolKind::MagicWand {
        let mut options = ed.select_transform.wand;
        let o = &mut options;
        toggle(ui, "Match fill", &mut o.pick.fill);
        toggle(ui, "Match stroke", &mut o.pick.stroke);
        toggle(ui, "Match weight", &mut o.pick.weight);
        toggle(ui, "Match opacity", &mut o.pick.opacity);
        number(ui, "Colour tolerance", &mut o.colour, 0.0..=1.0);
        number(ui, "Weight tolerance", &mut o.weight, 0.0..=1000.0);
        number(ui, "Opacity tolerance", &mut o.opacity, 0.0..=1.0);
        if options != ed.select_transform.wand {
            ed.execute_ui(EditCommand::SetWandOptions(options));
        }
        return true;
    }
    if ed.tool == ToolKind::Eyedropper {
        let mut options = ed.select_transform.pick;
        let o = &mut options;
        toggle(ui, "Pick fill", &mut o.fill);
        toggle(ui, "Pick stroke", &mut o.stroke);
        toggle(ui, "Pick weight", &mut o.weight);
        toggle(ui, "Pick opacity", &mut o.opacity);
        if options != ed.select_transform.pick {
            ed.execute_ui(EditCommand::SetEyedropperOptions(options));
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provisional_sheet_starts_preview_headlessly_and_cancel_restores() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        let mut ed = Editor::new();
        let id = ed
            .try_execute_created(EditCommand::AddShape {
                kind: varos_core::model::ShapeKind::Rect,
                bounds: [0., 0., 10., 20.],
                parent: None,
                fill: Some([1., 0., 0., 1.]),
                stroke: None,
                stroke_width: 1.,
                opacity: 1.,
                name: None,
            })
            .unwrap();
        ed.execute_ui(EditCommand::SelectPaths(vec![id]));
        let before = ed.doc.clone();
        let hole = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
        for tool in [ToolKind::Rotate, ToolKind::Scale, ToolKind::Reflect, ToolKind::Shear, ToolKind::FreeTransform] {
            ed.select_transform.dialog = Some(tool);
            let _ = ctx.run_ui(egui::RawInput::default(), |_| draw(&ctx, &mut ed, hole));
            assert!(ed.transaction_open());
            ed.execute_ui(EditCommand::TransformCancel);
            assert_eq!(ed.doc, before);
        }
    }
    fn render(ctx: &egui::Context, ed: &mut Editor, events: Vec<egui::Event>) -> egui::FullOutput {
        let hole = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000., 900.));
        ctx.run_ui(egui::RawInput { screen_rect: Some(hole), events, ..Default::default() }, |_| draw(ctx, ed, hole))
    }
    fn text_position(ctx: &egui::Context, label: &str) -> egui::Pos2 {
        varos_app::shell::kit::text::paint_records(ctx)
            .into_iter()
            .find(|r| r.text == label)
            .expect("visible kit action")
            .rect
            .center()
    }
    fn click(ctx: &egui::Context, ed: &mut Editor, label: &str) {
        varos_app::shell::kit::text::enable_trace(ctx);
        let _output = render(ctx, ed, vec![]);
        let pos = text_position(ctx, label);
        for pressed in [true, false] {
            render(
                ctx,
                ed,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn transform_each_reflect_toggle_enable_disable_then_apply() {
        for disable in [false, true] {
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            let mut ed = Editor::new();
            ed.execute_ui(EditCommand::AddShape {
                kind: varos_core::model::ShapeKind::Rect,
                bounds: [0., 0., 10., 20.],
                parent: None,
                fill: None,
                stroke: None,
                stroke_width: 1.,
                opacity: 1.,
                name: None,
            });
            let id = ed.doc.paths[0].id;
            ed.execute_ui(EditCommand::SelectPaths(vec![id]));
            let before = ed.doc.clone();
            let rev = ed.rev;
            ed.select_transform.dialog = Some(ToolKind::FreeTransform);
            for _ in 0..3 {
                render(&ctx, &mut ed, vec![]);
            }
            click(&ctx, &mut ed, "Reflect");
            assert!(!ed.doc.content_eq(&before));
            if disable {
                click(&ctx, &mut ed, "Reflect");
                assert_eq!(ed.doc, before);
            }
            click(&ctx, &mut ed, "Apply");
            assert!(!ed.transaction_open());
            assert_eq!(ed.rev, rev + u64::from(!disable));
            if !disable {
                assert!((ed.doc.paths[0].anchors[0].p[0] - 10.).abs() < 0.001);
                ed.undo();
            }
            assert_eq!(ed.doc, before);
        }
    }
    #[test]
    fn options_dispatch_only_on_change_and_preserve_selection_during_idle_frames() {
        for (tool, label) in [(ToolKind::MagicWand, "Match fill"), (ToolKind::Eyedropper, "Pick fill")] {
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            let mut ed = Editor::new();
            ed.set_tool(tool);
            for _ in 0..3 {
                render(&ctx, &mut ed, vec![]);
            }
            click(&ctx, &mut ed, "Transform / selection options");
            for _ in 0..3 {
                render(&ctx, &mut ed, vec![]);
            }
            ed.select_transform.options_requested = false;
            // Sentinel proves idle drawing does not call execute/prune_inert_selection.
            ed.objsel.insert(u32::MAX);
            for _ in 0..3 {
                render(&ctx, &mut ed, vec![]);
            }
            assert!(!ed.select_transform.options_requested);
            assert!(ed.objsel.contains(&u32::MAX));
            click(&ctx, &mut ed, label);
            assert!(ed.select_transform.options_requested);
            assert!(!ed.objsel.contains(&u32::MAX));
            ed.select_transform.options_requested = false;
            render(&ctx, &mut ed, vec![]);
            assert!(!ed.select_transform.options_requested);
        }
    }
}
