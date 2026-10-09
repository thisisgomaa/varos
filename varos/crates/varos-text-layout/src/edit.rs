//! Source-preserving edit batch. The host publishes one SetText/AddText on commit.
use varos_core::text::{Run, TextBox};
use varos_text::{Affinity, Layout};
#[derive(Clone)]
pub struct EditSession {
    pub draft: TextBox,
    pub base: TextBox,
    pub anchor: usize,
    pub caret: usize,
    pub affinity: Affinity,
    pub preedit: String,
}
impl EditSession {
    pub fn new(draft: TextBox) -> Self {
        let caret = draft.source().len();
        Self {
            base: draft.clone(),
            draft,
            anchor: caret,
            caret,
            affinity: Affinity::Downstream,
            preedit: String::new(),
        }
    }
    pub fn range(&self) -> std::ops::Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }
    pub fn copy(&self) -> String {
        self.draft.source().get(self.range()).unwrap_or("").into()
    }
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.draft.source().len();
    }
    pub fn insert(&mut self, value: &str) -> Result<(), String> {
        let range = self.range();
        let source = self.draft.source();
        if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
            return Err("invalid text selection".into());
        }
        let mut runs = Vec::new();
        let mut offset = 0;
        let mut inserted = false;
        for run in &self.draft.runs {
            let end = offset + run.text.len();
            if offset < range.start {
                let n = (range.start - offset).min(run.text.len());
                if n > 0 {
                    runs.push(Run { text: run.text[..n].into(), style: run.style.clone() });
                }
            }
            if !inserted && end >= range.start {
                let mut style = run.style.clone();
                if varos_core::text::contains_arabic(value) {
                    style.letter_spacing = 0.;
                }
                runs.push(Run { text: value.into(), style });
                inserted = true;
            }
            if end > range.end {
                let n = range.end.saturating_sub(offset);
                runs.push(Run { text: run.text[n..].into(), style: run.style.clone() });
            }
            offset = end;
        }
        let mut merged: Vec<Run> = Vec::new();
        for run in runs {
            if let Some(last) = merged.last_mut().filter(|last| last.style == run.style) {
                last.text.push_str(&run.text);
            } else {
                merged.push(run);
            }
        }
        let mut next = self.draft.clone();
        next.runs = merged;
        next.validate()?;
        self.draft = next;
        self.caret = range.start + value.len();
        self.anchor = self.caret;
        self.preedit.clear();
        Ok(())
    }
    pub fn hit(&mut self, layout: &Layout, line: usize, x: f32, extend: bool) {
        if let Some(c) = layout
            .hit(line, x)
            .into_iter()
            .find(|c| c.affinity == self.affinity)
            .or_else(|| layout.hit(line, x).first().copied())
        {
            self.caret = c.byte;
            self.affinity = c.affinity;
            if !extend {
                self.anchor = self.caret;
            }
        }
    }
    pub fn arrow(&mut self, layout: &Layout, right: bool, extend: bool) {
        let motion = if right { varos_text::CaretMove::Right } else { varos_text::CaretMove::Left };
        if let Some(next) = self.current_caret(layout).and_then(|c| layout.caret_move(c, motion)) {
            self.caret = next.byte;
            self.affinity = next.affinity;
            if !extend {
                self.anchor = self.caret;
            }
        }
    }
    pub fn vertical(&mut self, layout: &Layout, down: bool, extend: bool) {
        if let Some(c) = self.current_caret(layout) {
            let line =
                if down { (c.line + 1).min(layout.lines.len().saturating_sub(1)) } else { c.line.saturating_sub(1) };
            self.hit(layout, line, c.x, extend);
        }
    }
    pub fn line_edge(&mut self, layout: &Layout, end: bool, extend: bool) {
        let motion = if end { varos_text::CaretMove::End } else { varos_text::CaretMove::Home };
        if let Some(next) = self.current_caret(layout).and_then(|c| layout.caret_move(c, motion)) {
            self.caret = next.byte;
            self.affinity = next.affinity;
            if !extend {
                self.anchor = self.caret;
            }
        }
    }
    pub fn current_caret<'a>(&self, layout: &'a Layout) -> Option<&'a varos_text::Caret> {
        layout
            .carets
            .iter()
            .find(|c| c.byte == self.caret && c.affinity == self.affinity)
            .or_else(|| layout.carets.iter().find(|c| c.byte == self.caret))
    }
    pub fn delete(&mut self, layout: &Layout, backward: bool) -> Result<(), String> {
        if self.range().is_empty() {
            let next = if backward {
                layout.carets.iter().map(|c| c.byte).filter(|b| *b < self.caret).max()
            } else {
                layout.carets.iter().map(|c| c.byte).filter(|b| *b > self.caret).min()
            };
            if let Some(next) = next {
                self.anchor = next;
            } else {
                return Ok(());
            }
        }
        self.insert("")
    }
}

impl EditSession {
    pub fn display_draft(&self) -> TextBox {
        if self.preedit.is_empty() {
            return self.draft.clone();
        }
        let mut preview = self.clone();
        let preedit = preview.preedit.clone();
        if preview.insert(&preedit).is_ok() {
            preview.draft
        } else {
            self.draft.clone()
        }
    }
}
