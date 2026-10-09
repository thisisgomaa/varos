//! Lane F: logical UTF-8 editor; visual navigation comes exclusively from varos-text.
use super::{layout, paint};
use crate::shell::tokens as t;
use egui::{Event, Id, Key, Rect, Response, Ui};
use unicode_segmentation::UnicodeSegmentation;
use varos_text::{Affinity, Caret, CaretMove, Layout};

#[derive(Clone, Default)]
pub struct State {
    anchor: usize,
    focus: usize,
    affinity: Option<Affinity>,
    preedit: String,
    undo: Vec<(String, usize, usize)>,
    redo: Vec<(String, usize, usize)>,
    scroll: egui::Vec2,
    desired_x: Option<f32>,
}
impl State {
    fn range(&self) -> std::ops::Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
    fn caret<'a>(&self, layout: &'a Layout) -> Option<&'a Caret> {
        layout
            .carets
            .iter()
            .find(|c| c.byte == self.focus && Some(c.affinity) == self.affinity)
            .or_else(|| layout.carets.iter().find(|c| c.byte == self.focus))
    }
    fn checkpoint(&mut self, text: &str) {
        self.undo.push((text.into(), self.anchor, self.focus));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn replace(&mut self, text: &mut String, value: &str) {
        let range = self.range();
        if text.get(range.clone()).is_none() {
            return;
        }
        self.checkpoint(text);
        text.replace_range(range.clone(), value);
        self.focus = range.start + value.len();
        self.anchor = self.focus;
        self.affinity = Some(Affinity::Upstream);
    }
    fn move_to(&mut self, c: &Caret, extend: bool) {
        self.focus = c.byte;
        self.affinity = Some(c.affinity);
        if !extend {
            self.anchor = self.focus;
        }
    }
    fn delete(&mut self, text: &mut String, backwards: bool) {
        if self.anchor == self.focus {
            let stops: Vec<_> = text.grapheme_indices(true).map(|(i, _)| i).chain([text.len()]).collect();
            if backwards {
                self.anchor = stops.into_iter().rev().find(|i| *i < self.focus).unwrap_or(self.focus);
            } else {
                self.focus = stops.into_iter().find(|i| *i > self.focus).unwrap_or(self.focus);
            }
        }
        if self.anchor != self.focus {
            self.replace(text, "");
        }
    }
    fn history(&mut self, text: &mut String, redo: bool) {
        let source = if redo { &mut self.redo } else { &mut self.undo };
        if let Some((old, anchor, focus)) = source.pop() {
            let current = (text.clone(), self.anchor, self.focus);
            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }
            *text = old;
            self.anchor = anchor;
            self.focus = focus;
            self.preedit.clear();
        }
    }
}
fn key(id: Id) -> Id {
    id.with("lane-f-text-editor")
}
pub fn select_all(ctx: &egui::Context, id: Id, text: &str) {
    ctx.data_mut(|d| d.insert_temp(key(id), State { anchor: 0, focus: text.len(), ..Default::default() }));
}
pub fn clear(ctx: &egui::Context, id: Id) {
    ctx.data_mut(|d| d.remove::<State>(key(id)));
}
pub fn show(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    text: &mut String,
    font: egui::FontId,
    hint: &str,
    multiline: bool,
) -> Response {
    let previous = text.clone();
    let mut state = ui.ctx().data(|d| d.get_temp::<State>(key(id))).unwrap_or_default();
    if !text.is_char_boundary(state.focus) || !text.is_char_boundary(state.anchor) {
        state = State::default();
    }
    let mut response = ui.interact(rect, id, egui::Sense::click_and_drag());
    let width = multiline.then_some(rect.width());
    let Some(mut shaped) = layout(ui.ctx(), text, &font, width, false) else {
        return response;
    };
    let rtl_offset = if !multiline && shaped.layout.lines.first().is_some_and(|l| l.rtl) {
        (rect.width() - shaped.size().x).max(0.0)
    } else {
        0.0
    };
    let mut origin = rect.min + egui::vec2(rtl_offset, 0.0) - state.scroll;
    let pointer_down =
        response.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
    if pointer_down || response.clicked() || response.dragged() {
        response.request_focus();
        if let Some(pos) = response.interact_pointer_pos() {
            let line = shaped
                .layout
                .lines
                .iter()
                .position(|l| pos.y - origin.y <= l.baseline + l.descent)
                .unwrap_or(shaped.layout.lines.len().saturating_sub(1));
            if let Some(c) = shaped.layout.hit(line, pos.x - origin.x).first() {
                state.move_to(c, (!pointer_down && response.dragged()) || ui.input(|i| i.modifiers.shift));
                if response.double_clicked() {
                    if let Some((start, word)) = text
                        .split_word_bound_indices()
                        .find(|(start, word)| *start <= c.byte && c.byte < *start + word.len())
                    {
                        state.anchor = start;
                        state.focus = start + word.len();
                    }
                }
            }
        }
    }
    if ui.input(|i| {
        i.events.iter().any(|e| matches!(e, Event::PointerButton {pos, pressed: true, ..} if !rect.contains(*pos)))
    }) {
        ui.memory_mut(|m| m.surrender_focus(id));
    }
    let focused = ui.memory(|m| m.has_focus(id));
    if focused {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() },
            )
        });
        let events = ui.input(|i| i.events.clone());
        let committed =
            events
                .iter()
                .find_map(|e| if let Event::Ime(egui::ImeEvent::Commit(s)) = e { Some(s.clone()) } else { None });
        for event in events {
            match event {
                Event::Text(s) if committed.as_ref() == Some(&s) => {}
                Event::Text(s) | Event::Paste(s) => {
                    if state.preedit.is_empty() {
                        state.replace(text, &clean_input(&s, multiline));
                    }
                }
                Event::Copy => {
                    if let Some(s) = text.get(state.range()) {
                        ui.ctx().copy_text(s.into());
                    }
                }
                Event::Cut => {
                    if let Some(s) = text.get(state.range()) {
                        ui.ctx().copy_text(s.into());
                    }
                    state.replace(text, "");
                }
                Event::Ime(egui::ImeEvent::Preedit { text: s, .. }) => state.preedit = s,
                Event::Ime(egui::ImeEvent::Commit(s)) => {
                    state.preedit.clear();
                    state.replace(text, &clean_input(&s, multiline));
                }
                Event::Key { key, pressed: true, modifiers, .. } => {
                    match key {
                        Key::A if modifiers.command => {
                            state.anchor = 0;
                            state.focus = text.len();
                        }
                        Key::Z if modifiers.command => state.history(text, modifiers.shift),
                        Key::Backspace if state.preedit.is_empty() => state.delete(text, true),
                        Key::Delete if state.preedit.is_empty() => state.delete(text, false),
                        Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End => {
                            state.desired_x = None;
                            let motion = match key {
                                Key::Home => CaretMove::Home,
                                Key::End => CaretMove::End,
                                Key::ArrowLeft if modifiers.command => CaretMove::Home,
                                Key::ArrowRight if modifiers.command => CaretMove::End,
                                Key::ArrowLeft if modifiers.alt => CaretMove::WordLeft,
                                Key::ArrowRight if modifiers.alt => CaretMove::WordRight,
                                Key::ArrowLeft => CaretMove::Left,
                                _ => CaretMove::Right,
                            };
                            if let Some(c) =
                                state.caret(&shaped.layout).and_then(|c| shaped.layout.caret_move(c, motion))
                            {
                                state.move_to(c, modifiers.shift);
                            }
                        }
                        Key::ArrowUp | Key::ArrowDown => {
                            if let Some(c) = state.caret(&shaped.layout).cloned() {
                                let desired = *state.desired_x.get_or_insert(c.x);
                                let line = if key == Key::ArrowUp { c.line.saturating_sub(1) } else { c.line + 1 };
                                if let Some(next) = shaped.layout.hit(line, desired).first() {
                                    state.move_to(next, modifiers.shift);
                                }
                            }
                        }
                        Key::Enter | Key::Escape => {
                            state.preedit.clear();
                            ui.memory_mut(|m| m.surrender_focus(id));
                        }
                        _ => continue,
                    }
                    ui.input_mut(|i| {
                        i.consume_key(modifiers, key);
                    });
                }
                _ => {}
            }
            if let Some(updated) = layout(ui.ctx(), text, &font, width, false) {
                shaped = updated;
            }
        }
        ui.input_mut(|i| {
            i.events
                .retain(|e| !matches!(e, Event::Text(_) | Event::Paste(_) | Event::Copy | Event::Cut | Event::Ime(_)))
        });
    } else {
        state.preedit.clear();
    }
    if focused {
        if let Some(c) = state.caret(&shaped.layout).cloned() {
            if c.x - state.scroll.x > rect.width() - t::KIT_STROKE {
                state.scroll.x = (c.x - rect.width() + t::FIELD_INSET_X).max(0.0);
            }
            if c.x < state.scroll.x {
                state.scroll.x = c.x;
            }
            if let Some(line) = shaped.layout.lines.get(c.line) {
                let top = line.baseline - line.ascent;
                let bottom = line.baseline + line.descent;
                state.scroll.y = state.scroll.y.max(bottom - rect.height()).min(top).max(0.0);
            }
        }
    }
    let rtl_offset = if !multiline && shaped.layout.lines.first().is_some_and(|l| l.rtl) {
        (rect.width() - shaped.size().x).max(0.0)
    } else {
        0.0
    };
    origin = rect.min + egui::vec2(rtl_offset, 0.0) - state.scroll;
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if focused {
        for [x0, y0, x1, y1] in shaped.layout.selection_rects(state.range()) {
            painter.rect_filled(
                Rect::from_min_max(origin + egui::vec2(x0, y0), origin + egui::vec2(x1, y1)),
                0,
                t::ACCENT,
            );
        }
    }
    if text.is_empty() && !focused {
        if let Some(h) = layout(ui.ctx(), &crate::i18n::translate(ui.ctx(), hint), &font, width, false) {
            paint(&painter, origin, &h, t::MUTED);
        }
    } else {
        paint(&painter, origin, &shaped, t::TEXT);
    }
    if focused {
        if let Some(c) = state.caret(&shaped.layout) {
            if let Some(line) = shaped.layout.lines.get(c.line) {
                let cursor = Rect::from_min_max(
                    origin + egui::vec2(c.x, line.baseline - line.ascent),
                    origin + egui::vec2(c.x + t::KIT_STROKE, line.baseline + line.descent),
                );
                painter.rect_filled(cursor, 0, t::TEXT);
                ui.output_mut(|o| {
                    o.ime =
                        Some(egui::output::IMEOutput { rect, cursor_rect: cursor, should_interrupt_composition: false })
                });
                if !state.preedit.is_empty() {
                    if let Some(pre) = layout(ui.ctx(), &state.preedit, &font, None, false) {
                        paint(&painter, cursor.min, &pre, t::TEXT);
                        painter.hline(
                            cursor.left()..=cursor.left() + pre.size().x,
                            cursor.bottom(),
                            egui::Stroke::new(t::KIT_STROKE, t::TEXT),
                        );
                    }
                }
            }
        }
    }
    if *text != previous {
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::text_edit(ui.is_enabled(), &previous, text.as_str(), hint));
    ui.ctx().data_mut(|d| d.insert_temp(key(id), state));
    response
}
fn clean_input(text: &str, multiline: bool) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    if multiline {
        text
    } else {
        text.replace('\n', " ")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arabic_selection_copy_paste_roundtrip() {
        let mut text = "لوحة — Café logo".to_owned();
        let original = text.clone();
        let mut s = State { anchor: 0, focus: "لوحة".len(), ..Default::default() };
        let copy = text[s.range()].to_owned();
        s.replace(&mut text, "");
        s.replace(&mut text, &copy);
        assert_eq!(text, original);
    }
    #[test]
    fn logical_grapheme_delete_and_undo() {
        let mut text = "سَّلَامُ".to_owned();
        let original = text.clone();
        let mut s = State { anchor: text.len(), focus: text.len(), ..Default::default() };
        s.delete(&mut text, true);
        assert_eq!(text, "سَّلَا");
        s.history(&mut text, false);
        assert_eq!(text, original);
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    fn frame(ctx: &egui::Context, text: &mut String, events: Vec<Event>) {
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 100.0))),
                events,
                ..Default::default()
            },
            |ui| {
                show(
                    ui,
                    Id::new("field"),
                    Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(200.0, 50.0)),
                    text,
                    t::small(),
                    "",
                    false,
                );
            },
        );
    }
    #[test]
    fn ime_preedit_never_commits_on_blur() {
        let ctx = egui::Context::default();
        let mut text = "لوحة".to_owned();
        frame(&ctx, &mut text, vec![]);
        ctx.memory_mut(|m| m.request_focus(Id::new("field")));
        frame(
            &ctx,
            &mut text,
            vec![Event::Ime(egui::ImeEvent::Preedit { text: "مؤقت".into(), active_range_chars: None })],
        );
        ctx.memory_mut(|m| m.surrender_focus(Id::new("field")));
        frame(&ctx, &mut text, vec![]);
        assert_eq!(text, "لوحة");
    }
    #[test]
    fn ime_commit_inserts_once_and_copy_is_logical() {
        let ctx = egui::Context::default();
        let mut text = String::new();
        frame(&ctx, &mut text, vec![]);
        ctx.memory_mut(|m| m.request_focus(Id::new("field")));
        frame(
            &ctx,
            &mut text,
            vec![
                Event::Ime(egui::ImeEvent::Preedit { text: "لوحة".into(), active_range_chars: None }),
                Event::Ime(egui::ImeEvent::Commit("لوحة".into())),
            ],
        );
        assert_eq!(text, "لوحة");
        select_all(&ctx, Id::new("field"), &text);
        frame(&ctx, &mut text, vec![Event::Paste("لوحة — Café logo".into())]);
        assert_eq!(text, "لوحة — Café logo");
    }
}

#[cfg(test)]
mod fix_tests {
    use super::*;
    fn run(ctx: &egui::Context, text: &mut String, events: Vec<Event>, multiline: bool) -> egui::FullOutput {
        ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))),
                ..Default::default()
            },
            |ui| {
                show(
                    ui,
                    Id::new("fix-field"),
                    Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(100.0, 54.0)),
                    text,
                    t::small(),
                    "",
                    multiline,
                );
            },
        )
    }
    #[test]
    fn drag_selection_starts_at_pointer_down_and_replaces_only_selection() {
        let ctx = egui::Context::default();
        let mut text = "abcdefghij".to_owned();
        run(&ctx, &mut text, vec![], false);
        let shaped = layout(&ctx, &text, &t::small(), None, false).unwrap();
        let pos = |byte| egui::pos2(10.0 + shaped.layout.carets.iter().find(|c| c.byte == byte).unwrap().x, 20.0);
        run(
            &ctx,
            &mut text,
            vec![
                Event::PointerMoved(pos(3)),
                Event::PointerButton {
                    pos: pos(3),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            false,
        );
        run(&ctx, &mut text, vec![Event::PointerMoved(pos(7))], false);
        run(
            &ctx,
            &mut text,
            vec![Event::PointerButton {
                pos: pos(7),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            false,
        );
        let out = run(&ctx, &mut text, vec![Event::Copy], false);
        assert!(out
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(s) if s == "defg")));
        run(&ctx, &mut text, vec![Event::Text("X".into())], false);
        assert_eq!(text, "abcXhij");
    }
    #[test]
    fn multiline_arabic_scroll_keeps_caret_and_ime_in_field() {
        let ctx = egui::Context::default();
        let mut text = String::new();
        run(&ctx, &mut text, vec![], true);
        ctx.memory_mut(|m| m.request_focus(Id::new("fix-field")));
        let out = run(&ctx, &mut text, vec![Event::Paste("وصف عربي طويل ".repeat(20))], true);
        let ime = out.platform_output.ime.unwrap();
        assert!(ime.rect.contains_rect(ime.cursor_rect), "{ime:?}");
        let state = ctx.data(|d| d.get_temp::<State>(key(Id::new("fix-field")))).unwrap();
        assert!(state.scroll.y > 0.0);
        ctx.data_mut(|d| {
            d.insert_temp(key(Id::new("fix-field")), State { scroll: state.scroll, ..Default::default() })
        });
        let out = run(&ctx, &mut text, vec![], true);
        let ime = out.platform_output.ime.unwrap();
        assert!(ime.rect.contains_rect(ime.cursor_rect));
        assert_eq!(ctx.data(|d| d.get_temp::<State>(key(Id::new("fix-field")))).unwrap().scroll.y, 0.0);
    }
}
