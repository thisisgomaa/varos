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
    object_drag: Option<[f32; 2]>,
    last_click: Option<(f64, u32)>,
    selecting: bool,
    pending_copy: Option<String>,
    pub generation: u64,
    pub error: Option<String>,
    /// Integration (w2): the text preview renders from a per-frame `Editor` clone, whose canvas stroke
    /// cache would start empty every frame (`CanvasStrokeCache::clone` is fresh). Keep one cache here
    /// and lend it to the preview so main's cross-frame coverage cache survives documents with text.
    stroke_cache: varos_core::stroke::canvas::CanvasStrokeCache,
}
impl TextProduct {
    pub fn finish_ops(&mut self, ed: &mut Editor, ops: &mut Vec<crate::ui::ops::Op>) {
        *ops = ops
            .drain(..)
            .map(|op| match op {
                crate::ui::ops::Op::Field(inner) if matches!(inner.as_ref(), crate::ui::ops::Op::Text(_)) => *inner,
                op => op,
            })
            .collect();
        ops.retain(|op| {
            if let crate::ui::ops::Op::Text(text) = op {
                if let Some(session) = self.session.as_mut().filter(|s| s.draft.id == text.id) {
                    if let Err(error) = text.validate() {
                        self.error = Some(error);
                    } else {
                        session.draft = text.clone();
                        self.generation += 1;
                    }
                    return false;
                }
            }
            true
        });
        if ops.iter().any(|op| matches!(op, crate::ui::ops::Op::Tool(_))) {
            if let Err(error) = self.commit(ed) {
                self.error = Some(error);
                ops.retain(|op| !matches!(op, crate::ui::ops::Op::Tool(_)));
            }
        }
    }

    fn engine(&mut self) -> Result<&mut TextLayout, String> {
        if self.engine.is_none() {
            self.engine = Some(TextLayout::new(varos_text_layout::host_fonts::snapshot()?)?);
        }
        self.engine.as_mut().ok_or_else(|| "text engine unavailable".into())
    }
    pub fn selected_text(&self, ed: &Editor) -> Option<TextBox> {
        self.session
            .as_ref()
            .map(|s| s.draft.clone())
            .or_else(|| ed.doc.text_boxes.iter().find(|t| ed.objsel.contains(&t.id)).cloned())
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
            self.selected = Some(text.id);
            ed.try_execute(EditCommand::SetText { id: text.id, text })?;
        }
        if let Some(id) = self.selected {
            let tool = ed.tool;
            ed.try_execute(EditCommand::SelectPaths(vec![id]))?;
            ed.set_tool(tool);
        }
        self.session = None;
        self.generation += 1;
        Ok(())
    }
    fn hit(&mut self, ed: &Editor, p: [f32; 2], zoom: f32) -> Option<u32> {
        let mut order = Vec::new();
        let path_hit = ed.path_under(p);
        let mut stack: Vec<_> = ed.doc.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let n = ed.doc.node(id)?;
            stack.extend(n.children.iter().rev().copied());
            match n.kind {
                NodeKind::Text(id) => order.push((id, Some(n.id))),
                NodeKind::Path(id) if path_hit == Some(id) => order.push((id, Some(n.id))),
                _ => {}
            }
        }
        if self.session.as_ref().is_some_and(|s| s.draft.id == 0) {
            order.insert(0, (0, None));
        }
        for (id, node) in order {
            let mut ancestor = node;
            let mut hidden = false;
            while let Some(id) = ancestor {
                let node = ed.doc.node(id)?;
                hidden |= node.hidden || node.locked;
                ancestor = node.parent;
            }
            if hidden {
                continue;
            }
            if node.and_then(|n| ed.doc.node(n)).is_some_and(|n| matches!(n.kind, NodeKind::Path(_))) {
                return None;
            }
            let t = self
                .session
                .as_ref()
                .filter(|s| s.draft.id == id)
                .map(|s| &s.draft)
                .or_else(|| ed.doc.text_boxes.iter().find(|t| t.id == id))?
                .clone();
            let p = text_transform(ed, id).inverse_apply(p);
            let c = self.engine().ok()?.compose(&t, zoom).ok()?;
            let bounds = match t.box_kind {
                TextBoxKind::Area(r) => r,
                TextBoxKind::Point => {
                    let left = c.layout.carets.iter().map(|c| c.x).fold(0f32, f32::min);
                    let right = c.layout.carets.iter().map(|c| c.x).fold(0f32, f32::max);
                    let h = c.layout.lines.last().map_or(24., |l| l.baseline + l.descent);
                    [c.origin[0] + left, c.origin[1], (right - left).max(8.), h]
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
                self.object_drag.is_some()
                    || !over_panel
                        && (ed.tool == ToolKind::Text
                            || self.session.is_some()
                            || ed.tool == ToolKind::Object && self.hit(ed, view.s2w(screen), view.zoom).is_some())
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
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.draft.id != 0 && !ed.doc.text_boxes.iter().any(|t| t.id == s.draft.id))
        {
            self.session = None;
            self.selected = None;
        }
        if let Some(session) = self.session.as_mut() {
            if let Some(current) = ed.doc.text_boxes.iter().find(|t| t.id == session.draft.id) {
                if current != &session.base {
                    // Bridge edits are blocked during a draft; external undo/revert replaces it.
                    *session = EditSession::new(current.clone());
                    self.generation += 1;
                }
            }
        }
        let mut copied = None;
        for event in &input.events {
            if !matches!(ed.tool, ToolKind::Text | ToolKind::Object)
                && self.session.is_none()
                && !matches!(event, Event::Key { key: Key::T, .. })
            {
                continue;
            }
            if let Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers } = event {
                if !*pressed {
                    self.selecting = false;
                }
                let outside = !hole.is_some_and(|r| r.contains(*pos))
                    || ctx.layer_id_at(*pos).is_some_and(|l| l.order != egui::Order::Background);
                if outside && (*pressed || self.press.is_none() && self.object_drag.is_none()) {
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
                        self.selecting = double || ed.tool == ToolKind::Text || self.session.is_some();
                        if ed.tool == ToolKind::Object && !double && self.session.is_none() {
                            let mut ids: Vec<_> = if modifiers.shift || ed.objsel.contains(&id) {
                                ed.objsel.iter().copied().collect()
                            } else {
                                vec![]
                            };
                            if modifiers.shift && ids.contains(&id) {
                                ids.retain(|i| *i != id);
                            } else if !ids.contains(&id) {
                                ids.push(id);
                            }
                            ed.try_execute(EditCommand::SelectPaths(ids))?;
                            if ed.objsel.contains(&id) {
                                ed.try_execute(EditCommand::TransformBegin)?;
                                self.object_drag = Some(p);
                            }
                        }
                        if double || ed.tool == ToolKind::Text {
                            if self.session.as_ref().is_none_or(|s| s.draft.id != id) {
                                self.commit(ed)?;
                                self.session =
                                    ed.doc.text_boxes.iter().find(|t| t.id == id).cloned().map(EditSession::new);
                                self.selected = Some(id);
                            }
                            if let Some(draft) = self.session.as_ref().map(|s| s.draft.clone()) {
                                let p = text_transform(ed, draft.id).inverse_apply(p);
                                let c = self.engine()?.compose(&draft, view.zoom)?.clone();
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
                                if let Some(session) = &mut self.session {
                                    session.hit(&c.layout, line, p[0] - c.origin[0], modifiers.shift);
                                }
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
                } else if let Some(start) = self.object_drag.take() {
                    ed.try_execute(EditCommand::TransformLive(varos_core::select_transform::Transform {
                        movement: [p[0] - start[0], p[1] - start[1]],
                        ..Default::default()
                    }))?;
                    ed.try_execute(EditCommand::TransformCommit)?;
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
                if let Some(start) = self.object_drag {
                    let p = view.s2w([pos.x * ppp, pos.y * ppp]);
                    ed.try_execute(EditCommand::TransformLive(varos_core::select_transform::Transform {
                        movement: [p[0] - start[0], p[1] - start[1]],
                        ..Default::default()
                    }))?;
                }
                if self.selecting {
                    if let Some(draft) = self.session.as_ref().map(|s| s.draft.clone()) {
                        let p = text_transform(ed, draft.id).inverse_apply(view.s2w([pos.x * ppp, pos.y * ppp]));
                        let c = self.engine()?.compose(&draft, view.zoom)?.clone();
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
                        if let Some(session) = &mut self.session {
                            session.hit(&c.layout, line, p[0] - c.origin[0], true);
                        }
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
                    Event::Ime(egui::ImeEvent::Disabled) => session.preedit.clear(),
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
                        Key::ArrowUp | Key::ArrowDown => {
                            let layout = &self.engine()?.compose(&session.draft, view.zoom)?.layout;
                            session.vertical(layout, *key == Key::ArrowDown, modifiers.shift);
                        }
                        Key::Home | Key::End => {
                            let layout = &self.engine()?.compose(&session.draft, view.zoom)?.layout;
                            session.line_edge(layout, *key == Key::End, modifiers.shift);
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
        // Source identities remain selected in the editor; preview outlines carry that frame.
        for id in varos_core::text::selected_ids(ed) {
            preview.objsel.remove(&id);
            if let Some(node) = varos_core::text::node_id(&ed.doc, id) {
                preview.objsel.extend(preview.doc.node_paths(node));
            }
        }
        std::mem::swap(&mut preview.canvas_stroke_cache, &mut self.stroke_cache);
        let scene = varos_core::scene::build_scene_in_view_styled(&preview, view, frame, style);
        std::mem::swap(&mut preview.canvas_stroke_cache, &mut self.stroke_cache);
        scene
    }
    pub fn paint(&mut self, ctx: &egui::Context, ed: &Editor, view: View, ppp: f32) {
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
        let transform = text_transform(ed, display.id);
        let screen = |x: f32, y: f32| {
            let p = view.w2s(transform.apply([x + c.origin[0], y + c.origin[1]]));
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
        for [left, top, right, bottom] in c.layout.selection_rects(range) {
            painter.add(egui::Shape::convex_polygon(
                vec![screen(left, top), screen(right, top), screen(right, bottom), screen(left, bottom)],
                varos_app::shell::tokens::ACCENT_TINT,
                egui::Stroke::NONE,
            ));
        }
        let caret_byte =
            if session.preedit.is_empty() { session.caret } else { session.range().start + session.preedit.len() };
        let mut display_session = session.clone();
        display_session.caret = caret_byte;
        if let Some(caret) = display_session.current_caret(&c.layout) {
            if let Some(line) = c.layout.lines.get(caret.line) {
                let cursor_rect = egui::Rect::from_two_pos(
                    screen(caret.x, line.baseline - line.ascent),
                    screen(caret.x, line.baseline + line.descent),
                );
                if !ctx.egui_wants_keyboard_input() {
                    ctx.output_mut(|o| {
                        o.ime = Some(egui::output::IMEOutput {
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

// Match the existing outline pipeline's top-level unit rotation.
fn text_transform(ed: &Editor, id: u32) -> varos_core::model::Xform {
    let mut node = ed.doc.nodes.iter().find(|n| n.kind == NodeKind::Text(id));
    let mut transform = node.map(|n| n.xform).unwrap_or_default();
    while let Some(n) = node {
        if n.kind == NodeKind::Group {
            transform = n.xform;
        }
        node = n.parent.and_then(|id| ed.doc.node(id));
    }
    transform
}

#[cfg(test)]
mod tests {
    use super::*;
    fn left_text(source: &str, frame: [f32; 2]) -> Result<TextBox, String> {
        let mut text = varos_text_layout::default_text(source, frame)?;
        text.para.align = varos_core::text::Alignment::Left;
        Ok(text)
    }
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
    fn object_drag_selects_core_identity_and_moves_one_undo_step() {
        let mut ed = Editor::new();
        let id = ed
            .try_execute_created(EditCommand::AddText { text: left_text("ABC", [20., 50.]).unwrap(), parent: None })
            .unwrap();
        ed.set_tool(ToolKind::Object);
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        send(&mut tool, &mut ed, vec![pointer(25., 45., true)], 1.);
        assert!(ed.objsel.contains(&id));
        send(&mut tool, &mut ed, vec![Event::PointerMoved(egui::pos2(45., 65.)), pointer(45., 65., false)], 2.);
        assert_eq!(ed.doc.text_boxes[0].frame, [40., 70.]);
        assert_eq!(ed.rev, 2);
        ed.undo();
        assert_eq!(ed.doc.text_boxes[0].frame, [20., 50.]);
    }
    #[test]
    fn path_above_text_occludes_and_below_does_not() {
        let mut ed = Editor::new();
        let id = ed
            .try_execute_created(EditCommand::AddText { text: left_text("ABC", [20., 50.]).unwrap(), parent: None })
            .unwrap();
        let path = ed.doc.nid();
        let anchors = [[0., 0.], [100., 0.], [100., 100.], [0., 100.]]
            .into_iter()
            .map(|p| varos_core::model::Anchor { id: ed.doc.nid(), p, hin: None, hout: None, smooth: false })
            .collect();
        ed.doc.paths.push(varos_core::model::Path::new(path, anchors, true, Some([0., 0., 0., 1.]), None, 1.));
        ed.doc.sync_tree();
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        assert_eq!(tool.hit(&ed, [25., 45.], 1.), None);
        ed.doc.nodes.iter_mut().find(|n| n.id == ed.doc.active_layer).unwrap().children.reverse();
        assert_eq!(tool.hit(&ed, [25., 45.], 1.), Some(id));
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
            session: Some(EditSession::new(left_text("", [0., 0.]).unwrap())),
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
        let mut session = EditSession::new(left_text("سلام", [20., 50.]).unwrap());
        session.select_all();
        let mut tool =
            TextProduct { engine: Some(TextLayout::bundled().unwrap()), session: Some(session), ..Default::default() };
        let mut ed = Editor::new();
        send(&mut tool, &mut ed, vec![Event::Copy], 1.);
        let ctx = egui::Context::default();
        let output = ctx.run_ui(Default::default(), |root| tool.paint(root.ctx(), &ed, View::identity(), 1.));
        assert!(output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c,egui::OutputCommand::CopyText(text) if text=="سلام")));
        assert!(output.platform_output.ime.is_some());
    }
    #[test]
    fn draft_properties_commit_with_source_in_one_undo_step() {
        let mut tool = TextProduct {
            engine: Some(TextLayout::bundled().unwrap()),
            session: Some(EditSession::new(left_text("سلام", [20., 50.]).unwrap())),
            ..Default::default()
        };
        let mut ed = Editor::new();
        let mut text = tool.selected_text(&ed).unwrap();
        text.runs[0].style.size = 48.;
        text.para.kashida = varos_core::text::Kashida::Balanced;
        let mut ops = vec![crate::ui::ops::Op::Text(text)];
        tool.finish_ops(&mut ed, &mut ops);
        assert!(ops.is_empty());
        assert!(ed.doc.text_boxes.is_empty());
        send(&mut tool, &mut ed, vec![Event::Text("!".into())], 1.);
        tool.commit(&mut ed).unwrap();
        assert_eq!(ed.doc.text_boxes[0].source(), "سلام!");
        assert_eq!(ed.doc.text_boxes[0].runs[0].style.size, 48.);
        assert_eq!(ed.rev, 1);
        ed.undo();
        assert!(ed.doc.text_boxes.is_empty());
    }
    #[test]
    fn double_click_edit_escape_commit_and_undo() {
        let mut ed = Editor::new();
        let id = ed
            .try_execute_created(EditCommand::AddText {
                text: left_text("سلام", [20., 50.]).unwrap(), parent: None
            })
            .unwrap();
        ed.set_tool(ToolKind::Object);
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        send(&mut tool, &mut ed, vec![pointer(25., 45., true), pointer(25., 45., false)], 1.);
        assert_eq!(tool.selected, Some(id));
        assert!(tool.session.is_none());
        send(&mut tool, &mut ed, vec![pointer(25., 45., true), pointer(25., 45., false)], 1.2);
        assert!(tool.session.is_some());
        let key = |key, modifiers| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        send(
            &mut tool,
            &mut ed,
            vec![
                key(Key::A, egui::Modifiers::COMMAND),
                Event::Paste("مرحبا Varos".into()),
                key(Key::Escape, egui::Modifiers::NONE),
            ],
            2.,
        );
        assert!(tool.session.is_none());
        assert_eq!(ed.doc.text_boxes[0].source(), "مرحبا Varos");
        ed.undo();
        assert_eq!(ed.doc.text_boxes[0].source(), "سلام");
    }
    #[test]
    fn hit_uses_tree_order_and_unsaved_draft_without_stealing_pen_clicks() {
        let mut ed = Editor::new();
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        let a = ed
            .try_execute_created(EditCommand::AddText { text: left_text("ABC", [20., 50.]).unwrap(), parent: None })
            .unwrap();
        ed.try_execute_created(EditCommand::AddText { text: left_text("DEF", [20., 50.]).unwrap(), parent: None })
            .unwrap();
        ed.doc.nodes.iter_mut().find(|n| n.id == ed.doc.active_layer).unwrap().children.reverse();
        assert_eq!(tool.hit(&ed, [25., 45.], 1.), Some(a));
        ed.set_tool(ToolKind::Pen);
        send(&mut tool, &mut ed, vec![pointer(25., 45., true), pointer(25., 45., false)], 1.);
        assert!(tool.selected.is_none());
        ed.set_tool(ToolKind::Text);
        tool.session = Some(EditSession::new(left_text("draft", [20., 50.]).unwrap()));
        assert_eq!(tool.hit(&ed, [25., 45.], 1.), Some(0));
        send(&mut tool, &mut ed, vec![pointer(25., 45., true), pointer(25., 45., false)], 2.);
        assert_eq!(tool.session.as_ref().unwrap().draft.id, 0);
        assert_eq!(ed.doc.text_boxes.len(), 2);
    }
    #[test]
    fn layout_error_keeps_edit_session_and_ime_cancel_keeps_source() {
        let mut tool = TextProduct {
            engine: Some(TextLayout::bundled().unwrap()),
            session: Some(EditSession::new(left_text("safe", [20., 50.]).unwrap())),
            selecting: true,
            ..Default::default()
        };
        tool.session.as_mut().unwrap().draft.runs[0].style.font.hash = "0".repeat(64);
        let mut ed = Editor::new();
        tool.input(
            &egui::Context::default(),
            &egui::RawInput { events: vec![Event::PointerMoved(egui::pos2(30., 40.))], ..Default::default() },
            &mut ed,
            View::identity(),
            1.,
            None,
        );
        assert!(tool.error.is_some());
        assert_eq!(tool.session.as_ref().unwrap().draft.source(), "safe");
        tool.error = None;
        send(
            &mut tool,
            &mut ed,
            vec![
                Event::Ime(egui::ImeEvent::Preedit { text: "سلام".into(), active_range_chars: None }),
                Event::Ime(egui::ImeEvent::Disabled),
            ],
            1.,
        );
        assert!(tool.session.as_ref().unwrap().preedit.is_empty());
        assert_eq!(tool.session.as_ref().unwrap().draft.source(), "safe");
    }
    #[test]
    fn rotated_hit_and_repeated_scene_reuse_layout_without_gpu() {
        let mut ed = Editor::new();
        let id = ed
            .try_execute_created(EditCommand::AddText { text: left_text("ABC", [20., 50.]).unwrap(), parent: None })
            .unwrap();
        let xf = varos_core::model::Xform { rot: std::f32::consts::FRAC_PI_2, piv: [0., 0.] };
        ed.doc.nodes.iter_mut().find(|n| n.kind == NodeKind::Text(id)).unwrap().xform = xf;
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        assert_eq!(tool.hit(&ed, xf.apply([25., 45.]), 1.), Some(id));
        let _ = tool.scene(
            &ed,
            View::identity(),
            [500, 500],
            SceneStyle {
                checkerboard: varos_app::shell::tokens::DOC_CHECKERBOARD,
                outline: varos_app::shell::tokens::OUTLINE_RGBA,
                canvas: varos_app::shell::tokens::CANVAS_RGBA,
            },
        );
        let layouts = tool.engine.as_ref().unwrap().layouts;
        for _ in 0..3 {
            let _ = tool.scene(
                &ed,
                View::identity(),
                [500, 500],
                SceneStyle {
                    checkerboard: varos_app::shell::tokens::DOC_CHECKERBOARD,
                    outline: varos_app::shell::tokens::OUTLINE_RGBA,
                    canvas: varos_app::shell::tokens::CANVAS_RGBA,
                },
            );
        }
        assert_eq!(tool.engine.as_ref().unwrap().layouts, layouts);
    }
    #[test]
    fn text_preview_keeps_canvas_stroke_cache_across_frames() {
        let mut ed = Editor::new();
        ed.try_execute_created(EditCommand::AddText { text: left_text("ABC", [20., 50.]).unwrap(), parent: None })
            .unwrap();
        ed.try_execute_created(EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Rect,
            bounds: [100., 100., 200., 200.],
            parent: None,
            fill: None,
            stroke: Some([0., 0., 0., 1.]),
            stroke_width: 2.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
        let path = ed.doc.paths.last_mut().unwrap();
        path.stroke = varos_core::model::Paint::Solid([0., 0., 0., 1.]);
        path.stroke_style.dash = vec![6., 3.];
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        let style = SceneStyle {
            checkerboard: varos_app::shell::tokens::DOC_CHECKERBOARD,
            outline: varos_app::shell::tokens::OUTLINE_RGBA,
            canvas: varos_app::shell::tokens::CANVAS_RGBA,
        };
        let _ = tool.scene(&ed, View::identity(), [500, 500], style);
        let cold = tool.stroke_cache.evaluations();
        assert!(cold >= 1);
        for _ in 0..3 {
            let _ = tool.scene(&ed, View::identity(), [500, 500], style);
        }
        assert_eq!(tool.stroke_cache.evaluations(), cold);
    }
    #[test]
    fn area_drag_released_outside_canvas_ends_gesture() {
        let mut ed = Editor::new();
        ed.set_tool(ToolKind::Text);
        let mut tool = TextProduct { engine: Some(TextLayout::bundled().unwrap()), ..Default::default() };
        send(&mut tool, &mut ed, vec![pointer(30., 50., true), pointer(530., 550., false)], 1.);
        assert!(tool.press.is_none());
        assert!(!tool.selecting);
        assert!(matches!(tool.session.as_ref().unwrap().draft.box_kind, TextBoxKind::Area(_)));
    }
}
