//! Provisional breadcrumb; isolation scope is transient and never persisted.
// ---- Lane F: shaped chrome ----
use varos_app::shell::kit::text::ShapedUi as _;
// ---- end Lane F ----
use varos_app::shell::{kit, tokens as t};
use varos_core::{EditCommand, Editor};
pub(super) fn draw(ctx: &egui::Context, ed: &mut Editor, hole: egui::Rect) {
    let Some(node) = ed.select_transform.isolation else { return };
    if ed.doc.node(node).is_none() {
        ed.execute_ui(EditCommand::Isolate(None));
        return;
    }
    let mut path = vec![];
    let mut at = Some(node);
    while let Some(n) = at.and_then(|n| ed.doc.node(n)) {
        path.push(if n.name.is_empty() { "Group".into() } else { n.name.clone() });
        at = n.parent;
    }
    path.reverse();
    egui::Area::new(egui::Id::new("isolation-breadcrumb"))
        .order(egui::Order::Middle)
        .fixed_pos(hole.left_top() + egui::vec2(t::SLICE4A_TOOLS_BREADCRUMB_X, t::KIT_PAD))
        .show(ctx, |ui| {
            egui::Frame {
                fill: t::PANEL,
                stroke: egui::Stroke::new(t::KIT_STROKE, t::LINE),
                corner_radius: egui::CornerRadius::same(t::RBOX),
                inner_margin: egui::Margin::same(t::SLICE4A_TOOLS_MARGIN),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if kit::action(
                        ui,
                        kit::Control::new(super::doc_id(ui, "isolation-exit"), "Exit isolation (Esc)"),
                        false,
                    )
                    .activated
                    {
                        ed.execute_ui(EditCommand::Isolate(None));
                    }
                    ui.shaped_authored_label(t::micro_label(path.join(" / ")));
                });
            });
        });
}
