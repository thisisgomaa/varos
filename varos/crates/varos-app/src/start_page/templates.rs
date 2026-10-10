//! Provisional Templates section: reuse Recent's kit table rows and manual keyboard ring.
// ---- Lane F: shaped chrome ----
use crate::shell::kit::text::ShapedUi as _;
// ---- end Lane F ----
use super::{Frame, Slot, StartPage};
use crate::{
    shell::{
        kit::{self, board as kb},
        tokens as t,
    },
    start::StartAction,
};
pub(super) fn draw(page: &mut StartPage, ui: &mut egui::Ui, f: &mut Frame<'_>) {
    ui.add_space(t::KIT_GAP);
    ui.shaped_label(egui::RichText::new("Templates").color(t::TEXT).font(t::body()));
    let model = f.model;
    for path in &model.templates {
        let label = path.file_stem().unwrap_or_default().to_string_lossy();
        let id = ui.id().with(("template", path));
        let slot = Slot::Template(path.clone());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), t::SB_ROW_H), egui::Sense::hover());
        let response = kb::table_row(ui, id, rect, page.ring_on(&slot), &label);
        page.mark(f, &slot, rect);
        let g = ui.painter().layout_no_wrap(label.into_owned(), t::body(), t::TEXT);
        kb::galley_in_line(ui.painter(), rect.left() + t::SB_NUM_PAD, rect.top(), rect.height(), g, t::TEXT);
        if response.activated {
            f.actions.push(StartAction::OpenTemplate(path.clone()));
            f.clicked = Some(slot);
        }
    }
    if f.model.templates.is_empty() {
        kit::notice(ui, "Save a document as a template to reuse it here.");
    }
}
