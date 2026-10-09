//! Provisional PDF section, inheriting the export sheet's kit. Owner design review pending.
use egui::Id;
use varos_app::shell::{
    kit::{self, Availability, Control},
    tokens as t,
};
use varos_pdf::{PdfOptions, PdfPreset};
/// Tiny sheet hook. The caller disables editing while a job is running.
pub fn draw(ui: &mut egui::Ui, options: &mut PdfOptions, expanded: &mut bool, running: bool) {
    if kit::action(ui, Control::new(Id::new("pdf-options-toggle"), "PDF options…"), false).activated {
        *expanded = !*expanded;
    }
    if !*expanded {
        return;
    }
    ui.horizontal(|ui| {
        for (label, preset) in [
            // ---- w3-cmyk ----
            ("PDF/X-4", PdfPreset::PdfX4),
            ("Print", PdfPreset::Print),
            ("Press", PdfPreset::Press),
            ("Smallest", PdfPreset::Smallest),
            ("Custom", PdfPreset::Custom),
        ] {
            let mut control = Control::new(Id::new(("pdf-preset", label)), label);
            control.selected = options.preset == preset;
            if running {
                control.availability = Availability::Disabled("Exporting…");
            }
            if kit::action(ui, control, false).activated {
                *options = PdfOptions::preset(preset);
            }
        }
    });
    ui.horizontal(|ui| {
        for ppi in [72, 150, 300] {
            let label = ppi.to_string();
            let mut c = Control::new(Id::new(("pdf-ppi", ppi)), &label);
            c.selected = options.image_ppi == ppi;
            if running {
                c.availability = Availability::Disabled("Exporting…");
            }
            if kit::action(ui, c, false).activated {
                options.image_ppi = ppi;
                options.preset = PdfPreset::Custom;
            }
        }
    });
    let edit = kit::field::number_field(
        ui,
        kit::field::NumberField {
            id: Id::new("pdf-custom-ppi"),
            width: ui.available_width(),
            label: kit::field::Label::Letter("ppi"),
            tip: "Raster content only; this document has no raster content.",
            value: options.image_ppi as f32,
            decimals: 0,
            speed: 1.0,
            range: 1.0..=9600.0,
            disabled: running,
        },
    );
    if let Some(value) = edit.commit.or(edit.live) {
        options.image_ppi = value.round() as u32;
        options.preset = PdfPreset::Custom;
    }
    for (label, value) in [
        ("Compress streams", &mut options.compress_streams),
        ("Use document bleed", &mut options.boxes.bleed),
        ("Crop marks", &mut options.marks.crop),
        ("Registration marks", &mut options.marks.registration),
        ("Page information", &mut options.marks.page_info),
    ] {
        let mut c = Control::new(Id::new(("pdf-option", label)), label);
        c.selected = *value;
        if running {
            c.availability = Availability::Disabled("Exporting…");
        }
        if kit::list_row(ui, c, "").activated {
            *value = !*value;
            options.preset = PdfPreset::Custom;
        }
    }
    ui.add_space(t::KIT_GAP);
    kit::notice(ui, "Image ppi: no raster content. Colour bars and PDF/X come later.");
}
