//! Provisional Advanced rows use the incumbent kit; owner design review pending.
use super::ExportSheet;
use egui::Id;
use varos_app::shell::{
    kit::{
        self,
        field::{self, Label, NumberField},
        Control,
    },
    tokens as t,
};
use varos_raster::{
    export::Format,
    screens::{Row, Subfolders},
};
fn button(ui: &mut egui::Ui, key: impl std::hash::Hash + std::fmt::Debug, label: &str) -> bool {
    kit::action(ui, Control::new(Id::new(key), label), false).activated
}
fn toggle(ui: &mut egui::Ui, label: &str, value: &mut bool) -> bool {
    let mut c = Control::new(Id::new(("advanced", label)), label);
    c.selected = *value;
    if kit::action(ui, c, false).activated {
        *value = !*value;
        true
    } else {
        false
    }
}
fn text(ui: &mut egui::Ui, key: impl std::hash::Hash + std::fmt::Debug, label: &str, value: &mut String) -> bool {
    kit::notice(ui, label);
    let (_, rect) = ui.allocate_space(egui::vec2(t::EXPORT_FIELD_W, t::KIT_CONTROL_H));
    let edit = field::text_field(
        ui,
        field::TextField {
            id: Id::new(key),
            rect,
            value,
            font: t::numeric_value(t::T_MICRO),
            framed: true,
            open: false,
            hint: label,
        },
        |s| Ok(s.to_owned()),
    );
    if let Some(s) = edit.commit {
        *value = s;
        true
    } else {
        false
    }
}
pub(super) fn draw(ui: &mut egui::Ui, sheet: &mut ExportSheet, running: bool) {
    if running {
        kit::notice(ui, "Exporting…");
        return;
    }
    let m = &mut sheet.minimal;
    let s = &mut m.screen_settings;
    let mut changed = false;
    ui.horizontal(|ui| {
        for (i, label) in ["iOS", "Android", "Web"].iter().enumerate() {
            if button(ui, ("export-preset", i), label) {
                s.preset(i);
                changed = true;
            }
        }
    });
    kit::notice(ui, "Scale · Suffix · Format · ×");
    let mut remove = None;
    for (i, row) in s.rows.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let e = field::number_field(
                ui,
                NumberField {
                    id: Id::new(("screen-scale", i)),
                    width: t::EXPORT_SCALE_W,
                    label: Label::Letter("×"),
                    tip: "Export scale",
                    value: row.scale,
                    decimals: 2,
                    speed: 0.1,
                    range: 0.01..=64.,
                    disabled: Format::parse(&row.format).is_ok_and(Format::vector),
                },
            );
            if let Some(v) = e.commit.or(e.live) {
                row.scale = v;
                changed = true;
            }
            let (_, rect) = ui.allocate_space(egui::vec2(t::EXPORT_SUFFIX_W, t::KIT_CONTROL_H));
            let e = field::text_field(
                ui,
                field::TextField {
                    id: Id::new(("screen-suffix", i)),
                    rect,
                    value: &row.suffix,
                    font: t::numeric_value(t::T_MICRO),
                    framed: true,
                    open: false,
                    hint: "Suffix",
                },
                |s| if s.contains(['/', '\\']) { Err("No path separators") } else { Ok(s.to_owned()) },
            );
            if let Some(v) = e.commit {
                row.suffix = v;
                changed = true;
            }
            let labels: Vec<_> = Format::ALL.iter().map(|f| f.label()).collect();
            if let Some(index) = super::paint::dropdown(ui, &format!("screen-format-{i}"), &row.format, &labels, false)
            {
                row.format = Format::ALL[index].extension().into();
                changed = true;
            }
            if button(ui, ("remove-scale", i), "×") {
                remove = Some(i);
            }
        });
        if row.format == "svg" {
            let mut expanded = ui.ctx().data(|d| d.get_temp::<bool>(Id::new(("svg-options", i)))).unwrap_or(false);
            if toggle(ui, "SVG options", &mut expanded) {
                ui.ctx().data_mut(|d| d.insert_temp(Id::new(("svg-options", i)), expanded));
            }
            if expanded {
                let mut inline = row.svg.styling == varos_core::svg::options::Styling::Inline;
                if toggle(ui, "Inline styling", &mut inline) {
                    row.svg.styling = if inline {
                        varos_core::svg::options::Styling::Inline
                    } else {
                        varos_core::svg::options::Styling::Attributes
                    };
                    changed = true;
                }
                let e = field::number_field(
                    ui,
                    NumberField {
                        id: Id::new(("svg-decimals", i)),
                        width: t::EXPORT_FIELD_W,
                        label: Label::Letter("Decimals"),
                        tip: "SVG decimal precision",
                        value: row.svg.decimals as f32,
                        decimals: 0,
                        speed: 1.,
                        range: 0.0..=8.0,
                        disabled: false,
                    },
                );
                if let Some(v) = e.commit.or(e.live) {
                    row.svg.decimals = v as u8;
                    changed = true;
                }
                changed |= toggle(ui, "Include IDs", &mut row.svg.ids);
                changed |= toggle(ui, "Minify", &mut row.svg.minify);
            }
        }
    }
    if let Some(i) = remove {
        if s.rows.len() > 1 {
            s.rows.remove(i);
            changed = true;
        }
    }
    if s.rows.len() < 32 && button(ui, "add-scale", "+ Add scale") {
        s.rows.push(Row { scale: 2., suffix: "@2x".into(), ..Row::default() });
        changed = true;
    }
    changed |= text(ui, "screen-prefix", "Prefix", &mut s.prefix);
    if text(ui, "screen-range", "Range (empty = All)", &mut s.range) {
        match s.checks(m.boards.assets.len()) {
            Ok(checks) => m.boards.checked = checks,
            Err(reason) => {
                kit::notice(ui, &reason);
            }
        };
        changed = true;
    }
    if button(ui, "screen-all", "All") {
        m.boards.checked.fill(true);
        s.range.clear();
        changed = true;
    }
    changed |= toggle(ui, "Include bleed", &mut s.include_bleed);
    changed |= toggle(ui, "Include artboard colour", &mut s.include_colour);
    changed |= toggle(ui, "Whole board as one file", &mut s.whole_board);
    changed |= toggle(ui, "Open folder after export", &mut s.open_folder);
    let label = match s.subfolders {
        Subfolders::None => "No sub-folders",
        Subfolders::Scale => "By scale",
        Subfolders::Format => "By format",
    };
    if let Some(i) =
        super::paint::dropdown(ui, "screen-subfolders", label, &["No sub-folders", "By scale", "By format"], false)
    {
        s.subfolders = [Subfolders::None, Subfolders::Scale, Subfolders::Format][i];
        changed = true;
    }
    let label = if s.pdf_single { "PDF: single file" } else { "PDF: per artboard" };
    if let Some(i) =
        super::paint::dropdown(ui, "screen-pdf-pages", label, &["PDF: single file", "PDF: per artboard"], false)
    {
        s.pdf_single = i == 0;
        changed = true;
    }
    if s.rows.iter().any(|r| r.format == "pdf") {
        crate::pdf_options::draw(ui, &mut sheet.pdf_options, &mut sheet.pdf_options_expanded, running);
    }
    if let Err(reason) = s.checks(m.boards.assets.len()) {
        kit::notice(ui, &reason);
    }
    m.preferences_dirty |= changed;
}
