//! DFS S6-C: the Export PDF sheet — the minimal page-scope choice the work order requires before the
//! save panel (`DFS_S4_S6_ASSOCIATION_EXPORT.md` §3.3: All visible artboards / Active artboard /
//! Artwork bounds; a scope that cannot export is disabled and says why). File ▸ Export ▸ PDF… and the
//! Windows burger's Export… row send `AppCommand::ShowExport` (4b removed the band's button), which opens
//! this sheet; its Export… sends `AppCommand::ExportPdf(id, scope)` — the save panel and the job.
//!
//! Hand-painted from `shell::kit` (rows, notice, buttons) on kit tokens: no egui default widgets, no
//! shadow, no animation. Azure appears only as the kit's keyboard-focus ring. The model is pure
//! ([`ExportSheet::new`]); [`draw`] only paints it and reports what was chosen.

use egui::{Id, Layout, Margin, RichText, Stroke, TextStyle};
use varos_app::shell::kit::{self, Availability, Control};
use varos_app::shell::tokens as t;
use varos_core::model::Document;
use varos_pdf::ExportScope;

use crate::app_command::SessionId;

/// The sheet's width (points).
const SHEET_W: f32 = 320.0;

/// One page-scope row.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeRow {
    pub scope: ExportScope,
    pub label: &'static str,
    /// “2 pages”, or the plain-English reason this scope cannot export.
    pub detail: String,
    pub available: bool,
}

/// The open sheet: which tab it exports and the scope chosen. Built when it opens (the plans are
/// computed once, not every frame).
#[derive(Clone, Debug, PartialEq)]
pub struct ExportSheet {
    pub sid: SessionId,
    pub rows: Vec<ScopeRow>,
    pub selected: ExportScope,
}

/// The sheet's copy.
pub const NOTE: &str = "For sharing. Editable Varos data is not included.";

impl ExportSheet {
    /// The rows for `doc`. The selection is `remembered` (this tab's last export) when it can still
    /// export, else the library's default scope, else the first scope that can.
    pub fn new(sid: SessionId, doc: &Document, remembered: Option<ExportScope>) -> Self {
        let rows: Vec<ScopeRow> = [
            (ExportScope::AllVisibleArtboards, "All artboards"),
            (ExportScope::ActiveArtboard, "Active artboard"),
            (ExportScope::ArtworkBounds, "Artwork bounds"),
        ]
        .into_iter()
        .map(|(scope, label)| match varos_pdf::plan_pdf_export(doc, scope) {
            Ok(plan) => ScopeRow { scope, label, detail: pages(plan.page_count()), available: true },
            Err(why) => ScopeRow { scope, label, detail: why.reason().to_string(), available: false },
        })
        .collect();
        let can = |s: ExportScope| rows.iter().any(|r| r.scope == s && r.available);
        let default = varos_pdf::default_scope(doc);
        let selected = remembered
            .filter(|&s| can(s))
            .or_else(|| can(default).then_some(default))
            .or_else(|| rows.iter().find(|r| r.available).map(|r| r.scope))
            .unwrap_or(default);
        ExportSheet { sid, rows, selected }
    }

    /// Export… is enabled: the selected scope can export.
    pub fn can_export(&self) -> bool {
        self.rows.iter().any(|r| r.scope == self.selected && r.available)
    }

    /// Why Export… is disabled (the selected scope's reason).
    pub fn reason(&self) -> Option<&str> {
        self.rows.iter().find(|r| r.scope == self.selected && !r.available).map(|r| r.detail.as_str())
    }

    /// Choose `scope` (a disabled row is never selected).
    pub fn select(&mut self, scope: ExportScope) {
        if self.rows.iter().any(|r| r.scope == scope && r.available) {
            self.selected = scope;
        }
    }
}

fn pages(n: usize) -> String {
    if n == 1 {
        "1 page".into()
    } else {
        format!("{n} pages")
    }
}

/// What the user did with the sheet this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SheetAction {
    Stay,
    /// Cancel, Esc, or a press outside the sheet.
    Close,
    /// Export…: the host runs `AppCommand::ExportPdf(id, scope)`.
    Export(SessionId, ExportScope),
}

/// Where the sheet's top-left goes (4b: the band has no Export button to hang from any more): its
/// top `KIT_MENU_GAP` under the band, right-aligned to the Board box's right edge (one seam left of
/// the panel column, `panel_column`); with no column, centred. Never off the window's left edge.
pub fn sheet_pos(screen: egui::Rect, band_h: f32, panel_column: Option<egui::Rangef>) -> egui::Pos2 {
    let top = screen.top() + band_h + t::KIT_MENU_GAP;
    let left = match panel_column {
        Some(col) => col.min - t::SEAM_GAP - SHEET_W,
        None => screen.center().x - SHEET_W / 2.0,
    };
    egui::pos2(left.max(screen.left()), top)
}

/// Paint the sheet under the band (`sheet_pos`).
pub fn draw(ctx: &egui::Context, sheet: &mut ExportSheet, panel_column: Option<egui::Rangef>) -> SheetAction {
    let pos = sheet_pos(ctx.content_rect(), crate::chrome::TOPBAR.height, panel_column);
    let mut action = SheetAction::Stay;
    let pad = (t::KIT_PAD * 2.0) as i8;
    let area = egui::Area::new(Id::new("export-sheet")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::new()
            .fill(t::PANEL)
            .stroke(Stroke::new(t::KIT_STROKE, t::LINE2))
            .corner_radius(t::r_box())
            .inner_margin(Margin::same(pad))
            .show(ui, |ui| {
                ui.set_width(SHEET_W - t::KIT_PAD * 4.0);
                ui.spacing_mut().item_spacing = egui::vec2(t::KIT_GAP, t::KIT_TEXT_GAP);
                ui.label(RichText::new("Export PDF").text_style(TextStyle::Button).color(t::TEXT));
                ui.add_space(t::KIT_GAP);
                let mut picked = None;
                for row in &sheet.rows {
                    let mut c = Control::new(Id::new(("export-scope", row.label)), row.label);
                    c.selected = row.scope == sheet.selected;
                    if !row.available {
                        c.availability = Availability::Disabled(&row.detail);
                    }
                    if kit::list_row(ui, c, &row.detail).activated {
                        picked = Some(row.scope);
                    }
                }
                if let Some(scope) = picked {
                    sheet.select(scope);
                }
                ui.add_space(t::KIT_GAP);
                kit::notice(ui, NOTE);
                ui.add_space(t::KIT_GAP);
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut export = Control::new(Id::new("export-pdf"), "Export\u{2026}");
                    if let Some(reason) = sheet.reason() {
                        export.availability = Availability::Disabled(reason);
                    }
                    if kit::action(ui, export, false).activated && sheet.can_export() {
                        action = SheetAction::Export(sheet.sid, sheet.selected);
                    }
                    if kit::action(ui, Control::new(Id::new("export-cancel"), "Cancel"), false).activated {
                        action = SheetAction::Close;
                    }
                });
            });
    });
    let rect = area.response.rect;
    let (escape, outside) = ctx.input(|i| {
        let escape = i.key_pressed(egui::Key::Escape);
        let outside = i.events.iter().any(|e| {
            matches!(e, egui::Event::PointerButton { pressed: true, pos, .. }
                if !rect.contains(*pos))
        });
        (escape, outside)
    });
    if action == SheetAction::Stay && (escape || outside) {
        action = SheetAction::Close;
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use varos_core::model::{Anchor, Artboard, Path as VPath};

    fn square(id: u32, x: f32) -> VPath {
        let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
        VPath::new(
            id,
            vec![a(id * 10, [x, 10.0]), a(id * 10 + 1, [x + 20.0, 10.0]), a(id * 10 + 2, [x + 20.0, 30.0])],
            true,
            Some([1.0, 0.0, 0.0, 1.0]),
            None,
            1.0,
        )
    }
    fn board(x: f32, hidden: bool) -> Artboard {
        Artboard { x, y: 0.0, w: 100.0, h: 100.0, name: "B".into(), hidden, ..Artboard::default() }
    }

    #[test]
    fn two_boards_default_to_all_with_page_counts_and_artwork_bounds_disabled() {
        let doc = Document { artboards: vec![board(0.0, false), board(200.0, false)], ..Document::default() };
        let sheet = ExportSheet::new(SessionId(1), &doc, None);
        assert_eq!(sheet.selected, ExportScope::AllVisibleArtboards);
        assert!(sheet.can_export());
        let detail: Vec<(&str, &str, bool)> =
            sheet.rows.iter().map(|r| (r.label, r.detail.as_str(), r.available)).collect();
        assert_eq!(
            detail,
            [
                ("All artboards", "2 pages", true),
                ("Active artboard", "1 page", true),
                ("Artwork bounds", "Artwork bounds is only for documents without artboards.", false),
            ]
        );
    }

    #[test]
    fn boardless_document_selects_artwork_bounds_and_a_disabled_row_is_never_selected() {
        let doc = Document { paths: vec![square(1, 0.0)], ids: 100, ..Document::default() };
        let mut sheet = ExportSheet::new(SessionId(1), &doc, Some(ExportScope::AllVisibleArtboards));
        assert_eq!(sheet.selected, ExportScope::ArtworkBounds, "the remembered scope no longer applies");
        sheet.select(ExportScope::ActiveArtboard);
        assert_eq!(sheet.selected, ExportScope::ArtworkBounds);
        // nothing visible at all: nothing can export, Export… is disabled with the reason
        let empty = ExportSheet::new(SessionId(1), &Document::default(), None);
        assert!(!empty.can_export());
        assert_eq!(empty.reason(), Some("There is no visible artwork to export."));
    }

    /// 4b: with the Export button gone, the sheet opens under the 52-pt band (never inside it),
    /// right-aligned to the Board box (a seam left of the panel column); centred without a column.
    #[test]
    fn export_sheet_opens_below_the_band() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1512.0, 982.0));
        let band = varos_app::shell::tokens::BAND_H;
        let col = egui::Rangef::new(1212.0, 1506.0);
        let at = sheet_pos(screen, band, Some(col));
        assert_eq!(at.y, band + t::KIT_MENU_GAP, "under the band");
        assert_eq!(at.x + SHEET_W, col.min - t::SEAM_GAP, "right edge on the Board box's right edge");
        let centred = sheet_pos(screen, band, None);
        assert_eq!((centred.x + SHEET_W / 2.0, centred.y), (756.0, band + t::KIT_MENU_GAP));
        // a column so far left the sheet would leave the window: clamped to the left edge
        assert_eq!(sheet_pos(screen, band, Some(egui::Rangef::new(100.0, 400.0))).x, 0.0);
    }

    #[test]
    fn the_remembered_scope_wins_when_it_still_exports() {
        let doc = Document { artboards: vec![board(0.0, false), board(200.0, true)], ..Document::default() };
        let sheet = ExportSheet::new(SessionId(1), &doc, Some(ExportScope::ActiveArtboard));
        assert_eq!(sheet.selected, ExportScope::ActiveArtboard);
        assert_eq!(sheet.rows[0].detail, "1 page", "the hidden board is not a page");
    }
}
