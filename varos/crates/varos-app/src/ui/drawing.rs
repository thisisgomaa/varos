//! Lane D: provisional kit numeric sheets, freehand options and drawing preview.
use varos_app::shell::{
    kit::{
        self,
        field::{self, Label, NumberField},
        Control, Icon,
    },
    tokens as t,
};
use varos_core::{
    drawing::{self, Action, Shape, ShapeSpec},
    geom::View,
    EditCommand, Editor, ToolKind,
};
pub(crate) fn tool_name(t: ToolKind) -> &'static str {
    match t {
        ToolKind::RoundedRect => "Rounded Rectangle",
        ToolKind::Star => "Star",
        ToolKind::Line => "Line Segment (\\)",
        ToolKind::Arc => "Arc",
        ToolKind::Spiral => "Spiral",
        ToolKind::RectGrid => "Rectangular Grid",
        ToolKind::PolarGrid => "Polar Grid",
        ToolKind::Pencil => "Pencil (N)",
        ToolKind::Smooth => "Smooth",
        ToolKind::PathEraser => "Path Eraser",
        ToolKind::Join => "Join",
        ToolKind::Curvature => "Curvature (Shift+~)",
        ToolKind::Rect => "Rectangle (M)",
        ToolKind::Ellipse => "Ellipse (L)",
        ToolKind::Polygon => "Polygon",
        ToolKind::Triangle => "Triangle",
        ToolKind::Pen => "Pen (P)",
        ToolKind::Object => "Selection (V)",
        ToolKind::Direct => "Direct Selection (A)",
        ToolKind::Lasso => "Lasso (Q)",
        ToolKind::Convert => "Anchor Point (Shift+C)",
        ToolKind::AddAnchor => "Add Anchor (+)",
        ToolKind::DeleteAnchor => "Delete Anchor (-)",
        ToolKind::Artboard => "Artboard (Shift+O)",
        ToolKind::Width => "Width (Shift+W)",
        ToolKind::Text => "Type (T)",
        ToolKind::Gradient => "Gradient (G)",
        ToolKind::Hand => "Hand (H)",
        ToolKind::Zoom => "Zoom (Z)",
        ToolKind::Eyedropper => "Eyedropper (I)",
        ToolKind::Rotate => "Rotate (R)",
        ToolKind::Scale => "Scale (S)",
        ToolKind::Reflect => "Reflect (O)",
        ToolKind::Shear => "Shear",
        ToolKind::FreeTransform => "Free Transform (E)",
        ToolKind::MagicWand => "Magic Wand (Y)",
        ToolKind::ShapeBuilder => "Shape Builder (Shift+M)",
        ToolKind::Scissors => "Scissors (C)",
        ToolKind::Knife => "Knife",
        ToolKind::Eraser => "Eraser (Shift+E)",
    }
}
pub(super) fn shape_icon(tool: ToolKind) -> Icon {
    match tool {
        ToolKind::RoundedRect => Icon::DrawRoundedRect,
        ToolKind::Star => Icon::DrawStar,
        ToolKind::Ellipse => Icon::DrawEllipse,
        ToolKind::Polygon => Icon::DrawPolygon,
        ToolKind::Triangle => Icon::DrawTriangle,
        _ => Icon::DrawRect,
    }
}
pub(super) fn frame() -> egui::Frame {
    egui::Frame {
        fill: t::PANEL,
        stroke: egui::Stroke::new(t::KIT_STROKE, t::LINE),
        corner_radius: egui::CornerRadius::same(t::RBOX),
        inner_margin: egui::Margin::same(t::DRAW_MARGIN),
        ..Default::default()
    }
}
fn action(ui: &mut egui::Ui, label: &str) -> bool {
    kit::action(ui, Control::new(super::doc_id(ui, ("lane-d", label)), label), false).activated
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    let e = field::number_field(
        ui,
        NumberField {
            id: super::doc_id(ui, ("lane-d-field", label)),
            width: t::DRAW_FIELD_W,
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
    }
}
fn count(ui: &mut egui::Ui, label: &str, value: &mut usize, min: f32) {
    let mut v = *value as f32;
    number(ui, label, &mut v, min..=1000.);
    *value = v.round() as usize;
}
pub(crate) fn key(ed: &mut Editor, code: &str, shift: bool, alt: bool) -> bool {
    if code == "ArrowUp" && drawing::arrow(ed, true) {
        return true;
    }
    if code == "ArrowDown" && drawing::arrow(ed, false) {
        return true;
    }
    if alt {
        return false;
    }
    match code {
        "KeyN" if !shift => ed.set_tool(ToolKind::Pencil),
        "Backslash" if !shift => ed.set_tool(ToolKind::Line),
        "Backquote" if shift => ed.set_tool(ToolKind::Curvature),
        "ArrowUp" if drawing::arrow(ed, true) => {}
        "ArrowDown" if drawing::arrow(ed, false) => {}
        "Enter" if ed.tool == ToolKind::Curvature => drawing::finish(ed, false),
        _ => return false,
    }
    true
}
fn fields(ui: &mut egui::Ui, s: &mut ShapeSpec) {
    if s.kind != Shape::Line {
        number(ui, "Width", &mut s.size[0], 0.01..=1.0e7);
        number(ui, "Height", &mut s.size[1], 0.01..=1.0e7);
    }
    match s.kind {
        Shape::RoundedRectangle => number(ui, "Corner radius", &mut s.radius, 0. ..=1.0e7),
        Shape::Polygon | Shape::Star => {
            count(
                ui,
                if s.kind == Shape::Star { "Points" } else { "Sides" },
                &mut s.sides,
                if s.kind == Shape::Star { 2. } else { 3. },
            );
            if s.kind == Shape::Star {
                number(ui, "Inner / outer", &mut s.inner_ratio, 0.01..=1.);
            }
            number(ui, "Rotation °", &mut s.rotation, -360. ..=360.);
        }
        Shape::Line => {
            let mut length = (s.size[0] * s.size[0] + s.size[1] * s.size[1]).sqrt();
            let mut angle = s.size[1].atan2(s.size[0]).to_degrees();
            number(ui, "Length", &mut length, 0.01..=1.0e7);
            number(ui, "Angle °", &mut angle, -360. ..=360.);
            s.size = [length * angle.to_radians().cos(), length * angle.to_radians().sin()];
        }
        Shape::Arc => {
            number(ui, "Start °", &mut s.rotation, -360. ..=360.);
            number(ui, "Sweep °", &mut s.sweep, -360. ..=360.);
        }
        Shape::Spiral => {
            number(ui, "Turns", &mut s.turns, -100. ..=100.);
            number(ui, "Decay", &mut s.decay, 0.0001..=1.);
        }
        Shape::RectangularGrid | Shape::PolarGrid => {
            count(ui, if s.kind == Shape::PolarGrid { "Rings" } else { "Rows" }, &mut s.rows, 1.);
            count(ui, if s.kind == Shape::PolarGrid { "Radials" } else { "Columns" }, &mut s.columns, 1.);
        }
        _ => {}
    }
    let mut c = Control::new(super::doc_id(ui, "lane-d-centre"), "From centre");
    c.selected = s.centre;
    if kit::action(ui, c, false).activated {
        s.centre = !s.centre;
    }
}
pub(super) fn draw(ctx: &egui::Context, ed: &mut Editor, hole: egui::Rect, view: View, ppp: f32) {
    let modal_id = sheet_id(ctx, "lane-d-modal");
    ctx.data_mut(|d| d.insert_temp(modal_id, ed.drawing.dialog.is_some()));
    drawing::refresh(ed);
    preview(ctx, ed, hole, view, ppp);
    if matches!(ed.tool, ToolKind::Pencil | ToolKind::Smooth | ToolKind::PathEraser | ToolKind::Join) {
        let id = egui::Id::new((
            "lane-d-options",
            ctx.data(|d| d.get_temp::<Option<crate::app_command::SessionId>>(super::doc_salt_key())),
        ));
        egui::Area::new(id)
            .order(egui::Order::Middle)
            .fixed_pos(hole.right_top() + egui::vec2(-t::DRAW_SHEET_W, t::KIT_PAD))
            .show(ctx, |ui| {
                frame().show(ui, |ui| {
                    let mut open = ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
                    if action(ui, "Freehand options") {
                        open = !open;
                    }
                    ctx.data_mut(|d| d.insert_temp(id, open));
                    if open {
                        let mut o = ed.drawing.options;
                        number(ui, "Fidelity (canvas units)", &mut o.fidelity, 0.01..=100.);
                        number(ui, "Smoothness", &mut o.smoothness, 0. ..=1.);
                        number(ui, "Continue distance", &mut o.endpoint_distance, 0. ..=1000.);
                        number(ui, "Brush radius", &mut o.brush_radius, 0.01..=1000.);
                        if o != ed.drawing.options {
                            ed.execute_ui(EditCommand::Drawing(Action::Options { options: o }));
                        }
                    }
                });
            });
    }
    let Some(mut s) = ed.drawing.dialog else { return };
    let mut close = false;
    let mut apply = false;
    egui::Area::new(sheet_id(ctx, "lane-d-sheet-blocker")).order(egui::Order::Foreground).fixed_pos(hole.min).show(
        ctx,
        |ui| {
            ui.allocate_exact_size(hole.size(), egui::Sense::click());
        },
    );
    egui::Area::new(sheet_id(ctx, "lane-d-shape-sheet"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            frame().show(ui, |ui| {
                ui.set_width(t::DRAW_SHEET_W);
                ui.label(t::panel_title(tool_name(ed.tool)));
                fields(ui, &mut s);
                ui.horizontal(|ui| {
                    if action(ui, "Cancel") {
                        close = true;
                    }
                    if action(ui, "Create") {
                        apply = true;
                    }
                });
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
        apply = false;
    } else if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        apply = true;
    }
    if apply {
        let action = Action::Shape { spec: s };
        match drawing::check(ed, &action) {
            Ok(()) => {
                ed.drawing.shape = s;
                ed.execute_ui(EditCommand::Drawing(action));
                close = true;
            }
            Err(error) => ed.stroke_error = Some(error),
        }
    }
    ed.drawing.dialog = if close { None } else { Some(s) };
    ctx.data_mut(|d| d.insert_temp(modal_id, ed.drawing.dialog.is_some()));
}
pub(super) fn blocks_keyboard(ctx: &egui::Context) -> bool {
    let id = sheet_id(ctx, "lane-d-modal");
    ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false))
}
fn sheet_id(ctx: &egui::Context, label: &str) -> egui::Id {
    egui::Id::new((label, ctx.data(|d| d.get_temp::<Option<crate::app_command::SessionId>>(super::doc_salt_key()))))
}
fn preview(ctx: &egui::Context, ed: &Editor, hole: egui::Rect, view: View, ppp: f32) {
    let painter = ctx
        .layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("lane-d-preview")))
        .with_clip_rect(hole);
    if matches!(ed.tool, ToolKind::Smooth | ToolKind::PathEraser | ToolKind::Join) {
        let p = view.w2s(ed.cursor);
        let ink = if ed.drawing.start.is_some() { t::ACCENT } else { t::MUTED };
        painter.circle_stroke(
            egui::pos2(p[0] / ppp, p[1] / ppp),
            ed.drawing.options.brush_radius * view.zoom / ppp,
            egui::Stroke::new(t::DRAW_PREVIEW_STROKE, ink),
        );
    }
    for p in &ed.drawing.preview {
        let n = p.anchors.len();
        if n < 2 {
            continue;
        }
        for i in 0..if p.closed { n } else { n - 1 } {
            let a = &p.anchors[i];
            let b = &p.anchors[(i + 1) % n];
            let pts: Vec<_> = (0..=24)
                .map(|j| {
                    let q =
                        varos_core::geom::cubic(a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p, j as f32 / 24.);
                    let screen = view.w2s(q);
                    egui::pos2(screen[0] / ppp, screen[1] / ppp)
                })
                .collect();
            painter.add(egui::Shape::line(pts, egui::Stroke::new(t::DRAW_PREVIEW_STROKE, t::ACCENT)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render(ctx: &egui::Context, ed: &mut Editor, events: Vec<egui::Event>) -> egui::FullOutput {
        let hole = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900., 700.));
        ctx.run_ui(egui::RawInput { screen_rect: Some(hole), events, ..Default::default() }, |_| {
            draw(ctx, ed, hole, View::identity(), 1.)
        })
    }
    fn position(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
        fn find(s: &egui::epaint::Shape, label: &str) -> Option<egui::Pos2> {
            match s {
                egui::epaint::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.pos + t.galley.rect.center().to_vec2())
                }
                egui::epaint::Shape::Vec(ss) => ss.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        out.shapes.iter().find_map(|s| find(&s.shape, label)).expect("visible kit action")
    }
    fn click(ctx: &egui::Context, ed: &mut Editor, label: &str) {
        let out = render(ctx, ed, vec![]);
        let pos = position(&out, label);
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
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    #[test]
    fn numeric_sheets_create_every_shape_and_cancel_is_noop() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        for tool in [
            ToolKind::RoundedRect,
            ToolKind::Polygon,
            ToolKind::Star,
            ToolKind::Line,
            ToolKind::Arc,
            ToolKind::Spiral,
            ToolKind::RectGrid,
            ToolKind::PolarGrid,
        ] {
            let mut ed = Editor::new();
            ed.set_tool(tool);
            ed.drawing.dialog = Some(ShapeSpec { kind: drawing::shape_kind(tool).unwrap(), ..Default::default() });
            render(&ctx, &mut ed, vec![]);
            click(&ctx, &mut ed, "Create");
            assert!(ed.drawing.dialog.is_none());
            assert!(!ed.doc.paths.is_empty());
            assert_eq!(ed.rev, 1);
            let before = ed.doc.clone();
            ed.drawing.dialog = Some(ShapeSpec::default());
            click(&ctx, &mut ed, "Cancel");
            assert_eq!(ed.doc, before);
        }
    }
    #[test]
    fn refused_numeric_sheet_retains_values_until_cancel() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        let mut ed = Editor::new();
        ed.set_tool(ToolKind::Line);
        ed.drawing.dialog = Some(ShapeSpec { kind: Shape::Line, size: [100., 0.], ..Default::default() });
        let layer = ed.doc.active_layer;
        ed.doc.nodes.iter_mut().find(|n| n.id == layer).unwrap().locked = true;
        let before = ed.doc.clone();
        render(&ctx, &mut ed, vec![]);
        click(&ctx, &mut ed, "Create");
        assert!(ed.drawing.dialog.is_some());
        assert_eq!(ed.drawing.dialog.unwrap().size, [100., 0.]);
        assert_eq!(ed.doc, before);
        assert!(ed.stroke_error.is_some());
        assert!(blocks_keyboard(&ctx));
        click(&ctx, &mut ed, "Cancel");
        assert!(ed.drawing.dialog.is_none());
        assert!(!blocks_keyboard(&ctx));
    }
    #[test]
    fn only_illustrator_bindings_and_enter_finishes_curvature() {
        let mut ed = Editor::new();
        assert!(key(&mut ed, "KeyN", false, false));
        assert!(ed.tool == ToolKind::Pencil);
        assert!(!key(&mut ed, "KeyN", true, false));
        assert!(!key(&mut ed, "KeyN", false, true));
        assert!(key(&mut ed, "Backslash", false, false));
        assert!(ed.tool == ToolKind::Line);
        assert!(key(&mut ed, "Backquote", true, false));
        ed.pointer_down([0., 0.]);
        ed.pointer_up();
        ed.pointer_down([50., 50.]);
        ed.pointer_up();
        assert!(key(&mut ed, "Enter", false, false));
        assert_eq!(ed.doc.paths.len(), 1);
    }
}
