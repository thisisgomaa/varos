//! Lane G: semantic kit contract. Logical Unicode labels are passed unchanged (including Arabic),
//! never shaped glyph order. Native menus are AppKit-accessible; kit menus use MenuItem.
use accesskit::Role;
/// Attach a role/name/value to the existing egui ID without changing paint or keyboard traversal.
pub fn emit(ui: &egui::Ui, id: egui::Id, role: Role, name: &str, value: Option<&str>, enabled: bool) {
    ui.ctx().accesskit_node_builder(id, |node| {
        node.set_role(role);
        node.set_label(if name.trim().is_empty() { "Value" } else { name });
        if let Some(value) = value {
            node.set_value(value);
        }
        if !enabled {
            node.set_disabled();
        }
    });
}
/// Painter-only labels (pills/key chips) participate as static text, with deterministic identities.
pub fn painted_label(painter: &egui::Painter, rect: egui::Rect, text: &str) {
    let id = egui::Id::new(("kit-static-label", text, rect.min.x.to_bits(), rect.min.y.to_bits()));
    painter.ctx().accesskit_node_builder(id, |node| {
        node.set_role(Role::Label);
        node.set_label(text);
        node.set_bounds(accesskit::Rect::new(
            rect.min.x as f64,
            rect.min.y as f64,
            rect.max.x as f64,
            rect.max.y as f64,
        ));
    });
}
