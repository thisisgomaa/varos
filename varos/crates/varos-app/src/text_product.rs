//! Lane G: provisional canvas Type tool; all published source changes use EditCommand.
use egui::{Event, Key};
use varos_core::{
    editor::Editor,
    geom::View,
    model::NodeKind,
    scene::{Scene, SceneStyle},
    text::{TextBox, TextBoxKind},
    EditCommand, ToolKind,
};
use varos_text_layout::{edit::EditSession, TextLayout};
#[derive(Default)]
pub struct TextProduct {
    engine: Option<TextLayout>,
    pub session: Option<EditSession>,
    pub selected: Option<u32>,
    press: Option<[f32; 2]>,
    last_click: Option<(f64, u32)>,
    selecting: bool,
    pending_copy: Option<String>,
    pub generation: u64,
    pub error: Option<String>,
}
impl TextProduct {
    fn engine(&mut self) -> Result<&mut TextLayout, String> {
        if self.engine.is_none() {
            self.engine = Some(TextLayout::new(varos_text_layout::host_fonts::snapshot()?)?);
        }
        self.engine.as_mut().ok_or_else(|| "text engine unavailable".into())
    }
    pub fn selected_text(&self, ed: &Editor) -> Option<TextBox> {
        self.selected.and_then(|id| ed.doc.text_boxes.iter().find(|t| t.id == id)).cloned()
    }
    pub fn commit(&mut self, ed: &mut Editor) -> Result<(), String> {
        let Some(session) = self.session.as_ref() else {
            return Ok(());
        };
        let text = session.draft.clone();
        if text.id == 0 {
            let id = ed.try_execute_created(EditCommand::AddText { text, parent: None })?;
            self.selected = Some(id);
        } else {
            ed.try_execute(EditCommand::SetText { id: text.id, text })?;
        }
        self.session = None;
        self.generation += 1;
        Ok(())
    }
    fn hit(&mut self, ed: &Editor, p: [f32; 2], zoom: f32) -> Option<u32> {
        for n in ed.doc.nodes.iter().rev() {
            let NodeKind::Text(id) = n.kind else {
                continue;
            };
            let mut ancestor = Some(n.id);
            let mut hidden = false;
            while let Some(id) = ancestor {
                let node = ed.doc.node(id)?;
                hidden |= node.hidden || node.locked;
                ancestor = node.parent;
            }
            if hidden {
                continue;
            }
            let t = ed.doc.text_boxes.iter().find(|t| t.id == id)?;
            let c = self.engine().ok()?.compose(t, zoom).ok()?;
            let bounds = match t.box_kind {
                TextBoxKind::Area(r) => r,
                TextBoxKind::Point => {
                    let w = c.layout.lines.iter().map(|l| l.width).fold(0f32, f32::max).max(8.);
                    let h = c.layout.lines.last().map_or(24., |l| l.baseline + l.descent);
                    [c.origin[0], c.origin[1], w, h]
                }
            };
            if p[0] >= bounds[0] && p[0] <= bounds[0] + bounds[2] && p[1] >= bounds[1] && p[1] <= bounds[1] + bounds[3]
            {
                return Some(id);
            }
        }
        None
    }
    pub fn owns_native(
        &mut self,
        event: &winit::event::WindowEvent,
        ed: &Editor,
        view: View,
        screen: [f32; 2],
        over_panel: bool,
        field_focus: bool,
    ) -> bool {
        use winit::{
            event::WindowEvent,
            keyboard::{KeyCode, PhysicalKey},
        };
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                !field_focus
                    && (self.session.is_some()
                        || event.physical_key == PhysicalKey::Code(KeyCode::KeyT) && !ed.mods.ctrl)
            }
            WindowEvent::Ime(_) => self.session.is_some(),
            WindowEvent::MouseInput { .. } => {
                !over_panel
                    && (ed.tool == ToolKind::Text
                        || self.session.is_some()
                        || self.hit(ed, view.s2w(screen), view.zoom).is_some())
            }
            _ => false,
        }
    }
    pub fn input(
        &mut self,
        ctx: &egui::Context,
        input: &egui::RawInput,
        ed: &mut Editor,
        view: View,
        ppp: f32,
        hole: Option<egui::Rect>,
    ) {
        let result = self.process(ctx, input, ed, view, ppp, hole);
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
    fn process(
        &mut self,
        ctx: &egui::Context,
        input: &egui::RawInput,
        ed: &mut Editor,
        view: View,
        ppp: f32,
        hole: Option<egui::Rect>,
    ) -> Result<(), String> {
        if let Some(session) = self.session.as_mut() {
            if let Some(current) = ed.doc.text_boxes.iter().find(|t| t.id == session.draft.id) {
                if current != &session.base {
                    session.draft.para = current.para.clone();
                    session.draft.box_kind = current.box_kind;
                    session.draft.frame = current.frame;
                    for run in &mut session.draft.runs {
                        if let Some(style) = current.runs.first() {
                            run.style = style.style.clone();
                        }
                    }
                    session.base = current.clone();
                    self.generation += 1;
                }
            }
        }
        let mut copied = None;
        for event in &input.events {
            if let Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers } = event {
                if !hole.is_some_and(|r| r.contains(*pos))
                    || ctx.layer_id_at(*pos).is_some_and(|l| l.order != egui::Order::Background)
                {
                    continue;
                }
                let p = view.s2w([pos.x * ppp, pos.y * ppp]);
                if *pressed {
                    let hit = self.hit(ed, p, view.zoom);
                    if let Some(id) = hit {
                        let double = self
                            .last_click
                            .is_some_and(|(time, last)| last == id && input.time.unwrap_or(0.) - time < 0.4);
                        self.last_click = Some((input.time.unwrap_or(0.), id));
                        self.selected = Some(id);
                        self.selecting = true;
                        if double || ed.tool == ToolKind::Text {
                            if self.session.as_ref().is_none_or(|s| s.draft.id != id) {
                                self.commit(ed)?;
                                self.session =
                                    ed.doc.text_boxes.iter().find(|t| t.id == id).cloned().map(EditSession::new);
                            }
                            if let Some(mut session) = self.session.take() {
                                let c = self.engine()?.compose(&session.draft, view.zoom)?;
                                let line = c
                                    .layout
                                    .lines
                                    .iter()
                                    .enumerate()
                                    .min_by(|(_, a), (_, b)| {
                                        (p[1] - c.origin[1] - a.baseline)
                                            .abs()
                                            .total_cmp(&(p[1] - c.origin[1] - b.baseline).abs())
                                    })
                                    .map_or(0, |(i, _)| i);
                                session.hit(&c.layout, line, p[0] - c.origin[0], modifiers.shift);
                                self.session = Some(session);
                            }
                        }
                    } else if ed.tool == ToolKind::Text {
                        self.commit(ed)?;
                        self.press = Some(p);
                        self.selected = None;
                    } else {
                        self.commit(ed)?;
                        self.selected = None;
                    }
                } else if let Some(start) = self.press.take() {
                    let mut text = varos_text_layout::default_text("", start)?;
                    if varos_core::geom::dist(start, p) * view.zoom > 4. {
                        text.box_kind = TextBoxKind::Area([
                            start[0].min(p[0]),
                            start[1].min(p[1]),
                            (p[0] - start[0]).abs().max(1.),
                            (p[1] - start[1]).abs().max(1.),
                        ]);
                    }
                    self.session = Some(EditSession::new(text));
                }
                self.generation += 1;
            }
            if let Event::PointerButton { pressed: false, .. } = event {
                self.selecting = false;
            }
            if let Event::PointerMoved(pos) = event {
                if self.selecting {
                    if let Some(mut session) = self.session.take() {
                        let p = view.s2w([pos.x * ppp, pos.y * ppp]);
                        let c = self.engine()?.compose(&session.draft, view.zoom)?;
                        let line = c
                            .layout
                            .lines
                            .iter()
                            .enumerate()
                            .min_by(|(_, a), (_, b)| {
                                (p[1] - c.origin[1] - a.baseline)
                                    .abs()
                                    .total_cmp(&(p[1] - c.origin[1] - b.baseline).abs())
                            })
                            .map_or(0, |(i, _)| i);
                        session.hit(&c.layout, line, p[0] - c.origin[0], true);
                        self.session = Some(session);
                        self.generation += 1;
                    }
                }
            }
            if ctx.egui_wants_keyboard_input() {
                continue;
            }
            if matches!(event,Event::Key { key:Key::T,pressed:true,modifiers,.. } if !modifiers.command)
                && self.session.is_none()
            {
                ed.set_tool(ToolKind::Text);
                continue;
            }
            let Some(mut session) = self.session.take() else {
                continue;
            };
            let mut commit = false;
            let mut history = None;
            let result = (|| -> Result<(), String> {
                match event {
                    Event::Copy => copied = Some(session.copy()),
                    Event::Cut => {
                        copied = Some(session.copy());
                        session.insert("")?;
                    }
                    Event::Paste(text) | Event::Text(text) => {
                        if session.preedit.is_empty() {
                            session.insert(text)?;
                        }
                    }
                    Event::Ime(egui::ImeEvent::Preedit { text, .. }) => session.preedit = text.clone(),
                    Event::Ime(egui::ImeEvent::Commit(text)) => {
                        session.preedit.clear();
                        session.insert(text)?;
                    }
                    Event::Key { key, pressed: true, modifiers, .. } => match key {
                        Key::Escape => commit = true,
                        Key::Z if modifiers.command => history = Some(modifiers.shift),
                        Key::A if modifiers.command => session.select_all(),
                        Key::Enter => session.insert("\n")?,
                        Key::ArrowLeft | Key::ArrowRight => {
                            let layout = &self.engine()?.compose(&session.draft, view.zoom)?.layout;
                            session.arrow(layout, *key == Key::ArrowRight, modifiers.shift);
                        }
                        Key::Backspace | Key::Delete => {
                            let layout = &self.engine()?.compose(&session.draft, view.zoom)?.layout;
                            session.delete(layout, *key == Key::Backspace)?;
                        }
                        _ => {}
                    },
                    _ => {}
                }
                Ok(())
            })();
            self.session = Some(session);
            result?;
            if commit || history.is_some() {
                self.commit(ed)?;
            }
            if let Some(redo) = history {
                ed.try_execute(if redo { EditCommand::Redo } else { EditCommand::Undo })?;
                self.selected = None;
            }
            self.generation += 1;
        }
        if let Some(copy) = copied {
            self.pending_copy = Some(copy);
        }
        Ok(())
    }
    pub fn scene(&mut self, ed: &Editor, view: View, frame: [u32; 2], style: SceneStyle) -> Scene {
        if ed.doc.text_boxes.is_empty() && self.session.is_none() {
            return varos_core::scene::build_scene_in_view_styled(ed, view, frame, style);
        }
        let mut preview = ed.clone();
        if let Some(session) = &self.session {
            if session.draft.id == 0 {
                let _ = varos_core::text::add(&mut preview, session.display_draft(), None);
            } else if let Some(t) = preview.doc.text_boxes.iter_mut().find(|t| t.id == session.draft.id) {
                *t = session.display_draft();
            }
        }
        match self.engine().and_then(|e| e.outlined(&preview.doc, view.zoom)) {
            Ok(doc) => preview.doc = doc,
            Err(e) => {
                let mut scene = varos_core::scene::build_scene_in_view_styled(ed, view, frame, style);
                scene.errors.push(e);
                return scene;
            }
        }
        varos_core::scene::build_scene_in_view_styled(&preview, view, frame, style)
    }
    pub fn paint(&mut self, ctx: &egui::Context, view: View, ppp: f32) {
        if let Some(copy) = self.pending_copy.take() {
            ctx.copy_text(copy);
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let Ok(engine) = self.engine() else {
            return;
        };
        let display = session.display_draft();
        let Ok(c) = engine.compose(&display, view.zoom) else {
            return;
        };
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("text-caret")));
        let screen = |x: f32, y: f32| {
            let p = view.w2s([x + c.origin[0], y + c.origin[1]]);
            egui::pos2(p[0] / ppp, p[1] / ppp)
        };
        let range = session.range();
        if c.overset {
            painter.text(
                screen(0., c.layout.lines.first().map_or(0., |l| l.baseline)),
                egui::Align2::LEFT_BOTTOM,
                "Overset text",
                egui::FontId::proportional(varos_app::shell::tokens::TEXT_OVERSET_SIZE),
                varos_app::shell::tokens::ACCENT,
            );
        }
        for line in &c.layout.lines {
            for g in &line.glyphs {
                if g.cluster.start < range.end && g.cluster.end > range.start {
                    let rect = egui::Rect::from_two_pos(
                        screen(g.x, line.baseline - line.ascent),
                        screen(g.x + g.advance, line.baseline + line.descent),
                    );
                    painter.rect_filled(rect, 0., varos_app::shell::tokens::ACCENT_TINT);
                }
            }
        }
        let caret_byte =
            if session.preedit.is_empty() { session.caret } else { session.range().start + session.preedit.len() };
        for caret in c.layout.carets.iter().filter(|c| c.byte == caret_byte && c.affinity == session.affinity) {
            if let Some(line) = c.layout.lines.get(caret.line) {
                let cursor_rect = egui::Rect::from_two_pos(
                    screen(caret.x, line.baseline - line.ascent),
                    screen(caret.x, line.baseline + line.descent),
                );
                if !ctx.egui_wants_keyboard_input() {
                    ctx.output_mut(|o| {
                        o.ime = Some(egui::IMEOutput {
                            rect: cursor_rect,
                            cursor_rect,
                            should_interrupt_composition: false,
                        })
                    });
                }
                painter.line_segment(
                    [screen(caret.x, line.baseline - line.ascent), screen(caret.x, line.baseline + line.descent)],
                    egui::Stroke::new(varos_app::shell::tokens::TEXT_CARET_W, varos_app::shell::tokens::ACCENT),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(tool: &mut TextProduct, ed: &mut Editor, events: Vec<Event>, time: f64) {
        let input = egui::RawInput { events, time: Some(time), ..Default::default() };
        tool.input(
            &egui::Context::default(),
            &input,
            ed,
            View::identity(),
            1.,
            Some(egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(500., 500.))),
        );
        assert!(tool.error.is_none(), "{:?}", tool.error);
    }
    fn pointer(x: f32, y: f32, pressed: bool) -> Event {
        Event::PointerButton {
            pos: egui::pos2(x, y),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn point_area_and_edit_batch_without_window() {
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        let mut ed = Editor::new();
        ed.set_tool(ToolKind::Text);
        send(&mut tool, &mut ed, vec![pointer(20., 50., true), pointer(20., 50., false)], 1.);
        assert!(matches!(tool.session.as_ref().unwrap().draft.box_kind, TextBoxKind::Point));
        send(&mut tool, &mut ed, vec![Event::Text("مرحبا ".into()), Event::Paste("Varos".into())], 2.);
        assert!(ed.doc.text_boxes.is_empty());
        tool.commit(&mut ed).unwrap();
        assert_eq!(ed.doc.text_boxes[0].source(), "مرحبا Varos");
        ed.undo();
        assert!(ed.doc.text_boxes.is_empty());
        send(&mut tool, &mut ed, vec![pointer(30., 100., true), pointer(230., 250., false)], 3.);
        assert_eq!(tool.session.as_ref().unwrap().draft.box_kind, TextBoxKind::Area([30., 100., 200., 150.]));
    }
    #[test]
    fn preedit_is_preview_only_and_commits_once() {
        let mut tool = TextProduct {
            engine: Some(TextLayout::bundled().unwrap()),
            session: Some(EditSession::new(varos_text_layout::default_text("", [0., 0.]).unwrap())),
            ..Default::default()
        };
        let mut ed = Editor::new();
        send(
            &mut tool,
            &mut ed,
            vec![Event::Ime(egui::ImeEvent::Preedit { text: "سلام".into(), active_range_chars: None })],
            1.,
        );
        let session = tool.session.as_ref().unwrap();
        assert_eq!(session.draft.source(), "");
        assert_eq!(session.display_draft().source(), "سلام");
        send(&mut tool, &mut ed, vec![Event::Ime(egui::ImeEvent::Commit("سلام".into()))], 2.);
        tool.commit(&mut ed).unwrap();
        assert_eq!(ed.doc.text_boxes[0].source(), "سلام");
        assert_eq!(ed.rev, 1);
    }
    #[test]
    fn clipboard_and_ime_are_published_inside_the_egui_pass() {
        let mut session = EditSession::new(varos_text_layout::default_text("سلام", [20., 50.]).unwrap());
        session.select_all();
        let mut tool =
            TextProduct { engine: Some(TextLayout::bundled().unwrap()), session: Some(session), ..Default::default() };
        let mut ed = Editor::new();
        send(&mut tool, &mut ed, vec![Event::Copy], 1.);
        let ctx = egui::Context::default();
        let output = ctx.run_ui(Default::default(), |root| tool.paint(root.ctx(), View::identity(), 1.));
        assert!(output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c,egui::OutputCommand::CopyText(text) if text=="سلام")));
        assert!(output.platform_output.ime.is_some());
    }
}
