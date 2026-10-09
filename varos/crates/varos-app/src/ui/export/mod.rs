//! Narrow sheet preset glue; the main shortcut table is untouched.
impl super::Ui {
    pub fn export_pdf_preset(&mut self) {
        if let Some(sheet) = self.export_sheet.as_mut() {
            sheet.minimal.options.format = varos_raster::export::Format::Pdf;
            sheet.minimal.options.scale = 1.0;
        }
    }
}

pub(super) fn wants_keyboard(ui: &super::Ui) -> bool {
    ui.export_sheet.is_some() || super::wants_keyboard(&ui.ctx)
}
