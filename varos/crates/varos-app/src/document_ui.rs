//! Provisional kit sheet; owner design review pending (Phase 1.1/1.5).
// ---- Lane F: shaped chrome ----
use varos_app::shell::kit::text::ShapedUi as _;
// ---- end Lane F ----
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
    revision: Option<u64>,
    scrubbing: bool,
}
impl Sheet {
    pub fn new(sid: SessionId, info: bool, doc: &Document) -> Self {
        Self { sid, info, linked: true, report: setup::info(doc), revision: None, scrubbing: false }
    }
    fn refresh_info(&mut self, ed: &Editor) -> bool {
        if self.revision == Some(ed.rev) {
            return false;
        }
        self.report = setup::info(&ed.doc);
        self.revision = Some(ed.rev);
        true
    }
}
#[allow(clippy::too_many_arguments)] // kit field parameters plus its setup gesture state
fn number(
    ui: &mut egui::Ui,
    label: &str,
    sid: SessionId,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    ops: &mut Vec<crate::ui::ops::Op>,
    scrubbing: &mut bool,
    command: impl Fn(f32) -> EditCommand,
) {
    let e = kit::field::number_field(
        ui,
        kit::field::NumberField {
            id: Id::new(("p1-docsetup-number", sid, label)),
            width: t::DOC_SHEET_FIELD_W,
            label: kit::field::Label::Letter(label),
            tip: label,
            value,
            decimals: 2,
            speed: 0.1,
            range,
            disabled: false,
        },
    );
    if let Some(v) = e.live {
        if ui.input(|i| i.pointer.any_down()) {
            ops.push(crate::ui::ops::Op::DocumentSetupLive(command(v), !*scrubbing));
            *scrubbing = true;
        } else {
            ops.push(crate::ui::ops::Op::DocumentSetup(command(v)));
        }
    }
    if let Some(v) = e.commit {
        ops.push(crate::ui::ops::Op::Field(Box::new(crate::ui::ops::Op::DocumentSetup(command(v)))));
    }
    if let Some(v) = e.pending {
        ops.push(crate::ui::ops::Op::FieldPending(e.id, Box::new(crate::ui::ops::Op::DocumentSetup(command(v)))));
    }
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
    if s.scrubbing && !ctx.input(|i| i.pointer.any_down()) {
        ops.push(crate::ui::ops::Op::DocumentSetupFinish);
        s.scrubbing = false;
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
                    ui.shaped_label(
                        RichText::new(if s.info { "Document Info" } else { "Document Setup" })
                            .color(t::TEXT)
                            .font(t::body()),
                    );
                    if s.info {
                        // Recompute after edits while open, using the same count projection as Bridge.
                        s.refresh_info(ed);
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
                        // ---- w3-cmyk ----
                        crate::ui::colour_management::setup(ui, ed, ops);
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
                        number(
                            ui,
                            "PPI",
                            s.sid,
                            ed.doc.units.ppi,
                            1.0..=9600.0,
                            ops,
                            &mut s.scrubbing,
                            EditCommand::SetPpi,
                        );
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
                                    s.sid,
                                    varos_core::units::from_pt(edges[i], units.display, units.ppi),
                                    0.0..=varos_core::units::from_pt(7200.0, units.display, units.ppi),
                                    ops,
                                    &mut s.scrubbing,
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

/// Settle before save, tab switch, close, or a canvas action.
pub fn settle(sheet: &mut Option<Sheet>, ed: &mut Editor) {
    if let Some(s) = sheet.as_mut().filter(|s| s.scrubbing) {
        ed.finish_document_setup();
        s.scrubbing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn info_refreshes_only_on_revision_changes() {
        let mut ed = Editor::new();
        let mut sheet = Sheet::new(SessionId(1), true, &ed.doc);
        assert!(sheet.refresh_info(&ed));
        for _ in 0..100 {
            assert!(!sheet.refresh_info(&ed));
        }
        ed.execute(EditCommand::SetPpi(300.0)).unwrap();
        assert!(sheet.refresh_info(&ed));
        assert!(!sheet.refresh_info(&ed));
    }
    #[test]
    fn setup_live_ops_share_history_and_settle_before_lifecycle() {
        use crate::ui::ops::{apply_ops, Op};
        let mut ed = Editor::new();
        ed.execute(EditCommand::AddArtboard).unwrap();
        let rev = ed.rev;
        let mut sheet = Some(Sheet::new(SessionId(1), false, &ed.doc));
        sheet.as_mut().unwrap().scrubbing = true;
        apply_ops(&mut ed, vec![Op::DocumentSetupLive(EditCommand::SetBleed { index: 0, edges: [1.0; 4] }, true)]);
        for v in [2.0, 3.0, 4.0] {
            apply_ops(&mut ed, vec![Op::DocumentSetupLive(EditCommand::SetBleed { index: 0, edges: [v; 4] }, false)]);
        }
        assert!(ed.transaction_open());
        assert_eq!(ed.rev, rev);
        settle(&mut sheet, &mut ed);
        assert!(!ed.transaction_open());
        assert_eq!(ed.rev, rev + 1);
        ed.undo();
        assert_eq!(setup::bleed(&ed.doc.artboards[0]), [0.0; 4]);
        apply_ops(
            &mut ed,
            vec![
                Op::DocumentSetupLive(EditCommand::SetPpi(144.0), true),
                Op::DocumentSetupLive(EditCommand::SetPpi(72.0), false),
                Op::DocumentSetupFinish,
            ],
        );
        ed.undo();
        assert!(ed.doc.artboards.is_empty());
    }
}
