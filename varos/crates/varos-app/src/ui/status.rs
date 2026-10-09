//! Shared muted status hint, including file autosave. No independent repaint timer.
// ---- Lane F: text adapters ----
use varos_app::shell::kit::text::{ShapedPainter as _, ShapedResponse as _};
// ---- end Lane F ----
use varos_app::shell::tokens as t;
pub(super) fn hint(ui: &mut egui::Ui, rect: egui::Rect, text: &str) {
    ui.painter().with_clip_rect(rect).shaped_text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        text,
        t::small(),
        t::MUTED,
    );
    ui.interact(rect, ui.id().with("file-status"), egui::Sense::hover()).shaped_hover_text(text);
}

/// A report is surfaced once for each live document revision, with no timer or dialog.
#[derive(Default)]
pub(crate) struct CanvasHint {
    seen: std::collections::HashMap<crate::app_command::SessionId, u64>,
    current: Option<(crate::app_command::SessionId, u64)>,
}
impl CanvasHint {
    pub fn observe(
        &mut self,
        id: crate::app_command::SessionId,
        revision: u64,
        report: &varos_core::ExportReport,
    ) -> bool {
        if !report.notes.iter().any(|note| note.kind == "stroke_simplified") || self.seen.get(&id) == Some(&revision) {
            return false;
        }
        self.seen.insert(id, revision);
        self.current = Some((id, revision));
        true
    }
    pub fn text(&self, id: Option<crate::app_command::SessionId>, revision: u64) -> &str {
        if self.current.is_some_and(|(sid, rev)| Some(sid) == id && rev == revision) {
            "Stroke simplified at this zoom"
        } else {
            ""
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canvas_hint_once_per_document_revision_without_dialogs() {
        let mut hint = CanvasHint::default();
        let id = crate::app_command::SessionId(1);
        let report = varos_core::ExportReport {
            notes: vec![varos_core::ExportNote {
                kind: "stroke_simplified".into(),
                object_id: Some(1),
                message: String::new(),
            }],
        };
        assert!(hint.observe(id, 0, &report));
        assert!(!hint.observe(id, 0, &report));
        assert_eq!(hint.text(Some(id), 0), "Stroke simplified at this zoom");
        assert_eq!(hint.text(Some(id), 1), "");
        assert!(hint.observe(id, 1, &report));
        assert!(hint.observe(crate::app_command::SessionId(2), 1, &report));
        let main = include_str!("../main.rs");
        let render = main
            .split("let rendered = if home")
            .nth(1)
            .expect("render block")
            .split("last_scene_signature = rendered")
            .next()
            .expect("render end");
        assert!(!render.contains("Dialogs::"));
    }
}
