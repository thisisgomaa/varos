//! Provisional kit sheet; owner design review pending (Phase 1.1/1.5).
use crate::app_command::SessionId;
use egui::{Id, RichText, Stroke};
use varos_app::shell::{
    kit::{self, Control},
    tokens as t,
};
use varos_core::{document_setup as setup, model::Document, EditCommand, Editor};

pub struct Sheet {
    sid: SessionId,
    info: bool,
    linked: bool,
    report: serde_json::Value,
}
impl Sheet {
    pub fn new(sid: SessionId, info: bool, doc: &Document) -> Self {
        Self { sid, info, linked: true, report: setup::info(doc) }
    }
}
fn number(
    ui: &mut egui::Ui,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    ops: &mut Vec<crate::ui::ops::Op>,
    command: impl Fn(f32) -> EditCommand,
) {
    crate::ui::fields::num(
        ui,
        t::DOC_SHEET_FIELD_W,
        kit::field::Label::Letter(label),
        label,
        value,
        2,
        0.1,
        range,
        ops,
        |v| crate::ui::ops::Op::DocumentSetup(command(v)),
    );
}
pub fn draw(
    ctx: &egui::Context,
    sheet: &mut Option<Sheet>,
    ed: &Editor,
    active: Option<SessionId>,
    ops: &mut Vec<crate::ui::ops::Op>,
) {
    let Some(s) = sheet.as_mut() else { return };
    if active != Some(s.sid) {
        *sheet = None;
        return;
    }
    let mut close = false;
    egui::Area::new(Id::new("document-sheet"))
        .order(egui::Order::Foreground)
        .fixed_pos(ctx.content_rect().center() - egui::vec2(t::DOC_SHEET_W / 2.0, t::DOC_SHEET_TOP))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(t::PANEL)
                .stroke(Stroke::new(t::KIT_STROKE, t::LINE2))
                .corner_radius(t::r_box())
                .inner_margin(egui::Margin::same((t::KIT_PAD * 2.0) as i8))
                .show(ui, |ui| {
                    ui.set_width(t::DOC_SHEET_W);
                    ui.spacing_mut().item_spacing = egui::vec2(t::KIT_GAP, t::KIT_TEXT_GAP);
                    ui.label(
                        RichText::new(if s.info { "Document Info" } else { "Document Setup" })
                            .color(t::TEXT)
                            .font(t::body()),
                    );
                    if s.info {
                        // Recompute after edits while open, using the same count projection as Bridge.
                        s.report = setup::info(&ed.doc);
                        for key in ["paths", "groups", "layers", "artboards"] {
                            kit::notice(ui, &format!("{}: {}", key, s.report["counts"][key]));
                        }
                        kit::notice(
                            ui,
                            &format!("Colours used: {}", s.report["colours"].as_array().map_or(0, Vec::len)),
                        );
                        egui::ScrollArea::vertical().max_height(t::DOC_INFO_LIST_H).show(ui, |ui| {
                            if let Some(colours) = s.report["colours"].as_array() {
                                for colour in colours {
                                    if let Some(parts) = colour.as_array().filter(|p| p.len() == 4) {
                                        let byte = |i: usize| {
                                            (parts[i].as_f64().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8
                                        };
                                        kit::notice(
                                            ui,
                                            &format!(
                                                "Colour #{:02X}{:02X}{:02X}{:02X}",
                                                byte(0),
                                                byte(1),
                                                byte(2),
                                                byte(3)
                                            ),
                                        );
                                    }
                                }
                            }
                            for ab in &ed.doc.artboards {
                                kit::notice(
                                    ui,
                                    &format!(
                                        "{} — {} × {} — {} ppi",
                                        ab.name,
                                        varos_core::units::format_pt(ab.w, ed.doc.units, 2),
                                        varos_core::units::format_pt(ab.h, ed.doc.units, 2),
                                        ed.doc.units.ppi
                                    ),
                                );
                            }
                        });
                        kit::notice(ui, "Links: —");
                        kit::notice(ui, "Fonts: —");
                    } else {
                        let units = varos_core::Unit::ALL.map(|u| u.suffix());
                        if let Some(i) = kit::text_dropdown(
                            ui,
                            Id::new("setup-units"),
                            ed.doc.units.display.suffix(),
                            &units,
                            t::DOC_UNITS_W,
                            "Document units",
                        ) {
                            ops.push(crate::ui::ops::Op::DocumentSetup(EditCommand::SetUnits(
                                varos_core::Unit::ALL[i],
                            )));
                        }
                        number(ui, "PPI", ed.doc.units.ppi, 1.0..=9600.0, ops, EditCommand::SetPpi);
                        if let Some(ab) = ed.doc.active_artboard() {
                            kit::notice(ui, &format!("Bleed — {} ({})", ab.name, ed.doc.units.display.suffix()));
                            let edges = setup::bleed(ab);
                            let index = ed.doc.active;
                            let units = ed.doc.units;
                            let linked = s.linked;
                            for (i, label) in ["Top", "Right", "Bottom", "Left"].iter().enumerate() {
                                number(
                                    ui,
                                    label,
                                    varos_core::units::from_pt(edges[i], units.display, units.ppi),
                                    0.0..=varos_core::units::from_pt(7200.0, units.display, units.ppi),
                                    ops,
                                    |v| {
                                        let value = varos_core::units::to_pt(v, units.display, units.ppi);
                                        let mut edges = edges;
                                        if linked {
                                            edges = [value; 4];
                                        } else {
                                            edges[i] = value;
                                        }
                                        EditCommand::SetBleed { index, edges }
                                    },
                                );
                            }
                            if kit::action(ui, Control::new(Id::new("bleed-link"), "Link bleed edges"), s.linked)
                                .activated
                            {
                                s.linked = !s.linked;
                            }
                        } else {
                            kit::notice(ui, "Create an artboard to set bleed.");
                        }
                        if kit::action(
                            ui,
                            Control::new(Id::new("setup-grid"), "Transparency grid"),
                            ed.doc.transparency_grid,
                        )
                        .activated
                        {
                            ops.push(crate::ui::ops::Op::DocumentSetup(EditCommand::SetTransparencyGrid(
                                !ed.doc.transparency_grid,
                            )));
                        }
                    }
                    if kit::action(ui, Control::new(Id::new("document-sheet-done"), "Done"), false).activated {
                        close = true;
                    }
                });
        });
    if close {
        *sheet = None;
    }
}

/// Thin screen-space, canvas-clipped bleed furniture. Never enters render/export content.
pub fn guides(ctx: &egui::Context, doc: &Document, view: varos_core::View, ppp: f32, hole: Option<egui::Rect>) {
    let Some(hole) = hole else {
        return;
    };
    let painter =
        ctx.layer_painter(egui::LayerId::new(egui::Order::Background, Id::new("bleed-guides"))).with_clip_rect(hole);
    for ab in &doc.artboards {
        if let Some([x0, y0, x1, y1]) = setup::bleed_rect(ab) {
            let a = view.w2s([x0, y0]);
            let b = view.w2s([x1, y1]);
            painter.rect_stroke(
                egui::Rect::from_min_max(egui::pos2(a[0] / ppp, a[1] / ppp), egui::pos2(b[0] / ppp, b[1] / ppp)),
                egui::CornerRadius::ZERO,
                Stroke::new(t::BLEED_GUIDE_W, t::NONE_RED),
                egui::StrokeKind::Middle,
            );
        }
    }
}
