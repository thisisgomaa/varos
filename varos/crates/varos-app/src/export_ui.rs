//! DFS S6-C: the Export PDF sheet — the minimal page-scope choice the work order requires before the
//! save panel (`DFS_S4_S6_ASSOCIATION_EXPORT.md` §3.3: All visible artboards / Active artboard /
//! Artwork bounds; a scope that cannot export is disabled and says why). File ▸ Export ▸ PDF… and the
//! Windows burger's Export… row send `AppCommand::ShowExport` (4b removed the band's button), which opens
//! this sheet; its Export… sends `AppCommand::ExportPdf(id, scope)` — the save panel and the job.
//!
//! Hand-painted from `shell::kit` (rows, notice, buttons) on kit tokens: no egui default widgets, no
//! shadow, no animation. Azure appears only as the kit's keyboard-focus ring. The model is pure
//! ([`ExportSheet::new`]); [`draw`] only paints it and reports what was chosen.
//!
//! Slice 0.6: a fourth scope, Selection (File ▸ Export Selection… opens the sheet on it), and the sheet
//! now STAYS through the export: [`Phase::Running`] offers Cancel (it raises the job's cancel flag;
//! until the file's final rename nothing is written), [`Phase::Cancelled`] says so, [`Phase::Done`]
//! names the file and offers Show in Finder ([`reveal`]). Each Export… takes a fresh ticket; the
//! lifecycle reports progress as `file_jobs::ExportEvent`s carrying it, and a sheet follows only its
//! own ticket ([`ExportSheet::on_event`]). A sheet opened while this tab is still exporting refuses to
//! start a second one ([`BUSY`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{Id, Layout, Margin, RichText, Stroke, TextStyle};
use varos_app::shell::kit::{self, Availability, Control};
use varos_app::shell::tokens as t;
use varos_core::model::Document;
use varos_pdf::ExportScope;

use crate::app_command::SessionId;
use crate::file_jobs::{next_ticket, CancelFlag, ExportEvent};

/// Why Export… is disabled while this tab's previous export is still on the worker.
pub const BUSY: &str = "An export of this document is still running.";

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

/// Where the sheet is in one export.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Choosing a scope (the rows and Export…).
    Choose,
    /// Export… was pressed with `ticket`: the save panel, then the job. `cancel` arrives with the
    /// job (`ExportEvent::Started`); Cancel raises it and the sheet waits (`cancelling`) for the
    /// outcome — Cancelled, or Done when the file was already committed.
    Running { ticket: u64, cancel: Option<CancelFlag>, cancelling: bool },
    /// The export was cancelled before its file was replaced: nothing was written.
    Cancelled,
    /// The PDF was written to `dest`: Show in Finder / Done, with the export report's notes.
    Done { dest: PathBuf, report: varos_core::ExportReport },
}

/// The open sheet: which tab it exports and the scope chosen. Built when it opens (the plans are
/// computed once, not every frame).
#[derive(Clone, Debug, PartialEq)]
pub struct ExportSheet {
    pub sid: SessionId,
    pub rows: Vec<ScopeRow>,
    pub selected: ExportScope,
    pub phase: Phase,
    /// This tab already has an export on the worker: Export… is disabled ([`BUSY`]).
    pub busy: bool,
}

/// The sheet's copy.
pub const NOTE: &str = "For sharing. Editable Varos data is not included.";

impl ExportSheet {
    /// The rows for `doc` with `selection` (the tab's selected path ids — the Selection row). The
    /// selection is `remembered` (this tab's last export, or Selection for File ▸ Export Selection…)
    /// when it can still export, else the library's default scope, else the first scope that can.
    /// `busy` = this tab still has an export on the worker.
    pub fn new(
        sid: SessionId,
        doc: &Document,
        selection: &HashSet<u32>,
        remembered: Option<ExportScope>,
        busy: bool,
    ) -> Self {
        let rows: Vec<ScopeRow> = [
            (ExportScope::AllVisibleArtboards, "All artboards"),
            (ExportScope::ActiveArtboard, "Active artboard"),
            (ExportScope::ArtworkBounds, "Artwork bounds"),
            (ExportScope::Selection, "Selection"),
        ]
        .into_iter()
        .map(|(scope, label)| {
            let plan = match scope {
                ExportScope::Selection => varos_pdf::plan_selection_export(doc, selection).map(|(_, plan)| plan),
                _ => varos_pdf::plan_pdf_export(doc, scope),
            };
            match plan {
                Ok(plan) => ScopeRow { scope, label, detail: pages(plan.page_count()), available: true },
                Err(why) => ScopeRow { scope, label, detail: why.reason().to_string(), available: false },
            }
        })
        .collect();
        let can = |s: ExportScope| rows.iter().any(|r| r.scope == s && r.available);
        let default = varos_pdf::default_scope(doc);
        let selected = remembered
            .filter(|&s| can(s))
            .or_else(|| can(default).then_some(default))
            .or_else(|| rows.iter().find(|r| r.available).map(|r| r.scope))
            .unwrap_or(default);
        ExportSheet { sid, rows, selected, phase: Phase::Choose, busy }
    }

    /// The sheet for tab `s`: its document, its selection, and whether it is still exporting. It opens
    /// on Selection for Export Selection… (`selection`), else on the tab's remembered scope (`scopes`).
    pub fn of(
        s: &crate::workspace::DocumentSession,
        selection: bool,
        scopes: &std::collections::HashMap<SessionId, ExportScope>,
    ) -> Self {
        let ed = &s.editor;
        let scope = if selection { Some(ExportScope::Selection) } else { scopes.get(&s.id).copied() };
        ExportSheet::new(s.id, &ed.doc, &ed.selected_pids(), scope, !s.exports.is_empty())
    }

    /// Export… pressed: a fresh ticket, the sheet running. `None` = it cannot export now.
    pub fn start(&mut self) -> Option<u64> {
        if !self.can_export() || !matches!(self.phase, Phase::Choose) {
            return None;
        }
        let ticket = next_ticket();
        self.phase = Phase::Running { ticket, cancel: None, cancelling: false };
        Some(ticket)
    }

    /// An export moved on. `true` = it is THIS sheet's export (same tab AND ticket) and the sheet
    /// shows it; `false` = not its export (an older one, another tab's), so the host tells a written
    /// PDF with a notice instead.
    pub fn on_event(&mut self, event: &ExportEvent) -> bool {
        let Phase::Running { ticket, cancelling, .. } = self.phase else {
            return false;
        };
        if event.sid() != self.sid || event.ticket() != ticket {
            return false;
        }
        self.phase = match event {
            ExportEvent::Started { cancel, .. } => {
                if cancelling {
                    cancel.cancel(); // Cancel was pressed before the job existed
                }
                Phase::Running { ticket, cancel: Some(cancel.clone()), cancelling }
            }
            // the commit boundary: once the file is renamed into place the export is done
            ExportEvent::Finished { dest, report, .. } => Phase::Done { dest: dest.clone(), report: report.clone() },
            ExportEvent::Cancelled { .. } => Phase::Cancelled,
            // the save panel was cancelled, or the export failed (already told): back to the choice
            ExportEvent::Ended { .. } => Phase::Choose,
        };
        true
    }

    /// Cancel / Esc while running: raise the job's flag and wait for the outcome (Cancelled, or Done
    /// when the file was already committed). `false` = nothing is running: the sheet just closes.
    pub fn cancel(&mut self) -> bool {
        let Phase::Running { cancel, cancelling, .. } = &mut self.phase else {
            return false;
        };
        if *cancelling {
            return false; // a second Cancel / Esc closes the sheet; the flag is already up
        }
        if let Some(flag) = cancel {
            flag.cancel();
        }
        *cancelling = true;
        true
    }

    /// Export… is enabled: the selected scope can export and no export of this tab is running.
    pub fn can_export(&self) -> bool {
        !self.busy && self.rows.iter().any(|r| r.scope == self.selected && r.available)
    }

    /// Why Export… is disabled (an export still running, else the selected scope's reason).
    pub fn reason(&self) -> Option<&str> {
        if self.busy {
            return Some(BUSY);
        }
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
#[derive(Clone, Debug, PartialEq)]
pub enum SheetAction {
    Stay,
    /// Cancel, Esc, or a press outside the sheet.
    Close,
    /// Export…: the host runs `AppCommand::ExportPdf(id, scope, ticket)`; the sheet stays, running.
    Export(SessionId, ExportScope, u64),
    /// Show in Finder (the done state): reveal the written PDF; the sheet closes.
    Reveal(PathBuf),
}

/// The Done state's line for the export report: “1 note: …” / “N notes: a; b”. `None` = nothing to say.
pub fn report_text(report: &varos_core::ExportReport) -> Option<String> {
    let n = report.notes.len();
    if n == 0 {
        return None;
    }
    let messages: Vec<&str> = report.notes.iter().map(|note| note.message.as_str()).collect();
    Some(format!("{n} {}: {}", if n == 1 { "note" } else { "notes" }, messages.join("; ")))
}

/// [`ExportSheet::on_event`] for the Ui's optional sheet.
pub fn on_event(sheet: &mut Option<ExportSheet>, event: &ExportEvent) -> bool {
    sheet.as_mut().is_some_and(|s| s.on_event(event))
}

/// The command that shows `path` selected in the file manager: Finder (`open -R`) on macOS, Explorer
/// (`/select,`) on Windows, the folder elsewhere.
pub fn reveal_command(path: &Path) -> std::process::Command {
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("/usr/bin/open");
        c.arg("-R").arg(path);
        c
    }
    #[cfg(windows)]
    {
        let mut c = std::process::Command::new("explorer");
        c.arg(format!("/select,{}", path.display()));
        c
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path.parent().unwrap_or(path));
        c
    }
}

/// Show in Finder: run [`reveal_command`] without waiting on the UI thread (a helper thread reaps it).
pub fn reveal(path: &Path) {
    match reveal_command(path).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => eprintln!("[varos] Show in Finder failed: {e}"),
    }
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
                if let Phase::Done { dest, report } = &sheet.phase {
                    let name =
                        dest.file_name().map_or_else(|| dest.display().to_string(), |n| n.to_string_lossy().into());
                    kit::notice(ui, &format!("Exported {name}. Your document has not changed."));
                    if let Some(notes) = report_text(report) {
                        kit::notice(ui, &notes); // muted kit text: what the export simplified or left out
                    }
                    ui.add_space(t::KIT_GAP);
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::action(ui, Control::new(Id::new("export-reveal"), "Show in Finder"), false).activated {
                            action = SheetAction::Reveal(dest.clone());
                        }
                        if kit::action(ui, Control::new(Id::new("export-done"), "Done"), false).activated {
                            action = SheetAction::Close;
                        }
                    });
                    return;
                }
                if sheet.phase == Phase::Cancelled {
                    kit::notice(ui, "Export cancelled. Nothing was written.");
                    ui.add_space(t::KIT_GAP);
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::action(ui, Control::new(Id::new("export-done"), "Done"), false).activated {
                            action = SheetAction::Close;
                        }
                    });
                    return;
                }
                let running = matches!(sheet.phase, Phase::Running { .. });
                let cancelling = matches!(sheet.phase, Phase::Running { cancelling: true, .. });
                let mut picked = None;
                for row in &sheet.rows {
                    let mut c = Control::new(Id::new(("export-scope", row.label)), row.label);
                    c.selected = row.scope == sheet.selected;
                    if !row.available {
                        c.availability = Availability::Disabled(&row.detail);
                    } else if running {
                        c.availability = Availability::Disabled("Exporting\u{2026}");
                    }
                    if kit::list_row(ui, c, &row.detail).activated {
                        picked = Some(row.scope);
                    }
                }
                if let Some(scope) = picked.filter(|_| !running) {
                    sheet.select(scope);
                }
                ui.add_space(t::KIT_GAP);
                kit::notice(ui, NOTE);
                ui.add_space(t::KIT_GAP);
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut export = Control::new(Id::new("export-pdf"), "Export\u{2026}");
                    if cancelling {
                        export.availability = Availability::Busy("Cancelling\u{2026}");
                    } else if running {
                        export.availability = Availability::Busy("Exporting\u{2026}");
                    } else if let Some(reason) = sheet.reason() {
                        export.availability = Availability::Disabled(reason);
                    }
                    if kit::action(ui, export, false).activated {
                        if let Some(ticket) = sheet.start() {
                            action = SheetAction::Export(sheet.sid, sheet.selected, ticket);
                        }
                    }
                    // running: Cancel raises the job's flag and waits for its outcome; else it closes
                    if kit::action(ui, Control::new(Id::new("export-cancel"), "Cancel"), false).activated
                        && !sheet.cancel()
                    {
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
    if action == SheetAction::Stay && escape {
        if !sheet.cancel() {
            action = SheetAction::Close; // Esc = Cancel: a running export is called off first
        }
    } else if action == SheetAction::Stay && outside && !matches!(sheet.phase, Phase::Running { .. }) {
        action = SheetAction::Close; // a press elsewhere never drops a running export's Cancel
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
    fn none() -> HashSet<u32> {
        HashSet::new()
    }
    fn board(x: f32, hidden: bool) -> Artboard {
        Artboard { x, y: 0.0, w: 100.0, h: 100.0, name: "B".into(), hidden, ..Artboard::default() }
    }

    #[test]
    fn two_boards_default_to_all_with_page_counts_and_artwork_bounds_disabled() {
        let doc = Document { artboards: vec![board(0.0, false), board(200.0, false)], ..Document::default() };
        let sheet = ExportSheet::new(SessionId(1), &doc, &none(), None, false);
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
                ("Selection", "Select something to export it.", false),
            ]
        );
    }

    #[test]
    fn boardless_document_selects_artwork_bounds_and_a_disabled_row_is_never_selected() {
        let doc = Document { paths: vec![square(1, 0.0)], ids: 100, ..Document::default() };
        let mut sheet = ExportSheet::new(SessionId(1), &doc, &none(), Some(ExportScope::AllVisibleArtboards), false);
        assert_eq!(sheet.selected, ExportScope::ArtworkBounds, "the remembered scope no longer applies");
        sheet.select(ExportScope::ActiveArtboard);
        assert_eq!(sheet.selected, ExportScope::ArtworkBounds);
        // nothing visible at all: nothing can export, Export… is disabled with the reason
        let empty = ExportSheet::new(SessionId(1), &Document::default(), &none(), None, false);
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
        let sheet = ExportSheet::new(SessionId(1), &doc, &none(), Some(ExportScope::ActiveArtboard), false);
        assert_eq!(sheet.selected, ExportScope::ActiveArtboard);
        assert_eq!(sheet.rows[0].detail, "1 page", "the hidden board is not a page");
    }

    /// Slice 0.6: File ▸ Export Selection… opens the sheet ON the Selection row (one page); with no
    /// selection that row is disabled with its reason and the sheet falls back to the default scope.
    #[test]
    fn export_selection_opens_on_the_selection_row() {
        let doc = Document {
            artboards: vec![board(0.0, false)],
            paths: vec![square(1, 0.0), square(2, 50.0)],
            ids: 100,
            ..Document::default()
        };
        let picked: HashSet<u32> = [2].into();
        let sheet = ExportSheet::new(SessionId(1), &doc, &picked, Some(ExportScope::Selection), false);
        assert_eq!(sheet.selected, ExportScope::Selection);
        let row = sheet.rows.iter().find(|r| r.scope == ExportScope::Selection).unwrap();
        assert_eq!((row.label, row.detail.as_str(), row.available), ("Selection", "1 page", true));
        let unselected = ExportSheet::new(SessionId(1), &doc, &none(), Some(ExportScope::Selection), false);
        assert_eq!(unselected.selected, ExportScope::AllVisibleArtboards, "no selection: the default scope");
    }

    fn board_doc() -> Document {
        Document { artboards: vec![board(0.0, false)], ..Document::default() }
    }

    /// Slice 0.6: the sheet stays through the export — Running (Cancel raises the job's flag and waits),
    /// Cancelled, Done (Show in Finder), back to the choice when the panel was cancelled; it follows
    /// only its own ticket.
    #[test]
    fn the_sheet_follows_its_export_and_cancel_raises_the_flag() {
        let sid = SessionId(1);
        let mut sheet = Some(ExportSheet::new(sid, &board_doc(), &none(), None, false));
        let flag = CancelFlag::default();
        assert!(!on_event(&mut sheet, &ExportEvent::Started { sid, ticket: 1, cancel: flag.clone() }), "choosing");
        let ticket = sheet.as_mut().unwrap().start().expect("Export… starts");
        let started = ExportEvent::Started { sid, ticket, cancel: flag.clone() };
        assert!(!on_event(&mut sheet, &ExportEvent::Started { sid: SessionId(2), ticket, cancel: flag.clone() }));
        assert!(on_event(&mut sheet, &started));
        assert_eq!(
            sheet.as_ref().unwrap().phase,
            Phase::Running { ticket, cancel: Some(flag.clone()), cancelling: false }
        );
        assert!(sheet.as_mut().unwrap().cancel(), "Cancel while running: the sheet stays and waits");
        assert!(flag.is_cancelled(), "Cancel raises the running job's flag");
        assert!(!sheet.as_mut().unwrap().cancel(), "a second Cancel closes the sheet");
        assert!(on_event(&mut sheet, &ExportEvent::Cancelled { sid, ticket }));
        assert_eq!(sheet.as_ref().unwrap().phase, Phase::Cancelled, "“Export cancelled. Nothing was written.”");
        // the save panel was cancelled: back to choosing
        let mut sheet = Some(ExportSheet::new(sid, &board_doc(), &none(), None, false));
        let ticket = sheet.as_mut().unwrap().start().unwrap();
        assert!(on_event(&mut sheet, &ExportEvent::Ended { sid, ticket }));
        assert_eq!(sheet.as_ref().unwrap().phase, Phase::Choose);
        // a written PDF (committed before Cancel could stop it): the done state, with the file
        let ticket = sheet.as_mut().unwrap().start().unwrap();
        let dest = PathBuf::from("/w/Logo.pdf");
        assert!(on_event(
            &mut sheet,
            &ExportEvent::Finished { sid, ticket, dest: dest.clone(), report: Default::default() }
        ));
        assert_eq!(sheet.as_ref().unwrap().phase, Phase::Done { dest, report: Default::default() });
        // Cancel pressed before the job existed: the flag is raised the moment it arrives
        let mut sheet = Some(ExportSheet::new(sid, &board_doc(), &none(), None, false));
        let ticket = sheet.as_mut().unwrap().start().unwrap();
        assert!(sheet.as_mut().unwrap().cancel());
        let late = CancelFlag::default();
        assert!(on_event(&mut sheet, &ExportEvent::Started { sid, ticket, cancel: late.clone() }));
        assert!(late.is_cancelled());
        // no sheet: nobody shows it (the host tells it instead)
        assert!(!on_event(
            &mut None,
            &ExportEvent::Finished { sid, ticket: 9, dest: "/w/a.pdf".into(), report: Default::default() }
        ));
    }

    /// Review R4: export A runs, the sheet is reopened and export B started — A's completion must not
    /// mark B's sheet done (tickets); and a sheet opened while A is still on the worker refuses to
    /// start another export, saying why.
    #[test]
    fn a_reopened_sheet_follows_only_its_own_export() {
        let sid = SessionId(1);
        let mut a = ExportSheet::new(sid, &board_doc(), &none(), None, false);
        let ticket_a = a.start().unwrap();
        // the sheet is reopened while A runs: Export… is refused with the reason
        let mut busy = Some(ExportSheet::new(sid, &board_doc(), &none(), None, true));
        assert!(!busy.as_ref().unwrap().can_export());
        assert_eq!(busy.as_ref().unwrap().reason(), Some(BUSY));
        assert_eq!(busy.as_mut().unwrap().start(), None);
        // A finished in the meantime: B's sheet (fresh ticket) ignores A's events
        let mut b = Some(ExportSheet::new(sid, &board_doc(), &none(), None, false));
        let ticket_b = b.as_mut().unwrap().start().unwrap();
        assert_ne!(ticket_a, ticket_b);
        let a_done =
            ExportEvent::Finished { sid, ticket: ticket_a, dest: "/w/A.pdf".into(), report: Default::default() };
        assert!(!on_event(&mut b, &a_done), "A's file is not B's: the host tells it with a notice");
        assert!(!on_event(&mut b, &ExportEvent::Cancelled { sid, ticket: ticket_a }));
        assert!(matches!(b.as_ref().unwrap().phase, Phase::Running { ticket, .. } if ticket == ticket_b));
        let b_done =
            ExportEvent::Finished { sid, ticket: ticket_b, dest: "/w/B.pdf".into(), report: Default::default() };
        assert!(on_event(&mut b, &b_done));
        assert_eq!(b.as_ref().unwrap().phase, Phase::Done { dest: "/w/B.pdf".into(), report: Default::default() });
    }

    /// Merge with 0.1: the export report reaches the sheet's Done state as one muted line.
    #[test]
    fn the_done_state_lists_the_export_report_notes() {
        use varos_core::export::ExportNote;
        let note = |m: &str| ExportNote { kind: "simplified".into(), object_id: Some(4), message: m.into() };
        assert_eq!(report_text(&Default::default()), None, "nothing to say: no line");
        let one = varos_core::ExportReport { notes: vec![note("Stroke dashes were drawn solid.")] };
        assert_eq!(report_text(&one).as_deref(), Some("1 note: Stroke dashes were drawn solid."));
        let two = varos_core::ExportReport { notes: vec![note("A"), note("B")] };
        assert_eq!(report_text(&two).as_deref(), Some("2 notes: A; B"));
        // the report travels with the Finished event into the Done state
        let sid = SessionId(1);
        let mut sheet = Some(ExportSheet::new(sid, &board_doc(), &none(), None, false));
        let ticket = sheet.as_mut().unwrap().start().unwrap();
        let dest = PathBuf::from("/w/x.pdf");
        assert!(on_event(&mut sheet, &ExportEvent::Finished { sid, ticket, dest: dest.clone(), report: two.clone() }));
        assert_eq!(sheet.unwrap().phase, Phase::Done { dest, report: two });
    }

    /// Show in Finder = `open -R <file>` (the file selected in its folder), never a shell string.
    #[cfg(target_os = "macos")]
    #[test]
    fn show_in_finder_is_open_dash_r_on_the_file() {
        let path = Path::new("/Users/me/Exports/Logo final.pdf");
        let c = reveal_command(path);
        assert_eq!(c.get_program(), "/usr/bin/open");
        let args: Vec<&std::ffi::OsStr> = c.get_args().collect();
        assert_eq!(args, [std::ffi::OsStr::new("-R"), path.as_os_str()]);
    }
}
