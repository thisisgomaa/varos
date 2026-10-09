use super::*;

pub(crate) fn tools(ctx: &egui::Context) -> Vec<ToolBtn> {
    let defs = [
        (ToolKind::Object, LEGACY_SELECT, "Selection (V)", false),
        (ToolKind::Direct, LEGACY_DIRECT, "Direct Selection (A); Option-click climbs groups", false),
        (ToolKind::Artboard, LEGACY_ARTBOARD, "Artboard (Shift+O)", true),
        (ToolKind::Pen, LEGACY_PEN, "Pen (P)", false),
        (ToolKind::Rotate, LEGACY_ROTATE, "Rotate (R)", false),
        (ToolKind::Scale, LEGACY_SCALE, "Scale (S)", true),
        (ToolKind::Eyedropper, LEGACY_EYE, "Eyedropper (I)", false),
    ];
    let mut tools: Vec<_> = defs
        .iter()
        .enumerate()
        .map(|(i, (kind, svg, tip, group_end))| ToolBtn {
            kind: *kind,
            tip,
            tex: legacy_texture(ctx, &format!("ic-{i}"), svg, false),
            group_end: *group_end,
        })
        .collect();
    for (at, kind, icon, tip, group_end) in [
        (2, ToolKind::Lasso, Icon::Lasso, "Lasso (Q); Shift adds; an object selection selects objects", false),
        (5, ToolKind::Convert, Icon::PenTool, "Anchor Point (Shift+C)", false),
        (6, ToolKind::AddAnchor, Icon::PenLine, "Add Anchor (+)", false),
        (7, ToolKind::DeleteAnchor, Icon::PenOff, "Delete Anchor (-)", true),
    ] {
        tools.insert(at, ToolBtn { kind, tip, tex: icon.texture(ctx), group_end });
    }
    for (kind, icon, tip) in [
        (ToolKind::Hand, Icon::Hand, "Hand (H)"),
        (ToolKind::Zoom, Icon::ZoomIn, "Zoom (Z); Option-click zooms out; drag frames the area"),
    ] {
        tools.push(ToolBtn { kind, tex: icon.texture(ctx), tip, group_end: false });
    }
    tools
}

/// HAND 2 — the floating tool rail (§4.4), pinned INSIDE the board box: a normal box look (panel
/// fill + hairline + rounded, NO shadow) floating over the canvas hole. Never a tile.
pub(crate) fn board_rail(
    ctx: &egui::Context,
    board: egui::Rect,
    tools: &[ToolBtn],
    shapes: &[ToolBtn],
    shape_active: &mut ToolKind,
    s: &Snap,
    ops: &mut Vec<Op>,
) {
    // the shapes slot mirrors whichever shape tool is actually active (so the M/L keys update it too)
    if shapes.iter().any(|t| t.kind == s.tool) {
        *shape_active = s.tool;
    }
    egui::Area::new(egui::Id::new("hand2-rail"))
        .order(egui::Order::Middle)
        .pivot(Align2::LEFT_CENTER)
        .fixed_pos(egui::pos2(board.left() + 16.0, board.center().y)) // ALWAYS vertically centred (Ahmed 07-07)
        .show(ctx, |ui| {
            egui::Frame {
                fill: SOLID_PANEL,
                stroke: Stroke::new(1.0, BORDER),
                corner_radius: CornerRadius::same(RBOX),
                inner_margin: Margin::same(6),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for t in tools {
                    if icon_button(ui, &t.tex, s.tool == t.kind).on_hover_text(t.tip).clicked() {
                        ops.push(Op::Tool(t.kind));
                    }
                    if t.group_end {
                        divider(ui);
                    }
                    if t.kind == ToolKind::Pen {
                        // the SHAPES slot sits right after Pen
                        shape_slot(ui, shapes, shape_active, s, ops);
                        divider(ui);
                    }
                }
                construction_tools(ui, s.tool, ops);
                divider(ui);
                fill_stroke_control(ui, s, ops); // Illustrator's fill/stroke box at the rail foot
            });
        });
}
