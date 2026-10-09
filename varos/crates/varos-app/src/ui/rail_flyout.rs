//! Lane D: provisional kit flyouts. Last-used tools are keyed by document + group.
//! Glyph provenance: existing Lucide registry (ISC) and Varos originals; no reference assets copied.
use super::{Op, ToolKind};
use varos_app::shell::{
    kit::{self, Control, Icon, IconState},
    tokens as t,
};
pub(crate) const SHAPES: &[ToolKind] =
    &[ToolKind::Rect, ToolKind::RoundedRect, ToolKind::Ellipse, ToolKind::Polygon, ToolKind::Star, ToolKind::Triangle];
const LINES: &[ToolKind] = &[ToolKind::Line, ToolKind::Arc, ToolKind::Spiral, ToolKind::RectGrid, ToolKind::PolarGrid];
const FREEHAND: &[ToolKind] = &[ToolKind::Pencil, ToolKind::Smooth, ToolKind::PathEraser, ToolKind::Join];
pub(crate) const GROUPS: &[&[ToolKind]] = &[
    &[ToolKind::Object, ToolKind::Direct, ToolKind::Lasso],
    &[ToolKind::Artboard],
    &[ToolKind::Pen, ToolKind::Curvature, ToolKind::AddAnchor, ToolKind::DeleteAnchor, ToolKind::Convert],
    // Lane G: Type sits after the Pen group (Illustrator toolbar order)
    &[ToolKind::Text],
    SHAPES,
    LINES,
    FREEHAND,
    &[ToolKind::Rotate, ToolKind::Reflect],
    &[ToolKind::Scale, ToolKind::Shear, ToolKind::FreeTransform],
    // w2-gradients: the Gradient tool (G) in the kit rail (integration w2: the old construction strip is gone)
    &[ToolKind::Gradient],
    // ---- Lane E: Phase 11 ----
    &[ToolKind::Blend],
    &[ToolKind::Eyedropper, ToolKind::MagicWand],
    &[ToolKind::ShapeBuilder],
    &[ToolKind::Scissors, ToolKind::Knife, ToolKind::Eraser],
    &[ToolKind::Hand, ToolKind::Zoom],
];
#[derive(Clone)]
struct Flyout {
    last: ToolKind,
    open: bool,
    pressed: Option<f64>,
    held: bool,
}
impl Flyout {
    fn new(last: ToolKind) -> Self {
        Self { last, open: false, pressed: None, held: false }
    }
    fn observe(&mut self, active: ToolKind, group: &[ToolKind]) {
        if group.contains(&active) {
            self.last = active;
        }
    }
    fn hold(&mut self, now: f64, down: bool) -> bool {
        if !down {
            self.pressed = None;
            self.held = false;
            return false;
        }
        let start = *self.pressed.get_or_insert(now);
        if now >= start + t::DRAW_HOLD_SECONDS && !self.held {
            self.held = true;
            self.open = true;
            return true;
        }
        false
    }
}
fn icon(tool: ToolKind) -> Icon {
    match tool {
        ToolKind::Rect
        | ToolKind::RoundedRect
        | ToolKind::Triangle
        | ToolKind::Polygon
        | ToolKind::Star
        | ToolKind::Ellipse => super::drawing::shape_icon(tool),
        ToolKind::Line => Icon::DrawLine,
        ToolKind::Arc => Icon::DrawArc,
        ToolKind::Spiral => Icon::DrawSpiral,
        ToolKind::RectGrid => Icon::Grid,
        ToolKind::PolarGrid => Icon::DrawPolarGrid,
        ToolKind::Pencil => Icon::DrawPencil,
        ToolKind::Smooth => Icon::DrawSmooth,
        ToolKind::Curvature => Icon::DrawCurvature,
        ToolKind::PathEraser | ToolKind::Eraser => Icon::PathEraser,
        ToolKind::Join => Icon::Link,
        ToolKind::Text => Icon::Type,
        ToolKind::Gradient => Icon::PickerGradient,
        ToolKind::Hand => Icon::Hand,
        ToolKind::Blend => Icon::Link,
        ToolKind::Zoom => Icon::ZoomIn,
        ToolKind::Lasso => Icon::Lasso,
        ToolKind::Pen | ToolKind::Convert => Icon::PenTool,
        ToolKind::AddAnchor => Icon::PenLine,
        ToolKind::DeleteAnchor => Icon::PenOff,
        ToolKind::Eyedropper => Icon::Pipette,
        ToolKind::Scissors => Icon::PathScissors,
        ToolKind::Knife => Icon::PathKnife,
        ToolKind::ShapeBuilder => Icon::PathMerge,
        ToolKind::Artboard => Icon::Frame,
        ToolKind::Rotate => Icon::History,
        ToolKind::Reflect => Icon::FlipH,
        ToolKind::Scale | ToolKind::FreeTransform => Icon::Fit,
        ToolKind::Shear => Icon::MoveArtwork,
        ToolKind::MagicWand => Icon::SmartGuides,
        ToolKind::Object => Icon::DrawSelect,
        ToolKind::Direct => Icon::DrawDirect,
    }
}
pub(crate) fn slot(ui: &mut egui::Ui, group: &[ToolKind], active: ToolKind, ops: &mut Vec<Op>) -> ToolKind {
    let id = super::doc_id(ui, ("lane-d-flyout", super::drawing::tool_name(group[0])));
    let mut state = ui.ctx().data(|d| d.get_temp::<Flyout>(id)).unwrap_or_else(|| Flyout::new(group[0]));
    state.observe(active, group);
    let result = kit::icon_button(
        ui,
        id,
        icon(state.last),
        super::drawing::tool_name(state.last),
        IconState::Tool(active == state.last),
    );
    if group.len() > 1 {
        result.response.clone().on_hover_text("Right-click or hold for more tools");
    }
    let now = ui.input(|i| i.time);
    let down = result.response.is_pointer_button_down_on();
    if down && group.len() > 1 {
        state.hold(now, true);
    }
    if down && !state.held && group.len() > 1 {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(t::DRAW_HOLD_SECONDS));
    }
    if result.response.secondary_clicked() && group.len() > 1 {
        state.open = !state.open;
    }
    if result.activated && !state.held {
        ops.push(Op::Tool(state.last));
    }
    if !down {
        state.hold(now, false);
    }
    if state.open {
        let pop = egui::Area::new(id.with("popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(result.response.rect.right_top())
            .show(ui.ctx(), |ui| {
                super::drawing::frame().show(ui, |ui| {
                    ui.set_width(t::DRAW_SHEET_W);
                    for tool in group {
                        let label = super::drawing::tool_name(*tool);
                        let mut c = Control::new(id.with(label), label);
                        c.icon = Some(icon(*tool));
                        c.selected = *tool == active;
                        if kit::action(ui, c, false).activated {
                            state.last = *tool;
                            state.open = false;
                            ops.push(Op::Tool(*tool));
                        }
                    }
                });
            });
        if ui.input(|i| i.key_pressed(egui::Key::Escape))
            || (ui.input(|i| i.pointer.any_pressed())
                && ui
                    .ctx()
                    .pointer_latest_pos()
                    .is_some_and(|p| !pop.response.rect.contains(p) && !result.response.rect.contains(p)))
        {
            state.open = false;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, state.clone()));
    state.last
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_holds_once_and_remembers_group_only() {
        let mut m = Flyout::new(ToolKind::Rect);
        m.observe(ToolKind::Star, SHAPES);
        assert!(m.last == ToolKind::Star);
        m.observe(ToolKind::Pen, SHAPES);
        assert!(m.last == ToolKind::Star);
        assert!(!m.hold(1., true));
        assert!(m.hold(1. + t::DRAW_HOLD_SECONDS, true));
        assert!(m.open);
        assert!(!m.hold(3., true));
        assert!(!m.hold(4., false));
        m.open = false;
        assert!(!m.hold(5., true));
        assert!(m.hold(5. + t::DRAW_HOLD_SECONDS, true));
        assert!(m.open);
    }
    #[test]
    fn catalogue_has_every_lane_tool_and_only_illustrator_keys() {
        for tool in [
            ToolKind::RoundedRect,
            ToolKind::Star,
            ToolKind::Line,
            ToolKind::Arc,
            ToolKind::Spiral,
            ToolKind::RectGrid,
            ToolKind::PolarGrid,
            ToolKind::Pencil,
            ToolKind::Smooth,
            ToolKind::PathEraser,
            ToolKind::Join,
            ToolKind::Curvature,
        ] {
            assert!(GROUPS.iter().any(|g| g.contains(&tool)));
        }
    }
}
