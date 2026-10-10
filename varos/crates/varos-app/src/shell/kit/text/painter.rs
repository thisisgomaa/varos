//! Lane F: small adapter for existing hand-painted kit rows.
use super::{layout, paint};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect};
pub trait ShapedPainter {
    fn shaped_chrome(&self, pos: Pos2, anchor: Align2, text: &str, font: FontId, ink: Color32) -> Rect;
    fn shaped_text(&self, pos: Pos2, anchor: Align2, text: impl ToString, font: FontId, ink: Color32) -> Rect;
    fn shaped_galley(&self, pos: Pos2, galley: impl Into<ShapedInput>, ink: Color32);
}
impl ShapedPainter for Painter {
    fn shaped_chrome(&self, pos: Pos2, anchor: Align2, text: &str, font: FontId, ink: Color32) -> Rect {
        self.shaped_text(pos, anchor, crate::i18n::translate(self.ctx(), text), font, ink)
    }
    fn shaped_text(&self, pos: Pos2, anchor: Align2, text: impl ToString, font: FontId, ink: Color32) -> Rect {
        let text = text.to_string();
        let Some(label) = layout(self.ctx(), &text, &font, None, false) else {
            return Rect::from_min_size(pos, egui::Vec2::ZERO);
        };
        let rect = anchor.anchor_size(pos, label.size());
        paint(self, rect.min, &label, ink);
        rect
    }
    fn shaped_galley(&self, pos: Pos2, galley: impl Into<ShapedInput>, ink: Color32) {
        let galley = match galley.into() {
            ShapedInput::Label(label) => {
                paint(self, pos, &label, ink);
                return;
            }
            ShapedInput::Galley(galley) => galley,
        };
        let Some(section) = galley.job.sections.first() else { return };
        let width = galley.job.wrap.max_width;
        if let Some(mut label) =
            super::layout_rows(self.ctx(), &galley.job.text, &section.format.font_id, width, galley.job.wrap.max_rows)
        {
            // Keep each shaped baseline centred in the incumbent row cell, as `label` does.
            // Clipping must not push the final row below a legacy two-line description cell.
            let row_height = galley.size().y / galley.rows.len().max(1) as f32;
            let shaped_height = label.size().y / label.layout.lines.len().max(1) as f32;
            for (i, line) in std::sync::Arc::make_mut(&mut label.layout).lines.iter_mut().enumerate() {
                let dy = (i as f32 + 0.5) * (row_height - shaped_height);
                line.baseline += dy;
                for glyph in &mut line.glyphs {
                    glyph.y += dy;
                }
            }
            label.size.y = row_height * label.layout.lines.len() as f32;
            let x = pos.x - label.size().x * galley.job.halign.to_factor();
            let cell = Rect::from_min_size(egui::pos2(x, pos.y), egui::vec2(width, galley.size().y));
            paint(&self.with_clip_rect(cell.intersect(self.clip_rect())), egui::pos2(x, pos.y), &label, ink);
        }
    }
}

pub trait ShapedUi {
    /// Authored names/paths: never look them up in the chrome catalog.
    fn shaped_authored_label(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response;
    /// Chrome copy: explicitly opt into catalog translation before painting.
    fn shaped_label(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response;
}
impl ShapedUi for egui::Ui {
    fn shaped_label(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response {
        let mut job = text.into().into_layout_job(self.style(), egui::FontSelection::Default, egui::Align::Center);
        let translated = crate::i18n::translate(self.ctx(), &job.text).into_owned();
        std::sync::Arc::make_mut(&mut job).text = translated;
        self.shaped_authored_label(egui::WidgetText::LayoutJob(job))
    }
    fn shaped_authored_label(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response {
        let job = text.into().into_layout_job(self.style(), egui::FontSelection::Default, egui::Align::Center);
        let font = job.sections.first().map(|s| s.format.font_id.clone()).unwrap_or_else(crate::shell::tokens::small);
        let color = job
            .sections
            .first()
            .map(|s| s.format.color)
            .filter(|c| *c != Color32::PLACEHOLDER)
            .unwrap_or(crate::shell::tokens::TEXT);
        super::label(self, &job.text, font, color)
    }
}

pub enum ShapedInput {
    Label(super::Label),
    Galley(std::sync::Arc<egui::Galley>),
}
impl From<super::Label> for ShapedInput {
    fn from(label: super::Label) -> Self {
        Self::Label(label)
    }
}
impl From<std::sync::Arc<egui::Galley>> for ShapedInput {
    fn from(galley: std::sync::Arc<egui::Galley>) -> Self {
        Self::Galley(galley)
    }
}

pub trait ShapedResponse {
    fn shaped_hover_text(self, text: impl Into<egui::WidgetText>) -> Self;
    fn shaped_disabled_hover_text(self, text: impl Into<egui::WidgetText>) -> Self;
}
impl ShapedResponse for egui::Response {
    fn shaped_hover_text(self, text: impl Into<egui::WidgetText>) -> Self {
        self.on_hover_ui(|ui| {
            ui.shaped_label(text);
        })
    }
    fn shaped_disabled_hover_text(self, text: impl Into<egui::WidgetText>) -> Self {
        self.on_disabled_hover_ui(|ui| {
            ui.shaped_label(text);
        })
    }
}

/// A bounded name cell: logical-end elision and Arabic right alignment without moving its box.
pub fn cell(painter: &Painter, rect: Rect, text: &str, font: FontId, ink: Color32) {
    if let Some(label) = layout(painter.ctx(), text, &font, Some(rect.width()), true) {
        let x =
            if label.layout.lines.first().is_some_and(|l| l.rtl) { rect.right() - label.size().x } else { rect.left() };
        paint(
            &painter.with_clip_rect(rect.intersect(painter.clip_rect())),
            egui::pos2(x, rect.center().y - label.size().y / 2.0),
            &label,
            ink,
        );
    }
}
